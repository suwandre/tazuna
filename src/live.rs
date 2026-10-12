//! Drives a real `claude` process (stream-json in and out) with the cc-mid profile, and records every event.
//!
//! One process serves the whole conversation, so follow-ups keep the prompt cache warm. Stopping a run sends the
//! interrupt control request the official Agent SDK uses; if claude does not answer, the process is killed and the
//! next message resumes the session with `--resume`.

use crate::{diag, review, route::{Lane, Profile}, runlog::Run, verify};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    fs::File,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{Receiver, Sender, channel},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const MID_PROMPT: &str = "You are a coding agent working in {cwd}. Today is {date}.
- Read the relevant code before editing. Use Grep/Glob to search.
- Fix root causes with the smallest change that follows the existing code style.
- Add or update tests when behavior changes. Run the relevant tests and fix failures before finishing.
- Never run destructive commands (rm -rf outside the working directory, git reset --hard, force push). Do not commit.
- Finish with a short summary of what changed and how you verified it.";

/// repo-lean only: with Claude Code's own tools the short prompt read ~25% less code and ran the type check less than
/// Claude Code's own prompt did (company tickets, 2026-10-07), and the judge marked down guessed names.
const CHECK_RULES: &str = "\n- Before changing code, read the code you change, its callers, and the definition of every name you rely on (database columns, fields, functions, APIs). Never assume one exists.\n- Before finishing, run the repo's type check and the relevant tests.";

/// `--self-review`: appended to the system prompt, against ending early and against unchecked assumptions.
const FINISH_RULES: &str = "Finish only when the task is fully done: every requirement and acceptance criterion in the request is met and checked. Do not stop to ask for confirmation or leave steps for the user. Before relying on a name you have not read (a database column, function, field, API), read where it is defined.";

/// `--explorer`: reading code is most of a run's cost (each turn re-reads everything read so far at the main model's
/// rate), so a cheaper subagent reads and answers in short; the main model reads a file only right before editing it.
const EXPLORER_RULE: &str = "Do not explore the code yourself: no Read, cat, sed, head, tail, grep, rg or find to look around. Send every question about the code to the explorer subagent (Task tool, subagent_type explorer); it reads and searches the repo and answers with file paths, line ranges and the lines that matter. Ask several questions in parallel when they are independent. Read a file yourself only right before you edit it, and only the part you edit.";

/// `--tester`: test and type-check output is long and stays in the main model's context for every later turn; a cheaper
/// subagent runs the checks and reports only what failed.
const TESTER_RULE: &str = "Do not run tests, type checks, builds or linters yourself. Ask the tester subagent (Task tool, subagent_type tester) to run them, naming the command or the files to check; it reports only the failures, with the lines that explain them. Run independent checks in parallel.";

/// `--boss`: the main model only directs. Reading, editing and running commands are blocked for it by a hook
/// (`tazuna boss-hook`); subagents on a cheaper model do that work.
const BOSS_RULE: &str = "You are the lead engineer. You do not touch the repository yourself: reading, searching, editing and running commands are blocked for you. Three subagents on a cheaper model do that work (Task tool):
- explorer: answers precise questions about the code with file paths, line numbers and the lines that matter.
- implementer: makes the code changes you specify. Give it exact instructions: files, functions, what to change and why, edge cases, tests to add or update.
- tester: runs the tests, type checks or builds you name and reports only the failures.
Work like this: ask the explorer until you understand the task and the code (independent questions in parallel); decide the change and hand it to the implementer with precise instructions (parallel implementers only for parts in different files); have the tester run the relevant checks; review the reports and delegate fixes until the task is done. Keep your own messages short.";

/// `tazuna boss-hook`: denies the main agent every tool but delegation (subagent calls carry an `agent_id`).
pub fn boss_hook() -> Result<()> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let v: Value = serde_json::from_str(&input).unwrap_or(Value::Null);
    if let Some(reason) = boss_denies(&v) {
        println!("{}", json!({"hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": "deny", "permissionDecisionReason": reason}}));
    }
    Ok(())
}

fn boss_denies(v: &Value) -> Option<&'static str> {
    const LEAD: [&str; 8] = ["Agent", "Task", "TaskOutput", "TaskStop", "TodoWrite", "ToolSearch", "ScheduleWakeup", "ExitPlanMode"];
    let tool = v["tool_name"].as_str().unwrap_or_default();
    (v.get("agent_id").is_none() && !LEAD.contains(&tool))
        .then_some("You are the lead: delegate this. Ask the explorer to read or search, the implementer to edit, the tester to run checks.")
}

/// The `--agents` JSON for the subagents that are on (`implementer` only in `--boss`).
fn subagents(explorer: Option<&str>, tester: Option<&str>, implementer: Option<&str>) -> String {
    let mut agents = serde_json::Map::new();
    if let Some(model) = implementer {
        agents.insert("implementer".into(), json!({
            "description": "Makes code changes exactly as the lead engineer specifies and reports what it did.",
            "prompt": "You implement changes for a lead engineer. Follow the instructions exactly; read the code you change first. If an instruction is wrong in a concrete detail (a name, a path, a signature), adapt minimally and say so. Run any quick check it names. Report briefly: files changed, what you did, anything you could not do or are unsure about.",
            "tools": ["Read", "Grep", "Glob", "Bash", "Edit", "Write"],
            "model": model,
        }));
    }
    if let Some(model) = explorer {
        agents.insert("explorer".into(), json!({
            "description": "Finds and reads code to answer a precise question about the repo; returns file paths, line ranges and the facts that matter, not whole files.",
            "prompt": "You explore a codebase for another agent. Answer its question with file paths, line numbers and short quotes of the lines that matter: complete, but brief. Do not edit anything.",
            "tools": ["Read", "Grep", "Glob", "Bash"],
            "model": model,
        }));
    }
    if let Some(model) = tester {
        agents.insert("tester".into(), json!({
            "description": "Runs tests, type checks, builds or linters and reports only the failures.",
            "prompt": "You run checks for another agent: the tests, type checks, builds or linters it names, using the repo's own commands. Report the command you ran, the pass and fail counts, and for each failure the test or file, the line, and the few error lines that explain it. Never paste full output. Do not edit anything.",
            "tools": ["Bash", "Read", "Grep", "Glob"],
            "model": model,
        }));
    }
    serde_json::Value::Object(agents).to_string()
}

/// `--self-review`: sent once when claude first says it is done and the run changed files.
const REVIEW: &str = "Before you finish, review your own change:
1. Run `git diff` and read all of it.
2. Go through every requirement and acceptance criterion in the task. For each, name the code that meets it, or fix the gap.
3. Revert changes the task did not ask for.
4. Check names and assumptions you have not read in the code (columns, functions, fields, APIs): read them now.
5. If you changed anything, run the relevant tests again.
Then finish with a short summary of what changed.";

/// Images a message names (a pasted or dragged-in path, `@shot.png`) go along as image blocks; plain text otherwise.
fn user_content(text: &str, cwd: &Path) -> Value {
    const MAX_IMAGE: u64 = 5 * 1024 * 1024; // the API's limit per image
    let images: Vec<Value> = text
        .split_whitespace()
        .map(|w| w.trim_start_matches('@').trim_matches(|c| c == '\'' || c == '"'))
        .filter_map(|w| {
            let ext = w.rsplit('.').next()?.to_ascii_lowercase();
            let media = match ext.as_str() {
                "png" => "image/png",
                "jpg" | "jpeg" => "image/jpeg",
                "gif" => "image/gif",
                "webp" => "image/webp",
                _ => return None,
            };
            let path = if Path::new(w).is_absolute() { PathBuf::from(w) } else { cwd.join(w) };
            std::fs::metadata(&path).ok().filter(|m| m.is_file() && m.len() <= MAX_IMAGE)?;
            let data = std::fs::read(&path).ok()?;
            Some(json!({"type": "image", "source": {"type": "base64", "media_type": media, "data": base64(&data)}}))
        })
        .collect();
    if images.is_empty() {
        return json!(text);
    }
    let mut parts = images;
    parts.push(json!({"type": "text", "text": text}));
    Value::Array(parts)
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            out.push(if i <= c.len() { T[(n >> shift & 63) as usize] as char } else { '=' });
        }
    }
    out
}

/// What "always" covers: one exact command for shells (a later command can be anything), the whole tool otherwise.
fn always_key(req: &Value) -> String {
    let tool = req["request"]["tool_name"].as_str().unwrap_or_default();
    match always_scope(tool) {
        "this command" => format!("{tool}\u{0}{}", req["request"]["input"]["command"].as_str().unwrap_or_default()),
        _ => tool.to_string(),
    }
}

/// How far "always" reaches for a tool, as the question shows it.
fn always_scope(tool: &str) -> &'static str {
    if matches!(tool, "Bash" | "mcp__lean__bash" | "PowerShell" | "bash") { "this command" } else { "this tool" }
}

/// The run-log prompt of a run claude started on its own.
const BACKGROUND_PROMPT: &str = "(claude continued on its own after a background command finished)";

/// The first event of a model turn.
fn starts_turn(e: &Value) -> bool {
    (e["type"] == "stream_event" && e["event"]["type"] == "message_start") || e["type"] == "assistant"
}

/// A message the user sent, as a point /rewind can go back to.
#[derive(Clone)]
pub struct Checkpoint {
    /// The id tazuna gave the message (claude keeps it): what `rewind_files` takes.
    pub uuid: String,
    pub text: String,
    /// The last reply before the message: a conversation rewind resumes there (None: before the first message).
    pub before: Option<String>,
}

/// A random version-4 UUID (claude takes the message ids it is given, and they must be UUIDs).
fn new_uuid() -> String {
    let mut b = [0u8; 16];
    let _ = std::fs::File::open("/dev/urandom").and_then(|mut f| std::io::Read::read_exact(&mut f, &mut b));
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

/// A background task claude runs (a shell command, a Monitor, a subagent), for the ctrl-b view.
#[derive(Clone)]
pub struct BgTask {
    pub id: String,
    /// claude's task_type: local_bash, local_agent, monitor, ...
    pub kind: String,
    pub description: String,
    /// running, completed, failed, killed, ...
    pub status: String,
    pub started: Instant,
    pub ended: Option<Instant>,
    /// The end of the output (at most 8 KiB) the last `get_task_output` returned.
    pub output: String,
}

enum Msg {
    Line(String),
    /// stderr closed: the process is exiting.
    Exited(String),
    /// A verification finished.
    Verified(verify::Outcome),
    /// The independent reviewer finished.
    Reviewed(review::Review),
}

pub struct Live {
    pub model: String,
    pub effort: String,
    cwd: PathBuf,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    /// Messages carry the process generation, so output from a killed process is ignored.
    tx: Sender<(u64, Msg)>,
    rx: Receiver<(u64, Msg)>,
    session_id: Option<String>,
    /// From claude's init event: its built-in prompt changes between versions, which moves token counts.
    claude_version: Option<String>,
    generation: u64,
    /// When an interrupt was requested and its run has not ended yet.
    interrupting: Option<Instant>,
    interrupts_sent: u64,
    rec: File,
    pub rec_path: PathBuf,
    t0: Instant,
    /// Runs in flight, oldest first (one per message; claude answers them in order).
    runs: VecDeque<Run>,
    /// claude reports cost cumulatively per process; this is the last total seen.
    proc_cost: f64,
    runs_log: PathBuf,
    /// "live" or "headless", recorded in the run log.
    pub mode: &'static str,
    /// Prompt cache lifetime for claude (`--cache-ttl`); None picks by mode: 5m headless (turns come seconds apart and cache
    /// writes cost 1.25× instead of 2×: −10% per run, same judge), 1h live (a pause over 5 minutes would re-write it all).
    pub cache_ttl: Option<String>,
    /// Put a repo map in front of the session's first message (`--repo-map`); `mapped` once it went out.
    pub repo_map: bool,
    mapped: bool,
    /// Model of the explorer subagent (`--explorer sonnet`), repo profile only.
    pub explorer: Option<String>,
    /// Model of the tester subagent (`--tester haiku`), repo profile only.
    pub tester: Option<String>,
    /// `--boss MODEL`: the main model only directs; explorer, implementer and tester subagents on MODEL do the work.
    pub boss: Option<String>,
    /// `--advisor MODEL`: Claude Code's advisor tool; the main model can ask MODEL for advice when it is stuck.
    pub advisor: Option<String>,
    pub verify: verify::Mode,
    /// Passed to the tool server: shrink bash output above this many chars (0: off).
    pub bash_cap: usize,
    /// Type/compile errors after each edit (on by default, `--no-diagnostics`): the hook's state directory once started.
    pub diagnostics: bool,
    /// Finish rules in the prompt plus one self-review turn per run (`--self-review`).
    pub self_review: bool,
    /// Permission mode as claude names it: bypassPermissions (tazuna's "auto"), default ("ask"), acceptEdits ("edits"),
    /// plan. Every mode but auto routes claude's permission questions to tazuna (`--permission-prompt-tool stdio`).
    pub permission_mode: String,
    /// Questions waiting for the user's answer: claude's `can_use_tool` control requests. The UI shows the front one
    /// from here, so what is shown and what an answer applies to cannot drift apart.
    pub pending: VecDeque<Value>,
    /// Events produced outside `poll` (an interrupt answering questions, a dead process dropping them), handed out by
    /// the next `poll` so the screen hears about them.
    outbox: Vec<(f64, Value)>,
    /// What the user allowed for the rest of the session ("always"): `always_key` of each answer.
    allowed: std::collections::HashSet<String>,
    requests_sent: u64,
    /// Commands claude started that have not finished: claude wakes up again for background ones, so the session is
    /// not idle until they are done.
    tasks: std::collections::HashSet<String>,
    /// Every background task of this session, oldest first: the ctrl-b view.
    pub bg: Vec<BgTask>,
    /// The messages the user sent in this process's conversation, oldest first: what /rewind offers.
    pub checkpoints: Vec<Checkpoint>,
    /// The uuid of claude's latest top-level reply.
    last_assistant: Option<String>,
    /// A conversation rewind waiting for the file rewind's answer: where to resume (None inside: start fresh).
    pending_resume: Option<Option<String>>,
    /// The next spawn resumes the session at this message (`--resume-session-at`, in a fork).
    resume_at: Option<String>,
    /// When the last of `tasks` finished.
    tasks_done: Option<Instant>,
    /// Independent reviewer: model and effort (`--review-model`, `--review-effort`).
    pub review_model: Option<(String, String)>,
    /// Own loop instead of claude (`--provider`): base URL and the environment variable with the key.
    pub provider: Option<(String, String)>,
    /// The user's last message and whether the reviewer has seen it: wake-up runs get no review of their own.
    task: String,
    task_reviewed: bool,
    diag_dir: Option<PathBuf>,
    /// Add the repo's CLAUDE.md / AGENTS.md to the system prompt (lean profile; the repo profile lets Claude Code load them).
    pub project_instructions: bool,
    pub profile: Profile,
    /// Also load the user's personal ~/.claude settings (repo profile).
    pub user_settings: bool,
    /// (bytes included, source files) for the run log.
    instructions_used: (usize, Vec<String>),
    verifying: bool,
    /// Messages typed while tests run; sent once the run is over.
    held: VecDeque<(String, bool)>,
    /// claude's last result event and the last run-log line (headless mode reports them).
    pub last_result: Option<Value>,
    pub last_log: Option<Value>,
    /// Every run logged this session (headless sums them: claude may continue on its own after the first).
    pub logs: Vec<Value>,
    /// Router decision: lane name and why.
    lane: (String, String),
    /// Id of the last run written to the run log (what `/good` and `/bad` label).
    pub last_run: Option<String>,
    /// Model and effort for fix rounds after failed verification, and whether the session already switched.
    escalate: Option<(String, String)>,
    escalated: bool,
}

impl Live {
    pub fn new(model: &str, effort: &str) -> Result<Self> {
        let cwd = std::env::current_dir()?;
        let data = data_dir()?;
        let sessions = data.join("sessions");
        std::fs::create_dir_all(&sessions)?;
        let dir = cwd.file_name().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default();
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        // the pid keeps sessions started in the same second in same-named directories apart
        let rec_path = sessions.join(format!("{stamp}-{dir}-{}.jsonl", std::process::id()));
        let mut rec = File::create(&rec_path).with_context(|| format!("creating {}", rec_path.display()))?;
        // same header as recordings/record.py, so a live session replays like any recording
        writeln!(rec, "{}", json!({"t": 0, "prompt": "", "effort": effort, "model": model, "cwd": cwd}))?;
        let (tx, rx) = channel();
        Ok(Live {
            model: model.into(),
            effort: effort.into(),
            cwd,
            child: None,
            stdin: None,
            tx,
            rx,
            session_id: None,
            claude_version: None,
            generation: 0,
            interrupting: None,
            interrupts_sent: 0,
            rec,
            rec_path,
            t0: Instant::now(),
            runs: VecDeque::new(),
            proc_cost: 0.0,
            runs_log: data.join("runs.jsonl"),
            mode: "live",
            cache_ttl: None,
            repo_map: false,
            mapped: false,
            explorer: None,
            tester: None,
            boss: None,
            advisor: None,
            verify: verify::Mode::Auto,
            bash_cap: crate::mcp::DEFAULT_BASH_CAP,
            diagnostics: false,
            self_review: false,
            review_model: None,
            provider: None,
            task: String::new(),
            task_reviewed: false,
            tasks: Default::default(),
            bg: Vec::new(),
            checkpoints: Vec::new(),
            last_assistant: None,
            pending_resume: None,
            resume_at: None,
            permission_mode: "bypassPermissions".into(),
            pending: VecDeque::new(),
            outbox: Vec::new(),
            allowed: Default::default(),
            requests_sent: 0,
            tasks_done: None,
            diag_dir: None,
            project_instructions: true,
            profile: Profile::Lean,
            user_settings: false,
            instructions_used: (0, Vec::new()),
            verifying: false,
            held: VecDeque::new(),
            last_result: None,
            last_log: None,
            logs: Vec::new(),
            lane: ("manual".into(), String::new()),
            last_run: None,
            escalate: None,
            escalated: false,
        })
    }

    pub fn elapsed_ms(&self) -> f64 {
        self.t0.elapsed().as_secs_f64() * 1000.0
    }

    /// Continue an earlier claude session (`--continue`, `--resume`): the first message resumes it instead of starting fresh.
    pub fn resume(&mut self, session_id: &str) {
        self.session_id = Some(session_id.to_string());
    }

    /// Sends a message, starting (or resuming) the claude process if needed. Returns the event to show. Image paths in
    /// it stay text: only `send_typed` attaches files.
    pub fn send(&mut self, text: &str) -> Result<(f64, Value)> {
        self.send_message(text, false)
    }

    /// A message the user typed: image files it names are read and sent as images. Never for text that came from
    /// claude, a command or a reviewer, which could name any file on the machine and bypass the permission mode.
    pub fn send_typed(&mut self, text: &str) -> Result<(f64, Value)> {
        self.send_message(text, true)
    }

    fn send_message(&mut self, text: &str, attach: bool) -> Result<(f64, Value)> {
        if self.verifying {
            self.held.push_back((text.to_string(), attach));
        } else {
            self.start_run(text, attach)?;
        }
        Ok(self.record(json!({"type": "harness_user", "text": text})))
    }

    fn start_run(&mut self, text: &str, attach: bool) -> Result<()> {
        // ponytail: taken while claude starts reading; an edit that lands first is counted as known, never as new
        let types_before = verify::typecheck_command(&self.verify, &self.cwd).map(|cmd| {
            let cwd = self.cwd.clone();
            crate::runlog::Baseline::Pending(std::thread::spawn(move || verify::run_raw(&cwd, &cmd).1))
        });
        // the map orients the first message of a session; the run log keeps the user's own words
        let first = if self.repo_map && !self.mapped && self.session_id.is_none() {
            self.mapped = true;
            crate::repomap::build(&self.cwd, text).map(|m| format!("{m}\n---\n\n{text}"))
        } else {
            None
        };
        let sent = first.as_deref().unwrap_or(text);
        // a typed message is a point /rewind can go back to (claude keeps the id we give it)
        let uuid = (self.provider.is_none()).then(new_uuid);
        if let Some(u) = &uuid {
            self.checkpoints.push(Checkpoint { uuid: u.clone(), text: text.to_string(), before: self.last_assistant.clone() });
        }
        self.write_content(if attach { user_content(sent, &self.cwd) } else { json!(sent) }, false, uuid.as_deref())?;
        let mut run = Run::new(text, self.elapsed_ms(), &self.cwd);
        run.types_before = types_before;
        (self.task, self.task_reviewed) = (text.to_string(), false);
        self.runs.push_back(run);
        Ok(())
    }

    /// tazuna's own messages to claude (feedback, review): plain text, never attachments.
    fn write_user(&mut self, text: &str) -> Result<()> {
        self.write_content(json!(text), true, None)
    }

    /// `followup`: tazuna's own message about the current task, which the own loop counts against the task's time and
    /// dollar budget instead of starting a new one (claude never sees the field).
    fn write_content(&mut self, content: Value, followup: bool, uuid: Option<&str>) -> Result<()> {
        if self.child.is_none() {
            self.spawn()?;
        }
        let mut line = json!({"type": "user", "message": {"role": "user", "content": content}});
        if let Some(u) = uuid {
            line["uuid"] = json!(u);
        }
        if followup && self.provider.is_some() {
            line["tazuna_followup"] = json!(true);
        }
        let stdin = self.stdin.as_mut().context("claude stdin closed")?;
        writeln!(stdin, "{line}")?;
        stdin.flush()?;
        Ok(())
    }

    /// Applies the router's decision (before the first message) and returns the event that shows it.
    pub fn set_lane(&mut self, lane: Lane) -> (f64, Value) {
        (self.model, self.effort, self.escalate, self.profile) = (lane.model.clone(), lane.effort.clone(), lane.escalate.clone(), lane.profile);
        self.lane = (lane.name.to_string(), lane.reason.clone());
        self.record(json!({"type": "harness_route", "lane": lane.name, "model": lane.model, "effort": lane.effort, "reason": lane.reason,
            "profile": lane.profile.name(), "escalate": lane.escalate.map(|(m, e)| format!("{m} {e}"))}))
    }

    /// Switches model and effort for the rest of the session: ends the idle process; the next message resumes the session
    /// on the new model (one cache rebuild, since the prompt cache is per model).
    fn respawn(&mut self, model: String, effort: String) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        self.stdin = None;
        self.generation += 1;
        self.proc_cost = 0.0;
        (self.model, self.effort) = (model, effort);
    }

    /// `--escalate MODEL[:EFFORT]`: fix rounds after failed tests run on MODEL (with `--provider`: on claude).
    pub fn set_escalate(&mut self, model: String, effort: String) {
        self.escalate = Some((model, effort));
    }

    /// Nothing in flight: every run has ended and no message is waiting.
    pub fn idle(&self) -> bool {
        // after the last background command ends, claude takes a moment to start the turn it wakes up for
        let settled = self.tasks_done.is_none_or(|t| t.elapsed().as_secs() >= 5);
        self.runs.is_empty() && !self.verifying && self.held.is_empty() && self.tasks.is_empty() && settled
    }

    /// Asks claude to stop the current run; the process stays up and keeps its cache.
    /// The user's answer to the oldest permission question. `always`: allow this tool for the rest of the session.
    pub fn answer(&mut self, allow: bool, always: bool) -> Option<(f64, Value)> {
        let req = self.pending.pop_front()?;
        let tool = req["request"]["tool_name"].as_str().unwrap_or_default().to_string();
        if allow && always {
            self.allowed.insert(always_key(&req));
        }
        // an approved plan leaves plan mode: edits go ahead, commands still ask
        let leave_plan = (allow && tool == "ExitPlanMode").then(|| json!([{"type": "setMode", "mode": "acceptEdits", "destination": "session"}]));
        if leave_plan.is_some() {
            self.permission_mode = "acceptEdits".into();
        }
        self.respond(&req, allow, leave_plan);
        Some(self.record(json!({"type": "harness_permission_done", "tool": tool, "allow": allow, "always": always, "mode": self.permission_mode})))
    }

    fn respond(&mut self, req: &Value, allow: bool, permissions: Option<Value>) {
        let mut answer = if allow {
            json!({"behavior": "allow", "updatedInput": req["request"]["input"]})
        } else if self.mode == "headless" {
            json!({"behavior": "deny", "message": "No user is present in this run, so nobody can answer or approve this. Decide with your best judgment and continue."})
        } else {
            json!({"behavior": "deny", "message": "The user declined this. Ask what they want instead, or try another way."})
        };
        if let Some(p) = permissions {
            answer["updatedPermissions"] = p;
        }
        let line = json!({"type": "control_response", "response": {"subtype": "success", "request_id": req["request_id"], "response": answer}});
        if let Some(stdin) = self.stdin.as_mut() {
            let _ = writeln!(stdin, "{line}").and_then(|_| stdin.flush());
        }
    }

    /// Switches the permission mode (also mid-run). Before the process starts it only sets the starting mode.
    pub fn set_mode(&mut self, mode: &str) -> (f64, Value) {
        if self.provider.is_some() && mode == "plan" {
            return self.record(json!({"type": "harness_notice", "text": "the own loop (--provider) has no plan mode yet: auto, ask or edits", "error": true}));
        }
        self.permission_mode = mode.to_string();
        if let Some(stdin) = self.stdin.as_mut() {
            self.requests_sent += 1;
            let req = json!({"type": "control_request", "request_id": format!("mode-{}", self.requests_sent), "request": {"subtype": "set_permission_mode", "mode": mode}});
            let _ = writeln!(stdin, "{req}").and_then(|_| stdin.flush());
        }
        self.record(json!({"type": "harness_mode", "mode": mode}))
    }

    /// Rewinds to just before the user message `checkpoints[i]`: the file changes made since (`files`, claude's
    /// `rewind_files`) and/or the conversation from there on (`conversation`: claude resumes at the last reply before
    /// it, in a forked session). Only while idle and on claude; the own loop keeps no checkpoints.
    pub fn rewind(&mut self, i: usize, files: bool, conversation: bool) -> Result<(f64, Value)> {
        if self.provider.is_some() {
            bail!("rewind needs claude's checkpoints; the own loop (--provider) has none yet");
        }
        if !self.idle() {
            bail!("wait until the run has finished (or stop it with esc), then rewind");
        }
        let cp = self.checkpoints.get(i).cloned().context("no such message")?;
        if files {
            if self.child.is_none() {
                self.spawn()?;
            }
            let stdin = self.stdin.as_mut().context("claude stdin closed")?;
            // a dry run first: its answer names the files (the real rewind's answer does not)
            for (dry, tag) in [(true, "rewind-dry"), (false, "rewind")] {
                self.requests_sent += 1;
                let req = json!({"type": "control_request", "request_id": format!("{tag}-{}", self.requests_sent),
                    "request": {"subtype": "rewind_files", "user_message_id": cp.uuid, "dry_run": dry}});
                writeln!(stdin, "{req}")?;
                stdin.flush()?;
            }
        }
        if conversation {
            self.checkpoints.truncate(i);
            self.pending_resume = Some(cp.before.clone());
            if !files {
                self.resume_before();
            } // with files: once claude has answered the file rewind (its checkpoints live in that process)
        }
        let what = match (files, conversation) {
            (true, true) => "files and conversation",
            (true, false) => "files",
            _ => "conversation",
        };
        Ok(self.record(json!({"type": "harness_notice", "text": format!("── rewound {what} to before: {} ──", crate::session::shown(&cp.text, 60)), "error": false})))
    }

    /// Ends the process so the next message resumes the conversation at the reply `pending_resume` names (or starts a
    /// fresh one when the rewind went back before the first message).
    fn resume_before(&mut self) {
        let Some(before) = self.pending_resume.take() else { return };
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        self.stdin = None;
        self.generation += 1;
        match before {
            Some(at) => self.resume_at = Some(at),
            None => self.session_id = None,
        }
        self.last_assistant = None;
    }

    /// Asks claude for the latest output of shell and Monitor tasks still running, and once more right after one ends
    /// (subagents have no output to read). The answers arrive through `poll`.
    pub fn read_task_outputs(&mut self) {
        let ids: Vec<String> = self.bg.iter()
            .filter(|t| t.kind != "local_agent" && t.ended.map_or(true, |e| e.elapsed() < Duration::from_secs(3)))
            .map(|t| t.id.clone()).collect();
        let Some(stdin) = self.stdin.as_mut() else { return };
        for id in ids {
            self.requests_sent += 1;
            let req = json!({"type": "control_request", "request_id": format!("out-{}-{id}", self.requests_sent), "request": {"subtype": "get_task_output", "task_id": id}});
            let _ = writeln!(stdin, "{req}").and_then(|_| stdin.flush());
        }
    }

    /// Stops one background task (claude's `stop_task`); the run and other tasks go on.
    pub fn stop_task(&mut self, id: &str) -> Option<(f64, Value)> {
        let stdin = self.stdin.as_mut()?;
        self.requests_sent += 1;
        let req = json!({"type": "control_request", "request_id": format!("stop-{}", self.requests_sent), "request": {"subtype": "stop_task", "task_id": id}});
        writeln!(stdin, "{req}").and_then(|_| stdin.flush()).ok()?;
        Some(self.record(json!({"type": "harness_info", "text": format!("stopping background task {id}")})))
    }

    pub fn interrupt(&mut self) -> Option<(f64, Value)> {
        // a stop also answers open questions: no
        while let Some(e) = self.answer(false, false) {
            self.outbox.push(e);
        }
        if self.verifying {
            // claude is idle while tests run: nothing to interrupt there, the run ends when the tests return
            self.runs.front_mut()?.interrupted();
            return Some(self.record(json!({"type": "harness_verify_cancel"})));
        }
        let stdin = self.stdin.as_mut()?;
        self.interrupts_sent += 1;
        let req = json!({"type": "control_request", "request_id": format!("interrupt-{}", self.interrupts_sent), "request": {"subtype": "interrupt"}});
        if writeln!(stdin, "{req}").and_then(|_| stdin.flush()).is_err() {
            return Some(self.kill("stopped"));
        }
        self.interrupting = Some(Instant::now());
        if let Some(run) = self.runs.front_mut() {
            run.interrupted();
        }
        Some(self.record(json!({"type": "harness_interrupt"})))
    }

    /// Last resort: kill the process (the next message resumes the session).
    fn kill(&mut self, why: &str) -> (f64, Value) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.stdin = None;
        self.generation += 1;
        self.interrupting = None;
        self.end_runs();
        self.record(json!({"type": "harness_notice", "text": format!("{why} · send a message to continue"), "error": false}))
    }

    /// Events that arrived since the last call, timestamped and recorded.
    pub fn poll(&mut self) -> Vec<(f64, Value)> {
        let mut out = std::mem::take(&mut self.outbox);
        while let Ok((generation, msg)) = self.rx.try_recv() {
            if generation != self.generation {
                continue;
            }
            match msg {
                Msg::Line(l) => {
                    let Ok(e) = serde_json::from_str::<Value>(&l) else { continue };
                    if e["type"] == "system" && e["subtype"] == "init" {
                        self.session_id = e["session_id"].as_str().map(String::from);
                        self.claude_version = e["claude_code_version"].as_str().map(String::from);
                    }
                    if e["type"] == "control_request" && e["request"]["subtype"] == "can_use_tool" {
                        if self.allowed.contains(&always_key(&e)) {
                            self.respond(&e, true, None);
                        } else if self.mode == "headless" {
                            // nobody is there to answer: a question (AskUserQuestion, or any tool in ask mode) would wait forever
                            self.respond(&e, false, None);
                        } else {
                            let tool = e["request"]["tool_name"].as_str().unwrap_or_default().to_string();
                            out.push(self.record(json!({"type": "harness_permission", "tool": tool, "input": e["request"]["input"], "scope": always_scope(&tool)})));
                            self.pending.push_back(e);
                        }
                        continue;
                    }
                    if e["type"] == "assistant" && e["parent_tool_use_id"].is_null() {
                        if let Some(u) = e["uuid"].as_str() {
                            self.last_assistant = Some(u.to_string()); // where a conversation rewind resumes
                        }
                    }
                    // answers to /rewind's file requests
                    if e["type"] == "control_response" {
                        let rid = e["response"]["request_id"].as_str().unwrap_or_default();
                        let r = &e["response"]["response"];
                        if rid.starts_with("rewind-dry-") {
                            let files: Vec<String> = r["filesChanged"].as_array().into_iter().flatten()
                                .filter_map(|f| f.as_str()).map(|f| f.rsplit('/').next().unwrap_or(f).to_string()).collect();
                            let text = if r["canRewind"] == true && files.is_empty() {
                                "no file changes recorded since that message (claude checkpoints only its own edit tools; the lean profile edits through tazuna's)".to_string()
                            } else if r["canRewind"] == true {
                                format!("restoring {} file(s): {} (+{} −{})", files.len(), files.join(", "), r["insertions"].as_i64().unwrap_or(0), r["deletions"].as_i64().unwrap_or(0))
                            } else {
                                format!("files cannot be rewound: {}", r["error"].as_str().or(e["response"]["error"].as_str()).unwrap_or("no checkpoint"))
                            };
                            out.push(self.record(json!({"type": "harness_info", "text": text})));
                            continue;
                        }
                        if rid.starts_with("rewind-") {
                            let ok = e["response"]["subtype"] == "success" && r["canRewind"] == true;
                            let text = if ok { "files restored".to_string() } else { format!("file rewind failed: {}", r["error"].as_str().or(e["response"]["error"].as_str()).unwrap_or("unknown")) };
                            out.push(self.record(json!({"type": "harness_info", "text": text})));
                            self.resume_before();
                            continue;
                        }
                    }
                    // a background task's output, asked for by the ctrl-b view: no event of its own
                    if e["type"] == "control_response" {
                        let id = e["response"]["request_id"].as_str().and_then(|r| r.strip_prefix("out-")).and_then(|r| r.split_once('-')).map(|(_, id)| id);
                        if let Some(t) = id.and_then(|id| self.bg.iter_mut().find(|t| t.id == id)) {
                            t.output = e["response"]["response"]["output"].as_str().unwrap_or_default().to_string();
                            continue;
                        }
                    }
                    match (e["type"].as_str(), e["subtype"].as_str(), e["task_id"].as_str()) {
                        (Some("system"), Some("task_started"), Some(id)) => {
                            self.tasks.insert(id.to_string());
                            if !self.bg.iter().any(|t| t.id == id) {
                                self.bg.push(BgTask { id: id.to_string(), kind: e["task_type"].as_str().unwrap_or("task").to_string(),
                                    description: e["description"].as_str().unwrap_or_default().to_string(), status: "running".into(),
                                    started: Instant::now(), ended: None, output: String::new() });
                            }
                        }
                        (Some("system"), Some("task_updated"), Some(id)) => {
                            if let (Some(t), Some(st)) = (self.bg.iter_mut().find(|t| t.id == id), e["patch"]["status"].as_str()) {
                                t.status = st.to_string();
                                if st != "running" {
                                    t.ended.get_or_insert_with(Instant::now);
                                }
                            }
                        }
                        (Some("system"), Some("task_notification"), Some(id)) => {
                            if self.tasks.remove(id) && self.tasks.is_empty() {
                                self.tasks_done = Some(Instant::now());
                            }
                            if let Some(t) = self.bg.iter_mut().find(|t| t.id == id) {
                                t.status = e["status"].as_str().unwrap_or("ended").to_string();
                                t.ended.get_or_insert_with(Instant::now);
                            }
                        }
                        _ => {}
                    }
                    if self.runs.is_empty() && !self.verifying && starts_turn(&e) {
                        // claude woke up without a message from us (a background command it started has finished):
                        // its turns and cost get a run of their own instead of going unlogged
                        let run = Run::new(BACKGROUND_PROMPT, self.elapsed_ms(), &self.cwd);
                        self.runs.push_back(run);
                        self.tasks_done = None;
                    }
                    if let Some(run) = self.runs.front_mut() {
                        run.observe(&e);
                    }
                    let is_result = e["type"] == "result";
                    if is_result {
                        self.interrupting = None;
                        let total = e["total_cost_usd"].as_f64().unwrap_or(self.proc_cost);
                        if let Some(run) = self.runs.front_mut() {
                            run.add_cost(total - self.proc_cost);
                        }
                        self.proc_cost = total;
                        self.last_result = Some(e.clone());
                        // a background task that finished during this turn wakes claude after its result: settle from here
                        if self.tasks_done.is_some() {
                            self.tasks_done = Some(Instant::now());
                        }
                    }
                    out.push(self.record(e));
                    if is_result {
                        out.extend(self.after_result());
                    }
                }
                Msg::Verified(o) => out.extend(self.after_verify(o)),
                Msg::Reviewed(r) => out.extend(self.after_review(r)),
                Msg::Exited(stderr) => {
                    // the process and its commands are gone
                    self.tasks.clear();
                    for t in self.bg.iter_mut().filter(|t| t.ended.is_none()) {
                        (t.status, t.ended) = ("lost".into(), Some(Instant::now())); // the process that ran it is gone
                    }
                    let code = self.child.take().and_then(|mut c| c.wait().ok()).and_then(|st| st.code());
                    self.stdin = None;
                    self.end_runs();
                    let tail: Vec<&str> = stderr.lines().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
                    let text = format!("claude exited ({}){}", code.map_or("signal".into(), |c| format!("code {c}")),
                        if tail.is_empty() { String::new() } else { format!(": {}", tail.join(" · ")) });
                    out.push(self.record(json!({"type": "harness_notice", "text": text, "error": code != Some(0)})));
                }
            }
        }
        if self.interrupting.is_some_and(|t| t.elapsed().as_secs() >= 3) {
            out.push(self.kill("stopped (claude did not respond, process restarted)"));
        }
        out
    }

    /// claude says the front run is done: verify it, or end it.
    fn after_result(&mut self) -> Vec<(f64, Value)> {
        let success = self.last_result.as_ref().is_some_and(|r| r["subtype"] == "success" && r["is_error"] != true);
        // cascade: the cheap model on the own loop stopped with an error or changed nothing, so there is nothing to test
        if let (Some(_), Some((m, e)), false) = (&self.provider, self.escalate.clone(), self.escalated) {
            let gave_up = self.runs.front().is_some_and(|r| !r.is_interrupted() && (!success || r.changed_files(&self.cwd).is_empty()));
            if gave_up {
                self.escalated = true;
                if let Some(run) = self.runs.front_mut() {
                    run.escalated();
                }
                let mut out = vec![self.record(json!({"type": "harness_escalate", "model": m, "effort": e}))];
                self.respawn(m, e);
                (self.provider, self.session_id) = (None, None);
                let why = if success { "changed no files" } else { "stopped with an error" };
                let msg = format!("{}\n\n---\nA cheaper model tried this task first and {why}; any changes it made are in the working tree (`git diff`). Do the task.", self.task);
                if let Err(e) = self.write_user(&msg) {
                    out.push(self.record(json!({"type": "harness_notice", "text": format!("could not hand the task over: {e:#}"), "error": true})));
                    out.extend(self.finish_front());
                }
                return out;
            }
        }
        let Some(run) = self.runs.front_mut() else { return vec![] };
        if !success || run.is_interrupted() {
            return self.finish_front();
        }
        let changed = run.changed_files(&self.cwd);
        if self.self_review && !run.reviewed && !changed.is_empty() {
            // review before verification: its fixes get tested too
            run.reviewed = true;
            return match self.write_user(REVIEW) {
                Ok(()) => vec![self.record(json!({"type": "harness_review"}))],
                Err(e) => {
                    let mut out = vec![self.record(json!({"type": "harness_notice", "text": format!("could not send the self-review: {e:#}"), "error": true}))];
                    out.extend(self.finish_front());
                    out
                }
            };
        }
        if let Some((model, effort)) = self.review_model.clone().filter(|_| !self.task_reviewed && !changed.is_empty()) {
            // the reviewer gets the task and the diff in a fresh context; verification waits for its verdict
            self.task_reviewed = true;
            self.verifying = true;
            let (tx, g, cwd, task) = (self.tx.clone(), self.generation, self.cwd.clone(), self.task.clone());
            let (m, e) = (model.clone(), effort.clone());
            std::thread::spawn(move || {
                let _ = tx.send((g, Msg::Reviewed(review::run(&cwd, &task, &m, &e))));
            });
            return vec![self.record(json!({"type": "harness_review_start", "model": model, "effort": effort}))];
        }
        let Some(run) = self.runs.front_mut() else { return vec![] };
        let tests = verify::command(&self.verify, &self.cwd, &changed);
        // the baseline stays with the run, so fix rounds compare against the state before the run too
        let before = run.types_before.as_mut().and_then(|b| b.get());
        let types = verify::typecheck_command(&self.verify, &self.cwd).filter(|_| verify::touches_types(&changed)).zip(before);
        if tests.is_none() && types.is_none() {
            return self.finish_front();
        }
        self.verifying = true;
        let round = run.failed_rounds() + 1;
        let cmd = [tests.clone(), types.as_ref().map(|t| t.0.clone())].into_iter().flatten().collect::<Vec<_>>().join(" + ");
        let (tx, g, cwd) = (self.tx.clone(), self.generation, self.cwd.clone());
        std::thread::spawn(move || {
            let _ = tx.send((g, Msg::Verified(verify::check(&cwd, tests, types))));
        });
        vec![self.record(json!({"type": "harness_verify_start", "cmd": cmd, "round": round}))]
    }

    /// The reviewer is back: findings go to claude once; otherwise the run moves on to verification.
    fn after_review(&mut self, r: review::Review) -> Vec<(f64, Value)> {
        self.verifying = false;
        let Some(run) = self.runs.front_mut() else { return vec![] };
        run.add_cost(r.cost);
        run.review = Some(json!({"ok": r.ok, "issues": r.issues.len(), "cost_usd": (r.cost * 1e6).round() / 1e6, "secs": (r.secs * 10.0).round() / 10.0, "error": r.error}));
        let interrupted = run.is_interrupted();
        let mut out = vec![self.record(json!({"type": "harness_review_done", "ok": r.ok, "issues": r.issues, "cost": r.cost, "secs": r.secs, "error": r.error}))];
        if r.ok || interrupted {
            out.extend(self.after_result());
            return out;
        }
        match self.write_user(&review::feedback(&r)) {
            Ok(()) => {}
            Err(e) => {
                out.push(self.record(json!({"type": "harness_notice", "text": format!("could not send the review: {e:#}"), "error": true})));
                out.extend(self.finish_front());
            }
        }
        out
    }

    /// Tests are back: pass ends the run; a failure goes back to claude while rounds remain.
    fn after_verify(&mut self, o: verify::Outcome) -> Vec<(f64, Value)> {
        self.verifying = false;
        if o.no_tests {
            // the runner found nothing to run: not verified, not failed
            let mut out = vec![self.record(json!({"type": "harness_verify_done", "cmd": o.cmd, "ok": true, "no_tests": true, "secs": o.secs, "output": "", "final": true}))];
            out.extend(self.finish_front());
            return out;
        }
        let Some(run) = self.runs.front_mut() else { return vec![] };
        run.add_verify(&o.cmd, o.ok, o.secs);
        let retry = !o.ok && !run.is_interrupted() && run.failed_rounds() <= verify::MAX_ROUNDS;
        let mut out = vec![self.record(json!({
            "type": "harness_verify_done", "cmd": o.cmd, "ok": o.ok, "code": o.code, "secs": o.secs, "output": o.output, "final": !retry,
        }))];
        if retry {
            let round = self.runs.front().map_or(1, |r| r.failed_rounds());
            if let Some((m, e)) = self.escalate.clone().filter(|_| !self.escalated) {
                // the cheap lane failed its tests: fix rounds run on the stronger model
                self.escalated = true;
                if let Some(run) = self.runs.front_mut() {
                    run.escalated();
                }
                out.push(self.record(json!({"type": "harness_escalate", "model": m, "effort": e})));
                self.respawn(m, e);
            }
            // own loop -> claude: a fresh claude session, which needs the task and what the cheap model left behind
            let cross = self.escalated && self.provider.take().is_some();
            if cross {
                self.session_id = None;
            }
            let feedback = verify::feedback(&o, round);
            let msg = if cross {
                format!("{}\n\n---\nA cheaper model already worked on this task; its changes are in the working tree (`git diff` shows them). Its result does not pass yet. Continue from there: fix what is wrong, or redo it.\n\n{feedback}", self.task)
            } else {
                feedback
            };
            match self.write_user(&msg) {
                Ok(()) => out.push(self.record(json!({"type": "harness_verify_feedback", "round": round, "max": verify::MAX_ROUNDS}))),
                Err(e) => {
                    out.push(self.record(json!({"type": "harness_notice", "text": format!("could not send test failures: {e:#}"), "error": true})));
                    out.extend(self.finish_front());
                }
            }
        } else {
            out.extend(self.finish_front());
        }
        out
    }

    /// Logs the front run, then sends any message typed while it was busy.
    fn finish_front(&mut self) -> Vec<(f64, Value)> {
        if let Some(run) = self.runs.pop_front() {
            let line = run.finish(self.last_result.as_ref(), self.elapsed_ms(), &self.meta(), &self.cwd);
            self.log(line);
        }
        let mut out = Vec::new();
        while self.runs.is_empty() && !self.verifying {
            let Some((text, attach)) = self.held.pop_front() else { break };
            if let Err(e) = self.start_run(&text, attach) {
                out.push(self.record(json!({"type": "harness_notice", "text": format!("{e:#}"), "error": true})));
            }
        }
        out
    }

    /// The process is gone: runs without a result end here (stopped if the user asked, otherwise crashed).
    fn end_runs(&mut self) {
        // questions from a process that is gone can no longer be answered
        if !self.pending.is_empty() {
            self.pending.clear();
            let e = self.record(json!({"type": "harness_permission_cleared"}));
            self.outbox.push(e);
        }
        // ponytail: a dead process reports no cost, so these runs log $0; the session's estimate is in the recording
        while let Some(run) = self.runs.pop_front() {
            let line = run.finish(None, self.elapsed_ms(), &self.meta(), &self.cwd);
            self.log(line);
        }
        self.proc_cost = 0.0;
        self.verifying = false;
    }

    fn meta(&self) -> Value {
        json!({
            "model": self.model, "effort": self.effort, "profile": "cc-mid", "version": env!("CARGO_PKG_VERSION"), "mode": self.mode,
            "verify": match &self.verify { verify::Mode::Auto => "auto", verify::Mode::Command(_) => "command", verify::Mode::Off => "off" },
            "bash_cap": self.bash_cap, "diagnostics": self.diagnostics, "repo_map": self.repo_map, "explorer": self.explorer, "tester": self.tester, "cache_ttl": self.cache_ttl.clone().unwrap_or_else(|| if self.mode == "headless" { "5m" } else { "1h" }.into()), "self_review": self.self_review, "review_model": self.review_model.as_ref().map(|(m, e)| format!("{m} {e}")),
            "profile": self.profile.name(), "setting_sources": self.setting_sources(),
            "project_instructions": {"bytes": self.instructions_used.0, "files": self.instructions_used.1},
            "lane": self.lane.0, "lane_reason": self.lane.1, "escalated": self.escalated,
            "claude_version": self.claude_version, "cwd": self.cwd, "session_id": self.session_id, "recording": self.rec_path,
        })
    }

    fn log(&mut self, line: Value) {
        self.last_run = line["id"].as_str().map(String::from);
        self.last_log = Some(line.clone());
        self.logs.push(line.clone());
        // one write per line: parallel tazuna runs append to the same log without interleaving
        let appended = std::fs::OpenOptions::new().create(true).append(true).open(&self.runs_log).and_then(|mut f| f.write_all(format!("{line}\n").as_bytes()));
        if let Err(e) = appended {
            eprintln!("run log: {e}");
        }
    }

    fn record(&mut self, e: Value) -> (f64, Value) {
        let t = self.elapsed_ms();
        let _ = writeln!(self.rec, "{}", json!({"t": t as u64, "e": e}));
        (t, e)
    }

    /// Which settings files claude loads (recorded in the run log).
    fn setting_sources(&self) -> &'static str {
        match (self.profile, self.user_settings) {
            (Profile::Lean, _) => "",
            (Profile::Repo | Profile::RepoLean, false) => "project,local",
            (Profile::Repo | Profile::RepoLean, true) => "user,project,local",
        }
    }

    fn mid_prompt(&self) -> String {
        let date = Command::new("date").arg("+%Y-%m-%d").output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
        let mut p = MID_PROMPT.replace("{cwd}", &self.cwd.to_string_lossy()).replace("{date}", &date);
        if self.self_review {
            p.push_str("\n- ");
            p.push_str(FINISH_RULES);
        }
        p
    }

    fn spawn(&mut self) -> Result<()> {
        if let Some((url, key_env)) = self.provider.clone() {
            // the engine asks before commands (ask, edits) and edits (ask); it has no plan mode yet
            if self.permission_mode == "plan" {
                bail!("--provider has no plan mode yet (auto, ask or edits)");
            }
            // the own loop: lean prompt and tools, the repo's instructions in the prompt
            let mut prompt = self.mid_prompt();
            if self.project_instructions {
                let (text, bytes, sources) = project_instructions(&self.cwd, &["CLAUDE.md", ".claude/CLAUDE.md", "AGENTS.md"]);
                prompt.push_str(&text);
                self.instructions_used = (bytes, sources);
            }
            let mut cmd = Command::new(std::env::current_exe().context("locating the tazuna binary")?);
            cmd.args(["engine", "--url", &url, "--key-env", &key_env, "--model", &self.model, "--effort", &self.effort, "--system-prompt", &prompt, "--permission-mode", &self.permission_mode])
                .env("LEAN_BASH_CAP", self.bash_cap.to_string());
            return self.start(cmd);
        }
        let mut cmd = Command::new("claude");
        // the user's own setting wins; otherwise the mode decides
        if std::env::var_os("CLAUDE_CODE_PROMPT_CACHE_TTL").is_none() {
            let ttl = self.cache_ttl.clone().unwrap_or_else(|| if self.mode == "headless" { "5m" } else { "1h" }.into());
            cmd.env("CLAUDE_CODE_PROMPT_CACHE_TTL", ttl);
        }
        cmd.args(["-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose", "--include-partial-messages"])
            .args(["--model", &self.model, "--effort", &self.effort])
            // claude asks tazuna (stdio) instead of a terminal it does not have; "auto" never asks
            .args(["--permission-prompt-tool", "stdio", "--allow-dangerously-skip-permissions", "--permission-mode", &self.permission_mode])
            .args(["--setting-sources", self.setting_sources()]);
        if self.profile != Profile::Lean && !self.user_settings {
            // MCP servers from the repo's .mcp.json only: without this, the user's claude.ai connectors (they come with the
            // account, not a settings file) load too, ~40k tokens of tool schemas per turn in a personal setup
            cmd.arg("--strict-mcp-config");
            let mcp = self.cwd.join(".mcp.json");
            if std::fs::symlink_metadata(&mcp).is_ok_and(|m| m.file_type().is_file()) {
                cmd.arg("--mcp-config").arg(&mcp);
            }
        }
        match self.profile {
            Profile::Repo => {
                // Claude Code's own prompt and tools; it loads the repo's CLAUDE.md (nested files, @imports), settings,
                // hooks and skills itself. Per-machine prompt sections move to the first message for cross-session caching.
                cmd.arg("--exclude-dynamic-system-prompt-sections");
                let boss = self.boss.is_some();
                let append: Vec<&str> = [self.self_review.then_some(FINISH_RULES), boss.then_some(BOSS_RULE), self.explorer.as_ref().filter(|_| !boss).map(|_| EXPLORER_RULE), self.tester.as_ref().filter(|_| !boss).map(|_| TESTER_RULE)]
                    .into_iter()
                    .flatten()
                    .collect();
                if !append.is_empty() {
                    cmd.args(["--append-system-prompt", &append.join("\n")]);
                }
                if let Some(m) = self.boss.as_deref() {
                    cmd.args(["--agents", &subagents(Some(m), Some(m), Some(m))]);
                } else if self.explorer.is_some() || self.tester.is_some() {
                    cmd.args(["--agents", &subagents(self.explorer.as_deref(), self.tester.as_deref(), None)]);
                }
            }
            Profile::RepoLean => {
                // Claude Code loads CLAUDE.md (and its @imports) itself, but not AGENTS.md: that one goes in the prompt
                let mut prompt = self.mid_prompt();
                prompt.push_str(CHECK_RULES);
                let imported = ["CLAUDE.md", ".claude/CLAUDE.md"].iter().any(|f| std::fs::read_to_string(self.cwd.join(f)).is_ok_and(|t| t.contains("@AGENTS.md")));
                if self.project_instructions && !imported {
                    let (text, bytes, sources) = project_instructions(&self.cwd, &["AGENTS.md"]);
                    prompt.push_str(&text);
                    self.instructions_used = (bytes, sources);
                }
                cmd.args(["--system-prompt", &prompt, "--tools", "Bash,Read,Edit,Write,Grep,Glob,Skill"]);
            }
            Profile::Lean => {
                let mut prompt = self.mid_prompt();
                if self.project_instructions {
                    let (text, bytes, sources) = project_instructions(&self.cwd, &["CLAUDE.md", ".claude/CLAUDE.md", "AGENTS.md"]);
                    prompt.push_str(&text);
                    self.instructions_used = (bytes, sources);
                }
                // the lean tools are this binary's `mcp` mode
                let exe = std::env::current_exe().context("locating the tazuna binary")?;
                let mcp = json!({"mcpServers": {"lean": {"type": "stdio", "command": exe, "args": ["mcp"], "env": {"LEAN_BASH_CAP": self.bash_cap.to_string()}}}}).to_string();
                // cc-mid: lean prompt and tools, no skills or other MCP servers
                cmd.args(["--strict-mcp-config", "--disable-slash-commands"])
                    .args(["--system-prompt", &prompt, "--tools", "Grep,Glob", "--mcp-config", &mcp])
                    // pre-allowed only in auto: the other modes ask about the lean tools too
                    .args(if self.permission_mode == "bypassPermissions" { &["--allowedTools", "mcp__lean__bash,mcp__lean__read,mcp__lean__write,mcp__lean__edit"][..] } else { &[] });
            }
        }
        let mut settings = json!({});
        if self.diagnostics {
            if self.diag_dir.is_none() {
                self.diag_dir = Some(diag::prepare(&self.cwd)?);
            }
            let exe = std::env::current_exe().context("locating the tazuna binary")?;
            settings = serde_json::from_str(&diag::settings(&exe, self.diag_dir.as_ref().expect("set above")))?;
        }
        if self.boss.is_some() && self.profile == Profile::Repo {
            let exe = std::env::current_exe().context("locating the tazuna binary")?;
            let q = format!("'{}'", exe.to_string_lossy().replace('\'', r"'\''"));
            settings["hooks"]["PreToolUse"] = json!([{"matcher": "", "hooks": [{"type": "command", "command": format!("{q} boss-hook")}]}]);
        }
        if let Some(m) = &self.advisor {
            settings["advisorModel"] = json!(m);
        }
        if settings != json!({}) {
            cmd.args(["--settings", &settings.to_string()]);
        }
        if let Some(id) = &self.session_id {
            cmd.args(["--resume", id]);
            // after a conversation rewind: the session up to that reply, as a fork (the original stays as it was)
            if let Some(at) = self.resume_at.take() {
                cmd.args(["--resume-session-at", &at, "--fork-session"]);
            }
        }
        if self.mode != "headless" {
            cmd.env("CLAUDE_CODE_ENABLE_SDK_FILE_CHECKPOINTING", "1"); // /rewind's file checkpoints
        }
        self.start(cmd)
    }

    /// Starts the agent process and the threads that read its output.
    fn start(&mut self, mut cmd: Command) -> Result<()> {
        let mut child = cmd
            .current_dir(&self.cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("starting claude (is it installed and on PATH?)")?;
        let (stdout, mut stderr) = (child.stdout.take().context("stdout")?, child.stderr.take().context("stderr")?);
        let g = self.generation;
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send((g, Msg::Line(line))).is_err() {
                    return;
                }
            }
        });
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let mut err = String::new();
            let _ = stderr.read_to_string(&mut err);
            let _ = tx.send((g, Msg::Exited(err)));
        });
        self.stdin = child.stdin.take();
        self.child = Some(child);
        Ok(())
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        if let Some(d) = self.diag_dir.take() {
            let _ = std::fs::remove_dir_all(d);
        }
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

fn data_dir() -> Result<PathBuf> {
    let home = std::env::var_os("HOME").context("HOME not set")?;
    let d = Path::new(&home).join(".local/share/tazuna");
    std::fs::create_dir_all(&d)?;
    Ok(d)
}

/// More than this much instruction text is cut (it is re-sent, cached, on every turn).
const MAX_INSTRUCTIONS: usize = 80_000;

/// The repo's own instructions for agents: root CLAUDE.md, .claude/CLAUDE.md and AGENTS.md in full, nested ones listed so
/// the agent reads them when it works in that directory (like Claude Code, which loads nested files lazily).
/// Returns (text for the system prompt, bytes included, files included).
// ponytail: no `@file` import expansion; add it when a repo relies on imports
pub fn project_instructions(cwd: &Path, names: &[&str]) -> (String, usize, Vec<String>) {
    let mut body = String::new();
    let mut files = Vec::new();
    let root = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
    for &name in names {
        // regular files that really live inside the repo: a symlinked file or directory (e.g. `.claude` -> ~/.ssh) could
        // otherwise put any file on the machine into the prompt
        let path = cwd.join(name);
        let inside = path.canonicalize().is_ok_and(|real| real.starts_with(&root) && real == root.join(name));
        if !inside || !std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_file()) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        if text.trim().is_empty() || body.contains(text.trim()) {
            continue;
        }
        body.push_str(&format!("\n\n## From {name}\n\n{}", text.trim()));
        files.push(name.to_string());
    }
    let mut patterns: Vec<String> = names.iter().map(|n| format!("*{}", n.rsplit('/').next().unwrap_or(n))).collect();
    patterns.dedup();
    let nested: Vec<String> = Command::new("git")
        .args(["ls-files", "--"])
        .args(&patterns)
        .current_dir(cwd)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().filter(|l| l.contains('/') && !l.starts_with(".claude/")).take(30).map(String::from).collect())
        .unwrap_or_default();
    if files.is_empty() && nested.is_empty() {
        return (String::new(), 0, files);
    }
    if body.len() > MAX_INSTRUCTIONS {
        let cut = body.char_indices().map(|(i, _)| i).take_while(|&i| i <= MAX_INSTRUCTIONS).last().unwrap_or(0);
        body.truncate(cut);
        body.push_str("\n\n[tazuna: instructions cut here; read the rest of the file if you need it]");
    }
    let mut text = format!("\n\n# Project instructions\n\nThe repository's own instructions for coding agents. Follow them.{body}");
    if !nested.is_empty() {
        text.push_str(&format!("\n\nMore instruction files in this repo; read the one for a directory before you edit there: {}", nested.join(", ")));
    }
    let bytes = text.len();
    (text, bytes, files)
}

/// Live mode only runs where every change can be reviewed and undone.
pub fn in_git_repo() -> bool {
    Command::new("git").args(["rev-parse", "--is-inside-work-tree"]).output().is_ok_and(|o| o.status.success())
}

#[cfg(test)]
mod tests {

    #[test]
    fn message_ids_are_version_4_uuids() {
        let (a, b) = (super::new_uuid(), super::new_uuid());
        assert_ne!(a, b);
        let parts: Vec<&str> = a.split('-').collect();
        assert_eq!(parts.iter().map(|p| p.len()).collect::<Vec<_>>(), [8, 4, 4, 4, 12]);
        assert!(parts[2].starts_with('4') && "89ab".contains(&parts[3][..1]));
    }

    use super::*;

    #[test]
    fn attaches_named_images() {
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
        let dir = std::env::temp_dir().join(format!("tazuna-img-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("shot.png"), b"\x89PNG").unwrap();
        let c = user_content("what is wrong in @shot.png ?", &dir);
        assert_eq!(c[0]["type"], "image");
        assert_eq!(c[0]["source"]["data"], base64(b"\x89PNG"));
        assert_eq!(c[1]["text"], "what is wrong in @shot.png ?");
        assert_eq!(user_content("no image here.png", &dir), json!("no image here.png"), "a path that is not a file stays text");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn boss_hook_blocks_only_the_lead() {
        assert!(boss_denies(&json!({"tool_name": "Read"})).is_some());
        assert!(boss_denies(&json!({"tool_name": "Bash"})).is_some());
        assert!(boss_denies(&json!({"tool_name": "Agent"})).is_none());
        assert!(boss_denies(&json!({"tool_name": "Read", "agent_id": "a1", "agent_type": "explorer"})).is_none());
    }

    #[test]
    fn subagents_only_the_ones_on() {
        let both: Value = serde_json::from_str(&subagents(Some("haiku"), Some("sonnet"), None)).unwrap();
        assert_eq!((both["explorer"]["model"].as_str(), both["tester"]["model"].as_str()), (Some("haiku"), Some("sonnet")));
        let one: Value = serde_json::from_str(&subagents(None, Some("haiku"), None)).unwrap();
        assert!(one.get("explorer").is_none() && one["tester"]["tools"][0] == "Bash");
    }

    #[test]
    fn spots_turns_claude_starts_itself() {
        assert!(starts_turn(&json!({"type": "stream_event", "event": {"type": "message_start"}})));
        assert!(starts_turn(&json!({"type": "assistant", "message": {}})));
        assert!(!starts_turn(&json!({"type": "result"})));
        assert!(!starts_turn(&json!({"type": "system", "subtype": "init"})));
    }

    const ALL: &[&str] = &["CLAUDE.md", ".claude/CLAUDE.md", "AGENTS.md"];

    #[test]
    fn repo_lean_adds_only_agents_md() {
        let dir = std::env::temp_dir().join(format!("tazuna-agents-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("CLAUDE.md"), "claude rules").unwrap();
        std::fs::write(dir.join("AGENTS.md"), "agent rules").unwrap();
        let (text, _, files) = project_instructions(&dir, &["AGENTS.md"]);
        assert!(text.contains("agent rules") && !text.contains("claude rules"), "Claude Code loads CLAUDE.md itself");
        assert_eq!(files, vec!["AGENTS.md"]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn reads_project_instructions() {
        let dir = std::env::temp_dir().join(format!("tazuna-instr-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("app")).unwrap();
        let git = |args: &[&str]| assert!(Command::new("git").args(args).current_dir(&dir).output().unwrap().status.success());
        git(&["init", "-q"]);
        assert_eq!(project_instructions(&dir, ALL).1, 0, "no instruction files, nothing added");
        std::fs::write(dir.join("CLAUDE.md"), "Use npm run test:unit -- <file>.\n").unwrap();
        std::fs::write(dir.join("AGENTS.md"), "Use npm run test:unit -- <file>.\n").unwrap();
        std::fs::write(dir.join("app/CLAUDE.md"), "App rules.\n").unwrap();
        git(&["add", "."]);
        let (text, bytes, files) = project_instructions(&dir, ALL);
        assert_eq!(files, vec!["CLAUDE.md"], "identical AGENTS.md is not repeated");
        assert!(text.contains("test:unit") && text.contains("app/CLAUDE.md") && !text.contains("App rules"), "{text}");
        assert_eq!(bytes, text.len());
        std::fs::write(dir.join("CLAUDE.md"), "x".repeat(MAX_INSTRUCTIONS + 5000)).unwrap();
        assert!(project_instructions(&dir, ALL).0.contains("instructions cut here"));
        std::fs::remove_file(dir.join("CLAUDE.md")).unwrap();
        std::fs::remove_file(dir.join("AGENTS.md")).unwrap();
        std::fs::write(dir.join("secret.txt"), "TOP SECRET").unwrap();
        std::os::unix::fs::symlink(dir.join("secret.txt"), dir.join("CLAUDE.md")).unwrap();
        assert!(!project_instructions(&dir, ALL).0.contains("TOP SECRET"), "symlinked instruction files are never read");
        std::fs::remove_file(dir.join("CLAUDE.md")).unwrap();
        let outside = std::env::temp_dir().join(format!("tazuna-outside-{}", std::process::id()));
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("CLAUDE.md"), "OUTSIDE SECRET").unwrap();
        std::os::unix::fs::symlink(&outside, dir.join(".claude")).unwrap();
        assert!(!project_instructions(&dir, ALL).0.contains("OUTSIDE SECRET"), "a symlinked .claude directory is not followed");
        std::fs::remove_dir_all(&outside).ok();
        std::fs::remove_dir_all(&dir).ok();
    }
}
