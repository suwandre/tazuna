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
    pub profile: Profile,
}

/// How claude runs.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Profile {
    /// Claude Code's own prompt and tools with the repo's settings, hooks, skills and CLAUDE.md (not the user's personal
    /// ~/.claude setup). For repos that carry agent config: their rules exist for a reason.
    Repo,
    /// tazuna's cc-mid profile: ~10-line prompt, Grep/Glob plus four lean tools, no settings. About 30% cheaper.
    Lean,
}

impl Profile {
    pub fn name(self) -> &'static str {
        match self {
            Profile::Repo => "repo",
            Profile::Lean => "lean",
        }
    }
}

/// What agent config a repo carries.
#[derive(Default, Debug)]
pub struct RepoConfig {
    pub instructions: Vec<String>,
    pub settings: bool,
    pub hooks: bool,
    pub skills: usize,
    pub commands: usize,
    pub agents: usize,
    pub mcp: bool,
    /// Anything else under a `.claude/` directory (hook scripts, output styles, ...): unknown, so it needs trust.
    pub other: usize,
}

impl RepoConfig {
    /// Config in `cwd` and every parent up to the git root (Claude Code also picks config up from parent directories).
    pub fn read(cwd: &Path) -> Self {
        let top = Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .current_dir(cwd)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| std::path::PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()));
        let start = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
        let mut c = RepoConfig::default();
        for dir in start.ancestors() {
            c.add(dir, dir == start);
            if top.as_deref().is_none_or(|t| dir == t) {
                break;
            }
        }
        c
    }

    fn add(&mut self, dir: &Path, here: bool) {
        let file = |p: &str| dir.join(p).is_file();
        let count = |d: &str| std::fs::read_dir(dir.join(d)).map(|r| r.flatten().count()).unwrap_or(0);
        let settings_text: String =
            [".claude/settings.json", ".claude/settings.local.json"].iter().filter_map(|f| std::fs::read_to_string(dir.join(f)).ok()).collect();
        for f in ["CLAUDE.md", ".claude/CLAUDE.md", "AGENTS.md", "CLAUDE.local.md"].iter().filter(|f| file(f)) {
            self.instructions.push(if here { f.to_string() } else { dir.join(f).to_string_lossy().into_owned() });
        }
        self.settings |= dir.join(".claude/settings.json").exists() || dir.join(".claude/settings.local.json").exists();
        self.hooks |= settings_text.contains("\"hooks\"");
        self.skills += count(".claude/skills");
        self.commands += count(".claude/commands");
        self.agents += count(".claude/agents");
        self.mcp |= file(".mcp.json");
        let known = ["CLAUDE.md", "settings.json", "settings.local.json", "skills", "commands", "agents"];
        self.other += std::fs::read_dir(dir.join(".claude"))
            .map(|r| r.flatten().filter(|e| !known.contains(&e.file_name().to_string_lossy().as_ref())).count())
            .unwrap_or(0);
    }

    pub fn any(&self) -> bool {
        !self.instructions.is_empty() || self.runs_code()
    }

    /// Fail closed: any settings file can run code (hooks, apiKeyHelper, statusLine, env, plugins, ...), and so can skills
    /// (they ship scripts), commands, subagent definitions and MCP servers. Only plain instruction files are inert.
    pub fn runs_code(&self) -> bool {
        self.settings || self.skills > 0 || self.commands > 0 || self.agents > 0 || self.mcp || self.other > 0
    }

    pub fn summary(&self) -> String {
        let mut parts: Vec<String> = self.instructions.clone();
        if self.hooks {
            parts.push("hooks".into());
        } else if self.settings {
            parts.push("settings".into());
        }
        if self.skills > 0 {
            parts.push(format!("{} skills", self.skills));
        }
        if self.commands > 0 {
            parts.push(format!("{} commands", self.commands));
        }
        if self.agents > 0 {
            parts.push(format!("{} subagents", self.agents));
        }
        if self.mcp {
            parts.push("MCP servers".into());
        }
        if self.other > 0 {
            parts.push(format!("{} other .claude entries", self.other));
        }
        parts.join(", ")
    }
}

/// Repos with agent config get the repo profile; the rest run lean (equal quality on config-less repos, cheaper).
pub fn profile_for(cwd: &Path) -> (Profile, String) {
    let c = RepoConfig::read(cwd);
    if c.any() { (Profile::Repo, format!("repo config: {}", c.summary())) } else { (Profile::Lean, "no repo config".into()) }
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
            profile: Profile::Lean,
        },
        _ => {
            let (profile, why) = profile_for(cwd);
            Lane {
                name: "default",
                model: "claude-opus-5-5".into(),
                effort: "medium".into(),
                reason: format!("{}; {why}", files.map_or("not a git repo".into(), |n| format!("{n} tracked files"))),
                escalate: None,
                profile,
            }
        }
    }
}

/// Explicit `--model`/`--effort` turn model routing off; the profile still follows the repo's config.
pub fn manual(cwd: &Path, model: &str, effort: &str) -> Lane {
    let (profile, why) = profile_for(cwd);
    Lane { name: "manual", model: model.into(), effort: effort.into(), reason: format!("set on the command line; {why}"), escalate: None, profile }
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
        let lane = choose(&dir);
        assert_eq!((lane.name, lane.profile), ("default", Profile::Lean), "no repo config: lean");
        std::fs::create_dir_all(dir.join(".claude/skills/review")).unwrap();
        std::fs::write(dir.join(".claude/settings.json"), r#"{"hooks": {}}"#).unwrap();
        std::fs::write(dir.join("CLAUDE.md"), "rules").unwrap();
        let c = RepoConfig::read(&dir);
        assert!(c.hooks && c.skills == 1 && c.runs_code() && c.summary() == "CLAUDE.md, hooks, 1 skills", "{c:?}");
        assert_eq!(choose(&dir).profile, Profile::Repo, "repo config is respected");
        std::fs::write(dir.join(".claude/settings.json"), r#"{"statusLine": {"type": "command", "command": "evil"}}"#).unwrap();
        std::fs::remove_dir_all(dir.join(".claude/skills")).unwrap();
        assert!(RepoConfig::read(&dir).runs_code(), "any settings file needs trust, not just ones with hooks");
        std::fs::remove_file(dir.join(".claude/settings.json")).unwrap();
        assert!(!RepoConfig::read(&dir).runs_code(), "a CLAUDE.md alone runs nothing");
        std::fs::create_dir_all(dir.join(".claude/hooks")).unwrap();
        assert!(RepoConfig::read(&dir).runs_code(), "unknown .claude entries (hook scripts) need trust");
        std::fs::remove_dir_all(dir.join(".claude/hooks")).unwrap();
        std::fs::write(dir.join(".claude/settings.json"), "{}").unwrap();
        std::fs::create_dir_all(dir.join("pkg/sub")).unwrap();
        assert!(RepoConfig::read(&dir.join("pkg/sub")).runs_code(), "config in a parent directory of the repo counts");
        std::fs::remove_dir_all(&dir).ok();
    }
}
