//! Session state built from Claude Code `stream-json` events.

use serde_json::Value;

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
    pub context_window: u64,
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
            context_window: 1_000_000,
            now_ms: 0.0,
            model: String::new(),
            effort: effort.to_string(),
            cwd: String::new(),
            usage: Usage::default(),
            turns: 0,
            activity: if prompt.trim().is_empty() { Activity::Ready } else { Activity::Starting },
            exact_cost: None,
            cost_base: 0.0,
            usage_at_result: Usage::default(),
            stopping: false,
            new_blocks: 1,
            finished_tools: 0,
            results: 0,
            profile: String::new(),
            mode: "auto".into(),
            commands: vec!["good".into(), "bad".into()],
            todos: Vec::new(),
            last_prompt: String::new(),
        }
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
        self.blocks.push(b);
        self.meta.push(Meta { at_ms: self.now_ms, turn: self.turns });
        self.new_blocks += 1;
    }

    fn end_live(&mut self) {
        if let Some(Block::Text { live, .. } | Block::Thinking { live, .. }) = self.blocks.last_mut() {
            *live = false;
        }
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
                cmds.extend(["good".to_string(), "bad".to_string()]);
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
            (Some("harness_user"), _) => {
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
                self.cost_base = self.exact_cost.unwrap_or(self.cost_base);
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
            (Some("harness_info"), _) => self.push(Block::Notice { text: s(&e["text"]), error: false }),
            (Some("harness_notice"), _) => {
                self.stopping = false;
                self.push(Block::Notice { text: s(&e["text"]), error: e["error"].as_bool().unwrap_or(false) });
                // the process ended: its reported cost is final, the next one starts from $0
                self.cost_base = self.exact_cost.unwrap_or(self.cost_base);
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
                // Full message: tool inputs arrive complete here (partial JSON is not worth rendering).
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
                if let Some(c) = e["total_cost_usd"].as_f64() {
                    self.exact_cost = Some(self.cost_base + c);
                    self.usage_at_result = self.usage;
                }
                if let Some(w) = e["modelUsage"].as_object().and_then(|m| m.values().next()).and_then(|m| m["contextWindow"].as_u64()) {
                    self.context_window = w;
                }
                let ok = e["subtype"] == "success" && !e["is_error"].as_bool().unwrap_or(false);
                let secs = e["duration_ms"].as_f64().unwrap_or(0.0) / 1000.0;
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
                self.turns += 1;
                self.turn_usage.push(Usage::default());
            }
            Some("content_block_start") => {
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
                match (d["type"].as_str(), self.blocks.last_mut()) {
                    (Some("text_delta"), Some(Block::Text { text, .. })) => text.push_str(d["text"].as_str().unwrap_or_default()),
                    (Some("thinking_delta"), Some(Block::Thinking { tokens, .. })) => {
                        *tokens = d["estimated_tokens"].as_u64().unwrap_or(*tokens)
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
        assert_eq!(s.context_window, 1_000_000);
        assert!(tools.iter().all(|t| t.secs().unwrap() >= 0.0));
        assert!(matches!(s.blocks.last(), Some(Block::Done(d)) if d.ok));
        assert!((s.usage.estimate() - s.exact_cost.unwrap()).abs() / s.exact_cost.unwrap() < 0.1, "estimate within 10%");
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
