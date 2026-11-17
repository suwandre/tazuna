//! The sandbox a reviewed command runs in (review mode: a cheap model approves the agent's commands instead of the user).
//! It may write only inside the project and the temporary and cache folders, reach no network, and read none of the
//! key folders (~/.ssh, ~/.aws, ~/.gnupg, tazuna's own config with its keys, ~/.netrc). macOS: sandbox-exec with a
//! Seatbelt profile; Linux: bubblewrap (bwrap) where installed; elsewhere none, and the user answers as before.

use std::path::Path;

/// A shell word: single-quoted.
fn q(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Seatbelt's string literal.
fn lit(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// The macOS profile for a project in `cwd` (resolved: /tmp is /private/tmp to the kernel).
fn profile(cwd: &Path, home: &Path) -> String {
    let h = |p: &str| lit(&home.join(p).to_string_lossy());
    format!(
        "(version 1)\n(allow default)\n(deny network*)\n(allow network* (remote unix-socket))\n(deny file-write*)\n\
         (allow file-write* (subpath {}) (subpath \"/private/tmp\") (subpath \"/private/var/folders\") (subpath \"/dev\") (subpath {}) (subpath {}))\n\
         (deny file-read* (subpath {}) (subpath {}) (subpath {}) (subpath {}) (literal {}))\n",
        lit(&cwd.to_string_lossy()), h("Library/Caches"), h(".cache"),
        h(".ssh"), h(".aws"), h(".gnupg"), h(".config/tazuna"), h(".netrc")
    )
}

/// `command` as a shell command line that runs it in the sandbox for the project in `cwd`; None where there is no
/// sandbox (Windows; Linux without bwrap).
pub fn wrap(command: &str, cwd: &Path) -> Option<String> {
    let cwd = cwd.canonicalize().ok()?;
    let home = crate::platform::home()?;
    if cfg!(target_os = "macos") && Path::new("/usr/bin/sandbox-exec").exists() {
        return Some(format!("/usr/bin/sandbox-exec -p {} /bin/bash -c {}", q(&profile(&cwd, &home)), q(command)));
    }
    if cfg!(target_os = "linux") && let Some(bwrap) = which("bwrap") {
        // ponytail: untested on a Linux machine (CI has no bwrap); a bwrap that fails fails the command, never widens it
        let c = cwd.to_string_lossy();
        let hidden: Vec<String> = [".ssh", ".aws", ".gnupg", ".config/tazuna"].iter().map(|p| home.join(p)).filter(|p| p.exists())
            .map(|p| format!("--tmpfs {}", q(&p.to_string_lossy()))).collect();
        return Some(format!("{} --ro-bind / / --dev /dev --proc /proc --bind {c2} {c2} --bind /tmp /tmp {} --unshare-net --die-with-parent -- /bin/bash -c {}",
            q(&bwrap), hidden.join(" "), q(command), c2 = q(&c)));
    }
    None
}

fn which(name: &str) -> Option<String> {
    std::env::var_os("PATH").and_then(|p| std::env::split_paths(&p).map(|d| d.join(name)).find(|f| f.is_file())).map(|f| f.to_string_lossy().into_owned())
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
        let _ = std::fs::remove_dir_all(&dir);
    }
}
