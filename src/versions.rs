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
