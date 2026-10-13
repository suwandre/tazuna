//! `tazuna engine`: tazuna's own agent loop for OpenAI-compatible providers (docs/OWN_LOOP.md, first slice). It reads and
//! writes claude's stream-json on stdin/stdout, so `Live` drives it like the `claude` process: the run log, verification
//! and the reviewer work unchanged. No streaming yet: each turn is one Chat Completions call.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    io::{BufRead, Write},
    sync::mpsc::{Receiver, TryRecvError},
    time::Duration,
};

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

pub fn serve(args: &[String]) -> Result<()> {
    let flag = |name: &str| args.windows(2).find(|w| w[0] == name).map(|w| w[1].clone());
    let url = flag("--url").context("--url")?.trim_end_matches('/').to_string();
    let key_env = flag("--key-env").context("--key-env")?;
    let model = flag("--model").context("--model")?;
    let effort = flag("--effort");
    let mode = flag("--permission-mode").unwrap_or_else(|| "bypassPermissions".into());
    let mut system = flag("--system-prompt").unwrap_or_default();
    if std::env::var_os("TAZUNA_BATCH").is_some() {
        system.push_str(BATCH_RULE);
    }
    let (call, responses, price) = connect(&url, &key_env, &model)?;

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
        responses,
        system: system.clone(),
        model,
        effort,
        price,
        tools: tools.as_array().into_iter().flatten().map(|t| json!({"type": "function", "function": {"name": t["name"], "description": t["description"], "parameters": t["inputSchema"]}})).collect(),
        // Responses keeps the system prompt in `instructions`
        msgs: if responses { vec![] } else { vec![json!({"role": "system", "content": system})] },
        server: crate::mcp::Server::new(cwd),
        session,
        cost: 0.0,
        task: (std::time::Instant::now(), 0.0),
        queued: VecDeque::new(),
        mode,
        asked: 0,
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
    /// OpenAI models use the Responses API: through Chat Completions with tools they do not reason at all (GPT-6 Luna:
    /// 0 reasoning tokens at any effort; Responses: 54-98).
    responses: bool,
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
    /// Permission questions asked so far (their request ids).
    asked: u32,
    out: Box<dyn FnMut(&Value)>,
}

impl Loop {
    /// One user message: model calls and tool runs until the model answers without tools, or a cap or interrupt ends it.
    fn run(&mut self, content: &Value, followup: bool, rx: &Receiver<Value>) {
        self.msgs.push(json!({"role": "user", "content": text_of(content)}));
        if !followup {
            self.task = (std::time::Instant::now(), self.cost);
        }
        let ((began, start), mut turns, mut usage) = (self.task, 0, [0u64; 3]);
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
            (self.out)(&json!({"type": "stream_event", "event": {"type": "message_start"}}));
            let body = if self.responses {
                // stateless (store: false): earlier reasoning comes back encrypted and is sent again with the history
                let tools: Vec<Value> = self.tools.iter().map(|t| json!({"type": "function", "name": t["function"]["name"], "description": t["function"]["description"], "parameters": t["function"]["parameters"]})).collect();
                let mut b = json!({"model": self.model, "instructions": self.system, "input": self.msgs, "tools": tools, "store": false, "include": ["reasoning.encrypted_content"]});
                if let Some(e) = &self.effort {
                    b["reasoning"] = json!({"effort": e});
                }
                b
            } else {
                let mut b = json!({"model": self.model, "messages": self.msgs, "tools": self.tools});
                if let Some(e) = &self.effort {
                    b["reasoning_effort"] = json!(e);
                }
                b
            };
            let reply = match (self.call)(&body) {
                Ok(r) => r,
                Err(e) => return end(self, "error_during_execution", format!("[provider error: {e:#}]"), turns, usage),
            };
            turns += 1;
            let u = &reply["usage"];
            let n = |chat: &Value, resp: &Value| if self.responses { resp } else { chat }.as_u64().unwrap_or(0);
            let (prompt, out) = (n(&u["prompt_tokens"], &u["input_tokens"]), n(&u["completion_tokens"], &u["output_tokens"]));
            let cached = n(&u["prompt_tokens_details"]["cached_tokens"], &u["input_tokens_details"]["cached_tokens"]).min(prompt);
            // the billed cost when the provider reports it (CheaperInference, OpenRouter), else the price list
            self.cost += u["cost"].as_f64().unwrap_or((prompt - cached) as f64 * self.price.0 + cached as f64 * self.price.1 + out as f64 * self.price.2);
            for (sum, n) in usage.iter_mut().zip([prompt - cached, cached, out]) {
                *sum += n;
            }
            (self.out)(&json!({"type": "stream_event", "event": {"type": "message_delta",
                "usage": {"input_tokens": prompt - cached, "cache_creation_input_tokens": 0, "cache_read_input_tokens": cached, "output_tokens": out}}}));

            // text and calls as (id, name, arguments), in Chat Completions' shape for both APIs
            let (text, calls): (String, Vec<Value>) = if self.responses {
                let items = reply["output"].as_array().cloned().unwrap_or_default();
                let text = items.iter().filter(|i| i["type"] == "message").flat_map(|i| i["content"].as_array().cloned().unwrap_or_default())
                    .filter_map(|c| c["text"].as_str().map(String::from)).collect::<Vec<_>>().join("\n");
                let calls = items.iter().filter(|i| i["type"] == "function_call")
                    .map(|i| json!({"id": i["call_id"], "function": {"name": i["name"], "arguments": i["arguments"]}})).collect();
                self.msgs.extend(items); // reasoning, messages and calls go back as they came
                (text, calls)
            } else {
                let m = &reply["choices"][0]["message"];
                let text = m["content"].as_str().unwrap_or_default().to_string();
                let calls: Vec<Value> = m["tool_calls"].as_array().cloned().unwrap_or_default();
                // only what the next call needs; reasoning fields are not sent back (some providers reject them)
                let mut keep = json!({"role": "assistant", "content": text});
                if !calls.is_empty() {
                    keep["tool_calls"] = json!(calls);
                }
                self.msgs.push(keep);
                (text, calls)
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
            (self.out)(&json!({"type": "assistant", "message": {"role": "assistant", "model": self.model, "content": blocks}}));
            if parsed.is_empty() {
                return end(self, "success", text, turns, usage);
            }
            let mut results = vec![];
            for (id, name, input) in parsed {
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
                self.msgs.push(if self.responses {
                    json!({"type": "function_call_output", "call_id": id, "output": text})
                } else {
                    json!({"role": "tool", "tool_call_id": id, "content": text})
                });
                results.push(json!({"type": "tool_result", "tool_use_id": id, "content": text, "is_error": is_error}));
            }
            (self.out)(&json!({"type": "user", "message": {"role": "user", "content": results}}));
        }
    }
}

/// A model call through the provider, its prices, and whether it takes the Responses API. Removes the key from this
/// process's environment: the agent's commands inherit it and must not see the key.
fn connect(url: &str, key_env: &str, model: &str) -> Result<(Box<dyn FnMut(&Value) -> Result<Value>>, bool, Price)> {
    let key = std::env::var(key_env).with_context(|| format!("{key_env} not set"))?;
    // SAFETY: called before any other thread starts.
    unsafe { std::env::remove_var(key_env) };
    let http: ureq::Agent = ureq::Agent::config_builder()
        // the system's TLS (macOS Security framework): ~1 MB less binary than rustls
        .tls_config(ureq::tls::TlsConfig::builder().provider(ureq::tls::TlsProvider::NativeTls).build())
        // a normal call answers in 15-60 s, a long reasoning one in a minute or two; 300 s let a provider that accepted a
        // call and never answered stall a run for an hour across the retries (GLM on Experiential Labs, 2026-10-12)
        .timeout_global(Some(CALL_TIMEOUT)).http_status_as_error(false).build().into();
    // retried like model calls: one benchmark run died at startup on a model-list request that never answered
    let (price, responses) = [2, 8, 30].iter().find_map(|w| price(&http, url, &key, model).map_err(|e| {
        eprintln!("model list failed ({e:#}); retrying in {w}s");
        std::thread::sleep(Duration::from_secs(*w));
    }).ok()).map_or_else(|| price(&http, url, &key, model), Ok)?;
    let url = url.to_string();
    Ok((Box::new(move |body: &Value| chat(&http, &url, &key, body)), responses, price))
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
    let (call, responses, price) = connect(&url, &key_env, &model)?;
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
        responses,
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
                l.msgs = if l.responses { vec![] } else { vec![json!({"role": "system", "content": l.system})] };
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

/// One model call (Chat Completions, or Responses for a body with `input`). Rate limits, server errors, timeouts and dropped
/// connections get more tries over about 15 minutes: two of 18 benchmark runs hung on a call that never answered, a short
/// network outage ended a 38-minute run when the tries spanned only 40 s, and a free model "at capacity" (HTTP 429) ended
/// all seven runs of a benchmark when they spanned 4 minutes.
fn chat(http: &ureq::Agent, url: &str, key: &str, body: &Value) -> Result<Value> {
    let path = if body.get("input").is_some() { "responses" } else { "chat/completions" };
    let body = body.to_string();
    // a call that times out is tried again with twice the time (up to 15 minutes): a slow provider's long reasoning answer
    // (MiMo V2.6 Pro at the official ~30 tok/s) takes over 3 minutes and was cut off at 180 s on every try
    let mut limit = CALL_TIMEOUT;
    for wait in [2, 8, 30, 60, 120, 120, 120, 120, 120, 120, 120, 0] {
        let sent = http.post(format!("{url}/{path}")).config().timeout_global(Some(limit)).build()
            .header("Authorization", format!("Bearer {key}")).header("Content-Type", "application/json").send(&body)
            .and_then(|mut r| Ok((r.status().as_u16(), r.body_mut().with_config().limit(50 << 20).read_to_string()?)));
        let (status, text) = match sent {
            Ok(st) => st,
            Err(e) if wait > 0 => {
                if matches!(e, ureq::Error::Timeout(_)) {
                    limit = (limit * 2).min(Duration::from_secs(900));
                }
                eprintln!("model call failed ({e}); retrying in {wait}s");
                std::thread::sleep(Duration::from_secs(wait));
                continue;
            }
            Err(e) => return Err(e.into()),
        };
        if status == 200 {
            return serde_json::from_str(&text).context("provider reply is not JSON");
        }
        // a 429 for a locked model or an empty balance never clears by waiting (Experiential Labs: "insufficient_quota")
        let permanent = text.contains("insufficient_quota") || text.contains("requires_purchase");
        if wait == 0 || permanent || !(status == 429 || status >= 500) {
            bail!("HTTP {status}: {}", text.chars().take(300).collect::<String>());
        }
        std::thread::sleep(Duration::from_secs(wait));
    }
    unreachable!()
}

/// The model's prices from the provider's model list (per million tokens there, per token here), and whether it is an
/// OpenAI model (Responses API).
fn price(http: &ureq::Agent, url: &str, key: &str, model: &str) -> Result<(Price, bool)> {
    let mut r = http.get(format!("{url}/models")).header("Authorization", format!("Bearer {key}")).call()?;
    let list: Value = serde_json::from_str(&r.body_mut().with_config().limit(50 << 20).read_to_string()?)?;
    let m = list["data"].as_array().into_iter().flatten().find(|m| m["id"] == model).with_context(|| format!("{model} is not in {url}/models"))?;
    parse_price(m)
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
    let openai = m["owned_by"] == "OpenAI" || m["provider"] == "OpenAI"
        || m["data_policy"]["providers"].as_array().is_some_and(|ps| !ps.is_empty() && ps.iter().all(|p| p["provider"] == "openai"));
    Ok(((input / 1e6, cached / 1e6, output / 1e6), openai))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    #[test]
    fn runs_tools_until_the_model_answers_and_counts_cost() {
        let dir = std::env::temp_dir().join(format!("tazuna-engine-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), "hello\n").unwrap();
        let replies = Rc::new(RefCell::new(VecDeque::from([
            json!({"choices": [{"message": {"content": null, "tool_calls": [{"id": "c1", "type": "function", "function": {"name": "read", "arguments": "{\"path\":\"a.txt\"}"}}]}}],
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
            responses: false,
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
            responses: false,
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
            responses: false,
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
