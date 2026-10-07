//! Labels for runs: the training data for routing (Jev). Kept in their own append-only file next to the run log.
//!
//! - `user`: your verdict, `/good` or `/bad` in the live UI or `tazuna --verdict`. Strongest signal.
//! - `git`: whether a run's changes later landed in a commit (`kept`) or were thrown away (`discarded`).

use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::{collections::HashMap, io::Write, path::{Path, PathBuf}, process::Command};

/// A run whose changes are in no commit and gone from the working tree after this long counts as discarded.
const DISCARD_AFTER_SECS: u64 = 24 * 3600;

pub fn dir() -> Result<PathBuf> {
    Ok(PathBuf::from(std::env::var("HOME").context("HOME")?).join(".local/share/tazuna"))
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn read_jsonl(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

pub fn runs(base: &Path) -> Vec<Value> {
    read_jsonl(&base.join("runs.jsonl"))
}

pub fn add(base: &Path, run: &str, verdict: &str, note: &str, source: &str) -> Result<()> {
    let line = json!({"ts": now(), "run": run, "verdict": verdict, "note": note, "source": source});
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(base.join("labels.jsonl"))?;
    // one write per line: concurrent appends (O_APPEND) then never interleave
    f.write_all(format!("{line}\n").as_bytes())?;
    Ok(())
}

/// The label that counts for each run: the latest user verdict, else the latest git one.
pub fn load(base: &Path) -> HashMap<String, Value> {
    let mut out: HashMap<String, Value> = HashMap::new();
    for l in read_jsonl(&base.join("labels.jsonl")) {
        let Some(run) = l["run"].as_str().map(String::from) else { continue };
        let keep_old = out.get(&run).is_some_and(|old| old["source"] == "user" && l["source"] != "user");
        if !keep_old {
            out.insert(run, l);
        }
    }
    out
}

/// `tazuna --verdict good|bad [--note ..] [--id RUN]`: labels a run (default: the latest one).
pub fn verdict(base: &Path, verdict: &str, note: &str, run: Option<&str>) -> Result<String> {
    anyhow::ensure!(matches!(verdict, "good" | "bad"), "verdict must be good or bad");
    let id = match run {
        Some(id) => id.to_string(),
        None => runs(base).iter().rev().find_map(|r| r["id"].as_str().map(String::from)).context("no run to label yet (runs from tazuna 0.0.7 on have ids)")?,
    };
    add(base, &id, verdict, note, "user")?;
    Ok(id)
}

fn git(cwd: &Path, args: &[&str]) -> Option<String> {
    Command::new("git").args(args).current_dir(cwd).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Labels finished runs from git: `kept` when any file the run produced was later committed with exactly that content,
/// `discarded` when a day has passed and none of it is committed or still in the working tree. `only` limits it to runs
/// in one repo. Returns (kept, discarded).
// ponytail: exact content match only; a run whose output you edited before committing stays unlabelled
pub fn label_git(base: &Path, only: Option<&Path>) -> Result<(usize, usize)> {
    let labelled = load(base);
    let (mut kept, mut discarded) = (0, 0);
    for r in runs(base) {
        let (Some(id), Some(cwd), Some(after)) = (r["id"].as_str(), r["meta"]["cwd"].as_str(), r["git"]["after"].as_object()) else { continue };
        let cwd = Path::new(cwd);
        if labelled.contains_key(id) || after.is_empty() || only.is_some_and(|o| o != cwd) || !cwd.is_dir() {
            continue;
        }
        let ts = r["ts"].as_u64().unwrap_or(0);
        let files: Vec<(&String, &str)> = after.iter().filter_map(|(p, h)| h.as_str().filter(|h| *h != "deleted").map(|h| (p, h))).collect();
        let committed = files.iter().any(|(path, hash)| {
            let commits = git(cwd, &["log", "--all", &format!("--since=@{ts}"), "--format=%H", "--", path]).unwrap_or_default();
            commits.lines().any(|c| git(cwd, &["rev-parse", &format!("{c}:{path}")]).as_deref() == Some(*hash))
        });
        if committed {
            add(base, id, "kept", "its changes were committed", "git")?;
            kept += 1;
            continue;
        }
        let still_here = files.iter().any(|(path, hash)| git(cwd, &["hash-object", "--", path]).as_deref() == Some(*hash));
        if !still_here && now().saturating_sub(ts) > DISCARD_AFTER_SECS {
            add(base, id, "discarded", "its changes are in no commit and gone from the working tree", "git")?;
            discarded += 1;
        }
    }
    Ok((kept, discarded))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_labels_committed_runs_as_kept() {
        let base = std::env::temp_dir().join(format!("tazuna-labels-{}", std::process::id()));
        let repo = base.join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        let g = |args: &[&str]| assert!(Command::new("git").args(args).current_dir(&repo).output().unwrap().status.success(), "{args:?}");
        g(&["init", "-q"]);
        std::fs::write(repo.join("a.txt"), "agent wrote this\n").unwrap();
        let hash = git(&repo, &["hash-object", "a.txt"]).unwrap();
        let run = json!({"id": "r1", "ts": now() - 5, "meta": {"cwd": repo}, "git": {"after": {"a.txt": hash}}});
        std::fs::write(base.join("runs.jsonl"), format!("{run}\n")).unwrap();
        assert_eq!(label_git(&base, None).unwrap(), (0, 0), "not committed yet, and too recent to call discarded");
        g(&["add", "a.txt"]);
        g(&["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-qm", "keep"]);
        assert_eq!(label_git(&base, None).unwrap(), (1, 0));
        assert_eq!(load(&base)["r1"]["verdict"], "kept");
        assert_eq!(verdict(&base, "bad", "wrong approach", None).unwrap(), "r1", "defaults to the latest run");
        add(&base, "r1", "kept", "", "git").unwrap();
        assert_eq!(load(&base)["r1"]["verdict"], "bad", "a user verdict beats git");
        std::fs::remove_dir_all(&base).ok();
    }
}
