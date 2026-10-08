//! The composer's text editing: a cursor, readline-style keys, and history of sent messages (kept across sessions).

use std::path::PathBuf;

const HISTORY_MAX: usize = 500;

#[derive(Default)]
pub struct Editor {
    pub text: String,
    /// Byte index, always on a char boundary.
    pub cursor: usize,
    history: Vec<String>,
    /// Position while browsing history (None: editing the draft).
    browsing: Option<usize>,
    draft: String,
    file: Option<PathBuf>,
}

impl Editor {
    /// Loads earlier messages from `file` (one JSON string per line) and appends new ones there.
    pub fn with_history(file: PathBuf) -> Self {
        let history = std::fs::read_to_string(&file)
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str::<String>(l).ok())
            .collect::<Vec<_>>();
        let start = history.len().saturating_sub(HISTORY_MAX);
        Editor { history: history[start..].to_vec(), file: Some(file), ..Default::default() }
    }

    pub fn insert(&mut self, s: &str) {
        self.text.insert_str(self.cursor, s);
        self.cursor += s.len();
        self.browsing = None;
    }

    pub fn backspace(&mut self) {
        if let Some(c) = self.text[..self.cursor].chars().next_back() {
            self.cursor -= c.len_utf8();
            self.text.remove(self.cursor);
        }
    }

    pub fn delete(&mut self) {
        if self.cursor < self.text.len() {
            self.text.remove(self.cursor);
        }
    }

    pub fn left(&mut self) {
        if let Some(c) = self.text[..self.cursor].chars().next_back() {
            self.cursor -= c.len_utf8();
        }
    }

    pub fn right(&mut self) {
        if let Some(c) = self.text[self.cursor..].chars().next() {
            self.cursor += c.len_utf8();
        }
    }

    fn line_start(&self) -> usize {
        self.text[..self.cursor].rfind('\n').map_or(0, |i| i + 1)
    }

    fn line_end(&self) -> usize {
        self.text[self.cursor..].find('\n').map_or(self.text.len(), |i| self.cursor + i)
    }

    pub fn home(&mut self) {
        self.cursor = self.line_start();
    }

    pub fn end(&mut self) {
        self.cursor = self.line_end();
    }

    /// Ctrl-U: delete back to the start of the line.
    pub fn kill_to_start(&mut self) {
        let s = self.line_start();
        self.text.replace_range(s..self.cursor, "");
        self.cursor = s;
    }

    /// Ctrl-K: delete to the end of the line.
    pub fn kill_to_end(&mut self) {
        let e = self.line_end();
        self.text.replace_range(self.cursor..e, "");
    }

    /// Ctrl-W: delete back over whitespace, then to the previous whitespace.
    pub fn kill_word(&mut self) {
        let before = &self.text[..self.cursor];
        let trimmed = before.trim_end_matches([' ', '\t']);
        let s = trimmed.rfind([' ', '\t', '\n']).map_or(0, |i| i + 1);
        self.text.replace_range(s..self.cursor, "");
        self.cursor = s;
    }

    /// Up: the line above in multi-line input; on the first line, the previous message from history.
    pub fn up(&mut self) {
        let s = self.line_start();
        if s > 0 {
            let col = self.text[s..self.cursor].chars().count();
            let prev_start = self.text[..s - 1].rfind('\n').map_or(0, |i| i + 1);
            self.cursor = column(&self.text, prev_start, s - 1, col);
            return;
        }
        let i = match self.browsing {
            None if self.history.is_empty() => return,
            None => {
                self.draft = std::mem::take(&mut self.text);
                self.history.len() - 1
            }
            Some(0) => return,
            Some(i) => i - 1,
        };
        self.browsing = Some(i);
        self.text = self.history[i].clone();
        self.cursor = self.text.len();
    }

    /// Down: the line below; on the last line, the next message from history, then back to the draft.
    pub fn down(&mut self) {
        let e = self.line_end();
        if e < self.text.len() {
            let col = self.text[self.line_start()..self.cursor].chars().count();
            let next_end = self.text[e + 1..].find('\n').map_or(self.text.len(), |i| e + 1 + i);
            self.cursor = column(&self.text, e + 1, next_end, col);
            return;
        }
        let Some(i) = self.browsing else { return };
        if i + 1 < self.history.len() {
            self.browsing = Some(i + 1);
            self.text = self.history[i + 1].clone();
        } else {
            self.browsing = None;
            self.text = std::mem::take(&mut self.draft);
        }
        self.cursor = self.text.len();
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
        self.browsing = None;
    }

    /// Enter: the message to send. It goes into history (and its file) unless it repeats the last one.
    pub fn take(&mut self) -> String {
        let text = std::mem::take(&mut self.text);
        self.clear();
        let t = text.trim().to_string();
        if !t.is_empty() && self.history.last() != Some(&t) {
            self.history.push(t.clone());
            if let Some(f) = &self.file {
                use std::io::Write;
                if let Ok(mut h) = std::fs::OpenOptions::new().create(true).append(true).open(f) {
                    let _ = h.write_all(format!("{}\n", serde_json::Value::String(t)).as_bytes());
                }
            }
        }
        text
    }
}

/// Byte index of character column `col` in the line `start..end`, or the line's end if it is shorter.
fn column(text: &str, start: usize, end: usize, col: usize) -> usize {
    text[start..end].char_indices().nth(col).map_or(end, |(i, _)| start + i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_at_the_cursor() {
        let mut e = Editor::default();
        e.insert("helo wörld");
        for _ in 0..7 {
            e.left();
        }
        e.insert("l");
        assert_eq!(e.text, "hello wörld");
        e.end();
        e.backspace();
        assert_eq!(e.text, "hello wörl");
        e.kill_word();
        assert_eq!(e.text, "hello ");
        e.insert("a\nsecond line");
        e.home();
        e.kill_to_end();
        assert_eq!(e.text, "hello a\n");
        e.up();
        assert_eq!(e.cursor, 0, "up keeps the column (0)");
        e.kill_to_end();
        assert_eq!(e.text, "\n");
    }

    #[test]
    fn browses_history_and_keeps_the_draft() {
        let dir = std::env::temp_dir().join(format!("tazuna-editor-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("history.jsonl");
        let mut e = Editor::with_history(file.clone());
        e.insert("first");
        assert_eq!(e.take(), "first");
        e.insert("second\nline");
        e.take();
        e.insert("draft");
        e.up();
        assert_eq!(e.text, "second\nline");
        e.up();
        assert_eq!(e.text, "second\nline".to_string(), "on the last line, up moves within the message first");
        e.up();
        assert_eq!(e.text, "first");
        e.down();
        e.down();
        e.down();
        assert_eq!(e.text, "draft", "back to what was being typed");
        let again = Editor::with_history(file);
        assert_eq!(again.history, vec!["first", "second\nline"], "history survives a restart");
        std::fs::remove_dir_all(&dir).ok();
    }
}
