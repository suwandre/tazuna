//! Compiler errors right after an edit (`--diagnostics`). tazuna adds a PostToolUse hook (`tazuna diag <state dir>`) to the
//! claude it starts. After any tool call that changed files (Edit, Write, or a shell command that rewrote them: agents do
//! most edits with python/sed through Bash), the hook runs the repo's type check (TypeScript) or `go vet` on the changed
//! Go packages and tells claude about errors the change added, while the edit is still fresh. Errors the repo already had
//! when the session started do not count.

use crate::{runlog, verify};
use anyhow::Result;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
};

/// Tool calls that can change files, in every profile.
const MATCHER: &str = "Edit|Write|MultiEdit|NotebookEdit|Bash|mcp__lean__edit|mcp__lean__write|mcp__lean__bash";
const MAX_SHOWN: usize = 30;

/// The session's starting point: the working tree snapshot now, and the type check output (written when it finishes).
pub fn prepare(cwd: &Path) -> Result<PathBuf> {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos();
    let dir = std::env::temp_dir().join(format!("tazuna-diag-{}-{nanos}", std::process::id()));
    crate::platform::private_dir(&dir, false)?;
    write_snapshot(&dir, &runlog::git_snapshot(cwd).unwrap_or_default())?;
    if let Some(cmd) = verify::typecheck_command(&verify::Mode::Auto, cwd) {
        let (cwd, dir) = (cwd.to_path_buf(), dir.clone());
        std::thread::spawn(move || {
            let out = verify::run_raw(&cwd, &cmd).1;
            // rename: the hook never reads a half-written baseline
            let _ = std::fs::write(dir.join("baseline.tmp"), out).and_then(|_| std::fs::rename(dir.join("baseline.tmp"), dir.join("baseline.txt")));
        });
    }
    Ok(dir)
}

/// The `--settings` value that installs the hook.
pub fn settings(exe: &Path, dir: &Path) -> String {
    let q = crate::platform::sh_quote;
    json!({"hooks": {"PostToolUse": [{"matcher": MATCHER, "hooks": [{"type": "command", "command": format!("{} diag {}", q(exe), q(dir)), "timeout": 600}]}]}})
        .to_string()
}

/// The hook itself. Prints claude's hook JSON when the last tool call added errors; silent otherwise.
pub fn hook(dir: &Path) -> Result<()> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let input: Value = serde_json::from_str(&input).unwrap_or(Value::Null);
    let cwd = input["cwd"].as_str().map(PathBuf::from).map_or_else(std::env::current_dir, Ok)?;
    // parallel tool calls: one check at a time, the others skip (the running one sees their changes too)
    let lock = dir.join("lock");
    if std::fs::OpenOptions::new().write(true).create_new(true).open(&lock).is_err() {
        return Ok(());
    }
    let out = check(dir, &cwd);
    let _ = std::fs::remove_file(&lock);
    if let Some(text) = out? {
        println!("{}", json!({"hookSpecificOutput": {"hookEventName": "PostToolUse", "additionalContext": text}}));
    }
    Ok(())
}

fn check(dir: &Path, cwd: &Path) -> Result<Option<String>> {
    let before: BTreeMap<String, String> = std::fs::read_to_string(dir.join("snapshot.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let Some(now) = runlog::git_snapshot(cwd) else { return Ok(None) };
    let changed: Vec<String> = runlog::changed(&before, &now).into_iter().cloned().collect();
    if changed.is_empty() {
        return Ok(None);
    }
    write_snapshot(dir, &now)?;
    let mut found: Vec<(String, Vec<String>)> = Vec::new();
    if verify::touches_types(&changed)
        && let Some(cmd) = verify::typecheck_command(&verify::Mode::Auto, cwd)
        && let Ok(baseline) = std::fs::read_to_string(dir.join("baseline.txt"))
    {
        let errors = verify::new_type_errors(&baseline, &verify::run_raw(cwd, &cmd).1);
        found.push((cmd, errors));
    }
    let pkgs = go_packages(cwd, &changed);
    if !pkgs.is_empty() {
        let cmd = format!("go vet {}", pkgs.join(" "));
        // a Go tree compiles before the agent starts, so every error in a file it touched is its own
        let errors = verify::run_raw(cwd, &cmd).1.lines().filter(|l| changed.iter().any(|f| f.ends_with(".go") && l.trim_start_matches("./").starts_with(&format!("{f}:")))).map(String::from).collect();
        found.push((cmd, errors));
    }
    let errors: Vec<String> = found.iter().flat_map(|(_, e)| e.iter().cloned()).collect();
    // say it once: the same errors after an unrelated edit are not news
    let last = std::fs::read_to_string(dir.join("reported.txt")).unwrap_or_default();
    let now_text = errors.join("\n");
    std::fs::write(dir.join("reported.txt"), &now_text)?;
    if errors.is_empty() || now_text == last {
        return Ok(None);
    }
    let cmds: Vec<&str> = found.iter().filter(|(_, e)| !e.is_empty()).map(|(c, _)| c.as_str()).collect();
    let shown: Vec<&str> = errors.iter().take(MAX_SHOWN).map(String::as_str).collect();
    let more = if errors.len() > MAX_SHOWN { format!("\n[... {} more]", errors.len() - MAX_SHOWN) } else { String::new() };
    Ok(Some(format!(
        "tazuna: your changes so far add {} type/compile error(s) (`{}`; errors the repo already had are left out):\n{}{more}\nFix them before you continue.",
        errors.len(),
        cmds.join("`, `"),
        shown.join("\n")
    )))
}

fn go_packages(cwd: &Path, changed: &[String]) -> Vec<String> {
    let mut pkgs: Vec<String> = changed
        .iter()
        .filter(|f| f.ends_with(".go") && !f.starts_with("vendor/") && verify::safe_path(f) && cwd.join(f).exists())
        .map(|f| Path::new(f).parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default())
        .map(|d| if d.is_empty() { "./".to_string() } else { format!("./{d}/") })
        .collect();
    pkgs.sort();
    pkgs.dedup();
    pkgs
}

fn write_snapshot(dir: &Path, snap: &BTreeMap<String, String>) -> Result<()> {
    std::fs::write(dir.join("snapshot.json"), serde_json::to_string(snap)?)?;
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;

    fn sh(dir: &Path, cmd: &str) {
        assert!(crate::platform::shell(cmd, &[]).current_dir(dir).status().unwrap().success(), "{cmd}");
    }

    #[test]
    fn reports_only_new_errors_once() {
        let repo = std::env::temp_dir().join(format!("tazuna-diag-test-{}", std::process::id()));
        std::fs::create_dir_all(&repo).unwrap();
        // a fake type check: prints a known error, plus one per line of new.ts (node: npm runs scripts in cmd on Windows)
        std::fs::write(repo.join("package.json"), r#"{"scripts": {"typecheck": "node check.js"}}"#).unwrap();
        std::fs::write(repo.join("check.js"), "const fs = require('fs');\nconsole.log('old.ts(1,1): error TS1: known');\n\
            if (fs.existsSync('new.ts')) for (const l of fs.readFileSync('new.ts', 'utf8').split('\\n').filter(Boolean)) console.log('new.ts(1,1): error TS2: ' + l);\n\
            process.exit(2);\n").unwrap();
        sh(&repo, "git init -q && git add -A && git -c user.name=t -c user.email=t@t commit -qm base");
        let dir = prepare(&repo).unwrap();
        for _ in 0..100 {
            if dir.join("baseline.txt").exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert_eq!(check(&dir, &repo).unwrap(), None, "nothing changed yet");
        std::fs::write(repo.join("readme.md"), "x").unwrap();
        assert_eq!(check(&dir, &repo).unwrap(), None, "a change that cannot affect types");
        std::fs::write(repo.join("new.ts"), "bad thing\n").unwrap();
        let msg = check(&dir, &repo).unwrap().expect("the new error is reported");
        assert!(msg.contains("1 type/compile error") && msg.contains("new.ts(1,1): error TS2: bad thing") && !msg.contains("known"), "{msg}");
        std::fs::write(repo.join("new.ts"), "bad thing\n").unwrap();
        std::fs::write(repo.join("other.ts"), "x").unwrap();
        assert_eq!(check(&dir, &repo).unwrap(), None, "the same errors are not repeated");
        std::fs::remove_file(repo.join("new.ts")).unwrap();
        assert_eq!(check(&dir, &repo).unwrap(), None, "fixed: nothing to say");
        let s: Value = serde_json::from_str(&settings(Path::new("/x/it's/tazuna"), &dir)).unwrap();
        assert!(s["hooks"]["PostToolUse"][0]["hooks"][0]["command"].as_str().unwrap().starts_with(r"'/x/it'\''s/tazuna' diag '"));
        std::fs::remove_dir_all(&repo).ok();
        std::fs::remove_dir_all(&dir).ok();
    }
}
