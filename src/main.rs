//! tazuna: a lean terminal UI around Claude Code.
//!
//!   tazuna [--model M] [--effort E] [--design 1-4] [--allow-non-git]   # live: drive claude in the current repo
//!          [--verify "<cmd>" | --no-verify]   # after claude says done, tazuna runs the tests (auto-detected by default)
//!   tazuna <recording.jsonl> [--design 1-4] [--speed 3] [--raw-timing]   # replay a recorded session
//!   tazuna <recording.jsonl> --snapshot <events> [--design N] [--size 110x36]   # render one frame as text
//!   tazuna --run <prompt-file> [--model M] [--effort E]   # headless: one task, prints claude's result JSON (benchmarks)
//!   tazuna --log [N]   # the last N runs from the run log (~/.local/share/tazuna/runs.jsonl)
//!   tazuna mcp   # the lean tool server (claude starts it; not for direct use)
//!
//! Designs: 1 aurora · 2 cockpit (default) · 3 zen · 4 timeline. Live sessions are saved to
//! ~/.local/share/tazuna/sessions/ and replay like any recording.

mod live;
mod mcp;
mod replay;
mod runlog;
mod session;
mod ui;
mod verify;

use anyhow::{Context, Result, bail};
use ratatui::{
    Frame, Terminal,
    backend::TestBackend,
    crossterm::{
        event::{
            self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture, Event, KeyCode, KeyEventKind,
            KeyModifiers, MouseEventKind,
        },
        execute,
    },
};
use replay::Replay;
use serde_json::json;
use session::Session;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use tachyonfx::EffectManager;
use ui::{Design, View};

struct Args {
    path: Option<PathBuf>,
    speed: f64,
    raw_timing: bool,
    snapshot: Option<usize>,
    size: (u16, u16),
    design: Design,
    model: String,
    effort: String,
    allow_non_git: bool,
    run: Option<PathBuf>,
    log: Option<usize>,
    verify: verify::Mode,
}

fn args() -> Result<Args> {
    let mut a = Args {
        path: None,
        speed: 3.0,
        raw_timing: false,
        snapshot: None,
        size: (110, 36),
        design: Design::Cockpit,
        // the best value in our benchmarks: Opus 5.5 at medium effort on the cc-mid profile
        model: "claude-opus-5-5".into(),
        effort: "medium".into(),
        allow_non_git: false,
        run: None,
        log: None,
        verify: verify::Mode::Auto,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut val = || it.next().context(format!("{arg} needs a value"));
        match arg.as_str() {
            "--speed" => a.speed = val()?.parse()?,
            "--raw-timing" => a.raw_timing = true,
            "--snapshot" => a.snapshot = Some(val()?.parse()?),
            "--design" => a.design = Design::from_number(val()?.parse()?).context("--design 1-4")?,
            "--model" => a.model = val()?,
            "--effort" => a.effort = val()?,
            "--allow-non-git" => a.allow_non_git = true,
            "--run" => a.run = Some(val()?.into()),
            "--verify" => a.verify = verify::Mode::Command(val()?),
            "--no-verify" => a.verify = verify::Mode::Off,
            "--log" => a.log = Some(it.next().and_then(|n| n.parse().ok()).unwrap_or(20)),
            "--size" => {
                let v = val()?;
                let (w, h) = v.split_once('x').context("--size WxH")?;
                a.size = (w.parse()?, h.parse()?);
            }
            "-h" | "--help" => {
                println!("{}", include_str!("main.rs").lines().take_while(|l| l.starts_with("//!")).map(|l| l.trim_start_matches("//!")).collect::<Vec<_>>().join("\n"));
                std::process::exit(0);
            }
            _ if arg.starts_with("--") => bail!("unknown flag {arg}"),
            _ => a.path = Some(arg.into()),
        }
    }
    Ok(a)
}

fn main() -> Result<()> {
    if std::env::args().nth(1).as_deref() == Some("mcp") {
        return mcp::serve();
    }
    let a = args()?;
    if let Some(n) = a.log {
        let home = std::env::var("HOME").context("HOME")?;
        return runlog::print_recent(&PathBuf::from(home).join(".local/share/tazuna/runs.jsonl"), n);
    }
    match (&a.path, a.snapshot) {
        (Some(p), Some(n)) => snapshot(Replay::load(p, a.raw_timing)?, n, a.size, a.design),
        (Some(p), None) => {
            let mut replay = Replay::load(p, a.raw_timing)?;
            replay.speed = a.speed;
            let mut term = ratatui::init();
            execute!(std::io::stdout(), EnableMouseCapture)?;
            let r = run_replay(&mut term, replay, &a);
            let _ = execute!(std::io::stdout(), DisableMouseCapture);
            ratatui::restore();
            r
        }
        (None, _) if a.run.is_some() => {
            if !live::in_git_repo() && !a.allow_non_git {
                bail!("not inside a git repo (or pass --allow-non-git)");
            }
            headless(a.run.as_ref().context("--run")?, &a)
        }
        (None, _) => {
            if !live::in_git_repo() && !a.allow_non_git {
                bail!("not inside a git repo: the agent edits files without asking, so run it where `git diff` can review and undo changes (or pass --allow-non-git)");
            }
            let mut live = live::Live::new(&a.model, &a.effort)?;
            live.verify = a.verify.clone();
            let mut term = ratatui::init();
            execute!(std::io::stdout(), EnableBracketedPaste, EnableMouseCapture)?;
            let r = run_live(&mut term, &mut live, &a);
            let _ = execute!(std::io::stdout(), DisableBracketedPaste, DisableMouseCapture);
            ratatui::restore();
            println!("session saved: {}", live.rec_path.display());
            r
        }
    }
}

/// Runs one task without a UI and prints claude's result event, the same JSON `claude -p --output-format json` prints.
fn headless(prompt_file: &PathBuf, a: &Args) -> Result<()> {
    let prompt = std::fs::read_to_string(prompt_file).with_context(|| format!("reading {}", prompt_file.display()))?;
    let mut live = live::Live::new(&a.model, &a.effort)?;
    live.mode = "headless";
    live.verify = a.verify.clone();
    live.send(&prompt)?;
    loop {
        for (_, e) in live.poll() {
            if e["type"] == "harness_notice" && e["error"] == true {
                bail!("{}", e["text"].as_str().unwrap_or("claude failed"));
            }
        }
        if live.idle() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    // claude's last result, with usage and turns summed over every round of the run
    let mut r = live.last_result.clone().context("no result")?;
    if let Some(log) = &live.last_log {
        let t = &log["tokens"];
        r["usage"]["input_tokens"] = t["input"].clone();
        r["usage"]["cache_creation_input_tokens"] = t["cache_write"].clone();
        r["usage"]["cache_read_input_tokens"] = t["cache_read"].clone();
        r["usage"]["output_tokens"] = t["output"].clone();
        r["num_turns"] = log["turns"].clone();
        r["tazuna"] = json!({"verify": log["verify"], "stop": log["stop"], "cost_usd": log["cost_usd"]});
    }
    println!("{r}");
    Ok(())
}

/// Effects state shared by replay and live mode: which changes have already been animated.
struct Fx {
    mgr: EffectManager<&'static str>,
    boot: bool,
    blocks: usize,
    tools: usize,
    results: usize,
}

impl Fx {
    fn new() -> Self {
        Fx { mgr: EffectManager::default(), boot: true, blocks: 0, tools: 0, results: 0 }
    }

    fn frame(&mut self, f: &mut Frame, s: &Session, v: &View, dt: Duration) {
        let areas = ui::draw(f, s, v);
        let d = v.design;
        if self.boot {
            self.mgr.add_unique_effect("boot", ui::boot_fx(d));
            self.boot = false;
        }
        if s.new_blocks != self.blocks {
            self.blocks = s.new_blocks;
            if let Some(r) = areas.last_block {
                self.mgr.add_unique_effect("block", ui::block_fx(d).with_area(r));
            }
        }
        if s.finished_tools != self.tools {
            self.tools = s.finished_tools;
            self.mgr.add_unique_effect("pulse", ui::pulse_fx(d).with_area(areas.pulse));
        }
        if s.results != self.results {
            self.results = s.results;
            if let Some(r) = areas.last_block {
                self.mgr.add_unique_effect("done", ui::done_fx(d).with_area(r));
            }
        }
        let area = f.area();
        self.mgr.process_effects(dt.into(), f.buffer_mut(), area);
    }
}

fn view(design: Design, live: bool, speed: f64) -> View {
    View { design, expand_all: false, scroll: 0, clock_ms: 0.0, speed, paused: false, live, input: String::new() }
}

fn snapshot(mut replay: Replay, n: usize, (w, h): (u16, u16), design: Design) -> Result<()> {
    let mut s = Session::new(&replay.prompt, &replay.effort);
    for (t, e) in replay.take(n) {
        s.apply(e, *t);
    }
    let mut v = view(design, false, replay.speed);
    v.clock_ms = replay.clock;
    let mut term = Terminal::new(TestBackend::new(w, h))?;
    term.draw(|f| {
        ui::draw(f, &s, &v);
    })?;
    let buf = term.backend().buffer();
    for y in 0..h {
        let row: String = (0..w).map(|x| buf[(x, y)].symbol()).collect();
        println!("{}", row.trim_end());
    }
    Ok(())
}

const QUIT_MS: u128 = 480;

fn run_replay(term: &mut ratatui::DefaultTerminal, mut replay: Replay, a: &Args) -> Result<()> {
    let mut s = Session::new(&replay.prompt, &replay.effort);
    let mut v = view(a.design, false, replay.speed);
    let mut fx = Fx::new();
    let mut quit_at: Option<Instant> = None;
    let mut last = Instant::now();
    loop {
        let dt = last.elapsed();
        last = Instant::now();
        for (t, e) in replay.tick(dt.as_secs_f64() * 1000.0) {
            s.apply(e, *t);
        }
        (v.clock_ms, v.speed, v.paused) = (replay.clock, replay.speed, replay.paused);
        term.draw(|f| fx.frame(f, &s, &v, dt))?;

        if let Some(t) = quit_at {
            if t.elapsed().as_millis() > QUIT_MS {
                return Ok(());
            }
        } else if event::poll(Duration::from_millis(16))? {
            let k = match event::read()? {
                Event::Mouse(m) => {
                    scroll_mouse(&mut v, m.kind);
                    continue;
                }
                Event::Key(k) if k.kind == KeyEventKind::Press => k,
                _ => continue,
            };
            match k.code {
                KeyCode::Char('q') | KeyCode::Esc => {
                    fx.mgr.add_unique_effect("quit", ui::quit_fx());
                    quit_at = Some(Instant::now());
                }
                KeyCode::Char(' ') => replay.paused = !replay.paused,
                KeyCode::Right => replay.speed = (replay.speed * 2.0).min(32.0),
                KeyCode::Left => replay.speed = (replay.speed / 2.0).max(0.25),
                KeyCode::Up | KeyCode::Char('k') => v.scroll += 1,
                KeyCode::Down | KeyCode::Char('j') => v.scroll = v.scroll.saturating_sub(1),
                KeyCode::PageUp => v.scroll += 15,
                KeyCode::PageDown => v.scroll = v.scroll.saturating_sub(15),
                KeyCode::Char('G') | KeyCode::End => v.scroll = 0,
                KeyCode::Char('e') => v.expand_all = !v.expand_all,
                KeyCode::Char(c @ '1'..='4') => {
                    v.design = Design::from_number(c as usize - '0' as usize).unwrap_or(v.design);
                    fx.boot = true;
                }
                KeyCode::Tab => {
                    v.design = v.design.next();
                    fx.boot = true;
                }
                KeyCode::Char('r') => {
                    let p = a.path.as_ref().context("no recording")?;
                    replay = Replay::load(p, a.raw_timing)?;
                    replay.speed = v.speed;
                    s = Session::new(&replay.prompt, &replay.effort);
                    v.scroll = 0;
                    fx = Fx::new();
                }
                _ => {}
            }
        } else if replay.finished() && !fx.mgr.is_running() {
            // idle after the replay ends: no need to redraw at 60 fps
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

/// Trackpad and mouse wheel scroll the transcript.
fn scroll_mouse(v: &mut View, kind: MouseEventKind) {
    match kind {
        MouseEventKind::ScrollUp => v.scroll += 3,
        MouseEventKind::ScrollDown => v.scroll = v.scroll.saturating_sub(3),
        _ => {}
    }
}

/// Copies the agent's latest reply to the clipboard (selecting text is awkward while the app captures the mouse).
// ponytail: macOS pbcopy only; add wl-copy/xclip when tazuna runs on Linux
fn copy_last_reply(s: &Session) -> Result<usize> {
    use std::io::Write;
    let text = s.blocks.iter().rev().find_map(|b| if let session::Block::Text { text, .. } = b { Some(text.clone()) } else { None }).context("no reply yet")?;
    let mut child = std::process::Command::new("pbcopy").stdin(std::process::Stdio::piped()).spawn().context("pbcopy")?;
    child.stdin.take().context("pbcopy stdin")?.write_all(text.as_bytes())?;
    child.wait()?;
    Ok(text.chars().count())
}

fn busy(s: &Session) -> bool {
    !matches!(s.activity, session::Activity::Ready | session::Activity::Done)
}

fn run_live(term: &mut ratatui::DefaultTerminal, live: &mut live::Live, a: &Args) -> Result<()> {
    let mut s = Session::new("", &live.effort);
    s.model = live.model.clone();
    s.cwd = std::env::current_dir()?.to_string_lossy().into_owned();
    let mut v = view(a.design, true, 1.0);
    let mut fx = Fx::new();
    let mut quit_at: Option<Instant> = None;
    let mut last = Instant::now();
    loop {
        let dt = last.elapsed();
        last = Instant::now();
        for (t, e) in live.poll() {
            s.apply(&e, t);
        }
        v.clock_ms = live.elapsed_ms();
        term.draw(|f| fx.frame(f, &s, &v, dt))?;

        if let Some(t) = quit_at {
            if t.elapsed().as_millis() > QUIT_MS {
                return Ok(());
            }
            continue;
        }
        if !event::poll(Duration::from_millis(16))? {
            continue;
        }
        let key = match event::read()? {
            Event::Paste(text) => {
                v.input.push_str(&text.replace("\r\n", "\n"));
                continue;
            }
            Event::Mouse(m) => {
                scroll_mouse(&mut v, m.kind);
                continue;
            }
            Event::Key(k) if k.kind == KeyEventKind::Press => k,
            _ => continue,
        };
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            // ctrl-c stops a run in progress; when idle it quits
            KeyCode::Char('c') if ctrl => match busy(&s).then(|| live.interrupt()).flatten() {
                Some((t, e)) => s.apply(&e, t),
                None => {
                    fx.mgr.add_unique_effect("quit", ui::quit_fx());
                    quit_at = Some(Instant::now());
                }
            },
            KeyCode::Char('d') if ctrl => {
                fx.mgr.add_unique_effect("quit", ui::quit_fx());
                quit_at = Some(Instant::now());
            }
            KeyCode::Char('t') if ctrl => {
                v.design = v.design.next();
                fx.boot = true;
            }
            KeyCode::Char('e') if ctrl => v.expand_all = !v.expand_all,
            KeyCode::Char('y') if ctrl => {
                let text = match copy_last_reply(&s) {
                    Ok(n) => format!("copied the last reply ({n} chars)"),
                    Err(e) => format!("copy failed: {e:#}"),
                };
                s.apply(&json!({"type": "harness_info", "text": text}), live.elapsed_ms());
            }
            KeyCode::Enter if key.modifiers.intersects(KeyModifiers::ALT | KeyModifiers::SHIFT) => v.input.push('\n'),
            KeyCode::Enter => {
                let text = v.input.trim().to_string();
                if text.is_empty() {
                    continue;
                }
                match live.send(&text) {
                    Ok((t, e)) => s.apply(&e, t),
                    Err(err) => s.apply(&json!({"type": "harness_notice", "text": format!("{err:#}"), "error": true}), live.elapsed_ms()),
                }
                v.input.clear();
                v.scroll = 0;
            }
            KeyCode::Backspace => {
                v.input.pop();
            }
            KeyCode::Esc => v.input.clear(),
            KeyCode::PageUp => v.scroll += 15,
            KeyCode::PageDown => v.scroll = v.scroll.saturating_sub(15),
            KeyCode::Up => v.scroll += 1,
            KeyCode::Down => v.scroll = v.scroll.saturating_sub(1),
            KeyCode::End => v.scroll = 0,
            KeyCode::Char(c) if !ctrl => v.input.push(c),
            _ => {}
        }
    }
}
