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
    /// `ended`: the run ended on it (a message that could not start): that message's turn is over.
    Notice { text: String, error: bool, ended: bool },
    /// tazuna running the tests after claude said it was done.
    Verify(Verify),
    /// A model call the provider refused, tried again by the own loop: one block for all its tries. Each try was a
    /// notice line of its own, cut mid-sentence, under a status that still said "thinking" (design review, 2026-11-19).
    Retry(Retry),
    /// The first session after an upgrade: the versions it brought (versions::update_news), drawn as a block of its own.
    News(crate::versions::News),
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
    /// An edit's place in its file (`locate`): found by its old text when it was asked for, by its new text when its result
    /// came, so the screen reads no file while it draws (design review 13).
    pub at: Option<Located>,
}

/// Where an edit is in its file: the line it starts on, and the lines just above and under it.
pub type Located = (usize, Option<String>, Option<String>);

/// Where `needle` is in the file at `path`, when the file holds it exactly once (twice, the edit's place cannot be told);
/// None for anything but a regular file up to 4 MB, which is all it reads.
pub fn locate(path: &std::path::Path, needle: &str) -> Option<Located> {
    // a regular file only, and not a big one: /dev/zero, a FIFO or a tty would never end the read (on the screen's thread)
    let m = std::fs::metadata(path).ok()?;
    if needle.is_empty() || !m.is_file() || m.len() > 4 << 20 {
        return None;
    }
    let text = std::fs::read_to_string(path).ok()?;
    let mut found = text.match_indices(needle);
    let (at, _) = found.next()?;
    if found.next().is_some() {
        return None;
    }
    let line = text[..at].matches('\n').count() + 1;
    let near = |i: usize| text.lines().nth(i).map(String::from);
    Some((line, if line > 1 { near(line - 2) } else { None }, near(line - 1 + needle.lines().count())))
}

/// How many lines two texts share at their start and, after those, at their end.
pub fn common_ends(o: &[&str], n: &[&str]) -> (usize, usize) {
    let p = o.iter().zip(n).take_while(|(a, b)| a == b).count();
    (p, o[p..].iter().rev().zip(n[p..].iter().rev()).take_while(|(a, b)| a == b).count())
}

/// The lines an edit adds and removes: the lines its old and new text share at either end are context, not change.
pub fn change_counts(old: &str, new: &str) -> (u64, u64) {
    let (o, n): (Vec<&str>, Vec<&str>) = (old.lines().collect(), new.lines().collect());
    let (p, q) = common_ends(&o, &n);
    ((n.len() - p - q) as u64, (o.len() - p - q) as u64)
}

/// A file a run changed with its edit tools (the line under its answer): the lines added and removed, or written over
/// whole (a write onto a file that was there: its lines are all new, "+200" said nothing of what changed).
#[derive(Clone, Debug, PartialEq)]
pub struct FileChange {
    pub path: String,
    pub added: u64,
    pub removed: u64,
    pub rewritten: bool,
}

impl Tool {
    /// What an edit tool replaced, as (old, new) pairs, borrowed from its input: a write or a notebook cell all new. None
    /// for any other tool.
    pub fn replaced(&self) -> Option<Vec<(&str, &str)>> {
        fn s(v: &Value) -> &str {
            v.as_str().unwrap_or_default()
        }
        Some(match self.name.strip_prefix("mcp__lean__").unwrap_or(&self.name) {
            "Edit" | "edit" => vec![(crate::ui::tool_arg(self, "old"), crate::ui::tool_arg(self, "new"))],
            "MultiEdit" => self.input["edits"].as_array().into_iter().flatten().map(|e| (s(&e["old_string"]), s(&e["new_string"]))).collect(),
            "Write" | "write" => vec![("", s(&self.input["content"]))],
            "NotebookEdit" => vec![("", s(&self.input["new_source"]))],
            _ => return None,
        })
    }

    /// A call the user said no to. Its result is the sentence written for the model ("The user declined this. Ask what
    /// they want instead…"), which the screen showed in red as an error and counted as a failure (design review 11).
    pub fn declined(&self) -> bool {
        self.error && self.result.as_deref().is_some_and(|r| r.starts_with("The user declined this"))
    }

    pub fn secs(&self) -> Option<f64> {
        self.done_ms.map(|d| (d - self.started_ms) / 1000.0)
    }
}

/// A finished turn on the agent that changed files (the desktop's change card): the work tree before and after it as git
/// trees (turns.rs), its files, its `done` block (where the card goes), whether it was undone. Kept as recorded events
/// (`harness_turn`, `harness_undone`), so a conversation opened again has its cards: they lived in the app server alone,
/// and a resumed conversation said "Changes 0" beside the agent's files (review 12).
pub struct Change {
    pub before: String,
    pub after: String,
    pub files: Vec<crate::turns::File>,
    pub at_block: usize,
    pub undone: bool,
}

impl Change {
    /// The event that records it.
    #[inline(never)]
    pub fn event(before: &str, after: &str, files: &[crate::turns::File]) -> Value {
        json!({"type": "harness_turn", "before": before, "after": after,
            "files": files.iter().map(|f| json!({"path": f.path, "added": f.added, "removed": f.removed, "status": f.status.to_string()})).collect::<Vec<_>>()})
    }
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
    /// totals were shown there until 0.1.183 ("$12.13, 4.8M tokens" under a 31 s reply, design review 6). `run_cost` None:
    /// no amount can be told (a stopped turn on a Claude model without list prices in the core), and none is in the totals.
    pub run_cost: Option<f64>,
    pub run_usage: Usage,
    /// How this run was paid, as it ran; None when no model had said itself yet (a recording without an init event).
    pub bill: Option<Bill>,
    /// `run_cost` is tazuna's estimate at list prices: nobody reported it (a stopped turn: claude says $0; a hard stop or a
    /// crash: no result at all).
    pub estimate: bool,
    /// Tries the provider may have billed that are not in `run_cost`: empty replies sent again, timed-out or cut-off
    /// replies tried again (the own loop says so).
    pub uncounted: u32,
    /// Its subagents' model calls (in its cost, not among the calls), fixed when it ended; those of a background subagent
    /// that go on after it count in the run whose total they are in (they kept counting on the first, design review 12).
    pub sub_calls: usize,
    /// The files its edit tools changed, fixed when it ended: the line under the answer was worked out again every frame.
    pub changes: Vec<FileChange>,
}

/// How one run was paid, fixed when it ended: a receipt under an answer says this, not what the conversation's model of
/// now would cost (the window said "not billed" under answers a provider had billed, design review 11).
#[derive(Clone, Debug, PartialEq)]
pub struct Bill {
    pub model: String,
    /// On the Claude plan: nothing billed, the cost is at API prices.
    pub plan: bool,
    /// Who was paid (models::paid_to, models::claude_payer); None on the plan, on a machine of the user's, through a gateway
    /// claude does not name, and in recordings from before 0.1.193.
    pub provider: Option<String>,
    /// USD per million (uncached input, cached input, output) as the run was priced; None where the core has no prices.
    /// A cache write's surcharge is not in it (the own loop charges one at 1.25x input, Claude's list at 1.5x): a call's
    /// `cost` holds what it was charged.
    pub price_m: Option<(f64, f64, f64)>,
    /// The run's calls went to more than one provider (the own loop changed route inside it): their names in order, and
    /// then neither `provider` nor `price_m` stands for the whole run.
    pub providers: Vec<String>,
}

/// One model call's bill, fixed as it ran: its model, the prices and the provider in force when it began, and what the
/// own loop charged for it (None on claude and in recordings from before 0.1.193: its cost is then an estimate).
/// `unreported`: the provider sent no usage, so nobody knows the cost (it read "Paid to X $0", design review 12);
/// `uncounted`: tries before its answer the provider may have billed, not in its cost.
struct CallBill {
    model: String,
    price: Option<(f64, f64, f64)>,
    provider: Option<String>,
    cost: Option<f64>,
    unreported: bool,
    uncounted: u32,
}

/// Claude's list prices per million (uncached input, cached input, output) where the core has them: Opus 5.5's, the
/// rates of `Usage::estimate`. ponytail: one model; a table when the others' prices are in the core.
// not inlined into the event loop: a release build (lto, one codegen unit) did not finish in 15 minutes with these in `apply`
#[inline(never)]
fn claude_list(model: &str) -> Option<(f64, f64, f64)> {
    model.starts_with("claude-opus-5-5").then_some((4.0, 0.4, 20.0))
}

#[inline(never)]
fn price_of(e: &Value) -> Option<(f64, f64, f64)> {
    e["price_per_m"].as_array().filter(|p| p.len() == 3).map(|p| (p[0].as_f64().unwrap_or(0.0), p[1].as_f64().unwrap_or(0.0), p[2].as_f64().unwrap_or(0.0)))
}

/// `u` at `price` (per million: uncached input, cached input, output), else at Claude's list prices.
#[inline(never)]
fn priced(u: &Usage, price: Option<(f64, f64, f64)>) -> f64 {
    match price {
        Some((i, c, o)) => ((u.input + u.cache_write) as f64 * i + u.cache_read as f64 * c + u.output as f64 * o) / 1e6,
        None => u.estimate(),
    }
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
    /// Usage per model turn (index = turn - 1).
    pub turn_usage: Vec<Usage>,
    /// The own loop's model prices per million (uncached input, cached input, output), from its init event.
    pub price_m: Option<(f64, f64, f64)>,
    /// Who the own loop's calls are paid to, from its init event (and a route switch's); None on claude.
    pub provider: Option<String>,
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
    /// Each keyed provider's reading (accounts::credits): its name and what is left, where its API tells.
    pub credits: Vec<(String, Option<String>)>,
    /// Part of the cost is priced at list rates: a stopped turn reported $0.
    pub estimated: bool,
    /// Time the agent spent working (message to its last token, the tests), summed over finished runs; `run_since`: when
    /// the work in progress began (ms into the session), what "Working N s" counts from. Set and cleared in `clock_on` and
    /// `clock_off` alone: only a message set it, so the tests, the rounds that fix them and a message queued behind
    /// another stood at "Working 0 s", and a hard stop left it set for the next reply (design review 11).
    busy_ms: f64,
    run_since: Option<f64>,
    /// claude's runs are billed, not on the plan: an API key, Bedrock, Vertex, a gateway (models::claude_payer).
    claude_billed: bool,
    /// `call_bill`'s length at the last result: the calls from there on are the run in progress.
    billed: usize,
    /// Subagent calls already counted into a run's `sub_calls`.
    subs_counted: usize,
    /// Every model call's timing, in order; the one in flight: when it started, when its first token came, its output so
    /// far (estimated: thinking as claude estimates it, text and tool input at ~4 characters a token) and its thinking
    /// blocks, which get the exact count when it ends.
    pub calls: Vec<CallTime>,
    /// Where each model call began: the number of blocks then, by call (turn_usage's order). The desktop puts each call
    /// under its reply, with the tools it called.
    call_blocks: Vec<usize>,
    /// Each call's model and prices as it began (turn_usage's order): its cost stays what it was after a `/model` switch.
    call_bill: Vec<CallBill>,
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
    /// The totals when the transcript was last cut (`truncate`): the next run's share counts from here, not from the last
    /// Done left, which would hand it the cut part's spend.
    cut_at: Option<(f64, Usage)>,
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
    /// Permission mode in tazuna's words: auto, full, ask, edits, review, plan.
    pub mode: String,
    /// The router's choice at the start (its `harness_route` event), for its line again after a model picked before the
    /// first message.
    lane: Value,
    /// Slash commands claude offers in this session (built-ins, the repo's commands and skills), plus tazuna's own.
    pub commands: Vec<String>,
    /// Each finished turn's changed files, in order.
    pub changes: Vec<Change>,
    /// The notices of sends the app refused (`mark_rejected`), by block.
    rejected: Vec<usize>,
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
/// A run's own share of the session's totals: `cost` and `usage` less those at the previous run's end. The totals only
/// grow (a new conversation is a new Session, a /clear's restarted count is folded in): where they went down, a turn's
/// share was the whole conversation's ("$0 … the same without the cache $4.14" after /clear, design review 12).
fn run_share(blocks: &[Block], cost: f64, usage: Usage, cut: Option<(f64, Usage)>) -> (f64, Usage) {
    let Some((pc, pu)) = cut.or_else(|| blocks.iter().rev().find_map(|b| if let Block::Done(d) = b { Some((d.cost, d.usage)) } else { None })) else { return (cost, usage) };
    ((cost - pc).max(0.0), Usage { input: usage.input.saturating_sub(pu.input), cache_write: usage.cache_write.saturating_sub(pu.cache_write),
        cache_read: usage.cache_read.saturating_sub(pu.cache_read), output: usage.output.saturating_sub(pu.output) })
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
pub const OWN_COMMANDS: [&str; 30] = ["agent", "good", "bad", "boss", "changelog", "chat", "create", "data", "diff", "effort", "explorer", "hook", "jira", "keys", "login", "loop", "mcp", "mode", "model", "new", "preset", "profile", "resume", "review", "route", "verify", "rewind", "usage", "update", "vim"];

impl Session {
    pub fn new(prompt: &str, effort: &str) -> Self {
        Session {
            // live sessions start empty; their messages arrive as `harness_user` events
            blocks: if prompt.trim().is_empty() { vec![] } else { vec![Block::Prompt(prompt.trim().to_string())] },
            turn_usage: Vec::new(),
            price_m: None,
            provider: None,
            context: 0,
            // unknown (null in the view, no stand-in) until a result reports it
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
            credits: Vec::new(),
            estimated: false,
            busy_ms: 0.0,
            run_since: None,
            claude_billed: false,
            billed: 0,
            subs_counted: 0,
            calls: vec![],
            call_blocks: vec![],
            call_bill: vec![],
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
            cut_at: None,
            stopping: false,
            asked: Default::default(),
            verify_cancelled: false,
            new_blocks: 1,
            finished_tools: 0,
            results: 0,
            profile: String::new(),
            lane: Value::Null,
            mode: "auto".into(),
            commands: vec!["agent".into(), "bad".into(), "boss".into(), "changelog".into(), "chat".into(), "create".into(), "data".into(), "diff".into(), "effort".into(), "explorer".into(), "good".into(), "hook".into(), "jira".into(), "keys".into(), "login".into(), "loop".into(), "mcp".into(), "model".into(), "new".into(), "preset".into(), "profile".into(), "resume".into(), "review".into(), "route".into(), "rewind".into(), "update".into(), "usage".into(), "verify".into(), "vim".into()],
            todos: Vec::new(),
            changes: Vec::new(),
            rejected: Vec::new(),
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
        s.credits = self.credits.clone();
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

    /// How long the work in progress has been at it by `now_ms` (0 when nothing runs): since its message, the call that
    /// began it, or the tests.
    pub fn working_for(&self, now_ms: f64) -> f64 {
        self.run_since.map_or(0.0, |r| (now_ms - r).max(0.0))
    }

    /// The run in progress's output tokens so far: its finished calls' as reported, the one in flight's as estimated (the
    /// working row's "↓ 1.2k tokens", design review 13).
    pub fn run_output(&self) -> u64 {
        self.usage.output.saturating_sub(self.usage_at_result.output) + self.call_start.map_or(0, |_| self.call_est as u64)
    }

    /// Work begins at `at_ms` (a message, a call, the tests, a fix round) unless some already runs.
    #[inline(never)]
    fn clock_on(&mut self, at_ms: f64) {
        self.run_since.get_or_insert(at_ms);
    }

    /// It stopped at `at_ms` (a result, the tests' end, a hard stop or the process gone): its time joins the work done
    /// (`secs`: the process's own count of it, where it gave one).
    #[inline(never)]
    fn clock_off(&mut self, at_ms: f64, secs: f64) {
        let ran = if secs > 0.0 { secs * 1000.0 } else { self.run_since.map_or(0.0, |r| (at_ms - r).max(0.0)) };
        self.busy_ms += ran;
        self.run_since = None;
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
        let blocks: Vec<Value> = self.blocks.iter().enumerate().map(|(i, b)| match b {
            Block::Prompt(text) => json!({"kind": "prompt", "text": text}),
            Block::Text { text, live } => json!({"kind": "text", "text": if self.hidden(i) { "" } else { text }, "live": live}),
            Block::Thinking { tokens, live, exact, text } => json!({"kind": "thinking", "tokens": tokens, "live": live, "exact": exact, "text": text.chars().take(20_000).collect::<String>()}),
            Block::Tool(t) => json!({"kind": "tool", "id": t.id, "name": t.name, "input": t.input, "result": t.result.as_deref().map(|r| cut(r, 4000)),
                "error": t.error, "declined": t.declined(), "secs": t.secs(), "running": t.done_ms.is_none(), "stopped": t.done_ms.is_some() && t.result.is_none(), "sub": t.sub.as_ref().map(|(n, last)| json!({"calls": n, "last": last}))}),
            Block::Done(d) => {
                let mut v = json!({"kind": "done", "ok": d.ok, "secs": d.secs, "turns": d.turns, "cost": d.cost, "usage": usage(&d.usage), "stopped": d.stopped,
                    "run_cost": d.run_cost, "run_usage": usage(&d.run_usage)});
                // how the run was paid, where it is known: absent otherwise, and the window then says only its total
                if let Some(b) = &d.bill {
                    (v["model"], v["plan"], v["provider"], v["price_m"]) = (json!(b.model), json!(b.plan), json!(b.provider), json!(b.price_m.map(|(i, c, o)| [i, c, o])));
                    if !b.providers.is_empty() {
                        v["providers"] = json!(b.providers);
                    }
                }
                // its subagents' model calls are in its cost and not among `calls`: how many, so the window can say why
                // the rows do not add up to the total; an estimate says so, and tries not in the total are counted
                if d.sub_calls > 0 {
                    v["sub_calls"] = json!(d.sub_calls);
                }
                if d.estimate {
                    v["estimate"] = json!(true);
                }
                if d.uncounted > 0 {
                    v["uncounted"] = json!(d.uncounted);
                }
                v
            }
            // a switch that carries the conversation to another model (live.rs switch): a line where it continues
            Block::Notice { text, error, ended } => {
                let mut v = match text.split_once(": the conversation continues on ").filter(|_| text.starts_with("model: ")) {
                    Some((_, model)) => json!({"kind": "notice", "text": format!("── continued on {model} ──"), "error": error}),
                    None if *ended => json!({"kind": "notice", "text": text, "error": error, "ended": true}),
                    None => json!({"kind": "notice", "text": text, "error": error}),
                };
                if self.rejected.contains(&i) {
                    v["rejected"] = json!(true);
                }
                v
            }
            // the app has no block of its own for it: a notice, an error once the run ended on it
            Block::News(n) => json!({"kind": "notice", "error": false, "text": crate::versions::news_text(&n.from, &n.to, &n.items)}),
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
    /// cost at the prices in force when it began (the session's prices of now until 0.1.192: after `/model` from a cheap
    /// model to Opus every earlier call read at Opus's rates). `model` goes with a cost the core can stand for: the own
    /// loop's prices, or Claude's where the list has the model; without it the cost is a rough figure the window hides.
    /// The calls here are the conversation's own: a subagent's run under its tool call, so `role` is null.
    /// `cost` is what the own loop charged for the call (its provider's own figure where it sends one, a cache write at
    /// its surcharge): a run's calls add up to its `run_cost`; null where the provider reported no usage (not known). On
    /// claude, and in older recordings, it is an estimate at list prices and the call says so (`estimate`): claude's exact
    /// total comes with the result alone. `provider`: who that call was paid to (the own loop can change route inside a
    /// run).
    fn calls_view(&self) -> Vec<Value> {
        self.turn_usage.iter().zip(&self.call_blocks).enumerate().map(|(i, (u, &block))| {
            let c = self.calls.get(i);
            let b = self.call_bill.get(i);
            let price = b.map_or(self.price_m, |b| b.price);
            let (charged, unreported) = (b.and_then(|b| b.cost), b.is_some_and(|b| b.unreported));
            let cost = (!unreported).then(|| charged.unwrap_or_else(|| priced(u, price)));
            let mut v = json!({"block": block, "fresh": u.input + u.cache_write, "cached": u.cache_read, "out": u.output, "cost": cost,
                "first_ms": c.and_then(|c| c.ttft_ms), "tps": c.filter(|c| c.gen_ms > 0.0).map(|c| c.out as f64 / c.gen_ms * 1000.0),
                "total_ms": c.map(|c| c.total_ms)});
            if charged.is_none() && !unreported {
                v["estimate"] = json!(true);
            }
            if let Some(b) = b.filter(|b| !b.model.is_empty() && (price.is_some() || claude_list(&b.model).is_some())) {
                (v["model"], v["role"], v["provider"]) = (json!(b.model), Value::Null, json!(b.provider));
            }
            v
        }).collect()
    }

    /// The model calls a subagent made under tool call `id`: its assistant messages, one per message id.
    #[inline(never)]
    fn sub_calls(&self, id: &str) -> usize {
        let mut ids: Vec<&str> = self.sub_events.get(id).into_iter().flatten().filter(|(_, e)| e["type"] == "assistant")
            .map(|(_, e)| e["message"]["id"].as_str().unwrap_or_default()).collect();
        ids.dedup();
        ids.len()
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
        priced(u, self.price_m)
    }

    /// The run in progress is the own loop's: its results say so (`marked`), its init brought prices (a chat answer on its
    /// route has no init of its own and read "not billed" under a `claude-…` id through SubRouter, design review 12), or, in
    /// older recordings, its model is not claude's.
    #[inline(never)]
    fn own_loop(&self, marked: bool) -> bool {
        marked || self.price_m.is_some() || !self.model.is_empty() && !self.model.starts_with("claude-")
    }

    /// Whether the run in progress is on the Claude plan: not the own loop, and claude on its login with nothing that bills
    /// it elsewhere (models::claude_payer). The receipt and the header's split ask this alone; the run log asks
    /// claude_payer too (live.rs).
    #[inline(never)]
    fn on_plan(&self, marked: bool) -> bool {
        !self.own_loop(marked) && !self.claude_billed
    }

    /// What was used since the last result cost, for a run that ended without a total of its own (claude's stopped turn
    /// reports $0; a hard stop or a crash, nothing): the own loop's calls as it charged them, the rest at list prices (then
    /// `true`: an estimate). None where the rest has no list price in the core (a Claude model but Opus 5.5: its tokens at
    /// Opus's rates read as Sonnet's price, design review 12). A call the provider sent no usage for is not known: 0.
    #[inline(never)]
    fn unbilled(&self) -> (Option<f64>, bool) {
        let (u, b) = (self.usage, self.usage_at_result);
        let mut rest = [u.input - b.input, u.cache_write - b.cache_write, u.cache_read - b.cache_read, u.output - b.output];
        let mut charged = 0.0;
        for (c, t) in self.call_bill.iter().zip(&self.turn_usage).skip(self.billed).filter(|(c, _)| c.cost.is_some() || c.unreported) {
            charged += c.cost.unwrap_or(0.0);
            for (r, n) in rest.iter_mut().zip([t.input, t.cache_write, t.cache_read, t.output]) {
                *r = r.saturating_sub(n);
            }
        }
        let rest = Usage { input: rest[0], cache_write: rest[1], cache_read: rest[2], output: rest[3] };
        if rest.total_in() + rest.output == 0 {
            return (Some(charged), false);
        }
        // claude's: Usage::estimate, its calibrated rates (a cache write at 1.5x), as the header's live figure
        let listed = self.price_m.is_some() || !self.model.starts_with("claude-") || claude_list(&self.model).is_some();
        (listed.then(|| charged + self.estimate(&rest)), true)
    }

    /// The run in progress ends: its `done` block, with its own share of the totals, how it was paid, and its subagents'
    /// calls and tries not in the total (its calls', and `lost`: those of a call that never answered); the calls from here
    /// on are the next run's.
    #[inline(never)]
    fn end_run(&mut self, ok: bool, secs: f64, turns: u64, stopped: bool, marked: bool, priced: bool, estimate: bool, lost: u32) {
        let (cost, usage) = (self.cost(), self.usage);
        // after a cut, from where it was cut (the Done left is older than that)
        let (run_cost, run_usage) = run_share(&self.blocks, cost, usage, self.cut_at.take());
        let bill = self.bill(marked);
        let uncounted = self.call_bill.iter().skip(self.billed).map(|b| b.uncounted).sum::<u32>() + lost;
        let subs: usize = self.sub_events.keys().map(|id| self.sub_calls(id)).sum();
        let sub_calls = subs.saturating_sub(std::mem::replace(&mut self.subs_counted, subs));
        self.billed = self.call_bill.len();
        let changes = self.run_changes();
        self.push(Block::Done(Summary { ok, secs, turns, cost, usage, stopped, run_cost: priced.then_some(run_cost), run_usage, bill, estimate, uncounted, sub_calls, changes }));
    }

    /// The files the run ending now changed with its edit tools, in the order first changed (design review 13). A write
    /// is a rewrite where claude says it updated the file, or where the session read or changed that path before (the own
    /// loop's write answers "ok" either way, and the file is not looked at: a replay would find it written already).
    fn run_changes(&self) -> Vec<FileChange> {
        let from = self.blocks.iter().rposition(|b| matches!(b, Block::Done(_))).map_or(0, |k| k + 1);
        let path_of = |t: &Tool| crate::ui::tool_arg(t, "path").to_string();
        let mut out: Vec<FileChange> = Vec::new();
        for (i, b) in self.blocks.iter().enumerate().skip(from) {
            let Block::Tool(t) = b else { continue };
            let Some(pairs) = t.replaced().filter(|_| t.result.is_some() && !t.error && !path_of(t).is_empty()) else { continue };
            let path = path_of(t);
            let rel = crate::ui::repo_path(&self.cwd, &path);
            let write = matches!(crate::ui::short_name(&t.name), "write");
            let result = t.result.as_deref().unwrap_or_default();
            let seen = || self.blocks[..i].iter().any(|b| matches!(b, Block::Tool(x) if path_of(x) == path && matches!(crate::ui::short_name(&x.name), "read" | "edit" | "write")));
            let rewritten = write && !result.starts_with("File created") && (result.contains("has been updated") || seen());
            let k = out.iter().position(|c| c.path == rel).unwrap_or_else(|| {
                out.push(FileChange { path: rel, added: 0, removed: 0, rewritten: false });
                out.len() - 1
            });
            out[k].rewritten |= rewritten;
            for (old, new) in pairs {
                let (a, r) = change_counts(old, new);
                (out[k].added, out[k].removed) = (out[k].added + a, out[k].removed + r);
            }
        }
        out
    }

    /// A run the process took with it (a hard stop, a crash): no result comes, so it ends here, with what it spent so far
    /// in the totals and its own receipt, and the call `cut` off as one whose cost nobody knows. Its calls went under the
    /// next answer's receipt and its spend left the conversation's total (design review 12).
    #[inline(never)]
    fn close_run(&mut self, secs: f64, stopped: bool, cut: bool) {
        let asked = self.blocks.iter().rev().take_while(|b| !matches!(b, Block::Done(_))).any(|b| matches!(b, Block::Prompt(_)));
        if self.call_bill.len() == self.billed && !asked {
            return;
        }
        if let Some(b) = self.call_bill.last_mut().filter(|_| cut) {
            b.unreported = true;
        }
        let (spent, estimate) = self.unbilled();
        let s = spent.unwrap_or(0.0);
        if self.on_plan(false) {
            self.plan_now += s;
        } else {
            self.paid_now += s;
        }
        self.exact_cost = Some(self.exact_cost.unwrap_or(self.cost_base) + s);
        self.usage_at_result = self.usage;
        self.estimated |= estimate;
        let turns = (self.call_bill.len() - self.billed) as u64;
        self.end_run(false, secs, turns, stopped, false, spent.is_some(), estimate || spent.is_none(), 0);
    }

    /// How the run that just ended was paid. Paid as its cost was split (`on_plan`).
    #[inline(never)]
    fn bill(&self, marked: bool) -> Option<Bill> {
        let own = self.own_loop(marked);
        let paid = !self.on_plan(marked);
        // who the run's calls were paid to, in order, each once, and the first one's prices; a call that used nothing (a
        // refused one) paid nobody
        let (mut providers, mut price): (Vec<String>, Option<(f64, f64, f64)>) = (vec![], None);
        for (b, u) in self.call_bill.iter().zip(&self.turn_usage).skip(self.billed) {
            if let Some(p) = b.provider.as_ref().filter(|p| u.total_in() + u.output > 0 && !providers.contains(p)) {
                providers.push(p.clone());
                price = price.or(b.price);
            }
        }
        let mixed = paid && providers.len() > 1;
        (!self.model.is_empty()).then(|| Bill { model: self.model.clone(), plan: !paid,
            provider: if mixed { None } else { providers.first().cloned().or_else(|| self.provider.clone()).filter(|_| paid) },
            price_m: if mixed { None } else if own { price.or(self.price_m) } else { claude_list(&self.model) },
            providers: if mixed { providers } else { vec![] } })
    }

    /// A chat's hand-off (`[[agent]]`, live.rs) never shows in a chat's reply (a chat-profile session's, a router chat
    /// answer): one that starts with it, nor, while it streams, the first characters that may still become it. An agent's
    /// answer shows as it is, one that starts with the marker too.
    pub fn hidden(&self, i: usize) -> bool {
        let Some(Block::Text { text, live }) = self.blocks.get(i) else { return false };
        let t = text.trim_start();
        if !(t.starts_with(crate::live::HANDOFF) || (*live && !t.is_empty() && crate::live::HANDOFF.starts_with(t))) {
            return false;
        }
        // its turn: between the messages before and after it
        let prompt = |b: &Block| matches!(b, Block::Prompt(_));
        let from = self.blocks[..i].iter().rposition(prompt).map_or(0, |p| p + 1);
        let to = self.blocks[i..].iter().position(prompt).map_or(self.blocks.len(), |p| i + p);
        let said = |line: &str| self.blocks[from..to].iter().any(|b| matches!(b, Block::Notice { text, .. } if text.starts_with(line)));
        !said("answered by the agent") && (self.profile == "chat" || said("answered as a chat"))
    }

    /// The app's send started nothing (main.rs send_rejected): the notices it left from block `from` on carry `rejected` in
    /// the view, so the window pairs them with the refusal exactly; by timing, an unrelated error in the same tick passed for
    /// it (design review 13).
    pub fn mark_rejected(&mut self, from: usize) {
        // the refusal's own notice: the last one since the send began
        if let Some(i) = (from..self.blocks.len()).rev().find(|&i| matches!(self.blocks[i], Block::Notice { .. })) {
            self.rejected.push(i);
        }
    }

    /// Blocks went (`gone`, by index): the refusal marks stay on the blocks they were on.
    fn unmark(&mut self, gone: impl Fn(usize) -> bool) {
        self.rejected = self.rejected.iter().filter(|&&i| !gone(i)).map(|&i| i - (0..i).filter(|&j| gone(j)).count()).collect();
    }

    /// The transcript back to its first `n` blocks (/rewind, the window's edit from a message), and the model calls made
    /// from there on with it: the window's receipts and token counts kept the calls of the cut part (design review 13).
    pub fn truncate(&mut self, n: usize) {
        self.blocks.truncate(n);
        self.unmark(|i| i >= n);
        let k = self.call_blocks.partition_point(|&b| b < n);
        self.call_blocks.truncate(k);
        self.turn_usage.truncate(k);
        self.call_bill.truncate(k);
        self.calls.truncate(k);
        self.billed = self.billed.min(k);
        (self.cut_at, self.usage_at_result) = (Some((self.cost(), self.usage)), self.usage);
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
                self.push_aside(Block::Notice { text: format!("MCP {name}: {why}"), error: true, ended: false });
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
                self.price_m = price_of(e);
                // who pays claude's runs (models::claude_payer): its key's source and the billing variables tazuna saw set
                // where it runs; the receipt must not say "on your plan, not billed" under an API key, Bedrock or a gateway
                let set: Vec<&str> = e["tazuna_env"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                let (plan, payer) = crate::models::claude_payer(e["apiKeySource"].as_str(), |k| set.contains(&k));
                self.claude_billed = self.model.starts_with("claude-") && self.price_m.is_none() && !plan;
                self.provider = e["provider"].as_str().map(String::from).or_else(|| payer.filter(|_| self.claude_billed).map(String::from));
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
                // review and auto run as acceptEdits underneath: claude's report of that does not rename them
                if let Some(m) = e["permissionMode"].as_str() && !(matches!(self.mode.as_str(), "review" | "auto") && m == "acceptEdits") {
                    self.mode = mode_name(m).to_string();
                }
                self.activity = Activity::Waiting;
            }
            (Some("system"), Some("compact_boundary")) => {
                self.push(Block::Notice { text: "── context compacted: earlier turns are now a summary ──".into(), error: false, ended: false });
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
                // the own loop moved to another route: its prices and who is paid, for the calls from here on
                if sub == "route_switch" && e["price_per_m"].is_array() {
                    (self.price_m, self.provider) = (price_of(e), e["provider"].as_str().map(String::from));
                }
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
                self.push(Block::Notice { text: format!("{what} · {}", shown(&text, 4000)), error: false, ended: false });
            }
            (Some("harness_user"), _) => {
                self.clock_on(at_ms);
                self.ready_at = Some(at_ms);
                self.last_prompt = s(&e["text"]);
                self.push(Block::Prompt(s(&e["text"])));
                self.activity = Activity::Waiting;
                self.verify_cancelled = false;
            }
            (Some("harness_interrupt"), _) => self.stopping = true,
            // a queued message the user's stop gave back to the box (live.rs stop): its line says so in its place, so the
            // blocks after it keep their places (design review 13)
            (Some("harness_unqueued"), _) => {
                let text = s(&e["text"]);
                if let Some(i) = self.blocks.iter().rposition(|b| matches!(b, Block::Prompt(p) if *p == text)) {
                    let first: String = text.lines().next().unwrap_or_default().chars().take(80).collect();
                    let more = if first.chars().count() < text.trim_end().chars().count() { "…" } else { "" };
                    self.blocks[i] = Block::Notice { text: format!("not sent, back in the box: {first}{more}"), error: false, ended: false };
                    self.last_prompt = self.blocks.iter().rev().find_map(|b| if let Block::Prompt(p) = b { Some(p.clone()) } else { None }).unwrap_or_default();
                }
            }
            // the window's Retry (live.rs retry): the last message's failed try goes (its error, its receipt, a card), its
            // calls go under the new reply's receipt, and the work on it begins again under the same message
            // what came after it stays (a switch's "continued on" line: the new reply comes under it)
            (Some("harness_retry"), _) => {
                let p = self.blocks.iter().rposition(|b| matches!(b, Block::Prompt(_)));
                let d = p.and_then(|p| self.blocks[p..].iter().rposition(|b| matches!(b, Block::Done(_))).map(|d| p + d));
                if let (Some(p), Some(d)) = (p, d) {
                    self.blocks.drain(p + 1..=d);
                    self.unmark(|i| (p + 1..=d).contains(&i));
                    let moved = |i: usize| if i <= p { i } else if i <= d { p + 1 } else { i - (d - p) };
                    self.changes.retain(|c| c.at_block <= p || c.at_block > d);
                    for c in &mut self.changes {
                        c.at_block = moved(c.at_block);
                    }
                    for b in &mut self.call_blocks {
                        *b = moved(*b);
                    }
                    (self.retry, self.stopping) = (None, false);
                    self.call_thinking.clear();
                    self.new_blocks += 1;
                }
                self.clock_on(at_ms);
                self.ready_at = Some(at_ms);
                self.activity = Activity::Waiting;
            }
            // a turn's changes (main.rs), its card under the turn's answer
            (Some("harness_turn"), _) => {
                let files = e["files"].as_array().into_iter().flatten().map(|f| crate::turns::File { path: s(&f["path"]), added: f["added"].as_u64(),
                    removed: f["removed"].as_u64(), status: f["status"].as_str().and_then(|x| x.chars().next()).unwrap_or('M') }).collect();
                let at_block = self.blocks.iter().rposition(|b| matches!(b, Block::Done(_))).unwrap_or(0);
                self.changes.push(Change { before: s(&e["before"]), after: s(&e["after"]), files, at_block, undone: false });
            }
            (Some("harness_undone"), _) => {
                if let Some(c) = self.changes.iter_mut().rev().find(|c| !c.undone && c.after == s(&e["after"])) {
                    c.undone = true;
                }
            }
            (Some("harness_verify_start"), _) => {
                self.clock_on(at_ms);
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
                self.clock_off(at_ms, 0.0);
                self.activity = if e["final"] == true { Activity::Done } else { Activity::Waiting };
            }
            (Some("harness_verify_feedback"), _) => {
                let text = format!("↻ sent the failing tests back to claude (round {} of {})", e["round"], e["max"]);
                self.push(Block::Notice { text, error: false, ended: false });
                self.clock_on(at_ms);
                self.activity = Activity::Waiting;
            }
            (Some("harness_route"), _) => {
                (self.model, self.effort) = (s(&e["model"]), s(&e["effort"]));
                self.profile = s(&e["profile"]);
                self.lane = e.clone();
                self.push(Block::Notice { text: lane_text(e), error: false, ended: false });
            }
            (Some("harness_profile"), _) => {
                self.profile = s(&e["profile"]);
                self.push(Block::Notice { text: s(&e["text"]), error: false, ended: false });
            }
            (Some("harness_escalate"), _) => {
                // the old process is gone: its reported cost is final. The hand-over to the stronger model is work ("Working
                // 0 s" until its first call)
                self.close_process();
                self.clock_on(at_ms);
                (self.model, self.effort) = (s(&e["model"]), s(&e["effort"]));
                (self.provider, self.price_m) = (None, None);
                let text = format!("↑ tests failed on the cheap lane: fixing on {} {}", self.model.trim_start_matches("claude-"), self.effort);
                self.push(Block::Notice { text, error: false, ended: false });
            }
            (Some("harness_verify_cancel"), _) => {
                self.push(Block::Notice { text: "verification cancelled · the run ends when the tests return".into(), error: false, ended: false });
                self.activity = Activity::Ready;
                self.verify_cancelled = true;
            }
            // UI-only message (not from the process): no effect on cost or tools
            (Some("harness_review_start"), _) => {
                self.push(Block::Notice { text: format!("review: {} is checking the change against the task", s(&e["model"])), error: false, ended: false })
            }
            (Some("harness_review_done"), _) => {
                let n = e["issues"].as_array().map_or(0, Vec::len);
                let text = match (e["error"].as_str(), e["ok"] == true) {
                    (Some(err), _) => format!("review skipped: {err}"),
                    (None, true) => "review: no problems found".to_string(),
                    (None, false) => format!("review: {n} problem{} sent back", if n == 1 { "" } else { "s" }),
                };
                self.push(Block::Notice { text, error: false, ended: false })
            }
            (Some("harness_review"), _) => {
                self.clock_on(at_ms); // a round of its own, after the result
                self.push(Block::Notice { text: "self-review: checking the change against the task".into(), error: false, ended: false })
            }
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
                self.push(Block::Notice { text: "open permission questions dropped: claude stopped".into(), error: false, ended: false });
            }
            (Some("harness_permission_done"), _) => {
                // the step runs from the answer: the wait for it was the user's ("Ran ls -la 7.7 s" after 7 s of it, review 12).
                // Only the call the question names: one from inside a subagent is not on this screen, and the oldest open call
                // then is the subagent's own (its minutes read as seconds)
                if let Some((tool, input)) = self.asked.front().cloned()
                    && let Some(Block::Tool(t)) = self.blocks.iter_mut().rev().find(|b| matches!(b, Block::Tool(x) if x.result.is_none() && x.done_ms.is_none() && x.name == tool && x.input == input))
                {
                    t.started_ms = at_ms;
                }
                self.asked.pop_front();
                self.mode = mode_name(e["mode"].as_str().unwrap_or_default()).to_string();
                let verb = match (e["allow"] == true, e["always"] == true) {
                    (true, true) => "allowed for this session:",
                    (true, false) => "allowed",
                    _ => "declined",
                };
                // the lean tools by their own names ("allowed bash")
                let tool = s(&e["tool"]);
                self.push(Block::Notice { text: format!("{verb} {}", tool.strip_prefix("mcp__lean__").unwrap_or(&tool)), error: false, ended: false });
            }
            (Some("harness_mode"), _) => {
                self.mode = mode_name(e["mode"].as_str().unwrap_or_default()).to_string();
                // shift-tab through the modes changes one line, not one line a press (design review 4)
                match self.blocks.last_mut() {
                    Some(Block::Notice { text, .. }) if text.starts_with("mode: ") => *text = format!("mode: {}", self.mode),
                    _ => self.push(Block::Notice { text: format!("mode: {}", self.mode), error: false, ended: false }),
                }
            }
            // the first session after an upgrade: its versions as a block of their own (ui news_lines)
            (Some("harness_news"), _) => {
                let items = e["items"].as_array().map(|a| a.iter().filter_map(|i| Some((i[0].as_str()?.to_string(), i[1].as_str()?.to_string()))).collect()).unwrap_or_default();
                self.push_aside(Block::News(crate::versions::News { from: s(&e["from"]), to: s(&e["to"]), items }));
            }
            // tazuna's own line (a hook that failed, a setting saved): beside the run, which goes on. `ended`: a message on
            // screen that could not start, so nothing runs for it
            (Some("harness_info"), _) => {
                self.push_aside(Block::Notice { text: s(&e["text"]), error: e["error"] == true, ended: e["ended"] == true });
                if e["ended"] == true {
                    self.clock_off(at_ms, 0.0);
                    self.activity = Activity::Ready;
                }
            }
            // the agent takes a chat's message (live.rs hand_off) in a process of its own (the one before it is gone). The
            // chat's reply that handed it over goes (its text, the thinking before it, its receipt, its "answered as a chat"
            // line), and only its (`q`: another router chat's answer may stand after the same message); its cost stays in
            // the totals and joins the agent's answer. The line over that answer, if any, names the question when later
            // messages came before the agent was free
            (Some("harness_handoff"), _) => {
                self.close_process();
                let q = s(&e["q"]);
                let last = self.blocks.iter().rposition(|b| matches!(b, Block::Prompt(_)));
                let at = self.blocks.iter().rposition(|b| matches!(b, Block::Prompt(p) if *p == q)).or(last);
                let from = at.map_or(0, |i| i + 1);
                let marker = self.blocks[from..].iter().position(|b| matches!(b, Block::Text { text, .. } if crate::live::hands_off(text)));
                if let Some(m) = marker.map(|i| from + i) {
                    let think = self.blocks[..m].iter().rev().take_while(|b| matches!(b, Block::Thinking { .. })).count();
                    let done = self.blocks[m..].iter().position(|b| matches!(b, Block::Done(_))).map(|i| m + i);
                    let chat = self.blocks[from..m].iter().position(|b| matches!(b, Block::Notice { text, .. } if text.starts_with("answered as a chat"))).map(|i| from + i);
                    let gone = |i: usize| (m - think..=m).contains(&i) || Some(i) == done || Some(i) == chat;
                    let mut i = 0;
                    self.blocks.retain(|_| {
                        i += 1;
                        !gone(i - 1)
                    });
                    self.unmark(gone);
                    self.call_thinking.clear();
                }
                let line = s(&e["text"]);
                if !line.is_empty() {
                    let line = if at == last { line } else { format!("{line} · {}", shown(&q, 60)) };
                    self.push(Block::Notice { text: line, error: false, ended: false });
                }
                self.clock_on(at_ms);
                self.ready_at = Some(at_ms);
                self.activity = Activity::Waiting;
            }
            // a message held while the tests ran is sent now: its work begins here, not at its first call's start
            (Some("harness_held_start"), _) => {
                self.clock_on(at_ms);
                self.ready_at = Some(at_ms);
            }
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
            // a replayed recording's process ended there (`/resume` replays several, without their notices); a run it was
            // in ends with it, as after a hard stop
            (Some("harness_process_end"), _) => {
                let (secs, cut) = (self.working_for(at_ms) / 1000.0, self.call_start.take().is_some());
                self.clock_off(at_ms, 0.0);
                self.close_run(secs, false, cut);
                self.close_process();
            }
            (Some("harness_effort"), _) => {
                self.effort = s(&e["effort"]);
                if let Some(m) = e["model"].as_str() {
                    self.model = m.to_string(); // a level that is its own id (OmniRoute's)
                }
                self.push(Block::Notice { text: s(&e["text"]), error: false, ended: false });
            }
            (Some("harness_switch"), _) => {
                // /model: the old process is gone
                self.close_process();
                (self.model, self.effort) = (s(&e["model"]), s(&e["effort"]));
                // picked before the first message: the route line names the model that runs, without the lane's cascade
                // (a pick ends it, live.rs picked). It named the router's Sonnet and its escalation over an own-loop model
                // (review 12)
                let picked = !self.blocks.iter().any(|b| matches!(b, Block::Prompt(_)));
                if picked && let Some(Block::Notice { text, .. }) = self.blocks.iter_mut().find(|b| matches!(b, Block::Notice { text, .. } if text.starts_with("lane "))) {
                    let mut lane = self.lane.clone();
                    (lane["model"], lane["effort"], lane["escalate"]) = (e["model"].clone(), e["effort"].clone(), Value::Null);
                    *text = lane_text(&lane);
                }
                (self.provider, self.price_m) = (None, None); // the next process says its own
                self.context_window = None; // the next model reports its own (the own loop's: none)
                self.cache_ttl_ms = None;
                self.push(Block::Notice { text: s(&e["text"]), error: false, ended: false });
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
            // the process is gone (a hard stop, an exit, a restart; tazuna's other lines are `harness_info`)
            (Some("harness_notice"), _) => {
                let stopped = std::mem::take(&mut self.stopping);
                // a killed process ends its call without message_delta or result: "waiting for first token" kept counting
                let cut = std::mem::take(&mut self.call_start).is_some();
                self.first_token = None;
                // and no result either: its run ends here, or the next reply would count from the killed one's start
                let secs = self.working_for(at_ms) / 1000.0;
                self.clock_off(at_ms, 0.0);
                self.push(Block::Notice { text: s(&e["text"]), error: e["error"].as_bool().unwrap_or(false), ended: false });
                self.close_run(secs, stopped, cut);
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
            // claude's own error as a message ("Not logged in · Please run /login"): its result says it, once, as the
            // error; it showed as the answer too (review 12)
            (Some("assistant"), _) if e["is_api_error_message"] == true => {}
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
                        let (id, cwd) = (s(&c["id"]), self.cwd.clone());
                        if let Some(t) = self.tool_mut(&id) {
                            t.input = c["input"].clone();
                            // an edit asked for: where its old text is now
                            if crate::ui::short_name(&t.name) == "edit" && t.result.is_none() {
                                t.at = locate(&std::path::Path::new(&cwd).join(crate::ui::tool_arg(t, "path")), crate::ui::tool_arg(t, "old"));
                            }
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
                    let cwd = self.cwd.clone();
                    if let Some(t) = self.tool_mut(&s(&c["tool_use_id"])) {
                        // an edit done: where its new text landed
                        if crate::ui::short_name(&t.name) == "edit" {
                            t.at = if error { None } else { locate(&std::path::Path::new(&cwd).join(crate::ui::tool_arg(t, "path")), crate::ui::tool_arg(t, "new")) };
                        }
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
                        self.push(Block::Notice { text: "── conversation cleared: claude starts fresh from here ──".into(), error: false, ended: false });
                    } else if !text.trim().is_empty() {
                        self.push(Block::Text { text, live: false });
                    }
                }
                let (mut priced, mut estimate) = (true, false);
                if let Some(mut c) = e["total_cost_usd"].as_f64() {
                    let paid = !self.on_plan(e["tazuna_paid"] == true);
                    let prev = if paid { self.paid_now } else { self.plan_now };
                    let (u, b) = (self.usage, self.usage_at_result);
                    let since = Usage { input: u.input - b.input, cache_write: u.cache_write - b.cache_write, cache_read: u.cache_read - b.cache_read, output: u.output - b.output };
                    if e["tazuna_chat"] == true {
                        // a chat answer's own process (main.rs chat_turn): its one total is all its own, beside the
                        // conversation's process, whose count goes on (it was read as a stopped turn, then lost in the next)
                        let now = self.exact_cost.unwrap_or(self.cost_base) - self.cost_base;
                        self.cost_base += c;
                        if paid {
                            self.paid_base += c;
                        } else {
                            self.plan_base += c;
                        }
                        c = now;
                    } else if c <= prev && since.total_in() + since.output > 0 {
                        // a stopped turn reports nothing for itself (claude: total_cost_usd 0 after 21k tokens, 2026-11-02):
                        // its calls as charged or at list prices, and the cost is marked as an estimate
                        let (spent, est) = self.unbilled();
                        (priced, estimate) = (spent.is_some(), est || spent.is_none());
                        c = prev + spent.unwrap_or(0.0);
                        self.estimated = true;
                    } else if c + 1e-9 < prev {
                        // claude counts from $0 again after /clear: what it reported before stays in the totals
                        self.close_process();
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
                self.clock_off(at_ms, secs);
                let turns = e["num_turns"].as_u64().unwrap_or(0);
                let stopped = std::mem::take(&mut self.stopping);
                // why it failed, in the transcript: the result carried it but only "Failed" showed (a provider's content
                // filter refused a question, 2026-11-05)
                if !ok && !stopped {
                    if let Some(why) = e["result"].as_str().filter(|r| !r.trim().is_empty()) {
                        // claude's own login: tazuna's words for it, on claude only (a gateway's "OAuth token has expired" is not
                        // claude's)
                        let said = if !self.own_loop(e["tazuna_paid"] == true) && claude_login_error(why) { CLAUDE_NOT_SIGNED_IN.to_string() } else { failure_reason(why) };
                        // the call that was being tried again failed for good: its block says so, not a second one
                        match self.retry.and_then(|i| self.blocks.get_mut(i)) {
                            Some(Block::Retry(r)) => (r.why, r.raw, r.tries, r.gave_up) = (said, shown(why, 4000), r.tries + 1, true),
                            _ => self.push(Block::Notice { text: said, error: true, ended: false }),
                        }
                    }
                }
                self.end_run(ok, secs, turns, stopped, e["tazuna_paid"] == true, priced, estimate, e["tazuna_uncounted"].as_u64().unwrap_or(0) as u32);
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
                self.call_bill.push(CallBill { model: self.model.clone(), price: self.price_m, provider: self.provider.clone(), cost: None, unreported: false, uncounted: 0 });
                // a call with nothing running (a queued message's, after the run before it ended): work begins here
                self.clock_on(self.now_ms);
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
                            at: None,
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
                // the own loop says what it charged for the call (null: the provider reported no usage), and the tries
                // before it the provider may have billed
                if let Some(b) = self.call_bill.last_mut() {
                    match ev.get("tazuna_cost") {
                        Some(Value::Null) => b.unreported = true,
                        Some(c) => b.cost = Some(b.cost.unwrap_or(0.0) + c.as_f64().unwrap_or(0.0)),
                        None => {}
                    }
                    b.uncounted += ev["tazuna_uncounted"].as_u64().unwrap_or(0) as u32;
                }
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

    /// The last message and its block, when its turn ended without an answer (an error, a stop): what Retry sends again.
    /// The answer is the text after the turn's last step; text before it ("I'll check the config.", then a 529) is not one.
    #[inline(never)]
    pub fn retryable(&self) -> Option<(usize, String)> {
        let p = self.blocks.iter().rposition(|b| matches!(b, Block::Prompt(_)))?;
        let after = &self.blocks[p + 1..];
        let failed = after.iter().any(|b| matches!(b, Block::Done(d) if !d.ok));
        let from = after.iter().rposition(|b| matches!(b, Block::Tool(_))).map_or(p + 1, |t| p + 2 + t);
        let answered = (from..self.blocks.len()).any(|i| matches!(&self.blocks[i], Block::Text { text, .. } if !text.trim().is_empty() && !self.hidden(i)));
        let Block::Prompt(text) = &self.blocks[p] else { return None };
        (failed && !answered).then(|| (p, text.clone()))
    }

    /// The block of the call the oldest permission question is about: the open call with the question's tool and input,
    /// then its tool alone, then the oldest open call (a question is asked in the calls' order).
    #[inline(never)]
    pub fn asked_call(&self) -> Option<usize> {
        let open = || self.blocks.iter().enumerate().filter_map(|(i, b)| match b {
            Block::Tool(x) if x.result.is_none() && x.done_ms.is_none() => Some((i, x)),
            _ => None,
        });
        let about = self.asked.front().and_then(|(tool, input)| open().find(|(_, x)| x.name == *tool && x.input == *input).or_else(|| open().find(|(_, x)| x.name == *tool)));
        about.or_else(|| open().next()).map(|(i, _)| i)
    }

    fn tool_mut(&mut self, id: &str) -> Option<&mut Tool> {
        self.blocks.iter_mut().rev().find_map(|b| match b {
            Block::Tool(t) if t.id == id => Some(t),
            _ => None,
        })
    }
}

/// The router's line at a session's start: "lane small: sonnet-5-5 medium · lean profile (tiny repo, 5 tracked files) ·
/// escalates to Opus 5.5 high if tests fail" (the terminal says it as "started on …", ui::lane_notice).
#[inline(never)]
fn lane_text(e: &Value) -> String {
    let s = |v: &Value| v.as_str().unwrap_or_default().to_string();
    let up = e["escalate"].as_str().map_or(String::new(), |x| {
        let (m, effort) = x.split_once(' ').map_or((x, String::new()), |(m, e)| (m, format!(" {e}")));
        format!(" · escalates to {}{effort} if tests fail", crate::ui::model_short(m))
    });
    let profile = e["profile"].as_str().filter(|p| !p.is_empty()).map_or(String::new(), |p| format!(" · {p} profile"));
    format!("lane {}: {} {}{profile} ({}){up}", s(&e["lane"]), s(&e["model"]).trim_start_matches("claude-"), s(&e["effort"]), s(&e["reason"]))
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

/// A permission mode's name in tazuna's words as `Live::set_mode` takes it: claude's name, or tazuna's own auto and
/// review. None: no such mode.
pub fn mode_from(name: &str) -> Option<&'static str> {
    Some(match name {
        "auto" => "auto",
        "full" => "bypassPermissions",
        "ask" => "default",
        "edits" => "acceptEdits",
        "review" => "review",
        "plan" => "plan",
        _ => return None,
    })
}

/// claude's permission mode names in tazuna's words.
pub fn mode_name(m: &str) -> &str {
    match m {
        "bypassPermissions" => "full",
        "default" => "ask",
        "acceptEdits" => "edits",
        "plan" => "plan",
        other => other,
    }
}

/// claude's error for a login that is missing, expired or revoked, in the window's words; the terminal leaves out the
/// window's parts (ui::block_lines). claude's own says "Please run /login", which in tazuna saves API keys (review 12).
pub const CLAUDE_NOT_SIGNED_IN: &str = "Claude Code isn't signed in. Run `claude auth login` in a terminal (or Settings → Providers → Claude), then Retry.";

/// claude's texts for that (its CLI 2.1.330): "Not logged in · Please run /login", "Login expired · Please run /login",
/// "OAuth token revoked · Please run /login", "Session expired. Please run /login to sign in again.", "Failed to
/// authenticate: OAuth session expired and could not be refreshed", "Please run /login · API Error: 401 …".
#[inline(never)]
fn claude_login_error(text: &str) -> bool {
    ["Please run /login", "run /login to sign in", "Not logged in", "OAuth session expired", "OAuth token revoked", "OAuth token has expired"].iter().any(|p| text.contains(p))
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

    /// The window's edit from a message cuts the transcript and the model calls made after it: the calls' receipts and
    /// token counts kept the cut part's, and the next run's receipt carried the cut run's spend (design review 13).
    #[test]
    fn a_cut_transcript_drops_the_calls_made_after_it() {
        let mut s = Session::new("", "medium");
        // a run: its message, one call with `out` output tokens, the process's running total cost at its end
        let run = |s: &mut Session, text: &str, out: u64, total: f64, at: f64| {
            s.apply(&json!({"type": "harness_user", "text": text}), at);
            s.apply(&json!({"type": "stream_event", "event": {"type": "message_start"}}), at + 1.0);
            s.apply(&json!({"type": "stream_event", "event": {"type": "message_delta", "usage": {"input_tokens": 10, "output_tokens": out}}}), at + 2.0);
            s.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": total}), at + 3.0);
        };
        run(&mut s, "first", 100, 0.10, 0.0);
        let cut = s.blocks.len();
        run(&mut s, "second", 200, 0.30, 10.0);
        assert_eq!((s.turn_usage.len(), s.calls.len(), s.calls_view().len()), (2, 2, 2));
        s.truncate(cut);
        assert_eq!((s.turn_usage.len(), s.call_blocks.len(), s.call_bill.len(), s.calls.len()), (1, 1, 1, 1));
        assert_eq!(s.calls_view()[0]["out"], 100, "the first call's figures stay");
        // the run after the cut: its own calls' tokens and cost, not the cut run's as well
        run(&mut s, "second, again", 50, 0.35, 20.0);
        let Some(Block::Done(d)) = s.blocks.last() else { panic!("a Done") };
        assert_eq!((d.run_usage.output, d.run_usage.input), (50, 10));
        assert!((d.run_cost.unwrap() - 0.05).abs() < 1e-9, "{:?}", d.run_cost);
    }

    /// An edit is placed only in a regular file up to 4 MB: a device or a FIFO would never end the read (design review 13).
    #[test]
    #[cfg(unix)]
    fn an_edit_is_placed_only_in_a_regular_file() {
        let dir = std::env::temp_dir().join(format!("tazuna-locate-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.rs"), "one\ntwo\nthree\n").unwrap();
        assert_eq!(locate(&dir.join("a.rs"), "two"), Some((2, Some("one".into()), Some("three".into()))));
        let fifo = dir.join("pipe");
        let _ = std::process::Command::new("mkfifo").arg(&fifo).status();
        assert_eq!((locate(std::path::Path::new("/dev/zero"), "x"), locate(&fifo, "x"), locate(&dir, "x")), (None, None, None), "returns at once");
        let _ = std::fs::remove_dir_all(&dir);
    }

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
    fn a_message_that_could_not_start_says_its_turn_is_over() {
        let mut s = Session::new("", "high");
        s.apply(&json!({"type": "harness_user", "text": "keep this draft"}), 0.0);
        s.apply(&json!({"type": "harness_info", "text": "starting claude: program not found", "error": true, "ended": true}), 1.0);
        s.apply(&json!({"type": "harness_info", "text": "hook failed", "error": true}), 2.0);
        let blocks = s.view()["blocks"].clone();
        assert_eq!((blocks[1]["ended"].clone(), blocks[2]["ended"].clone()), (json!(true), json!(null)));
    }

    #[test]
    fn claude_not_signed_in_is_one_error_in_tazunas_words_not_also_the_answer() {
        let mut s = Session::new("", "high");
        s.apply(&json!({"type": "harness_user", "text": "what is a haiku?"}), 0.0);
        s.apply(&json!({"type": "assistant", "error": "authentication_failed", "is_api_error_message": true, "parent_tool_use_id": null,
            "message": {"model": "<synthetic>", "content": [{"type": "text", "text": "Not logged in · Please run /login"}]}}), 10.0);
        s.apply(&json!({"type": "result", "subtype": "success", "is_error": true, "result": "Not logged in · Please run /login", "total_cost_usd": 0, "duration_ms": 42}), 20.0);
        let kinds: Vec<String> = s.view()["blocks"].as_array().unwrap().iter().map(|b| b["kind"].as_str().unwrap().to_string()).collect();
        assert_eq!(kinds, ["prompt", "notice", "done"]);
        assert!(matches!(&s.blocks[1], Block::Notice { text, error: true, .. } if text == CLAUDE_NOT_SIGNED_IN));
        assert!(claude_login_error("Login expired · Please run /login") && claude_login_error("Failed to authenticate: OAuth session expired and could not be refreshed"));
        // a gateway on the own loop saying the same words is not claude: no claude advice
        let mut own = Session::new("", "high");
        own.apply(&json!({"type": "harness_user", "text": "hi"}), 0.0);
        own.apply(&json!({"type": "result", "subtype": "error", "is_error": true, "result": "HTTP 401: OAuth token has expired", "tazuna_paid": true}), 1.0);
        assert!(matches!(&own.blocks[1], Block::Notice { text, .. } if text != CLAUDE_NOT_SIGNED_IN));
        // what Retry sends again, and the failed try gone once it does
        assert_eq!(s.retryable(), Some((0, "what is a haiku?".to_string())));
        s.apply(&json!({"type": "harness_retry"}), 30.0);
        assert!(s.blocks.len() == 1 && s.retryable().is_none() && s.working_for(40.0) == 10.0);
        s.apply(&json!({"type": "assistant", "message": {"content": [{"type": "text", "text": "Five, seven, five."}]}}), 50.0);
        s.apply(&json!({"type": "result", "subtype": "success", "result": "Five, seven, five.", "total_cost_usd": 0.01}), 60.0);
        assert!(s.retryable().is_none(), "answered");
        // text before a step is not the answer: a turn that said what it would do, then failed, is retried
        s.apply(&json!({"type": "harness_user", "text": "check the config"}), 70.0);
        s.apply(&json!({"type": "assistant", "message": {"content": [{"type": "text", "text": "I'll check the config."}, {"type": "tool_use", "id": "r1", "name": "Read", "input": {"file_path": "a"}}]}}), 71.0);
        s.apply(&json!({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": "r1", "content": "x"}]}}), 72.0);
        s.apply(&json!({"type": "result", "subtype": "error", "is_error": true, "result": "API Error: 529 overloaded", "total_cost_usd": 0.02}), 73.0);
        assert_eq!(s.retryable().map(|r| r.1), Some("check the config".to_string()));
        // a switch after it: its line stays, and the new reply comes under it
        s.apply(&json!({"type": "harness_switch", "model": "mock-coder-7b", "effort": "", "text": "model: Mock Coder 7B: the conversation continues on Mock Coder 7B"}), 74.0);
        s.apply(&json!({"type": "harness_retry"}), 75.0);
        let last: Vec<String> = s.view()["blocks"].as_array().unwrap().iter().rev().take(2).map(|b| b["text"].as_str().unwrap_or_default().to_string()).collect();
        assert_eq!(last, ["── continued on Mock Coder 7B ──", "check the config"]);
    }

    #[test]
    fn a_turns_changes_and_their_undo_come_back_with_the_recording() {
        let events = [json!({"type": "harness_user", "text": "change it"}), json!({"type": "result", "subtype": "success", "result": "done", "total_cost_usd": 0}),
            Change::event("t0", "t1", &[crate::turns::File { path: "NOTES.md".into(), added: Some(3), removed: Some(0), status: 'A' }]),
            json!({"type": "harness_undone", "after": "t1"})];
        let mut s = Session::new("", "high");
        for e in &events {
            s.apply(e, 0.0);
        }
        let c = &s.changes[0];
        assert_eq!((c.before.as_str(), c.after.as_str(), c.files[0].path.as_str(), c.files[0].status, c.at_block, c.undone), ("t0", "t1", "NOTES.md", 'A', 1, true));
    }

    #[test]
    fn a_step_runs_from_the_users_answer() {
        let mut s = Session::new("", "high");
        s.apply(&json!({"type": "assistant", "message": {"content": [{"type": "tool_use", "id": "c1", "name": "bash", "input": {"command": "ls -la"}}]}}), 1000.0);
        s.apply(&json!({"type": "harness_permission", "tool": "bash", "input": {"command": "ls -la"}}), 1010.0);
        s.apply(&json!({"type": "harness_permission_done", "tool": "bash", "allow": true, "mode": "default"}), 8000.0);
        s.apply(&json!({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": "c1", "content": "ok"}]}}), 8700.0);
        let Block::Tool(t) = &s.blocks[0] else { panic!() };
        assert_eq!(t.secs(), Some(0.7), "the 7 s of waiting for Allow are not the step's");
        // a question from inside a subagent: its call is not on this screen, and the subagent's own step keeps its time
        s.apply(&json!({"type": "assistant", "message": {"content": [{"type": "tool_use", "id": "t1", "name": "Task", "input": {"prompt": "look"}}]}}), 10_000.0);
        s.apply(&json!({"type": "harness_permission", "tool": "Bash", "input": {"command": "rm x"}}), 20_000.0);
        s.apply(&json!({"type": "harness_permission_done", "tool": "Bash", "allow": true, "mode": "default"}), 190_000.0);
        let Some(Block::Tool(t)) = s.blocks.iter().rev().find(|b| matches!(b, Block::Tool(x) if x.id == "t1")) else { panic!() };
        assert_eq!(t.started_ms, 10_000.0);
    }

    #[test]
    fn a_model_picked_before_the_first_message_is_the_one_the_route_line_names() {
        let mut s = Session::new("", "");
        s.apply(&json!({"type": "harness_route", "lane": "small", "model": "claude-sonnet-5-5", "effort": "medium", "reason": "tiny repo, 5 tracked files",
            "profile": "lean", "escalate": "claude-opus-5-5 high"}), 0.0);
        assert!(matches!(&s.blocks[0], Block::Notice { text, .. } if text.ends_with("· escalates to Opus 5.5 high if tests fail")));
        s.apply(&json!({"type": "harness_switch", "model": "mock-coder-7b", "effort": "", "text": "model: Mock Coder 7B from here on"}), 1.0);
        let Block::Notice { text, .. } = &s.blocks[0] else { panic!() };
        assert_eq!(crate::ui::lane_notice(text).as_deref(), Some("started on Mock Coder 7B · lean profile (tiny repo, 5 tracked files)"), "{text}");
        // a switch that carries the conversation: a line where it continues, in the window
        s.apply(&json!({"type": "harness_switch", "model": "claude-opus-5-5", "effort": "high", "text": "model: Opus 5.5 · high: the conversation continues on Opus 5.5"}), 2.0);
        assert_eq!(s.view()["blocks"][2]["text"], "── continued on Opus 5.5 ──");
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
    fn a_turn_keeps_how_it_was_paid_through_a_model_switch() {
        // a call as the own loop sends it (`charged`: what it says the call cost) or as claude does (None)
        let call = |s: &mut Session, t: f64, fresh: u64, cached: u64, out: u64, charged: Option<f64>| {
            s.apply(&json!({"type": "stream_event", "event": {"type": "message_start"}}), t);
            let mut d = json!({"type": "message_delta", "usage": {"input_tokens": fresh, "cache_creation_input_tokens": 0, "cache_read_input_tokens": cached, "output_tokens": out}});
            if let Some(c) = charged {
                d["tazuna_cost"] = json!(c);
            }
            s.apply(&json!({"type": "stream_event", "event": d}), t + 1.0);
        };
        let mut s = Session::new("", "high");
        // two calls on a provider's model, a billed result; then /model to Opus on the plan and one more turn. The own
        // loop's figures are not what the price list gives for the tokens: the first is the provider's own (`usage.cost`),
        // the second has a cache write's surcharge in it. The rows are those figures, and they add up to the run.
        s.apply(&json!({"type": "system", "subtype": "init", "model": "deepseek-v4.1-flash-1017", "price_per_m": [0.15, 0.003, 0.6], "provider": "SubRouter"}), 0.0);
        call(&mut s, 1.0, 10_000, 0, 500, Some(0.0021));
        call(&mut s, 3.0, 1_000, 10_000, 2_000, Some(0.00141));
        s.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 0.0021 + 0.00141, "tazuna_paid": true}), 5.0);
        let v = s.view();
        let cheap: Vec<f64> = v["calls"].as_array().unwrap().iter().map(|c| c["cost"].as_f64().unwrap()).collect();
        assert_eq!(cheap, [0.0021, 0.00141]);
        assert!((cheap[0] - s.estimate(&Usage { input: 10_000, cache_write: 0, cache_read: 0, output: 500 })).abs() > 1e-6, "not the list price of its tokens");
        let run = v["blocks"].as_array().unwrap().last().unwrap()["run_cost"].as_f64().unwrap();
        assert!((cheap.iter().sum::<f64>() - run).abs() < 1e-12 && v["calls"][0].get("estimate").is_none(), "{cheap:?} against {run}");
        s.apply(&json!({"type": "harness_switch", "model": "claude-opus-5-5", "effort": "medium", "text": "now Opus"}), 6.0);
        s.apply(&json!({"type": "system", "subtype": "init", "model": "claude-opus-5-5"}), 7.0);
        call(&mut s, 8.0, 2_000, 11_000, 300, None);
        s.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 0.02}), 10.0);
        let v = s.view();
        let done: Vec<&Value> = v["blocks"].as_array().unwrap().iter().filter(|b| b["kind"] == "done").collect();
        assert_eq!((&done[0]["model"], &done[0]["plan"], &done[0]["provider"], &done[0]["price_m"]), (&json!("deepseek-v4.1-flash-1017"), &json!(false), &json!("SubRouter"), &json!([0.15, 0.003, 0.6])));
        assert_eq!((&done[1]["model"], &done[1]["plan"], &done[1]["provider"], &done[1]["price_m"]), (&json!("claude-opus-5-5"), &json!(true), &Value::Null, &json!([4.0, 0.4, 20.0])));
        // the earlier calls cost what they cost then, each under its own model: not Opus's rates after the switch
        let calls = v["calls"].as_array().unwrap();
        assert_eq!(calls.iter().take(2).map(|c| c["cost"].as_f64().unwrap()).collect::<Vec<_>>(), cheap);
        assert_eq!((&calls[0]["model"], &calls[0]["provider"], &calls[2]["model"], &calls[2]["role"], &calls[2]["provider"]),
            (&json!("deepseek-v4.1-flash-1017"), &json!("SubRouter"), &json!("claude-opus-5-5"), &Value::Null, &Value::Null));
        // claude's own call: an estimate at list prices, and it says so (its exact total comes with the result)
        assert!((calls[2]["cost"].as_f64().unwrap() - Usage { input: 2_000, cache_write: 0, cache_read: 11_000, output: 300 }.estimate()).abs() < 1e-12);
        assert_eq!(calls[2]["estimate"], json!(true));
        // a route switch inside the own loop before any call used anything (the first route refused it): the run is the
        // new route's alone
        let mut r = Session::new("", "high");
        r.apply(&json!({"type": "system", "subtype": "init", "model": "glm-5.3", "price_per_m": [1.0, 0.1, 2.0], "provider": "SubRouter"}), 0.0);
        r.apply(&json!({"type": "stream_event", "event": {"type": "message_start"}}), 0.5);
        r.apply(&json!({"type": "system", "subtype": "route_switch", "message": "now https://openrouter.ai/api/v1: HTTP 503", "price_per_m": [2.0, 0.2, 4.0], "provider": "OpenRouter"}), 1.0);
        call(&mut r, 2.0, 1_000_000, 0, 0, Some(2.0));
        r.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 2.0, "tazuna_paid": true}), 4.0);
        let v = r.view();
        let done = v["blocks"].as_array().unwrap().iter().rfind(|b| b["kind"] == "done").unwrap();
        assert_eq!((&done["provider"], &done["price_m"], done.get("providers"), v["calls"][1]["cost"].as_f64()), (&json!("OpenRouter"), &json!([2.0, 0.2, 4.0]), None, Some(2.0)));
        // the route changes after a call was paid: two providers in one run. Nobody stands for the whole of it, no one
        // price list either: both are named in order, and each call keeps its own
        let mut m = Session::new("", "high");
        m.apply(&json!({"type": "system", "subtype": "init", "model": "glm-5.3", "price_per_m": [1.0, 0.1, 2.0], "provider": "SubRouter"}), 0.0);
        call(&mut m, 1.0, 1_000_000, 0, 0, Some(1.0));
        m.apply(&json!({"type": "system", "subtype": "route_switch", "message": "now https://openrouter.ai/api/v1: HTTP 503", "price_per_m": [2.0, 0.2, 4.0], "provider": "OpenRouter"}), 3.0);
        call(&mut m, 4.0, 1_000_000, 0, 0, Some(2.0));
        m.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 3.0, "tazuna_paid": true}), 6.0);
        // the next run stays on the second route: one provider again
        m.apply(&json!({"type": "harness_user", "text": "more"}), 7.0);
        call(&mut m, 8.0, 500_000, 0, 0, Some(1.0));
        m.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 4.0, "tazuna_paid": true}), 9.0);
        let v = m.view();
        let done: Vec<&Value> = v["blocks"].as_array().unwrap().iter().filter(|b| b["kind"] == "done").collect();
        assert_eq!((&done[0]["provider"], &done[0]["price_m"], &done[0]["providers"], &done[0]["plan"]), (&Value::Null, &Value::Null, &json!(["SubRouter", "OpenRouter"]), &json!(false)));
        assert_eq!((&v["calls"][0]["provider"], &v["calls"][1]["provider"]), (&json!("SubRouter"), &json!("OpenRouter")));
        assert_eq!((&done[1]["provider"], done[1].get("providers")), (&json!("OpenRouter"), None));
    }

    #[test]
    fn a_claude_run_on_an_api_key_is_paid_not_on_the_plan() {
        // claude's init names where its key comes from: "none" is the subscription's login
        let run = |source: Value| {
            let mut s = Session::new("", "medium");
            s.apply(&json!({"type": "system", "subtype": "init", "model": "claude-opus-5-5", "apiKeySource": source}), 0.0);
            s.apply(&json!({"type": "stream_event", "event": {"type": "message_start"}}), 0.5);
            s.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 0.4}), 1.0);
            let v = s.view();
            let d = v["blocks"].as_array().unwrap().last().unwrap().clone();
            (d["plan"].clone(), d["provider"].clone(), d["price_m"].clone(), v["calls"][0]["provider"].clone())
        };
        assert_eq!(run(json!("ANTHROPIC_API_KEY")), (json!(false), json!("Anthropic API"), json!([4.0, 0.4, 20.0]), json!("Anthropic API")));
        assert_eq!(run(json!("none")), (json!(true), Value::Null, json!([4.0, 0.4, 20.0]), Value::Null));
        assert_eq!(run(Value::Null).0, json!(true), "a recording whose init does not say");
    }

    #[test]
    fn a_runs_subagent_calls_are_counted_for_its_receipt() {
        let mut s = Session::new("", "medium");
        s.apply(&json!({"type": "system", "subtype": "init", "model": "claude-opus-5-5"}), 0.0);
        s.apply(&json!({"type": "harness_user", "text": "look"}), 1.0);
        s.apply(&json!({"type": "stream_event", "event": {"type": "message_start"}}), 2.0);
        s.apply(&json!({"type": "assistant", "message": {"content": [{"type": "tool_use", "id": "t1", "name": "Agent", "input": {"description": "explore"}}]}}), 3.0);
        // the subagent: two model calls, the first in two parts (one message id)
        for (id, name) in [("m1", "Read"), ("m1", "Grep"), ("m2", "Read")] {
            s.apply(&json!({"type": "assistant", "parent_tool_use_id": "t1", "message": {"id": id, "content": [{"type": "tool_use", "id": "x", "name": name, "input": {"path": "a"}}]}}), 4.0);
        }
        s.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 0.4}), 9.0);
        s.apply(&json!({"type": "harness_user", "text": "thanks"}), 10.0);
        s.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 0.5}), 11.0);
        let v = s.view();
        let done: Vec<&Value> = v["blocks"].as_array().unwrap().iter().filter(|b| b["kind"] == "done").collect();
        assert_eq!((&done[0]["sub_calls"], done[1].get("sub_calls"), v["calls"].as_array().unwrap().len()), (&json!(2), None, 1));
    }

    /// A send the app refused: its notice says so in the view (`rejected`), an error before it does not.
    #[test]
    fn a_refused_sends_notice_is_marked_in_the_view() {
        let mut s = Session::new("", "");
        s.apply(&json!({"type": "harness_info", "text": "the model's process exited", "error": true}), 0.0);
        let from = s.blocks.len();
        s.apply(&json!({"type": "harness_user", "text": "hi"}), 1.0);
        s.apply(&json!({"type": "harness_info", "text": crate::models::CLAUDE_MISSING, "error": true, "ended": true}), 2.0);
        s.mark_rejected(from);
        let v = s.view();
        let notices: Vec<&Value> = v["blocks"].as_array().unwrap().iter().filter(|b| b["kind"] == "notice").collect();
        assert_eq!(notices.iter().map(|b| b.get("rejected").cloned()).collect::<Vec<_>>(), [None, Some(json!(true))], "{notices:?}");
        assert_eq!((notices[1]["error"].as_bool(), notices[1]["ended"].as_bool()), (Some(true), Some(true)), "the other fields as they were");
        // a mode picked just before the send says so first: only the refusal's own notice is marked
        let mut s = Session::new("", "");
        s.apply(&json!({"type": "harness_user", "text": "a"}), 0.0);
        s.apply(&json!({"type": "result", "subtype": "success", "result": "ok", "total_cost_usd": 0.0}), 1.0);
        let from = s.blocks.len();
        s.apply(&json!({"type": "harness_info", "text": "mode: ask"}), 2.0);
        s.apply(&json!({"type": "harness_info", "text": "refused", "error": true, "ended": true}), 3.0);
        s.mark_rejected(from);
        let marked = |s: &Session| s.view()["blocks"].as_array().unwrap().iter().filter(|b| b["rejected"] == true).map(|b| b["text"].as_str().unwrap().to_string()).collect::<Vec<_>>();
        assert_eq!(marked(&s), ["refused"]);
        // blocks before it go (a retry of the message before it): the mark stays on its notice
        s.apply(&json!({"type": "harness_retry"}), 4.0);
        assert_eq!(marked(&s), ["refused"], "{:?}", s.rejected);
        // and goes with it (/rewind)
        let n = s.blocks.len() - 1;
        s.truncate(n);
        assert!(marked(&s).is_empty() && s.rejected.is_empty());
    }

    /// A queued message the stop gave back to the box says so in its place, and the blocks after it keep theirs.
    #[test]
    fn a_queued_message_given_back_says_so_in_its_place() {
        let mut s = Session::new("", "");
        s.apply(&json!({"type": "harness_user", "text": "slow one"}), 0.0);
        s.apply(&json!({"type": "harness_user", "text": "a follow up\nwith a second line"}), 1.0);
        s.apply(&json!({"type": "assistant", "message": {"content": [{"type": "text", "text": "Thinking slowly."}]}}), 2.0);
        let n = s.blocks.len();
        s.apply(&json!({"type": "harness_unqueued", "text": "a follow up\nwith a second line"}), 3.0);
        assert_eq!(s.blocks.len(), n);
        assert!(s.blocks.iter().any(|b| matches!(b, Block::Notice { text, .. } if text == "not sent, back in the box: a follow up…")));
        assert!(!s.blocks.iter().any(|b| matches!(b, Block::Prompt(p) if p.starts_with("a follow up"))) && s.last_prompt == "slow one");
    }

    #[test]
    fn working_counts_the_tests_a_fix_round_a_queued_message_and_stops_with_the_run() {
        let call = |s: &mut Session, t: f64| s.apply(&json!({"type": "stream_event", "event": {"type": "message_start"}}), t);
        let result = |s: &mut Session, t: f64| s.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 0.1}), t);
        let mut s = Session::new("", "");
        s.apply(&json!({"type": "harness_user", "text": "a"}), 10_000.0);
        call(&mut s, 10_500.0);
        assert_eq!(s.working_for(15_000.0), 5_000.0);
        result(&mut s, 20_000.0);
        assert_eq!((s.working_for(20_050.0), s.busy(20_050.0)), (0.0, 10_000.0));
        // the tests after the result count from their start ("Working 0 s" stood through a verification), and add to
        // the work done; so does the round that fixes them
        s.apply(&json!({"type": "harness_verify_start", "cmd": "cargo test", "round": 1}), 20_100.0);
        assert_eq!((s.working_for(24_000.0), s.busy(24_000.0)), (3_900.0, 13_900.0));
        s.apply(&json!({"type": "harness_verify_done", "ok": false, "secs": 6.0, "output": "", "final": false}), 26_100.0);
        assert_eq!((s.working_for(26_150.0), s.busy(26_150.0)), (0.0, 16_000.0));
        s.apply(&json!({"type": "harness_verify_feedback", "round": 1, "max": 2}), 26_200.0);
        call(&mut s, 26_300.0);
        assert_eq!(s.working_for(30_000.0), 3_800.0);
        result(&mut s, 31_000.0);
        assert_eq!(s.busy(40_000.0), 20_800.0);
        // the next message, an hour on: its own count
        s.apply(&json!({"type": "harness_user", "text": "b"}), 3_600_000.0);
        assert_eq!(s.working_for(3_602_000.0), 2_000.0);
        // a second message queued while the first runs: the first's result ends that count, the queued one's call starts
        // its own (it stood at 0 s)
        s.apply(&json!({"type": "harness_user", "text": "c"}), 3_603_000.0);
        assert_eq!(s.working_for(3_604_000.0), 4_000.0);
        result(&mut s, 3_605_000.0);
        call(&mut s, 3_605_100.0);
        assert_eq!(s.working_for(3_610_000.0), 4_900.0);
        // a hard stop (no result): the run ends there, and the next message counts from itself, not from the killed one
        s.apply(&json!({"type": "harness_notice", "text": "stopped · send a message to continue", "error": false}), 3_611_000.0);
        let worked = s.busy(3_611_000.0);
        assert_eq!((s.working_for(3_700_000.0), s.busy(3_700_000.0)), (0.0, worked), "nothing runs after the stop");
        s.apply(&json!({"type": "harness_user", "text": "d"}), 3_900_000.0);
        assert_eq!(s.working_for(3_903_000.0), 3_000.0);
        // the process gone mid-run: the same
        s.apply(&json!({"type": "harness_notice", "text": "claude exited (code 1)", "error": true}), 3_904_000.0);
        assert_eq!((s.working_for(3_950_000.0), s.busy(3_950_000.0)), (0.0, worked + 4_000.0));
    }

    #[test]
    fn an_old_recording_says_nothing_it_does_not_hold() {
        // no init event (a result alone): no billing fields at all, and a call without its model
        let mut s = Session::new("", "low");
        s.apply(&json!({"type": "stream_event", "event": {"type": "message_start"}}), 0.5);
        s.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 0.01}), 1.0);
        let v = s.view();
        let done = v["blocks"].as_array().unwrap().last().unwrap().as_object().unwrap();
        assert!(!["model", "plan", "provider", "price_m"].iter().any(|k| done.contains_key(*k)), "{done:?}");
        assert!(v["calls"][0].get("model").is_none());
        // the own loop before 0.1.193: its init names the model and its prices, not who was paid: nobody is named
        let mut o = Session::new("", "high");
        o.apply(&json!({"type": "system", "subtype": "init", "model": "deepseek-v4.1-flash-1017", "price_per_m": [0.15, 0.003, 0.6]}), 0.0);
        o.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 0.01}), 1.0);
        let v = o.view();
        let done = v["blocks"].as_array().unwrap().last().unwrap();
        assert_eq!((&done["plan"], &done["provider"], &done["price_m"]), (&json!(false), &Value::Null, &json!([0.15, 0.003, 0.6])));
        // a Claude model the core has no list prices for: on the plan, no prices, and its calls without a model (their
        // cost is a rough figure)
        let mut c = Session::new("", "medium");
        c.apply(&json!({"type": "system", "subtype": "init", "model": "claude-sonnet-5-5"}), 0.0);
        c.apply(&json!({"type": "stream_event", "event": {"type": "message_start"}}), 0.5);
        c.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 0.01}), 1.0);
        let v = c.view();
        let done = v["blocks"].as_array().unwrap().last().unwrap();
        assert_eq!((&done["plan"], &done["price_m"], v["calls"][0].get("model")), (&json!(true), &Value::Null, None));
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
        // under each answer its own share, not the total so far: $0.01, $0.02, $0.005 (design review 6); the stopped run in
        // between ends on its own
        let own: Vec<f64> = s.blocks.iter().filter_map(|b| if let Block::Done(d) = b { d.run_cost } else { None }).collect();
        assert_eq!(own.len(), 4);
        assert!((own[0] - 0.01).abs() < 1e-9 && (own[1] - 0.02).abs() < 1e-9 && own[2] == 0.0 && (own[3] - 0.005).abs() < 1e-9, "{own:?}");
        assert!(matches!(&s.blocks[3], Block::Done(d) if !d.stopped), "a notice that is no stop of the user's: not `stopped`");
        assert!(s.view()["calls"][0]["cost"].is_null(), "the call cut off: its cost is not known");
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

    /// A model call as the own loop sends it: `charged` what it says the call cost.
    fn paid_call(s: &mut Session, t: f64, fresh: u64, cached: u64, out: u64, charged: f64) {
        s.apply(&json!({"type": "stream_event", "event": {"type": "message_start"}}), t);
        s.apply(&json!({"type": "stream_event", "event": {"type": "message_delta", "tazuna_cost": charged,
            "usage": {"input_tokens": fresh, "cache_creation_input_tokens": 0, "cache_read_input_tokens": cached, "output_tokens": out}}}), t + 1.0);
    }

    fn dones(v: &Value) -> Vec<(usize, Value)> {
        v["blocks"].as_array().unwrap().iter().enumerate().filter(|(_, b)| b["kind"] == "done").map(|(i, b)| (i, b.clone())).collect()
    }

    #[test]
    fn a_hard_stop_closes_its_run_into_its_own_receipt() {
        // two calls on SubRouter ($1.00, $0.10) and a third cut off, esc twice (the process killed: no result), a new message
        let mut s = Session::new("", "high");
        s.apply(&json!({"type": "system", "subtype": "init", "model": "glm-5.3", "price_per_m": [1.0, 0.1, 2.0], "provider": "SubRouter"}), 0.0);
        s.apply(&json!({"type": "harness_user", "text": "a"}), 1.0);
        paid_call(&mut s, 2.0, 1000, 0, 10, 1.0);
        paid_call(&mut s, 4.0, 1000, 0, 10, 0.1);
        s.apply(&json!({"type": "stream_event", "event": {"type": "message_start"}}), 6.0);
        s.apply(&json!({"type": "harness_interrupt"}), 7.0);
        s.apply(&json!({"type": "harness_notice", "text": "stopped · send a message to continue", "error": false}), 8.0);
        assert!((s.cost() - 1.1).abs() < 1e-9, "the stopped run's spend stays in the conversation's total: {}", s.cost());
        s.apply(&json!({"type": "harness_user", "text": "b"}), 10.0);
        paid_call(&mut s, 11.0, 1000, 0, 10, 0.1);
        s.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 0.1, "tazuna_paid": true}), 13.0);
        assert!((s.cost() - 1.2).abs() < 1e-9, "it fell to $0.10: {}", s.cost());
        let v = s.view();
        let d = dones(&v);
        assert_eq!(d.len(), 2, "the stopped run has a receipt of its own");
        assert_eq!((&d[0].1["stopped"], &d[0].1["ok"], &d[0].1["provider"], &d[0].1["plan"], d[0].1.get("estimate")), (&json!(true), &json!(false), &json!("SubRouter"), &json!(false), None));
        assert!((d[0].1["run_cost"].as_f64().unwrap() - 1.1).abs() < 1e-9 && (d[1].1["run_cost"].as_f64().unwrap() - 0.1).abs() < 1e-9, "{:?}", d);
        // the next answer's receipt lists its own call alone (it listed $1.00, $0.10, $0.10 under "Paid to SubRouter $0.10")
        let calls = v["calls"].as_array().unwrap();
        // a call goes to the first done block from where it began (`block`: the blocks there were then)
        let under = |lo: usize, hi: usize| calls.iter().filter(|c| (lo..=hi).contains(&(c["block"].as_u64().unwrap() as usize))).map(|c| c["cost"].clone()).collect::<Vec<_>>();
        assert_eq!(under(0, d[0].0), [json!(1.0), json!(0.1), Value::Null], "the call cut off: its cost is not known");
        assert_eq!(under(d[0].0 + 1, d[1].0), [json!(0.1)]);
        // a stop and then /model: the old calls stay under their own payer
        let mut m = Session::new("", "high");
        m.apply(&json!({"type": "system", "subtype": "init", "model": "glm-5.3", "price_per_m": [1.0, 0.1, 2.0], "provider": "SubRouter"}), 0.0);
        m.apply(&json!({"type": "harness_user", "text": "a"}), 1.0);
        paid_call(&mut m, 2.0, 1000, 0, 10, 1.0);
        m.apply(&json!({"type": "harness_notice", "text": "stopped · send a message to continue", "error": false}), 4.0);
        m.apply(&json!({"type": "harness_switch", "model": "claude-opus-5-5", "effort": "high", "text": "model: Opus"}), 5.0);
        m.apply(&json!({"type": "system", "subtype": "init", "model": "claude-opus-5-5", "apiKeySource": "none"}), 6.0);
        m.apply(&json!({"type": "harness_user", "text": "b"}), 7.0);
        m.apply(&json!({"type": "stream_event", "event": {"type": "message_start"}}), 8.0);
        m.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 0.3}), 9.0);
        let d = dones(&m.view());
        assert_eq!((&d[0].1["provider"], &d[1].1["plan"]), (&json!("SubRouter"), &json!(true)));
        assert!((m.cost() - 1.3).abs() < 1e-9 && (m.split_cost().0 - 1.0).abs() < 1e-9, "{:?}", m.split_cost());
        // a crash mid-run on claude: its calls at list prices, said to be an estimate, and the error is no stop of the user's
        let mut c = Session::new("", "medium");
        c.apply(&json!({"type": "system", "subtype": "init", "model": "claude-opus-5-5", "apiKeySource": "none"}), 0.0);
        c.apply(&json!({"type": "harness_user", "text": "a"}), 1.0);
        c.apply(&json!({"type": "stream_event", "event": {"type": "message_start"}}), 2.0);
        c.apply(&json!({"type": "stream_event", "event": {"type": "message_delta", "usage": {"input_tokens": 1000, "output_tokens": 100}}}), 3.0);
        c.apply(&json!({"type": "harness_notice", "text": "claude exited (code 1)", "error": true}), 4.0);
        let d = dones(&c.view());
        assert_eq!((&d[0].1["stopped"], &d[0].1["estimate"], &d[0].1["plan"]), (&json!(false), &json!(true), &json!(true)));
        assert!((d[0].1["run_cost"].as_f64().unwrap() - 0.006).abs() < 1e-9 && (c.cost() - 0.006).abs() < 1e-9);
        // a restart while nothing runs closes no run
        c.apply(&json!({"type": "harness_notice", "text": "── new conversation ──", "error": false}), 5.0);
        assert_eq!(dones(&c.view()).len(), 1);
    }

    #[test]
    fn a_turn_after_clear_is_its_own() {
        // claude counts from $0 again after /clear: the next turn's receipt is its own tokens and cost, not the
        // conversation's ("$0 … The same without the cache $4.14")
        let mut s = Session::new("", "medium");
        s.apply(&json!({"type": "system", "subtype": "init", "model": "claude-opus-5-5", "apiKeySource": "none"}), 0.0);
        s.apply(&json!({"type": "harness_user", "text": "a"}), 1.0);
        s.apply(&json!({"type": "stream_event", "event": {"type": "message_start"}}), 2.0);
        s.apply(&json!({"type": "stream_event", "event": {"type": "message_delta", "usage": {"input_tokens": 500_000, "cache_read_input_tokens": 9_000_000, "output_tokens": 20_000}}}), 3.0);
        s.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 4.0}), 4.0);
        s.apply(&json!({"type": "harness_user", "text": "/clear"}), 5.0);
        s.apply(&json!({"type": "result", "subtype": "success", "num_turns": 0, "total_cost_usd": 0.0}), 6.0);
        s.apply(&json!({"type": "harness_user", "text": "hi"}), 7.0);
        s.apply(&json!({"type": "stream_event", "event": {"type": "message_start"}}), 8.0);
        s.apply(&json!({"type": "stream_event", "event": {"type": "message_delta", "usage": {"input_tokens": 2_000, "cache_read_input_tokens": 0, "output_tokens": 50}}}), 9.0);
        s.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 0.01}), 10.0);
        let d: Vec<&Block> = s.blocks.iter().filter(|b| matches!(b, Block::Done(_))).collect();
        let [Block::Done(_), Block::Done(clear), Block::Done(hi)] = d[..] else { panic!() };
        assert_eq!((clear.run_cost, clear.run_usage.total_in()), (Some(0.0), 0));
        assert_eq!((hi.run_usage.input, hi.run_usage.cache_read, hi.run_usage.output), (2_000, 0, 50));
        assert!((hi.run_cost.unwrap() - 0.01).abs() < 1e-9, "{:?}", hi.run_cost);
        assert!((s.cost() - 4.01).abs() < 1e-9, "what was reported before the clear stays: {}", s.cost());
    }

    #[test]
    fn a_stopped_turn_says_its_cost_is_an_estimate_or_not_known() {
        // claude reports $0 for a stopped turn: tazuna's figure at list prices is "about", and on a model without list
        // prices in the core no amount is claimed (it was Opus's rates, shown as "Paid to Anthropic API $0.18")
        let stopped = |model: &str| {
            let mut s = Session::new("", "medium");
            s.apply(&json!({"type": "system", "subtype": "init", "model": model, "apiKeySource": "ANTHROPIC_API_KEY"}), 0.0);
            s.apply(&json!({"type": "harness_user", "text": "a"}), 1.0);
            s.apply(&json!({"type": "stream_event", "event": {"type": "message_start"}}), 2.0);
            s.apply(&json!({"type": "stream_event", "event": {"type": "message_delta", "usage": {"input_tokens": 20_000, "output_tokens": 5_000}}}), 3.0);
            s.apply(&json!({"type": "harness_interrupt"}), 4.0);
            s.apply(&json!({"type": "result", "subtype": "error_during_execution", "total_cost_usd": 0.0}), 5.0);
            let v = s.view();
            (dones(&v)[0].1.clone(), s.cost())
        };
        let (opus, total) = stopped("claude-opus-5-5");
        assert_eq!((&opus["estimate"], &opus["provider"], &opus["stopped"]), (&json!(true), &json!("Anthropic API"), &json!(true)));
        assert!((opus["run_cost"].as_f64().unwrap() - 0.18).abs() < 1e-9 && (total - 0.18).abs() < 1e-9);
        let (sonnet, total) = stopped("claude-sonnet-5-5");
        assert_eq!((&sonnet["estimate"], &sonnet["run_cost"], &sonnet["price_m"]), (&json!(true), &Value::Null, &Value::Null));
        assert_eq!(total, 0.0, "nor in the total");
        // a turn that ended as reported says nothing of the kind
        let mut s = Session::new("", "medium");
        s.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 0.1}), 1.0);
        assert!(dones(&s.view())[0].1.get("estimate").is_none());
    }

    #[test]
    fn bedrock_vertex_or_a_gateway_is_not_the_plan() {
        // claude says apiKeySource "none" there too: tazuna writes what it saw set into the init (live.rs)
        let run = |env: Value| {
            let mut s = Session::new("", "medium");
            s.apply(&json!({"type": "system", "subtype": "init", "model": "claude-opus-5-5", "apiKeySource": "none", "tazuna_env": env}), 0.0);
            s.apply(&json!({"type": "stream_event", "event": {"type": "message_start"}}), 0.5);
            s.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 0.4}), 1.0);
            let d = dones(&s.view())[0].1.clone();
            (d["plan"].clone(), d["provider"].clone(), crate::ui::cost_str(&s))
        };
        assert_eq!(run(json!(["CLAUDE_CODE_USE_BEDROCK"])), (json!(false), json!("Bedrock"), "$0.40".to_string()));
        assert_eq!(run(json!(["CLAUDE_CODE_USE_VERTEX"])), (json!(false), json!("Vertex"), "$0.40".to_string()));
        assert_eq!(run(json!(["ANTHROPIC_BASE_URL"])), (json!(false), Value::Null, "$0.40".to_string()), "a gateway: billed, the payer not known");
        assert_eq!(run(json!([])), (json!(true), Value::Null, "$0.40 on Claude plan".to_string()));
    }

    #[test]
    fn a_chat_answer_carries_the_sessions_billing() {
        // a session on a `claude-…` id through SubRouter (the own loop); the router answers a question as a chat, whose
        // process's init main.rs drops: it is paid to SubRouter, and its cost is its own beside the session's process
        let mut s = Session::new("", "high");
        s.apply(&json!({"type": "system", "subtype": "init", "model": "claude-opus-5-5", "price_per_m": [5.0, 0.5, 25.0], "provider": "SubRouter"}), 0.0);
        s.apply(&json!({"type": "harness_user", "text": "fix it"}), 1.0);
        paid_call(&mut s, 2.0, 100_000, 0, 1_000, 1.0);
        s.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 1.0, "tazuna_paid": true}), 4.0);
        s.apply(&json!({"type": "harness_user", "text": "what is a monad"}), 5.0);
        paid_call(&mut s, 6.0, 2_000, 0, 400, 0.02);
        s.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 0.02, "tazuna_chat": true}), 8.0);
        // the session's process goes on: its next result is cumulative ($1.30 = $1.00 + $0.30)
        s.apply(&json!({"type": "harness_user", "text": "and the tests"}), 9.0);
        paid_call(&mut s, 10.0, 30_000, 0, 500, 0.3);
        s.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 1.3, "tazuna_paid": true}), 12.0);
        let d = dones(&s.view());
        assert_eq!((&d[1].1["plan"], &d[1].1["provider"], d[1].1.get("estimate")), (&json!(false), &json!("SubRouter"), None), "it said: not billed");
        let own: Vec<f64> = d.iter().map(|(_, b)| b["run_cost"].as_f64().unwrap()).collect();
        assert!((own[1] - 0.02).abs() < 1e-9 && (own[2] - 0.3).abs() < 1e-9 && (s.cost() - 1.32).abs() < 1e-9, "{own:?} {}", s.cost());
    }

    #[test]
    fn a_background_subagents_later_calls_count_in_the_next_turn() {
        // a subagent that goes on after its turn ended: its count on the first turn is fixed there, the calls after count in
        // the turn whose total they are in (claude's next result)
        let sub = |s: &mut Session, id: &str, t: f64| s.apply(&json!({"type": "assistant", "parent_tool_use_id": "t1", "message": {"id": id, "content": [{"type": "text", "text": "…"}]}}), t);
        let mut s = Session::new("", "medium");
        s.apply(&json!({"type": "system", "subtype": "init", "model": "claude-opus-5-5"}), 0.0);
        s.apply(&json!({"type": "harness_user", "text": "look around in the background"}), 1.0);
        s.apply(&json!({"type": "assistant", "message": {"content": [{"type": "tool_use", "id": "t1", "name": "Agent", "input": {"run_in_background": true}}]}}), 2.0);
        sub(&mut s, "m1", 3.0);
        s.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 0.4}), 4.0);
        sub(&mut s, "m2", 5.0);
        sub(&mut s, "m3", 6.0);
        let first = |s: &Session| dones(&s.view())[0].1["sub_calls"].clone();
        assert_eq!(first(&s), json!(1), "it kept growing after the turn ended");
        s.apply(&json!({"type": "harness_user", "text": "thanks"}), 7.0);
        s.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 0.6}), 8.0);
        assert_eq!((first(&s), &dones(&s.view())[1].1["sub_calls"]), (json!(1), &json!(2)));
    }

    #[test]
    fn working_starts_at_a_hand_over_a_review_and_a_held_message() {
        let result = |s: &mut Session, t: f64| s.apply(&json!({"type": "result", "subtype": "success", "total_cost_usd": 0.1}), t);
        let mut s = Session::new("", "");
        s.apply(&json!({"type": "harness_user", "text": "a"}), 1_000.0);
        result(&mut s, 2_000.0);
        // the cascade hands the task over to the stronger model: work, before that model's first call
        s.apply(&json!({"type": "harness_escalate", "model": "claude-opus-5-5", "effort": "high"}), 3_000.0);
        assert_eq!(s.working_for(5_000.0), 2_000.0, "Working 0 s");
        result(&mut s, 6_000.0);
        // a self-review round
        s.apply(&json!({"type": "harness_review"}), 7_000.0);
        assert_eq!(s.working_for(8_000.0), 1_000.0);
        result(&mut s, 9_000.0);
        // a message sent during the tests waits for them, then starts
        s.apply(&json!({"type": "harness_verify_start", "cmd": "cargo test", "round": 1}), 10_000.0);
        s.apply(&json!({"type": "harness_user", "text": "b"}), 11_000.0);
        s.apply(&json!({"type": "harness_verify_done", "ok": true, "secs": 2.0, "output": "", "final": true}), 12_000.0);
        assert_eq!(s.working_for(12_500.0), 0.0);
        s.apply(&json!({"type": "harness_held_start"}), 13_000.0);
        assert_eq!(s.working_for(14_000.0), 1_000.0);
    }

    #[test]
    fn a_chats_hand_off_never_shows_and_its_cost_joins_the_agents_answer() {
        // a chat-profile session on the own loop: the chat streams [[agent]], the agent answers in a process of its own
        let ev = |e: Value| json!({"type": "stream_event", "event": e});
        let delta = |t: &str| ev(json!({"type": "content_block_delta", "delta": {"type": "text_delta", "text": t}}));
        let shown = |s: &Session| s.view()["blocks"].as_array().unwrap().iter().filter(|b| b["kind"] == "text").map(|b| b["text"].as_str().unwrap().to_string()).collect::<String>();
        let mut s = Session::new("", "high");
        s.profile = "chat".into();
        s.apply(&json!({"type": "system", "subtype": "init", "model": "glm-5.3", "price_per_m": [1.0, 0.1, 2.0], "provider": "SubRouter"}), 0.0);
        s.apply(&json!({"type": "harness_user", "text": "weather in Berlin now?"}), 1.0);
        paid_call(&mut s, 2.0, 1000, 0, 5, 0.001);
        s.apply(&ev(json!({"type": "content_block_start", "content_block": {"type": "text"}})), 3.5);
        for (i, part) in ["\n[", "[age", "nt]", "]"].into_iter().enumerate() {
            s.apply(&delta(part), 4.0 + i as f64);
            assert_eq!(shown(&s), "", "the marker never flashes ({part:?})");
        }
        s.apply(&ev(json!({"type": "content_block_stop"})), 8.0);
        s.apply(&json!({"type": "result", "subtype": "success", "result": "[[agent]]", "total_cost_usd": 0.001, "tazuna_paid": true}), 9.0);
        s.apply(&json!({"type": "harness_handoff", "text": crate::live::HANDOFF_NEEDS}), 10.0);
        // the chat's reply and its receipt are gone; the line stands over what the agent does next
        assert!(!s.blocks.iter().any(|b| matches!(b, Block::Done(_) | Block::Text { .. })));
        assert!(matches!(s.blocks.last(), Some(Block::Notice { text, error: false, .. }) if text == crate::live::HANDOFF_NEEDS));
        assert!(s.working_for(11.0) > 0.0 && s.activity == Activity::Waiting, "at work from the hand-off on");
        paid_call(&mut s, 12.0, 3000, 0, 50, 0.02);
        s.apply(&json!({"type": "assistant", "message": {"content": [{"type": "text", "text": "12 °C and rain."}]}}), 13.5);
        s.apply(&json!({"type": "result", "subtype": "success", "result": "12 °C and rain.", "total_cost_usd": 0.02, "tazuna_paid": true}), 14.0);
        assert_eq!(shown(&s), "12 °C and rain.");
        let d = dones(&s.view());
        assert_eq!(d.len(), 1, "one answer");
        assert!((d[0].1["run_cost"].as_f64().unwrap() - 0.021).abs() < 1e-9 && (s.cost() - 0.021).abs() < 1e-9, "the chat's call in it: {} {}", d[0].1["run_cost"], s.cost());
        // the chat again, in a new process: its count starts over and adds to the total
        s.apply(&json!({"type": "harness_process_end"}), 15.0);
        s.apply(&json!({"type": "harness_user", "text": "and is [1] a list?"}), 16.0);
        paid_call(&mut s, 17.0, 1000, 0, 5, 0.001);
        s.apply(&ev(json!({"type": "content_block_start", "content_block": {"type": "text"}})), 18.5);
        s.apply(&delta("["), 19.0);
        assert_eq!(shown(&s), "12 °C and rain.", "held while it may still become the marker");
        s.apply(&delta("1] is a list."), 20.0);
        assert_eq!(shown(&s), "12 °C and rain.[1] is a list.", "an ordinary reply shows once it is clear");
        s.apply(&json!({"type": "result", "subtype": "success", "result": "[1] is a list.", "total_cost_usd": 0.001, "tazuna_paid": true}), 21.0);
        assert!((s.cost() - 0.022).abs() < 1e-9 && dones(&s.view()).iter().all(|(_, b)| b.get("estimate").is_none()), "{}", s.cost());
        // sent to the agent (/agent, "send to agent"): no chat reply came, nothing goes
        s.apply(&json!({"type": "harness_user", "text": "how full is my disk?"}), 22.0);
        s.apply(&json!({"type": "harness_handoff", "text": crate::live::HANDOFF_SENT}), 22.5);
        assert_eq!(dones(&s.view()).len(), 2);
        assert!(matches!(&s.blocks[s.blocks.len() - 2], Block::Prompt(p) if p.starts_with("how full")));
    }

    /// Two router chats that overlap: the second hands its question over after the first's answer came in under it. Only the
    /// second's reply goes (its [[agent]], receipt and "answered as a chat" line), not the first's answer.
    #[test]
    fn a_hand_off_takes_away_only_its_own_chats_reply() {
        let mut s = Session::new("", "high");
        for q in ["capital of France?", "weather in Berlin now?"] {
            s.apply(&json!({"type": "harness_user", "text": q}), 1.0);
            s.apply(&json!({"type": "harness_info", "text": "answered as a chat: not about this project"}), 1.5);
        }
        for a in ["Paris.", "[[agent]]"] {
            s.apply(&json!({"type": "assistant", "message": {"content": [{"type": "text", "text": a}]}}), 2.0);
            s.apply(&json!({"type": "result", "subtype": "success", "result": a, "total_cost_usd": 0.001, "tazuna_chat": true}), 3.0);
        }
        s.apply(&json!({"type": "harness_handoff", "text": crate::live::HANDOFF_NEEDS, "q": "weather in Berlin now?"}), 4.0);
        let texts: Vec<&str> = s.blocks.iter().filter_map(|b| if let Block::Text { text, .. } = b { Some(text.as_str()) } else { None }).collect();
        assert_eq!(texts, ["Paris."], "the first chat's answer stays");
        assert_eq!(s.blocks.iter().filter(|b| matches!(b, Block::Done(_))).count(), 1);
        assert_eq!(s.blocks.iter().filter(|b| matches!(b, Block::Notice { text, .. } if text.starts_with("answered as a chat"))).count(), 1);
        assert!(matches!(s.blocks.last(), Some(Block::Notice { text, .. }) if text == crate::live::HANDOFF_NEEDS));
    }

    #[test]
    fn only_a_chats_reply_hides_the_marker() {
        let text = |s: &Session| s.view()["blocks"].as_array().unwrap().iter().filter(|b| b["kind"] == "text").map(|b| b["text"].as_str().unwrap().to_string()).collect::<Vec<_>>();
        let reply = |s: &mut Session, q: &str, line: Option<&str>, a: &str| {
            s.apply(&json!({"type": "harness_user", "text": q}), 1.0);
            if let Some(l) = line {
                s.apply(&json!({"type": "harness_info", "text": l}), 1.5);
            }
            s.apply(&json!({"type": "assistant", "message": {"content": [{"type": "text", "text": a}]}}), 2.0);
        };
        // a coding agent's answer that starts with the marker is an answer (it was blanked)
        let mut s = Session::new("", "high");
        reply(&mut s, "what does [[agent]] do in tazuna?", None, "[[agent]] is the chat's hand-off marker.");
        assert_eq!(text(&s), ["[[agent]] is the chat's hand-off marker."]);
        // a router chat answer's hand-off never shows, nor does the coding answer before it go
        reply(&mut s, "weather in Berlin now?", Some("answered as a chat: not about this project"), "[[agent]]");
        assert_eq!(text(&s), ["[[agent]] is the chat's hand-off marker.", ""]);
        // a chat-profile session's reply, and the agent's answer in its place under the hand-off's line
        let mut s = Session::new("", "high");
        s.profile = "chat".into();
        reply(&mut s, "weather in Berlin now?", None, "[[agent]]");
        assert_eq!(text(&s), [""]);
        reply(&mut s, "and this one?", Some(crate::live::HANDOFF_SENT), "[[agent]] is a marker; here is the answer.");
        assert_eq!(text(&s), ["", "[[agent]] is a marker; here is the answer."]);
    }
}
