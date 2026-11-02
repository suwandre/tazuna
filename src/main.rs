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
//!          [--explorer MODEL@URL [--explorer-key-env VAR]]   # the reading goes to MODEL on an OpenAI-compatible API instead
//!                                                          # (tazuna's own loop, read-only; the code leaves the machine,
//!                                                          # so the host must be trusted or --data any)
//!          [--advisor <model>]   # Claude Code's advisor: the main model can consult <model> when stuck
//!          [--boss <model> | --no-boss]   # the main model only directs: explorer, implementer and tester subagents on
//!                         # <model> do all reading, editing and test runs (a hook blocks those tools for the main model);
//!                         # on by default (haiku) for --run on Opus on the claude path
//!          [--tester <model>]   # a subagent on <model> runs tests and type checks and reports only the failures
//!          [--repo-map]   # a map of the files that match the task, in front of the first message
//!          [--cache-ttl 5m|1h]   # prompt cache lifetime (default: 5m for --run, 1h live)
//!          [--mode auto|ask|edits|plan]   # permissions: never ask (default), ask before edits and commands, ask before
//!                                         # commands only, or plan read-only first; shift-tab cycles them live
//!          [--review-model M [--review-effort E]]  # an independent reviewer on model M checks the diff once
//!          [--review-key-env VAR]  # with M as MODEL@URL: the reviewer on that provider's API, its key in VAR
//!          [--review-only]   # with --run TASK-FILE and --review-model: review the uncommitted change here, verdict as JSON
//!          [--escalate MODEL[:EFFORT]]   # when tests still fail, the fix rounds run on MODEL (with --provider: the cheap
//!                                       # model's work is handed to MODEL on claude)
//!          [--preset default|boss|max|reviewed|NAME]   # a bundle of model/effort per role; add your own in
//!                                                     # ~/.config/tazuna/presets ("name: --flag value ...")
//!          [--data trusted|any]   # trusted (default): --provider only for hosts in ~/.config/tazuna/trusted-providers;
//!                                 # any: also providers that may store or use the code
//!          [--provider URL [--key-env VAR]]   # tazuna's own loop on an OpenAI-compatible API instead of claude; --model
//!                                             # is the provider's model id; key from VAR (default OPENAI_API_KEY);
//!                                             # a task it fails goes to Opus 5.5 medium on claude (the cascade)
//!          [--no-escalate]   # with --provider: no hand-over, the own loop's result stands
//!          [--pick]   # open the model picker (/model) before the first message
//!          [--repo-tools core|all]   # repo profile: Claude Code's coding tools only (core) or all of them (default)
//!          [--rtk]   # claude's Bash commands through rtk (shortened output; rtk must be installed)
//!          [--batch-calls]   # repo profile: claude told to make independent tool calls in one turn
//!          [--boss-effort LEVEL]   # boss mode's workers at LEVEL (default medium); /boss switches boss mode live
//!          [--price IN,CACHED,OUT]   # own loop: USD per million tokens, for providers whose model list has no prices
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
//!   tazuna --loop [FLAGS]   # build TODO.md's open items one by one (fresh headless run each, with FLAGS, e.g.
//!                           # --preset reviewed), commit each; in a session: /loop [PRESET|FLAGS], /loop stop
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
mod looprun;
mod mcp;
mod models;
mod outcap;
mod platform;
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
    /// `--repo-tools core`: the repo profile with Claude Code's coding tools only (Bash, Read, Edit, Write, Task, plan mode,
    /// web, Skill, AskUserQuestion): 29 tools to 11, 20.1k to 14.6k tokens a call (2026-11-02). Under measurement.
    repo_tools_core: bool,
    /// `--rtk`: claude's Bash commands through rtk (its output shortened). Under measurement.
    rtk: bool,
    /// `--batch-calls`: claude told to make independent tool calls in one turn. Under measurement.
    batch_calls: bool,
    /// `--boss-effort LEVEL`: the boss's workers at LEVEL (unset: medium, measured best).
    boss_effort: Option<String>,
    review_model: Option<String>,
    review_key_env: Option<String>,
    review_only: bool,
    /// The reviewer's key itself, read at start and gone from the environment by then (see main).
    review_key: Option<String>,
    /// Own loop: base URL and the environment variable with the key.
    provider: Option<(String, String)>,
    /// `Some(None)`: this repo's latest session; `Some(Some(id))`: that session.
    resume: Option<Option<String>>,
    /// Starting permission mode, in claude's names.
    mode: String,
    cache_ttl: Option<String>,
    repo_map: bool,
    explorer: Option<String>,
    explorer_key_env: String,
    tester: Option<String>,
    boss: Option<String>,
    no_boss: bool,
    /// `--pick`: open the model picker before the first message.
    pick: bool,
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
/// `reviewed`: DeepSeek 1017 implements on SubRouter, Opus (claude, the default medium effort) reviews the diff once:
/// the reviewer caught DeepSeek's blind spot (terraform, hidden tests 2/3 to 3/3) and flagged nothing on good diffs at
/// medium (2026-10-29). No `--data any`: SubRouter has to be on the trusted list, so a work repo's code does not go there
/// by accident. DeepSeek at high: the same judge as max, 41% faster, 32% cheaper (2026-10-31, measured as "xhigh", which
/// DeepSeek does not have and runs as high, its default; bench/effortprobe.py, 2026-11-01). It replaced `cascade` (DeepSeek V4.1 Flash on CheaperInference with `--data any`, 2026-10-29): the same
/// hand-over to Opus when tests fail, a newer model, the reviewer, and the data check. SubRouter's model list has no prices,
/// so the preset carries the one its dollar cap counts with (SubRouter's for 1017, 2026-10-31; without it every run
/// stopped at the start: the bench always set one, real use did not). The key is a checked route (bench/routes.py; LingAI,
/// where every DeepSeek measurement ran): SUBROUTER_API_KEY let SubRouter pick any provider, among them ones that cached
/// 22% or billed four times their listing (2026-11-01).
const PRESETS: [(&str, &str); 4] = [
    ("default", ""),
    ("boss", "--boss haiku"),
    ("max", "--model claude-opus-5-5 --effort max"),
    ("reviewed", "--provider https://subrouter.ai/v1 --key-env SUBROUTER_ROUTE_DEEPSEEK_FAST_KEY --model deepseek-v4.1-flash-1017 --effort high --price 0.022,0.0022,0.088 --review-model claude-opus-5-5"),
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
    let file = platform::home().unwrap_or_default().join(".config/tazuna/presets");
    let user = std::fs::read_to_string(&file).unwrap_or_default();
    let flags = user
        .lines()
        .filter_map(|l| l.split_once(':'))
        .find(|(n, _)| n.trim() == name)
        .map(|(_, f)| f.to_string())
        .or_else(|| PRESETS.iter().find(|(n, _)| *n == name).map(|(_, f)| f.to_string()))
        .with_context(|| format!("no preset {name} (built in: default, boss, max, reviewed; yours: {})", file.display()))?;
    let mut out: Vec<String> = flags.split_whitespace().map(String::from).collect();
    out.extend(args.into_iter().enumerate().filter(|(j, _)| *j != i && *j != i + 1).map(|(_, a)| a));
    Ok(out)
}

fn args() -> Result<Args> {
    parse(std::env::args().skip(1).collect())
}

fn parse(argv: Vec<String>) -> Result<Args> {
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
        repo_tools_core: false,
        rtk: false,
        batch_calls: false,
        boss_effort: None,
        review_model: None,
        review_key_env: None,
        review_only: false,
        review_key: None,
        provider: None,
        resume: None,
        mode: "bypassPermissions".into(),
        cache_ttl: None,
        repo_map: false,
        explorer: Some("haiku".into()),
        explorer_key_env: "OPENAI_API_KEY".into(),
        tester: None,
        boss: None,
        no_boss: false,
        pick: false,
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
    let mut it = with_preset(argv)?.into_iter();
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
            "--rtk" => a.rtk = true,
            "--batch-calls" => a.batch_calls = true,
            "--repo-tools" => match val()?.as_str() {
                "core" => a.repo_tools_core = true,
                "all" => a.repo_tools_core = false,
                v => bail!("--repo-tools takes core or all, not {v}"),
            },
            "--review-model" => a.review_model = Some(val()?),
            "--review-key-env" => a.review_key_env = Some(val()?),
            "--review-only" => a.review_only = true,
            "--provider" => a.provider = Some((val()?, a.provider.take().map_or("OPENAI_API_KEY".into(), |p| p.1))),
            "--key-env" => a.provider = Some((a.provider.take().map_or(String::new(), |p| p.0), val()?)),
            "-c" | "--continue" => a.resume = Some(None),
            "--resume" => a.resume = Some(Some(val()?)),
            "--repo-map" => a.repo_map = true,
            "--explorer" => a.explorer = Some(val()?),
            "--no-explorer" => a.explorer = None,
            "--explorer-key-env" => a.explorer_key_env = val()?,
            "--tester" => a.tester = Some(val()?),
            "--boss" => a.boss = Some(val()?),
            "--boss-effort" => {
                let e = val()?;
                if !models::LEVELS.contains(&e.as_str()) {
                    bail!("--boss-effort low|medium|high|xhigh|max, not {e}");
                }
                a.boss_effort = Some(e);
            }
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
            "--price" => {
                let v = val()?;
                let ok = v.split(',').filter(|x| x.trim().parse::<f64>().is_ok()).count() == 3 && v.split(',').count() == 3;
                anyhow::ensure!(ok, "--price takes input,cached,output in USD per million tokens, not {v}");
                // SAFETY: flags are parsed before any thread starts; the own loop (a child process) reads it
                unsafe { std::env::set_var("TAZUNA_PRICE", v) };
            }
            "--no-escalate" => a.no_escalate = true,
            "--no-boss" => a.no_boss = true,
            "--pick" => a.pick = true,
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
    if std::env::args().nth(1).as_deref() == Some("explore-mcp") {
        return engine::explore_serve(&std::env::args().skip(2).collect::<Vec<_>>());
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
    // `tazuna --loop [FLAGS]`: TODO.md's open items, one fresh headless run each, with FLAGS (looprun.rs)
    let all: Vec<String> = std::env::args().skip(1).collect();
    if all.iter().any(|x| x == "--loop") {
        let flags: Vec<String> = all.into_iter().filter(|x| x != "--loop").collect();
        parse(flags.clone())?; // a wrong flag fails here, not in every run
        if !live::in_git_repo() {
            bail!("--loop commits each item: run it inside a git repo");
        }
        let end = looprun::run(&std::env::current_dir()?, &flags, &std::sync::atomic::AtomicBool::new(false), &mut |m| eprintln!("{m}"))?;
        looprun::notify(&end);
        println!("{end}");
        return Ok(());
    }
    let mut a = args()?;
    // a reviewer on a provider's API reads the code too; its key leaves the environment before anything starts, so the
    // agent's commands and the test runs on its code never see it
    if let Some((_, url)) = a.review_model.as_deref().and_then(|m| m.split_once('@')) {
        check_data(url, a.data_any)?;
        let var = a.review_key_env.clone().context("--review-model MODEL@URL needs --review-key-env VAR")?;
        a.review_key = Some(std::env::var(&var).with_context(|| format!("{var} not set"))?);
        // SAFETY: no other thread has started yet
        unsafe { std::env::remove_var(&var) };
    }
    if a.review_only {
        let task = std::fs::read_to_string(a.run.as_ref().context("--review-only needs --run TASK-FILE")?)?;
        let model = a.review_model.as_deref().context("--review-only needs --review-model")?;
        let r = review::run(&std::env::current_dir()?, &task, model, &a.review_effort, a.review_key.as_deref());
        println!("{}", json!({"ok": r.ok, "issues": r.issues, "cost": r.cost, "secs": r.secs, "error": r.error}));
        return Ok(());
    }
    if let Some((url, _)) = &a.provider {
        check_data(url, a.data_any)?;
        // every other route sees the code too (0.0.79, `TAZUNA_ROUTES`)
        for r in std::env::var("TAZUNA_ROUTES").unwrap_or_default().split([';', '\n']) {
            if let Some(u) = r.split_whitespace().next() {
                check_data(u, a.data_any)?;
            }
        }
    }
    if let Some((_, url)) = a.explorer.as_deref().and_then(|e| e.split_once('@')) {
        check_data(url, a.data_any)?; // the explorer reads the code, so the code goes to its provider
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
            let (lane, boss) = lane(&a, true)?;
            let route = live.set_lane(lane);
            live.verify = a.verify.clone();
            live.bash_cap = a.bash_cap;
            live.diagnostics = a.diagnostics;
            live.self_review = a.self_review;
            live.repo_tools_core = a.repo_tools_core;
            live.rtk = a.rtk;
            live.batch_calls = a.batch_calls;
            live.review_model = a.review_model.clone().map(|m| (m, a.review_effort.clone()));
            live.review_key = a.review_key.clone();
            live.provider = a.provider.clone();
            live.project_instructions = a.project_instructions;
            live.cache_ttl = a.cache_ttl.clone();
            live.repo_map = a.repo_map;
            set_explorer(&mut live, &a);
            live.tester = a.tester.clone();
            live.boss = boss;
            live.boss_effort = a.boss_effort.clone();
            live.advisor = a.advisor.clone();
            if let Some((m, e)) = a.escalate.clone() {
                live.set_escalate(m, e);
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
/// Also the boss model, if any: `--boss`, or by default haiku for `--run` on Opus (company tickets: −37% cost, judge
/// −0.65, within noise; Go: −32%, −0.13; 0.0.49). Boss needs Claude Code's own tools, so the repo profile, and is dropped
/// when the repo is not trusted (its hooks must stay off).
fn lane(a: &Args, interactive: bool) -> Result<(route::Lane, Option<String>)> {
    let cwd = std::env::current_dir()?;
    let mut lane = match (&a.model, &a.effort) {
        (None, None) => route::choose(&cwd),
        (m, e) => route::manual(&cwd, m.as_deref().unwrap_or("claude-opus-5-5"), e.as_deref().unwrap_or("medium")),
    };
    let boss = default_boss(a, interactive, &lane.model);
    if let Some(p) = a.profile {
        lane.profile = p;
        lane.reason.push_str(&format!("; --profile {}", p.name()));
    } else if boss.is_some() {
        lane.profile = route::Profile::Repo;
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
    let boss = boss.filter(|_| lane.profile == route::Profile::Repo);
    if boss.is_some() {
        lane.reason.push_str("; boss (the main model directs, subagents do the work)");
    } else if a.boss.is_some() {
        eprintln!("--boss needs the repo profile (Claude Code's own tools and a hook), and this repo runs lean: boss is off");
    }
    Ok((lane, boss))
}

fn default_boss(a: &Args, interactive: bool, model: &str) -> Option<String> {
    if a.no_boss {
        return None;
    }
    let on = !interactive && a.provider.is_none() && model.starts_with("claude-opus");
    a.boss.clone().or_else(|| on.then(|| "haiku".to_string()))
}

/// Runs one task without a UI and prints claude's result event, the same JSON `claude -p --output-format json` prints.
/// Providers allowed under `--data trusted`, one host per line in ~/.config/tazuna/trusted-providers (Anthropic through
/// the claude CLI is always allowed). Providers that may store or use the code need `--data any` or an entry there.
/// `--explorer haiku`: a Claude subagent; `--explorer MODEL@URL`: the explore tool on tazuna's own loop.
fn set_explorer(live: &mut live::Live, a: &Args) {
    match a.explorer.as_deref().and_then(|e| e.split_once('@')) {
        Some((model, url)) => {
            live.explorer = None;
            live.explorer_provider = Some((url.trim_end_matches('/').to_string(), a.explorer_key_env.clone(), model.to_string()));
        }
        None => live.explorer = a.explorer.clone(),
    }
}

fn check_data(url: &str, any: bool) -> Result<()> {
    if any {
        return Ok(());
    }
    let host = host_of(url);
    if !trusted_host(&host) {
        let file = platform::home().unwrap_or_default().join(".config/tazuna/trusted-providers");
        bail!("{host} is not on the trusted list ({}): it may store or use your code. Add it there, or run with --data any", file.display());
    }
    Ok(())
}

/// A provider host on the trusted list (~/.config/tazuna/trusted-providers): it may see the code under `--data trusted`.
fn trusted_host(host: &str) -> bool {
    let file = platform::home().unwrap_or_default().join(".config/tazuna/trusted-providers");
    std::fs::read_to_string(&file).unwrap_or_default().lines().any(|l| l.trim() == host)
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
    let (lane, boss) = lane(&a, false)?;
    live.set_lane(lane);
    live.mode = "headless";
    live.verify = a.verify.clone();
    live.bash_cap = a.bash_cap;
    live.diagnostics = a.diagnostics;
    live.self_review = a.self_review;
    live.repo_tools_core = a.repo_tools_core;
    live.rtk = a.rtk;
    live.batch_calls = a.batch_calls;
    live.review_model = a.review_model.clone().map(|m| (m, a.review_effort.clone()));
    live.review_key = a.review_key.clone();
    live.provider = a.provider.clone();
    live.project_instructions = a.project_instructions;
    live.cache_ttl = a.cache_ttl.clone();
    live.repo_map = a.repo_map;
    set_explorer(&mut live, &a);
    live.tester = a.tester.clone();
    live.advisor = a.advisor.clone();
    live.boss = boss;
    live.boss_effort = a.boss_effort.clone();
    if let Some((m, e)) = a.escalate.clone() {
        live.set_escalate(m, e);
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
        // every round, process and run, the reviewer included (claude's own figure covers one process), plus the explorer's
        // answers when it runs on another provider (`--explorer MODEL@URL`)
        let explorer = live.explorer_cost();
        r["explorer_cost_usd"] = json!(explorer);
        r["total_cost_usd"] = json!(((cost + explorer) * 1e6).round() / 1e6);
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

    /// Draws one frame; returns where the transcript is and the block on each of its rows.
    fn frame(&mut self, f: &mut Frame, s: &Session, v: &View, dt: Duration) -> ui::Areas {
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
        areas
    }
}

fn view(design: Design, live: bool, speed: f64) -> View {
    View { design, expand_all: false, scroll: 0, clock_ms: 0.0, speed, paused: false, live, input: String::new(), cursor: 0, asking: None, mode: "auto".into(), hints: Vec::new(), bg_running: 0, running: vec![], tasks: None, rewind: None, models: None, vim: "", open: Default::default(), hover: None, since_ms: 0.0, sessions: None, toast: None, changed: vec![], diff: None }
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
            let _ = fx.frame(f, &s, &v, dt);
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
/// The transcript block on screen at (column, row), if any.
fn block_at(area: ratatui::layout::Rect, rows: &[Option<usize>], column: u16, row: u16) -> Option<usize> {
    let inside = column >= area.x && column < area.x + area.width && row >= area.y;
    inside.then(|| rows.get((row - area.y) as usize).copied().flatten()).flatten()
}

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
    platform::private_dir(&dir, true)?;
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos();
    let path = dir.join(format!("paste-{nanos}.png"));
    platform::clipboard_image(&path)?;
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
/// The files changed since `start` (the work tree when the conversation began), with git's line counts against HEAD;
/// a new, untracked file counts its lines as added.
fn changed_since(start: Option<&std::collections::BTreeMap<String, String>>, cwd: &std::path::Path) -> Vec<ui::Changed> {
    let (Some(start), Some(now)) = (start, runlog::git_snapshot(cwd)) else { return vec![] };
    let paths: Vec<String> = runlog::changed(start, &now).into_iter().cloned().collect();
    if paths.is_empty() {
        return vec![];
    }
    let numstat = std::process::Command::new("git").args(["diff", "--numstat", "HEAD", "--"]).args(&paths).current_dir(cwd).output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    paths.into_iter().map(|p| {
        let counts = numstat.lines().find_map(|l| {
            let mut f = l.splitn(3, '\t');
            let (a, d, path) = (f.next()?, f.next()?, f.next()?);
            (path == p).then(|| (a.parse().unwrap_or(0), d.parse().unwrap_or(0)))
        });
        let (added, removed) = counts.unwrap_or_else(|| (std::fs::read_to_string(cwd.join(&p)).map_or(0, |t| t.lines().count() as u64), 0));
        ui::Changed { path: p, added, removed }
    }).collect()
}

/// A file's diff against HEAD, line by line; a new, untracked file as all added.
fn file_diff(path: &str) -> Vec<String> {
    let out = std::process::Command::new("git").args(["diff", "HEAD", "--", path]).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    if !out.trim().is_empty() {
        return out.lines().map(String::from).collect();
    }
    std::fs::read_to_string(path).map(|t| std::iter::once(format!("+++ {path} (new file)")).chain(t.lines().map(|l| format!("+{l}"))).collect()).unwrap_or_default()
}

/// A conversation this large gets the /new hint after a task (then again at twice the size).
const CTX_HINT: u64 = 60_000;

/// One of this repo's earlier claude sessions (`/resume`): its id, first message, when it was last used, messages sent,
/// and the recordings that hold it (one per tazuna run it went through).
struct Earlier {
    id: String,
    first: String,
    last_at: u64,
    messages: usize,
    recordings: Vec<std::path::PathBuf>,
}

impl Earlier {
    /// "8m ago · 2 messages · fix the login (4cb8a388)"
    fn row(&self) -> String {
        let ago = session::unix_now().saturating_sub(self.last_at) / 60;
        let when = if ago < 60 { format!("{ago}m") } else if ago < 1440 { format!("{}h", ago / 60) } else { format!("{}d", ago / 1440) };
        format!("{when} ago · {} message{} · {}  ({})", self.messages, if self.messages == 1 { "" } else { "s" },
            session::shown(self.first.lines().next().unwrap_or_default(), 100), &self.id[..8.min(self.id.len())])
    }
}

/// This repo's claude sessions, the latest used first. The own loop's are left out: its engine resumes no conversation.
fn sessions_of(base: &std::path::Path, cwd: &std::path::Path) -> Vec<Earlier> {
    let cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
    let mut out: Vec<Earlier> = vec![];
    // the run log is in order: a session's last line says when it was last used
    let mut last_line: Vec<usize> = vec![];
    for (n, r) in std::fs::read_to_string(base.join("runs.jsonl")).unwrap_or_default().lines().filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok()).enumerate() {
        let m = &r["meta"];
        let Some(sid) = m["session_id"].as_str().filter(|s| !s.starts_with("own-")) else { continue };
        if !m["cwd"].as_str().map(std::path::PathBuf::from).is_some_and(|c| c.canonicalize().unwrap_or(c) == cwd) {
            continue;
        }
        let (ts, rec) = (r["ts"].as_u64().unwrap_or(0), m["recording"].as_str().map(std::path::PathBuf::from));
        let i = out.iter().position(|s| s.id == sid).unwrap_or_else(|| {
            out.push(Earlier { id: sid.to_string(), first: r["prompt"].as_str().unwrap_or_default().to_string(), last_at: ts, messages: 0, recordings: vec![] });
            last_line.push(0);
            out.len() - 1
        });
        last_line[i] = n;
        let e = &mut out[i];
        e.messages += 1;
        e.last_at = e.last_at.max(ts);
        if let Some(p) = rec.filter(|p| !e.recordings.contains(p)) {
            e.recordings.push(p);
        }
    }
    let mut both: Vec<(usize, Earlier)> = last_line.into_iter().zip(out).collect();
    both.sort_by_key(|(n, _)| std::cmp::Reverse(*n));
    both.into_iter().map(|(_, e)| e).collect()
}

/// The part of a recording that is conversation `id`: a recording holds several when `/new` or `/resume` started another
/// in the same run. Parts are split at their notices; a part is `id`'s when claude's init in it names that session.
fn conversation<'a>(events: &'a [(f64, serde_json::Value)], id: &str) -> Vec<&'a (f64, serde_json::Value)> {
    let mut parts: Vec<Vec<&(f64, serde_json::Value)>> = vec![vec![]];
    for e in events {
        let text = e.1["text"].as_str().unwrap_or_default();
        if e.1["type"] == "harness_notice" && (text.starts_with("── new conversation") || text.starts_with("── resumed session")) {
            parts.push(vec![]);
            continue;
        }
        parts.last_mut().expect("one at least").push(e);
    }
    parts.into_iter().filter(|p| p.iter().any(|e| e.1["type"] == "system" && e.1["session_id"] == id)).flatten().collect()
}

/// `/new` (None) or `/resume`: the next message starts a fresh conversation or continues `earlier` on claude. The screen
/// starts over: an earlier session's recordings are replayed into it, so what was said before shows again.
fn new_conversation(live: &mut live::Live, s: &mut Session, v: &mut View, earlier: Option<&Earlier>) {
    match live.restart(earlier.map(|e| e.id.clone())) {
        Ok((t, notice)) => {
            *s = s.fresh();
            if let Some(e) = earlier {
                for p in &e.recordings {
                    if let Ok(mut r) = Replay::load(p, false) {
                        for (t, ev) in conversation(r.take(usize::MAX), &e.id) {
                            s.apply(ev, *t);
                        }
                    }
                }
                s.model = live.model.clone();
            }
            s.apply(&notice, t);
            v.open.clear();
            (v.hover, v.scroll, v.since_ms) = (None, 0, live.elapsed_ms());
        }
        Err(err) => s.apply(&json!({"type": "harness_info", "text": format!("{err:#}")}), live.elapsed_ms()),
    }
}

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
            platform::private_file(&path, true).ok().map(|f| (path, f))
        })
        .context("could not create a private temporary file")?;
    std::io::Write::write_all(&mut file, text.as_bytes())?;
    drop(file);
    let status = platform::shell(&format!("{editor} \"$1\""), &[path.as_os_str()]).status();
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
    // the plan's windows as last read (claude reports them only now and then); SubRouter's balance, on a thread
    if let Some((h5, d7, at)) = labels::dir().ok().and_then(|d| session::last_limits(&d.join("sessions"))) {
        (s.limits, s.limits_at) = (Some((h5, d7)), Some(at));
    }
    let (status_tx, status_rx) = std::sync::mpsc::channel::<serde_json::Value>();
    fetch_balance(&status_tx);
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
    // the block on each transcript row, from the last frame: a click there opens or closes it, hovering shades it
    let mut rows: Vec<Option<usize>> = vec![];
    // links on screen: a click opens one in the browser
    let mut links: Vec<(ratatui::layout::Rect, String)> = vec![];
    // the files panel (cockpit): a click opens /diff
    let mut files_area: Option<ratatui::layout::Rect> = None;
    // the work tree when this conversation began: what it changed since is the files panel and /diff
    let mut git_start = runlog::git_snapshot(&std::env::current_dir()?);
    let mut changed_at = Instant::now();
    let mut sel: Option<select::Selection> = None;
    let (mut copy_now, mut copied) = (false, None::<String>);
    // ctrl-b: the background-task view (which row is selected, when its output was last asked for)
    let (mut tasks_open, mut task_sel, mut outputs_read) = (false, 0usize, Instant::now());
    // /rewind: the picker's selected message, while it is open
    let mut rewind_sel: Option<usize> = None;
    // /resume: this repo's sessions and the selected one, while the picker is open
    let mut resume_pick: Option<(Vec<Earlier>, usize)> = None;
    // /loop: the running loop (its stop flag, its progress), or the flags of one waiting for PRD.md and TODO.md; and
    // TODO.md's open items when the /loop hint last showed
    let mut looping: Option<(std::sync::Arc<std::sync::atomic::AtomicBool>, std::sync::mpsc::Receiver<String>)> = None;
    let mut loop_after_plan: Option<Vec<String>> = None;
    // /model: the choices, the selected row (in the filtered list), each choice's effort and what has been typed to filter
    let mut pick: Option<Picker> = None;
    if a.pick {
        pick = Some(open_picker(live)?);
    }
    let mut hinted = 0usize;
    // the context size at which /new was last suggested (0: not yet in this conversation)
    let mut ctx_hint = 0u64;
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
        let row = |t: &live::BgTask| ui::TaskRow {
            kind: t.kind.clone(),
            description: t.description.clone(),
            status: t.status.clone(),
            secs: t.ended.unwrap_or_else(Instant::now).duration_since(t.started).as_secs(),
            output: t.output.clone(),
            agent: t.agent.clone(),
            tokens: t.tokens,
            tools: t.tools,
            doing: t.doing.clone(),
        };
        v.running = live.bg.iter().filter(|t| t.ended.is_none()).map(row).collect();
        v.tasks = tasks_open.then(|| ui::TasksPanel { selected: task_sel, rows: live.bg.iter().map(row).collect() });
        v.vim = if !vim { "" } else if normal { "NORMAL" } else { "INSERT" };
        v.sessions = resume_pick.as_ref().map(|(l, i)| ui::SessionsPanel { rows: l.iter().map(Earlier::row).collect(), selected: *i });
        v.rewind = rewind_sel.map(|i| ui::RewindPanel { items: live.checkpoints.iter().map(|c| c.text.clone()).collect(), selected: i });
        v.models = pick.as_ref().map(|Picker { cs, sel, eff, prov, filter }| ui::ModelsPanel {
            rows: shown(cs, filter).into_iter().map(|i| {
                let c = &models::pinned(&cs[i], prov[i]);
                ui::ModelRow { label: c.label.clone(), place: c.place.clone(), effort: c.efforts.get(eff[i]).cloned().unwrap_or_else(|| "default".into()),
                    price: c.price.clone(), note: c.note.clone(), bench: c.bench.clone(), aa: c.aa.clone(),
                    state: match c.state { models::State::Ready => 0, models::State::NoKey => 1, models::State::Untrusted => 2, models::State::Unchecked => 3 },
                    stale: c.stale, current: c.model == live.model && c.route.as_ref().map(|r| (&r.0, &r.1)) == live.provider.as_ref().map(|p| (&p.0, &p.1)) }
            }).collect(),
            selected: *sel,
            filter: filter.clone(),
        });
        let dt = last.elapsed();
        last = Instant::now();
        for (t, e) in live.poll() {
            s.apply(&e, t);
            if e["type"] == "result" && e["tazuna_paid"] == true {
                fetch_balance(&status_tx); // a run on SubRouter spent some of it
            }
            // what the conversation changed: after each run, and while one runs at most every 5 s (a few git calls each)
            if e["type"] == "result" || (e["type"] == "user" && changed_at.elapsed() > Duration::from_secs(5)) {
                v.changed = changed_since(git_start.as_ref(), &std::env::current_dir().unwrap_or_default());
                changed_at = Instant::now();
            }
            // a plan in TODO.md: say once that /loop can build it
            if e["type"] == "result" && looping.is_none() && loop_after_plan.is_none() {
                let n = looprun::open_items(&std::fs::read_to_string("TODO.md").unwrap_or_default()).len();
                if n >= 3 && n != hinted {
                    s.apply(&json!({"type": "harness_info", "text": format!("TODO.md has {n} open items: /loop builds them one by one without you (/loop reviewed: on DeepSeek)")}), t);
                }
                hinted = n;
            }
            // a large conversation: every message re-reads all of it (a review session sent 1.16M tokens over 16 calls
            // at 94k, 2026-11-02), so a new, unrelated task is cheaper in a fresh one. Said at 60k, then at each doubling.
            if e["type"] == "result" && s.context >= CTX_HINT.max(ctx_hint * 2) {
                ctx_hint = s.context;
                s.apply(&json!({"type": "harness_info", "text": format!("this conversation is {} tokens, and every message re-reads all of it: for an unrelated task, /new starts fresh (/resume comes back)", ui::human(s.context))}), t);
            }
        }
        while let Ok(e) = status_rx.try_recv() {
            match e["type"].as_str() {
                Some("tazuna_balance") => s.balance = e["usd"].as_f64(),
                Some("tazuna_usage_done") => {
                    let text = ui::limits_str(&s, v.design.theme()).map_or("no reading: claude sent none, and no SubRouter key is set".into(), |(t, _)| t);
                    s.apply(&json!({"type": "harness_info", "text": format!("usage: {text}")}), live.elapsed_ms());
                }
                _ => s.apply(&e, live.elapsed_ms()),
            }
        }
        if let Some((_, rx)) = &looping {
            let mut over = false;
            loop {
                match rx.try_recv() {
                    Ok(m) => s.apply(&json!({"type": "harness_info", "text": m}), live.elapsed_ms()),
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        over = true;
                        break;
                    }
                }
            }
            if over {
                looping = None;
            }
        }
        if loop_after_plan.is_some() && live.idle() {
            let flags = loop_after_plan.take().unwrap_or_default();
            let msg = if looprun::open_items(&std::fs::read_to_string("TODO.md").unwrap_or_default()).is_empty() {
                "no open items in TODO.md: the loop did not start".to_string()
            } else {
                looping = Some(start_loop(flags));
                LOOP_STARTED.to_string()
            };
            s.apply(&json!({"type": "harness_info", "text": msg}), live.elapsed_ms());
        }
        v.clock_ms = live.elapsed_ms();
        term.draw(|f| {
            let areas = fx.frame(f, &s, &v, dt);
            (transcript, rows, links, files_area) = (areas.transcript, areas.rows, areas.links, areas.files);
            if let Some(sel) = &sel {
                sel.highlight(f.buffer_mut());
                if copy_now {
                    copied = Some(sel.text(f.buffer_mut()));
                }
            }
        })?;
        if let Some(text) = copied.take() {
            // copied only on cmd-c / ctrl-c, and said in the composer for a moment, not in the transcript (a "copied 0
            // chars" line after every stray drag, 2026-11-02); a selection of blank cells copies nothing
            copy_now = false;
            sel = None;
            if !text.trim().is_empty() {
                let msg = match to_clipboard(&text) {
                    Ok(()) => format!("copied {} chars", text.chars().count()),
                    Err(e) => format!("copy failed: {e:#}"),
                };
                v.toast = Some((msg, Instant::now()));
            }
        }
        if v.toast.as_ref().is_some_and(|(_, at)| at.elapsed() > Duration::from_millis(2500)) {
            v.toast = None;
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
                        // the selection stays highlighted until cmd-c / ctrl-c copies it or another key drops it
                        Some(x) if !x.is_empty() => v.toast = Some(("⌘C or ctrl+c copies the selection".into(), Instant::now())),
                        // a click without a drag: the block under it opens or closes
                        _ => {
                            sel = None;
                            let pos = ratatui::layout::Position::new(m.column, m.row);
                            if files_area.is_some_and(|r| r.contains(pos)) && !v.changed.is_empty() {
                                v.diff = Some(ui::DiffPanel { selected: 0, diff: file_diff(&v.changed[0].path), scroll: 0 });
                            } else if let Some((_, url)) = links.iter().find(|(r, _)| r.contains(pos)) {
                                let msg = match platform::open_url(url) {
                                    Ok(()) => format!("opened {url}"),
                                    Err(e) => format!("could not open {url}: {e}"),
                                };
                                s.apply(&json!({"type": "harness_info", "text": msg}), live.elapsed_ms());
                            } else if let Some(b) = block_at(transcript, &rows, m.column, m.row)
                                && !v.open.remove(&b)
                            {
                                v.open.insert(b);
                            }
                        }
                    },
                    MouseEventKind::Moved => v.hover = block_at(transcript, &rows, m.column, m.row),
                    k => scroll_mouse(&mut v, k),
                }
                continue;
            }
            Event::Key(k) if k.kind == KeyEventKind::Press => k,
            _ => continue,
        };
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        // cmd-c (where the terminal passes it on) or ctrl-c with text selected copies it; ctrl-c without a selection
        // still stops or quits
        if key.code == KeyCode::Char('c') && (ctrl || key.modifiers.contains(KeyModifiers::SUPER)) && sel.as_ref().is_some_and(|x| !x.is_empty()) {
            copy_now = true;
            continue;
        }
        sel = None;
        // an open permission question owns the keyboard: only y(es), a(lways), n(o)/esc answer it, every other key is
        // ignored, and keys in the first moment after it appears count for nothing (typing must not approve by accident)
        // ctrl-c and ctrl-d still stop or quit: stopping answers every open question with no
        let stop_key = ctrl && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('d'));
        // the /model picker owns the keyboard while open
        if let (Some(Picker { cs, sel, eff, prov, filter }), false) = (pick.as_mut(), stop_key) {
            let rows = shown(cs, filter);
            let at = rows.get(*sel).copied();
            let mut chosen = None;
            match key.code {
                KeyCode::Up => *sel = sel.saturating_sub(1),
                KeyCode::Down => *sel = (*sel + 1).min(rows.len().saturating_sub(1)),
                KeyCode::Left => if let Some(i) = at { eff[i] = eff[i].saturating_sub(1) },
                KeyCode::Right => if let Some(i) = at { eff[i] = (eff[i] + 1).min(cs[i].efforts.len().saturating_sub(1)) },
                // tab: the next checked provider pinned (after the last, any of them again); shift-tab: back
                KeyCode::Tab => if let Some(i) = at { prov[i] = (prov[i] + 1) % (cs[i].pins.len() + 1) },
                KeyCode::BackTab => if let Some(i) = at { prov[i] = (prov[i] + cs[i].pins.len()) % (cs[i].pins.len() + 1) },
                KeyCode::Backspace => {
                    filter.pop();
                    *sel = 0;
                }
                KeyCode::Char(c) if !ctrl => {
                    filter.push(c);
                    *sel = 0;
                }
                KeyCode::Enter => chosen = at.map(|i| (models::pinned(&cs[i], prov[i]), cs[i].efforts.get(eff[i]).cloned())),
                KeyCode::Esc => chosen = None,
                _ => {}
            }
            if matches!(key.code, KeyCode::Enter | KeyCode::Esc) {
                pick = None;
            }
            if let Some((c, effort)) = chosen {
                let msg = match c.state {
                    models::State::NoKey => Err(anyhow::anyhow!("{}: its key ({}) is not set: `source ~/.subrouter-key` before starting tazuna",
                        c.label, c.route.as_ref().map(|r| r.1.as_str()).unwrap_or_default())),
                    models::State::Untrusted => Err(anyhow::anyhow!("{}: the provider is not on the trusted list (~/.config/tazuna/trusted-providers), so the code may not go there", c.label)),
                    // a model without levels (the API ignores them) keeps the session's
                    models::State::Ready => live.switch(c.model.clone(), effort.unwrap_or_else(|| live.effort.clone()), c.route.clone()),
                    models::State::Unchecked => live.switch(c.model.clone(), effort.unwrap_or_else(|| live.effort.clone()), c.route.clone()).map(|(t, e)| {
                        let text = format!("{} · unchecked route: SubRouter picks the provider, so cache, price and quality are unknown \
                            (bench/routes.py refresh {} checks it)", e["text"].as_str().unwrap_or_default(), c.model);
                        (t, json!({"type": "harness_switch", "model": e["model"], "effort": e["effort"], "text": text}))
                    }),
                };
                match msg {
                    Ok((t, e)) => s.apply(&e, t),
                    Err(err) => s.apply(&json!({"type": "harness_info", "text": format!("{err:#}")}), live.elapsed_ms()),
                }
            }
            continue;
        }
        // the /rewind picker owns the keyboard while open
        // the /diff view owns the keyboard while open
        if let (Some(p), false) = (v.diff.as_mut(), stop_key) {
            let last = v.changed.len().saturating_sub(1);
            let page = 20;
            match key.code {
                KeyCode::Up | KeyCode::Down => {
                    p.selected = if key.code == KeyCode::Up { p.selected.saturating_sub(1) } else { (p.selected + 1).min(last) };
                    p.diff = v.changed.get(p.selected).map(|c| file_diff(&c.path)).unwrap_or_default();
                    p.scroll = 0;
                }
                KeyCode::PageDown | KeyCode::Char(' ') => p.scroll = (p.scroll + page).min(p.diff.len().saturating_sub(1)),
                KeyCode::PageUp => p.scroll = p.scroll.saturating_sub(page),
                KeyCode::Esc | KeyCode::Char('q') => v.diff = None,
                _ => {}
            }
            continue;
        }
        // the /resume picker owns the keyboard while open
        if let (Some((list, i)), false) = (resume_pick.as_mut(), stop_key) {
            match key.code {
                KeyCode::Up => *i = i.saturating_sub(1),
                KeyCode::Down => *i = (*i + 1).min(list.len().saturating_sub(1)),
                KeyCode::Enter => {
                    let (list, i) = resume_pick.take().expect("open");
                    if let Some(e) = list.get(i) {
                        new_conversation(live, &mut s, &mut v, Some(e));
                                git_start = runlog::git_snapshot(&std::env::current_dir().unwrap_or_default());
                                v.changed.clear();
                        ctx_hint = 0;
                    }
                }
                KeyCode::Esc => resume_pick = None,
                _ => {}
            }
            continue;
        }
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
                if text == "/model" {
                    pick = Some(open_picker(live)?);
                    continue;
                }
                if text == "/usage" {
                    s.apply(&json!({"type": "harness_info", "text": "reading the plan's windows (one short Haiku call through claude) and SubRouter's balance…"}), live.elapsed_ms());
                    let tx = status_tx.clone();
                    std::thread::spawn(move || {
                        if let Some(e) = plan_reading() {
                            let _ = tx.send(e);
                        }
                        if let Some(b) = models::subrouter_balance() {
                            let _ = tx.send(json!({"type": "tazuna_balance", "usd": b}));
                        }
                        let _ = tx.send(json!({"type": "tazuna_usage_done"}));
                    });
                    continue;
                }
                // a command typed short ("/mo") would go to claude as a message, at a model turn's cost
                if let Some(word) = text.strip_prefix('/').filter(|w| !w.is_empty() && !w.contains(char::is_whitespace)) {
                    let like: Vec<&String> = s.commands.iter().filter(|c| c.starts_with(word)).collect();
                    if !like.is_empty() && !like.iter().any(|c| *c == word) {
                        let names: Vec<String> = like.iter().take(4).map(|c| format!("/{c}")).collect();
                        s.apply(&json!({"type": "harness_info", "text": format!("no command /{word}: did you mean {}? (nothing was sent; tab completes)", names.join(", "))}), live.elapsed_ms());
                        ed.insert(&text);
                        continue;
                    }
                }
                // /new: a fresh conversation; /resume: a picker of this repo's sessions (/resume ID opens one directly)
                // /boss: boss mode on (haiku workers, or /boss MODEL) or off for the rest of the session
                if text == "/boss" || text.starts_with("/boss ") {
                    let arg = text["/boss".len()..].trim();
                    let want = match arg {
                        "" => if live.boss.is_some() { None } else { Some("haiku".to_string()) },
                        "off" => None,
                        m => Some(m.to_string()),
                    };
                    let msg = live.set_boss(want);
                    match msg {
                        Ok((t, e)) => s.apply(&e, t),
                        Err(err) => s.apply(&json!({"type": "harness_info", "text": format!("{err:#}")}), live.elapsed_ms()),
                    }
                    continue;
                }
                if text == "/diff" {
                    v.changed = changed_since(git_start.as_ref(), &std::env::current_dir().unwrap_or_default());
                    match v.changed.first() {
                        Some(c) => v.diff = Some(ui::DiffPanel { selected: 0, diff: file_diff(&c.path), scroll: 0 }),
                        None => s.apply(&json!({"type": "harness_info", "text": "no file changed in this conversation yet"}), live.elapsed_ms()),
                    }
                    continue;
                }
                if text == "/new" {
                    new_conversation(live, &mut s, &mut v, None);
                    git_start = runlog::git_snapshot(&std::env::current_dir()?);
                    v.changed.clear();
                    ctx_hint = 0;
                    continue;
                }
                if text == "/resume" || text.starts_with("/resume ") {
                    let list = sessions_of(&labels::dir()?, &std::env::current_dir()?);
                    match text["/resume".len()..].trim() {
                        "" => resume_pick = Some((list, 0)),
                        id => match list.iter().find(|e| e.id.starts_with(id)) {
                            Some(e) => {
                                new_conversation(live, &mut s, &mut v, Some(e));
                                git_start = runlog::git_snapshot(&std::env::current_dir().unwrap_or_default());
                                v.changed.clear();
                                ctx_hint = 0;
                            }
                            None => s.apply(&json!({"type": "harness_info", "text": format!("no session {id} in this repo: /resume lists them")}), live.elapsed_ms()),
                        },
                    }
                    continue;
                }
                if text == "/keys" {
                    let all = ui::LIVE_KEYS.split(" · ").collect::<Vec<_>>().join("\n");
                    s.apply(&json!({"type": "harness_info", "text": format!("keys:\n{all}")}), live.elapsed_ms());
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
                if text == "/loop" || text.starts_with("/loop ") {
                    let rest = text["/loop".len()..].trim();
                    let msg = if rest == "stop" {
                        match &looping {
                            Some((stop, _)) => {
                                stop.store(true, std::sync::atomic::Ordering::Relaxed);
                                "the loop stops after the current item".to_string()
                            }
                            None => "no loop running".to_string(),
                        }
                    } else if looping.is_some() || loop_after_plan.is_some() {
                        "a loop is already running (/loop stop ends it after the current item)".to_string()
                    } else {
                        // flags are checked by each run (parsing them here would set the own loop's variables in
                        // this process, with threads running): a wrong one stops the loop after two quick failures
                        let flags = looprun::flags(rest);
                        match () {
                            _ if looprun::open_items(&std::fs::read_to_string("TODO.md").unwrap_or_default()).is_empty() => {
                                match live.send(looprun::PLAN) {
                                    Ok((t, e)) => s.apply(&e, t),
                                    Err(err) => s.apply(&json!({"type": "harness_notice", "text": format!("{err:#}"), "error": true}), live.elapsed_ms()),
                                }
                                loop_after_plan = Some(flags);
                                "no open items in TODO.md yet: asking for PRD.md and TODO.md first; the loop starts once they are written".to_string()
                            }
                            _ => {
                                looping = Some(start_loop(flags));
                                LOOP_STARTED.to_string()
                            }
                        }
                    };
                    s.apply(&json!({"type": "harness_info", "text": msg}), live.elapsed_ms());
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

/// The /model picker's state, opened on the session's model.
/// SubRouter's balance, read on a thread of its own; it arrives through `tx`.
fn fetch_balance(tx: &std::sync::mpsc::Sender<serde_json::Value>) {
    let tx = tx.clone();
    std::thread::spawn(move || {
        if let Some(b) = models::subrouter_balance() {
            let _ = tx.send(json!({"type": "tazuna_balance", "usd": b}));
        }
    });
}

/// The plan's windows now: one short Haiku call through the official `claude` CLI, which reports them in its
/// rate_limit_event (bench/guard.py reads them the same way). None when it sends none.
fn plan_reading() -> Option<serde_json::Value> {
    let dir = std::env::temp_dir();
    let out = std::process::Command::new("claude")
        .args(["-p", "Reply with: ok", "--model", "claude-haiku-5-5", "--output-format", "stream-json", "--verbose", "--no-session-persistence",
               "--setting-sources", "", "--strict-mcp-config", "--tools", "", "--system-prompt", "Reply briefly."])
        .current_dir(dir).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).output().ok()?;
    String::from_utf8_lossy(&out.stdout).lines().filter(|l| l.contains("\"rate_limit_event\"")).find_map(|l| serde_json::from_str(l).ok())
}

/// The /model picker: the choices, the selected row, each row's effort and pinned provider (0: any checked one), the filter.
struct Picker {
    cs: Vec<models::Choice>,
    sel: usize,
    eff: Vec<usize>,
    prov: Vec<usize>,
    filter: String,
}

fn open_picker(live: &live::Live) -> Result<Picker> {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let cs = models::catalog(&labels::dir()?.join("routes.json"), trusted_host, now);
    let eff: Vec<usize> = cs.iter().map(|c| models::start_effort(c, &live.model, &live.effort)).collect();
    // a session on a pinned provider's key opens with that provider pinned
    let key = live.provider.as_ref().map(|p| p.1.as_str());
    let prov: Vec<usize> = cs.iter().map(|c| c.pins.iter().position(|p| Some(p.key_env.as_str()) == key).map_or(0, |i| i + 1)).collect();
    let sel = shown(&cs, "").iter().position(|&i| cs[i].model == live.model).unwrap_or(0);
    Ok(Picker { cs, sel, eff, prov, filter: String::new() })
}

/// The picker rows that match what was typed (label or place, any case), as indices into the choices.
fn shown(cs: &[models::Choice], filter: &str) -> Vec<usize> {
    let f = filter.to_lowercase();
    (0..cs.len()).filter(|&i| f.is_empty() || cs[i].label.to_lowercase().contains(&f) || cs[i].place.to_lowercase().contains(&f)).collect()
}

const LOOP_STARTED: &str = "loop started: one fresh run per TODO.md item, a commit each, a notification at the end; messages you \
    send meanwhile edit the same files (/loop stop ends it after the current item)";

/// Runs the PRD loop on a thread of its own; its progress lines come through the receiver, which closes when it ends.
fn start_loop(flags: Vec<String>) -> (std::sync::Arc<std::sync::atomic::AtomicBool>, std::sync::mpsc::Receiver<String>) {
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (tx, rx) = std::sync::mpsc::channel();
    let flag = stop.clone();
    std::thread::spawn(move || {
        let cwd = std::env::current_dir().unwrap_or_default();
        let end = looprun::run(&cwd, &flags, &flag, &mut |m| {
            let _ = tx.send(m);
        })
        .unwrap_or_else(|e| format!("failed: {e:#}"));
        looprun::notify(&format!("loop {end}"));
        let _ = tx.send(format!("loop {end}"));
    });
    (stop, rx)
}

#[cfg(test)]
mod tests {
    #[test]
    fn boss_by_default_only_for_headless_opus_on_claude() {
        let p = |a: &[&str]| super::parse(a.iter().map(|s| s.to_string()).collect()).unwrap();
        let opus = "claude-opus-5-5";
        assert_eq!(super::default_boss(&p(&["--run", "t.txt"]), false, opus).as_deref(), Some("haiku"));
        assert_eq!(super::default_boss(&p(&["--run", "t.txt"]), true, opus), None, "live sessions: unmeasured, off");
        assert_eq!(super::default_boss(&p(&["--run", "t.txt"]), false, "claude-sonnet-5-5"), None, "tiny-repo lane");
        assert_eq!(super::default_boss(&p(&["--run", "t.txt", "--no-boss"]), false, opus), None);
        assert_eq!(super::default_boss(&p(&["--preset", "boss", "--no-boss"]), true, opus), None, "--no-boss wins");
        assert_eq!(super::default_boss(&p(&["--boss", "sonnet"]), true, opus).as_deref(), Some("sonnet"));
        let own = p(&["--run", "t.txt", "--data", "any", "--provider", "https://x/v1", "--model", "deepseek-v4.1-flash"]);
        assert_eq!(super::default_boss(&own, false, opus), None, "own loop");
    }

    #[test]
    fn changed_files_count_their_lines_since_the_conversation_began() {
        let repo = std::env::temp_dir().join(format!("tazuna-changed-{}", std::process::id()));
        std::fs::create_dir_all(&repo).unwrap();
        let git = |a: &[&str]| assert!(std::process::Command::new("git").args(a).current_dir(&repo).output().unwrap().status.success());
        std::fs::write(repo.join("a.txt"), "1\n2\n3\n").unwrap();
        std::fs::write(repo.join("old.txt"), "dirty before\n").unwrap();
        git(&["init", "-q"]);
        git(&["add", "-A"]);
        git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "base"]);
        std::fs::write(repo.join("old.txt"), "changed before the conversation\n").unwrap();
        let start = crate::runlog::git_snapshot(&repo);
        std::fs::write(repo.join("a.txt"), "1\ntwo\n3\n4\n").unwrap();
        std::fs::write(repo.join("new.txt"), "x\ny\n").unwrap();
        let c = super::changed_since(start.as_ref(), &repo);
        let got: Vec<(&str, u64, u64)> = c.iter().map(|c| (c.path.as_str(), c.added, c.removed)).collect();
        assert_eq!(got, [("a.txt", 2, 1), ("new.txt", 2, 0)], "only what changed since the start, old.txt left out");
        std::fs::remove_dir_all(&repo).ok();
    }

    #[test]
    fn presets_expand_first_so_own_flags_win() {
        let v = |x: &[&str]| x.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let out = super::with_preset(v(&["--run", "t.txt", "--preset", "max", "--effort", "high"])).unwrap();
        assert_eq!(out, v(&["--model", "claude-opus-5-5", "--effort", "max", "--run", "t.txt", "--effort", "high"]));
        assert_eq!(super::with_preset(v(&["--run", "t.txt"])).unwrap(), v(&["--run", "t.txt"]));
        assert!(super::with_preset(v(&["--preset", "nope"])).is_err());
        let r = super::parse(super::with_preset(v(&["--run", "t.txt", "--preset", "reviewed"])).unwrap()).unwrap();
        assert_eq!((r.review_model.as_deref(), r.review_effort.as_str(), r.data_any), (Some("claude-opus-5-5"), "medium", false));
    }

    #[test]
    fn the_external_editor_round_trips_the_message() {
        assert_eq!(super::edit_in("printf 'edited\\n' >", "draft").unwrap(), "edited");
        assert_eq!(super::edit_in("true", "keep me").unwrap(), "keep me");
        assert!(super::edit_in("false", "x").is_err());
        // the file the editor gets is private to the user (GNU stat on Linux, BSD stat on macOS)
        #[cfg(unix)]
        let mode = super::edit_in("sh -c '(stat -c %a \"$0\" 2>/dev/null || stat -f %Lp \"$0\") > \"$0.mode\"; cp \"$0.mode\" \"$0\"; rm \"$0.mode\"'", "x").unwrap();
        #[cfg(unix)]
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
        let list = sessions_of(&base, &repo);
        assert_eq!(list.first().map(|e| e.id.as_str()), Some("ccc-3"), "the latest used first, this repo only");
        // a recording with two conversations (a /new between them): only the one asked for is replayed
        let ev = |v: serde_json::Value| (0.0, v);
        let rec = vec![ev(json!({"type": "harness_user", "text": "one"})), ev(json!({"type": "system", "subtype": "init", "session_id": "s1"})),
            ev(json!({"type": "harness_notice", "text": "── new conversation ──"})),
            ev(json!({"type": "harness_user", "text": "two"})), ev(json!({"type": "system", "subtype": "init", "session_id": "s2"}))];
        let texts = |id: &str| super::conversation(&rec, id).iter().filter_map(|e| e.1["text"].as_str().map(String::from)).collect::<Vec<_>>();
        assert_eq!((texts("s1"), texts("s2")), (vec!["one".to_string()], vec!["two".to_string()]));
        std::fs::remove_dir_all(&base).ok();
    }
}
