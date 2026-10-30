//! `tazuna engine`: tazuna's own agent loop for API providers (docs/OWN_LOOP.md). It reads and writes claude's stream-json
//! on stdin/stdout, so `Live` drives it like the `claude` process: the run log, verification and the reviewer work
//! unchanged. Each turn is one streamed call: Chat Completions, OpenAI's Responses or Anthropic's Messages API (see `Api`).

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    io::{BufRead, Write},
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{Receiver, TryRecvError},
    },
    time::Duration,
};

/// UnoRouter groups to pin, cheapest first (`TAZUNA_GROUPS=a,b,c`, the groups that passed `bench/uno_route.py`). Calls go
/// to the current one with an `X-Group` header; a group that drops out or stops caching is left for the next. In one day
/// four groups vanished between check and run or mid-run, two of them taking working runs down.
/// ponytail: one list per process (one model per engine) and no price re-check during a run; a per-call price check if
/// prices turn out to move within a task
struct Groups {
    list: Mutex<Vec<String>>,
    /// `TAZUNA_GROUPS_FILE`: groups added while the run goes (bench/realwork/uno_run.sh checks groups that appear and adds
    /// those that pass), read again at each switch: a run whose listed groups all vanished ended (1017, a7-mega, call 43)
    file: Option<std::path::PathBuf>,
    at: AtomicUsize,
    /// (input, cached) of the last calls on the current group
    recent: Mutex<VecDeque<(u64, u64)>>,
    /// switches not yet in the run log (stderr went unrecorded in benchmark runs)
    notes: Mutex<Vec<String>>,
}

fn group_list(spec: &str) -> Vec<String> {
    spec.split([',', '\n']).map(str::trim).filter(|g| !g.is_empty()).map(String::from).collect()
}

impl Groups {
    fn from(spec: &str, file: Option<std::path::PathBuf>) -> Groups {
        let g = Groups { list: Mutex::new(group_list(spec)), file, at: AtomicUsize::new(0), recent: Mutex::new(VecDeque::new()),
            notes: Mutex::new(vec![]) };
        g.refresh();
        g
    }

    /// Adds the groups in the file not listed yet.
    fn refresh(&self) {
        let Some(text) = self.file.as_ref().and_then(|f| std::fs::read_to_string(f).ok()) else { return };
        let mut list = self.list.lock().unwrap();
        for g in group_list(&text) {
            if !list.contains(&g) {
                list.push(g);
            }
        }
    }

    fn current(&self) -> Option<String> {
        self.list.lock().unwrap().get(self.at.load(Ordering::Relaxed)).cloned()
    }

    /// Moves to the next group; false when none is left.
    fn next(&self, why: &str) -> bool {
        self.refresh();
        let i = self.at.fetch_add(1, Ordering::Relaxed) + 1;
        self.recent.lock().unwrap().clear();
        let list = self.list.lock().unwrap();
        let note = format!("group {} left ({why}); now {}", list.get(i - 1).map_or("?", String::as_str), list.get(i).map_or("no group left", String::as_str));
        eprintln!("{note}");
        self.notes.lock().unwrap().push(note);
        i < list.len()
    }

    /// Switches since the last call, for the run log.
    fn drain(&self) -> Vec<String> {
        std::mem::take(&mut *self.notes.lock().unwrap())
    }

    /// A call's cache use: under 70% over the last 10 calls on a group moves to the next (a cheap group that rotates
    /// accounts cached 47%; the next miss there costs the whole prompt again).
    fn note(&self, input: u64, cached: u64) {
        self.refresh();
        let len = self.list.lock().unwrap().len();
        if len < 2 || self.at.load(Ordering::Relaxed) + 1 >= len {
            return;
        }
        let mut r = self.recent.lock().unwrap();
        r.push_back((input, cached));
        if r.len() > 10 {
            r.pop_front();
        }
        let (i, c) = r.iter().fold((0, 0), |(i, c), x| (i + x.0, c + x.1));
        if r.len() == 10 && (c as f64) < 0.7 * i as f64 {
            drop(r);
            self.next(&format!("{:.0}% cached over the last 10 calls", 100.0 * c as f64 / i.max(1) as f64));
        }
    }
}

/// Another route is waiting (0.0.79): a failing call then gives up after 3 tries instead of ~15 minutes of backing off,
/// and the loop moves to that route.
static FALLBACK: AtomicBool = AtomicBool::new(false);

/// Why the current route should be left, from its own calls as (seconds, input, cached) (0.0.79): the last 10 cached under
/// 70% of their input (the group rule since 0.0.68), or slow: their median over 60 s, or over 40 s and 3x the route's first
/// 10 (on one key a run landed on a pool at 56-148 s a call, another on one at 8-16 s, and stayed there).
fn route_trouble(calls: &[(f64, u64, u64)]) -> Option<String> {
    if calls.len() < 10 {
        return None;
    }
    let last = &calls[calls.len() - 10..];
    let (i, c) = last.iter().fold((0, 0), |(i, c), x| (i + x.1, c + x.2));
    // the cache is judged after a route's first 10 calls (0.0.92): they start cold and grow fast, so LingAI itself
    // cached 45-98% there and 84-100% over the next 10 (round12); every a6aa run left at call 10
    if calls.len() >= 20 && (c as f64) < 0.7 * i as f64 {
        return Some(format!("{:.0}% cached over the last 10 calls", 100.0 * c as f64 / i.max(1) as f64));
    }
    let median = |s: &[(f64, u64, u64)]| {
        let mut v: Vec<f64> = s.iter().map(|x| x.0).collect();
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    };
    let (now, first) = (median(last), median(&calls[..10]));
    (now > 60.0 || (calls.len() >= 20 && now > 40.0 && now > 3.0 * first)).then(|| format!("calls take {now:.0} s (median of the last 10; first 10: {first:.0} s)"))
}

fn groups() -> &'static Groups {
    static G: OnceLock<Groups> = OnceLock::new();
    G.get_or_init(|| Groups::from(&std::env::var("TAZUNA_GROUPS").unwrap_or_default(), std::env::var_os("TAZUNA_GROUPS_FILE").map(Into::into)))
}

/// A backstop against loops: the dollar cap is the real limit, so cheap models get proportionally more turns. 200 cut
/// DeepSeek off after ~75 minutes (180 turns cost it $0.09); 2000 stays above where its $1 cap lands.
const MAX_TURNS: usize = 2000;
/// Dollar cap per task, tazuna's follow-ups included; `TAZUNA_MAX_USD` overrides it (an expensive model on the own loop
/// needs more). $0.50 ended DeepSeek after ~2 hours on hard tasks (lazygit, prometheus: 230-320 calls), before it was done.
const MAX_USD: f64 = 1.0;

/// Experiment (`TAZUNA_BATCH=1`): each turn re-sends the whole conversation, so turns drive the token count. DeepSeek made
/// 1.14 tool calls per response; 70% of its calls were searches and reads, 17% test runs (October tasks, 0.0.38).
const BATCH_RULE: &str = "\n- Work in few turns: every turn re-sends the whole conversation. When several searches, reads or commands do not depend on each other, make all of those tool calls in one response. Read a whole relevant file once rather than many small slices. While working, run only the tests that cover your change; run the wider suite once at the end.";

/// A model call's first time limit; each timeout doubles it for the next try (see `chat`).
const CALL_TIMEOUT: Duration = Duration::from_secs(180);

/// USD per token: input, cached input, output.
type Price = (f64, f64, f64);

/// Room for a Claude model's thinking and answer in one turn (the Messages API requires a limit).
const MAX_OUTPUT: u32 = 64000;

/// The API a model is called through.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Api {
    Chat,
    /// OpenAI models: through Chat Completions with tools they do not reason at all (GPT-6 Luna: 0 reasoning tokens at any
    /// effort; Responses: 54-98).
    Responses,
    /// Claude models (`claude-`): their thinking goes back with the history, as Anthropic requires during tool use, and
    /// effort is set. Through a gateway's Chat Completions the thinking is dropped and effort rejected: Haiku 5.5 on BeatAPI
    /// ended one rclone run in nothing but empty replies.
    Messages,
}

pub fn serve(args: &[String]) -> Result<()> {
    crate::mcp::reap_left(); // background processes an earlier, killed engine left running
    let flag = |name: &str| args.windows(2).find(|w| w[0] == name).map(|w| w[1].clone());
    let url = flag("--url").context("--url")?.trim_end_matches('/').to_string();
    let key_env = flag("--key-env").context("--key-env")?;
    let model = flag("--model").context("--model")?;
    let effort = flag("--effort");
    let mode = flag("--permission-mode").unwrap_or_else(|| "bypassPermissions".into());
    // the prompt (with the repo's CLAUDE.md/AGENTS.md) comes in the environment, not on the command line: an agent's
    // `pkill -f "integration.test"` matched the AGENTS.md text in every engine's arguments and stopped ten runs (0.0.76)
    let mut system = std::env::var("TAZUNA_SYSTEM_PROMPT").ok().or_else(|| flag("--system-prompt")).unwrap_or_default();
    // SAFETY: no other thread runs yet; the agent's commands must not inherit the prompt
    unsafe { std::env::remove_var("TAZUNA_SYSTEM_PROMPT") };
    if std::env::var_os("TAZUNA_BATCH").is_some() {
        system.push_str(BATCH_RULE);
    }
    // other routes for the model, tried in order when the first fails, slows down or stops caching (0.0.79):
    // `TAZUNA_ROUTES="url key-env; url key-env"`; their keys are read now and leave the environment like the first one
    let spec = std::env::var("TAZUNA_ROUTES").unwrap_or_default();
    let mut routes = VecDeque::new();
    for r in spec.split([';', '\n']) {
        let mut w = r.split_whitespace();
        if let (Some(u), Some(k)) = (w.next(), w.next()) {
            match std::env::var(k) {
                Ok(key) => routes.push_back((u.trim_end_matches('/').to_string(), key)),
                Err(_) => eprintln!("route {u}: {k} not set; left out"),
            }
        }
    }
    for k in spec.split([';', '\n']).filter_map(|r| r.split_whitespace().nth(1)).filter(|k| *k != key_env) {
        // SAFETY: no other thread runs yet; the agent's commands must not inherit the keys
        unsafe { std::env::remove_var(k) };
    }
    FALLBACK.store(!routes.is_empty(), Ordering::Relaxed);
    let (call, api, price) = connect(&url, &key_env, &model)?;

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines().map_while(Result::ok) {
            if let Ok(v) = serde_json::from_str::<Value>(&line) {
                if tx.send(v).is_err() {
                    return;
                }
            }
        }
    });
    let cwd = std::env::current_dir()?;
    let session = format!("own-{}-{:x}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos());
    let tools = crate::mcp::tools();
    let names: Vec<&Value> = tools.as_array().into_iter().flatten().map(|t| &t["name"]).collect();
    emit(&json!({"type": "system", "subtype": "init", "session_id": session, "model": model, "cwd": cwd, "tools": names, "slash_commands": []}));
    let mut loop_ = Loop {
        call,
        api,
        system: system.clone(),
        model,
        effort,
        price,
        tools: tools.as_array().into_iter().flatten().map(|t| json!({"type": "function", "function": {"name": t["name"], "description": t["description"], "parameters": t["inputSchema"]}})).collect(),
        // Responses and Messages keep the system prompt outside the history
        msgs: if api == Api::Chat { vec![json!({"role": "system", "content": system})] } else { vec![] },
        server: crate::mcp::Server::new(cwd).with_read_dedupe(),
        session,
        cost: 0.0,
        task: (std::time::Instant::now(), 0.0),
        queued: VecDeque::new(),
        mode,
        asked: 0,
        send_reasoning: reasoning_back(),
        cache_key: std::env::var_os("TAZUNA_NO_CACHE_KEY").is_none(),
        routes,
        route_calls: vec![],
        connector: connect_key,
        out: Box::new(|e: &Value| emit(e)),
    };
    loop {
        let msg = match loop_.queued.pop_front() {
            Some(m) => m,
            None => match rx.recv() {
                Ok(m) => m,
                Err(_) => return Ok(()), // stdin closed: tazuna is done with us
            },
        };
        if msg["type"] == "user" {
            loop_.run(&msg["message"]["content"], msg["tazuna_followup"] == true, &rx);
        }
        loop_.set_mode(&msg); // a mode switch while idle; an interrupt while idle has nothing to stop
    }
}

struct Loop {
    call: Box<dyn FnMut(&Value) -> Result<Value>>,
    api: Api,
    system: String,
    model: String,
    /// `reasoning_effort` (low ... max); without it the provider's default, which can be far lower.
    effort: Option<String>,
    price: Price,
    tools: Vec<Value>,
    msgs: Vec<Value>,
    server: crate::mcp::Server,
    session: String,
    /// Spent by this process so far; `result` reports it like claude does.
    cost: f64,
    /// When the current task began and what it had cost by then: tazuna's follow-ups (test failures, review) share the
    /// task's budget. Before 0.0.38 each follow-up got a fresh hour, and one run passed the benchmark's 90-minute limit.
    task: (std::time::Instant, f64),
    /// User messages that arrived during a run.
    queued: VecDeque<Value>,
    /// Permission mode in claude's names: bypassPermissions (auto: never asks), default (ask before commands and edits),
    /// acceptEdits (ask before commands). Plan mode is not offered for the own loop.
    mode: String,
    /// Chat Completions: send each assistant turn's `reasoning_content` back. Off by default (some providers reject the
    /// field); switched on for the rest of the run when a provider refuses a call without it.
    send_reasoning: bool,
    /// Responses API: send `prompt_cache_key` (the session id). On unless `TAZUNA_NO_CACHE_KEY` is set; switched off for
    /// the rest of the run when a provider refuses requests that carry it.
    cache_key: bool,
    /// Other routes for the model as (url, key), next first (0.0.79, `TAZUNA_ROUTES`)
    routes: VecDeque<(String, String)>,
    /// (seconds, input, cached) of each call on the current route, for `route_trouble`
    route_calls: Vec<(f64, u64, u64)>,
    /// How a route is reached: `connect_key`; a stand-in in tests
    connector: fn(&str, &str, &str) -> Result<(Box<dyn FnMut(&Value) -> Result<Value>>, Api, Price)>,
    /// Permission questions asked so far (their request ids).
    asked: u32,
    out: Box<dyn FnMut(&Value)>,
}

/// A provider's refusal of requests that carry `prompt_cache_key`: SubRouter's "no available channel" (503
/// model_not_found) for them while the same request without the key goes through, or an error naming the field.
fn refused_cache_key(e: &str) -> bool {
    (e.contains("503") || e.contains("400") || e.contains("404")) && (e.contains("model_not_found") || e.contains("无可用渠道") || e.contains("prompt_cache_key"))
}

/// Removes every thinking block from the conversation; true when there was one.
fn drop_thinking(msgs: &mut [Value]) -> bool {
    let mut dropped = false;
    for m in msgs.iter_mut().filter(|m| m["role"] == "assistant") {
        if let Some(blocks) = m["content"].as_array_mut() {
            let n = blocks.len();
            blocks.retain(|b| b["type"] != "thinking" && b["type"] != "redacted_thinking");
            dropped |= blocks.len() < n;
        }
    }
    dropped
}

/// A conversation's pool is gone (SubRouter's shared ChatGPT pools, see the loop).
fn pinned_away(e: &str) -> bool {
    e.contains("responses_source_unavailable")
}

fn empty_reply(reply: &Value, api: Api) -> bool {
    match api {
        Api::Responses => !reply["output"].as_array().is_some_and(|items| items.iter().any(|i| i["type"] == "function_call" || i["type"] == "message")),
        // thinking alone is empty: nothing to show and nothing to run
        Api::Messages => !reply["content"].as_array().is_some_and(|bs| bs.iter().any(|b| b["type"] == "tool_use" || b["text"].as_str().is_some_and(|t| !t.trim().is_empty()))),
        Api::Chat => {
            let m = &reply["choices"][0]["message"];
            m["content"].as_str().is_none_or(|t| t.trim().is_empty()) && m["tool_calls"].as_array().is_none_or(|c| c.is_empty())
        }
    }
}

const NUDGE: &str = "You stopped without changing any file. If the task needs no change, reply DONE and say why. \
Otherwise continue now and make the change with the tools.";

/// Clean DeepSeek runs on rclone spent ~53 turns each after their tests first passed, against ~25 in OpenCode: re-reading
/// their own code, diffs, builds and repros, and 7-15 test runs with nothing changed since the last pass (2026-10-24).
const GREEN: &str = "The tests pass and your change includes tests. Check the diff against the request once, \
then finish with a short summary. Run tests, builds or linters again only if you change code.";
const RERUN: &str = "Nothing changed since the last passing run, so this run told you nothing new.";

fn is_test(cmd: &str) -> bool {
    ["go test", "cargo test", "cargo nextest", "pytest", "npm test", "npm run test", "yarn test", "pnpm test", "jest",
     "vitest", "mvn test", "gradle test", "make test", "ctest", "rspec", "phpunit", "dotnet test", "mix test"]
        .iter().any(|t| cmd.contains(t))
}

/// A test run that passed: no failing exit, no failure in the output (a `| tail` hides the exit code), and some sign of
/// passing tests (`[no test files]` alone is not one).
fn passed(out: &str) -> bool {
    let failed = out.match_indices(" failed").any(|(i, _)| {
        out[..i].rsplit(|c: char| !c.is_ascii_digit()).next().is_none_or(|n| n.is_empty() || n.chars().any(|c| c != '0'))
    });
    !out.contains("[exit ") && !out.contains("[timed out") && !out.contains("FAIL") && !out.contains("panic:") && !failed
        && ["ok ", "PASS", " passed", "test result: ok"].iter().any(|p| out.contains(p))
}

/// The working tree's state (a hash of the diff and of the untracked files' sizes and times) and whether the changes hold
/// both a test file and other code; None outside a git repo. Tests alone are not enough: a repro test written before the
/// fix passed at turn 35 on caddy-unix, and the note came 100+ turns early (2026-10-25).
fn tree_state() -> Option<(u64, bool)> {
    tree_state_in(&std::env::current_dir().ok()?)
}

fn tree_state_in(dir: &std::path::Path) -> Option<(u64, bool)> {
    tree_kinds(dir).map(|(h, tests, code)| (h, tests && code))
}

/// The tree's hash, whether the changes hold a test file, and whether they hold other code.
fn tree_kinds(dir: &std::path::Path) -> Option<(u64, bool, bool)> {
    use std::hash::{Hash, Hasher};
    let git = |args: &[&str]| std::process::Command::new("git").arg("-C").arg(dir).args(args).output().ok().filter(|o| o.status.success()).map(|o| o.stdout);
    let (diff, status) = (git(&["diff", "HEAD"])?, git(&["status", "--porcelain", "-uall"])?);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    diff.hash(&mut h);
    let status = String::from_utf8_lossy(&status).into_owned();
    let (mut tests, mut code) = (false, false);
    for path in status.lines().filter_map(|l| l.get(3..)) {
        let name = path.rsplit('/').next().unwrap_or(path);
        let test = name.contains("_test.") || name.starts_with("test_") || name.contains(".test.") || name.contains(".spec.")
            || path.starts_with("tests/") || path.contains("/tests/") || path.starts_with("test/") || path.contains("/test/");
        (tests, code) = (tests || test, code || !test);
        if let Ok(m) = std::fs::metadata(dir.join(path)) {
            (path, m.len(), m.modified().ok()).hash(&mut h);
        }
    }
    Some((h.finish(), tests, code))
}

/// The note a test run's result gets, if any: GREEN at the first pass with tests and code in the change, RERUN for a pass with the
/// tree unchanged since the last one. `green` is the tree at the last pass; `told`, whether GREEN was given.
fn test_note(cmd: &str, out: &str, tree: impl FnOnce() -> Option<(u64, bool)>, green: &mut Option<u64>, told: &mut bool) -> Option<&'static str> {
    if !is_test(cmd) || !passed(out) {
        return None;
    }
    let (now, tests) = tree()?;
    // a command that writes a test file, runs it and deletes it leaves the tree as it was: not a rerun (v341, 2026-10-25)
    let note = if *green == Some(now) && !(cmd.contains("_test.") && cmd.contains('>')) {
        Some(RERUN)
    } else if !*told && tests {
        *told = true;
        Some(GREEN)
    } else {
        None
    };
    *green = Some(now);
    note
}

/// The model's earlier reasoning goes back with the history only when TAZUNA_SEND_REASONING is set, or when a provider
/// demands it (the fallback in the loop). 0.0.85 sent it by default for DeepSeek's re-planning, but no SubRouter provider
/// passes it to the model and none bills it (2026-10-27/28); 0.0.97's note of a long step's end is what fixed the
/// re-planning. MiniMax-M3.1-Flash on foxrouter keeps and bills it: 3.6M characters of reasoning took one run's prompt to
/// 910K tokens and past the context limit (0.1.0, 2026-10-29).
fn reasoning_back() -> bool {
    std::env::var_os("TAZUNA_SEND_REASONING").is_some()
}

/// The end of a long reasoning step, kept in the history as visible text (0.0.97). Providers drop a model's earlier
/// reasoning (DeepSeek through SubRouter: a code word planted in it was never recalled), so a design worked out once was
/// worked out again at every new finding: six times, 13-26K characters each, on one caddy run; telling the model to write
/// its plan down (0.0.96) changed nothing. A step's conclusion is at its end. Only long steps; appended once, never
/// changed, so the cached history stays the same.
fn kept_reasoning(r: &str) -> Option<String> {
    const LONG: usize = 4000;
    const TAIL: usize = 2500;
    if r.chars().count() < LONG {
        return None;
    }
    let start = r.char_indices().rev().nth(TAIL - 1).map_or(0, |(i, _)| i);
    Some(format!("[the end of my reasoning at this step, kept as a note]\n...{}\n[end of note]\n\n", r[start..].trim()))
}

fn headless() -> bool {
    std::env::var_os("TAZUNA_HEADLESS").is_some()
}

/// Uncommitted changes in the working tree (outside a git repo: assume yes, so nothing is nudged).
fn repo_changed() -> bool {
    std::process::Command::new("git").args(["status", "--porcelain"]).output()
        .map_or(true, |o| !o.status.success() || !o.stdout.is_empty())
}

impl Loop {
    /// Moves to the next route that answers with the same API (0.0.79); false when none is left. What is bound to the old
    /// upstream leaves the history: Claude thinking (its signature holds on the account that wrote it) and Responses
    /// reasoning items (encrypted per organisation); the per-route flags start over. The new route's cache starts cold.
    fn switch_route(&mut self, why: &str) -> bool {
        while let Some((url, key)) = self.routes.pop_front() {
            FALLBACK.store(!self.routes.is_empty(), Ordering::Relaxed);
            match (self.connector)(&url, &key, &self.model) {
                Ok((call, api, price)) if api == self.api => {
                    self.call = call;
                    self.price = price;
                    drop_thinking(&mut self.msgs);
                    self.msgs.retain(|m| m["type"] != "reasoning");
                    self.cache_key = std::env::var_os("TAZUNA_NO_CACHE_KEY").is_none();
                    self.send_reasoning = reasoning_back();
                    self.route_calls.clear();
                    (self.out)(&json!({"type": "system", "subtype": "route_switch", "message": format!("now {url}: {why}")}));
                    return true;
                }
                Ok(_) => (self.out)(&json!({"type": "system", "subtype": "route_switch", "message": format!("{url} skipped: another API for this model")})),
                Err(e) => (self.out)(&json!({"type": "system", "subtype": "route_switch", "message": format!("{url} skipped: {e:#}")})),
            }
        }
        false
    }

    /// The history for Chat Completions: assistant turns without their `reasoning_content` (0.1.0), or with it when a
    /// provider demands it or with TAZUNA_SEND_REASONING.
    fn chat_messages(&self) -> Value {
        if self.send_reasoning {
            return json!(self.msgs);
        }
        json!(self.msgs.iter().map(|m| match m.get("reasoning_content") {
            Some(_) => {
                let mut m = m.clone();
                m.as_object_mut().expect("a message is an object").remove("reasoning_content");
                m
            }
            None => m.clone(),
        }).collect::<Vec<_>>())
    }

    /// One user message: model calls and tool runs until the model answers without tools, or a cap or interrupt ends it.
    fn run(&mut self, content: &Value, followup: bool, rx: &Receiver<Value>) {
        self.msgs.push(json!({"role": "user", "content": text_of(content)}));
        if !followup {
            self.task = (std::time::Instant::now(), self.cost);
        }
        let ((began, start), mut turns, mut usage) = (self.task, 0, [0u64; 3]);
        let mut nudges = 0;
        let (mut green, mut told) = (None, false);
        let end = |s: &mut Self, subtype: &str, text: String, turns: usize, usage: [u64; 3]| {
            (s.out)(&json!({"type": "result", "subtype": subtype, "is_error": subtype != "success", "result": text,
                "num_turns": turns, "total_cost_usd": s.cost, "session_id": s.session,
                "usage": {"input_tokens": usage[0], "cache_creation_input_tokens": 0, "cache_read_input_tokens": usage[1], "output_tokens": usage[2]}}));
        };
        loop {
            loop {
                match rx.try_recv() {
                    Ok(m) if m["type"] == "control_request" && m["request"]["subtype"] == "interrupt" => {
                        return end(self, "error_during_execution", "[interrupted]".into(), turns, usage);
                    }
                    Ok(m) if m["type"] == "user" => self.queued.push_back(m),
                    Ok(m) => self.set_mode(&m),
                    Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
                }
            }
            if turns >= MAX_TURNS {
                return end(self, "error_max_turns", format!("[stopped: {MAX_TURNS} turns]"), turns, usage);
            }
            // `--max-minutes`, opt-in: no time limit by default, since a cheap model's extra hour costs cents
            if let Some(m) = std::env::var("TAZUNA_MAX_MINUTES").ok().and_then(|v| v.parse::<u64>().ok()) {
                if began.elapsed() >= Duration::from_secs(m * 60) {
                    return end(self, "error_max_time", format!("[stopped: {m} min]"), turns, usage);
                }
            }
            let cap = std::env::var("TAZUNA_MAX_USD").ok().and_then(|v| v.parse::<f64>().ok()).unwrap_or(MAX_USD);
            if self.cost - start >= cap {
                return end(self, "error_max_budget_usd", format!("[stopped: ${cap:.2} spent]"), turns, usage);
            }
            // a route that has slowed down or stopped caching is left before the next call (0.0.79)
            if let Some(why) = (!self.routes.is_empty()).then(|| route_trouble(&self.route_calls)).flatten() {
                self.switch_route(&why);
            }
            (self.out)(&json!({"type": "stream_event", "event": {"type": "message_start"}}));
            let mut body = match self.api {
                Api::Responses => {
                    // stateless (store: false): earlier reasoning comes back encrypted and is sent again with the history.
                    // The cache key keeps a conversation on one cache: without it BeatAPI cached 18-47% of GPT input
                    // (99.97% with it, from the second call on)
                    let tools: Vec<Value> = self.tools.iter().map(|t| json!({"type": "function", "name": t["function"]["name"], "description": t["function"]["description"], "parameters": t["function"]["parameters"]})).collect();
                    let mut b = json!({"model": self.model, "instructions": self.system, "input": self.msgs, "tools": tools, "store": false,
                        "include": ["reasoning.encrypted_content"]});
                    if self.cache_key {
                        b["prompt_cache_key"] = json!(self.session);
                    }
                    if let Some(e) = &self.effort {
                        b["reasoning"] = json!({"effort": e});
                    }
                    b
                }
                Api::Messages => {
                    let tools: Vec<Value> = self.tools.iter().map(|t| json!({"name": t["function"]["name"], "description": t["function"]["description"], "input_schema": t["function"]["parameters"]})).collect();
                    // adaptive is the only thinking mode from Claude 4.7 on; the top-level cache breakpoint moves to the
                    // end of the conversation on each call
                    let mut b = json!({"model": self.model, "max_tokens": MAX_OUTPUT, "messages": self.msgs, "tools": tools,
                        "thinking": {"type": "adaptive"}, "cache_control": {"type": "ephemeral"}});
                    if !self.system.is_empty() {
                        b["system"] = json!(self.system);
                    }
                    if let Some(e) = &self.effort {
                        b["output_config"] = json!({"effort": e});
                    }
                    b
                }
                Api::Chat => {
                    let mut b = json!({"model": self.model, "messages": self.chat_messages(), "tools": self.tools});
                    if let Some(e) = &self.effort {
                        b["reasoning_effort"] = json!(e);
                    }
                    b
                }
            };
            let t0 = std::time::Instant::now();
            let mut reply = match (self.call)(&body) {
                Ok(r) => r,
                // a thinking block's signature holds only on the upstream account that wrote it: a gateway that moves the
                // conversation to another account mid-run (Mortera Cloud's failover) answers 400 "invalid" (Haiku 5.5
                // after 39 calls); the same request without them goes through
                Err(e) if self.api == Api::Messages && format!("{e:#}").contains("400") && drop_thinking(&mut self.msgs) => {
                    (self.out)(&json!({"type": "system", "subtype": "thinking_dropped", "message": format!("{e:#}")}));
                    body["messages"] = json!(self.msgs);
                    match (self.call)(&body) {
                        Ok(r) => r,
                        Err(e) => return end(self, "error_during_execution", format!("[provider error: {e:#}]"), turns, usage),
                    }
                }
                // SubRouter's shared ChatGPT pools: a conversation stays on the pool whose account wrote its encrypted reasoning,
                // and while that pool is down every call fails 503 "responses_source_unavailable" (a tazuna run ended on it,
                // 2026-10-28). Without that reasoning and under a new cache key it goes on in another pool of the key, the
                // first call there uncached
                Err(e) if self.api == Api::Responses && pinned_away(&format!("{e:#}")) => {
                    self.msgs.retain(|m| m["type"] != "reasoning");
                    self.session.push_str("-moved");
                    (self.out)(&json!({"type": "system", "subtype": "conversation_moved", "message": format!("{e:#}")}));
                    body["input"] = json!(self.msgs);
                    if self.cache_key {
                        body["prompt_cache_key"] = json!(self.session);
                    }
                    match (self.call)(&body) {
                        Ok(r) => r,
                        Err(e) => return end(self, "error_during_execution", format!("[provider error: {e:#}]"), turns, usage),
                    }
                }
                // DeepSeek's thinking with tools: a provider behind a gateway that moved the run there answered 400 "This
                // reasoning model requires previous reasoning_content in multi-turn requests" (call 29 of a caddy run)
                // SubRouter answered every request carrying `prompt_cache_key` 503 "该分组下模型无可用渠道" (no channel for the
                // model in this group) while the same request without it went through (2026-10-23)
                Err(e) if self.api == Api::Responses && self.cache_key && refused_cache_key(&format!("{e:#}")) => {
                    (self.out)(&json!({"type": "system", "subtype": "cache_key_dropped", "message": format!("{e:#}")}));
                    self.cache_key = false;
                    if let Some(o) = body.as_object_mut() {
                        o.remove("prompt_cache_key");
                    }
                    match (self.call)(&body) {
                        Ok(r) => r,
                        Err(e) => return end(self, "error_during_execution", format!("[provider error: {e:#}]"), turns, usage),
                    }
                }
                // either way round: a provider that demands the reasoning back gets it, one that refuses the field gets
                // the history without it (once each per route)
                Err(e) if self.api == Api::Chat && format!("{e:#}").contains("reasoning_content") => {
                    let subtype = if self.send_reasoning { "reasoning_dropped" } else { "reasoning_resent" };
                    (self.out)(&json!({"type": "system", "subtype": subtype, "message": format!("{e:#}")}));
                    self.send_reasoning = !self.send_reasoning;
                    body["messages"] = self.chat_messages();
                    match (self.call)(&body) {
                        Ok(r) => r,
                        Err(e) => return end(self, "error_during_execution", format!("[provider error: {e:#}]"), turns, usage),
                    }
                }
                // the route failed for good (a refusal, or 3 failed tries while another route waits): the same turn on the next
                Err(e) if self.switch_route(&format!("{e:#}").chars().take(120).collect::<String>()) => continue,
                Err(e) => return end(self, "error_during_execution", format!("[provider error: {e:#}]"), turns, usage),
            };
            // an empty reply (no text, no call) is a provider hiccup, not an answer: the same request again, up to 3
            // times (a saturated gateway sent three in a row, and the task ended on them)
            for wait in [5, 15, 30] {
                if !empty_reply(&reply, self.api) {
                    break;
                }
                std::thread::sleep(Duration::from_secs(if cfg!(test) { 0 } else { wait }));
                reply = match (self.call)(&body) {
                    Ok(r) => r,
                    Err(e) => return end(self, "error_during_execution", format!("[provider error: {e:#}]"), turns, usage),
                };
            }
            if empty_reply(&reply, self.api) && self.switch_route("empty replies") {
                continue;
            }
            let secs = t0.elapsed().as_secs_f64();
            turns += 1;
            let u = &reply["usage"];
            let (prompt, out, cached, written) = if self.api == Api::Messages {
                // Anthropic's input_tokens leaves out cache reads and writes
                let n = |k: &str| u[k].as_u64().unwrap_or(0);
                let (read, written) = (n("cache_read_input_tokens"), n("cache_creation_input_tokens"));
                (n("input_tokens") + read + written, n("output_tokens"), read, written)
            } else {
                // either API's field names: some gateways answer the Responses API with Chat Completions usage (UnoRouter)
                let n = |chat: &Value, resp: &Value| {
                    let (a, b) = if self.api == Api::Responses { (resp, chat) } else { (chat, resp) };
                    a.as_u64().or(b.as_u64()).unwrap_or(0)
                };
                let prompt = n(&u["prompt_tokens"], &u["input_tokens"]);
                (prompt, n(&u["completion_tokens"], &u["output_tokens"]), n(&u["prompt_tokens_details"]["cached_tokens"], &u["input_tokens_details"]["cached_tokens"]).min(prompt), 0)
            };
            groups().note(prompt, cached);
            if !self.routes.is_empty() {
                self.route_calls.push((secs, prompt, cached));
            }
            for note in groups().drain() {
                (self.out)(&json!({"type": "system", "subtype": "group_switch", "message": note}));
            }
            // the billed cost when the provider reports it (CheaperInference, OpenRouter), else the price list; a cache write
            // costs 1.25x input (Anthropic, 5 minutes)
            self.cost += u["cost"].as_f64().unwrap_or((prompt - cached) as f64 * self.price.0 + written as f64 * self.price.0 * 0.25
                + cached as f64 * self.price.1 + out as f64 * self.price.2);
            for (sum, n) in usage.iter_mut().zip([prompt - cached, cached, out]) {
                *sum += n;
            }
            (self.out)(&json!({"type": "stream_event", "event": {"type": "message_delta",
                "usage": {"input_tokens": prompt - cached, "cache_creation_input_tokens": 0, "cache_read_input_tokens": cached, "output_tokens": out}}}));

            // the step's reasoning, for the transcript only (0.0.95): which steps the model deliberates on, and why, could
            // not be read before (41 of tazuna's 176 calls wrote over 5K tokens against Claude Code's 8 of 190, 2026-10-27)
            let mut thought = String::new();
            // text and calls as (id, name, arguments), in Chat Completions' shape for both APIs
            let (text, calls): (String, Vec<Value>) = match self.api {
                Api::Responses => {
                    let items = reply["output"].as_array().cloned().unwrap_or_default();
                    let text = items.iter().filter(|i| i["type"] == "message").flat_map(|i| i["content"].as_array().cloned().unwrap_or_default())
                        .filter_map(|c| c["text"].as_str().map(String::from)).collect::<Vec<_>>().join("\n");
                    let calls = items.iter().filter(|i| i["type"] == "function_call")
                        .map(|i| json!({"id": i["call_id"], "function": {"name": i["name"], "arguments": i["arguments"]}})).collect();
                    self.msgs.extend(items); // reasoning, messages and calls go back as they came
                    (text, calls)
                }
                Api::Messages => {
                    // thinking (with its signature), text and calls go back as they came; empty text blocks are rejected
                    let blocks: Vec<Value> = reply["content"].as_array().cloned().unwrap_or_default().into_iter()
                        .filter(|b| b["type"] != "text" || b["text"].as_str().is_some_and(|t| !t.is_empty())).collect();
                    let text = blocks.iter().filter(|b| b["type"] == "text").filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("\n");
                    let calls = blocks.iter().filter(|b| b["type"] == "tool_use")
                        .map(|b| json!({"id": b["id"], "function": {"name": b["name"], "arguments": b["input"]}})).collect();
                    thought = blocks.iter().filter_map(|b| b["thinking"].as_str()).collect::<Vec<_>>().join("\n");
                    // a turn of thinking alone is not kept: the nudge that follows is the next user turn
                    if blocks.iter().any(|b| b["type"] == "text" || b["type"] == "tool_use") {
                        self.msgs.push(json!({"role": "assistant", "content": blocks}));
                    }
                    (text, calls)
                }
                Api::Chat => {
                    let m = &reply["choices"][0]["message"];
                    let text = m["content"].as_str().unwrap_or_default().to_string();
                    let mut calls: Vec<Value> = m["tool_calls"].as_array().cloned().unwrap_or_default();
                    // LingAI sent 16 calls with an empty id in one DeepSeek run; the history was accepted until call 161,
                    // then refused: "Assistant tool_calls require distinct non-empty ids" (2026-10-24)
                    for (i, c) in calls.iter_mut().enumerate() {
                        let id = c["id"].as_str().unwrap_or_default();
                        if id.is_empty() || self.msgs.iter().any(|m| m["tool_calls"].as_array().is_some_and(|t| t.iter().any(|t| t["id"] == id))) {
                            c["id"] = json!(format!("call_{turns}_{i}"));
                        }
                    }
                    // only what the next call needs; the reasoning is kept but sent back only when a provider asks for it
                    let mut keep = json!({"role": "assistant", "content": text});
                    if !calls.is_empty() {
                        keep["tool_calls"] = json!(calls);
                    }
                    if let Some(r) = m["reasoning_content"].as_str().or(m["reasoning"].as_str()).filter(|r| !r.is_empty()) {
                        keep["reasoning_content"] = json!(r);
                        thought = r.to_string();
                        if let Some(note) = kept_reasoning(r) {
                            keep["content"] = json!(format!("{note}{text}"));
                        }
                    }
                    self.msgs.push(keep);
                    (text, calls)
                }
            };
            let mut blocks = vec![];
            if !text.is_empty() {
                blocks.push(json!({"type": "text", "text": text}));
            }
            let parsed: Vec<(String, String, Option<Value>)> = calls
                .iter()
                .map(|c| {
                    let a = &c["function"]["arguments"];
                    // arguments are a JSON string by the spec; some providers send the object itself
                    let input = a.as_str().map_or(Some(a.clone()), |s| serde_json::from_str(if s.trim().is_empty() { "{}" } else { s }).ok());
                    (c["id"].as_str().unwrap_or_default().to_string(), c["function"]["name"].as_str().unwrap_or_default().to_string(), input)
                })
                .collect();
            for (id, name, input) in &parsed {
                blocks.push(json!({"type": "tool_use", "id": id, "name": name, "input": input.clone().unwrap_or(json!({}))}));
            }
            if !thought.is_empty() {
                (self.out)(&json!({"type": "reasoning", "text": thought}));
            }
            (self.out)(&json!({"type": "assistant", "message": {"role": "assistant", "model": self.model, "content": blocks}}));
            if parsed.is_empty() {
                // a headless task that stops before changing anything gets up to two nudges: some models and routes
                // stop after a plan or with an empty reply (two rclone runs on two gateways ended with no change at all)
                if nudges < 2 && !followup && (text.trim().is_empty() || (headless() && !repo_changed())) {
                    nudges += 1;
                    self.msgs.push(json!({"role": "user", "content": NUDGE}));
                    (self.out)(&json!({"type": "user", "message": {"role": "user", "content": [{"type": "text", "text": NUDGE}]}}));
                    continue;
                }
                return end(self, "success", text, turns, usage);
            }
            let (mut results, mut note) = (vec![], None);
            for (id, name, input) in parsed {
                let cmd = (name == "bash").then(|| input.as_ref().and_then(|i| i["command"].as_str()).map(String::from)).flatten();
                let (text, is_error) = match input {
                    None => ("the arguments were not valid JSON; call the tool again with a JSON object".to_string(), true),
                    Some(input) => match self.permit(&name, &input, rx) {
                        Permit::Allow(input) if self.mode == "explore" && (name == "search" || name == "list") => {
                            explore_tool(&name, &input, &std::env::current_dir().unwrap_or_default())
                        }
                        Permit::Allow(input) => match self.server.call(&json!({"name": name, "arguments": input})) {
                            Ok(r) => (r["content"][0]["text"].as_str().unwrap_or_default().to_string(), r["isError"] == true),
                            Err((_, e)) => (e, true),
                        },
                        Permit::Deny(why) => (why, true),
                        Permit::Stop => return end(self, "error_during_execution", "[interrupted]".into(), turns, usage),
                    },
                };
                note = cmd.and_then(|c| test_note(&c, &text, tree_state, &mut green, &mut told)).or(note);
                match self.api {
                    Api::Responses => self.msgs.push(json!({"type": "function_call_output", "call_id": id, "output": text})),
                    Api::Chat => self.msgs.push(json!({"role": "tool", "tool_call_id": id, "content": text})),
                    Api::Messages => {} // all results in one user turn, below
                }
                results.push(json!({"type": "tool_result", "tool_use_id": id, "content": text, "is_error": is_error}));
            }
            // the note is a user turn of its own: appended to the tool output (0.0.80), DeepSeek ran 79-95 more turns after it
            let note = note.map(|n| json!({"type": "text", "text": n.trim()}));
            match (self.api, &note) {
                (Api::Messages, Some(n)) => results.push(n.clone()),
                (_, Some(n)) => self.msgs.push(json!({"role": "user", "content": n["text"]})),
                _ => {}
            }
            if self.api == Api::Messages {
                self.msgs.push(json!({"role": "user", "content": results}));
            }
            (self.out)(&json!({"type": "user", "message": {"role": "user", "content": results}}));
            if let (false, Some(n)) = (self.api == Api::Messages, note) {
                (self.out)(&json!({"type": "user", "message": {"role": "user", "content": [n]}}));
            }
        }
    }
}

/// A model call through the provider, the API it takes, and its prices. Removes the key from this process's environment:
/// the agent's commands inherit it and must not see the key.
fn connect(url: &str, key_env: &str, model: &str) -> Result<(Box<dyn FnMut(&Value) -> Result<Value>>, Api, Price)> {
    let key = std::env::var(key_env).with_context(|| format!("{key_env} not set"))?;
    // SAFETY: called before any other thread starts.
    unsafe { std::env::remove_var(key_env) };
    connect_key(url, &key, model)
}

/// One call with no tools and no history: the independent reviewer on a provider's API (review.rs). The API follows the
/// model's name as for the own loop; returns the reply's text and (input, cached, output) tokens.
pub fn ask(url: &str, key: &str, model: &str, effort: &str, system: &str, prompt: &str) -> Result<(String, (u64, u64, u64))> {
    let api = if model.starts_with("claude-") { Api::Messages } else if model.starts_with("gpt-") { Api::Responses } else { Api::Chat };
    let http: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(CALL_TIMEOUT)).http_status_as_error(false).build().into();
    let body = match api {
        Api::Responses => json!({"model": model, "instructions": system, "input": [{"role": "user", "content": prompt}], "store": false,
            "reasoning": {"effort": effort}}),
        Api::Messages => json!({"model": model, "max_tokens": MAX_OUTPUT, "system": system, "messages": [{"role": "user", "content": prompt}],
            "thinking": {"type": "adaptive"}, "output_config": {"effort": effort}}),
        Api::Chat => json!({"model": model, "messages": [{"role": "system", "content": system}, {"role": "user", "content": prompt}],
            "reasoning_effort": effort}),
    };
    let r = chat(&http, url.trim_end_matches('/'), key, api, &body)?;
    let blocks = |v: &Value, kind: &str| v.as_array().into_iter().flatten().filter(|b| b["type"] == kind)
        .filter_map(|b| b["text"].as_str().map(String::from)).collect::<Vec<_>>().join("\n");
    let text = match api {
        Api::Responses => r["output"].as_array().into_iter().flatten().filter(|i| i["type"] == "message")
            .map(|i| blocks(&i["content"], "output_text")).collect::<Vec<_>>().join("\n"),
        Api::Messages => blocks(&r["content"], "text"),
        Api::Chat => r["choices"][0]["message"]["content"].as_str().unwrap_or_default().to_string(),
    };
    let u = &r["usage"];
    let n = |a: &Value, b: &Value| a.as_u64().or(b.as_u64()).unwrap_or(0);
    let cached = n(&u["input_tokens_details"]["cached_tokens"], &u["prompt_tokens_details"]["cached_tokens"]) + u["cache_read_input_tokens"].as_u64().unwrap_or(0);
    let input = n(&u["input_tokens"], &u["prompt_tokens"]) + if api == Api::Messages { cached } else { 0 };
    Ok((text, (input, cached.min(input), n(&u["output_tokens"], &u["completion_tokens"]))))
}

/// `connect` with the key itself (a later route's, read at start).
fn connect_key(url: &str, key: &str, model: &str) -> Result<(Box<dyn FnMut(&Value) -> Result<Value>>, Api, Price)> {
    let key = key.to_string();
    let http: ureq::Agent = ureq::Agent::config_builder()
        // a normal call answers in 15-60 s, a long reasoning one in a minute or two; 300 s let a provider that accepted a
        // call and never answered stall a run for an hour across the retries (GLM on Experiential Labs, 2026-10-12)
        .timeout_global(Some(CALL_TIMEOUT)).http_status_as_error(false).build().into();
    // retried like model calls: one benchmark run died at startup on a model-list request that never answered
    let (price, api) = [2, 8, 30].iter().find_map(|w| price(&http, url, &key, model).map_err(|e| {
        eprintln!("model list failed ({e:#}); retrying in {w}s");
        std::thread::sleep(Duration::from_secs(*w));
    }).ok()).map_or_else(|| price(&http, url, &key, model), Ok)?;
    let url = url.to_string();
    Ok((Box::new(move |body: &Value| chat(&http, &url, &key, api, body)), api, price))
}

/// The explorer's own tools instead of bash: no shell, fixed arguments, paths inside the repo only. (A word list over bash
/// commands was bypassable: `rg --pre`, `git -c core.pager`, `sed 'w file'`, `sort -o` run or write, found by a security
/// review.)
fn explore_tools() -> Vec<Value> {
    vec![
        json!({"type": "function", "function": {"name": "search",
            "description": "Search file contents with an extended regular expression (grep -rnIE). Returns path:line:text, at most 200 lines.",
            "parameters": {"type": "object", "properties": {
                "pattern": {"type": "string"}, "path": {"type": "string", "description": "directory or file, default ."},
                "glob": {"type": "string", "description": "only files matching this name pattern, e.g. *.go"}}, "required": ["pattern"]}}}),
        json!({"type": "function", "function": {"name": "list",
            "description": "List files and directories under a path (find), at most 500 entries.",
            "parameters": {"type": "object", "properties": {
                "path": {"type": "string", "description": "default ."}, "glob": {"type": "string", "description": "only names matching this pattern"},
                "depth": {"type": "integer", "description": "1-6, default 3"}}}}}),
    ]
}

/// A path the explorer may look at: inside the working directory (no `..` out of it, no absolute path elsewhere).
fn inside(cwd: &std::path::Path, path: &str) -> Option<std::path::PathBuf> {
    let root = cwd.canonicalize().ok()?;
    let p = root.join(if path.is_empty() { "." } else { path }).canonicalize().ok()?;
    p.starts_with(&root).then_some(p)
}

/// Runs `search` or `list` with fixed arguments (no shell): (text, is_error).
fn explore_tool(name: &str, input: &Value, cwd: &std::path::Path) -> (String, bool) {
    let Some(path) = inside(cwd, input["path"].as_str().unwrap_or(".")) else {
        return ("That path is outside the repository or does not exist.".into(), true);
    };
    let glob = input["glob"].as_str().filter(|g| !g.is_empty());
    let (mut cmd, max) = if name == "search" {
        let mut c = std::process::Command::new("grep");
        c.args(["-rnIE", "--exclude-dir=.git", "--exclude-dir=node_modules", "--exclude-dir=vendor"]);
        if let Some(g) = glob {
            c.arg(format!("--include={g}"));
        }
        c.arg("--").arg(input["pattern"].as_str().unwrap_or_default()).arg(&path);
        (c, 200)
    } else {
        let depth = input["depth"].as_u64().unwrap_or(3).clamp(1, 6).to_string();
        let mut c = std::process::Command::new("find");
        c.arg(&path).args(["-maxdepth", &depth, "-not", "-path", "*/.git/*", "-not", "-name", ".git"]);
        if let Some(g) = glob {
            c.args(["-name", g]);
        }
        (c, 500)
    };
    let out = match cmd.current_dir(cwd).output() {
        Ok(o) => String::from_utf8_lossy(&o.stdout).into_owned(),
        Err(e) => return (format!("{name} failed: {e}"), true),
    };
    let root = cwd.canonicalize().map(|r| format!("{}/", r.display())).unwrap_or_default();
    let lines: Vec<&str> = out.lines().collect();
    let mut text = lines.iter().take(max).map(|l| l.replacen(&root, "", 1)).collect::<Vec<_>>().join("\n");
    if lines.len() > max {
        text.push_str(&format!("\n[{} more lines: narrow the pattern, path or glob]", lines.len() - max));
    }
    (if text.is_empty() { "(nothing found)".into() } else { text }, false)
}

/// The explorer's instructions: it answers another agent's question about the code, briefly, and never edits.
const EXPLORE_PROMPT: &str = "You explore a codebase for another agent that will change it. Answer its question with file paths, line numbers and short quotes of the lines that matter: complete, but brief. Use search (grep) and list (find) to locate code, and read to read the parts you need (read takes an offset and limit). You cannot edit anything or run commands.";

/// `tazuna explore-mcp`: an MCP server with one tool, `explore`, answered by a cheap model on tazuna's own loop with
/// read-only tools. It lets Opus on the claude path hand off reading code (`--explorer MODEL@URL`), the part of a run that
/// costs most at Opus's rates. Each answer's cost goes to `$TAZUNA_EXPLORER_COSTS` (one number per line) when set.
pub fn explore_serve(args: &[String]) -> Result<()> {
    let flag = |name: &str| args.windows(2).find(|w| w[0] == name).map(|w| w[1].clone());
    let url = flag("--url").context("--url")?.trim_end_matches('/').to_string();
    let key_env = flag("--key-env").context("--key-env")?;
    let model = flag("--model").context("--model")?;
    let effort = flag("--effort").or(Some("max".into()));
    let (call, api, price) = connect(&url, &key_env, &model)?;
    let cwd = std::env::current_dir()?;
    let mut tools: Vec<Value> = crate::mcp::tools().as_array().into_iter().flatten()
        .filter(|t| t["name"] == "read")
        .map(|t| json!({"type": "function", "function": {"name": t["name"], "description": t["description"], "parameters": t["inputSchema"]}}))
        .collect();
    tools.extend(explore_tools());
    let answer = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
    let sink = answer.clone();
    let mut l = Loop {
        call,
        api,
        system: EXPLORE_PROMPT.into(),
        model,
        effort,
        price,
        tools,
        msgs: vec![],
        server: crate::mcp::Server::new(cwd),
        session: format!("explore-{}", std::process::id()),
        cost: 0.0,
        task: (std::time::Instant::now(), 0.0),
        queued: VecDeque::new(),
        mode: "explore".into(),
        asked: 0,
        send_reasoning: false,
        cache_key: true,
        routes: VecDeque::new(),
        route_calls: vec![],
        connector: connect_key,
        out: Box::new(move |e: &Value| {
            if e["type"] == "result" {
                *sink.borrow_mut() = e["result"].as_str().unwrap_or_default().to_string();
            }
        }),
    };
    let (_tx, rx) = std::sync::mpsc::channel(); // nothing interrupts an explorer answer
    let costs = std::env::var_os("TAZUNA_EXPLORER_COSTS");
    let mut out = std::io::stdout().lock();
    for line in std::io::stdin().lock().lines() {
        let Ok(msg) = serde_json::from_str::<Value>(&line?) else { continue };
        let Some(id) = msg.get("id").cloned() else { continue }; // a notification: no reply
        let result = match msg["method"].as_str().unwrap_or_default() {
            "initialize" => json!({"protocolVersion": msg["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
                "capabilities": {"tools": {}}, "serverInfo": {"name": "explore", "version": env!("CARGO_PKG_VERSION")}}),
            "tools/list" => json!({"tools": [{"name": "explore",
                "description": "Ask a cheap explorer model a precise question about this repository's code. It searches and reads the code itself and answers with file paths, line numbers and the lines that matter. Ask several questions in parallel when they are independent.",
                "inputSchema": {"type": "object", "properties": {"question": {"type": "string", "description": "the question, with any names, paths or symptoms you already know"}}, "required": ["question"]}}]}),
            "tools/call" => {
                let question = msg["params"]["arguments"]["question"].as_str().unwrap_or_default().to_string();
                // a fresh conversation per question: answers stay focused and its context small
                l.msgs = if l.api == Api::Chat { vec![json!({"role": "system", "content": l.system})] } else { vec![] };
                let before = l.cost;
                l.run(&json!(question), false, &rx);
                if let Some(f) = &costs {
                    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(f) {
                        let _ = writeln!(file, "{}", l.cost - before);
                    }
                }
                let text = std::mem::take(&mut *answer.borrow_mut());
                json!({"content": [{"type": "text", "text": if text.is_empty() { "(the explorer found no answer)".to_string() } else { text }}]})
            }
            m => {
                writeln!(out, "{}", json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": format!("unknown method {m}")}}))?;
                out.flush()?;
                continue;
            }
        };
        writeln!(out, "{}", json!({"jsonrpc": "2.0", "id": id, "result": result}))?;
        out.flush()?;
    }
    Ok(())
}


/// The user's answer to a permission question.
enum Permit {
    Allow(Value),
    Deny(String),
    Stop,
}

impl Loop {
    fn set_mode(&mut self, m: &Value) {
        if m["type"] == "control_request" && m["request"]["subtype"] == "set_permission_mode" {
            if let Some(mode) = m["request"]["mode"].as_str() {
                self.mode = mode.to_string();
            }
        }
    }

    /// Asks the user before a command or an edit when the mode says so, in claude's protocol (a `can_use_tool` control
    /// request, answered by a `control_response`), so tazuna's existing question UI and "always" answers work unchanged.
    fn permit(&mut self, tool: &str, input: &Value, rx: &Receiver<Value>) -> Permit {
        if self.mode == "explore" {
            // the explorer (`tazuna explore-mcp`) only reads: no edits, and bash only for read-only commands
            // inside the repo only: what the explorer reads goes to its provider, so no key files or other projects
            let cwd = std::env::current_dir().unwrap_or_default();
            return match tool {
                "read" if inside(&cwd, input["path"].as_str().unwrap_or_default()).is_some() => Permit::Allow(input.clone()),
                "read" => Permit::Deny("That file is outside the repository.".into()),
                "search" | "list" => Permit::Allow(input.clone()),
                _ => Permit::Deny("The explorer can only search, list and read.".into()),
            };
        }
        let ask = match self.mode.as_str() {
            "default" => matches!(tool, "bash" | "write" | "edit"),
            "acceptEdits" => tool == "bash",
            _ => false,
        };
        if !ask {
            return Permit::Allow(input.clone());
        }
        self.asked += 1;
        let id = format!("perm-{}", self.asked);
        (self.out)(&json!({"type": "control_request", "request_id": id, "request": {"subtype": "can_use_tool", "tool_name": tool, "input": input}}));
        loop {
            let Ok(m) = rx.recv() else { return Permit::Stop }; // tazuna is gone: nobody can answer
            if m["type"] == "control_response" && m["response"]["request_id"] == id.as_str() {
                let r = &m["response"]["response"];
                return if r["behavior"] == "allow" {
                    Permit::Allow(if r["updatedInput"].is_object() { r["updatedInput"].clone() } else { input.clone() })
                } else {
                    Permit::Deny(r["message"].as_str().unwrap_or("The user declined this.").to_string())
                };
            }
            match (m["type"].as_str(), m["request"]["subtype"].as_str()) {
                (Some("control_request"), Some("interrupt")) => return Permit::Stop,
                (Some("user"), _) => self.queued.push_back(m),
                _ => self.set_mode(&m),
            }
        }
    }
}

/// claude's user content is a string or blocks; this slice sends text only.
fn text_of(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        _ => content.as_array().into_iter().flatten().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("\n"),
    }
}

fn emit(e: &Value) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{e}").and_then(|_| out.flush());
}

/// One model call through `api`. Rate limits, server errors, timeouts and dropped
/// connections get more tries over about 15 minutes: two of 18 benchmark runs hung on a call that never answered, a short
/// network outage ended a 38-minute run when the tries spanned only 40 s, and a free model "at capacity" (HTTP 429) ended
/// all seven runs of a benchmark when they spanned 4 minutes.
fn chat(http: &ureq::Agent, url: &str, key: &str, api: Api, body: &Value) -> Result<Value> {
    chat_in(http, url, key, api, body, groups())
}

/// Whether a call starts streamed (see chat_in). SubRouter streams again (0.0.93; unstreamed in 0.0.91-0.0.92): its
/// gateway cuts an unstreamed reply at ~125 s with a 524, so every long reasoning call waited two minutes for nothing,
/// and the providers that cached only unstreamed calls (a6aa) took 2-3x LingAI's time a call and failed under four
/// conversations at once; LingAI caches the same either way (2026-10-27).
fn streams_by_default(url: &str, api: Api) -> bool {
    !url.contains("unorouter.com") && !(url.contains("mortera.cloud") && matches!(api, Api::Responses))
}

fn chat_in(http: &ureq::Agent, url: &str, key: &str, api: Api, body: &Value, groups: &Groups) -> Result<Value> {
    let path = match api {
        Api::Chat => "chat/completions",
        Api::Responses => "responses",
        Api::Messages => "messages",
    };
    // streamed by default: gateways cap a reply that is not streamed (UnoRouter: complete within 90 s, which a 130K-token
    // prompt cannot meet, and the task ended with a 413), and a started stream is not cut by the time limit. UnoRouter
    // alone caches only calls that are not streamed (Luna on its si group: 0% streamed, 92% over a run not streamed), so
    // there a call starts unstreamed and is streamed after a 413 or an empty 200. TAZUNA_STREAM=0 or 1 decides for any provider.
    // Mortera Cloud sets its own cache breakpoints and caches nothing when a request brings a top-level one (Haiku 5.5:
    // 0 tokens written with it, 6,286 without), and its streamed Responses carry no usage (GPT-6 Luna looked 0% cached
    // and the cost cap saw nothing), so GPT goes unstreamed there
    let mortera = url.contains("mortera.cloud");
    let mut stream = std::env::var("TAZUNA_STREAM").map_or(streams_by_default(url, api), |v| v != "0");
    let mut body = body.clone();
    if mortera {
        body.as_object_mut().map(|o| o.remove("cache_control"));
        // one key per conversation keeps it on one upstream source for up to an hour (Mortera's cache_affinity_key, removed
        // before dispatch): warm caches, and no switch to an account that rejects the earlier thinking blocks
        let first: String = ["system", "instructions"].iter().map(|k| body[*k].to_string())
            .chain((0..2).flat_map(|i| [body["messages"][i].to_string(), body["input"][i].to_string()])).collect();
        let mut h = std::hash::DefaultHasher::new();
        std::hash::Hash::hash(&first, &mut h);
        body["cache_affinity_key"] = json!(format!("tz-{:016x}", std::hash::Hasher::finish(&h)));
        // its sources take OpenRouter's effort levels, which end at xhigh: max passed without an error and thought no
        // longer than low (DeepSeek: max 16K and 14K tokens on a counting question, low 11-21K, xhigh 21K and 25K)
        for p in ["/reasoning/effort", "/reasoning_effort", "/output_config/effort"] {
            if let Some(e) = body.pointer_mut(p).filter(|e| **e == "max") {
                *e = json!("xhigh");
            }
        }
    }
    // a call that times out is tried again with twice the time (up to 15 minutes): a slow provider's long reasoning answer
    // (MiMo V2.6 Pro at the official ~30 tok/s) takes over 3 minutes and was cut off at 180 s on every try. The limit is on
    // the reply's start; a stream that has started may run 15 minutes: cut at 180 s, UnoRouter's slow Luna calls were
    // billed anyway (with the input twice) and then sent again.
    // Shannon queues GPT calls before the first token (median 69 s, up to 396 s for gpt-6-luna) and bills a call tazuna
    // gave up on: a 180 s limit sent the first request of a run three times
    let mut limit = if url.contains("shannonapi") { Duration::from_secs(600) } else { CALL_TIMEOUT };
    const TRIES: [u64; 12] = [2, 8, 30, 60, 120, 120, 120, 120, 120, 120, 120, 0];
    let (mut i, mut limited) = (0, 0u64);
    while i < TRIES.len() {
        // with another route waiting, 3 tries are enough: that route is the better wait (0.0.79)
        let wait = if FALLBACK.load(Ordering::Relaxed) && i >= 2 { 0 } else { TRIES[i] };
        i += 1;
        if stream {
            body["stream"] = json!(true);
            if api == Api::Chat {
                body["stream_options"] = json!({"include_usage": true});
            }
        }
        let req = http.post(format!("{url}/{path}")).config().timeout_recv_response(Some(limit))
            .timeout_global(Some(Duration::from_secs(900))).build().header("Content-Type", "application/json");
        let req = match api {
            Api::Messages => req.header("x-api-key", key).header("anthropic-version", "2023-06-01"),
            _ => req.header("Authorization", format!("Bearer {key}")),
        };
        // one extra header for every call, e.g. `X-Group: <group>` to pin an UnoRouter group per run (its automatic routing
        // picked groups whose long reasoning calls never returned)
        let extra = std::env::var("TAZUNA_HEADER").ok();
        let req = match extra.as_deref().and_then(|h| h.split_once(':')) {
            Some((k, v)) => req.header(k.trim(), v.trim()),
            None => req,
        };
        let req = match groups.current() {
            Some(g) => req.header("X-Group", &g),
            None => req,
        };
        let sent = req.send(&body.to_string())
            .and_then(|mut r| Ok((r.status().as_u16(), r.body_mut().with_config().limit(50 << 20).read_to_string()?)));
        let (status, text) = match sent {
            Ok(st) => st,
            Err(e) if wait > 0 => {
                if matches!(e, ureq::Error::Timeout(_)) {
                    limit = (limit * 2).min(Duration::from_secs(900));
                }
                eprintln!("model call failed ({e}); retrying in {wait}s");
                pause(wait);
                continue;
            }
            Err(e) => return Err(e.into()),
        };
        if status == 200 {
            match sse(&text).map(|events| from_stream(&events, api)) {
                // an error inside a stream (a gateway's upstream failed after the headers) is retried like a 5xx
                Some(Err(e)) if wait > 0 => {
                    eprintln!("model call failed ({e:#}); retrying in {wait}s");
                    pause(wait);
                    continue;
                }
                Some(r) => return r,
                None => {
                    let r = serde_json::from_str(&text).context("provider reply is not JSON")?;
                    // some UnoRouter upstreams cut a slow unstreamed call at ~90 s with an empty 200 (0/0 tokens) instead of 413
                    if !stream && wait > 0 && empty_reply(&r, api) {
                        stream = true;
                        continue;
                    }
                    return Ok(r);
                }
            }
        }
        // 524: the gateway's own time limit on a reply that is not streamed (SubRouter's Cloudflare, ~100 s; a6aa,
        // 2026-10-26), which a started stream does not hit
        if matches!(status, 413 | 524) && !stream && wait > 0 {
            stream = true;
            continue;
        }
        // an upstream that tops out at xhigh rejects max (one of UnoRouter's si upstreams, HTTP 400, which ended a run):
        // that call goes again at xhigh
        if status == 400 && text.contains("'max'") && wait > 0 {
            let mut lowered = false;
            for p in ["/reasoning/effort", "/reasoning_effort", "/output_config/effort"] {
                if let Some(e) = body.pointer_mut(p).filter(|e| **e == "max") {
                    *e = json!("xhigh");
                    lowered = true;
                }
            }
            if lowered {
                continue;
            }
        }
        // the pinned group dropped out, or one of its upstreams refused the call (UnoRouter passes that on as a 400: "Provider
        // returned error ... Baidu", "bad_response_status_code"; it ended two runs, one after the task was solved): the
        // same call at once on the next group, not counted as a try
        let gone = ["get_channel_failed", "No permission to access this group", "no available provider"];
        let upstream = status == 400 && ["Provider returned error", "bad_response_status_code"].iter().any(|m| text.contains(m));
        if groups.current().is_some() && (upstream || gone.iter().any(|m| text.contains(m))) && groups.next(&text.chars().take(80).collect::<String>()) {
            i -= 1;
            continue;
        }
        // a 429 for a locked model or an empty balance never clears by waiting (Experiential Labs: "insufficient_quota"),
        // but the same type with code free_limit_reached is its hourly free allowance, which does
        let permanent = (text.contains("insufficient_quota") || text.contains("requires_purchase")) && !text.contains("free_limit_reached");
        // a rate limit clears by waiting, but an hourly allowance outlasts the ~15 minutes of tries above (Experiential
        // Labs' free models: 166,500 output tokens an hour, reset on the hour; a run that crossed it ended): the last try
        // is repeated every 5 minutes for up to 70 more minutes
        if status == 429 && !permanent && wait == 0 && limited < 70 * 60 && !FALLBACK.load(Ordering::Relaxed) {
            eprintln!("rate limited ({}); trying again in 300s", text.chars().take(120).collect::<String>());
            pause(300);
            limited += 300;
            i -= 1;
            continue;
        }
        // the pool holding this conversation is down: waiting does not bring it back soon, the caller moves the conversation
        if pinned_away(&text) {
            bail!("HTTP {status}: {}", text.chars().take(300).collect::<String>());
        }
        // Mortera Cloud's 422 "model_unavailable" says not to retry, yet passes within a minute or two (Muse Spark came
        // back after one; it ended a Haiku run at call 38): tried again like a 5xx
        if wait == 0 || permanent || !(status == 429 || status >= 500 || text.contains("model_unavailable")) {
            bail!("HTTP {status}: {}", text.chars().take(300).collect::<String>());
        }
        pause(wait);
    }
    unreachable!()
}

/// Sleeps between tries (not in tests).
fn pause(secs: u64) {
    if !cfg!(test) {
        std::thread::sleep(Duration::from_secs(secs));
    }
}

/// The `data:` payloads of a server-sent-event body, or None when the body is plain JSON (a provider that ignores `stream`).
fn sse(text: &str) -> Option<Vec<Value>> {
    if !text.trim_start().starts_with("data:") && !text.trim_start().starts_with("event:") && !text.trim_start().starts_with(':') {
        return None;
    }
    Some(text.lines().filter_map(|l| l.strip_prefix("data:")).map(str::trim).filter(|d| *d != "[DONE]")
        .filter_map(|d| serde_json::from_str(d).ok()).collect())
}

/// A streamed reply put back into the shape of a reply that was not streamed.
fn from_stream(events: &[Value], api: Api) -> Result<Value> {
    if api == Api::Messages {
        // blocks start whole and grow by deltas: text, thinking, signature, and a tool's input as pieces of JSON
        let (mut blocks, mut usage) = (Vec::<Value>::new(), json!({}));
        for e in events {
            match e["type"].as_str().unwrap_or_default() {
                "error" => bail!("provider error in stream: {}", e.to_string().chars().take(300).collect::<String>()),
                "message_start" => usage = e["message"]["usage"].clone(),
                "content_block_start" => blocks.push(e["content_block"].clone()),
                "content_block_delta" => {
                    let i = e["index"].as_u64().map_or(blocks.len().saturating_sub(1), |i| i as usize);
                    let Some(b) = blocks.get_mut(i) else { continue };
                    for k in ["text", "thinking", "signature", "partial_json"] {
                        if let Some(part) = e["delta"][k].as_str() {
                            b[k] = json!(format!("{}{part}", b[k].as_str().unwrap_or_default()));
                        }
                    }
                }
                // the final output count, and on some servers the input counts again
                "message_delta" => usage.as_object_mut().into_iter().for_each(|u| u.extend(e["usage"].as_object().cloned().unwrap_or_default())),
                _ => {}
            }
        }
        for b in blocks.iter_mut().filter(|b| b["type"] == "tool_use") {
            let input = b.as_object_mut().and_then(|o| o.remove("partial_json")).and_then(|j| j.as_str().map(String::from));
            if let Some(j) = input.filter(|j| !j.trim().is_empty()) {
                b["input"] = serde_json::from_str(&j).unwrap_or(json!({}));
            }
        }
        return Ok(json!({"content": blocks, "usage": usage}));
    }
    if api == Api::Responses {
        // the closing event carries the whole response
        if let Some(e) = events.iter().find(|e| e["type"] == "error" || e["type"] == "response.failed") {
            bail!("provider error in stream: {}", e.to_string().chars().take(300).collect::<String>());
        }
        return events.iter().rev().find(|e| e["type"] == "response.completed").map(|e| e["response"].clone())
            .context("stream ended without response.completed");
    }
    // calls by their `index`, which some gateways count across content blocks (text 0, tools 1, 2, ...)
    let (mut content, mut reasoning, mut usage, mut calls) = (String::new(), String::new(), Value::Null, Vec::<(u64, Value)>::new());
    for e in events {
        if let Some(err) = e.get("error") {
            bail!("provider error in stream: {}", err.to_string().chars().take(300).collect::<String>());
        }
        if !e["usage"].is_null() {
            usage = e["usage"].clone();
        }
        let d = &e["choices"][0]["delta"];
        content.push_str(d["content"].as_str().unwrap_or_default());
        // DeepSeek's field; OpenRouter-style gateways (SubRouter's foxrouter) stream it as `reasoning` instead, and
        // without it the reasoning never went back (0.0.94: twice the output a call on foxrouter, 2026-10-27)
        reasoning.push_str(d["reasoning_content"].as_str().filter(|r| !r.is_empty()).or(d["reasoning"].as_str()).unwrap_or_default());
        for t in d["tool_calls"].as_array().into_iter().flatten() {
            // a piece without an index continues the last call
            let i = t["index"].as_u64().or(calls.last().map(|c| c.0)).unwrap_or(0);
            if !calls.iter().any(|c| c.0 == i) {
                calls.push((i, json!({"id": "", "type": "function", "function": {"name": "", "arguments": ""}})));
            }
            let c = &mut calls.iter_mut().find(|c| c.0 == i).expect("just added").1;
            if let Some(id) = t["id"].as_str() {
                c["id"] = json!(id);
            }
            for k in ["name", "arguments"] {
                if let Some(part) = t["function"][k].as_str() {
                    let joined = format!("{}{part}", c["function"][k].as_str().unwrap_or_default());
                    c["function"][k] = json!(joined);
                }
            }
        }
    }
    let calls: Vec<Value> = calls.into_iter().map(|c| c.1).filter(|c| c["function"]["name"].as_str().is_some_and(|n| !n.is_empty())).collect();
    let mut message = json!({"role": "assistant", "content": content});
    if !calls.is_empty() {
        message["tool_calls"] = json!(calls);
    }
    if !reasoning.is_empty() {
        message["reasoning_content"] = json!(reasoning);
    }
    Ok(json!({"choices": [{"message": message}], "usage": usage}))
}

/// The model's prices from the provider's model list (per million tokens there, per token here), and the API it takes.
fn price(http: &ureq::Agent, url: &str, key: &str, model: &str) -> Result<(Price, Api)> {
    let api = |openai: bool| if model.starts_with("claude-") { Api::Messages } else if openai { Api::Responses } else { Api::Chat };
    // a set price needs no list: Shannon lists a model only while an account serving it is online (gpt-6-luna came and
    // went within the hour)
    if let Ok(v) = std::env::var("TAZUNA_PRICE") {
        return Ok((price_override(&v)?, api(model.starts_with("gpt-"))));
    }
    let mut r = http.get(format!("{url}/models")).header("Authorization", format!("Bearer {key}")).call()?;
    let list: Value = serde_json::from_str(&r.body_mut().with_config().limit(50 << 20).read_to_string()?)?;
    let m = list["data"].as_array().into_iter().flatten().find(|m| m["id"] == model).with_context(|| format!("{model} is not in {url}/models"))?;
    let (p, openai) = parse_price(m)?;
    Ok((p, api(openai)))
}

fn price_override(v: &str) -> Result<Price> {
    let n: Vec<f64> = v.split(',').map(|x| x.trim().parse::<f64>()).collect::<Result<_, _>>().context("TAZUNA_PRICE: three numbers")?;
    let [i, c, o] = n[..] else { bail!("TAZUNA_PRICE needs input,cached,output (USD per million tokens)") };
    Ok((i / 1e6, c / 1e6, o / 1e6))
}

/// CheaperInference and OpenRouter-style lists give USD per million (`input_per_million`, as string or number);
/// Experiential Labs gives nano-USD per million (`input_nano_usd_per_million_tokens`) and names the upstream provider in
/// `data_policy` instead of `owned_by`.
fn parse_price(m: &Value) -> Result<(Price, bool)> {
    let model = &m["id"];
    let p = |usd: &str, nano: &str| {
        let v = &m["pricing"][usd];
        v.as_str().and_then(|s| s.parse::<f64>().ok()).or(v.as_f64()).or(m["pricing"][nano].as_f64().map(|n| n / 1e9))
    };
    let input = p("input_per_million", "input_nano_usd_per_million_tokens").with_context(|| format!("no input price for {model}: the dollar cap needs one"))?;
    let cached = p("cache_read_input_per_million", "cached_input_nano_usd_per_million_tokens").unwrap_or(input);
    let output = p("output_per_million", "output_nano_usd_per_million_tokens").context("no output price")?;
    // gateways that list every model as "openai" (BeatAPI) still serve GPT models on the Responses API
    let openai = m["owned_by"] == "OpenAI" || m["provider"] == "OpenAI" || model.as_str().is_some_and(|id| id.starts_with("gpt-"))
        || m["data_policy"]["providers"].as_array().is_some_and(|ps| !ps.is_empty() && ps.iter().all(|p| p["provider"] == "openai"));
    Ok(((input / 1e6, cached / 1e6, output / 1e6), openai))
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_pass_with_tests_says_finish_once_and_a_rerun_without_change_says_so() {
        use super::{passed, test_note, GREEN, RERUN};
        assert!(passed("ok  \tgithub.com/rclone/rclone/backend/archive\t1.2s\n"));
        assert!(passed("test result: ok. 5 passed; 0 failed; 0 ignored\n"));
        assert!(!passed("--- FAIL: TestX (0.00s)\nFAIL\n"), "a failure behind `| tail` exits 0");
        assert!(!passed("3 passed, 1 failed\n"));
        assert!(!passed("ok  \tpkg\t1s\n\n[exit 1]"));
        assert!(!passed("?   \tpkg\t[no test files]\n"), "nothing ran");
        let (mut green, mut told) = (None, false);
        let ok = "ok  \tpkg\t1s\n";
        // tests pass before any test file changed: no note, but the tree is remembered
        assert_eq!(test_note("go test ./...", ok, || Some((1, false)), &mut green, &mut told), None);
        assert_eq!(test_note("go test ./...", ok, || Some((1, false)), &mut green, &mut told), Some(RERUN));
        assert_eq!(test_note("go test ./... 2>&1 | tail", ok, || Some((2, true)), &mut green, &mut told), Some(GREEN));
        assert_eq!(test_note("go test -race ./...", ok, || Some((2, true)), &mut green, &mut told), Some(RERUN));
        assert_eq!(test_note("cat > zz_probe_test.go <<EOF ... && go test -run Probe; rm zz_probe_test.go", ok, || Some((2, true)), &mut green, &mut told), None, "a probe written and removed in the command");
        // after a change, a pass gets nothing: the finish note comes once
        assert_eq!(test_note("go test ./...", ok, || Some((3, true)), &mut green, &mut told), None);
        assert_eq!(test_note("go vet ./...", ok, || panic!("not a test run"), &mut green, &mut told), None);
        assert_eq!(test_note("go test ./...", "FAIL\n", || panic!("failed runs leave the tree alone"), &mut green, &mut told), None);
        assert_eq!(test_note("go test ./...", ok, || None, &mut green, &mut told), None, "outside git: nothing");
        // the tree: a test file alone (a repro written before the fix) is not yet a change with tests
        let dir = std::env::temp_dir().join(format!("tazuna-tree-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let git = |a: &[&str]| std::process::Command::new("git").arg("-C").arg(&dir).args(a).output().unwrap();
        git(&["init", "-q"]);
        std::fs::write(dir.join("a.go"), "package a\n").unwrap();
        git(&["add", "."]);
        git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "base"]);
        std::fs::write(dir.join("a_test.go"), "package a\n").unwrap();
        assert!(matches!(super::tree_kinds(&dir), Some((_, true, false))), "a test file only");
        let (h1, both) = super::tree_state_in(&dir).unwrap();
        assert!(!both, "test file only");
        std::fs::write(dir.join("a.go"), "package a\n\nfunc F() {}\n").unwrap();
        let (h2, both) = super::tree_state_in(&dir).unwrap();
        assert!(both && h1 != h2, "test and code");
        std::fs::remove_file(dir.join("a_test.go")).unwrap();
        assert!(matches!(super::tree_kinds(&dir), Some((_, false, true))), "code without a test");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn route_left_when_it_fails_slows_or_stops_caching() {
        // the rules
        let ok = |n: usize, secs: f64| vec![(secs, 1000u64, 900u64); n];
        assert_eq!(route_trouble(&ok(9, 100.0)), None, "fewer than 10 calls: no verdict");
        assert_eq!(route_trouble(&ok(10, 10.0)), None);
        assert_eq!(route_trouble(&vec![(5.0, 1000, 500); 10]), None, "the first 10 calls start cold: no cache verdict yet");
        assert!(route_trouble(&vec![(5.0, 1000, 500); 20]).unwrap().contains("50% cached"));
        assert!(route_trouble(&ok(10, 70.0)).unwrap().contains("70 s"), "slow from the start");
        let mut slowed = ok(10, 10.0);
        slowed.extend(ok(10, 45.0));
        assert!(route_trouble(&slowed).is_some(), "over 40 s and 3x the first 10");
        let mut steady = ok(10, 30.0);
        steady.extend(ok(10, 45.0));
        assert_eq!(route_trouble(&steady), None, "45 s after 30 s is not a slowdown");

        // a refusal moves the same turn to the next route, which finishes it
        fn next(_: &str, _: &str, _: &str) -> Result<(Box<dyn FnMut(&Value) -> Result<Value>>, Api, Price)> {
            Ok((Box::new(|_: &Value| Ok(json!({"choices": [{"message": {"role": "assistant", "content": "done"}}], "usage": {}}))), Api::Chat, (1e-6, 1e-7, 4e-6)))
        }
        let dir = std::env::temp_dir().join(format!("tazuna-routes-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let events = Rc::new(RefCell::new(vec![]));
        let ev = events.clone();
        let mut l = Loop {
            call: Box::new(|_: &Value| anyhow::bail!("HTTP 403: no permission")),
            api: Api::Chat,
            system: "sys".into(),
            model: "deepseek-v4.1-flash-1017".into(),
            effort: None,
            price: (1e-6, 1e-7, 4e-6),
            tools: vec![],
            msgs: vec![json!({"role": "system", "content": "sys"})],
            server: crate::mcp::Server::new(dir.clone()),
            session: "s".into(),
            cost: 0.0,
            task: (std::time::Instant::now(), 0.0),
            queued: VecDeque::new(),
            mode: "bypassPermissions".into(),
            asked: 0,
            send_reasoning: false,
            cache_key: true,
            routes: VecDeque::from([("https://b.example/v1".to_string(), "k".to_string())]),
            route_calls: vec![],
            connector: next,
            out: Box::new(move |e: &Value| ev.borrow_mut().push(e.clone())),
        };
        let (_tx, rx) = std::sync::mpsc::channel();
        l.run(&json!("hi"), false, &rx);
        let events = events.borrow();
        let switch = events.iter().find(|e| e["subtype"] == "route_switch").expect("a route_switch event");
        assert!(switch["message"].as_str().unwrap().contains("b.example") && switch["message"].as_str().unwrap().contains("403"));
        assert_eq!(events.last().unwrap()["subtype"], "success", "the next route finished the turn");
        assert!(l.send_reasoning == reasoning_back() && l.routes.is_empty(), "per-route flags start over; the list is used up");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_conversation_whose_pool_is_gone_moves_without_its_reasoning() {
        let dir = std::env::temp_dir().join(format!("tazuna-moved-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sent = Rc::new(RefCell::new(vec![]));
        let s = sent.clone();
        let events = Rc::new(RefCell::new(vec![]));
        let ev = events.clone();
        let mut l = Loop {
            call: Box::new(move |b: &Value| {
                s.borrow_mut().push(b.clone());
                if b["input"].as_array().unwrap().iter().any(|m| m["type"] == "reasoning") {
                    anyhow::bail!("HTTP 503: {{\"error\":{{\"code\":\"responses_source_unavailable\",\"message\":\"original conversation supply is currently unavailable\"}}}}");
                }
                Ok(json!({"output": [{"type": "message", "content": [{"type": "output_text", "text": "done"}]}], "usage": {}}))
            }),
            api: Api::Responses,
            system: "sys".into(),
            model: "gpt-6.1-sol".into(),
            effort: Some("max".into()),
            price: (1e-6, 1e-7, 4e-6),
            tools: vec![],
            msgs: vec![json!({"role": "user", "content": "fix it"}), json!({"type": "reasoning", "encrypted_content": "pool-50"}),
                       json!({"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "plan: parser first"}]})],
            server: crate::mcp::Server::new(dir.clone()),
            session: "s1".into(),
            cost: 0.0,
            task: (std::time::Instant::now(), 0.0),
            queued: VecDeque::new(),
            mode: "bypassPermissions".into(),
            asked: 0,
            send_reasoning: false,
            cache_key: true,
            routes: VecDeque::new(),
            route_calls: vec![],
            connector: connect_key,
            out: Box::new(move |e: &Value| ev.borrow_mut().push(e.clone())),
        };
        let (_tx, rx) = std::sync::mpsc::channel();
        l.run(&json!("go on"), false, &rx);
        let sent = sent.borrow();
        assert_eq!(sent.len(), 2, "one refused call, one moved");
        assert_eq!(sent[0]["prompt_cache_key"], "s1");
        assert!(!sent[1]["input"].as_array().unwrap().iter().any(|m| m["type"] == "reasoning"), "the reasoning stays behind");
        assert!(sent[1]["input"].as_array().unwrap().iter().any(|m| m["content"][0]["text"] == "plan: parser first"), "the visible text goes along");
        assert_eq!(sent[1]["prompt_cache_key"], "s1-moved", "a new cache key, so the gateway treats it as a new conversation");
        let events = events.borrow();
        assert!(events.iter().any(|e| e["subtype"] == "conversation_moved"));
        assert_eq!(events.last().unwrap()["subtype"], "success");
        assert!(pinned_away("HTTP 503: responses_source_unavailable") && !pinned_away("HTTP 503: shared_route_recovering"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn cache_key_dropped_when_refused() {
        let dir = std::env::temp_dir().join(format!("tazuna-cachekey-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sent = Rc::new(RefCell::new(vec![]));
        let s = sent.clone();
        let mut l = Loop {
            call: Box::new(move |b: &Value| {
                s.borrow_mut().push(b.clone());
                if b.get("prompt_cache_key").is_some() {
                    anyhow::bail!("HTTP 503: {{\"error\":{{\"code\":\"model_not_found\",\"message\":\"该分组下模型无可用渠道\"}}}}");
                }
                Ok(json!({"output": [{"type": "message", "content": [{"type": "output_text", "text": "done"}]}], "usage": {}}))
            }),
            api: Api::Responses,
            system: "sys".into(),
            model: "gpt-6-luna".into(),
            effort: Some("max".into()),
            price: (1e-6, 1e-7, 4e-6),
            tools: vec![],
            msgs: vec![],
            server: crate::mcp::Server::new(dir.clone()),
            session: "s1".into(),
            cost: 0.0,
            task: (std::time::Instant::now(), 0.0),
            queued: VecDeque::new(),
            mode: "bypassPermissions".into(),
            asked: 0,
            send_reasoning: false,
            cache_key: true,
        routes: VecDeque::new(),
        route_calls: vec![],
        connector: connect_key,
            out: Box::new(|_: &Value| {}),
        };
        let (_tx, rx) = std::sync::mpsc::channel();
        l.run(&json!("hi"), false, &rx);
        let sent = sent.borrow();
        assert_eq!(sent[0]["prompt_cache_key"], "s1", "sent by default");
        assert!(sent[1].get("prompt_cache_key").is_none(), "the refused call goes again without it");
        assert!(!l.cache_key, "off for the rest of the run");
        assert!(refused_cache_key("HTTP 503: model_not_found") && !refused_cache_key("HTTP 503: overloaded"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn reasoning_goes_back_only_when_asked() {
        // the stream's reasoning is kept with the turn
        let stream = "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"look at a\"}}]}\n\n\
data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"c1\",\"function\":{\"name\":\"read\",\"arguments\":\"{\\\"path\\\":\\\"a.txt\\\"}\"}}]}}]}\n\n\
data: {\"choices\":[],\"usage\":{\"prompt_tokens\":5,\"completion_tokens\":3}}\n";
        let first = from_stream(&sse(stream).unwrap(), Api::Chat).unwrap();
        assert_eq!(first["choices"][0]["message"]["reasoning_content"], "look at a");
        // OpenRouter-style gateways stream it as `reasoning`
        let other = "data: {\"choices\":[{\"delta\":{\"reasoning\":\"look \",\"reasoning_details\":[]}}]}\n\n\
data: {\"choices\":[{\"delta\":{\"reasoning\":\"at a\"}}]}\n\n";
        assert_eq!(from_stream(&sse(other).unwrap(), Api::Chat).unwrap()["choices"][0]["message"]["reasoning_content"], "look at a");

        let dir = std::env::temp_dir().join(format!("tazuna-reasoning-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), "a\n").unwrap();
        let sent = Rc::new(RefCell::new(vec![]));
        let (s, mut n) = (sent.clone(), 0);
        let mut l = Loop {
            call: Box::new(move |b: &Value| {
                s.borrow_mut().push(b.clone());
                n += 1;
                let has = b["messages"].as_array().unwrap().iter().any(|m| m.get("reasoning_content").is_some());
                match n {
                    1 => Ok(first.clone()),
                    // a provider that needs the reasoning back refuses the call without it
                    _ if !has => anyhow::bail!("HTTP 400: This reasoning model requires previous reasoning_content in multi-turn requests."),
                    _ => Ok(json!({"choices": [{"message": {"role": "assistant", "content": "done"}}], "usage": {}})),
                }
            }),
            api: Api::Chat,
            system: "sys".into(),
            model: "deepseek-v4.1-flash-1017".into(),
            effort: Some("max".into()),
            price: (1e-6, 1e-7, 4e-6),
            tools: vec![],
            msgs: vec![json!({"role": "system", "content": "sys"})],
            server: crate::mcp::Server::new(dir.clone()),
            session: "s".into(),
            cost: 0.0,
            task: (std::time::Instant::now(), 0.0),
            queued: VecDeque::new(),
            mode: "bypassPermissions".into(),
            asked: 0,
            send_reasoning: false,
        cache_key: true,
        routes: VecDeque::new(),
        route_calls: vec![],
        connector: connect_key,
            out: Box::new(|_: &Value| {}),
        };
        let (_tx, rx) = std::sync::mpsc::channel();
        l.run(&json!("read a"), false, &rx);
        let sent = sent.borrow();
        assert_eq!(sent.len(), 3, "the first call, the refused one, the same again with the reasoning");
        assert!(sent[1]["messages"].as_array().unwrap().iter().all(|m| m.get("reasoning_content").is_none()), "not sent by default");
        assert_eq!(sent[2]["messages"][2]["reasoning_content"], "look at a");
        assert!(l.send_reasoning, "kept on for the rest of the run");
        std::fs::remove_dir_all(&dir).ok();
    }
    #[test]
    fn a_long_reasoning_step_leaves_its_end_as_a_note() {
        use super::kept_reasoning;
        assert_eq!(kept_reasoning("short plan"), None, "short steps leave nothing");
        let long = format!("{}PLAN: add Token.IsBlockOpen and use it in parse.go", "é".repeat(5000));
        let note = kept_reasoning(&long).unwrap();
        assert!(note.contains("PLAN: add Token.IsBlockOpen") && note.chars().count() < 2700, "the end, bounded, cut on a char boundary");
    }

    #[test]
    fn unorouter_starts_unstreamed_subrouter_streams() {
        use super::{streams_by_default, Api};
        assert!(streams_by_default("https://subrouter.ai/v1", Api::Chat));
        assert!(!streams_by_default("https://api.unorouter.com/v1", Api::Chat));
        assert!(streams_by_default("https://api.cheaperinference.com/v1", Api::Chat));
        assert!(!streams_by_default("https://mortera.cloud/v1", Api::Responses) && streams_by_default("https://mortera.cloud/v1", Api::Chat));
    }

    #[test]
    fn reasoning_goes_back_by_default_and_not_where_refused() {
        let dir = std::env::temp_dir().join(format!("tazuna-reasoning2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), "a\n").unwrap();
        let first = json!({"choices": [{"message": {"role": "assistant", "content": null, "reasoning_content": "look at a",
            "tool_calls": [{"id": "c1", "type": "function", "function": {"name": "read", "arguments": "{\"path\":\"a.txt\"}"}}]}}], "usage": {}});
        let sent = Rc::new(RefCell::new(vec![]));
        let (s, mut n) = (sent.clone(), 0);
        let mut l = Loop {
            call: Box::new(move |b: &Value| {
                s.borrow_mut().push(b.clone());
                n += 1;
                let has = b["messages"].as_array().unwrap().iter().any(|m| m.get("reasoning_content").is_some());
                match n {
                    1 => Ok(first.clone()),
                    _ if has => anyhow::bail!("HTTP 400: unknown field `reasoning_content`"),
                    _ => Ok(json!({"choices": [{"message": {"role": "assistant", "content": "done"}}], "usage": {}})),
                }
            }),
            api: Api::Chat,
            system: "sys".into(),
            model: "m".into(),
            effort: None,
            price: (1e-6, 1e-7, 4e-6),
            tools: vec![],
            msgs: vec![json!({"role": "system", "content": "sys"})],
            server: crate::mcp::Server::new(dir.clone()),
            session: "s".into(),
            cost: 0.0,
            task: (std::time::Instant::now(), 0.0),
            queued: VecDeque::new(),
            mode: "bypassPermissions".into(),
            asked: 0,
            send_reasoning: true,
            cache_key: true,
            routes: VecDeque::new(),
            route_calls: vec![],
            connector: connect_key,
            out: Box::new(|_: &Value| {}),
        };
        let (_tx, rx) = std::sync::mpsc::channel();
        l.run(&json!("read a"), false, &rx);
        let sent = sent.borrow();
        assert_eq!(sent.len(), 3, "the first call, the refused one, the same again without the reasoning");
        assert_eq!(sent[1]["messages"][2]["reasoning_content"], "look at a", "sent by default");
        assert!(sent[2]["messages"].as_array().unwrap().iter().all(|m| m.get("reasoning_content").is_none()));
        assert!(!l.send_reasoning, "off for the rest of the run");
        std::fs::remove_dir_all(&dir).ok();
    }

    use super::*;
    use std::{cell::RefCell, rc::Rc};

    #[test]
    fn price_override_is_usd_per_million() {
        assert_eq!(price_override("0.3, 0.006,1.2").unwrap(), (0.3 / 1e6, 0.006 / 1e6, 1.2 / 1e6));
        assert!(price_override("0.3,1.2").is_err());
        assert!(price_override("a,b,c").is_err());
    }

    #[test]
    fn streamed_replies_become_whole_replies() {
        let chat = "data: {\"choices\":[{\"delta\":{\"content\":\"Let me \"}}]}\n\n\
data: {\"choices\":[{\"delta\":{\"content\":\"look.\",\"tool_calls\":[{\"index\":0,\"id\":\"c1\",\"function\":{\"name\":\"read\",\"arguments\":\"{\\\"pa\"}}]}}]}\n\n\
data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"th\\\":\\\"a\\\"}\"}}]}}]}\n\n\
data: {\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":3}}\n\ndata: [DONE]\n";
        let r = from_stream(&sse(chat).unwrap(), Api::Chat).unwrap();
        let m = &r["choices"][0]["message"];
        assert_eq!(m["content"], "Let me look.");
        assert_eq!(m["tool_calls"][0]["id"], "c1");
        assert_eq!(m["tool_calls"][0]["function"]["name"], "read");
        assert_eq!(m["tool_calls"][0]["function"]["arguments"], "{\"path\":\"a\"}");
        assert_eq!(r["usage"]["prompt_tokens"], 10);
        let gapped = "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":1,\"id\":\"t1\",\"function\":{\"name\":\"bash\",\"arguments\":\"{\"}}]}}]}\n\n\
data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"function\":{\"arguments\":\"}\"}}]}}]}\n\n\
data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":2,\"id\":\"t2\",\"function\":{\"name\":\"read\",\"arguments\":\"{}\"}}]}}]}\n";
        let g = from_stream(&sse(gapped).unwrap(), Api::Chat).unwrap();
        let calls = g["choices"][0]["message"]["tool_calls"].as_array().unwrap();
        assert_eq!(calls.len(), 2, "no empty call for index 0");
        assert_eq!((calls[0]["function"]["name"].as_str(), calls[0]["function"]["arguments"].as_str()), (Some("bash"), Some("{}")));
        assert_eq!(calls[1]["id"], "t2");
        let resp = "event: response.created\ndata: {\"type\":\"response.created\"}\n\nevent: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"output\":[],\"usage\":{\"input_tokens\":5}}}\n";
        assert_eq!(from_stream(&sse(resp).unwrap(), Api::Responses).unwrap()["usage"]["input_tokens"], 5);
        assert!(sse("{\"choices\":[]}").is_none(), "a plain JSON reply is used as it is");
    }

    #[test]
    fn messages_api_keeps_thinking_and_sends_results_in_one_turn() {
        let stream = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":10,\"cache_read_input_tokens\":900,\"cache_creation_input_tokens\":90,\"output_tokens\":1}}}\n\n\
data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\",\"signature\":\"\"}}\n\n\
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"read both\"}}\n\n\
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"signature_delta\",\"signature\":\"sig\"}}\n\n\
data: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n\
data: {\"type\":\"content_block_start\",\"index\":2,\"content_block\":{\"type\":\"tool_use\",\"id\":\"t1\",\"name\":\"read\",\"input\":{}}}\n\n\
data: {\"type\":\"content_block_delta\",\"index\":2,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"path\\\":\"}}\n\n\
data: {\"type\":\"content_block_delta\",\"index\":2,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"\\\"a.txt\\\"}\"}}\n\n\
data: {\"type\":\"content_block_start\",\"index\":3,\"content_block\":{\"type\":\"tool_use\",\"id\":\"t2\",\"name\":\"read\",\"input\":{}}}\n\n\
data: {\"type\":\"content_block_delta\",\"index\":3,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"path\\\":\\\"b.txt\\\"}\"}}\n\n\
data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"},\"usage\":{\"output_tokens\":40}}\n\n\
data: {\"type\":\"message_stop\"}\n";
        let first = from_stream(&sse(stream).unwrap(), Api::Messages).unwrap();
        assert_eq!(first["content"][0], json!({"type": "thinking", "thinking": "read both", "signature": "sig"}));
        assert_eq!(first["content"][2], json!({"type": "tool_use", "id": "t1", "name": "read", "input": {"path": "a.txt"}}));
        assert_eq!((first["usage"]["input_tokens"].as_u64(), first["usage"]["output_tokens"].as_u64()), (Some(10), Some(40)));
        assert!(from_stream(&sse("data: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\"}}\n").unwrap(), Api::Messages).is_err());

        let dir = std::env::temp_dir().join(format!("tazuna-messages-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), "a\n").unwrap();
        std::fs::write(dir.join("b.txt"), "b\n").unwrap();
        let replies = Rc::new(RefCell::new(VecDeque::from([first, json!({"content": [{"type": "text", "text": "done"}], "usage": {"input_tokens": 5}})])));
        let sent = Rc::new(RefCell::new(vec![]));
        let (r, s) = (replies.clone(), sent.clone());
        let mut l = Loop {
            call: Box::new(move |b: &Value| {
                s.borrow_mut().push(b.clone());
                Ok(r.borrow_mut().pop_front().unwrap())
            }),
            api: Api::Messages,
            system: "sys".into(),
            model: "claude-haiku-5-5".into(),
            effort: Some("max".into()),
            price: (1e-6, 1e-7, 4e-6),
            tools: vec![],
            msgs: vec![],
            server: crate::mcp::Server::new(dir.clone()),
            session: "s".into(),
            cost: 0.0,
            task: (std::time::Instant::now(), 0.0),
            queued: VecDeque::new(),
            mode: "bypassPermissions".into(),
            asked: 0,
        send_reasoning: false,
        cache_key: true,
        routes: VecDeque::new(),
        route_calls: vec![],
        connector: connect_key,
            out: Box::new(|_: &Value| {}),
        };
        let (_tx, rx) = std::sync::mpsc::channel();
        l.run(&json!("read a and b"), false, &rx);
        let sent = sent.borrow();
        assert_eq!((sent[0]["system"].as_str(), sent[0]["output_config"]["effort"].as_str()), (Some("sys"), Some("max")));
        let msgs = sent[1]["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 3, "user, assistant, one user turn with both results");
        assert_eq!(msgs[1]["content"][0]["signature"], "sig", "thinking goes back with its signature");
        assert_eq!(msgs[1]["content"].as_array().unwrap().len(), 3, "the empty text block is dropped");
        let results = msgs[2]["content"].as_array().unwrap();
        assert_eq!((results.len(), results[0]["tool_use_id"].as_str(), results[1]["tool_use_id"].as_str()), (2, Some("t1"), Some("t2")));
        assert_eq!(results[1]["content"].as_str().unwrap().trim_end(), "b");
        // 10 in + 90 written at 1.25x + 900 read + 40 out, then 5 in
        let want = 10.0 * 1e-6 + 90.0 * 1.25e-6 + 900.0 * 1e-7 + 40.0 * 4e-6 + 5.0 * 1e-6;
        assert!((l.cost - want).abs() < 1e-12, "{}", l.cost);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// One whole HTTP request (headers, then Content-Length bytes of body) from a test server's connection: answering
    /// before the body is read made the client fail now and then (closed with unread data).
    fn read_request(c: &mut std::net::TcpStream) -> String {
        use std::io::Read;
        let (mut req, mut buf) = (vec![], [0u8; 65536]);
        loop {
            let n = c.read(&mut buf).unwrap();
            req.extend_from_slice(&buf[..n]);
            let t = String::from_utf8_lossy(&req).to_string();
            if let Some((head, body)) = t.split_once("\r\n\r\n") {
                let len = head.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap())).unwrap_or(0);
                if body.len() >= len || n == 0 {
                    return t;
                }
            }
        }
    }

    #[test]
    fn a_gone_pool_is_not_waited_on() {
        use std::io::Write;
        let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", server.local_addr().unwrap());
        server.set_nonblocking(true).unwrap();
        let seen = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let s = seen.clone();
        let t = std::thread::spawn(move || {
            let end = std::time::Instant::now() + Duration::from_secs(3);
            while std::time::Instant::now() < end {
                match server.accept() {
                    Ok((mut c, _)) => {
                        c.set_nonblocking(false).unwrap();
                        read_request(&mut c);
                        c.write_all(b"HTTP/1.1 503 Service Unavailable\r\nConnection: close\r\n\r\n{\"error\":{\"code\":\"responses_source_unavailable\"}}").unwrap();
                        s.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(20)),
                }
            }
        });
        let http: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).build().into();
        let e = chat(&http, &url, "k", Api::Responses, &json!({"model": "gpt-6.1-sol", "input": []})).unwrap_err();
        assert!(pinned_away(&format!("{e:#}")));
        t.join().unwrap();
        assert_eq!(seen.load(Ordering::Relaxed), 1, "one request: the loop moves the conversation instead of waiting");
    }

    #[test]
    fn a_group_that_drops_out_or_stops_caching_is_left_for_the_next() {
        use std::io::Write;
        // a server on which group "a" fails with `fail` and every other group answers
        fn serve(fail: &'static str) -> (String, std::thread::JoinHandle<Vec<String>>) {
            let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}", server.local_addr().unwrap());
            (url, std::thread::spawn(move || {
                let mut groups = vec![];
                for _ in 0..2 {
                    let (mut c, _) = server.accept().unwrap();
                    let head = read_request(&mut c).to_lowercase();
                    let g = head.lines().find_map(|l| l.strip_prefix("x-group:").map(|v| v.trim().to_string())).unwrap_or_default();
                    let reply = if g == "a" { fail } else {
                        "200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"choices\":[{\"message\":{\"content\":\"ok\"}}]}"
                    };
                    c.write_all(format!("HTTP/1.1 {reply}").as_bytes()).unwrap();
                    groups.push(g);
                }
                groups
            }))
        }
        let http: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).build().into();
        // the group dropped out, or one of its upstreams refused the call (passed on as a 400)
        for fail in ["503 Service Unavailable\r\nConnection: close\r\n\r\n{\"error\":{\"code\":\"get_channel_failed\"}}",
            "400 Bad Request\r\nConnection: close\r\n\r\n{\"error\":{\"message\":\"Provider returned error (Baidu)\"}}"] {
            let (url, seen) = serve(fail);
            let g = Groups::from("a, b, c", None);
            let r = chat_in(&http, &url, "k", Api::Chat, &json!({"model": "m", "messages": []}), &g).unwrap();
            assert_eq!(r["choices"][0]["message"]["content"], "ok");
            assert_eq!(seen.join().unwrap(), ["a", "b"], "the call went again at once on the next group");
            assert_eq!(g.drain().len(), 1, "the switch is kept for the run log");
            assert!(g.drain().is_empty());
        }
        // ten calls at half cached move b on to c; on the last group nothing moves
        let g = Groups::from("a, b, c", None);
        g.at.store(1, Ordering::Relaxed);
        for _ in 0..10 {
            g.note(1000, 500);
        }
        assert_eq!(g.current().as_deref(), Some("c"));
        for _ in 0..10 {
            g.note(1000, 0);
        }
        assert_eq!(g.current().as_deref(), Some("c"));
        // groups added to the file while the run goes are there for the next switch
        let f = std::env::temp_dir().join(format!("tz-groups-{}", std::process::id()));
        std::fs::write(&f, "a\n").unwrap();
        let g = Groups::from("", Some(f.clone()));
        assert_eq!(g.current().as_deref(), Some("a"));
        std::fs::write(&f, "a\nb, a\n").unwrap();
        assert!(g.next("gone") && g.current().as_deref() == Some("b"));
        assert!(!g.next("gone"), "nothing left after b");
        std::fs::remove_file(f).ok();
    }

    #[test]
    fn a_rate_limit_is_waited_out_past_the_normal_tries() {
        use std::io::Write;
        let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", server.local_addr().unwrap());
        // 12 normal tries and 3 of the long waits answered 429, then the allowance is back
        let replies: Vec<&str> = std::iter::repeat_n("429 Too Many Requests\r\n\r\n{\"error\":{\"type\":\"insufficient_quota\",\"code\":\"free_limit_reached\"}}", 15)
            .chain(["200 OK\r\nContent-Type: application/json\r\n\r\n{\"choices\":[{\"message\":{\"content\":\"ok\"}}]}"]).collect();
        let n = replies.len();
        std::thread::spawn(move || {
            for reply in replies {
                let (mut c, _) = server.accept().unwrap();
                read_request(&mut c);
                let reply = format!("HTTP/1.1 {}", reply.replacen("\r\n\r\n", "\r\nConnection: close\r\n\r\n", 1));
                c.write_all(reply.as_bytes()).unwrap();
            }
        });
        let http: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).build().into();
        let r = chat(&http, &url, "k", Api::Chat, &json!({"model": "m", "messages": []})).unwrap();
        assert_eq!((r["choices"][0]["message"]["content"].as_str(), n), (Some("ok"), 16));
    }

    #[test]
    fn a_call_rejected_for_max_effort_goes_again_at_xhigh() {
        use std::io::{Read, Write};
        let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/unorouter.com", server.local_addr().unwrap());
        let got = std::thread::spawn(move || {
            let mut bodies = vec![];
            for reply in ["400 Bad Request\r\n\r\n{\"error\":\"'reasoning_effort' does not support 'max'\"}",
                "200 OK\r\nContent-Type: application/json\r\n\r\n{\"choices\":[{\"message\":{\"content\":\"\"}}]}",
                "200 OK\r\nContent-Type: application/json\r\n\r\n{\"choices\":[{\"message\":{\"content\":\"ok\"}}]}"] {
                let (mut c, _) = server.accept().unwrap();
                let mut req = vec![];
                let mut buf = [0u8; 4096];
                // headers, then a body of Content-Length bytes
                let body = loop {
                    let n = c.read(&mut buf).unwrap();
                    req.extend_from_slice(&buf[..n]);
                    let t = String::from_utf8_lossy(&req).to_string();
                    if let Some((head, body)) = t.split_once("\r\n\r\n") {
                        let len: usize = head.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap())).unwrap();
                        if body.len() >= len {
                            break body.to_string();
                        }
                    }
                };
                bodies.push(body);
                let reply = format!("HTTP/1.1 {}", reply.replacen("\r\n\r\n", "\r\nConnection: close\r\n\r\n", 1));
                c.write_all(reply.as_bytes()).unwrap();
            }
            bodies
        });
        let http: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).build().into();
        let r = chat(&http, &url, "k", Api::Chat, &json!({"model": "m", "messages": [], "reasoning_effort": "max"})).unwrap();
        assert_eq!(r["choices"][0]["message"]["content"], "ok");
        let bodies = got.join().unwrap();
        assert!(bodies[0].contains("\"max\"") && bodies[1].contains("\"xhigh\""), "{bodies:?}");
        assert!(!bodies[1].contains("\"stream\"") && bodies[2].contains("\"stream\":true"), "{bodies:?}");
    }

    #[test]
    fn a_model_marked_unavailable_is_tried_again() {
        use std::io::Write;
        let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1", server.local_addr().unwrap());
        std::thread::spawn(move || {
            for (status, reply) in [("422 Unprocessable Entity", "{\"error\":{\"code\":\"model_unavailable\",\"isRetryable\":false}}"),
                ("200 OK", "{\"choices\":[{\"message\":{\"content\":\"ok\"}}]}")] {
                let (mut c, _) = server.accept().unwrap();
                read_request(&mut c);
                write!(c, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}", reply.len()).unwrap();
            }
        });
        let http: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).build().into();
        let r = chat(&http, &url, "k", Api::Chat, &json!({"model": "m", "messages": [], "stream": false})).unwrap();
        assert_eq!(r["choices"][0]["message"]["content"], "ok");
    }

    #[test]
    fn mortera_gets_its_cache_key_xhigh_for_max_no_top_level_breakpoint_and_gpt_unstreamed() {
        use std::io::Write;
        let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/mortera.cloud", server.local_addr().unwrap());
        let got = std::thread::spawn(move || {
            let (mut c, _) = server.accept().unwrap();
            let req = read_request(&mut c);
            let reply = "{\"output\":[{\"type\":\"message\",\"content\":[{\"type\":\"output_text\",\"text\":\"ok\"}]}]}";
            write!(c, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}", reply.len()).unwrap();
            req.split_once("\r\n\r\n").unwrap().1.to_string()
        });
        let http: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).build().into();
        chat(&http, &url, "k", Api::Responses, &json!({"model": "gpt-6-luna", "input": [], "cache_control": {"type": "ephemeral"}, "reasoning": {"effort": "max"}})).unwrap();
        let body = got.join().unwrap();
        assert!(!body.contains("cache_control") && !body.contains("\"stream\"") && body.contains("\"cache_affinity_key\":\"tz-") && body.contains("\"effort\":\"xhigh\""), "{body}");
    }

    #[test]
    fn a_400_on_thinking_from_another_account_drops_the_thinking_and_goes_again() {
        let sent = Rc::new(RefCell::new(vec![]));
        let s = sent.clone();
        let mut l = Loop {
            call: Box::new(move |b: &Value| {
                s.borrow_mut().push(b.clone());
                if s.borrow().len() == 1 {
                    bail!("HTTP 400: {{\"error\":{{\"code\":\"invalid_request\"}}}}")
                }
                Ok(json!({"content": [{"type": "text", "text": "done"}], "usage": {}}))
            }),
            api: Api::Messages,
            system: "sys".into(),
            model: "claude-haiku-5-5".into(),
            effort: None,
            price: (0.0, 0.0, 0.0),
            tools: vec![],
            msgs: vec![json!({"role": "user", "content": "fix it"}),
                json!({"role": "assistant", "content": [{"type": "thinking", "thinking": "hm", "signature": "x"}, {"type": "tool_use", "id": "t1", "name": "bash", "input": {}}]}),
                json!({"role": "user", "content": [{"type": "tool_result", "tool_use_id": "t1", "content": "ok"}]})],
            server: crate::mcp::Server::new(std::env::temp_dir()),
            session: "s".into(),
            cost: 0.0,
            task: (std::time::Instant::now(), 0.0),
            queued: VecDeque::new(),
            mode: "bypassPermissions".into(),
            asked: 0,
        send_reasoning: false,
        cache_key: true,
        routes: VecDeque::new(),
        route_calls: vec![],
        connector: connect_key,
            out: Box::new(|_: &Value| {}),
        };
        let (_tx, rx) = std::sync::mpsc::channel();
        l.run(&json!("go on"), true, &rx);
        let sent = sent.borrow();
        assert_eq!(sent.len(), 2);
        assert!(sent[0].to_string().contains("\"thinking\"") && !sent[1]["messages"].to_string().contains("\"thinking\""), "{sent:?}");
        assert!(sent[1]["messages"].to_string().contains("tool_use"));
    }

    #[test]
    fn an_empty_reply_gets_a_nudge_instead_of_ending_the_task() {
        let empty = json!({"choices": [{"message": {"content": ""}}], "usage": {}});
        // each empty reply is first asked again three times (a provider hiccup), then nudged
        let replies = Rc::new(RefCell::new(VecDeque::from(vec![empty; 12])));
        let sent = Rc::new(RefCell::new(vec![]));
        let events = Rc::new(RefCell::new(vec![]));
        let (r, s, ev) = (replies.clone(), sent.clone(), events.clone());
        let mut l = Loop {
            call: Box::new(move |b: &Value| {
                s.borrow_mut().push(b.clone());
                Ok(r.borrow_mut().pop_front().unwrap())
            }),
            api: Api::Chat,
            system: "sys".into(),
            model: "m".into(),
            effort: None,
            price: (0.0, 0.0, 0.0),
            tools: vec![],
            msgs: vec![json!({"role": "system", "content": "sys"})],
            server: crate::mcp::Server::new(std::env::temp_dir()),
            session: "s".into(),
            cost: 0.0,
            task: (std::time::Instant::now(), 0.0),
            queued: VecDeque::new(),
            mode: "bypassPermissions".into(),
            asked: 0,
        send_reasoning: false,
        cache_key: true,
        routes: VecDeque::new(),
        route_calls: vec![],
        connector: connect_key,
            out: Box::new(move |e: &Value| ev.borrow_mut().push(e.clone())),
        };
        let (_tx, rx) = std::sync::mpsc::channel();
        l.run(&json!("fix the bug"), false, &rx);
        assert_eq!(sent.borrow().len(), 12, "three turns of four calls each: two nudges, then the task ends");
        assert_eq!(sent.borrow()[3], sent.borrow()[0], "a retry sends the same request");
        let last = sent.borrow()[8]["messages"].as_array().unwrap().last().unwrap().clone();
        assert_eq!((last["role"].as_str(), last["content"].as_str()), (Some("user"), Some(NUDGE)));
        assert_eq!(events.borrow().last().unwrap()["num_turns"].as_u64(), Some(3));
    }

    #[test]
    fn runs_tools_until_the_model_answers_and_counts_cost() {
        let dir = std::env::temp_dir().join(format!("tazuna-engine-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), "hello\n").unwrap();
        let replies = Rc::new(RefCell::new(VecDeque::from([
            // no id, as LingAI sometimes sends: tazuna gives it one
            json!({"choices": [{"message": {"content": null, "tool_calls": [{"id": "", "type": "function", "function": {"name": "read", "arguments": "{\"path\":\"a.txt\"}"}}]}}],
                   "usage": {"prompt_tokens": 1000, "completion_tokens": 10, "prompt_tokens_details": {"cached_tokens": 0}}}),
            json!({"choices": [{"message": {"content": "it says hello"}}], "usage": {"prompt_tokens": 1100, "completion_tokens": 5, "prompt_tokens_details": {"cached_tokens": 1000}}}),
        ])));
        let sent = Rc::new(RefCell::new(vec![]));
        let events = Rc::new(RefCell::new(vec![]));
        let (r, s, ev) = (replies.clone(), sent.clone(), events.clone());
        let mut l = Loop {
            call: Box::new(move |b: &Value| {
                s.borrow_mut().push(b.clone());
                Ok(r.borrow_mut().pop_front().unwrap())
            }),
            api: Api::Chat,
            system: "sys".into(),
            model: "m".into(),
            effort: None,
            price: (1e-6, 1e-7, 4e-6),
            tools: vec![],
            msgs: vec![json!({"role": "system", "content": "sys"})],
            server: crate::mcp::Server::new(dir.clone()),
            session: "s".into(),
            cost: 0.0,
            task: (std::time::Instant::now(), 0.0),
            queued: VecDeque::new(),
            mode: "bypassPermissions".into(),
            asked: 0,
        send_reasoning: false,
        cache_key: true,
        routes: VecDeque::new(),
        route_calls: vec![],
        connector: connect_key,
            out: Box::new(move |e: &Value| ev.borrow_mut().push(e.clone())),
        };
        let (_tx, rx) = std::sync::mpsc::channel();
        l.run(&json!("what is in a.txt?"), false, &rx);
        let events = events.borrow();
        let types: Vec<&str> = events.iter().map(|e| e["type"].as_str().unwrap()).collect();
        assert_eq!(types, ["stream_event", "stream_event", "assistant", "user", "stream_event", "stream_event", "assistant", "result"]);
        assert_eq!(events[3]["message"]["content"][0]["content"].as_str().unwrap().trim_end(), "hello");
        let r = events.last().unwrap();
        assert_eq!((r["subtype"].as_str(), r["result"].as_str(), r["num_turns"].as_u64()), (Some("success"), Some("it says hello"), Some(2)));
        // 1000 in + 10 out, then 100 in + 1000 cached + 5 out
        let want = 1000.0 * 1e-6 + 10.0 * 4e-6 + 100.0 * 1e-6 + 1000.0 * 1e-7 + 5.0 * 4e-6;
        assert!((r["total_cost_usd"].as_f64().unwrap() - want).abs() < 1e-12);
        // the second call carries the tool result
        assert_eq!(sent.borrow()[1]["messages"][3]["role"], "tool");
        let id = sent.borrow()[1]["messages"][2]["tool_calls"][0]["id"].clone();
        assert!(id.as_str().is_some_and(|i| !i.is_empty()) && sent.borrow()[1]["messages"][3]["tool_call_id"] == id, "{id}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn follow_ups_share_the_tasks_budget() {
        let calls = Rc::new(RefCell::new(0));
        let c = calls.clone();
        let mut l = Loop {
            call: Box::new(move |_: &Value| {
                *c.borrow_mut() += 1;
                Ok(json!({"choices": [{"message": {"content": "done"}}], "usage": {"cost": 1.2}}))
            }),
            api: Api::Chat,
            system: String::new(),
            model: "m".into(),
            effort: None,
            price: (0.0, 0.0, 0.0),
            tools: vec![],
            msgs: vec![],
            server: crate::mcp::Server::new(std::env::temp_dir()),
            session: "s".into(),
            cost: 0.0,
            task: (std::time::Instant::now(), 0.0),
            queued: VecDeque::new(),
            mode: "bypassPermissions".into(),
            asked: 0,
        send_reasoning: false,
        cache_key: true,
        routes: VecDeque::new(),
        route_calls: vec![],
        connector: connect_key,
            out: Box::new(|_: &Value| {}),
        };
        let (_tx, rx) = std::sync::mpsc::channel();
        l.run(&json!("task"), false, &rx); // spends $1.20, past the $1 cap
        l.run(&json!("tests fail"), true, &rx); // a follow-up: the cap is used up, no call
        assert_eq!(*calls.borrow(), 1);
        l.run(&json!("next task"), false, &rx); // a new task: a fresh budget
        assert_eq!(*calls.borrow(), 2);
    }

    #[test]
    fn reads_both_price_list_formats() {
        let ci = json!({"id": "deepseek-v4.1-flash", "owned_by": "DeepSeek", "pricing": {"input_per_million": "0.075", "cache_read_input_per_million": "0.0015", "output_per_million": 0.3}});
        let ((i, c, o), openai) = parse_price(&ci).unwrap();
        assert!(((i * 1e6) - 0.075).abs() < 1e-12 && ((c * 1e6) - 0.0015).abs() < 1e-12 && ((o * 1e6) - 0.3).abs() < 1e-12 && !openai);
        let el = json!({"id": "gpt-6-luna", "owned_by": "exp", "data_policy": {"providers": [{"provider": "openai"}]},
            "pricing": {"input_nano_usd_per_million_tokens": 100000000, "cached_input_nano_usd_per_million_tokens": 10000000, "output_nano_usd_per_million_tokens": 500000000}});
        let ((i, c, o), openai) = parse_price(&el).unwrap();
        assert!(((i * 1e6) - 0.1).abs() < 1e-12 && ((c * 1e6) - 0.01).abs() < 1e-12 && ((o * 1e6) - 0.5).abs() < 1e-12 && openai);
        assert!(parse_price(&json!({"id": "x", "pricing": {}})).is_err());
    }

    /// A loop whose model wants `write` once, then answers; `mode` decides whether that asks. A question is answered with
    /// `reply` (sent to the loop's input) when it is asked, as tazuna would.
    fn asking_loop(mode: &str, dir: &std::path::Path, events: Rc<RefCell<Vec<Value>>>, reply: Option<(std::sync::mpsc::Sender<Value>, Value)>) -> Loop {
        let replies = RefCell::new(VecDeque::from([
            json!({"choices": [{"message": {"content": null, "tool_calls": [{"id": "c1", "type": "function", "function": {"name": "write", "arguments": "{\"path\":\"b.txt\",\"content\":\"hi\\n\"}"}}]}}], "usage": {}}),
            json!({"choices": [{"message": {"content": "done"}}], "usage": {}}),
        ]));
        Loop {
            call: Box::new(move |_: &Value| Ok(replies.borrow_mut().pop_front().unwrap())),
            api: Api::Chat,
            system: String::new(),
            model: "m".into(),
            effort: None,
            price: (0.0, 0.0, 0.0),
            tools: vec![],
            msgs: vec![],
            server: crate::mcp::Server::new(dir.to_path_buf()),
            session: "s".into(),
            cost: 0.0,
            task: (std::time::Instant::now(), 0.0),
            queued: VecDeque::new(),
            mode: mode.into(),
            asked: 0,
        send_reasoning: false,
        cache_key: true,
        routes: VecDeque::new(),
        route_calls: vec![],
        connector: connect_key,
            out: Box::new(move |e: &Value| {
                if e["type"] == "control_request" {
                    if let Some((tx, r)) = &reply {
                        tx.send(r.clone()).unwrap();
                    }
                }
                events.borrow_mut().push(e.clone());
            }),
        }
    }

    #[test]
    fn ask_mode_asks_before_an_edit_and_honours_the_answer() {
        let dir = std::env::temp_dir().join(format!("tazuna-ask-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let answer = |behavior: &str| json!({"type": "control_response", "response": {"subtype": "success", "request_id": "perm-1",
            "response": {"behavior": behavior, "message": "not now"}}});
        // denied: nothing written, the model hears why
        let ev = Rc::new(RefCell::new(vec![]));
        let (tx, rx) = std::sync::mpsc::channel();
        asking_loop("default", &dir, ev.clone(), Some((tx, answer("deny")))).run(&json!("write b.txt"), false, &rx);
        let asked: Vec<_> = ev.borrow().iter().filter(|e| e["type"] == "control_request").cloned().collect();
        assert_eq!((asked.len(), asked[0]["request"]["tool_name"].as_str()), (1, Some("write")));
        assert!(!dir.join("b.txt").exists());
        let result = ev.borrow().iter().find(|e| e["type"] == "user").cloned().unwrap();
        assert_eq!(result["message"]["content"][0]["content"], "not now");
        // allowed: written
        let (tx, rx) = std::sync::mpsc::channel();
        asking_loop("default", &dir, Rc::new(RefCell::new(vec![])), Some((tx, answer("allow")))).run(&json!("write b.txt"), false, &rx);
        assert_eq!(std::fs::read_to_string(dir.join("b.txt")).unwrap(), "hi\n");
        // edits mode: an edit goes ahead without a question
        std::fs::remove_file(dir.join("b.txt")).unwrap();
        let ev = Rc::new(RefCell::new(vec![]));
        let (_tx, rx) = std::sync::mpsc::channel();
        asking_loop("acceptEdits", &dir, ev.clone(), None).run(&json!("write b.txt"), false, &rx);
        assert!(dir.join("b.txt").exists() && !ev.borrow().iter().any(|e| e["type"] == "control_request"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_interrupt_while_asking_ends_the_run() {
        let dir = std::env::temp_dir().join(format!("tazuna-ask-stop-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ev = Rc::new(RefCell::new(vec![]));
        let (tx, rx) = std::sync::mpsc::channel();
        let stop = json!({"type": "control_request", "request_id": "i1", "request": {"subtype": "interrupt"}});
        asking_loop("default", &dir, ev.clone(), Some((tx, stop))).run(&json!("write b.txt"), false, &rx);
        let asked = ev.borrow().iter().any(|e| e["type"] == "control_request");
        let last = ev.borrow().last().cloned().unwrap();
        assert!(asked); // the stop came while the question was open
        assert_eq!((last["type"].as_str(), last["result"].as_str()), (Some("result"), Some("[interrupted]")));
        assert!(!dir.join("b.txt").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_explorer_searches_and_lists_inside_the_repo_only() {
        let dir = std::env::temp_dir().join(format!("tazuna-explore-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/a.go"), "package a\nfunc Timeout() {}\n").unwrap();
        let (hit, err) = super::explore_tool("search", &json!({"pattern": "func Time", "glob": "*.go"}), &dir);
        assert!(!err && hit.contains("src/a.go:2:func Timeout"), "{hit}");
        // a pattern that looks like a flag is still a pattern
        let (none, err) = super::explore_tool("search", &json!({"pattern": "--version"}), &dir);
        assert!(!err && none == "(nothing found)", "{none}");
        let (files, _) = super::explore_tool("list", &json!({"glob": "*.go"}), &dir);
        assert!(files.contains("src/a.go"), "{files}");
        for out in ["..", "/etc", "../../"] {
            assert!(super::explore_tool("list", &json!({"path": out}), &dir).1, "{out}");
        }
        assert!(super::inside(&dir, "src/a.go").is_some() && super::inside(&dir, "/etc/hosts").is_none());
        assert!(super::inside(&dir, &format!("{}/../x", dir.display())).is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
