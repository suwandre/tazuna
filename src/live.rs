//! Drives a real `claude` process (stream-json in and out) with the cc-mid profile, and records every event.
//!
//! One process serves the whole conversation, so follow-ups keep the prompt cache warm. Stopping a run sends the
//! interrupt control request the official Agent SDK uses; if claude does not answer, the process is killed and the
//! next message resumes the session with `--resume`.

use crate::{runlog::Run, verify};
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
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
    /// A verification finished.
    Verified(verify::Outcome),
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
    /// From claude's init event: its built-in prompt changes between versions, which moves token counts.
    claude_version: Option<String>,
    generation: u64,
    /// When an interrupt was requested and its run has not ended yet.
    interrupting: Option<Instant>,
    interrupts_sent: u64,
    rec: File,
    pub rec_path: PathBuf,
    t0: Instant,
    /// Runs in flight, oldest first (one per message; claude answers them in order).
    runs: VecDeque<Run>,
    /// claude reports cost cumulatively per process; this is the last total seen.
    proc_cost: f64,
    runs_log: PathBuf,
    /// "live" or "headless", recorded in the run log.
    pub mode: &'static str,
    pub verify: verify::Mode,
    verifying: bool,
    /// Messages typed while tests run; sent once the run is over.
    held: VecDeque<String>,
    /// claude's last result event and the last run-log line (headless mode reports them).
    pub last_result: Option<Value>,
    pub last_log: Option<Value>,
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
            claude_version: None,
            generation: 0,
            interrupting: None,
            interrupts_sent: 0,
            rec,
            rec_path,
            t0: Instant::now(),
            runs: VecDeque::new(),
            proc_cost: 0.0,
            runs_log: data.join("runs.jsonl"),
            mode: "live",
            verify: verify::Mode::Auto,
            verifying: false,
            held: VecDeque::new(),
            last_result: None,
            last_log: None,
        })
    }

    pub fn elapsed_ms(&self) -> f64 {
        self.t0.elapsed().as_secs_f64() * 1000.0
    }

    /// Sends a user message, starting (or resuming) the claude process if needed. Returns the event to show.
    pub fn send(&mut self, text: &str) -> Result<(f64, Value)> {
        if self.verifying {
            self.held.push_back(text.to_string());
        } else {
            self.start_run(text)?;
        }
        Ok(self.record(json!({"type": "harness_user", "text": text})))
    }

    fn start_run(&mut self, text: &str) -> Result<()> {
        self.write_user(text)?;
        let run = Run::new(text, self.elapsed_ms(), &self.cwd);
        self.runs.push_back(run);
        Ok(())
    }

    fn write_user(&mut self, text: &str) -> Result<()> {
        if self.child.is_none() {
            self.spawn()?;
        }
        let line = json!({"type": "user", "message": {"role": "user", "content": text}});
        let stdin = self.stdin.as_mut().context("claude stdin closed")?;
        writeln!(stdin, "{line}")?;
        stdin.flush()?;
        Ok(())
    }

    /// Nothing in flight: every run has ended and no message is waiting.
    pub fn idle(&self) -> bool {
        self.runs.is_empty() && !self.verifying && self.held.is_empty()
    }

    /// Asks claude to stop the current run; the process stays up and keeps its cache.
    pub fn interrupt(&mut self) -> Option<(f64, Value)> {
        if self.verifying {
            // claude is idle while tests run: nothing to interrupt there, the run ends when the tests return
            self.runs.front_mut()?.interrupted();
            return Some(self.record(json!({"type": "harness_verify_cancel"})));
        }
        let stdin = self.stdin.as_mut()?;
        self.interrupts_sent += 1;
        let req = json!({"type": "control_request", "request_id": format!("interrupt-{}", self.interrupts_sent), "request": {"subtype": "interrupt"}});
        if writeln!(stdin, "{req}").and_then(|_| stdin.flush()).is_err() {
            return Some(self.kill("stopped"));
        }
        self.interrupting = Some(Instant::now());
        if let Some(run) = self.runs.front_mut() {
            run.interrupted();
        }
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
        self.end_runs();
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
                        self.claude_version = e["claude_code_version"].as_str().map(String::from);
                    }
                    if let Some(run) = self.runs.front_mut() {
                        run.observe(&e);
                    }
                    let is_result = e["type"] == "result";
                    if is_result {
                        self.interrupting = None;
                        let total = e["total_cost_usd"].as_f64().unwrap_or(self.proc_cost);
                        if let Some(run) = self.runs.front_mut() {
                            run.add_cost(total - self.proc_cost);
                        }
                        self.proc_cost = total;
                        self.last_result = Some(e.clone());
                    }
                    out.push(self.record(e));
                    if is_result {
                        out.extend(self.after_result());
                    }
                }
                Msg::Verified(o) => out.extend(self.after_verify(o)),
                Msg::Exited(stderr) => {
                    let code = self.child.take().and_then(|mut c| c.wait().ok()).and_then(|st| st.code());
                    self.stdin = None;
                    self.end_runs();
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

    /// claude says the front run is done: verify it, or end it.
    fn after_result(&mut self) -> Vec<(f64, Value)> {
        let Some(run) = self.runs.front() else { return vec![] };
        let success = self.last_result.as_ref().is_some_and(|r| r["subtype"] == "success" && r["is_error"] != true);
        let cmd = (success && !run.is_interrupted()).then(|| verify::command(&self.verify, &self.cwd, &run.changed_files(&self.cwd))).flatten();
        let Some(cmd) = cmd else { return self.finish_front() };
        self.verifying = true;
        let round = run.failed_rounds() + 1;
        let (tx, g, cwd, c) = (self.tx.clone(), self.generation, self.cwd.clone(), cmd.clone());
        std::thread::spawn(move || {
            let _ = tx.send((g, Msg::Verified(verify::run(&cwd, &c))));
        });
        vec![self.record(json!({"type": "harness_verify_start", "cmd": cmd, "round": round}))]
    }

    /// Tests are back: pass ends the run; a failure goes back to claude while rounds remain.
    fn after_verify(&mut self, o: verify::Outcome) -> Vec<(f64, Value)> {
        self.verifying = false;
        let Some(run) = self.runs.front_mut() else { return vec![] };
        run.add_verify(&o.cmd, o.ok, o.secs);
        let retry = !o.ok && !run.is_interrupted() && run.failed_rounds() <= verify::MAX_ROUNDS;
        let mut out = vec![self.record(json!({
            "type": "harness_verify_done", "cmd": o.cmd, "ok": o.ok, "code": o.code, "secs": o.secs, "output": o.output, "final": !retry,
        }))];
        if retry {
            let round = self.runs.front().map_or(1, |r| r.failed_rounds());
            match self.write_user(&verify::feedback(&o, round)) {
                Ok(()) => out.push(self.record(json!({"type": "harness_verify_feedback", "round": round, "max": verify::MAX_ROUNDS}))),
                Err(e) => {
                    out.push(self.record(json!({"type": "harness_notice", "text": format!("could not send test failures: {e:#}"), "error": true})));
                    out.extend(self.finish_front());
                }
            }
        } else {
            out.extend(self.finish_front());
        }
        out
    }

    /// Logs the front run, then sends any message typed while it was busy.
    fn finish_front(&mut self) -> Vec<(f64, Value)> {
        if let Some(run) = self.runs.pop_front() {
            let line = run.finish(self.last_result.as_ref(), self.elapsed_ms(), &self.meta(), &self.cwd);
            self.log(line);
        }
        let mut out = Vec::new();
        while self.runs.is_empty() && !self.verifying {
            let Some(text) = self.held.pop_front() else { break };
            if let Err(e) = self.start_run(&text) {
                out.push(self.record(json!({"type": "harness_notice", "text": format!("{e:#}"), "error": true})));
            }
        }
        out
    }

    /// The process is gone: runs without a result end here (stopped if the user asked, otherwise crashed).
    fn end_runs(&mut self) {
        // ponytail: a dead process reports no cost, so these runs log $0; the session's estimate is in the recording
        while let Some(run) = self.runs.pop_front() {
            let line = run.finish(None, self.elapsed_ms(), &self.meta(), &self.cwd);
            self.log(line);
        }
        self.proc_cost = 0.0;
        self.verifying = false;
    }

    fn meta(&self) -> Value {
        json!({
            "model": self.model, "effort": self.effort, "profile": "cc-mid", "version": env!("CARGO_PKG_VERSION"), "mode": self.mode,
            "verify": match &self.verify { verify::Mode::Auto => "auto", verify::Mode::Command(_) => "command", verify::Mode::Off => "off" },
            "claude_version": self.claude_version, "cwd": self.cwd, "session_id": self.session_id, "recording": self.rec_path,
        })
    }

    fn log(&mut self, line: Value) {
        self.last_log = Some(line.clone());
        let appended = std::fs::OpenOptions::new().create(true).append(true).open(&self.runs_log).and_then(|mut f| writeln!(f, "{line}"));
        if let Err(e) = appended {
            eprintln!("run log: {e}");
        }
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
