//! The composer's text editing: a cursor, readline-style keys, and history of sent messages (kept across sessions).

use std::path::PathBuf;

const HISTORY_MAX: usize = 500;
/// Undo steps kept; the oldest go first.
const UNDO_MAX: usize = 200;
/// A pause this long ends a typing run: the next key starts an undo step of its own.
const RUN_PAUSE: std::time::Duration = std::time::Duration::from_millis(1000);

/// The box as an undo step finds it: text, cursor, and the stash (an undone ctrl-x takes the message back out of it).
type Snapshot = (String, usize, Option<String>);

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
    /// Undo and redo: the text, cursor and stash before each step, newest last (design review 13: Cmd+Z did nothing). A
    /// run of typed keys or of backspaces is one step: `run` is where its last key left the cursor, when, and whether it
    /// typed.
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    run: Option<(usize, std::time::Instant, bool)>,
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

    /// The text as it is now becomes an undo step, before an edit changes it; what was undone can no longer be redone.
    fn step(&mut self) {
        if self.undo.len() == UNDO_MAX {
            self.undo.remove(0);
        }
        self.undo.push((self.text.clone(), self.cursor, self.stash.clone()));
        self.redo.clear();
        self.run = None;
    }

    /// A key that goes on the run before it (typing, or backspacing: `typing`) adds no step: right where the last one
    /// left the cursor, within a pause, and for typing not at a word's start, so an undo takes back a word at a time.
    fn run_on(&mut self, typing: bool, word_start: bool, now: std::time::Instant) {
        let on = self.anchor.is_none() && !word_start && self.run.is_some_and(|(at, when, t)| at == self.cursor && t == typing && now.duration_since(when) < RUN_PAUSE);
        if !on {
            self.step();
        }
    }

    /// A paste (or anything else put in whole) replaces the selection, as one undo step.
    pub fn insert(&mut self, s: &str) {
        self.step();
        self.put(s);
    }

    fn put(&mut self, s: &str) {
        self.cut_selection();
        self.text.insert_str(self.cursor, s);
        self.cursor += s.len();
        self.browsing = None;
    }

    /// A typed character: a run of them is one undo step.
    pub fn type_char(&mut self, c: char) {
        self.type_at(c, std::time::Instant::now());
    }

    fn type_at(&mut self, c: char, now: std::time::Instant) {
        let word_start = !c.is_whitespace() && self.text[..self.cursor].chars().next_back().is_some_and(char::is_whitespace);
        self.run_on(true, word_start, now);
        self.put(c.encode_utf8(&mut [0; 4]));
        self.run = Some((self.cursor, now, true));
    }

    pub fn backspace(&mut self) {
        self.run_on(false, false, std::time::Instant::now());
        if !self.cut_selection() && let Some(c) = self.text[..self.cursor].chars().next_back() {
            self.cursor -= c.len_utf8();
            self.text.remove(self.cursor);
        }
        self.run = Some((self.cursor, std::time::Instant::now(), false));
    }

    pub fn delete(&mut self) {
        self.run_on(false, false, std::time::Instant::now());
        if !self.cut_selection() && self.cursor < self.text.len() {
            self.text.remove(self.cursor);
        }
        self.run = Some((self.cursor, std::time::Instant::now(), false));
    }

    /// Cmd+Z, ctrl+z, ctrl+_: the text and cursor before the last step. False when there is none.
    pub fn undo(&mut self) -> bool {
        let Some(back) = self.undo.pop() else { return false };
        let now = self.restore(back);
        self.redo.push(now);
        true
    }

    /// Cmd+Shift+Z, ctrl+shift+z: the step undone last, again. False when there is none.
    pub fn redo(&mut self) -> bool {
        let Some(again) = self.redo.pop() else { return false };
        let now = self.restore(again);
        self.undo.push(now);
        true
    }

    /// The box as `to` has it; returns how it was.
    fn restore(&mut self, (text, cursor, stash): Snapshot) -> Snapshot {
        let was = (std::mem::replace(&mut self.text, text), self.cursor, std::mem::replace(&mut self.stash, stash));
        (self.cursor, self.anchor, self.browsing, self.run) = (cursor, None, None, None);
        was
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
        self.step();
        if self.cut_selection() {
            return;
        }
        let s = self.line_start();
        if let Some(c) = self.text[..self.cursor].chars().next_back().filter(|_| s == self.cursor) {
            self.cursor -= c.len_utf8();
            self.text.remove(self.cursor);
            return;
        }
        self.text.replace_range(s..self.cursor, "");
        self.cursor = s;
    }

    /// Ctrl-K: delete to the end of the line.
    pub fn kill_to_end(&mut self) {
        self.step();
        let e = self.line_end();
        self.text.replace_range(self.cursor..e, "");
    }

    /// Ctrl-W: delete back over whitespace, then to the previous whitespace.
    pub fn kill_word(&mut self) {
        self.step();
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
            Some(0) => return,
            None => self.history.len() - 1,
            Some(i) => i - 1,
        };
        self.step();
        if self.browsing.is_none() {
            self.draft = std::mem::take(&mut self.text);
        }
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
        self.step();
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
        self.step();
        let (start, _) = self.token();
        self.text.replace_range(start..self.cursor, with);
        self.cursor = start + with.len();
    }

    /// Ctrl-X: puts the message aside and clears the composer, or brings a stashed message back when the composer is
    /// empty (with both, they swap). A stash also comes back by itself after the next message is sent.
    pub fn toggle_stash(&mut self) -> &'static str {
        self.step();
        match (self.text.is_empty(), self.stash.take()) {
            (true, Some(s)) => {
                self.replace_text(s);
                "unstashed"
            }
            (true, None) => "nothing to stash",
            (false, old) => {
                let text = std::mem::take(&mut self.text);
                self.replace_text(String::new());
                self.stash = Some(text);
                if let Some(o) = old {
                    self.replace_text(o);
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
        self.step();
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
                    self.step();
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

    /// Replaces the whole text (back from the external editor), cursor at the end: one undo step.
    pub fn set(&mut self, text: String) {
        self.step();
        self.replace_text(text);
    }

    /// Esc: the draft cleared, as one undo step.
    pub fn clear(&mut self) {
        self.step();
        self.replace_text(String::new());
    }

    fn replace_text(&mut self, text: String) {
        self.cursor = text.len();
        self.text = text;
        self.browsing = None;
    }

    /// Ctrl-C at idle: the box cleared, its text kept in history (and its file) so ↑ brings it back; a stashed message stays
    /// stashed.
    #[inline(never)]
    pub fn shelve(&mut self) {
        let stash = self.stash.take();
        self.take();
        self.stash = stash;
    }

    /// Enter: the message to send. It goes into history (and its file) unless it repeats the last one. Undo starts over:
    /// a sent message is not taken back into the box.
    pub fn take(&mut self) -> String {
        let text = std::mem::take(&mut self.text);
        self.clear();
        if let Some(stashed) = self.stash.take() {
            self.set(stashed); // a stashed message comes back once this one is sent
        }
        (self.undo, self.redo, self.run) = (Vec::new(), Vec::new(), None);
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

/// The slash commands whose names start with `prefix`, then those that hold it elsewhere, each with what it does: the
/// list that opens over the message box when it holds only `/...` (OpenCode's command menu, the user's ask 2026-11-16).
/// `mine`: the user's own commands (behave.rs) with their descriptions; claude's names for them ("tazuna:x") are left
/// out, since a typed /x runs them.
/// Claude's commands come after tazuna's own (theirs said "Claude: …", `claude_row`); they led the list on a model that is
/// not Claude (design review 14).
pub fn command_menu(prefix: &str, commands: &[String], mine: &[(String, String)]) -> Vec<(String, String)> {
    let p = prefix.to_lowercase();
    let mut all: Vec<&String> = commands.iter().filter(|c| !c.starts_with("tazuna:")).chain(mine.iter().map(|m| &m.0)).collect();
    all.sort();
    all.dedup();
    let about = |c: &str| mine.iter().find(|m| m.0 == c).map_or_else(|| describe(c).to_string(), |m| m.1.clone());
    let starts = all.iter().filter(|c| c.starts_with(&p));
    let inside = all.iter().filter(|c| !c.starts_with(&p) && c.contains(&p));
    let mut rows: Vec<(String, String)> = starts.chain(inside).map(|c| (format!("/{c}"), about(c))).collect();
    rows.sort_by_key(|r| claude_row(&r.1).is_some()); // stable: the matches' order within each group
    rows.truncate(60);
    rows
}

/// A menu row's description when the command is Claude's: without its "Claude: ", for the menu's Claude group.
pub fn claude_row(about: &str) -> Option<&str> {
    about.strip_prefix("Claude: ")
}

/// claude's common built-in commands (those `describe` knows): the menu's guess on Claude before claude has listed its
/// own (live.rs, `commands`).
pub const CLAUDE_COMMON: [&str; 10] = ["agents", "clear", "compact", "config", "context", "fast", "init", "output-style", "rename", "security-review"];

/// What a slash command does, in a few words: tazuna's own, then claude's common ones; anything else is one of claude's
/// commands or skills from the repo or the user.
pub fn describe(cmd: &str) -> &'static str {
    match cmd {
        "agent" => "Send to the coding agent, past the chat router",
        "good" | "bad" => "Rate the last run (kept on this machine)",
        "boss" => "A lead model directs Haiku workers (on, off, or a model)",
        "chat" => "A plain chat, or back to the profile before it",
        "changelog" => "What each tazuna version changed (click one for its notes)",
        "update" => "Install the newest tazuna (the next start runs it)",
        "create" => "Make a command, skill or tool for every model (enable, discard)",
        "hook" => "A command of yours after each turn that changed files, at no token cost (off)",
        "data" => "Which providers may see your code (trusted, any)",
        "diff" => "What this conversation changed, file by file",
        "effort" => "The model's effort level",
        "mode" => "The permission mode: auto, full, ask, edits, review or plan",
        "explorer" => "The model that searches the repo for the main one",
        "jira" => "A ticket's text as the message: /jira KEY [request]",
        "keys" => "All keyboard shortcuts",
        "login" => "Save a provider's API key, or sign in (ChatGPT, Copilot, SuperGrok)",
        "loop" => "Build TODO.md's items one run at a time (/loop stop)",
        "mcp" => "Your MCP servers for every model (on, off, add)",
        "model" => "Pick the model and its effort",
        "new" => "A fresh conversation",
        "preset" => "A preset's settings for the rest of the session",
        "profile" => "How much of the repo's setup loads (repo, lean, chat)",
        "resume" => "Continue an earlier conversation of this folder",
        "review" => "A review of each finished change (on, off, a model)",
        "route" => "Whether general questions may go to a chat (on, off)",
        "verify" => "Tests and type checks after edits (on, off)",
        "rewind" => "Go back to before an earlier message",
        "usage" => "The plan's 5-hour and weekly use, the API balance; /usage track <provider>",
        "vim" => "Vim keys in the message box (on, off)",
        "clear" => "Claude: clear the conversation",
        "compact" => "Claude: summarise the conversation to free context",
        "context" => "Claude: what fills the context",
        "config" => "Claude: its settings",
        "init" => "Claude: write a CLAUDE.md for this repository",
        "security-review" => "Claude: a security review of the pending changes",
        "agents" => "Claude: its subagents",
        "rename" => "Claude: rename this conversation",
        "fast" => "Claude: fast mode",
        "output-style" => "Claude: how answers are written",
        _ => "Claude: a command or skill",
    }
}

/// Byte index of character column `col` in the line `start..end`, or the line's end if it is shorter.
fn column(text: &str, start: usize, end: usize, col: usize) -> usize {
    text[start..end].char_indices().nth(col).map_or(end, |(i, _)| start + i)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_command_menu_lists_matches_with_what_they_do() {
        let cmds: Vec<String> = ["compact", "diff", "model", "new", "review", "security-review", "my-skill"].map(String::from).to_vec();
        let m = super::command_menu("re", &cmds, &[]);
        assert_eq!(m.iter().map(|(c, _)| c.as_str()).collect::<Vec<_>>(), ["/review", "/security-review"], "names that start with it first");
        assert_eq!(m[0].1, "A review of each finished change (on, off, a model)");
        assert_eq!(super::command_menu("", &cmds, &[]).len(), 7);
        assert_eq!(super::command_menu("my", &cmds, &[])[0].1, "Claude: a command or skill");
        // tazuna's own first, then Claude's
        let all: Vec<String> = super::command_menu("", &cmds, &[]).into_iter().map(|r| r.0).collect();
        assert_eq!(all, ["/diff", "/model", "/new", "/review", "/compact", "/my-skill", "/security-review"]);
        // the user's own command: its description, and claude's plugin name for it left out
        let with = [cmds.clone(), vec!["tazuna:standup".into()]].concat();
        let m = super::command_menu("st", &with, &[("standup".into(), "Write today's standup".into())]);
        assert_eq!(m, [("/standup".to_string(), "Write today's standup".to_string())]);
    }

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
    fn ctrl_c_clears_the_box_and_up_brings_the_text_back() {
        let mut e = Editor::default();
        e.insert("long draft");
        e.toggle_stash();
        e.insert("half typed");
        e.shelve();
        assert!(e.text.is_empty(), "cleared, the stash not put back");
        e.up();
        assert_eq!(e.text, "half typed");
        e.down();
        assert_eq!(e.toggle_stash(), "unstashed");
        assert_eq!(e.text, "long draft");
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

    /// Undo (design review 13): a typing run is one step, broken at a word's start, a pause or a cursor move; a paste, a
    /// kill and a stash are one step each; redo goes until new typing; the cursor comes back too; at most UNDO_MAX steps.
    #[test]
    fn undo_takes_back_a_step_and_redo_brings_it_again() {
        let mut e = Editor::default();
        let t0 = std::time::Instant::now();
        for (k, c) in "hello world".chars().enumerate() {
            e.type_at(c, t0 + std::time::Duration::from_millis(50 * k as u64));
        }
        assert!(e.undo() && e.text == "hello " && e.cursor == 6, "a word at a time: {:?}", e.text);
        assert!(e.undo() && e.text.is_empty() && !e.undo());
        assert!(e.redo() && e.text == "hello " && e.redo() && e.text == "hello world" && !e.redo());
        // a pause ends the run
        e.type_at('!', t0 + std::time::Duration::from_secs(5));
        e.type_at('?', t0 + std::time::Duration::from_secs(7));
        assert!(e.undo() && e.text == "hello world!");
        // new typing ends redo
        e.type_char('x');
        assert!(!e.redo() && e.text == "hello world!x");
        // a paste is one step, the cursor comes back with the text
        e.home();
        e.insert("pasted text ");
        assert!(e.undo() && e.text == "hello world!x" && e.cursor == 0);
        // backspaces are one run; a cursor move ends it
        e.end();
        e.backspace();
        e.backspace();
        e.left();
        e.backspace();
        assert_eq!(e.text, "hello word");
        assert!(e.undo() && e.text == "hello world" && e.undo() && e.text == "hello world!x");
        // a kill and a stash
        e.kill_word();
        assert!(e.undo() && e.text == "hello world!x");
        e.toggle_stash();
        assert!(e.text.is_empty() && e.undo() && e.text == "hello world!x" && e.stash.is_none(), "an undone stash is out of the stash too");
        // sending starts over
        e.take();
        assert!(!e.undo());
        // the cap
        for k in 0..(UNDO_MAX + 50) {
            e.insert(&format!("{k} "));
        }
        let mut n = 0;
        while e.undo() {
            n += 1;
        }
        assert_eq!(n, UNDO_MAX);
        assert!(e.text.starts_with("0 1 2 ") && e.text.ends_with("49 "), "the oldest steps went: {}", e.text);
    }

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
