//! "Verified, not claimed": when claude says it is done, run the tests for what the run changed. Failures go back to
//! claude as the next message (a few rounds at most); the run log records whether the work was verified.

use std::{
    io::Read,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[derive(Clone)]
pub enum Mode {
    Auto,
    Command(String),
    Off,
}

pub struct Outcome {
    pub cmd: String,
    pub ok: bool,
    pub code: Option<i32>,
    pub secs: f64,
    /// Trimmed to what claude needs to fix the failure.
    pub output: String,
}

/// Extra rounds claude gets to fix failing tests before the run ends as `verify_failed`.
pub const MAX_ROUNDS: u32 = 2;
const TIMEOUT: Duration = Duration::from_secs(600);
const MAX_LINES: usize = 120;
const MAX_CHARS: usize = 8000;

/// The shell command that tests what changed, or None when nothing testable changed.
pub fn command(mode: &Mode, cwd: &Path, changed: &[String]) -> Option<String> {
    match mode {
        Mode::Off => None,
        Mode::Command(c) => Some(c.clone()),
        Mode::Auto => detect(cwd, changed),
    }
}

fn detect(cwd: &Path, changed: &[String]) -> Option<String> {
    let has = |ext: &str| changed.iter().any(|f| f.ends_with(ext));
    if has(".go") {
        // the packages that contain changed Go files (tests of other packages are not this run's business)
        let mut pkgs: Vec<String> = changed
            .iter()
            // the command runs through `sh -c`: only plain path characters, so a file name can't inject shell syntax
            .filter(|f| f.ends_with(".go") && !f.starts_with("vendor/") && safe_path(f) && cwd.join(f).exists())
            .map(|f| Path::new(f).parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default())
            .map(|d| if d.is_empty() { "./".to_string() } else { format!("./{d}/") })
            .collect();
        pkgs.sort();
        pkgs.dedup();
        return (!pkgs.is_empty()).then(|| format!("go test {}", pkgs.join(" ")));
    }
    if has(".rs") && cwd.join("Cargo.toml").exists() {
        return Some("cargo test --quiet".into());
    }
    if has(".py") && ["pytest.ini", "conftest.py", "tests"].iter().any(|f| cwd.join(f).exists()) {
        return Some("python3 -m pytest -q -x".into());
    }
    if [".ts", ".tsx", ".js", ".jsx"].iter().any(|e| has(e)) {
        let pkg = std::fs::read_to_string(cwd.join("package.json")).unwrap_or_default();
        let script = serde_json::from_str::<serde_json::Value>(&pkg).ok().and_then(|p| p["scripts"]["test"].as_str().map(String::from));
        if script.is_some_and(|s| !s.contains("no test specified")) {
            return Some("npm test --silent".into());
        }
    }
    None
}

fn safe_path(p: &str) -> bool {
    !p.is_empty() && !p.starts_with('-') && p.bytes().all(|b| b.is_ascii_alphanumeric() || b"._/-".contains(&b))
}

/// Runs the command (blocking; call from a thread) with a timeout.
pub fn run(cwd: &Path, cmd: &str) -> Outcome {
    let t0 = Instant::now();
    let spawned = Command::new("sh").args(["-c", &format!("{cmd} 2>&1")]).current_dir(cwd).stdin(Stdio::null()).stdout(Stdio::piped()).spawn();
    let mut child = match spawned {
        Ok(c) => c,
        Err(e) => return Outcome { cmd: cmd.into(), ok: false, code: None, secs: 0.0, output: format!("could not start: {e}") },
    };
    let mut stdout = child.stdout.take().expect("piped");
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stdout.read_to_string(&mut s);
        s
    });
    let status = loop {
        if let Ok(Some(st)) = child.try_wait() {
            break Some(st);
        }
        if t0.elapsed() > TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let mut output = reader.join().unwrap_or_default();
    if status.is_none() {
        output.push_str(&format!("\n[tazuna: killed after {}s]", TIMEOUT.as_secs()));
    }
    let code = status.and_then(|s| s.code());
    Outcome { cmd: cmd.into(), ok: code == Some(0), code, secs: t0.elapsed().as_secs_f64(), output: trim(&output) }
}

/// Keeps failures readable and short: Go's per-test noise (=== RUN, --- PASS, ok lines) is dropped, then the head and
/// tail are kept within the line and character limits.
fn trim(out: &str) -> String {
    let lines: Vec<&str> = out
        .lines()
        .filter(|l| !crate::outcap::is_test_noise(l))
        .collect();
    let kept: Vec<&str> = if lines.len() > MAX_LINES {
        let (head, tail) = (20, MAX_LINES - 20);
        let mut v = lines[..head].to_vec();
        v.push("[... trimmed ...]");
        v.extend_from_slice(&lines[lines.len() - tail..]);
        v
    } else {
        lines
    };
    let s = kept.join("\n");
    if s.len() <= MAX_CHARS {
        return s;
    }
    // keep the end (where test summaries are), cut on a character boundary
    let cut = s.char_indices().map(|(i, _)| i).find(|&i| i >= s.len() - MAX_CHARS).unwrap_or(0);
    format!("[... trimmed ...]\n{}", &s[cut..])
}

/// The message that sends a failure back to claude.
pub fn feedback(o: &Outcome, round: u32) -> String {
    format!(
        "Verification failed (round {round} of {MAX_ROUNDS}): `{}` exited with {}.\n\n```\n{}\n```\n\nFix the cause of these failures, run the tests again, and finish with a short summary.",
        o.cmd,
        o.code.map_or("a timeout".into(), |c| format!("code {c}")),
        o.output
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_go_packages_of_changed_files() {
        let dir = std::env::temp_dir().join("tazuna-verify-test");
        std::fs::create_dir_all(dir.join("pkg/a")).unwrap();
        for f in ["pkg/a/x.go", "pkg/a/x_test.go", "main.go"] {
            std::fs::write(dir.join(f), "package x").unwrap();
        }
        let changed = vec!["pkg/a/x.go".into(), "pkg/a/x_test.go".into(), "main.go".into(), "README.md".into(), "gone.go".into()];
        assert_eq!(detect(&dir, &changed).unwrap(), "go test ./ ./pkg/a/");
        assert_eq!(detect(&dir, &["README.md".into()]), None);
        std::fs::create_dir_all(dir.join("x;rm -rf ~")).unwrap();
        std::fs::write(dir.join("x;rm -rf ~/a.go"), "package x").unwrap();
        assert_eq!(detect(&dir, &["x;rm -rf ~/a.go".into()]), None, "unsafe path characters are never put in the shell command");
        assert_eq!(command(&Mode::Off, &dir, &changed), None);
        assert_eq!(command(&Mode::Command("make test".into()), &dir, &[]).unwrap(), "make test");
    }

    #[test]
    fn trims_go_noise_and_runs_commands() {
        let o = run(Path::new("."), "printf '=== RUN   TestA\\n--- PASS: TestA\\n--- FAIL: TestB\\n    b_test.go:9: want 1\\nFAIL\\n'; exit 1");
        assert!(!o.ok);
        assert_eq!(o.code, Some(1));
        assert_eq!(o.output, "--- FAIL: TestB\n    b_test.go:9: want 1\nFAIL");
        assert!(run(Path::new("."), "true").ok);
    }
}
