//! Rendering. Four designs share one transcript builder and differ in palette, chrome and layout.

mod aurora;
mod cockpit;
mod timeline;
mod zen;

use crate::session::{Activity, Block, Session, Tool};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use tachyonfx::{Effect, Interpolation::*, Motion, fx};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub struct Theme {
    pub bg: Color,
    pub surface: Color,
    pub border: Color,
    pub text: Color,
    pub dim: Color,
    pub faint: Color,
    pub accent: Color,
    pub accent2: Color,
    pub warn: Color,
    pub err: Color,
    pub ok: Color,
    pub code: Color,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

const AURORA: Theme = Theme {
    bg: rgb(13, 15, 20), surface: rgb(20, 23, 31), border: rgb(44, 49, 63), text: rgb(214, 218, 226), dim: rgb(122, 129, 148),
    faint: rgb(72, 78, 95), accent: rgb(139, 156, 255), accent2: rgb(94, 234, 212), warn: rgb(246, 193, 119), err: rgb(235, 111, 146),
    ok: rgb(156, 207, 145), code: rgb(186, 196, 222),
};
const COCKPIT: Theme = Theme {
    bg: rgb(9, 12, 18), surface: rgb(15, 20, 30), border: rgb(34, 46, 66), text: rgb(205, 216, 232), dim: rgb(110, 128, 156),
    faint: rgb(58, 72, 96), accent: rgb(86, 214, 255), accent2: rgb(181, 140, 255), warn: rgb(255, 196, 92), err: rgb(255, 99, 120),
    ok: rgb(98, 230, 160), code: rgb(170, 200, 235),
};
const ZEN: Theme = Theme {
    bg: rgb(25, 23, 30), surface: rgb(33, 31, 40), border: rgb(52, 49, 62), text: rgb(224, 219, 232), dim: rgb(144, 138, 158),
    faint: rgb(88, 83, 100), accent: rgb(235, 188, 186), accent2: rgb(156, 207, 216), warn: rgb(246, 193, 119), err: rgb(235, 111, 146),
    ok: rgb(170, 200, 160), code: rgb(200, 192, 214),
};
const TIMELINE: Theme = Theme {
    bg: rgb(7, 7, 11), surface: rgb(17, 17, 26), border: rgb(40, 40, 60), text: rgb(222, 222, 232), dim: rgb(120, 120, 145),
    faint: rgb(62, 62, 84), accent: rgb(0, 229, 255), accent2: rgb(255, 64, 160), warn: rgb(255, 214, 0), err: rgb(255, 70, 90),
    ok: rgb(124, 255, 132), code: rgb(190, 205, 255),
};

#[derive(Clone, Copy, PartialEq)]
pub enum Design {
    Aurora,
    Cockpit,
    Zen,
    Timeline,
}

impl Design {
    pub const ALL: [Design; 4] = [Design::Aurora, Design::Cockpit, Design::Zen, Design::Timeline];

    pub fn theme(self) -> &'static Theme {
        [&AURORA, &COCKPIT, &ZEN, &TIMELINE][self as usize]
    }

    /// `n` is 1-based, as typed on the keyboard.
    pub fn from_number(n: usize) -> Option<Design> {
        Self::ALL.get(n.checked_sub(1)?).copied()
    }

    pub fn next(self) -> Design {
        Self::ALL[(self as usize + 1) % Self::ALL.len()]
    }
}

pub struct View {
    pub design: Design,
    pub expand_all: bool,
    /// Blocks opened by a click (their full input and output), and the block under the mouse (shaded: a click opens it).
    pub open: std::collections::HashSet<usize>,
    pub hover: Option<usize>,
    /// Where the mouse is (column, row): a link under it is drawn highlighted.
    pub mouse: Option<(u16, u16)>,
    /// When the conversation on screen began (ms into tazuna's run): `/new` and `/resume` start the elapsed clock again.
    pub since_ms: f64,
    /// A short note in the composer for a moment ("copied 42 chars"), and when it came.
    pub toast: Option<(String, std::time::Instant)>,
    /// The files this conversation changed (git), and the `/diff` view of them when open.
    pub changed: Vec<Changed>,
    pub diff: Option<DiffPanel>,
    /// A tool's calls or the model calls (cockpit), over the screen.
    pub list: Option<ListPanel>,
    /// The model call under the mouse in the tokens chart.
    pub hover_call: Option<usize>,
    /// Lines scrolled up from the bottom; 0 follows the live tail.
    pub scroll: usize,
    pub clock_ms: f64,
    pub speed: f64,
    pub paused: bool,
    /// Live mode: the user drives a real claude process and the composer takes input.
    pub live: bool,
    pub input: String,
    /// Byte index of the cursor in `input`.
    pub cursor: usize,
    /// Text selected in `input` with the mouse (byte range), drawn highlighted.
    pub select: Option<(usize, usize)>,
    /// A permission question waiting for y/a/n: "Bash: npm test".
    pub asking: Option<String>,
    /// Permission mode (auto, ask, edits, plan).
    pub mode: String,
    /// Tab completions for the word at the cursor (slash commands, `@` files).
    pub hints: Vec<String>,
    /// Background tasks still running (shown by the composer).
    pub bg_running: usize,
    /// Those tasks: a footer under the transcript follows them live.
    pub running: Vec<TaskRow>,
    /// The ctrl-b view of background tasks, when open.
    pub tasks: Option<TasksPanel>,
    /// The /rewind picker, when open.
    pub rewind: Option<RewindPanel>,
    pub sessions: Option<SessionsPanel>,
    /// The /model picker, when open.
    pub models: Option<ModelsPanel>,
    /// /login, when open.
    pub login: Option<LoginPanel>,
    /// Claude's cache has expired: the tokens the next message re-reads without it (0: warm, small, or the own loop).
    pub cold: u64,
    /// Vim mode's state for the composer ("NORMAL", "INSERT"); empty when vim mode is off.
    pub vim: &'static str,
}

/// `/login`: the providers (name, how it logs in, its state now), and a provider's page while its key is typed.
pub struct LoginPanel {
    pub rows: Vec<(String, String, String)>,
    pub selected: usize,
    pub form: Option<LoginForm>,
}

/// A provider's login page, OMP-style: where keys are made (opened in the browser), the key typed so far (its length
/// only: it shows masked), what saving it does.
pub struct LoginForm {
    pub name: String,
    pub page: String,
    pub opened: bool,
    pub place: String,
    pub example: String,
    pub typed: usize,
    pub host: String,
    pub var: String,
    /// Where its key comes from now (`models::key_source`); empty: none.
    pub has: String,
}

/// `/resume`: this repo's earlier sessions, newest first ("8m ago · 2 messages · fix the login"), and the one selected.
pub struct SessionsPanel {
    pub rows: Vec<String>,
    pub selected: usize,
}

pub struct RewindPanel {
    /// The messages sent so far, oldest first.
    pub items: Vec<String>,
    pub selected: usize,
}

pub struct ModelsPanel {
    pub rows: Vec<ModelRow>,
    pub selected: usize,
    /// What has been typed to narrow the list.
    pub filter: String,
    /// The left pane, OMP-style: "All models" then each provider (name, how many models, usable now, why not), the one
    /// selected, and whether the keys move in the right pane (its models) or in the left (the providers).
    pub groups: Vec<(String, usize, bool, String)>,
    pub group: usize,
    pub in_models: bool,
    /// A model chosen, its effort being picked: (its label, its levels, the one selected, the key line, what the switch
    /// costs where it rebuilds Claude's cache; empty: nothing to say).
    pub effort: Option<(String, Vec<String>, usize, &'static str, String)>,
}

pub struct ModelRow {
    pub label: String,
    pub place: String,
    pub price: String,
    pub note: String,
    /// "6.8 (308)": our judge and its runs; "32": Artificial Analysis' index (empty where unknown).
    pub bench: String,
    pub aa: String,
    /// 0 ready, 1 no key, 2 provider not trusted (the code would go there), 3 unchecked route, 4 not allowed on the key
    pub state: u8,
    /// Why it cannot be picked and what to do (the key variable, the trusted list's path), where it cannot.
    pub why: String,
    pub stale: bool,
    pub current: bool,
}

pub struct TasksPanel {
    pub rows: Vec<TaskRow>,
    pub selected: usize,
}

pub struct TaskRow {
    pub kind: String,
    pub description: String,
    pub status: String,
    pub secs: u64,
    pub output: String,
    /// A subagent's type, tokens and tool calls so far, and what it is doing now.
    pub agent: String,
    pub tokens: u64,
    pub tools: u64,
    pub doing: String,
}

/// Screen regions the effects target.
#[derive(Default)]
pub struct Areas {
    /// Flashes when a tool finishes.
    pub pulse: Rect,
    /// Rows of the newest block, when it is on screen at the bottom.
    pub last_block: Option<Rect>,
    /// Where the transcript is drawn: mouse selection stays inside it.
    pub transcript: Rect,
    /// The block shown on each row of the transcript (None: a gap), for clicks and hovering.
    pub rows: Vec<Option<usize>>,
    /// Links on screen and where they open to: a click opens one in the browser.
    pub links: Vec<(Rect, String)>,
    /// The files panel (cockpit): a click there opens the changed files and their diffs.
    pub files: Option<Rect>,
    /// The diff view's file rows and which file each is: a click opens that file in the editor.
    pub diff_files: Vec<(Rect, usize)>,
    /// The tools panel's rows and the tool on each (cockpit): a click lists that tool's calls.
    pub tool_rows: Vec<(Rect, String)>,
    /// The tokens panel (cockpit): a click lists every model call.
    pub tokens: Option<Rect>,
    /// The bar chart of output per call and the call its first bar is: the mouse over a bar names that call.
    pub bars: Option<(Rect, usize)>,
    /// The transcript line on the transcript's first row.
    pub top_line: usize,
    /// The composer's text area (inside its border): a click there puts the cursor under it, a drag selects.
    pub composer: Option<Rect>,
    /// A list modal: where it is (a click outside closes it), its link labels (a click opens one) and its rows (a
    /// click unfolds one).
    pub list: Option<Rect>,
    pub list_links: Vec<(Rect, String)>,
    pub list_rows: Vec<(Rect, usize)>,
    /// ctrl-b's task rows: a click opens one.
    pub task_rows: Vec<(Rect, usize)>,
}

/// One file this conversation changed, with its lines added and removed (git).
#[derive(Clone)]
pub struct Changed {
    pub path: String,
    pub added: u64,
    pub removed: u64,
    /// The diff as the agent's edit tools wrote it, for a file git shows no change in now (committed during the
    /// conversation, or outside the repo); None: git has the diff.
    pub diff: Option<Vec<String>>,
}

/// A list over the screen (cockpit): a tool's calls, or every model call. ↑↓ and the wheel scroll it, esc closes.
pub struct ListPanel {
    pub title: String,
    /// The column heads, drawn above the rows.
    pub head: String,
    pub rows: Vec<(String, bool)>,
    pub scroll: usize,
    /// A row's link (a fetched page): its text from `link_from` on is drawn as a link, a click opens it.
    pub links: Vec<Option<String>>,
    pub link_from: usize,
    /// Columns scrolled to the right (←/→, shift+wheel): long rows ran off the modal with no way to read them.
    pub hscroll: usize,
    /// A row's whole text (a long command), unfolded under it when the row is clicked; rows show cut to the modal.
    pub details: Vec<Option<String>>,
    pub open: std::collections::BTreeSet<usize>,
}

/// `/diff` (or a click on the files panel): the changed files, the selected one's diff, how far it is scrolled.
pub struct DiffPanel {
    pub selected: usize,
    pub diff: Vec<String>,
    pub scroll: usize,
}

pub fn draw(f: &mut Frame, s: &Session, v: &View) -> Areas {
    let th = v.design.theme();
    f.render_widget(ratatui::widgets::Block::new().style(Style::new().bg(th.bg)), f.area());
    let areas = match v.design {
        Design::Aurora => aurora::draw(f, s, v, th),
        Design::Cockpit => cockpit::draw(f, s, v, th),
        Design::Zen => zen::draw(f, s, v, th),
        Design::Timeline => timeline::draw(f, s, v, th),
    };
    let mut areas = areas;
    areas.links = link_cells(f.buffer_mut(), areas.transcript, &links_in(s));
    // the link under the mouse, highlighted (reversed): it is clickable, and without a change it looked like plain text
    if let Some((x, y)) = v.mouse {
        let pos = ratatui::layout::Position::new(x, y);
        if let Some((r, _)) = areas.links.iter().find(|(r, _)| r.contains(pos)) {
            let buf = f.buffer_mut();
            for cy in r.y..r.y + r.height {
                for cx in r.x..r.x + r.width {
                    if let Some(c) = buf.cell_mut((cx, cy)) {
                        c.modifier.insert(Modifier::REVERSED);
                    }
                }
            }
        }
    }
    // a modal over the screen: what is behind it fades to the border colour, so the modal reads on its own (a user
    // found the list mixed into the transcript and panels, 2026-11-05; a terminal cannot blur)
    if v.tasks.is_some() || v.rewind.is_some() || v.sessions.is_some() || v.diff.is_some() || v.models.is_some() || v.list.is_some() {
        let area = f.area();
        let buf = f.buffer_mut();
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                if let Some(c) = buf.cell_mut((x, y)) {
                    c.set_style(Style::new().fg(th.border).bg(th.bg).remove_modifier(Modifier::all()));
                }
            }
        }
    }
    if let Some(p) = &v.tasks {
        areas.task_rows = draw_tasks(f, p, th);
    }
    if let Some(p) = &v.rewind {
        draw_rewind(f, p, th);
    }
    if let Some(p) = &v.sessions {
        draw_sessions(f, p, th);
    }
    if let Some(p) = &v.diff {
        areas.diff_files = draw_diff(f, &v.changed, p, th, v.mouse);
    }
    if let Some(p) = &v.models {
        draw_models(f, p, th);
    }
    if let Some(p) = &v.login {
        draw_login(f, p, th);
    }
    if let Some(p) = &v.list {
        (areas.list, areas.list_links, areas.list_rows) = draw_list(f, p, th);
    }
    areas
}

/// A ListPanel in the middle of the screen: its title, the column heads, the rows that fit (failed ones in red).
fn draw_list(f: &mut Frame, p: &ListPanel, th: &Theme) -> (Option<Rect>, Vec<(Rect, String)>, Vec<(Rect, usize)>) {
    use ratatui::widgets::{Block, Borders, Clear, Paragraph};
    let full = f.area();
    let want = p.rows.iter().map(|r| r.0.width()).chain([p.head.width(), p.title.width()]).max().unwrap_or(20) as u16 + 4;
    let (w, h) = (want.clamp(40, full.width), (p.rows.len() as u16 + 4).clamp(6, full.height * 85 / 100));
    let area = Rect::new(full.x + (full.width - w) / 2, full.y + (full.height - h) / 2, w, h);
    f.render_widget(Clear, area);
    let block = Block::new().borders(Borders::ALL).border_style(st(th.accent)).style(Style::new().bg(th.bg))
        .title(Span::styled(format!(" {} · click a row: all of it · ↑↓ scroll · ←→ or shift+wheel sideways · esc or click outside closes ", p.title), st(th.accent).add_modifier(Modifier::BOLD)));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let width = inner.width as usize;
    // sideways scrolled: the text from there on; else cut to the modal with "…" (a click unfolds it)
    let cut = |t: &str| -> String {
        let t: String = t.chars().skip(p.hscroll).collect();
        if p.hscroll == 0 && t.width() > width { format!("{}…", t.chars().take(width.saturating_sub(1)).collect::<String>()) } else { t }
    };
    let mut lines = vec![Line::from(Span::styled(cut(&p.head), st(th.faint).add_modifier(Modifier::BOLD)))];
    let (mut links, mut rows) = (vec![], vec![]);
    let room = inner.height.saturating_sub(1) as usize;
    for (i, (r, bad)) in p.rows.iter().enumerate().skip(p.scroll) {
        if lines.len() > room {
            break;
        }
        let y = inner.y + lines.len() as u16;
        let color = st(if *bad { th.err } else { th.text });
        let shown = cut(r);
        rows.push((Rect::new(inner.x, y, inner.width, 1), i));
        match p.links.get(i).cloned().flatten() {
            Some(url) => {
                // the label after the fixed columns is the link: underlined, opened on a click
                let at = p.link_from.saturating_sub(p.hscroll).min(shown.chars().count());
                let (a, b): (String, String) = (shown.chars().take(at).collect(), shown.chars().skip(at).collect());
                links.push((Rect::new(inner.x + at as u16, y, (b.width() as u16).min(inner.width.saturating_sub(at as u16)), 1), url));
                lines.push(Line::from(vec![Span::styled(a, color), Span::styled(b, st(th.accent).add_modifier(Modifier::UNDERLINED))]));
            }
            None => lines.push(Line::from(Span::styled(shown, color))),
        }
        // an unfolded row: its whole text under it, wrapped, indented to the label's column, dimmed
        if let Some(d) = p.open.contains(&i).then(|| p.details.get(i).cloned().flatten()).flatten() {
            let indent = p.link_from.min(width / 3);
            let w = width.saturating_sub(indent).max(10);
            for chunk in d.chars().collect::<Vec<_>>().chunks(w) {
                if lines.len() > room {
                    break;
                }
                lines.push(Line::from(Span::styled(format!("{}{}", " ".repeat(indent), chunk.iter().collect::<String>()), st(th.dim))));
            }
        }
    }
    f.render_widget(Paragraph::new(lines), inner);
    (Some(area), links, rows)
}

/// The files the agent's edit tools wrote that `skip` (git's list) leaves out, each with a diff built from the calls
/// themselves: an edit's old text as removed lines and its new text as added ones, a write's content as added. For a
/// file committed during the conversation, or one outside the repo: git shows no change there (a user's panel read
/// "no changes in the repo · 5 files written outside it" with nothing to open, 2026-11-05).
pub fn written_by_tools(s: &Session, skip: &[String]) -> Vec<Changed> {
    let mut out: Vec<Changed> = Vec::new();
    for b in &s.blocks {
        let Block::Tool(t) = b else { continue };
        let path = tool_arg(t, "path");
        if path.is_empty() || t.error {
            continue;
        }
        let pairs: Vec<(String, String)> = match t.name.as_str() {
            "Edit" | "edit" => vec![(tool_arg(t, "old").to_string(), tool_arg(t, "new").to_string())],
            "MultiEdit" => t.input["edits"].as_array().into_iter().flatten()
                .map(|e| (e["old_string"].as_str().unwrap_or_default().to_string(), e["new_string"].as_str().unwrap_or_default().to_string())).collect(),
            "Write" | "write" => vec![(String::new(), t.input["content"].as_str().unwrap_or_default().to_string())],
            "NotebookEdit" => vec![(String::new(), t.input["new_source"].as_str().unwrap_or_default().to_string())],
            _ => continue,
        };
        let f = rel(s, path);
        if skip.contains(&f) {
            continue;
        }
        let i = match out.iter().position(|c| c.path == f) {
            Some(i) => i,
            None => {
                out.push(Changed { path: f.clone(), added: 0, removed: 0, diff: Some(vec![format!("+++ {f} (as the agent wrote it: git shows no change here now)")]) });
                out.len() - 1
            }
        };
        let c = &mut out[i];
        let d = c.diff.get_or_insert_with(Vec::new);
        d.push(format!("@@ {} @@", if t.name.eq_ignore_ascii_case("write") { "written" } else { "edit" }));
        for (old, new) in pairs {
            d.extend(old.lines().map(|l| format!("-{l}")));
            d.extend(new.lines().map(|l| format!("+{l}")));
            c.removed += old.lines().count() as u64;
            c.added += new.lines().count() as u64;
        }
    }
    out
}

/// The ctrl-b view: background tasks with status and age, and the latest output of the selected one.
fn draw_tasks(f: &mut Frame, p: &TasksPanel, th: &Theme) -> Vec<(Rect, usize)> {
    use ratatui::widgets::{Block, Borders, Clear, Paragraph};
    let full = f.area();
    let (w, h) = ((full.width * 9 / 10).max(20).min(full.width), (full.height * 8 / 10).max(8).min(full.height));
    let area = Rect::new(full.x + (full.width - w) / 2, full.y + (full.height - h) / 2, w, h);
    f.render_widget(Clear, area);
    let block = Block::new().borders(Borders::ALL).border_style(st(th.accent)).style(Style::new().bg(th.bg))
        .title(Span::styled(" background tasks · ↑↓ select · enter open an agent · x stop · esc close ", st(th.accent).add_modifier(Modifier::BOLD)));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let mut lines: Vec<Line<'static>> = Vec::new();
    if p.rows.is_empty() {
        lines.push(Line::from(Span::styled("No background tasks in this session yet.", st(th.faint))));
    }
    // a long list scrolls with the selection: the window kept the first rows, and the selected one went off its bottom
    // edge (a session with 30-odd tasks, 2026-11-10); the output below keeps a third of the height
    let list_room = (inner.height as usize).saturating_sub(2 + (inner.height as usize / 3)).max(3);
    let (first, last) = if p.rows.len() <= list_room {
        (0, p.rows.len())
    } else {
        let room = list_room - 2; // a line each for "↑ N earlier" and "↓ N more"
        let first = p.selected.saturating_sub(room / 2).min(p.rows.len() - room);
        (first, first + room)
    };
    if first > 0 {
        lines.push(Line::from(Span::styled(format!("  ↑ {first} earlier"), st(th.faint))));
    }
    let mut rows = vec![];
    for (i, r) in p.rows.iter().enumerate().take(last).skip(first) {
        rows.push((Rect::new(inner.x, inner.y + lines.len() as u16, inner.width, 1), i));
        let mark = if i == p.selected { "›" } else { " " };
        let age = if r.secs >= 60 { format!("{}m{:02}s", r.secs / 60, r.secs % 60) } else { format!("{}s", r.secs) };
        let colour = if r.status == "running" { th.accent } else if r.status == "completed" { th.faint } else { th.warn };
        let row = format!("{mark} {:<10} {:>7}  {:<11} {}", plain(&r.status), age, plain(&r.kind), plain(&r.description).replace('\n', " "));
        // the selected row in reverse video, as in the other lists: bold alone on a completed (faint) row was hard to find
        let style = if i == p.selected { st(th.accent).add_modifier(Modifier::BOLD | Modifier::REVERSED) } else { st(colour) };
        lines.push(Line::from(Span::styled(format!("{:<w$}", row.chars().take(inner.width as usize).collect::<String>(), w = inner.width as usize), style)));
    }
    if p.rows.len() > last {
        lines.push(Line::from(Span::styled(format!("  ↓ {} more", p.rows.len() - last), st(th.faint))));
    }
    if let Some(r) = p.rows.get(p.selected) {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(format!("── output of: {} ──", plain(&r.description).replace('\n', " ")).chars().take(inner.width as usize).collect::<String>(), st(th.faint))));
        let text = plain(&r.output);
        let room = (inner.height as usize).saturating_sub(lines.len());
        let out: Vec<&str> = text.lines().collect();
        let shown = if r.kind == "local_agent" { vec!["(a subagent: enter or a click opens its conversation)"] } else if out.is_empty() { vec!["(no output yet)"] } else { out[out.len().saturating_sub(room)..].to_vec() };
        for l in shown {
            lines.push(Line::from(Span::styled(l.chars().take(inner.width as usize).collect::<String>(), st(th.text))));
        }
    }
    f.render_widget(Paragraph::new(lines), inner);
    rows.retain(|(r, _)| r.y < inner.bottom());
    rows
}

/// /rewind: the messages sent so far; the chosen one and everything after it can be undone.
/// The files this conversation changed beside the selected one's diff (added lines green, removed red, hunks in the
/// accent), scrollable.
fn draw_diff(f: &mut Frame, files: &[Changed], p: &DiffPanel, th: &Theme, mouse: Option<(u16, u16)>) -> Vec<(Rect, usize)> {
    // the changed files as a list, each opened in the editor at its first change: a diff squeezed into a modal ran off
    // its right edge and could not be scrolled sideways (a user, 2026-11-06); the editor shows diffs better
    use ratatui::widgets::{Block, Borders, Clear, Paragraph};
    let full = f.area();
    let (add, del) = files.iter().fold((0, 0), |(a, d), c| (a + c.added, d + c.removed));
    let most = files.iter().map(|c| c.added + c.removed).max().unwrap_or(1).max(1);
    let cwd = std::env::current_dir().unwrap_or_default();
    let rows: Vec<(String, String, String, bool)> = files.iter().map(|c| {
        // a file no longer on disk (a removed worktree): listed, but it cannot be opened
        let gone = c.diff.is_some() && !cwd.join(&c.path).exists();
        (plain(&c.path), format!("+{}", c.added), format!("−{}", c.removed), gone)
    }).collect();
    let pw = rows.iter().map(|r| r.0.width()).max().unwrap_or(10);
    let title = format!(" {} changed · +{add} −{del} · ↑↓ · enter or click: open in your editor · esc ", files_n(files.len()));
    let w = ((pw + 40).max(title.width() + 2) as u16).min(full.width.saturating_sub(4)).max(30);
    let h = (files.len() as u16 + 2).clamp(4, full.height * 85 / 100);
    let area = Rect::new(full.x + (full.width - w) / 2, full.y + (full.height - h) / 2, w, h);
    f.render_widget(Clear, area);
    let block = Block::new().borders(Borders::ALL).border_style(st(th.accent)).style(Style::new().bg(th.bg))
        .title(Span::styled(title, st(th.accent).add_modifier(Modifier::BOLD)));
    let inner = block.inner(area);
    f.render_widget(block, area);
    // the selected file stays in view
    let first = (p.selected + 1).saturating_sub(inner.height as usize);
    let room = (inner.width as usize).saturating_sub(34);
    let mut hits = vec![];
    let lines: Vec<Line> = rows.iter().enumerate().skip(first).take(inner.height as usize).map(|(i, (path, a, d, gone))| {
        let y = inner.y + (i - first) as u16;
        let r = Rect::new(inner.x, y, inner.width, 1);
        hits.push((r, i));
        let hover = mouse.is_some_and(|(x, my)| r.contains(ratatui::layout::Position::new(x, my)));
        let name = if path.width() > room { format!("…{}", path.chars().rev().take(room.saturating_sub(1)).collect::<Vec<_>>().into_iter().rev().collect::<String>()) } else { path.clone() };
        let c = &files[i];
        let (ba, bd) = ((c.added as f64 / most as f64 * 12.0).ceil() as usize, (c.removed as f64 / most as f64 * 12.0).ceil() as usize);
        let base = if i == p.selected { Style::new().bg(th.surface) } else if hover { Style::new().bg(th.surface) } else { Style::new() };
        let mut l = vec![
            Span::styled(if i == p.selected { "› " } else { "  " }, st(th.accent)),
            Span::styled(format!("{name:<room$}"), if hover { st(th.accent).add_modifier(Modifier::UNDERLINED) } else { st(th.text) }),
            Span::styled(format!(" {a:>6}"), st(th.ok)),
            Span::styled(format!(" {d:>6} "), st(th.err)),
            Span::styled("▮".repeat(ba.min(12)), st(th.ok)),
            Span::styled("▮".repeat(bd.min(12 - ba.min(12))), st(th.err)),
        ];
        if *gone {
            l.push(Span::styled(" gone", st(th.faint)));
        }
        Line::from(l).style(base)
    }).collect();
    f.render_widget(Paragraph::new(lines), inner);
    hits
}

/// The new-file line where a diff's first change is (its first added line; a deletion: where it was), for opening the
/// file there. 1 when the diff has no hunk (a new file).
pub fn first_change(diff: &[String]) -> u32 {
    let mut line = 0;
    for l in diff {
        if let Some(h) = l.strip_prefix("@@ ") {
            // "@@ -a,b +c,d @@": the new file's hunk starts at c
            line = h.split_whitespace().find_map(|p| p.strip_prefix('+')).and_then(|p| p.split(',').next()?.parse().ok()).unwrap_or(1);
        } else if line > 0 {
            if l.starts_with('+') || l.starts_with('-') {
                return line.max(1);
            }
            line += 1;
        }
    }
    1
}

/// "1 file", "3 files".
pub fn files_n(n: usize) -> String {
    if n == 1 { "1 file".into() } else { format!("{n} files") }
}

fn draw_sessions(f: &mut Frame, p: &SessionsPanel, th: &Theme) {
    use ratatui::widgets::{Block, Borders, Clear, Paragraph};
    let full = f.area();
    let (w, h) = ((full.width * 9 / 10).max(20).min(full.width), ((p.rows.len().max(1) as u16 + 4).min(full.height * 8 / 10)).max(6).min(full.height));
    let area = Rect::new(full.x + (full.width - w) / 2, full.y + (full.height - h) / 2, w, h);
    f.render_widget(Clear, area);
    let block = Block::new().borders(Borders::ALL).border_style(st(th.accent)).style(Style::new().bg(th.bg))
        .title(Span::styled(" resume a session · ↑↓ select · enter open · esc ", st(th.accent).add_modifier(Modifier::BOLD)));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let room = inner.height.saturating_sub(2) as usize;
    let first = p.selected.saturating_sub(room.saturating_sub(1));
    let mut lines: Vec<Line<'static>> = vec![Line::from(Span::styled("This repo's sessions on claude, newest first; the conversation opens where it ended.", st(th.faint))), Line::from("")];
    if p.rows.is_empty() {
        lines.push(Line::from(Span::styled("no earlier claude session in this repo", st(th.faint))));
    }
    for (i, row) in p.rows.iter().enumerate().skip(first).take(room) {
        let (mark, style) = if i == p.selected { ("›", st(th.accent).add_modifier(Modifier::BOLD | Modifier::REVERSED)) } else { (" ", st(th.text)) };
        lines.push(Line::from(Span::styled(format!("{mark} {}", plain(row).replace('\n', " ⏎ ")).chars().take(inner.width as usize).collect::<String>(), style)));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_login(f: &mut Frame, p: &LoginPanel, th: &Theme) {
    use ratatui::widgets::{Block, Borders, Clear, Paragraph};
    let full = f.area();
    let w = full.width.min(90).max(20.min(full.width));
    let iw = w.saturating_sub(4) as usize;
    let wrapped = |text: &str, c: Color| -> Vec<Line<'static>> {
        wrap_words(text, iw).into_iter().map(|l| Line::from(Span::styled(l, st(c)))).collect()
    };
    let (title, lines) = match &p.form {
        Some(fm) => {
            let mut lines = vec![Line::from(""), Line::from(Span::styled(fm.page.clone(), st(th.accent).add_modifier(Modifier::UNDERLINED)))];
            lines.extend(wrapped(if fm.opened { "opened in your browser" } else { "open it in your browser" }, th.faint));
            lines.push(Line::from(""));
            lines.extend(wrapped(&format!("Create or copy your API key in {}.", fm.place), th.warn));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(format!("Paste your {} API key", fm.name), st(th.text).add_modifier(Modifier::BOLD))));
            if !fm.example.is_empty() {
                lines.push(Line::from(Span::styled(format!("e.g. {}", fm.example), st(th.faint))));
            }
            lines.push(Line::from(vec![Span::styled("> ", st(th.accent)), Span::styled("•".repeat(fm.typed.min(iw.saturating_sub(3))), st(th.text)),
                                       Span::styled("▍", st(th.accent))]));
            lines.push(Line::from(Span::styled(if fm.has.is_empty() { "(esc to cancel, enter to submit)" } else { "(esc to cancel, enter to submit; enter with nothing typed keeps the key it has)" }, st(th.faint))));
            lines.push(Line::from(""));
            lines.extend(wrapped(&format!("{} will see the code of the sessions you run on it, so saving also puts {} on your trusted providers. \
                The key stays in ~/.config/tazuna/keys.json, readable by you only, and out of the agent's commands.", fm.name, fm.host), th.dim));
            if fm.has.starts_with("key in the environment") {
                lines.extend(wrapped(&format!("{} is set in your environment, and it wins over a key saved here.", fm.var), th.dim));
            }
            (format!(" Login to {} ", fm.name), lines)
        }
        None => {
            let mut lines = wrapped("Log in to a provider to use its models in /model. None of these has a login for third-party tools, so it is an API key: the provider's page opens in your browser to make one.", th.faint);
            lines.push(Line::from(""));
            let nw = p.rows.iter().map(|r| r.0.chars().count()).max().unwrap_or(0);
            let kw = p.rows.iter().map(|r| r.1.chars().count()).max().unwrap_or(0);
            for (i, (name, how, state)) in p.rows.iter().enumerate() {
                let on = i == p.selected;
                let mark = if on { "›" } else { " " };
                let ok = !state.contains("not ");
                let row = format!("{mark} {name:<nw$}  {how:<kw$}  ");
                let left = row.chars().count();
                lines.push(Line::from(vec![
                    Span::styled(row, if on { st(th.accent).add_modifier(Modifier::BOLD) } else { st(th.text) }),
                    Span::styled(state.chars().take(iw.saturating_sub(left)).collect::<String>(), st(if ok { th.ok } else { th.faint })),
                ]));
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled("↑↓ provider · enter log in · esc close", st(th.faint))));
            (" login ".to_string(), lines)
        }
    };
    let h = (lines.len() as u16 + 2).min(full.height);
    let area = Rect::new(full.x + (full.width - w) / 2, full.y + (full.height - h) / 2, w, h);
    f.render_widget(Clear, area);
    let block = Block::new().borders(Borders::ALL).border_style(st(th.accent)).style(Style::new().bg(th.bg))
        .title(Span::styled(title, st(th.accent).add_modifier(Modifier::BOLD)));
    let inner = block.inner(area);
    f.render_widget(block, area);
    f.render_widget(Paragraph::new(lines), Rect::new(inner.x + 1, inner.y, inner.width.saturating_sub(2), inner.height));
}

fn draw_rewind(f: &mut Frame, p: &RewindPanel, th: &Theme) {
    use ratatui::widgets::{Block, Borders, Clear, Paragraph};
    let full = f.area();
    let (w, h) = ((full.width * 9 / 10).max(20).min(full.width), ((p.items.len() as u16 + 6).min(full.height * 8 / 10)).max(8).min(full.height));
    let area = Rect::new(full.x + (full.width - w) / 2, full.y + (full.height - h) / 2, w, h);
    f.render_widget(Clear, area);
    let block = Block::new().borders(Borders::ALL).border_style(st(th.accent)).style(Style::new().bg(th.bg))
        .title(Span::styled(" rewind to before · ↑↓ select · enter files + conversation · f files · c conversation · esc ", st(th.accent).add_modifier(Modifier::BOLD)));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let room = inner.height.saturating_sub(2) as usize;
    let first = p.selected.saturating_sub(room.saturating_sub(1));
    let mut lines: Vec<Line<'static>> = vec![Line::from(Span::styled("Undo the chosen message and everything after it.", st(th.faint))), Line::from("")];
    for (i, text) in p.items.iter().enumerate().skip(first).take(room) {
        let mark = if i == p.selected { "›" } else { " " };
        let row = format!("{mark} {:>2}. {}", i + 1, plain(text).replace('\n', " ⏎ "));
        let style = if i == p.selected { st(th.accent).add_modifier(Modifier::BOLD) } else { st(th.text) };
        lines.push(Line::from(Span::styled(row.chars().take(inner.width as usize).collect::<String>(), style)));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

/// /model: Claude's models and the checked routes; ready ones in the text colour, a missing key greyed out, an untrusted
/// provider (the code would go there) in the warning colour, a route checked long ago marked stale. The columns fit the
/// panel's width: the label gives way first (cut in the middle, so "· fast" stays), then the scores go; the price always
/// shows. The selected row in full (provider, price, cache, age, the reason it cannot be picked) wraps below the list.
fn draw_models(f: &mut Frame, p: &ModelsPanel, th: &Theme) {
    use ratatui::widgets::{Block, Borders, Clear, Paragraph};
    let full = f.area();
    // OMP's layout: the providers on the left, the selected one's models on the right; a narrow screen keeps the models
    let two = full.width >= 70 && !p.groups.is_empty();
    let gw = if two { p.groups.iter().map(|(n, c, ..)| n.chars().count() + c.to_string().len() + 6).max().unwrap_or(18).clamp(18, 30) } else { 0 };
    // column widths: as wide as their header or longest entry
    let width = |head: &str, f: &dyn Fn(&ModelRow) -> usize| p.rows.iter().map(f).chain([head.chars().count()]).max().unwrap_or(0);
    let pw = width("$/M in / out", &|r| r.price.chars().count());
    let bw = width("bench (runs)", &|r| r.bench.chars().count());
    let aw = width("AA", &|r| r.aa.chars().count());
    let longest = width("model", &|r| r.label.chars().count());
    let keys = if let Some((_, _, _, k, _)) = &p.effort {
        k
    } else if p.in_models || !two {
        "↑↓ model · ← providers · tab provider pin · enter choose · esc close"
    } else {
        "↑↓ providers · → its models · type to search · esc close"
    };
    // as wide as the panes need (and the key line), at most 9/10 of the screen
    let table = 2 + longest + 1 + pw + 1 + bw + 1 + aw;
    // one width while it is open (filtering does not make it jump): what the panes want, but at least 100 and at most
    // 9/10 of the screen
    let w = ((gw + usize::from(two) + table).max(keys.chars().count()).max(100) as u16 + 2).min(full.width * 9 / 10).max(20).min(full.width);
    let h = (full.height * 8 / 10).max(12).min(full.height);
    let area = Rect::new(full.x + (full.width - w) / 2, full.y + (full.height - h) / 2, w, h);
    f.render_widget(Clear, area);
    let block = Block::new().borders(Borders::ALL).border_style(st(th.accent)).style(Style::new().bg(th.bg))
        .title(Span::styled(" model ", st(th.accent).add_modifier(Modifier::BOLD)));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let body = Rect::new(inner.x, inner.y, inner.width, inner.height.saturating_sub(1));
    f.render_widget(Paragraph::new(Line::from(Span::styled(keys, st(th.faint)))), Rect::new(inner.x, inner.y + inner.height.saturating_sub(1), inner.width, 1));
    // the left pane: All models, then the providers, ● usable now, ○ not (no key, not trusted)
    if two {
        let mut lines = vec![Line::from(Span::styled("providers", st(th.faint)))];
        for (i, (name, n, ok, _)) in p.groups.iter().enumerate() {
            let mark = if i == p.group { "›" } else { " " };
            let dot = if i == 0 { "◆" } else if *ok { "●" } else { "○" };
            let room = gw.saturating_sub(5 + n.to_string().len());
            let label: String = name.chars().take(room).collect();
            let text = format!("{mark}{dot} {label:<room$} {n}");
            let color = if *ok || i == 0 { th.text } else { th.faint };
            let style = match (i == p.group, p.in_models) {
                (true, false) => st(color).add_modifier(Modifier::BOLD | Modifier::REVERSED),
                (true, true) => st(th.accent).add_modifier(Modifier::BOLD),
                _ => st(color),
            };
            let mut spans = vec![Span::styled(text, style)];
            if *ok && i > 0 && !(i == p.group && !p.in_models) {
                // the dot in the theme's ok colour, the rest as is
                let t = spans.remove(0).content.to_string();
                let (head, tail) = t.split_at(t.char_indices().nth(1).map_or(t.len(), |(b, _)| b));
                let (dot_s, rest) = tail.split_at(tail.char_indices().nth(1).map_or(tail.len(), |(b, _)| b));
                spans = vec![Span::styled(head.to_string(), style), Span::styled(dot_s.to_string(), st(th.ok)), Span::styled(rest.to_string(), style)];
            }
            lines.push(Line::from(spans));
        }
        f.render_widget(Paragraph::new(lines), Rect::new(body.x, body.y, gw as u16, body.height));
        let sep: Vec<Line<'static>> = (0..body.height).map(|_| Line::from(Span::styled("│", st(th.border)))).collect();
        f.render_widget(Paragraph::new(sep), Rect::new(body.x + gw as u16, body.y, 1, body.height));
    }
    let right = if two { Rect::new(body.x + gw as u16 + 2, body.y, body.width.saturating_sub(gw as u16 + 2), body.height) } else { body };
    let iw = right.width as usize;
    let cut = |s: String| s.chars().take(iw).collect::<String>();
    let (gname, _, gok, gwhy) = p.groups.get(p.group).cloned().unwrap_or_else(|| ("models".into(), p.rows.len(), true, String::new()));
    // the details under the list: the selected model in full, wrapped; with the providers in hand, the provider's state
    let detail: Vec<Line<'static>> = if !p.in_models && two {
        let state = if gok || p.group == 0 { format!("{} models · enter or → to pick one", p.rows.len()) } else { gwhy.clone() };
        [(gname.clone(), th.text), (state, if gok || p.group == 0 { th.dim } else { th.warn })].into_iter()
            .flat_map(|(t, c)| wrap_words(&t, iw).into_iter().map(move |l| Line::from(Span::styled(l, st(c))))).collect()
    } else {
        p.rows.get(p.selected).map(|r| {
            let price = if r.price == "plan" { "on the plan".to_string() } else { format!("${} per M tokens (in / out)", r.price.replace(" / ", " / $")) };
            let scores = [(!r.bench.is_empty()).then(|| format!("bench {}", r.bench)), (!r.aa.is_empty()).then(|| format!("AA {}", r.aa))];
            let why = match r.state {
                1 | 2 | 4 if !r.why.is_empty() => r.why.clone(),
                _ if r.stale => format!("stale: {}", r.note),
                _ => r.note.clone(),
            };
            let color = match r.state { 1 | 4 => th.faint, 2 => th.warn, _ if r.stale => th.warn, _ => th.dim };
            let head: Vec<String> = [Some(r.label.clone()), Some(price)].into_iter().chain(scores).flatten().collect();
            [(head.join(" · "), th.text), (r.place.clone(), th.dim), (why, color)].into_iter()
                .filter(|(text, _)| !text.is_empty())
                .flat_map(|(text, c)| wrap_words(&text, iw).into_iter().map(move |l| Line::from(Span::styled(l, st(c)))))
                .collect()
        }).unwrap_or_default()
    };
    let dh = (detail.len() as u16).min(right.height.saturating_sub(4)); // at least one row stays
    let room = right.height.saturating_sub(2 + dh + u16::from(dh > 0)) as usize; // search, header; a gap before the details
    let first = p.selected.saturating_sub(room.saturating_sub(1));
    let fixed = 2 + pw;
    let scores = iw >= fixed + 1 + bw + 1 + aw + 1 + 16.min(longest); // room for the scores and a 16-character label
    // the label column takes what is left, so the table spans the pane
    let lw = iw.saturating_sub(fixed + 1 + if scores { bw + 1 + aw + 1 } else { 0 }).max(4);
    let fit = |s: &str| {
        let n = s.chars().count();
        if n <= lw {
            return s.to_string();
        }
        let tail = (lw - 1) / 3;
        format!("{}…{}", s.chars().take(lw - 1 - tail).collect::<String>(), s.chars().skip(n - tail).collect::<String>())
    };
    let cols = |mark: &str, l: &str, pr: &str, b: &str, a: &str| {
        let mut s = format!("{mark} {:<lw$} {pr:<pw$}", fit(l));
        if scores {
            s.push_str(&format!(" {b:<bw$} {a:>aw$}"));
        }
        cut(s)
    };
    let search = if p.filter.is_empty() {
        Line::from(Span::styled(cut(format!("{gname} · {} models · type to search", p.rows.len())), st(th.faint)))
    } else {
        Line::from(vec![Span::styled("> ", st(th.accent)), Span::styled(cut(p.filter.clone()), st(th.text)), Span::styled("▍", st(th.accent))])
    };
    // a model chosen: the right pane picks its effort
    if let Some((label, levels, at, _, note)) = &p.effort {
        let mut lines = vec![Line::from(vec![Span::styled(cut(label.clone()), st(th.text).add_modifier(Modifier::BOLD)), Span::styled(" · effort", st(th.faint))]), Line::from("")];
        for (i, l) in levels.iter().enumerate() {
            let style = if i == *at { st(th.accent).add_modifier(Modifier::BOLD | Modifier::REVERSED) } else { st(th.text) };
            lines.push(Line::from(Span::styled(cut(format!("{} {l}", if i == *at { "›" } else { " " })), style)));
        }
        lines.push(Line::from(""));
        lines.extend(wrap_words("A higher level thinks longer and costs more. /effort changes it later; the conversation goes on.", iw)
            .into_iter().map(|l| Line::from(Span::styled(l, st(th.faint)))));
        if !note.is_empty() {
            lines.push(Line::from(""));
            lines.extend(wrap_words(note, iw).into_iter().map(|l| Line::from(Span::styled(l, st(th.warn)))));
        }
        f.render_widget(Paragraph::new(lines), right);
        return;
    }
    let mut lines: Vec<Line<'static>> = vec![search, Line::from(Span::styled(cols(" ", "model", "$/M in / out", "bench (runs)", "AA"), st(th.faint)))];
    for (i, r) in p.rows.iter().enumerate().skip(first).take(room) {
        let on = i == p.selected && (p.in_models || !two);
        let mark = if on { "›" } else if r.current { "•" } else { " " };
        let row = cols(mark, &r.label, &r.price, &r.bench, &r.aa);
        let color = match r.state { 1 | 4 => th.faint, 2 => th.warn, 3 => th.dim, _ if r.stale => th.warn, _ => th.text };
        let style = if on { st(color).add_modifier(Modifier::BOLD | Modifier::REVERSED) } else { st(color) };
        lines.push(Line::from(Span::styled(row, style)));
    }
    if p.rows.is_empty() {
        lines.push(Line::from(Span::styled("no model matches", st(th.faint))));
    }
    f.render_widget(Paragraph::new(lines), right);
    if dh > 0 {
        let at = Rect::new(right.x, right.y + right.height - dh, right.width, dh);
        f.render_widget(Paragraph::new(detail), at);
    }
}

/// `s` in lines of at most `w` characters, broken between words (a word longer than a line is split).
fn wrap_words(s: &str, w: usize) -> Vec<String> {
    let w = w.max(1);
    let mut out: Vec<String> = vec![];
    let mut line = String::new();
    for word in s.split(' ').filter(|x| !x.is_empty()) {
        let mut word: Vec<char> = word.chars().collect();
        let n = line.chars().count();
        if n > 0 && n + 1 + word.len() > w {
            out.push(std::mem::take(&mut line));
        }
        while word.len() > w {
            out.push(word.drain(..w).collect());
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.extend(word);
    }
    if !line.is_empty() || out.is_empty() {
        out.push(line);
    }
    out
}

/// Command output (untrusted: whatever a command printed) as plain text for the screen. Escape sequences go entirely:
/// CSI (colours, cursor moves), OSC up to BEL or ST (window titles, clipboard writes), DCS/APC/PM/SOS strings and
/// two-character ones. Carriage returns go; any other control or invisible character (C1 controls such as U+009B, BEL,
/// backspace, bidi overrides) becomes a visible `�`, as in permission questions (`session::shown`).
pub fn plain(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '\x1b' => match it.next() {
                Some('[') => {
                    // parameters and intermediates, up to the final byte @ ... ~
                    while let Some(n) = it.next() {
                        if ('@'..='~').contains(&n) {
                            break;
                        }
                    }
                }
                Some(']' | 'P' | '_' | '^' | 'X') => {
                    while let Some(n) = it.next() {
                        if n == '\x07' {
                            break;
                        }
                        if n == '\x1b' && it.peek() == Some(&'\\') {
                            it.next();
                            break;
                        }
                    }
                }
                _ => {} // a two-character sequence: both dropped
            },
            '\r' => {}
            '\n' | '\t' => out.push(c),
            c if c.is_control() || crate::session::invisible(c) => out.push('\u{FFFD}'),
            c => out.push(c),
        }
    }
    out
}

// ---------------------------------------------------------------- effects

pub fn boot_fx(d: Design) -> Effect {
    let bg = d.theme().bg;
    match d {
        Design::Aurora => fx::parallel(&[fx::sweep_in(Motion::LeftToRight, 40, 12, bg, (900, QuadOut)), fx::fade_from_fg(bg, (700, SineOut))]),
        Design::Cockpit => fx::parallel(&[fx::slide_in(Motion::RightToLeft, 24, 4, bg, (650, QuadOut)), fx::fade_from_fg(bg, (500, QuadOut))]),
        Design::Zen => fx::fade_from(bg, bg, (1400, SineInOut)),
        Design::Timeline => fx::parallel(&[fx::sweep_in(Motion::UpToDown, 16, 0, bg, (800, QuadOut)), fx::fade_from_fg(bg, (600, ExpoOut))]),
    }
}

pub fn block_fx(d: Design) -> Effect {
    let bg = d.theme().bg;
    match d {
        Design::Aurora => fx::fade_from_fg(bg, (380, QuadOut)),
        Design::Cockpit => fx::fade_from_fg(bg, (240, QuadOut)),
        Design::Zen => fx::fade_from_fg(bg, (750, SineOut)),
        Design::Timeline => fx::parallel(&[fx::fade_from_fg(bg, (200, QuadOut)), fx::ping_pong(fx::hsl_shift_fg([0.0, 25.0, 30.0], (150, QuadOut)))]),
    }
}

pub fn pulse_fx(d: Design) -> Effect {
    let shift = match d {
        Design::Zen => [0.0, 0.0, 10.0],
        Design::Timeline => [0.0, 40.0, 30.0],
        _ => [0.0, 15.0, 22.0],
    };
    fx::ping_pong(fx::hsl_shift_fg(shift, (170, QuadOut)))
}

pub fn done_fx(d: Design) -> Effect {
    let bg = d.theme().bg;
    match d {
        Design::Aurora => fx::sequence(&[fx::coalesce((650, ExpoOut)), fx::ping_pong(fx::hsl_shift_fg([30.0, 15.0, 18.0], (300, SineInOut)))]),
        Design::Cockpit => fx::sweep_in(Motion::LeftToRight, 20, 0, bg, (600, QuadOut)),
        Design::Zen => fx::fade_from_fg(bg, (1200, SineInOut)),
        Design::Timeline => fx::sequence(&[fx::coalesce((500, ExpoOut)), fx::ping_pong(fx::hsl_shift_fg([0.0, 40.0, 25.0], (220, QuadOut)))]),
    }
}

pub fn quit_fx() -> Effect {
    fx::dissolve((450, QuadIn))
}

// ---------------------------------------------------------------- shared pieces

pub fn st(c: Color) -> Style {
    Style::new().fg(c)
}

const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn spinner(clock_ms: f64) -> &'static str {
    SPINNER[(clock_ms / 80.0) as usize % SPINNER.len()]
}

/// Glyph, label and color for what the agent is doing right now.
pub fn activity(s: &Session, v: &View, th: &Theme) -> (&'static str, String, Color) {
    let spin = spinner(v.clock_ms);
    match s.activity {
        // the turn is over, but claude's background work goes on: say so where "done" would be (ctrl-b lists it)
        Activity::Ready | Activity::Done if v.bg_running > 0 => (spin, format!("{} working in background · ctrl-b", v.bg_running), th.accent2),
        Activity::Ready => ("●", "ready".into(), th.dim),
        Activity::Starting => (spin, "starting".into(), th.dim),
        Activity::Waiting => (spin, "waiting for model".into(), th.dim),
        Activity::Thinking => (spin, "thinking".into(), th.warn),
        Activity::Writing => (spin, "writing".into(), th.accent),
        Activity::Tool => (spin, format!("running {}", last_tool_name(s)), th.accent2),
        Activity::Verifying => (spin, "verifying".into(), th.accent2),
        Activity::Done => ("✓", "done".into(), th.ok),
    }
}

fn last_tool_name(s: &Session) -> String {
    s.blocks.iter().rev().find_map(|b| if let Block::Tool(t) = b { Some(short_name(&t.name).to_string()) } else { None }).unwrap_or_default()
}

/// Money and plan apart: "$0.50 + $7.46 on Claude plan"; "$7.46 on Claude plan" when only claude ran, "$0.50" when only the own loop did.
pub fn cost_str(s: &Session) -> String {
    let (paid, plan) = s.split_cost();
    let about = if s.estimated { "≈" } else { "" };
    match s.exact_cost {
        None => format!("≈{}", usd(s.cost())),
        Some(_) if paid > 0.0 && plan > 0.0 => format!("{about}{} + {} on Claude plan", usd(paid), usd(plan)),
        Some(_) if paid > 0.0 => format!("{about}{}", usd(paid)),
        Some(c) => format!("{about}{} on Claude plan", usd(c)),
    }
}

/// Dollars in cents, and below a cent in two significant digits: a cheap model's whole task ($0.00019 on DeepSeek,
/// 2026-11-01) read as $0.00.
pub fn usd(x: f64) -> String {
    if x <= 0.0 {
        "$0".into()
    } else if x < 0.01 {
        format!("${x:.*}", (1 - x.log10().floor() as i32) as usize)
    } else {
        format!("${x:.2}")
    }
}

/// Claude's plan windows and SubRouter's balance on one line ("5h 13% · 7d 21% · SubRouter $8.82 left"), and the colour
/// to show it in: warning at 70%, error at 85% (where the benchmark guard pauses).
pub fn limits_str(s: &Session, th: &Theme) -> Option<(String, Color)> {
    match (plan_str(s, th), balance_str(s)) {
        (Some((p, color)), Some(b)) => Some((format!("{p} · {b}"), color)),
        (Some(p), None) => Some(p),
        (None, Some(b)) => Some((b, th.dim)),
        (None, None) => None,
    }
}

/// "SubRouter $8.82 left", where a SubRouter key is set.
pub fn balance_str(s: &Session) -> Option<String> {
    s.balance.map(|b| format!("SubRouter {} left", usd(b)))
}

/// The plan's windows alone: "5h 12% · 7d 14% (2h ago)".
pub fn plan_str(s: &Session, th: &Theme) -> Option<(String, Color)> {
    let mut parts = vec![];
    let mut color = th.dim;
    if let Some((h5, d7)) = s.limits {
        color = match h5.max(d7) { x if x >= 85.0 => th.err, x if x >= 70.0 => th.warn, _ => th.dim };
        // the windows are the whole account's (Claude Code beside tazuna counts too), so no "this session" share: a
        // "(+2%)" read as tazuna's (2026-11-02). Claude reports them only now and then: an older reading says how old.
        let age = s.limits_at.map(|t| crate::session::unix_now().saturating_sub(t)).filter(|a| *a >= 600).map_or(String::new(), |a| match a {
            0..3600 => format!(" · {}m ago", a / 60),
            3600..86_400 => format!(" · {}h ago", a / 3600),
            _ => format!(" · {}d ago", a / 86_400),
        });
        // named: a second subscription (Codex, say) would get a line of its own
        parts.push(format!("Claude 5h {h5:.0}% · 7d {d7:.0}%{age}"));
    }
    (!parts.is_empty()).then(|| (parts.join(" · "), color))
}

pub fn clock_str(ms: f64) -> String {
    let secs = (ms / 1000.0) as u64;
    if secs >= 3600 {
        format!("{}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60)
    } else {
        format!("{:02}:{:02}", secs / 60, secs % 60)
    }
}

/// The session's elapsed time and, beside it, the agent's working time (message to last token): "26:51 · 06:08 working".
pub fn times_str(s: &Session, v: &View) -> String {
    format!("{} · {} working", clock_str(v.clock_ms - v.since_ms), clock_str(s.busy(v.clock_ms)))
}

pub fn human(n: u64) -> String {
    match n {
        0..1_000 => n.to_string(),
        1_000..1_000_000 => format!("{:.1}k", n as f64 / 1e3),
        _ => format!("{:.2}M", n as f64 / 1e6),
    }
}

/// One name per kind of tool, whichever profile ran it: the lean tools (`mcp__lean__edit`) and Claude Code's own (`Edit`).
pub fn short_name(name: &str) -> &str {
    match name.strip_prefix("mcp__lean__").unwrap_or(name) {
        "Edit" | "MultiEdit" => "edit",
        "Write" => "write",
        "Read" => "read",
        "Bash" => "bash",
        n => n,
    }
}

/// A tool argument under either profile's key: lean `path`/`old`/`new`, Claude Code `file_path`/`old_string`/`new_string`.
pub fn tool_arg<'a>(t: &'a Tool, key: &str) -> &'a str {
    let keys: &[&str] = match key {
        "path" => &["path", "file_path", "notebook_path"],
        "old" => &["old", "old_string"],
        "new" => &["new", "new_string"],
        k => return t.input[k].as_str().unwrap_or_default(),
    };
    keys.iter().find_map(|k| t.input[*k].as_str()).unwrap_or_default()
}

pub fn model_short(m: &str) -> String {
    if m.is_empty() { "connecting…".into() } else { m.strip_prefix("claude-").unwrap_or(m).to_string() }
}

/// Files the agent touched, most recent first: tool paths plus file-looking words in shell commands.
/// Files the agent wrote with an edit tool (Write, Edit, MultiEdit, NotebookEdit; the own loop's write and edit), newest
/// first. Commands are not parsed for paths: that counted grep patterns and scratch files as files.
pub fn files_touched(s: &Session) -> Vec<String> {
    const EDITS: [&str; 6] = ["Write", "Edit", "MultiEdit", "NotebookEdit", "write", "edit"];
    let mut out: Vec<String> = Vec::new();
    for b in s.blocks.iter().rev() {
        let Block::Tool(t) = b else { continue };
        let p = tool_arg(t, "path");
        if EDITS.contains(&t.name.as_str()) && !p.is_empty() {
            let f = rel(s, p);
            if !out.contains(&f) {
                out.push(f);
            }
        }
    }
    out
}

fn rel(s: &Session, p: &str) -> String {
    p.strip_prefix(&s.cwd).map(|r| r.trim_start_matches('/')).unwrap_or(p).to_string()
}

pub fn tool_summary(t: &Tool, s: &Session) -> String {
    let i = &t.input;
    let arg = |k: &str| tool_arg(t, k);
    match short_name(&t.name) {
        _ if i.is_null() => "…".to_string(), // input is still streaming
        "bash" => arg("command").lines().next().unwrap_or_default().to_string(),
        "read" | "write" | "edit" => rel(s, arg("path")),
        "Grep" | "Glob" => format!("{} {}", arg("pattern"), rel(s, arg("path"))).trim().to_string(),
        // a subagent: who and what for; its prompt shows when the call is opened
        "Agent" | "Task" => format!("{}: {}", i["subagent_type"].as_str().unwrap_or("agent"), i["description"].as_str().unwrap_or_default()),
        "WebSearch" | "ToolSearch" => i["query"].as_str().unwrap_or_default().to_string(),
        "fetch" | "WebFetch" => fetch_label(t),
        "Skill" => i["skill"].as_str().or(i["name"].as_str()).unwrap_or_default().to_string(),
        _ => i.to_string(),
    }
}

/// A fetch call as its page's title (from the result's first line) or the URL's host and path, not the URL in full
/// (a user found the raw `{"url":...}` rows hard to read, 2026-11-05).
pub fn fetch_label(t: &Tool) -> String {
    let url = t.input["url"].as_str().unwrap_or_default();
    let title = t.result.as_deref().and_then(|r| r.lines().next()).and_then(|l| l.split_once(" · \"")).and_then(|(_, r)| r.split_once("\", HTTP")).map(|(t, _)| t.to_string());
    title.unwrap_or_else(|| {
        let bare = url.split_once("://").map_or(url, |(_, r)| r);
        let bare = bare.split(['?', '#']).next().unwrap_or(bare).trim_end_matches('/');
        bare.chars().take(80).collect()
    })
}

/// A closed notice shows this many characters; opened, all of it.
const NOTICE_CHARS: usize = 160;

/// A closed call's summary is cut to this many characters (about two rows); opened, it shows whole.
const SUMMARY_CHARS: usize = 160;

// ---------------------------------------------------------------- transcript

#[derive(Clone, Copy, PartialEq)]
pub enum PromptStyle {
    Bar,
    Quote,
}

/// How a design wants the shared transcript drawn.
pub struct Opts {
    /// Glyph before each agent reply ("" for none).
    pub marker: &'static str,
    pub prompt: PromptStyle,
    /// Collapse runs of finished tool calls into one line (unless expanded).
    pub group_tools: bool,
    /// Time gutter, nodes and turn dividers.
    pub timeline: bool,
    /// Show how long each tool took.
    pub durations: bool,
}

type Prefixed = (Line<'static>, Vec<Span<'static>>);

const PROMPT_LINES: usize = 8;
const TOOL_LINES: usize = 10;
const GUTTER: usize = 8;

/// Builds the wrapped transcript; also returns how many rows the newest block takes.
pub fn transcript(s: &Session, v: &View, th: &Theme, o: &Opts, width: usize) -> (Vec<Line<'static>>, usize, Vec<Option<usize>>) {
    let mut lines: Vec<Line<'static>> = Vec::new();
    // the block each line belongs to (None: the gap between blocks)
    let mut owners: Vec<Option<usize>> = Vec::new();
    let mut last_rows = 0;
    let mut i = 0;
    let mut prev_turn = 0;
    while i < s.blocks.len() {
        let b = &s.blocks[i];
        let meta = s.meta[i];
        // a run of tool calls collapses into one line in grouped designs
        let foldable = |b: &Block| matches!(b, Block::Tool(_) | Block::Thinking { .. });
        let mut group_end = i + 1;
        // an opened block stays out of a folded run
        if o.group_tools && !v.expand_all && foldable(b) && !v.open.contains(&i) {
            while group_end < s.blocks.len() && foldable(&s.blocks[group_end]) && !v.open.contains(&group_end) {
                group_end += 1;
            }
        }
        let grouped = o.group_tools && !v.expand_all && foldable(b) && !v.open.contains(&i);
        let open = v.expand_all || v.open.contains(&i);
        let after_tool = i > 0 && matches!((&s.blocks[i - 1], b), (Block::Tool(_), Block::Tool(_)));
        if o.timeline && meta.turn != prev_turn && meta.turn > 0 {
            prev_turn = meta.turn;
            lines.push(turn_divider(s, th, meta.turn));
            owners.push(None);
        } else if i > 0 && !after_tool {
            // the timeline keeps its rail unbroken between steps
            lines.push(if o.timeline { Line::from(vec![Span::raw("      "), Span::styled("│", st(th.border))]) } else { Line::default() });
            owners.push(None);
        }
        let before = lines.len();
        let gutter_w = if o.timeline { GUTTER } else { 0 };
        let body = if grouped { group_lines(&s.blocks[i..group_end], s, v, th) } else { block_lines(b, s, v, th, o, open, width.saturating_sub(gutter_w)) };
        for (k, (mut l, mut cont)) in body.into_iter().enumerate() {
            if o.timeline {
                let lead = if k == 0 {
                    // the node replaces the step's own status glyph
                    if matches!(b, Block::Tool(_) | Block::Thinking { .. } | Block::Verify(_)) && !l.spans.is_empty() {
                        let glyph_w = l.spans.remove(0).width();
                        // continuation rows lose the same width so they stay aligned
                        if let Some(first) = cont.first_mut() {
                            let pad = first.content.width().saturating_sub(glyph_w);
                            *first = Span::raw(" ".repeat(pad));
                        }
                    }
                    let (node, c) = node(b, v, th);
                    vec![Span::styled(format!("{} ", clock_str(meta.at_ms)), st(th.faint)), Span::styled(node, st(c)), Span::raw(" ")]
                } else {
                    vec![Span::raw("      "), Span::styled("│", st(th.border)), Span::raw(" ")]
                };
                l.spans.splice(0..0, lead);
                cont.splice(0..0, [Span::raw("      "), Span::styled("│", st(th.border)), Span::raw(" ")]);
            }
            lines.extend(wrap(l, width.max(gutter_w + 10), &cont));
        }
        // the block under the mouse is shaded: a click opens or closes it
        if v.hover.is_some_and(|h| (i..group_end).contains(&h)) {
            for l in &mut lines[before..] {
                l.style = l.style.bg(th.surface);
            }
        }
        owners.resize(lines.len(), Some(i));
        last_rows = lines.len() - before;
        i = group_end;
    }
    let footer = background_lines(s, v, th, width);
    if !footer.is_empty() {
        last_rows = 0; // the newest block is no longer the bottom rows: no effect on them
        owners.resize(lines.len() + footer.len(), None);
        lines.extend(footer);
    }
    (lines, last_rows, owners)
}

/// Under the transcript while background tasks run: a slowly pulsing line (the screen otherwise looks still while the
/// agent waits on them), then one row per task with what Claude Code shows for it: its time, and for a subagent its
/// tokens, tool calls and current step, as they come.
fn background_lines(s: &Session, v: &View, th: &Theme, width: usize) -> Vec<Line<'static>> {
    let n = v.running.len();
    if n == 0 {
        return vec![];
    }
    let pulse = lerp(th.dim, th.accent, 0.5 + 0.5 * (v.clock_ms / 1600.0 * std::f64::consts::TAU).sin());
    let waiting = matches!(s.activity, Activity::Ready | Activity::Done);
    let head = format!("{} {}{n} background task{}", spinner(v.clock_ms), if waiting { "waiting on " } else { "" }, if n == 1 { "" } else { "s" });
    let mut out = vec![Line::default(), Line::from(vec![Span::styled(head, st(pulse).add_modifier(Modifier::ITALIC)), Span::styled("  ctrl-b: all of them", st(th.faint))])];
    let cut = |s: String| if s.width() > width { format!("{}…", s.chars().take(width.saturating_sub(1)).collect::<String>()) } else { s };
    for t in v.running.iter().take(4) {
        let who = if !t.agent.is_empty() { t.agent.clone() } else { t.kind.trim_start_matches("local_").to_string() };
        let time = if t.secs >= 60 { format!("{}m {:02}s", t.secs / 60, t.secs % 60) } else { format!("{}s", t.secs) };
        let mut facts = vec![time];
        if t.tokens > 0 {
            facts.push(format!("{} tokens", human(t.tokens)));
        }
        if t.tools > 0 {
            facts.push(format!("{} tool call{}", t.tools, if t.tools == 1 { "" } else { "s" }));
        }
        out.push(Line::from(vec![Span::styled(cut(format!("  {who} · {} · {}", t.description, facts.join(" · "))), st(th.dim))]));
        if !t.doing.is_empty() {
            out.push(Line::from(Span::styled(cut(format!("    ⎿ {}", t.doing)), st(th.faint))));
        }
    }
    if n > 4 {
        out.push(Line::from(Span::styled(format!("  +{} more", n - 4), st(th.faint))));
    }
    out
}

/// A colour between `a` (t = 0) and `b` (t = 1); `b` itself when either is not RGB.
fn lerp(a: Color, b: Color, t: f64) -> Color {
    match (a, b) {
        (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) => {
            let m = |x: u8, y: u8| (x as f64 + (y as f64 - x as f64) * t.clamp(0.0, 1.0)).round() as u8;
            Color::Rgb(m(r1, r2), m(g1, g2), m(b1, b2))
        }
        _ => b,
    }
}

fn node(b: &Block, v: &View, th: &Theme) -> (&'static str, Color) {
    match b {
        Block::Prompt(_) => ("◇", th.accent),
        Block::Text { .. } => ("●", th.accent),
        Block::Thinking { live: true, .. } => (spinner(v.clock_ms), th.warn),
        Block::Thinking { .. } => ("◌", th.warn),
        Block::Tool(t) if t.result.is_none() => (spinner(v.clock_ms), th.warn),
        Block::Tool(t) if t.error => ("✗", th.err),
        Block::Tool(_) => ("◆", th.accent2),
        Block::Done(_) => ("★", th.ok),
        Block::Verify(x) if x.ok.is_none() => (spinner(v.clock_ms), th.accent2),
        Block::Verify(x) if x.ok == Some(true) => ("✓", th.ok),
        Block::Verify(_) => ("✗", th.err),
        Block::Notice { error: true, .. } => ("!", th.err),
        Block::Notice { .. } => ("·", th.warn),
    }
}

fn turn_divider(s: &Session, th: &Theme, turn: u64) -> Line<'static> {
    let mut spans = vec![Span::raw("      "), Span::styled("├─ ", st(th.border)), Span::styled(format!("turn {turn}"), st(th.dim).add_modifier(Modifier::BOLD))];
    if let Some(u) = s.turn_usage.get(turn as usize - 1).filter(|u| u.total_in() > 0) {
        spans.push(Span::styled(
            format!("  ↑ {}  ↓ {}  ≈{}", human(u.total_in()), human(u.output), usd(s.estimate(u))),
            st(th.faint),
        ));
    }
    Line::from(spans)
}

/// Draws the transcript tail (or the scrolled position); returns the newest block's rows when following.
/// Draws the transcript's lines that fit, `scroll` rows up from the end. Returns the newest block's rows (when at the
/// bottom) and the block on each screen row.
/// Also returns the transcript line on the area's first row (a mouse selection holds lines, not rows).
pub fn render_transcript(f: &mut Frame, area: Rect, (lines, last_rows, owners): (Vec<Line<'static>>, usize, Vec<Option<usize>>), scroll: usize) -> (Option<Rect>, Vec<Option<usize>>, usize) {
    let h = area.height as usize;
    let max_scroll = lines.len().saturating_sub(h);
    let start = max_scroll.saturating_sub(scroll.min(max_scroll));
    let visible: Vec<Line> = lines.into_iter().skip(start).take(h).collect();
    let shown = visible.len();
    f.render_widget(Paragraph::new(visible), area);
    let rows = owners.into_iter().skip(start).take(h).collect();
    let last = (scroll == 0 && last_rows > 0 && shown > 0).then(|| {
        let rows = last_rows.min(shown) as u16;
        Rect { y: area.y + shown as u16 - rows, height: rows, ..area }
    });
    (last, rows, start)
}

/// The whole transcript as the current design draws it at `width`, for copying a selection that runs off screen.
pub fn transcript_lines(s: &Session, v: &View, width: u16) -> Vec<Line<'static>> {
    let th = v.design.theme();
    let o = match v.design {
        Design::Cockpit => &cockpit::OPTS,
        Design::Aurora => &aurora::OPTS,
        Design::Zen => &zen::OPTS,
        Design::Timeline => &timeline::OPTS,
    };
    transcript(s, v, th, o, width as usize).0
}

fn block_lines(b: &Block, s: &Session, v: &View, th: &Theme, o: &Opts, open: bool, width: usize) -> Vec<Prefixed> {
    match b {
        Block::Prompt(p) => prompt_lines(p, first_prompt(s, b), th, o, open, width),
        Block::Verify(x) => verify_lines(x, v, th),
        // a long notice (a provider's error) shows its start until clicked, then all of it
        Block::Notice { text, error } => vec![(
            Line::from(vec![
                Span::styled(if *error { "! " } else { "· " }, st(if *error { th.err } else { th.warn })),
                Span::styled(if open || text.chars().count() <= NOTICE_CHARS { text.clone() } else {
                    format!("{}… (+{} chars · click to show)", text.chars().take(NOTICE_CHARS).collect::<String>(), text.chars().count() - NOTICE_CHARS)
                }, st(if *error { th.err } else { th.dim }).add_modifier(Modifier::ITALIC)),
            ]),
            vec![Span::raw("  ")],
        )],
        Block::Text { text, live } => {
            let mut out = markdown(text, th, width.saturating_sub(if o.marker.is_empty() { 0 } else { 2 }));
            if out.is_empty() {
                out.push((Line::default(), vec![]));
            }
            if !o.marker.is_empty() {
                for (i, (l, cont)) in out.iter_mut().enumerate() {
                    l.spans.insert(0, if i == 0 { Span::styled(format!("{} ", o.marker), st(th.accent)) } else { Span::raw("  ") });
                    cont.insert(0, Span::raw("  "));
                }
            }
            if *live {
                if let Some((l, _)) = out.last_mut() {
                    l.spans.push(Span::styled("▍", st(th.accent)));
                }
            }
            out
        }
        Block::Thinking { tokens, live, exact } => {
            let glyph = if *live { spinner(v.clock_ms) } else { "✻" };
            let mut spans = vec![Span::styled(format!("{glyph} "), st(th.warn)), Span::styled("thinking", st(th.dim).add_modifier(Modifier::ITALIC))];
            // claude reports thinking in steps of 50-200 tokens about once a second: while live, the count runs up to
            // each new total over the second after it, as Claude Code's does
            let shown = if *live { s.thinking_shown(*tokens, v.clock_ms) } else { *tokens };
            if shown > 0 {
                spans.push(Span::styled(format!(" · {}{shown} tok", if *exact { "" } else { "~" }), st(th.faint)));
            }
            vec![(Line::from(spans), vec![])]
        }
        Block::Tool(t) => tool_lines(t, s, v, th, o, open, width),
        Block::Done(d) => {
            let (glyph, word, color) = match (d.stopped, d.ok) {
                (true, _) => ("■", "Stopped", th.warn),
                (false, true) => ("✔", "Done", th.ok),
                (false, false) => ("✗", "Failed", th.err),
            };
            let u = &d.usage;
            // the repo's changes when there are any (git); else the files an edit tool wrote: a path guessed from a command
            // line counted grep patterns and /tmp scripts as files (2026-11-04)
            let files = if v.changed.is_empty() { files_touched(s).len() } else { v.changed.len() };
            // the session in one row: sent ↑, calls, the cache's share and size, output ↓, files, and how fast
            let (ttft, tps) = crate::session::speed(&s.calls);
            let mut facts = vec![format!("↑ {}", human(u.total_in())), format!("{} call{}", s.turns, if s.turns == 1 { "" } else { "s" }),
                format!("cache {:.0}% ({})", u.cache_hit() * 100.0, human(u.cache_read)), format!("↓ {}", human(u.output)), files_n(files)];
            if let Some(f) = ttft {
                facts.push(format!("{:.1}s to first token", f / 1000.0));
            }
            if let Some(r) = tps {
                facts.push(format!("{r:.0} tok/s"));
            }
            vec![
                (
                    Line::from(vec![
                        Span::styled(format!("{glyph} {word}"), st(color).add_modifier(Modifier::BOLD)),
                        Span::styled(
                            format!("   {}m {:02}s · {} turn{} · {}", d.secs as u64 / 60, d.secs as u64 % 60, d.turns, if d.turns == 1 { "" } else { "s" }, usd(d.cost)),
                            st(th.text),
                        ),
                    ]),
                    vec![],
                ),
                (
                    Line::from(Span::styled(
                        format!("  {}", facts.join(" · ")),
                        st(th.dim),
                    )),
                    vec![],
                ),
            ]
        }
    }
}

const VERIFY_FAIL_LINES: usize = 8;

fn verify_lines(x: &crate::session::Verify, v: &View, th: &Theme) -> Vec<Prefixed> {
    let (glyph, color, state) = match x.ok {
        _ if x.no_tests => ("·", th.faint, "no tests found".to_string()),
        _ if x.timed_out => ("·", th.faint, format!("timed out after {:.0}s · not verified", x.secs)),
        None => (spinner(v.clock_ms), th.accent2, "running…".to_string()),
        Some(true) => ("✓", th.ok, format!("passed · {:.1}s", x.secs)),
        Some(false) => ("✗", th.err, format!("failed · {:.1}s", x.secs)),
    };
    let round = if x.round > 1 { format!(" (round {})", x.round) } else { String::new() };
    let mut out = vec![(
        Line::from(vec![
            Span::styled(format!("{glyph} "), st(color)),
            Span::styled("verify", st(th.accent2).add_modifier(Modifier::BOLD)),
            Span::styled(format!(" {}{round}", x.cmd), st(th.dim)),
            Span::styled(format!("  · {state}"), st(th.faint)),
        ]),
        vec![Span::raw("         ")],
    )];
    if x.ok == Some(false) {
        let lines: Vec<&str> = x.output.lines().filter(|l| !l.trim().is_empty()).collect();
        let shown = if v.expand_all { lines.len() } else { VERIFY_FAIL_LINES.min(lines.len()) };
        for l in &lines[lines.len() - shown..] {
            out.push((Line::from(vec![Span::raw("    "), Span::styled("│ ", st(th.faint)), Span::styled(l.replace('\t', "    "), st(th.err))]), vec![Span::raw("      ")]));
        }
    }
    out
}

/// The opening message is the task; later ones are follow-ups.
fn first_prompt(s: &Session, b: &Block) -> bool {
    s.blocks.iter().find(|x| matches!(x, Block::Prompt(_))).is_some_and(|x| std::ptr::eq(x, b))
}

/// A long prompt shows its first PROMPT_LINES lines until it is opened (a click on it, or ctrl+o for every block).
fn prompt_lines(p: &str, first: bool, th: &Theme, o: &Opts, open: bool, width: usize) -> Vec<Prefixed> {
    let md = markdown(p, th, width.saturating_sub(4));
    let keep = if open { md.len() } else { PROMPT_LINES };
    let more = md.len().saturating_sub(keep);
    let mut out: Vec<Prefixed> = Vec::new();
    match o.prompt {
        PromptStyle::Bar => {
            let bar = || Span::styled("▎ ", st(th.accent));
            let label = if first { "task" } else { "you" };
            out.push((Line::from(vec![bar(), Span::styled(label, st(th.accent).add_modifier(Modifier::BOLD))]), vec![bar()]));
            for (mut l, mut cont) in md.into_iter().take(keep) {
                l.spans.insert(0, bar());
                cont.insert(0, bar());
                out.push((l, cont));
            }
            if more > 0 {
                out.push((Line::from(vec![bar(), Span::styled(format!("… {more} more lines · click to show"), st(th.faint))]), vec![]));
            }
        }
        PromptStyle::Quote => {
            // quiet: the task in italics, dimmed, no chrome
            for (l, cont) in md.into_iter().take(keep) {
                let spans = l.spans.into_iter().map(|sp| Span::styled(sp.content, st(th.dim).add_modifier(Modifier::ITALIC))).collect::<Vec<_>>();
                out.push((Line::from(spans), cont));
            }
            if more > 0 {
                out.push((Line::from(Span::styled(format!("… {more} more lines · click to show"), st(th.faint))), vec![]));
            }
        }
    }
    out
}

fn tool_lines(t: &Tool, s: &Session, v: &View, th: &Theme, o: &Opts, open: bool, width: usize) -> Vec<Prefixed> {
    let arg = |k: &str| tool_arg(t, k);
    let name = short_name(&t.name);
    let (glyph, gcolor) = match (&t.result, t.error) {
        (None, _) => (spinner(v.clock_ms), th.warn),
        (Some(_), false) => ("✓", th.ok),
        (Some(_), true) => ("✗", th.err),
    };
    let mut meta = match (&t.result, name) {
        (Some(_), _) if t.error => "error".to_string(),
        (Some(_), "edit") => format!("-{} +{}", arg("old").lines().count(), arg("new").lines().count()),
        (Some(_), "write") => lines_n(arg("content").lines().count()),
        (Some(r), _) => lines_n(r.lines().count()),
        (None, _) => String::new(),
    };
    if let Some((n, last)) = &t.sub {
        // a running subagent: its progress, not its whole transcript
        let last: String = last.chars().take(60).collect();
        meta = if meta.is_empty() { format!("{n} steps · {last}") } else { format!("{meta} · {n} steps") };
    }
    if let (true, Some(secs)) = (o.durations, t.secs()) {
        meta.push_str(&format!(" · {secs:.1}s"));
    }
    let gutter = || vec![Span::raw("    "), Span::styled("│ ", st(th.faint))];
    // closed, the call is one short summary (a subagent's whole prompt ran 25 rows, 2026-11-02); opened (a click, or
    // ctrl-o for all), its full input and output
    let summary = tool_summary(t, s);
    let cut = !open && summary.chars().count() > SUMMARY_CHARS;
    let summary = if cut { format!("{}…", summary.chars().take(SUMMARY_CHARS).collect::<String>()) } else { summary };
    let prompt = if matches!(name, "Agent" | "Task") { t.input["prompt"].as_str().unwrap_or_default() } else { "" };
    let more = cut || !prompt.is_empty() || t.result.as_deref().is_some_and(|r| !r.trim().is_empty());
    if more && !matches!(name, "edit" | "TodoWrite") {
        meta.push_str(if open { "  ▾" } else { "  ▸" });
    }
    // the counts and time on the same row when it all fits, else on a row of their own under the summary (wrapped
    // straight after a long command they were hard to find)
    let meta = if meta.is_empty() { String::new() } else { format!("  · {meta}") };
    let fits = 2 + name.width().max(6) + 1 + summary.width() + meta.width() <= width;
    let mut head = vec![
        Span::styled(format!("{glyph} "), st(gcolor)),
        Span::styled(format!("{name:<6}"), st(th.accent2).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {summary}"), st(th.dim)),
    ];
    if fits {
        head.push(Span::styled(meta.clone(), st(th.faint)));
    }
    let mut out = vec![(Line::from(head), vec![Span::raw("         ")])];
    if !fits && !meta.is_empty() {
        out.push((Line::from(vec![Span::raw("         "), Span::styled(meta.trim_start().to_string(), st(th.faint))]), vec![Span::raw("         ")]));
    }
    let body: Vec<(String, Color)> = match name {
        // edits always show their diff: it is the most useful thing on screen
        "edit" => arg("old").lines().map(|l| (format!("- {l}"), th.err)).chain(arg("new").lines().map(|l| (format!("+ {l}"), th.ok))).collect(),
        "write" if open => arg("content").lines().map(|l| (format!("+ {l}"), th.ok)).collect(),
        // an opened subagent: its prompt, then its answer
        "Agent" | "Task" if open => prompt.lines().map(|l| (l.to_string(), th.text))
            .chain(t.result.as_deref().unwrap_or_default().lines().map(|l| (l.to_string(), th.dim))).collect(),
        // the to-do list as a checklist, always: it is short and it is the plan
        "TodoWrite" => t.input["todos"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|x| {
                let (mark, c) = match x["status"].as_str() {
                    Some("completed") => ("☑", th.ok),
                    Some("in_progress") => ("◐", th.warn),
                    _ => ("☐", th.dim),
                };
                (format!("{mark} {}", x["content"].as_str().unwrap_or_default()), c)
            })
            .collect(),
        _ if open => t.result.as_deref().unwrap_or_default().lines().map(|l| (l.to_string(), if t.error { th.err } else { th.dim })).collect(),
        _ => vec![],
    };
    // a click opens one call whole; ctrl-o opens all, each up to a screenful
    let limit = if name == "edit" && !open { TOOL_LINES } else if name == "TodoWrite" { 30 } else if v.open.is_empty() || v.expand_all { TOOL_LINES * 2 } else { 400 };
    for (l, c) in body.iter().take(limit) {
        let mut spans = gutter();
        spans.push(Span::styled(l.replace('\t', "    "), st(*c)));
        out.push((Line::from(spans), gutter()));
    }
    if body.len() > limit {
        let mut spans = gutter();
        spans.push(Span::styled(format!("… +{}", lines_n(body.len() - limit)), st(th.faint)));
        out.push((Line::from(spans), gutter()));
    }
    out
}

/// "1 line", "2 lines".
fn lines_n(n: usize) -> String {
    if n == 1 { "1 line".into() } else { format!("{n} lines") }
}

/// One quiet line for a run of tool calls: "⌁ ran 4 commands · bash ×3 read ×1 · 210 lines".
fn group_lines(blocks: &[Block], s: &Session, v: &View, th: &Theme) -> Vec<Prefixed> {
    let tools: Vec<&Tool> = blocks.iter().filter_map(|b| if let Block::Tool(t) = b { Some(t) } else { None }).collect();
    let thought: u64 = blocks.iter().map(|b| if let Block::Thinking { tokens, .. } = b { *tokens } else { 0 }).sum();
    let approx = if blocks.iter().any(|b| matches!(b, Block::Thinking { exact: false, .. })) { "~" } else { "" };
    let thinking_now = matches!(blocks.last(), Some(Block::Thinking { live: true, .. }));
    let running = tools.iter().find(|t| t.result.is_none());
    let mut counts: Vec<(&str, usize)> = Vec::new();
    for t in &tools {
        let n = short_name(&t.name);
        match counts.iter_mut().find(|(k, _)| *k == n) {
            Some((_, c)) => *c += 1,
            None => counts.push((n, 1)),
        }
    }
    let lines: usize = tools.iter().filter_map(|t| t.result.as_ref()).map(|r| r.lines().count()).sum();
    let errors = tools.iter().filter(|t| t.error).count();
    let mix = counts.iter().map(|(n, c)| format!("{n} ×{c}")).collect::<Vec<_>>().join("  ");
    let busy = running.is_some() || thinking_now;
    let what = match (tools.len(), thinking_now) {
        (0, _) => "thinking".to_string(),
        (n, true) => format!("ran {n} tools, thinking"),
        (1, false) => "ran 1 tool".to_string(),
        (n, false) => format!("ran {n} tools"),
    };
    let mut spans = vec![
        Span::styled(if busy { format!("{} ", spinner(v.clock_ms)) } else { "⌁ ".into() }, st(th.accent2)),
        Span::styled(what, st(th.dim)),
        Span::styled(if tools.is_empty() { String::new() } else { format!("   {mix}   {}", lines_n(lines)) }, st(th.faint)),
    ];
    if thought > 0 {
        spans.push(Span::styled(format!("   thought {approx}{thought} tok"), st(th.faint)));
    }
    if errors > 0 {
        spans.push(Span::styled(format!("   {errors} failed"), st(th.err)));
    }
    let mut out = vec![(Line::from(spans), vec![Span::raw("  ")])];
    if let Some(t) = running {
        out.push((Line::from(vec![Span::raw("  "), Span::styled(tool_summary(t, s), st(th.faint).add_modifier(Modifier::ITALIC))]), vec![Span::raw("  ")]));
    }
    out
}

/// Just enough markdown for agent replies: headings, bullets, code fences, `code`, **bold**.
fn markdown(text: &str, th: &Theme, width: usize) -> Vec<Prefixed> {
    let mut out = Vec::new();
    let mut in_code = false;
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let raw = lines[i];
        i += 1;
        // a table: `| a | b |` rows, the second a `|---|---|` rule
        if !in_code && raw.trim_start().starts_with('|') && lines.get(i).is_some_and(|n| is_rule(n)) {
            let mut rows = vec![cells(raw)];
            i += 1;
            while i < lines.len() && lines[i].trim_start().starts_with('|') {
                rows.push(cells(lines[i]));
                i += 1;
            }
            out.extend(table(&rows, th, width));
            continue;
        }
        if raw.trim_start().starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            out.push((Line::from(Span::styled(format!(" {raw} "), Style::new().fg(th.code).bg(th.surface))), vec![]));
            continue;
        }
        let t = raw.trim_start();
        if let Some(h) = t.strip_prefix('#') {
            out.push((Line::from(Span::styled(h.trim_start_matches('#').trim().to_string(), st(th.accent).add_modifier(Modifier::BOLD))), vec![]));
        } else if let Some((num, item)) = t.split_once(". ").filter(|(n, _)| (1..=3).contains(&n.len()) && n.bytes().all(|b| b.is_ascii_digit())) {
            // numbered item: wrapped rows hang under the text, not the number
            let indent = " ".repeat(raw.len() - t.len());
            let mut spans = vec![Span::raw(indent.clone()), Span::styled(format!("{num}. "), st(th.accent))];
            spans.extend(inline(item, th));
            out.push((Line::from(spans), vec![Span::raw(format!("{indent}{}", " ".repeat(num.len() + 2)))]));
        } else if let Some(item) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
            let indent = " ".repeat(raw.len() - t.len());
            let mut spans = vec![Span::raw(indent.clone()), Span::styled("• ", st(th.accent))];
            spans.extend(inline(item, th));
            out.push((Line::from(spans), vec![Span::raw(format!("{indent}  "))]));
        } else {
            out.push((Line::from(inline(raw, th)), vec![]));
        }
    }
    out
}

fn inline(s: &str, th: &Theme) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for (i, part) in s.split('`').enumerate() {
        if i % 2 == 1 {
            spans.push(Span::styled(part.to_string(), Style::new().fg(th.accent2).bg(th.surface)));
            continue;
        }
        for (j, seg) in part.split("**").enumerate() {
            let style = if j % 2 == 1 { st(th.text).add_modifier(Modifier::BOLD) } else { st(th.text) };
            for (text, url) in links(seg) {
                // a link shows its title only, underlined; the address is attached where it is drawn (`link_cells`)
                spans.push(Span::styled(text, if url.is_some() { style.fg(th.accent).add_modifier(Modifier::UNDERLINED) } else { style }));
            }
        }
    }
    spans.retain(|s| !s.content.is_empty());
    spans
}

/// Text split into plain runs and links: `[title](https://...)` as its title, a bare http(s) address as itself.
fn links(s: &str) -> Vec<(String, Option<String>)> {
    let mut out: Vec<(String, Option<String>)> = vec![];
    let mut text = String::new();
    let mut rest = s;
    let flush = |text: &mut String, out: &mut Vec<(String, Option<String>)>| {
        // bare addresses inside plain text
        let mut t = text.as_str();
        while let Some(at) = t.find("https://").or_else(|| t.find("http://")) {
            let end = t[at..].find(|c: char| c.is_whitespace()).map_or(t.len(), |e| at + e);
            let url = t[at..end].trim_end_matches(['.', ',', ';', ':', ')', '!', '?']);
            if !safe_url(url) {
                break;
            }
            out.push((t[..at].to_string(), None));
            out.push((url.to_string(), Some(url.to_string())));
            t = &t[at + url.len()..];
        }
        out.push((t.to_string(), None));
        text.clear();
    };
    while let Some(open) = rest.find('[') {
        let link = rest[open..].find("](").map(|c| open + c).and_then(|close| {
            let end = close + 2 + rest[close + 2..].find(')')?;
            let (title, url) = (&rest[open + 1..close], &rest[close + 2..end]);
            (!title.is_empty() && !title.contains('[') && safe_url(url)).then(|| (title.to_string(), url.to_string(), end))
        });
        match link {
            Some((title, url, end)) => {
                text.push_str(&rest[..open]);
                flush(&mut text, &mut out);
                out.push((title, Some(url)));
                rest = &rest[end + 1..];
            }
            None => {
                text.push_str(&rest[..open + 1]);
                rest = &rest[open + 1..];
            }
        }
    }
    text.push_str(rest);
    flush(&mut text, &mut out);
    out
}

/// An address safe to hand to a terminal or a browser: http(s), printable ASCII, no spaces or control characters (it
/// goes into an escape sequence, so nothing in it may end one).
pub fn safe_url(u: &str) -> bool {
    (u.starts_with("https://") || u.starts_with("http://")) && u.len() > 8 && u.len() < 2048 && u.bytes().all(|b| b.is_ascii_graphic())
}

/// Every link in the agent's replies: (what is shown, where it goes).
fn links_in(s: &Session) -> Vec<(String, String)> {
    s.blocks.iter().filter_map(|b| if let Block::Text { text, .. } = b { Some(reply_links(text)) } else { None }).flatten().collect()
}

/// A reply's links as drawn: bold marks out first, as `inline` takes them out before it finds links. A bare address in
/// **bold** carried the closing marks into its address ("…/pull/1437**": a 404 from cmd-click and Copy Link, 2026-11-09).
fn reply_links(text: &str) -> Vec<(String, String)> {
    links(&text.replace("**", " ")).into_iter().filter_map(|(text, url)| url.map(|u| (text, u))).collect()
}

/// Makes the drawn links real terminal hyperlinks (OSC 8: cmd-click opens them where the terminal supports it) and
/// returns where each one is, for a click to open it. A link is a run of underlined cells; its address is that of the
/// first link whose title contains the run (a title wrapped over two rows is two runs, both found).
fn link_cells(buf: &mut ratatui::buffer::Buffer, area: Rect, links: &[(String, String)]) -> Vec<(Rect, String)> {
    use ratatui::buffer::CellDiffOption;
    let mut found = vec![];
    if links.is_empty() {
        return found;
    }
    for y in area.top()..area.bottom() {
        let mut x = area.left();
        while x < area.right() {
            if !buf[(x, y)].modifier.contains(Modifier::UNDERLINED) {
                x += 1;
                continue;
            }
            let start = x;
            while x < area.right() && buf[(x, y)].modifier.contains(Modifier::UNDERLINED) {
                x += 1;
            }
            let run: String = (start..x).map(|c| buf[(c, y)].symbol().to_string()).collect();
            let Some((_, url)) = links.iter().find(|(t, _)| !run.trim().is_empty() && t.contains(run.trim())) else { continue };
            // each cell opens and closes the link itself: ratatui writes only the cells that changed, and an opening on
            // the first cell without the closing on the last left everything drawn after it a link, underlined by the
            // terminal (2026-11-11). The id joins the cells into one link for the terminal's hover
            let id = url.bytes().fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32));
            let mut c = start;
            while c < x {
                let sym = buf[(c, y)].symbol().to_string();
                let w = std::num::NonZeroU16::new(sym.width().max(1) as u16).expect("at least 1");
                buf[(c, y)].set_symbol(&format!("\x1b]8;id={id:x};{url}\x1b\\{sym}\x1b]8;;\x1b\\")).set_diff_option(CellDiffOption::ForcedWidth(w));
                c += w.get(); // a wide character's second cell stays as ratatui left it
            }
            found.push((Rect::new(start, y, x - start, 1), url.clone()));
        }
    }
    found
}

/// A table row's cells: `| a | b |` -> ["a", "b"].
fn cells(row: &str) -> Vec<String> {
    let r = row.trim();
    let r = r.strip_prefix('|').unwrap_or(r);
    let r = r.strip_suffix('|').unwrap_or(r);
    r.split('|').map(|c| c.trim().to_string()).collect()
}

/// The `|---|:---:|` line under a table's header.
fn is_rule(line: &str) -> bool {
    let c = cells(line);
    line.trim_start().starts_with('|') && !c.is_empty() && c.iter().all(|x| x.contains('-') && x.chars().all(|ch| matches!(ch, '-' | ':' | ' ')))
}

/// A markdown table drawn with borders, its columns fitted to `width`: a column wider than its share wraps inside its
/// cell (rows then get a rule between them). Too narrow for that, the rows read as "a · b · c". The pipes of the raw
/// markdown wrapped across the screen (2026-11-02).
fn table(rows: &[Vec<String>], th: &Theme, width: usize) -> Vec<Prefixed> {
    let n = rows.iter().map(Vec::len).max().unwrap_or(0);
    let room = width.saturating_sub(3 * n + 1);
    if n == 0 || room < 3 * n {
        return rows.iter().map(|r| (Line::from(inline(&r.join(" · "), th)), vec![])).collect();
    }
    let cell = |r: usize, c: usize| -> Vec<Span<'static>> {
        let spans = inline(rows[r].get(c).map_or("", String::as_str), th);
        if r == 0 { spans.into_iter().map(|s| { let st = s.style.add_modifier(Modifier::BOLD); s.style(st) }).collect() } else { spans }
    };
    let natural: Vec<usize> = (0..n).map(|c| (0..rows.len()).map(|r| cell(r, c).iter().map(|s| s.content.width()).sum::<usize>()).max().unwrap_or(0).max(1)).collect();
    let mut w = natural.clone();
    if natural.iter().sum::<usize>() > room {
        // a short column (12 or less) stays whole, a long one keeps its longest word whole (a path cut mid-word reads
        // badly); 8 columns each when even that does not fit. The rest is shared in proportion to what each still wants.
        let word = |c: usize| (0..rows.len()).flat_map(|r| cell(r, c)).flat_map(|s| s.content.split(' ').map(|x| x.width()).collect::<Vec<_>>()).max().unwrap_or(1);
        let mut base: Vec<usize> = (0..n).map(|c| if natural[c] <= 12 { natural[c] } else { natural[c].min(word(c)) }).collect();
        if base.iter().sum::<usize>() > room {
            base = natural.iter().map(|&x| x.min(8)).collect();
        }
        let spare = room.saturating_sub(base.iter().sum());
        let want: usize = natural.iter().zip(&base).map(|(a, b)| a - b).sum();
        w = natural.iter().zip(&base).map(|(&a, &b)| b + (a - b) * spare / want.max(1)).collect();
    }
    let border = st(th.border);
    let rule = |l: &str, m: &str, r: &str| (Line::from(Span::styled(format!("{l}{}{r}", w.iter().map(|x| "─".repeat(x + 2)).collect::<Vec<_>>().join(m)), border)), vec![]);
    let mut body: Vec<Vec<Line<'static>>> = vec![];
    for r in 0..rows.len() {
        let cols: Vec<Vec<Line<'static>>> = (0..n).map(|c| {
            let mut ls = wrap(Line::from(cell(r, c)), w[c], &[]);
            for l in &mut ls {
                if let Some(last) = l.spans.last_mut() {
                    last.content = last.content.trim_end().to_string().into();
                }
            }
            ls
        }).collect();
        let h = cols.iter().map(Vec::len).max().unwrap_or(1);
        body.push((0..h).map(|k| {
            let mut sp = vec![Span::styled("│ ", border)];
            for (c, col) in cols.iter().enumerate() {
                let l = col.get(k).cloned().unwrap_or_default();
                let lw: usize = l.spans.iter().map(|s| s.content.width()).sum();
                sp.extend(l.spans);
                sp.push(Span::raw(" ".repeat(w[c].saturating_sub(lw))));
                sp.push(Span::styled(if c + 1 < n { " │ " } else { " │" }, border));
            }
            Line::from(sp)
        }).collect());
    }
    let tall = body.iter().skip(1).any(|r| r.len() > 1);
    let mut out = vec![rule("┌", "┬", "┐")];
    for (r, lines) in body.into_iter().enumerate() {
        if r == 1 || (r > 1 && tall) {
            out.push(rule("├", "┼", "┤"));
        }
        out.extend(lines.into_iter().map(|l| (l, vec![])));
    }
    out.push(rule("└", "┴", "┘"));
    out
}

/// Word-wraps a styled line to `width`, starting continuation rows with `cont`.
/// Every transcript row passes through here, so this is where text from outside (model replies, tool output, notices with
/// file names or errors) is made inert: escape sequences dropped, other control characters shown as `�` (`plain`).
pub fn wrap(mut line: Line<'static>, width: usize, cont: &[Span<'static>]) -> Vec<Line<'static>> {
    for span in &mut line.spans {
        if span.content.chars().any(|c| c.is_control() || crate::session::invisible(c)) {
            span.content = plain(&span.content).replace('\t', "    ").replace('\n', " ").into();
        }
    }
    let cont_w: usize = cont.iter().map(|s| s.content.width()).sum();
    let mut out = Vec::new();
    // `start` is the width of the current row's prefix (0 on the first row, `cont_w` after a wrap)
    let (mut cur, mut w, mut start): (Vec<Span<'static>>, usize, usize) = (Vec::new(), 0, 0);
    let flush = |cur: &mut Vec<Span<'static>>, w: &mut usize, start: &mut usize, out: &mut Vec<Line<'static>>| {
        out.push(Line::from(std::mem::take(cur)).style(line.style));
        cur.extend_from_slice(cont);
        *w = cont_w;
        *start = cont_w;
    };
    for span in line.spans.clone() {
        for word in span.content.split_inclusive(' ') {
            let ww = word.trim_end().width();
            if w + ww > width && w > start {
                flush(&mut cur, &mut w, &mut start, &mut out);
            }
            if ww > width.saturating_sub(w) {
                // longer than a whole row: split by character
                let mut buf = String::new();
                for ch in word.chars() {
                    let cw = ch.width().unwrap_or(0);
                    if w + cw > width && w > start {
                        cur.push(Span::styled(std::mem::take(&mut buf), span.style));
                        flush(&mut cur, &mut w, &mut start, &mut out);
                    }
                    buf.push(ch);
                    w += cw;
                }
                cur.push(Span::styled(buf, span.style));
            } else {
                cur.push(Span::styled(word.to_string(), span.style));
                w += word.width();
            }
        }
    }
    out.push(Line::from(cur).style(line.style));
    out
}

const REPLAY_KEYS: &str = "1-4 design · space pause · ←/→ speed · ↑/↓ scroll · e expand · q quit";
pub const LIVE_KEYS: &str = "enter send · \\⏎ or ctrl-j new line · shift-tab mode · tab complete / @ · !cmd shell · ctrl-v image · ctrl-g editor · ctrl-b tasks · ctrl-x stash · /model · /boss · /mcp · /diff · /new · /resume · /rewind · /loop · /usage · /vim · ↑↓ history · esc stop · drag copies · ctrl-t design · ctrl-o expand";

/// As many whole key hints as fit in `room` columns, most used first; "/keys all" ends them when some are left out.
fn fitted_keys(keys: &str, room: usize) -> String {
    let all: Vec<&str> = keys.split(" · ").collect();
    let mut shown: Vec<&str> = vec![];
    for (i, k) in all.iter().enumerate() {
        let more = if i + 1 < all.len() { " · /keys all".width() } else { 0 };
        if shown.iter().map(|s| s.width() + 3).sum::<usize>() + k.width() + more > room {
            shown.push("/keys all");
            return shown.join(" · ");
        }
        shown.push(k);
    }
    shown.join(" · ")
}

/// The composer row: typed text with a cursor in live mode, a placeholder and key hints otherwise.
pub fn composer_line(v: &View, th: &Theme, glyph: &str, placeholder: &str, width: u16) -> Line<'static> {
    let mut spans = vec![Span::styled(format!(" {glyph} "), st(th.accent).add_modifier(Modifier::BOLD))];
    if v.live && !v.input.is_empty() {
        // one row (zen's footer): the end of what is being typed, newlines as ⏎
        let text = with_cursor(v).replace('\n', " ⏎ ");
        let room = (width as usize).saturating_sub(6);
        let shown: String = text.chars().rev().take(room).collect::<Vec<_>>().into_iter().rev().collect();
        spans.push(Span::styled(shown, st(th.text)));
        if !v.vim.is_empty() {
            spans.push(Span::styled(format!("  -- {} --", v.vim), st(th.accent)));
        }
    } else {
        if v.live {
            spans.push(Span::styled("▍", st(th.accent)));
        }
        spans.push(Span::styled(placeholder.to_string(), st(th.faint)));
        if v.live && v.mode != "auto" {
            spans.push(Span::styled(format!("  [{}]", v.mode), st(th.warn)));
        }
        if !v.vim.is_empty() {
            spans.push(Span::styled(format!("  [{}]", v.vim), st(th.accent)));
        }
        if v.bg_running > 0 {
            spans.push(Span::styled(format!("  [{} background · ctrl-b]", v.bg_running), st(th.accent)));
        }
        if v.cold > 0 {
            spans.push(Span::styled(format!("  [cache cold: the next message re-reads {} tokens]", human(v.cold)), st(th.warn)));
        }
        if let Some((t, _)) = &v.toast {
            spans.push(Span::styled(format!("   {t}"), st(th.ok)));
        } else {
            let used: usize = spans.iter().map(|s| s.content.width()).sum::<usize>() + 3;
            let keys = if v.live { fitted_keys(LIVE_KEYS, (width as usize).saturating_sub(used)) } else { REPLAY_KEYS.to_string() };
            spans.push(Span::styled(format!("   {keys}"), st(th.faint)));
        }
    }
    Line::from(spans)
}

/// Rows the composer box may grow to while typing (multi-line input); the latest rows stay visible.
const COMPOSER_MAX_ROWS: usize = 6;

/// The composer box: typed text on real lines (wrapped to `width`, the box's inside), or the one-line placeholder.
pub fn composer_rows(v: &View, th: &Theme, glyph: &str, placeholder: &str, width: u16) -> Vec<Line<'static>> {
    if let Some(q) = &v.asking {
        let q: String = q.chars().take((width as usize).saturating_sub(40)).collect();
        return vec![Line::from(vec![
            Span::styled(" ? ", st(th.warn).add_modifier(Modifier::BOLD)),
            Span::styled(format!("allow {q}"), st(th.text).add_modifier(Modifier::BOLD)),
            Span::styled("   y yes · a always · n no", st(th.faint)),
        ])];
    }
    if !(v.live && !v.input.is_empty()) {
        return vec![composer_line(v, th, glyph, placeholder, width)];
    }
    let room = (width as usize).saturating_sub(4).max(1);
    let rows = composer_layout(v, room);
    let skip = composer_skip(&rows, cursor_at(v));
    let hints = hint_row(v, th, width);
    let sel = v.select;
    rows.into_iter()
        .enumerate()
        .skip(skip)
        .take(COMPOSER_MAX_ROWS)
        .map(|(i, (cells, _))| {
            let lead = if i == 0 { format!(" {glyph} ") } else { "   ".to_string() };
            let mut spans = vec![Span::styled(lead, st(th.accent).add_modifier(Modifier::BOLD))];
            // runs of one style: the text, the cursor, and a mouse selection shown reversed
            let mut run: Option<(Style, String)> = None;
            for (c, at) in cells {
                let style = match at {
                    None => st(th.accent),
                    Some(b) if sel.is_some_and(|(a, z)| b >= a && b < z) => st(th.text).add_modifier(Modifier::REVERSED),
                    _ => st(th.text),
                };
                match &mut run {
                    Some((s0, t)) if *s0 == style => t.push(c),
                    _ => {
                        if let Some((s0, t)) = run.replace((style, c.to_string())) {
                            spans.push(Span::styled(t, s0));
                        }
                    }
                }
            }
            if let Some((s0, t)) = run {
                spans.push(Span::styled(t, s0));
            }
            // vim mode's state stays in sight while typing
            if i == skip && !v.vim.is_empty() {
                spans.push(Span::styled(format!("  -- {} --", v.vim), st(th.accent)));
            }
            Line::from(spans)
        })
        .chain(hints)
        .collect()
}

/// The typed text as the composer draws it: wrapped rows of (char, its byte index in the input; None for the cursor),
/// each with the index a click past its end lands on. Drawing and clicking share it, so a click lands where it looks.
/// While text is selected the cursor is not drawn: its cell would shift the text under a drag.
fn composer_layout(v: &View, room: usize) -> Vec<(Vec<(char, Option<usize>)>, usize)> {
    let at = cursor_at(v);
    let cursor = v.select.is_none();
    let mut cells: Vec<(char, Option<usize>)> = Vec::with_capacity(v.input.len() + 1);
    for (i, c) in v.input.char_indices() {
        if cursor && i == at {
            cells.push((CURSOR, None));
        }
        cells.push((c, Some(i)));
    }
    if cursor && at == v.input.len() {
        cells.push((CURSOR, None));
    }
    let (mut rows, mut row, mut w) = (vec![], vec![], 0);
    for (c, i) in cells {
        if let (Some(i), '\n') = (i, c) {
            rows.push((std::mem::take(&mut row), i));
            w = 0;
            continue;
        }
        let cw = c.width().unwrap_or(0);
        if w + cw > room {
            rows.push((std::mem::take(&mut row), i.unwrap_or(at)));
            w = 0;
        }
        row.push((c, i));
        w += cw;
    }
    rows.push((row, v.input.len()));
    rows
}

fn cursor_at(v: &View) -> usize {
    let at = v.cursor.min(v.input.len());
    if v.input.is_char_boundary(at) { at } else { v.input.len() }
}

/// Rows scrolled off the top: the cursor's row stays in view when the input is taller than the box.
fn composer_skip(rows: &[(Vec<(char, Option<usize>)>, usize)], at: usize) -> usize {
    let row = rows.iter().position(|(r, end)| *end == at || r.iter().any(|(_, i)| i.is_none_or(|i| i == at))).unwrap_or(rows.len().saturating_sub(1));
    (row + 1).saturating_sub(COMPOSER_MAX_ROWS)
}

/// The input's byte index under the mouse at (`col`, `row`) of the composer's text area, `width` wide: the same
/// wrapping and scrolling as drawn. A row below the text counts as the last one, so a drag past it keeps selecting.
/// None while there is nothing typed or a question owns the box.
pub fn composer_index(v: &View, width: u16, col: u16, row: u16) -> Option<usize> {
    if !v.live || v.input.is_empty() || v.asking.is_some() {
        return None;
    }
    let rows = composer_layout(v, (width as usize).saturating_sub(4).max(1));
    let at = cursor_at(v);
    let skip = composer_skip(&rows, at);
    let last = (rows.len() - skip).min(COMPOSER_MAX_ROWS) - 1;
    let (cells, end) = &rows[skip + (row as usize).min(last)];
    // the text starts after the 3-column lead (" › ")
    let mut x = 3;
    for (c, i) in cells {
        x += c.width().unwrap_or(0);
        if (col as usize) < x {
            return Some(i.unwrap_or(at));
        }
    }
    Some(*end)
}

/// One row under the input: a note for a moment ("copied 12 chars"), else the completions ("   tab › /compact  /clear").
fn hint_row(v: &View, th: &Theme, width: u16) -> Option<Line<'static>> {
    if let Some((t, _)) = &v.toast {
        return Some(Line::from(Span::styled(format!("   {t}"), st(th.ok))));
    }
    if v.hints.is_empty() {
        return None;
    }
    let mut spans = vec![Span::styled("   tab › ", st(th.faint))];
    let mut used = 9;
    for (i, h) in v.hints.iter().enumerate() {
        let w = h.width() + 2;
        if used + w > width as usize {
            break;
        }
        used += w;
        spans.push(Span::styled(format!("{h}  "), if i == 0 { st(th.accent) } else { st(th.dim) }));
    }
    Some(Line::from(spans))
}

const CURSOR: char = '▍';

/// The input with the cursor drawn in at its position.
fn with_cursor(v: &View) -> String {
    let at = v.cursor.min(v.input.len());
    let at = if v.input.is_char_boundary(at) { at } else { v.input.len() };
    format!("{}{CURSOR}{}", &v.input[..at], &v.input[at..])
}

/// Height of a bordered composer box for this input at this outer width.
pub fn composer_height(v: &View, outer_width: u16) -> u16 {
    if !(v.live && !v.input.is_empty()) {
        return 3;
    }
    let room = (outer_width as usize).saturating_sub(2 + 4).max(1);
    let rows: usize = with_cursor(v).split('\n').map(|l| l.width().div_ceil(room).max(1)).sum();
    rows.clamp(1, COMPOSER_MAX_ROWS) as u16 + 2 + u16::from(!v.hints.is_empty() || v.toast.is_some())
}

/// Left and right content on one row.
pub fn split_row(f: &mut Frame, area: Rect, left: Line<'static>, right: Line<'static>, bg: Option<Color>) {
    // paint the row background first: a styled Paragraph would repaint over the spans' own colors
    if let Some(c) = bg {
        f.render_widget(ratatui::widgets::Block::new().style(Style::new().bg(c)), area);
    }
    f.render_widget(Paragraph::new(left), area);
    f.render_widget(Paragraph::new(right).right_aligned(), area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_redrawn_in_part_is_still_closed() {
        use ratatui::backend::Backend;
        let area = Rect::new(0, 0, 40, 1);
        let mut a = ratatui::buffer::Buffer::empty(area);
        a.set_string(0, 0, "see GitHub changelog now", Style::default());
        for x in 4..20 {
            a[(x, 0)].modifier.insert(Modifier::UNDERLINED);
        }
        link_cells(&mut a, area, &[("GitHub changelog".into(), "https://github.blog/c".into())]);
        // the next frame changes only the link's first cell (a drag selection that starts there): ratatui writes only
        // that cell, and an opening without its closing left every later cell a link, underlined (2026-11-11)
        let mut b = a.clone();
        b[(4, 0)].modifier.insert(Modifier::REVERSED);
        let mut out = Vec::new();
        ratatui::backend::CrosstermBackend::new(&mut out).draw(b.diff(&a).into_iter()).unwrap();
        let out = String::from_utf8_lossy(&out);
        let (all, closes) = (out.matches("\x1b]8;").count(), out.matches("\x1b]8;;\x1b\\").count());
        assert!(all > closes && all - closes == closes, "every opened link is closed: {out:?}");
    }

    #[test]
    fn claude_and_lean_edits_read_the_same() {
        let tool = |name: &str, input: serde_json::Value| Tool { id: String::new(), name: name.into(), input, result: None, error: false, started_ms: 0.0, done_ms: None, sub: None };
        let lean = tool("mcp__lean__edit", serde_json::json!({"path": "/r/a.ts", "old": "x", "new": "y"}));
        let claude = tool("Edit", serde_json::json!({"file_path": "/r/a.ts", "old_string": "x", "new_string": "y", "replace_all": false}));
        for t in [&lean, &claude] {
            assert_eq!(short_name(&t.name), "edit");
            assert_eq!((tool_arg(t, "path"), tool_arg(t, "old"), tool_arg(t, "new")), ("/r/a.ts", "x", "y"));
        }
        assert_eq!(short_name("Grep"), "Grep");
        assert_eq!(short_name("Bash"), "bash");
    }

    #[test]
    fn task_output_is_shown_as_plain_text() {
        assert_eq!(plain("\x1b[32mok\x1b[0m 3 passed\r\n\x1b[2Kdone"), "ok 3 passed\ndone");
        // untrusted output cannot drive the terminal: OSC (clipboard write, title) with either terminator, DCS, C1 CSI,
        // BEL, backspace and a bidi override are gone or visible
        assert_eq!(plain("a\x1b]52;c;cm0gLXJm\x07b\x1b]0;title\x1b\\c"), "abc");
        assert_eq!(plain("\x1bPq#0;2;0;0;0\x1b\\x"), "x");
        assert_eq!(plain("\u{9b}31m\x07\x08\u{202e}rm"), "\u{FFFD}31m\u{FFFD}\u{FFFD}\u{FFFD}rm");
        assert!(!plain("\x1b[?1049h\x1b(0\x1b=x").chars().any(|c| c.is_control()));
    }

    #[test]
    fn a_long_task_list_follows_the_selection() {
        let row = |i: usize| TaskRow { kind: "local_bash".into(), description: format!("task number {i}"), status: "completed".into(), secs: 2,
            output: String::new(), agent: String::new(), tokens: 0, tools: 0, doing: String::new() };
        let draw = |selected: usize| {
            let p = TasksPanel { rows: (0..40).map(row).collect(), selected };
            let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 30)).unwrap();
            term.draw(|f| { draw_tasks(f, &p, Design::Cockpit.theme()); }).unwrap();
            let b = term.backend().buffer().clone();
            (0..b.area.height).map(|y| (0..b.area.width).map(|x| b[(x, y)].symbol()).collect::<String>()).collect::<Vec<_>>().join("\n")
        };
        let top = draw(0);
        assert!(top.contains("› completed") && top.contains("task number 0 ") && top.contains("more") && !top.contains("earlier"), "{top}");
        let low = draw(39);
        assert!(low.contains("task number 39") && low.contains("earlier") && !low.contains(" more"), "the last one in view: {low}");
        let mid = draw(20);
        assert!(mid.contains("task number 20") && mid.contains("earlier") && mid.contains("more"), "{mid}");
    }

    #[test]
    fn model_picker_shows_rows_effort_and_why_not() {
        let row = |label: &str, state: u8, stale: bool| ModelRow { label: label.into(), place: "SubRouter · kriora, shared-openai-pro-50, shared-openai-pro-52".into(),
            price: "0.0387 / 0.194".into(), note: "cached 81% · checked today".into(), bench: "6.8 (308)".into(), aa: "32".into(),
            state, why: match state {
                1 => "no key: set the environment variable SUBROUTER_API_KEY before starting tazuna".into(),
                2 => "not trusted: the code would go to subrouter.ai; add the line subrouter.ai to ~/.config/tazuna/trusted-providers".into(),
                _ => String::new(),
            }, stale, current: false };
        let mut p = ModelsPanel { rows: vec![row("Opus 5.5", 0, false), row("deepseek-v4.1-flash-1017 · fast", 1, false), row("gpt-6.1-sol", 2, true), row("gpt-6-luna", 0, true)], selected: 0, filter: String::new(),
            groups: vec![("All models".into(), 4, true, String::new()), ("SubRouter".into(), 4, true, String::new())], group: 1, in_models: true, effort: None };
        let draw = |p: &ModelsPanel, w: u16| {
            let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(w, 24)).unwrap();
            term.draw(|f| draw_models(f, p, Design::Cockpit.theme())).unwrap();
            let b = term.backend().buffer().clone();
            (0..b.area.height).map(|y| (0..b.area.width).map(|x| b[(x, y)].symbol()).collect::<String>()).collect::<Vec<_>>()
        };
        // 78 columns (the user's screen, 2026-11-01: a 70-column panel cut every price), 50 and 120: every price whole and
        // in one column; "· fast" survives a cut label
        for w in [50u16, 78, 120] {
            let lines = draw(&p, w);
            let rows: Vec<&String> = lines.iter().filter(|l| l.contains("0.0387 / 0.194")).collect();
            assert_eq!(rows.len(), 4, "{w}: {lines:#?}");
            let at: Vec<usize> = rows.iter().map(|l| l[..l.find("0.0387 / 0.194").unwrap_or_else(|| panic!("{w}: price cut: {l}"))].chars().count()).collect();
            assert!(at.windows(2).all(|x| x[0] == x[1]), "{w}: {rows:?}");
            assert!(rows[1].contains("· fast"), "{w}: {}", rows[1]);
            assert_eq!(rows[0].contains("6.8 (308)"), w >= 120, "{w}: scores shown where they fit beside the providers");
            let text = lines.concat();
            assert!(text.contains("shared-openai-pro-52"), "{w}: the provider list wraps whole: {lines:#?}");
        }
        assert_eq!(wrap_words("ab cd efghij k", 4), ["ab", "cd", "efgh", "ij k"]);
        // a wide screen (the user's, 2026-11-01): the panel ends where the table does, not at 9/10 of the screen
        let wide = draw(&p, 200);
        let border = wide.iter().find(|l| l.contains('┌')).unwrap();
        assert!(border.trim().chars().count() < 140, "{border}");
        assert!(wide.concat().contains("SubRouter") && wide.concat().contains("All models"), "the providers pane");
        // the detail line says what to do: the variable to set, the line to add (it named `source ~/.subrouter-key`, a
        // file only the bench's machines have, until 0.1.77)
        for (i, want) in [(0, "cached 81%"), (1, "SUBROUTER_API_KEY"), (2, "trusted-providers"), (3, "stale")] {
            p.selected = i;
            assert!(draw(&p, 78).concat().contains(want), "row {i}: {want}");
        }
        // effort is not on the model rows: ← → move between the panes, and a model chosen gets its effort after
        let text = draw(&p, 120).concat();
        assert!(!text.contains("‹") && text.contains("← providers") && text.contains("enter choose"), "{text}");
        p.effort = Some(("deepseek-v4.1-flash-1017".into(), vec!["high".into(), "max".into()], 1, "↑↓ effort · enter use · esc models", String::new()));
        let text = draw(&p, 120).join("\n");
        assert!(text.contains("deepseek-v4.1-flash-1017 · effort") && text.contains("› max") && text.contains("  high") && text.contains("esc models"), "{text}");
        // a Claude switch in a long conversation shows its price under the levels, and asks for a second enter
        p.effort = Some(("Opus 5.5".into(), vec!["medium".into(), "high".into()], 1, "enter again to switch · esc models",
            "Enter again to switch: the next message re-reads ~514k tokens without the cache (≈ $4.11 of plan usage).".into()));
        let text = draw(&p, 120).join(" ");
        assert!(text.contains("Enter again to switch: the next") && text.contains("enter again to switch · esc models"), "{text}");
    }

    #[test]
    fn login_lists_providers_then_a_page_that_asks_for_the_key() {
        let row = |n: &str, h: &str, s: &str| (n.to_string(), h.to_string(), s.to_string());
        let mut p = LoginPanel { rows: vec![row("Claude", "subscription", "the claude CLI's own login"), row("SubRouter", "API key", "logged in"),
            row("Mortera", "API key", "not logged in")], selected: 2, form: None };
        let draw = |p: &LoginPanel, w: u16| {
            let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(w, 30)).unwrap();
            term.draw(|f| draw_login(f, p, Design::Cockpit.theme())).unwrap();
            let b = term.backend().buffer().clone();
            (0..b.area.height).map(|y| (0..b.area.width).map(|x| b[(x, y)].symbol()).collect::<String>().trim_end().to_string()).collect::<Vec<_>>()
        };
        let list = draw(&p, 100).join("\n");
        assert!(list.contains("› Mortera") && list.contains("not logged in") && list.contains("enter log in"), "{list}");
        p.form = Some(LoginForm { name: "Mortera".into(), page: "https://mortera.cloud".into(), opened: true, place: "your Mortera dashboard".into(),
            example: String::new(), typed: 5, host: "mortera.cloud".into(), var: "MORTERA_API_KEY".into(), has: String::new() });
        // 60 columns: nothing cut, the long lines wrap
        for w in [60u16, 100] {
            let page = draw(&p, w).join("\n");
            for want in ["Login to Mortera", "https://mortera.cloud", "opened in your browser", "Paste your Mortera API key", "> •••••▍",
                         "(esc to cancel, enter to submit)", "mortera.cloud on your trusted"] {
                assert!(page.contains(want), "{w}: {want}\n{page}");
            }
        }
    }

    #[test]
    fn plan_and_balance_show_whole_in_every_design() {
        let mut s = Session::new("", "medium");
        s.limits = Some((12.0, 14.0));
        s.balance = Some(8.816);
        for d in [Design::Cockpit, Design::Aurora, Design::Zen, Design::Timeline] {
            let v = View { design: d, expand_all: false, scroll: 0, clock_ms: 0.0, speed: 1.0, paused: false, live: true, input: String::new(), cursor: 0, select: None, asking: None, mode: "auto".into(), hints: Vec::new(), bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None, mouse: None, list: None, hover_call: None, login: None, cold: 0 };
            let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(160, 40)).unwrap();
            term.draw(|f| { draw(f, &s, &v); }).unwrap();
            let text: String = term.backend().buffer().content().iter().map(|c| c.symbol()).collect();
            // the cockpit's sidebar cut "SubRouter $8.82 left" to "$8" when it shared the plan's line (2026-11-02)
            assert!(text.contains("SubRouter $8.82 left") && text.contains("5h 12%"), "design {}", d as u8);
        }
    }

    #[test]
    fn long_calls_close_to_a_line_and_open_on_a_click() {
        let mut s = Session::new("", "medium");
        let prompt = "Repo: /x. Read scripts/load-secrets.sh from the PR branch.\n\nAnswer with line numbers:\n".to_string() + &"1. How is fetch_secret invoked? ".repeat(40);
        s.blocks.push(Block::Tool(Tool { id: "a".into(), name: "Agent".into(),
            input: serde_json::json!({"subagent_type": "explorer", "description": "Inspect load-secrets.sh retry impact", "prompt": prompt}),
            result: Some("It runs sequentially.".into()), error: false, started_ms: 0.0, done_ms: Some(6700.0), sub: None }));
        s.meta.push(crate::session::Meta { at_ms: 0.0, turn: 1 });
        s.blocks.push(Block::Tool(Tool { id: "b".into(), name: "Bash".into(), input: serde_json::json!({"command": format!("gh run view 1 --jq '{}'", "x".repeat(400))}),
            result: Some("ok".into()), error: false, started_ms: 0.0, done_ms: Some(4300.0), sub: None }));
        s.meta.push(crate::session::Meta { at_ms: 0.0, turn: 1 });
        let mut v = View { design: Design::Cockpit, expand_all: false, scroll: 0, clock_ms: 0.0, speed: 1.0, paused: false, live: true, input: String::new(), cursor: 0, select: None, asking: None, mode: "auto".into(), hints: Vec::new(), bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None, mouse: None, list: None, hover_call: None, login: None, cold: 0 };
        let text = |v: &View| { let (l, _, owners) = transcript(&s, v, Design::Cockpit.theme(), &cockpit::OPTS, 100); (l.iter().map(|l| l.spans.iter().map(|x| x.content.to_string()).collect::<String>()).collect::<Vec<_>>(), owners) };
        let (closed, owners) = text(&v);
        // closed: the subagent as who and what for (its 25-row prompt hidden), the long command cut, both marked ▸
        assert!(closed[0].contains("explorer: Inspect load-secrets.sh retry impact") && closed[0].contains("▸") && !closed.concat().contains("fetch_secret"), "{closed:#?}");
        assert!(closed.len() <= 6, "{} rows: {closed:#?}", closed.len());
        assert_eq!((owners[0], owners.last().copied().flatten()), (Some(0), Some(1)));
        assert!(closed.last().unwrap().starts_with("         · 1 line · 4.3s"), "a long call's counts on a row of their own: {closed:#?}");
        v.open.insert(0);
        let (opened, _) = text(&v);
        assert!(opened.concat().contains("How is fetch_secret invoked") && opened.concat().contains("It runs sequentially") && opened[0].contains("▾"), "{opened:#?}");
        v.hover = Some(1);
        let (l, _, _) = transcript(&s, &v, Design::Cockpit.theme(), &cockpit::OPTS, 100);
        assert_eq!(l.last().unwrap().style.bg, Some(Design::Cockpit.theme().surface), "the block under the mouse is shaded");
    }

    #[test]
    fn tables_get_borders_and_fit_the_width() {
        let th = Design::Cockpit.theme();
        let md = "Free space:\n\n| Path | Size | What |\n|---|---:|---|\n| `~/bench-gocache` | 243 GB | Go build cache for the benchmark; safe to delete after runs |\n| ~/.omlx/models | 61 GB | Local MLX models |\n\nDone.";
        let text = |w: usize| markdown(md, th, w).iter().map(|(l, _)| l.spans.iter().map(|s| s.content.to_string()).collect::<String>()).collect::<Vec<_>>();
        let wide = text(100);
        assert!(wide.iter().any(|l| l.starts_with("┌")) && wide.iter().any(|l| l.starts_with("└")), "{wide:#?}");
        assert!(wide.iter().any(|l| l.contains("│ ~/bench-gocache │ 243 GB │")), "{wide:#?}");
        assert!(!wide.concat().contains("|---"), "the rule is not shown as text");
        // narrow: cells wrap inside their columns, every row of the table the same width, none past the width
        let narrow = text(44);
        let rows: Vec<&String> = narrow.iter().filter(|l| l.starts_with(['┌', '│', '├', '└'])).collect();
        assert!(rows.len() > 6 && rows.iter().all(|l| l.width() == rows[0].width() && l.width() <= 44), "{narrow:#?}");
        assert_eq!(text(10).iter().filter(|l| l.contains('│')).count(), 0, "too narrow: rows as plain lines");
    }

    #[test]
    fn links_show_their_title_and_open_where_they_point() {
        let l = links("Sources: [1 vCPU runner GA](https://github.blog/changelog/2026-01-22-x/), [preview](https://github.blog/p). See https://example.com/a.");
        assert_eq!(l.iter().filter(|(_, u)| u.is_some()).map(|(t, u)| (t.as_str(), u.as_deref().unwrap())).collect::<Vec<_>>(),
                   [("1 vCPU runner GA", "https://github.blog/changelog/2026-01-22-x/"), ("preview", "https://github.blog/p"), ("https://example.com/a", "https://example.com/a")]);
        assert_eq!(l.iter().map(|(t, _)| t.as_str()).collect::<String>(), "Sources: 1 vCPU runner GA, preview. See https://example.com/a.");
        assert!(links("[x](javascript:alert(1)) [y](https://a.b/\u{1b}]8;;)").iter().all(|(_, u)| u.is_none()), "only plain http(s) addresses");
        let mut s = Session::new("", "medium");
        s.blocks.push(Block::Text { text: "Sources: [GitHub changelog](https://github.blog/c)".into(), live: false });
        s.meta.push(crate::session::Meta { at_ms: 0.0, turn: 1 });
        let v = View { design: Design::Cockpit, expand_all: false, scroll: 0, clock_ms: 0.0, speed: 1.0, paused: false, live: true, input: String::new(), cursor: 0, select: None, asking: None, mode: "auto".into(), hints: Vec::new(), bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None, mouse: None, list: None, hover_call: None, login: None, cold: 0 };
        let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 20)).unwrap();
        let mut areas = None;
        term.draw(|f| areas = Some(draw(f, &s, &v))).unwrap();
        let areas = areas.unwrap();
        assert_eq!(areas.links.iter().map(|(r, u)| (r.width, u.as_str())).collect::<Vec<_>>(), [(16, "https://github.blog/c")]);
        let text: String = term.backend().buffer().content().iter().map(|c| c.symbol()).collect();
        assert!(text.contains(";https://github.blog/c\x1b\\G\x1b]8;;\x1b\\") && !text.contains("](https"), "a terminal hyperlink, no raw markdown");
        assert_eq!(plain(&text).matches("GitHub changelog").count(), 1, "copied text keeps the title, not the escape");
        // the mouse over the link: its cells reversed, the rest of the row not
        let r = areas.links[0].0;
        let mut v = v;
        v.mouse = Some((r.x + 1, r.y));
        term.draw(|f| { draw(f, &s, &v); }).unwrap();
        let buf = term.backend().buffer();
        assert!(buf[(r.x, r.y)].modifier.contains(Modifier::REVERSED), "the hovered link stands out (its first cell carries the whole hyperlink)");
        assert!(!buf[(r.x + r.width + 1, r.y)].modifier.contains(Modifier::REVERSED), "only the link");
    }

    #[test]
    fn the_session_picker_lists_and_marks_the_selected_one() {
        let s = Session::new("", "medium");
        let mut v = View { design: Design::Cockpit, expand_all: false, scroll: 0, clock_ms: 0.0, speed: 1.0, paused: false, live: true, input: String::new(), cursor: 0, select: None, asking: None, mode: "auto".into(), hints: Vec::new(), bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None, mouse: None, list: None, hover_call: None, login: None, cold: 0 };
        v.sessions = Some(SessionsPanel { rows: vec!["8m ago · 2 messages · review PR 1370  (4cb8a388)".into(), "2h ago · 5 messages · fix the login  (aa11bb22)".into()], selected: 1 });
        let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 20)).unwrap();
        term.draw(|f| { draw(f, &s, &v); }).unwrap();
        let text: String = term.backend().buffer().content().iter().map(|c| c.symbol()).collect();
        assert!(text.contains("resume a session") && text.contains("  8m ago · 2 messages · review PR 1370") && text.contains("› 2h ago · 5 messages · fix the login"), "{text}");
    }

    #[test]
    fn background_tasks_show_live_under_the_transcript() {
        let s = Session::new("", "medium");
        let mut v = View { design: Design::Cockpit, expand_all: false, scroll: 0, clock_ms: 400.0, speed: 1.0, paused: false, live: true, input: String::new(), cursor: 0, select: None, asking: None, mode: "auto".into(), hints: Vec::new(), bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None, mouse: None, list: None, hover_call: None, login: None, cold: 0 };
        let row = |agent: &str, kind: &str, secs, tokens, tools, doing: &str| TaskRow { kind: kind.into(), description: "Tick workflows loss window check".into(), status: "running".into(), secs, output: String::new(), agent: agent.into(), tokens, tools, doing: doing.into() };
        assert!(transcript(&s, &v, Design::Cockpit.theme(), &cockpit::OPTS, 100).0.is_empty(), "nothing while none run");
        v.running = vec![row("explorer", "local_agent", 72, 21_129, 3, "Running Show Ledger tick workflow"), row("", "local_bash", 12, 0, 0, "")];
        let (lines, last, owners) = transcript(&s, &v, Design::Cockpit.theme(), &cockpit::OPTS, 100);
        let text: Vec<String> = lines.iter().map(|l| l.spans.iter().map(|x| x.content.to_string()).collect()).collect();
        assert!(text[1].contains("waiting on 2 background tasks"), "{text:#?}");
        assert_eq!(text[2], "  explorer · Tick workflows loss window check · 1m 12s · 21.1k tokens · 3 tool calls");
        assert_eq!(text[3], "    ⎿ Running Show Ledger tick workflow");
        assert_eq!(text[4], "  bash · Tick workflows loss window check · 12s");
        assert_eq!((last, owners.iter().all(Option::is_none)), (0, true));
        assert_eq!(lines_n(1), "1 line");
    }

    #[test]
    fn files_panel_totals_and_the_diff_view() {
        let s = Session::new("", "medium");
        let mut v = View { design: Design::Cockpit, expand_all: false, scroll: 0, clock_ms: 0.0, speed: 1.0, paused: false, live: true, input: String::new(), cursor: 0, select: None, asking: None, mode: "auto".into(), hints: Vec::new(), bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None, mouse: None, list: None, hover_call: None, login: None, cold: 0 };
        v.changed = (0..12).map(|i| Changed { path: format!("shared/src/testing/file{i}.ts"), added: 10 + i, removed: i, diff: None }).collect();
        let text = |v: &View| {
            let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(140, 40)).unwrap();
            term.draw(|f| { draw(f, &s, v); }).unwrap();
            term.backend().buffer().content().iter().map(|c| c.symbol()).collect::<String>()
        };
        let t = text(&v);
        assert!(t.contains("12 files +186 −66") && t.contains("click: diffs") && t.contains("more"), "a total, the files that fit, the rest counted");
        v.diff = Some(DiffPanel { selected: 1, diff: vec!["@@ -1,3 +1,4 @@".into(), "-old line".into(), "+new line".into()], scroll: 0 });
        let d = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(first_change(&d(&["diff --git a/x b/x", "@@ -10,6 +10,7 @@ fn x()", " a", " b", "+c", " d"])), 12, "the first added line");
        assert_eq!(first_change(&d(&["@@ -3,3 +3,2 @@", " a", "-b", " c"])), 4, "a deletion: where it was");
        assert_eq!(first_change(&d(&["+++ new.txt (new file)", "+a"])), 1, "a new file: its top");
        let t = text(&v);
        assert!(t.contains("12 files changed · +186 −66") && !t.contains("+new line") && t.contains("› ") , "the file list with the selected file marked, no diff text (2026-11-06)");
    }

    #[test]
    fn key_hints_fit_whole_and_point_to_the_rest() {
        assert_eq!(fitted_keys("a · bb · cc", 100), "a · bb · cc");
        assert_eq!(fitted_keys("a · bb · cc", 16), "a · /keys all", "never a hint cut in half");
        let v = View { design: Design::Cockpit, expand_all: false, scroll: 0, clock_ms: 0.0, speed: 1.0, paused: false, live: true, input: String::new(), cursor: 0, select: None, asking: None, mode: "auto".into(), hints: Vec::new(), bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None, mouse: None, list: None, hover_call: None, login: None, cold: 0 };
        let line: String = composer_line(&v, Design::Cockpit.theme(), "›", "Message tazuna…", 90).spans.iter().map(|s| s.content.to_string()).collect();
        assert!(line.width() <= 90 && line.ends_with("/keys all"), "{line}");
        let mut v = v;
        v.toast = Some(("copied 42 chars".into(), std::time::Instant::now()));
        let line: String = composer_line(&v, Design::Cockpit.theme(), "›", "Message tazuna…", 90).spans.iter().map(|s| s.content.to_string()).collect();
        assert!(line.ends_with("copied 42 chars") && !line.contains("/keys"), "a note takes the hints' place for a moment: {line}");
    }

    #[test]
    fn composer_grows_with_lines_and_keeps_the_latest() {
        let mut v = View { design: Design::Cockpit, expand_all: false, scroll: 0, clock_ms: 0.0, speed: 1.0, paused: false, live: true, input: String::new(), cursor: 0, select: None, asking: None, mode: "auto".into(), hints: Vec::new(), bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None, mouse: None, list: None, hover_call: None, login: None, cold: 0 };
        let th = Design::Cockpit.theme();
        assert_eq!(composer_height(&v, 40), 3, "empty: one row");
        v.input = "first\nsecond".into();
        v.cursor = v.input.len();
        assert_eq!(composer_height(&v, 40), 4);
        let rows = composer_rows(&v, th, "›", "", 38);
        let text: Vec<String> = rows.iter().map(|r| r.spans.iter().map(|s| s.content.to_string()).collect()).collect();
        assert_eq!(text, vec![" › first", "   second▍"]);
        v.input = "x".repeat(70);
        v.cursor = v.input.len(); // wraps at 34 columns: 3 rows
        assert_eq!(composer_height(&v, 40), 5);
        assert_eq!(composer_rows(&v, th, "›", "", 38).len(), 3);
        v.input = (1..=10).map(|i| i.to_string()).collect::<Vec<_>>().join("\n");
        v.cursor = v.input.len();
        assert_eq!(composer_height(&v, 40), 8, "capped at 6 rows");
        let rows = composer_rows(&v, th, "›", "", 38);
        assert_eq!(rows.len(), 6);
        assert!(rows[5].spans.iter().any(|s| s.content == "10"), "the latest line stays visible");
        v.cursor = 0;
        let rows = composer_rows(&v, th, "›", "", 38);
        assert!(rows[0].spans.iter().any(|s| s.content == "1"), "the cursor's row stays visible");
    }

    #[test]
    fn a_bold_address_links_without_its_marks() {
        let l = reply_links("PR: **https://github.com/acme/app/pull/1**. And [**the docs**](https://example.com/d).");
        assert_eq!(l.iter().map(|(_, u)| u.as_str()).collect::<Vec<_>>(), ["https://github.com/acme/app/pull/1", "https://example.com/d"]);
    }

    #[test]
    fn a_click_lands_where_the_text_is_drawn() {
        let mut v = View { design: Design::Cockpit, expand_all: false, scroll: 0, clock_ms: 0.0, speed: 1.0, paused: false, live: true, input: "hello world\nsecond".into(), cursor: 0, select: None, asking: None, mode: "auto".into(), hints: Vec::new(), bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None, mouse: None, list: None, hover_call: None, login: None, cold: 0 };
        // cursor at 0 is drawn as a cell before "h": columns 3 (cursor), 4 (h) ...
        assert_eq!(composer_index(&v, 38, 3, 0), Some(0));
        assert_eq!(composer_index(&v, 38, 4 + 6, 0), Some(6), "the w of world");
        assert_eq!(composer_index(&v, 38, 37, 0), Some(11), "past the end: the line's end");
        assert_eq!(composer_index(&v, 38, 0, 0), Some(0), "the lead: the line's start");
        assert_eq!(composer_index(&v, 38, 4, 1), Some(13), "the e of second (no cursor on that row)");
        assert_eq!(composer_index(&v, 38, 4, 5), Some(13), "below the text: the last row");
        v.cursor = v.input.len();
        assert_eq!(composer_index(&v, 38, 3, 0), Some(0), "without the cursor cell, column 3 is the h");
        (v.select, v.cursor) = (Some((0, 5)), 0);
        let th = Design::Cockpit.theme();
        let rows = composer_rows(&v, th, "›", "", 38);
        let hello = rows[0].spans.iter().find(|s| s.content == "hello").expect("the selection is a span of its own");
        assert!(hello.style.add_modifier.contains(Modifier::REVERSED));
        assert!(!rows[0].spans.iter().any(|s| s.content.contains(CURSOR)), "no cursor cell while selecting");
        assert_eq!(composer_index(&v, 38, 3 + 5, 0), Some(5), "nothing shifts under a drag: column 8 is the space");
        v.input.clear();
        assert_eq!(composer_index(&v, 38, 3, 0), None, "nothing typed: the placeholder");
    }

    #[test]
    fn transcript_rows_cannot_drive_the_terminal() {
        let row = Line::from(vec![Span::raw("restoring a\x1b]0;owned\x07.txt"), Span::raw(" \x1b[2J\u{9b}x")]);
        let text: String = wrap(row, 80, &[]).iter().flat_map(|l| l.spans.iter().map(|s| s.content.to_string())).collect();
        assert!(!text.chars().any(|c| c.is_control()), "{text:?}");
        assert!(text.contains("restoring a.txt") && text.contains('\u{FFFD}'));
    }

    #[test]
    fn cockpit_tools_show_the_average_and_tokens_an_axis() {
        let mut s = Session::new("", "medium");
        for i in 0..14 {
            s.blocks.push(Block::Tool(Tool { id: format!("a{i}"), name: "Agent".into(), input: serde_json::json!({"description": "look"}), result: Some("ok".into()), error: false, started_ms: 0.0, done_ms: Some(131_600.0), sub: None }));
            s.meta.push(crate::session::Meta { at_ms: 0.0, turn: 1 });
        }
        s.turn_usage = (0..40).map(|i| crate::session::Usage { input: 10, cache_write: 100, cache_read: 50_000, output: 200 + 300 * (i % 7) }).collect();
        let mut v = View { design: Design::Cockpit, expand_all: false, scroll: 0, clock_ms: 0.0, speed: 1.0, paused: false, live: true, input: String::new(), cursor: 0, select: None, asking: None, mode: "auto".into(), hints: Vec::new(), bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None, mouse: None, list: None, hover_call: None, login: None, cold: 0 };
        let render = |v: &View| {
            let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 44)).unwrap();
            let mut areas = Areas::default();
            term.draw(|f| { areas = draw(f, &s, v); }).unwrap();
            let buf = term.backend().buffer().clone();
            let rows: Vec<String> = (0..buf.area.height).map(|y| (82..120).map(|x| buf[(x, y)].symbol().to_string()).collect()).collect();
            (rows.join("\n"), areas)
        };
        let (side, areas) = render(&v);
        assert!(side.contains("×14") && side.contains("30.7m") && side.contains("⌀2.2m"), "{side}");
        assert!(side.contains("2.0k") && side.contains("out per call · #"), "an axis label and the caption: {side}");
        assert_eq!(areas.tool_rows.len(), 1);
        let (rect, first) = areas.bars.unwrap();
        v.hover_call = Some(first + 2);
        let (side, _) = render(&v);
        assert!(side.contains(&format!("#{}", first + 3)) && rect.width > 10, "{side}");
    }

    #[test]
    fn fetch_rows_show_a_title_link_and_scroll_sideways() {
        let tool = |result: &str, url: &str| Tool { id: "f".into(), name: "fetch".into(), input: serde_json::json!({"url": url}), result: Some(result.into()), error: false, started_ms: 0.0, done_ms: Some(1.0), sub: None };
        assert_eq!(fetch_label(&tool("[fetched https://x.com/a · \"Crypto prices today\", HTTP 200: web content]\nbody", "https://x.com/a")), "Crypto prices today");
        assert_eq!(fetch_label(&tool("[fetched https://api.coingecko.com/api/v3/global, HTTP 200: web content]\n{}", "https://api.coingecko.com/api/v3/global?x=1")), "api.coingecko.com/api/v3/global");
        let mut p = ListPanel { title: "fetch".into(), head: "  #   took     what".into(), rows: vec![(format!("{:>3}  {:>7}  {}  {}", 1, "0.5s", "✓", "Crypto prices today"), false)], scroll: 0,
            links: vec![Some("https://x.com/a".into())], link_from: 17, hscroll: 0, details: vec![None], open: Default::default() };
        let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 20)).unwrap();
        let mut out = (None, vec![], vec![]);
        term.draw(|f| { out = draw_list(f, &p, Design::Cockpit.theme()); }).unwrap();
        let (area, links, _) = out.clone();
        assert!(area.is_some() && links.len() == 1 && links[0].1 == "https://x.com/a");
        let r = links[0].0;
        assert_eq!(term.backend().buffer()[(r.x, r.y)].symbol(), "C", "the link starts at the title");
        p.hscroll = 10;
        term.draw(|f| { out = draw_list(f, &p, Design::Cockpit.theme()); }).unwrap();
        assert_eq!(out.1[0].0.x, r.x - 10, "scrolled sideways, the link moves with its text");
    }

    #[test]
    fn the_diff_view_lists_files_and_long_notices_open() {
        let files = vec![Changed { path: "tazuna/examples/studio.rs".into(), added: 868, removed: 0, diff: None }, Changed { path: "src/a.rs".into(), added: 3, removed: 2, diff: None }];
        let p = DiffPanel { selected: 0, diff: vec!["+//! a line of code".into()], scroll: 0 };
        let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 20)).unwrap();
        let mut hits = vec![];
        term.draw(|f| { hits = draw_diff(f, &files, &p, Design::Cockpit.theme(), None); }).unwrap();
        let text: String = term.backend().buffer().content().iter().map(|c| c.symbol()).collect();
        assert!(text.contains("tazuna/examples/studio.rs") && text.contains("+868") && !text.contains("a line of code"), "files and counts, no diff text");
        assert_eq!(hits.len(), 2);
        // a notice longer than NOTICE_CHARS: cut and clickable, opened all of it
        let mut s = Session::new("", "medium");
        s.blocks.push(Block::Notice { text: "x".repeat(300), error: false });
        s.meta.push(crate::session::Meta { at_ms: 0.0, turn: 0 });
        let lines = |open: bool| block_lines(&s.blocks[0], &s, &View { design: Design::Cockpit, expand_all: open, scroll: 0, clock_ms: 0.0, speed: 1.0, paused: false, live: true, input: String::new(), cursor: 0, select: None, asking: None, mode: "auto".into(), hints: Vec::new(), bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None, mouse: None, list: None, hover_call: None, login: None, cold: 0 }, Design::Cockpit.theme(), &cockpit::OPTS, open, 400).iter().map(|(l, _)| l.to_string()).collect::<String>();
        assert!(lines(false).contains("click to show") && !lines(true).contains("click to show"));
    }

    #[test]
    fn a_modal_fades_what_is_behind_it() {
        let s = Session::new("", "medium");
        let mut v = View { design: Design::Cockpit, expand_all: false, scroll: 0, clock_ms: 0.0, speed: 1.0, paused: false, live: true, input: String::new(), cursor: 0, select: None, asking: None, mode: "auto".into(), hints: Vec::new(), bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None, mouse: None, list: None, hover_call: None, login: None, cold: 0 };
        let th = Design::Cockpit.theme();
        let cell = |v: &View| {
            let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 40)).unwrap();
            term.draw(|f| { draw(f, &s, v); }).unwrap();
            let b = term.backend().buffer();
            (b[(3, 0)].fg, b[(60, 20)].symbol().to_string())
        };
        assert_ne!(cell(&v).0, th.border, "the header in its own colours without a modal");
        v.list = Some(ListPanel { title: "model calls".into(), head: "#".into(), rows: vec![("1".into(), false)], scroll: 0, links: vec![], link_from: 0, hscroll: 0, details: vec![], open: Default::default() });
        let (fg, _) = cell(&v);
        assert_eq!(fg, th.border, "behind a modal everything takes the border colour");
    }

    #[test]
    fn files_git_misses_get_the_tools_own_diff() {
        let mut s = Session::new("", "medium");
        s.cwd = "/repo".into();
        let tool = |id: &str, name: &str, input: serde_json::Value, error: bool| Block::Tool(Tool { id: id.into(), name: name.into(), input, result: Some("ok".into()), error, started_ms: 0.0, done_ms: Some(10.0), sub: None });
        s.blocks.push(tool("a", "Edit", serde_json::json!({"file_path": "/repo/src/a.ts", "old_string": "let x = 1;", "new_string": "let x = 2;\nlet y = 3;"}), false));
        s.blocks.push(tool("b", "Write", serde_json::json!({"file_path": "/tmp/notes.md", "content": "one\ntwo"}), false));
        s.blocks.push(tool("c", "Edit", serde_json::json!({"file_path": "/repo/src/b.ts", "old_string": "x", "new_string": "y"}), true));
        s.blocks.push(tool("d", "Edit", serde_json::json!({"file_path": "/repo/src/git.ts", "old_string": "x", "new_string": "y"}), false));
        let got = written_by_tools(&s, &["src/git.ts".to_string()]);
        let paths: Vec<&str> = got.iter().map(|c| c.path.as_str()).collect();
        assert_eq!(paths, ["src/a.ts", "/tmp/notes.md"], "git's files and failed edits left out; repo paths relative");
        assert_eq!((got[0].added, got[0].removed), (2, 1));
        let d = got[0].diff.as_ref().unwrap();
        assert!(d.contains(&"-let x = 1;".to_string()) && d.contains(&"+let y = 3;".to_string()), "{d:?}");
        assert_eq!((got[1].added, got[1].removed), (2, 0));
    }

    #[test]
    fn a_long_prompt_opens_in_full() {
        let th = Design::Cockpit.theme();
        let o = Opts { marker: "", prompt: PromptStyle::Bar, group_tools: true, timeline: false, durations: false };
        let p = (1..=20).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n\n");
        let text = |open| prompt_lines(&p, true, &th, &o, open, 80).iter().map(|(l, _)| l.to_string()).collect::<Vec<_>>().join("\n");
        let closed = text(false);
        assert!(closed.contains("more lines · click to show") && !closed.contains("line 20"), "{closed}");
        let opened = text(true);
        assert!(opened.contains("line 20") && !opened.contains("more lines"), "{opened}");
    }

    #[test]
    fn wrap_respects_width_and_prefix() {
        let line = Line::from(vec![Span::raw("alpha beta gamma delta "), Span::raw("epsilonepsilonepsilon")]);
        let rows = wrap(line, 12, &[Span::raw("  ")]);
        for r in &rows {
            assert!(r.width() <= 12, "row too wide: {r:?}");
        }
        assert!(rows.len() >= 4);
        assert!(rows[1].spans[0].content == "  ", "continuation rows start with the prefix");
        let text: String = rows.iter().flat_map(|r| r.spans.iter().map(|s| s.content.to_string())).collect();
        assert!(text.contains("epsilon"));
    }
}
