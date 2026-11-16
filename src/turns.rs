//! What each turn changed in a project, and its undo (the desktop app's change card, 2026-11-16).
//!
//! Before and after a turn the work tree is written as a git tree (a private index file seeded from the repo's, `git add
//! -A`, `git write-tree`): ignored files stay out, nothing is staged or committed, and the user's index is untouched. The
//! two trees give the turn's files and line counts and each file's diff, whichever engine made the change (an edit tool
//! or a shell command). Undo writes the files back from the tree before, only while they are still as the turn left them.

use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

fn git(cwd: &Path, args: &[&str], index: Option<&Path>) -> Result<String> {
    let mut c = Command::new("git");
    c.args(args).current_dir(cwd);
    if let Some(i) = index {
        c.env("GIT_INDEX_FILE", i);
    }
    let o = c.output().context("running git")?;
    if !o.status.success() {
        bail!("git {}: {}", args.join(" "), String::from_utf8_lossy(&o.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
}

/// The work tree as a git tree id; None outside a git repo. Writes blob and tree objects to the repo (unreferenced: git's
/// gc drops them in time) and nothing else.
pub fn snapshot(cwd: &Path) -> Option<String> {
    let real = git(cwd, &["rev-parse", "--path-format=absolute", "--git-path", "index"], None).ok()?;
    let index = std::env::temp_dir().join(format!("tazuna-index-{}", std::process::id()));
    // seeded from the repo's index, `add -A` hashes only the files whose stat changed
    if std::fs::copy(real.trim(), &index).is_err() {
        let _ = std::fs::remove_file(&index);
    }
    let tree = git(cwd, &["add", "-A"], Some(&index)).and_then(|_| git(cwd, &["write-tree"], Some(&index))).ok();
    let _ = std::fs::remove_file(&index);
    tree.map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
}

/// One file a turn changed: its path, lines added and removed (None for a binary file), and A, M or D.
#[derive(Clone, Debug, PartialEq)]
pub struct File {
    pub path: String,
    pub added: Option<u64>,
    pub removed: Option<u64>,
    pub status: char,
}

/// The files that differ between two trees.
pub fn files(cwd: &Path, before: &str, after: &str) -> Vec<File> {
    if before == after {
        return vec![];
    }
    let (Ok(num), Ok(names)) = (
        git(cwd, &["diff", "--no-renames", "--numstat", "-z", before, after], None),
        git(cwd, &["diff", "--no-renames", "--name-status", "-z", before, after], None),
    ) else {
        return vec![];
    };
    let mut status = std::collections::HashMap::new();
    let mut parts = names.split('\0');
    while let (Some(s), Some(p)) = (parts.next(), parts.next()) {
        status.insert(p.to_string(), s.chars().next().unwrap_or('M'));
    }
    num.split('\0').filter(|l| !l.is_empty()).filter_map(|l| {
        let mut f = l.splitn(3, '\t');
        let (a, r, p) = (f.next()?, f.next()?, f.next()?);
        Some(File { path: p.to_string(), added: a.parse().ok(), removed: r.parse().ok(), status: *status.get(p).unwrap_or(&'M') })
    }).collect()
}

/// One file's unified diff between two trees, cut at `max` bytes.
pub fn diff(cwd: &Path, before: &str, after: &str, path: &str, max: usize) -> String {
    let mut d = git(cwd, &["diff", "--no-renames", before, after, "--", path], None).unwrap_or_default();
    if d.len() > max {
        let mut cut = max;
        while !d.is_char_boundary(cut) {
            cut -= 1;
        }
        d.truncate(cut);
        d.push_str("\n… (cut)");
    }
    d
}

/// The blob id of `path` in `tree`, or None when the tree has no such file.
fn blob(cwd: &Path, tree: &str, path: &str) -> Option<String> {
    git(cwd, &["rev-parse", "--verify", "--quiet", &format!("{tree}:{path}")], None).ok().map(|s| s.trim().to_string())
}

/// The file on disk now, as a blob id (None when it does not exist).
fn on_disk(cwd: &Path, path: &str) -> Option<String> {
    cwd.join(path).is_file().then(|| git(cwd, &["hash-object", "--", path], None).ok()).flatten().map(|s| s.trim().to_string())
}

/// Puts the turn's files back as they were before it. Refused, with nothing written, when any of them changed after the
/// turn (by the user or a later turn). ponytail: file modes are not restored (an executable bit a turn added stays).
pub fn undo(cwd: &Path, before: &str, after: &str, changed: &[File]) -> Result<()> {
    for f in changed {
        if on_disk(cwd, &f.path) != blob(cwd, after, &f.path) {
            bail!("{} changed after that turn: undo works only while the files are as it left them", f.path);
        }
    }
    for f in changed {
        let full = cwd.join(&f.path);
        match blob(cwd, before, &f.path) {
            Some(b) => {
                let o = Command::new("git").args(["cat-file", "blob", &b]).current_dir(cwd).output().context("running git")?;
                if !o.status.success() {
                    bail!("could not read {} from before the turn", f.path);
                }
                if let Some(dir) = full.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                std::fs::write(&full, o.stdout).with_context(|| format!("writing {}", f.path))?;
            }
            // the turn added it
            None => std::fs::remove_file(&full).with_context(|| format!("removing {}", f.path))?,
        }
    }
    Ok(())
}

/// The work tree against HEAD as one unified diff, new untracked files included (as added whole): what "uncommitted"
/// shows in the app's review.
pub fn worktree_diff(cwd: &Path, max: usize) -> String {
    let mut d = git(cwd, &["diff", "--no-renames", "HEAD"], None).unwrap_or_default();
    let untracked = git(cwd, &["ls-files", "--others", "--exclude-standard", "-z"], None).unwrap_or_default();
    for p in untracked.split('\0').filter(|p| !p.is_empty()) {
        // `git diff --no-index` exits 1 when the files differ, which they always do here
        let o = Command::new("git").args(["diff", "--no-index", "--", "/dev/null", p]).current_dir(cwd).output();
        if let Ok(o) = o {
            d.push_str(&String::from_utf8_lossy(&o.stdout));
        }
        if d.len() > max {
            break;
        }
    }
    cut(d, max)
}

/// The branch against where it left the default branch (origin's, else main or master): (that base, the diff of the
/// work tree against it).
pub fn branch_diff(cwd: &Path, max: usize) -> Result<(String, String)> {
    let base = ["origin/HEAD", "origin/main", "origin/master", "main", "master"].iter()
        .find_map(|b| git(cwd, &["merge-base", "HEAD", b], None).ok().map(|m| (b.to_string(), m.trim().to_string())))
        .context("no default branch to compare with (origin/HEAD, main or master)")?;
    let d = git(cwd, &["diff", "--no-renames", &base.1], None)?;
    Ok((base.0, cut(d, max)))
}

fn cut(mut d: String, max: usize) -> String {
    if d.len() > max {
        let mut c = max;
        while !d.is_char_boundary(c) {
            c -= 1;
        }
        d.truncate(c);
        d.push_str("\n… (cut)\n");
    }
    d
}

/// One piece of a diff (a file's header and one hunk) applied: reversed in the work tree (`revert`), or added to the
/// index (`stage`). git checks the piece still fits and refuses a path outside the repository.
pub fn apply_piece(cwd: &Path, patch: &str, stage: bool) -> Result<()> {
    use std::io::Write;
    let args: &[&str] = if stage { &["apply", "--cached", "--recount", "-"] } else { &["apply", "-R", "--recount", "-"] };
    let mut child = Command::new("git").args(args).current_dir(cwd).stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::piped()).spawn().context("running git apply")?;
    child.stdin.take().context("git stdin")?.write_all(patch.as_bytes())?;
    let o = child.wait_with_output()?;
    if !o.status.success() {
        bail!("git apply: {}", String::from_utf8_lossy(&o.stderr).trim());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_turn_shows_its_files_and_undo_puts_them_back() {
        let dir = std::env::temp_dir().join(format!("tazuna-turns-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let run = |args: &[&str]| assert!(Command::new("git").args(args).current_dir(&dir).output().unwrap().status.success());
        run(&["init", "-q"]);
        run(&["config", "user.email", "t@t"]);
        run(&["config", "user.name", "t"]);
        // the files as written (a Windows runner's git turns on autocrlf, and git apply then writes CRLF, as it should there)
        run(&["config", "core.autocrlf", "false"]);
        std::fs::write(dir.join("a.txt"), "one\ntwo\n").unwrap();
        std::fs::write(dir.join("gone.txt"), "x\n").unwrap();
        std::fs::write(dir.join(".gitignore"), "ignored/\n").unwrap();
        run(&["add", "-A"]);
        run(&["commit", "-qm", "init"]);
        let index_before = std::fs::read(dir.join(".git/index")).unwrap();

        let before = snapshot(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), "one\nTWO\nthree\n").unwrap();
        std::fs::write(dir.join("new.txt"), "n\n").unwrap();
        std::fs::remove_file(dir.join("gone.txt")).unwrap();
        std::fs::create_dir_all(dir.join("ignored")).unwrap();
        std::fs::write(dir.join("ignored/big.bin"), "zzz").unwrap();
        let after = snapshot(&dir).unwrap();

        let fs = files(&dir, &before, &after);
        let by = |p: &str| fs.iter().find(|f| f.path == p).cloned();
        assert_eq!(fs.len(), 3, "{fs:?}");
        assert_eq!(by("a.txt").map(|f| (f.added, f.removed, f.status)), Some((Some(2), Some(1), 'M')));
        assert_eq!(by("new.txt").map(|f| f.status), Some('A'));
        assert_eq!(by("gone.txt").map(|f| f.status), Some('D'));
        assert!(diff(&dir, &before, &after, "a.txt", 10_000).contains("+TWO"));
        // the user's index is untouched (nothing staged)
        assert_eq!(std::fs::read(dir.join(".git/index")).unwrap(), index_before);

        // changed again after the turn: refused, nothing written
        std::fs::write(dir.join("new.txt"), "edited by the user\n").unwrap();
        assert!(undo(&dir, &before, &after, &fs).is_err());
        assert!(!dir.join("gone.txt").exists());
        std::fs::write(dir.join("new.txt"), "n\n").unwrap();

        undo(&dir, &before, &after, &fs).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "one\ntwo\n");
        assert_eq!(std::fs::read_to_string(dir.join("gone.txt")).unwrap(), "x\n");
        assert!(!dir.join("new.txt").exists());
        assert_eq!(snapshot(&dir).unwrap(), before);

        // review: the uncommitted diff (an untracked file too), one piece reverted, one staged
        std::fs::write(dir.join("a.txt"), "one\nTWO\n").unwrap();
        std::fs::write(dir.join("new.txt"), "n\n").unwrap();
        let d = worktree_diff(&dir, 100_000);
        assert!(d.contains("+TWO") && d.contains("+++ b/new.txt"), "{d}");
        let piece = |path: &str| {
            let start = d.find(&format!("diff --git a/{path}")).unwrap();
            let end = d[start + 1..].find("diff --git").map_or(d.len(), |e| start + 1 + e);
            d[start..end].to_string()
        };
        apply_piece(&dir, &piece("a.txt"), true).unwrap();
        assert!(git(&dir, &["diff", "--cached", "--name-only"], None).unwrap().contains("a.txt"), "staged");
        apply_piece(&dir, &piece("a.txt"), false).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "one\ntwo\n", "reverted in the work tree");
        assert!(apply_piece(&dir, &piece("a.txt"), false).is_err(), "no longer fits");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
