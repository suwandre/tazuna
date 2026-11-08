//! What differs between macOS/Linux and Windows, in one place: the home folder, the shell commands run in, stopping a
//! command with everything it started, private files and folders, notifications, copying text and pasting an image,
//! quoting a path for a hook. Windows runs commands in Git Bash, as Claude Code itself does there, so the agent's commands, the test check
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

/// Opens an http(s) address in the default browser (a link clicked in the transcript). The caller checked it with
/// `ui::safe_url`; it goes to the opener as one argument, never through a shell.
pub fn open_url(url: &str) -> std::io::Result<()> {
    if !crate::ui::safe_url(url) {
        return Err(std::io::Error::other("not an http(s) address"));
    }
    #[cfg(target_os = "macos")]
    let mut c = Command::new("open");
    #[cfg(windows)]
    let mut c = {
        let mut c = Command::new("rundll32");
        c.arg("url.dll,FileProtocolHandler");
        c
    };
    #[cfg(not(any(target_os = "macos", windows)))]
    let mut c = Command::new("xdg-open");
    c.arg(url).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().map(|_| ())
}

/// Opens `path` at `line` in the user's editor, detached; returns which editor. TAZUNA_EDITOR is a command template
/// ("code -g {path}:{line}", "idea --line {line} {path}"); otherwise the first of Cursor, VS Code, Zed, Windsurf, Sublime
/// Text or a JetBrains IDE found on PATH, else a plain text editor (macOS TextEdit through `open -t`, Windows Notepad).
/// Never the system's default app: the file is the agent's, and a `.command` or `.app` would run instead of opening.
/// Each word of the template is one argument (no shell): a path with spaces or quotes stays one argument.
pub fn open_in_editor(path: &Path, line: u32) -> std::io::Result<String> {
    let (p, l) = (path.to_string_lossy().into_owned(), line.max(1).to_string());
    let fill = |w: &str| w.replace("{path}", &p).replace("{line}", &l);
    let (cmd, args): (String, Vec<String>) = if let Ok(t) = std::env::var("TAZUNA_EDITOR") {
        let mut words = t.split_whitespace().map(fill);
        (words.next().unwrap_or_default(), words.collect())
    } else {
        const KNOWN: [(&str, &str); 9] = [("cursor", "-g {path}:{line}"), ("code", "-g {path}:{line}"), ("zed", "{path}:{line}"), ("windsurf", "-g {path}:{line}"),
            ("subl", "{path}:{line}"), ("idea", "--line {line} {path}"), ("webstorm", "--line {line} {path}"), ("goland", "--line {line} {path}"), ("pycharm", "--line {line} {path}")];
        let on_path = |name: &str| std::env::var_os("PATH").is_some_and(|ps| std::env::split_paths(&ps).any(|d| d.join(name).is_file() || d.join(format!("{name}.cmd")).is_file()));
        match KNOWN.iter().find(|(n, _)| on_path(n)) {
            Some((n, a)) => (n.to_string(), a.split_whitespace().map(fill).collect()),
            #[cfg(target_os = "macos")]
            None => ("open".into(), vec!["-t".into(), p.clone()]),
            #[cfg(windows)]
            None => ("notepad".into(), vec![p.clone()]),
            #[cfg(not(any(target_os = "macos", windows)))]
            None => return Err(std::io::Error::other("no editor found: set TAZUNA_EDITOR")),
        }
    };
    Command::new(&cmd).args(&args).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn()?;
    Ok(cmd)
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

/// Where `copy_text` put the text.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Copied {
    Clipboard,
    /// Handed to the terminal (OSC 52), which cannot say whether it took it.
    Terminal,
}

impl Copied {
    /// What a "copied N chars" message adds.
    pub fn how(self) -> &'static str {
        match self {
            Copied::Clipboard => "",
            Copied::Terminal => " (through the terminal)",
        }
    }
}

/// Puts `text` on the clipboard. Over SSH this machine's clipboard is the wrong one, so the terminal gets it (OSC 52:
/// Windows Terminal, iTerm2, kitty, WezTerm, Ghostty; tmux with `set-clipboard on`). Otherwise the system's: pbcopy on
/// macOS, the clipboard API on Windows, wl-copy, xclip or xsel on Linux; the terminal when none of them takes it. Copying
/// ran pbcopy everywhere until 0.1.77: "pbcopy: program not found" on Windows.
pub fn copy_text(text: &str) -> std::io::Result<Copied> {
    let remote = std::env::var_os("SSH_TTY").is_some() || std::env::var_os("SSH_CONNECTION").is_some();
    if !remote && system_copy(text).is_ok() {
        return Ok(Copied::Clipboard);
    }
    use std::io::Write;
    let mut out = std::io::stdout().lock();
    write!(out, "\x1b]52;c;{}\x07", base64(text.as_bytes()))?;
    out.flush()?;
    Ok(Copied::Terminal)
}

fn system_copy(text: &str) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    return pipe_to("pbcopy", &[], text);
    #[cfg(windows)]
    return windows_clipboard(text);
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        if std::env::var_os("WAYLAND_DISPLAY").is_some() && pipe_to("wl-copy", &[], text).is_ok() {
            return Ok(());
        }
        pipe_to("xclip", &["-selection", "clipboard"], text).or_else(|_| pipe_to("xsel", &["--clipboard", "--input"], text))
    }
}

/// `text` on the standard input of `cmd`, which must exit 0.
#[cfg(not(windows))]
fn pipe_to(cmd: &str, args: &[&str], text: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new(cmd).args(args).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn()?;
    child.stdin.take().ok_or_else(|| std::io::Error::other("no stdin"))?.write_all(text.as_bytes())?;
    let status = child.wait()?;
    if status.success() { Ok(()) } else { Err(std::io::Error::other(format!("{cmd}: {status}"))) }
}

/// The text as CF_UNICODETEXT through the clipboard API. Not clip.exe: it guesses the input's encoding and guessed wrong
/// both ways on Windows 11 (2026-11-08): UTF-8 "Bush hid the facts" came out as CJK, UTF-16 "ä" alone as mojibake, and a
/// byte-order mark stays in as a character.
#[cfg(windows)]
fn windows_clipboard(text: &str) -> std::io::Result<()> {
    use std::ffi::c_void;
    #[link(name = "user32")]
    unsafe extern "system" {
        fn OpenClipboard(owner: *mut c_void) -> i32;
        fn EmptyClipboard() -> i32;
        fn SetClipboardData(format: u32, mem: *mut c_void) -> *mut c_void;
        fn CloseClipboard() -> i32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GlobalAlloc(flags: u32, bytes: usize) -> *mut c_void;
        fn GlobalLock(mem: *mut c_void) -> *mut c_void;
        fn GlobalUnlock(mem: *mut c_void) -> i32;
        fn GlobalFree(mem: *mut c_void) -> *mut c_void;
    }
    const CF_UNICODETEXT: u32 = 13;
    const GMEM_MOVEABLE: u32 = 0x0002;
    // Windows text has CRLF line ends; NUL-terminated UTF-16
    let wide: Vec<u16> = text.replace("\r\n", "\n").replace('\n', "\r\n").encode_utf16().chain([0]).collect();
    // another program can hold the clipboard for a moment
    let mut tries = 0;
    while unsafe { OpenClipboard(std::ptr::null_mut()) } == 0 {
        tries += 1;
        if tries == 10 {
            return Err(std::io::Error::last_os_error());
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let put = || -> std::io::Result<()> {
        unsafe {
            if EmptyClipboard() == 0 {
                return Err(std::io::Error::last_os_error());
            }
            let mem = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2);
            if mem.is_null() {
                return Err(std::io::Error::last_os_error());
            }
            let p = GlobalLock(mem) as *mut u16;
            if p.is_null() {
                GlobalFree(mem);
                return Err(std::io::Error::last_os_error());
            }
            std::ptr::copy_nonoverlapping(wide.as_ptr(), p, wide.len());
            GlobalUnlock(mem);
            // the clipboard owns the memory once this succeeds
            if SetClipboardData(CF_UNICODETEXT, mem).is_null() {
                let e = std::io::Error::last_os_error();
                GlobalFree(mem);
                return Err(e);
            }
            Ok(())
        }
    };
    let r = put();
    unsafe { CloseClipboard() };
    r
}

/// Standard base64 with padding, as OSC 52 carries the text.
fn base64(b: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(b.len().div_ceil(3) * 4);
    for c in b.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            s.push(if i <= c.len() { A[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    s
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
    fn base64_pads_like_the_standard() {
        assert_eq!([&b""[..], b"f", b"fo", b"foo", b"foob", b"fooba", b"foobar"].map(base64), ["", "Zg==", "Zm8=", "Zm9v", "Zm9vYg==", "Zm9vYmE=", "Zm9vYmFy"]);
        assert_eq!(base64("Grüße 🙂".as_bytes()), "R3LDvMOfZSDwn5mC");
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
