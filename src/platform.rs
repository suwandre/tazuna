//! What differs between macOS/Linux and Windows, in one place: the home folder, the shell commands run in, stopping a
//! command with everything it started, private files and folders, notifications, the clipboard image, quoting a path for
//! a hook. Windows runs commands in Git Bash, as Claude Code itself does there, so the agent's commands, the test check
//! and `!` read the same on every platform.

use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The user's home: HOME, or USERPROFILE on Windows. Data lives under it everywhere (~/.local/share/tazuna,
/// ~/.config/tazuna), so one machine's folders can be synced to another as they are.
pub fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).filter(|h| !h.is_empty()).map(PathBuf::from)
}

/// Git Bash: TAZUNA_BASH, CLAUDE_CODE_GIT_BASH_PATH (Claude Code's own setting), Git's usual install folders; never
/// System32's bash.exe, which starts WSL instead. The script and its arguments go in an environment variable: on the
/// command line MSYS would re-parse quotes and backslashes its own way and expand wildcards (a failing CI run showed both).
/// The fixed command line turns off splitting and wildcards just long enough to hand the script to eval.
#[cfg(windows)]
fn git_bash(login: bool, script: &str, args: &[&OsStr]) -> Command {
    let found = [std::env::var_os("TAZUNA_BASH"), std::env::var_os("CLAUDE_CODE_GIT_BASH_PATH")]
        .into_iter()
        .flatten()
        .map(PathBuf::from)
        .chain(["C:\\Program Files\\Git\\bin\\bash.exe", "C:\\Program Files (x86)\\Git\\bin\\bash.exe"].map(PathBuf::from))
        .find(|p| p.is_file());
    let mut c = Command::new(found.unwrap_or_else(|| PathBuf::from("bash.exe")));
    let words: Vec<String> = args.iter().map(|a| format!("'{}'", a.to_string_lossy().replace('\'', r"'\''"))).collect();
    c.env("CHERE_INVOKING", "1") // a login shell stays in the folder it was started in
        .env("TAZUNA_SH", format!("set +f; unset IFS TAZUNA_SH; set -- {}\n{script}", words.join(" ")))
        .args([if login { "-lc" } else { "-c" }, "set -f;IFS=;eval $TAZUNA_SH"]);
    c
}

/// A command line through the shell: `sh -c` on Unix, Git Bash on Windows. `args` become $1, $2 ... (never part of the
/// command line itself, so a path with spaces or quotes stays one word).
pub fn shell(cmd: &str, args: &[&OsStr]) -> Command {
    #[cfg(windows)]
    return git_bash(false, cmd, args);
    #[cfg(not(windows))]
    {
        let mut c = Command::new("sh");
        c.arg("-c").arg(cmd).arg("sh").args(args);
        c
    }
}

/// `bash -lc`: the agent's commands, in a login shell like a terminal's.
pub fn login_bash(cmd: &str) -> Command {
    #[cfg(windows)]
    return git_bash(true, cmd, &[]);
    #[cfg(not(windows))]
    {
        let mut c = Command::new("bash");
        c.args(["-lc", cmd]);
        c
    }
}

/// grep, find and the like with their GNU flags: on Windows Git's, through Git Bash (Windows has a `find` of its own).
pub fn tool(name: &str, args: &[&OsStr]) -> Command {
    #[cfg(windows)]
    return git_bash(false, &format!("exec /usr/bin/{name} \"$@\""), args);
    #[cfg(not(windows))]
    {
        let mut c = Command::new(name);
        c.args(args);
        c
    }
}

/// A path as tools print it: on Windows without the \\?\ prefix canonicalize adds, with forward slashes (Git's tools
/// take and print C:/...).
pub fn plain(p: &Path) -> String {
    let s = p.display().to_string();
    #[cfg(windows)]
    let s = s.strip_prefix(r"\\?\").unwrap_or(&s).replace('\\', "/");
    s
}

/// In a process group of its own, so `kill_tree` can stop everything it starts.
pub fn own_group(cmd: &mut Command) -> &mut Command {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0)
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0000_0200) // CREATE_NEW_PROCESS_GROUP
    }
}

/// Stops a process started with `own_group` and everything under it.
pub fn kill_tree(pid: u32) {
    #[cfg(unix)]
    let _ = Command::new("kill").args(["-KILL", "--", &format!("-{pid}")]).status();
    #[cfg(windows)]
    let _ = Command::new("taskkill").args(["/T", "/F", "/PID", &pid.to_string()]).output();
}

/// Stops one process.
pub fn kill(pid: u32) {
    #[cfg(unix)]
    let _ = Command::new("kill").args(["-KILL", &pid.to_string()]).status();
    #[cfg(windows)]
    let _ = Command::new("taskkill").args(["/F", "/PID", &pid.to_string()]).output();
}

/// A folder only the user can open: 0700 on Unix; on Windows the user's profile and temp folders are private already.
pub fn private_dir(path: &Path, recursive: bool) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new().mode(0o700).recursive(recursive).create(path)
    }
    #[cfg(not(unix))]
    std::fs::DirBuilder::new().recursive(recursive).create(path)
}

/// An existing folder made private too (0700 on Unix).
pub fn make_private(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700));
    }
    #[cfg(not(unix))]
    let _ = path;
}

/// A file only the user can read (0600 on Unix). `create_new`: fails when something is already there, so a planted file
/// or symlink is never written through.
pub fn private_file(path: &Path, create_new: bool) -> std::io::Result<File> {
    let mut o = OpenOptions::new();
    o.write(true);
    if create_new {
        o.create_new(true);
    } else {
        o.create(true).truncate(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    o.open(path)
}

/// A desktop notification: macOS through osascript; elsewhere none (the session shows the same line).
pub fn notify(title: &str, text: &str) {
    #[cfg(target_os = "macos")]
    {
        let esc = |s: &str| s.replace(['"', '\\'], "'");
        let script = format!("display notification \"{}\" with title \"{}\"", esc(text), esc(title));
        let _ = Command::new("osascript").args(["-e", &script]).output();
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (title, text);
}

/// Saves the clipboard's image as a PNG at `path` (macOS: osascript; Windows: PowerShell). Nothing is written when the
/// clipboard holds no image.
pub fn clipboard_image(path: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "set f to (open for access POSIX file \"{}\" with write permission)\ntry\nwrite (the clipboard as «class PNGf») to f\nend try\nclose access f",
            path.display()
        );
        Command::new("osascript").args(["-e", &script]).output().map(|_| ())
    }
    #[cfg(windows)]
    {
        // the path goes in as an argument ($args[0]), not into the script text
        let script = "Add-Type -AssemblyName System.Windows.Forms; $i = [System.Windows.Forms.Clipboard]::GetImage(); \
                      if ($i) { $i.Save($args[0], [System.Drawing.Imaging.ImageFormat]::Png) }";
        Command::new("powershell").args(["-NoProfile", "-STA", "-Command", script]).arg(path).output().map(|_| ())
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = path;
        Err(std::io::Error::other("pasting an image works on macOS and Windows"))
    }
}

/// A path as one word in a command claude runs through its shell (the diagnostics and boss hooks): single-quoted for
/// bash; on Windows with forward slashes, which Git Bash reads as the same path, where backslashes would be escapes.
pub fn sh_quote(p: &Path) -> String {
    let s = p.to_string_lossy();
    #[cfg(windows)]
    let s = s.replace('\\', "/");
    format!("'{}'", s.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_args_stay_one_word_and_quoting_survives() {
        let out = shell("printf '%s|' \"$1\" \"$2\" *.none", &["a b".as_ref(), "it's \"q\" C:\\x".as_ref()]).output().unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout), "a b|it's \"q\" C:\\x|*.none|");
        let q = sh_quote(Path::new("/tmp/it's here/tazuna"));
        let out = shell(&format!("printf '%s' {q}"), &[]).output().unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout), "/tmp/it's here/tazuna");
        assert!(home().is_some());
    }

    #[test]
    fn kill_tree_stops_what_the_command_started() {
        let mut c = shell("sleep 30 & sleep 30; wait", &[]);
        let mut child = own_group(&mut c).spawn().unwrap();
        let t0 = std::time::Instant::now();
        kill_tree(child.id());
        child.wait().unwrap();
        assert!(t0.elapsed() < std::time::Duration::from_secs(5));
    }
}
