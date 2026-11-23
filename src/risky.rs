//! The commands "auto" asks about first; everything else it runs without a question, and "full" never asks (the user,
//! 2026-11-23: auto had no guard but one line of the prompt). The list: rm, rmdir and unlink outside the project (and
//! `find -delete`, `find -exec rm`, `find | xargs rm` there), chmod, chown and chgrp outside it, git push --force or
//! --mirror, git reset --hard, git clean -f, sudo, doas, su and pkexec, disk and system tools (dd, diskutil, mkfs,
//! fdisk, shutdown...). The temporary folders count as inside: agents clean up there all the time.
//!
//! A reading of the shell line, not a sandbox: it splits on ; & | ( ) and newlines, skips here-documents, checks what
//! `$(...)` and backticks run, follows `cd` and the line's own variables (`d=$(mktemp -d)`, `for f in ...`), looks
//! through `sh -c`, `eval`, `env`, `xargs`, `if`, `{ ...; }` and the like, and takes a path it cannot work out ($VAR,
//! ~user) as outside.
// ponytail: a lexical reading; a symlink in the project pointing out, aliases and functions, and a script the agent wrote
// and then runs pass. On Windows PowerShell's Remove-Item is not read, and Git Bash's "/c/proj" is not matched to C:\proj
// (it errs towards asking). A real fence is the review sandbox (sandbox.rs); widen this list when a miss is seen.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

/// Why `command`, run in `project` by a shell that is in `start` (None: somewhere unknown), asks first in auto ("rm
/// outside the project: ~/x"; None: it runs without a question), and where the line leaves that shell: the lean bash
/// keeps its folder from one call to the next (mcp.rs), so `cd ..` and then `rm -rf sibling` is two calls.
pub fn scan(command: &str, project: &Path, start: Option<&Path>) -> (Option<String>, Option<PathBuf>) {
    let mut cx = Cx { project, dir: start.map(Path::to_path_buf), vars: HashMap::new() };
    let why = cx.line(command);
    (why, cx.dir)
}

/// One simple command's words, and whether a pipe feeds it the one before.
struct Seg {
    words: Vec<String>,
    piped: bool,
}

/// The line read so far.
#[derive(Default)]
struct Tok {
    segs: Vec<Seg>,
    /// what `$(...)` and backticks run: checked as lines of their own
    inner: Vec<String>,
    words: Vec<String>,
    word: String,
    in_word: bool,
    piped: bool,
    /// the next word is a redirection's target ("> file"), or a here-document's end marker ("<< EOF")
    target: bool,
    marker: bool,
    /// here-documents whose text starts at the next newline: their end markers
    heredocs: Vec<String>,
}

impl Tok {
    fn push(&mut self, c: char) {
        self.word.push(c);
        self.in_word = true;
    }

    fn end_word(&mut self) {
        if !std::mem::take(&mut self.in_word) {
            return;
        }
        let w = std::mem::take(&mut self.word);
        if std::mem::take(&mut self.marker) {
            self.heredocs.push(w);
        } else if std::mem::take(&mut self.target) {
        } else if let Some(op) = Some(w.trim_start_matches(|c: char| c.is_ascii_digit())).filter(|op| op.starts_with(['>', '<'])) {
            // "2>/dev/null" is no path to remove; "<<EOF" starts a here-document, "<<<" is a here-string
            match op.strip_prefix("<<").filter(|r| !r.starts_with('<')).map(|r| r.trim_start_matches('-')) {
                Some("") => self.marker = true,
                Some(m) => self.heredocs.push(m.to_string()),
                None => self.target = op.trim_start_matches(['>', '<', '&', '|']).is_empty(),
            }
        } else {
            self.words.push(w);
        }
    }

    fn end_seg(&mut self, piped_next: bool) {
        self.end_word();
        if !self.words.is_empty() {
            self.segs.push(Seg { words: std::mem::take(&mut self.words), piped: self.piped });
        }
        self.piped = piped_next;
    }
}

/// The line as simple commands with their quotes taken off, and what its command substitutions run.
fn split(line: &str) -> (Vec<Seg>, Vec<String>) {
    let cs: Vec<char> = line.chars().collect();
    let mut t = Tok::default();
    let mut quote = None::<char>;
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        i += 1;
        // $(...) and `...`: kept whole in the word ("$(pwd)/build" is read as a path) and checked on their own
        if quote != Some('\'') && (c == '`' || (c == '$' && cs.get(i) == Some(&'('))) {
            let (start, mut depth, mut j) = (if c == '`' { i } else { i + 1 }, 1, if c == '`' { i } else { i + 1 });
            while j < cs.len() {
                match (c, cs[j]) {
                    ('`', '`') => depth = 0,
                    ('$', '(') => depth += 1,
                    ('$', ')') => depth -= 1,
                    _ => {}
                }
                if depth == 0 {
                    break;
                }
                j += 1;
            }
            t.inner.push(cs[start..j.min(cs.len())].iter().collect());
            for &k in &cs[i - 1..(j + 1).min(cs.len())] {
                t.push(k);
            }
            i = j + 1;
            continue;
        }
        match (quote, c) {
            (Some('\''), '\'') | (Some('"'), '"') => quote = None,
            (Some('"'), '\\') if i < cs.len() => {
                t.push(cs[i]);
                i += 1;
            }
            (Some(_), c) => t.push(c),
            (None, '\'' | '"') => {
                quote = Some(c);
                t.in_word = true;
            }
            (None, '\\') => {
                if cs.get(i).is_some_and(|n| *n != '\n') {
                    t.push(cs[i]);
                }
                i += 1;
            }
            (None, '#') if !t.in_word => {
                while i < cs.len() && cs[i] != '\n' {
                    i += 1;
                }
            }
            (None, '\n') => {
                t.end_seg(false);
                // a here-document's text is data, up to its end marker on a line of its own
                for m in std::mem::take(&mut t.heredocs) {
                    while i < cs.len() {
                        let end = cs[i..].iter().position(|x| *x == '\n').map_or(cs.len(), |p| i + p);
                        let found = cs[i..end].iter().collect::<String>().trim() == m;
                        i = end + 1;
                        if found {
                            break;
                        }
                    }
                }
            }
            (None, '|') if cs.get(i) == Some(&'|') => {
                i += 1;
                t.end_seg(false);
            }
            (None, '|') => t.end_seg(true),
            (None, ';' | '&' | '(' | ')') => t.end_seg(false),
            (None, c) if c.is_whitespace() => t.end_word(),
            (None, c) => t.push(c),
        }
    }
    t.end_seg(false);
    (t.segs, t.inner)
}

/// A word's file name: "rm" for "/bin/rm".
fn base(w: &str) -> &str {
    w.rsplit('/').next().unwrap_or(w)
}

/// NAME=value, as (NAME, value).
fn assignment(w: &str) -> Option<(&str, &str)> {
    w.split_once('=').filter(|(k, _)| !k.is_empty() && !k.starts_with(|c: char| c.is_ascii_digit()) && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
}

const RM: [&str; 3] = ["rm", "rmdir", "unlink"];

/// The shell as the line leaves it: where it is (None: unknown) and the variables the line set (None: a value it cannot
/// tell).
struct Cx<'a> {
    project: &'a Path,
    dir: Option<PathBuf>,
    vars: HashMap<String, Option<PathBuf>>,
}

impl Cx<'_> {
    /// A subshell (`sh -c`, `$(...)`): it starts here, and its `cd` and variables stay its own.
    fn sub(&self) -> Cx<'_> {
        Cx { project: self.project, dir: self.dir.clone(), vars: self.vars.clone() }
    }

    fn line(&mut self, line: &str) -> Option<String> {
        let (segs, inner) = split(line);
        if let Some(why) = inner.iter().find_map(|l| self.sub().line(l)) {
            return Some(why);
        }
        let mut before: Option<&[String]> = None;
        for s in &segs {
            if let Some(why) = self.check(&s.words, if s.piped { before } else { None }) {
                return Some(why);
            }
            before = Some(&s.words);
        }
        None
    }

    /// One simple command: why it asks, if it does. `piped`: the command whose output it reads.
    fn check(&mut self, words: &[String], piped: Option<&[String]>) -> Option<String> {
        let mut w = words;
        let mut via_xargs = false;
        // VAR=value in front, and the wrappers that run the rest as a command of their own
        loop {
            let first = w.first()?;
            if let Some((k, v)) = assignment(first) {
                // assignments alone set the shell's variables for the rest of the line
                if w.len() == 1 {
                    let v = self.value(v);
                    self.vars.insert(k.to_string(), v);
                }
                w = &w[1..];
                continue;
            }
            match base(first) {
                n @ ("sudo" | "doas" | "su" | "pkexec") => return Some(format!("{n}: runs as the administrator")),
                n @ ("env" | "command" | "builtin" | "exec" | "nohup" | "time" | "nice" | "xargs" | "caffeinate" | "timeout"
                | "{" | "if" | "elif" | "while" | "until" | "then" | "do" | "else" | "!") => {
                    via_xargs |= n == "xargs";
                    // their own flags and numbers ("nice -n 10", "timeout 30s") are not the command
                    w = &w[1..];
                    while w.first().is_some_and(|x| x.starts_with('-') || x.starts_with(|c: char| c.is_ascii_digit())) {
                        w = &w[1..];
                    }
                }
                "sh" | "bash" | "zsh" | "dash" | "ksh" | "fish" => {
                    let c = w.iter().position(|x| x.starts_with('-') && !x.starts_with("--") && x.contains('c'))?;
                    return self.sub().line(w.get(c + 1)?);
                }
                // eval runs in this shell: its cd counts
                "eval" => return self.line(&w[1..].join(" ")),
                _ => break,
            }
        }
        let name = base(&w[0]).to_string();
        let args = &w[1..];
        // the words that are not options ("--" ends the options)
        let mut past = false;
        let operands: Vec<&String> = args.iter().filter(|a| {
            let keep = past || !a.starts_with('-') || a.as_str() == "-";
            past |= a.as_str() == "--";
            keep && a.as_str() != "--"
        }).collect();
        match name.as_str() {
            "cd" | "pushd" => {
                self.dir = match operands.first().map(|s| s.as_str()) {
                    None | Some("~") => crate::platform::home(),
                    Some("-") => None,
                    Some(d) => self.resolve(d),
                };
                None
            }
            "popd" => {
                self.dir = None;
                None
            }
            "for" => {
                // the loop variable: the first item outside the project, else the first item
                let var = args.first()?;
                let items: Vec<Option<PathBuf>> = args.iter().skip_while(|a| *a != "in").skip(1).map(|a| self.resolve(a)).collect();
                let pick = items.iter().find(|p| !self.inside(p.as_deref(), false)).or(items.first()).cloned().flatten();
                self.vars.insert(var.clone(), pick);
                None
            }
            n if RM.contains(&n) => match operands.iter().find(|a| !self.inside(self.resolve(a).as_deref(), true)) {
                Some(a) => Some(format!("{n} outside the project: {a}")),
                // `find ~ | xargs rm`: the paths are find's
                None if operands.is_empty() && via_xargs => piped.filter(|p| p.first().is_some_and(|f| base(f) == "find")).and_then(|p| self.find_outside(&p[1..])),
                None => None,
            },
            // chmod's mode and chown's owner ("755", "me:staff") read as a name in the project: inside, never the reason
            "chmod" | "chown" | "chgrp" => operands.iter().find(|a| !self.inside(self.resolve(a).as_deref(), false)).map(|a| format!("{name} outside the project: {a}")),
            "find" => {
                let deletes = args.iter().enumerate().any(|(i, a)| {
                    a == "-delete" || (matches!(a.as_str(), "-exec" | "-execdir" | "-ok" | "-okdir") && args.get(i + 1).is_some_and(|x| RM.contains(&base(x))))
                });
                if deletes { self.find_outside(args) } else { None }
            }
            "git" => {
                // past git's own options: -C dir, -c key=value, --git-dir=...
                let mut i = 0;
                while args.get(i).is_some_and(|a| a.starts_with('-')) {
                    i += if matches!(args[i].as_str(), "-C" | "-c" | "--git-dir" | "--work-tree" | "--namespace") { 2 } else { 1 };
                }
                let rest = args.get(i + 1..).unwrap_or_default();
                let has = |short: char, long: &str| rest.iter().any(|a| a.starts_with(long) || (a.starts_with('-') && !a.starts_with("--") && a.contains(short)));
                match args.get(i).map(String::as_str) {
                    Some("push") if has('f', "--force") || rest.iter().any(|a| a == "--mirror" || (a.starts_with('+') && a.len() > 1)) => {
                        Some("git push --force: rewrites the remote's history".into())
                    }
                    Some("reset") if rest.iter().any(|a| a == "--hard") => Some("git reset --hard: drops uncommitted changes".into()),
                    Some("clean") if has('f', "--force") && !has('n', "--dry-run") => Some("git clean: deletes untracked files".into()),
                    _ => None,
                }
            }
            "dd" | "diskutil" | "fdisk" | "sfdisk" | "gdisk" | "parted" | "wipefs" | "shutdown" | "reboot" | "halt" | "poweroff" => {
                Some(format!("{name}: a disk or system tool"))
            }
            n if n.starts_with("mkfs") || n.starts_with("newfs") => Some(format!("{name}: a disk or system tool")),
            _ => None,
        }
    }

    /// find's starting folders ("." when none is given), where one is outside the project: why it asks.
    fn find_outside(&self, args: &[String]) -> Option<String> {
        let starts: Vec<&String> = args.iter().take_while(|a| !a.starts_with(['-', '(', '!'])).collect();
        let dot = String::from(".");
        let starts = if starts.is_empty() { vec![&dot] } else { starts };
        // the project itself is a fine start: what find removes is under it
        starts.into_iter().find(|s| !self.inside(self.resolve(s).as_deref(), false)).map(|s| format!("find deletes outside the project: {s}"))
    }

    /// An assignment's value as a path: `$(mktemp ...)` a temporary one.
    fn value(&self, v: &str) -> Option<PathBuf> {
        if v.starts_with("$(mktemp") || v.starts_with("`mktemp") {
            return Some(std::env::temp_dir().join("tazuna-mktemp"));
        }
        self.resolve(v)
    }

    /// `path` as an absolute path without `.` and `..`, from where the shell is; None where it cannot be told. A variable
    /// is read at the start only ("$HOME/x", "${PWD}/x", "$(pwd)/x", or one the line set).
    fn resolve(&self, path: &str) -> Option<PathBuf> {
        let (from, rest): (Option<PathBuf>, &str) = if let Some(r) = path.strip_prefix("$(pwd)").or(path.strip_prefix("`pwd`")) {
            (self.dir.clone(), r)
        } else if let Some(r) = path.strip_prefix('$') {
            let (name, rest) = match r.strip_prefix('{') {
                Some(b) => b.split_once('}')?,
                None => r.split_at(r.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(r.len())),
            };
            let v = match name {
                "PWD" => self.dir.clone(),
                "HOME" => crate::platform::home(),
                "TMPDIR" => Some(std::env::temp_dir()),
                n => self.vars.get(n).cloned().flatten(),
            };
            (Some(v?), rest)
        } else {
            (None, path)
        };
        if rest.contains(['$', '`']) || (from.is_none() && rest.starts_with('~') && !(rest == "~" || rest.starts_with("~/"))) {
            return None;
        }
        let p = match (from, rest.strip_prefix('~')) {
            (Some(f), _) => f.join(rest.trim_start_matches('/')),
            (None, Some(r)) => crate::platform::home()?.join(r.trim_start_matches('/')),
            // "/x" is rooted in Git Bash too, where Path counts only "C:\x" as absolute
            (None, None) if rest.starts_with('/') || Path::new(rest).is_absolute() => PathBuf::from(rest),
            (None, None) => self.dir.as_ref()?.join(rest),
        };
        let mut out = PathBuf::new();
        for c in p.components() {
            match c {
                Component::ParentDir => {
                    out.pop();
                }
                Component::CurDir => {}
                c => out.push(c),
            }
        }
        Some(out)
    }

    /// Under the project or a temporary folder. The folder itself counts as outside for a removal (`rm -rf .` from above
    /// is the whole project) and for a temporary folder; `chmod -R u+w .` in the project is inside.
    fn inside(&self, p: Option<&Path>, removal: bool) -> bool {
        let Some(p) = p else { return false };
        let project = [self.project.to_path_buf(), self.project.canonicalize().unwrap_or_default()];
        let tmp = std::env::temp_dir();
        let temps = [tmp.canonicalize().unwrap_or_default(), tmp, "/tmp".into(), "/private/tmp".into(), "/var/folders".into(), "/private/var/folders".into()];
        let under = |r: &PathBuf, itself: bool| !r.as_os_str().is_empty() && p.starts_with(r) && (itself || p != r);
        project.iter().any(|r| under(r, !removal)) || temps.iter().any(|r| under(r, false))
    }
}

#[cfg(test)]
mod tests {
    use super::scan;
    use std::path::Path;

    fn risky(command: &str, project: &Path) -> Option<String> {
        scan(command, project, Some(project)).0
    }

    #[test]
    fn auto_asks_about_the_destructive_commands_only() {
        let cwd = Path::new("/work/proj");
        let home = crate::platform::home().unwrap().to_string_lossy().into_owned();
        let asks = [
            "rm -rf ~",
            "rm -rf ~/Documents",
            "rm -rf /",
            "rm -rf /work/proj",
            "rm -rf ../other",
            "cd .. && rm -rf proj2",
            "cd /etc; rm hosts",
            "rm -rf \"$HOME/x\"",
            "rm -rf $DIR",
            "ls && sudo make install",
            "FOO=1 sudo -E true",
            "su -c 'rm x'",
            "echo x | xargs rm -rf /opt/thing",
            "bash -c 'rm -rf ~/.config'",
            "sh -lc \"cd / && rm -rf usr\"",
            "eval rm -rf /var/db",
            // a subshell after a cd still checks against the project
            "cd \"$HOME\" && bash -c 'rm -rf ~'",
            "cd .. && sh -c 'rm -rf proj2'",
            "cd / && eval rm -rf usr",
            "git push --force origin main",
            "git push -f",
            "git push origin +main",
            "git push --force-with-lease",
            "git push --mirror",
            "git -C sub reset --hard HEAD~1",
            "git clean -fdx",
            "git clean -f",
            "chmod -R 777 /usr/local",
            "chown -R me ~/.ssh",
            "chmod -w /etc/hosts",
            "dd if=/dev/zero of=/dev/disk2",
            "diskutil eraseDisk APFS x disk2",
            "mkfs.ext4 /dev/sdb1",
            "/bin/rm -r /Users",
            "nice -n 10 rm -rf /srv",
            // after a here-document, and through braces, conditions, loops, find and xargs
            "cat > notes.md <<'EOF'\nIt's done\nEOF\nrm -rf ~/old",
            "{ rm -rf ~/x; } 2>/dev/null",
            "if rm -rf ~/x; then echo ok; fi",
            "until rm -rf ~/x; do sleep 1; done",
            "for d in ~/a ~/b; do rm -rf \"$d\"; done",
            "find ~ -name '*.bak' | xargs rm -f",
            "find ~/x -exec rm -rf {} +",
            "find / -name core -delete",
            "echo \"$(rm -rf ~/y)\"",
            "x=$(rm -rf /opt/z)",
        ];
        for c in asks {
            assert!(risky(c, cwd).is_some(), "should ask: {c}");
        }
        let runs = [
            "rm -rf target",
            "rm -rf ./build dist/*",
            "rm -rf /work/proj/node_modules",
            "rm -f /tmp/tzr12/x.png",
            "rm -rf /private/tmp/scratch",
            "cd sub && rm -rf out",
            "rm -rf -- -weird",
            "cargo test 2>/dev/null; rm out.txt > /dev/null",
            "echo 'rm -rf /' > notes.txt",
            "git push origin main",
            "git push -u origin feature",
            "git reset HEAD~1",
            "git clean -n",
            "git clean -nfd",
            "chmod +x scripts/run.sh",
            "chown me:staff out.log",
            "chmod -R u+rw .",
            "grep -rn 'sudo' src",
            "ls -la # rm -rf /",
            "find . -name '*.o' | xargs rm",
            "find . -name '*.pyc' -delete",
            "find build -exec rm {} +",
            "npm run build && rm -rf .cache",
            // here-documents are data, and the line's own variables are read
            "cat > clean.sh <<'EOF'\nrm -rf \"$OUT\"\nsudo make install\nEOF\nchmod +x clean.sh",
            "cat <<-END > README.md\n\tsudo make install\n\tEND",
            "tmp=$(mktemp -d) && rm -rf \"$tmp\"",
            "rm -rf \"$TMPDIR/foo\"",
            "rm -rf \"$(pwd)/build\"",
            "rm -rf $PWD/build",
            "rm -rf ${PWD}/build",
            "for f in scripts/*.sh; do chmod +x \"$f\"; done",
            "out=build && rm -rf \"$out\"",
        ];
        for c in runs {
            assert!(risky(c, cwd).is_none(), "should run: {c} ({:?})", risky(c, cwd));
        }
        // quoted: unquoted, bash reads a Windows home's backslashes as escapes
        assert_eq!(risky(&format!("rm -rf '{home}/x'"), cwd).as_deref(), Some(format!("rm outside the project: {home}/x").as_str()));
    }

    /// It reads every question's command, so no line may make it panic.
    #[test]
    fn odd_lines_do_not_panic() {
        for c in ["$(", "`", "<<", "cat <<", "a\\", "'", "\"$(", "${", "$", "rm $", "rm ${X", "for", "for x in", "cd", "git", "git -C",
            "sh -c", "find", "x=", "|", "| xargs rm", "((", "))", "<<EOF\n", "echo \"\\", "$(echo `", "rm \u{e9}$\u{e9}", "{ }", "if"] {
            let _ = scan(c, Path::new("/p"), None);
            let _ = scan(c, Path::new("/p"), Some(Path::new("/p")));
        }
    }

    /// The lean bash keeps its folder between calls: `cd ..` in one, the sibling's removal in the next asks.
    #[test]
    fn a_cd_carries_to_the_next_call() {
        let cwd = Path::new("/work/proj");
        let (why, at) = scan("cd .. && ls", cwd, Some(cwd));
        assert_eq!((why, at.as_deref()), (None, Some(Path::new("/work"))));
        assert!(scan("rm -rf proj-backup", cwd, at.as_deref()).0.is_some(), "the shell is in /work now");
        assert!(scan("rm -rf build", cwd, None).0.is_some(), "a shell somewhere unknown: a relative path asks");
        let (_, back) = scan("cd /work/proj/sub", cwd, None);
        assert!(scan("rm -rf build", cwd, back.as_deref()).0.is_none());
    }
}
