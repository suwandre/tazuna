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

/// A backstop against loops: the dollar cap is the real limit, so cheap models get proportionally more turns.
const MAX_TURNS: usize = 200;
const MAX_USD: f64 = 0.50;

/// USD per token: input, cached input, output.
type Price = (f64, f64, f64);

pub fn serve(args: &[String]) -> Result<()> {
    let flag = |name: &str| args.windows(2).find(|w| w[0] == name).map(|w| w[1].clone());
    let url = flag("--url").context("--url")?.trim_end_matches('/').to_string();
    let key_env = flag("--key-env").context("--key-env")?;
    let model = flag("--model").context("--model")?;
    let effort = flag("--effort");
    let system = flag("--system-prompt").unwrap_or_default();
    let key = std::env::var(&key_env).with_context(|| format!("{key_env} not set"))?;
    // SAFETY: no other thread runs yet. The agent's commands inherit this environment and must not see the key.
    unsafe { std::env::remove_var(&key_env) };
    let http: ureq::Agent = ureq::Agent::config_builder()
        // the system's TLS (macOS Security framework): ~1 MB less binary than rustls
        .tls_config(ureq::tls::TlsConfig::builder().provider(ureq::tls::TlsProvider::NativeTls).build())
        // a normal call answers in 15-60 s, a long reasoning one in a few minutes
        .timeout_global(Some(Duration::from_secs(300))).http_status_as_error(false).build().into();
    let (price, responses) = price(&http, &url, &key, &model)?;

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
        call: Box::new(move |body: &Value| chat(&http, &url, &key, body)),
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
        queued: VecDeque::new(),
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
            loop_.run(&msg["message"]["content"], &rx);
        }
        // control requests while idle (interrupt, permission mode): nothing to do; the auto mode never asks
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
    /// User messages that arrived during a run.
    queued: VecDeque<Value>,
    out: Box<dyn FnMut(&Value)>,
}

impl Loop {
    /// One user message: model calls and tool runs until the model answers without tools, or a cap or interrupt ends it.
    fn run(&mut self, content: &Value, rx: &Receiver<Value>) {
        self.msgs.push(json!({"role": "user", "content": text_of(content)}));
        let (start, mut turns, mut usage) = (self.cost, 0, [0u64; 3]);
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
                    Ok(_) => {}
                    Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
                }
            }
            if turns >= MAX_TURNS {
                return end(self, "error_max_turns", format!("[stopped: {MAX_TURNS} turns]"), turns, usage);
            }
            if self.cost - start >= MAX_USD {
                return end(self, "error_max_budget_usd", format!("[stopped: ${MAX_USD:.2} spent]"), turns, usage);
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
                    Some(input) => match self.server.call(&json!({"name": name, "arguments": input})) {
                        Ok(r) => (r["content"][0]["text"].as_str().unwrap_or_default().to_string(), r["isError"] == true),
                        Err((_, e)) => (e, true),
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
/// connections get three more tries: two of 18 benchmark runs hung on a call that never answered.
fn chat(http: &ureq::Agent, url: &str, key: &str, body: &Value) -> Result<Value> {
    let path = if body.get("input").is_some() { "responses" } else { "chat/completions" };
    let body = body.to_string();
    for wait in [2, 8, 30, 0] {
        let sent = http.post(format!("{url}/{path}")).header("Authorization", format!("Bearer {key}")).header("Content-Type", "application/json").send(&body)
            .and_then(|mut r| Ok((r.status().as_u16(), r.body_mut().with_config().limit(50 << 20).read_to_string()?)));
        let (status, text) = match sent {
            Ok(st) => st,
            Err(e) if wait > 0 => {
                eprintln!("model call failed ({e}); retrying in {wait}s");
                std::thread::sleep(Duration::from_secs(wait));
                continue;
            }
            Err(e) => return Err(e.into()),
        };
        if status == 200 {
            return serde_json::from_str(&text).context("provider reply is not JSON");
        }
        if wait == 0 || !(status == 429 || status >= 500) {
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
    let p = |k: &str| m["pricing"][k].as_str().and_then(|s| s.parse::<f64>().ok()).or(m["pricing"][k].as_f64());
    let input = p("input_per_million").with_context(|| format!("no input price for {model}: the dollar cap needs one"))?;
    let price = (input / 1e6, p("cache_read_input_per_million").unwrap_or(input) / 1e6, p("output_per_million").context("no output price")? / 1e6);
    Ok((price, m["owned_by"] == "OpenAI" || m["provider"] == "OpenAI"))
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
            queued: VecDeque::new(),
            out: Box::new(move |e: &Value| ev.borrow_mut().push(e.clone())),
        };
        let (_tx, rx) = std::sync::mpsc::channel();
        l.run(&json!("what is in a.txt?"), &rx);
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
}
