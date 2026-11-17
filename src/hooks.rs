//! A hook: the user's own shell command, run after each turn that changed files (a formatter, a linter, a sync), in the
//! project's folder and at no token cost: the model never sees it, and only a failure shows, as a notice. One per
//! project, set on the desktop's settings page or by hand in ~/.config/tazuna/hooks.json
//! (`{"/path/to/project": {"after_turn": "cargo fmt"}}`). It lives in the user's config, never the repository's: a
//! repository cannot make tazuna run anything.

use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn file() -> Option<PathBuf> {
    crate::platform::home().map(|h| h.join(".config/tazuna/hooks.json"))
}

fn key(cwd: &Path) -> String {
    cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf()).to_string_lossy().into_owned()
}

/// The command to run after a turn that changed files in `cwd`, if the user set one.
pub fn after_turn(cwd: &Path) -> Option<String> {
    let conf: Value = serde_json::from_str(&std::fs::read_to_string(file()?).ok()?).ok()?;
    conf[key(cwd)]["after_turn"].as_str().map(str::trim).filter(|c| !c.is_empty()).map(String::from)
}

/// Sets `cwd`'s command (empty: none).
pub fn set_after_turn(cwd: &Path, cmd: &str) -> Result<()> {
    let path = file().context("no home folder")?;
    let mut conf: Value = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| json!({}));
    let cmd = cmd.trim();
    if cmd.is_empty() {
        conf.as_object_mut().map(|m| m.shift_remove(&key(cwd)));
    } else {
        conf[key(cwd)] = json!({"after_turn": cmd});
    }
    std::fs::create_dir_all(path.parent().context("no config folder")?)?;
    std::fs::write(&path, serde_json::to_string_pretty(&conf)?)?;
    Ok(())
}

/// Runs `cmd` in `cwd` on a thread of its own; its outcome comes back on `tx` as `{"type": "tazuna_hook", "ok", "text"}`
/// (text: a line on success, the output's tail on a failure).
pub fn spawn(cwd: PathBuf, cmd: String, tx: std::sync::mpsc::Sender<Value>) {
    std::thread::spawn(move || {
        let o = crate::verify::run(&cwd, &cmd);
        let text = if o.ok {
            format!("hook `{cmd}` ran ({:.1} s)", o.secs)
        } else {
            let tail: Vec<&str> = o.output.lines().rev().take(12).collect::<Vec<_>>().into_iter().rev().collect();
            format!("hook `{cmd}` failed{}:\n{}", if o.timed_out { " (timed out)" } else { "" }, tail.join("\n"))
        };
        let _ = tx.send(json!({"type": "tazuna_hook", "ok": o.ok, "text": text}));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hook_is_set_per_project_and_runs_its_command() {
        let dir = std::env::temp_dir().join(format!("tazuna-hook-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        spawn(dir.clone(), "echo formatted > out.txt".into(), tx.clone());
        let v = rx.recv().unwrap();
        assert_eq!(v["ok"], true, "{v}");
        assert_eq!(std::fs::read_to_string(dir.join("out.txt")).unwrap().trim(), "formatted", "in the project's folder");
        spawn(dir.clone(), "echo nope; exit 3".into(), tx);
        let v = rx.recv().unwrap();
        assert!(v["ok"] == false && v["text"].as_str().unwrap().contains("nope"), "{v}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
