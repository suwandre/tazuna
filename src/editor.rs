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
    /// A message put aside with ctrl-x.
    stash: Option<String>,
    /// Where a mouse selection started (the cursor is its other end).
    pub anchor: Option<usize>,
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

    /// The selected text's byte range, when there is some.
    pub fn selection(&self) -> Option<(usize, usize)> {
        let a = self.anchor.filter(|&a| a <= self.text.len() && self.text.is_char_boundary(a))?;
        (a != self.cursor).then(|| (a.min(self.cursor), a.max(self.cursor)))
    }

    /// Removes the selected text; true when there was some. The selection ends either way.
    fn cut_selection(&mut self) -> bool {
        let sel = self.selection();
        self.anchor = None;
        let Some((a, z)) = sel else { return false };
        self.text.replace_range(a..z, "");
        self.cursor = a;
        true
    }

    /// Typing (or a paste) replaces the selection.
    pub fn insert(&mut self, s: &str) {
        self.cut_selection();
        self.text.insert_str(self.cursor, s);
        self.cursor += s.len();
        self.browsing = None;
    }

    pub fn backspace(&mut self) {
        if self.cut_selection() {
            return;
        }
        if let Some(c) = self.text[..self.cursor].chars().next_back() {
            self.cursor -= c.len_utf8();
            self.text.remove(self.cursor);
        }
    }

    pub fn delete(&mut self) {
        if self.cut_selection() {
            return;
        }
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

    /// Ctrl-U (cmd-backspace on a Mac): delete back to the start of the line; at the start of a line, the line break
    /// before it, so pressing it again goes on into the line above (it stopped there, and took a backspace first).
    pub fn kill_to_start(&mut self) {
        if self.cut_selection() {
            return;
        }
        let s = self.line_start();
        if s == self.cursor {
            self.backspace();
            return;
        }
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

    /// The word the cursor is at the end of, and where it starts: what Tab completes ("/com", "@src/ma").
    pub fn token(&self) -> (usize, &str) {
        let before = &self.text[..self.cursor];
        let start = before.rfind([' ', '\n', '\t']).map_or(0, |i| i + 1);
        (start, &before[start..])
    }

    /// Tab: the word at the cursor becomes `with`.
    pub fn complete(&mut self, with: &str) {
        let (start, _) = self.token();
        self.text.replace_range(start..self.cursor, with);
        self.cursor = start + with.len();
    }

    /// Ctrl-X: puts the message aside and clears the composer, or brings a stashed message back when the composer is
    /// empty (with both, they swap). A stash also comes back by itself after the next message is sent.
    pub fn toggle_stash(&mut self) -> &'static str {
        match (self.text.is_empty(), self.stash.take()) {
            (true, Some(s)) => {
                self.set(s);
                "unstashed"
            }
            (true, None) => "nothing to stash",
            (false, old) => {
                let text = std::mem::take(&mut self.text);
                self.clear();
                self.stash = Some(text);
                if let Some(o) = old {
                    self.set(o);
                    "swapped with the stash"
                } else {
                    "stashed: it comes back after you send the next message (ctrl-x brings it back now)"
                }
            }
        }
    }

    fn is_word(c: char) -> bool {
        c.is_alphanumeric() || c == '_'
    }

    /// Vim `w`: to the start of the next word.
    pub fn word_right(&mut self) {
        let rest = &self.text[self.cursor..];
        let mut it = rest.char_indices().peekable();
        let first = it.peek().map(|&(_, c)| Self::is_word(c));
        while let Some(&(_, c)) = it.peek() {
            if Some(Self::is_word(c)) != first || c.is_whitespace() {
                break;
            }
            it.next();
        }
        while let Some(&(_, c)) = it.peek() {
            if !c.is_whitespace() {
                break;
            }
            it.next();
        }
        self.cursor += it.peek().map_or(rest.len(), |&(i, _)| i);
    }

    /// Vim `b`: to the start of this or the previous word.
    pub fn word_left(&mut self) {
        let before: Vec<(usize, char)> = self.text[..self.cursor].char_indices().collect();
        let mut i = before.len();
        while i > 0 && before[i - 1].1.is_whitespace() {
            i -= 1;
        }
        if i > 0 {
            let kind = Self::is_word(before[i - 1].1);
            while i > 0 && !before[i - 1].1.is_whitespace() && Self::is_word(before[i - 1].1) == kind {
                i -= 1;
            }
        }
        self.cursor = before.get(i).map_or(if i == 0 { 0 } else { self.cursor }, |&(b, _)| b);
    }

    /// Vim `e`: to the end of this or the next word.
    pub fn word_end(&mut self) {
        self.right();
        let rest = &self.text[self.cursor..];
        let mut last = None;
        let mut started = false;
        let mut kind = None;
        for (i, c) in rest.char_indices() {
            if c.is_whitespace() {
                if started {
                    break;
                }
                continue;
            }
            if started && Some(Self::is_word(c)) != kind {
                break;
            }
            (started, kind, last) = (true, Some(Self::is_word(c)), Some(i));
        }
        if let Some(i) = last {
            self.cursor += i;
        }
    }

    /// Vim `dd`: deletes the line the cursor is on (with its line break).
    pub fn delete_line(&mut self) {
        let (s, e) = (self.line_start(), self.line_end());
        let (s, e) = if e < self.text.len() { (s, e + 1) } else if s > 0 { (s - 1, e) } else { (s, e) };
        self.text.replace_range(s..e, "");
        self.cursor = s.min(self.text.len());
        self.cursor = self.line_start();
    }

    /// One key in vim's normal mode. `op` holds a pending operator (`d`, `c`) across keys. Returns true when the key
    /// switches to insert mode.
    pub fn vim_normal(&mut self, c: char, op: &mut Option<char>) -> bool {
        if let Some(o) = op.take() {
            match (o, c) {
                ('d', 'd') => self.delete_line(),
                ('c', 'c') => {
                    self.home();
                    self.kill_to_end();
                    return true;
                }
                ('d' | 'c', 'w') => {
                    let s = self.cursor;
                    self.word_right();
                    self.text.replace_range(s..self.cursor, "");
                    self.cursor = s;
                    return o == 'c';
                }
                _ => {}
            }
            return false;
        }
        match c {
            'h' => self.left(),
            'l' => self.right(),
            '0' | '^' => self.home(),
            '$' => self.end(),
            'w' => self.word_right(),
            'b' => self.word_left(),
            'e' => self.word_end(),
            'x' => self.delete(),
            'X' => self.backspace(),
            'D' => self.kill_to_end(),
            'j' => self.down(),
            'k' => self.up(),
            'd' | 'c' => *op = Some(c),
            'C' => {
                self.kill_to_end();
                return true;
            }
            'i' => return true,
            'a' => {
                self.right();
                return true;
            }
            'I' => {
                self.home();
                return true;
            }
            'A' => {
                self.end();
                return true;
            }
            'o' => {
                self.end();
                self.insert("\n");
                return true;
            }
            _ => {}
        }
        false
    }

    /// Replaces the whole text (back from the external editor), cursor at the end.
    pub fn set(&mut self, text: String) {
        self.cursor = text.len();
        self.text = text;
        self.browsing = None;
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
        if let Some(stashed) = self.stash.take() {
            self.set(stashed); // a stashed message comes back once this one is sent
        }
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

/// Tab completions for the word at the cursor: slash commands at the start of a message, `@` file paths anywhere.
/// Paths that start with the typed text, or whose file name does, come before other matches.
pub fn suggest(token: &str, at_start: bool, commands: &[String], files: &[String]) -> Vec<String> {
    const MAX: usize = 6;
    if let Some(cmd) = token.strip_prefix('/').filter(|_| at_start) {
        return commands.iter().filter(|c| c.starts_with(cmd)).take(MAX).map(|c| format!("/{c}")).collect();
    }
    let Some(want) = token.strip_prefix('@') else { return vec![] };
    let want = want.to_lowercase();
    let name = |f: &str| f.rsplit('/').next().unwrap_or(f).to_lowercase();
    let mut hits: Vec<(u8, &String)> = files
        .iter()
        .filter_map(|f| {
            let l = f.to_lowercase();
            let rank = if l.starts_with(&want) || name(f).starts_with(&want) { 0 } else if l.contains(&want) { 1 } else { return None };
            Some((rank, f))
        })
        .collect();
    hits.sort_by_key(|(r, f)| (*r, f.len()));
    hits.into_iter().take(MAX).map(|(_, f)| format!("@{f}")).collect()
}

/// Byte index of character column `col` in the line `start..end`, or the line's end if it is shorter.
fn column(text: &str, start: usize, end: usize, col: usize) -> usize {
    text[start..end].char_indices().nth(col).map_or(end, |(i, _)| start + i)
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_selection_is_replaced_or_removed() {
        let mut e = super::Editor::default();
        e.insert("hello world");
        (e.anchor, e.cursor) = (Some(0), 5);
        assert_eq!(e.selection(), Some((0, 5)));
        e.insert("bye");
        assert_eq!((e.text.as_str(), e.cursor, e.anchor), ("bye world", 3, None), "typing replaces it");
        (e.anchor, e.cursor) = (Some(9), 3); // dragged right to left
        e.backspace();
        assert_eq!((e.text.as_str(), e.cursor), ("bye", 3));
        (e.anchor, e.cursor) = (Some(1), 1);
        e.backspace();
        assert_eq!(e.text, "ye", "a click without a drag selects nothing: a plain backspace");
    }

    #[test]
    fn stash_sets_the_message_aside_and_brings_it_back_after_sending() {
        let mut e = Editor::default();
        e.insert("long draft");
        e.toggle_stash();
        assert!(e.text.is_empty());
        e.insert("quick question");
        assert_eq!(e.take(), "quick question");
        assert_eq!(e.text, "long draft"); // back by itself after sending
        e.toggle_stash();
        assert_eq!(e.toggle_stash(), "unstashed");
        assert_eq!(e.text, "long draft");
    }

    #[test]
    fn vim_normal_mode_moves_and_edits() {
        let mut e = Editor::default();
        e.insert("fix the parser bug");
        let mut op = None;
        e.vim_normal('0', &mut op);
        e.vim_normal('w', &mut op);
        assert_eq!(&e.text[e.cursor..], "the parser bug");
        e.vim_normal('d', &mut op);
        e.vim_normal('w', &mut op);
        assert_eq!(e.text, "fix parser bug");
        e.vim_normal('e', &mut op);
        assert_eq!(&e.text[e.cursor..], "r bug");
        e.vim_normal('b', &mut op);
        assert_eq!(&e.text[e.cursor..], "parser bug");
        assert!(e.vim_normal('A', &mut op));
        assert_eq!(e.cursor, e.text.len());
        e.insert("\nsecond line");
        e.vim_normal('d', &mut op);
        e.vim_normal('d', &mut op);
        assert_eq!(e.text, "fix parser bug");
        assert!(e.vim_normal('c', &mut op) == false && e.vim_normal('c', &mut op));
        assert!(e.text.is_empty());
    }

    use super::*;

    #[test]
    fn line_kill_goes_on_into_the_line_above() {
        let mut e = Editor::default();
        e.insert("first\nsecond");
        e.kill_to_start();
        assert_eq!(e.text, "first\n");
        e.kill_to_start();
        assert_eq!((e.text.as_str(), e.cursor), ("first", 5), "at a line's start: the line break, not nothing");
        e.kill_to_start();
        assert_eq!(e.text, "");
        e.kill_to_start();
        assert_eq!(e.text, "", "at the very start: nothing to delete");
    }

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
    fn suggests_commands_and_files() {
        let cmds: Vec<String> = ["compact", "clear", "cost", "magic"].map(String::from).to_vec();
        let files: Vec<String> = ["src/main.rs", "src/ui/mod.rs", "README.md", "domain/main_test.go"].map(String::from).to_vec();
        assert_eq!(suggest("/c", true, &cmds, &files), vec!["/compact", "/clear", "/cost"]);
        assert!(suggest("/c", false, &cmds, &files).is_empty(), "commands only start a message");
        assert_eq!(suggest("@main", true, &cmds, &files), vec!["@src/main.rs", "@domain/main_test.go"]);
        assert_eq!(suggest("@ui/", true, &cmds, &files), vec!["@src/ui/mod.rs"]);
        assert!(suggest("plain", true, &cmds, &files).is_empty());
    }

    #[test]
    fn completes_the_word_at_the_cursor() {
        let mut e = Editor::default();
        e.insert("look at @src/ma");
        assert_eq!(e.token(), (8, "@src/ma"));
        e.complete("@src/main.rs ");
        assert_eq!(e.text, "look at @src/main.rs ");
        assert_eq!(e.token(), (21, ""));
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
