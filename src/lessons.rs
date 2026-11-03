//! `tazuna lessons`: what this repository's reviewers keep raising, from the review comments on its merged pull requests
//! (GitHub, through `gh`), grouped by one `claude -p` call into a short checklist at `.tazuna/review-lessons.md`. With
//! `--lessons` the agent gets that list with its instructions. Nothing in here is about one repository: any repo with a
//! review history on GitHub gets its own list, one without gets none. A first draft repeats most of what reviewers later
//! find (company tickets, 2026-11-03: 76% of the applicable findings); this is the repo's own memory of those findings.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

const FILE: &str = ".tazuna/review-lessons.md";
/// What goes into the instructions at most: the list is meant to be short; a longer file is cut, not dropped.
const MAX_CHARS: usize = 12_000;
const COMMENT_CHARS: usize = 600;
const PROMPT_CHARS: usize = 300_000;

const SYSTEM: &str = "You turn a repository's code review comments into a checklist for the engineer who writes the next change. \
Group comments that make the same point in different pull requests. Keep only points raised in at least two different pull \
requests. For each point write one line on what goes wrong, one line on how to check a new change for it, and the pull \
request numbers it came from. At most 20 points, the most frequent first. Generic advice (add tests, naming, formatting) only \
if this repository's reviewers ask for it again and again. No names of people. Answer with the markdown list only.";

/// One review comment worth keeping: long enough to say something, not a nit or an approval, not from a bot.
fn keep(body: &str, author: &str) -> bool {
    let b = body.trim().to_lowercase();
    !author.ends_with("[bot]") && b.chars().count() >= 40 && !b.starts_with("nit") && !b.starts_with("lgtm") && !b.starts_with("looks good")
}

/// The comments of one search page of pull requests, as "#123 path: text" lines.
fn comments(page: &Value) -> Vec<String> {
    let mut out = vec![];
    for pr in page["data"]["search"]["nodes"].as_array().into_iter().flatten() {
        let n = pr["number"].as_u64().unwrap_or(0);
        for t in pr["reviewThreads"]["nodes"].as_array().into_iter().flatten() {
            for c in t["comments"]["nodes"].as_array().into_iter().flatten() {
                let (body, who) = (c["body"].as_str().unwrap_or_default(), c["author"]["login"].as_str().unwrap_or_default());
                if keep(body, who) {
                    out.push(format!("#{n} {}: {}", t["path"].as_str().unwrap_or("-"), body.trim().chars().take(COMMENT_CHARS).collect::<String>()));
                }
            }
        }
        for r in pr["reviews"]["nodes"].as_array().into_iter().flatten() {
            let (body, who) = (r["body"].as_str().unwrap_or_default(), r["author"]["login"].as_str().unwrap_or_default());
            if keep(body, who) {
                out.push(format!("#{n} (review): {}", body.trim().chars().take(COMMENT_CHARS).collect::<String>()));
            }
        }
    }
    out
}

fn gh(args: &[&str], cwd: &Path) -> Result<String> {
    let o = Command::new("gh").args(args).current_dir(cwd).stdin(Stdio::null()).output().context("running gh (GitHub CLI)")?;
    if !o.status.success() {
        bail!("gh {}: {}", args.first().unwrap_or(&""), String::from_utf8_lossy(&o.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
}

const QUERY: &str = "query($q: String!, $cursor: String) { search(query: $q, type: ISSUE, first: 25, after: $cursor) {
  pageInfo { hasNextPage endCursor }
  nodes { ... on PullRequest { number
    reviewThreads(first: 40) { nodes { path comments(first: 1) { nodes { body author { login } } } } }
    reviews(first: 20) { nodes { body author { login } } } } } } }";

/// `tazuna lessons [--prs N] [--before YYYY-MM-DD] [--model M] [--effort E] [--repo OWNER/NAME] [--out FILE]`, in the
/// repository (or for `--repo`, written to `--out`: a benchmark's copy has no GitHub remote).
pub fn cli(args: &[String]) -> Result<()> {
    let val = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    let prs: usize = val("--prs").map(|v| v.parse()).transpose().context("--prs takes a number")?.unwrap_or(150);
    let before = val("--before");
    let model = val("--model").unwrap_or_else(|| "claude-sonnet-5-5".into());
    let effort = val("--effort").unwrap_or_else(|| "medium".into());
    let cwd = std::env::current_dir()?;
    let repo = match val("--repo") {
        Some(r) => r,
        None => gh(&["repo", "view", "--json", "nameWithOwner", "-q", ".nameWithOwner"], &cwd)?.trim().to_string(),
    };
    // only pull requests merged before a date: a benchmark task must not learn from its own review
    let q = format!("repo:{repo} is:pr is:merged sort:created-desc{}", before.as_deref().map(|d| format!(" merged:<{d}")).unwrap_or_default());
    let (mut lines, mut seen, mut cursor) = (vec![], 0, None::<String>);
    while seen < prs {
        let mut a = vec!["api", "graphql", "-f", &*QUERY_ARG, "-f"];
        let qa = format!("q={q}");
        a.push(&qa);
        let ca;
        if let Some(c) = &cursor {
            ca = format!("cursor={c}");
            a.extend(["-f", &ca]);
        }
        let page: Value = serde_json::from_str(&gh(&a, &cwd)?)?;
        seen += page["data"]["search"]["nodes"].as_array().map_or(0, |n| n.len());
        lines.extend(comments(&page));
        let info = &page["data"]["search"]["pageInfo"];
        match (info["hasNextPage"].as_bool(), info["endCursor"].as_str()) {
            (Some(true), Some(c)) => cursor = Some(c.to_string()),
            _ => break,
        }
    }
    if lines.is_empty() {
        bail!("{repo}: no review comments in its last {seen} merged pull requests; nothing to learn from");
    }
    let mut prompt = format!("Review comments from the last {seen} merged pull requests of {repo}, one per line (#PR path: text):\n\n");
    for l in &lines {
        if prompt.len() + l.len() > PROMPT_CHARS {
            break;
        }
        prompt.push_str(&l.replace('\n', " "));
        prompt.push('\n');
    }
    eprintln!("{repo}: {} review comments from {seen} pull requests; asking {model} for the checklist", lines.len());
    let list = ask(&cwd, &prompt, &model, &effort)?;
    let path = val("--out").map(PathBuf::from).unwrap_or_else(|| git_top(&cwd).unwrap_or(cwd).join(FILE));
    std::fs::create_dir_all(path.parent().expect("has a parent"))?;
    let head = format!(
        "# What this repository's reviewers keep raising\n\nWritten by `tazuna lessons` on {} from {} review comments in the last {seen} merged pull requests{}. Edit freely; run it again to refresh.\n\n",
        crate::platform::shell("date +%Y-%m-%d", &[]).output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default(),
        lines.len(),
        before.map(|d| format!(" merged before {d}")).unwrap_or_default()
    );
    std::fs::write(&path, format!("{head}{}\n", list.trim()))?;
    println!("{}", path.display());
    Ok(())
}

// the query as one -f argument for gh
static QUERY_ARG: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| format!("query={QUERY}"));

fn ask(cwd: &Path, prompt: &str, model: &str, effort: &str) -> Result<String> {
    let mut child = Command::new("claude")
        // on stdin: a few hundred kilobytes do not belong on a command line
        .args(["-p", "--model", model, "--effort", effort, "--output-format", "json", "--no-session-persistence"])
        .args(["--setting-sources", "", "--strict-mcp-config", "--disable-slash-commands", "--tools", "", "--system-prompt", SYSTEM])
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("starting claude")?;
    let mut stdin = child.stdin.take().expect("piped");
    let p = prompt.to_string();
    let w = std::thread::spawn(move || stdin.write_all(p.as_bytes()));
    let mut out = String::new();
    child.stdout.take().expect("piped").read_to_string(&mut out)?;
    let _ = w.join();
    child.wait()?;
    let d: Value = out.lines().rev().find_map(|l| serde_json::from_str(l).ok()).context("no answer from claude")?;
    if d["is_error"] == json!(true) {
        bail!("claude: {}", d["result"].as_str().unwrap_or_default());
    }
    Ok(d["result"].as_str().unwrap_or_default().to_string())
}

fn git_top(cwd: &Path) -> Option<PathBuf> {
    let o = Command::new("git").args(["rev-parse", "--show-toplevel"]).current_dir(cwd).output().ok()?;
    o.status.success().then(|| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()))
}

/// The repository's list for the agent's instructions, when there is one.
pub fn load(cwd: &Path) -> Option<String> {
    let text = std::fs::read_to_string(git_top(cwd).unwrap_or_else(|| cwd.to_path_buf()).join(FILE)).ok()?;
    let text: String = text.chars().take(MAX_CHARS).collect();
    (!text.trim().is_empty()).then(|| {
        format!("This repository's reviewers have raised the points below again and again. Check your change against each one that applies before you finish.\n\n{text}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_substantive_human_comments_only() {
        let page = json!({"data": {"search": {"nodes": [{"number": 7,
            "reviewThreads": {"nodes": [
                {"path": "a.ts", "comments": {"nodes": [{"body": "This retries the Ledger call but the batch was already removed from Redis.", "author": {"login": "ana"}}]}},
                {"path": "b.ts", "comments": {"nodes": [{"body": "nit: rename this variable to something clearer please", "author": {"login": "ana"}}]}},
                {"path": "c.ts", "comments": {"nodes": [{"body": "Automated: consider handling the error case here somehow.", "author": {"login": "review-bot[bot]"}}]}},
                {"path": "d.ts", "comments": {"nodes": [{"body": "ok", "author": {"login": "bo"}}]}}]},
            "reviews": {"nodes": [{"body": "LGTM, nice work on the migration and the tests here", "author": {"login": "bo"}},
                                  {"body": "The cleanup removes every duplicate on the step, not only stale ones.", "author": {"login": "bo"}}]}}]}}});
        assert_eq!(
            comments(&page),
            ["#7 a.ts: This retries the Ledger call but the batch was already removed from Redis.", "#7 (review): The cleanup removes every duplicate on the step, not only stale ones."]
        );
    }
}
