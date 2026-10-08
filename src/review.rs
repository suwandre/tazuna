//! An independent reviewer (`--review-model`). When claude first says it is done, a separate `claude -p` on the review
//! model reads the task and the diff in a fresh context (read-only tools) and lists what is wrong; the findings go back to
//! the implementer once. A worker and its checker never share a context, and a cheap implementer gets a strong check
//! without a strong model re-reading the whole session.

use serde_json::Value;
use std::{
    io::Read,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const SYSTEM: &str = "You review a coding agent's change against its task. You did not write it. Read the task, the diff, and any code you need with your read-only tools. Report only real problems: a requirement or acceptance criterion not met, wrong or broken behavior, a change the task did not ask for, an assumption about the code that is false. No style preferences, no praise. Reply with only JSON: {\"verdict\": \"ok\" or \"fix\", \"issues\": [\"one concrete problem: file, what is wrong, what to change\"]}";
const MAX_DIFF: usize = 80_000;
const TIMEOUT: Duration = Duration::from_secs(900);

pub struct Review {
    pub ok: bool,
    pub issues: Vec<String>,
    pub cost: f64,
    pub secs: f64,
    /// The reviewer failed (did not start, timed out, unreadable answer): the run goes on unreviewed.
    pub error: Option<String>,
}

/// Blocking; call from a thread.
pub fn run(cwd: &Path, task: &str, model: &str, effort: &str) -> Review {
    let t0 = Instant::now();
    let prompt = format!("# Task\n\n{task}\n\n# The change (against HEAD; new files in full)\n\n```diff\n{}\n```", diff(cwd));
    // TAZUNA_REVIEWER: a stand-in for `claude` (tests of the feedback path need a reviewer that finds something)
    let spawned = Command::new(std::env::var("TAZUNA_REVIEWER").unwrap_or_else(|_| "claude".into()))
        .args(["-p", &prompt, "--model", model, "--effort", effort, "--output-format", "json", "--no-session-persistence"])
        .args(["--setting-sources", "", "--strict-mcp-config", "--disable-slash-commands", "--tools", "Read,Grep,Glob", "--system-prompt", SYSTEM])
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let fail = |e: String, secs: f64| Review { ok: true, issues: vec![], cost: 0.0, secs, error: Some(e) };
    let mut child = match spawned {
        Ok(c) => c,
        Err(e) => return fail(format!("could not start claude: {e}"), 0.0),
    };
    let mut stdout = child.stdout.take().expect("piped");
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stdout.read_to_string(&mut s);
        s
    });
    loop {
        if let Ok(Some(_)) = child.try_wait() {
            break;
        }
        if t0.elapsed() > TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return fail(format!("timed out after {}s", TIMEOUT.as_secs()), t0.elapsed().as_secs_f64());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    let out = reader.join().unwrap_or_default();
    let secs = t0.elapsed().as_secs_f64();
    let Some(d) = out.lines().rev().find_map(|l| serde_json::from_str::<Value>(l).ok()) else { return fail("no answer".into(), secs) };
    let cost = d["total_cost_usd"].as_f64().unwrap_or(0.0);
    let text = d["result"].as_str().unwrap_or_default();
    match parse(text) {
        Some((ok, issues)) => Review { ok, issues, cost, secs, error: None },
        None => Review { cost, ..fail(format!("unreadable answer: {}", text.chars().take(200).collect::<String>()), secs) },
    }
}

/// `{"verdict": .., "issues": [..]}` anywhere in the reply. "fix" with no issues counts as ok.
fn parse(text: &str) -> Option<(bool, Vec<String>)> {
    let v: Value = serde_json::from_str(&text[text.find('{')?..=text.rfind('}')?]).ok()?;
    let issues: Vec<String> = v["issues"].as_array()?.iter().filter_map(|i| i.as_str().map(String::from)).collect();
    Some((v["verdict"] != "fix" || issues.is_empty(), issues))
}

/// The run's change: `git diff HEAD` plus untracked files in full, cut at MAX_DIFF.
fn diff(cwd: &Path) -> String {
    let git = |args: &[&str]| Command::new("git").args(args).current_dir(cwd).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    let mut d = git(&["diff", "HEAD"]);
    for f in git(&["ls-files", "--others", "--exclude-standard"]).lines() {
        if let Ok(text) = std::fs::read_to_string(cwd.join(f)) {
            d.push_str(&format!("\n--- /dev/null\n+++ b/{f}\n{}", text.lines().map(|l| format!("+{l}\n")).collect::<String>()));
        }
    }
    if d.len() > MAX_DIFF {
        let cut = d.char_indices().map(|(i, _)| i).take_while(|&i| i <= MAX_DIFF).last().unwrap_or(0);
        d.truncate(cut);
        d.push_str("\n[diff cut here; read the files for the rest]");
    }
    d
}

/// The message that sends the findings back to the implementer.
pub fn feedback(r: &Review) -> String {
    let list: Vec<String> = r.issues.iter().enumerate().map(|(i, s)| format!("{}. {s}", i + 1)).collect();
    format!(
        "An independent reviewer checked your change against the task and found:\n\n{}\n\nFix these. If you are sure one is wrong, say why in one sentence instead. Run the relevant tests again, then finish with a short summary.",
        list.join("\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_verdicts() {
        assert_eq!(parse(r#"{"verdict": "ok", "issues": []}"#), Some((true, vec![])));
        assert_eq!(parse("Here:\n```json\n{\"verdict\": \"fix\", \"issues\": [\"a.ts: x is never set\"]}\n```"), Some((false, vec!["a.ts: x is never set".into()])));
        assert_eq!(parse(r#"{"verdict": "fix", "issues": []}"#), Some((true, vec![])), "nothing concrete to fix");
        assert_eq!(parse("looks fine"), None);
        let r = Review { ok: false, issues: vec!["one".into(), "two".into()], cost: 0.0, secs: 0.0, error: None };
        assert!(feedback(&r).contains("1. one\n2. two"));
    }

    #[test]
    fn diff_includes_new_files() {
        let repo = std::env::temp_dir().join(format!("tazuna-review-{}", std::process::id()));
        std::fs::create_dir_all(&repo).unwrap();
        let sh = |c: &str| assert!(Command::new("sh").args(["-c", c]).current_dir(&repo).status().unwrap().success());
        sh("git init -q && echo a > old.txt && git add -A && git -c user.name=t -c user.email=t@t commit -qm base");
        std::fs::write(repo.join("old.txt"), "b\n").unwrap();
        std::fs::write(repo.join("new.txt"), "fresh\n").unwrap();
        let d = diff(&repo);
        assert!(d.contains("-a\n+b") && d.contains("+++ b/new.txt\n+fresh"), "{d}");
        std::fs::remove_dir_all(&repo).ok();
    }
}
