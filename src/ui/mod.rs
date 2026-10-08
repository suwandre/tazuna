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
    /// Lines scrolled up from the bottom; 0 follows the live tail.
    pub scroll: usize,
    pub clock_ms: f64,
    pub speed: f64,
    pub paused: bool,
    /// Live mode: the user drives a real claude process and the composer takes input.
    pub live: bool,
    pub input: String,
}

/// Screen regions the effects target.
pub struct Areas {
    /// Flashes when a tool finishes.
    pub pulse: Rect,
    /// Rows of the newest block, when it is on screen at the bottom.
    pub last_block: Option<Rect>,
}

pub fn draw(f: &mut Frame, s: &Session, v: &View) -> Areas {
    let th = v.design.theme();
    f.render_widget(ratatui::widgets::Block::new().style(Style::new().bg(th.bg)), f.area());
    match v.design {
        Design::Aurora => aurora::draw(f, s, v, th),
        Design::Cockpit => cockpit::draw(f, s, v, th),
        Design::Zen => zen::draw(f, s, v, th),
        Design::Timeline => timeline::draw(f, s, v, th),
    }
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

pub fn cost_str(s: &Session) -> String {
    match s.exact_cost {
        Some(c) => format!("${c:.2}"),
        None => format!("≈${:.2}", s.cost()),
    }
}

pub fn clock_str(ms: f64) -> String {
    let secs = (ms / 1000.0) as u64;
    format!("{:02}:{:02}", secs / 60, secs % 60)
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
pub fn files_touched(s: &Session) -> Vec<String> {
    const EXT: [&str; 12] = [".go", ".rs", ".py", ".ts", ".tsx", ".js", ".md", ".json", ".toml", ".yaml", ".yml", ".sh"];
    let mut out: Vec<String> = Vec::new();
    for b in s.blocks.iter().rev() {
        let Block::Tool(t) = b else { continue };
        let mut found: Vec<String> = Vec::new();
        if !tool_arg(t, "path").is_empty() {
            found.push(rel(s, tool_arg(t, "path")));
        }
        if let Some(cmd) = t.input["command"].as_str() {
            for w in cmd.split(|c: char| c.is_whitespace() || "'\";|&<>()".contains(c)) {
                if EXT.iter().any(|e| w.ends_with(e)) && !w.contains('*') && !w.starts_with('-') {
                    found.push(rel(s, w));
                }
            }
        }
        for f in found {
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
        _ => i.to_string(),
    }
}

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
pub fn transcript(s: &Session, v: &View, th: &Theme, o: &Opts, width: usize) -> (Vec<Line<'static>>, usize) {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut last_rows = 0;
    let mut i = 0;
    let mut prev_turn = 0;
    while i < s.blocks.len() {
        let b = &s.blocks[i];
        let meta = s.meta[i];
        // a run of tool calls collapses into one line in grouped designs
        let foldable = |b: &Block| matches!(b, Block::Tool(_) | Block::Thinking { .. });
        let mut group_end = i + 1;
        if o.group_tools && !v.expand_all && foldable(b) {
            while group_end < s.blocks.len() && foldable(&s.blocks[group_end]) {
                group_end += 1;
            }
        }
        let grouped = o.group_tools && !v.expand_all && foldable(b);
        let after_tool = i > 0 && matches!((&s.blocks[i - 1], b), (Block::Tool(_), Block::Tool(_)));
        if o.timeline && meta.turn != prev_turn && meta.turn > 0 {
            prev_turn = meta.turn;
            lines.push(turn_divider(s, th, meta.turn));
        } else if i > 0 && !after_tool {
            // the timeline keeps its rail unbroken between steps
            lines.push(if o.timeline { Line::from(vec![Span::raw("      "), Span::styled("│", st(th.border))]) } else { Line::default() });
        }
        let before = lines.len();
        let body = if grouped { group_lines(&s.blocks[i..group_end], s, v, th) } else { block_lines(b, s, v, th, o) };
        let gutter_w = if o.timeline { GUTTER } else { 0 };
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
        last_rows = lines.len() - before;
        i = group_end;
    }
    (lines, last_rows)
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
            format!("  ↑ {}  ↓ {}  ≈${:.3}", human(u.total_in()), human(u.output), u.estimate()),
            st(th.faint),
        ));
    }
    Line::from(spans)
}

/// Draws the transcript tail (or the scrolled position); returns the newest block's rows when following.
pub fn render_transcript(f: &mut Frame, area: Rect, lines: Vec<Line<'static>>, last_rows: usize, scroll: usize) -> Option<Rect> {
    let h = area.height as usize;
    let max_scroll = lines.len().saturating_sub(h);
    let start = max_scroll.saturating_sub(scroll.min(max_scroll));
    let visible: Vec<Line> = lines.into_iter().skip(start).take(h).collect();
    let shown = visible.len();
    f.render_widget(Paragraph::new(visible), area);
    (scroll == 0 && last_rows > 0 && shown > 0).then(|| {
        let rows = last_rows.min(shown) as u16;
        Rect { y: area.y + shown as u16 - rows, height: rows, ..area }
    })
}

fn block_lines(b: &Block, s: &Session, v: &View, th: &Theme, o: &Opts) -> Vec<Prefixed> {
    match b {
        Block::Prompt(p) => prompt_lines(p, first_prompt(s, b), th, o),
        Block::Verify(x) => verify_lines(x, v, th),
        Block::Notice { text, error } => vec![(
            Line::from(vec![
                Span::styled(if *error { "! " } else { "· " }, st(if *error { th.err } else { th.warn })),
                Span::styled(text.clone(), st(if *error { th.err } else { th.dim }).add_modifier(Modifier::ITALIC)),
            ]),
            vec![Span::raw("  ")],
        )],
        Block::Text { text, live } => {
            let mut out = markdown(text, th);
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
        Block::Thinking { tokens, live } => {
            let glyph = if *live { spinner(v.clock_ms) } else { "✻" };
            let mut spans = vec![Span::styled(format!("{glyph} "), st(th.warn)), Span::styled("thinking", st(th.dim).add_modifier(Modifier::ITALIC))];
            if *tokens > 0 {
                spans.push(Span::styled(format!(" · ~{tokens} tok"), st(th.faint)));
            }
            vec![(Line::from(spans), vec![])]
        }
        Block::Tool(t) => tool_lines(t, s, v, th, o),
        Block::Done(d) => {
            let (glyph, word, color) = match (d.stopped, d.ok) {
                (true, _) => ("■", "Stopped", th.warn),
                (false, true) => ("✔", "Done", th.ok),
                (false, false) => ("✗", "Failed", th.err),
            };
            let u = &d.usage;
            let files = files_touched(s).len();
            vec![
                (
                    Line::from(vec![
                        Span::styled(format!("{glyph} {word}"), st(color).add_modifier(Modifier::BOLD)),
                        Span::styled(
                            format!("   {}m {:02}s · {} turn{} · ${:.2}", d.secs as u64 / 60, d.secs as u64 % 60, d.turns, if d.turns == 1 { "" } else { "s" }, d.cost),
                            st(th.text),
                        ),
                    ]),
                    vec![],
                ),
                (
                    Line::from(Span::styled(
                        format!("  session: in {} ({:.0}% cached) · out {} · {files} file{} touched", human(u.total_in()), u.cache_hit() * 100.0, human(u.output), if files == 1 { "" } else { "s" }),
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

fn prompt_lines(p: &str, first: bool, th: &Theme, o: &Opts) -> Vec<Prefixed> {
    let md = markdown(p, th);
    let more = md.len().saturating_sub(PROMPT_LINES);
    let mut out: Vec<Prefixed> = Vec::new();
    match o.prompt {
        PromptStyle::Bar => {
            let bar = || Span::styled("▎ ", st(th.accent));
            let label = if first { "task" } else { "you" };
            out.push((Line::from(vec![bar(), Span::styled(label, st(th.accent).add_modifier(Modifier::BOLD))]), vec![bar()]));
            for (mut l, mut cont) in md.into_iter().take(PROMPT_LINES) {
                l.spans.insert(0, bar());
                cont.insert(0, bar());
                out.push((l, cont));
            }
            if more > 0 {
                out.push((Line::from(vec![bar(), Span::styled(format!("… {more} more lines"), st(th.faint))]), vec![]));
            }
        }
        PromptStyle::Quote => {
            // quiet: the task in italics, dimmed, no chrome
            for (l, cont) in md.into_iter().take(PROMPT_LINES) {
                let spans = l.spans.into_iter().map(|sp| Span::styled(sp.content, st(th.dim).add_modifier(Modifier::ITALIC))).collect::<Vec<_>>();
                out.push((Line::from(spans), cont));
            }
            if more > 0 {
                out.push((Line::from(Span::styled(format!("… {more} more lines"), st(th.faint))), vec![]));
            }
        }
    }
    out
}

fn tool_lines(t: &Tool, s: &Session, v: &View, th: &Theme, o: &Opts) -> Vec<Prefixed> {
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
        (Some(_), "write") => format!("{} lines", arg("content").lines().count()),
        (Some(r), _) => format!("{} lines", r.lines().count()),
        (None, _) => String::new(),
    };
    if let (true, Some(secs)) = (o.durations, t.secs()) {
        meta.push_str(&format!(" · {secs:.1}s"));
    }
    let gutter = || vec![Span::raw("    "), Span::styled("│ ", st(th.faint))];
    let mut out = vec![(
        Line::from(vec![
            Span::styled(format!("{glyph} "), st(gcolor)),
            Span::styled(format!("{name:<6}"), st(th.accent2).add_modifier(Modifier::BOLD)),
            Span::styled(format!(" {}", tool_summary(t, s)), st(th.dim)),
            Span::styled(if meta.is_empty() { String::new() } else { format!("  · {meta}") }, st(th.faint)),
        ]),
        vec![Span::raw("         ")],
    )];
    let body: Vec<(String, Color)> = match name {
        // edits always show their diff: it is the most useful thing on screen
        "edit" => arg("old").lines().map(|l| (format!("- {l}"), th.err)).chain(arg("new").lines().map(|l| (format!("+ {l}"), th.ok))).collect(),
        "write" if v.expand_all => arg("content").lines().map(|l| (format!("+ {l}"), th.ok)).collect(),
        _ if v.expand_all => t.result.as_deref().unwrap_or_default().lines().map(|l| (l.to_string(), if t.error { th.err } else { th.dim })).collect(),
        _ => vec![],
    };
    let limit = if name == "edit" && !v.expand_all { TOOL_LINES } else { TOOL_LINES * 2 };
    for (l, c) in body.iter().take(limit) {
        let mut spans = gutter();
        spans.push(Span::styled(l.replace('\t', "    "), st(*c)));
        out.push((Line::from(spans), gutter()));
    }
    if body.len() > limit {
        let mut spans = gutter();
        spans.push(Span::styled(format!("… +{} lines", body.len() - limit), st(th.faint)));
        out.push((Line::from(spans), gutter()));
    }
    out
}

/// One quiet line for a run of tool calls: "⌁ ran 4 commands · bash ×3 read ×1 · 210 lines".
fn group_lines(blocks: &[Block], s: &Session, v: &View, th: &Theme) -> Vec<Prefixed> {
    let tools: Vec<&Tool> = blocks.iter().filter_map(|b| if let Block::Tool(t) = b { Some(t) } else { None }).collect();
    let thought: u64 = blocks.iter().map(|b| if let Block::Thinking { tokens, .. } = b { *tokens } else { 0 }).sum();
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
        Span::styled(if tools.is_empty() { String::new() } else { format!("   {mix}   {lines} lines") }, st(th.faint)),
    ];
    if thought > 0 {
        spans.push(Span::styled(format!("   thought ~{thought} tok"), st(th.faint)));
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
fn markdown(text: &str, th: &Theme) -> Vec<Prefixed> {
    let mut out = Vec::new();
    let mut in_code = false;
    for raw in text.lines() {
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
            if !seg.is_empty() {
                spans.push(Span::styled(seg.to_string(), style));
            }
        }
    }
    spans
}

/// Word-wraps a styled line to `width`, starting continuation rows with `cont`.
pub fn wrap(line: Line<'static>, width: usize, cont: &[Span<'static>]) -> Vec<Line<'static>> {
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
const LIVE_KEYS: &str = "enter send · ctrl-j new line · /good /bad rate run · ctrl-c stop/quit · ctrl-y copy · ctrl-s select · ctrl-t design · ctrl-e expand";

/// The composer row: typed text with a cursor in live mode, a placeholder and key hints otherwise.
pub fn composer_line(v: &View, th: &Theme, glyph: &str, placeholder: &str, width: u16) -> Line<'static> {
    let mut spans = vec![Span::styled(format!(" {glyph} "), st(th.accent).add_modifier(Modifier::BOLD))];
    if v.live && !v.input.is_empty() {
        // one row (zen's footer): the end of what is being typed, newlines as ⏎
        let text = v.input.replace('\n', " ⏎ ");
        let room = (width as usize).saturating_sub(6);
        let shown: String = text.chars().rev().take(room).collect::<Vec<_>>().into_iter().rev().collect();
        spans.push(Span::styled(shown, st(th.text)));
        spans.push(Span::styled("▍", st(th.accent)));
    } else {
        if v.live {
            spans.push(Span::styled("▍", st(th.accent)));
        }
        spans.push(Span::styled(placeholder.to_string(), st(th.faint)));
        spans.push(Span::styled(format!("   {}", if v.live { LIVE_KEYS } else { REPLAY_KEYS }), st(th.faint)));
    }
    Line::from(spans)
}

/// Rows the composer box may grow to while typing (multi-line input); the latest rows stay visible.
const COMPOSER_MAX_ROWS: usize = 6;

/// The composer box: typed text on real lines (wrapped to `width`, the box's inside), or the one-line placeholder.
pub fn composer_rows(v: &View, th: &Theme, glyph: &str, placeholder: &str, width: u16) -> Vec<Line<'static>> {
    if !(v.live && !v.input.is_empty()) {
        return vec![composer_line(v, th, glyph, placeholder, width)];
    }
    let room = (width as usize).saturating_sub(4).max(1);
    let mut rows: Vec<String> = Vec::new();
    for line in v.input.split('\n') {
        let mut row = String::new();
        let mut w = 0;
        for c in line.chars() {
            let cw = c.width().unwrap_or(0);
            if w + cw > room {
                rows.push(std::mem::take(&mut row));
                w = 0;
            }
            row.push(c);
            w += cw;
        }
        rows.push(row);
    }
    let skip = rows.len().saturating_sub(COMPOSER_MAX_ROWS);
    let last = rows.len() - 1;
    rows.into_iter()
        .enumerate()
        .skip(skip)
        .map(|(i, r)| {
            let lead = if i == 0 { format!(" {glyph} ") } else { "   ".to_string() };
            let mut spans = vec![Span::styled(lead, st(th.accent).add_modifier(Modifier::BOLD)), Span::styled(r, st(th.text))];
            if i == last {
                spans.push(Span::styled("▍", st(th.accent)));
            }
            Line::from(spans)
        })
        .collect()
}

/// Height of a bordered composer box for this input at this outer width.
pub fn composer_height(v: &View, outer_width: u16) -> u16 {
    if !(v.live && !v.input.is_empty()) {
        return 3;
    }
    let room = (outer_width as usize).saturating_sub(2 + 4).max(1);
    let rows: usize = v.input.split('\n').map(|l| l.width().div_ceil(room).max(1)).sum();
    rows.clamp(1, COMPOSER_MAX_ROWS) as u16 + 2
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
    fn claude_and_lean_edits_read_the_same() {
        let tool = |name: &str, input: serde_json::Value| Tool { id: String::new(), name: name.into(), input, result: None, error: false, started_ms: 0.0, done_ms: None };
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
    fn composer_grows_with_lines_and_keeps_the_latest() {
        let mut v = View { design: Design::Cockpit, expand_all: false, scroll: 0, clock_ms: 0.0, speed: 1.0, paused: false, live: true, input: String::new() };
        let th = Design::Cockpit.theme();
        assert_eq!(composer_height(&v, 40), 3, "empty: one row");
        v.input = "first\nsecond".into();
        assert_eq!(composer_height(&v, 40), 4);
        let rows = composer_rows(&v, th, "›", "", 38);
        let text: Vec<String> = rows.iter().map(|r| r.spans.iter().map(|s| s.content.to_string()).collect()).collect();
        assert_eq!(text, vec![" › first", "   second▍"]);
        v.input = "x".repeat(70); // wraps at 34 columns: 3 rows
        assert_eq!(composer_height(&v, 40), 5);
        assert_eq!(composer_rows(&v, th, "›", "", 38).len(), 3);
        v.input = (1..=10).map(|i| i.to_string()).collect::<Vec<_>>().join("\n");
        assert_eq!(composer_height(&v, 40), 8, "capped at 6 rows");
        let rows = composer_rows(&v, th, "›", "", 38);
        assert_eq!(rows.len(), 6);
        assert!(rows[5].spans.iter().any(|s| s.content == "10"), "the latest line stays visible");
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
