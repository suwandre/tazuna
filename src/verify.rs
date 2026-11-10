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
    /// Go tests that failed, top-level names ("(build)" for a package that did not build), from the untrimmed output.
    pub failed: Vec<String>,
    /// Of those, the ones that fail on the last commit too: not the change's doing, so not sent back as its failure.
    pub preexisting: Vec<String>,
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
    let mut o = tests.map(|c| preexisting(run(cwd, &c), |o| failing_before(cwd, &o.cmd, &o.failed)));
    let Some((cmd, errors, secs)) = tc.and_then(|h| h.join().ok()) else {
        return o.unwrap_or_else(|| Outcome { cmd: String::new(), ok: true, no_tests: true, timed_out: false, code: None, secs: 0.0, output: String::new(), failed: vec![], preexisting: vec![] });
    };
    let base = o.take().unwrap_or(Outcome { cmd: String::new(), ok: true, no_tests: false, timed_out: false, code: Some(0), secs: 0.0, output: String::new(), failed: vec![], preexisting: vec![] });
    let cmd = if base.cmd.is_empty() { cmd } else { format!("{} + {cmd}", base.cmd) };
    if errors.is_empty() {
        return Outcome { cmd, no_tests: false, secs: base.secs.max(secs), ..base };
    }
    let shown: Vec<&str> = errors.iter().take(40).map(String::as_str).collect();
    let mut output = format!("{} new type error(s):\n{}", errors.len(), shown.join("\n"));
    if !base.ok {
        output = format!("{}\n\n{output}", base.output);
    }
    Outcome { cmd, ok: false, no_tests: false, timed_out: false, code: base.code.filter(|&c| c != 0).or(Some(1)), secs: base.secs.max(secs), output: trim(&output), ..base }
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
    Outcome { cmd: cmd.into(), ok: code == Some(0), no_tests, timed_out, code, secs, failed: go_failures(&output), preexisting: vec![], output: trim(&output) }
}

/// The Go tests a `go test` output reports failing, by top-level name (a subtest counts as its test), once each; a
/// package that did not build or set up counts as "(build)", which no earlier run explains away.
fn go_failures(out: &str) -> Vec<String> {
    let mut v: Vec<String> = vec![];
    for l in out.lines() {
        let name = if let Some(rest) = l.trim_start().strip_prefix("--- FAIL: ") {
            rest.split([' ', '/']).next().unwrap_or_default().to_string()
        } else if l.starts_with("FAIL") && (l.contains("[build failed]") || l.contains("[setup failed]")) {
            "(build)".to_string()
        } else {
            continue;
        };
        if !name.is_empty() && !v.contains(&name) {
            v.push(name);
        }
    }
    v
}

/// A failed verify whose failing tests also fail on the last commit is not the change's doing: those go back to the
/// agent as "not yours", and when all of them do, the run counts as verified. k6's TestStdLogOutputIsSet times out
/// behind the bench's proxy on untouched code too; tazuna sent it back twice in every run, the agent proved twice by hand
/// that it failed without its change, and one run cleared the proxy variables in the test to make it pass (2026-11-10).
fn preexisting(mut o: Outcome, before: impl FnOnce(&Outcome) -> Option<Vec<String>>) -> Outcome {
    if o.ok || o.timed_out || o.failed.is_empty() || o.failed.iter().any(|t| t == "(build)") {
        return o;
    }
    let Some(old) = before(&o).filter(|old| !old.is_empty()) else { return o };
    let new: Vec<&String> = o.failed.iter().filter(|t| !old.contains(t)).collect();
    if new.is_empty() {
        o.ok = true;
        o.output = format!("{} fail on the last commit too, without this change: left alone", old.join(", "));
    } else {
        o.output = format!("{} also fail on the last commit, without your change: not yours to fix. Fix the others.\n\n{}", old.join(", "), o.output);
    }
    o.preexisting = old;
    o
}

/// Of `tests` (failing now), those that fail on the last commit as well: the same packages in a git worktree of HEAD.
/// Only for verify's own `go test ./pkg/` commands; None where it cannot tell (no git, no worktree). Remembered for the
/// session: the next fix round does not check the same test again.
// ponytail: HEAD, not the tree as the session found it; changes the user had not committed count as the agent's
fn failing_before(cwd: &Path, cmd: &str, tests: &[String]) -> Option<Vec<String>> {
    static KNOWN: std::sync::Mutex<Vec<(std::path::PathBuf, String)>> = std::sync::Mutex::new(Vec::new());
    let pkgs = cmd.strip_prefix("go test ").filter(|p| p.split(' ').all(|p| p.starts_with("./") && safe_path(p)))?;
    if !tests.iter().all(|t| !t.is_empty() && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')) {
        return None;
    }
    let known: Vec<String> = KNOWN.lock().ok()?.iter().filter(|(d, _)| d == cwd).map(|(_, t)| t.clone()).collect();
    let ask: Vec<&String> = tests.iter().filter(|t| !known.contains(t)).collect();
    let mut old: Vec<String> = tests.iter().filter(|t| known.contains(t)).cloned().collect();
    if !ask.is_empty() {
        let dir = std::env::temp_dir().join(format!("tazuna-base-{}-{}", std::process::id(), crate::platform::random::<4>().iter().map(|b| format!("{b:02x}")).collect::<String>()));
        let git = |args: &[&str]| Command::new("git").args(args).current_dir(cwd).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();
        if !git(&["worktree", "add", "--detach", "-q", &dir.to_string_lossy(), "HEAD"]).is_ok_and(|s| s.success()) {
            return None;
        }
        let pattern = ask.iter().map(|t| t.as_str()).collect::<Vec<_>>().join("|");
        let (_, out, _) = run_limited(&dir, &format!("go test -count=1 -run '^({pattern})$' {pkgs}"), TIMEOUT);
        let _ = git(&["worktree", "remove", "--force", &dir.to_string_lossy()]);
        let failed = go_failures(&out);
        let mut k = KNOWN.lock().ok()?;
        for t in ask.into_iter().filter(|t| failed.contains(t)) {
            k.push((cwd.to_path_buf(), t.clone()));
            old.push(t.clone());
        }
    }
    Some(old)
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
        "Verification failed (round {round} of {MAX_ROUNDS}): `{}` exited with {}.\n\n```\n{}\n```\n\nFix the cause of these failures, run the tests again, and finish with a short summary.{}",
        o.cmd,
        o.code.map_or("a timeout".into(), |c| format!("code {c}")),
        o.output,
        if std::env::var_os("TAZUNA_FIX_CODE").is_some() { FIX_CODE } else { "" }
    )
}

/// Experiment (TAZUNA_FIX_CODE): Opus at low broke hugo's `return add . 42` syntax in two runs and rewrote the existing
/// test that caught it, which the judge marked down as hiding a regression (2026-11-10).
const FIX_CODE: &str = " A test that existed before your change and fails now protects behavior your change broke: fix the code, not the test, unless the task itself changes that behavior.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_commit_is_checked_in_a_worktree_of_its_own() {
        if Command::new("go").arg("version").stdout(Stdio::null()).status().is_err() {
            return; // no Go here (some CI runners)
        }
        let dir = std::env::temp_dir().join(format!("tazuna-before-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let sh = |c: &str| assert!(crate::platform::shell(c, &[]).current_dir(&dir).stdout(Stdio::null()).stderr(Stdio::null()).status().unwrap().success(), "{c}");
        std::fs::write(dir.join("go.mod"), "module x\n\ngo 1.21\n").unwrap();
        std::fs::write(dir.join("a_test.go"), "package x\n\nimport \"testing\"\n\nfunc TestOld(t *testing.T) { t.Fatal(\"broken before\") }\n").unwrap();
        sh("git init -q && git add -A && git -c user.email=a@b -c user.name=a commit -qm base");
        std::fs::write(dir.join("b_test.go"), "package x\n\nimport \"testing\"\n\nfunc TestNew(t *testing.T) { t.Fatal(\"broken by the change\") }\n").unwrap();
        let o = check(&dir, Some("go test ./".into()), None);
        assert!(!o.ok && o.preexisting == ["TestOld"] && o.output.starts_with("TestOld also fail on the last commit"), "{}", o.output);
        std::fs::remove_file(dir.join("b_test.go")).unwrap();
        let o = check(&dir, Some("go test ./".into()), None);
        assert!(o.ok && o.preexisting == ["TestOld"], "only the old failure left: {}", o.output);
        let wt = String::from_utf8(Command::new("git").args(["worktree", "list"]).current_dir(&dir).output().unwrap().stdout).unwrap();
        assert_eq!(wt.lines().count(), 1, "the worktree is gone: {wt}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn failures_that_predate_the_change_are_not_sent_back_as_its() {
        let out = "=== RUN   TestA\n--- FAIL: TestA (60.01s)\n=== RUN   TestB\n    --- FAIL: TestB/sub (0.00s)\n--- FAIL: TestB (0.00s)\nFAIL\tgo.k6.io/k6/v2/internal/cmd\t62.3s\n";
        assert_eq!(go_failures(out), ["TestA", "TestB"]);
        assert_eq!(go_failures("FAIL\tx/y [build failed]\n"), ["(build)"]);
        let failed = |names: &[&str]| Outcome { cmd: "go test ./a/".into(), ok: false, no_tests: false, timed_out: false, code: Some(1), secs: 1.0,
            output: "the failure".into(), failed: names.iter().map(|s| s.to_string()).collect(), preexisting: vec![] };
        let old = |names: &[&str]| { let v: Vec<String> = names.iter().map(|s| s.to_string()).collect(); move |_: &Outcome| Some(v) };
        let o = preexisting(failed(&["TestA"]), old(&["TestA"]));
        assert!(o.ok && o.preexisting == ["TestA"] && o.output.contains("left alone"), "{}", o.output);
        let o = preexisting(failed(&["TestA", "TestB"]), old(&["TestA"]));
        assert!(!o.ok && o.output.starts_with("TestA also fail on the last commit") && o.output.ends_with("the failure"), "{}", o.output);
        let o = preexisting(failed(&["TestB"]), old(&[]));
        assert!(!o.ok && o.preexisting.is_empty() && o.output == "the failure", "new failures stay as they were");
        let o = preexisting(failed(&["(build)", "TestA"]), |_| panic!("a build failure is never the last commit's"));
        assert!(!o.ok);
    }

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
