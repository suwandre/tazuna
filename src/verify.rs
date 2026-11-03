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
    /// The test runner found no tests (pytest and unittest exit 5): nothing was verified, nothing failed.
    pub no_tests: bool,
    /// The tests ran past the time limit and were killed (0.0.89): no result, so no fix round. Before, a whole Go
    /// package that took over 600 s under load came back as a failure and the model hunted bugs that were not there
    /// (three prometheus runs, 2026-10-26).
    pub timed_out: bool,
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
    if has(".py") && has_python_tests(cwd) {
        let pytest = Command::new("python3").args(["-c", "import pytest"]).stdin(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success());
        let uv = cwd.join("pyproject.toml").exists() && Command::new("uv").arg("--version").stdout(Stdio::null()).status().is_ok_and(|s| s.success());
        return Some(
            if pytest { "python3 -m pytest -q -x" } else if uv { "uv run -q pytest -q -x" } else { "python3 -m unittest discover -q" }.into(),
        );
    }
    if JS.iter().any(|e| has(e)) {
        if npm_script(cwd, "test").is_some_and(|s| !s.contains("no test specified")) {
            return Some("npm test --silent".into());
        }
        // no root test script (a monorepo whose packages test themselves): the nearest package of each changed file
        let mut pkgs: Vec<String> = changed
            .iter()
            .filter(|f| JS.iter().any(|e| f.ends_with(e)) && safe_path(f))
            .filter_map(|f| Path::new(f).ancestors().skip(1).take_while(|d| !d.as_os_str().is_empty()).find(|d| {
                npm_script(&cwd.join(d), "test").is_some_and(|s| !s.contains("no test specified"))
            }).map(|d| d.to_string_lossy().into_owned()))
            .collect();
        pkgs.sort();
        pkgs.dedup();
        return (!pkgs.is_empty()).then(|| pkgs.iter().map(|d| format!("npm --prefix {d} test --silent")).collect::<Vec<_>>().join(" && "));
    }
    None
}

const JS: [&str; 6] = [".ts", ".tsx", ".js", ".jsx", ".mts", ".cts"];

fn npm_script(dir: &Path, name: &str) -> Option<String> {
    let pkg = std::fs::read_to_string(dir.join("package.json")).ok()?;
    serde_json::from_str::<serde_json::Value>(&pkg).ok()?["scripts"][name].as_str().map(String::from)
}

/// The repo's own type check (its `typecheck` script). Tests alone miss type errors: runners like tsx strip types.
pub fn typecheck_command(mode: &Mode, cwd: &Path) -> Option<String> {
    if !matches!(mode, Mode::Auto) {
        return None;
    }
    ["typecheck", "type-check"].into_iter().find(|s| npm_script(cwd, s).is_some()).map(|s| format!("npm run {s} --silent"))
}

/// Whether a change can affect types (worth running the type check after it).
pub fn touches_types(changed: &[String]) -> bool {
    changed.iter().any(|f| JS.iter().any(|e| f.ends_with(e)) || f.ends_with("tsconfig.json") || f.ends_with("package.json"))
}

/// Type errors in `after` that `before` did not have. Errors are compared without their line and column, so code that
/// only moved is not new; many repos carry known errors, so a failing type check alone means nothing.
pub fn new_type_errors(before: &str, after: &str) -> Vec<String> {
    let key = |l: &str| {
        let i = l.find("): error TS")?;
        let path = &l[..l[..i].rfind('(')?];
        Some(format!("{path}: {}", &l[i + 3..]))
    };
    let mut known: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for k in before.lines().filter_map(key) {
        *known.entry(k).or_default() += 1;
    }
    after
        .lines()
        .filter(|l| match key(l) {
            Some(k) => match known.get_mut(&k) {
                Some(n) if *n > 0 => {
                    *n -= 1;
                    false
                }
                _ => true,
            },
            None => false,
        })
        .map(String::from)
        .collect()
}

/// Runs the tests and the type check side by side and folds them into one outcome. `types` is the type check command
/// and its output from before the run.
pub fn check(cwd: &Path, tests: Option<String>, types: Option<(String, String)>) -> Outcome {
    let tc = types.map(|(cmd, before)| {
        let cwd = cwd.to_path_buf();
        std::thread::spawn(move || {
            let (_, after, secs) = run_raw(&cwd, &cmd);
            (cmd, new_type_errors(&before, &after), secs)
        })
    });
    let mut o = tests.map(|c| run(cwd, &c));
    let Some((cmd, errors, secs)) = tc.and_then(|h| h.join().ok()) else {
        return o.unwrap_or_else(|| Outcome { cmd: String::new(), ok: true, no_tests: true, timed_out: false, code: None, secs: 0.0, output: String::new() });
    };
    let base = o.take().unwrap_or(Outcome { cmd: String::new(), ok: true, no_tests: false, timed_out: false, code: Some(0), secs: 0.0, output: String::new() });
    let cmd = if base.cmd.is_empty() { cmd } else { format!("{} + {cmd}", base.cmd) };
    if errors.is_empty() {
        return Outcome { cmd, no_tests: false, secs: base.secs.max(secs), ..base };
    }
    let shown: Vec<&str> = errors.iter().take(40).map(String::as_str).collect();
    let mut output = format!("{} new type error(s):\n{}", errors.len(), shown.join("\n"));
    if !base.ok {
        output = format!("{}\n\n{output}", base.output);
    }
    Outcome { cmd, ok: false, no_tests: false, timed_out: false, code: base.code.filter(|&c| c != 0).or(Some(1)), secs: base.secs.max(secs), output: trim(&output) }
}

/// Whether verify can test this repo at all: a command given, or one it would find (a Go module, a Cargo crate, Python
/// tests, a package.json test script at the root or in a package up to three folders down). Test scope tells claude that
/// tazuna runs the tests at the end; without a test command that would not happen.
pub fn covers(mode: &Mode, cwd: &Path) -> bool {
    match mode {
        Mode::Off => false,
        Mode::Command(_) => true,
        Mode::Auto => ["go.mod", "go.work", "Cargo.toml"].iter().any(|f| cwd.join(f).exists()) || has_python_tests(cwd) || js_tests(cwd, 3),
    }
}

fn js_tests(dir: &Path, depth: u32) -> bool {
    if npm_script(dir, "test").is_some_and(|s| !s.contains("no test specified")) {
        return true;
    }
    depth > 0
        && std::fs::read_dir(dir).into_iter().flatten().flatten().any(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            e.file_type().is_ok_and(|t| t.is_dir()) && !name.starts_with('.') && name != "node_modules" && js_tests(&e.path(), depth - 1)
        })
}

fn has_python_tests(cwd: &Path) -> bool {
    let named_test = |n: &str| n.ends_with(".py") && (n.starts_with("test_") || n.ends_with("_test.py"));
    ["pytest.ini", "conftest.py", "tests"].iter().any(|f| cwd.join(f).exists())
        || std::fs::read_dir(cwd).into_iter().flatten().flatten().any(|e| named_test(&e.file_name().to_string_lossy()))
}

pub(crate) fn safe_path(p: &str) -> bool {
    !p.is_empty() && !p.starts_with('-') && p.bytes().all(|b| b.is_ascii_alphanumeric() || b"._/-".contains(&b))
}

/// Runs the command (blocking; call from a thread) with a timeout.
pub fn run(cwd: &Path, cmd: &str) -> Outcome {
    run_for(cwd, cmd, TIMEOUT)
}

fn run_for(cwd: &Path, cmd: &str, limit: Duration) -> Outcome {
    let (code, output, secs) = run_limited(cwd, cmd, limit);
    let no_tests = code == Some(5) && (cmd.contains("pytest") || cmd.contains("unittest"));
    let timed_out = code.is_none() && secs >= limit.as_secs_f64();
    Outcome { cmd: cmd.into(), ok: code == Some(0), no_tests, timed_out, code, secs, output: trim(&output) }
}

/// Exit code (None: did not start or timed out), full output, seconds.
pub fn run_raw(cwd: &Path, cmd: &str) -> (Option<i32>, String, f64) {
    run_limited(cwd, cmd, TIMEOUT)
}

fn run_limited(cwd: &Path, cmd: &str, limit: Duration) -> (Option<i32>, String, f64) {
    let t0 = Instant::now();
    let spawned = crate::platform::shell(&format!("{cmd} 2>&1"), &[]).current_dir(cwd).stdin(Stdio::null()).stdout(Stdio::piped()).spawn();
    let mut child = match spawned {
        Ok(c) => c,
        Err(e) => return (None, format!("could not start: {e}"), 0.0),
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
        if t0.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let mut output = reader.join().unwrap_or_default();
    if status.is_none() {
        output.push_str(&format!("\n[tazuna: killed after {}s]", limit.as_secs()));
    }
    (status.and_then(|s| s.code()), output, t0.elapsed().as_secs_f64())
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
    fn covers_only_repos_it_can_test() {
        let dir = std::env::temp_dir().join(format!("tazuna-covers-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("packages/web/node_modules/x")).unwrap();
        assert!(!covers(&Mode::Auto, &dir), "nothing to test");
        assert!(covers(&Mode::Command("make check".into()), &dir) && !covers(&Mode::Off, &dir));
        std::fs::write(dir.join("packages/web/node_modules/x/package.json"), r#"{"scripts":{"test":"jest"}}"#).unwrap();
        assert!(!covers(&Mode::Auto, &dir), "node_modules does not count");
        std::fs::write(dir.join("package.json"), r#"{"scripts":{"test":"echo \"Error: no test specified\" && exit 1"}}"#).unwrap();
        assert!(!covers(&Mode::Auto, &dir), "npm's placeholder is no test");
        std::fs::write(dir.join("packages/web/package.json"), r#"{"scripts":{"test":"vitest run"}}"#).unwrap();
        assert!(covers(&Mode::Auto, &dir), "a package's own tests");
        std::fs::remove_dir_all(&dir).unwrap();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("go.mod"), "module x").unwrap();
        assert!(covers(&Mode::Auto, &dir));
        std::fs::remove_dir_all(&dir).unwrap();
    }

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
    fn only_new_type_errors_count() {
        let before = "src/a.ts(3,5): error TS2322: Type 'x' is not assignable.\n  Type 'undefined' is not assignable.\nsrc/b.ts(9,1): error TS2304: Cannot find name 'y'.";
        // a.ts's known error moved lines, b.ts's error is fixed, one new error in a.ts, one duplicate of a known message
        let after = "src/a.ts(7,5): error TS2322: Type 'x' is not assignable.\nsrc/a.ts(12,2): error TS2304: Cannot find name 'z'.\nsrc/a.ts(20,5): error TS2322: Type 'x' is not assignable.";
        assert_eq!(new_type_errors(before, after), vec!["src/a.ts(12,2): error TS2304: Cannot find name 'z'.", "src/a.ts(20,5): error TS2322: Type 'x' is not assignable."]);
        assert_eq!(new_type_errors(after, before), vec!["src/b.ts(9,1): error TS2304: Cannot find name 'y'."], "continuation lines are ignored");
        assert!(new_type_errors(before, before).is_empty());
    }

    #[test]
    fn checks_types_beside_tests() {
        let dir = std::env::temp_dir().join(format!("tazuna-verify-types-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let before = "a.ts(1,1): error TS1: old".to_string();
        let tc = |out: &str| Some((format!("printf '{out}\\n'; exit 2"), before.clone()));
        let o = check(&dir, Some("true".into()), tc("a.ts(5,1): error TS1: old"));
        assert!(o.ok, "a known error is not a failure even though the type check exits non-zero");
        assert_eq!(o.cmd, format!("true + printf 'a.ts(5,1): error TS1: old\\n'; exit 2"));
        let o = check(&dir, Some("true".into()), tc("a.ts(5,1): error TS2: new"));
        assert!(!o.ok);
        assert!(o.output.starts_with("1 new type error(s):\na.ts(5,1): error TS2: new"), "{}", o.output);
        let o = check(&dir, None, tc(""));
        assert!(o.ok && !o.no_tests, "the type check alone verifies");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn tests_packages_when_the_root_has_no_test_script() {
        let dir = std::env::temp_dir().join(format!("tazuna-verify-npm-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("api/src")).unwrap();
        std::fs::create_dir_all(dir.join("web/src")).unwrap();
        std::fs::write(dir.join("package.json"), r#"{"scripts": {"typecheck": "tsc"}}"#).unwrap();
        std::fs::write(dir.join("api/package.json"), r#"{"scripts": {"test": "node --test"}}"#).unwrap();
        std::fs::write(dir.join("web/package.json"), r#"{"scripts": {"test": "echo \"Error: no test specified\" && exit 1"}}"#).unwrap();
        let changed = vec!["api/src/x.ts".into(), "api/src/y.ts".into(), "web/src/z.ts".into()];
        assert_eq!(detect(&dir, &changed).unwrap(), "npm --prefix api test --silent");
        assert_eq!(typecheck_command(&Mode::Auto, &dir).unwrap(), "npm run typecheck --silent");
        assert_eq!(typecheck_command(&Mode::Off, &dir), None);
        assert!(touches_types(&changed) && !touches_types(&["README.md".into()]));
        std::fs::write(dir.join("package.json"), r#"{"scripts": {"test": "vitest run"}}"#).unwrap();
        assert_eq!(detect(&dir, &changed).unwrap(), "npm test --silent", "a root test script covers the whole repo");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn trims_go_noise_and_runs_commands() {
        let o = run(Path::new("."), "printf '=== RUN   TestA\\n--- PASS: TestA\\n--- FAIL: TestB\\n    b_test.go:9: want 1\\nFAIL\\n'; exit 1");
        assert!(!o.ok);
        assert_eq!(o.code, Some(1));
        assert_eq!(o.output, "--- FAIL: TestB\n    b_test.go:9: want 1\nFAIL");
        assert!(run(Path::new("."), "true").ok);
    }

    #[test]
    fn a_run_past_the_limit_is_no_result_not_a_failure() {
        let o = run_for(Path::new("."), "sleep 5", Duration::from_millis(300));
        assert!(o.timed_out && !o.ok && o.code.is_none(), "killed at the limit");
        assert!(!run_for(Path::new("."), "exit 1", Duration::from_secs(5)).timed_out, "a failure is a failure");
    }
}
