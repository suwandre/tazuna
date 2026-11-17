//! The sandbox a reviewed command runs in (review mode: a cheap model approves the agent's commands instead of the user).
//! It may write only inside the project and the temporary and cache folders, reach no network (no unix sockets either:
//! docker's is root), and read none of the credential stores (CREDENTIALS) or the keychain. Inside the project it may
//! not write what runs later outside the sandbox: anything in .git (hooks, config with core.fsmonitor, which runs on
//! tazuna's own `git status`, or .git replaced by a gitdir file pointing elsewhere), the project's .claude, .mcp.json
//! and .tazuna (claude loads a trusted repository's settings, hooks among them), .envrc (direnv runs it). So git add
//! and commit cannot run in it: the reviewer leaves those to the user. Secret-looking environment variables are unset
//! first (security reviews, 2026-11-17). macOS only (sandbox-exec, a Seatbelt profile); elsewhere the user answers as
//! before (a Linux sandbox was dropped untested, with known gaps: bubblewrap cannot cover a path that does not exist).

use std::path::Path;

/// A shell word: single-quoted.
fn q(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Under the home folder: what holds keys and logins.
const CREDENTIALS: [&str; 16] = [".ssh", ".aws", ".gnupg", ".config/tazuna", ".netrc", ".config/gh", ".kube", ".docker", ".npmrc",
    ".git-credentials", ".pypirc", ".cargo/credentials.toml", ".config/gcloud", ".azure", ".claude.json", "Library/Keychains"];

/// In the project: what git, claude, tazuna or the shell later run outside the sandbox (the entry itself included: no
/// renaming it away and putting another in its place).
const RUN_LATER: [&str; 5] = [".git", ".claude", ".mcp.json", ".tazuna", ".envrc"];

/// The command after unsetting environment variables that look like secrets (names with TOKEN, SECRET, PASSWORD,
/// CREDENTIAL or KEY, and AWS_, GH_, GITHUB_ ones): its output goes to the model.
fn scrubbed(command: &str) -> String {
    format!("shopt -s nocasematch; for __v in $(compgen -e); do case \"$__v\" in *TOKEN*|*SECRET*|*PASSW*|*CREDENTIAL*|*_KEY|*_KEY_*|*APIKEY*|AWS_*|GH_*|GITHUB_*) unset \"$__v\";; esac; done; unset __v; shopt -u nocasematch\n{command}")
}

/// Seatbelt's string literal.
fn lit(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// The macOS profile for a project in `cwd` (resolved: /tmp is /private/tmp to the kernel).
fn profile(cwd: &Path, home: &Path) -> String {
    let h = |p: &str| lit(&home.join(p).to_string_lossy());
    // subpath covers a folder and a file of that name alike
    let later: Vec<String> = RUN_LATER.iter().map(|p| format!("(subpath {})", lit(&cwd.join(p).to_string_lossy()))).collect();
    let secret: Vec<String> = CREDENTIALS.iter().map(|p| format!("(subpath {})", h(p))).collect();
    format!(
        "(version 1)\n(allow default)\n(deny network*)\n(deny file-write*)\n\
         (allow file-write* (subpath {}) (subpath \"/private/tmp\") (subpath \"/private/var/folders\") (subpath \"/dev\") (subpath {}) (subpath {}))\n\
         (deny file-write* {})\n(deny file-read* {})\n\
         (deny mach-lookup (global-name \"com.apple.SecurityServer\") (global-name \"com.apple.securityd\"))\n",
        lit(&cwd.to_string_lossy()), h("Library/Caches"), h(".cache"), later.join(" "), secret.join(" ")
    )
}

/// `command` as a shell command line that runs it in the sandbox for the project in `cwd`; None where there is no
/// sandbox (anywhere but macOS).
pub fn wrap(command: &str, cwd: &Path) -> Option<String> {
    let cwd = cwd.canonicalize().ok()?;
    let home = crate::platform::home()?;
    if cfg!(target_os = "macos") && Path::new("/usr/bin/sandbox-exec").exists() {
        return Some(format!("/usr/bin/sandbox-exec -p {} /bin/bash -c {}", q(&profile(&cwd, &home)), q(&scrubbed(command))));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "macos")]
    fn a_sandboxed_command_writes_in_the_project_only_and_reaches_no_network() {
        let dir = std::env::temp_dir().join(format!("tazuna-sbx-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let home = crate::platform::home().unwrap();
        let outside = home.join(format!(".tazuna-sbx-test-{}", std::process::id()));
        let run = |cmd: &str| std::process::Command::new("/bin/sh").arg("-c").arg(wrap(cmd, &dir).unwrap()).current_dir(&dir).output().unwrap();
        assert!(run(r#"echo "it's here" > inside.txt"#).status.success() && dir.join("inside.txt").exists(), "a quote in the command survives");
        assert!(!run(&format!("echo no > {}", outside.display())).status.success() && !outside.exists(), "nothing written outside");
        assert!(!run("curl -sS -m 5 https://example.com -o /dev/null").status.success(), "no network");
        assert!(!run("ls ~/.ssh").status.success() || !home.join(".ssh").exists(), "no key folders");
        // what runs later outside the sandbox stays as it was
        // a repository made outside (inside, git init writes .git/config: refused like any config change)
        assert!(std::process::Command::new("git").args(["init", "-q"]).current_dir(&dir).status().unwrap().success());
        assert!(!run("echo x > .git/hooks/pre-commit").status.success() && !dir.join(".git/hooks/pre-commit").exists(), "no git hook");
        assert!(!run("git config core.fsmonitor 'touch /tmp/x'").status.success(), "no git config");
        assert!(!run("mv .git .g").status.success() && dir.join(".git").is_dir(), ".git stays where it is");
        assert!(!run("mkdir -p .claude && echo {} > .claude/settings.json").status.success() && !dir.join(".claude").exists(), "no claude settings");
        assert!(run("git status --short").status.success(), "reading the repository works");
        assert!(!run("echo a > a.txt && git add a.txt").status.success(), "writing it does not: the user answers git add and commit");
        // no secrets in the environment, no keychain, no unix sockets
        let env = std::process::Command::new("/bin/sh").arg("-c").arg(wrap("printenv MY_TOKEN; printenv github_token; printenv PATH >/dev/null && echo path-kept", &dir).unwrap())
            .env("MY_TOKEN", "s3cret").env("github_token", "s3cret").current_dir(&dir).output().unwrap();
        assert_eq!(String::from_utf8_lossy(&env.stdout).trim(), "path-kept", "secret-looking variables unset, others kept");
        assert!(!run("ls ~/Library/Keychains").status.success(), "no keychain files");
        assert!(!run("/usr/bin/security find-generic-password -s tazuna-sbx-test").status.success(), "no keychain service");
        assert!(!run("python3 -c 'import socket; s = socket.socket(socket.AF_UNIX); s.connect(\"/var/run/syslog\")'").status.success(), "no unix sockets");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
