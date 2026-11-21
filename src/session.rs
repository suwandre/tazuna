//! Session state built from Claude Code `stream-json` events.

use serde_json::{Value, json};

pub enum Block {
    Prompt(String),
    Text { text: String, live: bool },
    /// `exact`: the count claude reported when the call ended; before that (and in the own loop) an estimate.
    /// `text`: the reasoning itself where the model shows it (the own loop's providers; claude's stream sends none).
    Thinking { tokens: u64, live: bool, exact: bool, text: String },
    Tool(Tool),
    Done(Summary),
    /// Harness messages: stopped runs, process exits.
    Notice { text: String, error: bool },
    /// tazuna running the tests after claude said it was done.
    Verify(Verify),
    /// A model call the provider refused, tried again by the own loop: one block for all its tries. Each try was a
    /// notice line of its own, cut mid-sentence, under a status that still said "thinking" (design review, 2026-11-19).
    Retry(Retry),
}

pub struct Retry {
    /// What failed, in words; `raw`: the provider's own text, shown when the block is opened.
    pub why: String,
    pub raw: String,
    /// Tries that failed so far.
    pub tries: u32,
    /// When the next try starts (ms into the session); None once the retrying is over.
    pub next_ms: Option<f64>,
    /// The last try failed as well: the run ended on this error.
    pub gave_up: bool,
}

pub struct Verify {
    pub cmd: String,
    /// The runner found no tests.
    pub no_tests: bool,
    /// The tests outran the time limit: no result.
    pub timed_out: bool,
    /// The test command is not on this machine (the shell's exit 127): no result.
    pub cannot_run: bool,
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
    /// A call the user said no to. Its result is the sentence written for the model ("The user declined this. Ask what
    /// they want instead…"), which the screen showed in red as an error and counted as a failure (design review 11).
    pub fn declined(&self) -> bool {
        self.error && self.result.as_deref().is_some_and(|r| r.starts_with("The user declined this"))
    }

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
    /// This run's own cost and tokens: the totals less the previous run's. Under each answer these are what it took; the
    /// totals were shown there until 0.1.183 ("$12.13, 4.8M tokens" under a 31 s reply, design review 6).
    pub run_cost: f64,
    pub run_usage: Usage,
}

/// One model call's timing: how long from its input being ready (the prompt sent, the last tool's result in) to its first
/// streamed token, how long it then generated, and its output tokens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CallTime {
    pub ttft_ms: Option<f64>,
    pub gen_ms: f64,
    pub out: u64,
    /// The whole call, from its input being ready to its end: what end to end counts (the wait included).
    pub total_ms: f64,
}

/// (mean time to first token in ms, output tokens a second while writing, output tokens a second end to end) over
/// `calls`; None where unknown. Writing: the output over the time after the first token, of the calls where that can be
/// measured (`gen_ms` > 0). End to end: all output over all the calls' whole time, the wait included (how fast answers
/// reach you; every call has it).
pub fn speed(calls: &[CallTime]) -> (Option<f64>, Option<f64>, Option<f64>) {
    let firsts: Vec<f64> = calls.iter().filter_map(|c| c.ttft_ms).collect();
    let ttft = (!firsts.is_empty()).then(|| firsts.iter().sum::<f64>() / firsts.len() as f64);
    let rate = |cs: Vec<&CallTime>, ms: fn(&CallTime) -> f64| {
        let t: f64 = cs.iter().map(|c| ms(c)).sum();
        (t > 0.0).then(|| cs.iter().map(|c| c.out).sum::<u64>() as f64 / t * 1000.0)
    };
    let tps = rate(calls.iter().filter(|c| c.gen_ms > 0.0).collect(), |c| c.gen_ms);
    let e2e = rate(calls.iter().filter(|c| c.total_ms > 0.0 && c.out > 0).collect(), |c| c.total_ms);
    (ttft, tps, e2e)
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
#[derive(Clone, Copy, PartialEq, Debug)]
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
    /// The own loop's model prices per million (uncached input, cached input, output), from its init event.
    pub price_m: Option<(f64, f64, f64)>,
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
    /// When the 5-hour window resets (Unix seconds), as the last reading said.
    pub limits_reset: Option<u64>,
    /// SubRouter's balance in USD, where a SubRouter key is set.
    pub balance: Option<f64>,
    /// Part of the cost is priced at list rates: a stopped turn reported $0.
    pub estimated: bool,
    /// Time the agent spent working (message to its last token), summed over finished runs; `run_since`: when the run
    /// in progress began (ms into the session).
    busy_ms: f64,
    run_since: Option<f64>,
    /// Every model call's timing, in order; the one in flight: when it started, when its first token came, its output so
    /// far (estimated: thinking as claude estimates it, text and tool input at ~4 characters a token) and its thinking
    /// blocks, which get the exact count when it ends.
    pub calls: Vec<CallTime>,
    /// Where each model call began: the number of blocks then, by call (turn_usage's order). The desktop puts each call
    /// under its reply, with the tools it called.
    call_blocks: Vec<usize>,
    call_start: Option<f64>,
    first_token: Option<f64>,
    /// The own loop's provider is sending keep-alive comments and no output yet (cleared when the call answers or ends).
    pub waiting_note: Option<String>,
    /// The retry block of the call being tried again now (its place in `blocks`); None once it answered or the run ended.
    retry: Option<usize>,
    /// Each subagent's own messages, by the call that started it: ctrl-b opens one as a conversation of its own.
    pub sub_events: std::collections::HashMap<String, Vec<(f64, Value)>>,
    call_est: f64,
    /// How many pieces the call in flight has streamed (a speed needs a few, not one handed over at the end).
    call_deltas: u32,
    call_thinking: Vec<usize>,
    /// When the last call started (ms into the session) and the cache life claude wrote it with (5 minutes or 1 hour, from
    /// its usage): each call that reads the cache renews it, counted from the request's start (Anthropic's docs: a reply
    /// that streams for 4 minutes leaves a 5-minute cache 1 minute). None on the own loop's providers.
    pub last_call_ms: Option<f64>,
    pub cache_ttl_ms: Option<f64>,
    /// When the next call's input was ready: the prompt sent or a tool's result in. Claude's "requesting" note comes when
    /// the answer is already streaming (15 ms before the first token after a 2 s pause, 2026-11-03), too late to time it.
    ready_at: Option<f64>,
    usage_at_result: Usage,
    /// An interrupt was requested; the next result ends a stopped run.
    stopping: bool,
    /// The open permission questions, oldest first, each as its tool and input: the screen marks the call the front one
    /// is about (with two calls in one message it marked the newest, and the question was about the first).
    pub asked: std::collections::VecDeque<(String, Value)>,
    /// Esc during verification: the run ends when the tests return, as stopped (the status said a green "✓ done").
    pub verify_cancelled: bool,
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
    pub last_prompt: String,
    /// The current model message arrived as content blocks (claude streams them; the own loop sends whole messages).
    streamed: bool,
    /// This call's text came as a stream (its message then adds no text block of its own).
    streamed_text: bool,
}

pub fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// The newest plan reading in the session recordings under `dir` (claude's rate_limit_event): (5-hour %, 7-day %, read
/// at, when the 5-hour window resets). A window whose reset time has passed since counts from 0.
/// A run's own share of the session's totals: `cost` and `usage` less those at the previous run's end. Totals that went
/// down (a new conversation on the same screen) start again: the run's share is all of them.
fn run_share(blocks: &[Block], cost: f64, usage: Usage) -> (f64, Usage) {
    let Some((pc, pu)) = blocks.iter().rev().find_map(|b| if let Block::Done(d) = b { Some((d.cost, d.usage)) } else { None }) else { return (cost, usage) };
    let down = cost + 1e-9 < pc || usage.input < pu.input || usage.cache_read < pu.cache_read || usage.output < pu.output || usage.cache_write < pu.cache_write;
    if down {
        return (cost, usage);
    }
    (cost - pc, Usage { input: usage.input - pu.input, cache_write: usage.cache_write - pu.cache_write, cache_read: usage.cache_read - pu.cache_read, output: usage.output - pu.output })
}

pub fn last_limits(dir: &std::path::Path) -> Option<(f64, f64, u64, Option<u64>)> {
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
        let reset = w["five_hour"]["resetsAt"].as_u64().filter(|r| *r >= now);
        return Some((pct("five_hour")?, pct("seven_day")?, began + d["t"].as_u64().unwrap_or(0) / 1000, reset));
    }
    None
}

/// tazuna's own slash commands, beside claude's in a session's list. Most are the terminal's (panels, switches): the
/// window lists only the ones a sent message runs (main.rs, "complete").
pub const OWN_COMMANDS: [&str; 28] = ["agent", "good", "bad", "boss", "changelog", "chat", "create", "data", "diff", "effort", "explorer", "hook", "jira", "keys", "login", "loop", "mcp", "model", "new", "preset", "profile", "resume", "review", "route", "verify", "rewind", "usage", "vim"];

impl Session {
    pub fn new(prompt: &str, effort: &str) -> Self {
        Session {
            // live sessions start empty; their messages arrive as `harness_user` events
            blocks: if prompt.trim().is_empty() { vec![] } else { vec![Block::Prompt(prompt.trim().to_string())] },
            meta: if prompt.trim().is_empty() { vec![] } else { vec![Meta { at_ms: 0.0, turn: 0 }] },
            turn_usage: Vec::new(),
            price_m: None,
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
            limits_reset: None,
            balance: None,
            estimated: false,
            busy_ms: 0.0,
            run_since: None,
            calls: vec![],
            call_blocks: vec![],
            call_start: None,
            first_token: None,
            sub_events: Default::default(),
            waiting_note: None,
            retry: None,
            call_est: 0.0,
            call_deltas: 0,
            call_thinking: vec![],
            last_call_ms: None,
            cache_ttl_ms: None,
            ready_at: None,
            usage_at_result: Usage::default(),
            stopping: false,
            asked: Default::default(),
            verify_cancelled: false,
            new_blocks: 1,
            finished_tools: 0,
            results: 0,
            profile: String::new(),
            mode: "auto".into(),
            commands: vec!["agent".into(), "bad".into(), "boss".into(), "changelog".into(), "chat".into(), "create".into(), "data".into(), "diff".into(), "effort".into(), "explorer".into(), "good".into(), "hook".into(), "jira".into(), "keys".into(), "login".into(), "loop".into(), "mcp".into(), "model".into(), "new".into(), "preset".into(), "profile".into(), "resume".into(), "review".into(), "route".into(), "rewind".into(), "usage".into(), "verify".into(), "vim".into()],
            todos: Vec::new(),
            last_prompt: String::new(),
            streamed: false,
            streamed_text: false,
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

    /// The latest call's share of its input read from the cache (Claude Code's cache meters show this one).
    pub fn last_call_hit(&self) -> Option<f64> {
        self.turn_usage.last().filter(|u| u.total_in() > 0).map(|u| u.cache_read as f64 / u.total_in() as f64)
    }

    /// Milliseconds the cache stays warm after `now_ms`, from the latest call's start (the one in flight renewed it), None
    /// when unknown.
    /// What Claude re-reads without its cache after a model or effort switch, or once the cache has expired: the whole
    /// context, written again at the cache-write price (1.25x input for a 5-minute cache, 2x for an hour). None when
    /// it is too small to mention, or on the own loop. In the user's sessions (2026-11-10) the first call after
    /// `/effort` read 13k from the cache and wrote 24k again: only the fixed start of the prompt survived.
    pub fn rebuild(&self) -> Option<(u64, Option<f64>)> {
        if self.context < 50_000 || self.price_m.is_some() {
            return None;
        }
        // ponytail: list prices per M input (2026-11), a model not in the list gets the tokens without a price
        let m = self.model.as_str();
        let input = if m.contains("opus") { Some(4.0) } else if m.contains("sonnet") { Some(2.0) } else if m.contains("haiku") { Some(0.10) } else if m.contains("fable") { Some(8.0) } else { None };
        let write = if self.cache_ttl_ms.is_some_and(|t| t < 3_600_000.0) { 1.25 } else { 2.0 };
        Some((self.context, input.map(|p| self.context as f64 / 1e6 * p * write)))
    }

    pub fn cache_left(&self, now_ms: f64) -> Option<f64> {
        let ttl = self.cache_ttl_ms?;
        Some((self.call_start.or(self.last_call_ms)? + ttl - now_ms).max(0.0))
    }

    /// The call in flight: when it started, when its first token came, its output so far (estimated). None between calls.
    pub fn in_flight(&self) -> Option<(f64, Option<f64>, f64)> {
        self.call_start.map(|start| (start, self.first_token, self.call_est))
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

    /// The session as the desktop app draws it (`tazuna app-server`, 2026-11-15): blocks and the numbers around them,
    /// from the same state the terminal UI renders. An explicit shape rather than serde on the internals, so the app's
    /// protocol stays stable when this struct changes. ponytail: a full snapshot each time; diffs when long threads lag.
    pub fn view(&self) -> Value {
        fn usage(u: &Usage) -> Value {
            json!({"input": u.input, "cache_write": u.cache_write, "cache_read": u.cache_read, "output": u.output})
        }
        fn cut(t: &str, n: usize) -> String {
            if t.chars().count() > n { format!("{}…", t.chars().take(n).collect::<String>()) } else { t.to_string() }
        }
        let blocks: Vec<Value> = self.blocks.iter().map(|b| match b {
            Block::Prompt(text) => json!({"kind": "prompt", "text": text}),
            Block::Text { text, live } => json!({"kind": "text", "text": text, "live": live}),
            Block::Thinking { tokens, live, exact, text } => json!({"kind": "thinking", "tokens": tokens, "live": live, "exact": exact, "text": text.chars().take(20_000).collect::<String>()}),
            Block::Tool(t) => json!({"kind": "tool", "id": t.id, "name": t.name, "input": t.input, "result": t.result.as_deref().map(|r| cut(r, 4000)),
                "error": t.error, "declined": t.declined(), "secs": t.secs(), "running": t.done_ms.is_none(), "stopped": t.done_ms.is_some() && t.result.is_none(), "sub": t.sub.as_ref().map(|(n, last)| json!({"calls": n, "last": last}))}),
            Block::Done(d) => json!({"kind": "done", "ok": d.ok, "secs": d.secs, "turns": d.turns, "cost": d.cost, "usage": usage(&d.usage), "stopped": d.stopped,
                "run_cost": d.run_cost, "run_usage": usage(&d.run_usage)}),
            Block::Notice { text, error } => json!({"kind": "notice", "text": text, "error": error}),
            // the app has no block of its own for it: a notice, an error once the run ended on it
            Block::Retry(r) => json!({"kind": "notice", "error": r.gave_up, "text": format!("{}: {} {} failed{} ({})", r.why, r.tries,
                if r.tries == 1 { "try" } else { "tries" }, if r.next_ms.is_some() { ", trying again" } else { "" }, r.raw)}),
            Block::Verify(v) => json!({"kind": "verify", "cmd": v.cmd, "ok": v.ok, "secs": v.secs, "round": v.round, "no_tests": v.no_tests,
                "timed_out": v.timed_out, "cannot_run": v.cannot_run, "output": cut(&v.output, 4000)}),
        }).collect();
        json!({"type": "view", "blocks": blocks, "model": self.model, "effort": self.effort, "cwd": self.cwd, "profile": self.profile,
            "mode": self.mode, "activity": format!("{:?}", self.activity), "cost": self.cost(), "usage": usage(&self.usage), "turns": self.turns,
            "context": self.context, "context_window": self.context_window, "limits": self.limits, "limits_at": self.limits_at, "limits_reset": self.limits_reset, "balance": self.balance,
            "calls": self.calls_view()})
    }

    /// Each model call for the desktop's per-turn view: where it began (a block index), its tokens (new input: sent
    /// uncached or written into the cache), its timing as /calls shows it (None: in flight, or not measured) and its
    /// cost at this model's prices.
    fn calls_view(&self) -> Vec<Value> {
        self.turn_usage.iter().zip(&self.call_blocks).enumerate().map(|(i, (u, &block))| {
            let c = self.calls.get(i);
            json!({"block": block, "fresh": u.input + u.cache_write, "cached": u.cache_read, "out": u.output, "cost": self.estimate(u),
                "first_ms": c.and_then(|c| c.ttft_ms), "tps": c.filter(|c| c.gen_ms > 0.0).map(|c| c.out as f64 / c.gen_ms * 1000.0),
                "total_ms": c.map(|c| c.total_ms)})
        }).collect()
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
        self.exact_cost.unwrap_or(0.0) + self.estimate(&since)
    }

    /// Usage at this session's model's prices when the own loop reported them (per million: uncached input, cached
    /// input, output), else at Claude's list prices. A live own-loop turn was priced like Claude: ~$3 shown while a
    /// Luna turn ran, $0.007 when it ended (2026-11-06).
    pub fn estimate(&self, u: &Usage) -> f64 {
        match self.price_m {
            Some((i, c, o)) => ((u.input + u.cache_write) as f64 * i + u.cache_read as f64 * c + u.output as f64 * o) / 1e6,
            None => u.estimate(),
        }
    }

    fn push(&mut self, b: Block) {
        self.end_live();
        self.end_retry();
        self.push_aside(b);
    }

    /// The call that was tried again answered, or the run ended: its block stops counting down.
    fn end_retry(&mut self) {
        if let Some(Block::Retry(r)) = self.retry.take().and_then(|i| self.blocks.get_mut(i)) {
            r.next_ms = None;
        }
    }

    /// Seconds until the own loop tries a refused model call again; None while no call is being retried.
    pub fn retry_in(&self, now_ms: f64) -> Option<f64> {
        match self.retry.and_then(|i| self.blocks.get(i)) {
            Some(Block::Retry(r)) => r.next_ms.map(|t| ((t - now_ms) / 1000.0).max(0.0)),
            _ => None,
        }
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
    /// The user's own MCP servers (tazuna's list) that did not connect. claude.ai connectors waiting for a login stay
    /// quiet: claude.ai lists those, and an account has several.
    fn mcp_failures(&mut self, e: &Value) {
        let ours: Vec<String> = crate::mcpc::servers().into_iter().map(|s| s.0).collect();
        for m in e["mcp_servers"].as_array().into_iter().flatten().filter(|m| matches!(m["status"].as_str(), Some("failed" | "needs-auth"))) {
            let name = m["name"].as_str().unwrap_or_default().to_string();
            if ours.contains(&name) {
                let why = m["error"].as_str().map_or_else(|| format!("did not connect (/mcp login {name} if it needs a login)"), String::from);
                self.push_aside(Block::Notice { text: format!("MCP {name}: {why}"), error: true });
            }
        }
    }

    pub fn apply(&mut self, e: &Value, at_ms: f64) {
        self.now_ms = at_ms;
        if let Some(parent) = e["parent_tool_use_id"].as_str() {
            // a subagent's own messages: they belong under the call that started it, not in the main transcript
            self.sub_events.entry(parent.to_string()).or_default().push((at_ms, e.clone()));
            self.subagent(parent, e);
            return;
        }
        let s = |v: &Value| v.as_str().unwrap_or_default().to_string();
        match (e["type"].as_str(), e["subtype"].as_str()) {
            (Some("system"), Some("init")) => {
                self.model = s(&e["model"]);
                self.price_m = e["price_per_m"].as_array().filter(|p| p.len() == 3).map(|p| (p[0].as_f64().unwrap_or(0.0), p[1].as_f64().unwrap_or(0.0), p[2].as_f64().unwrap_or(0.0)));
                self.cwd = s(&e["cwd"]);
                let mut cmds: Vec<String> = e["slash_commands"].as_array().into_iter().flatten().filter_map(|c| c.as_str().map(String::from)).collect();
                cmds.extend(OWN_COMMANDS.map(String::from));
                cmds.sort();
                cmds.dedup();
                self.commands = cmds;
                self.activity = Activity::Waiting;
                self.mcp_failures(e);
            }
            (Some("system"), Some("mcp_status")) => self.mcp_failures(e),
            (Some("system"), Some("call_waiting")) => self.waiting_note = e["message"].as_str().map(String::from),
            (Some("system"), Some("status")) => {
                // review mode runs as acceptEdits underneath: claude's report of that does not rename it
                if let Some(m) = e["permissionMode"].as_str() && !(self.mode == "review" && m == "acceptEdits") {
                    self.mode = mode_name(m).to_string();
                }
                self.activity = Activity::Waiting;
            }
            (Some("system"), Some("compact_boundary")) => {
                self.push(Block::Notice { text: "── context compacted: earlier turns are now a summary ──".into(), error: false });
            }
            // a refused call that is tried again ("HTTP 429: … · trying again in 8s"): the block of the call's earlier
            // tries is updated, a first try starts one
            (Some("system"), Some("call_retry")) if e["message"].is_string() => {
                let msg = s(&e["message"]);
                let (raw, secs) = msg.rsplit_once(" · trying again in ").map_or((msg.as_str(), 0.0), |(w, t)| (w, t.trim_end_matches('s').parse().unwrap_or(0.0)));
                let (why, raw, next_ms) = (retry_words(raw), shown(raw, 4000), Some(at_ms + secs * 1000.0));
                match self.retry.and_then(|i| self.blocks.get_mut(i)) {
                    Some(Block::Retry(r)) => (r.why, r.raw, r.next_ms, r.tries) = (why, raw, next_ms, r.tries + 1),
                    _ => {
                        self.push(Block::Retry(Retry { why, raw, tries: 1, next_ms, gave_up: false }));
                        self.retry = Some(self.blocks.len() - 1);
                    }
                }
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
                    "resume_failed" => "session",
                    _ => return,
                };
                // whole (the transcript cuts it until clicked), a known provider refusal said in plain words: a route switch
                // read "HTTP 403: 请求内容未通过安全检查…" and could not be opened (2026-11-06)
                let msg = s(&e["message"]);
                let plain = failure_reason(&msg);
                let text = if plain != msg.trim().chars().take(200).collect::<String>() {
                    format!("{} · {plain} ({})", msg.split(": HTTP").next().unwrap_or_default(), msg.trim())
                } else {
                    msg
                };
                self.push(Block::Notice { text: format!("{what} · {}", shown(&text, 4000)), error: false });
            }
            (Some("harness_user"), _) => {
                self.run_since.get_or_insert(at_ms);
                self.ready_at = Some(at_ms);
                self.last_prompt = s(&e["text"]);
                self.push(Block::Prompt(s(&e["text"])));
                self.activity = Activity::Waiting;
                self.verify_cancelled = false;
            }
            (Some("harness_interrupt"), _) => self.stopping = true,
            (Some("harness_verify_start"), _) => {
                self.push(Block::Verify(Verify { cmd: s(&e["cmd"]), no_tests: false, timed_out: false, cannot_run: false, round: e["round"].as_u64().unwrap_or(1), ok: None, secs: 0.0, output: String::new() }));
                self.activity = Activity::Verifying;
            }
            (Some("harness_verify_done"), _) => {
                if let Some(v) = self.blocks.iter_mut().rev().find_map(|b| if let Block::Verify(v) = b { Some(v) } else { None }) {
                    (v.ok, v.secs, v.output) = (e["ok"].as_bool(), e["secs"].as_f64().unwrap_or(0.0), s(&e["output"]));
                    v.no_tests = e["no_tests"] == true;
                    v.timed_out = e["timed_out"] == true;
                    v.cannot_run = e["cannot_run"] == true;
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
            (Some("harness_profile"), _) => {
                self.profile = s(&e["profile"]);
                self.push(Block::Notice { text: s(&e["text"]), error: false });
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
                self.verify_cancelled = true;
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
                self.asked.push_back((s(&e["tool"]), e["input"].clone()));
                // the question itself is drawn from live's queue; the transcript only gets the plan to approve
                if e["tool"] == "ExitPlanMode" {
                    let plan: String = e["input"]["plan"].as_str().unwrap_or_default().chars().map(|c| if invisible(c) || (c.is_control() && c != '\n') { '�' } else { c }).collect();
                    self.push(Block::Text { text: format!("**Plan**\n\n{plan}"), live: false });
                }
                self.activity = Activity::Waiting;
            }
            (Some("harness_permission_cleared"), _) => {
                self.asked.clear();
                self.push(Block::Notice { text: "open permission questions dropped: claude stopped".into(), error: false });
            }
            (Some("harness_permission_done"), _) => {
                self.asked.pop_front();
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
                // shift-tab through the modes changes one line, not one line a press (design review 4)
                match self.blocks.last_mut() {
                    Some(Block::Notice { text, .. }) if text.starts_with("mode: ") => *text = format!("mode: {}", self.mode),
                    _ => self.push(Block::Notice { text: format!("mode: {}", self.mode), error: false }),
                }
            }
            (Some("harness_info"), _) => self.push_aside(Block::Notice { text: s(&e["text"]), error: false }),
            // the own loop's reasoning before a message: a thinking row like claude's, its size estimated (~4 characters a token)
            (Some("reasoning"), _) => self.push(Block::Thinking { tokens: (s(&e["text"]).chars().count() / 4) as u64, live: false, exact: false, text: s(&e["text"]) }),
            // claude's thinking estimate, about once a second (50-150 tokens, then the rest when it ends); the exact count
            // comes with the call's end. Its stream's thinking_delta carries the same steps rounded to 50.
            (Some("system"), Some("thinking_tokens")) => {
                let n = e["estimated_tokens_delta"].as_u64().unwrap_or(0);
                self.call_est += n as f64;
                if let Some(&i) = self.call_thinking.last()
                    && let Some(Block::Thinking { tokens, .. }) = self.blocks.get_mut(i)
                {
                    self.think_from = (*tokens, at_ms);
                    *tokens += n;
                }
            }
            // a replayed recording's process ended there (`/resume` replays several, without their notices)
            (Some("harness_process_end"), _) => self.close_process(),
            (Some("harness_effort"), _) => {
                self.effort = s(&e["effort"]);
                if let Some(m) = e["model"].as_str() {
                    self.model = m.to_string(); // a level that is its own id (OmniRoute's)
                }
                self.push(Block::Notice { text: s(&e["text"]), error: false });
            }
            (Some("harness_switch"), _) => {
                // /model: the old process is gone
                self.close_process();
                (self.model, self.effort) = (s(&e["model"]), s(&e["effort"]));
                self.context_window = None; // the next model reports its own (the own loop's: none)
                self.cache_ttl_ms = None;
                self.push(Block::Notice { text: s(&e["text"]), error: false });
            }
            (Some("rate_limit_event"), _) => {
                let w = &e["rate_limit_info"]["unifiedWindows"];
                if let (Some(h5), Some(d7)) = (w["five_hour"]["utilization"].as_f64(), w["seven_day"]["utilization"].as_f64()) {
                    self.limits = Some((h5 * 100.0, d7 * 100.0));
                    self.limits_start.get_or_insert((h5 * 100.0, d7 * 100.0));
                    self.limits_at = Some(unix_now());
                    self.limits_reset = w["five_hour"]["resetsAt"].as_u64();
                }
            }
            (Some("harness_notice"), _) => {
                self.stopping = false;
                // a killed process ends its call without message_delta or result: "waiting for first token" kept counting
                (self.call_start, self.first_token) = (None, None);
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
                // since 0.1.98 the own loop streams its text and reasoning but not its tool calls: those still come from
                // here, and its text too when none was streamed
                for c in e["message"]["content"].as_array().into_iter().flatten() {
                    match c["type"].as_str() {
                        Some("text") if !self.streamed_text && !s(&c["text"]).trim().is_empty() => self.push(Block::Text { text: s(&c["text"]), live: false }),
                        Some("tool_use") if !self.streamed || self.tool_mut(&s(&c["id"])).is_none() => {
                            self.stream(&json!({"type": "content_block_start", "content_block": c}))
                        }
                        _ => {}
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
                    self.ready_at = Some(at_ms);
                    if let Some(t) = self.tool_mut(&s(&c["tool_use_id"])) {
                        // a command that ran and failed is a failure on screen too: the lean tool server reports it as
                        // text alone ("[exit 1]", which every provider's model reads the same), and the step was ticked
                        // green (design review, 2026-11-19)
                        t.error = error || (t.name.to_lowercase().ends_with("bash") && exit_line(&text).is_some());
                        t.result = Some(text);
                        t.done_ms = Some(at_ms);
                        self.finished_tools += 1;
                    }
                }
                self.activity = Activity::Waiting;
            }
            (Some("result"), _) => {
                self.waiting_note = None;
                self.end_live();
                // a call still open when the run ends was cut off (a stop, an error) and gets no result: it ends here, as
                // stopped. It kept its spinner in the terminal and went on counting seconds in the window: "Asked a
                // subagent 502 s" under a reply stopped 1.4 s after the call began (the user, 2026-11-20)
                for b in &mut self.blocks {
                    if let Block::Tool(t) = b && t.done_ms.is_none() {
                        t.done_ms = Some(at_ms);
                    }
                }
                (self.call_start, self.first_token) = (None, None); // a stopped call never ends with message_delta
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
                        c = prev + self.estimate(&since);
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
                // why it failed, in the transcript: the result carried it but only "Failed" showed (a provider's content
                // filter refused a question, 2026-11-05)
                if !ok && !stopped {
                    if let Some(why) = e["result"].as_str().filter(|r| !r.trim().is_empty()) {
                        // the call that was being tried again failed for good: its block says so, not a second one
                        match self.retry.and_then(|i| self.blocks.get_mut(i)) {
                            Some(Block::Retry(r)) => (r.why, r.raw, r.tries, r.gave_up) = (failure_reason(why), shown(why, 4000), r.tries + 1, true),
                            _ => self.push(Block::Notice { text: failure_reason(why), error: true }),
                        }
                    }
                }
                let (cost, usage) = (self.cost(), self.usage);
                let (run_cost, run_usage) = run_share(&self.blocks, cost, usage);
                self.push(Block::Done(Summary { ok, secs, turns, cost, usage, stopped, run_cost, run_usage }));
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
                self.waiting_note = None;
                (self.streamed, self.streamed_text) = (false, false);
                self.turns += 1;
                // the own loop sends this when its request goes out, right after the tool results
                self.call_start = Some(self.ready_at.take().unwrap_or(self.now_ms));
                (self.first_token, self.call_est, self.call_deltas) = (None, 0.0, 0);
                self.call_thinking.clear();
                self.turn_usage.push(Usage::default());
                self.call_blocks.push(self.blocks.len());
            }
            Some("content_block_start") => {
                self.streamed = true;
                let b = &ev["content_block"];
                match b["type"].as_str() {
                    Some("text") => {
                        self.push(Block::Text { text: String::new(), live: true });
                        self.activity = Activity::Writing;
                        self.streamed_text = true;
                    }
                    Some("thinking") => {
                        self.push(Block::Thinking { tokens: 0, live: true, exact: false, text: String::new() });
                        self.call_thinking.push(self.blocks.len() - 1);
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
                self.first_token.get_or_insert(self.now_ms);
                self.call_deltas += 1;
                self.waiting_note = None;
                // reasoning that streams (DeepSeek's thinking) is output as well: without it, a call whose reasoning was
                // shown read as reasoning kept back, and its speed counted the wait before the first token (0.1.137-0.1.147:
                // 88 tok/s shown for 169 on a DeepSeek V4.1 Flash call, 2026-11-16)
                self.call_est += d["text"].as_str().or(d["partial_json"].as_str()).or(d["thinking"].as_str()).map_or(0, str::len) as f64 / 4.0;
                if let (Some("text_delta"), Some(Block::Text { text, .. })) = (d["type"].as_str(), self.live_block()) {
                    text.push_str(d["text"].as_str().unwrap_or_default());
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
                // the cache's life: claude splits what it wrote by 5-minute and 1-hour entries (a call that wrote
                // nothing keeps the last one known)
                let cc = if u["cache_creation"].is_object() { &u["cache_creation"] } else { &u["iterations"][0]["cache_creation"] };
                if cc["ephemeral_1h_input_tokens"].as_u64().unwrap_or(0) > 0 {
                    self.cache_ttl_ms = Some(3_600_000.0);
                } else if cc["ephemeral_5m_input_tokens"].as_u64().unwrap_or(0) > 0 {
                    self.cache_ttl_ms = Some(300_000.0);
                }
                self.last_call_ms = self.call_start.or(Some(self.now_ms));
                // the exact thinking count, shared among the call's thinking blocks by their estimates
                let thinking = std::mem::take(&mut self.call_thinking);
                if let Some(exact) = u["output_tokens_details"]["thinking_tokens"].as_u64() {
                    let est = |b: Option<&Block>| if let Some(Block::Thinking { tokens, .. }) = b { *tokens } else { 0 };
                    let sum: u64 = thinking.iter().map(|&i| est(self.blocks.get(i))).sum();
                    let n = thinking.len() as u64;
                    for &i in &thinking {
                        if let Some(Block::Thinking { tokens, exact: x, .. }) = self.blocks.get_mut(i) {
                            (*tokens, *x) = (if sum > 0 { exact * *tokens / sum } else { exact / n }, true);
                        }
                    }
                }
                if let Some(start) = self.call_start.take() {
                    // the speed is the output over the time after the first token, only where the stream shows the model
                    // writing (all recorded calls of every model, 2026-11-16); otherwise the call has none (gen_ms 0)
                    // rather than a figure that takes the wait in:
                    // - nothing streamed: the first token's time is unknown (unstreamed providers, the own loop before
                    //   streaming);
                    // - under 20 tokens, under 5 streamed pieces or under half a second after the first: too short to
                    //   time (two short calls on Mistral read 545 tok/s, the model's long ones far less);
                    // - faster than 1,000 tok/s: handed over in a burst after it was written (claude: 66 deltas in 57 ms;
                    //   providers buffering); no tested model streams near that, and its first token came late too;
                    // - a model other than Claude whose output mostly never streamed: reasoning kept back before the first
                    //   visible token (ERNIE: 102 tokens over 53 ms, 20 s in). Claude's thinking streams as a summary
                    //   while it thinks (with thinking 124 tok/s, without 155, Opus 5.5 over 1,691 calls), so it counts;
                    // - a count far below what streamed: not this call's usage (8 tokens reported for ~1,500 streamed).
                    let first = self.first_token.take();
                    let after = first.map_or(0.0, |f| self.now_ms - f);
                    let out = t.output as f64;
                    let burst = after > 0.0 && out / after > 1.0;
                    let kept_back = !self.model.starts_with("claude-") && self.call_est < 0.5 * out;
                    let measured = first.is_some() && t.output >= 20 && self.call_deltas >= 5 && after >= 500.0 && !burst && !kept_back
                        && self.call_est <= 2.0 * out + 50.0;
                    self.calls.push(CallTime { ttft_ms: first.filter(|_| !burst).map(|f| f - start), gen_ms: if measured { after } else { 0.0 }, out: t.output,
                        total_ms: self.now_ms - start });
                }
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

/// A failed turn's reason in a line: known provider refusals in plain words, anything else cut to its start.
pub fn failure_reason(result: &str) -> String {
    let code = ["content_policy_violation", "insufficient_quota", "model_not_found", "rate_limit"].into_iter().find(|c| result.contains(c));
    match code {
        Some("content_policy_violation") => "the provider's content filter refused this message (its policy, not tazuna's); another provider or other words may go through".into(),
        Some("insufficient_quota") => "the provider account has no balance left".into(),
        Some("model_not_found") => "no provider serves this model on the route right now (it may have dropped out)".into(),
        Some(_) => "rate limited by the provider: try again in a moment".into(),
        None => result.trim().trim_start_matches('[').trim_end_matches(']').chars().take(200).collect(),
    }
}

/// What a refused model call that is tried again says: the usual refusals in words, else the provider's own.
fn retry_words(why: &str) -> String {
    let status = why.split("HTTP ").nth(1).and_then(|r| r.get(..3)).and_then(|c| c.parse::<u16>().ok());
    match status {
        Some(429) => "the provider is rate limiting this model".into(),
        Some(c @ 500..) => format!("the provider failed (HTTP {c})"),
        _ if why.contains("timeout") || why.contains("timed out") => "the provider did not answer in time".into(),
        _ => failure_reason(why),
    }
}

/// The last line of a command's output when it says the command failed: "[exit 1]", "[timed out after 120s; …]" (the
/// lean tool server ends a failed command's output with one).
pub fn exit_line(output: &str) -> Option<&str> {
    output.trim_end().lines().last().filter(|l| l.starts_with("[exit ") || l.starts_with("[timed out after "))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every recorded call of every model, replayed through this code: the speed it shows against output over the time
    /// after the first token, and calls no real model could make (over 1,500 tok/s, or under 2). Reads the user's
    /// recordings: `cargo test --release audit_speeds -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn audit_speeds() {
        let dir = crate::labels::dir().unwrap().join("sessions");
        // model -> (calls, shown out, shown ms, formula out, formula ms, kept-back calls, burst calls, odd calls)
        let mut by: std::collections::BTreeMap<String, (u32, u64, f64, u64, f64, u32, u32, Vec<String>)> = Default::default();
        for f in std::fs::read_dir(&dir).unwrap().flatten() {
            let Ok(text) = std::fs::read_to_string(f.path()) else { continue };
            let mut s = Session::new("", "medium");
            for l in text.lines() {
                let Ok(d) = serde_json::from_str::<Value>(l) else { continue };
                let (t, e) = (d["t"].as_f64().unwrap_or(0.0), &d["e"]);
                let before = (s.call_start, s.first_token, s.call_est, s.calls.len());
                s.apply(e, t);
                if s.calls.len() > before.3 {
                    let c = *s.calls.last().unwrap();
                    let (Some(start), first) = (before.0, before.1) else { continue };
                    let m = by.entry(s.model.clone()).or_default();
                    m.0 += 1;
                    if c.out == 0 || c.gen_ms <= 0.0 {
                        continue;
                    }
                    (m.1, m.2) = (m.1 + c.out, m.2 + c.gen_ms);
                    let after_first = first.map(|f| t - f);
                    if let Some(g) = after_first.filter(|g| *g > 0.0) {
                        (m.3, m.4) = (m.3 + c.out, m.4 + g);
                    }
                    let kept = first.is_some() && (c.gen_ms - (t - start)).abs() < 1.0 && c.ttft_ms.is_some();
                    let burst = first.is_some() && c.ttft_ms.is_none();
                    m.5 += kept as u32;
                    m.6 += burst as u32;
                    let shown = c.out as f64 / c.gen_ms * 1000.0;
                    if (shown > 1500.0 || shown < 2.0) && m.7.len() < 3 {
                        m.7.push(format!("{} tok/{:.1}s={shown:.0} (first after {:.1}s, streamed ~{:.0})", c.out, c.gen_ms / 1000.0,
                            first.map_or(f64::NAN, |f| (f - start) / 1000.0), before.2));
                    }
                }
            }
        }
        println!("{:<44} {:>6} {:>9} {:>9} {:>6} {:>6}  odd", "model", "calls", "shown", "formula", "kept", "burst");
        for (model, m) in &by {
            let rate = |o: u64, ms: f64| if ms > 0.0 { format!("{:.0}", o as f64 / ms * 1000.0) } else { "-".into() };
            println!("{:<44} {:>6} {:>9} {:>9} {:>6} {:>6}  {}", model.chars().take(44).collect::<String>(), m.0, rate(m.1, m.2), rate(m.3, m.4), m.5, m.6, m.7.join(" | "));
        }
    }

    #[test]
    fn permission_text_cannot_hide_anything() {
        assert_eq!(shown("ls\nrm -rf ~", 100), "ls⏎rm -rf ~");
        assert_eq!(shown("ok\u{1b}[2K\rfake", 100), "ok�[2K�fake");
        assert_eq!(shown(&"x".repeat(10), 4), "xxxx… (+6 chars)");
        assert_eq!(shown("echo hi\u{202E}fr- mr", 100), "echo hi�fr- mr", "a right-to-left override cannot reverse what is shown");
        assert_eq!(shown("a\u{200B}b\u{2028}c", 100), "a�b�c");
    }

    #[test]
    fn a_call_cut_off_by_a_stop_ends_with_the_run() {
        let mut s = Session::new("", "medium");
        s.apply(&json!({"type": "assistant", "message": {"content": [{"type": "tool_use", "id": "done", "name": "Bash", "input": {"command": "x"}}]}}), 1000.0);
        s.apply(&json!({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": "done", "content": "ok", "is_error": false}]}}), 2000.0);
        s.apply(&json!({"type": "stream_event", "event": {"type": "content_block_start", "content_block": {"type": "tool_use", "id": "cut", "name": "Agent"}}}), 3000.0);
        let state = |s: &Session| -> Vec<(bool, bool, Option<f64>)> { s.view()["blocks"].as_array().unwrap().iter().filter(|b| b["kind"] == "tool")
            .map(|b| (b["running"] == true, b["stopped"] == true, b["secs"].as_f64())).collect() };
        assert_eq!(state(&s), [(false, false, Some(1.0)), (true, false, None)], "before the stop it runs");
        s.apply(&json!({"type": "result", "subtype": "error_during_execution", "is_error": true, "result": ""}), 4400.0);
        assert_eq!(state(&s), [(false, false, Some(1.0)), (false, true, Some(1.4))], "the run ended: the open call ends with it, as stopped, at its 1.4 s");
    }

    #[test]
    fn a_failed_command_and_a_call_given_up_read_as_failures() {
        let mut s = Session::new("", "medium");
        let run = |s: &mut Session, id: &str, out: &str| {
            s.apply(&json!({"type": "assistant", "message": {"content": [{"type": "tool_use", "id": id, "name": "bash", "input": {"command": "x"}}]}}), 1.0);
            s.apply(&json!({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": id, "content": out, "is_error": false}]}}), 2.0);
        };
        run(&mut s, "c1", "cat: nope.txt: No such file or directory\n\n[exit 1]");
        run(&mut s, "c2", "the log says [exit 1] somewhere\nok");
        let errors: Vec<bool> = s.blocks.iter().filter_map(|b| if let Block::Tool(t) = b { Some(t.error) } else { None }).collect();
        assert_eq!(errors, [true, false], "the exit code on the output's last line decides");
        // the last try fails too: the retry block carries the end, no second error under it
        s.apply(&json!({"type": "system", "subtype": "call_retry", "message": "HTTP 503: down · trying again in 2s"}), 3.0);
        s.apply(&json!({"type": "result", "subtype": "error_during_execution", "is_error": true, "result": "[provider error: HTTP 503: down]"}), 4.0);
        let n = s.blocks.len();
        let (Block::Retry(r), Block::Done(d)) = (&s.blocks[n - 2], &s.blocks[n - 1]) else { panic!() };
        assert!(r.gave_up && r.tries == 2 && r.next_ms.is_none() && !d.ok, "{} {}", r.why, r.tries);
    }

    #[test]
    fn shows_the_own_loops_provider_events() {
        let mut s = Session::new("", "medium");
        s.apply(&json!({"type": "system", "subtype": "conversation_moved", "message": "responses_source_unavailable"}), 1.0);
        let Block::Notice { text, .. } = s.blocks.last().unwrap() else { panic!() };
        assert_eq!(text, "conversation moved to another pool, without its reasoning · responses_source_unavailable");
        // a call tried again is said, not hidden behind "waiting for first token": one block for all its tries
        s.apply(&json!({"type": "system", "subtype": "call_retry", "message": "HTTP 429: busy · trying again in 30s"}), 1.5);
        s.apply(&json!({"type": "system", "subtype": "call_retry", "message": "HTTP 429: busy · trying again in 60s"}), 2000.0);
        let Block::Retry(r) = s.blocks.last().unwrap() else { panic!() };
        assert_eq!((r.why.as_str(), r.raw.as_str(), r.tries, r.next_ms), ("the provider is rate limiting this model", "HTTP 429: busy", 2, Some(62_000.0)));
        assert_eq!(s.retry_in(52_000.0), Some(10.0));
        let n = s.blocks.len();
        s.apply(&json!({"type": "system", "subtype": "hook_response", "message": "x"}), 2.0);
        assert_eq!(s.blocks.len(), n, "other system events stay out of the transcript");
        // the own loop sends whole messages, no content blocks: its text and tool calls are drawn from them, once
        s.apply(&json!({"type": "stream_event", "event": {"type": "message_start"}}), 3.0);
        s.apply(&json!({"type": "reasoning", "text": "Ambiguous; likely run the tests."}), 3.5);
        assert_eq!(s.retry_in(4.0), None, "the call answered: no retry is going on");
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
        let think = |n: u64| json!({"type": "system", "subtype": "thinking_tokens", "estimated_tokens_delta": n});
        s.apply(&ev(json!({"type": "message_start"})), 1.0);
        s.apply(&ev(json!({"type": "content_block_start", "content_block": {"type": "thinking"}})), 2.0);
        for n in [50, 100, 150, 100] {
            s.apply(&think(n), 3.0);
        }
        s.apply(&json!({"type": "harness_info", "text": "copied 12 chars"}), 4.0);
        s.apply(&think(150), 5.0);
        s.apply(&ev(json!({"type": "content_block_stop"})), 6.0);
        s.apply(&think(75), 6.5); // claude's last estimate can come after the block's stop
        s.apply(&ev(json!({"type": "content_block_start", "content_block": {"type": "text"}})), 7.0);
        s.apply(&ev(json!({"type": "content_block_delta", "delta": {"type": "text_delta", "text": "Hel"}})), 8.0);
        s.apply(&json!({"type": "harness_info", "text": "copied 3 chars"}), 9.0);
        s.apply(&ev(json!({"type": "content_block_delta", "delta": {"type": "text_delta", "text": "lo"}})), 10.0);
        let thinking = |s: &Session| s.blocks.iter().find_map(|b| if let Block::Thinking { tokens, live, exact, .. } = b { Some((*tokens, *live, *exact)) } else { None });
        assert_eq!(thinking(&s), Some((625, false, false)), "50+100+150+100+150+75, closed by its stop");
        // the call's end brings the exact count
        s.apply(&ev(json!({"type": "message_delta", "usage": {"output_tokens": 700, "output_tokens_details": {"thinking_tokens": 478}}})), 11.0);
        assert_eq!(thinking(&s), Some((478, false, true)));
        let calls = s.view()["calls"].clone();
        assert_eq!((calls[0]["block"].as_u64(), calls[0]["out"].as_u64()), (Some(0), Some(700)), "each call in the view: where it began, its tokens");
        // while live, the count runs from the last total to the new one over a second (eased), not in one jump
        let mut t = Session::new("", "medium");
        t.apply(&ev(json!({"type": "message_start"})), 0.0);
        t.apply(&ev(json!({"type": "content_block_start", "content_block": {"type": "thinking"}})), 0.0);
        t.apply(&think(100), 1000.0);
        t.apply(&think(200), 2000.0);
        assert_eq!([2000.0, 2250.0, 2500.0, 3000.0, 4000.0].map(|ms| t.thinking_shown(300, ms)), [100, 187, 250, 300, 300]);
        // a call's timing: from its input being ready (the prompt, a tool's result) to its first streamed token; claude's
        // "requesting" note comes too late to count
        let mut c = Session::new("", "medium");
        c.apply(&json!({"type": "harness_user", "text": "go"}), 600.0);
        c.apply(&json!({"type": "system", "subtype": "status", "status": "requesting"}), 1000.0);
        c.apply(&ev(json!({"type": "message_start"})), 1010.0);
        assert_eq!(c.in_flight(), Some((600.0, None, 0.0)), "waiting for the first token");
        c.apply(&ev(json!({"type": "content_block_start", "content_block": {"type": "text"}})), 2000.0);
        c.apply(&ev(json!({"type": "content_block_delta", "delta": {"type": "text_delta", "text": "12345678"}})), 2000.0);
        assert_eq!(c.in_flight(), Some((600.0, Some(2000.0), 2.0)), "8 characters: ~2 tokens so far");
        for at in [3000.0, 4000.0, 5000.0, 6900.0] {
            c.apply(&ev(json!({"type": "content_block_delta", "delta": {"type": "text_delta", "text": "x".repeat(358)}})), at);
        }
        c.apply(&ev(json!({"type": "message_delta", "usage": {"input_tokens": 10, "output_tokens": 360}})), 7000.0);
        assert_eq!(c.in_flight(), None);
        // the own loop, unstreamed: its message_start goes out with the request, and no token streams
        c.apply(&json!({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": "x", "content": "ok"}]}}), 7900.0);
        c.apply(&ev(json!({"type": "message_start"})), 8000.0);
        c.apply(&ev(json!({"type": "message_delta", "usage": {"input_tokens": 10, "output_tokens": 100}})), 10100.0);
        assert_eq!(c.calls, [CallTime { ttft_ms: Some(1400.0), gen_ms: 5000.0, out: 360, total_ms: 6400.0 }, CallTime { ttft_ms: None, gen_ms: 0.0, out: 100, total_ms: 2200.0 }]);
        let (first, rate, e2e) = speed(&c.calls);
        assert_eq!((first, rate.map(|r| r.round())), (Some(1400.0), Some(72.0)), "360 tokens over the 5 s after the first; the unstreamed call has no speed");
        assert_eq!(e2e.map(|r| r.round()), Some(53.0), "end to end: all 460 tokens over both calls' 8.6 s, the waits included");
        // a reply claude hands over in one burst: neither its first token's time nor its speed is known
        let mut b = Session::new("", "medium");
        b.model = "claude-opus-5-5".into();
        b.apply(&json!({"type": "harness_user", "text": "go"}), 0.0);
        b.apply(&ev(json!({"type": "message_start"})), 3900.0);
        b.apply(&ev(json!({"type": "content_block_delta", "delta": {"type": "text_delta", "text": "x"}})), 3900.0);
        b.apply(&ev(json!({"type": "message_delta", "usage": {"output_tokens": 380}})), 3957.0);
        assert_eq!(b.calls, [CallTime { ttft_ms: None, gen_ms: 0.0, out: 380, total_ms: 3957.0 }]);
        assert_eq!(speed(&b.calls).1, None);
        // reasoning a provider keeps back: 102 tokens, ~10 of them streamed in 53 ms after 20 s of nothing; no speed, not
        // "1933 tok/s", and handed over in a burst, so no time to first token either
        let mut h = Session::new("", "high");
        h.model = "ernie-5-5-flash-preview".into();
        h.apply(&json!({"type": "harness_user", "text": "go"}), 0.0);
        h.apply(&ev(json!({"type": "message_start"})), 0.0);
        h.apply(&ev(json!({"type": "content_block_start", "content_block": {"type": "text"}})), 20_000.0);
        h.apply(&ev(json!({"type": "content_block_delta", "delta": {"type": "text_delta", "text": "x".repeat(40)}})), 20_000.0);
        h.apply(&ev(json!({"type": "message_delta", "usage": {"output_tokens": 102}})), 20_053.0);
        assert_eq!(h.calls, [CallTime { ttft_ms: None, gen_ms: 0.0, out: 102, total_ms: 20_053.0 }]);
        assert_eq!((speed(&h.calls).0, speed(&h.calls).1), (None, None));
        assert_eq!(speed(&h.calls).2.map(|r| r.round()), Some(5.0), "end to end it is known: 102 tokens over 20 s");
        // kept back without a burst (a few visible tokens over a while): no speed; the wait to the first one stands
        let mut k2 = Session::new("", "high");
        k2.model = "deepseek-v4-1-flash-1017".into();
        k2.apply(&json!({"type": "harness_user", "text": "go"}), 0.0);
        k2.apply(&ev(json!({"type": "message_start"})), 0.0);
        k2.apply(&ev(json!({"type": "content_block_delta", "delta": {"type": "text_delta", "text": "x".repeat(400)}})), 9_000.0);
        k2.apply(&ev(json!({"type": "message_delta", "usage": {"output_tokens": 900}})), 10_200.0);
        assert_eq!(k2.calls, [CallTime { ttft_ms: Some(9_000.0), gen_ms: 0.0, out: 900, total_ms: 10_200.0 }], "750 tok/s on 100 visible tokens: reasoning kept back");
        // two pieces over 0.1 s (Mistral, 2026-11-16: read 960 tok/s): too short to time, whatever the rate
        let mut sh = Session::new("", "high");
        sh.model = "deepseek-v4-1-flash-1017".into();
        sh.apply(&json!({"type": "harness_user", "text": "go"}), 0.0);
        sh.apply(&ev(json!({"type": "message_start"})), 0.0);
        sh.apply(&ev(json!({"type": "content_block_delta", "delta": {"type": "text_delta", "text": "x".repeat(240)}})), 1_000.0);
        sh.apply(&ev(json!({"type": "content_block_delta", "delta": {"type": "text_delta", "text": "x".repeat(240)}})), 1_050.0);
        sh.apply(&ev(json!({"type": "message_delta", "usage": {"output_tokens": 120}})), 1_125.0);
        assert_eq!(sh.calls[0].gen_ms, 0.0);
        // reasoning that streams counts as output: the speed is output over the time after the first token
        let mut d = Session::new("", "high");
        d.model = "deepseek-v4.1-flash-1017-fast".into();
        d.apply(&json!({"type": "harness_user", "text": "go"}), 0.0);
        d.apply(&ev(json!({"type": "message_start"})), 0.0);
        d.apply(&ev(json!({"type": "content_block_start", "content_block": {"type": "thinking"}})), 2_400.0);
        for at in [2_400.0, 2_900.0, 3_400.0] {
            d.apply(&ev(json!({"type": "content_block_delta", "delta": {"type": "thinking_delta", "thinking": "t".repeat(341)}})), at);
        }
        d.apply(&ev(json!({"type": "content_block_start", "content_block": {"type": "text"}})), 4_000.0);
        for at in [4_000.0, 4_500.0] {
            d.apply(&ev(json!({"type": "content_block_delta", "delta": {"type": "text_delta", "text": "x".repeat(446)}})), at);
        }
        d.apply(&ev(json!({"type": "message_delta", "usage": {"output_tokens": 451}})), 5_100.0);
        assert_eq!(d.calls, [CallTime { ttft_ms: Some(2_400.0), gen_ms: 2_700.0, out: 451, total_ms: 5_100.0 }]);
        assert_eq!(speed(&d.calls).1.map(|r| r.round()), Some(167.0), "451 tokens over the 2.7 s after the first one, not 88 over 5.1 s");
        // Claude's thinking streams as a summary (a third of its output) while it thinks: timed from the first token
        let mut o = Session::new("", "high");
        o.model = "claude-opus-5-5".into();
        o.apply(&json!({"type": "harness_user", "text": "go"}), 0.0);
        o.apply(&ev(json!({"type": "message_start"})), 0.0);
        for (at, kind, n) in [(1_500.0, "thinking", 400), (3_000.0, "thinking", 400), (6_000.0, "text", 200), (7_000.0, "text", 100), (8_000.0, "text", 100)] {
            o.apply(&ev(json!({"type": "content_block_delta", "delta": {"type": format!("{kind}_delta"), kind: "x".repeat(n)}})), at);
        }
        o.apply(&ev(json!({"type": "message_delta", "usage": {"output_tokens": 900}})), 8_500.0);
        assert_eq!(o.calls, [CallTime { ttft_ms: Some(1_500.0), gen_ms: 7_000.0, out: 900, total_ms: 8_500.0 }]);
        // too short to time, and a count that is not this call's (8 reported for ~1,500 streamed): no speed
        let mut q = Session::new("", "high");
        q.model = "claude-fable-5-2".into();
        for (out, chars) in [(4u64, 16usize), (8, 6_000)] {
            q.apply(&json!({"type": "harness_user", "text": "go"}), 0.0);
            q.apply(&ev(json!({"type": "message_start"})), 0.0);
            q.apply(&ev(json!({"type": "content_block_delta", "delta": {"type": "text_delta", "text": "x".repeat(chars)}})), 1_000.0);
            q.apply(&ev(json!({"type": "message_delta", "usage": {"output_tokens": out}})), 4_000.0);
        }
        assert_eq!(speed(&q.calls).1, None);
        // the cache's life from what the call wrote, renewed by each call; the last call's cached share
        let mut k = Session::new("", "medium");
        k.apply(&ev(json!({"type": "message_start"})), 0.0);
        k.apply(&ev(json!({"type": "message_delta", "usage": {"input_tokens": 2, "cache_read_input_tokens": 90762, "cache_creation_input_tokens": 1486,
            "iterations": [{"cache_creation": {"ephemeral_5m_input_tokens": 0, "ephemeral_1h_input_tokens": 1486}}]}})), 1000.0);
        assert_eq!((k.cache_ttl_ms, k.cache_left(61_000.0), k.cache_left(4_000_000.0)), (Some(3_600_000.0), Some(3_539_000.0), Some(0.0)), "from the call's start");
        // a rebuild's price: the whole context written again, at twice Opus 5.5's input price for an hour's cache
        let mut r = Session::new("", "medium");
        (r.model, r.context, r.cache_ttl_ms) = ("claude-opus-5-5".into(), 514_000, Some(3_600_000.0));
        let (t, usd) = r.rebuild().unwrap();
        assert!(t == 514_000 && (usd.unwrap() - 4.112).abs() < 0.001, "{usd:?}");
        r.context = 20_000;
        assert!(r.rebuild().is_none(), "too small to mention");
        (r.context, r.price_m) = (514_000, Some((0.02, 0.002, 0.07)));
        assert!(r.rebuild().is_none(), "the own loop's providers keep their cache across an effort change");
        assert_eq!(k.last_call_hit().map(|h| (h * 1000.0).round()), Some(984.0));
        k.apply(&ev(json!({"type": "message_start"})), 5_000_000.0);
        assert_eq!(k.cache_left(5_000_500.0), Some(3_599_500.0), "a call in flight renews it");
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
        assert_eq!(crate::ui::cost_str(&s), "≈$0.05 on Claude plan");
        // the last plan reading of an earlier session, a window past its reset counting from 0
        let dir = std::env::temp_dir().join(format!("tazuna-limits-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let now = unix_now();
        let ev = |h5: f64, reset: u64| json!({"t": 5000, "e": {"type": "rate_limit_event", "rate_limit_info": {"unifiedWindows": {
            "five_hour": {"utilization": h5, "resetsAt": reset}, "seven_day": {"utilization": 0.14, "resetsAt": now + 9999}}}}}).to_string();
        std::fs::write(dir.join(format!("{}-a-1.jsonl", now - 7200)), format!("{{}}\n{}\n", ev(0.5, now + 99))).unwrap();
        std::fs::write(dir.join(format!("{}-b-2.jsonl", now - 600)), format!("{{}}\n{}\n", ev(0.12, now - 1))).unwrap();
        std::fs::write(dir.join(format!("{}-c-3.jsonl", now - 60)), "{}\n").unwrap();
        let (h5, d7, at, _) = last_limits(&dir).unwrap();
        assert_eq!((h5, (d7 * 10.0).round(), at), (0.0, 140.0, now - 600 + 5), "the newest reading; its 5-hour window has reset");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_failed_turn_says_why() {
        let msg = r#"[provider error: HTTP 403: {"error":{"message":"请求内容未通过安全检查。","type":"content_policy_violation","code":"content_policy_violation"}}]"#;
        assert!(super::failure_reason(msg).starts_with("the provider's content filter refused"));
        assert_eq!(super::failure_reason("[provider error: HTTP 500: boom]"), "provider error: HTTP 500: boom");
    }

    #[test]
    fn a_running_own_loop_turn_costs_its_models_prices() {
        let mut s = Session::new("", "high");
        s.apply(&json!({"type": "system", "subtype": "init", "model": "deepseek-v4.1-flash-1017", "price_per_m": [0.15, 0.003, 0.6]}), 0.0);
        s.apply(&json!({"type": "stream_event", "event": {"type": "message_delta", "usage": {"input_tokens": 10000, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 90000, "output_tokens": 2000}}}), 10.0);
        assert!((s.cost() - 0.00297).abs() < 1e-6, "{} not at Claude's prices", s.cost());
        let mut c = Session::new("", "medium");
        c.apply(&json!({"type": "system", "subtype": "init", "model": "claude-opus-5-5"}), 0.0);
        assert!(c.price_m.is_none(), "claude sessions keep the list prices until claude reports the cost");
    }

    #[test]
    fn cost_adds_up_across_restarted_processes() {
        use serde_json::json;
        let mut s = Session::new("", "low");
        s.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 0.01}), 1.0);
        s.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 0.03}), 2.0); // same process: cumulative
        s.apply(&json!({"type": "stream_event", "event": {"type": "message_start"}}), 2.5);
        s.apply(&json!({"type": "harness_notice", "text": "stopped"}), 3.0);
        assert!(s.in_flight().is_none(), "a killed call stops its clock");
        s.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 0.005}), 4.0); // resumed process restarts at $0
        assert!((s.cost() - 0.035).abs() < 1e-9, "{}", s.cost());
        // under each answer its own share, not the total so far: $0.01, $0.02, $0.005 (design review 6)
        let own: Vec<f64> = s.blocks.iter().filter_map(|b| if let Block::Done(d) = b { Some(d.run_cost) } else { None }).collect();
        assert_eq!(own.len(), 3);
        assert!((own[0] - 0.01).abs() < 1e-9 && (own[1] - 0.02).abs() < 1e-9 && (own[2] - 0.005).abs() < 1e-9, "{own:?}");
        // `/resume` replays: each recording's process starts again at $0; a slash command (no tokens) kept claude's
        // figure for its process alone: $55.63 on screen, then $17.98 after /mcp (2026-11-06)
        let mut r = Session::new("", "low");
        r.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 32.5}), 1.0);
        r.apply(&json!({"type": "harness_process_end"}), 2.0);
        r.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 18.0}), 3.0);
        r.apply(&json!({"type": "result", "subtype": "success", "num_turns": 0, "total_cost_usd": 18.0}), 4.0);
        assert!((r.cost() - 50.5).abs() < 1e-9, "{}", r.cost());
        assert!(!r.blocks.iter().any(|b| matches!(b, Block::Notice { .. })), "the end of a process shows nothing");
    }
}
