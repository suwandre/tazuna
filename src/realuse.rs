//! Real use as a running score (bench/realuse.py, bench/suite/DESIGN.md): the user's own live runs, scored from what
//! happened next, with no labels asked for. Strong signals are a clear miss: the run was stopped or rewound, ended in an
//! error or crash, git shows its change discarded, a chat answer was sent to the agent after all, or the next message
//! says it failed or asks the same again. Soft ones are pushback ("wait, ...", "why create ...?") or waiting
//! ("progress?"). The desktop's usage page shows them per version and model. Checked against 390 runs labelled by hand
//! (2026-11-12): strong and soft together caught 12 of 14 bad runs and flagged 4 of 231 good ones.

use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};

/// A typed message in a recording: the next one, the seconds between them, whether the run was stopped or rewound.
#[derive(Clone)]
pub struct Msg {
    pub next: String,
    pub gap: Option<f64>,
    pub stopped: bool,
    pub rewound: bool,
}

/// Each recording's messages as last read, with its length and time: the usage page asks again on every finished turn while
/// it is open, and reading every recording each time was 2,940 files, 515 MB (2026-11-26). A finished one never changes.
type Read = HashMap<String, ((u64, Option<std::time::SystemTime>), Vec<(String, Msg)>)>;
static READ: std::sync::LazyLock<std::sync::Mutex<Read>> = std::sync::LazyLock::new(Default::default);

/// The typed messages of these recordings, by (recording, the message's first line up to 200 characters), in order. Only
/// the lines that can matter are parsed (a recording holds every tool output too); a file unchanged since it was last read
/// is not read again.
pub fn messages(recordings: &HashSet<String>) -> HashMap<(String, String), VecDeque<Msg>> {
    let mut out: HashMap<(String, String), VecDeque<Msg>> = HashMap::new();
    let mut read = READ.lock().unwrap_or_else(|e| e.into_inner());
    for path in recordings {
        let Ok(meta) = std::fs::metadata(path) else { continue };
        let stamp = (meta.len(), meta.modified().ok());
        if read.get(path).is_none_or(|(s, _)| *s != stamp) {
            let Ok(text) = std::fs::read_to_string(path) else { continue };
            read.insert(path.clone(), (stamp, typed(&text)));
        }
        for (key, m) in &read[path].1 {
            out.entry((path.clone(), key.clone())).or_default().push_back(m.clone());
        }
    }
    out
}

/// One recording's typed messages, keyed by their first line, in order.
fn typed(text: &str) -> Vec<(String, Msg)> {
    let mut out = vec![];
    let ev: Vec<(f64, Value)> = text.lines()
        .filter(|l| l.contains("\"harness_user\"") || l.contains("\"harness_interrupt\"") || l.contains("\"harness_rewind"))
        .filter_map(|l| serde_json::from_str::<Value>(l).ok()).map(|d| (d["t"].as_f64().unwrap_or(0.0), d["e"].clone())).collect();
    let users: Vec<usize> = ev.iter().enumerate().filter(|(_, e)| e.1["type"] == "harness_user").map(|(i, _)| i).collect();
    for (k, &i) in users.iter().enumerate() {
        let nxt = users.get(k + 1).copied();
        let span = &ev[i..nxt.unwrap_or(ev.len())];
        let text = ev[i].1["text"].as_str().unwrap_or_default();
        let key: String = text.lines().next().unwrap_or_default().chars().take(200).collect();
        out.push((key, Msg {
            next: nxt.and_then(|j| ev[j].1["text"].as_str()).unwrap_or_default().to_string(),
            gap: nxt.map(|j| (ev[j].0 - ev[i].0) / 1000.0),
            stopped: span.iter().any(|e| e.1["type"] == "harness_interrupt"),
            rewound: span.iter().any(|e| e.1["type"].as_str().is_some_and(|t| t.starts_with("harness_rewind"))),
        }));
    }
    out
}

/// Lower case, apostrophes dropped (don't, dont and don’t read alike), spaces squeezed.
fn norm(s: &str) -> String {
    s.to_lowercase().replace(['\'', '’', '‘'], "").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `s` starts with the word or phrase `w` (followed by a non-letter or the end).
fn starts_word(s: &str, w: &str) -> bool {
    s.strip_prefix(w).is_some_and(|rest| rest.chars().next().is_none_or(|c| !c.is_alphanumeric()))
}

/// A check on the model rather than a request for work ("what is your model name", "test"): asked again after each
/// model switch, which read as a re-ask.
pub fn probe(text: &str) -> bool {
    let t = norm(text);
    let t = t.trim_end_matches(|c: char| !c.is_alphanumeric());
    if ["test", "hi", "hello", "ping", "which model are you", "who are you", "what model", "what model is this", "what model are you"].contains(&t) {
        return true;
    }
    if ["developed", "released", "trained", "made"].iter().any(|w| t == format!("when were you {w}")) {
        return true;
    }
    let rest = t.strip_prefix("whats your ").or_else(|| t.strip_prefix("what is your "));
    rest.is_some_and(|r| {
        let r = r.strip_prefix("model ").unwrap_or(r);
        r == "name" || r == "model" || (r.chars().all(char::is_alphanumeric) && (3..=4).contains(&r.len()) && (r.starts_with('n') || r.starts_with('a')))
    })
}

fn correction(head: &str) -> bool {
    let h = norm(head);
    let no = starts_word(&h, "no") && !["need", "worries", "problem", "rush"].iter().any(|w| starts_word(h.get(3..).unwrap_or(""), w));
    let start = no || ["nope", "wrong", "stop", "undo", "revert", "dont", "do not", "thats not", "not what", "this is wrong", "incorrect",
        "you missed", "you forgot", "you didnt", "why did you", "why didnt"].iter().any(|w| starts_word(&h, w));
    let any = [
        "doesnt work", "didnt work", "still not", "still fail", "still broken", "still the same", "still wrong", "still doesn", "same error",
        "same issue", "same problem", "ci failed", "ci fails", "ci is failing", "ci broke", "error though", "fails again", "fail again",
        "fails still", "fail still", "it broke", "broke the", "broke it", "not working", "not correct", "not right", "thats wrong", "wasnt asked",
        "wasnt what",
    ].iter().any(|p| h.contains(p))
        || ["has", "have", "got", "getting", "threw", "throws"].iter().any(|v| h.contains(&format!("{v} an error")) || h.contains(&format!("{v} a error")));
    start || any
}

fn pushback(head: &str) -> bool {
    let h = norm(head);
    starts_word(&h, "wait") || ["hold on", "why create", "why make", "why add", "why open", "why use", "why is it", "why are you", "why not", "cant we",
        "shouldnt we", "shouldnt it", "shouldnt you"].iter().any(|w| starts_word(&h, w))
}

fn waited(head: &str) -> bool {
    let h = norm(head);
    ["progress", "any progress", "still running", "still working", "status", "hello", "are you there", "are you stuck"].contains(&h.trim_end_matches('?'))
}

fn words(t: &str) -> HashSet<String> {
    t.to_lowercase().split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| w.len() >= 3).map(String::from).collect()
}

/// A run's strong and soft signals from what happened in it and the message after it (`stop`: the run log's; `verdict`:
/// labels.jsonl's, "discarded" when git showed its change thrown away).
pub fn signals(prompt: &str, m: Option<&Msg>, stop: &str, verdict: Option<&str>) -> (Vec<&'static str>, Vec<&'static str>) {
    let (mut strong, mut soft) = (vec![], vec![]);
    if probe(prompt) {
        return (strong, soft);
    }
    if m.is_some_and(|m| m.stopped) {
        strong.push("stopped");
    }
    if m.is_some_and(|m| m.rewound) {
        strong.push("rewound");
    }
    if stop == "error" || stop == "crashed" {
        strong.push(if stop == "error" { "error" } else { "crashed" });
    }
    if verdict == Some("discarded") {
        strong.push("discarded");
    }
    let head: String = m.map_or("", |m| m.next.trim()).chars().take(240).collect();
    if head.starts_with("/agent ") {
        strong.push("misrouted");
    }
    if !head.is_empty() && correction(&head) {
        strong.push("correction");
    }
    if !head.is_empty() && m.and_then(|m| m.gap).is_some_and(|g| g < 1800.0) {
        let (a, b) = (words(prompt), words(&head));
        if a.len() >= 4 && b.len() >= 4 && a.intersection(&b).count() as f64 / a.union(&b).count() as f64 >= 0.6 {
            strong.push("re-ask");
        }
    }
    if !head.is_empty() && pushback(&head) {
        soft.push("pushback");
    }
    if !head.is_empty() && waited(&head) {
        soft.push("waited");
    }
    (strong, soft)
}

/// Runs that are not the user's own work: benchmarks and test folders.
pub fn skipped_folder(cwd: &str) -> bool {
    ["/bench", "/tmp", "/private/var", "/T/", "/rw-", "looptest", "livetest", "mcp-e2e", "rv-probe"].iter().any(|p| cwd.contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recordings_are_read_again_only_when_they_change() {
        use serde_json::json;
        let path = std::env::temp_dir().join(format!("tazuna-realuse-{}.jsonl", std::process::id()));
        let line = |t: u64, text: &str| format!("{}\n", json!({"t": t, "e": {"type": "harness_user", "text": text}}));
        std::fs::write(&path, line(0, "fix it") + &line(60_000, "still broken")).unwrap();
        let p = path.to_string_lossy().to_string();
        let set = HashSet::from([p.clone()]);
        let first = messages(&set);
        assert_eq!(first[&(p.clone(), "fix it".into())][0].next, "still broken");
        assert_eq!(messages(&set)[&(p.clone(), "fix it".into())].len(), 1, "a second read from what was kept, the same");
        // grown (a conversation went on): read again
        std::fs::write(&path, line(0, "fix it") + &line(60_000, "still broken") + &line(90_000, "thanks")).unwrap();
        assert_eq!(messages(&set)[&(p.clone(), "still broken".into())][0].next, "thanks");
        let _ = std::fs::remove_file(&path);
        assert!(messages(&set).is_empty(), "a recording gone is left out");
    }

    #[test]
    fn signals_read_the_next_message_as_the_script_does() {
        let m = |next: &str, gap: f64| Msg { next: next.into(), gap: Some(gap), stopped: false, rewound: false };
        let strong = |p: &str, next: &str| signals(p, Some(&m(next, 60.0)), "completed", None).0;
        assert_eq!(strong("fix the parser", "No, that's wrong: it still fails"), vec!["correction"]);
        assert!(strong("fix the parser", "no worries, thanks").is_empty(), "no worries is not a no");
        assert!(strong("fix it", "it doesn't work").contains(&"correction"));
        assert!(strong("fix it", "now I'm getting an error in the log").contains(&"correction"));
        assert_eq!(strong("make the cache gauge show hits over reads", "make the cache gauge show hits over reads please"), vec!["re-ask"]);
        let late = m("make the cache gauge show hits over reads", 4000.0);
        assert!(signals("make the cache gauge show hits over reads", Some(&late), "completed", None).0.is_empty(), "asked again an hour later: a new task");
        assert!(strong("fix it", "no—that broke it").contains(&"correction"), "a dash after no is not split mid-character");
        assert_eq!(signals("add the flag", Some(&m("wait, why add a new file?", 60.0)), "completed", None).1, vec!["pushback"]);
        assert_eq!(signals("add the flag", Some(&m("progress?", 60.0)), "completed", None).1, vec!["waited"]);
        assert_eq!(signals("add the flag", None, "crashed", Some("discarded")).0, vec!["crashed", "discarded"]);
        let stopped = Msg { next: String::new(), gap: None, stopped: true, rewound: true };
        assert_eq!(signals("add the flag", Some(&stopped), "completed", None).0, vec!["stopped", "rewound"]);
        assert!(signals("What's your model name?", Some(&m("no", 5.0)), "completed", None).0.is_empty(), "a probe has no signals");
        assert!(probe("what is your name") && probe("test") && probe("When were you trained?") && !probe("test the parser"));
        assert!(skipped_folder("/Users/x/bench/runs/a") && !skipped_folder("/Users/x/code/app"));
    }
}
