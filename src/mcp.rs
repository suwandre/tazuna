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
/// The longest `timeout` (seconds) a bash call may ask for (0.0.88): at a fixed 120 s the model ran long test runs in the
/// background and polled them with `sleep 115; tail`, one turn each (33-47 a prometheus run, 2026-10-25).
const MAX_TIMEOUT: u64 = 1800;

pub fn serve() -> anyhow::Result<()> {
    let mut server = Server::new(std::env::current_dir()?);
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

pub(crate) struct Server {
    /// Persists between bash calls (`cd` sticks); relative paths resolve against it.
    cwd: PathBuf,
    max: usize,
    /// Shrink bash output above this many chars (0: never).
    cap: usize,
    /// Full outputs saved so far (file names).
    saved: usize,
    /// Private directory for saved outputs, created on first use.
    out_dir: Option<PathBuf>,
    /// Own loop only: reads already returned, by (file, offset, limit) -> (hash of the text, read number). The own loop
    /// keeps every earlier result in the conversation, so a repeat of an unchanged read can point back to it instead of
    /// sending the text again. (Claude Code may compact the conversation, so the claude path does not use this.)
    seen: Option<std::collections::HashMap<(PathBuf, u64, u64), (u64, usize)>>,
    reads: usize,
    /// Processes a command left running in the background (`server &`): (pid, start time). Killed when the server
    /// ends; also kept in a file, so the next tazuna start can kill them if this one was killed outright.
    left: Vec<(u32, String)>,
    /// This server's file for `left` (one engine can run several servers).
    left_at: PathBuf,
}

static SERVERS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

impl Drop for Server {
    fn drop(&mut self) {
        kill_matching(&self.left);
        let _ = std::fs::remove_file(&self.left_at);
    }
}

/// `pid lstart` for every process, to tell a live process from a later one that got the same pid.
fn processes() -> Vec<(u32, u32, String)> {
    let out = Command::new("ps").args(["-axo", "pid=,pgid=,lstart="]).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    out.lines().filter_map(|l| {
        let mut f = l.split_whitespace();
        Some((f.next()?.parse().ok()?, f.next()?.parse().ok()?, f.collect::<Vec<_>>().join(" ")))
    }).collect()
}

fn kill_matching(left: &[(u32, String)]) {
    for (pid, _, start) in processes() {
        if left.iter().any(|(p, s)| *p == pid && *s == start) {
            let _ = Command::new("kill").args(["-KILL", &pid.to_string()]).status();
        }
    }
}

/// Owner-only, under the user's own data directory: never a shared /tmp, where another user could plant a list or a
/// symlink.
fn left_dir() -> PathBuf {
    let d = crate::labels::dir().unwrap_or_else(|_| std::env::temp_dir()).join("left");
    if std::fs::create_dir_all(&d).is_ok() {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o700));
    }
    d
}

fn left_file(owner: u32) -> PathBuf {
    let n = SERVERS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    left_dir().join(format!("{owner}-{n}.txt"))
}

/// Kills what an earlier tazuna, killed before it could clean up, left running (its file names a dead pid).
pub(crate) fn reap_left() {
    let Ok(dir) = std::fs::read_dir(left_dir()) else { return };
    let alive: Vec<u32> = processes().iter().map(|p| p.0).collect();
    for f in dir.flatten() {
        let name = f.file_name().to_string_lossy().into_owned();
        // only plain files: a symlink is never followed
        if !f.path().symlink_metadata().is_ok_and(|m| m.is_file()) {
            continue;
        }
        let Some(owner) = name.split('-').next().and_then(|n| n.parse::<u32>().ok()) else { continue };
        if alive.contains(&owner) {
            continue;
        }
        let left: Vec<(u32, String)> = std::fs::read_to_string(f.path()).unwrap_or_default().lines()
            .filter_map(|l| l.split_once(' ').and_then(|(p, s)| Some((p.parse().ok()?, s.to_string())))).collect();
        kill_matching(&left);
        let _ = std::fs::remove_file(f.path());
    }
}

/// `read` without a limit returns this many lines (or as many as fit the output cap) and says how to get the rest. 250
/// had DeepSeek reading files in slices, mostly through `cat`/`sed` cut at the 8K bash cap; OpenCode reads up to 2000 lines
/// and its runs read 12-24K-character files in one call (2026-10-24).
const READ_WINDOW: u64 = 2000;

impl Server {
    pub(crate) fn new(cwd: PathBuf) -> Self {
        Server { cwd, max: max_output(), cap: bash_cap(), saved: 0, out_dir: None, seen: None, reads: 0, left: vec![], left_at: left_file(std::process::id()) }
    }

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

    pub(crate) fn call(&mut self, params: &Value) -> Result<Value, (i64, String)> {
        let name = params["name"].as_str().unwrap_or_default();
        let a = &params["arguments"];
        let path = a["path"].as_str().map(|p| self.resolve(p));
        let s = |k: &str| a[k].as_str().unwrap_or_default();
        let out: Result<String, String> = match name {
            "bash" => {
                // seconds, a number or a numeric string (some models quote it)
                let t = a["timeout"].as_u64().or_else(|| a["timeout"].as_str()?.trim().parse().ok());
                Ok(self.bash(s("command"), t.map_or(BASH_TIMEOUT, |t| Duration::from_secs(t.clamp(1, MAX_TIMEOUT)))))
            }
            "read" => self.read(path.unwrap_or_default(), a["offset"].as_u64().unwrap_or(1), a["limit"].as_u64()),
            "write" => write(&path.unwrap_or_default(), s("content")),
            "edit" => edit(&path.unwrap_or_default(), s("old"), s("new")),
            _ => return Err((-32000, format!("unknown tool {name}"))),
        };
        Ok(match out {
            Ok(text) => json!({"content": [{"type": "text", "text": text}]}),
            Err(e) => json!({"content": [{"type": "text", "text": e}], "isError": true}),
        })
    }

    pub(crate) fn with_read_dedupe(mut self) -> Self {
        self.seen = Some(Default::default());
        self
    }

    fn read(&mut self, path: PathBuf, offset: u64, limit: Option<u64>) -> Result<String, String> {
        let text = cap(&read(&path, offset, Some(limit.unwrap_or(READ_WINDOW)), self.max)?, self.max);
        self.reads += 1;
        if let Some(seen) = self.seen.as_mut() {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            text.hash(&mut h);
            let key = (path, offset, limit.unwrap_or(READ_WINDOW));
            match seen.get(&key) {
                Some(&(hash, n)) if hash == h.finish() => return Ok(format!("[unchanged since read #{n} above: same lines, same content]")),
                _ => {
                    seen.insert(key, (h.finish(), self.reads));
                }
            }
        }
        // the number lets a later repeat point back to this result
        Ok(if self.seen.is_some() { format!("[read #{}]\n{text}", self.reads) } else { text })
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
        let (b_out, b_err) = (std::sync::Arc::new(std::sync::Mutex::new(Vec::new())), std::sync::Arc::new(std::sync::Mutex::new(Vec::new())));
        let t_out = { let b = b_out.clone(); std::thread::spawn(move || read_into(&mut so, &b)) };
        let t_err = { let b = b_err.clone(); std::thread::spawn(move || read_into(&mut se, &b)) };
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
        if status.is_some() {
            // the command finished; anything still in its process group was started in the background
            let group = child.id();
            let before = self.left.len();
            self.left.extend(processes().into_iter().filter(|p| p.1 == group).map(|p| (p.0, p.2)));
            if self.left.len() > before {
                let lines: String = self.left.iter().map(|(p, s)| format!("{p} {s}\n")).collect();
                let _ = std::fs::write(&self.left_at, lines);
            }
        }
        // a background process started without redirecting its output keeps the pipes open: do not wait for it
        let t1 = Instant::now();
        while !(t_out.is_finished() && t_err.is_finished()) && t1.elapsed() < Duration::from_secs(2) {
            std::thread::sleep(Duration::from_millis(20));
        }
        let held = !(t_out.is_finished() && t_err.is_finished());
        let text = |b: &std::sync::Arc<std::sync::Mutex<Vec<u8>>>| String::from_utf8_lossy(&b.lock().map(|v| v.clone()).unwrap_or_default()).into_owned();
        let stdout = text(&b_out);
        let mut stderr = text(&b_err);
        if let Some(i) = stderr.rfind(CWD_MARK) {
            self.cwd = PathBuf::from(&stderr[i + CWD_MARK.len()..]);
            stderr.truncate(i);
        }
        if held {
            stderr.push_str("\n[a background process still holds this output; redirect it, e.g. `cmd > log 2>&1 &`]");
        }
        let tail = match status {
            Some(st) => st.code().filter(|&c| c != 0).map(|c| format!("\n[exit {c}]")).unwrap_or_default(),
            None => format!("\n[timed out after {}s; the command and everything it started were killed; give a larger timeout \
                             (up to {MAX_TIMEOUT} s) for a long run]", timeout.as_secs()),
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

fn read_into(r: &mut impl Read, buf: &std::sync::Mutex<Vec<u8>>) {
    let mut chunk = [0u8; 8192];
    while let Ok(n) = r.read(&mut chunk) {
        if n == 0 {
            break;
        }
        if let Ok(mut b) = buf.lock() {
            b.extend_from_slice(&chunk[..n]);
        }
    }
}

pub(crate) fn tools() -> Value {
    let (s, i) = (json!({"type": "string"}), json!({"type": "integer"}));
    json!([
        {"name": "bash", "description": "Run shell command. Working directory persists between calls. It stops after 120 s unless you give a timeout in seconds (up to 1800): give one for long builds and test runs rather than running them in the background.",
         "inputSchema": {"type": "object", "properties": {"command": s, "timeout": i}, "required": ["command"]}},
        {"name": "read", "description": "Read file: the whole file (up to 2000 lines) unless you give a 1-based offset or a limit. Use it rather than cat, head or sed.",
         "inputSchema": {"type": "object", "properties": {"path": s, "offset": i, "limit": i}, "required": ["path"]}},
        {"name": "write", "description": "Write file, creating dirs.",
         "inputSchema": {"type": "object", "properties": {"path": s, "content": s}, "required": ["path", "content"]}},
        {"name": "edit", "description": "Replace exact unique text in file. Use it for every change to an existing file, rather than sed or scripts.",
         "inputSchema": {"type": "object", "properties": {"path": s, "old": s, "new": s}, "required": ["path", "old", "new"]}},
    ])
}

fn io_err(path: &Path, e: std::io::Error) -> String {
    format!("{}: {e}", path.display())
}

/// Lines `offset..offset+limit`, stopping early at whole lines when `max` characters are reached: the note at the end says
/// where to go on, which a plain cut would not.
fn read(path: &Path, offset: u64, limit: Option<u64>, max: usize) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| io_err(path, e))?;
    let text = String::from_utf8_lossy(&bytes);
    let mut all: Vec<&str> = text.split('\n').collect();
    if text.ends_with('\n') {
        all.pop(); // the final newline ends the last line; it does not start another
    }
    let start = offset.max(1) as usize - 1;
    let mut end = limit.map_or(all.len(), |n| (start + n as usize).min(all.len()));
    let mut chars = 0;
    for (i, l) in all.iter().enumerate().take(end).skip(start) {
        chars += l.chars().count() + 1;
        if chars > max.saturating_sub(100) && i > start {
            end = i;
            break;
        }
    }
    let mut out = all.get(start..end.max(start)).unwrap_or_default().join("\n");
    if end < all.len() {
        out.push_str(&format!("\n[lines {}-{end} of {}; read with offset={} for more]", start + 1, all.len(), end + 1));
    }
    Ok(out)
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
    if n > 1 {
        return Err(format!("old text found {n} times, need exactly 1: include more of the surrounding lines"));
    }
    if old.is_empty() {
        return Err("`old` is missing: give the exact text to replace".into());
    }
    if n == 0 {
        // where the first line of `old` is, if anywhere: a failed edit was most often whitespace or a stale copy, and
        // DeepSeek then fell back to python/sed edits in bash (up to 17 failed edits a run, 2026-10-24)
        let first = old.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or_default();
        let lines: Vec<&str> = text.lines().collect();
        return Err(match lines.iter().position(|l| !first.is_empty() && l.contains(first)) {
            Some(i) => format!("old text not found. Its first line is at line {}; the file there now reads:\n{}", i + 1,
                lines[i.saturating_sub(2)..(i + 8).min(lines.len())].join("\n")),
            None => "old text not found, nor its first line: read the file again, it may have changed".into(),
        });
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

    #[test]
    fn background_processes_neither_block_nor_outlive_the_server() {
        let mut s = Server::new(std::env::temp_dir());
        let t0 = Instant::now();
        let out = s.bash("sleep 300 & echo started", BASH_TIMEOUT);
        assert!(t0.elapsed() < Duration::from_secs(10), "returned without waiting for the background sleep");
        assert!(out.contains("started") && out.contains("still holds this output"), "{out}");
        assert_eq!(s.left.len(), 1, "the sleep is remembered");
        let pid = s.left[0].0;
        let file = s.left_at.clone();
        assert!(file.exists());
        drop(s);
        std::thread::sleep(Duration::from_millis(300));
        assert!(!processes().iter().any(|p| p.0 == pid), "killed when the server ended");
        assert!(!file.exists());
    }

    #[test]
    fn reads_a_window_and_points_back_to_unchanged_repeats() {
        let dir = std::env::temp_dir().join(format!("tazuna-read-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let body: String = (1..=2300).map(|i| format!("line {i}\n")).collect();
        std::fs::write(dir.join("f.txt"), &body).unwrap();
        let call = |s: &mut Server, a: Value| s.call(&json!({"name": "read", "arguments": a})).unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        let mut lean = Server::new(dir.clone());
        let t = call(&mut lean, json!({"path": "f.txt"}));
        assert!(t.starts_with("line 1\n") && t.contains("line 2000\n[lines 1-2000 of 2300; read with offset=2001 for more]"), "{t}");
        assert!(!t.contains("line 2001"));
        // the output cap ends a read at a whole line, with the offset to go on from
        let short = read(&dir.join("f.txt"), 1, None, 200).unwrap();
        assert!(short.ends_with("line 13\n[lines 1-13 of 2300; read with offset=14 for more]"), "{short}");
        assert_eq!(call(&mut lean, json!({"path": "f.txt"})), t, "the claude path never shortens a repeat");
        let mut own = Server::new(dir.clone()).with_read_dedupe();
        assert!(call(&mut own, json!({"path": "f.txt", "offset": 290})).starts_with("[read #1]\nline 290"));
        assert_eq!(call(&mut own, json!({"path": "f.txt", "offset": 290})), "[unchanged since read #1 above: same lines, same content]");
        std::fs::write(dir.join("f.txt"), body.replace("line 295", "changed")).unwrap();
        assert!(call(&mut own, json!({"path": "f.txt", "offset": 290})).contains("changed"), "a changed file is sent again");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Replays the requests captured from the Node server and expects the same replies.
    #[test]
    fn matches_the_node_server() {
        let dir = std::env::temp_dir().join(format!("tazuna-mcp-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("f.txt"), "line1\nline2\nline3\n").unwrap();
        let mut s = Server { cwd: dir.canonicalize().unwrap(), max: 30000, cap: DEFAULT_BASH_CAP, saved: 0, out_dir: None, seen: None, reads: 0, left: vec![], left_at: left_file(std::process::id()) };
        let ask = |s: &mut Server, req: &str| s.handle(req);
        assert_eq!(
            ask(&mut s, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#).unwrap(),
            r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18","capabilities":{"tools":{}},"serverInfo":{"name":"lean","version":"0.1.0"}}}"#
        );
        assert_eq!(ask(&mut s, r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#), None);
        assert_eq!(
            ask(&mut s, r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#).unwrap(),
            r#"{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"bash","description":"Run shell command. Working directory persists between calls. It stops after 120 s unless you give a timeout in seconds (up to 1800): give one for long builds and test runs rather than running them in the background.","inputSchema":{"type":"object","properties":{"command":{"type":"string"},"timeout":{"type":"integer"}},"required":["command"]}},{"name":"read","description":"Read file: the whole file (up to 2000 lines) unless you give a 1-based offset or a limit. Use it rather than cat, head or sed.","inputSchema":{"type":"object","properties":{"path":{"type":"string"},"offset":{"type":"integer"},"limit":{"type":"integer"}},"required":["path"]}},{"name":"write","description":"Write file, creating dirs.","inputSchema":{"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"}},"required":["path","content"]}},{"name":"edit","description":"Replace exact unique text in file. Use it for every change to an existing file, rather than sed or scripts.","inputSchema":{"type":"object","properties":{"path":{"type":"string"},"old":{"type":"string"},"new":{"type":"string"}},"required":["path","old","new"]}}]}}"#
        );
        assert_eq!(
            ask(&mut s, r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"read","arguments":{"path":"f.txt","offset":2,"limit":1}}}"#).unwrap(),
            r#"{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"line2\n[lines 2-2 of 3; read with offset=3 for more]"}]}}"#
        );
        assert_eq!(
            ask(&mut s, r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"bash","arguments":{"command":"mkdir -p sub && cd sub && echo hi; echo err >&2; false"}}}"#).unwrap(),
            r#"{"jsonrpc":"2.0","id":4,"result":{"content":[{"type":"text","text":"hi\nerr\n\n[exit 1]"}]}}"#
        );
        let pwd = ask(&mut s, r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"bash","arguments":{"command":"pwd"}}}"#).unwrap();
        assert!(pwd.contains(r#"/sub\n""#), "cd persists between calls: {pwd}");
        assert_eq!(
            ask(&mut s, r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"edit","arguments":{"path":"../f.txt","old":"line","new":"x"}}}"#).unwrap(),
            r#"{"jsonrpc":"2.0","id":6,"result":{"content":[{"type":"text","text":"old text found 3 times, need exactly 1: include more of the surrounding lines"}],"isError":true}}"#
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
        assert!(edit(&f, "missing", "x").unwrap_err().contains("nor its first line"));
        write(&f, "a\nfn x() {\n    y();\n}\n").unwrap();
        let e = edit(&f, "fn x() {\n  y();", "z").unwrap_err();
        assert!(e.contains("first line is at line 2") && e.contains("    y();"), "shows the lines as they are: {e}");
        assert!(edit(&f, "", "z").unwrap_err().contains("`old` is missing"));
        assert_eq!(cap("abcdef", 4), "abcd\n[truncated 2 chars]");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn shrinks_long_output_and_saves_it() {
        let mut s = Server { cwd: std::env::temp_dir(), max: 30000, cap: 2000, saved: 0, out_dir: None, seen: None, reads: 0, left: vec![], left_at: left_file(std::process::id()) };
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
        let mut off = Server { cwd: std::env::temp_dir(), max: 30000, cap: 0, saved: 0, out_dir: None, seen: None, reads: 0, left: vec![], left_at: left_file(std::process::id()) };
        assert_eq!(off.bash("seq 1 3000 | wc -l", Duration::from_secs(10)).trim(), "3000");
        assert!(off.bash("seq 1 3000", Duration::from_secs(10)).chars().count() > 13000, "cap 0 leaves output alone");
    }

    #[test]
    fn timeout_kills_the_whole_command() {
        let mut s = Server { cwd: std::env::temp_dir(), max: 30000, cap: DEFAULT_BASH_CAP, saved: 0, out_dir: None, seen: None, reads: 0, left: vec![], left_at: left_file(std::process::id()) };
        let t0 = Instant::now();
        // the background sleep would outlive a plain kill of bash
        let out = s.bash("sleep 30 & echo started; wait", Duration::from_millis(500));
        assert!(t0.elapsed() < Duration::from_secs(5), "returned at the timeout, not after the child");
        assert!(out.starts_with("started"), "{out}");
        assert!(out.contains("timed out"), "{out}");
    }

    #[test]
    fn bash_takes_a_timeout_in_seconds() {
        let mut s = Server::new(std::env::temp_dir());
        let call = |s: &mut Server, t: Value| s.call(&json!({"name": "bash", "arguments": {"command": "sleep 2; echo done", "timeout": t}})).unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        let out = call(&mut s, json!(1));
        assert!(out.contains("timed out after 1s") && out.contains("larger timeout"), "{out}");
        assert!(call(&mut s, json!("5")).starts_with("done"), "a quoted number counts");
        assert!(call(&mut s, json!(0)).contains("timed out after 1s"), "at least 1 s");
    }
}
