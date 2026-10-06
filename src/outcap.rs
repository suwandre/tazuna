//! Shrinking long command output before the model sees it: every byte of a tool result is re-sent on each later turn.

/// Go's per-test chatter that says nothing about failures.
pub fn is_test_noise(line: &str) -> bool {
    let t = line.trim_start();
    ["=== RUN", "=== PAUSE", "=== CONT", "--- PASS", "ok "].iter().any(|p| t.starts_with(p))
}

/// Keeps whole lines from the start (up to `head` chars) and the end (the rest of `budget`). None if it already fits.
/// Returns the kept text with `marker(omitted_lines)` in the middle.
pub fn head_tail(s: &str, budget: usize, head: usize, marker: impl Fn(usize) -> String) -> Option<String> {
    if s.chars().count() <= budget {
        return None;
    }
    let lines: Vec<&str> = s.lines().collect();
    let width = |l: &&str| l.chars().count() + 1;
    let mut used = 0;
    let first: Vec<&str> = lines.iter().take_while(|l| { used += width(l); used <= head }).copied().collect();
    let mut used = 0;
    let mut last: Vec<&str> = lines[first.len()..].iter().rev().take_while(|l| { used += width(l); used <= budget - head }).copied().collect();
    last.reverse();
    let omitted = lines.len() - first.len() - last.len();
    if omitted == 0 {
        return None;
    }
    let mut out = first.join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(&marker(omitted));
    if !last.is_empty() {
        out.push('\n');
        out.push_str(&last.join("\n"));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_head_and_tail_lines() {
        let s: String = (1..=100).map(|i| format!("line {i}\n")).collect();
        let out = head_tail(&s, 60, 20, |n| format!("[{n} omitted]")).unwrap();
        assert!(out.starts_with("line 1\nline 2\n"), "{out}");
        assert!(out.ends_with("line 100"), "{out}");
        assert!(out.contains(" omitted]"));
        assert!(out.chars().count() < 60 + 20, "stays near the budget: {out}");
        assert_eq!(head_tail("short", 60, 20, |_| String::new()), None);
    }

    #[test]
    fn recognises_go_noise() {
        assert!(is_test_noise("=== RUN   TestX") && is_test_noise("    --- PASS: TestX/sub (0.00s)") && is_test_noise("ok  \tpkg\t0.1s"));
        assert!(!is_test_noise("--- FAIL: TestY") && !is_test_noise("FAIL\tpkg"));
    }
}
