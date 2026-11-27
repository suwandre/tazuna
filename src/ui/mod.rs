//! Rendering. Two designs (zen and cockpit) share one transcript builder, one palette and one overlay frame, and differ
//! in chrome and layout. Until 0.1.193 there were four: aurora and timeline went (design review 11).

mod cockpit;
mod zen;

use crate::session::{Activity, Block, Located, Session, Tool, change_counts, common_ends};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use tachyonfx::{ColorSpace, Effect, Interpolation::*, fx};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// The colours, named as the desktop app's tokens (desktop/DESIGN.md) so both apps read as one. How they are used:
/// `accent` for the live state or the thing to act on, `ok` and `err` for results only, `warn` for limits, risk and a
/// question that waits for you; the text levels are `text`, `ink2` (names and labels), `dim` and `faint`.
pub struct Theme {
    /// The background colours are mixed from, and the ink on the accent's tile.
    pub bg: Color,
    /// What the screen is painted with: `bg`, or the terminal's own (Color::Reset) with `--theme terminal`.
    pub canvas: Color,
    pub surface: Color,
    /// Behind code, as the desktop's `field`.
    pub field: Color,
    pub border: Color,
    pub text: Color,
    pub ink2: Color,
    pub dim: Color,
    pub faint: Color,
    pub accent: Color,
    pub warn: Color,
    pub err: Color,
    pub ok: Color,
}

const fn hex(c: u32) -> Color {
    Color::Rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

// One palette for both designs: they differ in arrangement, not hue. Until 0.1.184 each had its own (navy and cyan,
// teal and violet, grey and pink, neon magenta and yellow), none of them the desktop app's (design review, 2026-11-19).
// bg canvas, surface card, border line-strong (the hairline `line` is lost on a terminal's cell grid), dim muted.
const DARK: Theme = Theme {
    bg: hex(0x171613), canvas: hex(0x171613), surface: hex(0x211f1b), field: hex(0x2a2823), border: hex(0x3a3731), text: hex(0xece8df), ink2: hex(0xd2cdc2),
    dim: hex(0xafaa9e), faint: hex(0x9a958d), accent: hex(0x8f9bf0), warn: hex(0xe0b46a), err: hex(0xe08b7f), ok: hex(0x7cc08f),
};
const LIGHT: Theme = Theme {
    bg: hex(0xf6f4ef), canvas: hex(0xf6f4ef), surface: hex(0xfffefb), field: hex(0xefebe3), border: hex(0xd8d2c5), text: hex(0x23221e), ink2: hex(0x3f3c35),
    dim: hex(0x5b574f), faint: hex(0x69655d), accent: hex(0x3d4aa8), warn: hex(0xa76a14), err: hex(0xb4473a), ok: hex(0x3b7d4f),
};

/// The light palette (`--theme light`, or a light terminal), and a terminal that does not say it takes 24-bit colour: set
/// once at start (`setup_colors`).
static LIGHT_ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static INDEXED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// `--theme terminal`: the palette on the terminal's own background, built once at start.
static TERMINAL: std::sync::OnceLock<Theme> = std::sync::OnceLock::new();

/// The theme, and whether colours go out as 24-bit: only where the terminal says so (COLORTERM truecolor or 24bit;
/// macOS Terminal before 26 sets none and shows 24-bit colours wrong), else as the nearest of the 256. `theme` is
/// `--theme`'s word; `bg` the background the terminal reported (`terminal_background`): auto and terminal take light or
/// dark from it (design review 13: a light terminal opened in the dark palette until `--theme light`).
pub fn setup_colors(theme: &str, bg: Option<(u8, u8, u8)>) {
    use std::sync::atomic::Ordering::Relaxed;
    let light = light_for(theme, bg);
    LIGHT_ON.store(light, Relaxed);
    INDEXED.store(!matches!(std::env::var("COLORTERM").as_deref(), Ok("truecolor" | "24bit")), Relaxed);
    if theme == "terminal" {
        let _ = TERMINAL.set(on_terminal(if light { &LIGHT } else { &DARK }, bg));
    }
}

/// The light palette for `--theme light`, and for auto and terminal on a light background (its perceived brightness past
/// half); dark without an answer.
fn light_for(theme: &str, bg: Option<(u8, u8, u8)>) -> bool {
    match theme {
        "light" => true,
        "dark" => false,
        _ => bg.is_some_and(|(r, g, b)| 299 * r as u32 + 587 * g as u32 + 114 * b as u32 > 127_500),
    }
}

/// `base` on the terminal's own background: the canvas left to the terminal, so its colour and padding show no frame
/// around tazuna; panels, bands and pills on a surface a step from what it reported. A terminal that did not say (Windows,
/// screen, mosh, tmux before 3.3) keeps the theme's canvas: under its unknown colour a band could vanish or glare.
fn on_terminal(base: &Theme, bg: Option<(u8, u8, u8)>) -> Theme {
    let Some((r, g, b)) = bg else { return Theme { ..*base } };
    let ground = Color::Rgb(r, g, b);
    Theme { bg: ground, canvas: Color::Reset, surface: lerp(ground, base.text, 0.06), field: lerp(ground, base.text, 0.10), ..*base }
}

/// What the terminal said when asked at start (`terminal_background`): its background, the keys typed while it answered
/// (read with the answers, so they go into the message box), and whether it left the question open: an answer may still
/// come, and the live screen drops it if it does.
#[derive(Default)]
pub struct Probe {
    pub bg: Option<(u8, u8, u8)>,
    pub typed: String,
    /// An Enter typed after them: the draft is sent, as an Enter typed while tazuna starts sends it (0.1.194).
    pub enter: bool,
    /// Ctrl-C among them: tazuna ends before its screen opens, as it would have on that key.
    pub quit: bool,
    pub unanswered: bool,
}

/// The terminal's background as it reports it (OSC 11), for `--theme auto|terminal`. A device-attributes query goes
/// behind it: terminals answer that one, in order, so one that keeps quiet about its background (tmux before 3.3) is known
/// at once; one that answers neither gets 150 ms, over ssh a second. Asked in raw mode, so no answer is echoed. Not asked
/// at all while keys wait in the input: they would be read with the answers (type-ahead kept since 0.1.194); keys that
/// come during the wait are handed back in `typed`. On stdin, not /dev/tty: macOS's poll() reports /dev/tty invalid at
/// once, and a read then waited for ever on a terminal that never answers.
#[cfg(unix)]
pub fn terminal_background() -> Probe {
    use std::io::Write;
    let ready = |ms: i32| {
        let mut p = libc::pollfd { fd: 0, events: libc::POLLIN, revents: 0 };
        // SAFETY: one pollfd on stdin
        unsafe { libc::poll(&mut p, 1, ms) > 0 && p.revents & libc::POLLIN != 0 }
    };
    // SAFETY: plain libc calls on stdin's descriptor; termios is plain data, filled by tcgetattr before use
    unsafe {
        if libc::isatty(0) != 1 || libc::isatty(1) != 1 || ready(0) {
            return Probe::default();
        }
        let mut old: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(0, &mut old) != 0 {
            return Probe::default();
        }
        let mut raw = old;
        libc::cfmakeraw(&mut raw);
        libc::tcsetattr(0, libc::TCSANOW, &raw);
        let mut out = std::io::stdout();
        let _ = out.write_all(b"\x1b]11;?\x1b\\\x1b[c").and_then(|_| out.flush());
        let remote = std::env::var_os("SSH_CONNECTION").is_some() || std::env::var_os("SSH_TTY").is_some();
        let end = std::time::Instant::now() + std::time::Duration::from_millis(if remote { 1000 } else { 150 });
        let (mut got, mut answered) = (Vec::new(), false);
        while !answered {
            let left = end.saturating_duration_since(std::time::Instant::now()).as_millis() as i32;
            if left <= 0 || !ready(left) {
                break;
            }
            let mut buf = [0u8; 256];
            match libc::read(0, buf.as_mut_ptr().cast(), buf.len()) {
                n if n > 0 => got.extend_from_slice(&buf[..n as usize]),
                _ => break,
            }
            answered = attributes_answered(&got);
        }
        libc::tcsetattr(0, libc::TCSANOW, &old);
        Probe { unanswered: !answered, ..split_answers(&got) }
    }
}

#[cfg(not(unix))]
pub fn terminal_background() -> Probe {
    Probe::default()
}

/// What came in while the terminal was asked: its background (OSC 11) and the keys typed meanwhile, as the message box
/// would have taken them: the answers and any other escape sequence (an arrow key's) dropped whole, backspace applied, a
/// last Enter sending the draft (`Probe::enter`) and Ctrl-C ending tazuna (`Probe::quit`).
fn split_answers(got: &[u8]) -> Probe {
    let (bg, text) = (osc11_colour(got), String::from_utf8_lossy(got));
    let (mut typed, mut quit, mut c) = (String::new(), false, text.chars().peekable());
    while let Some(ch) = c.next() {
        match ch {
            '\x1b' => match c.next() {
                // OSC (the background's answer): to BEL or ST
                Some(']') => while let Some(x) = c.next() {
                    if x == '\x07' || (x == '\x1b' && c.next_if_eq(&'\\').is_some()) {
                        break;
                    }
                },
                // CSI (the device attributes' answer, an arrow key): to its final byte
                Some('[') => while c.next().is_some_and(|x| !('\x40'..='\x7e').contains(&x)) {},
                // SS3 (an arrow key in application mode), or Alt and a key: one more
                Some('O') => _ = c.next(),
                _ => {}
            },
            '\x7f' | '\x08' => _ = typed.pop(),
            '\x03' => quit = true,
            '\r' | '\n' => typed.push('\n'),
            x if !x.is_control() => typed.push(x),
            _ => {}
        }
    }
    let enter = typed.ends_with('\n');
    Probe { bg, typed: typed.trim_end_matches('\n').to_string(), enter, quit, unanswered: false }
}

/// The device-attributes answer (`ESC [ ? 62;22 c`) is in: nothing more is coming.
fn attributes_answered(got: &[u8]) -> bool {
    got.windows(3).enumerate().filter(|(_, w)| w == b"\x1b[?").any(|(i, _)| {
        got[i + 3..].iter().position(|b| !(b.is_ascii_digit() || *b == b';')).is_some_and(|k| got[i + 3 + k] == b'c')
    })
}

/// The colour in an OSC 11 answer, `ESC ] 11 ; rgb:RRRR/GGGG/BBBB` (1 to 4 hex digits a channel) ended by BEL or ST.
fn osc11_colour(got: &[u8]) -> Option<(u8, u8, u8)> {
    let text = String::from_utf8_lossy(got);
    let rest = &text[text.find("]11;")? + 4..];
    let body = rest.split(['\x07', '\x1b']).next()?;
    let mut ch = body.strip_prefix("rgb:").or_else(|| body.strip_prefix("rgba:"))?.split('/').map(|h| {
        let v = u32::from_str_radix(h, 16).ok().filter(|_| (1..=4).contains(&h.len()))?;
        Some((v * 255 / ((1 << (4 * h.len())) - 1)) as u8)
    });
    Some((ch.next()??, ch.next()??, ch.next()??))
}

/// The frame's colours as the nearest of the 256, where the terminal takes no more. On the drawn cells, not on the
/// theme: the effects mix colours of their own.
pub fn fit_colors(buf: &mut ratatui::buffer::Buffer) {
    use std::sync::atomic::Ordering::Relaxed;
    if INDEXED.load(Relaxed) {
        fit_to_256(buf, Design::Zen.theme(), LIGHT_ON.load(Relaxed));
    }
}

/// The colours a frame mixes for the diff's bands and marks and for a pill's ground, and the cube colour each takes in 256
/// colours ([dark, light]): the nearest of the 256 rounded a 16% tint into the background's own grey, so the bands and the
/// pills vanished (design review 13).
fn tints(th: &Theme) -> [(Color, [u8; 2]); 6] {
    [(lerp(th.bg, th.err, 0.16), [52, 224]), (lerp(th.bg, th.ok, 0.16), [22, 194]), (lerp(th.bg, th.err, 0.38), [88, 217]),
        (lerp(th.bg, th.ok, 0.38), [28, 157]), (lerp(th.bg, th.warn, 0.18), [58, 223]), (th.surface, [236, 254])]
}

fn fit_to_256(buf: &mut ratatui::buffer::Buffer, th: &Theme, light: bool) {
    let tints = tints(th);
    let fit = |c: Color| match (tints.iter().find(|(t, _)| *t == c), c) {
        (Some((_, cube)), _) => Color::Indexed(cube[usize::from(light)]),
        (None, Color::Rgb(r, g, b)) => Color::Indexed(to_256(r, g, b)),
        _ => c,
    };
    for cell in &mut buf.content {
        (cell.fg, cell.bg) = (fit(cell.fg), fit(cell.bg));
    }
}

/// The nearest xterm colour: of the 6x6x6 cube (16-231) or the 24 greys (232-255), whichever is closer.
fn to_256(r: u8, g: u8, b: u8) -> u8 {
    const STEPS: [i32; 6] = [0, 95, 135, 175, 215, 255];
    let near = |x: u8| (0..6).min_by_key(|&i| (STEPS[i] - x as i32).abs()).unwrap_or(0);
    let off = |x: i32, y: i32, z: i32| (x - r as i32).pow(2) + (y - g as i32).pow(2) + (z - b as i32).pow(2);
    let (ri, gi, bi) = (near(r), near(g), near(b));
    let grey = ((r as i32 + g as i32 + b as i32) / 3 - 3).clamp(0, 235) / 10;
    let level = 8 + 10 * grey.min(23);
    if off(level, level, level) < off(STEPS[ri], STEPS[gi], STEPS[bi]) { (232 + grey.min(23)) as u8 } else { (16 + 36 * ri + 6 * gi + bi) as u8 }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Design {
    Cockpit,
    Zen,
}

impl Design {
    /// One palette for every design (they differ in arrangement); the light one on a light terminal or with `--theme
    /// light`, on the terminal's own background with `--theme terminal`.
    pub fn theme(self) -> &'static Theme {
        TERMINAL.get().unwrap_or(if LIGHT_ON.load(std::sync::atomic::Ordering::Relaxed) { &LIGHT } else { &DARK })
    }

    /// `--design N`, a replay's digit and the saved choice. The numbers are the ones scripts know from the four designs:
    /// 2 cockpit, 3 zen; 1 (aurora) and 4 (timeline) are gone since 0.1.193 and open zen (`gone` names them).
    pub fn from_number(n: usize) -> Option<Design> {
        match n {
            2 => Some(Design::Cockpit),
            1 | 3 | 4 => Some(Design::Zen),
            _ => None,
        }
    }

    /// The design number `n` named until 0.1.193, when it opens zen now: said once, quietly.
    pub fn gone(n: usize) -> Option<&'static str> {
        match n {
            1 => Some("aurora"),
            4 => Some("timeline"),
            _ => None,
        }
    }

    /// The words for that, in the message box for a moment, with the key that switches here (`key`: ctrl-t in a
    /// session, tab in a replay, where ctrl-t does nothing).
    pub fn gone_note(name: &str, key: &str) -> String {
        format!("{name} is gone: this is zen · {key}: cockpit")
    }

    pub fn number(self) -> usize {
        match self {
            Design::Cockpit => 2,
            Design::Zen => 3,
        }
    }

    /// The other one (ctrl-t).
    pub fn next(self) -> Design {
        match self {
            Design::Cockpit => Design::Zen,
            Design::Zen => Design::Cockpit,
        }
    }

    /// Its name, said once when ctrl-t switches to it (the names left the screens' headers in design review 10).
    pub fn name(self) -> &'static str {
        match self {
            Design::Cockpit => "cockpit",
            Design::Zen => "zen",
        }
    }

    /// The design the terminal opens in without --design: the last one chosen with ctrl-t, else Zen: a transcript and
    /// one quiet row, as Claude Code, Codex, pi and OpenCode open (design review 9, the user's choice 2026-11-20; until
    /// 0.1.189 Cockpit for someone with a run log and Aurora on a fresh install, and until 0.1.179 always Cockpit). A
    /// choice made with ctrl-t stays. With it, the design a saved 1 or 4 named (now zen; the file then says 3, so this is
    /// said once).
    pub fn saved() -> (Design, Option<&'static str>) {
        crate::labels::dir().map_or((Design::Zen, None), |d| Design::saved_in(&d))
    }

    fn saved_in(dir: &std::path::Path) -> (Design, Option<&'static str>) {
        let n: Option<usize> = std::fs::read_to_string(dir.join("design")).ok().and_then(|t| t.trim().parse().ok());
        (n.and_then(Design::from_number).unwrap_or(Design::Zen), n.and_then(Design::gone))
    }

    /// A saved 1 or 4 becomes zen's 3, so the note about it is said once: called when a session or a replay opens its
    /// screen. Reading the choice wrote it until review 11, so `--run`, `--help` and the window's app-server (which never
    /// show the note) used it up.
    pub fn settle() {
        if let Ok(dir) = crate::labels::dir() {
            Design::settle_in(&dir);
        }
    }

    fn settle_in(dir: &std::path::Path) {
        if Design::saved_in(dir).1.is_some() {
            Design::Zen.save_in(dir);
        }
    }

    pub fn save(self) {
        if let Ok(dir) = crate::labels::dir() {
            self.save_in(&dir);
        }
    }

    fn save_in(self, dir: &std::path::Path) {
        let _ = std::fs::create_dir_all(dir).and_then(|_| std::fs::write(dir.join("design"), format!("{}\n", self.number())));
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
    /// Tab completions for the word at the cursor (`@` files).
    pub hints: Vec<String>,
    /// The command menu over the message box while it holds only `/...`: each command and what it does, and the one
    /// selected (up and down move, enter runs it, tab puts it in the box).
    pub menu: Vec<(String, String)>,
    pub menu_sel: usize,
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
    /// "y" chose "allow once" on a destructive command auto asked about: enter confirms it, any other key lets it go (a
    /// message typed into the question can start with "y", design review 13).
    pub confirm: bool,
    /// The question in front is a destructive command auto asked about (live.rs risky), whichever note it shows.
    pub ask_risky: bool,
    /// Vim mode's state for the composer ("NORMAL", "INSERT"); empty when vim mode is off.
    pub vim: &'static str,
    /// The session is on Claude and Claude cannot run (not installed, signed out): what the status says in place of "ready"
    /// (design review 14).
    pub cant_run: Option<&'static str>,
}

/// `/login`: the providers (name, its state now), and a provider's page while its key is typed.
pub struct LoginPanel {
    /// Each provider: its name, its state, the line on its terms (empty: none). How each logs in went with 0.1.196's
    /// longer names: an API key for all but Claude, whose state says it, and the column cut the state at 80 columns.
    pub rows: Vec<(String, String, String)>,
    pub selected: usize,
    pub form: Option<LoginForm>,
    /// What was typed to find a provider (the rows are the ones it finds).
    pub filter: String,
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
    /// The line on its terms (`models::Login::note`); empty: none.
    pub note: String,
    /// A sign-in instead of a key (oauth.rs): how it goes; empty for a key.
    pub sign_in: String,
    /// The sign-in's warning (its note) not confirmed yet: the enter that signs in confirms it.
    pub confirm: bool,
    /// Another account can take its place (ChatGPT): `a`.
    pub other: bool,
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
    /// The left pane, OMP-style: "All models" then each provider (name, how many models, usable now, why not and the line
    /// on its terms), the one
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
    /// Whose it is, said after its name in faint where the list mixes providers ("Mistral"); empty: not said.
    pub provider: String,
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
    /// The cost panel (cockpit): a click there opens /usage, every account.
    pub cost: Option<Rect>,
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
    /// The open list's last scroll that still fills its panel: the caller holds the list's scroll to it.
    pub list_top: Option<usize>,
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
/// How a list row is drawn: plain, failed (red), a section's heading, a note (dimmed), or a line across the panel.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Tone {
    #[default]
    Plain,
    Bad,
    Head,
    Dim,
    Rule,
}

pub struct ListPanel {
    pub title: String,
    /// The column heads, drawn above the rows.
    pub head: String,
    pub rows: Vec<(String, Tone)>,
    pub scroll: usize,
    /// A row's link (a fetched page): its text from `link_from` on is drawn as a link, a click opens it.
    pub links: Vec<Option<String>>,
    pub link_from: usize,
    /// Columns scrolled to the right (←/→, shift+wheel): long rows ran off the modal with no way to read them.
    pub hscroll: usize,
    /// A row's whole text (a long command), unfolded under it when the row is clicked; rows show cut to the modal.
    pub details: Vec<Option<String>>,
    pub open: std::collections::BTreeSet<usize>,
    /// Still being read (/usage asks several accounts): the first row turns with a spinner until the rows come.
    pub pending: bool,
}

/// `/diff` (or a click on the files panel): the changed files, the selected one's diff, how far it is scrolled.
pub struct DiffPanel {
    pub selected: usize,
    pub diff: Vec<String>,
    pub scroll: usize,
}

pub fn draw(f: &mut Frame, s: &Session, v: &View) -> Areas {
    let th = v.design.theme();
    f.render_widget(ratatui::widgets::Block::new().style(Style::new().bg(th.canvas)), f.area());
    let areas = match v.design {
        Design::Cockpit => cockpit::draw(f, s, v, th),
        Design::Zen => zen::draw(f, s, v, th),
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
    if v.tasks.is_some() || v.rewind.is_some() || v.sessions.is_some() || v.diff.is_some() || v.models.is_some() || v.list.is_some() || v.login.is_some() {
        let area = f.area();
        let buf = f.buffer_mut();
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                if let Some(c) = buf.cell_mut((x, y)) {
                    c.set_style(Style::new().fg(th.border).bg(th.canvas).remove_modifier(Modifier::all()));
                }
            }
        }
    }
    // where every overlay opens: one column (zen's own; in cockpit the same width over the screen's middle), over the
    // transcript's rows, so the message box stays in sight; a screen too short for that gives all of itself. Until
    // 0.1.193 each overlay had a width and a place of its own (five of them, design review 11).
    let full = f.area();
    let w = full.width.saturating_sub(4).min(zen::MAX_WIDTH).max(full.width.min(20));
    let band = if areas.transcript.height >= 8 { Rect { x: full.x + (full.width - w) / 2, width: w, y: areas.transcript.y, height: areas.transcript.height } } else { full };
    // the box holds only `/…`: its commands over it, or that none matches (the menu just went)
    let slash = v.asking.is_none() && v.input.len() > 1 && v.input.starts_with('/') && !v.input.contains(char::is_whitespace);
    if v.live && (!v.menu.is_empty() || (slash && v.hints.is_empty())) {
        draw_menu(f, v, th, Rect { x: full.x + (full.width - w) / 2, width: w, y: areas.transcript.y, height: areas.transcript.height });
    }
    if let Some(p) = &v.tasks {
        areas.task_rows = draw_tasks(f, p, th, band);
    }
    if let Some(p) = &v.rewind {
        draw_rewind(f, p, th, band);
    }
    if let Some(p) = &v.sessions {
        draw_sessions(f, p, th, band);
    }
    if let Some(p) = &v.diff {
        areas.diff_files = draw_diff(f, &v.changed, p, th, v.mouse, band);
    }
    if let Some(p) = &v.models {
        draw_models(f, p, th, band);
    }
    if let Some(p) = &v.login {
        draw_login(f, p, th, band);
    }
    if let Some(p) = &v.list {
        let top;
        (areas.list, areas.list_links, areas.list_rows, top) = draw_list(f, p, th, v.clock_ms, band);
        areas.list_top = Some(top);
    }
    areas
}

/// One frame for everything that opens over the screen: a grey border, the title on its left (bold; what follows a
/// " · " in it faint), a count on its right, the keys along the bottom (whole ones only, most needed first). Returns its
/// inside. There were three styles: a grey border with the keys in its title, an accent border with the keys in a last
/// row, an accent border with the keys bold in the title (design review 11, desktop/mockups/v8.html).
fn overlay(f: &mut Frame, th: &Theme, area: Rect, title: &str, count: &str, keys: &str) -> Rect {
    use ratatui::widgets::{Block, Borders, Clear};
    f.render_widget(Clear, area);
    let room = (area.width as usize).saturating_sub(4);
    let (name, more) = title.split_once(" · ").unwrap_or((title, ""));
    // the count only where the title's name fits whole beside it
    let count = if name.width() + count.width() + 4 <= room { count } else { "" };
    let left = room.saturating_sub(if count.is_empty() { 0 } else { count.width() + 4 });
    let name = ellipsis(name, left);
    let more = if more.is_empty() || left < name.width() + 6 { String::new() } else { format!("  {}", ellipsis(more, left - name.width() - 2)) };
    let mut block = Block::new().borders(Borders::ALL).border_style(st(th.border)).style(Style::new().bg(th.canvas))
        .title_top(Line::from(vec![Span::raw(" "), Span::styled(name, st(th.text).add_modifier(Modifier::BOLD)), Span::styled(more, st(th.faint)), Span::raw(" ")]));
    if !count.is_empty() {
        block = block.title_top(Line::from(vec![Span::styled(format!(" {count} "), st(th.faint)), Span::styled("─", st(th.border))]).right_aligned());
    }
    let keys = match keys.rsplit_once(" · ").filter(|(_, last)| last.starts_with("esc ")) {
        // the way out is said last and always: at 40 columns "↑↓ move · enter runs" left no room for it
        Some((first, esc)) if esc.width() <= room => [whole_hints(first, room.saturating_sub(esc.width() + 3)), esc.to_string()].into_iter().filter(|k| !k.is_empty()).collect::<Vec<_>>().join(" · "),
        _ => whole_hints(keys, room),
    }.replace(" · ", "   ");
    if !keys.is_empty() {
        block = block.title_bottom(Line::from(Span::styled(format!(" {keys} "), st(th.faint))));
    }
    let inner = block.inner(area);
    f.render_widget(block, area);
    inner
}

/// An overlay's place in the band: `rows` of content high (as many as fit), from the band's top; a completion menu sits
/// at its bottom, over the message box.
fn overlay_area(band: Rect, rows: usize, bottom: bool) -> Rect {
    let h = (rows.min(u16::MAX as usize - 2) as u16 + 2).min(band.height);
    Rect { y: if bottom { band.bottom() - h } else { band.y }, height: h, ..band }
}

/// The selected row of every list: the accent as a fill, bold; each list marks it with `›` too, so it shows without
/// colour. The model list's was the row's own ink reversed, /rewind's the accent without a fill (design review 11).
fn picked(th: &Theme) -> Style {
    Style::new().fg(th.accent).bg(th.bg).add_modifier(Modifier::REVERSED | Modifier::BOLD)
}

/// The command menu, just over the message box (the transcript's bottom): up to 10 rows, the selected one marked and
/// kept in view.
fn draw_menu(f: &mut Frame, v: &View, th: &Theme, over: Rect) {
    if over.height < 4 || over.width < 20 {
        return;
    }
    let none = v.menu.is_empty();
    // Claude's commands after tazuna's, under a heading of their own (editor::command_menu): one row more
    let claude_at = v.menu.iter().position(|(_, d)| crate::editor::claude_row(d).is_some());
    let area = overlay_area(over, if none { 1 } else { (v.menu.len() + claude_at.is_some() as usize).min(10) }, true);
    let sel = v.menu_sel.min(v.menu.len().saturating_sub(1));
    let count = if none { String::new() } else { format!("{}/{}", sel + 1, v.menu.len()) };
    // esc clears the draft, and the menu goes with it ("esc closes" promised the draft would stay, design review 11)
    let inner = overlay(f, th, area, "commands", &count, if none { "esc clears" } else { "↑↓ move · enter runs · tab fills in · esc clears" });
    let w = inner.width as usize;
    if none {
        // the menu just went when nothing matched, as if the box had stopped completing (design review 11)
        let said = Line::from(vec![Span::styled(" no command matches ", st(th.faint)), Span::styled(ellipsis(&plain(&v.input), w.saturating_sub(21)), st(th.text))]);
        f.render_widget(Paragraph::new(said), inner);
        return;
    }
    // by the rows the box shows, which may be fewer than the list's ten: in a short window the selected row scrolled out
    // of sight under the border (design review 11)
    let shown = inner.height as usize;
    // the rows drawn: a command by its index, the heading as None
    let rows: Vec<Option<usize>> = (0..v.menu.len()).flat_map(|i| [(claude_at == Some(i)).then_some(None), Some(Some(i))]).flatten().collect();
    let at = rows.iter().position(|r| *r == Some(sel)).unwrap_or(0);
    let first = at.saturating_sub(shown.saturating_sub(1));
    let name_w = v.menu.iter().map(|(c, _)| c.width()).max().unwrap_or(8).min(24) + 3;
    let lines: Vec<Line> = rows.iter().skip(first).take(shown).map(|r| {
        let Some(i) = *r else { return Line::from(Span::styled(" Claude", st(th.faint).add_modifier(Modifier::BOLD))) };
        let (c, d) = &v.menu[i];
        // "Clear the conversation", as tazuna's own read
        let d: String = match crate::editor::claude_row(d) {
            Some(r) => r.chars().take(1).flat_map(char::to_uppercase).chain(r.chars().skip(1)).collect(),
            None => d.clone(),
        };
        let on = i == sel;
        // the selected row has a mark too, as the model and session lists: with NO_COLOR the terminal library resets
        // every attribute with its (empty) colour, so the fill alone left no row marked (design review 11)
        let name: String = format!("{}{c:<n$}", if on { "›" } else { " " }, n = name_w - 1).chars().take(name_w).collect();
        // a description cut short ends in "…" (cockpit's narrower menu cut "(enable, disca" with nothing, design review 10)
        let about = ellipsis(&d, w.saturating_sub(name_w + 1));
        let pad = " ".repeat(w.saturating_sub(name.width() + about.width()));
        let style = if on { picked(th) } else { st(th.text).add_modifier(Modifier::BOLD) };
        Line::from(vec![Span::styled(name, style), Span::styled(about, if on { style.remove_modifier(Modifier::BOLD) } else { st(th.dim) }), Span::styled(pad, style)])
    }).collect();
    f.render_widget(Paragraph::new(lines), inner);
}

/// The hints of `keys` (apart by " · ") that fit whole in `room` columns, from the first on: a row cut at the edge read
/// "enter ch" and "tab provi" (design review 11).
fn whole_hints(keys: &str, room: usize) -> String {
    let mut out = String::new();
    for k in keys.split(" · ") {
        let next = if out.is_empty() { k.to_string() } else { format!("{out} · {k}") };
        if next.trim_end().width() > room {
            break;
        }
        out = next;
    }
    out
}

/// A meter's row with its bar (█ used, ░ left) `by` cells shorter, at least 6, the used share kept.
fn shrink_bar(r: &str, by: usize) -> String {
    let n = r.chars().filter(|c| matches!(c, '█' | '░')).count();
    let to = n.saturating_sub(by).max(6.min(n));
    let full = (r.chars().filter(|&c| c == '█').count() * to + n / 2) / n.max(1);
    let (mut out, mut done) = (String::new(), false);
    for c in r.chars() {
        if !matches!(c, '█' | '░') {
            out.push(c);
        } else if !std::mem::replace(&mut done, true) {
            out.push_str(&"█".repeat(full));
            out.push_str(&"░".repeat(to - full));
        }
    }
    out
}

/// `t` in at most `w` columns, ending in "…" when it had to be cut.
pub fn ellipsis(t: &str, w: usize) -> String {
    if t.width() <= w {
        return t.to_string();
    }
    let mut out = String::new();
    for c in t.chars() {
        if out.width() + c.width().unwrap_or(0) + 1 > w {
            break;
        }
        out.push(c);
    }
    // "Z.ai Coding …" read as a word left out: the cut ends on the last letter (design review 13)
    out.truncate(out.trim_end().len());
    out + "…"
}

/// A ListPanel in the middle of the screen: its title, the column heads, the rows that fit (failed ones in red).
/// The list drawn, its links and rows for clicks, and the last scroll that still fills the panel (the caller holds
/// `scroll` to it).
fn draw_list(f: &mut Frame, p: &ListPanel, th: &Theme, clock_ms: f64, band: Rect) -> (Option<Rect>, Vec<(Rect, String)>, Vec<(Rect, usize)>, usize) {
    // a rule straight under a heading is not drawn: a section is its bold heading, as the window's (v8.html)
    let ruled = |i: usize| p.rows[i].1 == Tone::Rule && i > 0 && p.rows[i - 1].1 == Tone::Head;
    // the text's width: the band less the border and a column of margin on each side
    let width = band.width.saturating_sub(4) as usize;
    // the meters' bars shrink together, down to 6 cells, before a row is cut: "5-hour session ▬▬▬▬▬▬▬▬ 15%   resets …" at
    // 80 columns (/usage, design review 13)
    let meter = |r: &str| r.contains('█') || r.contains('░');
    let over = p.rows.iter().filter(|r| meter(&r.0)).map(|r| r.0.width().saturating_sub(width)).max().unwrap_or(0);
    let fit = |r: &str| if over > 0 && p.hscroll == 0 && meter(r) { shrink_bar(r, over) } else { r.to_string() };
    // sideways scrolled: the text from there on; else cut to the modal with "…" (a click unfolds it)
    let cut = |t: &str| -> String {
        let t: String = t.chars().skip(p.hscroll).collect();
        if p.hscroll == 0 && t.width() > width { format!("{}…", t.chars().take(width.saturating_sub(1)).collect::<String>()) } else { t }
    };
    // a note (dim prose) longer than the panel wraps, its later lines under its text: "55%  Review sessions…" ended
    // "8 or m…" at 80 columns (the user, 2026-11-24). Meters, links and tables keep their columns and scroll sideways.
    let wraps = |i: usize| p.hscroll == 0 && p.rows[i].1 == Tone::Dim && p.rows[i].0.width() > width;
    let wrapped = |r: &str| -> Vec<String> {
        let indent = r.len() - r.trim_start().len();
        let body = r.trim_start();
        // "55%  " leads, the text's own column holds the later lines
        let (lead, rest) = body.split_at(body.find("  ").filter(|&k| k <= 5).map_or(0, |k| k + 2));
        let hang = indent + lead.len();
        wrap_words(rest, width.saturating_sub(hang).max(10)).into_iter().enumerate()
            .map(|(n, l)| if n == 0 { format!("{}{lead}{l}", " ".repeat(indent)) } else { format!("{}{l}", " ".repeat(hang)) }).collect()
    };
    let detail_lines = |i: usize| p.open.contains(&i).then(|| p.details.get(i).cloned().flatten()).flatten()
        .map_or(0, |d| d.chars().count().div_ceil(width.saturating_sub(p.link_from.min(width / 3)).max(10)));
    let height = |i: usize| if ruled(i) { 0 } else { (if wraps(i) { wrapped(&p.rows[i].0).len() } else { 1 }) + detail_lines(i) };
    // as tall as its rows as drawn: a note's wrapped lines counted, its last line ("…Atlassian Rovo" / "1%") fell under the
    // panel's edge (design review 13)
    let drawn: usize = (0..p.rows.len()).map(height).sum();
    let area = overlay_area(band, drawn + usize::from(!p.head.is_empty()), false);
    // the keys that do something here: a click only where rows unfold, sideways only where a row is cut ("←→ scroll"
    // stood over every list)
    let cut_row = p.hscroll > 0 || p.rows.iter().enumerate().any(|(i, r)| !wraps(i) && fit(&r.0).width() > width);
    let keys = ["↑↓ scroll", if p.details.iter().any(Option::is_some) { "click unfolds a row" } else { "" }, if cut_row { "←→ sideways" } else { "" }, "esc closes"]
        .into_iter().filter(|k| !k.is_empty()).collect::<Vec<_>>().join(" · ");
    let inner = overlay(f, th, area, &p.title, "", &keys);
    let inner = Rect { x: inner.x + 1, width: inner.width.saturating_sub(2), ..inner };
    let room = inner.height as usize;
    // the last top that still fills the panel: scrolling went on to the last row alone, a page of empty space under it
    // (/usage, the user, 2026-11-24)
    let (mut top_max, mut used) = (p.rows.len(), usize::from(!p.head.is_empty()));
    while top_max > 0 && used + height(top_max - 1) <= room {
        top_max -= 1;
        used += height(top_max);
    }
    let mut lines = if p.head.is_empty() { vec![] } else { vec![Line::from(Span::styled(cut(&p.head), st(th.faint)))] };
    let (mut links, mut rows) = (vec![], vec![]);
    for (i, (r, tone)) in p.rows.iter().enumerate().skip(p.scroll.min(top_max)) {
        if lines.len() >= room {
            break;
        }
        if ruled(i) {
            continue;
        }
        let y = inner.y + lines.len() as u16;
        let color = match tone {
            Tone::Bad => st(th.err),
            Tone::Head => st(th.text).add_modifier(Modifier::BOLD),
            Tone::Dim => st(th.faint),
            Tone::Plain | Tone::Rule => st(th.text),
        };
        if *tone == Tone::Rule {
            lines.push(Line::from(Span::styled("─".repeat(width), st(th.border))));
            continue;
        }
        let shown = if p.pending && i == 0 { cut(&format!("{} {r}", spinner(clock_ms))) } else { cut(&fit(r)) };
        rows.push((Rect::new(inner.x, y, inner.width, 1), i));
        match p.links.get(i).cloned().flatten() {
            None if wraps(i) => {
                for l in wrapped(r) {
                    if lines.len() >= room {
                        break;
                    }
                    lines.push(Line::from(Span::styled(l, color)));
                }
            }
            Some(url) => {
                // the label after the fixed columns is the link: underlined, opened on a click
                let at = p.link_from.saturating_sub(p.hscroll).min(shown.chars().count());
                let (a, b): (String, String) = (shown.chars().take(at).collect(), shown.chars().skip(at).collect());
                links.push((Rect::new(inner.x + at as u16, y, (b.width() as u16).min(inner.width.saturating_sub(at as u16)), 1), url));
                lines.push(Line::from(vec![Span::styled(a, color), Span::styled(b, st(th.accent).add_modifier(Modifier::UNDERLINED))]));
            }
            // a meter (█ used, ░ left in its row's text) as a thin bar: the used part heavy in the accent (red on a failed
            // row), the rest a light line, so it reads without colour too
            None if shown.contains('█') || shown.contains('░') => {
                let bar = if *tone == Tone::Bad { st(th.err) } else { st(th.accent) };
                let mut spans: Vec<Span> = vec![];
                for c in shown.chars() {
                    let (c, s) = match c { '█' => ('━', bar), '░' => ('─', st(th.border)), _ => (c, color) };
                    match spans.last_mut() {
                        Some(last) if last.style == s => last.content.to_mut().push(c),
                        _ => spans.push(Span::styled(c.to_string(), s)),
                    }
                }
                lines.push(Line::from(spans));
            }
            None => lines.push(Line::from(Span::styled(shown, color))),
        }
        // an unfolded row: its whole text under it, wrapped, indented to the label's column, dimmed
        if let Some(d) = p.open.contains(&i).then(|| p.details.get(i).cloned().flatten()).flatten() {
            let indent = p.link_from.min(width / 3);
            let w = width.saturating_sub(indent).max(10);
            for chunk in d.chars().collect::<Vec<_>>().chunks(w) {
                if lines.len() >= room {
                    break;
                }
                lines.push(Line::from(Span::styled(format!("{}{}", " ".repeat(indent), chunk.iter().collect::<String>()), st(th.dim))));
            }
        }
    }
    f.render_widget(Paragraph::new(lines), inner);
    (Some(area), links, rows, top_max)
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
        let Some(pairs) = t.replaced() else { continue };
        let f = repo_path(&s.cwd, path);
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

/// The span of `a` that differs from `b`, in characters, widened to whole words: None where the two share nothing at
/// either end (the whole line is new), or `a` lost nothing.
fn changed_span(a: &str, b: &str) -> Option<(usize, usize)> {
    let (x, y): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let pre = x.iter().zip(&y).take_while(|(p, q)| p == q).count();
    let suf = x[pre..].iter().rev().zip(y[pre..].iter().rev()).take_while(|(p, q)| p == q).count();
    let word = |c: &char| c.is_alphanumeric() || *c == '_';
    let start = (0..=pre).rev().find(|&i| i == 0 || !word(&x[i - 1])).unwrap_or(0);
    let end = (x.len() - suf..=x.len()).find(|&i| i == x.len() || !word(&x[i])).unwrap_or(x.len());
    (pre + suf > 0 && start < end).then_some((start, end))
}

/// An edit as diff rows the column's width (desktop/mockups/v10.html T3): the old and new line numbers in a gutter where
/// the file shows where it went (else the bar), a line of the file above and under it and the lines it kept as context,
/// its removed and added lines on bands of the err and ok colours mixed into the background, and the words a line changed
/// on a stronger band against the line that replaced it. No syntax colours: a grammar engine is not worth its size here.
fn diff_rows(old: &str, new: &str, at: Option<Located>, th: &Theme, width: usize, limit: usize) -> Vec<Prefixed> {
    let tabs = |l: &str| l.replace('\t', "    ");
    let (o, n): (Vec<String>, Vec<String>) = (old.lines().map(tabs).collect(), new.lines().map(tabs).collect());
    let (or, nr): (Vec<&str>, Vec<&str>) = (o.iter().map(String::as_str).collect(), n.iter().map(String::as_str).collect());
    let (p, q) = common_ends(&or, &nr);
    let start = at.as_ref().map(|a| a.0);
    let num = |k: usize| start.map(|s| s + k);
    // (old line, new line, sign, text, the changed span)
    let mut rows: Vec<(Option<usize>, Option<usize>, char, String, Option<(usize, usize)>)> = Vec::new();
    // context past three lines folds to a `⋯` row, and the file's line above or under goes with it: it sat beside lines it
    // is not next to
    let skipped = (None, None, '⋯', String::new(), None);
    match &at {
        _ if p > 3 => rows.push(skipped.clone()),
        Some((line, Some(above), _)) => rows.push((Some(line - 1), Some(line - 1), ' ', tabs(above), None)),
        _ => {}
    }
    rows.extend((p.saturating_sub(3)..p).map(|k| (num(k), num(k), ' ', o[k].clone(), None)));
    let (gone, came) = (&o[p..o.len() - q], &n[p..n.len() - q]);
    rows.extend(gone.iter().enumerate().map(|(j, l)| (num(p + j), None, '-', l.clone(), came.get(j).and_then(|r| changed_span(l, r)))));
    rows.extend(came.iter().enumerate().map(|(j, l)| (None, num(p + j), '+', l.clone(), gone.get(j).and_then(|r| changed_span(l, r)))));
    rows.extend((0..q.min(3)).map(|j| (num(o.len() - q + j), num(n.len() - q + j), ' ', o[o.len() - q + j].clone(), None)));
    match &at {
        _ if q > 3 => rows.push(skipped),
        Some((_, _, Some(under))) => rows.push((num(o.len()), num(n.len()), ' ', tabs(under), None)),
        _ => {}
    }
    let w = rows.iter().flat_map(|r| [r.0, r.1]).flatten().max().map_or(0, |m| m.to_string().len());
    let cell = |x: Option<usize>| x.map_or(String::new(), |x| x.to_string());
    let mut out: Vec<Prefixed> = Vec::new();
    for (i, (a, b, sign, text, mark)) in rows.iter().enumerate() {
        if i == limit {
            out.push((Line::from(vec![Span::raw("    "), Span::styled(format!("… +{}", lines_n(rows.len() - limit)), st(th.faint))]), vec![]));
            break;
        }
        let (fg, band) = match sign {
            '-' => (th.err, Some(lerp(th.bg, th.err, 0.16))),
            '+' => (th.ok, Some(lerp(th.bg, th.ok, 0.16))),
            _ => (th.dim, None),
        };
        let on = |s: Style| band.map_or(s, |c| s.bg(c));
        let gutter = if start.is_some() { format!("    {:>w$} {:>w$} ", cell(*a), cell(*b)) } else { format!("    {BAR}") };
        if *sign == '⋯' {
            out.push((Line::from(vec![Span::raw(" ".repeat(gutter.width())), Span::styled("⋯", st(th.faint))]), vec![]));
            continue;
        }
        let body: Vec<char> = format!("{sign} {text}").chars().collect();
        let mut spans = vec![Span::styled(gutter.clone(), on(st(th.faint)))];
        let (x, y) = mark.map_or((body.len(), body.len()), |(x, y)| (x + 2, y + 2));
        spans.push(Span::styled(body[..x].iter().collect::<String>(), on(st(fg))));
        spans.push(Span::styled(body[x..y].iter().collect::<String>(), st(fg).bg(lerp(th.bg, fg, 0.38))));
        spans.push(Span::styled(body[y..].iter().collect::<String>(), on(st(fg))));
        spans.retain(|s| !s.content.is_empty());
        let cont = [Span::styled(" ".repeat(gutter.width() + 2), on(Style::new()))];
        for mut l in wrap(Line::from(spans), width.max(10), &cont) {
            if let Some(c) = band {
                l.spans.push(Span::styled(" ".repeat(width.max(10).saturating_sub(l.width())), Style::new().bg(c)));
            }
            out.push((l, vec![]));
        }
    }
    out
}

/// The ctrl-b view: background tasks with status and age, and the latest output of the selected one.
fn draw_tasks(f: &mut Frame, p: &TasksPanel, th: &Theme, band: Rect) -> Vec<(Rect, usize)> {
    // no task: the sentence in a frame its size (a 28-row empty box held it, design review 11); else the list and, under
    // it, the selected task's output, in the band's height
    let area = if p.rows.is_empty() { overlay_area(band, 1, false) } else { band };
    let count = if p.rows.is_empty() { String::new() } else { format!("{}/{}", p.selected.min(p.rows.len() - 1) + 1, p.rows.len()) };
    let inner = overlay(f, th, area, "background tasks", &count, if p.rows.is_empty() { "esc closes" } else { "↑↓ move · enter opens an agent · x stops · esc closes" });
    let mut lines: Vec<Line<'static>> = Vec::new();
    if p.rows.is_empty() {
        lines.push(Line::from(Span::styled(" no background task in this conversation yet", st(th.faint))));
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
        let style = if i == p.selected { picked(th) } else { st(colour) };
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
fn draw_diff(f: &mut Frame, files: &[Changed], p: &DiffPanel, th: &Theme, mouse: Option<(u16, u16)>, band: Rect) -> Vec<(Rect, usize)> {
    // the changed files as a list, each opened in the editor at its first change: a diff squeezed into a modal ran off
    // its right edge and could not be scrolled sideways (a user, 2026-11-06); the editor shows diffs better
    let (add, del) = files.iter().fold((0, 0), |(a, d), c| (a + c.added, d + c.removed));
    let most = files.iter().map(|c| c.added + c.removed).max().unwrap_or(1).max(1);
    let cwd = std::env::current_dir().unwrap_or_default();
    let rows: Vec<(String, String, String, bool)> = files.iter().map(|c| {
        // a file no longer on disk (a removed worktree): listed, but it cannot be opened
        let gone = c.diff.is_some() && !cwd.join(&c.path).exists();
        (plain(&c.path), format!("+{}", c.added), format!("−{}", c.removed), gone)
    }).collect();
    let area = overlay_area(band, files.len().max(1), false);
    let count = if files.is_empty() { String::new() } else { format!("{}/{}", p.selected.min(files.len() - 1) + 1, files.len()) };
    let inner = overlay(f, th, area, &format!("{} changed · +{add} −{del}", files_n(files.len())), &count, "↑↓ move · enter opens it in your editor · esc closes");
    // the selected file stays in view
    let first = (p.selected + 1).saturating_sub(inner.height as usize);
    // the bars go where they would leave the names no room (40 columns showed "…" for every file)
    let bars = inner.width >= 60;
    let room = (inner.width as usize).saturating_sub(if bars { 34 } else { 22 });
    let mut hits = vec![];
    let lines: Vec<Line> = rows.iter().enumerate().skip(first).take(inner.height as usize).map(|(i, (path, a, d, gone))| {
        let y = inner.y + (i - first) as u16;
        let r = Rect::new(inner.x, y, inner.width, 1);
        hits.push((r, i));
        let hover = mouse.is_some_and(|(x, my)| r.contains(ratatui::layout::Position::new(x, my)));
        let name = if path.width() > room { format!("…{}", path.chars().rev().take(room.saturating_sub(1)).collect::<Vec<_>>().into_iter().rev().collect::<String>()) } else { path.clone() };
        let c = &files[i];
        let (ba, bd) = ((c.added as f64 / most as f64 * 12.0).ceil() as usize, (c.removed as f64 / most as f64 * 12.0).ceil() as usize);
        // the selected file as every list's selected row; its counts keep their colours beside it
        let on = i == p.selected;
        let base = if hover && !on { Style::new().bg(th.surface) } else { Style::new() };
        let mut l = vec![
            Span::styled(if on { "› " } else { "  " }, if on { picked(th) } else { st(th.accent) }),
            Span::styled(format!("{name:<room$}"), if on { picked(th) } else if hover { st(th.accent).add_modifier(Modifier::UNDERLINED) } else { st(th.text) }),
            Span::styled(format!(" {a:>6}"), st(th.ok)),
            Span::styled(format!(" {d:>6} "), st(th.err)),
            Span::styled("▮".repeat(if bars { ba.min(12) } else { 0 }), st(th.ok)),
            Span::styled("▮".repeat(if bars { bd.min(12 - ba.min(12)) } else { 0 }), st(th.err)),
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

fn draw_sessions(f: &mut Frame, p: &SessionsPanel, th: &Theme, band: Rect) {
    // "conversation" and "folder", as /keys and the window say them (it said "session" and "repo", design review 11)
    let area = overlay_area(band, p.rows.len().max(1) + 2, false);
    let count = if p.rows.is_empty() { String::new() } else { format!("{}/{}", p.selected.min(p.rows.len() - 1) + 1, p.rows.len()) };
    let inner = overlay(f, th, area, "resume a conversation", &count, if p.rows.is_empty() { "esc closes" } else { "↑↓ move · enter opens · esc closes" });
    let room = inner.height.saturating_sub(2) as usize;
    let first = p.selected.saturating_sub(room.saturating_sub(1));
    let mut lines: Vec<Line<'static>> = vec![Line::from(Span::styled(ellipsis(" This folder's conversations, newest first; each opens where it ended, on the model it last ran on.", inner.width as usize), st(th.faint))), Line::from("")];
    if p.rows.is_empty() {
        lines.push(Line::from(Span::styled(" no earlier conversation in this folder", st(th.faint))));
    }
    for (i, row) in p.rows.iter().enumerate().skip(first).take(room) {
        let (mark, style) = if i == p.selected { ("›", picked(th)) } else { (" ", st(th.text)) };
        // a row past the edge ends in "…"; the selected one is filled to the frame
        let text = ellipsis(&format!("{mark} {}", plain(row).replace('\n', " ⏎ ")), inner.width as usize);
        lines.push(Line::from(Span::styled(format!("{text:<w$}", w = if i == p.selected { inner.width as usize } else { 0 }), style)));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_login(f: &mut Frame, p: &LoginPanel, th: &Theme, band: Rect) {
    let iw = band.width.saturating_sub(4) as usize;
    let wrapped = |text: &str, c: Color| -> Vec<Line<'static>> {
        wrap_words(text, iw).into_iter().map(|l| Line::from(Span::styled(l, st(c)))).collect()
    };
    let (title, keys, lines) = match &p.form {
        // a sign-in's page: its warning first where it has one, how it goes, where the tokens stay
        Some(fm) if !fm.sign_in.is_empty() => {
            let mut lines = vec![Line::from("")];
            if !fm.note.is_empty() {
                lines.extend(wrapped(&fm.note, if fm.confirm { th.warn } else { th.faint }));
                lines.push(Line::from(""));
            }
            lines.extend(wrapped(&fm.sign_in, th.text));
            if fm.has == "signed in" {
                lines.extend(wrapped(&format!("Signed in now: enter signs in again{}, s signs out.", if fm.other { ", a signs in with another account" } else { "" }), th.ok));
            } else if fm.has.starts_with("signed in") {
                lines.extend(wrapped("Signed in, but ChatGPT plan use was not allowed: enter signs in again and asks for it.", th.warn));
            }
            lines.push(Line::from(""));
            lines.extend(wrapped(&format!("{} will see the code of the sessions you run on it, so signing in also puts {} on your trusted providers. \
                The tokens stay in ~/.config/tazuna/logins/, readable by you only, never in the environment the agent's commands see.", fm.name, fm.host), th.dim));
            let keys = match (fm.confirm, fm.has.starts_with("signed in"), fm.other) {
                (true, ..) => "enter: I understand, sign in · esc back",
                (false, true, true) => "enter signs in again · a another account · s signs out · esc",
                (false, true, false) => "enter signs in again · s signs out · esc back",
                (false, false, _) => "enter signs in · esc back",
            };
            (format!("sign in to {}", fm.name), keys, lines)
        }
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
            if !fm.note.is_empty() {
                lines.extend(wrapped(&fm.note, th.warn));
            }
            lines.push(Line::from(vec![Span::styled("> ", st(th.accent)), Span::styled("•".repeat(fm.typed.min(iw.saturating_sub(3))), st(th.text)),
                                       Span::styled("▍", st(th.accent))]));
            // Mistral's unsure key: said where enter's meaning is, so it fits the page at 80x24
            if fm.has.starts_with("same key as") {
                lines.extend(wrapped("The same key is saved for Mistral and Mistral Vibe: tazuna cannot tell whether it bills per token or the \
                    Vibe plan, so neither uses it. Enter with nothing typed keeps it as this provider's key and removes it from the other.", th.warn));
            } else if fm.has.starts_with("kept from before") {
                lines.extend(wrapped("This key was saved for both Mistral and Mistral Vibe before; a new key replaced it under Mistral. Enter \
                    with nothing typed keeps it as your Vibe key; delete removes it.", th.warn));
            } else if !fm.has.is_empty() {
                lines.push(Line::from(Span::styled("Enter with nothing typed keeps the key it has.", st(th.faint))));
            }
            lines.push(Line::from(""));
            lines.extend(wrapped(&format!("{} will see the code of the sessions you run on it, so saving also puts {} on your trusted providers. \
                The key stays in ~/.config/tazuna/keys.json, readable by you only, and out of the agent's commands.", fm.name, fm.host), th.dim));
            if fm.has == "key in this terminal only" {
                lines.extend(wrapped(&format!("{} is set in this terminal only: tazuna started in another terminal or folder does not see it. \
                    Enter with nothing typed saves it for every folder.", fm.var), th.warn));
            } else if fm.has.starts_with("key in the environment") {
                lines.extend(wrapped(&format!("{} is set in your environment, and it wins over a key saved here.", fm.var), th.dim));
            }
            (format!("log in to {}", fm.name), "enter saves the key · esc cancels", lines)
        }
        None => {
            let mut lines = wrapped("Log in to a provider to use its models in /model: an API key (the provider's page opens in your browser to make one), or for ChatGPT, GitHub Copilot, SuperGrok, Gemini CLI and Antigravity a sign-in.", th.faint);
            lines.push(Line::from(""));
            // the search, as /model's: what was typed, or how to start one
            lines.push(if p.filter.is_empty() {
                Line::from(Span::styled("  type to search", st(th.faint)))
            } else {
                Line::from(vec![Span::styled("› ", st(th.accent)), Span::styled(p.filter.clone(), st(th.text)), Span::styled("▍", st(th.accent)),
                    Span::styled(match p.rows.len() { 0 => "   no provider matches".to_string(), 1 => "   1 provider".to_string(), n => format!("   {n} providers") }, st(th.faint))])
            });
            lines.push(Line::from(""));
            let nw = p.rows.iter().map(|r| r.0.chars().count()).max().unwrap_or(0);
            // the rows that fit (some 70 providers since 0.1.196), the selected one in view, its terms' line under them
            let note = p.rows.get(p.selected).map(|r| wrapped(&r.2, th.warn)).unwrap_or_default();
            let room = (band.height as usize).saturating_sub(lines.len() + 3 + note.len() + usize::from(!note.is_empty())).max(3);
            let first = p.selected.saturating_sub(room.saturating_sub(1)).min(p.rows.len().saturating_sub(room));
            for (i, (name, state, _)) in p.rows.iter().enumerate().skip(first).take(room) {
                let on = i == p.selected;
                let mark = if on { "›" } else { " " };
                let ok = !state.contains("not ");
                let row = format!("{mark} {name:<nw$}  ");
                let left = row.chars().count();
                lines.push(Line::from(vec![
                    Span::styled(row, if on { picked(th) } else { st(th.text) }),
                    Span::styled(ellipsis(state, iw.saturating_sub(left)), st(if ok { th.ok } else { th.faint })),
                ]));
            }
            if !note.is_empty() {
                lines.push(Line::from(""));
                lines.extend(note);
            }
            ("login".to_string(), if p.filter.is_empty() { "type to search · ↑↓ move · enter logs in · esc closes" } else { "↑↓ move · enter logs in · esc clears the search" }, lines)
        }
    };
    let inner = overlay(f, th, overlay_area(band, lines.len(), false), &title, "", keys);
    f.render_widget(Paragraph::new(lines), Rect::new(inner.x + 1, inner.y, inner.width.saturating_sub(2), inner.height));
}

fn draw_rewind(f: &mut Frame, p: &RewindPanel, th: &Theme, band: Rect) {
    let area = overlay_area(band, p.items.len().max(1) + 2, false);
    let count = if p.items.is_empty() { String::new() } else { format!("{}/{}", p.selected.min(p.items.len() - 1) + 1, p.items.len()) };
    let inner = overlay(f, th, area, "rewind to before", &count, "↑↓ move · enter files + conversation · f files · c conversation · esc closes");
    let room = inner.height.saturating_sub(2) as usize;
    let first = p.selected.saturating_sub(room.saturating_sub(1));
    let mut lines: Vec<Line<'static>> = vec![Line::from(Span::styled(" Undo the chosen message and everything after it.", st(th.faint))), Line::from("")];
    for (i, text) in p.items.iter().enumerate().skip(first).take(room) {
        let on = i == p.selected;
        let row = ellipsis(&format!("{} {:>2}. {}", if on { "›" } else { " " }, i + 1, plain(text).replace('\n', " ⏎ ")), inner.width as usize);
        lines.push(Line::from(Span::styled(format!("{row:<w$}", w = if on { inner.width as usize } else { 0 }), if on { picked(th) } else { st(th.text) })));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

/// `name` cut in its middle to `room` characters: the head keeps at least `keep` of them where it can (what tells it from a
/// row that would read the same), the tail a third of the rest (", fast").
fn mid_cut(name: &str, room: usize, keep: usize) -> String {
    let n = name.chars().count();
    let room = room.max(2);
    if n <= room {
        return name.to_string();
    }
    if keep >= room - 1 {
        // the difference lies past the room: the stretch that ends with it
        return format!("…{}", name.chars().skip(keep.min(n) + 1 - room).take(room - 1).collect::<String>());
    }
    let tail = ((room - 1) / 3).min(room - 1 - keep);
    format!("{}…{}", name.chars().take(room - 1 - tail).collect::<String>(), name.chars().skip(n - tail).collect::<String>())
}

/// The model column of the rows on screen, (name, whose it is) in `lw` columns. The model's name comes first: whose it is
/// shortens (to 4 characters and "…") or goes before the name is cut; at 100 columns the name gave way to the provider and
/// "Mistral …tral Mistral Vibe" stood three times (design review 13). Rows that would then read the same keep what tells
/// them apart: one model at two providers its providers, the name giving way (design review 11), two names their first
/// differing characters.
fn model_cells(rows: &[(&str, &str)], lw: usize) -> Vec<(String, String)> {
    let one = |name: &str, prov: &str| -> (String, String) {
        let (n, p) = (name.chars().count(), prov.chars().count());
        match prov {
            "" => (mid_cut(name, lw, 0), String::new()),
            _ if n + 1 + p <= lw => (name.to_string(), prov.to_string()),
            _ if n + 1 + 5 <= lw => (name.to_string(), ellipsis(prov, lw - n - 1)),
            _ => (mid_cut(name, lw, 0), String::new()),
        }
    };
    let base: Vec<(String, String)> = rows.iter().map(|(n, p)| one(n, p)).collect();
    let common = |a: &str, b: &str| a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count();
    (0..rows.len()).map(|i| {
        let (name, prov) = rows[i];
        let same: Vec<usize> = (0..rows.len()).filter(|&j| j != i && base[j] == base[i] && rows[j] != rows[i]).collect();
        if same.is_empty() {
            base[i].clone()
        } else if same.iter().any(|&j| rows[j].0 == name) {
            // a provider and its region's twin ("MiniMax Plan", "MiniMax Plan China") shortened alike keep their ends
            let twin = same.iter().any(|&j| rows[j].0 == name && rows[j].1 != prov && ellipsis(rows[j].1, lw / 2) == ellipsis(prov, lw / 2));
            let prov = if prov.is_empty() { String::new() } else if twin { mid_cut(prov, lw / 2, 0) } else { ellipsis(prov, lw / 2) };
            (mid_cut(name, lw.saturating_sub(prov.chars().count() + usize::from(!prov.is_empty())), 0), prov)
        } else {
            let keep = same.iter().map(|&j| common(name, rows[j].0)).max().unwrap_or(0) + 1;
            let p = &base[i].1;
            (mid_cut(name, lw.saturating_sub(p.chars().count() + usize::from(!p.is_empty())), keep), p.clone())
        }
    }).collect()
}

/// /model: Claude's models and the checked routes; ready ones in the text colour, a missing key greyed out, an untrusted
/// provider (the code would go there) in the warning colour, a route checked long ago marked stale. The columns fit the
/// panel's width: the label gives way first (cut in the middle, so "· fast" stays), then the scores go; the price always
/// shows. The selected row in full (provider, price, cache, age, the reason it cannot be picked) wraps below the list.
fn draw_models(f: &mut Frame, p: &ModelsPanel, th: &Theme, band: Rect) {
    // OMP's layout: the providers on the left, the selected one's models on the right; a narrow screen keeps the models
    let two = band.width >= 70 && !p.groups.is_empty();
    // the providers' pane as wide as its longest row, but at 80 columns no wider than before 0.1.196's long names (they
    // took 11 columns from the models' names, review 11: 19, as then); wider screens give it more, up to 30
    let gw = if two { p.groups.iter().map(|(n, c, ..)| n.chars().count() + c.to_string().len() + 6).max().unwrap_or(18).clamp(18, (band.width as usize).saturating_sub(60).clamp(19, 30)) } else { 0 };
    // our judge's score without its runs ("6.8", the runs are in the details under the list): two numbers to the right
    // under "bench" and "AA", where "bench (runs)" and "AA index" stood as one double heading (design review 11)
    let score = |b: &str| b.split(" (").next().unwrap_or_default().to_string();
    // column widths: as wide as their header or longest entry
    let width = |head: &str, f: &dyn Fn(&ModelRow) -> usize| p.rows.iter().map(f).chain([head.chars().count()]).max().unwrap_or(0);
    let pw = width("$/M in / out", &|r| r.price.chars().count());
    let bw = width("bench", &|r| score(&r.bench).chars().count());
    let aw = width("AA", &|r| r.aa.chars().count());
    let longest = width("model", &|r| r.label.width() + if r.provider.is_empty() { 0 } else { r.provider.width() + 1 });
    let keys = if let Some((_, _, _, k, _)) = &p.effort {
        k
    } else if !p.filter.is_empty() {
        // esc empties the search first (it said "esc closes", design review 11)
        "↑↓ move · enter chooses · ← providers · tab pins a provider · esc clears the search"
    } else if p.in_models || !two {
        "↑↓ move · enter chooses · ← providers · tab pins a provider · type to search · esc closes"
    } else {
        "↑↓ providers · → its models · type to search · esc closes"
    };
    // the band's width and height while it is open: filtering does not make it jump
    let body = overlay(f, th, band, "model", &format!("{} model{}", p.rows.len(), if p.rows.len() == 1 { "" } else { "s" }), keys);
    // the left pane: All models, then the providers, ● usable now, ○ not (no key, not trusted)
    if two {
        let mut lines = vec![Line::from(Span::styled(" providers", st(th.faint)))];
        // the rows that fit, the selected one in view (some 70 providers since 0.1.196: it went off the pane, review 11)
        let room = (body.height as usize).saturating_sub(1).max(1);
        let first = p.group.saturating_sub(room - 1);
        for (i, (name, n, ok, _)) in p.groups.iter().enumerate().skip(first).take(room) {
            let mark = if i == p.group { "›" } else { " " };
            let dot = if i == 0 { "◆" } else if *ok { "●" } else { "○" };
            // a name too long for the pane is cut, its count kept: without it "Qwen Cloud Tok…" said nothing of how many
            // models it holds (design review 13)
            let count = format!(" {n}");
            let room = gw.saturating_sub(4 + count.len());
            let label = ellipsis(name, room);
            let text = format!("{mark}{dot} {label:<room$}{count}");
            let color = if *ok || i == 0 { th.text } else { th.faint };
            let style = match (i == p.group, p.in_models) {
                (true, false) => picked(th),
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
        let terms = if gok && p.group > 0 { gwhy.clone() } else { String::new() };
        [(gname.clone(), th.text), (state, if gok || p.group == 0 { th.dim } else { th.warn }), (terms, th.warn)].into_iter().filter(|(t, _)| !t.is_empty())
            .flat_map(|(t, c)| wrap_words(&t, iw.saturating_sub(1)).into_iter().map(move |l| Line::from(Span::styled(format!(" {l}"), st(c))))).collect()
    } else {
        p.rows.get(p.selected).map(|r| {
            let price = match r.price.as_str() { "plan" => "on the plan".to_string(), "free" => "free".to_string(), p => format!("${} per M tokens (in / out)", p.replace(" / ", " / $")) };
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
                .flat_map(|(text, c)| wrap_words(&text, iw.saturating_sub(1)).into_iter().map(move |l| Line::from(Span::styled(format!(" {l}"), st(c)))))
                .collect()
        }).unwrap_or_default()
    };
    let dh = (detail.len() as u16).min(right.height.saturating_sub(4)); // at least one row stays
    let room = right.height.saturating_sub(2 + dh + u16::from(dh > 0)) as usize; // search, header; a gap before the details
    let first = p.selected.saturating_sub(room.saturating_sub(1));
    let fixed = 2 + pw;
    let scores = iw >= fixed + 2 + bw + 2 + aw + 1 + 16.min(longest); // room for the scores and a 16-character label
    // the label column takes what is left, so the table spans the pane
    let lw = iw.saturating_sub(fixed + 1 + if scores { bw + aw + 4 } else { 0 }).max(4);
    // a row: the name, then whose it is in faint where both fit ("Codestral 2508 Mistral"; ", mistral" was part of the
    // name, and a cut one ran into it: "ERNIE 5.5 Flash Previe…openrouter"), the price, the two scores to the right
    // `name` and `provider` as model_cells fitted them to the column
    let cols = |mark: &str, name: &str, provider: &str, pr: &str, b: &str, a: &str, base: Style, on: bool| -> Line<'static> {
        let prov = if provider.is_empty() { String::new() } else { format!(" {provider}") };
        let pad = " ".repeat(lw.saturating_sub(name.width() + prov.width()));
        let price = if on { base } else { match pr { "free" => st(th.ok), "plan" => st(th.faint), _ => base } };
        let mut sp = vec![Span::styled(format!("{mark} {name}"), base), Span::styled(prov, if on { base } else { st(th.faint) }), Span::styled(format!("{pad} "), base),
            Span::styled(format!("{pr:<pw$}"), price)];
        if scores {
            sp.push(Span::styled(format!("  {b:>bw$}  {a:>aw$}"), base));
        }
        if on {
            let used: usize = sp.iter().map(|x| x.content.width()).sum();
            sp.push(Span::styled(" ".repeat(iw.saturating_sub(used)), base));
        }
        Line::from(sp)
    };
    let search = if p.filter.is_empty() {
        Line::from(Span::styled(ellipsis(&format!(" {gname}"), iw), st(th.faint)))
    } else {
        Line::from(vec![Span::styled("> ", st(th.accent)), Span::styled(cut(p.filter.clone()), st(th.text)), Span::styled("▍", st(th.accent))])
    };
    // a model chosen: the right pane picks its effort
    if let Some((label, levels, at, _, note)) = &p.effort {
        let mut lines = vec![Line::from(vec![Span::styled(cut(label.clone()), st(th.text).add_modifier(Modifier::BOLD)), Span::styled(" · effort", st(th.faint))]), Line::from("")];
        for (i, l) in levels.iter().enumerate() {
            let style = if i == *at { picked(th) } else { st(th.text) };
            lines.push(Line::from(Span::styled(cut(format!("{} {l} ", if i == *at { "›" } else { " " })), style)));
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
    let mut lines: Vec<Line<'static>> = vec![search, cols(" ", "model", "", "$/M in / out", "bench", "AA", st(th.faint), false)];
    let shown: Vec<(&str, &str)> = p.rows.iter().skip(first).take(room).map(|r| (r.label.as_str(), r.provider.as_str())).collect();
    for ((i, r), (name, prov)) in p.rows.iter().enumerate().skip(first).take(room).zip(model_cells(&shown, lw)) {
        let on = i == p.selected && (p.in_models || !two);
        let mark = if on { "›" } else if r.current { "•" } else { " " };
        let color = match r.state { 1 | 4 => th.faint, 2 => th.warn, 3 => th.dim, _ if r.stale => th.warn, _ => th.text };
        // the selected row as every list's: the accent fill (it was the row's own ink reversed)
        lines.push(cols(mark, &name, &prov, &r.price, &score(&r.bench), &r.aa, if on { picked(th) } else { st(color) }, on));
    }
    if p.rows.is_empty() {
        lines.push(Line::from(Span::styled(" no model matches", st(th.faint))));
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

// Every fade is the text coming up from the background in RGB, under 200 ms: tachyonfx blends colours in HSL by default,
// and the accent (232°) coming from the warm background (45°) went the short way round the hue circle, through magenta,
// for the 0.7-1.4 s the fades lasted; a finished answer dimmed for 1.2 s in zen; sweeps, coalesce and a dissolve on quit
// came and went with it (design review 10).
fn fade_in(d: Design, ms: u32) -> Effect {
    fx::fade_from_fg(d.theme().bg, (ms, QuadOut)).with_color_space(ColorSpace::Rgb)
}

pub fn boot_fx(d: Design) -> Effect {
    fade_in(d, 180)
}

pub fn block_fx(d: Design) -> Effect {
    fade_in(d, 160)
}

/// Whether the newest block only grows a folded run of tool calls (zen draws one line for them): that line is already on
/// screen, and fading it in again at each call kept it near the background for most of a run (design review 9).
pub fn grows_a_fold(d: Design, s: &Session) -> bool {
    let fold = |b: &Block| matches!(b, Block::Tool(_) | Block::Thinking { .. });
    d == Design::Zen && s.blocks.len() > 1 && s.blocks[s.blocks.len() - 2..].iter().all(fold)
}

pub fn pulse_fx(d: Design) -> Effect {
    // lighter for a moment, the hue and saturation as they are: a flash of another colour is not in the palette
    let shift = match d {
        Design::Zen => [0.0, 0.0, 8.0],
        _ => [0.0, 0.0, 14.0],
    };
    fx::ping_pong(fx::hsl_shift_fg(shift, (90, QuadOut)))
}

// ---------------------------------------------------------------- shared pieces

pub fn st(c: Color) -> Style {
    Style::new().fg(c)
}

const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn spinner(clock_ms: f64) -> &'static str {
    SPINNER[(clock_ms / 80.0) as usize % SPINNER.len()]
}

/// Glyph, label and color for what the agent is doing right now. Every working state takes the accent (the one live
/// state); a question, a refused call and a failed run each read as what they are, where all three said "thinking" or
/// "done" (design review, 2026-11-19).
pub fn activity(s: &Session, v: &View, th: &Theme) -> (&'static str, String, Color) {
    // Before a recording's init event there is no live agent to connect to or animate as thinking.
    if !v.live && s.model.is_empty() && matches!(s.activity, Activity::Starting) {
        return ("·", "recorded".into(), th.dim);
    }
    let spin = spinner(v.clock_ms);
    // a permission question: nothing runs until it is answered, so no spinner; it and ready are words in their colour, no
    // dot (design review 14)
    if v.asking.is_some() {
        return ("", "needs you".into(), th.warn);
    }
    if let Some(secs) = s.retry_in(v.clock_ms) {
        return (spin, format!("retrying in {}", wait_str(secs)), th.err);
    }
    match s.activity {
        // the turn is over, but claude's background work goes on: say so where "done" would be (ctrl-b lists it)
        Activity::Ready | Activity::Done if v.bg_running > 0 => (spin, format!("{} working in background · ctrl-b", v.bg_running), th.accent),
        Activity::Ready if let Some(why) = v.cant_run => ("!", why.into(), th.warn),
        Activity::Ready => ("", "ready".into(), th.dim),
        // a compaction (claude's or the own loop's): its own word, not "thinking" nor the provider's queue in warn
        Activity::Starting | Activity::Waiting | Activity::Thinking if s.compacting => (spin, "compacting".into(), th.accent),
        // keep-alive pings and no output (the own loop's provider queueing it): said, so it does not look like tazuna hangs
        Activity::Waiting if s.waiting_note.is_some() => (spin, "provider still processing (no output yet)".into(), th.warn),
        // from Enter to the first output it reads "thinking" throughout: "starting", "waiting for model" and then "thinking"
        // a second or more later looked like three different hold-ups (2026-11-16)
        Activity::Starting | Activity::Waiting | Activity::Thinking => (spin, "thinking".into(), th.accent),
        Activity::Writing => (spin, "writing".into(), th.accent),
        Activity::Tool => (spin, format!("running {}", last_tool_name(s)), th.accent),
        Activity::Verifying => (spin, "verifying".into(), th.accent),
        Activity::Done if matches!(s.blocks.iter().rev().find_map(|b| if let Block::Done(d) = b { Some(d) } else { None }), Some(d) if !d.ok && !d.stopped) => {
            ("✗", "failed".into(), th.err)
        }
        // stopped with esc: the row above says "■ Stopped", and the status said a green "✓ done" (design review 11)
        // (esc while the tests ran too: the reply's own end row is a finished one)
        Activity::Done if s.verify_cancelled || matches!(s.blocks.iter().rev().find_map(|b| if let Block::Done(d) = b { Some(d) } else { None }), Some(d) if d.stopped) => ("■", "stopped".into(), th.dim),
        Activity::Done => ("✓", "done".into(), th.ok),
    }
}

/// Seconds to a tenth: "1.5 s", and "<0.1 s" for less, never "0.0s" (design review 14).
pub fn tenths(secs: f64) -> String {
    if secs < 0.05 { "<0.1 s".into() } else { format!("{secs:.1} s") }
}

/// A wait in the units the line under an answer uses: "8 s", "4 min 52 s".
fn wait_str(secs: f64) -> String {
    let s = secs.ceil() as u64;
    if s < 60 { format!("{s} s") } else { format!("{} min {:02} s", s / 60, s % 60) }
}

/// The call the permission question is about, while a question is open: the open call with the question's tool and
/// input (then its tool alone, then the oldest open call: a question is asked in the calls' order). It waits for the
/// user, so it draws no spinner. It was the newest open call: with two edits in one message the second was marked while
/// the question was about the first (design review 11).
fn asked(t: &Tool, s: &Session, v: &View) -> bool {
    if v.asking.is_none() || t.result.is_some() || t.done_ms.is_some() {
        return false;
    }
    s.asked_call().is_some_and(|i| matches!(&s.blocks[i], Block::Tool(x) if std::ptr::eq(x, t)))
}

fn last_tool_name(s: &Session) -> String {
    s.blocks.iter().rev().find_map(|b| if let Block::Tool(t) = b { Some(short_name(&t.name).to_string()) } else { None }).unwrap_or_default()
}

/// Money and plan apart: "$0.50 + $7.46 on Claude plan"; "$7.46 on Claude plan" when only claude ran, "$0.50" when only the own loop did.
/// A plan's own loop says its plan ("$0.12 on Mistral Vibe plan"), and the plan alone where it has no reference price.
pub fn cost_str(s: &Session) -> String {
    let (paid, plan) = s.split_cost();
    let about = if s.estimated { "≈" } else { "" };
    let (word, unpriced) = plan_label(&s.plans);
    match s.exact_cost {
        // before the first result: an unpriced plan's estimate is no price either
        None => s.unpriced_plan_now().map_or_else(|| format!("≈{}", usd(s.cost())), plan_word),
        Some(_) if paid > 0.0 && plan > 0.0 => format!("{about}{} + {} {word}", usd(paid), usd(plan)),
        Some(_) if paid > 0.0 => format!("{about}{}", usd(paid)),
        Some(c) if plan > 0.0 => format!("{about}{} {word}", usd(c)),
        // nothing on a plan (the own loop's free models): just the sum, which said "$0 on Claude plan" (design review 9); a
        // plan without a reference price: the plan, no money, never $0 at API prices (the user, 2026-11-25)
        Some(_) if unpriced => word,
        Some(c) => format!("{about}{}", usd(c)),
    }
}

/// Whose plan the plan money is on, from the plans that paid (name, no reference price): "on Claude plan" (also where none
/// is known), "on Mistral Vibe plan", "on Z.ai Coding Plan", "on plans" where several mixed; and whether one had no
/// reference price. It said "on Claude plan" under a Mistral Vibe run (the user, 2026-11-25).
fn plan_label(plans: &[(String, bool)]) -> (String, bool) {
    let mut names: Vec<&str> = plans.iter().map(|p| p.0.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    let word = match names[..] {
        [] => "on Claude plan".to_string(),
        [one] => plan_word(one),
        _ => "on plans".to_string(),
    };
    (word, plans.iter().any(|p| p.1))
}

/// "on Mistral Vibe plan"; a name that says plan already once: "on Alibaba Coding Plan China".
fn plan_word(name: &str) -> String {
    if name.split_whitespace().any(|w| w.eq_ignore_ascii_case("plan")) { format!("on {name}") } else { format!("on {name} plan") }
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

/// A notice that carries a provider's raw error ("provider error in stream: {\"message\":\"[404]: … not found.\", …}",
/// cut at 300 characters, so no longer JSON) as its message alone: "provider error: [404]: … not found.".
pub fn readable_notice(text: &str) -> Option<String> {
    let (before, rest) = text.split_once("provider error in stream: ")?;
    let start = rest.find("\"message\":\"")? + "\"message\":\"".len();
    let mut msg = String::new();
    let mut chars = rest[start..].chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => if let Some(n) = chars.next() { msg.push(if n == 'n' { ' ' } else { n }) },
            '"' => break,
            c => msg.push(c),
        }
    }
    // a retry's notice already starts "provider · "
    let before = before.strip_suffix("provider · ").unwrap_or(before);
    let msg = without_repeated_code(msg.trim());
    (!msg.is_empty()).then(|| format!("{before}provider error: {msg}"))
}

/// A provider's message without a leading "[404]: " when the code comes again in it: "[404]: Antigravity upstream error
/// (404): …" said 404 twice (design review 9). The window does the same (desktop/src/lib/fromView.ts, readableError).
fn without_repeated_code(msg: &str) -> &str {
    let rest = msg.strip_prefix('[').and_then(|r| r.split_once("]: ")).filter(|(code, rest)| code.len() == 3 && code.bytes().all(|b| b.is_ascii_digit()) && rest.contains(*code));
    rest.map_or(msg, |(_, rest)| rest)
}

/// The plan's windows as spans, each in its own colour (dim, amber from 75%, red from 90%, as the desktop app's): one
/// colour by the higher made "5h 13%" red beside a week at 85% (2026-11-19).
pub fn plan_spans(s: &Session, th: &Theme) -> Option<Vec<Span<'static>>> {
    let (h5, d7) = s.limits?;
    let c = |x: f64| if x >= 90.0 { th.err } else if x >= 75.0 { th.warn } else { th.dim };
    Some(vec![Span::styled(format!("5h {h5:.0}%"), st(c(h5))), Span::raw("  "), Span::styled(format!("wk {d7:.0}%"), st(c(d7))),
        Span::styled(limits_age(s), st(th.faint))])
}

/// How old the plan's reading is, when 10 minutes or more: " · 2h ago".
fn limits_age(s: &Session) -> String {
    s.limits_at.map(|t| crate::session::unix_now().saturating_sub(t)).filter(|a| *a >= 600).map_or(String::new(), |a| match a {
        0..3600 => format!("  {}m ago", a / 60),
        3600..86_400 => format!("  {}h ago", a / 3600),
        _ => format!("  {}d ago", a / 86_400),
    })
}

/// The cost box's two account rows: the account this session runs on (Claude's plan windows; a provider's balance where
/// its API tells, short, "plan" for a subscription), then another only when it needs you (Claude's plan at 80% or more, a
/// reading the core says needs you: low, used up, past zero, a key that may not read it). It named Claude and SubRouter always and no other provider (the user,
/// 2026-11-23: "how will users know the current usage remaining of other providers"); every account is in /usage.
pub fn account_rows(s: &Session, th: &Theme) -> (Line<'static>, Line<'static>) {
    let name = |n: &str| Span::styled(format!("{n} "), st(th.ink2).add_modifier(Modifier::BOLD));
    let on_claude = s.provider.is_none() && s.price_m.is_none();
    let first = if on_claude {
        match plan_spans(s, th) {
            Some(plan) => Line::from([vec![name("Claude")], plan].concat()),
            None => Line::from(Span::styled(if s.exact_cost.is_some() && !s.estimated { "exact (reported)" } else { "estimate (list prices)" }, st(th.faint))),
        }
    } else if let Some(p) = &s.provider {
        let reading = s.credits.iter().find(|(n, ..)| n == p).and_then(|(_, l, needs)| Some((l.clone()?, *needs)));
        let plan = crate::models::LOGINS.iter().any(|l| l.name == p && l.plan);
        match reading {
            Some((left, needs)) => Line::from(vec![name(p), Span::styled(left, st(if needs { th.warn } else { th.dim }))]),
            None => Line::from(vec![name(p), Span::styled(if plan { "plan" } else { "no balance it reports" }, st(th.faint))]),
        }
    } else {
        Line::from(Span::styled("on this machine: free", st(th.faint)))
    };
    // another account, only when it needs you
    let claude_high = s.limits.filter(|(h5, d7)| !on_claude && (*h5 >= 80.0 || *d7 >= 80.0));
    let second = if let Some(plan) = claude_high.and_then(|_| plan_spans(s, th)) {
        Line::from([vec![name("Claude")], plan].concat())
    } else if let Some((n, left)) = s.credits.iter().filter(|(n, ..)| Some(n) != s.provider.as_ref())
        .find_map(|(n, l, needs)| l.as_ref().filter(|_| *needs).map(|l| (n, l))) {
        Line::from(vec![name(n), Span::styled(left.clone(), st(th.warn))])
    } else {
        Line::default()
    };
    (first, second)
}

/// "SubRouter $8.82 left", where a SubRouter key is set.
pub fn balance_str(s: &Session) -> Option<String> {
    s.balance.map(|b| format!("SubRouter {} left", usd(b)))
}

pub fn clock_str(ms: f64) -> String {
    let secs = (ms / 1000.0) as u64;
    if secs >= 3600 {
        format!("{}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60)
    } else {
        format!("{:02}:{:02}", secs / 60, secs % 60)
    }
}

/// The replay's speed ("▶ 4.0×", "⏸ paused"); nothing in a live session, where it is no control of the user's (until
/// 0.1.180 a live Aurora, Zen and Timeline showed "▶ 1.0×", and Zen the replay's keys "1-4 · e · q").
pub fn play_str(v: &View, paused: &str) -> String {
    match (v.live, v.paused) {
        (true, _) => String::new(),
        (false, true) => paused.into(),
        (false, false) => format!("▶ {:.1}×", v.speed),
    }
}

/// Replay's secondary controls live above the conversation, leaving a single strip for state and essential keys.
fn draw_replay_metadata(f: &mut Frame, area: Rect, v: &View, th: &Theme) {
    let speed = if v.speed < 1.0 { format!("{}×", v.speed) } else { format!("{:.1}×", v.speed) };
    let details = format!("{}  {speed}  ←/→ speed · ↑/↓ scroll", clock_str((v.clock_ms - v.since_ms).max(0.0)));
    split_row(f, area, Line::from(Span::styled("Offline recording", st(th.dim))),
        Line::from(Span::styled(details, st(th.dim))), None);
}

/// Compact strip A: no input caret or live placeholder; always keep the way out when narrower than 80 columns.
fn draw_replay_footer(f: &mut Frame, area: Rect, v: &View, th: &Theme) {
    let [rule, row] = ratatui::layout::Layout::vertical([ratatui::layout::Constraint::Length(1), ratatui::layout::Constraint::Min(0)]).areas(area);
    f.render_widget(Paragraph::new(Span::styled("─".repeat(rule.width as usize), st(th.border))), rule);
    let state = format!("Replay · {}", if v.paused { "Paused" } else { "Playing" });
    let label = if row.width as usize >= state.width() + 10 { state.as_str() } else { "Replay" };
    let mut keys = vec![format!("Space {}", if v.paused { "resume" } else { "pause" }), "Tab layout".into(),
        format!("e {}", if v.expand_all { "fold" } else { "expand" }), "q quit".into()];
    while keys.len() > 1 && 1 + label.width() + 3 + keys.join("  ").width() > row.width as usize {
        keys.remove(if keys.len() > 2 { 1 } else { 0 });
    }
    f.render_widget(ratatui::widgets::Block::new().style(Style::new().bg(th.field)), row);
    f.render_widget(Paragraph::new(Line::from(vec![Span::styled(format!(" {label}   "), st(th.accent)),
        Span::styled(keys.join("  "), st(th.text))])), row);
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
    // OmniRoute's ids carry their level (agy/gemini-4-argon-high), which the effort beside the name already says
    let m = if m.contains('/') { crate::models::tier_of(m).map_or(m, |(base, _)| base) } else { m };
    if m.is_empty() {
        return "connecting…".into();
    }
    pretty_model(m)
}

/// A model's name as people say it, from its id, by the desktop app's rule (desktop/src/lib/types.ts modelLabel, whose
/// MODEL_NAMES are the cases of both): "claude-opus-5-5" is "Opus 5.5", "deepseek-v4.1-flash-1017-fast" "DeepSeek V4.1
/// Flash 1017 Fast", "gpt-6.1-sol" "GPT-6.1 Sol" (design review 6: only Claude's were named, the rest showed their id).
pub fn pretty_model(id: &str) -> String {
    let m = id.trim().split_whitespace().next().unwrap_or_default();
    let bare = m.strip_prefix("claude-").unwrap_or(m);
    if let Some((fam, ver)) = bare.split_once('-') && ["opus", "sonnet", "haiku", "fable"].contains(&fam) {
        let v: Vec<&str> = ver.split(['-', '.']).take_while(|p| p.len() <= 2 && p.chars().all(|c| c.is_ascii_digit())).take(2).collect();
        if !v.is_empty() {
            // a thinking variant (Antigravity's "claude-opus-4-6-thinking") says so, beside its plain sibling
            let thinking = if ver.split('-').any(|p| p == "thinking") { " thinking" } else { "" };
            return format!("{}{} {}{thinking}", fam[..1].to_uppercase(), &fam[1..], v.join("."));
        }
    }
    const BRAND: [(&str, &str); 21] = [("zai", "Z.ai"), ("deepseek", "DeepSeek"), ("gpt", "GPT"), ("glm", "GLM"), ("qwen", "Qwen"), ("gemini", "Gemini"), ("gemma", "Gemma"),
        ("grok", "Grok"), ("mistral", "Mistral"), ("codestral", "Codestral"), ("devstral", "Devstral"), ("ernie", "ERNIE"), ("nemotron", "Nemotron"),
        ("kimi", "Kimi"), ("llama", "Llama"), ("muse", "Muse"), ("minimax", "MiniMax"), ("phi", "Phi"), ("olmo", "OLMo"), ("hermes", "Hermes"), ("sonar", "Sonar")];
    const UPPER: [&str; 9] = ["ud", "vl", "hc", "it", "xxs", "xs", "moe", "r1", "v3"];
    let (route, rest) = m.split_once('/').unwrap_or(("", m));
    let free = rest.ends_with(":free");
    let mut rest = rest.trim_end_matches(":free").to_string();
    if route == "agy" {
        for l in ["-low", "-medium", "-high", "-xhigh", "-max"] {
            if let Some(r) = rest.strip_suffix(l) { rest = r.to_string(); break; }
        }
    }
    let t: Vec<&str> = rest.split('-').filter(|w| !w.is_empty()).collect();
    let digit = |s: &str| s.chars().next().is_some_and(|c| c.is_ascii_digit());
    let (mut out, mut i): (Vec<String>, usize) = (vec![], 0);
    while i < t.len() {
        let mut w = t[i].to_string();
        let versionish = !w.contains('.') && (w.chars().all(|c| c.is_ascii_digit()) || (w.starts_with(['v', 'V']) && digit(&w[1..]))
            || (w.chars().last().is_some_and(|c| c.is_ascii_digit()) && w.chars().next().is_some_and(|c| c.is_ascii_alphabetic())));
        if versionish && t.get(i + 1).is_some_and(|n| n.len() == 1 && digit(n)) {
            w = format!("{w}.{}", t[i + 1]);
            i += 1;
        }
        let lower = w.to_lowercase();
        let brand = BRAND.iter().find(|(b, _)| lower == *b || (lower.starts_with(b) && digit(&lower[b.len()..])));
        let short_code = lower.len() <= 8 && lower.chars().any(|c| c.is_ascii_digit()) && lower.chars().any(|c| c.is_ascii_alphabetic())
            && lower.chars().take_while(|c| c.is_ascii_alphabetic()).count() <= 3 && lower.chars().skip_while(|c| c.is_ascii_alphabetic()).next().is_some_and(|c| c.is_ascii_digit());
        let word = match brand {
            Some((b, name)) => format!("{name}{}", &w[b.len()..]),
            None if UPPER.contains(&lower.as_str()) || short_code => w.to_uppercase(),
            None if lower.starts_with('v') && digit(&lower[1..]) => format!("V{}", &w[1..]),
            None => { let mut c = w.chars(); c.next().map_or(String::new(), |f| f.to_uppercase().chain(c).collect()) }
        };
        if let Some(last) = out.last_mut() && (last == "GPT" || last == "GLM") && digit(&word) {
            last.push('-');
            last.push_str(&word);
        } else {
            out.push(word);
        }
        i += 1;
    }
    out.join(" ") + if free { " (free)" } else { "" }
}

/// A model switch said as 0.1.181 says it, for recordings made before: "model: claude-fable-5-2 at high for the rest of
/// the session (…)" is "model: Fable 5.2 · high from here on (…)".
pub fn model_notice(text: &str) -> Option<String> {
    let rest = text.strip_prefix("model: ")?;
    let (model, after) = rest.split_once(" at ")?;
    let (effort, tail) = after.split_once(" for the rest of the session").or_else(|| after.split_once(": "))?;
    let tail = tail.trim_start_matches(':').trim();
    Some(format!("model: {} · {effort}{}{}", model_short(model), if after.contains(" for the rest of the session") { " from here on" } else { ":" },
        if tail.is_empty() { String::new() } else { format!(" {tail}") }))
}

/// A /model row's label by name: "deepseek-v4.1-flash-1017 · fast" is "DeepSeek V4.1 Flash 1017, fast"; a label that is
/// already a name ("Opus 5.5") stays (design review 7: the desktop's picker named them, the terminal's showed ids).
pub fn pretty_label(label: &str) -> String {
    let (name, rest) = label.split_once(" · ").unwrap_or((label, ""));
    let n = if name.contains(' ') { name.to_string() } else { pretty_model(name) };
    if rest.is_empty() { n } else { format!("{n}, {rest}") }
}

/// "$ per M in / out" said evenly, as the desktop's picker: "0.017 / 0.068" for "0.0169 / 0.0677", "free" for "0 / 0".
pub fn even_price(p: &str) -> String {
    let n: Vec<Option<f64>> = p.split('/').map(|x| x.trim().parse::<f64>().ok()).collect();
    let [Some(a), Some(b)] = n[..] else { return p.to_string() };
    if a == 0.0 && b == 0.0 {
        return "free".into();
    }
    let one = |x: f64| {
        if x == 0.0 {
            "0".to_string()
        } else if x >= 1.0 {
            format!("{x:.2}").trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            let digits = (1 - x.log10().floor() as i32).max(0) as usize;
            format!("{x:.digits$}").trim_end_matches('0').trim_end_matches('.').to_string()
        }
    };
    format!("{} / {}", one(a), one(b))
}

/// session::CLAUDE_NOT_SIGNED_IN as the terminal says it: no Settings page, no Retry button.
const CLAUDE_NOT_SIGNED_IN: &str = "Claude Code isn't signed in. Run `claude auth login` in a terminal, then send your message again.";

/// The lane's notice at a session's start, said plainly: "lane default: opus-5-5 medium · repo profile (…)" is "started
/// on Opus 5.5 · medium · repo profile (…)" (it read as a debug line at the top of every recording, design review 5).
pub fn lane_notice(text: &str) -> Option<String> {
    let rest = text.strip_prefix("lane ")?.split_once(": ")?.1;
    let (model_effort, profile) = rest.split_once(" · ").unwrap_or((rest, ""));
    let (model, effort) = model_effort.rsplit_once(' ').unwrap_or((model_effort, ""));
    Some(format!("started on {}{}{}", model_short(model), if effort.is_empty() { String::new() } else { format!(" · {effort}") },
        if profile.is_empty() { String::new() } else { format!(" · {profile}") }))
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

/// A file a tool named (absolute, or under `cwd`) as git names it: from the repository's root, with `/`, so one file is
/// one file beside git's list from a subfolder or on Windows (the exit's recap counted it twice, design review 13).
/// Outside a repository, as `rel`: from `cwd`.
pub fn repo_path(cwd: &str, path: &str) -> String {
    let slashes = |p: String| if cfg!(windows) { p.replace('\\', "/") } else { p };
    let joined = std::path::Path::new(cwd).join(path);
    let full = slashes(std::fs::canonicalize(&joined).unwrap_or_else(|_| joined.clone()).to_string_lossy().into_owned());
    match repo_root(cwd) {
        Some(root) => full.strip_prefix(&root).and_then(|r| r.strip_prefix('/')).map_or(full.clone(), str::to_string),
        None => slashes(rel_to(cwd, &joined.to_string_lossy())),
    }
}

fn rel_to(cwd: &str, p: &str) -> String {
    p.strip_prefix(cwd).filter(|_| !cwd.is_empty()).map(|r| r.trim_start_matches(['/', '\\'])).unwrap_or(p).to_string()
}

/// The repository root above `cwd` (git rev-parse --show-toplevel, canonical, with `/`), asked once per folder.
pub fn repo_root(cwd: &str) -> Option<String> {
    static KEPT: std::sync::Mutex<Vec<(String, Option<String>)>> = std::sync::Mutex::new(Vec::new());
    let mut kept = KEPT.lock().ok()?;
    if let Some((_, root)) = kept.iter().find(|(c, _)| c == cwd) {
        return root.clone();
    }
    let dir = if cwd.is_empty() { std::env::current_dir().ok()? } else { std::path::PathBuf::from(cwd) };
    let root = std::process::Command::new("git").args(["rev-parse", "--show-toplevel"]).current_dir(&dir).output().ok().filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).filter(|r| !r.is_empty())
        .map(|r| std::fs::canonicalize(&r).map_or(r, |p| p.to_string_lossy().into_owned())).map(|r| if cfg!(windows) { r.replace('\\', "/") } else { r });
    kept.push((cwd.to_string(), root.clone()));
    root
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
        "WebSearch" | "ToolSearch" | "search" => i["query"].as_str().unwrap_or_default().to_string(),
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

/// How a design wants the shared transcript drawn.
pub struct Opts {
    /// Collapse runs of finished tool calls into one line (unless expanded).
    pub group_tools: bool,
    /// Show how long each tool took.
    pub durations: bool,
}

type Prefixed = (Line<'static>, Vec<Span<'static>>);

/// The bar down the side of what is set apart: your message, a quote, a tool's output. One glyph for the three; the
/// output's was the box-drawing line (design review 11).
const BAR: &str = "▎ ";

const PROMPT_LINES: usize = 8;
const TOOL_LINES: usize = 10;

/// Builds the wrapped transcript; also returns how many rows the newest block takes and how many rows under it are not
/// its own (the working row), for the effect that fades a new block in.
pub fn transcript(s: &Session, v: &View, th: &Theme, o: &Opts, width: usize) -> (Vec<Line<'static>>, (usize, usize), Vec<Option<usize>>) {
    let mut lines: Vec<Line<'static>> = Vec::new();
    // the block each line belongs to (None: the gap between blocks)
    let mut owners: Vec<Option<usize>> = Vec::new();
    let mut last_rows = 0;
    let mut i = 0;
    // a live session before its first message opens on who and where it is, as Claude Code's does; the lane's notice
    // ("lane default: opus-5-5 medium · repo profile (…)") is in it (until 0.1.180 that debug line was the whole screen)
    // (once the lane is known, and only while nothing but notices has come)
    let fresh = v.live && s.blocks.iter().all(|b| matches!(b, Block::Notice { .. } | Block::News(_)))
        && s.blocks.iter().any(|b| matches!(b, Block::Notice { text, .. } if text.starts_with("lane ")));
    if fresh {
        for l in welcome(s, th, width) {
            lines.push(l);
            owners.push(None);
        }
    }
    while i < s.blocks.len() {
        let b = &s.blocks[i];
        if fresh && matches!(b, Block::Notice { text, .. } if text.starts_with("lane ")) {
            i += 1;
            continue;
        }
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
        if i > 0 && !after_tool {
            lines.push(Line::default());
            owners.push(None);
        }
        let before = lines.len();
        let body = if grouped { group_lines(&s.blocks[i..group_end], s, v, th, o, width) } else { block_lines(b, i, s, v, th, o, open, width) };
        for (l, cont) in body {
            lines.extend(wrap(l, width.max(10), &cont));
        }
        // your message on a band the column's width, so it stands out when you scroll back (design review 13)
        if matches!(b, Block::Prompt(_)) {
            for l in &mut lines[before..] {
                l.spans.push(Span::raw(" ".repeat(width.saturating_sub(l.width()))));
                l.style = l.style.bg(th.surface);
            }
        }
        // the block under the mouse is shaded: a click opens or closes it. Your message is on the surface already: a step
        // past it, so a folded long one shows it opens (design review 13)
        if v.hover.is_some_and(|h| (i..group_end).contains(&h)) {
            let shade = if matches!(b, Block::Prompt(_)) { th.field } else { th.surface };
            for l in &mut lines[before..] {
                l.style = l.style.bg(shade);
            }
        }
        owners.resize(lines.len(), Some(i));
        last_rows = lines.len() - before;
        i = group_end;
    }
    // the run at work, at the live end: rows of no block, so a new block's fade leaves them be (design review 13)
    let before = lines.len();
    if working(s, v) && !lines.is_empty() {
        lines.push(Line::default());
        lines.extend(wrap(working_row(s, v, th), width.max(10), &[Span::raw("   ")]));
        owners.resize(lines.len(), None);
    }
    // messages typed during the run wait under it, dim, until they go: they were drawn as sent, amid its retries (design
    // review 14)
    for q in &s.queued {
        let more = if q.trim_end().contains('\n') { " …" } else { "" };
        let first = ellipsis(q.lines().next().unwrap_or_default(), width.saturating_sub(4 + more.len()));
        lines.push(Line::default());
        lines.push(Line::from(vec![Span::styled(BAR, st(th.faint)), Span::styled(format!("{first}{more}"), st(th.dim))]));
        lines.push(Line::from(vec![Span::styled(BAR, st(th.faint)), Span::styled("queued · goes after this reply", st(th.faint))]));
        owners.resize(lines.len(), None);
    }
    let tail = lines.len() - before;
    let footer = background_lines(s, v, th, width);
    if !footer.is_empty() {
        last_rows = 0; // the newest block is no longer the bottom rows: no effect on them
        owners.resize(lines.len(), None);
        for (line, owner) in footer {
            lines.push(line);
            owners.push(owner);
        }
    }
    (lines, (last_rows, tail), owners)
}

/// A run is at work and nothing else speaks for it (a question, a retry's wait and a provider's keep-alive have words of
/// their own): the working row shows, and zen's status says it in short.
pub fn working(s: &Session, v: &View) -> bool {
    v.live && v.asking.is_none() && s.retry_in(v.clock_ms).is_none() && s.waiting_note.is_none()
        && matches!(s.activity, Activity::Starting | Activity::Waiting | Activity::Thinking | Activity::Writing | Activity::Tool | Activity::Verifying)
}

/// The working row (desktop/mockups/v10.html T1): the mark pulsing between the accent and a dimmer one, "Working" with one
/// cell lit in the accent, then the time, the output so far and the step. Both move on the spinner's 80 ms frame, which is
/// drawn anyway: no timer and no frame of their own. Until design review 13 the run said only "⠋ Working 12 s", at the
/// status line's right edge.
fn working_row(s: &Session, v: &View, th: &Theme) -> Line<'static> {
    const WORD: &str = "Working";
    let frame = (v.clock_ms / 80.0) as usize;
    let pulse = 0.5 + 0.5 * (frame as f64 / 20.0 * std::f64::consts::TAU).cos();
    let mut spans = vec![Span::styled("手", st(lerp(lerp(th.accent, th.bg, 0.45), th.accent, pulse)).add_modifier(Modifier::BOLD)), Span::raw(" ")];
    let lit = frame % (WORD.len() + 5);
    spans.extend(WORD.chars().enumerate().map(|(i, c)| Span::styled(c.to_string(), st(if i == lit { th.accent } else { th.dim }).add_modifier(Modifier::BOLD))));
    let mut facts = vec![wait_str(s.working_for(v.clock_ms) / 1000.0)];
    if s.run_output() > 0 {
        facts.push(format!("↓ {} tokens", human(s.run_output())));
    }
    facts.push(doing(s));
    // each fact whole, the dot held to the one before it: the row breaks between facts at 60 columns
    spans.push(Span::styled(format!("  {}", facts.iter().map(|f| whole(f)).collect::<Vec<_>>().join("\u{a0}· ")), st(th.faint)));
    Line::from(spans)
}

/// What the run does now, in a few words: "editing main.rs", "running ls -la", "thinking".
fn doing(s: &Session) -> String {
    let open = s.blocks.iter().rev().find_map(|b| match b {
        Block::Tool(t) if t.result.is_none() && t.done_ms.is_none() => Some(t),
        _ => None,
    });
    match (s.activity, open) {
        (Activity::Tool, Some(t)) => {
            let name = short_name(&t.name);
            let verb = match name {
                "edit" | "write" | "NotebookEdit" => "editing",
                "read" => "reading",
                "bash" => "running",
                "Grep" | "Glob" | "search" | "WebSearch" => "searching",
                "fetch" | "WebFetch" => "fetching",
                _ => name,
            };
            match tool_summary(t, s) {
                what if what == "…" || what.is_empty() => verb.to_string(),
                what => format!("{verb} {}", ellipsis(&what, 40)),
            }
        }
        (Activity::Writing, _) => "writing".into(),
        (Activity::Verifying, _) => "running the tests".into(),
        _ => "thinking".into(),
    }
}

/// The rows of a running background task under the transcript are owned by `TASK_ROW + its place in View::running`, not
/// by a block: a click there opens the task.
pub const TASK_ROW: usize = usize::MAX - 64;

/// Under the transcript while background tasks run: a slowly pulsing line (the screen otherwise looks still while the
/// agent waits on them), then one row per task with what Claude Code shows for it: its time, and for a subagent its
/// tokens, tool calls and current step, as they come.
fn background_lines(s: &Session, v: &View, th: &Theme, width: usize) -> Vec<(Line<'static>, Option<usize>)> {
    let n = v.running.len();
    if n == 0 {
        return vec![];
    }
    let pulse = lerp(th.dim, th.accent, 0.5 + 0.5 * (v.clock_ms / 1600.0 * std::f64::consts::TAU).sin());
    let waiting = matches!(s.activity, Activity::Ready | Activity::Done);
    let head = format!("{} {}{n} background task{}", spinner(v.clock_ms), if waiting { "waiting on " } else { "" }, if n == 1 { "" } else { "s" });
    let mut out = vec![(Line::default(), None), (Line::from(vec![Span::styled(head, st(pulse).add_modifier(Modifier::ITALIC)), Span::styled("  click one to open it · ctrl-b: all of them", st(th.faint))]), None)];
    let cut = |s: String| if s.width() > width { format!("{}…", s.chars().take(width.saturating_sub(1)).collect::<String>()) } else { s };
    for (i, t) in v.running.iter().enumerate().take(4) {
        let who = if !t.agent.is_empty() { t.agent.clone() } else { t.kind.trim_start_matches("local_").to_string() };
        let time = if t.secs >= 60 { format!("{}m {:02}s", t.secs / 60, t.secs % 60) } else { format!("{}s", t.secs) };
        let mut facts = vec![time];
        if t.tokens > 0 {
            facts.push(format!("{} tokens", human(t.tokens)));
        }
        if t.tools > 0 {
            facts.push(format!("{} tool call{}", t.tools, if t.tools == 1 { "" } else { "s" }));
        }
        out.push((Line::from(vec![Span::styled(cut(format!("  {who} · {} · {}", t.description, facts.join(" · "))), st(th.dim))]), Some(TASK_ROW + i)));
        if !t.doing.is_empty() {
            out.push((Line::from(Span::styled(cut(format!("    └ {}", t.doing)), st(th.faint))), Some(TASK_ROW + i)));
        }
    }
    if n > 4 {
        out.push((Line::from(Span::styled(format!("  +{} more", n - 4), st(th.faint))), None));
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

/// Draws the transcript tail (or the scrolled position); returns the newest block's rows when following.
/// Draws the transcript's lines that fit, `scroll` rows up from the end. Returns the newest block's rows (when at the
/// bottom) and the block on each screen row.
/// Also returns the transcript line on the area's first row (a mouse selection holds lines, not rows).
pub fn render_transcript(f: &mut Frame, area: Rect, (lines, (last_rows, tail), owners): (Vec<Line<'static>>, (usize, usize), Vec<Option<usize>>), scroll: usize, th: &Theme) -> (Option<Rect>, Vec<Option<usize>>, usize) {
    // scrolled up: the last row says how far the end is (nothing said the view was not at the end, design review 11);
    // the transcript keeps the rows above it
    let marked = scroll > 0 && area.height > 2 && lines.len() > area.height as usize;
    let h = area.height as usize - usize::from(marked);
    let max_scroll = lines.len().saturating_sub(h);
    let start = max_scroll.saturating_sub(scroll.min(max_scroll));
    let visible: Vec<Line> = lines.into_iter().skip(start).take(h).collect();
    let shown = visible.len();
    f.render_widget(Paragraph::new(visible), area);
    if marked {
        let pill = format!(" ↓ {} ", lines_n(scroll.min(max_scroll)));
        let w = (pill.width() as u16).min(area.width);
        let at = Rect { x: area.x + (area.width - w) / 2, y: area.bottom() - 1, width: w, height: 1 };
        f.render_widget(Paragraph::new(Span::styled(pill, Style::new().fg(th.ink2).bg(th.field))), at);
    }
    let rows = owners.into_iter().skip(start).take(h).collect();
    // the newest block's rows, above the rows of no block under it
    let last = (scroll == 0 && last_rows > 0 && shown > tail).then(|| {
        let rows = last_rows.min(shown - tail) as u16;
        Rect { y: area.y + (shown - tail) as u16 - rows, height: rows, ..area }
    });
    (last, rows, start)
}

/// The line tazuna leaves in the terminal after a conversation, over the way back to it (design review 13,
/// desktop/mockups/v10.html T6): "手 tazuna · 3 turns · 2 files changed +5 −1 · $0.04 · 4 min". This sitting's runs only:
/// those after the notice a continued (`-c`) or resumed (/resume) conversation opens with, which follows the earlier
/// conversation's replayed blocks. Its files are git's since the conversation began (`git`) and those its runs' edit tools
/// changed outside git; the money as the header splits it (`cost_str`); the time is the runs' own. None before a run
/// finished: the last line was a session file's path, or nothing.
pub fn recap(s: &Session, git: &[Changed]) -> Option<String> {
    let opened = |t: &str| ["── continued from here", "── resumed session", "This conversation ran on"].iter().any(|n| t.starts_with(n));
    let from = s.blocks.iter().rposition(|b| matches!(b, Block::Notice { text, .. } if opened(text))).map_or(0, |k| k + 1);
    let runs: Vec<&crate::session::Summary> = s.blocks[from..].iter().filter_map(|b| if let Block::Done(d) = b { Some(d) } else { None }).collect();
    if runs.is_empty() {
        return None;
    }
    let mut files: Vec<(&str, u64, u64)> = git.iter().map(|c| (c.path.as_str(), c.added, c.removed)).collect();
    for c in runs.iter().flat_map(|d| &d.changes) {
        match files.iter().position(|f| f.0 == c.path) {
            Some(k) if k >= git.len() => (files[k].1, files[k].2) = (files[k].1 + c.added, files[k].2 + c.removed),
            Some(_) => {}
            None => files.push((&c.path, c.added, c.removed)),
        }
    }
    let mut parts = vec!["手 tazuna".to_string(), format!("{} turn{}", runs.len(), if runs.len() == 1 { "" } else { "s" })];
    if !files.is_empty() {
        let (added, removed) = files.iter().fold((0, 0), |(a, r), f| (a + f.1, r + f.2));
        parts.push(format!("{} changed +{added} −{removed}", files_n(files.len())));
    }
    let known: Vec<(f64, bool)> = runs.iter().filter_map(|d| d.run_cost.map(|c| (c, d.bill.as_ref().is_some_and(|b| b.plan)))).collect();
    let about = if runs.iter().any(|d| d.estimate || d.run_cost.is_none()) { "≈" } else { "" };
    let (paid, plan): (f64, f64) = (known.iter().filter(|k| !k.1).map(|k| k.0).sum(), known.iter().filter(|k| k.1).map(|k| k.0).sum());
    let plans: Vec<(String, bool)> = runs.iter().filter_map(|d| d.bill.as_ref()).filter_map(|b| Some((b.plan_name()?.to_string(), b.unpriced_plan()))).collect();
    let (word, unpriced) = plan_label(&plans);
    parts.push(match (known.is_empty(), paid > 0.0, plan > 0.0) {
        (true, ..) => "cost not known".to_string(),
        (_, true, true) => format!("{about}{} + {} {word}", usd(paid), usd(plan)),
        (_, false, true) => format!("{about}{} {word}", usd(plan)),
        // a plan without a reference price: the plan, no money (cost_str)
        (_, false, false) if unpriced => word,
        _ => format!("{about}{}", usd(paid)),
    });
    let secs: f64 = runs.iter().map(|d| d.secs).sum();
    parts.push(if secs < 60.0 { format!("{secs:.0} s") } else { format!("{} min", (secs / 60.0).round() as u64) });
    Some(parts.join(" · "))
}

/// tazuna's mark in two cells, as the window's: 手 (te, the hand of 手綱, the reins) on the accent, between half blocks that
/// round its tile off (design review 10; it was a ◆).
pub fn mark(th: &Theme) -> Vec<Span<'static>> {
    vec![Span::styled("▐", st(th.accent)), Span::styled("手", Style::new().fg(th.bg).bg(th.accent).add_modifier(Modifier::BOLD)),
        Span::styled("▌", st(th.accent))]
}

/// The first screen of a live session: tazuna and its version, the folder as `~/…` with its branch, what the start knows
/// (start_facts). The model says itself once, in the header (it stood here too, over "set on the command line; no repo
/// config": that is in the first turn's "· started on …" note; design review 11).
fn welcome(s: &Session, th: &Theme, width: usize) -> Vec<Line<'static>> {
    let dir = std::env::current_dir().unwrap_or_default();
    let cwd = tilde(&dir, crate::platform::home().as_deref());
    // a line too long for the column ends in "…"; the branch only where it fits whole beside the folder
    let room = width.saturating_sub(6);
    let cwd = ellipsis(&cwd, room);
    let branch = branch_of(&dir).filter(|b| cwd.width() + 6 + b.width() <= room);
    let mut place = vec![Span::raw("      "), Span::styled(cwd, st(th.ink2))];
    if let Some(b) = branch {
        place.extend([Span::styled("  on ", st(th.faint)), Span::styled(b, st(th.text))]);
    }
    vec![
        Line::from([vec![Span::raw(" ")], mark(th), vec![Span::raw(" "), Span::styled("tazuna", st(th.text).add_modifier(Modifier::BOLD)), Span::styled(format!(" {}", env!("CARGO_PKG_VERSION")), st(th.faint))]].concat()),
        Line::from(place),
        Line::default(),
        Line::from(vec![Span::raw("      "), Span::styled(start_facts(s, room), st(th.faint))]),
    ]
}

/// The welcome's own line, from the router's start (the "started on …" note's facts): "lean profile · 3 tracked files ·
/// /keys for every key", outside a project "chat · /keys for every key". It was a first thing to try, as Claude Code's
/// (design review 14). Where `room` is short whole facts go, the file count first, then the profile ("… /keys for every…"
/// at 60 columns); the keys' hint is cut last.
fn start_facts(s: &Session, room: usize) -> String {
    let profile = match s.profile.as_str() {
        "" => None,
        "chat" => Some("chat".to_string()),
        p => Some(format!("{p} profile")),
    };
    // the reason reads "tiny repo, 3 tracked files" or "4150 tracked files; no repo config"
    let files = s.lane["reason"].as_str().unwrap_or_default().split([',', ';']).map(str::trim).find(|p| p.ends_with(" tracked files") || p.ends_with(" tracked file"));
    let mut facts: Vec<String> = profile.into_iter().chain(files.map(String::from)).collect();
    loop {
        let line = facts.iter().map(String::as_str).chain(["/keys for every key"]).collect::<Vec<_>>().join(" · ");
        if line.width() <= room || facts.is_empty() {
            return ellipsis(&line, room);
        }
        facts.pop();
    }
}

/// The branch checked out in `dir`'s repository, from its HEAD file (no git process: the welcome is drawn every frame,
/// so the answer is kept for a second); None outside a repository and on a detached HEAD.
fn branch_of(dir: &std::path::Path) -> Option<String> {
    thread_local! { static KEPT: std::cell::RefCell<Option<(std::path::PathBuf, std::time::Instant, Option<String>)>> = const { std::cell::RefCell::new(None) }; }
    KEPT.with(|k| {
        let mut k = k.borrow_mut();
        if let Some((_, _, b)) = k.as_ref().filter(|(d, at, _)| d == dir && at.elapsed().as_secs() < 1) {
            return b.clone();
        }
        let b = read_branch(dir);
        *k = Some((dir.to_path_buf(), std::time::Instant::now(), b.clone()));
        b
    })
}

/// `dir` as `~/…` under `home`, at a path boundary only (HOME /Users/al made /Users/alice/x read "~ice/x", design review 11).
fn tilde(dir: &std::path::Path, home: Option<&std::path::Path>) -> String {
    match home.filter(|h| !h.as_os_str().is_empty()).and_then(|h| dir.strip_prefix(h).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".into(),
        Some(rest) => format!("~/{}", rest.display()),
        None => dir.display().to_string(),
    }
}

/// HEAD's `ref: refs/heads/<name>`; anything else (a detached HEAD's hash, a reftable repo's stub `refs/heads/.invalid`)
/// names no branch.
fn read_branch(dir: &std::path::Path) -> Option<String> {
    let git = dir.ancestors().map(|d| d.join(".git")).find(|g| g.exists())?;
    // a worktree's (and a submodule's) .git is a file that names the directory
    let git = if git.is_file() { git.parent()?.join(std::fs::read_to_string(&git).ok()?.trim().strip_prefix("gitdir: ")?) } else { git };
    Some(std::fs::read_to_string(git.join("HEAD")).ok()?.trim().strip_prefix("ref: refs/heads/")?.to_string()).filter(|b| !b.is_empty() && b != ".invalid")
}

/// What esc does now, said beside a run's state: " · esc stops" stood while a draft was typed, which the first esc
/// clears (design review 11). Nothing in a replay, nor in vim's modes, where esc changes mode or leaves the draft.
pub fn esc_hint(v: &View) -> &'static str {
    match (v.live, v.vim, v.input.is_empty()) {
        (false, ..) | (true, "INSERT", _) | (true, "NORMAL", false) => "",
        (true, _, true) => " · esc stops",
        _ => " · esc clears the draft",
    }
}

/// The whole transcript as the current design draws it at `width`, for copying a selection that runs off screen.
pub fn transcript_lines(s: &Session, v: &View, width: u16) -> Vec<Line<'static>> {
    let th = v.design.theme();
    let o = match v.design {
        Design::Cockpit => &cockpit::OPTS,
        Design::Zen => &zen::OPTS,
    };
    transcript(s, v, th, o, width as usize).0
}

/// The update's block: "↑ updated to 0.1.199  from 0.1.197", up to three versions with their titles, then /changelog.
#[inline(never)]
fn news_lines(n: &crate::versions::News, th: &Theme) -> Vec<Prefixed> {
    const SHOWN: usize = 3;
    let from = match n.items.len() {
        0 | 1 => format!("  from {}", n.from),
        k => format!("  from {} · {k} versions", n.from),
    };
    let mut out: Vec<Prefixed> = vec![(
        Line::from(vec![
            Span::styled("↑ ", st(th.accent)),
            Span::styled(format!("updated to {}", n.to), st(th.accent).add_modifier(Modifier::BOLD)),
            Span::styled(from, st(th.faint)),
        ]),
        vec![Span::raw("  ")],
    )];
    let pad = n.items.iter().take(SHOWN).map(|(v, _)| v.chars().count()).max().unwrap_or(0) + 2;
    for (v, t) in n.items.iter().take(SHOWN) {
        out.push((
            Line::from(vec![Span::raw("  "), Span::styled(format!("{v:<pad$}"), st(th.faint)), Span::styled(t.clone(), st(th.text))]),
            vec![Span::raw(" ".repeat(2 + pad))],
        ));
    }
    let more = n.items.len().saturating_sub(SHOWN);
    out.push((
        Line::from(vec![
            Span::raw("  "),
            Span::styled(if more > 0 { format!("and {more} more · ") } else { String::new() }, st(th.faint)),
            Span::styled("/changelog", st(th.accent)),
            Span::styled(if more > 0 { " has them all" } else { " has every note" }, st(th.faint)),
        ]),
        vec![Span::raw("  ")],
    ));
    out
}

fn block_lines(b: &Block, i: usize, s: &Session, v: &View, th: &Theme, o: &Opts, open: bool, width: usize) -> Vec<Prefixed> {
    match b {
        Block::Prompt(p) => prompt_lines(p, th, open, width, v.live),
        Block::Verify(x) => verify_lines(x, v, th),
        // a long notice (a provider's error) shows its start until clicked, then all of it
        // a provider's error reads as its message ("provider error: [404] … not found"), the raw reply on a click
        Block::Notice { text, error, .. } => {
            // claude not signed in: without the window's Settings and Retry
            let text = if text == crate::session::CLAUDE_NOT_SIGNED_IN { CLAUDE_NOT_SIGNED_IN } else { text.as_str() };
            let shown = if open { text.to_string() } else { readable_notice(text).or_else(|| lane_notice(text)).or_else(|| model_notice(text)).unwrap_or_else(|| text.to_string()) };
            vec![(
                Line::from(vec![
                    Span::styled(if *error { "! " } else { "· " }, st(if *error { th.err } else { th.faint })),
                    // cut only where that saves room: "(+6 chars · click to show)" took 26 to hide 6 (design review 11)
                    Span::styled(if open || shown.chars().count() <= NOTICE_CHARS + 30 { shown } else {
                        format!("{}… (+{} chars · {})", shown.chars().take(NOTICE_CHARS).collect::<String>(), shown.chars().count() - NOTICE_CHARS, if v.live { "click to show" } else { "e to expand" })
                    }, st(if *error { th.err } else { th.dim })),
                ]),
                vec![Span::raw("  ")],
            )]
        }
        // the first session after an upgrade (versions::update_news): a heading, then a line per version brought (its
        // title under the version, wrapped beside it), then where every note is. One dim line of "   " gaps read as a run-on
        // sentence (the user, 2026-11-23: "it should be new line with styling so its obvious")
        Block::News(n) => news_lines(n, th),
        // a refused call being tried again: what failed, which try, what happens next and the key that stops it, in the
        // error colour while it lasts; the provider's own words when the block is opened
        Block::Retry(r) => {
            let loud = r.next_ms.is_some() || r.gave_up;
            let color = if loud { th.err } else { th.dim };
            let tries = if r.tries == 1 { "1 try".to_string() } else { format!("{} tries", r.tries) };
            let (state, next) = match (r.next_ms, r.gave_up) {
                (Some(t), _) => (format!("try {} failed", r.tries), format!("trying again in {}{}", wait_str((t - v.clock_ms) / 1000.0), esc_hint(v))),
                (None, true) => (format!("gave up after {tries}"), "send the message again, or pick another model with /model".to_string()),
                (None, false) => (format!("{tries} failed"), String::new()),
            };
            let mut out = vec![(
                Line::from(vec![
                    Span::styled(if loud { "! " } else { "· " }, st(if loud { th.err } else { th.faint })),
                    Span::styled(r.why.clone(), st(color).add_modifier(if loud { Modifier::BOLD } else { Modifier::empty() })),
                    Span::styled(format!("  {}", whole(&state)), st(color)),
                    Span::styled(if open { FOLD_OPEN } else { FOLD_SHUT }, st(th.faint)),
                ]),
                vec![Span::raw("  ")],
            )];
            for (text, c) in [(next, th.dim), (if open { r.raw.clone() } else { String::new() }, th.faint)] {
                if !text.is_empty() {
                    out.push((Line::from(vec![Span::raw("  "), Span::styled(text, st(c))]), vec![Span::raw("  ")]));
                }
            }
            out
        }
        Block::Text { text, live } => {
            // no mark before a reply in either design: zen had none, cockpit a ▸ that was also the fold's arrow (design
            // review 11)
            let mut out = markdown(if s.hidden(i) { "" } else { text }, th, width);
            if out.is_empty() {
                out.push((Line::default(), vec![]));
            }
            if *live {
                if let Some((l, _)) = out.last_mut() {
                    // in a code block's row the cursor takes the first cell of the fill, not one past the block's edge
                    match l.spans.last().filter(|p| !p.content.is_empty() && p.content.chars().all(|c| c == ' ')).map(|p| p.style) {
                        Some(fill) => {
                            let at = l.spans.len() - 1;
                            l.spans[at].content = l.spans[at].content[1..].to_string().into();
                            l.spans.insert(at, Span::styled("▍", fill.fg(th.accent)));
                        }
                        None => l.spans.push(Span::styled("▍", st(th.accent))),
                    }
                }
            }
            out
        }
        Block::Thinking { tokens, live, exact, .. } => {
            // finished: the faint ⌁ of the folded runs' "⌁ ran 4 tools" (it was Claude Code's ✻, design review 14)
            let glyph = if *live { spinner(v.clock_ms) } else { "⌁" };
            let mut spans = vec![Span::styled(format!("{glyph} "), st(if *live { th.accent } else { th.faint })), Span::styled("thinking", st(th.dim).add_modifier(Modifier::ITALIC))];
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
                (true, _) => ("■", "Stopped", th.dim),
                (false, true) => ("✓", "Done", th.ok),
                (false, false) => ("✗", "Failed", th.err),
            };
            let u = &d.usage;
            // the repo's changes when there are any (git); else the files an edit tool wrote: a path guessed from a command
            // line counted grep patterns and /tmp scripts as files (2026-11-04)
            let files = if v.changed.is_empty() { files_touched(s).len() } else { v.changed.len() };
            // the session in one row: sent ↑, calls, the cache's share and size, output ↓, files, and how fast
            let (ttft, tps, e2e) = crate::session::speed(&s.calls);
            let mut facts = vec![format!("↑ {}", human(u.total_in())), format!("{} call{}", s.turns, if s.turns == 1 { "" } else { "s" }),
                format!("cache {:.0}% ({})", u.cache_hit() * 100.0, human(u.cache_read)), format!("↓ {}", human(u.output)), files_n(files)];
            if let Some(f) = ttft {
                facts.push(format!("{} to first token", tenths(f / 1000.0)));
            }
            // writing (after the first token) and end to end (the waits included), each where known
            match (tps, e2e) {
                (Some(r), Some(e)) => facts.extend([format!("{r:.0} t/s writing"), format!("{e:.0} end to end")]),
                (Some(r), None) => facts.push(format!("{r:.0} t/s writing")),
                (None, Some(e)) => facts.push(format!("{e:.0} t/s end to end")),
                (None, None) => {}
            }
            // one short line under each answer: how long and what it cost (design review, 2026-11-19). The second
            // line of numbers was the whole conversation's so far, the same under every answer and wrapping onto two
            // lines: it shows when the line is clicked, or with everything expanded.
            // tenths under 10 s: "Done 0 s" for a quick turn (design review 14)
            let took = if d.secs < 10.0 { tenths(d.secs) } else if d.secs < 60.0 { format!("{:.0} s", d.secs) } else { format!("{} min {:02} s", d.secs as u64 / 60, d.secs as u64 % 60) };
            // an estimate says so, and a cost nobody can tell is not shown as one (design review 12); a plan without a
            // reference price: the plan, not "$0" (the user, 2026-11-25)
            let cost = match d.bill.as_ref() {
                Some(b) if b.unpriced_plan() => plan_word(b.plan_name().unwrap_or_default()),
                _ => d.run_cost.map_or("cost not known".into(), |c| format!("{}{}", if d.estimate { "≈" } else { "" }, usd(c))),
            };
            let mut first = vec![Span::styled(format!("{glyph} {word}"), st(color).add_modifier(Modifier::BOLD)), Span::styled(format!("  {}  {}", whole(&took), whole(&cost)), st(th.dim))];
            // what the run changed, file by file, without ctrl-o: it was folded into "ran 4 tools" until opened (design
            // review 13, desktop/mockups/v10.html T3). Each file whole, so the row wraps between them.
            // (worked out once, when the run ended: Summary::changes); a file written over whole says so, not "+200"
            let files = &d.changes;
            const SHOWN: usize = 4;
            for (k, c) in files.iter().take(SHOWN).enumerate() {
                let counts = [(c.added > 0 || c.removed == 0).then(|| format!("+{}", c.added)), (c.removed > 0).then(|| format!("−{}", c.removed))];
                first.push(Span::styled(format!("{}{}", if k == 0 { "   " } else { "  " }, whole(&c.path)), st(th.text)));
                let said = if c.rewritten { "rewritten".to_string() } else { counts.into_iter().flatten().collect::<Vec<_>>().join("\u{a0}") };
                first.push(Span::styled(format!("\u{a0}{said}"), st(if c.rewritten { th.dim } else { th.ok })));
            }
            if files.len() > SHOWN {
                first.push(Span::styled(format!("  {}", whole(&format!("+{} more", files.len() - SHOWN))), st(th.faint)));
            }
            if !files.is_empty() && !v.expand_all {
                first.push(Span::styled(format!("   {}", whole(if v.live { "ctrl-o diffs" } else { "e diffs" })), st(th.faint)));
            } else if files.is_empty() && !open {
                first.push(Span::styled(FOLD_SHUT, st(th.faint)));
            }
            let mut out = vec![(Line::from(first), vec![Span::raw("  ")])];
            if open {
                // rows of whole facts, the separator between two on a row only: it wrapped as "· 424" / "tok/s end to end"
                // (design review 13), then with a "·" left at a row's end (14)
                for row in fact_rows("so far:", &facts, width.saturating_sub(2)) {
                    out.push((Line::from(Span::styled(format!("  {row}"), st(th.dim))), vec![Span::raw("  ")]));
                }
            }
            out
        }
    }
}

const VERIFY_FAIL_LINES: usize = 8;

fn verify_lines(x: &crate::session::Verify, v: &View, th: &Theme) -> Vec<Prefixed> {
    let (glyph, color, state) = match x.ok {
        _ if x.no_tests => ("·", th.faint, "no tests found".to_string()),
        _ if x.timed_out => ("·", th.faint, format!("timed out after {:.0}s · not verified", x.secs)),
        _ if x.cannot_run => ("·", th.faint, "could not run (command not found) · not verified".to_string()),
        None => (spinner(v.clock_ms), th.accent, "running…".to_string()),
        Some(true) => ("✓", th.ok, format!("passed · {}", tenths(x.secs))),
        Some(false) => ("✗", th.err, format!("failed · {}", tenths(x.secs))),
    };
    let round = if x.round > 1 { format!(" (round {})", x.round) } else { String::new() };
    let mut out = vec![(
        Line::from(vec![
            Span::styled(format!("{glyph} "), st(color)),
            Span::styled("verify", st(th.ink2).add_modifier(Modifier::BOLD)),
            Span::styled(format!(" {}{round}", x.cmd), st(th.dim)),
            Span::styled(format!("  · {state}"), st(th.faint)),
        ]),
        vec![Span::raw("         ")],
    )];
    if x.ok == Some(false) {
        let lines: Vec<&str> = x.output.lines().filter(|l| !l.trim().is_empty()).collect();
        let shown = if v.expand_all { lines.len() } else { VERIFY_FAIL_LINES.min(lines.len()) };
        for l in &lines[lines.len() - shown..] {
            out.push((Line::from(vec![Span::raw("    "), Span::styled(BAR, st(th.faint)), Span::styled(l.replace('\t', "    "), st(th.err))]), vec![Span::raw("      ")]));
        }
    }
    out
}

/// Your message, the same in both designs: an accent bar down its side and upright text in the second ink, on a band
/// (`transcript`). Zen set it in dim italics, which read as a note from the model, and cockpit under a "│ you" label
/// (design review 13, desktop/mockups/v10.html T2). A long one shows its first PROMPT_LINES lines until it is opened (a
/// click on it, or ctrl+o for every block).
fn prompt_lines(p: &str, th: &Theme, open: bool, width: usize, live: bool) -> Vec<Prefixed> {
    let md = markdown(p, th, width.saturating_sub(4));
    let keep = if open { md.len() } else { PROMPT_LINES };
    let more = md.len().saturating_sub(keep);
    let fold_hint = if live { "click to show" } else { "e to expand" };
    let bar = || Span::styled(BAR, st(th.accent));
    let mut out: Vec<Prefixed> = md.into_iter().take(keep).map(|(l, mut cont)| {
        let ink = l.spans.into_iter().map(|sp| if sp.style.fg == Some(th.text) { Span::styled(sp.content, sp.style.fg(th.ink2)) } else { sp });
        cont.insert(0, bar());
        (Line::from([vec![bar()], ink.collect()].concat()), cont)
    }).collect();
    if more > 0 {
        out.push((Line::from(vec![bar(), Span::styled(format!("… {more} more lines · {fold_hint}"), st(th.faint))]), vec![bar()]));
    }
    out
}

fn tool_lines(t: &Tool, s: &Session, v: &View, th: &Theme, o: &Opts, open: bool, width: usize) -> Vec<Prefixed> {
    let arg = |k: &str| tool_arg(t, k);
    let name = short_name(&t.name);
    let (glyph, gcolor) = match (&t.result, t.error) {
        (None, _) if asked(t, s, v) => ("?", th.warn),
        // cut off when its run was stopped: it has an end and no result
        (None, _) if t.done_ms.is_some() => ("■", th.dim),
        (None, _) => (spinner(v.clock_ms), th.accent),
        (Some(_), _) if t.declined() => ("■", th.dim),
        (Some(_), false) => ("✓", th.ok),
        (Some(_), true) => ("✗", th.err),
    };
    // each fact whole, so the row's fold mark takes no half of one with it (`wrap`)
    let mut meta = whole(&match (&t.result, name) {
        (Some(_), _) if t.declined() => "declined".to_string(),
        // a failed command says how it ended ("exit 1"), anything else "error"
        (Some(r), _) if t.error => crate::session::exit_line(r).and_then(|l| l.strip_prefix("[exit ")).map_or("error".to_string(), |c| format!("exit {}", c.trim_end_matches(']'))),
        (Some(_), "edit") => {
            let (added, removed) = change_counts(arg("old"), arg("new"));
            format!("+{added} −{removed}")
        }
        (Some(_), "write") => lines_n(arg("content").lines().count()),
        (Some(r), _) => lines_n(r.lines().count()),
        (None, _) if t.done_ms.is_some() => "stopped".to_string(),
        (None, _) if asked(t, s, v) => "waits for your answer".to_string(),
        (None, _) => String::new(),
    });
    if let Some((n, last)) = &t.sub {
        // a running subagent: its progress, not its whole transcript
        let last: String = last.chars().take(60).collect();
        meta = if meta.is_empty() { format!("{} · {last}", whole(&format!("{n} steps"))) } else { format!("{meta} · {}", whole(&format!("{n} steps"))) };
    }
    if let (true, Some(secs)) = (o.durations, t.secs()) {
        meta.push_str(&format!(" · {}", tenths(secs)));
    }
    let gutter = || vec![Span::raw("    "), Span::styled(BAR, st(th.faint))];
    // closed, the call is one short summary (a subagent's whole prompt ran 25 rows, 2026-11-02); opened (a click, or
    // ctrl-o for all), its full input and output
    let summary = tool_summary(t, s);
    let cut = !open && summary.chars().count() > SUMMARY_CHARS;
    let summary = if cut { format!("{}…", summary.chars().take(SUMMARY_CHARS).collect::<String>()) } else { summary };
    let prompt = if matches!(name, "Agent" | "Task") { t.input["prompt"].as_str().unwrap_or_default() } else { "" };
    let more = cut || !prompt.is_empty() || (!t.declined() && t.result.as_deref().is_some_and(|r| !r.trim().is_empty()));
    if more && !matches!(name, "edit" | "TodoWrite") {
        meta.push_str(if open { FOLD_OPEN } else { FOLD_SHUT });
    }
    // the counts and time on the same row when it all fits, else on a row of their own under the summary (wrapped
    // straight after a long command they were hard to find)
    let meta = if meta.is_empty() { String::new() } else { format!("  · {meta}") };
    let fits = 2 + name.width().max(6) + 1 + summary.width() + meta.width() <= width;
    let mut head = vec![
        Span::styled(format!("{glyph} "), st(gcolor)),
        Span::styled(format!("{name:<6}"), st(th.ink2).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {summary}"), st(th.dim)),
    ];
    if fits {
        head.push(Span::styled(meta.clone(), st(th.faint)));
    }
    let mut out = vec![(Line::from(head), vec![Span::raw("         ")])];
    if !fits && !meta.is_empty() {
        out.push((Line::from(vec![Span::raw("         "), Span::styled(meta.trim_start().to_string(), st(th.faint))]), vec![Span::raw("         ")]));
    }
    // a click opens one call whole; ctrl-o opens all, each up to a screenful
    let limit = if name == "edit" && !open { TOOL_LINES } else if name == "TodoWrite" { 30 } else if v.open.is_empty() || v.expand_all { TOOL_LINES * 2 } else { 400 };
    // edits always show their diff: it is the most useful thing on screen; an opened write its content, as all new
    match name {
        "edit" => return [out, diff_rows(arg("old"), arg("new"), t.at.clone(), th, width, limit)].concat(),
        "write" if open => return [out, diff_rows("", arg("content"), Some((1, None, None)), th, width, limit)].concat(),
        _ => {}
    }
    let body: Vec<(String, Color)> = match name {
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
                    Some("in_progress") => ("◐", th.accent),
                    _ => ("☐", th.dim),
                };
                (format!("{mark} {}", x["content"].as_str().unwrap_or_default()), c)
            })
            .collect(),
        // a JSON reply laid out, and a line past three screen widths cut: one 4,000-character line of a gh query filled
        // the screen as a wall (design review 5)
        _ if open && !t.declined() => {
            let raw = t.result.as_deref().unwrap_or_default();
            let pretty = raw.trim_start().starts_with(['{', '[']).then(|| serde_json::from_str::<serde_json::Value>(raw).ok()).flatten()
                .and_then(|j| serde_json::to_string_pretty(&j).ok());
            let room = width.max(40) * 3;
            pretty.as_deref().unwrap_or(raw).lines().map(|l| {
                let l = if l.chars().count() > room { format!("{}…", l.chars().take(room).collect::<String>()) } else { l.to_string() };
                (l, if t.error { th.err } else { th.dim })
            }).collect()
        }
        _ => vec![],
    };
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
fn group_lines(blocks: &[Block], s: &Session, v: &View, th: &Theme, o: &Opts, width: usize) -> Vec<Prefixed> {
    let tools: Vec<&Tool> = blocks.iter().filter_map(|b| if let Block::Tool(t) = b { Some(t) } else { None }).collect();
    let thought: u64 = blocks.iter().map(|b| if let Block::Thinking { tokens, .. } = b { *tokens } else { 0 }).sum();
    let approx = if blocks.iter().any(|b| matches!(b, Block::Thinking { exact: false, .. })) { "~" } else { "" };
    let thinking_now = matches!(blocks.last(), Some(Block::Thinking { live: true, .. }));
    // the call a permission question is about has not run: it waits for the answer ("? ran 1 tool … 0 lines", design review 11)
    let waits = tools.iter().copied().find(|t| asked(t, s, v));
    // any other open call keeps the spinner and its line
    let running = tools.iter().copied().find(|t| t.result.is_none() && !waits.is_some_and(|w| std::ptr::eq(*t, w)));
    let mut counts: Vec<(&str, usize)> = Vec::new();
    // what ran: not the one that waits, nor one the user said no to
    // (nor, while a question is open, the calls queued behind it: "⠋ ran 2 tools" over a question about the first)
    let did: Vec<&Tool> = tools.iter().copied().filter(|t| !(waits.is_some() && t.result.is_none()) && !t.declined()).collect();
    for t in &did {
        let n = short_name(&t.name);
        match counts.iter_mut().find(|(k, _)| *k == n) {
            Some((_, c)) => *c += 1,
            None => counts.push((n, 1)),
        }
    }
    let lines: usize = did.iter().filter_map(|t| t.result.as_ref()).map(|r| r.lines().count()).sum();
    let said_no = tools.iter().filter(|t| t.declined()).count();
    let errors = did.iter().filter(|t| t.error).count();
    let ran = did.len();
    // each count whole: the row wraps between them ("2" / "failed" at 80 columns, design review 13)
    let mix = counts.iter().map(|(n, c)| whole(&format!("{n} ×{c}"))).collect::<Vec<_>>().join("  ");
    let busy = (running.is_some() || thinking_now) && waits.is_none();
    let what = match (ran, thinking_now) {
        (0, false) if said_no > 0 => format!("declined {}", tools.iter().filter(|t| t.declined()).map(|t| short_name(&t.name)).collect::<Vec<_>>().join(", ")),
        (0, _) => "thinking".to_string(),
        (n, true) => format!("ran {n} tools, thinking"),
        (1, false) => "ran 1 tool".to_string(),
        (n, false) => format!("ran {n} tools"),
    };
    let mut spans = vec![
        // the call a question is about waits for the user: no spinner, and it is drawn under this line, not counted in it
        Span::styled(if busy { format!("{} ", spinner(v.clock_ms)) } else { "⌁ ".into() }, st(if busy { th.accent } else { th.faint })),
        Span::styled(whole(&what), st(th.dim)),
        Span::styled(if ran == 0 { String::new() } else { format!("   {mix}   {}", whole(&lines_n(lines))) }, st(th.faint)),
    ];
    if thought > 0 {
        spans.push(Span::styled(format!("   {}", whole(&format!("thought {approx}{thought} tok"))), st(th.faint)));
    }
    if errors > 0 {
        spans.push(Span::styled(format!("   {}", whole(&format!("{errors} failed"))), st(th.err)));
    }
    if said_no > 0 && ran > 0 {
        spans.push(Span::styled(format!("   {}", whole(&format!("{said_no} declined"))), st(th.faint)));
    }
    let mut out = vec![(Line::from(spans), vec![Span::raw("  ")])];
    if waits.is_some() && ran == 0 && thought == 0 && said_no == 0 {
        out.clear(); // nothing ran before the question
    }
    // what the question is about, as the other designs show it: the command, the edit's diff, the file's content. Zen
    // asked "allow edit: main.rs" with only the file's name above it (design review 11)
    // a plan to approve is the text above the question, a question to the user is the question: their call's raw JSON says
    // neither again; any other tool is its one-line summary
    if let Some(t) = waits.filter(|t| !matches!(t.name.as_str(), "ExitPlanMode" | "AskUserQuestion")) {
        out.extend(tool_lines(t, s, v, th, o, matches!(short_name(&t.name), "bash" | "edit" | "write"), width));
    }
    if let Some(t) = running {
        out.push((Line::from(vec![Span::raw("  "), Span::styled(tool_summary(t, s), st(th.faint).add_modifier(Modifier::ITALIC))]), vec![Span::raw("  ")]));
    }
    out
}

/// Just enough markdown for agent replies: headings, bullets, code fences, `code`, **bold**.
fn markdown(text: &str, th: &Theme, width: usize) -> Vec<Prefixed> {
    let mut out: Vec<Prefixed> = Vec::new();
    let mut in_code = false;
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let raw = lines[i];
        i += 1;
        // a table: `| a | b |` rows, the second a `|---|---|` rule
        if !in_code && raw.trim_start().starts_with('|') && lines.get(i).is_some_and(|n| is_rule(n)) {
            let mut rows = vec![cells(raw)];
            // `---:` under a column: its cells stand to the right (numbers)
            let right: Vec<bool> = cells(lines[i]).iter().map(|c| c.ends_with(':') && !c.starts_with(':')).collect();
            i += 1;
            while i < lines.len() && lines[i].trim_start().starts_with('|') {
                rows.push(cells(lines[i]));
                i += 1;
            }
            out.extend(table(&rows, &right, th, width));
            continue;
        }
        if let Some(lang) = raw.trim_start().strip_prefix("```") {
            in_code = !in_code;
            // the block's language on its first row, faint (the window's code block names it)
            if in_code && !lang.trim().is_empty() {
                out.extend(code_rows(lang.trim(), 0, st(th.faint), th, width));
            }
            continue;
        }
        if in_code {
            // a wrapped line hangs under its own indent and a little more, as an editor's soft wrap (it went back to the
            // first column, design review 10)
            let hang = 1 + raw.chars().take_while(|c| c.is_whitespace()).count() + 4;
            out.extend(code_rows(raw, hang, st(th.ink2), th, width));
            continue;
        }
        let t = raw.trim_start();
        // a formula on lines of its own between `$$` or `\[ \]`: read to its end first. Every line simple TeX: the lines as
        // maths without the fences; else the block as written, fences and all. Unclosed (still streaming), it is text.
        if let Some(close) = match t.trim_end() { "$$" => Some("$$"), "\\[" => Some("\\]"), _ => None }
            && let Some(len) = lines[i..].iter().position(|l| l.trim() == close)
        {
            let body = &lines[i..i + len];
            let set: Option<Vec<String>> = body.iter().map(|l| math(l.trim())).collect();
            match set.filter(|_| body.iter().any(|l| looks_like_tex(l, true))) {
                Some(rows) => out.extend(rows.into_iter().map(|m| (Line::from(Span::styled(m, st(th.text).add_modifier(Modifier::ITALIC))), vec![]))),
                None => out.extend(lines[i - 1..=i + len].iter().map(|l| (Line::from(Span::styled(l.to_string(), st(th.text))), vec![]))),
            }
            i += len + 1;
            continue;
        }
        // `---` alone on its line: a rule across the column (its three dashes showed, design review 11)
        if t.len() >= 3 && ["-", "*", "_"].iter().any(|c| t.trim_end().chars().all(|x| c.starts_with(x))) {
            out.push((Line::from(Span::styled("─".repeat(width.max(1)), st(th.border))), vec![]));
            continue;
        }
        if let Some(q) = t.strip_prefix('>') {
            // a quote: a bar and its words in the second ink, as the window draws it (it printed its ">", design review 10)
            let bar = || Span::styled(BAR, st(th.accent));
            let words = inline(q.trim_start_matches('>').trim_start(), th).into_iter().map(|mut w| {
                if w.style.fg == Some(th.text) {
                    w.style = w.style.fg(th.ink2);
                }
                w
            });
            out.push((Line::from([vec![bar()], words.collect()].concat()), vec![bar()]));
        } else if let Some(h) = t.strip_prefix('#') {
            // air above it and an accent mark before it: a heading sat straight under the paragraph above and read as one
            // more bold line (design review 13, desktop/mockups/v10.html T5)
            if out.last().is_some_and(|(l, _)| l.width() > 0) {
                out.push((Line::default(), vec![]));
            }
            out.push((Line::from(vec![Span::styled("▍", st(th.accent)), Span::styled(h.trim_start_matches('#').trim().to_string(), st(th.text).add_modifier(Modifier::BOLD))]),
                vec![Span::raw(" ")]));
        } else if let Some((num, item)) = t.split_once(". ").filter(|(n, _)| (1..=3).contains(&n.len()) && n.bytes().all(|b| b.is_ascii_digit())) {
            // numbered item: wrapped rows hang under the text, not the number
            let indent = " ".repeat(raw.len() - t.len());
            let mut spans = vec![Span::raw(indent.clone()), Span::styled(format!("{num}. "), st(th.dim))];
            spans.extend(inline(item, th));
            out.push((Line::from(spans), vec![Span::raw(format!("{indent}{}", " ".repeat(num.len() + 2)))]));
        } else if let Some(item) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
            let indent = " ".repeat(raw.len() - t.len());
            // a task list's boxes, drawn: "- [x] done" showed its brackets (design review 9)
            let (mark, item) = match item.get(..4) {
                Some("[x] " | "[X] ") => (Span::styled("☑ ", st(th.ok)), &item[4..]),
                Some("[ ] ") => (Span::styled("☐ ", st(th.faint)), &item[4..]),
                _ => (Span::styled("• ", st(th.dim)), item),
            };
            let mut spans = vec![Span::raw(indent.clone()), mark];
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
            // code in a sentence: ink on the field, as the desktop app's (every identifier in a second accent read as
            // a page of links, design review 2026-11-19)
            spans.push(Span::styled(part.to_string(), Style::new().fg(th.text).bg(th.field)));
            continue;
        }
        for (j, seg) in part.split("**").enumerate() {
            let bold = if j % 2 == 1 { st(th.text).add_modifier(Modifier::BOLD) } else { st(th.text) };
            for (text, url) in links(seg) {
                // a link shows its title only, underlined; the address is attached where it is drawn (`link_cells`)
                if url.is_some() {
                    spans.push(Span::styled(text, bold.fg(th.accent).add_modifier(Modifier::UNDERLINED)));
                    continue;
                }
                for (run, maths) in tex(&text) {
                    // simple maths in italics, as text (design review 13: it printed its TeX); a link's title stays as it is
                    if maths {
                        spans.push(Span::styled(run, bold.add_modifier(Modifier::ITALIC)));
                        continue;
                    }
                    // *emphasis* in italics and ~~struck~~ text struck: their marks showed (design review 9); a lone "~~"
                    // stays as written
                    let struck = run.matches("~~").count() % 2 == 0;
                    for (k, piece) in if struck { run.split("~~").collect() } else { vec![run.as_str()] }.into_iter().enumerate() {
                        let base = if k % 2 == 1 { bold.add_modifier(Modifier::CROSSED_OUT) } else { bold };
                        for (part, em) in emphasis(piece) {
                            spans.push(Span::styled(part, if em { base.add_modifier(Modifier::ITALIC) } else { base }));
                        }
                    }
                }
            }
        }
    }
    spans.retain(|s| !s.content.is_empty());
    spans
}

/// Text split into plain runs and simple maths (true), the maths as text (`math`) without its `$$…$$`, `\(…\)`, `\[…\]`
/// or `$…$` fences. Only what reads as TeX counts (`looks_like_tex`): shell and regular expressions use the same marks
/// ("$XDG_CONFIG_HOME/$APP_NAME", "cd $(git rev-parse …)/$DIR", "kill $$", "\(abc\)" in a BRE), and a formula `math` cannot
/// set stays as written, fences and all: half-converted TeX reads worse than its source. A `$…$` also counts only as Pandoc
/// reads it (no space inside either `$`, no digit after the closing one, design review 13).
fn tex(s: &str) -> Vec<(String, bool)> {
    let (mut out, mut plain, mut rest) = (vec![], String::new(), s);
    while let Some(at) = rest.find(['$', '\\']) {
        let fence = [("$$", "$$"), ("\\(", "\\)"), ("\\[", "\\]"), ("$", "$")].into_iter().find(|(o, _)| rest[at..].starts_with(o));
        let found = fence.and_then(|(o, c)| {
            let from = at + o.len();
            let end = from + rest[from..].find(c)?;
            let body = &rest[from..end];
            let pandoc = o != "$" || !(body.starts_with([' ', '(', '{']) || body.ends_with(' ') || rest[end + 1..].starts_with(|d: char| d.is_ascii_digit()));
            if pandoc && looks_like_tex(body, o == "$$") { math(body).map(|m| (m, end + c.len())) } else { None }
        });
        match found {
            Some((m, end)) => {
                plain.push_str(&rest[..at]);
                out.push((std::mem::take(&mut plain), false));
                out.push((m, true));
                rest = &rest[end..];
            }
            None => {
                let step = at + 1; // `$` and `\` are one byte
                plain.push_str(&rest[..step]);
                rest = &rest[step..];
            }
        }
    }
    plain.push_str(rest);
    out.push((plain, false));
    out.retain(|(t, _)| !t.is_empty());
    out
}

/// TeX and not shell or a pattern: a command (`\Theta`) or a braced index or power (`_{`, `^{`); a bare `^` only in a
/// display formula (`display`: `$$`), as "/^start$/,/^end$/" and "$x^2$y" are patterns and shell. Words, a lone `_` and
/// brackets are not enough: "$HOME_DIR/$X", "$(cmd)" and "\(abc\)" read as maths with them.
fn looks_like_tex(body: &str, display: bool) -> bool {
    (display && body.contains('^')) || body.contains("_{") || body.contains("^{") || body.as_bytes().windows(2).any(|w| w[0] == b'\\' && w[1].is_ascii_alphabetic())
}

/// Simple TeX as text: Greek letters and a few symbols as Unicode, `t_{lookup}` as `t_lookup`; None for anything else
/// (`\frac`, `\sum`, braces left over).
fn math(body: &str) -> Option<String> {
    const NAMES: [(&str, &str); 45] = [("alpha", "α"), ("beta", "β"), ("gamma", "γ"), ("delta", "δ"), ("epsilon", "ε"), ("varepsilon", "ε"), ("zeta", "ζ"),
        ("eta", "η"), ("theta", "θ"), ("iota", "ι"), ("kappa", "κ"), ("lambda", "λ"), ("mu", "μ"), ("nu", "ν"), ("xi", "ξ"), ("pi", "π"), ("rho", "ρ"),
        ("sigma", "σ"), ("tau", "τ"), ("phi", "φ"), ("varphi", "φ"), ("chi", "χ"), ("psi", "ψ"), ("omega", "ω"), ("Gamma", "Γ"), ("Delta", "Δ"),
        ("Theta", "Θ"), ("Lambda", "Λ"), ("Xi", "Ξ"), ("Pi", "Π"), ("Sigma", "Σ"), ("Phi", "Φ"), ("Psi", "Ψ"), ("Omega", "Ω"), ("times", "×"),
        ("cdot", "·"), ("le", "≤"), ("leq", "≤"), ("ge", "≥"), ("geq", "≥"), ("ne", "≠"), ("neq", "≠"), ("to", "→"), ("infty", "∞"), ("approx", "≈")];
    let (mut out, mut rest) = (String::new(), body.trim());
    while let Some(c) = rest.chars().next() {
        if c == '\\' {
            let name: String = rest[1..].chars().take_while(char::is_ascii_alphabetic).collect();
            out.push_str(NAMES.iter().find(|(n, _)| *n == name)?.1);
            rest = &rest[1 + name.len()..];
        } else if matches!(c, '_' | '^') && rest[1..].starts_with('{') {
            let end = rest.find('}')?;
            let inner = &rest[2..end];
            if inner.contains(['{', '\\']) {
                return None;
            }
            out.push(c);
            out.push_str(inner);
            rest = &rest[end + 1..];
        } else if matches!(c, '{' | '}') {
            return None;
        } else {
            out.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    Some(out)
}

/// Text split into plain runs and `*emphasised*` ones (true), without their asterisks. An asterisk counts only where
/// emphasis can start or end: not before or after a space, and a closing one not inside a word, so `a * b`, `2*3*4` and
/// `*.rs` stay as they are.
fn emphasis(s: &str) -> Vec<(String, bool)> {
    let c: Vec<char> = s.chars().collect();
    let opens = |i: usize| c[i] == '*' && c.get(i + 1).is_some_and(|n| !n.is_whitespace() && *n != '*') && (i == 0 || !c[i - 1].is_alphanumeric());
    let closes = |i: usize| c[i] == '*' && i > 0 && !c[i - 1].is_whitespace() && c.get(i + 1).is_none_or(|n| !n.is_alphanumeric());
    let (mut out, mut plain, mut i) = (vec![], String::new(), 0);
    while i < c.len() {
        if opens(i) && let Some(end) = (i + 2..c.len()).find(|&k| closes(k)) {
            out.push((std::mem::take(&mut plain), false));
            out.push((c[i + 1..end].iter().collect(), true));
            i = end + 1;
            continue;
        }
        plain.push(c[i]);
        i += 1;
    }
    out.push((plain, false));
    out.retain(|(t, _)| !t.is_empty());
    out
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

/// One line of a code block as rows of the block's whole width: wrapped under `hang` columns, every row filled to
/// `width` on the field colour, so the block is one even shape. The background followed each line's length, ragged down
/// its right side (design review 11). Wrapped here, not by the transcript: a row has to know where it ends to be filled.
fn code_rows(text: &str, hang: usize, style: Style, th: &Theme, width: usize) -> Vec<Prefixed> {
    let bg = Style::new().bg(th.field);
    let width = width.max(4);
    wrap(Line::from(Span::styled(format!(" {text}"), style.bg(th.field))), width - 1, &[Span::styled(" ".repeat(hang.min(width / 2)), bg)]).into_iter().map(|mut l| {
        let fill = width.saturating_sub(l.width());
        l.spans.push(Span::styled(" ".repeat(fill), bg));
        (l, vec![])
    }).collect()
}

/// A markdown table as the window draws it: a quiet header, one rule under it, the rows under each other with no grid
/// and no blank row between them; a `---:` column stands to the right. Its columns are fitted to `width`: a column wider
/// than its share wraps inside its cell. Too narrow for that, the rows read as "a · b · c". Until 0.1.193 a full grid of
/// box lines with a rule between wrapped rows (design review 11, desktop/mockups/v8.html).
fn table(rows: &[Vec<String>], right: &[bool], th: &Theme, width: usize) -> Vec<Prefixed> {
    let n = rows.iter().map(Vec::len).max().unwrap_or(0);
    let room = width.saturating_sub(2 * n.saturating_sub(1));
    if n == 0 || room < 3 * n {
        return rows.iter().map(|r| (Line::from(inline(&r.join(" · "), th)), vec![])).collect();
    }
    let cell = |r: usize, c: usize| -> Vec<Span<'static>> {
        let spans = inline(rows[r].get(c).map_or("", String::as_str), th);
        if r == 0 { spans.into_iter().map(|s| { let st = s.style.fg(th.faint); s.style(st) }).collect() } else { spans }
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
    let mut out: Vec<Prefixed> = vec![];
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
        for k in 0..h {
            let mut sp = vec![];
            for (c, col) in cols.iter().enumerate() {
                let l = col.get(k).cloned().unwrap_or_default();
                let pad = " ".repeat(w[c].saturating_sub(l.spans.iter().map(|s| s.content.width()).sum()));
                let to_right = right.get(c).copied().unwrap_or(false);
                if to_right {
                    sp.push(Span::raw(pad.clone()));
                }
                sp.extend(l.spans);
                if c + 1 < n {
                    sp.push(Span::raw(format!("{}  ", if to_right { "" } else { pad.as_str() })));
                }
            }
            out.push((Line::from(sp), vec![]));
        }
        if r == 0 {
            out.push((Line::from(Span::styled("─".repeat(w.iter().sum::<usize>() + 2 * (n - 1)), st(th.border))), vec![]));
        }
    }
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
    // the words in order, each with its style. No-break spaces (`whole`) hold an item together, and a word that starts with
    // one holds on to the word before it: a row's "▸" went alone to the next line (design review 13)
    // (only one right after it: after a plain space a row may break there as anywhere)
    let words: Vec<(&str, Style)> = line.spans.iter().flat_map(|s| s.content.split_inclusive(' ').map(move |x| (x, s.style))).collect();
    for (k, &(word, style)) in words.iter().enumerate() {
        let ww = word.trim_end().width();
        let glued: usize = if word.ends_with(' ') { 0 } else {
            words[k + 1..].iter().take_while(|(x, _)| x.starts_with(NBSP)).map(|(x, _)| x.trim_end().width()).sum()
        };
        let held = word.starts_with(NBSP) && k > 0 && !words[k - 1].0.ends_with(' ');
        if w + ww + glued > width && w > start && !held {
            flush(&mut cur, &mut w, &mut start, &mut out);
        }
        let word = word.replace(NBSP, " ");
        if ww > width.saturating_sub(w) {
            // longer than a whole row: split by character
            let mut buf = String::new();
            for ch in word.chars() {
                let cw = ch.width().unwrap_or(0);
                if w + cw > width && w > start {
                    cur.push(Span::styled(std::mem::take(&mut buf), style));
                    flush(&mut cur, &mut w, &mut start, &mut out);
                }
                buf.push(ch);
                w += cw;
            }
            cur.push(Span::styled(buf, style));
        } else {
            w += word.width();
            cur.push(Span::styled(word, style));
        }
    }
    out.push(Line::from(cur).style(line.style));
    out
}

/// `facts` after `head`, as many on a row as fit in `room` columns (each whole, " · " between two on one row).
fn fact_rows(head: &str, facts: &[String], room: usize) -> Vec<String> {
    let mut rows = vec![head.to_string()];
    for f in facts.iter().map(|f| whole(f)) {
        let last = rows.last_mut().expect("one row at least");
        let sep = if *last == head { " " } else { " · " };
        if last.width() + sep.width() + f.width() <= room {
            last.push_str(sep);
            last.push_str(&f);
        } else {
            rows.push(f);
        }
    }
    rows
}

/// The no-break space: `wrap` breaks a row only at plain spaces.
const NBSP: char = '\u{a0}';
/// A block's fold mark, held to the word before it.
const FOLD_SHUT: &str = "\u{a0}\u{a0}▸";
const FOLD_OPEN: &str = "\u{a0}\u{a0}▾";

/// `s` as one item for `wrap`: its spaces do not break it ("2 failed", "424 tok/s end to end").
fn whole(s: &str) -> String {
    s.replace(' ', "\u{a0}")
}

const REPLAY_KEYS: &str = "tab design · space pause · ←/→ speed · ↑/↓ scroll · e expand · q quit";

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
        // the mode, always (auto too; full in the warning colour, so it is never on unnoticed), and what is going on
        let mut tail = vec![];
        if v.live {
            tail.push(Span::raw("  "));
            tail.extend(mode_pill(&v.mode, th, false));
        }
        if !v.vim.is_empty() {
            tail.push(Span::styled(format!("  [{}]", v.vim), st(th.accent)));
        }
        if v.bg_running > 0 {
            tail.push(Span::styled(format!("  [{} background · ctrl-b]", v.bg_running), st(th.accent)));
        }
        if v.cold > 0 {
            tail.push(Span::styled(format!("  [cache cold: the next message re-reads {} tokens]", human(v.cold)), st(th.warn)));
        }
        // the placeholder gives way first, whole: to the cold cache's count and to "shift-tab mode" (design review 14)
        let used = 3 + usize::from(v.live) + Line::from(tail.clone()).width() + if v.live { "   shift-tab mode · /keys all".width() } else { 0 };
        if v.live {
            spans.push(Span::styled("▍", st(th.accent)));
        }
        spans.push(Span::styled(placeholder_fit(placeholder, (width as usize).saturating_sub(used)).to_string(), st(th.faint)));
        spans.extend(tail);
        if let Some((t, _)) = &v.toast {
            spans.push(Span::styled(format!("   {t}"), st(th.ok)));
        } else {
            let used: usize = spans.iter().map(|s| s.content.width()).sum::<usize>() + 3;
            // two hints and the way to the rest: seven of them read as a manual in the message box (design review 4); no
            // "@ files", which the placeholder says
            let keys = if v.live { fitted_keys("shift-tab mode · ctrl-v image · /keys all", (width as usize).saturating_sub(used)) } else { REPLAY_KEYS.to_string() };
            spans.push(Span::styled(format!("   {keys}"), st(th.faint)));
        }
    }
    Line::from(spans)
}

/// The placeholder in `room` columns, never cut: whole, its first part ("/ for commands"), or nothing.
pub fn placeholder_fit(placeholder: &str, room: usize) -> &str {
    [placeholder, placeholder.split(", ").next().unwrap_or_default()].into_iter().find(|p| p.width() <= room).unwrap_or_default()
}

/// Rows the composer box may grow to while typing (multi-line input); the latest rows stay visible.
const COMPOSER_MAX_ROWS: usize = 6;

/// The composer box: typed text on real lines (wrapped to `width`, the box's inside), or the one-line placeholder.
pub fn composer_rows(v: &View, th: &Theme, glyph: &str, placeholder: &str, width: u16) -> Vec<Line<'static>> {
    composer_rows_in(v, th, glyph, placeholder, width, COMPOSER_MAX_ROWS)
}

/// The same with at most `max` rows of the draft (zen in a short window: six rows and the completions under them pushed
/// its header and the completions off the screen, design review 11).
// ponytail: a click in a draft taller than a box capped under six rows lands by the six-row scroll (composer_index);
// give composer_index the cap when someone edits long drafts in a 12-row window.
pub fn composer_rows_in(v: &View, th: &Theme, glyph: &str, placeholder: &str, width: u16, max: usize) -> Vec<Line<'static>> {
    let typed = v.live && !v.input.is_empty();
    // an open question over the draft: what is typed meanwhile stays in sight under it (design review 13)
    let ask = v.asking.as_ref().map(|q| asking_rows(q, th, width, v.confirm && !typed, typed)).unwrap_or_default();
    if !ask.is_empty() && !typed {
        // a note for a moment ("answer first: …") in place of the answers' keys: beside them it narrowed the question to a
        // column of a few words (design review 14)
        let mut ask = ask;
        if let (Some((t, _)), Some(last)) = (&v.toast, ask.last_mut()) {
            *last = Line::from(Span::styled(format!("   {t}"), st(th.ok)));
        }
        return ask;
    }
    if !typed {
        return vec![composer_line(v, th, glyph, placeholder, width)];
    }
    let room = (width as usize).saturating_sub(4).max(1);
    let rows = composer_layout(v, room);
    let skip = composer_skip(&rows, cursor_at(v), max);
    COMPOSER_TOP.set((skip, max));
    let hints = hint_row(v, th, width);
    let sel = v.select;
    let draft: Vec<Line<'static>> = rows.into_iter()
        .enumerate()
        .skip(skip)
        .take(max)
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
        .collect();
    ask.into_iter().chain(draft).collect()
}

/// A permission question as the box draws it: the question on its own row (in the warn colour), the reason under it, the
/// call itself whole up to ASK_ROWS rows, then the answers. One row with the call cut to fit ("sysctl vm.swapusa") was easy
/// to miss and hid what was allowed (the user, 2026-11-23: "quite hidden, sometimes i dont realize until way later").
fn asking_rows(q: &str, th: &Theme, width: u16, confirm: bool, typed: bool) -> Vec<Line<'static>> {
    let (tool, note, call) = asking_parts(q, width);
    // the answers are what to act on: the brightest text, their keys in the accent (design review 2026-11-19); no
    // "always" where it is not given (main.rs question_parts)
    let key = |k: &'static str| Span::styled(k, st(th.accent).add_modifier(Modifier::BOLD));
    let word = |w: &'static str| Span::styled(w, st(th.text).add_modifier(Modifier::BOLD));
    // "Allow this command?" over its reason, wrapped whole: "allow bash?" with "once only · auto asks: git push --force: …"
    // beside it read as tazuna's internals (design review 14). The own loop names its tool with the reason ("edit (outside
    // the project)", engine.rs), which the row under it says already.
    let name = tool.split_once(" (").filter(|(_, r)| r.ends_with(')')).map_or(tool.as_str(), |(t, _)| t);
    let ask = match name {
        "Bash" | "bash" | "PowerShell" => "Allow this command?".to_string(),
        "Edit" | "Write" | "MultiEdit" | "NotebookEdit" | "edit" | "write" => "Allow this edit?".to_string(),
        t => format!("Allow {t}?"),
    };
    let mut out = vec![Line::from(vec![Span::styled(" ? ", st(th.warn).add_modifier(Modifier::BOLD)), Span::styled(ask, st(th.warn).add_modifier(Modifier::BOLD))])];
    out.extend(note.into_iter().map(|l| Line::from(vec![Span::raw("   "), Span::styled(l, st(th.dim))])));
    out.extend(call.into_iter().map(|l| Line::from(vec![Span::raw("   "), Span::styled(l, st(th.text))])));
    // a destructive command auto asked about: y chooses "allow", enter confirms it (main.rs question_key)
    let mut keys = vec![Span::raw("   ")];
    if confirm {
        keys.extend([Span::styled("allow", picked(th)), Span::styled("  enter", st(th.accent).add_modifier(Modifier::BOLD)), word(" confirms   ")]);
    } else {
        keys.extend([key("y"), word(" allow   ")]);
    }
    if !q.contains("  (once only") {
        keys.extend([key("a"), word(" always   ")]);
    }
    keys.extend([key("n"), word(" decline")]);
    // with a draft in the box every key types (main.rs): the answers wait for it to be empty, said beside them, or on a row
    // of its own where it does not fit (it was cut at 80 columns beside "a always")
    let empty_box = "   · answer with an empty box";
    if typed && Line::from(keys.clone()).width() + empty_box.width() <= width as usize {
        keys.push(Span::styled(empty_box, st(th.faint)));
        out.push(Line::from(keys));
    } else {
        out.push(Line::from(keys));
        if typed {
            out.push(Line::from(Span::styled("   answer with an empty box", st(th.faint))));
        }
    }
    out
}

/// The rows of a call shown in a question at most.
const ASK_ROWS: usize = 6;

/// A question's tool, its reason and its call wrapped to the box ("Bash: ls -la  (always = this command)", main.rs question):
/// the reason by whole words on the rows it takes; a call past ASK_ROWS rows ends in "…" (its step in the transcript shows it
/// whole).
fn asking_parts(q: &str, width: u16) -> (String, Vec<String>, Vec<String>) {
    let (head, note) = q.rsplit_once("  (").map_or((q, ""), |(h, n)| (h, n.strip_suffix(')').unwrap_or(n)));
    let (tool, call) = head.split_once(": ").unwrap_or(("this", head));
    let room = (width as usize).saturating_sub(4).max(10);
    let mut rows: Vec<String> = call.lines().flat_map(|l| wrap_words(l, room)).collect();
    if rows.len() > ASK_ROWS {
        rows.truncate(ASK_ROWS);
        if let Some(last) = rows.last_mut() {
            last.push_str(" …");
        }
    }
    let reason = reason(note);
    (tool.to_string(), if reason.is_empty() { vec![] } else { wrap_words(&reason, room) }, rows)
}

/// A question's note as the reason under it: auto's rule as a sentence ("once only · auto asks: git push --force: rewrites
/// the remote's history" is "git push --force rewrites the remote's history"), and "once only: " gone (the answers leave
/// "always" out, which says it). "always = this command" is no reason: none.
fn reason(note: &str) -> String {
    if note.starts_with("always = ") {
        return String::new();
    }
    if let Some(why) = note.strip_prefix("once only · auto asks: ") {
        // risky.rs words its rules "CMD: WHAT": "dd: a disk or system tool", "rm outside the project: /x" stays as it is
        return match why.split_once(": ") {
            Some((cmd, rest)) if rest.starts_with("a ") => format!("{cmd} is {rest}"),
            Some((cmd, rest)) if !cmd.ends_with("outside the project") => format!("{cmd} {rest}"),
            _ => why.to_string(),
        };
    }
    note.strip_prefix("once only: ").unwrap_or(note).to_string()
}

/// The typed text as the composer draws it: wrapped rows of (char, its byte index in the input; None for the cursor),
/// each with the index a click past its end lands on. Drawing and clicking share it, so a click lands where it looks.
/// While text is selected the cursor is not drawn: its cell would shift the text under a drag.
fn composer_layout(v: &View, room: usize) -> Vec<(Vec<(char, Option<usize>)>, usize)> {
    text_rows(&v.input, cursor_at(v), v.select.is_none(), room)
}

/// The wrapping itself: `cursor` draws the cursor's cell at `at` (it takes a column, so the rows after it shift by one).
fn text_rows(input: &str, at: usize, cursor: bool, room: usize) -> Vec<(Vec<(char, Option<usize>)>, usize)> {
    let mut cells: Vec<(char, Option<usize>)> = Vec::with_capacity(input.len() + 1);
    for (i, c) in input.char_indices() {
        if cursor && i == at {
            cells.push((CURSOR, None));
        }
        cells.push((c, Some(i)));
    }
    if cursor && at == input.len() {
        cells.push((CURSOR, None));
    }
    let (mut rows, mut row, mut w): (Vec<_>, Vec<(char, Option<usize>)>, usize) = (vec![], vec![], 0);
    for (c, i) in cells {
        if let (Some(i), '\n') = (i, c) {
            rows.push((std::mem::take(&mut row), i));
            w = 0;
            continue;
        }
        let cw = c.width().unwrap_or(0);
        // a space past the row's end hangs there, in the box's spare column
        if w + cw > room && !(c == ' ' && w <= room) {
            // a row breaks between words where the word fits on the next one ("…in the compos" / "er box", design review
            // 13): the word in progress goes down whole; one longer than a row breaks where it must
            let carry = match row.iter().rposition(|(x, _)| *x == ' ') {
                Some(k) if c != ' ' && k > 0 && k + 1 < row.len() => row.split_off(k + 1),
                _ => vec![],
            };
            let end = carry.first().map_or(i, |&(_, j)| j).unwrap_or(at);
            rows.push((std::mem::replace(&mut row, carry), end));
            w = row.iter().map(|(x, _)| x.width().unwrap_or(0)).sum();
        }
        row.push((c, i));
        w += cw;
    }
    rows.push((row, input.len()));
    rows
}

fn cursor_at(v: &View) -> usize {
    let at = v.cursor.min(v.input.len());
    if v.input.is_char_boundary(at) { at } else { v.input.len() }
}

thread_local! {
    /// The draft's first row shown and the rows the box held when it was last drawn: the box scrolls only as far as the
    /// cursor needs, as a text field does, and a click maps onto the rows on screen. Worked out afresh each frame, the
    /// cursor's row was always the bottom one, so the row under it could not be reached (the user, 2026-11-23: "there is
    /// another line after … but i cannot use arrow down to reach it").
    static COMPOSER_TOP: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, COMPOSER_MAX_ROWS)) };
}

/// The row the cursor is on: the one holding the cursor's cell, else the character at it, else the row it ends (the end of
/// the text, a row before a newline). A full row's end is also the next row's start: the next row is the cursor's.
fn cursor_row(rows: &[(Vec<(char, Option<usize>)>, usize)], at: usize) -> usize {
    rows.iter().position(|(r, _)| r.iter().any(|(_, i)| i.is_none_or(|i| i == at)))
        .or_else(|| rows.iter().position(|(_, end)| *end == at))
        .unwrap_or(rows.len().saturating_sub(1))
}

/// Rows scrolled off the top: the last frame's, moved only as far as keeps the cursor's row in view.
fn composer_skip(rows: &[(Vec<(char, Option<usize>)>, usize)], at: usize, max: usize) -> usize {
    let (max, row) = (max.max(1), cursor_row(rows, at));
    let top = COMPOSER_TOP.get().0.min(rows.len().saturating_sub(max));
    if row < top { row } else if row >= top + max { row + 1 - max } else { top }
}

/// The input's byte index under the mouse at (`col`, `row`) of the composer's text area, `width` wide: the same
/// wrapping and scrolling as drawn. A row below the text counts as the last one, so a drag past it keeps selecting.
/// None while there is nothing typed or a question owns the box.
pub fn composer_index(v: &View, width: u16, col: u16, row: i32) -> Option<usize> {
    if !v.live || v.input.is_empty() || v.asking.is_some() {
        return None;
    }
    let rows = composer_layout(v, (width as usize).saturating_sub(4).max(1));
    let at = cursor_at(v);
    let skip = composer_skip(&rows, at, COMPOSER_TOP.get().1);
    // a row above or below the box (a drag past its edge) is a row scrolled away: the cursor goes there, and the box
    // follows it on the next frame (the box did not scroll under a drag, the transcript did)
    let (cells, end) = &rows[(skip as i64 + i64::from(row)).clamp(0, rows.len() as i64 - 1) as usize];
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

/// Up or down a row of the draft as drawn (a long line wraps into several): the byte index under the cursor's column on
/// the row above or below. None on the first or last row, where up and down go through the history.
pub fn composer_step(v: &View, width: u16, down: bool) -> Option<usize> {
    if !v.live || v.input.is_empty() {
        return None;
    }
    // the rows as drawn, the cursor's cell in them: up and down follow the rows on screen
    let at = cursor_at(v);
    let rows = composer_layout(v, (width as usize).saturating_sub(4).max(1));
    let r = cursor_row(&rows, at);
    let (cells, end) = rows.get(if down { r + 1 } else { r.checked_sub(1)? })?;
    let col: usize = rows[r].0.iter().take_while(|(_, i)| i.is_some_and(|i| i < at)).map(|(c, _)| c.width().unwrap_or(0)).sum();
    let mut x = 0;
    for (c, i) in cells {
        let Some(i) = i else { continue };
        x += c.width().unwrap_or(0);
        if x > col {
            return Some(*i);
        }
    }
    Some(*end)
}

/// The permission mode as a word in its colour, auto included: ok for auto, accent for the modes that ask, and full in the
/// warn colour on a warn-tinted ground, saying what it means. The `[ask]` and `[full]` tags left auto unsaid (design review
/// 13); its dot went (design review 14: "● auto   ● ready" read as two lamps). `short`, where room is tight: full says "full".
pub fn mode_pill(mode: &str, th: &Theme, short: bool) -> Vec<Span<'static>> {
    let (word, style) = match (mode, short) {
        ("full", false) => (" full · never asks ".to_string(), st(th.warn).bg(lerp(th.bg, th.warn, 0.18))),
        ("full", true) => (" full ".to_string(), st(th.warn).bg(lerp(th.bg, th.warn, 0.18))),
        ("auto", _) => (" auto ".to_string(), st(th.ok)),
        _ => (format!(" {mode} "), st(th.accent)),
    };
    vec![Span::styled(word, style)]
}

/// One row under the input: a note for a moment ("copied 12 chars"), else the completions ("   tab › /compact  /clear").
fn hint_row(v: &View, th: &Theme, width: u16) -> Option<Line<'static>> {
    if let Some((t, _)) = &v.toast {
        return Some(Line::from(Span::styled(format!("   {t}"), st(th.ok))));
    }
    // what `!` does, while it is typed: it is not a shell alone, the output goes to the model and a turn starts
    if v.input.starts_with('!') {
        return Some(Line::from(Span::styled(format!("   {SHELL_HINT}"), st(th.dim))));
    }
    if v.hints.is_empty() {
        // "@xyz" with no such file: said, where the completions just went (design review 11)
        return at_nothing(v).map(|word| Line::from(vec![Span::styled("   no file matches ", st(th.faint)), Span::styled(ellipsis(word, (width as usize).saturating_sub(21)), st(th.dim))]));
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

/// The `@word` being typed at the cursor when no file completes it (and the box does not hold a `/command`).
fn at_nothing(v: &View) -> Option<&str> {
    if !v.live || !v.hints.is_empty() || v.input.starts_with('/') {
        return None;
    }
    let word = v.input[..cursor_at(v)].rsplit(char::is_whitespace).next()?;
    (word.len() > 1 && word.starts_with('@')).then_some(word)
}

const CURSOR: char = '▍';

/// The empty message box's line, in both designs and the window's (design review 14: "say something…", "Message tazuna…").
const PLACEHOLDER: &str = "/ for commands, @ for files";

/// Under a message that starts with `!`.
const SHELL_HINT: &str = "! runs it here, then sends the output to the model: a turn starts";

/// The input with the cursor drawn in at its position.
fn with_cursor(v: &View) -> String {
    let at = v.cursor.min(v.input.len());
    let at = if v.input.is_char_boundary(at) { at } else { v.input.len() };
    format!("{}{CURSOR}{}", &v.input[..at], &v.input[at..])
}

/// Height of a bordered composer box for this input at this outer width.
pub fn composer_height(v: &View, outer_width: u16) -> u16 {
    // a question's rows: what asks, its reason under it, the call, the answers (and the empty-box note under them), as
    // asking_rows draws them (its colours aside)
    let typed = v.live && !v.input.is_empty();
    let ask = v.asking.as_ref().map_or(0, |q| asking_rows(q, &DARK, outer_width.saturating_sub(2), false, typed).len() as u16);
    if !typed {
        return if ask > 0 { ask + 2 } else { 3 };
    }
    let room = (outer_width as usize).saturating_sub(2 + 4).max(1);
    let rows = composer_layout(v, room).len();
    ask + rows.clamp(1, COMPOSER_MAX_ROWS) as u16 + 2 + u16::from(!v.hints.is_empty() || v.toast.is_some() || v.input.starts_with('!') || at_nothing(v).is_some())
}

/// A row's two sides made to fit `width` by leaving whole parts out: the left's last first, then the right's, each down
/// to its first part; a separator left at an end goes too. At 40 columns the right side ran over the left ("手 taz● ready")
/// and a status row ended mid-word ("CAC", design review 11).
fn fit_row(mut left: Line<'static>, mut right: Line<'static>, width: usize) -> (Line<'static>, Line<'static>) {
    let drop_last = |l: &mut Line<'static>| {
        l.spans.pop();
        while l.spans.len() > 1 && l.spans.last().is_some_and(|s| matches!(s.content.trim(), "" | "│")) {
            l.spans.pop();
        }
    };
    let over = |l: &Line<'static>, r: &Line<'static>| l.width() + r.width() + usize::from(l.width() > 0 && r.width() > 0) > width;
    while left.spans.len() > 1 && over(&left, &right) {
        drop_last(&mut left);
    }
    while right.spans.len() > 1 && over(&left, &right) {
        drop_last(&mut right);
    }
    if over(&left, &right) {
        // one part a side and still too wide (a long tool's name in the status): it ends in "…" beside the mark, where it
        // ran over it
        let room = width.saturating_sub(left.width() + usize::from(left.width() > 0));
        if let Some(sp) = right.spans.first_mut() {
            *sp = Span::styled(ellipsis(&sp.content, room), sp.style);
        }
    }
    (left, right)
}

/// Left and right content on one row.
pub fn split_row(f: &mut Frame, area: Rect, left: Line<'static>, right: Line<'static>, bg: Option<Color>) {
    let (left, right) = fit_row(left, right, area.width as usize);
    // paint the row background first: a styled Paragraph would repaint over the spans' own colors
    if let Some(c) = bg {
        f.render_widget(ratatui::widgets::Block::new().style(Style::new().bg(c)), area);
    }
    f.render_widget(Paragraph::new(left), area);
    f.render_widget(Paragraph::new(right).right_aligned(), area);
}

#[cfg(test)]
mod tests {
    /// A live cockpit view with nothing open.
    fn live_view() -> View {
        View { design: Design::Cockpit, expand_all: false, scroll: 0, clock_ms: 0.0, speed: 1.0, paused: false, live: true, input: String::new(), cursor: 0, select: None, asking: None, mode: "auto".into(), hints: Vec::new(), menu: Vec::new(), menu_sel: 0, bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None, mouse: None, list: None, hover_call: None, login: None, cold: 0, confirm: false, ask_risky: false, cant_run: None }
    }

    /// Under 110 columns the cockpit's header carries the cost, after the status: "ready   ≈$0", not "≈$0ready".
    #[test]
    fn the_narrow_header_puts_the_cost_after_the_status() {
        let s = Session::new("", "medium");
        let v = View { design: Design::Cockpit, expand_all: false, scroll: 0, clock_ms: 0.0, speed: 1.0, paused: false, live: true, input: String::new(), cursor: 0, select: None, asking: None, mode: "auto".into(), hints: Vec::new(), menu: Vec::new(), menu_sel: 0, bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None, mouse: None, list: None, hover_call: None, login: None, cold: 0, confirm: false, ask_risky: false, cant_run: None };
        let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap();
        term.draw(|f| { draw(f, &s, &v); }).unwrap();
        let top = (0..80).map(|x| term.backend().buffer()[(x, 0)].symbol().to_string()).collect::<String>();
        let (status, cost) = (top.find(&activity(&s, &v, Design::Cockpit.theme()).1).unwrap(), top.find("$").unwrap());
        assert!(status < cost, "{top}");
    }

    #[test]
    fn replay_strip_keeps_state_and_real_keys_without_a_live_composer() {
        let mut s = Session::new("Why does the retry test fail?", "medium");
        for design in [Design::Cockpit, Design::Zen] {
            let mut v = View { design, expand_all: false, scroll: 0, clock_ms: 18_000.0, speed: 0.25, paused: false, live: false, input: String::new(), cursor: 0, select: None, asking: None, mode: "auto".into(), hints: Vec::new(), menu: Vec::new(), menu_sel: 0, bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None, mouse: None, list: None, hover_call: None, login: None, cold: 0, confirm: false, ask_risky: false, cant_run: None };
            for (width, height) in [(40, 12), (80, 24), (120, 34), (160, 40)] {
                for paused in [false, true] { for expanded in [false, true] { for initialized in [false, true] {
                    (v.paused, v.expand_all) = (paused, expanded);
                    s.model = if initialized { "example-model".into() } else { String::new() };
                    let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
                    term.draw(|f| assert!(draw(f, &s, &v).composer.is_none())).unwrap();
                    let buf = term.backend().buffer();
                    let rows = (0..height).map(|y| (0..width).map(|x| buf[(x, y)].symbol()).collect::<String>()).collect::<Vec<_>>();
                    let screen = rows.join("\n");
                    let strip = rows.last().unwrap();
                    assert!(strip.contains(if paused { "Replay · Paused" } else { "Replay · Playing" }) && strip.contains("q quit"), "{width} {design:?}: {screen}");
                    assert!(!screen.contains("connecting") && !screen.contains(PLACEHOLDER), "{screen}");
                    if !initialized { assert!(!screen.contains("thinking"), "{screen}"); }
                    assert!(screen.contains("Offline recording"), "{screen}");
                    if width >= 80 {
                        for key in [if paused { "Space resume" } else { "Space pause" }, "Tab layout", if expanded { "e fold" } else { "e expand" }] {
                            assert!(strip.contains(key), "{width} {design:?}: {strip}");
                        }
                        assert!(rows.iter().any(|r| r.contains("00:18") && r.contains("0.25×") && r.contains("←/→ speed") && r.contains("↑/↓ scroll")), "{screen}");
                    }
                } } }
                // The same layouts still expose a real draft area in live mode, never the replay strip.
                (v.live, v.input, v.cursor) = (true, "a draft".into(), 7);
                let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
                term.draw(|f| assert!(draw(f, &s, &v).composer.is_some())).unwrap();
                let screen = term.backend().buffer().content().iter().map(|c| c.symbol()).collect::<String>();
                assert!((width < 80 || screen.contains("a draft")) && !screen.contains("q quit") && !screen.contains("Offline recording"), "{screen}");
                (v.live, v.input, v.cursor) = (false, String::new(), 0);
            }
            // Missing model metadata must not hide a recording's actual completion state.
            s.model.clear();
            s.activity = Activity::Done;
            assert_eq!(activity(&s, &v, &DARK).1, "done");
            s.activity = Activity::Starting;
        }
    }

    #[test]
    fn model_rows_read_as_names_and_even_prices() {
        assert_eq!(super::pretty_label("deepseek-v4.1-flash-1017 · fast"), "DeepSeek V4.1 Flash 1017, fast");
        assert_eq!(super::pretty_label("Opus 5.5"), "Opus 5.5");
        assert_eq!([super::even_price("0.0169 / 0.0677"), super::even_price("0 / 0"), super::even_price("1.4 / 4.4"), super::even_price("0.15 / 0.6"), super::even_price("plan")],
            ["0.017 / 0.068", "free", "1.4 / 4.4", "0.15 / 0.6", "plan"].map(String::from));
    }

    #[test]
    fn model_names_match_the_desktop_app() {
        // the cases of desktop/src/lib/types.ts MODEL_NAMES
        for (id, want) in [("claude-opus-5-5", "Opus 5.5"), ("claude-haiku-4-5-20251001", "Haiku 4.5"), ("haiku", "Haiku"), ("claude-opus-4-6-thinking", "Opus 4.6 thinking"),
            ("claude-opus-4.6", "Opus 4.6"),
            ("deepseek-v4.1-flash-1017-fast", "DeepSeek V4.1 Flash 1017 Fast"), ("deepseek-v4-1-flash-1017", "DeepSeek V4.1 Flash 1017"),
            ("gpt-6.1-sol", "GPT-6.1 Sol"), ("gpt-5", "GPT-5"), ("glm-5.3-flash-ultrafast", "GLM-5.3 Flash Ultrafast"), ("mistral-small-2603", "Mistral Small 2603"),
            ("codestral-2508", "Codestral 2508"), ("grok-4.7", "Grok 4.7"), ("agy/gemini-4-hydrogen-high", "Gemini 4 Hydrogen"), ("muse-ember", "Muse Ember"),
            ("baidu/ernie-5.5-flash-preview:free", "ERNIE 5.5 Flash Preview (free)"), ("ernie-5-5-flash-preview", "ERNIE 5.5 Flash Preview"),
            ("nemotron-3.5-super-vl", "Nemotron 3.5 Super VL"), ("qwen3.8-flash-next-unsloth-ud-iq3_xxs", "Qwen3.8 Flash Next Unsloth UD IQ3_XXS"),
            ("qwen3-8-omni-flash", "Qwen3.8 Omni Flash"), ("gemma-3-4b-it", "Gemma 3 4B IT"), ("codestral-2508 off 0.3,0.03,0.9", "Codestral 2508"), ("zai-glm-5.3", "Z.ai GLM-5.3")] {
            assert_eq!(super::pretty_model(id), want, "{id}");
        }
    }

    #[test]
    fn models_and_the_lane_read_as_words() {
        assert_eq!([super::model_short("claude-opus-5-5"), super::model_short("fable-5-2"), super::model_short("claude-haiku-4-5-20251001"), super::model_short("gpt-6.2-sol")],
            ["Opus 5.5", "Fable 5.2", "Haiku 4.5", "GPT-6.2 Sol"].map(String::from));
        assert_eq!(super::model_notice("model: claude-fable-5-2 at high for the rest of the session (the conversation goes on; its cache is rebuilt once)").as_deref(),
            Some("model: Fable 5.2 · high from here on (the conversation goes on; its cache is rebuilt once)"));
        assert_eq!(super::lane_notice("lane default: opus-5-5 medium · repo profile (4150 tracked files)").as_deref(), Some("started on Opus 5.5 · medium · repo profile (4150 tracked files)"));
    }

    #[test]
    fn readable_notice_takes_the_message() {
        let raw = r#"provider · provider error in stream: {"message":"[404]: Antigravity upstream error (404): Requested entity was not found.","type":"invalid_request_error","code"#;
        assert_eq!(super::readable_notice(raw).as_deref(), Some("provider error: Antigravity upstream error (404): Requested entity was not found."));
        assert_eq!(super::without_repeated_code("[429]: slow down"), "[429]: slow down", "a code said once stays");
        assert_eq!(super::readable_notice("route · HTTP 403: refused"), None);
    }

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
        let tool = |name: &str, input: serde_json::Value| Tool { id: String::new(), name: name.into(), input, result: None, error: false, started_ms: 0.0, done_ms: None, sub: None, at: None };
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
            term.draw(|f| { draw_tasks(f, &p, Design::Cockpit.theme(), f.area()); }).unwrap();
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

    /// Rows break between items, each whole, a fold mark never alone, and the composer between words (design review 13).
    #[test]
    fn rows_break_between_items_and_the_composer_between_words() {
        let text = |l: &Line| l.spans.iter().map(|s| s.content.to_string()).collect::<String>();
        let tools = Line::from(vec![Span::raw("⌁ "), Span::raw(whole("ran 4 tools")), Span::raw(format!("   {}  {}   {}", whole("bash ×2"), whole("edit ×1"), whole("11 lines"))),
            Span::raw(format!("   {}", whole("thought ~20 tok"))), Span::raw(format!("   {}", whole("2 failed")))]);
        for width in [76usize, 60, 40] {
            let rows: Vec<String> = wrap(tools.clone(), width, &[Span::raw("  ")]).iter().map(text).collect();
            assert!(rows.iter().all(|r| !r.contains(NBSP) && r.width() <= width), "{width}: {rows:#?}");
            assert!(rows.iter().any(|r| r.contains("2 failed")) && rows.iter().any(|r| r.contains("thought ~20 tok")), "{width}: {rows:#?}");
        }
        let done = Line::from(vec![Span::raw("✓ Done"), Span::raw(format!("  {}  {}", whole("2 min 03 s"), whole("cost not known"))), Span::raw(FOLD_SHUT)]);
        let rows: Vec<String> = wrap(done, 33, &[]).iter().map(text).collect();
        assert!(rows.iter().all(|r| r.trim() != "▸") && rows.last().unwrap().ends_with("cost not known  ▸"), "{rows:#?}");
        // the ctrl-o "so far:" facts: whole, and no "·" at a row's start or end (design review 14: one ended "·" at 80 columns)
        let facts = ["↑ 15.0k", "5 calls", "cache 32% (4.8k)", "↓ 800", "1 file", "<0.1 s to first token", "486 t/s end to end"].map(String::from);
        for room in [78, 54, 30] {
            let rows: Vec<String> = fact_rows("so far:", &facts, room).iter().map(|r| r.replace(NBSP, " ")).collect();
            assert!(rows.iter().any(|r| r.contains("<0.1 s to first token")) && rows.iter().any(|r| r.contains("486 t/s end to end")), "{rows:#?}");
            assert!(rows.iter().all(|r| r.width() <= room && !r.starts_with('·') && !r.trim_end().ends_with('·')), "{room}: {rows:#?}");
        }
        assert_eq!(super::tenths(0.04), "<0.1 s");
        assert_eq!(super::tenths(1.46), "1.5 s");
        // the composer: words whole where they fit; a click and up/down land where the text is drawn
        let typed = "this is a long message that will wrap onto a second line in the composer box and keep going";
        let v = View { input: typed.into(), cursor: typed.len(), ..live_view() };
        let rows = composer_layout(&v, 30);
        let lines: Vec<String> = rows.iter().map(|(cells, _)| cells.iter().filter(|(_, i)| i.is_some()).map(|(c, _)| *c).collect()).collect();
        assert!(lines.iter().all(|l| l.chars().count() <= 31), "{lines:#?}");
        let words: Vec<&str> = typed.split(' ').collect();
        assert!(lines.iter().all(|l| l.split_whitespace().all(|w| words.contains(&w))), "no word cut: {lines:#?}");
        assert_eq!(lines.concat(), typed, "every character once");
        for k in 0..rows.len() - 1 {
            assert_eq!(rows[k].1, rows[k + 1].0.iter().find_map(|(_, i)| *i).unwrap(), "a click past a row's end lands on the next row's start");
        }
        assert_eq!(composer_height(&v, 36) as usize, rows.len().min(COMPOSER_MAX_ROWS) + 2, "the box as tall as the rows drawn");
        // up and down from several places: a drawn row up or down, by the rows drawn with the cursor where it is
        for word in ["this", "wrap", "second", "composer", "going"] {
            for at in [typed.find(word).unwrap(), typed.find(word).unwrap() + 2] {
                let here = View { input: typed.into(), cursor: at, ..live_view() };
                let rows = composer_layout(&here, 30);
                let row_of = |i: usize| rows.iter().position(|(cells, _)| cells.iter().any(|(_, j)| *j == Some(i))).unwrap_or(rows.len() - 1);
                let r = cursor_row(&rows, at);
                match composer_step(&here, 34, false) {
                    Some(up) => assert_eq!(row_of(up) + 1, r, "up from {at}"),
                    None => assert_eq!(r, 0, "up from the first row goes to the history"),
                }
                match composer_step(&here, 34, true) {
                    Some(down) => assert_eq!(row_of(down), r + 1, "down from {at}"),
                    None => assert_eq!(r, rows.len() - 1),
                }
            }
        }
        // a no-break space in the text itself: after a plain space a row may still break there
        let rows: Vec<String> = wrap(Line::from("aaaa \u{a0}bbbb"), 6, &[]).iter().map(text).collect();
        assert!(rows.len() == 2 && rows.iter().all(|r| r.width() <= 6), "{rows:?}");
        let rows: Vec<String> = wrap(Line::from("aa x\u{a0}y"), 4, &[]).iter().map(text).collect();
        assert_eq!(rows, ["aa ", "x y"], "an item in the text keeps together");
    }

    /// The model's name first, whose it is shortened or dropped, and no two rows alike: at 100 columns "Mistral …tral Mistral
    /// Vibe" stood three times and "Nemotron 3…ree) OpenRouter" four (design review 13).
    #[test]
    fn model_names_win_over_providers_and_never_read_alike() {
        let rows = [("Codestral 2508, mistral", "Mistral Vibe"), ("Mistral Large 4, mistral", "Mistral Vibe"), ("Mistral Medium 3.5, mistral", "Mistral Vibe"),
            ("Mistral Large 4", "Mistral"), ("Nemotron 3 Super 120B A12B (free)", "OpenRouter"), ("Nemotron 3 Nano 30B A3B (free)", "OpenRouter"),
            ("Nemotron 3 Nano 9B V2 (free)", "OpenRouter"), ("Nemotron 3 Nano 12B V2 VL (free)", "OpenRouter"), ("DeepSeek V4.1 Flash 1017", ""),
            ("DeepSeek V4.1 Flash 1017", "Mistral"), ("DeepSeek V4.1 Flash 1017", "OpenRouter"), ("GLM-5.3", "Mistral Vibe")];
        // a provider and its region's twin, one model at both
        let twins = [("MiniMax M3", "MiniMax Plan"), ("MiniMax M3", "MiniMax Plan China"), ("Kimi K3.1", "Kimi Code"), ("Kimi K3.1", "Kimi Code China")];
        for lw in [24usize, 20, 16] {
            let shown: Vec<String> = model_cells(&twins, lw).iter().map(|(n, p)| format!("{n} {p}")).collect();
            for (i, a) in shown.iter().enumerate() {
                assert!(shown.iter().skip(i + 1).all(|b| a != b), "{lw}: twins read alike: {shown:#?}");
            }
        }
        for lw in [26usize, 20, 14] {
            let cells = model_cells(&rows, lw);
            let shown: Vec<String> = cells.iter().map(|(n, p)| if p.is_empty() { n.clone() } else { format!("{n} {p}") }).collect();
            assert!(shown.iter().all(|t| t.chars().count() <= lw), "{lw}: {shown:#?}");
            for (i, a) in shown.iter().enumerate() {
                assert!(shown.iter().skip(i + 1).all(|b| a != b), "{lw}: two rows read \"{a}\": {shown:#?}");
            }
            if lw == 26 {
                assert_eq!(&shown[1], "Mistral Large 4, mistral", "the name whole, the provider dropped first");
                assert_eq!(&shown[3], "Mistral Large 4 Mistral", "both where both fit");
                assert_eq!(&shown[11], "GLM-5.3 Mistral Vibe");
            }
        }
    }

    #[test]
    fn model_picker_shows_rows_effort_and_why_not() {
        let row = |label: &str, state: u8, stale: bool| ModelRow { label: label.into(), provider: String::new(), place: "SubRouter · kriora, shared-openai-pro-50, shared-openai-pro-52".into(),
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
            term.draw(|f| draw_models(f, p, Design::Cockpit.theme(), f.area())).unwrap();
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
            // the judge's score and AA's index as two numbers to the right, under their own heads; the runs are in the details
            let head = lines.iter().find(|l| l.contains("$/M in / out")).unwrap();
            assert!(head.contains("bench") && head.trim_end_matches(['│', ' ']).ends_with("AA"), "{w}: {head}");
            assert!(rows[1].contains("  6.8  ") && !rows[1].contains("(308)"), "{w}: {}", rows[1]);
            let text = lines.concat();
            assert!(text.contains("shared-openai-pro-52"), "{w}: the provider list wraps whole: {lines:#?}");
        }
        assert_eq!(wrap_words("ab cd efghij k", 4), ["ab", "cd", "efgh", "ij k"]);
        let wide = draw(&p, 200);
        assert!(wide.concat().contains("SubRouter") && wide.concat().contains("All models"), "the providers pane");
        // whose a model is, after its name and not in it
        p.rows[3].provider = "Mistral".into();
        assert!(draw(&p, 120).iter().any(|l| l.contains("gpt-6-luna Mistral ")), "the provider beside the name");
        p.rows[3].provider.clear();
        // one model at two providers, its name filling the column at 80: the name gives way, the provider stays, and the two
        // rows do not read the same
        let mut two = ModelsPanel { rows: vec![row("DeepSeek V4.1 Flash 1017", 0, false), row("DeepSeek V4.1 Flash 1017", 0, false), row("DeepSeek V4.1 Flash 1017", 0, false)],
            selected: 0, filter: String::new(), groups: vec![("All models".into(), 3, true, String::new()), ("SubRouter".into(), 1, true, String::new()), ("OpenRouter".into(), 1, true, String::new())],
            group: 0, in_models: true, effort: None };
        (two.rows[1].provider, two.rows[2].provider) = ("OpenRouter".into(), "DeepSeek".into());
        let lines = draw(&two, 80);
        let rows: Vec<String> = lines.iter().filter(|l| l.contains("0.0387 / 0.194")).map(|l| l[..l.find("0.0387").unwrap()].to_string()).collect();
        assert!(rows.len() == 3 && rows[1].contains(" OpenRouter") && rows[2].contains(" DeepSeek") && rows[1].contains('…'), "{lines:#?}");
        assert!(rows[0] != rows[1] && rows[1] != rows[2] && rows[0] != rows[2], "{rows:#?}");
        // some 70 providers with long names (0.1.196): at 80 columns the models' names keep the room they had (the pane took
        // 11 columns, review 11), and the pane scrolls with the selection (it went off the pane after ~15 rows)
        let many = ModelsPanel { rows: vec![row("Qwen3 Coder Plus 2025 09", 0, false)], selected: 0, filter: String::new(),
            groups: std::iter::once(("All models".to_string(), 3458, true, String::new()))
                .chain((1..70).map(|i| (format!("Alibaba Model Studio China {i}"), 120, i < 5, String::new()))).collect(),
            group: 60, in_models: true, effort: None };
        let lines = draw(&many, 80);
        assert!(lines.iter().any(|l| l.contains("Qwen3 Coder Plus 2025 09 ")), "the name whole at 80 columns: {lines:#?}");
        assert!(lines.iter().any(|l| l.contains("›○ Alibaba Mod… 120")) && !lines.iter().any(|l| l.contains("All models")), "the selected provider in view, its count kept: {lines:#?}");
        // with a search typed, esc empties it first, and the hint says that
        assert!(draw(&two, 80).concat().contains("esc closes"));
        two.filter = "deepseek v4.1".into();
        let text = draw(&two, 80).concat();
        assert!(text.contains("esc clears the search") && !text.contains("esc closes"), "{text}");
        // the detail line says what to do: the variable to set, the line to add (it named `source ~/.subrouter-key`, a
        // file only the bench's machines have, until 0.1.77)
        for (i, want) in [(0, "cached 81%"), (1, "SUBROUTER_API_KEY"), (2, "trusted-providers"), (3, "stale")] {
            p.selected = i;
            assert!(draw(&p, 78).concat().contains(want), "row {i}: {want}");
        }
        // effort is not on the model rows: ← → move between the panes, and a model chosen gets its effort after
        let text = draw(&p, 120).concat();
        assert!(!text.contains("‹") && text.contains("← providers") && text.contains("enter chooses") && text.contains("esc closes") && text.contains("4 models"), "{text}");
        p.effort = Some(("deepseek-v4.1-flash-1017".into(), vec!["high".into(), "max".into()], 1, "↑↓ effort · enter use · esc models", String::new()));
        let text = draw(&p, 120).join("\n");
        assert!(text.contains("deepseek-v4.1-flash-1017 · effort") && text.contains("› max") && text.contains("  high") && text.contains("esc models"), "{text}");
        // a Claude switch in a long conversation shows its price under the levels, and asks for a second enter
        p.effort = Some(("Opus 5.5".into(), vec!["medium".into(), "high".into()], 1, "enter again to switch · esc models",
            "Enter again to switch: the next message re-reads ~514k tokens without the cache (≈ $4.11 of plan usage).".into()));
        let text = draw(&p, 120).join(" ");
        assert!(text.contains("Enter again to switch: the next") && text.contains("enter again to switch   esc models"), "{text}");
    }

    #[test]
    fn the_plan_is_named_only_on_claude() {
        // a free own-loop model: $0, not "$0 on Claude plan"
        let mut s = crate::session::Session::new("", "");
        s.apply(&serde_json::json!({"type": "result", "subtype": "success", "total_cost_usd": 0.0}), 1.0);
        assert_eq!(super::cost_str(&s), "$0");
    }

    #[test]
    fn the_command_menu_lists_commands_over_the_box() {
        let mut v = View { design: Design::Cockpit, expand_all: false, scroll: 0, clock_ms: 0.0, speed: 1.0, paused: false, live: true, input: "/re".into(), cursor: 3, select: None, asking: None, mode: "auto".into(), hints: Vec::new(), menu: Vec::new(), menu_sel: 0, bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None, mouse: None, list: None, hover_call: None, login: None, cold: 0, confirm: false, ask_risky: false, cant_run: None };
        let cmds: Vec<String> = ["new", "resume", "review", "rewind", "security-review"].map(String::from).to_vec();
        v.menu = crate::editor::command_menu("re", &cmds, &[]);
        v.menu_sel = 1;
        let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(110, 30)).unwrap();
        term.draw(|f| draw_menu(f, &v, Design::Cockpit.theme(), Rect::new(0, 0, 110, 26))).unwrap();
        let b = term.backend().buffer().clone();
        let text: Vec<String> = (0..b.area.height).map(|y| (0..b.area.width).map(|x| b[(x, y)].symbol()).collect::<String>().trim_end().to_string()).collect();
        let all = text.join("\n");
        for want in ["┌ commands ─", " 2/4 ─┐", "›/review", "↑↓ move   enter runs   tab fills in   esc clears", "/resume", "Continue an earlier conversation of this folder", "/security-review", "A security review"] {
            assert!(all.contains(want), "{want}\n{all}");
        }
        assert!(!all.contains("/new"), "only matches: {all}");
        // Claude's after tazuna's, under their heading, without "Claude: " in each (design review 14)
        let row = |w: &str| text.iter().position(|l| l.contains(w)).unwrap();
        assert!(row("/rewind") < row("│ Claude") && row("│ Claude") + 1 == row("/security-review") && !all.contains("Claude: "), "{all}");
        // the menu sits on the transcript's bottom rows, just over the box (a row more for the heading)
        let first = text.iter().position(|l| l.contains("commands")).unwrap();
        assert_eq!(first, 26 - 5 - 2, "{all}");
        // nothing matches: the frame says so (the menu just went)
        (v.menu, v.input) = (vec![], "/zz".into());
        term.draw(|f| { draw(f, &Session::new("", "medium"), &v); }).unwrap();
        let all: String = term.backend().buffer().content().iter().map(|c| c.symbol()).collect();
        assert!(all.contains("no command matches /zz") && all.contains("└ esc clears ─"), "{all}");
    }

    #[test]
    fn login_lists_providers_then_a_page_that_asks_for_the_key() {
        let row = |n: &str, s: &str| (n.to_string(), s.to_string(), String::new());
        let mut p = LoginPanel { filter: String::new(), rows: vec![row("Claude", "subscription · the claude CLI's own login"), row("SubRouter", "logged in"),
            row("Mortera", "not logged in")], selected: 2, form: None };
        let draw = |p: &LoginPanel, w: u16| {
            let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(w, 30)).unwrap();
            term.draw(|f| draw_login(f, p, Design::Cockpit.theme(), f.area())).unwrap();
            let b = term.backend().buffer().clone();
            (0..b.area.height).map(|y| (0..b.area.width).map(|x| b[(x, y)].symbol()).collect::<String>().trim_end().to_string()).collect::<Vec<_>>()
        };
        let list = draw(&p, 100).join("\n");
        assert!(list.contains("› Mortera") && list.contains("not logged in") && list.contains("↑↓ move   enter logs in   esc closes"), "{list}");
        // 80 columns with 0.1.196's longest names: a state whole beside the longest name (it lost "trusted yet", review 11)
        let mut wide = LoginPanel { filter: String::new(), rows: vec![row("Alibaba Model Studio China", "key in this terminal only · not trusted yet"), row("DeepSeek", "logged in")],
            selected: 1, form: None };
        assert!(draw(&wide, 80).join("\n").contains("Alibaba Model Studio China  key in this terminal only · not trusted yet"), "{}", draw(&wide, 80).join("\n"));
        wide.rows[0].1 = "x".repeat(200);
        assert!(draw(&wide, 80).join("\n").contains("xxx…"), "longer still: cut with …");
        p.form = Some(LoginForm { name: "Mortera".into(), page: "https://mortera.cloud".into(), opened: true, place: "your Mortera dashboard".into(),
            example: String::new(), typed: 5, host: "mortera.cloud".into(), var: "MORTERA_API_KEY".into(), has: String::new(), note: String::new(),
            sign_in: String::new(), confirm: false, other: false });
        // 60 columns: nothing cut, the long lines wrap
        for w in [60u16, 100] {
            let page = draw(&p, w).join("\n");
            for want in ["log in to Mortera", "https://mortera.cloud", "opened in your browser", "Paste your Mortera API key", "> •••••▍",
                         "enter saves the key esc cancels", "mortera.cloud on your trusted"] {
                // (a sentence may break at the frame's edge)
                assert!(page.replace('│', " ").split_whitespace().collect::<Vec<_>>().join(" ").contains(want), "{w}: {want}\n{page}");
            }
        }
    }

    #[test]
    fn a_sign_in_page_says_its_warning_how_it_goes_and_where_the_tokens_stay() {
        let mut p = LoginPanel { filter: String::new(), rows: vec![], selected: 1, form: Some(LoginForm { name: "SuperGrok".into(), page: "https://grok.com".into(), opened: false,
            place: String::new(), example: String::new(), typed: 0, host: "api.x.ai".into(), var: String::new(), has: String::new(),
            note: "tazuna signs in as xAI's Grok CLI. Use at your own risk.".into(), sign_in: "Enter shows a code to type on SuperGrok's page.".into(), confirm: true, other: false }) };
        let draw = |p: &LoginPanel| {
            let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap();
            term.draw(|f| draw_login(f, p, Design::Cockpit.theme(), f.area())).unwrap();
            let b = term.backend().buffer().clone();
            (0..b.area.height).map(|y| (0..b.area.width).map(|x| b[(x, y)].symbol()).collect::<String>()).collect::<Vec<_>>().join("\n")
                .replace('│', " ").split_whitespace().collect::<Vec<_>>().join(" ")
        };
        let page = draw(&p);
        for want in ["sign in to SuperGrok", "Use at your own risk.", "Enter shows a code", "~/.config/tazuna/logins/", "enter: I understand, sign in"] {
            assert!(page.contains(want), "{want}\n{page}");
        }
        let f = p.form.as_mut().unwrap();
        (f.confirm, f.has) = (false, "signed in".into());
        let page = draw(&p);
        assert!(page.contains("s signs out") && page.contains("Signed in now"), "{page}");
    }

    #[test]
    fn plan_and_balance_show_whole_in_every_design() {
        let mut s = Session::new("", "medium");
        s.limits = Some((12.0, 14.0));
        s.balance = Some(8.816);
        for d in [Design::Cockpit, Design::Zen] {
            let v = View { design: d, expand_all: false, scroll: 0, clock_ms: 0.0, speed: 1.0, paused: false, live: true, input: String::new(), cursor: 0, select: None, asking: None, mode: "auto".into(), hints: Vec::new(), menu: Vec::new(), menu_sel: 0, bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None, mouse: None, list: None, hover_call: None, login: None, cold: 0, confirm: false, ask_risky: false, cant_run: None };
            let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(160, 40)).unwrap();
            term.draw(|f| { draw(f, &s, &v); }).unwrap();
            let text: String = term.backend().buffer().content().iter().map(|c| c.symbol()).collect();
            // the cockpit's sidebar cut "SubRouter $8.82 left" to "$8" when it shared the plan's line (2026-11-02); since
            // 0.1.207 its cost box names the account in use (Claude here) and SubRouter only when it is in use or low
            let want = if d == Design::Cockpit { text.contains("5h 12%") } else { text.contains("SubRouter $8.82 left") && text.contains("5h 12%") };
            assert!(want, "design {}", d as u8);
        }
        // on SubRouter, the cockpit says its balance whole
        (s.provider, s.price_m, s.credits) = (Some("SubRouter".into()), Some((0.1, 0.01, 0.4)), vec![("SubRouter".into(), Some("$8.82 left".into()), false)]);
        let v = View { design: Design::Cockpit, expand_all: false, scroll: 0, clock_ms: 0.0, speed: 1.0, paused: false, live: true, input: String::new(), cursor: 0, select: None, asking: None, mode: "auto".into(), hints: Vec::new(), menu: Vec::new(), menu_sel: 0, bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None, mouse: None, list: None, hover_call: None, login: None, cold: 0, confirm: false, ask_risky: false, cant_run: None };
        let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(160, 40)).unwrap();
        term.draw(|f| { draw(f, &s, &v); }).unwrap();
        let text: String = term.backend().buffer().content().iter().map(|c| c.symbol()).collect();
        assert!(text.contains("SubRouter $8.82 left"), "{text}");
    }

    #[test]
    fn long_calls_close_to_a_line_and_open_on_a_click() {
        let mut s = Session::new("", "medium");
        let prompt = "Repo: /x. Read scripts/load-secrets.sh from the PR branch.\n\nAnswer with line numbers:\n".to_string() + &"1. How is fetch_secret invoked? ".repeat(40);
        s.blocks.push(Block::Tool(Tool { id: "a".into(), name: "Agent".into(),
            input: serde_json::json!({"subagent_type": "explorer", "description": "Inspect load-secrets.sh retry impact", "prompt": prompt}),
            result: Some("It runs sequentially.".into()), error: false, started_ms: 0.0, done_ms: Some(6700.0), sub: None, at: None }));
        s.blocks.push(Block::Tool(Tool { id: "b".into(), name: "Bash".into(), input: serde_json::json!({"command": format!("gh run view 1 --jq '{}'", "x".repeat(400))}),
            result: Some("ok".into()), error: false, started_ms: 0.0, done_ms: Some(4300.0), sub: None, at: None }));
        let mut v = View { design: Design::Cockpit, expand_all: false, scroll: 0, clock_ms: 0.0, speed: 1.0, paused: false, live: true, input: String::new(), cursor: 0, select: None, asking: None, mode: "auto".into(), hints: Vec::new(), menu: Vec::new(), menu_sel: 0, bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None, mouse: None, list: None, hover_call: None, login: None, cold: 0, confirm: false, ask_risky: false, cant_run: None };
        let text = |v: &View| { let (l, _, owners) = transcript(&s, v, Design::Cockpit.theme(), &cockpit::OPTS, 100); (l.iter().map(|l| l.spans.iter().map(|x| x.content.to_string()).collect::<String>()).collect::<Vec<_>>(), owners) };
        let (closed, owners) = text(&v);
        // closed: the subagent as who and what for (its 25-row prompt hidden), the long command cut, both marked ▸
        assert!(closed[0].contains("explorer: Inspect load-secrets.sh retry impact") && closed[0].contains("▸") && !closed.concat().contains("fetch_secret"), "{closed:#?}");
        assert!(closed.len() <= 6, "{} rows: {closed:#?}", closed.len());
        assert_eq!((owners[0], owners.last().copied().flatten()), (Some(0), Some(1)));
        assert!(closed.last().unwrap().starts_with("         · 1 line · 4.3 s"), "a long call's counts on a row of their own: {closed:#?}");
        v.open.insert(0);
        let (opened, _) = text(&v);
        assert!(opened.concat().contains("How is fetch_secret invoked") && opened.concat().contains("It runs sequentially") && opened[0].contains("▾"), "{opened:#?}");
        v.hover = Some(1);
        let (l, _, _) = transcript(&s, &v, Design::Cockpit.theme(), &cockpit::OPTS, 100);
        assert_eq!(l.last().unwrap().style.bg, Some(Design::Cockpit.theme().surface), "the block under the mouse is shaded");
    }

    #[test]
    fn tables_are_rules_and_fit_the_width() {
        let th = Design::Cockpit.theme();
        let md = "Free space:\n\n| Path | Size | What |\n|---|---:|---|\n| `~/bench-gocache` | 243 GB | Go build cache for the benchmark; safe to delete after runs |\n| ~/.omlx/models | 61 GB | Local MLX models |\n| 日本語のパス | 5 GB | wide characters |\n\nDone.";
        let text = |w: usize| markdown(md, th, w).iter().map(|(l, _)| l.spans.iter().map(|s| s.content.to_string()).collect::<String>()).collect::<Vec<_>>();
        let wide = text(100);
        // a quiet header, one rule under it, no grid, no blank row between rows; the `---:` column to the right
        let at = wide.iter().position(|l| l.starts_with("Path")).unwrap();
        assert!(wide[at + 1].chars().all(|c| c == '─') && wide[at + 2].starts_with("~/bench-gocache  243 GB  Go build") && wide[at + 3].starts_with("~/.omlx/models    61 GB  Local"), "{wide:#?}");
        assert!(wide[at + 4].starts_with("日本語のパス       5 GB  wide"), "a wide character takes two columns: {wide:#?}");
        assert!(!wide.concat().contains("|---") && !wide.concat().contains('│') && !wide.concat().contains('┌'), "{wide:#?}");
        // narrow: cells wrap inside their columns, no row past the width, the columns still in line
        let narrow = text(44);
        let from = narrow.iter().position(|l| l.starts_with("Path")).unwrap();
        let rows = &narrow[from..narrow.len() - 2];
        assert!(rows.len() > 6 && rows.iter().all(|l| l.width() <= 44), "{narrow:#?}");
        let col = |l: &String, what: &str| l.find(what).map(|b| l[..b].width());
        assert_eq!(col(&rows[0], "What"), col(rows.iter().find(|l| l.contains("Local")).unwrap(), "Local"), "{narrow:#?}");
        assert!(text(10).iter().all(|l| !l.contains('─')), "too narrow: rows as plain lines");
    }

    #[test]
    fn a_code_block_is_one_even_shape_with_its_language() {
        let th = Design::Cockpit.theme();
        let md = "```rust\nfn main() {\n    println!(\"a long line that has to wrap because the column is narrow\");\n}\n```\n\n---\n\n> quoted";
        let rows = markdown(md, th, 40);
        let code: Vec<&Line> = rows.iter().map(|(l, _)| l).filter(|l| !l.spans.is_empty() && l.spans.iter().all(|s| s.style.bg == Some(th.field))).collect();
        assert!(code.len() >= 5 && code.iter().all(|l| l.width() == 40), "every row of the block is the column's width: {:?}", code.iter().map(|l| l.width()).collect::<Vec<_>>());
        assert_eq!(code[0].spans.iter().map(|s| s.content.to_string()).collect::<String>().trim(), "rust", "its language on the first row");
        let text: Vec<String> = rows.iter().map(|(l, _)| l.spans.iter().map(|s| s.content.to_string()).collect()).collect();
        assert!(text.contains(&"─".repeat(40)) && text.iter().any(|l| l == "▎ quoted"), "a rule across the column, a quote behind the bar: {text:#?}");
    }

    #[test]
    fn two_designs_and_the_numbers_scripts_know() {
        assert_eq!([1, 2, 3, 4, 5].map(Design::from_number), [Some(Design::Zen), Some(Design::Cockpit), Some(Design::Zen), Some(Design::Zen), None]);
        assert_eq!([1, 2, 3, 4].map(Design::gone), [Some("aurora"), None, None, Some("timeline")]);
        assert_eq!((Design::Zen.number(), Design::Cockpit.number(), Design::Zen.next(), Design::Cockpit.next()), (3, 2, Design::Cockpit, Design::Zen));
        assert_eq!(Design::gone_note("timeline", "tab"), "timeline is gone: this is zen · tab: cockpit");
        // a saved 1 or 4: reading it (every start: --run, --help, the app-server) leaves the file, so the note waits for
        // a screen; a session or replay opening settles it to zen's 3, once
        let dir = std::env::temp_dir().join(format!("tazuna-design-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("design"), "1\n").unwrap();
        assert_eq!((Design::saved_in(&dir), Design::saved_in(&dir)), ((Design::Zen, Some("aurora")), (Design::Zen, Some("aurora"))));
        Design::settle_in(&dir);
        assert_eq!((Design::saved_in(&dir), std::fs::read_to_string(dir.join("design")).unwrap()), ((Design::Zen, None), "3\n".to_string()));
        Design::Cockpit.save_in(&dir);
        Design::settle_in(&dir);
        assert_eq!(Design::saved_in(&dir), (Design::Cockpit, None), "a design that is not gone stays");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_welcome_names_a_branch_only_where_head_names_one() {
        let root = std::env::temp_dir().join(format!("tazuna-branch-{}", std::process::id()));
        let (repo, wt) = (root.join("repo"), root.join("wt"));
        std::fs::create_dir_all(repo.join(".git/worktrees/wt")).unwrap();
        std::fs::create_dir_all(repo.join("src")).unwrap();
        std::fs::create_dir_all(&wt).unwrap();
        let head = |h: &str| std::fs::write(repo.join(".git/HEAD"), h).unwrap();
        head("ref: refs/heads/main\n");
        assert_eq!(read_branch(&repo.join("src")).as_deref(), Some("main"), "from a folder inside");
        head("4b825dc642cb6eb9a060e54bf8d69288fbee4904\n");
        assert_eq!(read_branch(&repo), None, "a detached HEAD");
        head("ref: refs/heads/.invalid\n");
        assert_eq!(read_branch(&repo), None, "a reftable repo's stub HEAD");
        // a worktree: its .git is a file naming its directory in the main repo
        std::fs::write(wt.join(".git"), format!("gitdir: {}\n", repo.join(".git/worktrees/wt").display())).unwrap();
        std::fs::write(repo.join(".git/worktrees/wt/HEAD"), "ref: refs/heads/fix/x\n").unwrap();
        assert_eq!(read_branch(&wt).as_deref(), Some("fix/x"));
        std::fs::remove_dir_all(&root).unwrap();
        // `~` at a path boundary only
        let p = std::path::Path::new;
        assert_eq!([tilde(p("/Users/al/x"), Some(p("/Users/al"))), tilde(p("/Users/alice/x"), Some(p("/Users/al"))), tilde(p("/Users/al"), Some(p("/Users/al/"))), tilde(p("/x"), None)],
            ["~/x", "/Users/alice/x", "~", "/x"]);
        // its own line: what the router's start found (design review 14; it was Claude Code's "Try \"…\"")
        let facts = |profile: &str, reason: &str| {
            let mut s = Session::new("", "medium");
            apply(&mut s, serde_json::json!({"type": "harness_route", "lane": "small", "model": "claude-sonnet-5-5", "effort": "medium", "reason": reason, "profile": profile}), 0.0);
            start_facts(&s, 100)
        };
        assert_eq!(facts("lean", "tiny repo, 3 tracked files"), "lean profile · 3 tracked files · /keys for every key");
        assert_eq!(facts("repo", "4150 tracked files; repo config: CLAUDE.md"), "repo profile · 4150 tracked files · /keys for every key");
        assert_eq!(facts("chat", "not a git repo; no repo config; not in a project: chat (/profile repo for the coding agent)"), "chat · /keys for every key");
        // narrow: whole facts go, the file count first, then the profile; the keys' hint is cut last
        let mut s = Session::new("", "medium");
        apply(&mut s, serde_json::json!({"type": "harness_route", "lane": "small", "model": "claude-sonnet-5-5", "effort": "medium", "reason": "tiny repo, 3 tracked files", "profile": "lean"}), 0.0);
        assert_eq!([start_facts(&s, 52), start_facts(&s, 50), start_facts(&s, 21), start_facts(&s, 10)],
            ["lean profile · 3 tracked files · /keys for every key", "lean profile · /keys for every key", "/keys for every key", "/keys for…"]);
        let rows = rows_of(&frame(&s, &View { design: Design::Zen, ..live_view() }, 60, 20).0);
        assert!(rows.iter().any(|r| r.trim() == "lean profile · /keys for every key"), "the welcome at 60 columns: {rows:#?}");
    }

    #[test]
    fn one_frame_for_every_overlay_at_any_size() {
        let th = Design::Zen.theme();
        let draw = |w: u16, h: u16, title: &str, count: &str, keys: &str| {
            let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(w, h)).unwrap();
            let mut inner = Rect::default();
            term.draw(|f| inner = overlay(f, th, f.area(), title, count, keys)).unwrap();
            let b = term.backend().buffer().clone();
            ((0..b.area.height).map(|y| (0..b.area.width).map(|x| b[(x, y)].symbol()).collect::<String>()).collect::<Vec<_>>(), inner)
        };
        let (rows, inner) = draw(60, 5, "commands", "3/28", "↑↓ move · enter runs · tab fills in · esc closes");
        assert_eq!(rows[0], format!("┌ commands {} 3/28 ─┐", "─".repeat(41)));
        assert_eq!(rows[4], format!("└ ↑↓ move   enter runs   tab fills in   esc closes {}┘", "─".repeat(8)));
        assert_eq!(inner, Rect::new(1, 1, 58, 3));
        // narrow: the keys that fit whole and always the way out, the count gone before the title is cut
        let (rows, _) = draw(24, 3, "resume a conversation", "12/138", "↑↓ move · enter opens · esc closes");
        assert_eq!(rows[0], "┌ resume a conversati… ┐");
        assert_eq!(rows[2], "└ ↑↓ move   esc closes ┘");
        assert_eq!(draw(16, 3, "model", "", "↑↓ move · enter chooses · esc closes").0[2], "└ esc closes ──┘");
        // what follows " · " in a title is said after it, quietly; nothing breaks when there is no room at all
        assert!(draw(60, 3, "usage · your accounts", "", "").0[0].starts_with("┌ usage  your accounts ─"));
        for (w, h) in [(1, 1), (2, 2), (3, 1), (5, 3)] {
            draw(w, h, "model", "824 models", "↑↓ move · esc closes");
        }
        // an overlay's place: from the band's top as many rows as fit; a completion menu at its bottom
        let band = Rect::new(9, 2, 92, 20);
        assert_eq!((overlay_area(band, 4, false), overlay_area(band, 4, true), overlay_area(band, 400, false)), (Rect::new(9, 2, 92, 6), Rect::new(9, 16, 92, 6), band));
    }

    #[test]
    fn task_boxes_and_struck_text_are_drawn() {
        let th = Design::Zen.theme();
        let text = |md: &str| markdown(md, th, 80).iter().map(|(l, _)| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>()).collect::<Vec<_>>();
        assert_eq!(text("- [x] done item\n- [ ] open item\n- plain"), ["☑ done item", "☐ open item", "• plain"]);
        let line = &markdown("and ~~struck~~ text", th, 80)[0].0;
        assert!(line.spans.iter().any(|s| s.content == "struck" && s.style.add_modifier.contains(Modifier::CROSSED_OUT)));
        // marks that do not pair up are left as written
        assert_eq!(text("a ~~ b ~~ c ~~ d"), ["a ~~ b ~~ c ~~ d"]);
    }

    #[test]
    fn emphasis_is_italic_and_lone_asterisks_stay() {
        assert_eq!(super::emphasis("which is *expected*."), [("which is ".to_string(), false), ("expected".to_string(), true), (".".to_string(), false)]);
        for plain in ["a * b", "2*3*4", "see *.rs and *.toml", "*"] {
            assert_eq!(super::emphasis(plain), [(plain.to_string(), false)], "{plain}");
        }
        let l = links("nested bullet with a link: [the Rust book](https://doc.rust-lang.org/book/ch01-02-hello-world.html)");
        assert_eq!(l.iter().find(|(_, u)| u.is_some()).map(|(t, _)| t.as_str()), Some("the Rust book"), "{l:?}");
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
        let v = View { design: Design::Cockpit, expand_all: false, scroll: 0, clock_ms: 0.0, speed: 1.0, paused: false, live: true, input: String::new(), cursor: 0, select: None, asking: None, mode: "auto".into(), hints: Vec::new(), menu: Vec::new(), menu_sel: 0, bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None, mouse: None, list: None, hover_call: None, login: None, cold: 0, confirm: false, ask_risky: false, cant_run: None };
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
        let mut v = View { design: Design::Cockpit, expand_all: false, scroll: 0, clock_ms: 0.0, speed: 1.0, paused: false, live: true, input: String::new(), cursor: 0, select: None, asking: None, mode: "auto".into(), hints: Vec::new(), menu: Vec::new(), menu_sel: 0, bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None, mouse: None, list: None, hover_call: None, login: None, cold: 0, confirm: false, ask_risky: false, cant_run: None };
        v.sessions = Some(SessionsPanel { rows: vec!["8m ago · 2 messages · review PR 1370  (4cb8a388)".into(), "2h ago · 5 messages · fix the login  (aa11bb22)".into()], selected: 1 });
        let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 20)).unwrap();
        term.draw(|f| { draw(f, &s, &v); }).unwrap();
        let text: String = term.backend().buffer().content().iter().map(|c| c.symbol()).collect();
        assert!(text.contains("┌ resume a conversation ─") && text.contains(" 2/2 ─┐") && text.contains("  8m ago · 2 messages · review PR 1370") && text.contains("› 2h ago · 5 messages · fix the login")
            && text.contains("↑↓ move   enter opens   esc closes"), "{text}");
    }

    #[test]
    fn background_tasks_show_live_under_the_transcript() {
        let s = Session::new("", "medium");
        let mut v = View { design: Design::Cockpit, expand_all: false, scroll: 0, clock_ms: 400.0, speed: 1.0, paused: false, live: true, input: String::new(), cursor: 0, select: None, asking: None, mode: "auto".into(), hints: Vec::new(), menu: Vec::new(), menu_sel: 0, bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None, mouse: None, list: None, hover_call: None, login: None, cold: 0, confirm: false, ask_risky: false, cant_run: None };
        let row = |agent: &str, kind: &str, secs, tokens, tools, doing: &str| TaskRow { kind: kind.into(), description: "Tick workflows loss window check".into(), status: "running".into(), secs, output: String::new(), agent: agent.into(), tokens, tools, doing: doing.into() };
        assert!(transcript(&s, &v, Design::Cockpit.theme(), &cockpit::OPTS, 100).0.is_empty(), "nothing while none run");
        v.running = vec![row("explorer", "local_agent", 72, 21_129, 3, "Running Show Ledger tick workflow"), row("", "local_bash", 12, 0, 0, "")];
        let (lines, last, owners) = transcript(&s, &v, Design::Cockpit.theme(), &cockpit::OPTS, 100);
        let text: Vec<String> = lines.iter().map(|l| l.spans.iter().map(|x| x.content.to_string()).collect()).collect();
        assert!(text[1].contains("waiting on 2 background tasks"), "{text:#?}");
        assert_eq!(text[2], "  explorer · Tick workflows loss window check · 1m 12s · 21.1k tokens · 3 tool calls");
        assert_eq!(text[3], "    └ Running Show Ledger tick workflow");
        assert_eq!(text[4], "  bash · Tick workflows loss window check · 12s");
        // a click on a task's rows opens it: they belong to the task, not to a block
        assert_eq!(&owners[owners.len() - 4..], [None, Some(TASK_ROW), Some(TASK_ROW), Some(TASK_ROW + 1)]);
        assert_eq!((last.0, owners[..owners.len() - 4].iter().all(Option::is_none)), (0, true));
        assert_eq!(lines_n(1), "1 line");
    }

    #[test]
    fn scrolled_up_the_last_row_says_how_far_the_end_is() {
        let th = Design::Zen.theme();
        let draw = |n: usize, scroll: usize| {
            let lines: Vec<Line<'static>> = (0..n).map(|i| Line::from(format!("line {i}"))).collect();
            let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(30, 6)).unwrap();
            term.draw(|f| { render_transcript(f, f.area(), (lines, (1, 0), vec![None; n]), scroll, th); }).unwrap();
            let b = term.backend().buffer().clone();
            (0..6).map(|y| (0..30).map(|x| b[(x, y)].symbol()).collect::<String>().trim().to_string()).collect::<Vec<_>>()
        };
        // 20 lines, 3 up: five rows of text and the mark; the text ends 3 lines before the end
        assert_eq!(draw(20, 3), ["line 12", "line 13", "line 14", "line 15", "line 16", "↓ 3 lines"]);
        // further than there is: the mark says what is really below
        assert_eq!(draw(20, 99)[..2], ["line 0", "line 1"]);
        assert_eq!(draw(20, 99)[5], "↓ 15 lines");
        // at the end, or with everything on screen: no mark
        assert_eq!(draw(20, 0)[5], "line 19");
        assert!(!draw(4, 2).concat().contains('↓'));
    }

    #[test]
    fn files_panel_totals_and_the_diff_view() {
        let s = Session::new("", "medium");
        let mut v = View { design: Design::Cockpit, expand_all: false, scroll: 0, clock_ms: 0.0, speed: 1.0, paused: false, live: true, input: String::new(), cursor: 0, select: None, asking: None, mode: "auto".into(), hints: Vec::new(), menu: Vec::new(), menu_sel: 0, bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None, mouse: None, list: None, hover_call: None, login: None, cold: 0, confirm: false, ask_risky: false, cant_run: None };
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
        assert!(t.contains("┌ 12 files changed  +186 −66 ─") && t.contains(" 2/12 ─┐") && !t.contains("+new line") && t.contains("› ") , "the file list with the selected file marked, no diff text (2026-11-06)");
    }

    #[test]
    fn key_hints_fit_whole_and_point_to_the_rest() {
        assert_eq!(fitted_keys("a · bb · cc", 100), "a · bb · cc");
        assert_eq!(fitted_keys("a · bb · cc", 16), "a · /keys all", "never a hint cut in half");
        let v = View { design: Design::Cockpit, expand_all: false, scroll: 0, clock_ms: 0.0, speed: 1.0, paused: false, live: true, input: String::new(), cursor: 0, select: None, asking: None, mode: "auto".into(), hints: Vec::new(), menu: Vec::new(), menu_sel: 0, bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None, mouse: None, list: None, hover_call: None, login: None, cold: 0, confirm: false, ask_risky: false, cant_run: None };
        let line: String = composer_line(&v, Design::Cockpit.theme(), "›", PLACEHOLDER, 90).spans.iter().map(|s| s.content.to_string()).collect();
        assert!(line.width() <= 90 && line.ends_with("/keys all"), "{line}");
        let mut v = v;
        v.toast = Some(("copied 42 chars".into(), std::time::Instant::now()));
        let line: String = composer_line(&v, Design::Cockpit.theme(), "›", PLACEHOLDER, 90).spans.iter().map(|s| s.content.to_string()).collect();
        assert!(line.ends_with("copied 42 chars") && !line.contains("/keys"), "a note takes the hints' place for a moment: {line}");
        // design review 14: the placeholder gives way whole, to "shift-tab mode" and to the cold cache's count; "@ files" is
        // the placeholder's, not a key hint too
        v.toast = None;
        let line = |v: &View, w: u16| composer_line(v, Design::Cockpit.theme(), "›", PLACEHOLDER, w).spans.iter().map(|s| s.content.to_string()).collect::<String>();
        let (at80, at58) = (line(&v, 78), line(&v, 58));
        assert!(at80.contains(PLACEHOLDER) && at80.contains("shift-tab mode") && !at80.contains("@ files"), "{at80}");
        assert!(at58.contains("/ for commands ") && !at58.contains("@ for") && at58.contains("shift-tab mode") && at58.width() <= 58, "{at58}");
        v.cold = 12_400;
        let cold = line(&v, 78);
        assert!(cold.contains("tokens]") && !cold.contains("/ for") && cold.width() <= 78, "{cold}");
        // zen: whole or its first part, never cut (full mode at 60, two background tasks at 60)
        let mut s = Session::new("", "medium");
        s.activity = Activity::Ready;
        for v in [View { design: Design::Zen, mode: "full".into(), ..live_view() }, View { design: Design::Zen, bg_running: 2, ..live_view() }] {
            let rows = rows_of(&frame(&s, &v, 60, 20).0);
            let row = rows.iter().rev().find(|r| r.contains(" › ")).unwrap();
            assert!(row.contains(PLACEHOLDER) || (row.contains("› / for commands ") && !row.contains(", @")) || !row.contains("/ for"), "{row}");
        }
    }

    #[test]
    fn composer_grows_with_lines_and_keeps_the_latest() {
        let mut v = View { design: Design::Cockpit, expand_all: false, scroll: 0, clock_ms: 0.0, speed: 1.0, paused: false, live: true, input: String::new(), cursor: 0, select: None, asking: None, mode: "auto".into(), hints: Vec::new(), menu: Vec::new(), menu_sel: 0, bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None, mouse: None, list: None, hover_call: None, login: None, cold: 0, confirm: false, ask_risky: false, cant_run: None };
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
    fn up_and_down_walk_the_rows_a_long_line_wraps_into() {
        // the user's case: a long last line wrapping past the box's rows, the cursor at a row's end
        let th = Design::Cockpit.theme();
        let mut v = View { design: Design::Cockpit, expand_all: false, scroll: 0, clock_ms: 0.0, speed: 1.0, paused: false, live: true, input: String::new(), cursor: 0, select: None, asking: None, mode: "auto".into(), hints: Vec::new(), menu: Vec::new(), menu_sel: 0, bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None, mouse: None, list: None, hover_call: None, login: None, cold: 0, confirm: false, ask_risky: false, cant_run: None };
        let line = "abcdefghij".repeat(20); // 200 chars: 6 rows of 34 at width 38 (room 34)
        v.input = format!("one\ntwo\n{line}");
        v.cursor = 8 + 34; // the end of the long line's first row
        let _ = composer_rows(&v, th, "›", "", 38);
        let down = composer_step(&v, 38, true).expect("a row below");
        assert!(down > v.cursor && down <= 8 + 68, "down lands on the next wrapped row, not the history: {down}");
        v.cursor = down;
        let rows = composer_rows(&v, th, "›", "", 38);
        assert!(rows.iter().any(|r| r.spans.iter().any(|s| s.content.contains(CURSOR))), "the cursor's row is drawn");
        let up = composer_step(&v, 38, false).expect("a row above");
        assert!((8..=8 + 34).contains(&up), "up comes back to the long line's first row: {up}");
        v.cursor = v.input.len();
        assert_eq!(composer_step(&v, 38, true), None, "the last row: down goes to the history");
        v.cursor = 0;
        assert_eq!(composer_step(&v, 38, false), None, "the first row: up goes to the history");
        // a drag below the box reaches a row scrolled away, and the box then shows it
        let _ = composer_rows(&v, th, "›", "", 38);
        let below = composer_index(&v, 38, 5, COMPOSER_MAX_ROWS as i32 + 1).unwrap();
        v.cursor = below;
        let rows = composer_rows(&v, th, "›", "", 38);
        assert!(below > 0 && rows.iter().any(|r| r.spans.iter().any(|s| s.content.contains(CURSOR))), "scrolled to the row under the drag");
    }

    #[test]
    fn a_bold_address_links_without_its_marks() {
        let l = reply_links("PR: **https://github.com/acme/app/pull/1**. And [**the docs**](https://example.com/d).");
        assert_eq!(l.iter().map(|(_, u)| u.as_str()).collect::<Vec<_>>(), ["https://github.com/acme/app/pull/1", "https://example.com/d"]);
    }

    #[test]
    fn a_click_lands_where_the_text_is_drawn() {
        let mut v = View { design: Design::Cockpit, expand_all: false, scroll: 0, clock_ms: 0.0, speed: 1.0, paused: false, live: true, input: "hello world\nsecond".into(), cursor: 0, select: None, asking: None, mode: "auto".into(), hints: Vec::new(), menu: Vec::new(), menu_sel: 0, bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None, mouse: None, list: None, hover_call: None, login: None, cold: 0, confirm: false, ask_risky: false, cant_run: None };
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
    fn cockpit_centers_its_reading_lane_and_hit_targets_inside_the_original_frame() {
        let mut s = Session::new("", "medium");
        s.blocks.push(Block::Text { text: "Read [the guide](https://example.com/guide).\n\n```rust\nlet answer = 42;\n```".into(), live: false });
        let mut v = View { design: Design::Cockpit, expand_all: false, scroll: 0, clock_ms: 0.0, speed: 1.0, paused: false, live: true, input: String::new(), cursor: 0, select: None, asking: None, mode: "auto".into(), hints: Vec::new(), menu: Vec::new(), menu_sel: 0, bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None, mouse: None, list: None, hover_call: None, login: None, cold: 0, confirm: false, ask_risky: false, cant_run: None };
        for width in [80, 109, 110, 120, 130, 131, 132, 175] {
            for live in [true, false] {
                v.live = live;
                let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, 34)).unwrap();
                let mut areas = Areas::default();
                term.draw(|f| { areas = draw(f, &s, &v); }).unwrap();
                let lane = areas.transcript;
                let frame_width = width - if width < 110 { 0 } else { 38 };
                let expected_width = if width < 110 { frame_width - 4 } else { (frame_width - 4).min(88) };
                assert_eq!(lane.width, expected_width, "reading width at {width}, live={live}");
                assert_eq!(lane.x, (frame_width - lane.width) / 2, "center inside the transcript frame, not the window");
                let buf = term.backend().buffer();
                assert_eq!(buf[(0, lane.y - 1)].symbol(), "╭");
                assert_eq!(buf[(frame_width - 1, lane.y - 1)].symbol(), "╮", "do not shrink the frame");
                assert_eq!(areas.tokens.map(|r| r.x), (width >= 110).then_some(width - 38), "keep the rail in place");
                assert!(areas.last_block.is_some_and(|r| r.x == lane.x && r.width == lane.width));
                assert_eq!(areas.links.len(), 1);
                let (link, url) = &areas.links[0];
                assert_eq!(url, "https://example.com/guide");
                assert!(link.x >= lane.x && link.right() <= lane.right(), "link hit targets follow the reading lane");
            }
        }
    }

    #[test]
    fn cockpit_tools_show_the_average_and_tokens_an_axis() {
        let mut s = Session::new("", "medium");
        for i in 0..14 {
            s.blocks.push(Block::Tool(Tool { id: format!("a{i}"), name: "Agent".into(), input: serde_json::json!({"description": "look"}), result: Some("ok".into()), error: false, started_ms: 0.0, done_ms: Some(131_600.0), sub: None, at: None }));
        }
        s.turn_usage = (0..40).map(|i| crate::session::Usage { input: 10, cache_write: 100, cache_read: 50_000, output: 200 + 300 * (i % 7) }).collect();
        s.calls.push(crate::session::CallTime { ttft_ms: Some(1500.0), gen_ms: 10_000.0, out: 12_000, total_ms: 11_500.0 });
        let mut v = View { design: Design::Cockpit, expand_all: false, scroll: 0, clock_ms: 0.0, speed: 1.0, paused: false, live: true, input: String::new(), cursor: 0, select: None, asking: None, mode: "auto".into(), hints: Vec::new(), menu: Vec::new(), menu_sel: 0, bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None, mouse: None, list: None, hover_call: None, login: None, cold: 0, confirm: false, ask_risky: false, cant_run: None };
        let render = |s: &Session, v: &View, width: u16, height: u16| {
            let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
            let mut areas = Areas::default();
            term.draw(|f| { areas = draw(f, s, v); }).unwrap();
            let buf = term.backend().buffer().clone();
            let rows: Vec<String> = (0..buf.area.height).map(|y| (width - 38..width).map(|x| buf[(x, y)].symbol().to_string()).collect()).collect();
            (rows.join("\n"), areas)
        };
        let (side, areas) = render(&s, &v, 120, 44);
        assert!(side.contains("×14") && side.contains("30.7m") && side.contains("avg 2.2m"), "{side}");
        assert!(side.contains("2.0k") && side.contains("out per call · #"), "an axis label and the caption: {side}");
        assert_eq!(areas.tool_rows.len(), 1);
        let (rect, first) = areas.bars.unwrap();
        v.hover_call = Some(first + 2);
        let (side, _) = render(&s, &v, 120, 44);
        assert!(side.contains(&format!("#{}", first + 3)) && rect.width > 10, "{side}");
        v.hover_call = None;
        for (width, height) in [(120, 34), (175, 48)] {
            for live in [true, false] {
                v.live = live;
                let (side, areas) = render(&s, &v, width, height);
                let text = side.lines().map(|l| l.trim_matches(['│', ' '])).collect::<Vec<_>>().join(" ");
                assert!(text.contains("1200 t/s writing") && text.contains("1043 end to end") && text.contains("1.5 s to first token"), "all pace metrics must fit the fixed rail at {width}×{height}, live={live}: {side}");
                let chart = areas.bars.unwrap().0;
                assert!(chart.height >= 2, "keep both the upper value and zero axis labels at {width}×{height}, live={live}: {side}");
                assert!(side.contains("out per call · #"), "keep the chart caption: {side}");
            }
        }
        // The real live case had three tool types. A replay/single-tool fixture left enough rail space and hid the bug.
        for (i, block) in s.blocks.iter_mut().enumerate() {
            if let Block::Tool(t) = block { t.name = ["bash", "edit", "write"][i % 3].into(); }
        }
        v.live = true;
        let (side, areas) = render(&s, &v, 120, 34);
        assert!(areas.bars.unwrap().0.height >= 2, "keep the zero axis with three live tool types: {side}");
        let text = side.lines().map(|l| l.trim_matches(['│', ' '])).collect::<Vec<_>>().join(" ");
        assert!(text.contains("1200 t/s writing") && text.contains("1043 end to end") && text.contains("1.5 s to first token"), "complete pace labels in a crowded live rail: {side}");
        assert_eq!(areas.tool_rows.len(), 3);
        for label in ["cache read", "cache write", "estimate (list prices)", "no file changed yet", "bash", "edit", "write"] {
            assert!(side.contains(label), "preserve other panel content ({label}): {side}");
        }
        for input in ["!", "first line\nsecond line"] {
            v.input = input.into();
            v.cursor = input.len();
            let (side, areas) = render(&s, &v, 120, 34);
            let text = side.lines().map(|l| l.trim_matches(['│', ' '])).collect::<Vec<_>>().join(" ");
            assert!(areas.bars.unwrap().0.height >= 2 && side.contains("out per call · #") && text.contains("1.5 s to first token"), "keep both axes, caption and pace with an extra composer row: {side}");
        }
        v.input.clear();
        v.cursor = 0;
        s.cache_ttl_ms = Some(300_000.0);
        s.last_call_ms = Some(0.0);
        s.credits.push(("other".into(), Some("$0.50".into()), true));
        let (side, _) = render(&s, &v, 120, 34);
        assert!(side.contains("cache life") && side.contains("other $0.50"), "populated optional rows must remain: {side}");
    }

    #[test]
    fn a_question_a_retry_and_a_failure_each_read_as_a_state() {
        let mut s = Session::new("", "medium");
        let on = |s: &mut Session, e: serde_json::Value, at: f64| s.apply(&e, at);
        on(&mut s, serde_json::json!({"type": "assistant", "message": {"content": [{"type": "tool_use", "id": "c1", "name": "edit", "input": {"path": "main.rs", "old": "a", "new": "b"}}]}}), 1.0);
        let mut v = View { design: Design::Cockpit, expand_all: false, scroll: 0, clock_ms: 0.0, speed: 1.0, paused: false, live: true, input: String::new(), cursor: 0, select: None, asking: None, mode: "auto".into(), hints: Vec::new(), menu: Vec::new(), menu_sel: 0, bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None, mouse: None, list: None, hover_call: None, login: None, cold: 0, confirm: false, ask_risky: false, cant_run: None };
        let render = |s: &Session, v: &View| {
            let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 30)).unwrap();
            term.draw(|f| { draw(f, s, v); }).unwrap();
            let buf = term.backend().buffer().clone();
            (0..buf.area.height).map(|y| (0..120).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()).collect::<Vec<_>>().join("\n")
        };
        // a permission question: the status says so without a spinner, the call is neither ticked nor spinning
        v.asking = Some("edit: main.rs".into());
        let th = v.design.theme();
        assert_eq!(activity(&s, &v, th), ("", "needs you".to_string(), th.warn));
        let screen = render(&s, &v);
        assert!(screen.contains("? edit") && screen.contains("Allow this edit?") && screen.contains("y allow") && screen.contains("edit ×1   …") && !screen.contains("✓"), "{screen}");
        // a refused call: one block that counts down, its raw text only when opened
        v.asking = None;
        on(&mut s, serde_json::json!({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": "c1", "content": "ok"}]}}), 2.0);
        for (at, wait) in [(1000.0, 2), (3000.0, 8)] {
            on(&mut s, serde_json::json!({"type": "system", "subtype": "call_retry", "message": format!("HTTP 429: Rate limit reached (rate_limit_error) · trying again in {wait}s")}), at);
        }
        v.clock_ms = 5000.0;
        assert_eq!(activity(&s, &v, th), (spinner(5000.0), "retrying in 6 s".to_string(), th.err));
        let screen = render(&s, &v);
        assert!(screen.contains("! the provider is rate limiting this model  try 2 failed") && screen.contains("trying again in 6 s · esc stops"), "{screen}");
        assert!(!screen.contains("Rate limit reached") && screen.contains("edit ×1   ✓"), "{screen}");
        v.expand_all = true;
        assert!(render(&s, &v).contains("HTTP 429: Rate limit reached (rate_limit_error)"), "opened: the provider's own words");
        // the last try fails: the block and the status say failed
        on(&mut s, serde_json::json!({"type": "result", "subtype": "error_during_execution", "is_error": true, "result": "[provider error: HTTP 429: Rate limit reached (rate_limit_error)]"}), 9000.0);
        assert_eq!(activity(&s, &v, th), ("✗", "failed".to_string(), th.err));
        let screen = render(&s, &v);
        assert!(screen.contains("gave up after 3 tries"), "{screen}");
    }

    /// Design review 11: zen asked about an edit without its diff, showed one row of a draft and no completions, called a
    /// waiting call "ran" and a declined one "failed"; narrow rows cut mid-word.
    #[test]
    fn zen_shows_what_is_asked_the_whole_draft_and_a_declined_call_as_declined() {
        let mut s = Session::new("", "medium");
        let on = |s: &mut Session, e: serde_json::Value, at: f64| s.apply(&e, at);
        on(&mut s, serde_json::json!({"type": "assistant", "message": {"content": [{"type": "tool_use", "id": "c1", "name": "edit", "input": {"path": "main.rs", "old": "old line", "new": "new line"}}]}}), 1.0);
        let mut v = View { design: Design::Zen, expand_all: false, scroll: 0, clock_ms: 0.0, speed: 1.0, paused: false, live: true, input: String::new(), cursor: 0, select: None, asking: Some("edit: main.rs".into()), mode: "ask".into(), hints: Vec::new(), menu: Vec::new(), menu_sel: 0, bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None, mouse: None, list: None, hover_call: None, login: None, cold: 0, confirm: false, ask_risky: false, cant_run: None };
        let render = |s: &Session, v: &View| {
            let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 30)).unwrap();
            term.draw(|f| { draw(f, s, v); }).unwrap();
            let buf = term.backend().buffer().clone();
            (0..buf.area.height).map(|y| (0..100).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()).collect::<Vec<_>>().join("\n")
        };
        let screen = render(&s, &v);
        assert!(screen.contains("- old line") && screen.contains("+ new line") && screen.contains("waits for your answer") && !screen.contains("ran 1 tool"), "{screen}");
        // declined: the model's sentence is not on screen, and nothing counts as failed
        v.asking = None;
        on(&mut s, serde_json::json!({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": "c1", "is_error": true, "content": "The user declined this. Ask what they want instead, or try another way."}]}}), 2.0);
        let screen = render(&s, &v);
        assert!(screen.contains("⌁ declined edit") && !screen.contains("failed") && !screen.contains("ran 1 tool") && screen.contains(" ask "), "{screen}");
        v.expand_all = true;
        let screen = render(&s, &v);
        assert!(screen.contains("declined") && !screen.contains("Ask what they want"), "{screen}");
        // a draft of three lines: all three, and the completions under them
        (v.input, v.hints) = ("first line\nsecond line\nlook at @ma".into(), vec!["main.rs".into()]);
        v.cursor = v.input.len();
        let screen = render(&s, &v);
        assert!(["› first line", "second line", "look at @ma", "tab › main.rs"].iter().all(|t| screen.contains(t)), "{screen}");
        // a row too narrow for both sides loses whole parts, never half a word
        let line = |parts: &[&str]| Line::from(parts.iter().map(|p| Span::raw(p.to_string())).collect::<Vec<_>>());
        let (l, r) = fit_row(line(&[" 手 ", "tazuna "]), line(&["ready", "   ", "≈$0", "   ", "Mock Model", "  medium "]), 40);
        assert_eq!((l.to_string(), r.to_string()), (" 手 ".to_string(), "ready   ≈$0   Mock Model  medium ".to_string()));
        let (l, _) = fit_row(line(&[" READY ", " T0 ", " CTX 0 ", " CACHE 0% "]), line(&[" 00:01 "]), 28);
        assert_eq!(l.to_string(), " READY  T0  CTX 0 ");
        assert_eq!(whole_hints("↑↓ model · ← providers · tab provider pin · enter choose · esc close", 50), "↑↓ model · ← providers · tab provider pin");
        // a reply running: how long it has worked on the working row, the key that stops it in the status; with a draft
        // the first esc clears the draft, and the status says that instead
        let mut w = Session::new("", "medium");
        on(&mut w, serde_json::json!({"type": "harness_user", "text": "go"}), 1_000.0);
        on(&mut w, serde_json::json!({"type": "stream_event", "event": {"type": "message_start"}}), 1_500.0);
        (v.input, v.hints, v.cursor, v.expand_all, v.clock_ms) = (String::new(), vec![], 0, false, 13_000.0);
        let screen = render(&w, &v);
        assert!(screen.contains("Working  12 s · thinking") && screen.contains(" working · esc stops"), "{screen}");
        (v.input, v.cursor) = ("half a thought".into(), 14);
        let screen = render(&w, &v);
        assert!(screen.contains("working · esc clears the draft") && !screen.contains("esc stops"), "{screen}");
        v.vim = "INSERT";
        assert!(!render(&w, &v).contains("· esc"), "vim's esc changes mode");
    }

    /// The review of design review 11's fixes: the question pinned to the wrong one of two calls, a plan's call as raw
    /// JSON, "✓ done" after esc during the tests, the menu's selected row out of sight in a short box, zen's draft pushing
    /// its header and completions off a 12-row screen, a long status over the mark.
    #[test]
    fn a_question_marks_its_own_call_and_small_screens_keep_what_matters() {
        let on = |s: &mut Session, e: serde_json::Value, at: f64| s.apply(&e, at);
        let view = |d: Design| View { design: d, expand_all: false, scroll: 0, clock_ms: 0.0, speed: 1.0, paused: false, live: true, input: String::new(), cursor: 0, select: None, asking: None, mode: "ask".into(), hints: Vec::new(), menu: Vec::new(), menu_sel: 0, bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None, mouse: None, list: None, hover_call: None, login: None, cold: 0, confirm: false, ask_risky: false, cant_run: None };
        let render = |s: &Session, v: &View, w: u16, h: u16| {
            let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(w, h)).unwrap();
            term.draw(|f| { draw(f, s, v); }).unwrap();
            let buf = term.backend().buffer().clone();
            (0..h).map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()).collect::<Vec<_>>().join("\n")
        };
        let row = |screen: &str, what: &str| screen.lines().find(|l| l.contains(what)).unwrap_or_else(|| panic!("no {what} in\n{screen}")).to_string();
        // two edits in one message, the question about the first
        let edit = |id: &str, path: &str| serde_json::json!({"type": "tool_use", "id": id, "name": "edit", "input": {"path": path, "old": format!("old {path}"), "new": format!("new {path}")}});
        let mut s = Session::new("", "medium");
        on(&mut s, serde_json::json!({"type": "assistant", "message": {"content": [edit("c1", "a.rs"), edit("c2", "b.rs")]}}), 1.0);
        on(&mut s, serde_json::json!({"type": "harness_permission", "tool": "edit", "input": {"path": "a.rs", "old": "old a.rs", "new": "new a.rs"}}), 2.0);
        let mut v = view(Design::Zen);
        v.asking = Some("edit: a.rs".into());
        let zen = render(&s, &v, 100, 30);
        assert!(row(&zen, "? edit").contains("a.rs") && zen.contains("- old a.rs") && !zen.contains("- old b.rs") && !zen.contains("ran 2 tools") && !zen.contains("ran 1 tool"), "{zen}");
        v.design = Design::Cockpit;
        let cockpit = render(&s, &v, 110, 30);
        assert!(row(&cockpit, "? edit").contains("a.rs") && row(&cockpit, "? edit").contains("waits for your answer") && !row(&cockpit, " b.rs").contains("waits"), "{cockpit}");
        // answered: the question about the second marks the second
        on(&mut s, serde_json::json!({"type": "harness_permission_done", "tool": "edit", "allow": true, "always": false, "mode": "default"}), 3.0);
        on(&mut s, serde_json::json!({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": "c1", "content": "ok"}]}}), 3.0);
        on(&mut s, serde_json::json!({"type": "harness_permission", "tool": "edit", "input": {"path": "b.rs", "old": "old b.rs", "new": "new b.rs"}}), 4.0);
        v.asking = Some("edit: b.rs".into());
        let cockpit = render(&s, &v, 110, 30);
        assert!(row(&cockpit, "? edit").contains("b.rs"), "{cockpit}");

        // a plan to approve: its text, not its call as JSON over the question
        let mut s = Session::new("", "medium");
        on(&mut s, serde_json::json!({"type": "assistant", "message": {"content": [{"type": "tool_use", "id": "p1", "name": "ExitPlanMode", "input": {"plan": "## Plan\n\n1. do a thing\n2. do another"}}]}}), 1.0);
        on(&mut s, serde_json::json!({"type": "harness_permission", "tool": "ExitPlanMode", "input": {"plan": "## Plan\n\n1. do a thing\n2. do another"}}), 2.0);
        let mut v = view(Design::Zen);
        v.asking = Some("ExitPlanMode: approve this plan and start editing".into());
        let zen = render(&s, &v, 100, 30);
        assert!(zen.contains("do a thing") && !zen.contains("{\"plan\"") && !zen.contains("\\n"), "{zen}");

        // esc while the tests run: the run ends as stopped, whatever the reply's own end row says
        let mut s = Session::new("", "medium");
        on(&mut s, serde_json::json!({"type": "harness_user", "text": "change it"}), 0.0);
        on(&mut s, serde_json::json!({"type": "result", "subtype": "success", "total_cost_usd": 0.0}), 1.0);
        on(&mut s, serde_json::json!({"type": "harness_verify_start", "cmd": "cargo test", "round": 1}), 2.0);
        on(&mut s, serde_json::json!({"type": "harness_verify_cancel"}), 3.0);
        on(&mut s, serde_json::json!({"type": "harness_verify_done", "cmd": "cargo test", "ok": true, "secs": 1.0, "output": "", "final": true}), 4.0);
        let v = view(Design::Zen);
        assert_eq!(activity(&s, &v, Design::Zen.theme()).1, "stopped");
        on(&mut s, serde_json::json!({"type": "harness_user", "text": "again"}), 5.0);
        on(&mut s, serde_json::json!({"type": "result", "subtype": "success", "total_cost_usd": 0.0}), 6.0);
        assert_eq!(activity(&s, &v, Design::Zen.theme()).1, "done", "the next run is its own");

        // the / menu in a short window: the selected row is on screen
        let s = Session::new("", "medium");
        for d in [Design::Cockpit, Design::Zen] {
            let mut v = view(d);
            (v.input, v.cursor) = ("/".into(), 1);
            v.menu = (0..12).map(|i| (format!("/cmd{i}"), "does a thing".to_string())).collect();
            v.menu_sel = 9;
            let screen = render(&s, &v, 40, 12);
            assert!(screen.contains("›/cmd9"), "{screen}");
        }
        // zen at 40x12 with a six-line draft: the header and the completions' row stay
        let mut v = view(Design::Zen);
        (v.input, v.hints) = ("one\ntwo\nthree\nfour\nfive\nlook at @ma".into(), vec!["main.rs".into()]);
        v.cursor = v.input.len();
        let screen = render(&s, &v, 40, 12);
        assert!(screen.lines().next().is_some_and(|l| l.contains("tazuna")) && screen.contains("look at @ma") && screen.contains("tab › main.rs"), "{screen}");

        // one part a side and still too wide: the status ends in "…" beside the mark
        let line = |parts: &[&str]| Line::from(parts.iter().map(|p| Span::raw(p.to_string())).collect::<Vec<_>>());
        let (l, r) = fit_row(line(&[" 手 ", "tazuna "]), line(&["⠋ running mcp__github__create_pull_request", "   ", "≈$0"]), 40);
        assert!(l.to_string() == " 手 " && r.to_string().ends_with('…') && l.width() + 1 + r.width() <= 40, "{l} | {r}");
    }

    #[test]
    fn colours_fall_back_to_the_256() {
        // the palette's three backgrounds stay apart as greys, the accent stays a blue of the cube
        let idx = |c: Color| if let Color::Rgb(r, g, b) = c { to_256(r, g, b) } else { 0 };
        assert_eq!([DARK.bg, DARK.surface, DARK.field, DARK.border].map(idx), [233, 234, 235, 237]);
        assert_eq!((idx(DARK.accent), to_256(0, 0, 0), to_256(255, 255, 255), to_256(255, 0, 0)), (105, 16, 231, 196));
    }

    /// A list scrolls no further than its last page (/usage went on to a blank page), and a long note wraps under its own
    /// text instead of ending in "…".
    #[test]
    fn a_list_stops_at_its_last_page_and_wraps_a_long_note() {
        let mut rows: Vec<(String, Tone)> = (1..=40).map(|i| (format!("  row {i}"), Tone::Plain)).collect();
        rows.push(("    55%  Review sessions active over many hours, whole sessions with requests in 8 or more distinct hours of the day".into(), Tone::Dim));
        let n = rows.len();
        let p = ListPanel { title: "usage".into(), head: String::new(), rows, scroll: 1000, links: vec![None; n], link_from: 22, hscroll: 0, details: vec![None; n], open: Default::default(), pending: false };
        let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap();
        let mut top = 0;
        term.draw(|f| { top = draw_list(f, &p, Design::Cockpit.theme(), 0.0, f.area()).3; }).unwrap();
        let screen: Vec<String> = (0..24).map(|y| (0..80).map(|x| term.backend().buffer()[(x, y)].symbol().to_string()).collect()).collect();
        let shown = screen.join("\n");
        assert!(top > 0 && top < 40, "a last page, not the last row: {top}");
        assert!(shown.contains("row 40") && shown.contains("8 or more distinct hours") && !shown.contains('…'), "{shown}");
        let filled = screen.iter().filter(|l| l.contains("row ") || l.contains("Review") || l.contains("hours")).count();
        assert!(filled >= 15, "the panel is full to its end, not one row and blank space: {shown}");
        let wrapped = screen.iter().find(|l| l.contains("8 or more")).unwrap();
        let first = screen.iter().find(|l| l.contains("55%  Review")).unwrap();
        assert_eq!(wrapped.find(|c: char| c.is_alphanumeric()), first.find("Review"), "the later line under the text, not under 55%");
    }

    /// /usage at 80 columns: the meters' bars shrink before a reset is cut, and a note's wrapped line is inside the panel
    /// (design review 13).
    #[test]
    fn usage_at_80_columns_keeps_resets_and_wraps_its_notes() {
        let v = serde_json::json!({"plan": [{"label": "Current session", "used": 15.0, "resets": ""}, {"label": "Current week (all models)", "used": 40.0, "resets": ""}],
            "plan_notes": ["Last 24h · 1179 requests · 7 sessions", "Top MCP servers: lean 22% · playwright 2% · claude.ai Atlassian Rovo 1%"]});
        let mut rows: Vec<(String, Tone)> = crate::accounts::lines(&v).into_iter().map(|(t, tone, _)| (t, tone)).collect();
        // claude's resets as the desktop words them (not read from the clock here)
        for r in rows.iter_mut().filter(|r| r.0.contains('█') || r.0.contains('░')) {
            r.0.push_str("   resets in 4 h 13 min");
        }
        let n = rows.len();
        let p = ListPanel { title: "usage".into(), head: String::new(), rows, scroll: 0, links: vec![None; n], link_from: 22, hscroll: 0, details: vec![None; n], open: Default::default(), pending: false };
        // the band at 80 columns, and a narrower one where the note wraps
        for w in [80u16, 66] {
            let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(w, 24)).unwrap();
            term.draw(|f| { draw_list(f, &p, Design::Cockpit.theme(), 0.0, f.area()); }).unwrap();
            let screen: Vec<String> = (0..24).map(|y| (0..w).map(|x| term.backend().buffer()[(x, y)].symbol().to_string()).collect()).collect();
            let shown = screen.join("\n");
            assert_eq!(shown.matches("resets in 4 h 13 min").count(), 2, "{shown}");
            let words: String = shown.replace('│', " ").split_whitespace().collect::<Vec<_>>().join(" ");
            assert!(words.contains("claude.ai Atlassian Rovo 1%"), "the note whole: {shown}");
            assert!(!shown.contains('…') && !shown.contains("sideways"), "nothing cut: {shown}");
            let bar = |t: &str| screen.iter().find(|l| l.contains(t)).and_then(|l| l.find('━').or(l.find('─')));
            assert_eq!(bar("5-hour session"), bar("Week, all models"), "the bars still line up: {shown}");
        }
    }

    #[test]
    fn fetch_rows_show_a_title_link_and_scroll_sideways() {
        let tool = |result: &str, url: &str| Tool { id: "f".into(), name: "fetch".into(), input: serde_json::json!({"url": url}), result: Some(result.into()), error: false, started_ms: 0.0, done_ms: Some(1.0), sub: None, at: None };
        assert_eq!(fetch_label(&tool("[fetched https://x.com/a · \"Crypto prices today\", HTTP 200: web content]\nbody", "https://x.com/a")), "Crypto prices today");
        assert_eq!(fetch_label(&tool("[fetched https://api.coingecko.com/api/v3/global, HTTP 200: web content]\n{}", "https://api.coingecko.com/api/v3/global?x=1")), "api.coingecko.com/api/v3/global");
        let mut p = ListPanel { title: "fetch".into(), head: "  #   took     what".into(), rows: vec![(format!("{:>3}  {:>7}  {}  {}", 1, "0.5s", "✓", "Crypto prices today"), Tone::Plain)], scroll: 0,
            links: vec![Some("https://x.com/a".into())], link_from: 17, hscroll: 0, details: vec![None], open: Default::default(), pending: false };
        let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 20)).unwrap();
        let mut out = (None, vec![], vec![], 0);
        term.draw(|f| { out = draw_list(f, &p, Design::Cockpit.theme(), 0.0, f.area()); }).unwrap();
        let (area, links, _, _) = out.clone();
        assert!(area.is_some() && links.len() == 1 && links[0].1 == "https://x.com/a");
        let r = links[0].0;
        assert_eq!(term.backend().buffer()[(r.x, r.y)].symbol(), "C", "the link starts at the title");
        p.hscroll = 10;
        term.draw(|f| { out = draw_list(f, &p, Design::Cockpit.theme(), 0.0, f.area()); }).unwrap();
        assert_eq!(out.1[0].0.x, r.x - 10, "scrolled sideways, the link moves with its text");
        // still reading (/usage): the first row turns
        let p = ListPanel { rows: vec![("reading your accounts…".into(), Tone::Plain)], links: vec![None], details: vec![None], hscroll: 0, pending: true, ..p };
        let shown = |t: &ratatui::Terminal<ratatui::backend::TestBackend>| t.backend().buffer().content().iter().map(|c| c.symbol()).collect::<String>();
        term.draw(|f| { draw_list(f, &p, Design::Cockpit.theme(), 0.0, f.area()); }).unwrap();
        assert!(shown(&term).contains("⠋ reading your accounts…"));
        term.draw(|f| { draw_list(f, &p, Design::Cockpit.theme(), 85.0, f.area()); }).unwrap();
        assert!(shown(&term).contains("⠙ reading your accounts…"), "the next frame");
    }

    #[test]
    fn the_diff_view_lists_files_and_long_notices_open() {
        let files = vec![Changed { path: "tazuna/examples/studio.rs".into(), added: 868, removed: 0, diff: None }, Changed { path: "src/a.rs".into(), added: 3, removed: 2, diff: None }];
        let p = DiffPanel { selected: 0, diff: vec!["+//! a line of code".into()], scroll: 0 };
        let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 20)).unwrap();
        let mut hits = vec![];
        term.draw(|f| { hits = draw_diff(f, &files, &p, Design::Cockpit.theme(), None, f.area()); }).unwrap();
        let text: String = term.backend().buffer().content().iter().map(|c| c.symbol()).collect();
        assert!(text.contains("tazuna/examples/studio.rs") && text.contains("+868") && !text.contains("a line of code"), "files and counts, no diff text");
        assert_eq!(hits.len(), 2);
        // a notice longer than NOTICE_CHARS: cut and clickable, opened all of it
        let mut s = Session::new("", "medium");
        s.blocks.push(Block::Notice { text: "x".repeat(300), error: false, ended: false });
        let lines = |open: bool| block_lines(&s.blocks[0], 0, &s, &View { design: Design::Cockpit, expand_all: open, scroll: 0, clock_ms: 0.0, speed: 1.0, paused: false, live: true, input: String::new(), cursor: 0, select: None, asking: None, mode: "auto".into(), hints: Vec::new(), menu: Vec::new(), menu_sel: 0, bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None, mouse: None, list: None, hover_call: None, login: None, cold: 0, confirm: false, ask_risky: false, cant_run: None }, Design::Cockpit.theme(), &cockpit::OPTS, open, 400).iter().map(|(l, _)| l.to_string()).collect::<String>();
        assert!(lines(false).contains("click to show") && !lines(true).contains("click to show"));
    }

    #[test]
    fn the_cost_box_names_the_account_in_use_and_one_that_needs_you() {
        let th = Design::Cockpit.theme();
        let text = |l: &Line| l.spans.iter().map(|x| x.content.to_string()).collect::<String>();
        let mut s = Session::new("", "medium");
        s.limits = Some((47.0, 65.0));
        s.credits = vec![("SubRouter".into(), Some("$5.00 left".into()), false), ("Mistral".into(), None, false)];
        let (a, b) = account_rows(&s, th);
        assert!(text(&a).starts_with("Claude 5h 47%") && text(&b).is_empty(), "on Claude: its plan, nothing else needs you");
        (s.provider, s.price_m) = (Some("Qwen Cloud Token Plan".into()), Some((0.0, 0.0, 0.0)));
        (s.credits[0].1, s.credits[0].2) = (Some("$0.22 left".into()), true);
        let (a, b) = account_rows(&s, th);
        assert_eq!((text(&a).as_str(), text(&b).as_str()), ("Qwen Cloud Token Plan plan", "SubRouter $0.22 left"), "a plan, and a low balance elsewhere");
        s.provider = Some("SubRouter".into());
        assert_eq!(text(&account_rows(&s, th).0), "SubRouter $0.22 left");
        (s.provider, s.limits) = (Some("Mistral".into()), Some((92.0, 65.0)));
        let (a, b) = account_rows(&s, th);
        assert!(text(&a) == "Mistral no balance it reports" && text(&b).starts_with("Claude 5h 92%"), "Claude's plan nearly used shows on another model");
    }

    #[test]
    fn a_question_shows_its_call_whole() {
        let (tool, note, call) = asking_parts("Bash: echo one; vm_stat | head -12; sysctl vm.swapusage  (always = this command)", 40);
        assert_eq!((tool.as_str(), note), ("Bash", vec![]), "\"always = this command\" is no reason (design review 14)");
        assert_eq!(call.join(" "), "echo one; vm_stat | head -12; sysctl vm.swapusage", "nothing cut at 40 columns: {call:?}");
        let long = format!("bash: {}  (once only: the agent is answering a chat's question)", "word ".repeat(200));
        let (_, note, call) = asking_parts(&long, 40);
        assert!(call.len() == ASK_ROWS && call.last().unwrap().ends_with('…') && note[0].starts_with("the agent is answering"), "{note:?}");
        let rows = asking_rows(&long, Design::Cockpit.theme(), 40, false, false);
        assert_eq!(rows.len(), 1 + note.len() + ASK_ROWS + 1, "what asks, its note under it, the call, the answers");
        assert!(!rows.last().unwrap().spans.iter().any(|s| s.content.contains("always")), "once only: no always");
        // what is typed while it waits stays in the box under it, and the answers wait for an empty box (design review 13)
        let v = View { asking: Some(long.clone()), input: "explain closures".into(), cursor: 16, ..live_view() };
        let rows: Vec<String> = composer_rows(&v, Design::Cockpit.theme(), "›", "", 40).iter().map(|l| l.spans.iter().map(|s| s.content.to_string()).collect()).collect();
        assert!(rows.iter().any(|r| r.contains("answer with an empty box")) && rows.last().unwrap().contains("explain closures"), "{rows:#?}");
        assert_eq!(composer_height(&v, 42) as usize, rows.len() + 2, "the box holds the question and the draft");
        // auto's reason wraps whole at any width: it was cut at the edge at 100 columns (design review 13)
        let q = "bash: git push --force origin main  (once only · auto asks: git push --force: rewrites the remote's history)";
        for outer in [100u16, 80, 60] {
            let v = View { asking: Some(q.into()), ..live_view() };
            let rows = asking_rows(q, Design::Cockpit.theme(), outer - 2, false, false);
            let text: Vec<String> = rows.iter().map(|l| l.spans.iter().map(|s| s.content.to_string()).collect()).collect();
            assert!(text.iter().all(|t| t.width() < (outer - 2) as usize), "{outer}: {text:#?}");
            assert!(text.join(" ").split_whitespace().collect::<Vec<_>>().join(" ").contains("git push --force rewrites the remote's history"), "{outer}: {text:#?}");
            assert_eq!(composer_height(&v, outer) as usize, rows.len() + 2, "{outer}: the box holds every row");
        }
        // design review 14: the question alone on its row, the reason as a sentence, "y allow   n decline"
        let text = |q: &str| asking_rows(q, Design::Cockpit.theme(), 98, false, false).iter().map(|l| l.spans.iter().map(|s| s.content.to_string()).collect::<String>()).collect::<Vec<_>>();
        assert_eq!(text(q), [" ? Allow this command?", "   git push --force rewrites the remote's history", "   git push --force origin main", "   y allow   n decline"]);
        assert_eq!(text("Edit: src/main.rs  (always = this tool)"), [" ? Allow this edit?", "   src/main.rs", "   y allow   a always   n decline"]);
        assert_eq!(text("WebFetch: https://x.dev  (always = this tool)")[0], " ? Allow WebFetch?");
        // the own loop's tools carry their reason in the name (engine.rs): said once, under the question
        assert_eq!(text("edit (outside the project): /etc/hosts  (once only: outside the project)"), [" ? Allow this edit?", "   outside the project", "   /etc/hosts", "   y allow   n decline"]);
        assert_eq!(text("bash (written as text by the model): ls  (once only: the model wrote this call as text)")[..2], [" ? Allow this command?", "   the model wrote this call as text"]);
        // a draft in the box: "answer with an empty box" beside the answers where it fits, else on its own row, never cut
        let q2 = "bash: ls -la  (always = this command)";
        for (outer, alone) in [(100u16, false), (80, false), (60, true)] {
            let v = View { asking: Some(q2.into()), input: "and then".into(), cursor: 8, ..live_view() };
            let rows: Vec<String> = composer_rows(&v, Design::Cockpit.theme(), "›", "", outer - 2).iter().map(|l| l.spans.iter().map(|s| s.content.to_string()).collect()).collect();
            let keys = rows.iter().position(|r| r.contains("y allow")).unwrap();
            assert!(rows.iter().all(|r| r.width() <= (outer - 2) as usize) && rows[keys + usize::from(alone)].ends_with("answer with an empty box"), "{outer}: {rows:#?}");
            assert_eq!(composer_height(&v, outer) as usize, rows.len() + 2, "{outer}: the box holds every row");
            // zen, its status beside the box: whole there too
            let mut idle = Session::new("", "medium");
            idle.activity = Activity::Ready;
            let screen = rows_of(&frame(&idle, &View { design: Design::Zen, ..v }, outer, 24).0).join("\n");
            assert!(screen.contains("answer with an empty box") && screen.contains("n decline"), "zen {outer}: {screen}");
        }
        assert_eq!((reason("once only · auto asks: dd: a disk or system tool"), reason("once only · auto asks: rm outside the project: /x"), reason("once only: outside the project")),
            ("dd is a disk or system tool".into(), "rm outside the project: /x".into(), "outside the project".into()));
    }

    #[test]
    fn an_update_says_each_version_on_its_own_line() {
        let th = Design::Cockpit.theme();
        let text = |n: &crate::versions::News| news_lines(n, th).iter().map(|(l, _)| l.spans.iter().map(|x| x.content.to_string()).collect::<String>()).collect::<Vec<_>>();
        let items = |k: usize| (0..k).map(|i| (format!("0.1.{}", 199 - i), format!("change {i}"))).collect();
        let five = text(&crate::versions::News { from: "0.1.194".into(), to: "0.1.199".into(), items: items(5) });
        assert_eq!(five, ["↑ updated to 0.1.199  from 0.1.194 · 5 versions", "  0.1.199  change 0", "  0.1.198  change 1", "  0.1.197  change 2",
            "  and 2 more · /changelog has them all"]);
        let one = text(&crate::versions::News { from: "0.1.198".into(), to: "0.1.199".into(), items: items(1) });
        assert_eq!(one, ["↑ updated to 0.1.199  from 0.1.198", "  0.1.199  change 0", "  /changelog has every note"]);
    }

    #[test]
    fn a_fade_runs_straight_from_the_background_to_the_colour() {
        // tachyonfx blends in HSL by default: the accent coming up from the warm background passed through magenta (design
        // review 10). Half-way, every channel must lie between the two ends.
        let (d, area) = (Design::Zen, Rect::new(0, 0, 2, 1));
        let th = d.theme();
        let mut buf = ratatui::buffer::Buffer::empty(area);
        buf[(0, 0)].set_symbol("x").set_fg(th.accent);
        let mut e = boot_fx(d);
        e.process(std::time::Duration::from_millis(60).into(), &mut buf, area);
        let (Color::Rgb(r, g, b), Color::Rgb(r0, g0, b0), Color::Rgb(r1, g1, b1)) = (buf[(0, 0)].fg, th.bg, th.accent) else { panic!("rgb") };
        let between = |v: u8, a: u8, z: u8| (a.min(z)..=a.max(z)).contains(&v);
        assert!(between(r, r0, r1) && between(g, g0, g1) && between(b, b0, b1) && (r, g, b) != (r1, g1, b1), "{r},{g},{b}");
    }

    #[test]
    fn a_modal_fades_what_is_behind_it() {
        let s = Session::new("", "medium");
        let mut v = View { design: Design::Cockpit, expand_all: false, scroll: 0, clock_ms: 0.0, speed: 1.0, paused: false, live: true, input: String::new(), cursor: 0, select: None, asking: None, mode: "auto".into(), hints: Vec::new(), menu: Vec::new(), menu_sel: 0, bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None, mouse: None, list: None, hover_call: None, login: None, cold: 0, confirm: false, ask_risky: false, cant_run: None };
        let th = Design::Cockpit.theme();
        let cell = |v: &View| {
            let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 40)).unwrap();
            term.draw(|f| { draw(f, &s, v); }).unwrap();
            let b = term.backend().buffer();
            (b[(3, 0)].fg, b[(60, 20)].symbol().to_string())
        };
        assert_ne!(cell(&v).0, th.border, "the header in its own colours without a modal");
        v.list = Some(ListPanel { title: "model calls".into(), head: "#".into(), rows: vec![("1".into(), Tone::Plain)], scroll: 0, links: vec![], link_from: 0, hscroll: 0, details: vec![], open: Default::default(), pending: false });
        let (fg, _) = cell(&v);
        assert_eq!(fg, th.border, "behind a modal everything takes the border colour");
    }

    #[test]
    fn files_git_misses_get_the_tools_own_diff() {
        let mut s = Session::new("", "medium");
        s.cwd = "/repo".into();
        let tool = |id: &str, name: &str, input: serde_json::Value, error: bool| Block::Tool(Tool { id: id.into(), name: name.into(), input, result: Some("ok".into()), error, started_ms: 0.0, done_ms: Some(10.0), sub: None, at: None });
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
        let p = (1..=20).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n\n");
        let text = |open| prompt_lines(&p, th, open, 80, true).iter().map(|(l, _)| l.to_string()).collect::<Vec<_>>().join("\n");
        let closed = text(false);
        assert!(closed.contains("more lines · click to show") && !closed.contains("line 20"), "{closed}");
        let opened = text(true);
        assert!(opened.contains("line 20") && !opened.contains("more lines"), "{opened}");
        let replay = prompt_lines(&p, th, false, 80, false).iter().map(|(l, _)| l.to_string()).collect::<String>();
        assert!(replay.contains("e to expand") && !replay.contains("click to show"), "{replay}");
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

    /// A frame of `s` as `v` draws it at `w`×`h`: its cells, and where the transcript went.
    fn frame(s: &Session, v: &View, w: u16, h: u16) -> (ratatui::buffer::Buffer, Rect) {
        let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(w, h)).unwrap();
        let mut at = Rect::default();
        term.draw(|f| at = draw(f, s, v).transcript).unwrap();
        (term.backend().buffer().clone(), at)
    }

    /// Its rows as text, a cell a character (a wide character's second cell a space).
    fn rows_of(b: &ratatui::buffer::Buffer) -> Vec<String> {
        (0..b.area.height).map(|y| (0..b.area.width).map(|x| b[(x, y)].symbol()).collect()).collect()
    }

    fn apply(s: &mut Session, e: serde_json::Value, at: f64) {
        s.apply(&e, at);
    }

    /// Design review 13, T1: while a run works its live end says so, in both designs at 80 and 60 columns: the mark
    /// pulsing in the accent, "Working" with one cell lit (the next on the next spinner frame), the time, the output so far
    /// and the step, the row breaking between facts; zen's status keeps a short state. A question or the run's end takes
    /// the row away.
    #[test]
    fn a_run_at_work_says_so_at_the_live_end() {
        let mut s = Session::new("", "medium");
        apply(&mut s, serde_json::json!({"type": "harness_user", "text": "change the greeting"}), 1_000.0);
        apply(&mut s, serde_json::json!({"type": "stream_event", "event": {"type": "message_start"}}), 1_100.0);
        apply(&mut s, serde_json::json!({"type": "stream_event", "event": {"type": "message_delta", "usage": {"output_tokens": 1200}}}), 2_000.0);
        apply(&mut s, serde_json::json!({"type": "assistant", "message": {"content": [{"type": "tool_use", "id": "c1", "name": "edit",
            "input": {"path": "src/components/settings/appearance.rs", "old": "a", "new": "b"}}]}}), 2_100.0);
        let th = Design::Zen.theme();
        for design in [Design::Zen, Design::Cockpit] {
            for (w, h) in [(80u16, 24u16), (60, 20)] {
                // frame 156 lights "W", 157 "o"; ten frames on the mark is half a pulse away
                let at = |clock_ms: f64| frame(&s, &View { design, clock_ms, ..live_view() }, w, h).0;
                let b = at(12_480.0);
                let rows = rows_of(&b);
                let screen = rows.join("\n");
                let y = rows.iter().position(|r| r.contains("Working")).unwrap_or_else(|| panic!("{design:?} {w}: {screen}"));
                assert!(rows[y].contains("手  Working  12 s · ↓ 1.2k tokens ·") && rows[y + 1].trim_matches([' ', '│']).starts_with("editing src/components/settings/appearance.rs"),
                    "{design:?} {w}: the facts whole, the row broken between them: {screen}");
                let (x, yy) = (rows[y].chars().position(|c| c == 'W').unwrap() as u16, y as u16);
                let lit = |b: &ratatui::buffer::Buffer| (0..7u16).filter(|i| b[(x + i, yy)].fg == th.accent).collect::<Vec<_>>();
                assert_eq!((lit(&b), lit(&at(12_560.0))), (vec![0], vec![1]), "{design:?} {w}: one cell lit, the next on the next frame");
                assert_eq!(b[(x, yy)].symbol(), "W");
                let mark = x - 3;
                assert!(b[(mark, yy)].symbol() == "手" && b[(mark, yy)].fg != at(13_280.0)[(mark, yy)].fg, "{design:?} {w}: the mark pulses");
                if design == Design::Zen {
                    let status = rows.iter().rev().find(|r| !r.trim().is_empty()).unwrap();
                    assert!(status.contains(" working · esc stops") && !status.contains("12 s"), "{w}: the status in short: {status}");
                }
                let asked = View { design, clock_ms: 12_480.0, asking: Some("edit: src/components/settings/appearance.rs".into()), ..live_view() };
                assert!(!rows_of(&frame(&s, &asked, w, h).0).concat().contains("Working"), "a question speaks for itself");
            }
        }
        apply(&mut s, serde_json::json!({"type": "result", "subtype": "success", "total_cost_usd": 0.0}), 13_000.0);
        assert!(!rows_of(&frame(&s, &View { design: Design::Zen, clock_ms: 13_100.0, ..live_view() }, 80, 24).0).concat().contains("Working"), "the run is over");
    }

    /// T2: your message reads as yours, the same in both designs at 80 and 60 columns: an accent bar, upright text in the
    /// second ink, on a band the transcript's width, wrapped rows too; cockpit's "you" label is gone.
    #[test]
    fn your_message_is_a_band_with_an_accent_bar() {
        let mut s = Session::new("", "medium");
        apply(&mut s, serde_json::json!({"type": "harness_user", "text": "now add a test for the second line too, and run it with cargo test"}), 0.0);
        apply(&mut s, serde_json::json!({"type": "result", "subtype": "success", "total_cost_usd": 0.0}), 1.0);
        let th = Design::Zen.theme();
        for design in [Design::Zen, Design::Cockpit] {
            for (w, h) in [(80u16, 24u16), (60, 20)] {
                let (b, area) = frame(&s, &View { design, ..live_view() }, w, h);
                let rows = rows_of(&b);
                let screen = rows.join("\n");
                let y = rows.iter().position(|r| r.contains("now add a test")).unwrap() as u16;
                let wrapped = if w == 60 { 2 } else { 1 };
                for row in y..y + wrapped {
                    assert_eq!((b[(area.x, row)].symbol(), b[(area.x, row)].fg), ("▎", th.accent), "{design:?} {w}: {screen}");
                    assert!((area.x..area.right()).all(|x| b[(x, row)].bg == th.surface), "{design:?} {w}: a band the column's width: {screen}");
                }
                let first = b[(area.x + 2, y)].clone();
                assert!(first.symbol() == "n" && first.fg == th.ink2 && !first.modifier.contains(Modifier::ITALIC), "{design:?} {w}: upright, second ink");
                assert!(b[(area.x, y - 1)].bg != th.surface && b[(area.x, y + wrapped)].bg != th.surface, "air around the band");
                assert!(!rows.iter().any(|r| r.contains("▎ you") || r.contains("│ you")), "{design:?} {w}: {screen}");
            }
        }
    }

    /// T3: a finished run says what it changed on its Done row, file by file, each file whole at 100, 80 and 60 columns,
    /// with "1 failed" kept on the tools row and a write over a file it had read as "rewritten"; ctrl-o's diffs are bands of
    /// the err and ok colours with old and new line numbers (the bar where the file does not hold the edit exactly once),
    /// the changed words on a stronger band, and context past three lines folded to a `⋯` row.
    #[test]
    fn a_run_says_what_it_changed_and_its_diffs_are_bands() {
        let dir = std::env::temp_dir().join(format!("tazuna-t3-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("main.rs"), "fn main() {\n    println!(\"hello, tazuna\");\n    println!(\"second line\");\n}\n").unwrap();
        std::fs::write(dir.join("dup.rs"), "x = 2;\nx = 2;\n").unwrap();
        std::fs::write(dir.join("long.rs"), "a\nb\nc\nd\ne\nNEW\nz\n").unwrap();
        let mut s = Session::new("", "medium");
        s.cwd = dir.to_string_lossy().into_owned();
        let call = |id: &str, name: &str, input: serde_json::Value| serde_json::json!({"type": "assistant", "message": {"content": [{"type": "tool_use", "id": id, "name": name, "input": input}]}});
        let done = |id: &str, text: &str, error: bool| serde_json::json!({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": id, "content": text, "is_error": error}]}});
        apply(&mut s, serde_json::json!({"type": "harness_user", "text": "change the greeting"}), 0.0);
        apply(&mut s, call("c1", "edit", serde_json::json!({"path": "main.rs", "old": "    println!(\"hi\");", "new": "    println!(\"hello, tazuna\");\n    println!(\"second line\");"})), 1.0);
        apply(&mut s, done("c1", "ok", false), 2.0);
        apply(&mut s, call("c2", "bash", serde_json::json!({"command": "cat nope.txt"})), 3.0);
        apply(&mut s, done("c2", "cat: nope.txt: No such file or directory\n[exit 1]", true), 4.0);
        apply(&mut s, call("c3", "write", serde_json::json!({"path": "NOTES.md", "content": "# Notes\n\nGreeting changed.\n"})), 5.0);
        apply(&mut s, done("c3", "ok", false), 6.0);
        apply(&mut s, call("c4", "edit", serde_json::json!({"path": "src/components/settings/appearance.rs", "old": "let a = 1;\nkeep", "new": "let b = 1;\nkeep"})), 7.0);
        apply(&mut s, done("c4", "ok", false), 8.0);
        apply(&mut s, call("c5", "read", serde_json::json!({"path": "config.rs"})), 8.1);
        apply(&mut s, done("c5", "old config", false), 8.2);
        apply(&mut s, call("c6", "write", serde_json::json!({"path": "config.rs", "content": "one\ntwo\nthree\n"})), 8.3);
        apply(&mut s, done("c6", "ok", false), 8.4);
        // the file holds the new text twice: where it went cannot be told, so no numbers
        apply(&mut s, call("c7", "edit", serde_json::json!({"path": "dup.rs", "old": "x = 1;", "new": "x = 2;"})), 8.5);
        apply(&mut s, done("c7", "ok", false), 8.6);
        // five shared lines above the change: three shown, the rest and the file's line above fold to ⋯
        apply(&mut s, call("c8", "edit", serde_json::json!({"path": "long.rs", "old": "a\nb\nc\nd\ne\nOLD", "new": "a\nb\nc\nd\ne\nNEW"})), 8.7);
        apply(&mut s, done("c8", "ok", false), 8.8);
        apply(&mut s, serde_json::json!({"type": "result", "subtype": "success", "total_cost_usd": 0.0, "duration_ms": 2000}), 9.0);
        let th = Design::Zen.theme();
        for (w, h) in [(100u16, 30u16), (80, 24), (60, 20)] {
            let rows = rows_of(&frame(&s, &View { design: Design::Zen, ..live_view() }, w, h).0);
            let screen = rows.join("\n");
            let at = rows.iter().position(|r| r.contains("✓ Done")).unwrap();
            let change: Vec<&str> = rows[at..].iter().map(|r| r.trim()).take_while(|r| !r.is_empty()).collect();
            assert!(change[0].starts_with("✓ Done  2.0 s  $0   main.rs +2 −1  NOTES.md +3"), "{w}: {screen}");
            for item in ["src/components/settings/appearance.rs +1 −1", "config.rs rewritten", "+2 more", "ctrl-o diffs"] {
                assert!(change.iter().any(|r| r.contains(item)), "{w}: {item} whole: {change:#?}");
            }
            assert!(screen.contains("1 failed") && !change[0].ends_with('▸'), "{w}: {screen}");
        }
        for (w, h) in [(80u16, 70u16), (60, 70)] {
            let (b, area) = frame(&s, &View { design: Design::Zen, expand_all: true, ..live_view() }, w, h);
            let rows = rows_of(&b);
            let screen = rows.join("\n");
            let row = |t: &str| rows.iter().position(|r| r.contains(t)).unwrap_or_else(|| panic!("{w}: no {t:?} in {screen}"));
            let text = |y: usize| rows[y][area.x as usize..].trim_end().to_string();
            let gone = row("println!(\"hi\")");
            assert_eq!([text(gone - 1), text(gone), text(gone + 1), text(gone + 2), text(gone + 3)].map(|t| t.trim_start().to_string()),
                ["1 1   fn main() {", "2   -     println!(\"hi\");", "2 +     println!(\"hello, tazuna\");", "3 +     println!(\"second line\");", "3 4   }"],
                "{w}: line numbers from the file, a line of it above and under: {screen}");
            let (del, add) = (lerp(th.bg, th.err, 0.16), lerp(th.bg, th.ok, 0.16));
            let y = gone as u16;
            assert!(b[(area.right() - 1, y)].bg == del && b[(area.right() - 1, y + 1)].bg == add && b[(area.right() - 1, y - 1)].bg != del, "{w}: bands across: {screen}");
            let hi = rows[gone].find("hi\"").map(|i| rows[gone][..i].chars().count() as u16).unwrap();
            let hello = rows[gone + 1].find("hello").map(|i| rows[gone + 1][..i].chars().count() as u16).unwrap();
            assert!(b[(hi, y)].bg == lerp(th.bg, th.err, 0.38) && b[(hi - 1, y)].bg == del, "{w}: the changed word marked");
            assert!(b[(hello, y + 1)].bg == lerp(th.bg, th.ok, 0.38) && b[(hello - 2, y + 1)].bg == add, "{w}: and its replacement");
            // where the file says nothing: the bar, the shared line as context, the line's change marked
            let a = row("let a = 1;");
            assert!(text(a).trim_start() == "▎ - let a = 1;" && text(a + 1).trim_start() == "▎ + let b = 1;" && text(a + 2).trim_start() == "▎   keep", "{w}: {screen}");
            assert!(text(row("# Notes")).trim_start() == "1 + # Notes", "{w}: a write's lines numbered: {screen}");
            let twice = row("x = 1;");
            assert!(text(twice).trim_start() == "▎ - x = 1;" && text(twice + 1).trim_start() == "▎ + x = 2;", "{w}: a needle found twice: {screen}");
            let changed = row("- OLD");
            assert_eq!([text(changed - 4), text(changed - 3), text(changed - 2), text(changed - 1), text(changed + 1), text(changed + 2)].map(|t| t.trim_start().to_string()),
                ["⋯", "3 3   c", "4 4   d", "5 5   e", "6 + NEW", "7 7   z"].map(String::from), "{w}: {screen}");
        }
        assert_eq!((change_counts("a\nb\nc", "a\nB\nc"), change_counts("", "x\ny"), change_counts("x", "")), ((1, 1), (2, 0), (0, 1)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// T4: the mode shows in both designs at 80 and 60 columns, auto included: a word in its colour, ok for auto, accent for
    /// a mode that asks, and full in the warn colour on a warn-tinted ground, saying it never asks. No [ask] or [full] tag is
    /// left, and no dot (design review 14).
    #[test]
    fn the_mode_is_a_pill_auto_included() {
        let mut s = Session::new("", "medium");
        s.activity = Activity::Ready;
        let th = Design::Zen.theme();
        for design in [Design::Zen, Design::Cockpit] {
            for (w, h) in [(80u16, 24u16), (60, 20)] {
                for (mode, word, colour, ground) in [("auto", " auto ", th.ok, None), ("ask", " ask ", th.accent, None), ("plan", " plan ", th.accent, None),
                    ("full", " full · never asks ", th.warn, Some(lerp(th.bg, th.warn, 0.18)))] {
                    let (b, _) = frame(&s, &View { design, mode: mode.into(), ..live_view() }, w, h);
                    let rows = rows_of(&b);
                    let screen = rows.join("\n");
                    assert!(!screen.contains("[ask]") && !screen.contains("[full]") && !screen.contains("[plan]") && !screen.contains('●'), "{design:?} {w}: {screen}");
                    let y = rows.iter().rposition(|r| r.contains(word)).unwrap_or_else(|| panic!("{design:?} {w} {mode}: {screen}"));
                    let x = rows[y][..rows[y].find(word).unwrap()].chars().count() as u16 + 1;
                    let cell = &b[(x, y as u16)];
                    assert!(cell.fg == colour && ground.is_none_or(|g| cell.bg == g && b[(x - 1, y as u16)].bg == g), "{design:?} {w} {mode}: the word's colour (full on its ground)");
                }
            }
        }
        // a draft typed while a run works: where the mode would leave it under 30 columns, full says just "full"; auto is a
        // word as short
        let mut busy = Session::new("", "medium");
        apply(&mut busy, serde_json::json!({"type": "harness_user", "text": "go"}), 0.0);
        let typed = "a follow up typed while it works";
        for (session, w, mode, shown, gone) in [(&busy, 60u16, "auto", " auto ", "●"), (&busy, 60, "full", " full ", "never asks"), (&s, 60, "auto", " auto ", "●"), (&busy, 100, "full", " full · never asks ", "●")] {
            let v = View { design: Design::Zen, input: typed.into(), cursor: typed.len(), mode: mode.into(), ..live_view() };
            let rows = rows_of(&frame(session, &v, w, 20).0);
            let status = rows.iter().rev().find(|r| r.contains("working") || r.contains("ready")).unwrap();
            assert!(status.contains(shown) && !status.contains(gone) && rows.iter().any(|r| r.contains("› a follow")), "{w} {mode}: {rows:#?}");
        }
        // cockpit under 110 columns: the mode under the draft, on the box's border (no session panel says it there)
        let v = View { input: typed.into(), cursor: typed.len(), mode: "full".into(), ..live_view() };
        let rows = rows_of(&frame(&s, &v, 80, 24).0);
        assert!(rows.iter().any(|r| r.starts_with('╰') && r.contains(" full ")), "{rows:#?}");
    }

    /// T5: a heading gets air above it and an accent mark, in bold ink; simple TeX reads as text in italics without its
    /// fences, at 80 and 60 columns. Money, shell variables and TeX it cannot set stay as written.
    #[test]
    fn headings_have_air_and_simple_maths_reads_as_text() {
        let text = "The lookup is constant: $$t_{lookup} = \\Theta(1)$$\nSection 2 follows.\n## Section 2\nLorem ipsum.\n$$\nn \\times m \\le k \\to \\infty\n$$\nIt costs $5 and $10; see $HOME/$USER, \\(\\alpha \\ge \\beta\\) and $\\frac{a}{b}$.";
        let mut s = Session::new("", "medium");
        s.activity = Activity::Ready;
        s.blocks.push(Block::Text { text: text.into(), live: false });
        let th = Design::Zen.theme();
        for (w, h) in [(80u16, 24u16), (60, 20)] {
            let (b, area) = frame(&s, &View { design: Design::Zen, ..live_view() }, w, h);
            let rows = rows_of(&b);
            let screen = rows.join("\n");
            let row = |t: &str| rows.iter().position(|r| r.contains(t)).unwrap_or_else(|| panic!("{w}: no {t:?} in {screen}")) as u16;
            let y = row("The lookup is constant: t_lookup = Θ(1)");
            assert!(b[(area.x + 24, y)].modifier.contains(Modifier::ITALIC) && !b[(area.x, y)].modifier.contains(Modifier::ITALIC), "{w}: the maths in italics");
            let head = row("▍Section 2");
            assert!(rows[head as usize - 1].trim().is_empty() && rows[head as usize - 2].contains("Section 2 follows."), "{w}: air above the heading: {screen}");
            assert!(b[(area.x, head)].fg == th.accent && b[(area.x + 1, head)].fg == th.text && b[(area.x + 1, head)].modifier.contains(Modifier::BOLD), "{w}");
            assert!(b[(area.x, row("n × m ≤ k → ∞"))].modifier.contains(Modifier::ITALIC), "{w}: a display formula without its fences");
            for kept in ["$5 and $10;", "$HOME/$USER,", "α ≥ β", "$\\frac{a}{b}$."] {
                assert!(screen.contains(kept), "{w}: {kept}: {screen}");
            }
            assert!(!screen.contains("$$") && !screen.contains("\\Theta") && !screen.contains("\\("), "{w}: {screen}");
        }
        // shell, patterns, links and code keep their marks: only what reads as TeX is maths (the review's cases)
        let plain = |t: &str| inline(t, th).iter().map(|sp| sp.content.to_string()).collect::<String>();
        let maths = |t: &str| inline(t, th).iter().filter(|sp| sp.style.add_modifier.contains(Modifier::ITALIC)).map(|sp| sp.content.to_string()).collect::<Vec<_>>();
        for kept in ["export $XDG_CONFIG_HOME/$APP_NAME", "cd $(git rev-parse --show-toplevel)/$DIR", "FOO=$BAR_DIR make", "echo $$ then kill $$",
            "grep '\\(abc\\)\\1' file", "a $x$ variable", "$(a^b)$ stays", "sed -n '/^start$/,/^end$/p'", "grep -E '^foo$|^bar$'", "echo $x^2$y"] {
            assert_eq!((plain(kept), maths(kept).len()), (kept.to_string(), 0), "{kept}");
        }
        assert_eq!(plain("[the $O(n^2)$ algorithm](https://example.com/a) is $n^{2}$"), "the $O(n^2)$ algorithm is n^2", "a link's title stays as written");
        assert_eq!(maths("$$e^x$$ and \\(x^{2}\\)"), ["e^x", "x^2"], "a bare power in $$, a braced one anywhere");
        assert_eq!(maths("`$\\alpha$` and $\\alpha$"), ["α"], "a code span keeps its TeX");
        // a display block sets every line or none: one it cannot set shows as written, fences and all; unclosed it is text
        let md = |t: &str| markdown(t, th, 80).iter().map(|(l, _)| l.to_string().trim_end().to_string()).collect::<Vec<_>>();
        assert_eq!(md("$$\n\\alpha \\to \\beta\n\\frac{a}{b}\n$$"), ["$$", "\\alpha \\to \\beta", "\\frac{a}{b}", "$$"]);
        assert_eq!(md("$$\n\\alpha \\to \\beta\n$$\nafter"), ["α → β", "after"]);
        assert_eq!(md("$$\n\\alpha still streaming"), ["$$", "\\alpha still streaming"]);
        assert_eq!(md("```\n$$\\alpha$$\n```"), [" $$\\alpha$$"], "a code block keeps its TeX");
    }

    /// T6: what a conversation came to, said once as tazuna ends: its runs, the files git and its tools changed (git's
    /// counts where both have a file), the money split as the header splits it, "cost not known" where no run's is, and how
    /// long its runs took; nothing before a run finished. (A continued or resumed one: main.rs's
    /// the_recap_counts_this_sitting_after_c_and_resume, through the real paths.)
    #[test]
    fn leaving_says_what_the_conversation_came_to() {
        let mut s = Session::new("", "medium");
        assert_eq!(recap(&s, &[]), None);
        let file = |path: &str, added: u64, removed: u64| crate::session::FileChange { path: path.into(), added, removed, rewritten: false };
        let run = |secs: f64, cost: Option<f64>, plan: bool, changes: Vec<crate::session::FileChange>| Block::Done(crate::session::Summary { ok: true, secs, turns: 1,
            cost: 0.0, usage: Default::default(), stopped: false, run_cost: cost, run_usage: Default::default(), estimate: false, uncounted: 0, sub_calls: 0, changes,
            bill: Some(crate::session::Bill { model: "m".into(), plan, provider: None, price_m: None, providers: vec![] }) });
        s.blocks.extend([run(60.0, Some(0.01), false, vec![file("notes.txt", 3, 0)]), run(90.0, Some(0.03), true, vec![file("main.rs", 1, 1)]), run(90.0, Some(0.02), false, vec![])]);
        let git = [Changed { path: "main.rs".into(), added: 2, removed: 1, diff: None }];
        assert_eq!(recap(&s, &git).as_deref(), Some("手 tazuna · 3 turns · 2 files changed +5 −1 · $0.03 + $0.03 on Claude plan · 4 min"));
        let mut unknown = Session::new("", "medium");
        unknown.blocks.push(run(5.0, None, false, vec![]));
        assert_eq!(recap(&unknown, &[]).as_deref(), Some("手 tazuna · 1 turn · cost not known · 5 s"));
        let mut one = Session::new("", "medium");
        apply(&mut one, serde_json::json!({"type": "harness_user", "text": "hi"}), 0.0);
        apply(&mut one, serde_json::json!({"type": "result", "subtype": "success", "duration_ms": 2_000, "total_cost_usd": 0.0}), 1.0);
        assert_eq!(recap(&one, &[]).as_deref(), Some("手 tazuna · 1 turn · $0 · 2 s"));
    }

    /// A plan's own loop is named in the money (the user, 2026-11-25): "plan" once whatever the name, the recap says whose
    /// plan, and a plan without a reference price shows the plan under its answer, not "$0".
    #[test]
    fn plan_money_names_its_plan() {
        assert_eq!([plan_word("Mistral Vibe"), plan_word("Alibaba Coding Plan China"), plan_word("MiniMax Plan"), plan_word("Xiaomi Token Plan Europe")],
            ["on Mistral Vibe plan", "on Alibaba Coding Plan China", "on MiniMax Plan", "on Xiaomi Token Plan Europe"]);
        let run = |cost: f64, provider: Option<&str>, price_m: Option<(f64, f64, f64)>| Block::Done(crate::session::Summary { ok: true, secs: 60.0, turns: 1,
            cost: 0.0, usage: Default::default(), stopped: false, run_cost: Some(cost), run_usage: Default::default(), estimate: false, uncounted: 0, sub_calls: 0,
            changes: vec![], bill: Some(crate::session::Bill { model: "m".into(), plan: true, provider: provider.map(String::from), price_m, providers: vec![] }) });
        let mut s = Session::new("", "medium");
        s.blocks.push(run(0.05, Some("Mistral Vibe"), Some((0.3, 0.007, 1.2))));
        assert_eq!(recap(&s, &[]).as_deref(), Some("手 tazuna · 1 turn · $0.05 on Mistral Vibe plan · 1 min"));
        s.blocks.push(run(0.4, None, Some((4.0, 0.4, 20.0))));
        assert_eq!(recap(&s, &[]).as_deref(), Some("手 tazuna · 2 turns · $0.45 on plans · 2 min"));
        let mut k = Session::new("", "medium");
        k.blocks.push(run(0.0, Some("Kimi Code"), None));
        assert_eq!(recap(&k, &[]).as_deref(), Some("手 tazuna · 1 turn · on Kimi Code plan · 1 min"));
        // its answer line, drawn
        let mut a = Session::new("", "medium");
        apply(&mut a, serde_json::json!({"type": "system", "subtype": "init", "model": "kimi-k3", "price_per_m": [0, 0, 0], "provider": "Kimi Code", "plan": true}), 0.0);
        apply(&mut a, serde_json::json!({"type": "harness_user", "text": "hi"}), 0.5);
        apply(&mut a, serde_json::json!({"type": "result", "subtype": "success", "total_cost_usd": 0.0, "duration_ms": 12_000, "tazuna_paid": true}), 13.0);
        let rows = rows_of(&frame(&a, &View { design: Design::Zen, ..live_view() }, 100, 30).0);
        assert!(rows.iter().any(|r| r.trim().starts_with("✓ Done  12 s  on Kimi Code plan")), "{}", rows.join("\n"));
        assert!(!rows.iter().any(|r| r.contains("$0")), "{}", rows.join("\n"));
    }

    /// T7: the terminal's background as it answers OSC 11 (any of its forms), when its answers are in, light or dark from it
    /// unless --theme says, and the terminal theme drawn at 80 and 60 columns: the terminal's own colour everywhere but the
    /// message's band and the pill, on a surface a step from it (the theme's own without an answer).
    #[test]
    fn the_terminal_background_picks_the_palette_and_shows_through() {
        assert_eq!(osc11_colour(b"\x1b]11;rgb:1e1e/1e1e/2e2e\x1b\\\x1b[?62;22c"), Some((0x1e, 0x1e, 0x2e)));
        assert_eq!(osc11_colour(b"typed\x1b]11;rgb:ff/80/0\x07"), Some((255, 128, 0)));
        assert_eq!((osc11_colour(b"\x1b[?62;22c"), osc11_colour(b"\x1b]11;rgb:zz/0/0\x07")), (None, None));
        assert!(attributes_answered(b"\x1b]11;rgb:0/0/0\x1b\\\x1b[?62;22c") && !attributes_answered(b"\x1b]11;rgb:0/0/0\x1b\\\x1b[?62;2"));
        assert!(light_for("auto", Some((0xef, 0xf1, 0xf5))) && !light_for("auto", Some((0x1e, 0x1e, 0x2e))) && !light_for("auto", None));
        assert!(light_for("light", Some((0, 0, 0))) && !light_for("dark", Some((255, 255, 255))) && light_for("terminal", Some((250, 250, 250))));
        let ground = Color::Rgb(0x1e, 0x1e, 0x2e);
        let cat = on_terminal(&DARK, Some((0x1e, 0x1e, 0x2e)));
        assert_eq!((cat.canvas, cat.bg, cat.surface, cat.accent), (Color::Reset, ground, lerp(ground, DARK.text, 0.06), DARK.accent));
        // no answer (Windows, screen, mosh, an older tmux): the theme's own canvas, painted
        let quiet = on_terminal(&LIGHT, None);
        assert_eq!((quiet.canvas, quiet.bg, quiet.surface, quiet.field), (LIGHT.bg, LIGHT.bg, LIGHT.surface, LIGHT.field));
        // an answer split over two reads is read whole once it is in; keys typed around it go to the message box
        let (first, rest) = (&b"fix\x1b]11;rgb:1e1e/1e"[..], &b"1e/2e2e\x1b\\ the\x1b[?62;22c bug\r"[..]);
        assert!(!attributes_answered(first) && attributes_answered(&[first, rest].concat()));
        let probe = |got: &[u8]| { let p = split_answers(got); (p.bg, p.typed, p.enter, p.quit) };
        assert_eq!(probe(&[first, rest].concat()), (Some((0x1e, 0x1e, 0x2e)), "fix the bug".to_string(), true, false), "the Enter after it sends the draft");
        assert_eq!(probe(b"fix the bug"), (None, "fix the bug".to_string(), false, false), "a terminal that never answers: the keys whole");
        assert_eq!(probe(b"\x1b]11;rgb:ff/ff/ff\x07\x1b[?1;2c"), (Some((255, 255, 255)), String::new(), false, false));
        // keys as the box takes them: an arrow key's sequence (CSI and SS3) dropped whole, backspace applied, ctrl-c ends
        assert_eq!(probe(b"fiz\x7fx\x1b[D\x1bOA the bug"), (None, "fix the bug".to_string(), false, false));
        assert_eq!(probe(b"oops\x03"), (None, "oops".to_string(), false, true));
        let mut s = Session::new("", "medium");
        apply(&mut s, serde_json::json!({"type": "harness_user", "text": "how is this repo laid out?"}), 0.0);
        apply(&mut s, serde_json::json!({"type": "result", "subtype": "success", "total_cost_usd": 0.0}), 1.0);
        for (w, h) in [(80u16, 24u16), (60, 20)] {
            let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(w, h)).unwrap();
            let v = View { design: Design::Zen, ..live_view() };
            term.draw(|f| {
                f.render_widget(ratatui::widgets::Block::new().style(Style::new().bg(cat.canvas)), f.area());
                zen::draw(f, &s, &v, &cat);
            }).unwrap();
            let b = term.backend().buffer();
            let rows = rows_of(b);
            let y = rows.iter().position(|r| r.contains("how is this repo")).unwrap() as u16;
            let pill = rows.iter().rposition(|r| r.contains(" auto ")).unwrap() as u16;
            let word = rows[pill as usize].find(" auto ").map(|i| rows[pill as usize][..i].chars().count() as u16 + 1).unwrap();
            assert!(b[(w / 2, y)].bg == cat.surface && b[(word, pill)].fg == cat.ok, "{w}: the band on the surface, the mode a word in its colour");
            assert!(b[(0, 0)].bg == Color::Reset && b[(w / 2, y - 1)].bg == Color::Reset && b[(1, pill)].bg == Color::Reset, "{w}: the canvas left to the terminal");
        }
    }

    /// 256 colours: the diff's bands and marks and a pill's ground take cube colours of their own (dark and light), where the
    /// nearest of the 256 rounded them into the background's grey and they vanished (design review 13).
    #[test]
    fn bands_and_pills_keep_their_tint_in_256_colours() {
        for (th, light, cube) in [(&DARK, false, [52u8, 22, 88, 28, 58, 236]), (&LIGHT, true, [224, 194, 217, 157, 223, 254])] {
            let colours = [lerp(th.bg, th.err, 0.16), lerp(th.bg, th.ok, 0.16), lerp(th.bg, th.err, 0.38), lerp(th.bg, th.ok, 0.38), lerp(th.bg, th.warn, 0.18), th.surface];
            let mut buf = ratatui::buffer::Buffer::empty(Rect::new(0, 0, colours.len() as u16 + 1, 1));
            for (x, c) in colours.iter().enumerate() {
                buf[(x as u16, 0)].set_bg(*c).set_fg(th.text);
            }
            buf[(colours.len() as u16, 0)].set_bg(th.bg);
            fit_to_256(&mut buf, th, light);
            let got: Vec<Color> = (0..colours.len() as u16).map(|x| buf[(x, 0)].bg).collect();
            assert_eq!(got, cube.map(Color::Indexed), "light {light}");
            let canvas = buf[(colours.len() as u16, 0)].bg;
            assert!(got.iter().all(|c| *c != canvas) && buf[(0, 0)].fg != Color::Rgb(0, 0, 0), "light {light}: each apart from the canvas {canvas:?}");
        }
    }
}
