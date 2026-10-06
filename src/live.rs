//! Drives a real `claude` process (stream-json in and out) with the cc-mid profile, and records every event.
//!
//! One process serves the whole conversation, so follow-ups keep the prompt cache warm. Stopping a run sends the
//! interrupt control request the official Agent SDK uses; if claude does not answer, the process is killed and the
//! next message resumes the session with `--resume`.

use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::{
    fs::File,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{Receiver, Sender, channel},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

/// The lean tool server we benchmarked as cc-mid (bash/read/write/edit with terse schemas).
// ponytail: still the Node server from the benchmarks; port it to Rust in its own step
const LEAN_MCP: &str = include_str!("../tools/lean-mcp.mjs");

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
}

pub struct Live {
    pub model: String,
    pub effort: String,
    cwd: PathBuf,
    mcp: PathBuf,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    /// Messages carry the process generation, so output from a killed process is ignored.
    tx: Sender<(u64, Msg)>,
    rx: Receiver<(u64, Msg)>,
    session_id: Option<String>,
    generation: u64,
    /// When an interrupt was requested and its run has not ended yet.
    interrupting: Option<Instant>,
    interrupts_sent: u64,
    rec: File,
    pub rec_path: PathBuf,
    t0: Instant,
}

impl Live {
    pub fn new(model: &str, effort: &str) -> Result<Self> {
        let cwd = std::env::current_dir()?;
        let data = data_dir()?;
        let mcp = data.join("lean-mcp.mjs");
        if std::fs::read_to_string(&mcp).ok().as_deref() != Some(LEAN_MCP) {
            std::fs::write(&mcp, LEAN_MCP)?;
        }
        let sessions = data.join("sessions");
        std::fs::create_dir_all(&sessions)?;
        let dir = cwd.file_name().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default();
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let rec_path = sessions.join(format!("{stamp}-{dir}.jsonl"));
        let mut rec = File::create(&rec_path).with_context(|| format!("creating {}", rec_path.display()))?;
        // same header as recordings/record.py, so a live session replays like any recording
        writeln!(rec, "{}", json!({"t": 0, "prompt": "", "effort": effort, "model": model, "cwd": cwd}))?;
        let (tx, rx) = channel();
        Ok(Live {
            model: model.into(),
            effort: effort.into(),
            cwd,
            mcp,
            child: None,
            stdin: None,
            tx,
            rx,
            session_id: None,
            generation: 0,
            interrupting: None,
            interrupts_sent: 0,
            rec,
            rec_path,
            t0: Instant::now(),
        })
    }

    pub fn elapsed_ms(&self) -> f64 {
        self.t0.elapsed().as_secs_f64() * 1000.0
    }

    /// Sends a user message, starting (or resuming) the claude process if needed. Returns the event to show.
    pub fn send(&mut self, text: &str) -> Result<(f64, Value)> {
        if self.child.is_none() {
            self.spawn()?;
        }
        let line = json!({"type": "user", "message": {"role": "user", "content": text}});
        let stdin = self.stdin.as_mut().context("claude stdin closed")?;
        writeln!(stdin, "{line}")?;
        stdin.flush()?;
        Ok(self.record(json!({"type": "harness_user", "text": text})))
    }

    /// Asks claude to stop the current run; the process stays up and keeps its cache.
    pub fn interrupt(&mut self) -> Option<(f64, Value)> {
        let stdin = self.stdin.as_mut()?;
        self.interrupts_sent += 1;
        let req = json!({"type": "control_request", "request_id": format!("interrupt-{}", self.interrupts_sent), "request": {"subtype": "interrupt"}});
        if writeln!(stdin, "{req}").and_then(|_| stdin.flush()).is_err() {
            return Some(self.kill("stopped"));
        }
        self.interrupting = Some(Instant::now());
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
                    }
                    if e["type"] == "result" {
                        self.interrupting = None;
                    }
                    out.push(self.record(e));
                }
                Msg::Exited(stderr) => {
                    let code = self.child.take().and_then(|mut c| c.wait().ok()).and_then(|st| st.code());
                    self.stdin = None;
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

    fn record(&mut self, e: Value) -> (f64, Value) {
        let t = self.elapsed_ms();
        let _ = writeln!(self.rec, "{}", json!({"t": t as u64, "e": e}));
        (t, e)
    }

    fn spawn(&mut self) -> Result<()> {
        let date = Command::new("date").arg("+%Y-%m-%d").output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
        let prompt = MID_PROMPT.replace("{cwd}", &self.cwd.to_string_lossy()).replace("{date}", &date);
        let mcp = json!({"mcpServers": {"lean": {"type": "stdio", "command": "node", "args": [self.mcp]}}}).to_string();
        let mut cmd = Command::new("claude");
        cmd.args(["-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose", "--include-partial-messages"])
            .args(["--model", &self.model, "--effort", &self.effort])
            // cc-mid: lean prompt and tools, none of the user's settings, skills or MCP servers
            .args(["--setting-sources", "", "--strict-mcp-config", "--disable-slash-commands", "--dangerously-skip-permissions"])
            .args(["--system-prompt", &prompt, "--tools", "Grep,Glob", "--mcp-config", &mcp])
            .args(["--allowedTools", "mcp__lean__bash,mcp__lean__read,mcp__lean__write,mcp__lean__edit"]);
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

/// Live mode only runs where every change can be reviewed and undone.
pub fn in_git_repo() -> bool {
    Command::new("git").args(["rev-parse", "--is-inside-work-tree"]).output().is_ok_and(|o| o.status.success())
}
