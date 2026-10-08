//! tazuna: a lean terminal UI around Claude Code.
//!
//!   tazuna [--model M] [--effort E] [--design 1-4] [--allow-non-git]   # live: drive claude in the current repo
//!          (without --model/--effort the router picks: tiny repos Sonnet medium, escalating to Opus high if tests
//!          fail; otherwise Opus medium)
//!          [--verify "<cmd>" | --no-verify]   # after claude says done, tazuna runs the tests (auto-detected by default)
//!          [--bash-cap N]   # shrink bash output longer than N chars (default 8000; 0 = off)
//!          [--no-diagnostics]  # do not tell claude about type/compile errors right after its edits
//!          [--self-review]  # finish rules in the prompt, and one self-review turn before verification
//!          [--review-model M [--review-effort E]]  # an independent reviewer on model M checks the diff once
//!          [--profile repo|lean]   # repo (default when the repo has agent config): Claude Code's prompt and tools with
//!                                  the repo's CLAUDE.md, settings, hooks, skills; lean: tazuna's ~30% cheaper cc-mid profile
//!          [--trust] [--user-settings]   # trust the repo's hooks/skills without asking; also load your ~/.claude settings
//!          [--no-project-instructions]   # lean profile: don't add the repo's CLAUDE.md / AGENTS.md to the prompt
//!   tazuna <recording.jsonl> [--design 1-4] [--speed 3] [--raw-timing]   # replay a recorded session
//!   tazuna <recording.jsonl> --snapshot <events> [--design N] [--size 110x36]   # render one frame as text
//!   tazuna --run <prompt-file> [--model M] [--effort E]   # headless: one task, prints claude's result JSON (benchmarks)
//!   tazuna --log [N]   # the last N runs from the run log (~/.local/share/tazuna/runs.jsonl)
//!   tazuna --verdict good|bad [--note "..."] [--id RUN]   # label a run (default: the latest); in the UI: /good, /bad
//!   tazuna --label-git   # label older runs from git: kept (committed) or discarded
//!   tazuna mcp   # the lean tool server (claude starts it; not for direct use)
//!
//! Designs: 1 aurora · 2 cockpit (default) · 3 zen · 4 timeline. Live sessions are saved to
//! ~/.local/share/tazuna/sessions/ and replay like any recording.

mod diag;
mod labels;
mod live;
mod mcp;
mod outcap;
mod replay;
mod review;
mod route;
mod runlog;
mod session;
mod trust;
mod ui;
mod verify;

use anyhow::{Context, Result, bail};
use ratatui::{
    Frame, Terminal,
    backend::TestBackend,
    crossterm::{
        event::{
            self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture, Event, KeyCode, KeyEventKind,
            KeyModifiers, KeyboardEnhancementFlags, MouseEventKind, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
        },
        execute,
        terminal::supports_keyboard_enhancement,
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
    /// None: the router decides.
    model: Option<String>,
    effort: Option<String>,
    allow_non_git: bool,
    run: Option<PathBuf>,
    log: Option<usize>,
    verify: verify::Mode,
    bash_cap: usize,
    diagnostics: bool,
    self_review: bool,
    review_model: Option<String>,
    review_effort: String,
    project_instructions: bool,
    profile: Option<route::Profile>,
    trust: bool,
    user_settings: bool,
    verdict: Option<String>,
    note: String,
    run_id: Option<String>,
    label_git: bool,
}

fn args() -> Result<Args> {
    let mut a = Args {
        path: None,
        speed: 3.0,
        raw_timing: false,
        snapshot: None,
        size: (110, 36),
        design: Design::Cockpit,
        model: None,
        effort: None,
        allow_non_git: false,
        run: None,
        log: None,
        verify: verify::Mode::Auto,
        bash_cap: mcp::DEFAULT_BASH_CAP,
        diagnostics: true,
        self_review: false,
        review_model: None,
        review_effort: "medium".into(),
        project_instructions: true,
        profile: None,
        trust: false,
        user_settings: false,
        verdict: None,
        note: String::new(),
        run_id: None,
        label_git: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut val = || it.next().context(format!("{arg} needs a value"));
        match arg.as_str() {
            "--speed" => a.speed = val()?.parse()?,
            "--raw-timing" => a.raw_timing = true,
            "--snapshot" => a.snapshot = Some(val()?.parse()?),
            "--design" => a.design = Design::from_number(val()?.parse()?).context("--design 1-4")?,
            "--model" => a.model = Some(val()?),
            "--effort" => a.effort = Some(val()?),
            "--allow-non-git" => a.allow_non_git = true,
            "--run" => a.run = Some(val()?.into()),
            "--verify" => a.verify = verify::Mode::Command(val()?),
            "--no-verify" => a.verify = verify::Mode::Off,
            "--bash-cap" => a.bash_cap = val()?.parse()?,
            "--diagnostics" => a.diagnostics = true,
            "--no-diagnostics" => a.diagnostics = false,
            "--self-review" => a.self_review = true,
            "--review-model" => a.review_model = Some(val()?),
            "--review-effort" => a.review_effort = val()?,
            "--no-project-instructions" => a.project_instructions = false,
            "--profile" => {
                a.profile = Some(match val()?.as_str() {
                    "repo" => route::Profile::Repo,
                    "lean" => route::Profile::Lean,
                    "repo-lean" => route::Profile::RepoLean,
                    p => bail!("--profile repo|repo-lean|lean, not {p}"),
                })
            }
            "--trust" => a.trust = true,
            "--user-settings" => a.user_settings = true,
            "--verdict" => a.verdict = Some(val()?),
            "--note" => a.note = val()?,
            "--run-id" | "--id" => a.run_id = Some(val()?),
            "--label-git" => a.label_git = true,
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
    if std::env::args().nth(1).as_deref() == Some("diag") {
        return diag::hook(std::path::Path::new(&std::env::args().nth(2).unwrap_or_default()));
    }
    let a = args()?;
    let base = labels::dir()?;
    if let Some(v) = &a.verdict {
        let id = labels::verdict(&base, v, &a.note, a.run_id.as_deref())?;
        println!("labelled run {id}: {v}");
        return Ok(());
    }
    if a.label_git {
        let (kept, discarded) = labels::label_git(&base, None)?;
        println!("labelled from git: {kept} kept, {discarded} discarded");
        return Ok(());
    }
    if let Some(n) = a.log {
        return runlog::print_recent(&base, n);
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
            // label this repo's older runs from git while we are here (cheap, read-only git)
            let _ = labels::label_git(&base, Some(&std::env::current_dir()?));
            let mut live = live::Live::new("", "")?;
            live.user_settings = a.user_settings;
            let route = live.set_lane(lane(&a, true)?);
            live.verify = a.verify.clone();
            live.bash_cap = a.bash_cap;
            live.diagnostics = a.diagnostics;
            live.self_review = a.self_review;
            live.review_model = a.review_model.clone().map(|m| (m, a.review_effort.clone()));
            live.project_instructions = a.project_instructions;
            let mut term = ratatui::init();
            execute!(std::io::stdout(), EnableBracketedPaste, EnableMouseCapture)?;
            // terminals with the kitty keyboard protocol report Shift-Enter apart from Enter; Ctrl-J works everywhere
            let enhanced = supports_keyboard_enhancement().unwrap_or(false);
            if enhanced {
                execute!(std::io::stdout(), PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES))?;
            }
            let r = run_live(&mut term, &mut live, &a, route);
            if enhanced {
                let _ = execute!(std::io::stdout(), PopKeyboardEnhancementFlags);
            }
            let _ = execute!(std::io::stdout(), DisableBracketedPaste, DisableMouseCapture);
            ratatui::restore();
            println!("session saved: {}", live.rec_path.display());
            r
        }
    }
}

/// The router's lane, unless --model/--effort pin it; then the profile, and trust for repos whose config runs code.
/// `interactive`: ask about trust on the terminal (live mode); headless needs --trust.
fn lane(a: &Args, interactive: bool) -> Result<route::Lane> {
    let cwd = std::env::current_dir()?;
    let mut lane = match (&a.model, &a.effort) {
        (None, None) => route::choose(&cwd),
        (m, e) => route::manual(&cwd, m.as_deref().unwrap_or("claude-opus-5-5"), e.as_deref().unwrap_or("medium")),
    };
    if let Some(p) = a.profile {
        lane.profile = p;
        lane.reason.push_str(&format!("; --profile {}", p.name()));
    }
    let config = route::RepoConfig::read(&cwd);
    if matches!(lane.profile, route::Profile::Repo | route::Profile::RepoLean) && config.runs_code() {
        let base = labels::dir()?;
        let trusted = trust::is_trusted(&base, &cwd) || a.trust || (interactive && trust::ask(&cwd, &config.summary()));
        if trusted {
            trust::trust(&base, &cwd)?;
        } else {
            // not trusted: nothing from the repo runs; its CLAUDE.md still reaches the lean prompt
            lane.profile = route::Profile::Lean;
            lane.reason.push_str("; repo not trusted, so lean (its hooks and skills stay off)");
        }
    }
    Ok(lane)
}

/// Runs one task without a UI and prints claude's result event, the same JSON `claude -p --output-format json` prints.
fn headless(prompt_file: &PathBuf, a: &Args) -> Result<()> {
    let prompt = std::fs::read_to_string(prompt_file).with_context(|| format!("reading {}", prompt_file.display()))?;
    let mut live = live::Live::new("", "")?;
    live.user_settings = a.user_settings;
    live.set_lane(lane(&a, false)?);
    live.mode = "headless";
    live.verify = a.verify.clone();
    live.bash_cap = a.bash_cap;
    live.diagnostics = a.diagnostics;
    live.self_review = a.self_review;
    live.review_model = a.review_model.clone().map(|m| (m, a.review_effort.clone()));
    live.project_instructions = a.project_instructions;
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
        r["tazuna"] = json!({"verify": log["verify"], "review": log["review"], "stop": log["stop"], "cost_usd": log["cost_usd"]});
        // every round and process of the run, the reviewer included (claude's own figure covers one process)
        r["total_cost_usd"] = log["cost_usd"].clone();
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

fn run_live(term: &mut ratatui::DefaultTerminal, live: &mut live::Live, a: &Args, route: (f64, serde_json::Value)) -> Result<()> {
    let mut s = Session::new("", &live.effort);
    s.cwd = std::env::current_dir()?.to_string_lossy().into_owned();
    s.apply(&route.1, route.0);
    let mut v = view(a.design, true, 1.0);
    let mut fx = Fx::new();
    let mut quit_at: Option<Instant> = None;
    let mut last = Instant::now();
    // mouse capture scrolls the transcript but keeps the terminal from selecting text; ctrl-s hands the mouse back
    let mut mouse = true;
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
            KeyCode::Char('s') if ctrl => {
                mouse = !mouse;
                let text = if mouse {
                    let _ = execute!(std::io::stdout(), EnableMouseCapture);
                    "mouse scrolling on"
                } else {
                    let _ = execute!(std::io::stdout(), DisableMouseCapture);
                    "select mode: drag to select and copy text; ctrl-s again for mouse scrolling"
                };
                s.apply(&json!({"type": "harness_info", "text": text}), live.elapsed_ms());
            }
            KeyCode::Char('y') if ctrl => {
                let text = match copy_last_reply(&s) {
                    Ok(n) => format!("copied the last reply ({n} chars)"),
                    Err(e) => format!("copy failed: {e:#}"),
                };
                s.apply(&json!({"type": "harness_info", "text": text}), live.elapsed_ms());
            }
            KeyCode::Enter if key.modifiers.intersects(KeyModifiers::ALT | KeyModifiers::SHIFT) => v.input.push('\n'),
            KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => v.input.push('\n'),
            KeyCode::Enter => {
                let text = v.input.trim().to_string();
                if text.is_empty() {
                    continue;
                }
                if let Some(rest) = text.strip_prefix("/good").or_else(|| text.strip_prefix("/bad")) {
                    // rate the last run locally; never sent to claude
                    let verdict = if text.starts_with("/good") { "good" } else { "bad" };
                    let msg = match &live.last_run {
                        Some(id) => labels::dir()
                            .and_then(|b| labels::add(&b, id, verdict, rest.trim(), "user"))
                            .map_or_else(|e| format!("could not save the label: {e:#}"), |_| format!("rated the last run {verdict}")),
                        None => "no finished run to rate yet".into(),
                    };
                    s.apply(&json!({"type": "harness_info", "text": msg}), live.elapsed_ms());
                    v.input.clear();
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
