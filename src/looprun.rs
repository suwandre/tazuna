//! The PRD loop (`/loop` in a session, `tazuna --loop` from a shell): TODO.md's unchecked items, built one at a time, each
//! in a fresh headless run (`tazuna --run` with the loop's flags, so a long project never fills one context), committed
//! once its run ticks it. Stops when every item is ticked, when an item is still open after two runs, at the dollar cap,
//! or before Claude's 5-hour limit runs out.

use anyhow::{Context, Result};
use serde_json::Value;
use std::{
    path::Path,
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};

/// What each run is told: one item, its tests, the tick. The loop commits, so the reviewer sees the item's change.
pub const PROMPT: &str = "Read PRD.md and TODO.md. Implement only the first unchecked item (`- [ ]`) in TODO.md, with tests \
for it. When its tests pass, tick it (`- [x]`) in TODO.md. Do not commit, and do not start later items. If the item cannot \
be done, write why under it, leave it unchecked and stop.";

/// What the session is asked when there is no TODO.md to build from yet.
pub const PLAN: &str = "Write PRD.md (the goal, scope and every decision from our conversation) and TODO.md (one `- [ ] item` \
per line, in build order, each small enough to implement and test in one go). Do not implement anything yet.";

/// What a whole loop may spend at list prices (`TAZUNA_LOOP_USD`); on the own loop each item also has its $1 cap.
const MAX_USD: f64 = 25.0;
/// Stop before the 5-hour window runs out: past it the subscription goes on as paid extra usage.
const MAX_FIVE_HOUR: f64 = 0.85;

/// The unchecked items in TODO.md, in order.
pub fn open_items(todo: &str) -> Vec<String> {
    todo.lines().filter_map(|l| l.trim_start().strip_prefix("- [ ]").map(|s| s.trim().to_string())).collect()
}

/// The flags `/loop ARGS` passes to each run: a bare word is a preset (`/loop reviewed`), flags go as they are.
pub fn flags(args: &str) -> Vec<String> {
    let mut f: Vec<String> = args.split_whitespace().map(String::from).collect();
    if f.first().is_some_and(|w| !w.starts_with('-')) {
        f.insert(0, "--preset".into());
    }
    f
}

/// Builds until done or stopped; `say` hears each step, the return value is the closing line.
pub fn run(cwd: &Path, flags: &[String], stop: &AtomicBool, say: &mut dyn FnMut(String)) -> Result<String> {
    let prompt = crate::labels::dir()?.join("loop-prompt.md");
    std::fs::write(&prompt, PROMPT)?;
    let max_usd = std::env::var("TAZUNA_LOOP_USD").ok().and_then(|v| v.parse().ok()).unwrap_or(MAX_USD);
    let exe = std::env::current_exe()?;
    let todo = || std::fs::read_to_string(cwd.join("TODO.md")).unwrap_or_default();
    let (mut built, mut spent, mut tries, mut last) = (0, 0.0, 0, String::new());
    loop {
        let open = open_items(&todo());
        let Some(item) = open.first().cloned() else { return Ok(format!("all done: {}", left(built, 0, spent))) };
        if stop.load(Ordering::Relaxed) {
            return Ok(format!("stopped as asked: {}", left(built, open.len(), spent)));
        }
        if spent >= max_usd {
            return Ok(format!("stopped at the ${max_usd:.0} cap (TAZUNA_LOOP_USD): {}", left(built, open.len(), spent)));
        }
        tries = if item == last { tries + 1 } else { 1 };
        if tries > 2 {
            return Ok(format!("stopped: \"{item}\" is still open after two runs (see TODO.md): {}", left(built, open.len(), spent)));
        }
        last = item.clone();
        say(format!("loop: building \"{item}\" ({} open{})", open.len(), if tries == 2 { ", second try" } else { "" }));
        let t = Instant::now();
        let child = Command::new(&exe).arg("--run").arg(&prompt).args(flags).current_dir(cwd)
            .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().context("starting a run")?;
        let pid = child.id();
        let out = child.wait_with_output()?;
        let result: Value = String::from_utf8_lossy(&out.stdout).lines().last().and_then(|l| serde_json::from_str(l).ok()).unwrap_or_default();
        let cost = result["total_cost_usd"].as_f64().unwrap_or(0.0);
        spent += cost;
        let after = open_items(&todo());
        let mins = t.elapsed().as_secs_f64() / 60.0;
        if !after.contains(&item) && after.len() < open.len() {
            git(cwd, &["add", "-A"])?;
            git(cwd, &["commit", "-q", "-m", &format!("loop: {item}")])?;
            built += 1;
            say(format!("loop: done \"{item}\" (${cost:.2}, {mins:.1} min)"));
        } else {
            let why = String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or_default().to_string();
            say(format!("loop: \"{item}\" still open after {mins:.1} min{}", if why.is_empty() { String::new() } else { format!(": {why}") }));
        }
        if let Some(u) = five_hour(pid).filter(|u| *u >= MAX_FIVE_HOUR) {
            return Ok(format!("stopped: Claude's 5-hour window at {:.0}% (past it the plan goes on as paid extra usage); /loop again \
                after it resets: {}", u * 100.0, left(built, after.len(), spent)));
        }
    }
}

fn left(built: usize, open: usize, spent: f64) -> String {
    format!("{built} built, {open} left, ${spent:.2}")
}

fn git(cwd: &Path, args: &[&str]) -> Result<()> {
    let out = Command::new("git").args(args).current_dir(cwd).output()?;
    anyhow::ensure!(out.status.success(), "git {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim());
    Ok(())
}

/// The 5-hour utilization (0-1) the run's claude last reported; its session file ends in the run's process id. None on the
/// own loop, which reports none.
fn five_hour(pid: u32) -> Option<f64> {
    let dir = crate::labels::dir().ok()?.join("sessions");
    let end = format!("-{pid}.jsonl");
    let f = std::fs::read_dir(dir).ok()?.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.to_string_lossy().ends_with(&end)).max()?;
    std::fs::read_to_string(f).ok()?.lines().rev().filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find(|d| d["e"]["type"] == "rate_limit_event")?["e"]["rate_limit_info"]["unifiedWindows"]["five_hour"]["utilization"].as_f64()
}

/// A macOS notification, for a loop that ends while nobody watches.
pub fn notify(text: &str) {
    let script = format!("display notification \"{}\" with title \"tazuna\"", text.replace(['"', '\\'], "'"));
    let _ = Command::new("osascript").args(["-e", &script]).output();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_open_items_and_loop_flags() {
        let todo = "# TODO\n- [x] map\n- [ ] combat\n  - [ ] nested counts too\n- [ ]   inventory  \nnotes";
        assert_eq!(open_items(todo), ["combat", "nested counts too", "inventory"]);
        assert_eq!(flags("reviewed"), ["--preset", "reviewed"]);
        assert_eq!(flags("--model claude-opus-5-5 --effort high"), ["--model", "claude-opus-5-5", "--effort", "high"]);
        assert!(flags("").is_empty());
    }
}
