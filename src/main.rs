//! tazuna: a lean terminal UI around Claude Code.
//!
//!   tazuna [--model M] [--effort E] [--design 1-4] [--allow-non-git]   # live: drive claude in the current repo
//!          (without --model/--effort the router picks: tiny repos Sonnet medium, escalating to Opus high if tests
//!          fail; otherwise Opus medium)
//!          [--verify "<cmd>" | --no-verify]   # after claude says done, tazuna runs the tests (auto-detected by default)
//!          [--bash-cap N]   # shrink bash output longer than N chars (default 8000; 0 = off)
//!          [--no-diagnostics]  # do not tell claude about type/compile errors right after its edits
//!          [--self-review]  # finish rules in the prompt, and one self-review turn before verification
//!          [-c | --continue | --resume <session-id>]   # pick up this repo's latest (or a given) session
//!          [--explorer <model> | --no-explorer]   # a subagent on <model> (default haiku) reads and searches code for the main model
//!          [--advisor <model>]   # Claude Code's advisor: the main model can consult <model> when stuck
//!          [--boss <model>]   # the main model only directs: explorer, implementer and tester subagents on <model> do
//!                         # all reading, editing and test runs (a hook blocks those tools for the main model)
//!          [--tester <model>]   # a subagent on <model> runs tests and type checks and reports only the failures
//!          [--repo-map]   # a map of the files that match the task, in front of the first message
//!          [--cache-ttl 5m|1h]   # prompt cache lifetime (default: 5m for --run, 1h live)
//!          [--mode auto|ask|edits|plan]   # permissions: never ask (default), ask before edits and commands, ask before
//!                                         # commands only, or plan read-only first; shift-tab cycles them live
//!          [--review-model M [--review-effort E]]  # an independent reviewer on model M checks the diff once
//!          [--escalate MODEL[:EFFORT]]   # when tests still fail, the fix rounds run on MODEL (with --provider: the cheap
//!                                       # model's work is handed to MODEL on claude)
//!          [--preset default|boss|max|cascade|NAME]   # a bundle of model/effort per role; add your own in
//!                                                     # ~/.config/tazuna/presets ("name: --flag value ...")
//!          [--data trusted|any]   # trusted (default): --provider only for hosts in ~/.config/tazuna/trusted-providers;
//!                                 # any: also providers that may store or use the code
//!          [--provider URL [--key-env VAR]]   # tazuna's own loop on an OpenAI-compatible API instead of claude; --model
//!                                             # is the provider's model id; key from VAR (default OPENAI_API_KEY);
//!                                             # a task it fails goes to Opus 5.5 medium on claude (the cascade)
//!          [--no-escalate]   # with --provider: no hand-over, the own loop's result stands
//!          [--max-minutes N]   # own loop: stop (or with --escalate, hand over) after N minutes per task; default: no
//!                              # time limit, only the $1 cap per task (TAZUNA_MAX_USD; cheap models are slow but
//!                              # cost cents)
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
//!   tazuna engine   # tazuna's own agent loop (--provider starts it; not for direct use)
//!
//! Designs: 1 aurora · 2 cockpit (default) · 3 zen · 4 timeline. Live sessions are saved to
//! ~/.local/share/tazuna/sessions/ and replay like any recording.

mod diag;
mod editor;
mod engine;
mod labels;
mod live;
mod mcp;
mod outcap;
mod replay;
mod repomap;
mod review;
mod route;
mod runlog;
mod select;
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
            KeyModifiers, KeyboardEnhancementFlags, MouseButton, MouseEventKind, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
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
    /// Own loop: base URL and the environment variable with the key.
    provider: Option<(String, String)>,
    /// `Some(None)`: this repo's latest session; `Some(Some(id))`: that session.
    resume: Option<Option<String>>,
    /// Starting permission mode, in claude's names.
    mode: String,
    cache_ttl: Option<String>,
    repo_map: bool,
    explorer: Option<String>,
    tester: Option<String>,
    boss: Option<String>,
    advisor: Option<String>,
    escalate: Option<(String, String)>,
    no_escalate: bool,
    /// `--data trusted|any`: with `trusted` (default) only providers on the trusted list may see the code.
    data_any: bool,
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

/// Built-in presets: a model and effort per role, bundled. `~/.config/tazuna/presets` adds or overrides them, one per
/// line as `name: --flag value ...`. Flags given on the command line win over the preset's.
const PRESETS: [(&str, &str); 4] = [
    ("default", ""),
    ("boss", "--boss haiku"),
    ("max", "--model claude-opus-5-5 --effort max"),
    ("cascade", "--data any --provider https://api.cheaperinference.com/v1 --key-env CHEAPER_INFERENCE_API_KEY --model deepseek-v4.1-flash --effort max"),
];

/// Replaces `--preset NAME` with the preset's flags, placed first so the user's own flags override them.
/// The cascade's strong model: where a task goes that the own loop could not finish.
const CASCADE_TO: (&str, &str) = ("claude-opus-5-5", "medium");

/// The own loop hands a failed task to Opus on claude by default (the cascade: 13 of 14 October runs solved at about a
/// fifth of tazuna-on-Opus's cost); `--no-escalate` keeps the own loop's result as it is.
fn escalation(provider: bool, escalate: Option<(String, String)>, no_escalate: bool) -> Option<(String, String)> {
    if no_escalate {
        return None;
    }
    escalate.or_else(|| provider.then(|| (CASCADE_TO.0.to_string(), CASCADE_TO.1.to_string())))
}

fn with_preset(args: Vec<String>) -> Result<Vec<String>> {
    let Some(i) = args.iter().position(|a| a == "--preset") else { return Ok(args) };
    let name = args.get(i + 1).context("--preset needs a name")?.clone();
    let file = std::path::Path::new(&std::env::var("HOME").unwrap_or_default()).join(".config/tazuna/presets");
    let user = std::fs::read_to_string(&file).unwrap_or_default();
    let flags = user
        .lines()
        .filter_map(|l| l.split_once(':'))
        .find(|(n, _)| n.trim() == name)
        .map(|(_, f)| f.to_string())
        .or_else(|| PRESETS.iter().find(|(n, _)| *n == name).map(|(_, f)| f.to_string()))
        .with_context(|| format!("no preset {name} (built in: default, boss, max, cascade; yours: {})", file.display()))?;
    let mut out: Vec<String> = flags.split_whitespace().map(String::from).collect();
    out.extend(args.into_iter().enumerate().filter(|(j, _)| *j != i && *j != i + 1).map(|(_, a)| a));
    Ok(out)
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
        provider: None,
        resume: None,
        mode: "bypassPermissions".into(),
        cache_ttl: None,
        repo_map: false,
        explorer: Some("haiku".into()),
        tester: None,
        boss: None,
        advisor: None,
        escalate: None,
        no_escalate: false,
        data_any: false,
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
    let mut it = with_preset(std::env::args().skip(1).collect())?.into_iter();
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
            "--provider" => a.provider = Some((val()?, a.provider.take().map_or("OPENAI_API_KEY".into(), |p| p.1))),
            "--key-env" => a.provider = Some((a.provider.take().map_or(String::new(), |p| p.0), val()?)),
            "-c" | "--continue" => a.resume = Some(None),
            "--resume" => a.resume = Some(Some(val()?)),
            "--repo-map" => a.repo_map = true,
            "--explorer" => a.explorer = Some(val()?),
            "--no-explorer" => a.explorer = None,
            "--tester" => a.tester = Some(val()?),
            "--boss" => a.boss = Some(val()?),
            "--advisor" => a.advisor = Some(val()?),
            "--data" => {
                a.data_any = match val()?.as_str() {
                    "any" => true,
                    "trusted" => false,
                    v => bail!("--data takes trusted or any, not {v}"),
                }
            }
            "--max-minutes" => {
                let n = val()?;
                n.parse::<u64>().with_context(|| format!("--max-minutes takes a number, not {n}"))?;
                // SAFETY: flags are parsed before any thread starts; the own loop (a child process) reads it
                unsafe { std::env::set_var("TAZUNA_MAX_MINUTES", n) };
            }
            "--no-escalate" => a.no_escalate = true,
            "--escalate" => {
                let v = val()?;
                let (m, e) = v.split_once(':').unwrap_or((&v, "medium"));
                a.escalate = Some((m.to_string(), e.to_string()));
            }
            "--cache-ttl" => {
                a.cache_ttl = Some(match val()?.as_str() {
                    t @ ("5m" | "1h") => t.to_string(),
                    t => bail!("--cache-ttl 5m|1h, not {t}"),
                })
            }
            "--mode" => {
                a.mode = match val()?.as_str() {
                    "auto" => "bypassPermissions",
                    "ask" => "default",
                    "edits" => "acceptEdits",
                    "plan" => "plan",
                    m => bail!("--mode auto|ask|edits|plan, not {m}"),
                }
                .into()
            }
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
    a.escalate = escalation(a.provider.is_some(), a.escalate.take(), a.no_escalate);
    Ok(a)
}

fn main() -> Result<()> {
    if std::env::args().nth(1).as_deref() == Some("mcp") {
        return mcp::serve();
    }
    if std::env::args().nth(1).as_deref() == Some("boss-hook") {
        return live::boss_hook();
    }
    if std::env::args().nth(1).as_deref() == Some("engine") {
        return engine::serve(&std::env::args().skip(2).collect::<Vec<_>>());
    }
    if std::env::args().nth(1).as_deref() == Some("map") {
        // `tazuna map <task-file>`: the repo map --repo-map would send, for checking it by eye
        let task = std::fs::read_to_string(std::env::args().nth(2).context("tazuna map <task-file>")?)?;
        println!("{}", repomap::build(&std::env::current_dir()?, &task).unwrap_or_else(|| "(no match)".into()));
        return Ok(());
    }
    if std::env::args().nth(1).as_deref() == Some("diag") {
        return diag::hook(std::path::Path::new(&std::env::args().nth(2).unwrap_or_default()));
    }
    let a = args()?;
    if let Some((url, _)) = &a.provider {
        check_data(url, a.data_any)?;
    }
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
            live.provider = a.provider.clone();
            live.project_instructions = a.project_instructions;
            live.cache_ttl = a.cache_ttl.clone();
            live.repo_map = a.repo_map;
            live.explorer = a.explorer.clone();
            live.tester = a.tester.clone();
            live.boss = a.boss.clone();
            live.advisor = a.advisor.clone();
            if let Some((m, e)) = a.escalate.clone() {
                live.set_escalate(m, e);
            }
            if live.boss.is_some() {
                live.profile = route::Profile::Repo; // subagents and the blocking hook need Claude Code's own tools
            }
            live.permission_mode = a.mode.clone();
            let earlier = match &a.resume {
                Some(id) => {
                    let (sid, rec) = previous_session(&base, &std::env::current_dir()?, id.as_deref())
                        .context("no earlier tazuna session in this repo to continue")?;
                    live.resume(&sid);
                    rec
                }
                None => None,
            };
            let mut term = ratatui::init();
            execute!(std::io::stdout(), EnableBracketedPaste, EnableMouseCapture)?;
            // terminals with the kitty keyboard protocol report Shift-Enter apart from Enter; Ctrl-J works everywhere
            let enhanced = supports_keyboard_enhancement().unwrap_or(false);
            if enhanced {
                execute!(std::io::stdout(), PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES))?;
            }
            let r = run_live(&mut term, &mut live, &a, route, earlier);
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
/// Providers allowed under `--data trusted`, one host per line in ~/.config/tazuna/trusted-providers (Anthropic through
/// the claude CLI is always allowed). Providers that may store or use the code need `--data any` or an entry there.
fn check_data(url: &str, any: bool) -> Result<()> {
    if any {
        return Ok(());
    }
    let host = host_of(url);
    let file = std::path::Path::new(&std::env::var("HOME").unwrap_or_default()).join(".config/tazuna/trusted-providers");
    let listed = std::fs::read_to_string(&file).unwrap_or_default().lines().any(|l| l.trim() == host);
    if !listed {
        bail!("{host} is not on the trusted list ({}): it may store or use your code. Add it there, or run with --data any", file.display());
    }
    Ok(())
}

fn host_of(url: &str) -> String {
    url.split("://").nth(1).unwrap_or(url).split(['/', ':']).next().unwrap_or_default().to_string()
}

fn headless(prompt_file: &PathBuf, a: &Args) -> Result<()> {
    let prompt = std::fs::read_to_string(prompt_file).with_context(|| format!("reading {}", prompt_file.display()))?;
    if a.mode != "bypassPermissions" {
        eprintln!("--mode does not apply to --run: nobody is there to answer questions, so headless runs never ask");
    }
    let mut live = live::Live::new("", "")?;
    live.user_settings = a.user_settings;
    live.set_lane(lane(&a, false)?);
    live.mode = "headless";
    live.verify = a.verify.clone();
    live.bash_cap = a.bash_cap;
    live.diagnostics = a.diagnostics;
    live.self_review = a.self_review;
    live.review_model = a.review_model.clone().map(|m| (m, a.review_effort.clone()));
    live.provider = a.provider.clone();
    live.project_instructions = a.project_instructions;
    live.cache_ttl = a.cache_ttl.clone();
    live.repo_map = a.repo_map;
    live.explorer = a.explorer.clone();
    live.tester = a.tester.clone();
    live.advisor = a.advisor.clone();
    live.boss = a.boss.clone();
    if let Some((m, e)) = a.escalate.clone() {
        live.set_escalate(m, e);
    }
    if live.boss.is_some() {
        live.profile = route::Profile::Repo; // subagents and the blocking hook need Claude Code's own tools
    }
    if let Some(id) = &a.resume {
        let (sid, _) = previous_session(&labels::dir()?, &std::env::current_dir()?, id.as_deref()).context("no earlier tazuna session in this repo to continue")?;
        live.resume(&sid);
    }
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
    // claude's last result, with usage, turns and cost summed over every run of the session: the task's run, plus any run
    // claude started on its own after a background command finished
    let mut r = live.last_result.clone().context("no result")?;
    if let Some(first) = live.logs.first() {
        let sum = |path: &[&str]| live.logs.iter().map(|l| path.iter().fold(l, |v, k| &v[*k]).as_f64().unwrap_or(0.0)).sum::<f64>();
        let tok = |k: &str| json!(sum(&["tokens", k]) as u64);
        r["usage"]["input_tokens"] = tok("input");
        r["usage"]["cache_creation_input_tokens"] = tok("cache_write");
        r["usage"]["cache_read_input_tokens"] = tok("cache_read");
        r["usage"]["output_tokens"] = tok("output");
        r["num_turns"] = json!(sum(&["turns"]) as u64);
        let cost = (sum(&["cost_usd"]) * 1e6).round() / 1e6;
        let last = live.logs.last().unwrap_or(first);
        r["tazuna"] = json!({"verify": first["verify"], "review": first["review"], "stop": last["stop"], "cost_usd": cost, "runs": live.logs.len()});
        // every round, process and run, the reviewer included (claude's own figure covers one process)
        r["total_cost_usd"] = json!(cost);
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

    /// Draws one frame; returns where the transcript is.
    fn frame(&mut self, f: &mut Frame, s: &Session, v: &View, dt: Duration) -> ratatui::layout::Rect {
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
        areas.transcript
    }
}

fn view(design: Design, live: bool, speed: f64) -> View {
    View { design, expand_all: false, scroll: 0, clock_ms: 0.0, speed, paused: false, live, input: String::new(), cursor: 0, asking: None, mode: "auto".into(), hints: Vec::new(), bg_running: 0, tasks: None, rewind: None, vim: "" }
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

/// A permission question ignores keys for this long after it appears.
const ANSWER_DELAY_MS: u64 = 600;
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
        term.draw(|f| {
            fx.frame(f, &s, &v, dt);
        })?;

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
    let text = s.blocks.iter().rev().find_map(|b| if let session::Block::Text { text, .. } = b { Some(text.clone()) } else { None }).context("no reply yet")?;
    to_clipboard(&text)?;
    Ok(text.chars().count())
}

fn to_clipboard(text: &str) -> Result<()> {
    use std::io::Write;
    let mut child = std::process::Command::new("pbcopy").stdin(std::process::Stdio::piped()).spawn().context("pbcopy")?;
    child.stdin.take().context("pbcopy stdin")?.write_all(text.as_bytes())?;
    child.wait()?;
    Ok(())
}

fn busy(s: &Session) -> bool {
    !matches!(s.activity, session::Activity::Ready | session::Activity::Done)
}

/// Saves the clipboard's image (macOS) as a PNG in a private temp folder.
fn paste_image() -> Result<std::path::PathBuf> {
    let dir = std::env::temp_dir().join(format!("tazuna-images-{}", std::process::id()));
    std::os::unix::fs::DirBuilderExt::mode(&mut std::fs::DirBuilder::new(), 0o700).recursive(true).create(&dir)?;
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos();
    let path = dir.join(format!("paste-{nanos}.png"));
    let script = format!(
        "set f to (open for access POSIX file \"{}\" with write permission)\ntry\nwrite (the clipboard as «class PNGf») to f\nend try\nclose access f",
        path.display()
    );
    std::process::Command::new("osascript").args(["-e", &script]).output()?;
    match std::fs::metadata(&path) {
        Ok(m) if m.len() > 0 => Ok(path),
        _ => {
            let _ = std::fs::remove_file(&path);
            bail!("the clipboard holds no image")
        }
    }
}

/// "Write: /repo/b.txt  (always = this tool)": what a permission request asks, made safe to show.
fn question(req: &serde_json::Value) -> String {
    let r = &req["request"];
    let tool = session::shown(r["tool_name"].as_str().unwrap_or_default(), 60);
    let i = &r["input"];
    let what = match ["command", "file_path", "path", "pattern", "url"].iter().find_map(|k| i[*k].as_str()) {
        _ if tool == "ExitPlanMode" => "approve this plan and start editing".to_string(),
        Some(w) => session::shown(w, 400),
        None => session::shown(&i.to_string(), 400),
    };
    let scope = if matches!(tool.as_str(), "Bash" | "mcp__lean__bash" | "PowerShell") { "this command" } else { "this tool" };
    format!("{tool}: {what}  (always = {scope})")
}

/// The latest session this repo ran (or the one whose id starts with `id`): claude's session id and tazuna's recording.
fn previous_session(base: &std::path::Path, cwd: &std::path::Path, id: Option<&str>) -> Option<(String, Option<std::path::PathBuf>)> {
    let cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
    let log = std::fs::read_to_string(base.join("runs.jsonl")).ok()?;
    log.lines().rev().filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok()).find_map(|r| {
        let m = &r["meta"];
        let sid = m["session_id"].as_str()?;
        let same_repo = m["cwd"].as_str().map(std::path::PathBuf::from).is_some_and(|c| c.canonicalize().unwrap_or(c) == cwd);
        let wanted = id.map_or(same_repo, |id| sid.starts_with(id));
        wanted.then(|| (sid.to_string(), m["recording"].as_str().map(std::path::PathBuf::from)))
    })
}

/// Opens `text` in `editor` (a shell command such as `vim` or `code --wait`; the file is its last argument) and returns
/// what was saved, without the final newline editors add. A failing editor leaves the message as it was.
fn edit_in(editor: &str, text: &str) -> Result<String> {
    // created exclusively and private: in a shared /tmp a planted file or symlink at a guessable name could otherwise
    // redirect the write or expose the message
    let (path, mut file) = (0..16)
        .find_map(|i| {
            let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos();
            let path = std::env::temp_dir().join(format!("tazuna-message-{}-{nanos:x}-{i}.md", std::process::id()));
            use std::os::unix::fs::OpenOptionsExt;
            std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&path).ok().map(|f| (path, f))
        })
        .context("could not create a private temporary file")?;
    std::io::Write::write_all(&mut file, text.as_bytes())?;
    drop(file);
    let status = std::process::Command::new("sh").arg("-c").arg(format!("{editor} \"$1\"")).arg("sh").arg(&path).status();
    let saved = std::fs::read_to_string(&path);
    let _ = std::fs::remove_file(&path);
    match status {
        Ok(s) if s.success() => {
            let saved = saved?;
            Ok(saved.strip_suffix('\n').unwrap_or(&saved).to_string())
        }
        Ok(s) => bail!("`{editor}` exited with {s}"),
        Err(e) => bail!("could not start `{editor}`: {e}"),
    }
}

fn run_live(
    term: &mut ratatui::DefaultTerminal,
    live: &mut live::Live,
    a: &Args,
    route: (f64, serde_json::Value),
    earlier: Option<std::path::PathBuf>,
) -> Result<()> {
    let mut s = Session::new("", &live.effort);
    // a continued session shows what came before (claude has it; the screen should too)
    if let Some(mut r) = earlier.and_then(|p| Replay::load(&p, false).ok()) {
        for (t, e) in r.take(usize::MAX) {
            s.apply(e, *t);
        }
        // a notice also closes the old process's cost: the resumed process reports from $0
        s.apply(&json!({"type": "harness_notice", "text": "── continued from here ──", "error": false}), 0.0);
    }
    s.cwd = std::env::current_dir()?.to_string_lossy().into_owned();
    s.apply(&route.1, route.0);
    s.mode = session::mode_name(&live.permission_mode).to_string();
    let mut v = view(a.design, true, 1.0);
    let mut fx = Fx::new();
    let mut quit_at: Option<Instant> = None;
    let mut last = Instant::now();
    // mouse capture scrolls the transcript but keeps the terminal from selecting text; ctrl-s hands the mouse back
    let mut mouse = true;
    let mut ed = editor::Editor::with_history(labels::dir()?.join("history.jsonl"));
    // when the current permission question first got a key
    let mut asked_at: Option<Instant> = None;
    // mouse selection in the transcript: drag selects, release copies
    let mut transcript = ratatui::layout::Rect::default();
    let mut sel: Option<select::Selection> = None;
    let (mut copy_now, mut copied) = (false, None::<String>);
    // ctrl-b: the background-task view (which row is selected, when its output was last asked for)
    let (mut tasks_open, mut task_sel, mut outputs_read) = (false, 0usize, Instant::now());
    // /rewind: the picker's selected message, while it is open
    let mut rewind_sel: Option<usize> = None;
    // /vim: modal editing in the composer (remembered across sessions); `normal` and a pending operator (d, c)
    let vim_flag = labels::dir().map(|d| d.join("vim")).ok();
    let mut vim = vim_flag.as_ref().is_some_and(|f| f.exists());
    let (mut normal, mut vim_op) = (false, None::<char>);
    // `@` completion: the repo's tracked files (ponytail: read once per session; new files show up next session)
    let files: Vec<String> = std::process::Command::new("git")
        .args(["ls-files"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().take(50_000).map(String::from).collect())
        .unwrap_or_default();
    loop {
        if live.pending.is_empty() {
            asked_at = None;
        } else if asked_at.is_none() {
            asked_at = Some(Instant::now());
        }
        if v.input != ed.text || v.cursor != ed.cursor {
            let (start, token) = ed.token();
            v.hints = editor::suggest(token, start == 0, &s.commands, &files);
        }
        (v.input, v.cursor) = (ed.text.clone(), ed.cursor);
        // the question on screen is the one an answer applies to: the front of live's queue
        v.asking = live.pending.front().map(|req| question(req));
        v.mode = s.mode.clone();
        v.bg_running = live.bg.iter().filter(|t| t.ended.is_none()).count();
        if tasks_open && outputs_read.elapsed() > Duration::from_secs(1) {
            live.read_task_outputs();
            outputs_read = Instant::now();
        }
        task_sel = task_sel.min(live.bg.len().saturating_sub(1));
        v.tasks = tasks_open.then(|| ui::TasksPanel {
            selected: task_sel,
            rows: live.bg.iter().map(|t| ui::TaskRow {
                kind: t.kind.clone(),
                description: t.description.clone(),
                status: t.status.clone(),
                secs: t.ended.unwrap_or_else(Instant::now).duration_since(t.started).as_secs(),
                output: t.output.clone(),
            }).collect(),
        });
        v.vim = if !vim { "" } else if normal { "NORMAL" } else { "INSERT" };
        v.rewind = rewind_sel.map(|i| ui::RewindPanel { items: live.checkpoints.iter().map(|c| c.text.clone()).collect(), selected: i });
        let dt = last.elapsed();
        last = Instant::now();
        for (t, e) in live.poll() {
            s.apply(&e, t);
        }
        v.clock_ms = live.elapsed_ms();
        term.draw(|f| {
            transcript = fx.frame(f, &s, &v, dt);
            if let Some(sel) = &sel {
                sel.highlight(f.buffer_mut());
                if copy_now {
                    copied = Some(sel.text(f.buffer_mut()));
                }
            }
        })?;
        if let Some(text) = copied.take() {
            copy_now = false;
            let msg = match to_clipboard(&text) {
                Ok(()) => format!("copied {} chars", text.chars().count()),
                Err(e) => format!("copy failed: {e:#}"),
            };
            s.apply(&json!({"type": "harness_info", "text": msg}), live.elapsed_ms());
        }

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
                ed.insert(&text.replace("\r\n", "\n"));
                continue;
            }
            Event::Mouse(m) => {
                match m.kind {
                    MouseEventKind::Down(MouseButton::Left) => sel = select::Selection::start(m.column, m.row, transcript),
                    MouseEventKind::Drag(MouseButton::Left) => {
                        if let Some(sel) = sel.as_mut() {
                            sel.drag(m.column, m.row);
                        }
                    }
                    MouseEventKind::Up(MouseButton::Left) => match &sel {
                        Some(x) if !x.is_empty() => copy_now = true,
                        _ => sel = None,
                    },
                    k => scroll_mouse(&mut v, k),
                }
                continue;
            }
            Event::Key(k) if k.kind == KeyEventKind::Press => k,
            _ => continue,
        };
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        sel = None;
        // an open permission question owns the keyboard: only y(es), a(lways), n(o)/esc answer it, every other key is
        // ignored, and keys in the first moment after it appears count for nothing (typing must not approve by accident)
        // ctrl-c and ctrl-d still stop or quit: stopping answers every open question with no
        let stop_key = ctrl && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('d'));
        // the /rewind picker owns the keyboard while open
        if let (Some(i), false) = (rewind_sel, stop_key) {
            let last = live.checkpoints.len().saturating_sub(1);
            let choice = match key.code {
                KeyCode::Up => {
                    rewind_sel = Some(i.saturating_sub(1));
                    None
                }
                KeyCode::Down => {
                    rewind_sel = Some((i + 1).min(last));
                    None
                }
                KeyCode::Enter => Some((true, true)),
                KeyCode::Char('f') => Some((true, false)),
                KeyCode::Char('c') => Some((false, true)),
                KeyCode::Esc => {
                    rewind_sel = None;
                    None
                }
                _ => None,
            };
            if let Some((files, conversation)) = choice {
                rewind_sel = None;
                let text = live.checkpoints.get(i).map(|c| c.text.clone()).unwrap_or_default();
                match live.rewind(i, files, conversation) {
                    Ok((t, e)) => {
                        s.apply(&e, t);
                        if conversation {
                            ed.set(text); // the message comes back to edit and send again
                        }
                    }
                    Err(e) => s.apply(&json!({"type": "harness_info", "text": format!("rewind: {e:#}")}), live.elapsed_ms()),
                }
            }
            continue;
        }
        // the background-task view owns the keyboard while open: select, stop, close
        if tasks_open && !stop_key && live.pending.is_empty() {
            match key.code {
                KeyCode::Up => task_sel = task_sel.saturating_sub(1),
                KeyCode::Down => task_sel = (task_sel + 1).min(live.bg.len().saturating_sub(1)),
                KeyCode::Char('x') => {
                    let id = live.bg.get(task_sel).filter(|t| t.ended.is_none()).map(|t| t.id.clone());
                    if let Some((t, e)) = id.and_then(|id| live.stop_task(&id)) {
                        s.apply(&e, t);
                    }
                }
                KeyCode::Esc => tasks_open = false,
                KeyCode::Char('b') if ctrl => tasks_open = false,
                _ => {}
            }
            continue;
        }
        if !live.pending.is_empty() && !stop_key {
            let shown_for = asked_at.get_or_insert_with(Instant::now).elapsed();
            let answer = match key.code {
                _ if shown_for < Duration::from_millis(ANSWER_DELAY_MS) => None,
                KeyCode::Char('y') => Some((true, false)),
                KeyCode::Char('a') => Some((true, true)),
                KeyCode::Char('n') | KeyCode::Esc => Some((false, false)),
                _ => None,
            };
            if let Some((allow, always)) = answer {
                asked_at = None;
                if let Some((t, e)) = live.answer(allow, always) {
                    s.apply(&e, t);
                }
            }
            continue;
        }
        asked_at = None;
        // vim mode: esc leaves insert mode; normal mode takes plain keys (enter still sends, ctrl keys work as always)
        if vim && !ctrl && !alt {
            if !normal && key.code == KeyCode::Esc {
                (normal, vim_op) = (true, None);
                ed.left();
                continue;
            }
            if normal {
                match key.code {
                    KeyCode::Char(c) => {
                        if ed.vim_normal(c, &mut vim_op) {
                            normal = false;
                        }
                        continue;
                    }
                    // esc in normal mode: stop the run when nothing is typed (never clears the text)
                    KeyCode::Esc => {
                        if ed.text.is_empty() {
                            if let Some((t, e)) = busy(&s).then(|| live.interrupt()).flatten() {
                                s.apply(&e, t);
                            }
                        }
                        continue;
                    }
                    _ => {}
                }
            }
        }
        match key.code {
            KeyCode::BackTab => {
                let next = match live.permission_mode.as_str() {
                    "bypassPermissions" => "default",
                    "default" => "acceptEdits",
                    "acceptEdits" if live.provider.is_none() => "plan", // the own loop has no plan mode
                    _ => "bypassPermissions",
                };
                let (t, e) = live.set_mode(next);
                s.apply(&e, t);
            }
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
            // ctrl-o like Claude Code's detail toggle; ctrl-e is end of line
            KeyCode::Char('o') if ctrl => v.expand_all = !v.expand_all,
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
            KeyCode::Char('x') if ctrl => {
                let msg = ed.toggle_stash();
                s.apply(&json!({"type": "harness_info", "text": msg}), live.elapsed_ms());
            }
            KeyCode::Char('b') if ctrl => {
                (tasks_open, task_sel) = (true, live.bg.iter().rposition(|t| t.ended.is_none()).unwrap_or(live.bg.len().saturating_sub(1)));
                live.read_task_outputs();
                outputs_read = Instant::now();
            }
            // ctrl-g: the message so far in $VISUAL / $EDITOR (vi without either); what is saved comes back here
            KeyCode::Char('g') if ctrl => {
                let editor = std::env::var("VISUAL").or_else(|_| std::env::var("EDITOR")).unwrap_or_else(|_| "vi".into());
                let _ = execute!(std::io::stdout(), PopKeyboardEnhancementFlags, DisableBracketedPaste, DisableMouseCapture);
                ratatui::restore();
                let edited = edit_in(&editor, &ed.text);
                *term = ratatui::init();
                let _ = execute!(std::io::stdout(), EnableBracketedPaste);
                if mouse {
                    let _ = execute!(std::io::stdout(), EnableMouseCapture);
                }
                if supports_keyboard_enhancement().unwrap_or(false) {
                    let _ = execute!(std::io::stdout(), PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES));
                }
                match edited {
                    Ok(text) => ed.set(text),
                    Err(e) => s.apply(&json!({"type": "harness_info", "text": format!("editor: {e:#} (the message is unchanged)")}), live.elapsed_ms()),
                }
            }
            // ctrl-v: an image on the clipboard becomes a file the message names (sent as an image with it)
            KeyCode::Char('v') if ctrl => match paste_image() {
                Ok(path) => ed.insert(&format!("@{} ", path.display())),
                Err(e) => s.apply(&json!({"type": "harness_info", "text": format!("no image to paste: {e:#}")}), live.elapsed_ms()),
            },
            KeyCode::Char('y') if ctrl => {
                let text = match copy_last_reply(&s) {
                    Ok(n) => format!("copied the last reply ({n} chars)"),
                    Err(e) => format!("copy failed: {e:#}"),
                };
                s.apply(&json!({"type": "harness_info", "text": text}), live.elapsed_ms());
            }
            KeyCode::Enter if key.modifiers.intersects(KeyModifiers::ALT | KeyModifiers::SHIFT) => ed.insert("\n"),
            KeyCode::Char('j') if ctrl => ed.insert("\n"),
            KeyCode::Char('a') if ctrl => ed.home(),
            KeyCode::Char('e') if ctrl => ed.end(),
            KeyCode::Char('u') if ctrl => ed.kill_to_start(),
            KeyCode::Char('k') if ctrl => ed.kill_to_end(),
            KeyCode::Char('w') if ctrl => ed.kill_word(),
            KeyCode::Backspace if alt => ed.kill_word(),
            // a backslash before enter means a new line, in any terminal (where shift-enter cannot be told apart)
            KeyCode::Enter if ed.text[..ed.cursor].ends_with('\\') => {
                ed.backspace();
                ed.insert("\n");
            }
            KeyCode::Enter => {
                if ed.text.trim().is_empty() {
                    continue;
                }
                let text = ed.take().trim().to_string();
                if text == "/vim" {
                    vim = !vim;
                    (normal, vim_op) = (false, None);
                    if let Some(f) = &vim_flag {
                        let _ = if vim { std::fs::write(f, "") } else { std::fs::remove_file(f) };
                    }
                    let msg = if vim { "vim mode on: esc for normal mode, i/a/o to type (/vim again turns it off)" } else { "vim mode off" };
                    s.apply(&json!({"type": "harness_info", "text": msg}), live.elapsed_ms());
                    continue;
                }
                if text == "/rewind" {
                    if live.checkpoints.is_empty() {
                        s.apply(&json!({"type": "harness_info", "text": "nothing to rewind yet: no message sent in this conversation"}), live.elapsed_ms());
                    } else {
                        rewind_sel = Some(live.checkpoints.len() - 1);
                    }
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
                    continue;
                }
                // `!cmd`: run it here, show the output, and hand both to claude (like Claude Code's shell mode)
                // only what the user typed may attach image files; command output never does
                let typed = !text.starts_with('!');
                let text = match text.strip_prefix('!') {
                    Some(cmd) => {
                        // ponytail: blocks the UI while it runs; fine for git status/ls, a thread when long commands matter
                        let (code, out, _) = verify::run_raw(&std::env::current_dir()?, cmd.trim());
                        let out: String = out.chars().take(8000).collect();
                        let code = code.map_or("timeout".into(), |c| c.to_string());
                        s.apply(&json!({"type": "harness_info", "text": format!("$ {} (exit {code})\n{out}", cmd.trim())}), live.elapsed_ms());
                        format!("I ran `{}` myself (exit {code}). Output:\n```\n{out}\n```", cmd.trim())
                    }
                    None => text,
                };
                normal = false; // the next message starts in insert mode
                match if typed { live.send_typed(&text) } else { live.send(&text) } {
                    Ok((t, e)) => s.apply(&e, t),
                    Err(err) => s.apply(&json!({"type": "harness_notice", "text": format!("{err:#}"), "error": true}), live.elapsed_ms()),
                }
                v.scroll = 0;
            }
            // tab takes the first suggestion; again, the next one
            KeyCode::Tab if !v.hints.is_empty() => {
                let (_, token) = ed.token();
                let token = token.trim_end().to_string();
                let next = v.hints.iter().position(|h| *h == token).map_or(0, |i| (i + 1) % v.hints.len());
                let pick = v.hints[next].clone();
                // files keep the list (the next tab cycles); a command is done, so a space follows
                ed.complete(&if pick.starts_with('/') { format!("{pick} ") } else { pick });
            }
            KeyCode::Backspace => ed.backspace(),
            KeyCode::Delete => ed.delete(),
            KeyCode::Left => ed.left(),
            KeyCode::Right => ed.right(),
            KeyCode::Home => ed.home(),
            // end of line while typing; with an empty composer, back to the newest output
            KeyCode::End if ed.text.is_empty() => v.scroll = 0,
            KeyCode::End => ed.end(),
            KeyCode::Up => ed.up(),
            KeyCode::Down => ed.down(),
            // esc: clear what is typed; with nothing typed, stop the run like ctrl-c
            KeyCode::Esc if ed.text.is_empty() => {
                if let Some((t, e)) = busy(&s).then(|| live.interrupt()).flatten() {
                    s.apply(&e, t);
                }
            }
            KeyCode::Esc => ed.clear(),
            KeyCode::PageUp => v.scroll += 15,
            KeyCode::PageDown => v.scroll = v.scroll.saturating_sub(15),
            KeyCode::Char(c) if !ctrl => ed.insert(c.encode_utf8(&mut [0; 4])),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn presets_expand_first_so_own_flags_win() {
        let v = |x: &[&str]| x.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let out = super::with_preset(v(&["--run", "t.txt", "--preset", "max", "--effort", "high"])).unwrap();
        assert_eq!(out, v(&["--model", "claude-opus-5-5", "--effort", "max", "--run", "t.txt", "--effort", "high"]));
        assert_eq!(super::with_preset(v(&["--run", "t.txt"])).unwrap(), v(&["--run", "t.txt"]));
        assert!(super::with_preset(v(&["--preset", "nope"])).is_err());
    }

    #[test]
    fn the_external_editor_round_trips_the_message() {
        assert_eq!(super::edit_in("printf 'edited\\n' >", "draft").unwrap(), "edited");
        assert_eq!(super::edit_in("true", "keep me").unwrap(), "keep me");
        assert!(super::edit_in("false", "x").is_err());
        // the file the editor gets is private to the user (macOS stat syntax, like the rest of the UI's helpers)
        let mode = super::edit_in("sh -c 'stat -f %Lp \"$0\" > \"$0.mode\"; cp \"$0.mode\" \"$0\"; rm \"$0.mode\"'", "x").unwrap();
        assert_eq!(mode, "600");
    }

    #[test]
    fn the_own_loop_escalates_unless_told_not_to() {
        let opus = Some(("claude-opus-5-5".to_string(), "medium".to_string()));
        let mine = Some(("claude-sonnet-5-5".to_string(), "high".to_string()));
        assert_eq!(super::escalation(true, None, false), opus);
        assert_eq!(super::escalation(true, mine.clone(), false), mine);
        assert_eq!(super::escalation(true, None, true), None);
        assert_eq!(super::escalation(false, None, false), None); // claude alone: no default hand-over
        assert_eq!(super::escalation(false, mine.clone(), false), mine);
    }

    #[test]
    fn provider_host_for_the_trusted_list() {
        assert_eq!(super::host_of("https://api.cheaperinference.com/v1"), "api.cheaperinference.com");
        assert_eq!(super::host_of("http://localhost:8080/v1"), "localhost");
        assert_eq!(super::host_of("api.example.com"), "api.example.com");
    }

    use super::*;

    #[test]
    fn finds_the_session_to_continue() {
        let base = std::env::temp_dir().join(format!("tazuna-resume-{}", std::process::id()));
        let repo = base.join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        let line = |sid: &str, cwd: &std::path::Path| json!({"meta": {"session_id": sid, "cwd": cwd, "recording": format!("/r/{sid}.jsonl")}}).to_string();
        let other = base.join("other");
        std::fs::write(base.join("runs.jsonl"), [line("aaa-1", &repo), line("bbb-2", &other), line("ccc-3", &repo), line("ddd-4", &other)].join("\n")).unwrap();
        assert_eq!(previous_session(&base, &repo, None).unwrap().0, "ccc-3", "this repo's latest");
        assert_eq!(previous_session(&base, &repo, Some("aaa")).unwrap().0, "aaa-1", "by id prefix");
        assert!(previous_session(&base, &base.join("none"), None).is_none());
        std::fs::remove_dir_all(&base).ok();
    }
}
