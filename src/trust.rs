//! Repos whose hooks, skills, commands and MCP servers may run on this machine. Like Claude Code's "trust this folder?",
//! asked once per repo and remembered in ~/.local/share/tazuna/trusted.json.

use anyhow::Result;
use std::path::Path;

fn key(cwd: &Path) -> String {
    cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf()).to_string_lossy().into_owned()
}

fn list(base: &Path) -> Vec<String> {
    std::fs::read_to_string(base.join("trusted.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn is_trusted(base: &Path, cwd: &Path) -> bool {
    list(base).contains(&key(cwd))
}

pub fn trust(base: &Path, cwd: &Path) -> Result<()> {
    let mut l = list(base);
    let k = key(cwd);
    if !l.contains(&k) {
        l.push(k);
        std::fs::create_dir_all(base)?;
        std::fs::write(base.join("trusted.json"), serde_json::to_string_pretty(&l)?)?;
    }
    Ok(())
}

/// Asks on the terminal (before the UI starts). Only an explicit yes trusts.
pub fn ask(cwd: &Path, what: &str) -> bool {
    use std::io::Write;
    print!("{} has {what}, which can run commands on this machine.\nTrust this repo? [y/N] ", key(cwd));
    let _ = std::io::stdout().flush();
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer).is_ok() && matches!(answer.trim().to_lowercase().as_str(), "y" | "yes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remembers_trusted_repos() {
        let base = std::env::temp_dir().join(format!("tazuna-trust-{}", std::process::id()));
        let repo = base.join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        assert!(!is_trusted(&base, &repo));
        trust(&base, &repo).unwrap();
        trust(&base, &repo).unwrap();
        assert!(is_trusted(&base, &repo));
        assert_eq!(list(&base).len(), 1, "no duplicates");
        std::fs::remove_dir_all(&base).ok();
    }
}
