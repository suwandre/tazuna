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

/// The newest release: its version (tag without the v) and GitHub's description of it.
fn latest(token: Option<&str>) -> anyhow::Result<(String, serde_json::Value)> {
    let mut r = github_get(&format!("https://api.github.com/repos/{REPO}/releases/latest"), "application/vnd.github+json", token)?;
    let rel: serde_json::Value = serde_json::from_str(&r.body_mut().read_to_string()?)?;
    let tag = rel["tag_name"].as_str().unwrap_or_default().trim_start_matches('v').to_string();
    anyhow::ensure!(!tag.is_empty(), "the newest release has no version tag");
    Ok((tag, rel))
}

/// `tazuna upgrade [--force]`: the newest release for this platform in place of this binary. A source build (under a
/// cargo target/ directory) is left alone unless --force: `git pull && cargo build --release` updates that one.
pub fn upgrade_cli(args: &[String]) -> anyhow::Result<()> {
    use anyhow::Context;
    let current = env!("CARGO_PKG_VERSION");
    let token = github_token();
    let (newest, rel) = latest(token.as_deref())?;
    if !older(current, &newest) {
        println!("tazuna {current} is the newest release");
        return Ok(());
    }
    let asset = asset_name().with_context(|| format!("no tazuna build for {}-{}: build it from source", std::env::consts::OS, std::env::consts::ARCH))?;
    let exe = std::env::current_exe()?.canonicalize()?;
    let shown = exe.to_string_lossy().replace('\\', "/");
    if (shown.contains("/target/release/") || shown.contains("/target/debug/")) && !args.iter().any(|a| a == "--force") {
        anyhow::bail!("this tazuna is a source build ({}): update it with `git pull && cargo build --release`, or `tazuna upgrade --force` to put {newest} there", exe.display());
    }
    let url = rel["assets"].as_array().into_iter().flatten().find(|a| a["name"] == asset).and_then(|a| a["url"].as_str())
        .with_context(|| format!("release {newest} has no {asset} (CI may still be building it)"))?;
    println!("tazuna {current} -> {newest}: downloading {asset}");
    let mut r = github_get(url, "application/octet-stream", token.as_deref())?;
    let bytes = r.body_mut().with_config().limit(200 << 20).read_to_vec()?;
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
        anyhow::ensure!(String::from_utf8_lossy(&out.stdout).contains(&newest), "the downloaded tazuna does not say {newest}: left as it was");
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
    result?;
    println!("tazuna is now {newest} ({})", exe.display());
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
            let found = latest(github_token().as_deref()).ok().map(|(v, _)| v);
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
        if !last.is_empty() && last != v {
            out.push(format!("Claude Code updated: {last} → {v}"));
        }
        if last != v {
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
    fn says_what_changed_once() {
        let state = std::env::temp_dir().join(format!("tazuna-claude-version-{}", std::process::id()));
        let _ = std::fs::remove_file(&state);
        let later = SystemTime::now() + std::time::Duration::from_secs(3600);
        assert!(notices(Some("2.1.314"), Some("claude-opus-5-5"), &state, later).is_empty(), "first session: nothing to compare");
        assert_eq!(notices(Some("2.1.318"), Some("claude-opus-5-5"), &state, later), ["Claude Code updated: 2.1.314 → 2.1.318"]);
        assert!(notices(Some("2.1.318"), None, &state, later).is_empty(), "said once");
        let n = notices(Some("2.1.200"), Some("claude-opus-6-0"), &state, later);
        assert!(n[1].contains("older than tazuna needs") && n[2].starts_with("claude-opus-6-0 is not in"), "{n:?}");
        assert!(notices(None, None, &state, SystemTime::UNIX_EPOCH).iter().any(|n| n.starts_with("tazuna was rebuilt")));
        assert!(older("2.1.99", "2.1.292") && !older("2.1.318", "2.1.292") && !older("2.2.0", "2.1.292"));
        let _ = std::fs::remove_file(&state);
    }
}
