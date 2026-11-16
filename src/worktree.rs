//! A conversation in a worktree of its own (the desktop's "new worktree conversation"): a branch off the project's
//! current one, checked out under tazuna's data folder, the project's untracked local files (`.env*`, and paths listed
//! in `.worktreeinclude`) copied in, a port range of its own (TAZUNA_PORT), and the project's `.tazuna/setup.sh` run there
//! when the project is trusted. Two agents can then work on one repository at once. Removing one keeps its branch.
//! Also the pull request of a folder's branch, through `gh` (the desktop's PR panel).

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Where tazuna's worktrees live: <data>/worktrees/<repository>/<name>.
pub fn root(base: &Path) -> PathBuf {
    base.join("worktrees")
}

fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let o = Command::new("git").args(args).current_dir(dir).output().context("running git")?;
    if !o.status.success() {
        bail!("git {}: {}", args.join(" "), String::from_utf8_lossy(&o.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&o.stdout).trim_end().to_string())
}

/// A worktree's ports: TAZUNA_PORT and the nine after it, from its name (4100-8090).
/// ponytail: a hash, so two worktrees can share a range (1 in 400); a registry if that bites.
pub fn port(name: &str) -> u16 {
    let h = name.bytes().fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619));
    4100 + (h % 400) as u16 * 10
}

/// The project's untracked files a worktree needs: ignored `.env*` files anywhere (outside ignored folders), and the
/// files named in `.worktreeinclude` (one path a line, relative to the project; `#` comments).
fn local_files(project: &Path) -> Vec<String> {
    let mut out: Vec<String> = git(project, &["ls-files", "--others", "--ignored", "--exclude-standard", "--directory"]).unwrap_or_default()
        .lines().filter(|p| !p.ends_with('/') && p.rsplit('/').next().is_some_and(|n| n.starts_with(".env"))).map(String::from).collect();
    for l in std::fs::read_to_string(project.join(".worktreeinclude")).unwrap_or_default().lines() {
        let l = l.trim().trim_start_matches("./");
        // inside the project only: no absolute paths, no climbing out
        if l.is_empty() || l.starts_with('#') || Path::new(l).is_absolute() || l.split(['/', '\\']).any(|c| c == "..") {
            continue;
        }
        if project.join(l).is_file() && !out.iter().any(|o| o == l) {
            out.push(l.to_string());
        }
    }
    out
}

/// Copies `rel` from the project into the worktree, both ends inside their trees: the source a plain file (not a link)
/// whose real path is in the project, the target's folder real and in the worktree, the target new (security review: a
/// repository's tracked link to a folder outside, say ~/.ssh, made source and target one file, and the copy truncated it).
fn copy_inside(top: &Path, wt: &Path, rel: &str) -> Result<()> {
    let (top_real, wt_real) = (top.canonicalize()?, wt.canonicalize()?);
    let src = top.join(rel);
    if !std::fs::symlink_metadata(&src)?.file_type().is_file() || !src.canonicalize()?.starts_with(&top_real) {
        bail!("{rel}: not a plain file inside the project");
    }
    let to = wt.join(rel);
    let dir = to.parent().context("no folder")?;
    std::fs::create_dir_all(dir)?;
    if !dir.canonicalize()?.starts_with(&wt_real) {
        bail!("{rel}: its folder leads out of the worktree");
    }
    let data = std::fs::read(&src)?;
    // create_new: an existing file or link at the target is never written through
    std::io::Write::write_all(&mut std::fs::OpenOptions::new().write(true).create_new(true).open(&to)?, &data)?;
    Ok(())
}

pub struct Made {
    pub path: PathBuf,
    pub branch: String,
    pub port: u16,
    pub copied: Vec<String>,
}

/// A new worktree of the repository at `project`: branch `tazuna/<name>` off the current HEAD, checked out at
/// <root>/<repository>/<name>, the local files copied in.
pub fn create(project: &Path, root: &Path, name: &str) -> Result<Made> {
    let top = PathBuf::from(git(project, &["rev-parse", "--show-toplevel"]).context("not a git repository")?);
    let repo = top.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "repo".into());
    let path = root.join(&repo).join(name);
    if path.exists() {
        bail!("{} exists already", path.display());
    }
    std::fs::create_dir_all(path.parent().context("no parent")?)?;
    let branch = format!("tazuna/{name}");
    git(&top, &["worktree", "add", "-b", &branch, &path.to_string_lossy(), "HEAD"])?;
    let mut copied = vec![];
    for f in local_files(&top) {
        if copy_inside(&top, &path, &f).is_ok() {
            copied.push(f);
        }
    }
    Ok(Made { path, branch, port: port(name), copied })
}

/// The project's `.tazuna/setup.sh` run in the new worktree (install dependencies and the like), with TAZUNA_PORT set:
/// None when there is none, else its outcome and the end of its output.
pub fn setup(project: &Path, wt: &Made) -> Option<(bool, String)> {
    let script = project.join(".tazuna/setup.sh");
    if !script.is_file() {
        return None;
    }
    let o = Command::new("sh").arg(&script).current_dir(&wt.path).env("TAZUNA_PORT", wt.port.to_string()).output();
    Some(match o {
        Ok(o) => {
            let text = String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr);
            let tail: Vec<&str> = text.lines().rev().take(8).collect::<Vec<_>>().into_iter().rev().collect();
            (o.status.success(), tail.join("\n"))
        }
        Err(e) => (false, e.to_string()),
    })
}

/// tazuna's worktrees of the repository at `project`: path, branch, and whether it has uncommitted changes.
pub fn list(project: &Path, root: &Path) -> Vec<Value> {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let text = git(project, &["worktree", "list", "--porcelain"]).unwrap_or_default();
    text.split("\n\n").filter_map(|block| {
        let path = PathBuf::from(block.lines().find_map(|l| l.strip_prefix("worktree "))?);
        let real = path.canonicalize().unwrap_or_else(|_| path.clone());
        if !real.starts_with(&root) {
            return None;
        }
        let branch = block.lines().find_map(|l| l.strip_prefix("branch refs/heads/")).unwrap_or("(detached)");
        let dirty = git(&path, &["status", "--porcelain"]).is_ok_and(|s| !s.is_empty());
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        Some(json!({"path": path, "branch": branch, "dirty": dirty, "port": port(&name)}))
    }).collect()
}

/// Removes one of tazuna's worktrees (its branch stays). Uncommitted changes refuse unless `force`.
pub fn remove(project: &Path, root: &Path, path: &Path, force: bool) -> Result<()> {
    let (root, real) = (root.canonicalize()?, path.canonicalize()?);
    if !real.starts_with(&root) || real == root {
        bail!("not one of tazuna's worktrees");
    }
    let p = real.to_string_lossy();
    let args: Vec<&str> = if force { vec!["worktree", "remove", "--force", &p] } else { vec!["worktree", "remove", &p] };
    git(project, &args)?;
    // the repository's folder under the root goes with its last worktree
    if let Some(parent) = real.parent().filter(|p| *p != root) {
        let _ = std::fs::remove_dir(parent);
    }
    Ok(())
}

/// The pull request of the branch checked out in `dir`, as `gh` sees it: number, title, state, checks, reviews and
/// comments (inline ones too), or {"none": true}; {"error": ...} when gh is missing or not logged in.
pub fn pr(dir: &Path) -> Value {
    let fields = "number,title,state,url,isDraft,headRefName,baseRefName,reviewDecision,statusCheckRollup,reviews,comments";
    let o = match Command::new("gh").args(["pr", "view", "--json", fields]).current_dir(dir).output() {
        Ok(o) => o,
        Err(_) => return json!({"error": "gh is not installed (https://cli.github.com)"}),
    };
    let err = String::from_utf8_lossy(&o.stderr);
    if !o.status.success() {
        let branch = git(dir, &["branch", "--show-current"]).unwrap_or_default();
        return if err.contains("no pull requests found") { json!({"none": true, "branch": branch}) } else { json!({"error": err.trim(), "branch": branch}) };
    }
    let Ok(mut v) = serde_json::from_slice::<Value>(&o.stdout) else { return json!({"error": "gh answered something else"}) };
    let n = v["number"].as_u64().unwrap_or(0);
    let inline = Command::new("gh").args(["api", &format!("repos/{{owner}}/{{repo}}/pulls/{n}/comments"), "--paginate"]).current_dir(dir).output()
        .ok().filter(|o| o.status.success()).and_then(|o| serde_json::from_slice::<Value>(&o.stdout).ok()).unwrap_or(json!([]));
    v["inline"] = json!(inline.as_array().into_iter().flatten().take(100).map(|c| json!({
        "author": c["user"]["login"], "association": c["author_association"], "path": c["path"], "line": c["line"].as_u64().or(c["original_line"].as_u64()),
        "body": c["body"], "url": c["html_url"],
    })).collect::<Vec<_>>());
    v
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_worktree_gets_a_branch_the_local_files_and_goes_away_again() {
        let base = std::env::temp_dir().join(format!("tazuna-wt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (repo, root) = (base.join("app"), base.join("data").join("worktrees"));
        std::fs::create_dir_all(repo.join("cfg")).unwrap();
        let run = |args: &[&str]| assert!(std::process::Command::new("git").args(args).current_dir(&repo).output().unwrap().status.success(), "{args:?}");
        run(&["init", "-q"]);
        run(&["config", "user.email", "t@t"]);
        run(&["config", "user.name", "t"]);
        std::fs::write(repo.join(".gitignore"), ".env*\nlocal.json\nnode_modules/\n").unwrap();
        std::fs::write(repo.join("a.txt"), "a\n").unwrap();
        run(&["add", "."]);
        run(&["commit", "-qm", "init"]);
        std::fs::write(repo.join(".env"), "KEY=1\n").unwrap();
        std::fs::write(repo.join("cfg/.env.local"), "X=2\n").unwrap();
        std::fs::write(repo.join("local.json"), "{}\n").unwrap();
        std::fs::create_dir_all(repo.join("node_modules/x")).unwrap();
        std::fs::write(repo.join("node_modules/x/.env"), "no\n").unwrap();
        std::fs::write(repo.join(".worktreeinclude"), "# extra\nlocal.json\n../outside\n/etc/passwd\n").unwrap();
        let wt = super::create(&repo, &root, "one").unwrap();
        assert_eq!(wt.branch, "tazuna/one");
        let mut copied = wt.copied.clone();
        copied.sort();
        assert_eq!(copied, vec![".env", "cfg/.env.local", "local.json"], "local files, not ones inside ignored folders or outside");
        assert_eq!(std::fs::read_to_string(wt.path.join(".env")).unwrap(), "KEY=1\n");
        assert!(wt.path.join("a.txt").exists() && (4100..8100).contains(&wt.port));
        assert!(super::create(&repo, &root, "one").is_err(), "the same name twice");
        // a tracked link to a folder outside: a file named through it is neither read nor written
        #[cfg(unix)]
        {
            let outside = base.join("outside");
            std::fs::create_dir_all(&outside).unwrap();
            std::fs::write(outside.join("config"), "keep me\n").unwrap();
            std::os::unix::fs::symlink(&outside, repo.join("linked")).unwrap();
            run(&["add", "linked"]);
            run(&["commit", "-qm", "link"]);
            std::fs::write(repo.join(".worktreeinclude"), "linked/config\n").unwrap();
            let two = super::create(&repo, &root, "two").unwrap();
            assert!(!two.copied.iter().any(|c| c.starts_with("linked")), "{:?}", two.copied);
            assert_eq!(std::fs::read_to_string(outside.join("config")).unwrap(), "keep me\n", "the file outside is untouched");
            super::remove(&repo, &root, &two.path, true).unwrap();
        }
        let list = super::list(&repo, &root);
        assert_eq!((list.len(), list[0]["branch"].as_str(), list[0]["dirty"].as_bool()), (1, Some("tazuna/one"), Some(false)));
        std::fs::write(wt.path.join("a.txt"), "changed\n").unwrap();
        assert!(super::remove(&repo, &root, &wt.path, false).is_err(), "uncommitted changes refuse");
        assert!(super::remove(&repo, &root, &repo, true).is_err(), "only tazuna's worktrees");
        super::remove(&repo, &root, &wt.path, true).unwrap();
        assert!(!wt.path.exists() && super::list(&repo, &root).is_empty());
        assert!(!wt.path.parent().unwrap().exists(), "the repository's folder goes with its last worktree");
        let _ = std::fs::remove_dir_all(&base);
    }
}
