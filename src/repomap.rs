//! A repo map for the first message (`--repo-map`): the folders, and for the files that match the task's words, the names
//! they define. Reading code is most of a run's cost (every turn re-reads the context it piled up), so a map that points
//! at the right files up front should cut exploration. Plain line matching, no parser: top-level definitions only.

use std::{collections::BTreeMap, path::Path, process::Command};

const MAX_CHARS: usize = 12_000;
const MAX_FILES: usize = 25;
const MAX_SYMBOLS: usize = 15;
const MAX_FILE_BYTES: u64 = 300_000;
const SOURCE: [&str; 9] = [".ts", ".tsx", ".js", ".jsx", ".go", ".rs", ".py", ".mts", ".cts"];
const STOP: [&str; 24] = [
    "that", "this", "with", "from", "have", "when", "into", "only", "will", "should", "does", "must", "they", "their", "there",
    "which", "what", "then", "than", "also", "each", "case", "cases", "make",
];

/// The map for `task` in `cwd`, or None when nothing matches.
pub fn build(cwd: &Path, task: &str) -> Option<String> {
    let out = Command::new("git").args(["ls-files"]).current_dir(cwd).output().ok()?;
    let files: Vec<String> = String::from_utf8_lossy(&out.stdout).lines().map(String::from).collect();
    let words = keywords(task);
    if files.is_empty() || words.is_empty() {
        return None;
    }
    let mut scored: Vec<(usize, &String, Vec<String>)> = files
        .iter()
        .filter(|f| SOURCE.iter().any(|e| f.ends_with(e)))
        .filter_map(|f| {
            let path = cwd.join(f);
            let text = (std::fs::metadata(&path).ok()?.len() <= MAX_FILE_BYTES).then(|| std::fs::read_to_string(&path).ok()).flatten()?;
            let symbols = symbols(&text);
            let lower = f.to_lowercase();
            let path_hits = words.iter().filter(|w| lower.contains(w.as_str())).count();
            let sym_hits = words.iter().filter(|w| symbols.iter().any(|s| s.to_lowercase().contains(w.as_str()))).count();
            // tests and e2e harnesses match the same words as the code they test; the code comes first
            let test = [".test.", ".unit.", ".e2e.", "_test.", "/tests/", "/e2e/", "test_"].iter().any(|t| lower.contains(t));
            let score = (path_hits * 2 + sym_hits) * if test { 1 } else { 2 };
            (score > 0).then_some((score, f, symbols))
        })
        .collect();
    if scored.is_empty() {
        return None;
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.len().cmp(&b.1.len())));
    let mut map = String::from("Repo map (orientation only: read a file before relying on it)\n\nFolders: ");
    map.push_str(&folders(&files));
    map.push_str("\n\nFiles that match the task, with the names they define:\n");
    for (_, f, syms) in scored.into_iter().take(MAX_FILES) {
        let shown: Vec<&str> = syms.iter().take(MAX_SYMBOLS).map(String::as_str).collect();
        let more = if syms.len() > MAX_SYMBOLS { format!(" (+{})", syms.len() - MAX_SYMBOLS) } else { String::new() };
        let line = format!("- {f}: {}{more}\n", shown.join(", "));
        if map.len() + line.len() > MAX_CHARS {
            break;
        }
        map.push_str(&line);
    }
    Some(map)
}

/// Words of the task worth matching: identifiers and words of 4+ letters, camelCase split, common words dropped.
fn keywords(task: &str) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    for token in task.split(|c: char| !c.is_alphanumeric() && c != '_') {
        let mut parts = vec![token.to_string()];
        // camelCase and PascalCase: "batchUpsertItemDocuments" also matches "batch", "upsert", "documents"
        let mut cur = String::new();
        for c in token.chars() {
            if c.is_uppercase() && !cur.is_empty() {
                parts.push(std::mem::take(&mut cur));
            }
            cur.push(c);
        }
        parts.push(cur);
        for p in parts {
            let p = p.to_lowercase();
            if p.len() >= 4 && !STOP.contains(&p.as_str()) && !p.chars().all(|c| c.is_ascii_digit()) && !words.contains(&p) {
                words.push(p);
            }
        }
    }
    words
}

/// Top-level definitions, by the first words of a line (TypeScript/JavaScript, Go, Rust, Python).
fn symbols(text: &str) -> Vec<String> {
    const KINDS: [&str; 12] = ["function", "class", "const", "interface", "type", "enum", "fn", "struct", "trait", "def", "func", "let"];
    const PREFIXES: [&str; 6] = ["export ", "default ", "async ", "pub ", "pub(crate) ", "declare "];
    let mut out = Vec::new();
    for line in text.lines() {
        if line.starts_with([' ', '\t']) {
            continue;
        }
        let mut rest = line;
        while let Some(p) = PREFIXES.iter().find(|p| rest.starts_with(**p)) {
            rest = &rest[p.len()..];
        }
        let Some((kind, after)) = rest.split_once(' ') else { continue };
        if !KINDS.contains(&kind) {
            continue;
        }
        // Go methods: `func (r *T) Name(`
        let after = if kind == "func" && after.starts_with('(') { after.split_once(") ").map_or("", |x| x.1) } else { after };
        let name: String = after.trim_start_matches('*').chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$').collect();
        if !name.is_empty() && !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

/// "api (812), app (640), shared (233) …": first-level folders by tracked-file count.
fn folders(files: &[String]) -> String {
    let mut count: BTreeMap<&str, usize> = BTreeMap::new();
    for f in files {
        if let Some((dir, _)) = f.split_once('/') {
            *count.entry(dir).or_default() += 1;
        }
    }
    let mut v: Vec<(&str, usize)> = count.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    v.iter().take(15).map(|(d, n)| format!("{d} ({n})")).collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_top_level_definitions() {
        let ts = "export async function batchUpsertItemDocuments(a) {\nexport const MAX = 1;\nclass Foo {\n  method() {}\n}\ninterface Bar {}\nexport default class Baz {}";
        assert_eq!(symbols(ts), vec!["batchUpsertItemDocuments", "MAX", "Foo", "Bar", "Baz"]);
        let go = "func (s *Server) Handle(w) {\nfunc main() {\ntype Config struct {";
        assert_eq!(symbols(go), vec!["Handle", "main", "Config"]);
        assert_eq!(symbols("pub fn run() {}\npub(crate) struct Live;\n    fn inner() {}"), vec!["run", "Live"]);
    }

    #[test]
    fn keywords_split_identifiers_and_skip_noise() {
        let k = keywords("Fix batchUpsertItemDocuments: the Ledger sync should only run when 2026 deals exist");
        for w in ["batchupsertitemdocuments", "batch", "upsert", "documents", "ledger", "sync", "deals", "exist"] {
            assert!(k.contains(&w.to_string()), "{w} in {k:?}");
        }
        assert!(!k.iter().any(|w| ["should", "only", "when", "2026", "fix", "the"].contains(&w.as_str())));
    }

    #[test]
    fn maps_matching_files_first() {
        let repo = std::env::temp_dir().join(format!("tazuna-map-{}", std::process::id()));
        std::fs::create_dir_all(repo.join("api/src")).unwrap();
        std::fs::write(repo.join("api/src/ledger.ts"), "export function syncDeal() {}\n").unwrap();
        std::fs::write(repo.join("api/src/other.ts"), "export function unrelated() {}\n").unwrap();
        let ok = Command::new("sh").args(["-c", "git init -q && git add -A"]).current_dir(&repo).status().unwrap().success();
        assert!(ok);
        let map = build(&repo, "The Ledger deal sync is broken").unwrap();
        assert!(map.contains("- api/src/ledger.ts: syncDeal") && !map.contains("other.ts"), "{map}");
        assert!(map.contains("Folders: api (2)"));
        std::fs::remove_dir_all(&repo).ok();
    }
}
