//! `tazuna mcp`: the lean tool server (stdio MCP): bash/read/write/edit with terse schemas.
//!
//! A port of the Node server we benchmarked as cc-mid. Tool names, descriptions and schemas are byte-identical, so
//! claude sees exactly the same tools. Differences: no Node, and a timed-out command takes its whole process group down
//! (the Node version left the command's children running).

use serde_json::{Value, json};
use std::{
    io::{BufRead, Read, Write},
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const CWD_MARK: &str = "\n__LEAN_CWD__";
const BASH_TIMEOUT: Duration = Duration::from_secs(120);

pub fn serve() -> anyhow::Result<()> {
    let mut server = Server { cwd: std::env::current_dir()?, max: max_output(), cap: bash_cap(), saved: 0, out_dir: None };
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(reply) = server.handle(&line) {
            writeln!(out, "{reply}")?;
            out.flush()?;
        }
    }
    Ok(())
}

fn max_output() -> usize {
    // hard limit for any tool reply (chars), carried over from the Node server
    std::env::var("LEAN_MAX_OUTPUT").ok().and_then(|v| v.parse().ok()).unwrap_or(30000)
}

/// Bash output longer than this (chars, ~4 per token) is shrunk; 0 turns shrinking off.
pub const DEFAULT_BASH_CAP: usize = 8000;
/// Of the kept text, this much comes from the start (compile errors); the rest from the end (failures, summaries).
const HEAD_CHARS: usize = 2500;

fn bash_cap() -> usize {
    std::env::var("LEAN_BASH_CAP").ok().and_then(|v| v.parse().ok()).unwrap_or(DEFAULT_BASH_CAP)
}

struct Server {
    /// Persists between bash calls (`cd` sticks); relative paths resolve against it.
    cwd: PathBuf,
    max: usize,
    /// Shrink bash output above this many chars (0: never).
    cap: usize,
    /// Full outputs saved so far (file names).
    saved: usize,
    /// Private directory for saved outputs, created on first use.
    out_dir: Option<PathBuf>,
}

impl Server {
    /// One JSON-RPC message in, at most one reply out (notifications get none).
    fn handle(&mut self, line: &str) -> Option<String> {
        let msg: Value = serde_json::from_str(line).ok()?;
        let id = msg.get("id").cloned();
        let method = msg["method"].as_str().unwrap_or_default();
        let result: Result<Value, (i64, String)> = match method {
            "initialize" => Ok(json!({
                "protocolVersion": msg["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "lean", "version": "0.1.0"},
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({"tools": tools()})),
            "tools/call" => self.call(&msg["params"]),
            _ if id.is_none() => return None,
            _ => Err((-32601, format!("method not found: {method}"))),
        };
        let id = id?;
        Some(match result {
            Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
            Err((code, message)) => json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}),
        }.to_string())
    }

    fn call(&mut self, params: &Value) -> Result<Value, (i64, String)> {
        let name = params["name"].as_str().unwrap_or_default();
        let a = &params["arguments"];
        let path = a["path"].as_str().map(|p| self.resolve(p));
        let s = |k: &str| a[k].as_str().unwrap_or_default();
        let out: Result<String, String> = match name {
            "bash" => Ok(self.bash(s("command"), BASH_TIMEOUT)),
            "read" => read(&path.unwrap_or_default(), a["offset"].as_u64().unwrap_or(1), a["limit"].as_u64()).map(|t| cap(&t, self.max)),
            "write" => write(&path.unwrap_or_default(), s("content")),
            "edit" => edit(&path.unwrap_or_default(), s("old"), s("new")),
            _ => return Err((-32000, format!("unknown tool {name}"))),
        };
        Ok(match out {
            Ok(text) => json!({"content": [{"type": "text", "text": text}]}),
            Err(e) => json!({"content": [{"type": "text", "text": e}], "isError": true}),
        })
    }

    fn resolve(&self, p: &str) -> PathBuf {
        let p = Path::new(p);
        if p.is_absolute() { p.to_path_buf() } else { self.cwd.join(p) }
    }

    fn bash(&mut self, command: &str, timeout: Duration) -> String {
        let script = format!("{command}\n__rc=$?; printf '\\n__LEAN_CWD__%s' \"$PWD\" >&2; exit $__rc");
        let spawned = Command::new("bash")
            .args(["-lc", &script])
            .current_dir(&self.cwd)
            // commands the agent runs must not see the auth token claude was started with
            .env_remove("CLAUDE_CODE_OAUTH_TOKEN")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0) // own group, so a timeout can kill everything the command started
            .spawn();
        let mut child = match spawned {
            Ok(c) => c,
            Err(e) => return cap(&format!("\n[{e}]"), self.max),
        };
        let (mut so, mut se) = (child.stdout.take().expect("piped"), child.stderr.take().expect("piped"));
        let t_out = std::thread::spawn(move || read_all(&mut so));
        let t_err = std::thread::spawn(move || read_all(&mut se));
        let t0 = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(st)) => break Some(st),
                Ok(None) if t0.elapsed() >= timeout => {
                    let _ = Command::new("kill").args(["-KILL", "--", &format!("-{}", child.id())]).status();
                    let _ = child.wait();
                    break None;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(20)),
                Err(_) => break None,
            }
        };
        let stdout = t_out.join().unwrap_or_default();
        let mut stderr = t_err.join().unwrap_or_default();
        if let Some(i) = stderr.rfind(CWD_MARK) {
            self.cwd = PathBuf::from(&stderr[i + CWD_MARK.len()..]);
            stderr.truncate(i);
        }
        let tail = match status {
            Some(st) => st.code().filter(|&c| c != 0).map(|c| format!("\n[exit {c}]")).unwrap_or_default(),
            None => format!("\n[timed out after {}s; the command and everything it started were killed]", timeout.as_secs()),
        };
        let full = format!("{stdout}{stderr}{tail}");
        let shrunk = self.shrink(&full);
        cap(&shrunk, self.max)
    }

    /// Long output: drop passing-test chatter, then keep the start and the end. The full text is saved outside the repo
    /// so the agent can still read any part of it.
    fn shrink(&mut self, full: &str) -> String {
        if self.cap == 0 || full.chars().count() <= self.cap {
            return full.to_string();
        }
        let total_lines = full.lines().count();
        let quiet: String = full.lines().filter(|l| !crate::outcap::is_test_noise(l)).collect::<Vec<_>>().join("\n");
        let dropped = total_lines - quiet.lines().count();
        let file = self.save(full);
        let where_ = file.as_deref().map_or("not saved".into(), |f| format!("full output: {f}"));
        let note = |omitted: usize| {
            let noise = if dropped > 0 { format!("{dropped} lines of passing-test output removed, ") } else { String::new() };
            format!("[tazuna: {noise}{omitted} more lines omitted; {where_} (read it with offset/limit or grep it)]")
        };
        match crate::outcap::head_tail(&quiet, self.cap, HEAD_CHARS, note) {
            Some(kept) => kept,
            None if dropped > 0 => format!("{quiet}\n[tazuna: {dropped} lines of passing-test output removed; {where_}]"),
            None => quiet,
        }
    }

    /// Saves into a private directory (0700, unpredictable name) with new 0600 files: on a shared /tmp nobody else can read
    /// the output or plant a symlink for it to be written through.
    fn save(&mut self, full: &str) -> Option<String> {
        use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
        if self.out_dir.is_none() {
            let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.subsec_nanos();
            let dir = std::env::temp_dir().join(format!("tazuna-output-{}-{nanos:x}", std::process::id()));
            std::fs::DirBuilder::new().mode(0o700).create(&dir).ok()?; // fails if it already exists
            self.out_dir = Some(dir);
        }
        self.saved += 1;
        let f = self.out_dir.as_ref()?.join(format!("bash-{}.txt", self.saved));
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&f).ok()?;
        file.write_all(full.as_bytes()).ok()?;
        Some(f.to_string_lossy().into_owned())
    }
}

fn read_all(r: &mut impl Read) -> String {
    let mut buf = Vec::new();
    let _ = r.read_to_end(&mut buf);
    String::from_utf8_lossy(&buf).into_owned()
}

fn tools() -> Value {
    let (s, i) = (json!({"type": "string"}), json!({"type": "integer"}));
    json!([
        {"name": "bash", "description": "Run shell command. Working directory persists between calls.",
         "inputSchema": {"type": "object", "properties": {"command": s}, "required": ["command"]}},
        {"name": "read", "description": "Read file. Optional 1-based line offset/limit.",
         "inputSchema": {"type": "object", "properties": {"path": s, "offset": i, "limit": i}, "required": ["path"]}},
        {"name": "write", "description": "Write file, creating dirs.",
         "inputSchema": {"type": "object", "properties": {"path": s, "content": s}, "required": ["path", "content"]}},
        {"name": "edit", "description": "Replace exact unique text in file.",
         "inputSchema": {"type": "object", "properties": {"path": s, "old": s, "new": s}, "required": ["path", "old", "new"]}},
    ])
}

fn io_err(path: &Path, e: std::io::Error) -> String {
    format!("{}: {e}", path.display())
}

fn read(path: &Path, offset: u64, limit: Option<u64>) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| io_err(path, e))?;
    let text = String::from_utf8_lossy(&bytes);
    let start = offset.max(1) as usize - 1;
    let lines = text.split('\n').skip(start);
    Ok(match limit {
        Some(n) => lines.take(n as usize).collect::<Vec<_>>().join("\n"),
        None => lines.collect::<Vec<_>>().join("\n"),
    })
}

fn write(path: &Path, content: &str) -> Result<String, String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| io_err(dir, e))?;
    }
    std::fs::write(path, content).map_err(|e| io_err(path, e))?;
    Ok("ok".into())
}

fn edit(path: &Path, old: &str, new: &str) -> Result<String, String> {
    let text = std::fs::read_to_string(path).map_err(|e| io_err(path, e))?;
    let n = if old.is_empty() { 0 } else { text.matches(old).count() };
    if n != 1 {
        return Err(format!("old text found {n} times, need exactly 1"));
    }
    std::fs::write(path, text.replacen(old, new, 1)).map_err(|e| io_err(path, e))?;
    Ok("ok".into())
}

/// Hard cap in characters, same marker as the Node server.
fn cap(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max).collect();
    format!("{cut}\n[truncated {} chars]", n - max)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Replays the requests captured from the Node server and expects the same replies.
    #[test]
    fn matches_the_node_server() {
        let dir = std::env::temp_dir().join(format!("tazuna-mcp-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("f.txt"), "line1\nline2\nline3\n").unwrap();
        let mut s = Server { cwd: dir.canonicalize().unwrap(), max: 30000, cap: DEFAULT_BASH_CAP, saved: 0, out_dir: None };
        let ask = |s: &mut Server, req: &str| s.handle(req);
        assert_eq!(
            ask(&mut s, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#).unwrap(),
            r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18","capabilities":{"tools":{}},"serverInfo":{"name":"lean","version":"0.1.0"}}}"#
        );
        assert_eq!(ask(&mut s, r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#), None);
        assert_eq!(
            ask(&mut s, r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#).unwrap(),
            r#"{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"bash","description":"Run shell command. Working directory persists between calls.","inputSchema":{"type":"object","properties":{"command":{"type":"string"}},"required":["command"]}},{"name":"read","description":"Read file. Optional 1-based line offset/limit.","inputSchema":{"type":"object","properties":{"path":{"type":"string"},"offset":{"type":"integer"},"limit":{"type":"integer"}},"required":["path"]}},{"name":"write","description":"Write file, creating dirs.","inputSchema":{"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"}},"required":["path","content"]}},{"name":"edit","description":"Replace exact unique text in file.","inputSchema":{"type":"object","properties":{"path":{"type":"string"},"old":{"type":"string"},"new":{"type":"string"}},"required":["path","old","new"]}}]}}"#
        );
        assert_eq!(
            ask(&mut s, r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"read","arguments":{"path":"f.txt","offset":2,"limit":1}}}"#).unwrap(),
            r#"{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"line2"}]}}"#
        );
        assert_eq!(
            ask(&mut s, r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"bash","arguments":{"command":"mkdir -p sub && cd sub && echo hi; echo err >&2; false"}}}"#).unwrap(),
            r#"{"jsonrpc":"2.0","id":4,"result":{"content":[{"type":"text","text":"hi\nerr\n\n[exit 1]"}]}}"#
        );
        let pwd = ask(&mut s, r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"bash","arguments":{"command":"pwd"}}}"#).unwrap();
        assert!(pwd.contains(r#"/sub\n""#), "cd persists between calls: {pwd}");
        assert_eq!(
            ask(&mut s, r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"edit","arguments":{"path":"../f.txt","old":"line","new":"x"}}}"#).unwrap(),
            r#"{"jsonrpc":"2.0","id":6,"result":{"content":[{"type":"text","text":"old text found 3 times, need exactly 1"}],"isError":true}}"#
        );
        assert_eq!(
            ask(&mut s, r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"nope","arguments":{}}}"#).unwrap(),
            r#"{"jsonrpc":"2.0","id":7,"error":{"code":-32000,"message":"unknown tool nope"}}"#
        );
        assert_eq!(
            ask(&mut s, r#"{"jsonrpc":"2.0","id":8,"method":"bogus"}"#).unwrap(),
            r#"{"jsonrpc":"2.0","id":8,"error":{"code":-32601,"message":"method not found: bogus"}}"#
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn edit_write_and_cap() {
        let dir = std::env::temp_dir().join(format!("tazuna-mcp-edit-{}", std::process::id()));
        let f = dir.join("a/b.txt");
        assert_eq!(write(&f, "one two").unwrap(), "ok");
        assert_eq!(edit(&f, "two", "2").unwrap(), "ok");
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "one 2");
        assert!(edit(&f, "missing", "x").unwrap_err().contains("found 0 times"));
        assert_eq!(cap("abcdef", 4), "abcd\n[truncated 2 chars]");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn shrinks_long_output_and_saves_it() {
        let mut s = Server { cwd: std::env::temp_dir(), max: 30000, cap: 2000, saved: 0, out_dir: None };
        let out = s.bash("for i in $(seq 1 400); do echo \"=== RUN   TestN$i\"; echo \"--- PASS: TestN$i\"; done; echo '--- FAIL: TestBroken'; echo '    x_test.go:3: boom'; seq 1 500; exit 1", Duration::from_secs(30));
        assert!(out.chars().count() < 2600, "shrunk to about the cap: {}", out.chars().count());
        assert!(out.contains("--- FAIL: TestBroken") || out.contains("800 lines of passing-test output removed"), "{out}");
        assert!(out.ends_with("500\n[exit 1]") || out.contains("[exit 1]"), "the end survives: {out}");
        let file = out.split("full output: ").nth(1).and_then(|r| r.split(' ').next()).map(|f| f.trim_end_matches(']')).expect("points to the saved file");
        let saved = std::fs::read_to_string(file).unwrap();
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(file).unwrap().permissions().mode() & 0o777, 0o600, "owner-only file");
        let dir = Path::new(file).parent().unwrap();
        assert_eq!(std::fs::metadata(dir).unwrap().permissions().mode() & 0o777, 0o700, "owner-only directory");
        assert!(saved.contains("=== RUN   TestN1\n") && saved.contains("TestBroken"), "the full output is kept");
        let mut off = Server { cwd: std::env::temp_dir(), max: 30000, cap: 0, saved: 0, out_dir: None };
        assert_eq!(off.bash("seq 1 3000 | wc -l", Duration::from_secs(10)).trim(), "3000");
        assert!(off.bash("seq 1 3000", Duration::from_secs(10)).chars().count() > 13000, "cap 0 leaves output alone");
    }

    #[test]
    fn timeout_kills_the_whole_command() {
        let mut s = Server { cwd: std::env::temp_dir(), max: 30000, cap: DEFAULT_BASH_CAP, saved: 0, out_dir: None };
        let t0 = Instant::now();
        // the background sleep would outlive a plain kill of bash
        let out = s.bash("sleep 30 & echo started; wait", Duration::from_millis(500));
        assert!(t0.elapsed() < Duration::from_secs(5), "returned at the timeout, not after the child");
        assert!(out.starts_with("started"), "{out}");
        assert!(out.contains("timed out"), "{out}");
    }
}
