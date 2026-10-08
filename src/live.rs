//! Drives a real `claude` process (stream-json in and out) with the cc-mid profile, and records every event.
//!
//! One process serves the whole conversation, so follow-ups keep the prompt cache warm. Stopping a run sends the
//! interrupt control request the official Agent SDK uses; if claude does not answer, the process is killed and the
//! next message resumes the session with `--resume`.

use crate::{diag, route::{Lane, Profile}, runlog::Run, verify};
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    fs::File,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{Receiver, Sender, channel},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

const MID_PROMPT: &str = "You are a coding agent working in {cwd}. Today is {date}.
- Read the relevant code before editing. Use Grep/Glob to search.
- Fix root causes with the smallest change that follows the existing code style.
- Add or update tests when behavior changes. Run the relevant tests and fix failures before finishing.
- Never run destructive commands (rm -rf outside the working directory, git reset --hard, force push). Do not commit.
- Finish with a short summary of what changed and how you verified it.";

enum Msg {
    Line(String),
    /// stderr closed: the process is exiting.
    Exited(String),
    /// A verification finished.
    Verified(verify::Outcome),
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
    pub verify: verify::Mode,
    /// Passed to the tool server: shrink bash output above this many chars (0: off).
    pub bash_cap: usize,
    /// Type/compile errors after each edit (on by default, `--no-diagnostics`): the hook's state directory once started.
    pub diagnostics: bool,
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
    held: VecDeque<String>,
    /// claude's last result event and the last run-log line (headless mode reports them).
    pub last_result: Option<Value>,
    pub last_log: Option<Value>,
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
            verify: verify::Mode::Auto,
            bash_cap: crate::mcp::DEFAULT_BASH_CAP,
            diagnostics: false,
            diag_dir: None,
            project_instructions: true,
            profile: Profile::Lean,
            user_settings: false,
            instructions_used: (0, Vec::new()),
            verifying: false,
            held: VecDeque::new(),
            last_result: None,
            last_log: None,
            lane: ("manual".into(), String::new()),
            last_run: None,
            escalate: None,
            escalated: false,
        })
    }

    pub fn elapsed_ms(&self) -> f64 {
        self.t0.elapsed().as_secs_f64() * 1000.0
    }

    /// Sends a user message, starting (or resuming) the claude process if needed. Returns the event to show.
    pub fn send(&mut self, text: &str) -> Result<(f64, Value)> {
        if self.verifying {
            self.held.push_back(text.to_string());
        } else {
            self.start_run(text)?;
        }
        Ok(self.record(json!({"type": "harness_user", "text": text})))
    }

    fn start_run(&mut self, text: &str) -> Result<()> {
        // ponytail: taken while claude starts reading; an edit that lands first is counted as known, never as new
        let types_before = verify::typecheck_command(&self.verify, &self.cwd).map(|cmd| {
            let cwd = self.cwd.clone();
            crate::runlog::Baseline::Pending(std::thread::spawn(move || verify::run_raw(&cwd, &cmd).1))
        });
        self.write_user(text)?;
        let mut run = Run::new(text, self.elapsed_ms(), &self.cwd);
        run.types_before = types_before;
        self.runs.push_back(run);
        Ok(())
    }

    fn write_user(&mut self, text: &str) -> Result<()> {
        if self.child.is_none() {
            self.spawn()?;
        }
        let line = json!({"type": "user", "message": {"role": "user", "content": text}});
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

    /// Nothing in flight: every run has ended and no message is waiting.
    pub fn idle(&self) -> bool {
        self.runs.is_empty() && !self.verifying && self.held.is_empty()
    }

    /// Asks claude to stop the current run; the process stays up and keeps its cache.
    pub fn interrupt(&mut self) -> Option<(f64, Value)> {
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
        let mut out = Vec::new();
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
                    }
                    out.push(self.record(e));
                    if is_result {
                        out.extend(self.after_result());
                    }
                }
                Msg::Verified(o) => out.extend(self.after_verify(o)),
                Msg::Exited(stderr) => {
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
        let Some(run) = self.runs.front_mut() else { return vec![] };
        if !success || run.is_interrupted() {
            return self.finish_front();
        }
        let changed = run.changed_files(&self.cwd);
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
            match self.write_user(&verify::feedback(&o, round)) {
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
            let Some(text) = self.held.pop_front() else { break };
            if let Err(e) = self.start_run(&text) {
                out.push(self.record(json!({"type": "harness_notice", "text": format!("{e:#}"), "error": true})));
            }
        }
        out
    }

    /// The process is gone: runs without a result end here (stopped if the user asked, otherwise crashed).
    fn end_runs(&mut self) {
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
            "bash_cap": self.bash_cap, "diagnostics": self.diagnostics,
            "profile": self.profile.name(), "setting_sources": self.setting_sources(),
            "project_instructions": {"bytes": self.instructions_used.0, "files": self.instructions_used.1},
            "lane": self.lane.0, "lane_reason": self.lane.1, "escalated": self.escalated,
            "claude_version": self.claude_version, "cwd": self.cwd, "session_id": self.session_id, "recording": self.rec_path,
        })
    }

    fn log(&mut self, line: Value) {
        self.last_run = line["id"].as_str().map(String::from);
        self.last_log = Some(line.clone());
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
        MID_PROMPT.replace("{cwd}", &self.cwd.to_string_lossy()).replace("{date}", &date)
    }

    fn spawn(&mut self) -> Result<()> {
        let mut cmd = Command::new("claude");
        cmd.args(["-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose", "--include-partial-messages"])
            .args(["--model", &self.model, "--effort", &self.effort, "--dangerously-skip-permissions"])
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
            }
            Profile::RepoLean => {
                // Claude Code loads CLAUDE.md (and its @imports) itself, but not AGENTS.md: that one goes in the prompt
                let mut prompt = self.mid_prompt();
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
                    .args(["--allowedTools", "mcp__lean__bash,mcp__lean__read,mcp__lean__write,mcp__lean__edit"]);
            }
        }
        if self.diagnostics {
            if self.diag_dir.is_none() {
                self.diag_dir = Some(diag::prepare(&self.cwd)?);
            }
            let exe = std::env::current_exe().context("locating the tazuna binary")?;
            cmd.args(["--settings", &diag::settings(&exe, self.diag_dir.as_ref().expect("set above"))]);
        }
        if let Some(id) = &self.session_id {
            cmd.args(["--resume", id]);
        }
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
    use super::*;

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
