//! Run log: one JSON line per run in ~/.local/share/tazuna/runs.jsonl.
//!
//! A run is one user message until claude's `result` (or the process dying). The log records why it stopped, what it
//! cost, how each turn used the prompt cache, and what changed in the repo: the data the router and Jev will learn from.

use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path, process::Command};

/// Per-turn token counts: [input, cache write, cache read, output].
type Turn = [u64; 4];

pub struct Run {
    started_ms: f64,
    prompt: String,
    turns: Vec<Turn>,
    /// tool name -> (calls, failures)
    tools: BTreeMap<String, (u32, u32)>,
    /// tool_use id -> tool name, to attribute failures
    ids: BTreeMap<String, String>,
    interrupted: bool,
    /// Content hashes of the files that differed from HEAD when the run started.
    git_before: Option<BTreeMap<String, String>>,
    /// $ across all rounds (claude reports per process; the caller adds each result's share).
    cost: f64,
    verify_cmd: Option<String>,
    /// One entry per verification: (passed, seconds).
    verify_rounds: Vec<(bool, f64)>,
    /// Failed tests moved this run to the stronger model.
    escalated: bool,
    /// The repo's type check output when the run started, so verification only fails on errors this run added.
    pub types_before: Option<Baseline>,
    /// The self-review turn was sent (`--self-review`): once per run.
    pub reviewed: bool,
}

/// Output of a command that started with the run: pending until the first verification needs it.
pub enum Baseline {
    Pending(std::thread::JoinHandle<String>),
    Done(Option<String>),
}

impl Baseline {
    pub fn get(&mut self) -> Option<String> {
        if let Baseline::Pending(_) = self {
            let Baseline::Pending(h) = std::mem::replace(self, Baseline::Done(None)) else { unreachable!() };
            *self = Baseline::Done(h.join().ok());
        }
        match self {
            Baseline::Done(v) => v.clone(),
            Baseline::Pending(_) => None,
        }
    }
}

impl Run {
    pub fn new(prompt: &str, at_ms: f64, cwd: &Path) -> Self {
        Run {
            started_ms: at_ms,
            prompt: prompt.to_string(),
            turns: Vec::new(),
            tools: BTreeMap::new(),
            ids: BTreeMap::new(),
            interrupted: false,
            git_before: git_snapshot(cwd),
            cost: 0.0,
            verify_cmd: None,
            verify_rounds: Vec::new(),
            escalated: false,
            types_before: None,
            reviewed: false,
        }
    }

    pub fn escalated(&mut self) {
        self.escalated = true;
    }

    pub fn add_cost(&mut self, usd: f64) {
        self.cost += usd;
    }

    pub fn add_verify(&mut self, cmd: &str, ok: bool, secs: f64) {
        self.verify_cmd = Some(cmd.to_string());
        self.verify_rounds.push((ok, secs));
    }

    /// Failed verifications so far (each one earned claude another round).
    pub fn failed_rounds(&self) -> u32 {
        self.verify_rounds.iter().filter(|(ok, _)| !ok).count() as u32
    }

    pub fn is_interrupted(&self) -> bool {
        self.interrupted
    }

    /// Files this run has changed so far.
    pub fn changed_files(&self, cwd: &Path) -> Vec<String> {
        match (&self.git_before, git_snapshot(cwd)) {
            (Some(before), Some(after)) => changed(before, &after).into_iter().cloned().collect(),
            _ => Vec::new(),
        }
    }

    pub fn interrupted(&mut self) {
        self.interrupted = true;
    }

    /// Feed every claude event that belongs to this run.
    pub fn observe(&mut self, e: &Value) {
        match e["type"].as_str() {
            Some("stream_event") if e["event"]["type"] == "message_delta" => {
                let u = &e["event"]["usage"];
                let n = |k: &str| u[k].as_u64().unwrap_or(0);
                self.turns.push([n("input_tokens"), n("cache_creation_input_tokens"), n("cache_read_input_tokens"), n("output_tokens")]);
            }
            Some("assistant") => {
                for c in e["message"]["content"].as_array().into_iter().flatten().filter(|c| c["type"] == "tool_use") {
                    let name = c["name"].as_str().unwrap_or_default();
                    let name = name.strip_prefix("mcp__lean__").unwrap_or(name).to_string();
                    self.ids.insert(c["id"].as_str().unwrap_or_default().to_string(), name.clone());
                    self.tools.entry(name).or_default().0 += 1;
                }
            }
            Some("user") => {
                for c in e["message"]["content"].as_array().into_iter().flatten().filter(|c| c["type"] == "tool_result" && c["is_error"] == true) {
                    if let Some(t) = c["tool_use_id"].as_str().and_then(|id| self.ids.get(id)).and_then(|n| self.tools.get_mut(n)) {
                        t.1 += 1;
                    }
                }
            }
            _ => {}
        }
    }

    /// Turns that re-read less than half of what the previous turn had in cache (models with a minimum cacheable length
    /// cache nothing on short prompts; that is not a miss).
    fn cache_misses(&self) -> Vec<usize> {
        self.turns
            .windows(2)
            .enumerate()
            .filter(|(_, w)| {
                let cached = w[0][1] + w[0][2];
                cached > 0 && w[1][2] * 2 < cached
            })
            .map(|(i, _)| i + 1)
            .collect()
    }

    /// Builds the log line. `result` is claude's last result event, or None if the process died first.
    pub fn finish(self, result: Option<&Value>, at_ms: f64, meta: &Value, cwd: &Path) -> Value {
        let verify = match (self.verify_rounds.last(), &self.verify_cmd) {
            _ if self.interrupted => "skipped",
            (Some((true, _)), _) => "verified",
            (Some((false, _)), _) => "verify_failed",
            (None, _) if result.is_none() => "skipped",
            (None, _) if meta["verify"] == "off" => "off",
            (None, _) => "claimed",
        };
        let stop = match result {
            _ if self.interrupted => "stopped",
            None => "crashed",
            Some(r) => match r["subtype"].as_str() {
                Some("success") if r["is_error"] != true => "completed",
                Some("error_max_turns") => "max_turns",
                Some(s) if s.contains("budget") => "budget",
                _ => "error",
            },
        };
        let sum = |k: usize| self.turns.iter().map(|t| t[k]).sum::<u64>();
        let total_in = sum(0) + sum(1) + sum(2);
        let first_line = self.prompt.lines().next().unwrap_or_default();
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
        json!({
            // what labels refer to: `tazuna --verdict good --id <id>`
            "id": format!("{:x}", now.as_millis()),
            "ts": now.as_secs(),
            "meta": meta,
            "prompt": first_line.chars().take(200).collect::<String>(),
            "prompt_chars": self.prompt.chars().count(),
            "stop": stop,
            "result_subtype": result.map(|r| r["subtype"].clone()).unwrap_or(Value::Null),
            "terminal_reason": result.map(|r| r["terminal_reason"].clone()).unwrap_or(Value::Null),
            "secs": ((at_ms - self.started_ms) / 100.0).round() / 10.0,
            "turns": self.turns.len(),
            "cost_usd": (self.cost * 1e6).round() / 1e6,
            "escalated": self.escalated,
            "verify": {
                "status": verify,
                "cmd": self.verify_cmd,
                "rounds": self.verify_rounds.len(),
                // `+ 0.0`: an empty f64 sum is -0.0
                "secs": (self.verify_rounds.iter().map(|r| r.1).sum::<f64>() * 10.0).round() / 10.0 + 0.0,
            },
            "tokens": {"input": sum(0), "cache_write": sum(1), "cache_read": sum(2), "output": sum(3)},
            "cache_hit": if total_in == 0 { 0.0 } else { (sum(2) as f64 / total_in as f64 * 1000.0).round() / 1000.0 },
            "cache_misses": self.cache_misses(),
            "per_turn": self.turns,
            "tools": self.tools.iter().map(|(k, (c, f))| (k.clone(), json!({"calls": c, "failed": f}))).collect::<serde_json::Map<_, _>>(),
            "git": git_changes(cwd, self.git_before.as_ref()),
        })
    }
}

fn git(cwd: &Path, args: &[&str]) -> Option<String> {
    Command::new("git").args(args).current_dir(cwd).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
}

/// Path -> content hash for every file that differs from HEAD (read-only: `git status`, `git hash-object`).
pub(crate) fn git_snapshot(cwd: &Path) -> Option<BTreeMap<String, String>> {
    let status = git(cwd, &["status", "--porcelain", "--untracked-files=all"])?;
    // "XY path" or "XY old -> new"
    let paths: Vec<String> = status.lines().filter_map(|l| l.get(3..)).map(|p| p.rsplit(" -> ").next().unwrap_or(p).trim_matches('"').to_string()).collect();
    let existing: Vec<&String> = paths.iter().filter(|p| cwd.join(p).is_file()).collect();
    let mut args = vec!["hash-object", "--"];
    args.extend(existing.iter().map(|p| p.as_str()));
    let hashes = if existing.is_empty() { String::new() } else { git(cwd, &args)? };
    let mut snap: BTreeMap<String, String> = paths.iter().map(|p| (p.clone(), "deleted".to_string())).collect();
    for (p, h) in existing.into_iter().zip(hashes.lines()) {
        snap.insert(p.clone(), h.to_string());
    }
    Some(snap)
}

pub(crate) fn changed<'a>(before: &'a BTreeMap<String, String>, after: &'a BTreeMap<String, String>) -> Vec<&'a String> {
    let mut v: Vec<&String> = after.iter().filter(|(p, h)| before.get(*p) != Some(h)).map(|(p, _)| p).collect();
    // files the run reverted to HEAD drop out of `git status`
    v.extend(before.keys().filter(|p| !after.contains_key(*p)));
    v
}

/// Files this run changed (compared with the snapshot at its start), plus the working tree's total diff against HEAD.
fn git_changes(cwd: &Path, before: Option<&BTreeMap<String, String>>) -> Value {
    let (Some(before), Some(after)) = (before, git_snapshot(cwd)) else { return Value::Null };
    let changed = changed(before, &after);
    let stat = git(cwd, &["diff", "--shortstat", "HEAD"]).unwrap_or_default();
    let num = |word: &str| stat.split(',').find(|p| p.contains(word)).and_then(|p| p.split_whitespace().next()?.parse::<u64>().ok()).unwrap_or(0);
    json!({
        "files_changed": changed.len(),
        "files": changed.iter().take(20).collect::<Vec<_>>(),
        // content hash of each changed file after the run: lets `--label-git` find it in later commits
        "after": changed.iter().take(200).map(|p| ((*p).clone(), json!(after.get(*p).map_or("deleted", |h| h.as_str())))).collect::<serde_json::Map<_, _>>(),
        "worktree_vs_head": {"insertions": num("insertion"), "deletions": num("deletion")},
    })
}

/// `tazuna --log [N]`: the last N runs as a table.
pub fn print_recent(base: &Path, n: usize) -> anyhow::Result<()> {
    let rows = crate::labels::runs(base);
    let labels = crate::labels::load(base);
    println!("{:<6} {:<18} {:<16} {:<10} {:<13} {:<10} {:>6} {:>5} {:>8} {:>6} {:>6} {:>5}  prompt", "ago", "repo", "model", "stop", "verify", "verdict", "secs", "turns", "cost", "cache", "misses", "files");
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
    for r in rows.iter().rev().take(n).rev() {
        let ago = now.saturating_sub(r["ts"].as_u64().unwrap_or(now));
        let ago = if ago < 3600 { format!("{}m", ago / 60) } else if ago < 86400 { format!("{}h", ago / 3600) } else { format!("{}d", ago / 86400) };
        let repo = r["meta"]["cwd"].as_str().unwrap_or_default().rsplit('/').next().unwrap_or_default().chars().take(18).collect::<String>();
        let model = r["meta"]["model"].as_str().unwrap_or_default().trim_start_matches("claude-").chars().take(16).collect::<String>();
        println!(
            "{:<6} {:<18} {:<16} {:<10} {:<13} {:<10} {:>6.0} {:>5} {:>8} {:>5.0}% {:>6} {:>5}  {}",
            ago,
            repo,
            model,
            r["stop"].as_str().unwrap_or("?"),
            r["verify"]["status"].as_str().unwrap_or("-"),
            r["id"].as_str().and_then(|id| labels.get(id)).and_then(|l| l["verdict"].as_str()).unwrap_or("-"),
            r["secs"].as_f64().unwrap_or(0.0),
            r["turns"],
            format!("${:.3}", r["cost_usd"].as_f64().unwrap_or(0.0)),
            r["cache_hit"].as_f64().unwrap_or(0.0) * 100.0,
            r["cache_misses"].as_array().map_or(0, |a| a.len()),
            r["git"]["files_changed"].as_u64().map_or("-".into(), |f| f.to_string()),
            r["prompt"].as_str().unwrap_or_default().chars().take(50).collect::<String>(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logs_a_recorded_run() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/recordings/caddy-opus-medium.jsonl");
        let mut r = crate::replay::Replay::load(Path::new(path), false).unwrap();
        let mut run = Run::new(&r.prompt.clone(), 0.0, Path::new("/nonexistent"));
        let mut result = None;
        for (_, e) in r.take(usize::MAX) {
            run.observe(e);
            if e["type"] == "result" {
                result = Some(e.clone());
            }
        }
        run.add_cost(result.as_ref().unwrap()["total_cost_usd"].as_f64().unwrap());
        let line = run.finish(result.as_ref(), 89_000.0, &json!({"model": "claude-opus-5-5"}), Path::new("/nonexistent"));
        assert_eq!(line["verify"]["status"], "claimed");
        assert!((line["cost_usd"].as_f64().unwrap() - 0.308).abs() < 0.001);
        assert_eq!(line["stop"], "completed");
        assert_eq!(line["turns"], 16);
        assert_eq!(line["tools"]["bash"]["calls"], 15);
        assert_eq!(line["tokens"]["output"], 5988);
        assert!(line["cache_hit"].as_f64().unwrap() > 0.85);
        assert_eq!(line["git"], Value::Null); // not a repo
    }

    #[test]
    fn flags_cache_misses() {
        let mut run = Run::new("x", 0.0, Path::new("/nonexistent"));
        run.turns = vec![[2, 3000, 0, 50], [2, 200, 3000, 40], [2, 3300, 0, 40], [2, 100, 3300, 10]];
        assert_eq!(run.cache_misses(), vec![2]); // turn 2 re-read nothing of a 3.2k cached context
        run.turns = vec![[2900, 0, 0, 50], [3000, 0, 0, 40]];
        assert!(run.cache_misses().is_empty(), "nothing was cacheable, so nothing was missed");
    }
}
