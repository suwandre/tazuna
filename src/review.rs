//! An independent reviewer (`--review-model`). When claude first says it is done, a separate `claude -p` on the review
//! model reads the task and the diff in a fresh context (read-only tools) and lists what is wrong; the findings go back to
//! the implementer once. A worker and its checker never share a context, and a cheap implementer gets a strong check
//! without a strong model re-reading the whole session.

use serde_json::Value;
use std::{
    io::{Read, Write},
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

/// Five lenses, from what a careful human reviewer raised on real PRs that a precision-first review missed: it found 13
/// of 19 such findings on Opus, against 6 for "report only a few high-confidence findings" (bench/review-lens.txt).
/// Only Critical and Major go back: fixing minor points widened the patch in an earlier test (scope fell).
const SYSTEM: &str = "You review a coding agent's change that you did not write. Find every defect a careful human reviewer would raise before merge. Do not trust the change's own comments or tests about its correctness. Read the task, the diff, and any code you need with your read-only tools, including code outside the diff.

Work through these lenses:
1. Acceptance criteria: take each criterion and stated behaviour of the task; find the code path that delivers it and try to build a concrete sequence of events that breaks it.
2. Everything else that touches the same data: every other writer, reader, publisher, subscriber, job and cleanup path (exports, pruning, deletion and privacy erasure, sync to external systems) of the state, events and keys the change adds or alters. Was the change made everywhere it needed to be?
3. Partial failure and retries: for each operation with more than one step, what happens when the process dies or a step throws between the steps, when it runs again (at-least-once jobs, client retries, double submits), and when two runs overlap? Look for missing transactions and idempotency keys, swallowed errors, and a failed statement that aborts an enclosing transaction.
4. Lifecycle of new state: who creates each new record, key, flag or id, when it ends, who deletes it, and whether deletion and privacy paths cover it.
5. Actors and variants: anonymous and signed-in, new and returning, another device or session, admin, service and system actors, a feature flag on and off, empty or missing data.

Report only Critical and Major defects you can show with a concrete failure scenario; no style points, no minor issues, no praise. Reply with only JSON: {\"verdict\": \"ok\" or \"fix\", \"issues\": [\"Critical|Major: the defect. file:line. Example: a sequence of events that fails. Fix: the smallest fix.\"]}";
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

/// Blocking; call from a thread. `model@url` reviews through that provider's API with `key` (one call, the changed files
/// in the prompt instead of tools); a plain model name through `claude -p`.
pub fn run(cwd: &Path, task: &str, model: &str, effort: &str, key: Option<&str>) -> Review {
    if let Some((m, url)) = model.split_once('@') {
        return run_api(cwd, task, m, url, effort, key);
    }
    let t0 = Instant::now();
    let prompt = format!("# Task\n\n{task}\n\n# The change (against HEAD; new files in full)\n\n```diff\n{}\n```", diff(cwd));
    // TAZUNA_REVIEWER: a stand-in for `claude` (tests of the feedback path need a reviewer that finds something)
    let spawned = Command::new(std::env::var("TAZUNA_REVIEWER").unwrap_or_else(|_| "claude".into()))
        // the task and the diff on stdin, not as an argument (an agent's `pkill -f` matched text in the arguments, 0.0.76)
        .args(["-p", "--model", model, "--effort", effort, "--output-format", "json", "--no-session-persistence"])
        .args(["--setting-sources", "", "--strict-mcp-config", "--disable-slash-commands", "--tools", "Read,Grep,Glob", "--system-prompt", SYSTEM])
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let fail = |e: String, secs: f64| Review { ok: true, issues: vec![], cost: 0.0, secs, error: Some(e) };
    let mut child = match spawned {
        Ok(c) => c,
        Err(e) => return fail(format!("could not start claude: {e}"), 0.0),
    };
    let mut stdin = child.stdin.take().expect("piped");
    std::thread::spawn(move || {
        let _ = stdin.write_all(prompt.as_bytes()); // dropped at the end: claude reads to EOF
    });
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

/// The reviewer on a provider's API (2026-10-29: GPT-6 Astra on SubRouter as a reviewer for a cheap implementer). It has
/// no tools, so the changed files come in full after the diff. Cost from TAZUNA_REVIEW_PRICE (USD per million: input,
/// cached, output) when set.
fn run_api(cwd: &Path, task: &str, model: &str, url: &str, effort: &str, key: Option<&str>) -> Review {
    let t0 = Instant::now();
    let fail = |e: String| Review { ok: true, issues: vec![], cost: 0.0, secs: t0.elapsed().as_secs_f64(), error: Some(e) };
    let Some(key) = key else { return fail("no key for the reviewer (--review-key-env)".into()) };
    let system = SYSTEM.replace("Read the task, the diff, and any code you need with your read-only tools, including code outside the diff.",
        "Read the task, the diff and the changed files in full below; you have no tools, so reason about code outside them from what they call.");
    let prompt = format!("# Task\n\n{task}\n\n# The change (against HEAD; new files in full)\n\n```diff\n{}\n```\n\n# The changed files now\n\n{}", diff(cwd), changed_files(cwd));
    // the same limit as the claude path: a call that stalls (one Astra review ran past 30 minutes and the run was taken for
    // hung, 2026-10-29) leaves the run unreviewed instead of holding it
    let (tx, rx) = std::sync::mpsc::channel();
    let (url, key, model, effort) = (url.to_string(), key.to_string(), model.to_string(), effort.to_string());
    std::thread::spawn(move || {
        let _ = tx.send(crate::engine::ask(&url, &key, &model, &effort, &system, &prompt));
    });
    let Ok(answer) = rx.recv_timeout(TIMEOUT) else { return fail(format!("timed out after {}s", TIMEOUT.as_secs())) };
    match answer {
        Err(e) => fail(format!("{e:#}")),
        Ok((text, (input, cached, out))) => {
            let price: Vec<f64> = std::env::var("TAZUNA_REVIEW_PRICE").unwrap_or_default().split(',').filter_map(|x| x.trim().parse().ok()).collect();
            let cost = match price[..] {
                [i, c, o] => ((input - cached) as f64 * i + cached as f64 * c + out as f64 * o) / 1e6,
                _ => 0.0,
            };
            let secs = t0.elapsed().as_secs_f64();
            match parse(&text) {
                Some((ok, issues)) => Review { ok, issues, cost, secs, error: None },
                None => Review { cost, ..fail(format!("unreadable answer: {}", text.chars().take(200).collect::<String>())) },
            }
        }
    }
}

/// A changed file's text, only when it is a regular file inside the work tree: the implementer could leave a symlink to a
/// secret elsewhere (a key file, ~/.ssh), and the review goes to a third party.
fn read_inside(cwd: &Path, f: &str) -> Option<String> {
    let p = cwd.join(f);
    let meta = std::fs::symlink_metadata(&p).ok()?;
    if !meta.file_type().is_file() || !p.canonicalize().ok()?.starts_with(cwd.canonicalize().ok()?) {
        return None;
    }
    std::fs::read_to_string(p).ok()
}

/// Every changed or new text file in full, each under its path, cut at MAX_DIFF in all.
fn changed_files(cwd: &Path) -> String {
    let git = |args: &[&str]| Command::new("git").args(args).current_dir(cwd).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    let mut out = String::new();
    for f in git(&["diff", "HEAD", "--name-only"]).lines().chain(git(&["ls-files", "--others", "--exclude-standard"]).lines()) {
        if let Some(text) = read_inside(cwd, f) {
            if out.len() + text.len() > MAX_DIFF {
                out.push_str(&format!("## {f}\n[left out: over the size limit]\n\n"));
                continue;
            }
            out.push_str(&format!("## {f}\n```\n{text}\n```\n\n"));
        }
    }
    out
}

/// `{"verdict": .., "issues": [..]}` anywhere in the reply. "fix" with no issues counts as ok. The first such object wins:
/// prose after it can hold braces of its own (Opus's "ok" followed by a note quoting `{args[0]}`, 2026-10-29).
fn parse(text: &str) -> Option<(bool, Vec<String>)> {
    let v = text.match_indices('{').find_map(|(i, _)| {
        serde_json::Deserializer::from_str(&text[i..]).into_iter::<Value>().next()?.ok().filter(|v| v.get("issues").is_some())
    })?;
    let issues: Vec<String> = v["issues"].as_array()?.iter().filter_map(|i| i.as_str().map(String::from)).collect();
    Some((v["verdict"] != "fix" || issues.is_empty(), issues))
}

/// The run's change: `git diff HEAD` plus untracked files in full, cut at MAX_DIFF.
fn diff(cwd: &Path) -> String {
    let git = |args: &[&str]| Command::new("git").args(args).current_dir(cwd).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    let mut d = git(&["diff", "HEAD"]);
    for f in git(&["ls-files", "--others", "--exclude-standard"]).lines() {
        if let Some(text) = read_inside(cwd, f) {
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
        "An independent reviewer checked your change against the task and found:\n\n{}\n\nFix these with the smallest change that removes each failure; do not refactor or widen the change otherwise. If you are sure one is wrong, say why in one sentence instead. Run the relevant tests again, then finish with a short summary.",
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
        assert_eq!(parse("```json\n{\"verdict\": \"ok\", \"issues\": []}\n```\n\nI checked `{args[0]}` and \"{\"."), Some((true, vec![])));
        let r = Review { ok: false, issues: vec!["one".into(), "two".into()], cost: 0.0, secs: 0.0, error: None };
        assert!(feedback(&r).contains("1. one\n2. two"));
    }

    #[test]
    fn reviews_through_a_providers_api() {
        use std::io::{BufRead, BufReader};
        let repo = std::env::temp_dir().join(format!("tazuna-review-api-{}", std::process::id()));
        std::fs::create_dir_all(&repo).unwrap();
        let sh = |c: &str| assert!(crate::platform::shell(c, &[]).current_dir(&repo).status().unwrap().success());
        sh("git init -q && echo 'fn a() {}' > lib.rs && git add -A && git -c user.name=t -c user.email=t@t commit -qm base");
        std::fs::write(repo.join("lib.rs"), "fn a() { b() }\nfn b() {}\n").unwrap();
        let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1", server.local_addr().unwrap());
        let got = std::thread::spawn(move || {
            let (c, _) = server.accept().unwrap();
            let mut r = BufReader::new(c.try_clone().unwrap());
            let (mut len, mut line, mut auth) = (0, String::new(), String::new());
            while r.read_line(&mut line).unwrap() > 2 {
                let l = line.to_lowercase();
                if let Some(v) = l.strip_prefix("content-length:") { len = v.trim().parse().unwrap() }
                if l.starts_with("authorization:") { auth = line.trim().to_string() }
                line.clear();
            }
            let mut body = vec![0; len];
            r.read_exact(&mut body).unwrap();
            let reply = serde_json::json!({"output": [{"type": "message", "content": [{"type": "output_text",
                "text": "{\"verdict\": \"fix\", \"issues\": [\"Major: b is dead code. lib.rs:2. Fix: remove it.\"]}"}]}],
                "usage": {"input_tokens": 1000, "input_tokens_details": {"cached_tokens": 400}, "output_tokens": 100}}).to_string();
            let mut c = c;
            write!(c, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}", reply.len()).unwrap();
            (serde_json::from_slice::<Value>(&body).unwrap(), auth)
        });
        // SAFETY: no other test reads this variable
        unsafe { std::env::set_var("TAZUNA_REVIEW_PRICE", "2,0.2,10") };
        let r = run(&repo, "make a call b", &format!("gpt-6-astra@{url}"), "max", Some("rk"));
        let (body, auth) = got.join().unwrap();
        assert_eq!(auth.to_lowercase(), "authorization: bearer rk");
        assert_eq!((body["model"].as_str(), body["reasoning"]["effort"].as_str()), (Some("gpt-6-astra"), Some("max")));
        assert!(body["instructions"].as_str().unwrap().contains("you have no tools") && !body["instructions"].as_str().unwrap().contains("read-only tools"));
        let prompt = body["input"][0]["content"].as_str().unwrap();
        assert!(prompt.contains("make a call b") && prompt.contains("+fn b() {}") && prompt.contains("## lib.rs\n```\nfn a() { b() }"), "{prompt}");
        assert!(!r.ok && r.issues == ["Major: b is dead code. lib.rs:2. Fix: remove it."] && r.error.is_none(), "{:?}", r.error);
        assert!((r.cost - (600.0 * 2.0 + 400.0 * 0.2 + 100.0 * 10.0) / 1e6).abs() < 1e-12, "{}", r.cost);
        std::fs::remove_dir_all(&repo).ok();
    }

    #[test]
    #[cfg(unix)] // symlinks need admin rights on Windows
    fn a_symlink_out_of_the_tree_is_not_read() {
        let base = std::env::temp_dir().join(format!("tazuna-review-link-{}", std::process::id()));
        let repo = base.join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::write(base.join("secret.txt"), "TOPSECRET\n").unwrap();
        let sh = |c: &str| assert!(crate::platform::shell(c, &[]).current_dir(&repo).status().unwrap().success());
        sh("git init -q && echo a > a.txt && git add -A && git -c user.name=t -c user.email=t@t commit -qm base");
        std::os::unix::fs::symlink(base.join("secret.txt"), repo.join("leak.txt")).unwrap();
        std::fs::write(repo.join("a.txt"), "b\n").unwrap();
        let (d, files) = (diff(&repo), changed_files(&repo));
        assert!(!d.contains("TOPSECRET") && !files.contains("TOPSECRET"), "{d}\n{files}");
        assert!(files.contains("## a.txt"), "{files}");
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn diff_includes_new_files() {
        let repo = std::env::temp_dir().join(format!("tazuna-review-{}", std::process::id()));
        std::fs::create_dir_all(&repo).unwrap();
        let sh = |c: &str| assert!(crate::platform::shell(c, &[]).current_dir(&repo).status().unwrap().success());
        sh("git init -q && echo a > old.txt && git add -A && git -c user.name=t -c user.email=t@t commit -qm base");
        std::fs::write(repo.join("old.txt"), "b\n").unwrap();
        std::fs::write(repo.join("new.txt"), "fresh\n").unwrap();
        let d = diff(&repo);
        assert!(d.contains("-a\n+b") && d.contains("+++ b/new.txt\n+fresh"), "{d}");
        std::fs::remove_dir_all(&repo).ok();
    }
}
