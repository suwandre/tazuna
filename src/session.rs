//! Session state built from Claude Code `stream-json` events.

use serde_json::{Value, json};

pub enum Block {
    Prompt(String),
    Text { text: String, live: bool },
    Thinking { tokens: u64, live: bool },
    Tool(Tool),
    Done(Summary),
    /// Harness messages: stopped runs, process exits.
    Notice { text: String, error: bool },
    /// tazuna running the tests after claude said it was done.
    Verify(Verify),
}

pub struct Verify {
    pub cmd: String,
    /// The runner found no tests.
    pub no_tests: bool,
    /// The tests outran the time limit: no result.
    pub timed_out: bool,
    pub round: u64,
    /// None while the tests run.
    pub ok: Option<bool>,
    pub secs: f64,
    pub output: String,
}

pub struct Tool {
    pub id: String,
    pub name: String,
    pub input: Value,
    pub result: Option<String>,
    pub error: bool,
    pub started_ms: f64,
    pub done_ms: Option<f64>,
    /// A subagent this call started (Task/Agent): how many tool calls it has made, and the latest one.
    pub sub: Option<(usize, String)>,
}

impl Tool {
    pub fn secs(&self) -> Option<f64> {
        self.done_ms.map(|d| (d - self.started_ms) / 1000.0)
    }
}

/// When a block appeared and in which model turn.
#[derive(Clone, Copy)]
pub struct Meta {
    pub at_ms: f64,
    pub turn: u64,
}

pub struct Summary {
    pub ok: bool,
    pub secs: f64,
    /// Model turns in this run (not the whole session).
    pub turns: u64,
    pub cost: f64,
    /// Session totals when the run finished.
    pub usage: Usage,
    /// The user stopped this run.
    pub stopped: bool,
}

#[derive(Default, Clone, Copy)]
pub struct Usage {
    pub input: u64,
    pub cache_write: u64,
    pub cache_read: u64,
    pub output: u64,
}

impl Usage {
    pub fn total_in(&self) -> u64 {
        self.input + self.cache_write + self.cache_read
    }

    pub fn cache_hit(&self) -> f64 {
        let t = self.total_in();
        if t == 0 { 0.0 } else { self.cache_read as f64 / t as f64 }
    }

    /// Rough $ estimate until Claude Code reports the exact total at the end.
    // ponytail: Opus 5.5 rates hardcoded ($/M: in 4, cache write 6, cache read 0.4, out 20; cache write calibrated on one
    // recorded run, ~3% off); per-model table when other models matter
    pub fn estimate(&self) -> f64 {
        (self.input as f64 * 4.0 + self.cache_write as f64 * 6.0 + self.cache_read as f64 * 0.4 + self.output as f64 * 20.0) / 1e6
    }
}

/// What the agent is doing right now (drives the status bar and spinners).
#[derive(Clone, Copy, PartialEq)]
pub enum Activity {
    /// Live mode, waiting for the user's next message.
    Ready,
    Starting,
    Waiting,
    Thinking,
    Writing,
    Tool,
    /// tazuna is running the tests.
    Verifying,
    Done,
}

pub struct Session {
    pub blocks: Vec<Block>,
    pub meta: Vec<Meta>,
    /// Usage per model turn (index = turn - 1).
    pub turn_usage: Vec<Usage>,
    /// Tokens in the latest request: how full the context is.
    pub context: u64,
    /// The model's context window, as claude's result reports it; None until then and after a switch (the own loop's
    /// providers report none).
    pub context_window: Option<u64>,
    /// The live thinking count before its latest step and when that step came (ms): the screen counts up between them.
    think_from: (u64, f64),
    now_ms: f64,
    pub model: String,
    pub effort: String,
    pub cwd: String,
    pub usage: Usage,
    pub turns: u64,
    pub activity: Activity,
    /// Exact $ reported at the last result (summed across processes).
    pub exact_cost: Option<f64>,
    /// Cost of processes that already exited: claude reports cost cumulatively per process, from $0 after a restart.
    cost_base: f64,
    /// The same split by who pays: own-loop providers (`paid`, real money) and claude on the plan (`plan`, at API prices),
    /// of exited processes (`_base`) and of the current one (`_now`).
    paid_base: f64,
    plan_base: f64,
    paid_now: f64,
    plan_now: f64,
    /// Claude's plan usage, 0-100: the 5-hour and 7-day windows as its last rate_limit_event reported them, and as the
    /// first one in this session did (the difference is this session's share, plus whatever else used the plan meanwhile).
    pub limits: Option<(f64, f64)>,
    pub limits_start: Option<(f64, f64)>,
    /// When the plan windows were read (Unix seconds): claude reports them only now and then, so a session opens with the
    /// last reading of any earlier one.
    pub limits_at: Option<u64>,
    /// SubRouter's balance in USD, where a SubRouter key is set.
    pub balance: Option<f64>,
    /// Part of the cost is priced at list rates: a stopped turn reported $0.
    pub estimated: bool,
    /// Time the agent spent working (message to its last token), summed over finished runs; `run_since`: when the run
    /// in progress began (ms into the session).
    busy_ms: f64,
    run_since: Option<f64>,
    usage_at_result: Usage,
    /// An interrupt was requested; the next result ends a stopped run.
    stopping: bool,
    /// Bumped whenever a new block appears or a tool finishes (effects key off it).
    pub new_blocks: usize,
    pub finished_tools: usize,
    /// Completed runs (one per result event).
    pub results: usize,
    /// The profile the router chose (live mode).
    pub profile: String,
    /// Permission mode in tazuna's words: auto, ask, edits, plan.
    pub mode: String,
    /// Slash commands claude offers in this session (built-ins, the repo's commands and skills), plus tazuna's own.
    pub commands: Vec<String>,
    /// Claude's to-do list (TodoWrite): (status, text).
    pub todos: Vec<(String, String)>,
    /// The last thing the user sent, to tell what a command result belongs to.
    last_prompt: String,
    /// The current model message arrived as content blocks (claude streams them; the own loop sends whole messages).
    streamed: bool,
}

pub fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// The newest plan reading in the session recordings under `dir` (claude's rate_limit_event): (5-hour %, 7-day %, read
/// at). A window whose reset time has passed since counts from 0.
pub fn last_limits(dir: &std::path::Path) -> Option<(f64, f64, u64)> {
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(dir).ok()?.filter_map(|e| e.ok().map(|e| e.path())).collect();
    files.sort();
    let now = unix_now();
    // the newest files first; a file's name starts with the second it began, each line carries its offset in ms
    for f in files.iter().rev().take(200) {
        let began: u64 = f.file_name()?.to_str()?.split('-').next()?.parse().ok()?;
        let text = std::fs::read_to_string(f).unwrap_or_default();
        let Some(line) = text.lines().rev().find(|l| l.contains("\"rate_limit_event\"")) else { continue };
        let d: Value = serde_json::from_str(line).ok()?;
        let w = &d["e"]["rate_limit_info"]["unifiedWindows"];
        let pct = |k: &str| {
            let u = w[k]["utilization"].as_f64()? * 100.0;
            Some(if w[k]["resetsAt"].as_u64().is_some_and(|r| r < now) { 0.0 } else { u })
        };
        return Some((pct("five_hour")?, pct("seven_day")?, began + d["t"].as_u64().unwrap_or(0) / 1000));
    }
    None
}

impl Session {
    pub fn new(prompt: &str, effort: &str) -> Self {
        Session {
            // live sessions start empty; their messages arrive as `harness_user` events
            blocks: if prompt.trim().is_empty() { vec![] } else { vec![Block::Prompt(prompt.trim().to_string())] },
            meta: if prompt.trim().is_empty() { vec![] } else { vec![Meta { at_ms: 0.0, turn: 0 }] },
            turn_usage: Vec::new(),
            context: 0,
            // ponytail: Opus 5.5's window as the default until the result event reports the real one
            context_window: None,
            think_from: (0, 0.0),
            now_ms: 0.0,
            model: String::new(),
            effort: effort.to_string(),
            cwd: String::new(),
            usage: Usage::default(),
            turns: 0,
            activity: if prompt.trim().is_empty() { Activity::Ready } else { Activity::Starting },
            exact_cost: None,
            cost_base: 0.0,
            paid_base: 0.0,
            plan_base: 0.0,
            paid_now: 0.0,
            plan_now: 0.0,
            limits: None,
            limits_start: None,
            limits_at: None,
            balance: None,
            estimated: false,
            busy_ms: 0.0,
            run_since: None,
            usage_at_result: Usage::default(),
            stopping: false,
            new_blocks: 1,
            finished_tools: 0,
            results: 0,
            profile: String::new(),
            mode: "auto".into(),
            commands: vec!["bad".into(), "good".into(), "keys".into(), "loop".into(), "model".into(), "new".into(), "resume".into(), "rewind".into(), "usage".into(), "vim".into()],
            todos: Vec::new(),
            last_prompt: String::new(),
            streamed: false,
        }
    }

    /// The screen for a new conversation (`/new`, `/resume`): the old one's messages, tools, files, tokens and costs left
    /// behind; the session's settings and the machine's readings kept (model, folder, profile, mode, commands, plan
    /// windows, SubRouter balance).
    pub fn fresh(&self) -> Session {
        let mut s = Session::new("", &self.effort);
        (s.model, s.cwd, s.profile, s.mode, s.commands) = (self.model.clone(), self.cwd.clone(), self.profile.clone(), self.mode.clone(), self.commands.clone());
        (s.limits, s.limits_at, s.balance, s.context_window) = (self.limits, self.limits_at, self.balance, self.context_window);
        s
    }

    /// The live thinking count to show at `now_ms`: from the total before the latest step up to `tokens` over the second
    /// after that step came, eased, so it reads as counting rather than jumping by 50-200.
    pub fn thinking_shown(&self, tokens: u64, now_ms: f64) -> u64 {
        let (from, at) = self.think_from;
        if from >= tokens {
            return tokens;
        }
        let p = ((now_ms - at) / 1000.0).clamp(0.0, 1.0);
        let eased = 1.0 - (1.0 - p) * (1.0 - p);
        from + ((tokens - from) as f64 * eased) as u64
    }

    /// Time the agent has spent working by `now_ms` (into the session): finished runs plus the one in progress. Next to
    /// the session's elapsed time, which includes the user's own.
    pub fn busy(&self, now_ms: f64) -> f64 {
        self.busy_ms + self.run_since.map_or(0.0, |r| (now_ms - r).max(0.0))
    }

    /// Real money (own-loop providers) and plan use at API prices (claude), both exact as reported.
    pub fn split_cost(&self) -> (f64, f64) {
        (self.paid_base + self.paid_now, self.plan_base + self.plan_now)
    }

    /// A process ended (restart, escalation, model switch): its reported costs are final, the next one starts from $0.
    fn close_process(&mut self) {
        self.cost_base = self.exact_cost.unwrap_or(self.cost_base);
        (self.paid_base, self.plan_base) = (self.paid_base + self.paid_now, self.plan_base + self.plan_now);
        (self.paid_now, self.plan_now) = (0.0, 0.0);
    }

    /// Exact cost so far plus an estimate for tokens used since the last reported result.
    pub fn cost(&self) -> f64 {
        let (u, b) = (&self.usage, &self.usage_at_result);
        let since = Usage {
            input: u.input - b.input,
            cache_write: u.cache_write - b.cache_write,
            cache_read: u.cache_read - b.cache_read,
            output: u.output - b.output,
        };
        self.exact_cost.unwrap_or(0.0) + since.estimate()
    }

    fn push(&mut self, b: Block) {
        self.end_live();
        self.push_aside(b);
    }

    /// A block beside the model's stream (tazuna's own notices: "copied 12 chars", a hint): the text or thinking being
    /// streamed stays live. Through `push` such a notice ended it: the spinner froze and what came after was dropped.
    fn push_aside(&mut self, b: Block) {
        self.blocks.push(b);
        self.meta.push(Meta { at_ms: self.now_ms, turn: self.turns });
        self.new_blocks += 1;
    }

    fn end_live(&mut self) {
        if let Some(Block::Text { live, .. } | Block::Thinking { live, .. }) = self.live_block() {
            *live = false;
        }
    }

    /// The text or thinking block still being streamed: the newest live one, not simply the last block (a notice such as
    /// "copied 12 chars" can land after it mid-stream, and the stream then went into the notice: the thinking counter
    /// stopped and text written meanwhile was lost, 2026-11-02).
    fn live_block(&mut self) -> Option<&mut Block> {
        self.blocks.iter_mut().rev().find(|b| matches!(b, Block::Text { live: true, .. } | Block::Thinking { live: true, .. }))
    }

    /// Apply one event that happened `at_ms` into the run.
    pub fn apply(&mut self, e: &Value, at_ms: f64) {
        self.now_ms = at_ms;
        if let Some(parent) = e["parent_tool_use_id"].as_str() {
            // a subagent's own messages: they belong under the call that started it, not in the main transcript
            self.subagent(parent, e);
            return;
        }
        let s = |v: &Value| v.as_str().unwrap_or_default().to_string();
        match (e["type"].as_str(), e["subtype"].as_str()) {
            (Some("system"), Some("init")) => {
                self.model = s(&e["model"]);
                self.cwd = s(&e["cwd"]);
                let mut cmds: Vec<String> = e["slash_commands"].as_array().into_iter().flatten().filter_map(|c| c.as_str().map(String::from)).collect();
                cmds.extend(["good", "bad", "keys", "loop", "model", "new", "resume", "rewind", "usage", "vim"].map(String::from));
                cmds.sort();
                cmds.dedup();
                self.commands = cmds;
                self.activity = Activity::Waiting;
            }
            (Some("system"), Some("status")) => {
                if let Some(m) = e["permissionMode"].as_str() {
                    self.mode = mode_name(m).to_string();
                }
                self.activity = Activity::Waiting;
            }
            (Some("system"), Some("compact_boundary")) => {
                self.push(Block::Notice { text: "── context compacted: earlier turns are now a summary ──".into(), error: false });
            }
            // the own loop's provider events: shown, not only in the session file (a pool that went down and a conversation
            // that moved were invisible until a run looked stuck, 2026-10-28)
            (Some("system"), Some(sub)) if e["message"].is_string() => {
                let what = match sub {
                    "route_switch" => "route",
                    "group_switch" => "provider group",
                    "conversation_moved" => "conversation moved to another pool, without its reasoning",
                    "thinking_dropped" => "the provider refused the thinking: sent again without it",
                    "reasoning_dropped" => "the provider refused the reasoning: sent again without it",
                    "reasoning_resent" => "the provider asked for the reasoning: sent again with it",
                    "cache_key_dropped" => "the provider refused the cache key: sent again without it",
                    _ => return,
                };
                self.push(Block::Notice { text: format!("{what} · {}", shown(&s(&e["message"]), 100)), error: false });
            }
            (Some("harness_user"), _) => {
                self.run_since.get_or_insert(at_ms);
                self.last_prompt = s(&e["text"]);
                self.push(Block::Prompt(s(&e["text"])));
                self.activity = Activity::Waiting;
            }
            (Some("harness_interrupt"), _) => self.stopping = true,
            (Some("harness_verify_start"), _) => {
                self.push(Block::Verify(Verify { cmd: s(&e["cmd"]), no_tests: false, timed_out: false, round: e["round"].as_u64().unwrap_or(1), ok: None, secs: 0.0, output: String::new() }));
                self.activity = Activity::Verifying;
            }
            (Some("harness_verify_done"), _) => {
                if let Some(v) = self.blocks.iter_mut().rev().find_map(|b| if let Block::Verify(v) = b { Some(v) } else { None }) {
                    (v.ok, v.secs, v.output) = (e["ok"].as_bool(), e["secs"].as_f64().unwrap_or(0.0), s(&e["output"]));
                    v.no_tests = e["no_tests"] == true;
                    v.timed_out = e["timed_out"] == true;
                }
                self.finished_tools += 1; // pulse like a finished tool
                self.activity = if e["final"] == true { Activity::Done } else { Activity::Waiting };
            }
            (Some("harness_verify_feedback"), _) => {
                let text = format!("↻ sent the failing tests back to claude (round {} of {})", e["round"], e["max"]);
                self.push(Block::Notice { text, error: false });
                self.activity = Activity::Waiting;
            }
            (Some("harness_route"), _) => {
                (self.model, self.effort) = (s(&e["model"]), s(&e["effort"]));
                let model = self.model.trim_start_matches("claude-").to_string();
                let up = e["escalate"].as_str().map_or(String::new(), |x| format!(" · escalates to {} if tests fail", x.trim_start_matches("claude-")));
                self.profile = s(&e["profile"]);
                let profile = e["profile"].as_str().map_or(String::new(), |p| format!(" · {p} profile"));
                self.push(Block::Notice { text: format!("lane {}: {model} {}{profile} ({}){up}", s(&e["lane"]), self.effort, s(&e["reason"])), error: false });
            }
            (Some("harness_escalate"), _) => {
                // the old process is gone: its reported cost is final
                self.close_process();
                (self.model, self.effort) = (s(&e["model"]), s(&e["effort"]));
                let text = format!("↑ tests failed on the cheap lane: fixing on {} {}", self.model.trim_start_matches("claude-"), self.effort);
                self.push(Block::Notice { text, error: false });
            }
            (Some("harness_verify_cancel"), _) => {
                self.push(Block::Notice { text: "verification cancelled · the run ends when the tests return".into(), error: false });
                self.activity = Activity::Ready;
            }
            // UI-only message (not from the process): no effect on cost or tools
            (Some("harness_review_start"), _) => {
                self.push(Block::Notice { text: format!("review: {} is checking the change against the task", s(&e["model"])), error: false })
            }
            (Some("harness_review_done"), _) => {
                let n = e["issues"].as_array().map_or(0, Vec::len);
                let text = match (e["error"].as_str(), e["ok"] == true) {
                    (Some(err), _) => format!("review skipped: {err}"),
                    (None, true) => "review: no problems found".to_string(),
                    (None, false) => format!("review: {n} problem{} sent back", if n == 1 { "" } else { "s" }),
                };
                self.push(Block::Notice { text, error: false })
            }
            (Some("harness_review"), _) => self.push(Block::Notice { text: "self-review: checking the change against the task".into(), error: false }),
            (Some("harness_permission"), _) => {
                // the question itself is drawn from live's queue; the transcript only gets the plan to approve
                if e["tool"] == "ExitPlanMode" {
                    let plan: String = e["input"]["plan"].as_str().unwrap_or_default().chars().map(|c| if invisible(c) || (c.is_control() && c != '\n') { '�' } else { c }).collect();
                    self.push(Block::Text { text: format!("**Plan**\n\n{plan}"), live: false });
                }
                self.activity = Activity::Waiting;
            }
            (Some("harness_permission_cleared"), _) => {
                self.push(Block::Notice { text: "open permission questions dropped: claude stopped".into(), error: false });
            }
            (Some("harness_permission_done"), _) => {
                self.mode = mode_name(e["mode"].as_str().unwrap_or_default()).to_string();
                let verb = match (e["allow"] == true, e["always"] == true) {
                    (true, true) => "allowed for this session:",
                    (true, false) => "allowed",
                    _ => "declined",
                };
                self.push(Block::Notice { text: format!("{verb} {}", s(&e["tool"])), error: false });
            }
            (Some("harness_mode"), _) => {
                self.mode = mode_name(e["mode"].as_str().unwrap_or_default()).to_string();
                self.push(Block::Notice { text: format!("mode: {}", self.mode), error: false });
            }
            (Some("harness_info"), _) => self.push_aside(Block::Notice { text: s(&e["text"]), error: false }),
            // the own loop's reasoning before a message: a thinking row like claude's, its size estimated (~4 characters a token)
            (Some("reasoning"), _) => self.push(Block::Thinking { tokens: (s(&e["text"]).chars().count() / 4) as u64, live: false }),
            (Some("harness_switch"), _) => {
                // /model: the old process is gone
                self.close_process();
                (self.model, self.effort) = (s(&e["model"]), s(&e["effort"]));
                self.context_window = None; // the next model reports its own (the own loop's: none)
                self.push(Block::Notice { text: s(&e["text"]), error: false });
            }
            (Some("rate_limit_event"), _) => {
                let w = &e["rate_limit_info"]["unifiedWindows"];
                if let (Some(h5), Some(d7)) = (w["five_hour"]["utilization"].as_f64(), w["seven_day"]["utilization"].as_f64()) {
                    self.limits = Some((h5 * 100.0, d7 * 100.0));
                    self.limits_start.get_or_insert((h5 * 100.0, d7 * 100.0));
                    self.limits_at = Some(unix_now());
                }
            }
            (Some("harness_notice"), _) => {
                self.stopping = false;
                self.push(Block::Notice { text: s(&e["text"]), error: e["error"].as_bool().unwrap_or(false) });
                // the process ended: its reported cost is final, the next one starts from $0
                self.close_process();
                for b in &mut self.blocks {
                    if let Block::Tool(t) = b {
                        if t.result.is_none() {
                            (t.result, t.error, t.done_ms) = (Some("interrupted".into()), true, Some(at_ms));
                        }
                    }
                }
                self.activity = Activity::Ready;
            }
            (Some("stream_event"), _) => self.stream(&e["event"]),
            (Some("assistant"), _) => {
                // Full message: tool inputs arrive complete here (partial JSON is not worth rendering). The own loop
                // streams no content blocks, only the message: its text and tool calls are drawn from here.
                if !self.streamed {
                    for c in e["message"]["content"].as_array().into_iter().flatten() {
                        match c["type"].as_str() {
                            Some("text") if !s(&c["text"]).trim().is_empty() => self.push(Block::Text { text: s(&c["text"]), live: false }),
                            Some("tool_use") => self.stream(&json!({"type": "content_block_start", "content_block": c})),
                            _ => {}
                        }
                    }
                }
                for c in e["message"]["content"].as_array().into_iter().flatten() {
                    if c["type"] == "tool_use" && c["name"] == "TodoWrite" {
                        self.todos = c["input"]["todos"].as_array().into_iter().flatten().map(|t| (s(&t["status"]), s(&t["content"]))).collect();
                    }
                    if c["type"] == "tool_use" {
                        let id = s(&c["id"]);
                        if let Some(t) = self.tool_mut(&id) {
                            t.input = c["input"].clone();
                        }
                    }
                }
            }
            (Some("user"), _) => {
                for c in e["message"]["content"].as_array().into_iter().flatten() {
                    if c["type"] != "tool_result" {
                        continue;
                    }
                    let text = match &c["content"] {
                        Value::String(t) => t.clone(),
                        Value::Array(parts) => parts.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join("\n"),
                        _ => String::new(),
                    };
                    let error = c["is_error"].as_bool().unwrap_or(false);
                    if let Some(t) = self.tool_mut(&s(&c["tool_use_id"])) {
                        t.result = Some(text);
                        t.error = error;
                        t.done_ms = Some(at_ms);
                        self.finished_tools += 1;
                    }
                }
                self.activity = Activity::Waiting;
            }
            (Some("result"), _) => {
                self.end_live();
                // a slash command answered without a model turn (/cost, /context …): its text is the only output
                let text = s(&e["result"]);
                if e["num_turns"] == 0 && self.last_prompt.starts_with('/') {
                    if self.last_prompt.trim() == "/clear" {
                        self.todos.clear();
                        self.push(Block::Notice { text: "── conversation cleared: claude starts fresh from here ──".into(), error: false });
                    } else if !text.trim().is_empty() {
                        self.push(Block::Text { text, live: false });
                    }
                }
                if let Some(mut c) = e["total_cost_usd"].as_f64() {
                    // tazuna marks the own loop's results; older recordings: a model that is not claude's
                    let paid = e["tazuna_paid"] == true || !self.model.is_empty() && !self.model.starts_with("claude-");
                    let prev = if paid { self.paid_now } else { self.plan_now };
                    let (u, b) = (self.usage, self.usage_at_result);
                    let since = Usage { input: u.input - b.input, cache_write: u.cache_write - b.cache_write, cache_read: u.cache_read - b.cache_read, output: u.output - b.output };
                    // a stopped turn reports nothing for itself (claude: total_cost_usd 0 after 21k tokens, 2026-11-02):
                    // its tokens are priced at list rates instead, and the cost is marked as an estimate
                    if c <= prev && since.total_in() + since.output > 0 {
                        c = prev + since.estimate();
                        self.estimated = true;
                    }
                    self.exact_cost = Some(self.cost_base + c);
                    self.usage_at_result = self.usage;
                    if paid {
                        self.paid_now = c;
                    } else {
                        self.plan_now = c;
                    }
                }
                if let Some(w) = e["modelUsage"].as_object().and_then(|m| m.values().next()).and_then(|m| m["contextWindow"].as_u64()) {
                    self.context_window = Some(w);
                }
                let ok = e["subtype"] == "success" && !e["is_error"].as_bool().unwrap_or(false);
                let secs = e["duration_ms"].as_f64().unwrap_or(0.0) / 1000.0;
                let ran = if secs > 0.0 { secs * 1000.0 } else { self.run_since.map_or(0.0, |r| (at_ms - r).max(0.0)) };
                self.busy_ms += ran;
                self.run_since = None;
                let turns = e["num_turns"].as_u64().unwrap_or(0);
                let stopped = std::mem::take(&mut self.stopping);
                self.push(Block::Done(Summary { ok, secs, turns, cost: self.cost(), usage: self.usage, stopped }));
                self.activity = Activity::Done;
                self.results += 1;
            }
            _ => {}
        }
    }

    /// Turns that read back less than half of what the turn before them had cached: a prompt cache miss (same rule as
    /// the run log). Each one re-sends the context at full price.
    pub fn cache_misses(&self) -> usize {
        self.turn_usage
            .windows(2)
            .filter(|w| {
                let cached = w[0].cache_write + w[0].cache_read;
                cached > 0 && w[1].cache_read * 2 < cached
            })
            .count()
    }

    fn stream(&mut self, ev: &Value) {
        match ev["type"].as_str() {
            Some("message_start") => {
                self.streamed = false;
                self.turns += 1;
                self.turn_usage.push(Usage::default());
            }
            Some("content_block_start") => {
                self.streamed = true;
                let b = &ev["content_block"];
                match b["type"].as_str() {
                    Some("text") => {
                        self.push(Block::Text { text: String::new(), live: true });
                        self.activity = Activity::Writing;
                    }
                    Some("thinking") => {
                        self.push(Block::Thinking { tokens: 0, live: true });
                        self.activity = Activity::Thinking;
                    }
                    Some("tool_use") => {
                        self.push(Block::Tool(Tool {
                            id: b["id"].as_str().unwrap_or_default().to_string(),
                            name: b["name"].as_str().unwrap_or_default().to_string(),
                            input: Value::Null,
                            result: None,
                            error: false,
                            started_ms: self.now_ms,
                            done_ms: None,
                            sub: None,
                        }));
                        self.activity = Activity::Tool;
                    }
                    _ => {}
                }
            }
            Some("content_block_delta") => {
                let d = &ev["delta"];
                match (d["type"].as_str(), self.live_block()) {
                    (Some("text_delta"), Some(Block::Text { text, .. })) => text.push_str(d["text"].as_str().unwrap_or_default()),
                    // each delta carries the tokens thought since the last one (50, 100, 150 ...): they add up, as in
                    // Claude Code's own counter
                    (Some("thinking_delta"), Some(Block::Thinking { tokens, .. })) => {
                        let before = *tokens;
                        *tokens += d["estimated_tokens"].as_u64().unwrap_or(0);
                        self.think_from = (before, self.now_ms);
                    }
                    _ => {}
                }
            }
            Some("content_block_stop") => self.end_live(),
            Some("message_delta") => {
                let u = &ev["usage"];
                let n = |k: &str| u[k].as_u64().unwrap_or(0);
                let t = Usage {
                    input: n("input_tokens"),
                    cache_write: n("cache_creation_input_tokens"),
                    cache_read: n("cache_read_input_tokens"),
                    output: n("output_tokens"),
                };
                self.usage.input += t.input;
                self.usage.cache_write += t.cache_write;
                self.usage.cache_read += t.cache_read;
                self.usage.output += t.output;
                self.context = t.total_in();
                if let Some(last) = self.turn_usage.last_mut() {
                    *last = t;
                }
            }
            _ => {}
        }
    }

    fn subagent(&mut self, parent: &str, e: &Value) {
        if e["type"] != "assistant" {
            return;
        }
        let calls: Vec<String> = e["message"]["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|c| c["type"] == "tool_use")
            .map(|c| {
                let i = &c["input"];
                let arg = ["file_path", "path", "pattern", "command", "description"].iter().find_map(|k| i[*k].as_str()).unwrap_or_default();
                format!("{} {}", c["name"].as_str().unwrap_or_default(), arg.lines().next().unwrap_or_default())
            })
            .collect();
        if let (Some(t), Some(last)) = (self.tool_mut(parent), calls.last().cloned()) {
            let n = t.sub.as_ref().map_or(0, |s| s.0) + calls.len();
            t.sub = Some((n, last));
        }
    }

    fn tool_mut(&mut self, id: &str) -> Option<&mut Tool> {
        self.blocks.iter_mut().rev().find_map(|b| match b {
            Block::Tool(t) if t.id == id => Some(t),
            _ => None,
        })
    }
}

/// Text from the model made safe to show in a permission question: control characters (terminal escapes, carriage
/// returns that could overwrite what is shown) become visible, line breaks become ⏎, and a cut says how much is hidden.
pub fn shown(text: &str, max: usize) -> String {
    let clean: String = text
        .chars()
        .map(|c| match c {
            '\n' => '⏎',
            '\t' => ' ',
            c if c.is_control() || invisible(c) => '�',
            c => c,
        })
        .collect();
    let n = clean.chars().count();
    if n <= max { clean } else { format!("{}… (+{} chars)", clean.chars().take(max).collect::<String>(), n - max) }
}

/// Characters that change how text around them looks without showing themselves: bidirectional overrides (which can
/// make "rm" read as "mr"), zero-width characters, line and paragraph separators.
pub fn invisible(c: char) -> bool {
    matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{2069}' | '\u{2028}' | '\u{2029}' | '\u{FEFF}' | '\u{061C}' | '\u{180E}')
}

/// claude's permission mode names in tazuna's words.
pub fn mode_name(m: &str) -> &str {
    match m {
        "bypassPermissions" => "auto",
        "default" => "ask",
        "acceptEdits" => "edits",
        "plan" => "plan",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_text_cannot_hide_anything() {
        assert_eq!(shown("ls\nrm -rf ~", 100), "ls⏎rm -rf ~");
        assert_eq!(shown("ok\u{1b}[2K\rfake", 100), "ok�[2K�fake");
        assert_eq!(shown(&"x".repeat(10), 4), "xxxx… (+6 chars)");
        assert_eq!(shown("echo hi\u{202E}fr- mr", 100), "echo hi�fr- mr", "a right-to-left override cannot reverse what is shown");
        assert_eq!(shown("a\u{200B}b\u{2028}c", 100), "a�b�c");
    }

    #[test]
    fn shows_the_own_loops_provider_events() {
        let mut s = Session::new("", "medium");
        s.apply(&json!({"type": "system", "subtype": "conversation_moved", "message": "responses_source_unavailable"}), 1.0);
        let Block::Notice { text, .. } = s.blocks.last().unwrap() else { panic!() };
        assert_eq!(text, "conversation moved to another pool, without its reasoning · responses_source_unavailable");
        let n = s.blocks.len();
        s.apply(&json!({"type": "system", "subtype": "hook_response", "message": "x"}), 2.0);
        assert_eq!(s.blocks.len(), n, "other system events stay out of the transcript");
        // the own loop sends whole messages, no content blocks: its text and tool calls are drawn from them, once
        s.apply(&json!({"type": "stream_event", "event": {"type": "message_start"}}), 3.0);
        s.apply(&json!({"type": "reasoning", "text": "Ambiguous; likely run the tests."}), 3.5);
        s.apply(&json!({"type": "assistant", "message": {"content": [{"type": "text", "text": "I'll run the tests."},
            {"type": "tool_use", "id": "c1", "name": "bash", "input": {"command": "cargo test"}}]}}), 4.0);
        s.apply(&json!({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": "c1", "content": "94 passed"}]}}), 5.0);
        s.apply(&json!({"type": "stream_event", "event": {"type": "message_start"}}), 6.0);
        s.apply(&json!({"type": "assistant", "message": {"content": [{"type": "text", "text": "All 94 pass."}]}}), 7.0);
        s.apply(&json!({"type": "result", "subtype": "success", "result": "All 94 pass.", "num_turns": 2, "total_cost_usd": 0.000187,
            "duration_ms": 33500, "tazuna_paid": true}), 8.0);
        let shown: Vec<String> = s.blocks[n..].iter().map(|b| match b {
            Block::Text { text, .. } => text.clone(),
            Block::Tool(t) => format!("{} {} -> {}", t.name, t.input["command"].as_str().unwrap_or(""), t.result.as_deref().unwrap_or("")),
            Block::Done(d) => format!("done {:.1}s", d.secs),
            Block::Thinking { tokens, .. } => format!("thinking ~{tokens}"),
            _ => "?".into(),
        }).collect();
        assert_eq!(shown, ["thinking ~8", "I'll run the tests.", "bash cargo test -> 94 passed", "All 94 pass.", "done 33.5s"]);
        assert_eq!(crate::ui::cost_str(&s), "$0.00019");
        assert_eq!([0.0, 0.004321, 0.0123, 1.5].map(crate::ui::usd), ["$0", "$0.0043", "$0.01", "$1.50"]);
    }

    #[test]
    fn thinking_counts_up_and_a_notice_mid_stream_does_not_take_the_stream() {
        let mut s = Session::new("", "medium");
        let ev = |e: Value| json!({"type": "stream_event", "event": e});
        s.apply(&ev(json!({"type": "message_start"})), 1.0);
        s.apply(&ev(json!({"type": "content_block_start", "content_block": {"type": "thinking"}})), 2.0);
        for est in [50, 100, 150, 100] {
            s.apply(&ev(json!({"type": "content_block_delta", "delta": {"type": "thinking_delta", "estimated_tokens": est}})), 3.0);
        }
        s.apply(&json!({"type": "harness_info", "text": "copied 12 chars"}), 4.0);
        s.apply(&ev(json!({"type": "content_block_delta", "delta": {"type": "thinking_delta", "estimated_tokens": 200}})), 5.0);
        s.apply(&ev(json!({"type": "content_block_stop"})), 6.0);
        s.apply(&ev(json!({"type": "content_block_start", "content_block": {"type": "text"}})), 7.0);
        s.apply(&ev(json!({"type": "content_block_delta", "delta": {"type": "text_delta", "text": "Hel"}})), 8.0);
        s.apply(&json!({"type": "harness_info", "text": "copied 3 chars"}), 9.0);
        s.apply(&ev(json!({"type": "content_block_delta", "delta": {"type": "text_delta", "text": "lo"}})), 10.0);
        let thinking = s.blocks.iter().find_map(|b| if let Block::Thinking { tokens, live } = b { Some((*tokens, *live)) } else { None });
        assert_eq!(thinking, Some((600, false)), "50+100+150+100+200, closed by its stop");
        // while live, the count runs from the last total to the new one over a second (eased), not in one jump
        let mut t = Session::new("", "medium");
        t.apply(&ev(json!({"type": "content_block_start", "content_block": {"type": "thinking"}})), 0.0);
        t.apply(&ev(json!({"type": "content_block_delta", "delta": {"type": "thinking_delta", "estimated_tokens": 100}})), 1000.0);
        t.apply(&ev(json!({"type": "content_block_delta", "delta": {"type": "thinking_delta", "estimated_tokens": 200}})), 2000.0);
        assert_eq!([2000.0, 2250.0, 2500.0, 3000.0, 4000.0].map(|ms| t.thinking_shown(300, ms)), [100, 187, 250, 300, 300]);
        let text = s.blocks.iter().find_map(|b| if let Block::Text { text, .. } = b { Some(text.clone()) } else { None });
        assert_eq!(text.as_deref(), Some("Hello"), "the text after the notice is not lost");
    }

    #[test]
    fn keeps_subagent_messages_under_their_call() {
        use serde_json::json;
        let mut s = Session::new("", "medium");
        s.apply(&json!({"type": "stream_event", "event": {"type": "content_block_start", "content_block": {"type": "tool_use", "id": "t1", "name": "Task"}}}), 1.0);
        let blocks = s.blocks.len();
        let sub = json!({"type": "assistant", "parent_tool_use_id": "t1", "message": {"content": [{"type": "tool_use", "id": "x", "name": "Read", "input": {"file_path": "src/a.ts"}}]}});
        s.apply(&sub, 2.0);
        s.apply(&json!({"type": "user", "parent_tool_use_id": "t1", "message": {"content": [{"type": "tool_result", "tool_use_id": "x", "content": "..."}]}}), 3.0);
        assert_eq!(s.blocks.len(), blocks, "nothing added to the main transcript");
        let Block::Tool(t) = s.blocks.last().unwrap() else { panic!() };
        assert_eq!(t.sub, Some((1, "Read src/a.ts".to_string())));
    }

    #[test]
    fn counts_cache_misses() {
        let mut s = Session::new("", "medium");
        let u = |cache_write, cache_read| Usage { input: 10, cache_write, cache_read, output: 50 };
        s.turn_usage = vec![u(9000, 0), u(200, 9000), u(100, 9200), u(9500, 0), u(0, 9600)];
        assert_eq!(s.cache_misses(), 1, "only the fourth turn lost the cache");
        assert_eq!(Session::new("", "medium").cache_misses(), 0);
    }

    #[test]
    fn replays_recorded_run() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/recordings/caddy-opus-medium.jsonl");
        let mut r = crate::replay::Replay::load(std::path::Path::new(path), false).unwrap();
        let mut s = Session::new(&r.prompt, &r.effort);
        for (t, e) in r.take(usize::MAX) {
            s.apply(e, *t);
        }
        let tools: Vec<&Tool> = s.blocks.iter().filter_map(|b| if let Block::Tool(t) = b { Some(t) } else { None }).collect();
        assert_eq!(tools.len(), 15);
        assert!(tools.iter().all(|t| t.result.is_some() && !t.input.is_null()));
        assert_eq!(s.turns, 16);
        assert_eq!(s.turn_usage.len(), 16);
        assert_eq!(s.turn_usage.iter().map(|u| u.output).sum::<u64>(), s.usage.output);
        assert_eq!(s.context_window, Some(1_000_000));
        assert!(tools.iter().all(|t| t.secs().unwrap() >= 0.0));
        assert!(matches!(s.blocks.last(), Some(Block::Done(d)) if d.ok));
        assert!((s.usage.estimate() - s.exact_cost.unwrap()).abs() / s.exact_cost.unwrap() < 0.1, "estimate within 10%");
    }

    #[test]
    fn cost_splits_into_money_and_plan_and_limits_show_the_session_share() {
        use serde_json::json;
        let mut s = Session::new("", "");
        s.apply(&json!({"type": "rate_limit_event", "rate_limit_info": {"unifiedWindows": {"five_hour": {"utilization": 0.01}, "seven_day": {"utilization": 0.05}}}}), 0.0);
        s.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 2.0}), 1.0); // claude on the plan
        s.apply(&json!({"type": "harness_switch", "model": "deepseek-v4.1-flash-1017", "effort": "high", "text": "model: deepseek"}), 2.0);
        s.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 0.1, "tazuna_paid": true}), 3.0);
        s.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 0.3, "tazuna_paid": true}), 4.0); // cumulative
        s.apply(&json!({"type": "rate_limit_event", "rate_limit_info": {"unifiedWindows": {"five_hour": {"utilization": 0.04}, "seven_day": {"utilization": 0.06}}}}), 5.0);
        let (paid, plan) = s.split_cost();
        assert!((paid - 0.3).abs() < 1e-9 && (plan - 2.0).abs() < 1e-9, "{paid} {plan}");
        assert!((s.exact_cost.unwrap() - 2.3).abs() < 1e-9, "the switch closed the first process's cost");
        assert_eq!((s.limits, s.limits_start), (Some((4.0, 6.0)), Some((1.0, 5.0))));
        assert!(!s.estimated);
        // working time: message to result, finished runs plus the one in progress; the user's idle time left out
        let mut w = Session::new("", "");
        w.apply(&json!({"type": "harness_user", "text": "a"}), 60_000.0);
        assert_eq!(w.busy(90_000.0), 30_000.0, "a run in progress counts up");
        w.apply(&json!({"type": "result", "subtype": "success", "duration_ms": 45_000, "total_cost_usd": 0.1}), 105_000.0);
        w.apply(&json!({"type": "harness_user", "text": "b"}), 3_600_000.0);
        w.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 0.2}), 3_620_000.0);
        assert_eq!(w.busy(7_200_000.0), 65_000.0);
        assert_eq!(crate::ui::clock_str(3_725_000.0), "1:02:05");
        // a stopped turn: claude reports $0 for 21k tokens; they are priced at list rates instead
        let mut s = Session::new("", "");
        s.apply(&json!({"type": "stream_event", "event": {"type": "message_delta", "usage": {"input_tokens": 2, "cache_creation_input_tokens": 8067,
            "cache_read_input_tokens": 13295, "output_tokens": 4}}}), 1.0);
        s.apply(&json!({"type": "result", "subtype": "error_during_execution", "total_cost_usd": 0, "terminal_reason": "aborted_streaming"}), 2.0);
        assert!(s.estimated && (s.split_cost().1 - (2.0 * 4.0 + 8067.0 * 6.0 + 13295.0 * 0.4 + 4.0 * 20.0) / 1e6).abs() < 1e-9, "{:?}", s.split_cost());
        assert_eq!(crate::ui::cost_str(&s), "≈$0.05 on plan");
        // the last plan reading of an earlier session, a window past its reset counting from 0
        let dir = std::env::temp_dir().join(format!("tazuna-limits-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let now = unix_now();
        let ev = |h5: f64, reset: u64| json!({"t": 5000, "e": {"type": "rate_limit_event", "rate_limit_info": {"unifiedWindows": {
            "five_hour": {"utilization": h5, "resetsAt": reset}, "seven_day": {"utilization": 0.14, "resetsAt": now + 9999}}}}}).to_string();
        std::fs::write(dir.join(format!("{}-a-1.jsonl", now - 7200)), format!("{{}}\n{}\n", ev(0.5, now + 99))).unwrap();
        std::fs::write(dir.join(format!("{}-b-2.jsonl", now - 600)), format!("{{}}\n{}\n", ev(0.12, now - 1))).unwrap();
        std::fs::write(dir.join(format!("{}-c-3.jsonl", now - 60)), "{}\n").unwrap();
        let (h5, d7, at) = last_limits(&dir).unwrap();
        assert_eq!((h5, (d7 * 10.0).round(), at), (0.0, 140.0, now - 600 + 5), "the newest reading; its 5-hour window has reset");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn cost_adds_up_across_restarted_processes() {
        use serde_json::json;
        let mut s = Session::new("", "low");
        s.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 0.01}), 1.0);
        s.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 0.03}), 2.0); // same process: cumulative
        s.apply(&json!({"type": "harness_notice", "text": "stopped"}), 3.0);
        s.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 0.005}), 4.0); // resumed process restarts at $0
        assert!((s.cost() - 0.035).abs() < 1e-9, "{}", s.cost());
    }
}
