//! What changed underneath a live session, said once when claude starts: Claude Code updated since the last session (it
//! updates itself; tazuna runs whichever `claude` is installed), a Claude Code older than tazuna needs, a model the picker
//! does not list yet, and a tazuna binary rebuilt since this window opened (the open one keeps running the old build: a
//! user saw a fixed counter still wrong for a day, 2026-11-03).

use std::{path::Path, time::SystemTime};

/// Per-call worker effort (boss mode) needs this Claude Code (2026-11-02).
const MIN_CLAUDE: &str = "2.1.292";

fn older(a: &str, b: &str) -> bool {
    let parts = |v: &str| v.split(|c: char| !c.is_ascii_digit()).filter(|p| !p.is_empty()).map(|p| p.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>();
    parts(a) < parts(b)
}

/// The notices for a claude `init`: `claude` its version, `model` the model it runs, `state` the file that keeps the last
/// version seen, `opened` when this window started.
/// GitHub's releases of tazuna, which CI builds per version tag (.github/workflows/release.yml, 2026-11-08): `tazuna
/// upgrade` replaces the running binary with the newest, and a live session says once a day when one is out.
const REPO: &str = "suwandre/tazuna";

/// This platform's release archive, as CI names it; None where CI builds none (build from source there).
fn asset_name() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Some("tazuna-aarch64-apple-darwin.tar.gz"),
        ("macos", "x86_64") => Some("tazuna-x86_64-apple-darwin.tar.gz"),
        ("linux", "x86_64") => Some("tazuna-x86_64-unknown-linux-gnu.tar.gz"),
        ("linux", "aarch64") => Some("tazuna-aarch64-unknown-linux-gnu.tar.gz"),
        ("windows", "x86_64") => Some("tazuna-x86_64-pc-windows-msvc.zip"),
        _ => None,
    }
}

/// A token for GitHub, where there is one: GH_TOKEN or GITHUB_TOKEN, else the gh CLI's login. The repo is public (since
/// 2026-11-08), so releases come without one; a token only lifts GitHub's limit of 60 unauthenticated calls an hour.
fn github_token() -> Option<String> {
    let set = |k: &str| std::env::var(k).ok().filter(|t| !t.trim().is_empty());
    set("GH_TOKEN").or_else(|| set("GITHUB_TOKEN")).or_else(|| {
        let o = std::process::Command::new("gh").args(["auth", "token"]).output().ok().filter(|o| o.status.success())?;
        Some(String::from_utf8_lossy(&o.stdout).trim().to_string()).filter(|t| !t.is_empty())
    })
}

fn github_get(url: &str, accept: &str, token: Option<&str>) -> anyhow::Result<ureq::http::Response<ureq::Body>> {
    let http: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(60))).http_status_as_error(false).build().into();
    let mut q = http.get(url).header("Accept", accept).header("User-Agent", "tazuna").header("X-GitHub-Api-Version", "2022-11-28");
    if let Some(t) = token {
        q = q.header("Authorization", format!("Bearer {t}"));
    }
    let r = q.call()?;
    match r.status().as_u16() {
        200 => Ok(r),
        404 => anyhow::bail!("no tazuna release yet ({url})"),
        s => anyhow::bail!("GitHub answered HTTP {s} for {url}"),
    }
}

/// The CLI's releases, newest version first: (version, GitHub's description). The desktop's own releases (desktop-v...)
/// and the rolling desktop-latest are left out, as are drafts and prereleases.
fn releases(token: Option<&str>) -> anyhow::Result<Vec<(String, serde_json::Value)>> {
    let mut r = github_get(&format!("https://api.github.com/repos/{REPO}/releases?per_page=40"), "application/vnd.github+json", token)?;
    let all: Vec<serde_json::Value> = serde_json::from_str(&r.body_mut().read_to_string()?)?;
    let mut out: Vec<(String, serde_json::Value)> = all.into_iter().filter(|r| r["draft"] != true && r["prerelease"] != true)
        .filter_map(|r| {
            let v = r["tag_name"].as_str()?.strip_prefix('v').filter(|v| v.starts_with(|c: char| c.is_ascii_digit()))?.to_string();
            Some((v, r))
        })
        .collect();
    out.sort_by(|a, b| if older(&a.0, &b.0) { std::cmp::Ordering::Greater } else if older(&b.0, &a.0) { std::cmp::Ordering::Less } else { std::cmp::Ordering::Equal });
    anyhow::ensure!(!out.is_empty(), "no tazuna release yet");
    Ok(out)
}

/// This platform's archive in a release (download URL and size), once CI has built it.
fn built(rel: &serde_json::Value, asset: &str) -> Option<(String, u64)> {
    let a = rel["assets"].as_array()?.iter().find(|a| a["name"] == asset && a["state"] != "starter")?;
    Some((a["url"].as_str()?.to_string(), a["size"].as_u64().unwrap_or(0)))
}

/// The newest release CI has finished for this platform (a release still building is not news yet).
fn latest(token: Option<&str>) -> anyhow::Result<String> {
    let asset = asset_name();
    let list = releases(token)?;
    Ok(list.iter().find(|(_, r)| asset.is_none_or(|a| built(r, a).is_some())).unwrap_or(&list[0]).0.clone())
}

/// The terminal's look for `tazuna upgrade`: a spinner while it waits, a bar while it downloads, colour; plain lines
/// where the output is not a terminal (a script, a log) and on Windows.
struct Ui {
    tty: bool,
}

impl Ui {
    fn new() -> Self {
        use std::io::IsTerminal;
        Ui { tty: std::io::stdout().is_terminal() && !cfg!(windows) }
    }

    fn paint(&self, code: &str, text: &str) -> String {
        if self.tty { format!("\x1b[{code}m{text}\x1b[0m") } else { text.to_string() }
    }

    fn line(&self, text: &str) {
        use std::io::Write;
        let mut o = std::io::stdout().lock();
        let _ = if self.tty { write!(o, "\r\x1b[2K{text}\n") } else { writeln!(o, "{text}") };
        let _ = o.flush();
    }

    /// A spinner with `text` (which `set` changes) until `stop`.
    fn spin(&self, text: &str) -> Spinner {
        let text = std::sync::Arc::new(std::sync::Mutex::new(text.to_string()));
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let handle = self.tty.then(|| {
            let (text, stop) = (text.clone(), stop.clone());
            std::thread::spawn(move || {
                use std::io::Write;
                let frames = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
                let mut i = 0;
                while !stop.load(std::sync::atomic::Ordering::SeqCst) {
                    let t = text.lock().map(|t| t.clone()).unwrap_or_default();
                    let mut o = std::io::stdout().lock();
                    let _ = write!(o, "\r\x1b[2K\x1b[36m{}\x1b[0m {t}", frames[i % frames.len()]);
                    let _ = o.flush();
                    drop(o);
                    i += 1;
                    std::thread::sleep(std::time::Duration::from_millis(80));
                }
            })
        });
        if !self.tty {
            println!("{}", text.lock().map(|t| t.clone()).unwrap_or_default());
        }
        Spinner { text, stop, handle }
    }
}

struct Spinner {
    text: std::sync::Arc<std::sync::Mutex<String>>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Spinner {
    fn set(&self, t: String) {
        if let Ok(mut x) = self.text.lock() {
            *x = t;
        }
    }

    /// Stops and clears its line (the caller prints what came of it).
    fn stop(mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
            use std::io::Write;
            let mut o = std::io::stdout().lock();
            let _ = write!(o, "\r\x1b[2K");
            let _ = o.flush();
        }
    }
}

/// "3.1 MB"
fn mb(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / 1_048_576.0)
}

/// One version in the CHANGELOG: "## 0.1.165 (2026-11-17): no system services from the sandbox" and its notes.
pub struct Entry {
    pub version: String,
    pub title: String,
    pub notes: String,
}

/// The CHANGELOG's versions, newest first, from its text.
fn entries(text: &str) -> Vec<Entry> {
    let mut out: Vec<Entry> = vec![];
    for l in text.lines() {
        if let Some((v, title)) = l.strip_prefix("## ").and_then(|r| r.split_once(' ')).and_then(|(v, rest)| Some((v, rest.split_once("): ")?.1)))
            && v.starts_with(|c: char| c.is_ascii_digit())
        {
            out.push(Entry { version: v.to_string(), title: title.trim().to_string(), notes: String::new() });
        } else if l.starts_with("## ") || l.starts_with("# ") {
            out.push(Entry { version: String::new(), title: String::new(), notes: String::new() }); // another section: its text is not a version's
        } else if let Some(e) = out.last_mut() {
            e.notes.push_str(l);
            e.notes.push('\n');
        }
    }
    out.retain(|e| !e.version.is_empty());
    for e in &mut out {
        e.notes = e.notes.trim().to_string();
    }
    out
}

/// The CHANGELOG as it was at `version`'s tag (GitHub); empty when it cannot be read.
pub fn changelog(version: &str) -> Vec<Entry> {
    let url = format!("https://raw.githubusercontent.com/{REPO}/v{version}/CHANGELOG.md");
    entries(&github_get(&url, "text/plain", None).ok().and_then(|mut r| r.body_mut().with_config().limit(4 << 20).read_to_string().ok()).unwrap_or_default())
}

/// What the last upgrade did, for `tazuna rollback` with no version (where it came from): upgrades.json in tazuna's data
/// folder.
fn upgrades_file() -> Option<std::path::PathBuf> {
    crate::labels::dir().ok().map(|d| d.join("upgrades.json"))
}

/// Said once, in the first session on a newer tazuna than the last one that ran (however it came: an upgrade by old
/// code, a manual install): from which version, and the newest few titles from the CHANGELOG. last-version in tazuna's
/// data folder keeps the version that ran last; the first run ever says nothing.
pub fn update_news(data: &Path) -> Option<String> {
    let file = data.join("last-version");
    let current = env!("CARGO_PKG_VERSION");
    let last = std::fs::read_to_string(&file).ok().map(|t| t.trim().to_string());
    let _ = std::fs::create_dir_all(data);
    let _ = std::fs::write(&file, current);
    let last = last.filter(|l| older(l, current))?;
    let items: Vec<(String, String)> = changelog(current).into_iter().filter(|e| older(&last, &e.version) && !older(current, &e.version))
        .map(|e| (e.version, e.title)).collect();
    Some(news_text(&last, current, &items))
}

/// "tazuna updated: 0.1.151 → 0.1.167 (16 versions)" and the newest four titles, then where the rest is.
fn news_text(from: &str, to: &str, items: &[(String, String)]) -> String {
    let n = items.len();
    let mut text = format!("tazuna updated: {from} → {to}{}", if n > 1 { format!(" ({n} versions)") } else { String::new() });
    for (v, t) in items.iter().take(4) {
        text.push_str(&format!("\n  {v}  {t}"));
    }
    text.push_str(if n > 4 { "\n  … and more: /changelog lists every version with its notes" } else { "\n  /changelog has the notes" });
    text
}

/// The first line of a release's first note, where the CHANGELOG has no title for it.
fn headline(rel: &serde_json::Value) -> String {
    let body = rel["body"].as_str().unwrap_or_default();
    let first = body.lines().map(str::trim).find(|l| l.starts_with("- ")).unwrap_or_default().trim_start_matches("- ");
    let one: String = first.chars().take(100).collect();
    if first.chars().count() > 100 { format!("{one}…") } else { one }
}

/// What `tazuna upgrade` installs, from the releases newest first with whether CI has built each for this machine:
/// Ok(version, a word for after it), Err(why nothing is to do); the outer Err: no such version.
#[allow(clippy::type_complexity)]
fn pick(current: &str, want: Option<&str>, wait: bool, list: &[(String, bool)]) -> Result<Result<(String, Option<String>), String>, String> {
    let newest = list.first().map(|r| r.0.as_str()).ok_or("no release yet")?;
    if let Some(v) = want {
        if !list.iter().any(|r| r.0 == v) {
            return Err(format!("there is no tazuna {v} (the newest is {newest})"));
        }
        return Ok(if v == current { Err(format!("tazuna {current} is already installed")) } else { Ok((v.to_string(), None)) });
    }
    if !older(current, newest) {
        return Ok(Err(format!("tazuna {current} is the newest release")));
    }
    // the newest CI has built for this machine, if it is newer than this one; --wait takes the newest whatever its state
    let ready = list.iter().find(|r| r.1 && older(current, &r.0)).map(|r| r.0.as_str());
    Ok(Ok(match ready {
        _ if wait => (newest.to_string(), None),
        Some(v) if v == newest => (v.to_string(), None),
        Some(v) => (v.to_string(), Some(format!("tazuna {newest} is still being built: run `tazuna upgrade` again in a few minutes, or `tazuna upgrade --wait` to wait for it"))),
        None => (newest.to_string(), None),
    }))
}

/// What `tazuna rollback` installs: VERSION (older than this one), else the version the last upgrade came from, else the
/// newest older release CI has built here. Err: why not.
fn pick_back(current: &str, want: Option<&str>, previous: Option<&str>, list: &[(String, bool)]) -> Result<String, String> {
    if let Some(v) = want {
        if !list.iter().any(|r| r.0 == v) {
            return Err(format!("there is no tazuna {v}"));
        }
        if !older(v, current) {
            return Err(if v == current { format!("tazuna {current} is already installed") } else { format!("{v} is newer than this tazuna ({current}): `tazuna upgrade {v}` installs it") });
        }
        return Ok(v.to_string());
    }
    if let Some(p) = previous.filter(|p| older(p, current) && list.iter().any(|r| r.0 == *p)) {
        return Ok(p.to_string());
    }
    list.iter().find(|r| r.1 && older(&r.0, current)).map(|r| r.0.clone()).ok_or_else(|| format!("there is no release older than {current} to go back to"))
}

/// `tazuna upgrade [VERSION] [--wait] [--force]` (or `update`) and `tazuna rollback [VERSION]` (`back`): the newest
/// release CI has finished for this platform, or VERSION, in place of this binary. While CI still builds the newest, the one before it comes, with a word that a
/// newer one is on its way; `--wait` waits for that build instead. A source build (under a cargo target/ directory) is
/// left alone unless --force: `git pull && cargo build --release` updates that one.
pub fn upgrade_cli(args: &[String], back: bool) -> anyhow::Result<()> {
    use anyhow::Context;
    let ui = Ui::new();
    let current = env!("CARGO_PKG_VERSION");
    let want = args.iter().find(|a| !a.starts_with('-')).map(|v| v.trim_start_matches('v').to_string());
    let wait = args.iter().any(|a| a == "--wait");
    let asset = asset_name().with_context(|| format!("no tazuna build for {}-{}: build it from source", std::env::consts::OS, std::env::consts::ARCH))?;
    let exe = std::env::current_exe()?.canonicalize()?;
    let shown = exe.to_string_lossy().replace('\\', "/");
    if (shown.contains("/target/release/") || shown.contains("/target/debug/")) && !args.iter().any(|a| a == "--force") {
        anyhow::bail!("this tazuna is a source build ({}): update it with `git pull && cargo build --release`, or `tazuna upgrade --force` to put a release there", exe.display());
    }
    let token = github_token();
    let spin = ui.spin(if back { "Looking for the version to go back to…" } else { "Checking for a new tazuna…" });
    let list = releases(token.as_deref());
    spin.stop();
    let list = list?;
    let ready: Vec<(String, bool)> = list.iter().map(|(v, r)| (v.clone(), built(r, asset).is_some())).collect();
    let previous = upgrades_file().and_then(|f| std::fs::read_to_string(f).ok()).and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|r| r["from"].as_str().map(String::from));
    let (version, mut note) = if back {
        (pick_back(current, want.as_deref(), previous.as_deref(), &ready).map_err(|e| anyhow::anyhow!(e))?, None)
    } else {
        // an older VERSION is a rollback: said so, and done by its own command
        if let Some(v) = want.as_deref().filter(|v| older(v, current)) {
            anyhow::bail!("{v} is older than this tazuna ({current}): `tazuna rollback {v}` goes back to it");
        }
        match pick(current, want.as_deref(), wait, &ready).map_err(|e| anyhow::anyhow!(e))? {
            Ok(go) => go,
            Err(done) => {
                ui.line(&format!("{} {done}", ui.paint("32", "✓")));
                return Ok(());
            }
        }
    };
    ui.line(&ui.paint("1", &format!("{} tazuna {current} → {version}", if back { "Rolling back" } else { "Upgrading" })));
    let rel = |v: &str| list.iter().find(|r| r.0 == v).map(|r| r.1.clone());
    let mut release = rel(&version).context("that release went away")?;
    // not built yet for this machine: wait for CI (--wait), or say so
    if built(&release, asset).is_none() {
        anyhow::ensure!(wait, "tazuna {version} is still being built for this machine (CI takes about 5 minutes): `tazuna upgrade{} --wait` installs it once it is ready",
            want.as_ref().map(|v| format!(" {v}")).unwrap_or_default());
        let spin = ui.spin(&format!("Waiting for CI to build tazuna {version} for this machine…"));
        let t0 = std::time::Instant::now();
        let got = loop {
            if t0.elapsed() > std::time::Duration::from_secs(40 * 60) {
                break None;
            }
            std::thread::sleep(std::time::Duration::from_secs(15));
            let s = t0.elapsed().as_secs();
            spin.set(format!("Waiting for CI to build tazuna {version} for this machine… {}m {:02}s", s / 60, s % 60));
            let fresh = github_get(&format!("https://api.github.com/repos/{REPO}/releases/tags/v{version}"), "application/vnd.github+json", token.as_deref())
                .ok().and_then(|mut r| r.body_mut().read_to_string().ok()).and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
            if let Some(f) = fresh.filter(|f| built(f, asset).is_some()) {
                break Some(f);
            }
        };
        spin.stop();
        release = got.with_context(|| format!("tazuna {version} was not built within 40 minutes: see https://github.com/{REPO}/actions"))?;
        note = None;
    }
    let (url, size) = built(&release, asset).context("no archive")?;
    let mut r = github_get(&url, "application/octet-stream", token.as_deref())?;
    let mut reader = r.body_mut().with_config().limit(200 << 20).reader();
    let mut bytes = Vec::with_capacity(size as usize);
    let mut buf = [0u8; 64 * 1024];
    let mut drawn = std::time::Instant::now() - std::time::Duration::from_secs(1);
    let label = "  Downloading".to_string();
    if !ui.tty {
        println!("  downloading {asset} ({})", mb(size));
    }
    loop {
        let n = std::io::Read::read(&mut reader, &mut buf)?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&buf[..n]);
        if ui.tty && drawn.elapsed() >= std::time::Duration::from_millis(50) {
            drawn = std::time::Instant::now();
            let frac = if size > 0 { (bytes.len() as f64 / size as f64).min(1.0) } else { 0.0 };
            let filled = (frac * 28.0).round() as usize;
            use std::io::Write;
            let mut o = std::io::stdout().lock();
            let _ = write!(o, "\r\x1b[2K{label}  {}{} {:>3}%  {} / {}", ui.paint("36", &"█".repeat(filled)), ui.paint("2", &"░".repeat(28 - filled)),
                (frac * 100.0).round(), mb(bytes.len() as u64), mb(size));
            let _ = o.flush();
        }
    }
    if ui.tty {
        ui.line(&format!("{label}  {} 100%  {}", ui.paint("36", &"█".repeat(28)), mb(bytes.len() as u64)));
    }
    let spin = ui.spin(&format!("Installing tazuna {version}…"));
    // unpacked beside the binary: the rename that swaps it in stays on one filesystem
    let dir = exe.parent().context("the binary's folder")?.join(format!(".tazuna-upgrade-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let result = (|| -> anyhow::Result<()> {
        let archive = dir.join(asset);
        std::fs::write(&archive, &bytes)?;
        // tar unpacks both: macOS and Linux have it, Windows 10 and later ship bsdtar, which reads zip
        let flags = if asset.ends_with(".zip") { "-xf" } else { "-xzf" };
        let ok = std::process::Command::new("tar").args([flags, &archive.to_string_lossy(), "-C", &dir.to_string_lossy()]).status()?.success();
        anyhow::ensure!(ok, "tar could not unpack {asset}");
        let new = dir.join(if cfg!(windows) { "tazuna.exe" } else { "tazuna" });
        let out = std::process::Command::new(&new).arg("--version").output().context("running the downloaded tazuna")?;
        anyhow::ensure!(String::from_utf8_lossy(&out.stdout).contains(&version), "the downloaded tazuna does not say {version}: left as it was");
        #[cfg(unix)]
        std::fs::set_permissions(&new, std::os::unix::fs::PermissionsExt::from_mode(0o755))?;
        // a running binary cannot be overwritten on Windows but can be renamed: it moves aside and goes on the next start
        if cfg!(windows) {
            std::fs::rename(&exe, exe.with_extension("old.exe"))?;
        }
        std::fs::rename(&new, &exe)?;
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&dir);
    spin.stop();
    result?;
    ui.line(&format!("{} {} {}", ui.paint("32", "✓"), if back { format!("Rolled back to tazuna {version}") } else { format!("Upgraded to tazuna {version}") },
        ui.paint("2", &format!("({})", exe.display()))));
    if back {
        let newest = &list[0].0;
        ui.line(&format!("{} `tazuna upgrade` brings {newest} back", ui.paint("2", "→")));
        return Ok(());
    }
    // what came: each release between the two, newest first; six here, all kept for the next session's word and rollback
    let span: Vec<&(String, serde_json::Value)> = list.iter().filter(|(v, _)| older(current, v) && !older(&version, v)).collect();
    let notes = if span.is_empty() { vec![] } else { changelog(&version) };
    let title = |v: &str, r: &serde_json::Value| notes.iter().find(|e| e.version == v).map(|e| e.title.clone()).unwrap_or_else(|| headline(r));
    let items: Vec<(String, String)> = span.iter().map(|(v, r)| (v.clone(), title(v, r))).collect();
    for (v, h) in items.iter().take(6) {
        if !h.is_empty() {
            ui.line(&format!("  {} {h}", ui.paint("2", &format!("{v:<8}"))));
        }
    }
    if items.len() > 6 {
        ui.line(&ui.paint("2", &format!("  … {} more", items.len() - 6)));
    }
    if !items.is_empty() {
        ui.line(&ui.paint("2", "  in tazuna: /changelog has every version's notes"));
    }
    if let Some(f) = upgrades_file() {
        let _ = f.parent().map(std::fs::create_dir_all);
        let _ = std::fs::write(f, serde_json::json!({"from": current, "to": version}).to_string());
    }
    if let Some(n) = note.take() {
        ui.line(&format!("{} {n}", ui.paint("33", "→")));
    }
    Ok(())
}

/// Once a day at most: "tazuna X is out" when the newest release is newer than this binary. The check is cached in
/// update-check.json; nothing when offline or with TAZUNA_NO_UPDATE_CHECK.
pub fn update_notice(data: &Path, now: u64) -> Option<String> {
    if std::env::var_os("TAZUNA_NO_UPDATE_CHECK").is_some() {
        return None;
    }
    let file = data.join("update-check.json");
    let cached: serde_json::Value = std::fs::read_to_string(&file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let newest = match cached["at"].as_u64() {
        Some(at) if now.saturating_sub(at) < 24 * 3600 => cached["latest"].as_str().map(String::from),
        _ => {
            let found = latest(github_token().as_deref()).ok();
            let _ = std::fs::write(&file, serde_json::json!({"at": now, "latest": found}).to_string());
            found
        }
    }?;
    let current = env!("CARGO_PKG_VERSION");
    older(current, &newest).then(|| format!("tazuna {newest} is out (this is {current}): `tazuna upgrade` installs it"))
}

/// Windows: the binary an upgrade moved aside, removed on the next start.
pub fn clean_old() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::fs::remove_file(exe.with_extension("old.exe"));
    }
}

pub fn notices(claude: Option<&str>, model: Option<&str>, state: &Path, opened: SystemTime) -> Vec<String> {
    let mut out = vec![];
    if let Some(v) = claude {
        let last = std::fs::read_to_string(state).unwrap_or_default();
        let last = last.trim();
        // only a newer version is news, and the file keeps the newest seen: a window open for days still runs the claude
        // it started with, and its next turn read as "updated: 2.1.325 → 2.1.323" after newer sessions (2026-11-16)
        if last.is_empty() || older(last, v) {
            if !last.is_empty() {
                out.push(format!("Claude Code updated: {last} → {v}"));
            }
            let _ = std::fs::write(state, v);
        }
        if older(v, MIN_CLAUDE) {
            out.push(format!("Claude Code {v} is older than tazuna needs ({MIN_CLAUDE}): run `claude update`"));
        }
    }
    if let Some(m) = model.filter(|m| m.starts_with("claude-") && !crate::models::known_claude(m)) {
        out.push(format!("{m} is not in tazuna's model list yet: it runs, but the picker and its effort levels need a tazuna update"));
    }
    let rebuilt = std::env::current_exe().and_then(std::fs::metadata).and_then(|m| m.modified()).is_ok_and(|t| t > opened);
    if rebuilt {
        out.push("tazuna was rebuilt since this window opened: restart it to use the new build".into());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_platform_has_a_release_archive() {
        let name = super::asset_name();
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            assert_eq!(name, Some("tazuna-aarch64-apple-darwin.tar.gz"));
        }
        assert!(super::older("0.1.75", "0.1.76") && !super::older("0.1.76", "0.1.76") && super::older("0.1.9", "0.1.10"));
    }

    #[test]
    fn upgrade_takes_the_newest_built_release_or_the_one_asked_for() {
        let l = |xs: &[(&str, bool)]| xs.iter().map(|(v, b)| (v.to_string(), *b)).collect::<Vec<_>>();
        let building = l(&[("0.1.165", false), ("0.1.164", true), ("0.1.163", true)]);
        let (v, note) = pick("0.1.163", None, false, &building).unwrap().unwrap();
        assert!(v == "0.1.164" && note.unwrap().contains("0.1.165 is still being built"), "the newest built one, and a word on the next");
        assert_eq!(pick("0.1.163", None, true, &building).unwrap().unwrap().0, "0.1.165", "--wait waits for the newest");
        assert_eq!(pick("0.1.164", None, false, &building).unwrap().unwrap().0, "0.1.165", "nothing built is newer: the newest (it says it is building)");
        assert_eq!(pick("0.1.165", None, false, &building).unwrap().unwrap_err(), "tazuna 0.1.165 is the newest release");
        assert_eq!(pick("0.1.165", Some("0.1.163"), false, &building).unwrap().unwrap().0, "0.1.163", "an older one when asked");
        assert!(pick("0.1.165", Some("0.1.99"), false, &building).unwrap_err().contains("no tazuna 0.1.99"));
        assert!(pick("0.1.163", Some("0.1.163"), false, &building).unwrap().unwrap_err().contains("already installed"));
        assert_eq!(pick("0.1.160", None, false, &l(&[("0.1.165", true), ("0.1.164", true)])).unwrap().unwrap(), ("0.1.165".to_string(), None));
    }

    #[test]
    fn rollback_goes_to_the_version_asked_for_or_the_one_before() {
        let l = |xs: &[(&str, bool)]| xs.iter().map(|(v, b)| (v.to_string(), *b)).collect::<Vec<_>>();
        let list = l(&[("0.1.167", true), ("0.1.166", true), ("0.1.165", true), ("0.1.151", true)]);
        assert_eq!(pick_back("0.1.167", None, Some("0.1.151"), &list).unwrap(), "0.1.151", "where the last upgrade came from");
        assert_eq!(pick_back("0.1.167", None, None, &list).unwrap(), "0.1.166", "else the release before");
        assert_eq!(pick_back("0.1.167", Some("0.1.165"), Some("0.1.151"), &list).unwrap(), "0.1.165");
        assert!(pick_back("0.1.166", Some("0.1.167"), None, &list).unwrap_err().contains("`tazuna upgrade 0.1.167`"));
        assert!(pick_back("0.1.151", None, None, &list).unwrap_err().contains("no release older"));
    }

    #[test]
    fn the_changelog_reads_as_versions_with_titles_and_notes() {
        let e = entries("# Changelog\n\nintro\n\n## 0.1.167 (2026-11-17): names fold\n\n- one\n- two\n\n## 0.1.166 (2026-11-17): upgrade\n\n- three\n");
        assert_eq!(e.iter().map(|e| (e.version.as_str(), e.title.as_str(), e.notes.as_str())).collect::<Vec<_>>(),
            [("0.1.167", "names fold", "- one\n- two"), ("0.1.166", "upgrade", "- three")]);
    }

    #[test]
    fn the_first_session_on_a_newer_tazuna_says_what_it_brought() {
        let items: Vec<(String, String)> = (0..6).map(|i| (format!("0.1.{}", 170 - i), format!("change {i}"))).collect();
        let text = news_text("0.1.151", "0.1.170", &items);
        assert!(text.starts_with("tazuna updated: 0.1.151 → 0.1.170 (6 versions)") && text.contains("change 3") && !text.contains("change 4"), "{text}");
        assert!(text.contains("/changelog lists every version"));
        assert!(news_text("0.1.169", "0.1.170", &items[..1]).ends_with("/changelog has the notes"));
        // the first run ever and a run on the same version say nothing; the file then holds this version
        let dir = std::env::temp_dir().join(format!("tazuna-news-{}", std::process::id()));
        assert!(update_news(&dir).is_none() && update_news(&dir).is_none());
        assert_eq!(std::fs::read_to_string(dir.join("last-version")).unwrap(), env!("CARGO_PKG_VERSION"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn says_what_changed_once() {
        let state = std::env::temp_dir().join(format!("tazuna-claude-version-{}", std::process::id()));
        let _ = std::fs::remove_file(&state);
        let later = SystemTime::now() + std::time::Duration::from_secs(3600);
        assert!(notices(Some("2.1.314"), Some("claude-opus-5-5"), &state, later).is_empty(), "first session: nothing to compare");
        assert_eq!(notices(Some("2.1.318"), Some("claude-opus-5-5"), &state, later), ["Claude Code updated: 2.1.314 → 2.1.318"]);
        assert!(notices(Some("2.1.318"), None, &state, later).is_empty(), "said once");
        assert!(notices(Some("2.1.316"), None, &state, later).is_empty(), "an older claude (a window open since before the update) is no update");
        assert_eq!(std::fs::read_to_string(&state).unwrap(), "2.1.318", "the newest seen stays");
        let n = notices(Some("2.1.200"), Some("claude-opus-6-0"), &state, later);
        assert!(n[0].contains("older than tazuna needs") && n[1].starts_with("claude-opus-6-0 is not in"), "{n:?}");
        assert!(notices(None, None, &state, SystemTime::UNIX_EPOCH).iter().any(|n| n.starts_with("tazuna was rebuilt")));
        assert!(older("2.1.99", "2.1.292") && !older("2.1.318", "2.1.292") && !older("2.2.0", "2.1.292"));
        let _ = std::fs::remove_file(&state);
    }
}
