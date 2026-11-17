//! Commands and skills on every model. The user's own ones that tazuna keeps (`creations`: ~/.config/tazuna/behave, a
//! Claude Code plugin named "tazuna": commands/<name>.md, skills/<name>/SKILL.md, what /create makes) load everywhere:
//! claude gets the folder with --plugin-dir (its commands then read /tazuna:name; tazuna sends a typed /name as that),
//! tazuna's own loop reads it itself. On the own loop also, as claude would: ~/.claude's when the user's settings load
//! (--user-settings), the repository's .claude ones where its settings load (the repo profiles, a trusted folder), and
//! only files that really live inside their folder. A typed `/name args` expands the command's file; the system prompt
//! lists the skills (name, what for, where SKILL.md is) for the model to read when a task fits.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// "review", or "frontend:test" for commands/frontend/test.md
    pub name: String,
    pub description: String,
    pub path: PathBuf,
}

/// The frontmatter's fields (`key: value`, folded `>`/`|` values joined) and the body after it.
pub(crate) fn frontmatter(text: &str) -> (Vec<(String, String)>, &str) {
    let Some(rest) = text.strip_prefix("---\n").or_else(|| text.strip_prefix("---\r\n")) else { return (vec![], text) };
    let Some(end) = rest.find("\n---") else { return (vec![], text) };
    let (head, tail) = (&rest[..end], &rest[end + 4..]);
    let body = tail.split_once('\n').map_or("", |(_, b)| b);
    let mut fields: Vec<(String, String)> = vec![];
    let mut lines = head.lines().peekable();
    while let Some(l) = lines.next() {
        let Some((k, v)) = l.split_once(':').filter(|(k, _)| !k.starts_with(' ') && !k.is_empty()) else { continue };
        let mut v = v.trim().to_string();
        if matches!(v.as_str(), ">" | ">-" | "|" | "|-" | "") {
            let mut parts = vec![];
            while let Some(n) = lines.peek().filter(|n| n.starts_with(' ') || n.starts_with('\t')) {
                parts.push(n.trim().to_string());
                lines.next();
            }
            v = parts.join(" ");
        }
        fields.push((k.trim().to_string(), v.trim_matches(|c| c == '"' || c == '\'').to_string()));
    }
    (fields, body)
}

pub(crate) fn field(fields: &[(String, String)], key: &str) -> String {
    fields.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone()).unwrap_or_default()
}

/// Where the user's creations live: a Claude Code plugin folder.
pub fn creations() -> Option<PathBuf> {
    crate::platform::home().map(|h| h.join(".config/tazuna/behave"))
}

/// Which folders count: tazuna's creations always, ~/.claude's with the user's settings, the repository's with its own.
#[derive(Clone, Copy)]
pub struct Scope {
    pub user: bool,
    pub project: bool,
}

/// The folders to read, later ones winning on a name: ~/.claude/<kind>, tazuna's creations, the repository's.
fn roots(cwd: &Path, kind: &str, scope: Scope) -> Vec<PathBuf> {
    let mut out = vec![];
    if let (true, Some(h)) = (scope.user, crate::platform::home()) {
        out.push(h.join(".claude").join(kind));
    }
    out.extend(creations().map(|c| c.join(kind)));
    if scope.project {
        out.push(cwd.join(".claude").join(kind));
    }
    out
}

/// The creations folder made ready for claude's --plugin-dir (its manifest written once), when it holds a command or a
/// skill; None otherwise (nothing to load).
pub fn plugin_dir() -> Option<PathBuf> {
    let root = creations()?;
    let any = |kind: &str| std::fs::read_dir(root.join(kind)).is_ok_and(|mut r| r.next().is_some());
    if !any("commands") && !any("skills") {
        return None;
    }
    let manifest = root.join(".claude-plugin/plugin.json");
    if !manifest.exists() {
        std::fs::create_dir_all(manifest.parent()?).ok()?;
        std::fs::write(&manifest, r#"{"name": "tazuna", "version": "1.0.0", "description": "Your commands and skills, made with tazuna's /create"}"#).ok()?;
    }
    Some(root)
}

/// A typed `/name ...` for claude: `/tazuna:name ...` when `name` is one of the creations' commands (claude names plugin
/// commands after their plugin); None otherwise.
pub fn for_claude(typed: &str) -> Option<String> {
    let rest = typed.strip_prefix('/')?;
    let name = rest.split(char::is_whitespace).next()?;
    let file = creations()?.join("commands").join(format!("{}.md", name.replace(':', "/")));
    (!name.is_empty() && !name.contains("..") && file.is_file()).then(|| format!("/tazuna:{rest}"))
}

/// A file under `root` that is really there: inside it after links are resolved (a repository's link to ~/.ssh must not
/// put that folder's files in the prompt), at most 64 KB.
fn inside(root: &Path, p: &Path) -> Option<String> {
    let (r, f) = (root.canonicalize().ok()?, p.canonicalize().ok()?);
    let meta = std::fs::metadata(&f).ok()?;
    (f.starts_with(&r) && meta.is_file() && meta.len() <= 64 * 1024).then(|| std::fs::read_to_string(&f).ok()).flatten()
}

/// The commands (`<root>/name.md`, `<root>/ns/name.md` as "ns:name"): the repository's replace the user's of the same name.
pub fn commands(cwd: &Path, scope: Scope) -> Vec<Item> {
    let mut out: Vec<Item> = vec![];
    for root in roots(cwd, "commands", scope) {
        let mut found = vec![];
        for e in std::fs::read_dir(&root).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                let ns = e.file_name().to_string_lossy().into_owned();
                for f in std::fs::read_dir(&p).into_iter().flatten().flatten() {
                    found.push((format!("{ns}:"), f.path()));
                }
            } else {
                found.push((String::new(), p));
            }
        }
        for (prefix, p) in found {
            let (Some(stem), true) = (p.file_stem().map(|s| s.to_string_lossy().into_owned()), p.extension().is_some_and(|x| x == "md")) else { continue };
            let Some(text) = inside(&root, &p) else { continue };
            let (fields, body) = frontmatter(&text);
            let description = Some(field(&fields, "description")).filter(|d| !d.is_empty())
                .unwrap_or_else(|| body.lines().find(|l| !l.trim().is_empty()).unwrap_or_default().chars().take(80).collect());
            let name = format!("{prefix}{stem}");
            out.retain(|c| c.name != name);
            out.push(Item { name, description, path: p });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// The skills (`<root>/<name>/SKILL.md`): name and description from the frontmatter (the folder's name when it has none).
pub fn skills(cwd: &Path, scope: Scope) -> Vec<Item> {
    let mut out: Vec<Item> = vec![];
    for root in roots(cwd, "skills", scope) {
        for e in std::fs::read_dir(&root).into_iter().flatten().flatten() {
            let p = e.path().join("SKILL.md");
            let Some(text) = inside(&root, &p) else { continue };
            let (fields, _) = frontmatter(&text);
            let name = Some(field(&fields, "name")).filter(|n| !n.is_empty()).unwrap_or_else(|| e.file_name().to_string_lossy().into_owned());
            out.retain(|s| s.name != name);
            out.push(Item { name, description: field(&fields, "description"), path: p });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// A typed `/name args` for the own loop: the command's text with `$ARGUMENTS` and `$1`..`$9` filled in (Claude Code's
/// form); the arguments after it when the text uses none. None: no such command.
pub fn expand(cwd: &Path, scope: Scope, typed: &str) -> Option<String> {
    let rest = typed.strip_prefix('/')?;
    let (name, args) = rest.split_once(char::is_whitespace).map_or((rest, ""), |(n, a)| (n, a.trim()));
    let name = name.strip_prefix("tazuna:").unwrap_or(name);
    let c = commands(cwd, scope).into_iter().find(|c| c.name == name)?;
    let root = c.path.parent()?.to_path_buf();
    let text = inside(&root, &c.path)?;
    let (_, body) = frontmatter(&text);
    let words: Vec<&str> = args.split_whitespace().collect();
    let mut out = body.trim_end().replace("$ARGUMENTS", args);
    for i in (1..=9).rev() {
        out = out.replace(&format!("${i}"), words.get(i - 1).copied().unwrap_or(""));
    }
    if !args.is_empty() && !body.contains("$ARGUMENTS") && !body.contains("$1") {
        out.push_str(&format!("\n\n{args}"));
    }
    Some(out.trim().to_string())
}

/// The skills as the own loop's system prompt lists them; empty when there are none.
pub fn skills_prompt(skills: &[Item]) -> String {
    if skills.is_empty() {
        return String::new();
    }
    let list: Vec<String> = skills.iter().map(|s| format!("- {}: {} ({})", s.name, s.description.chars().take(300).collect::<String>(), s.path.display())).collect();
    format!("\n\nSkills: instructions for kinds of tasks. When a task fits one, read its SKILL.md with your read tool first and follow it.\n{}", list.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_and_skills_read_as_claude_code_writes_them() {
        let dir = std::env::temp_dir().join(format!("tazuna-behave-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (cmds, skills_dir) = (dir.join(".claude/commands"), dir.join(".claude/skills"));
        std::fs::create_dir_all(cmds.join("fe")).unwrap();
        std::fs::create_dir_all(skills_dir.join("release")).unwrap();
        std::fs::write(cmds.join("fix-issue.md"), "---\ndescription: Fix a GitHub issue\nargument-hint: <number>\n---\nFix issue #$1 in this repository. Notes: $ARGUMENTS\n").unwrap();
        std::fs::write(cmds.join("fe/test.md"), "Run the frontend tests and fix what fails.\n").unwrap();
        std::fs::write(skills_dir.join("release/SKILL.md"), "---\nname: release\ndescription: >-\n  Cut a release:\n  bump, tag, push.\n---\n# Release\n").unwrap();
        let (all, none) = (Scope { user: false, project: true }, Scope { user: false, project: false });
        let c = commands(&dir, all);
        let mine: Vec<&Item> = c.iter().filter(|i| i.path.starts_with(&dir)).collect();
        assert_eq!(mine.iter().map(|i| (i.name.as_str(), i.description.as_str())).collect::<Vec<_>>(),
            [("fe:test", "Run the frontend tests and fix what fails."), ("fix-issue", "Fix a GitHub issue")]);
        assert!(!commands(&dir, none).iter().any(|i| i.path.starts_with(&dir)), "the repository's only where its settings load");
        assert_eq!(expand(&dir, all, "/fix-issue 42 the login one").unwrap(), "Fix issue #42 in this repository. Notes: 42 the login one");
        assert_eq!(expand(&dir, all, "/fe:test only unit").unwrap(), "Run the frontend tests and fix what fails.\n\nonly unit");
        assert_eq!(expand(&dir, all, "/nope"), None);
        let s = skills(&dir, all);
        let r = s.iter().find(|i| i.name == "release").unwrap();
        assert_eq!(r.description, "Cut a release: bump, tag, push.");
        assert!(skills_prompt(&[r.clone()]).contains("- release: Cut a release: bump, tag, push. ("));
        // a link out of the repository is not read
        #[cfg(unix)]
        {
            std::fs::write(dir.join("outside.md"), "secret\n").unwrap();
            std::os::unix::fs::symlink(dir.join("outside.md"), cmds.join("leak.md")).unwrap();
            assert_eq!(expand(&dir, all, "/leak"), None);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
