//! Rules router v1: picks model and effort per session from cheap signals, before any model call.
//!
//! Evidence (cc-lean sweeps, cc-mid profile): on tiny repos Sonnet 5.5 medium matched Opus 5.5 medium on easy and medium
//! tasks at about a third of the cost but was weaker on hard ones; on real tickets in large repos Opus medium was the best
//! value. No cheap text signal separated hard from medium tasks, so the cheap lane relies on escalation: when verification
//! fails there, the fix rounds run on Opus high.
//! ponytail: one size threshold and two lanes; replace with Jev once the run log has labelled outcomes

use std::{path::Path, process::Command};

pub struct Lane {
    pub name: &'static str,
    pub model: String,
    pub effort: String,
    pub reason: String,
    /// Model and effort for the fix rounds after failed verification (None: stay on the lane).
    pub escalate: Option<(String, String)>,
}

/// Repos with at most this many tracked files count as tiny.
const TINY_FILES: usize = 50;

pub fn choose(cwd: &Path) -> Lane {
    let files = Command::new("git")
        .args(["ls-files"])
        .current_dir(cwd)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| o.stdout.split(|b| *b == b'\n').filter(|l| !l.is_empty()).count());
    match files {
        Some(n) if n <= TINY_FILES => Lane {
            name: "small",
            model: "claude-sonnet-5-5".into(),
            effort: "medium".into(),
            reason: format!("tiny repo, {n} tracked file{}", if n == 1 { "" } else { "s" }),
            escalate: Some(("claude-opus-5-5".into(), "high".into())),
        },
        _ => Lane {
            name: "default",
            model: "claude-opus-5-5".into(),
            effort: "medium".into(),
            reason: files.map_or("not a git repo".into(), |n| format!("{n} tracked files")),
            escalate: None,
        },
    }
}

/// Explicit `--model`/`--effort` turn routing off.
pub fn manual(model: &str, effort: &str) -> Lane {
    Lane { name: "manual", model: model.into(), effort: effort.into(), reason: "set on the command line".into(), escalate: None }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiny_repos_get_the_cheap_lane_with_escalation() {
        let dir = std::env::temp_dir().join(format!("tazuna-route-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let git = |args: &[&str]| assert!(Command::new("git").args(args).current_dir(&dir).output().unwrap().status.success());
        git(&["init", "-q"]);
        std::fs::write(dir.join("a.py"), "x = 1\n").unwrap();
        git(&["add", "a.py"]);
        let lane = choose(&dir);
        assert_eq!((lane.name, lane.model.as_str(), lane.effort.as_str()), ("small", "claude-sonnet-5-5", "medium"));
        assert!(lane.escalate.is_some() && lane.reason.contains("1 tracked file"));
        for i in 0..60 {
            std::fs::write(dir.join(format!("f{i}.txt")), "x").unwrap();
        }
        git(&["add", "."]);
        assert_eq!(choose(&dir).name, "default");
        std::fs::remove_dir_all(&dir).ok();
    }
}
