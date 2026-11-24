//! Drives a real `claude` process (stream-json in and out) with the cc-mid profile, and records every event.
//!
//! One process serves the whole conversation, so follow-ups keep the prompt cache warm. Stopping a run sends the
//! interrupt control request the official Agent SDK uses; if claude does not answer, the process is killed and the
//! next message resumes the session with `--resume`.

use crate::{diag, review, route::{Lane, Profile}, runlog::Run, verify};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    fs::File,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{Receiver, Sender, channel},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// "Do not commit" until 0.1.116: asked to fix CI "in that PR", the own loop fixed the file and stopped without a commit
/// or a push (suite ops pilot, 2026-11-12).
const MID_PROMPT: &str = "You are a coding agent working in {cwd}. Today is {date}.
- Read the relevant code before editing. Use Grep/Glob to search.
- Fix root causes with the smallest change that follows the existing code style.
- Add or update tests when behavior changes. Run the relevant tests and fix failures before finishing.
- Never run destructive commands (rm -rf outside the working directory, git reset --hard, force push). Commit, push or open a pull request only when the request asks for it.
- Finish with a short summary of what changed and how you verified it.";

/// The own loop (0.0.90): code outside the repo implements and calls exported APIs; DeepSeek added a method to
/// prometheus's exported ExemplarStorage interface in 8 of 10 replayed plans, 4 of 9 with the first version of this line
/// (sigprobe.py, 2026-10-26). 0.0.99 narrows it to breaking changes: under "do not change exported APIs" gpt-6.1-sol added
/// nothing exported at all ("This keeps public APIs unchanged") and broke caddy-retry's upstream tests in 2 of 5 runs,
/// 8/8 and judge 7.75 against 5.30 with this text; DeepSeek on prometheus still left the interface alone.
const API_RULE: &str = "\n- Do not change or remove existing exported APIs (public functions, methods, interfaces and types other packages use), or add methods to exported interfaces, unless the task asks for it; adding a new exported function, or a method on a concrete type, is fine.";

/// Experiment (TAZUNA_API_SCOPE): the rule above held for code only the repo can reach too. On hugo the model quoted it in
/// 5 of 6 runs and worked around Go's internal/ template fork instead of changing it: a parallel interface, a dead field
/// kept, judge 3-4 on "heavier than needed" (2026-11-11). Same protection outside the repo, a free hand inside.
const API_SCOPE_RULE: &str = "\n- Do not change or remove exported APIs that code outside this repository can use (public functions, methods, interfaces and types of its importable packages; in Go, not those under internal/), or add methods to such interfaces, unless the task asks for it; adding a new exported function, or a method on a concrete type, is fine. Inside the repository, change signatures and their call sites when that is the simplest fix, and delete what your change leaves unused.";

/// A slash command and nothing else ("/clear", "/mcp off"): its first word is a command name, not a path ("/usr/bin").
pub fn is_command(text: &str) -> bool {
    text.trim_start().strip_prefix('/').and_then(|t| t.split_whitespace().next())
        .is_some_and(|w| !w.is_empty() && w.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == ':'))
}

/// The own loop (0.0.96): providers drop a model's earlier reasoning (DeepSeek through SubRouter's foxrouter and LingAI,
/// Chat and Messages alike: a code word planted in it was never recalled), so a design worked out once was worked out
/// again at every new finding (six times, 13-26K characters each, on one caddy run). A plan written in the reply stays.
const PLAN_RULE: &str = "\n- Your reasoning is not kept between steps. Once you have decided on a design or plan, write it in your reply in 2-4 lines before acting on it, and update it when it changes: that text is all you will see of your thinking later.";

/// repo-lean only: with Claude Code's own tools the short prompt read ~25% less code and ran the type check less than
/// Claude Code's own prompt did (company tickets, 2026-10-07), and the judge marked down guessed names.
const CHECK_RULES: &str = "\n- Before changing code, read the code you change, its callers, and the definition of every name you rely on (database columns, fields, functions, APIs). Never assume one exists.\n- Before finishing, run the repo's type check and the relevant tests.";

/// `--self-review`: appended to the system prompt, against ending early and against unchecked assumptions.
/// `--repo-tools core`: Claude Code's tools a coding task uses. Left out: scheduling (Cron*, ScheduleWakeup, RemoteTrigger),
/// messaging and sharing (SendMessage, SendToProject*, PushNotification, ShareOnboardingGuide, PublishPlugin), worktrees,
/// workflows, Monitor, DesignSync, ReportFindings, ListAgents. TaskStop and ToolSearch stay: background tasks and deferred
/// tools need them.
const REPO_CORE_TOOLS: &str = "Task,TaskStop,ToolSearch,AskUserQuestion,Bash,Read,Edit,Write,NotebookEdit,EnterPlanMode,ExitPlanMode,Skill,WebFetch,WebSearch";
/// `--boss-pick`: the stronger worker the lead may hand a hard change to.
const BOSS_STRONG: &str = "sonnet";

/// `--boss-pick`: the lead picks each worker call's effort, and for a hard change the stronger worker, instead of every
/// call running at one level. The levels come from what each worker's model takes (`models::claude_levels`).
fn boss_pick_rule(worker: &str, strong: &str) -> String {
    let levels = |m: &str| {
        let (all, default) = crate::models::claude_levels(m);
        format!("{m}: {} (default {default})", all.join(", "))
    };
    format!("Choose each worker call's effort with the Task tool's effort parameter; the levels each model takes: {}; {}. \
Use low for a lookup, a listing, a mechanical edit (a rename, a one-line change) or running a named check; medium for \
ordinary reading, edits and checks; high for logic that is easy to get wrong (parsing, concurrency, boundaries, a change \
across several files). Never xhigh or max for a worker. A change that is hard even at high (a subtle algorithm, a \
cross-module design, a fix that already failed once) goes to implementer-strong ({strong}) at medium or high instead.", levels(worker), levels(strong))
}

/// `--boss-escalate`: the stronger worker only redoes what the implementer got wrong. Boss runs whose implementer needed a
/// second attempt scored 5.83 against 7.04 on Haiku alone, 6.75 with Sonnet available (2026-11-03); the lead choosing
/// up front (`--boss-pick`) sent 17 of 21 first attempts to Sonnet, at twice the cost.
fn boss_escalate_rule(strong: &str) -> String {
    format!("A fourth subagent, implementer-strong, runs on a stronger model ({strong}) and costs several times as much. Never give it a first attempt: every change goes to the implementer first. Use implementer-strong only to redo a change the implementer got wrong (its checks still fail after its attempt, the tester reports failures it caused, or tazuna sends back failing tests), and give it your original instructions, what the implementer did, and the failure output.")
}

/// Boss mode's workers' effort when `--boss-effort` does not say (2026-11-02, six tasks x two runs, Opus medium leading:
/// Haiku at medium judge 6.79, $0.45 a run; high 6.58, $0.67; xhigh 6.55, $1.02; no boss 6.42, $0.70).
const BOSS_EFFORT: &str = "medium";

/// `--batch-calls`: fewer turns, each of which re-reads the whole conversation (a run's cost is about a third reading back
/// the cache, 2026-11-02). Measured on DeepSeek in the own loop only (TAZUNA_BATCH: rclone 39 against 77 turns, caddy-unix
/// 71 against 60).
const BATCH_RULE: &str = "Make independent tool calls together in one turn: read all the files you need at once, and run commands that do not depend on each other at once, instead of one call per turn. Only wait for a result when the next call depends on it.";
/// `--history`: one of the gaps to the human authors on company tickets (2026-11-03) was what they knew from earlier work
/// (why a sync has an echo guard, what a field's format is); the repository's history holds some of it. Only where there is
/// history to read. Under measurement.
const HISTORY_RULE: &str = "Before you change a file, read why it is the way it is: `git log -n 10 --format='%h %ad %s%n%b' -- <file>` (commit messages often carry the pull request's description) and `git blame -L <start>,<end> <file>` for the lines you change. Earlier decisions and their reasons are there; keep them unless the task says otherwise.";

/// `--test-scope`: package and repo-wide test runs took 37% of a no-boss run's time on tasks/ (5.0 of 13.6 minutes,
/// 2026-11-02), and tazuna runs the changed packages' tests itself when claude finishes (verify). Default since 0.1.40:
/// same judge, -17% cost, 40-58% faster (2026-11-03).
const CALLERS_RULE: &str = "Before you change a function, type or option, find every place that uses it and every code path that reaches it (other commands and entry points too), and make your change cover all of them.";

/// Experiment (TAZUNA_DIRECT_ANSWERS, 2026-11-14): on the general set (math, writing, guidance...) tazuna trailed Claude
/// Code by 0.3 and the model alone by 0.5, the judge citing remarks no other harness made ("not touching a repository"
/// above a recipe, "did not run code or change files" under a spreadsheet answer, a word-count footer on an essay).
const DIRECT_RULE: &str = "When a request is not about this repository's code, answer it directly: no remarks about files, tools, the repository or what you did or did not run, and keep to any length or form the request sets.";

/// Experiment (TAZUNA_ANSWER_RULES): how to answer a request that asks for no change.
const ANSWER_RULE: &str = "When the request asks for an answer, a review or a plan rather than a change, the rules about changing code do not apply. Base every claim on code you read, with file:line, and say what you did not check. In a review, look past the diff: callers, data and formats written before the change, error and retry paths, and what the change stops doing; call a blocker a blocker. In a plan, recommend the change that solves the reported problem with the least risk for this codebase, and weigh broader fixes as alternatives.";

/// Experiment (TAZUNA_PRECISE_ANSWERS, 2026-11-15): the held-out answer tasks, read after the own loop trailed OpenCode
/// there (5.93 against 6.80): its notes said "invents a story", "unsupported claims", "states jiter-internal behaviour as
/// fact with no caveat", "never identifies why the store is dirty". OpenCode's: "honest about limits", "nothing
/// invented". Same effort and reading; the own loop asserted more and traced less. Measured on Mistral (xhigh), 15 dev
/// answer tasks x 2 per arm: judge 7.67 to 7.60 (-0.07, CI -0.60 .. +0.33), precision 3.83 to 4.07, questions up
/// (6.9 to 7.6), plans down (plan-astro-18399 8.0 to 5.5); 10% cheaper, 15% less output. Stays off.
const PRECISE_ANSWER_RULE: &str = "When the request asks for an answer rather than a change: state as fact only what you read in the code or saw a command print, with file:line; call anything else (library internals, behaviour you did not run) unverified, or leave it out. For a cause, follow the chain to its start: say why each step happens, not only where. Keep to the length the request asks for.";

/// The chat profile's whole prompt: the model alone, as the general set's baselines ran it (plus the date).
const CHAT_PROMPT: &str = "You are a helpful assistant.";

/// The chat's hand-off (2026-11-21, on by default): a chat has no tools, so a question about live data or this computer
/// got a guess or "I cannot browse" (the user asked a chat for the weather and the crypto market). With this line the model
/// itself says that it needs to look something up (a word list cannot know every such question), and tazuna gives the
/// message to the agent (`hand_off`). Measured with bench/handoff.py on Opus 5.5, DeepSeek V4.1 Flash, GPT-6.2 Sol and
/// ERNIE: 266 of 268 live or machine questions handed off, 0 of 510 ordinary ones; ordinary answers unchanged on Opus,
/// DeepSeek and Sol, ERNIE's about 0.5 worse (accepted). An earlier wording ("reply with exactly [[agent]] and nothing
/// else") made every model's ordinary answers shorter (Sol judge 8.46 to 8.04): this one names the rule as an exception
/// and says that it changes nothing else. TAZUNA_CHAT_HANDOFF=0 leaves it out (benchmarks of the plain model).
const CHAT_HANDOFF: &str = "One exception to how you answer: if a good answer needs something you cannot know from your training alone \
(anything current or live: prices, weather, news, scores, schedules, recent releases or events; the content of a web page or URL; or \
anything about the user's own computer, files or accounts), your whole reply is [[agent]], and an agent that can look it up takes \
the question. A question whether you can see, check or reach such things (\"do you have access to my files?\", \"can you browse the \
web?\") counts too: the agent can, so that reply is [[agent]] as well. The exception changes nothing else: answer every other question just as you would without it, with the same length \
and detail.";

/// A chat reply that is a hand-off starts with this (trimmed); one that only mentions it later is an answer.
pub const HANDOFF: &str = "[[agent]]";

/// tazuna's line over an answer the agent gave in a chat's place.
pub const HANDOFF_NEEDS: &str = "answered by the agent: needs live data or this computer";
/// One the user sent there (`/agent`, the window's "send to agent") in a chat-profile session.
pub const HANDOFF_SENT: &str = "answered by the agent";
/// A question in a turn a chat handed over: allowed once or not, never "always" (that turn's process is gone after it).
pub const HANDOFF_ONCE: &str = "once only: the agent is answering a chat's question";

/// The chat profile's system prompt (both paths, and the router's chat answers): the one line, the date, the hand-off line.
pub fn chat_prompt(today: &str) -> String {
    let mut p = format!("{CHAT_PROMPT} Today is {today}.");
    if std::env::var_os("TAZUNA_CHAT_HANDOFF").is_none_or(|v| v != "0") {
        p.push(' ');
        p.push_str(CHAT_HANDOFF);
    }
    p
}

/// A chat's reply that hands its message to the agent.
pub fn hands_off(reply: &str) -> bool {
    reply.trim_start().starts_with(HANDOFF)
}

/// A chat message with the exchanges before it that its conversation does not hold (question, answer): a router chat
/// answer's follow-up (main.rs), and a chat's next message after the agent answered in its place (`start_run`).
pub fn with_earlier(text: &str, earlier: &[(String, String)]) -> String {
    let mut m: String = earlier.iter().map(|(q, a)| format!("Earlier in this chat you were asked: {}\nYou answered: {}\n\n",
        q.chars().take(1500).collect::<String>(), a.chars().take(3000).collect::<String>())).collect();
    if !m.is_empty() {
        m.push_str("The new message: ");
    }
    m + text
}

/// The exchanges a switch carries to a model that cannot read the conversation's history (`switch`): the latest that fit
/// in about 40k characters as `with_earlier` cuts them, a message without an answer said so.
#[inline(never)]
pub fn carried(conv: &[(String, String)]) -> Vec<(String, String)> {
    let mut room = 40_000usize;
    let from = conv.iter().rposition(|(q, a)| {
        let n = q.chars().count().min(1500) + a.chars().count().min(3000);
        let over = n > room;
        room = room.saturating_sub(n);
        over
    }).map_or(0, |i| i + 1);
    conv[from..].iter().map(|(q, a)| (q.clone(), if a.is_empty() { "(no answer)".into() } else { a.clone() })).collect()
}

/// Experiment (TAZUNA_ANSWER_REVIEW, 2026-11-15): the self-review turn (`--self-review`) runs only after a change, so a plan
/// or a review (the weakest kinds: 3-7 against 8-9 for questions) never got one. This turn follows an answer that changed
/// nothing. Measured on Mistral's DeepSeek (5 dev plans): judge 7.10 to 7.40 (5 runs), acceptable 80% to 60%, billed
/// $0.21 to $0.93 a plan; effective cost $1.54 to $3.60 (docs/WORKING.md). Stays off.
const ANSWER_REVIEW: &str = "Before you finish, check your answer against the code: re-open the places you cite and confirm each claim, look for what you missed (callers, other code paths, data written before, error and retry cases, a simpler or safer fix), and correct anything wrong. Then give your final answer in full, as the reply to the original request, not as a list of changes to the earlier one.";

const COVER_ALL_RULE: &str = "Make your change, and its tests, cover every command, flag or API the task names, and each one's error cases, not only the main path: read the code behind each one before you change it.";

const WRITE_TESTS_RULE: &str = "Add or update tests for the behavior you change, including its edge cases: tazuna's check only runs the tests, it does not write them.";

const TEST_SCOPE_RULE: &str = "While working, run only the tests that cover your change (in Go: `go test -run 'TestA|TestB' ./pkg/`), not whole packages or the whole repository: when you finish, tazuna runs the changed packages' tests and sends you any failure.";
/// Where verify does not cover the repo (JS workspaces since 0.1.120), no rule kept the agent's own checks narrow: on the
/// suite's Astro change tasks 74% of tazuna's time was its shell commands (a repo-wide `pnpm lint:ai` of 224 s, a biome
/// run of 213 s, full rebuilds, the whole unit suite, stashing the fix to rebuild and watch a test fail first), as with
/// Claude Code. With this rule (0.1.122): 27% less time on 5 dev change tasks and 35% on the 2 held-out ones, same
/// cost, hidden tests 16/21 against 17/21 (the miss on a task every harness fails a third of the time).
/// TAZUNA_WIDE_CHECKS turns it off (bench baselines).
const NARROW_CHECKS_RULE: &str = "Keep your checks narrow: run the test files that cover your change, and lint or type-check only the files you changed. Build only what those tests need. Do not run the whole test suite, a repository-wide lint or format check, or extra builds just to watch a test fail before your fix.";
/// Experiment (TAZUNA_QUICK_BUILD, 2026-11-14): with narrow checks, full builds were still 47% of tazuna's time on the
/// Astro change tasks (3.3 a run, 169 s): `pnpm build` there also runs `tsc -b` and astro-check, where the unit tests
/// need only the transpile (`build:ci`). OMP, whose shell could not start pnpm, called the transpile step directly and
/// took 59-107 s on three tasks tazuna took 212-385 s on, with the same hidden tests passing. Measured (5 dev change
/// tasks x 3, side by side): full builds 140 to 23 s a run, but quick builds took 44 s, turns +3, output +19%; time
/// 314 to 330 s (one run's 480 s of hanging repro scripts), cost +16%, hidden tests 12/15 both. Stays off.
const QUICK_BUILD_RULE: &str = "When a test needs compiled output, build with the package's quickest script that produces it (a transpile-only or CI build, if there is one) while you work; run the full build or type check once, at the end.";
const FINISH_RULES: &str = "Finish only when the task is fully done: every requirement and acceptance criterion in the request is met and checked. Do not stop to ask for confirmation or leave steps for the user. Before relying on a name you have not read (a database column, function, field, API), read where it is defined.";

/// `--explorer`: reading code is most of a run's cost (each turn re-reads everything read so far at the main model's
/// rate), so a cheaper subagent answers broad questions in short. Until 0.1.121 every lookup went to it ("Do not explore
/// the code yourself"): on Opus a two-file question waited 25 s on Haiku (52 s against stock Claude Code's 22 s). Now the
/// main model reads what it can find itself and sends only broad searches: 12 dev tasks of the suite took 36% less time
/// (24.5 to 15.8 min, level with Claude Code's 15.6) at 13% more cost (still 23% under Claude Code), judge 7.75 to 7.63.
/// Experiment (TAZUNA_EXPLORER_STRICT): 0.1.120's rule, for measuring where delegating every lookup pays off (hard tasks).
const EXPLORER_STRICT_RULE: &str = "Do not explore the code yourself: no Read, cat, sed, head, tail, grep, rg or find to look around. Send every question about the code to the explorer subagent (Task tool, subagent_type explorer); it reads and searches the repo and answers with file paths, line ranges and the lines that matter. Ask several questions in parallel when they are independent. Read a file yourself only right before you edit it, and only the part you edit.";
const EXPLORER_RULE: &str = "Read the code yourself when you know where to look (a file or two, or a function you can find with one grep). Send broader questions, such as where something is used across the repo or how a flow runs through many files, to the explorer subagent (Task tool, subagent_type explorer), several in parallel when they are independent; it answers with file paths, line ranges and the lines that matter.";

/// `--explorer MODEL@URL`: the same rule for the explore tool, a cheap model on tazuna's own loop (`tazuna explore-mcp`).
const EXPLORE_TOOL_RULE: &str = "Do not explore the code yourself: no Read, cat, sed, head, tail, grep, rg or find to look around. Send every question about the code to the explore tool (mcp__explore__explore); it searches and reads the repo and answers with file paths, line ranges and the lines that matter. Ask several questions in parallel when they are independent. Read a file yourself only right before you edit it, and only the part you edit.";

/// `--tester`: test and type-check output is long and stays in the main model's context for every later turn; a cheaper
/// subagent runs the checks and reports only what failed.
const TESTER_RULE: &str = "Do not run tests, type checks, builds or linters yourself. Ask the tester subagent (Task tool, subagent_type tester) to run them, naming the command or the files to check; it reports only the failures, with the lines that explain them. Run independent checks in parallel.";

/// `--boss`: the main model only directs. Reading, editing and running commands are blocked for it by a hook
/// (`tazuna boss-hook`); subagents on a cheaper model do that work.
const BOSS_RULE: &str = "You are the lead engineer. You do not touch the repository yourself: reading, searching, editing and running commands are blocked for you. Three subagents on a cheaper model do that work (Task tool):
- explorer: answers precise questions about the code with file paths, line numbers and the lines that matter.
- implementer: makes the code changes you specify. Give it exact instructions: files, functions, what to change and why, edge cases, tests to add or update.
- tester: runs the tests, type checks or builds you name and reports only the failures.
Work like this: ask the explorer until you understand the task and the code (independent questions in parallel); decide the change and hand it to the implementer with precise instructions (parallel implementers only for parts in different files); have the tester run the relevant checks; review the reports and delegate fixes until the task is done. Keep your own messages short.";

/// `tazuna boss-hook`: denies the main agent every tool but delegation (subagent calls carry an `agent_id`).
pub fn boss_hook() -> Result<()> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let v: Value = serde_json::from_str(&input).unwrap_or(Value::Null);
    if let Some(reason) = boss_denies(&v) {
        println!("{}", json!({"hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": "deny", "permissionDecisionReason": reason}}));
    }
    Ok(())
}

fn boss_denies(v: &Value) -> Option<&'static str> {
    const LEAD: [&str; 8] = ["Agent", "Task", "TaskOutput", "TaskStop", "TodoWrite", "ToolSearch", "ScheduleWakeup", "ExitPlanMode"];
    let tool = v["tool_name"].as_str().unwrap_or_default();
    (v.get("agent_id").is_none() && !LEAD.contains(&tool))
        .then_some("You are the lead: delegate this. Ask the explorer to read or search, the implementer to edit, the tester to run checks.")
}

/// The workers' half of test scope: the boss's implementer spent most of its 10 minutes a run in package-wide tests
/// while the lead waited (and lost its cache), 2026-11-03.
const WORKER_TEST_SCOPE: &str = " Run only the tests that cover the change (in Go: `go test -run 'TestA|TestB' ./pkg/`), not whole packages or the whole repository: tazuna runs the changed packages' tests at the end.";

/// More than one commit to read: a fresh repository or a benchmark's one-commit copy has no history worth a rule.
fn has_history(cwd: &Path) -> bool {
    Command::new("git").args(["rev-list", "--count", "--max-count=2", "HEAD"]).current_dir(cwd).output()
        .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<u32>().is_ok_and(|n| n > 1))
}

/// The `--agents` JSON for the subagents that are on (`implementer` only in `--boss`). `effort`: theirs, else they run
/// at the session's (Claude Code's rule for a subagent without one: a session at xhigh ran its Haiku workers at xhigh).
/// `scope`: the workers that run tests are told to run only those covering the change (test scope).
fn subagents(explorer: Option<&str>, tester: Option<&str>, implementer: Option<&str>, effort: Option<&str>, strong: Option<&str>, scope: bool) -> String {
    let mut agents = serde_json::Map::new();
    if let Some(model) = strong {
        agents.insert("implementer-strong".into(), json!({
            "description": "A stronger, dearer implementer: makes a code change the lead engineer specifies (a hard one, or one the implementer got wrong) and reports what it did.",
            "prompt": "You implement a hard change for a lead engineer. Read the code you change and its callers first. Follow the instructions; if one is wrong in a concrete detail, adapt minimally and say so. Run the checks it names. Report briefly: files changed, what you did, anything you could not do or are unsure about.",
            "tools": ["Read", "Grep", "Glob", "Bash", "Edit", "Write"],
            "model": model,
        }));
    }
    if let Some(model) = implementer {
        agents.insert("implementer".into(), json!({
            "description": "Makes code changes exactly as the lead engineer specifies and reports what it did.",
            "prompt": "You implement changes for a lead engineer. Follow the instructions exactly; read the code you change first. If an instruction is wrong in a concrete detail (a name, a path, a signature), adapt minimally and say so. Run any quick check it names. Report briefly: files changed, what you did, anything you could not do or are unsure about.",
            "tools": ["Read", "Grep", "Glob", "Bash", "Edit", "Write"],
            "model": model,
        }));
    }
    if let Some(model) = explorer {
        agents.insert("explorer".into(), json!({
            "description": "Finds and reads code to answer a precise question about the repo; returns file paths, line ranges and the facts that matter, not whole files.",
            "prompt": "You explore a codebase for another agent. Answer its question with file paths, line numbers and short quotes of the lines that matter: complete, but brief. Do not edit anything.",
            "tools": ["Read", "Grep", "Glob", "Bash"],
            "model": model,
        }));
    }
    if let Some(model) = tester {
        agents.insert("tester".into(), json!({
            "description": "Runs tests, type checks, builds or linters and reports only the failures.",
            "prompt": "You run checks for another agent: the tests, type checks, builds or linters it names, using the repo's own commands. Report the command you ran, the pass and fail counts, and for each failure the test or file, the line, and the few error lines that explain it. Never paste full output. Do not edit anything.",
            "tools": ["Bash", "Read", "Grep", "Glob"],
            "model": model,
        }));
    }
    if let Some(e) = effort {
        for a in agents.values_mut() {
            a["effort"] = json!(e);
        }
    }
    if scope {
        for (_, a) in agents.iter_mut().filter(|(name, _)| name.as_str() != "explorer") {
            a["prompt"] = json!(format!("{}{WORKER_TEST_SCOPE}", a["prompt"].as_str().unwrap_or_default()));
        }
    }
    serde_json::Value::Object(agents).to_string()
}

/// `--self-review`: sent once when claude first says it is done and the run changed files.
const REVIEW: &str = "Before you finish, review your own change:
1. Run `git diff` and read all of it.
2. Go through every requirement and acceptance criterion in the task. For each, name the code that meets it, or fix the gap.
3. Revert changes the task did not ask for.
4. Check names and assumptions you have not read in the code (columns, functions, fields, APIs): read them now.
5. If you changed anything, run the relevant tests again.
Then finish with a short summary of what changed.";

/// Images a message names (a pasted or dragged-in path, `@shot.png`) go along as image blocks; plain text otherwise.
fn user_content(text: &str, cwd: &Path) -> Value {
    const MAX_IMAGE: u64 = 5 * 1024 * 1024; // the API's limit per image
    let images: Vec<Value> = words(text)
        .iter()
        .map(|w| w.trim_start_matches('@'))
        .filter_map(|w| {
            let ext = w.rsplit('.').next()?.to_ascii_lowercase();
            let media = match ext.as_str() {
                "png" => "image/png",
                "jpg" | "jpeg" => "image/jpeg",
                "gif" => "image/gif",
                "webp" => "image/webp",
                _ => return None,
            };
            let path = if Path::new(w).is_absolute() { PathBuf::from(w) } else { cwd.join(w) };
            std::fs::metadata(&path).ok().filter(|m| m.is_file() && m.len() <= MAX_IMAGE)?;
            let data = std::fs::read(&path).ok()?;
            Some(json!({"type": "image", "source": {"type": "base64", "media_type": media, "data": base64(&data)}}))
        })
        .collect();
    if images.is_empty() {
        return json!(text);
    }
    let mut parts = images;
    parts.push(json!({"type": "text", "text": text}));
    Value::Array(parts)
}

/// A message's words for finding the files it names: a quoted path ("Screenshot 2026-11-16 at 10.00.png", as the
/// desktop app inserts one) or a terminal's escaped one (Screenshot\ 2026...png, as a drag into the terminal gives) is one
/// word, spaces and all.
fn words(text: &str) -> Vec<String> {
    let (mut out, mut cur, mut quote, mut chars) = (vec![], String::new(), None, text.chars().peekable());
    while let Some(c) = chars.next() {
        match (quote, c) {
            (None, '"' | '\'') if cur.is_empty() || cur == "@" => quote = Some(c),
            (Some(q), c) if c == q => quote = None,
            // an escaped space only: a Windows path's backslashes stay
            (None, '\\') if chars.peek().is_some_and(|n| n.is_whitespace()) => cur.extend(chars.next()),
            (None, c) if c.is_whitespace() => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            (_, c) => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            out.push(if i <= c.len() { T[(n >> shift & 63) as usize] as char } else { '=' });
        }
    }
    out
}

/// claude's slash commands by folder, as its last init there listed them (in the data directory).
const CLAUDE_COMMANDS: &str = "claude-commands.json";

/// The commands kept for `cwd` in `file`, if any.
fn claude_commands(file: &std::path::Path, cwd: &str) -> Option<Vec<String>> {
    let all: Value = serde_json::from_slice(&std::fs::read(file).ok()?).ok()?;
    serde_json::from_value(all.get(cwd)?.clone()).ok()
}

/// Keeps `cmds` for `cwd` in `file`, beside the other folders' lists; unchanged lists are not written again.
fn save_claude_commands(file: &std::path::Path, cwd: &str, cmds: &[String]) {
    let mut all: Value = std::fs::read(file).ok().and_then(|b| serde_json::from_slice(&b).ok()).filter(Value::is_object).unwrap_or_else(|| json!({}));
    if all[cwd] == json!(cmds) {
        return;
    }
    all[cwd] = json!(cmds);
    let _ = std::fs::write(file, all.to_string());
}

/// What "always" covers: one exact command for shells (a later command can be anything), the whole tool otherwise.
fn always_key(req: &Value) -> String {
    let tool = req["request"]["tool_name"].as_str().unwrap_or_default();
    match always_scope(tool) {
        "this command" => format!("{tool}\u{0}{}", req["request"]["input"]["command"].as_str().unwrap_or_default()),
        _ => tool.to_string(),
    }
}

/// How far "always" reaches for a tool, as the question shows it.
fn always_scope(tool: &str) -> &'static str {
    if matches!(tool, "Bash" | "mcp__lean__bash" | "PowerShell" | "bash") { "this command" } else { "this tool" }
}

/// The run-log prompt of a run claude started on its own.
const BACKGROUND_PROMPT: &str = "(claude continued on its own after a background command finished)";

/// The first event of a model turn.
fn starts_turn(e: &Value) -> bool {
    (e["type"] == "stream_event" && e["event"]["type"] == "message_start") || e["type"] == "assistant"
}

/// The cap on paid API spend (the own loop) in live sessions, in dollars per 24 hours: ~/.config/tazuna/spend-cap, "off"
/// for none, $1 when the file is missing or unreadable. Free models spend nothing against it.
pub fn spend_cap() -> Option<f64> {
    let text = crate::platform::home().and_then(|h| std::fs::read_to_string(h.join(".config/tazuna/spend-cap")).ok());
    match text.as_deref().map(str::trim) {
        Some("off") => None,
        Some(t) => Some(t.parse::<f64>().ok().filter(|v| v.is_finite() && *v >= 0.0).unwrap_or(1.0)),
        None => Some(1.0),
    }
}

/// Paid spend in the run log since `since` (seconds): the own loop's runs, whose log entry says claude_version null.
pub fn paid_since(log: &str, since: u64) -> f64 {
    log.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|r| r["ts"].as_u64().is_some_and(|t| t >= since) && r["meta"].get("claude_version").is_some_and(Value::is_null))
        .filter_map(|r| r["cost_usd"].as_f64()).sum()
}

/// A message the user sent, as a point /rewind can go back to.
#[derive(Clone)]
pub struct Checkpoint {
    /// The id tazuna gave the message (claude keeps it): what `rewind_files` takes.
    pub uuid: String,
    pub text: String,
    /// The last reply before the message: a conversation rewind resumes there (None: before the first message).
    pub before: Option<String>,
    /// The exchanges that went in front of it (the first message after a switch): a retry sends them again.
    pub carry: Vec<(String, String)>,
    /// Its images were read and sent (a typed message's; never tazuna's own text): a retry sends it the same way.
    pub attach: bool,
    /// The switches to a model that could not take the history before it (`switches`): one from before the last is not in
    /// the history of the session now.
    pub epoch: u64,
}

/// A random version-4 UUID (claude takes the message ids it is given, and they must be UUIDs).
fn new_uuid() -> String {
    let mut b = crate::platform::random::<16>();
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

/// A background task claude runs (a shell command, a Monitor, a subagent), for the ctrl-b view.
#[derive(Clone)]
pub struct BgTask {
    pub id: String,
    /// claude's task_type: local_bash, local_agent, monitor, ...
    pub kind: String,
    pub description: String,
    /// running, completed, failed, killed, ...
    pub status: String,
    pub started: Instant,
    pub ended: Option<Instant>,
    /// The end of the output (at most 8 KiB) the last `get_task_output` returned.
    pub output: String,
    /// A background subagent's type ("explorer") and its progress as claude reports it: tokens, tool calls, what it is
    /// doing now ("Running Show Ledger tick workflow").
    pub agent: String,
    pub tokens: u64,
    pub tools: u64,
    pub doing: String,
    /// A subagent's calls (the first, then each SendMessage that continued it) and the prompt of each: its messages come
    /// under the call's id (`Session::sub_events`).
    pub calls: Vec<(String, String)>,
}

enum Msg {
    Line(String),
    /// stderr closed: the process is exiting.
    Exited(String),
    /// A verification finished.
    Verified(verify::Outcome),
    /// The independent reviewer finished.
    Reviewed(review::Review),
}

pub struct Live {
    pub model: String,
    pub effort: String,
    cwd: PathBuf,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    /// Messages carry the process generation, so output from a killed process is ignored.
    tx: Sender<(u64, Msg)>,
    rx: Receiver<(u64, Msg)>,
    session_id: Option<String>,
    /// From claude's init event: its built-in prompt changes between versions, which moves token counts.
    claude_version: Option<String>,
    /// From claude's init too: on the Claude plan, and who is paid when not (models::claude_payer), for the run log.
    claude_bill: (bool, Option<&'static str>),
    /// When this window started: a tazuna binary newer than this is a rebuild the window does not run yet.
    opened: std::time::SystemTime,
    generation: u64,
    /// When an interrupt was requested and its run has not ended yet.
    interrupting: Option<Instant>,
    interrupts_sent: u64,
    rec: File,
    pub rec_path: PathBuf,
    t0: Instant,
    /// Runs in flight, oldest first (one per message; claude answers them in order).
    runs: VecDeque<Run>,
    /// claude reports cost cumulatively per process; this is the last total seen.
    proc_cost: f64,
    runs_log: PathBuf,
    /// "live" or "headless", recorded in the run log.
    pub mode: &'static str,
    /// Prompt cache lifetime for claude (`--cache-ttl`); None picks with `ttl()`.
    pub cache_ttl: Option<String>,
    /// Put a repo map in front of the session's first message (`--repo-map`); `mapped` once it went out.
    pub repo_map: bool,
    mapped: bool,
    /// Model of the explorer subagent (`--explorer sonnet`), repo profile only.
    pub explorer: Option<String>,
    /// `--explorer MODEL@URL`: (url, key env, model) of the explore tool's provider, instead of a subagent.
    pub explorer_provider: Option<(String, String, String)>,
    /// Where the explore tool writes the cost of each answer.
    explorer_costs: PathBuf,
    /// Model of the tester subagent (`--tester haiku`), repo profile only.
    pub tester: Option<String>,
    /// `--boss MODEL`: the main model only directs; explorer, implementer and tester subagents on MODEL do the work.
    pub boss: Option<String>,
    /// `--boss-effort LEVEL`: the boss's workers at LEVEL; unset, at BOSS_EFFORT (medium).
    pub boss_effort: Option<String>,
    /// `--boss-pick`: the lead picks each worker call's effort and, for a hard change, the stronger worker.
    pub boss_pick: bool,
    /// `--boss-escalate`: a stronger worker (Sonnet) that only redoes what the implementer got wrong. Under measurement.
    pub boss_escalate: bool,
    /// `--advisor MODEL`: Claude Code's advisor tool; the main model can ask MODEL for advice when it is stuck.
    pub advisor: Option<String>,
    pub verify: verify::Mode,
    /// Passed to the tool server: shrink bash output above this many chars (0: off).
    pub bash_cap: usize,
    /// Type/compile errors after each edit (on by default, `--no-diagnostics`): the hook's state directory once started.
    pub diagnostics: bool,
    /// Finish rules in the prompt plus one self-review turn per run (`--self-review`).
    pub self_review: bool,
    /// The repo profile with Claude Code's coding tools only (`--repo-tools core`).
    pub repo_tools_core: bool,
    /// `--batch-calls`: claude is told to make independent tool calls in the same turn. Under measurement.
    pub batch_calls: bool,
    /// Claude told to run only the tests covering its change while it works (verify runs the rest); `--no-test-scope`: off.
    pub test_scope: bool,
    /// `--lessons`: the repo's review lessons (`tazuna lessons`, from tazuna's data folder) go into the instructions, marked
    /// as notes. Under measurement.
    pub lessons: bool,
    /// `--history`: claude is told to read the git history of the files it changes. Under measurement.
    pub history: bool,
    /// `--rtk`: claude's Bash commands go through rtk (`rtk hook claude`), which shortens their output. Under measurement.
    pub rtk: bool,
    /// Permission mode as claude names it: bypassPermissions (tazuna's "full"), default ("ask"), acceptEdits ("edits"),
    /// plan; and tazuna's own "auto": claude runs as acceptEdits and tazuna answers its questions itself, except for the
    /// destructive commands (risky.rs), which ask (`auto_allows`). Every mode but full routes claude's permission
    /// questions to tazuna (`--permission-prompt-tool stdio`); the own loop takes "auto" by name (engine.rs `permit`).
    pub permission_mode: String,
    /// Review mode (tazuna's "review"): acceptEdits underneath, and a reviewer model answers the agent's command requests
    /// in the user's place (main.rs review_pending); a command it allows runs in the sandbox (sandbox.rs).
    pub reviewer: bool,
    /// Questions waiting for the user's answer: claude's `can_use_tool` control requests. The UI shows the front one
    /// from here, so what is shown and what an answer applies to cannot drift apart.
    pub pending: VecDeque<Value>,
    /// Where claude's lean bash is: it keeps its folder between calls (mcp.rs), so a `cd ..` in one call and an rm in the
    /// next is read from there (risky.rs `scan`). None: somewhere tazuna cannot tell. A new process starts in `cwd`.
    shell_dir: Option<PathBuf>,
    /// Events produced outside `poll` (an interrupt answering questions, a dead process dropping them), handed out by
    /// the next `poll` so the screen hears about them.
    outbox: Vec<(f64, Value)>,
    /// What the user allowed for the rest of the session ("always"): `always_key` of each answer.
    allowed: std::collections::HashSet<String>,
    requests_sent: u64,
    /// Commands claude started that have not finished: claude wakes up again for background ones, so the session is
    /// not idle until they are done. Each with the subagent that started it, if one did: nobody wakes up for a shell
    /// that outlives its subagent, so it stops counting when the subagent ends (a search a subagent left running held a
    /// headless run 25 minutes after its final check passed, 2026-10-31).
    tasks: std::collections::HashMap<String, Option<String>>,
    /// Every background task of this session, oldest first: the ctrl-b view.
    pub bg: Vec<BgTask>,
    /// The messages the user sent in this process's conversation, oldest first: what /rewind offers.
    pub checkpoints: Vec<Checkpoint>,
    /// The tree at the last run's start and the type check baseline taken there (`start_run`).
    types_last: Option<(Option<std::collections::BTreeMap<String, String>>, crate::runlog::SharedBaseline)>,
    /// The uuid of claude's latest top-level reply.
    last_assistant: Option<String>,
    /// A conversation rewind waiting for the file rewind's answer: where to resume (None inside: start fresh).
    pending_resume: Option<Option<String>>,
    /// The next spawn resumes the session at this message, in a fork: claude's reply (`--resume-session-at`), or the own
    /// loop's typed message it goes back to before (`--resume-before`).
    resume_at: Option<String>,
    /// The next spawn continues `session_id` in a fork (a side chat: the original stays as it was).
    pub fork_next: bool,
    /// When the last of `tasks` finished.
    tasks_done: Option<Instant>,
    /// Independent reviewer: model and effort (`--review-model`, `--review-effort`).
    pub review_model: Option<(String, String)>,
    /// The reviewer's key when it runs on a provider's API (`--review-key-env`), read at start; the variable is out of the
    /// environment by then, so no process tazuna starts inherits it.
    pub review_key: Option<String>,
    /// Own loop instead of claude (`--provider`): base URL and the environment variable with the key.
    pub provider: Option<(String, String)>,
    /// The own loop's price ("in,cached,out" USD per M) for a route picked with /model (`--price` sets the whole process's).
    pub price: Option<String>,
    /// The user's last message and whether the reviewer has seen it: wake-up runs get no review of their own.
    task: String,
    task_reviewed: bool,
    diag_dir: Option<PathBuf>,
    /// Add the repo's CLAUDE.md / AGENTS.md to the system prompt (lean profile; the repo profile lets Claude Code load them).
    pub project_instructions: bool,
    pub profile: Profile,
    /// The profile before the last `/profile` or `/chat` switch: where `/chat` goes back to.
    pub prev_profile: Option<Profile>,
    /// A turn a chat handed to the agent runs (`hand_off`): the session's profile, conversation and last reply, back after it.
    handoff: Option<(Profile, Option<String>, Option<String>)>,
    /// A chat's exchanges, oldest first (question, answer: the chat's own, or the agent's in its place; `heard`), and how many
    /// of them its own conversation holds. The agent's answers after that go with the chat's next message (`start_run`); the
    /// router's next chat answer has the last (`handed`); a message handed to the agent carries the last few (`earlier`).
    said: Vec<(String, String)>,
    told: usize,
    /// This conversation's exchanges, oldest first (a message, its final answer or empty), in every profile (`track`); what
    /// a model that cannot read its history gets as text (`switch`).
    conv: Vec<(String, String)>,
    /// The exchanges the next new session's first message carries (`switch`, `start_run`).
    carry: Vec<(String, String)>,
    /// The session this one continues (a switch to a model that could not take its history, a retry, an edit from an
    /// earlier message): the run log's `continues`, by which the conversation lists show the two as one (main.rs sessions_of).
    continues: Option<String>,
    /// Every earlier session this conversation continues from, oldest first: the view's `earlier_sessions`, so the window
    /// knows the conversation by any of its ids (its session id is null after a switch until the new one starts).
    earlier: Vec<String>,
    /// Switches to a model that could not take the history (a checkpoint's `epoch`).
    switches: u64,
    /// `--data any` or `/data any`: providers off the trusted list may see the code (the model picker allows them).
    pub data_any: bool,
    /// Also load the user's personal ~/.claude settings (repo profile).
    pub user_settings: bool,
    /// `--connectors`: the user's MCP servers and claude.ai connectors without the rest of ~/.claude (repo profile).
    pub connectors: bool,
    /// Claude Code's Artifact tool (claude.ai artifacts: read, open), switched on by the CLI's own CLAUDE_CODE_ARTIFACT once
    /// the conversation holds an artifact link: its tools cost ~13.5k tokens a request, and print mode leaves them out, so
    /// a linked doc was out of reach and the docs connector refused it ("this kind of conversation cannot reach it",
    /// the user, 2026-11-23; their interactive Claude Code read the same link).
    artifacts: bool,
    /// (bytes included, source files) for the run log.
    instructions_used: (usize, Vec<String>),
    verifying: bool,
    /// Messages typed while tests or a handed-over turn run, and hand-offs waiting for the agent (Some: what it is sent, the
    /// line over its answer): sent in order once it is free (`send_held`).
    held: VecDeque<(String, bool, Option<(String, &'static str)>)>,
    /// claude's last result event and the last run-log line (headless mode reports them).
    pub last_result: Option<Value>,
    pub last_log: Option<Value>,
    /// Every run logged this session (headless sums them: claude may continue on its own after the first).
    pub logs: Vec<Value>,
    /// Router decision: lane name and why.
    lane: (String, String),
    /// Id of the last run written to the run log (what `/good` and `/bad` label).
    pub last_run: Option<String>,
    /// Model and effort for fix rounds after failed verification, and whether the session already switched.
    escalate: Option<(String, String)>,
    escalated: bool,
}

impl Live {
    pub fn new(model: &str, effort: &str) -> Result<Self> {
        let cwd = std::env::current_dir()?;
        let data = data_dir()?;
        let sessions = data.join("sessions");
        std::fs::create_dir_all(&sessions)?;
        let dir = cwd.file_name().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default();
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        // the pid keeps sessions started in the same second in same-named directories apart
        let rec_path = sessions.join(format!("{stamp}-{dir}-{}.jsonl", std::process::id()));
        let mut rec = File::create(&rec_path).with_context(|| format!("creating {}", rec_path.display()))?;
        // same header as recordings/record.py, so a live session replays like any recording
        writeln!(rec, "{}", json!({"t": 0, "prompt": "", "effort": effort, "model": model, "cwd": cwd}))?;
        let (tx, rx) = channel();
        Ok(Live {
            model: model.into(),
            effort: effort.into(),
            cwd,
            child: None,
            stdin: None,
            tx,
            rx,
            session_id: None,
            claude_version: None,
            claude_bill: (true, None),
            opened: std::time::SystemTime::now(),
            generation: 0,
            interrupting: None,
            interrupts_sent: 0,
            rec,
            rec_path,
            t0: Instant::now(),
            runs: VecDeque::new(),
            proc_cost: 0.0,
            runs_log: data.join("runs.jsonl"),
            mode: "live",
            cache_ttl: None,
            repo_map: false,
            mapped: false,
            explorer: None,
            explorer_provider: None,
            explorer_costs: std::env::temp_dir().join(format!("tazuna-explorer-{}-{:x}.costs", std::process::id(),
                SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0))),
            tester: None,
            boss: None,
            boss_effort: None,
            boss_pick: false,
            boss_escalate: false,
            advisor: None,
            verify: verify::Mode::Auto,
            bash_cap: crate::mcp::DEFAULT_BASH_CAP,
            diagnostics: false,
            self_review: false,
            repo_tools_core: false,
            rtk: false,
            batch_calls: false,
            test_scope: true,
            lessons: false,
            history: false,
            review_model: None,
            review_key: None,
            provider: None,
            price: None,
            task: String::new(),
            task_reviewed: false,
            tasks: Default::default(),
            bg: Vec::new(),
            checkpoints: Vec::new(),
            types_last: None,
            last_assistant: None,
            pending_resume: None,
            resume_at: None,
            fork_next: false,
            permission_mode: "bypassPermissions".into(),
            pending: VecDeque::new(),
            shell_dir: None,
            reviewer: false,
            outbox: Vec::new(),
            allowed: Default::default(),
            requests_sent: 0,
            tasks_done: None,
            diag_dir: None,
            project_instructions: true,
            profile: Profile::Lean,
            prev_profile: None,
            handoff: None,
            said: Vec::new(),
            told: 0,
            conv: Vec::new(),
            carry: Vec::new(),
            continues: None,
            earlier: Vec::new(),
            switches: 0,
            data_any: false,
            user_settings: false,
            connectors: false,
            artifacts: std::env::var_os("CLAUDE_CODE_ARTIFACT").is_some(),
            instructions_used: (0, Vec::new()),
            verifying: false,
            held: VecDeque::new(),
            last_result: None,
            last_log: None,
            logs: Vec::new(),
            lane: ("manual".into(), String::new()),
            last_run: None,
            escalate: None,
            escalated: false,
        })
    }

    pub fn elapsed_ms(&self) -> f64 {
        self.t0.elapsed().as_secs_f64() * 1000.0
    }

    /// The claude session this conversation is (None before claude's first reply, and on the own loop's engine). While a
    /// turn a chat handed over runs, the session's own one: that turn's conversation is thrown away after it, so neither the
    /// run log (`tazuna -c`, the conversation lists) nor the window may take it for the session's.
    pub fn session_id(&self) -> Option<&str> {
        self.handoff.as_ref().map_or(self.session_id.as_deref(), |h| h.1.as_deref())
    }

    /// Continue an earlier claude session (`--continue`, `--resume`): the first message resumes it instead of starting fresh.
    pub fn resume(&mut self, session_id: &str) {
        self.session_id = Some(session_id.to_string());
    }

    /// Sends a message, starting (or resuming) the claude process if needed. Returns the event to show. Image paths in
    /// it stay text: only `send_typed` attaches files.
    pub fn send(&mut self, text: &str) -> Result<(f64, Value)> {
        self.send_message(text, false)
    }

    /// A message the user typed: image files it names are read and sent as images. Never for text that came from
    /// claude, a command or a reviewer, which could name any file on the machine and bypass the permission mode.
    pub fn send_typed(&mut self, text: &str) -> Result<(f64, Value)> {
        // one of the user's commands (behave.rs): the own loop gets its text, claude its plugin's name for it
        if text.starts_with('/') {
            let mine = if self.claude_commands() { crate::behave::for_claude(text) } else { crate::behave::expand(&self.cwd, self.behave_scope(), text) };
            if let Some(t) = mine {
                return self.send_message(&t, true);
            }
        }
        self.send_message(text, true)
    }

    /// `/agent TEXT` and the window's "send to agent": the agent, in a chat-profile session too, where it is the lean agent
    /// in this folder for one turn (`hand_off`; until 2026-11-21 such a message went to the chat).
    #[inline(never)]
    pub fn send_agent(&mut self, text: &str) -> Result<(f64, Value)> {
        if self.profile != Profile::Chat {
            return self.send_typed(text);
        }
        // shown at once; the agent takes it now or once it is free (`hand_off`)
        self.within_cap()?;
        let e = self.record(json!({"type": "harness_user", "text": text}));
        let sent = with_earlier(text, self.earlier());
        self.hand_off(text, &sent, HANDOFF_SENT)?;
        Ok(e)
    }

    /// A chat's message for the agent (the chat replied `[[agent]]`, or the user sent it there): one turn of this folder's
    /// lean agent, whatever the session's profile, in a fresh conversation, sent `sent` (the message with the chat's
    /// exchanges before it, which the chat saw). One plain agent: no settings, hooks, skills, subagents, boss mode or project
    /// instructions, the web at hand (its fetch, claude's web search) and claude's own file tools left out (`spawn`). Only
    /// the web goes without a question, whatever the session's mode (`turn_mode`). It starts now when nothing runs, else it
    /// waits its turn (`held`); then `line` stands over its answer. After it the session's own conversation goes on
    /// (`hand_back`). Records nothing but its start: the message is the caller's.
    #[inline(never)]
    pub fn hand_off(&mut self, text: &str, sent: &str, line: &'static str) -> Result<()> {
        // a paid model's turn like any other: every path to the agent comes here (the router's skipped the cap)
        self.within_cap()?;
        self.held.push_back((text.to_string(), false, Some((sent.to_string(), line))));
        let out = self.send_held();
        self.outbox.extend(out);
        Ok(())
    }

    /// The last few of a chat's exchanges, for a message handed to the agent. ponytail: the last six; more if a hand-off's
    /// follow-ups need a longer past.
    fn earlier(&self) -> &[(String, String)] {
        &self.said[self.said.len().saturating_sub(6)..]
    }

    /// A turn a chat handed over runs: its questions are the user's (review mode's reviewer does not answer them).
    #[inline(never)]
    pub fn handing_off(&self) -> bool {
        self.handoff.is_some()
    }

    /// The handed-over turn is over: the session's profile and conversation again (its next message resumes it, with the
    /// agent's answer: `heard`), and the event that tells the screen the agent's process is gone (its cost count starts over
    /// with the next one).
    fn hand_back(&mut self) -> Option<(f64, Value)> {
        let (profile, id, last) = self.handoff.take()?;
        self.respawn(self.model.clone(), self.effort.clone());
        (self.profile, self.session_id, self.last_assistant) = (profile, id, last);
        Some(self.record(json!({"type": "harness_process_end"})))
    }

    /// Keeps a chat's exchanges (`said`, `told`) by a recorded event: a chat's result or a handed-over turn's (`tazuna_q`, its
    /// question, set in `poll`), and the line that starts another conversation. A resumed chat's recordings give them back
    /// the same way (`heard_before`).
    fn heard(&mut self, e: &Value) {
        self.track(e);
        if e["type"] == "harness_notice" && e["text"].as_str().is_some_and(|t| t.starts_with("── ")) {
            (self.said, self.told) = (Vec::new(), 0);
        }
        let Some(q) = e["tazuna_q"].as_str() else { return };
        let a = e["result"].as_str().filter(|a| e["subtype"] == "success" && e["is_error"] != true && !a.trim().is_empty());
        match (a, e["tazuna_handoff"] == true) {
            // the agent's answer in the chat's place: not in the chat's conversation yet
            (Some(a), true) => self.said.push((q.to_string(), a.to_string())),
            // the chat's answer, or its [[agent]]: its conversation holds the message, and the answers that went with it
            (Some(a), false) => {
                if !hands_off(a) {
                    self.said.push((q.to_string(), a.to_string()));
                }
                self.told = self.said.len();
            }
            (None, _) => {}
        }
    }

    /// A resumed chat's recorded events, replayed (`tazuna -c`, /resume, the window's list): the agent's answers its
    /// conversation does not hold yet go with its next message, as they would have before.
    #[inline(never)]
    pub fn heard_before(&mut self, e: &Value) {
        if self.profile == Profile::Chat {
            self.heard(e);
        } else {
            self.track(e);
        }
    }

    /// Keeps the conversation's exchanges (`conv`) by a recorded event: a message, a result (its answer, to the message it
    /// answered: `tazuna_prompt`), a new or resumed conversation.
    #[inline(never)]
    fn track(&mut self, e: &Value) {
        let text = e["text"].as_str().unwrap_or_default();
        match e["type"].as_str() {
            Some("harness_user") => self.conv.push((text.to_string(), String::new())),
            Some("harness_notice") if text.starts_with("── new conversation") || text.starts_with("── resumed session") => self.conv.clear(),
            Some("result") => {
                let Some(a) = e["result"].as_str().filter(|a| e["subtype"] == "success" && e["is_error"] != true && !a.trim().is_empty() && !hands_off(a)) else { return };
                // to its message; claude's own turn after a background command, or a recording from before, to the first
                // message without an answer, else the last
                let q = e["tazuna_prompt"].as_str();
                let i = self.conv.iter().rposition(|c| Some(c.0.as_str()) == q).or_else(|| self.conv.iter().position(|c| c.1.is_empty()));
                if let Some(c) = i.or(self.conv.len().checked_sub(1)).and_then(|i| self.conv.get_mut(i)) {
                    c.1 = a.to_string();
                }
            }
            _ => {}
        }
    }

    /// The router's chat answers (main.rs): the question a chat handed over and the agent's answer, once that turn is over,
    /// for a follow-up's context. A chat-profile session's next message carries them itself (`start_run`).
    #[inline(never)]
    pub fn handed(&mut self) -> Option<(String, String)> {
        // the session's profile, not the lean one a turn handed over runs in (the main loops ask at the chat's [[agent]] too)
        if self.handoff.as_ref().map_or(self.profile, |h| h.0) == Profile::Chat {
            return None;
        }
        let last = self.said[self.told..].last().cloned();
        self.told = self.said.len();
        last
    }

    /// The permission mode a process starts in, and the tools claude may use without a question: the session's, or in a
    /// handed-over turn ask mode with the web alone allowed (the own loop's `handoff` mode, engine.rs `asks`). A file read
    /// asks there too: with reads and fetches both allowed, a fetched page could have the agent read ~/.ssh or .env and send
    /// it out in a URL (review, 2026-11-21).
    fn turn_mode(&self) -> (&str, &'static str) {
        let lean = "mcp__lean__read,mcp__lean__write,mcp__lean__edit";
        match (self.handoff.is_some(), self.provider.is_some(), self.profile) {
            (true, true, _) => ("handoff", ""),
            (true, false, _) => ("default", "mcp__lean__fetch,WebSearch"),
            // reads and edits pre-allowed only in full and auto: the other modes ask about them too. bash is never
            // pre-allowed: it comes to tazuna, which lets it run in full and in auto unless it is a destructive command
            // (`auto_allows`); pre-allowed in full, it stayed so after a switch to auto (review, 2026-11-23)
            (false, false, Profile::Lean) if self.permission_mode == "bypassPermissions" => (&self.permission_mode, lean),
            (false, false, Profile::Lean) if self.permission_mode == "auto" => ("acceptEdits", lean),
            (false, false, _) if self.permission_mode == "auto" => ("acceptEdits", ""),
            (false, ..) => (&self.permission_mode, ""),
        }
    }

    /// claude runs commands and skills itself here (the repo profiles; the lean ones turn its slash commands off, so tazuna
    /// expands them and lists the skills in its own prompt, as on the own loop).
    fn claude_commands(&self) -> bool {
        self.provider.is_none() && matches!(self.profile, Profile::Repo | Profile::RepoLean)
    }

    /// The user's commands (behave.rs) that count here: name and what each does.
    pub fn my_commands(&self) -> Vec<(String, String)> {
        crate::behave::commands(&self.cwd, self.behave_scope()).into_iter().map(|c| (c.name, c.description)).collect()
    }

    /// The slash commands for the menu: the session's (tazuna's own, and claude's once its init came), and on Claude
    /// before that init the ones claude's last init in this folder listed (claude-commands.json), else its common
    /// built-in ones. Without them the window's menu held only /create and /agent before a first turn (the user,
    /// 2026-11-20).
    pub fn commands(&self, session: &[String]) -> Vec<String> {
        let mut all = session.to_vec();
        if self.provider.is_none() && session.iter().all(|c| crate::session::OWN_COMMANDS.contains(&c.as_str())) {
            let kept = crate::labels::dir().ok().and_then(|d| claude_commands(&d.join(CLAUDE_COMMANDS), &self.cwd.to_string_lossy()));
            all.extend(kept.unwrap_or_else(|| crate::editor::CLAUDE_COMMON.map(String::from).to_vec()));
        }
        all.sort();
        all.dedup();
        all
    }

    /// Whose commands and skills count (behave.rs), as claude's own settings sources would have them.
    pub fn behave_scope(&self) -> crate::behave::Scope {
        crate::behave::Scope { user: self.user_settings, project: matches!(self.profile, Profile::Repo | Profile::RepoLean) }
    }

    /// The own loop's model costs nothing: its price (the catalog's route, or --price) is 0 for input and output.
    fn free(&self) -> bool {
        let p = self.price.clone().or_else(|| std::env::var("TAZUNA_PRICE").ok()).unwrap_or_default();
        let v: Vec<f64> = p.split(',').filter_map(|x| x.trim().parse().ok()).collect();
        v.len() == 3 && v[0] == 0.0 && v[2] == 0.0
    }

    /// A paid model (the own loop) in a live session stops at the user's cap; benchmarks have the usage guard instead.
    fn within_cap(&self) -> Result<()> {
        if self.provider.is_some() && self.mode != "headless" && !self.free() && let Some(cap) = spend_cap() {
            let log = std::fs::read_to_string(&self.runs_log).unwrap_or_default();
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
            let spent = paid_since(&log, now.saturating_sub(86_400));
            if spent >= cap {
                bail!("API spend in the last 24 hours is ${spent:.2}, at your cap of ${cap:.2}: not sent. Raise the cap in the desktop's \
                    Settings or ~/.config/tazuna/spend-cap (\"off\" for none), or switch to a Claude model on your plan (/model)");
            }
        }
        Ok(())
    }

    #[inline(never)]
    fn send_message(&mut self, text: &str, attach: bool) -> Result<(f64, Value)> {
        self.within_cap()?;
        // held while the tests run, and while a turn a chat handed over runs: it goes to the session's own conversation after
        // that turn (`hand_back`), not into the one thrown away; and behind anything held before it
        // behind a hand-off that waits only for background commands, it starts now (send_held lets it pass)
        let behind = self.held.iter().any(|h| h.2.is_none()) || (!self.held.is_empty() && !self.runs.is_empty());
        if self.verifying || self.handoff.is_some() || behind {
            self.held.push_back((text.to_string(), attach, None));
        } else {
            self.start_run(text, attach, None)?;
        }
        Ok(self.record(json!({"type": "harness_user", "text": text})))
    }

    /// `sent`: what the model gets instead of `text` (a handed-over turn's message with the chat's exchanges before it).
    fn start_run(&mut self, text: &str, attach: bool, sent: Option<&str>) -> Result<()> {
        // the first artifact link on the claude path: claude again with its Artifact tool, resuming the session (one cache
        // rebuild, as a model switch)
        if self.provider.is_none() && !self.artifacts && (artifact_link(text) || self.conv.iter().any(|(q, _)| artifact_link(q))) {
            self.artifacts = true;
            if self.child.is_some() {
                let (m, e) = (self.model.clone(), self.effort.clone());
                self.respawn(m, e);
            }
        }
        let mut run = Run::new(text, self.elapsed_ms(), &self.cwd);
        // ponytail: taken while claude starts reading; an edit that lands first is counted as known, never as new. The
        // tree of the last run's start again (a question, not an edit): its baseline holds. One per message cost a
        // workspace repo 12 s and ~38 CPU-seconds each (2026-11-11). ponytail: the key is git's view of the tree, so an
        // `npm install` in between keeps the old baseline
        run.types_before = verify::typecheck_command(&self.verify, &self.cwd).map(|cmd| match &self.types_last {
            Some((tree, b)) if run.git_before.is_some() && *tree == run.git_before => b.clone(),
            _ => {
                let cwd = self.cwd.clone();
                std::sync::Arc::new(std::sync::Mutex::new(crate::runlog::Baseline::Pending(std::thread::spawn(move || verify::run_raw(&cwd, &cmd).1))))
            }
        });
        self.types_last = run.types_before.clone().map(|b| (run.git_before.clone(), b));
        // a new session after a switch: the conversation so far in front of its first message (`switch`), kept with its
        // checkpoint for a retry of it; done with once it went out
        let carry = if sent.is_none() { self.carry.clone() } else { Vec::new() };
        // the map orients the first message of a session; the run log keeps the user's own words
        let first = if sent.is_some() {
            sent.map(String::from)
        } else if !carry.is_empty() {
            Some(with_earlier(text, &carry))
        } else if self.repo_map && !self.mapped && self.session_id.is_none() {
            self.mapped = true;
            crate::repomap::build(&self.cwd, text).map(|m| format!("{m}\n---\n\n{text}"))
        } else if self.profile == Profile::Chat && self.told < self.said.len() {
            // a chat after the agent answered in its place: the questions and those answers first, as its own
            Some(with_earlier(text, &self.said[self.told..]))
        } else {
            None
        };
        let sent = first.as_deref().unwrap_or(text);
        // a typed message is a point /rewind can go back to (claude keeps the id we give it; the own loop marks where it
        // begins in its saved history); a handed-over turn is in a conversation of its own, not one to go back in
        let uuid = self.handoff.is_none().then(new_uuid);
        if let Some(u) = &uuid {
            self.checkpoints.push(Checkpoint { uuid: u.clone(), text: text.to_string(), before: self.last_assistant.clone(), carry: carry.clone(), attach, epoch: self.switches });
        }
        // the images the user's own words name (not the earlier messages' or a repo map's), with all that is sent
        let content = match user_content(text, &self.cwd) {
            Value::Array(mut parts) if attach => {
                if let Some(t) = parts.last_mut() {
                    t["text"] = json!(sent);
                }
                Value::Array(parts)
            }
            _ => json!(sent),
        };
        self.write_content(content, false, uuid.as_deref())?;
        if !carry.is_empty() {
            self.carry.clear();
        }
        (self.task, self.task_reviewed) = (text.to_string(), false);
        self.runs.push_back(run);
        Ok(())
    }

    /// tazuna's own messages to claude (feedback, review): plain text, never attachments.
    fn write_user(&mut self, text: &str) -> Result<()> {
        self.write_content(json!(text), true, None)
    }

    /// `followup`: tazuna's own message about the current task, which the own loop counts against the task's time and
    /// dollar budget instead of starting a new one (claude never sees the field).
    fn write_content(&mut self, content: Value, followup: bool, uuid: Option<&str>) -> Result<()> {
        if self.child.is_none() {
            self.spawn()?;
        }
        let mut line = json!({"type": "user", "message": {"role": "user", "content": content}});
        if let Some(u) = uuid {
            line["uuid"] = json!(u);
        }
        if followup && self.provider.is_some() {
            line["tazuna_followup"] = json!(true);
        }
        // a hand-off's turn is tazuna's message to the agent, not one the user typed (Copilot's X-Initiator, engine.rs)
        if self.handoff.is_some() && self.provider.is_some() {
            line["tazuna_agent"] = json!(true);
        }
        let stdin = self.stdin.as_mut().context("claude stdin closed")?;
        writeln!(stdin, "{line}")?;
        stdin.flush()?;
        Ok(())
    }

    /// Applies the router's decision (before the first message) and returns the event that shows it.
    pub fn set_lane(&mut self, lane: Lane) -> (f64, Value) {
        (self.model, self.effort, self.escalate, self.profile) = (lane.model.clone(), lane.effort.clone(), lane.escalate.clone(), lane.profile);
        self.lane = (lane.name.to_string(), lane.reason.clone());
        self.record(json!({"type": "harness_route", "lane": lane.name, "model": lane.model, "effort": lane.effort, "reason": lane.reason,
            "profile": lane.profile.name(), "escalate": lane.escalate.map(|(m, e)| format!("{m} {e}"))}))
    }

    /// Switches model and effort for the rest of the session: ends the idle process; the next message resumes the session
    /// on the new model (one cache rebuild, since the prompt cache is per model).
    /// A `--tools` list with the Artifact tool in it once the conversation has an artifact link (`artifacts`).
    fn with_artifact(&self, tools: &str) -> String {
        match (self.artifacts, tools.is_empty()) {
            (false, _) => tools.to_string(),
            (true, true) => "Artifact".to_string(),
            (true, false) => format!("{tools},Artifact"),
        }
    }

    fn respawn(&mut self, model: String, effort: String) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        self.stdin = None;
        self.generation += 1;
        self.proc_cost = 0.0;
        (self.model, self.effort) = (model, effort);
    }

    /// `/model`: the rest of the session on another model. Between Claude models the conversation goes on, resumed on the
    /// new model; onto or within the own loop it starts afresh (the engine resumes no conversation). `route`: (provider URL,
    /// key variable, price) for the own loop, None for claude.
    pub fn switch(&mut self, model: String, effort: String, route: Option<(String, String, String)>) -> Result<(f64, Value)> {
        if !self.idle() {
            bail!("wait until the run has finished (or stop it with esc), then switch");
        }
        // claude resumes its session on the new model; the own loop starts its engine without one
        let goes_on = route.is_none() && self.provider.is_none();
        self.respawn(model.clone(), effort.clone());
        self.provider = route.as_ref().map(|(u, k, _)| (u.clone(), k.clone()));
        // a model with no price listed runs on the list's price, else --price (TAZUNA_PRICE)
        self.price = route.map(|r| r.2).filter(|p| !p.is_empty());
        // an id that ends in its level (OmniRoute's agy/gemini-4-argon-high) is said without it: "… argon at high"
        let shown = model.strip_suffix(&format!("-{effort}")).unwrap_or(&model).to_string();
        // a model without effort levels runs at none (its effort is empty)
        let level = if effort.is_empty() { String::new() } else { format!(" · {effort}") };
        let short = crate::ui::model_short(&shown);
        let text = if goes_on {
            format!("model: {short}{level} from here on (the conversation goes on; its cache is rebuilt once)")
        } else {
            // neither can read the other's history (claude and the own loop, two providers of the own loop): the conversation
            // so far goes with the next message as text, and the new session continues this one (one conversation in the
            // lists). It started over with nothing until review 12
            if let Some(id) = self.session_id.take() {
                self.continue_from(id);
            }
            self.carry = carried(&self.conv);
            // the messages before it are not in the next session's history: going back to one starts afresh (`back_before`),
            // and nothing goes back into the old one
            self.switches += 1;
            (self.resume_at, self.pending_resume, self.mapped) = (None, None, false);
            if self.carry.is_empty() {
                format!("model: {short}{level} from here on")
            } else {
                format!("model: {short}{level}: the conversation continues on {short}")
            }
        };
        Ok(self.record(json!({"type": "harness_switch", "model": model, "effort": effort, "text": text})))
    }

    /// `/effort` (and `/model` on the model in use): the rest of the session at another effort, the conversation going on.
    /// claude resumes its session at the new level; the own loop's engine takes it for its next calls.
    pub fn set_effort(&mut self, effort: String) -> Result<(f64, Value)> {
        if !self.idle() {
            bail!("wait until the run has finished (or stop it with esc)");
        }
        // an OmniRoute model with a level in its id (agy/gemini-4-argon-high): another level is that level's id, the
        // conversation going on
        let levelled = self.provider.as_ref().is_some_and(|p| p.0 == "http://localhost:20128/v1")
            && crate::models::tier_of(&self.model).is_some_and(|(_, t)| t == self.effort);
        let model = if levelled { crate::models::tier_of(&self.model).map(|(b, _)| format!("{b}-{effort}")) } else { None };
        if self.provider.is_some() {
            if let Some(stdin) = self.stdin.as_mut() {
                writeln!(stdin, "{}", json!({"type": "tazuna_effort", "effort": effort, "model": model}))?;
                stdin.flush()?;
            }
            self.effort = effort.clone();
            if let Some(m) = &model {
                self.model = m.clone();
            }
        } else {
            self.respawn(self.model.clone(), effort.clone());
        }
        let text = if self.provider.is_some() {
            format!("effort: {effort} for the rest of the session (the conversation goes on)")
        } else {
            format!("effort: {effort} for the rest of the session (the conversation goes on; Claude rebuilds its cache once)")
        };
        Ok(self.record(json!({"type": "harness_effort", "effort": effort, "model": model, "text": text})))
    }

    /// `/boss`: the rest of the session with boss mode on (Some: the workers' model) or off. claude restarts with the
    /// subagents and rules of the new mode; the conversation goes on.
    pub fn set_boss(&mut self, boss: Option<String>) -> Result<(f64, Value)> {
        if !self.idle() {
            bail!("wait until the run has finished (or stop it with esc)");
        }
        if self.provider.is_some() || self.profile != Profile::Repo {
            bail!("boss mode is for claude in the repo profile (this session: {})", if self.provider.is_some() { "the own loop" } else { "another profile" });
        }
        self.boss = boss;
        self.respawn(self.model.clone(), self.effort.clone());
        let text = match &self.boss {
            Some(m) => format!("boss mode on: {} directs, {m} subagents read, edit and run the checks{} (the conversation goes on)",
                self.model.trim_start_matches("claude-"), self.boss_effort.as_ref().map_or(String::new(), |e| format!(" at {e}"))),
            None => format!("boss mode off: {} works itself again (the conversation goes on)", self.model.trim_start_matches("claude-")),
        };
        Ok(self.record(json!({"type": "harness_info", "text": text})))
    }

    /// `/explorer`, `/verify`, `/review`: a setting the caller changed, for the rest of the session as its flag at start.
    /// claude restarts with it and resumes the conversation (the own loop's engine resumes its history).
    pub fn restart_with(&mut self, text: String) -> Result<(f64, Value)> {
        if !self.idle() {
            bail!("wait until the run has finished (or stop it with esc)");
        }
        self.respawn(self.model.clone(), self.effort.clone());
        Ok(self.record(json!({"type": "harness_info", "text": format!("{text} (the conversation goes on)")})))
    }

    /// `/profile NAME` and `/chat`: the rest of the session in another profile (as `--profile` at start). Between the coding
    /// profiles the conversation goes on (claude resumes it with the new prompt and tools); the caller starts a fresh one
    /// when switching to or from chat, which shares no context with the repo.
    pub fn set_profile(&mut self, p: Profile) -> Result<(f64, Value)> {
        if !self.idle() {
            bail!("wait until the run has finished (or stop it with esc)");
        }
        if p == self.profile {
            return Ok(self.record(json!({"type": "harness_profile", "profile": p.name(), "text": format!("already in the {} profile", p.name())})));
        }
        let mut note = "";
        if self.boss.is_some() && p != Profile::Repo {
            self.boss = None;
            note = " (boss mode off: it needs the repo profile)";
        }
        // a chat keeps auto or full outside git (main.rs start_mode); a project there asks, so leaving the chat for one
        // drops to ask, as starting in it would have (security review, 2026-11-24)
        let in_git = Command::new("git").args(["rev-parse", "--is-inside-work-tree"]).current_dir(&self.cwd).output().is_ok_and(|o| o.status.success());
        if p != Profile::Chat && !in_git && matches!(self.permission_mode.as_str(), "auto" | "bypassPermissions") {
            // the screen hears of it at the next poll, with this switch's own line
            let ask = self.set_mode("default");
            self.outbox.push(ask);
            note = " (ask mode: this folder is not a git repo, so nothing changed here could be undone)";
        }
        self.prev_profile = Some(self.profile);
        self.profile = p;
        (self.said, self.told) = (Vec::new(), 0);
        self.respawn(self.model.clone(), self.effort.clone());
        let what = match p {
            Profile::Chat => "a plain chat for questions that are not about code: no tools, no repo context (/chat again goes back)",
            Profile::Repo => "Claude Code's prompt and tools with the repo's settings, hooks, skills and CLAUDE.md",
            Profile::RepoLean => "tazuna's lean prompt with Claude Code's tools and the repo's settings",
            Profile::Lean => "tazuna's lean prompt and tools, no settings",
        };
        let own = if self.provider.is_some() && p != Profile::Chat { " (on the own loop the coding profiles are one agent)" } else { "" };
        Ok(self.record(json!({"type": "harness_profile", "profile": p.name(), "text": format!("{} profile: {what}{own}{note}", p.name())})))
    }

    /// `/mcp on|off`: the user's MCP servers (tazuna's list, ~/.config/tazuna/mcp.json) for every model, and on claude
    /// the account's claude.ai connectors too, for the rest of the session (as `--mcp` at start). claude restarts with
    /// them and resumes the conversation; the own loop connects them in place. Either way the conversation goes on.
    pub fn set_connectors(&mut self, on: bool) -> Result<(f64, Value)> {
        if !self.idle() {
            bail!("wait until the run has finished (or stop it with esc)");
        }
        if self.provider.is_some() {
            if let Some(stdin) = self.stdin.as_mut() {
                writeln!(stdin, "{}", json!({"type": "tazuna_mcp", "on": on}))?;
                stdin.flush()?;
            }
            self.connectors = on;
        } else {
            self.connectors = on;
            self.respawn(self.model.clone(), self.effort.clone());
        }
        let names: Vec<String> = crate::mcpc::servers().into_iter().map(|s| s.0).collect();
        let text = match (on, names.is_empty(), self.provider.is_some()) {
            (false, ..) => "MCP off (the conversation goes on; remembered for the next sessions)".to_string(),
            (true, true, true) => "MCP on, but no server yet: /mcp add NAME URL (remembered for the next sessions)".to_string(),
            (true, true, false) => "MCP on: your claude.ai connectors; /mcp add NAME URL adds servers for every model (the conversation goes on; remembered for the next sessions)".to_string(),
            (true, false, own) => format!("MCP on: {}{} (the conversation goes on; remembered for the next sessions)", names.join(", "), if own { "" } else { ", and your claude.ai connectors" }),
        };
        Ok(self.record(json!({"type": "harness_info", "text": text})))
    }

    /// `/new` (no id) and `/resume ID`: the next message starts a fresh conversation, or continues an earlier claude one,
    /// on the same model, effort and route. The conversation left behind stays resumable.
    pub fn restart(&mut self, resume: Option<String>) -> Result<(f64, Value)> {
        if !self.idle() {
            bail!("wait until the run has finished (or stop it with esc)");
        }
        // a conversation goes on where it ran: claude's on claude, the own loop's ("own-...") on the own loop
        match (&resume, &self.provider) {
            (Some(id), Some(_)) if !id.starts_with("own-") => bail!("this conversation ran on Claude: /model to a Claude model first"),
            (Some(id), None) if id.starts_with("own-") => bail!("this conversation ran on the own loop: /model to the model it used first"),
            _ => {}
        }
        self.respawn(self.model.clone(), self.effort.clone());
        self.checkpoints.clear();
        (self.carry, self.continues, self.earlier, self.resume_at, self.pending_resume) = (Vec::new(), None, Vec::new(), None, None);
        self.mapped = resume.is_some(); // a new conversation's first message gets the map
        let text = match &resume {
            Some(id) => format!("── resumed session {} ──", id.chars().take(8).collect::<String>()),
            None => "── new conversation ──".to_string(),
        };
        self.session_id = resume;
        // a notice also closes the old process's cost: the next one reports from $0
        Ok(self.record(json!({"type": "harness_notice", "text": text, "error": false})))
    }

    /// `--escalate MODEL[:EFFORT]`: fix rounds after failed tests run on MODEL (with `--provider`: on claude).
    pub fn set_escalate(&mut self, model: String, effort: String) {
        self.escalate = Some((model, effort));
    }

    /// A model the user picked (the /model picker, the app's): the lane's cascade to a stronger model no longer applies.
    /// Before 0.1.141 an own-loop model picked in a tiny repo (the "small" lane) had every reply that changed no file, an
    /// answer to a question too, done again on Opus.
    pub fn picked(&mut self) {
        self.escalate = None;
    }

    /// Where the run goes after a failed check or a cheap model giving up (None: nowhere).
    #[cfg(test)]
    pub fn escalation(&self) -> Option<&(String, String)> {
        self.escalate.as_ref()
    }

    /// Nothing in flight: every run has ended and no message is waiting.
    pub fn idle(&self) -> bool {
        // after the last background command ends, claude takes a moment to start the turn it wakes up for
        let settled = self.tasks_done.is_none_or(|t| t.elapsed().as_secs() >= 5);
        self.runs.is_empty() && !self.verifying && self.held.is_empty() && self.tasks.is_empty() && settled
    }

    /// Asks claude to stop the current run; the process stays up and keeps its cache.
    /// The user's answer to the oldest permission question. `always`: allow this tool for the rest of the session.
    pub fn answer(&mut self, allow: bool, always: bool) -> Option<(f64, Value)> {
        let req = self.pending.pop_front()?;
        let tool = req["request"]["tool_name"].as_str().unwrap_or_default().to_string();
        if allow && always && req["request"]["from_text"] != true && req["request"]["outside"] != true && req["request"]["handoff"] != true {
            self.allowed.insert(always_key(&req));
        }
        // an approved plan leaves plan mode: edits go ahead, commands still ask
        let leave_plan = (allow && tool == "ExitPlanMode").then(|| json!([{"type": "setMode", "mode": "acceptEdits", "destination": "session"}]));
        if leave_plan.is_some() {
            self.permission_mode = "acceptEdits".into();
        }
        self.respond(&req, allow, leave_plan);
        Some(self.record(json!({"type": "harness_permission_done", "tool": tool, "allow": allow, "always": always, "mode": self.permission_mode})))
    }

    fn respond(&mut self, req: &Value, allow: bool, permissions: Option<Value>) {
        // an allowed lean bash command may move its shell (`cd`): the next question is read from where it went
        if allow && self.provider.is_none() && req["request"]["tool_name"] == "mcp__lean__bash" {
            self.shell_dir = self.risky(req).1;
        }
        let mut answer = if allow {
            json!({"behavior": "allow", "updatedInput": req["request"]["input"]})
        } else if self.mode == "headless" {
            json!({"behavior": "deny", "message": "No user is present in this run, so nobody can answer or approve this. Decide with your best judgment and continue."})
        } else {
            json!({"behavior": "deny", "message": "The user declined this. Ask what they want instead, or try another way."})
        };
        if let Some(p) = permissions {
            answer["updatedPermissions"] = p;
        }
        let line = json!({"type": "control_response", "response": {"subtype": "success", "request_id": req["request_id"], "response": answer}});
        if let Some(stdin) = self.stdin.as_mut() {
            let _ = writeln!(stdin, "{line}").and_then(|_| stdin.flush());
        }
    }

    /// Why a command question is one of the destructive commands auto asks about (risky.rs), and where it leaves claude's
    /// lean bash; (None, _) for other tools. claude's own Bash goes back to the project when a `cd` leaves it (its CLI),
    /// so it is read from there: within the project that only errs towards asking.
    fn risky(&self, req: &Value) -> (Option<String>, Option<PathBuf>) {
        let r = &req["request"];
        let tool = r["tool_name"].as_str().unwrap_or_default();
        if always_scope(tool) != "this command" {
            return (None, self.shell_dir.clone());
        }
        let start = if tool == "mcp__lean__bash" { self.shell_dir.clone() } else { Some(self.cwd.clone()) };
        crate::risky::scan(r["input"]["command"].as_str().unwrap_or_default(), &self.cwd, start.as_deref())
    }

    /// tazuna answers claude's question itself: in full anything, in auto anything but a destructive command (`safe`: not
    /// one); never the user's own questions (AskUserQuestion) or a plan to approve. The own loop asks only what it
    /// decided to (engine.rs `permit`, which knows where its shell is). The caller leaves out calls written as text and
    /// handed-over turns.
    fn auto_allows(&self, req: &Value, safe: bool) -> bool {
        let mine = matches!(self.permission_mode.as_str(), "bypassPermissions") || (self.permission_mode == "auto" && safe);
        mine && self.provider.is_none() && !matches!(req["request"]["tool_name"].as_str(), Some("AskUserQuestion" | "ExitPlanMode"))
    }

    /// The questions waiting when the mode changes, answered where the new mode would not have asked them: in full each
    /// one, in auto each but a destructive command (risky.rs); the user's own questions, calls written as text and a plan
    /// to approve still wait. From the front, up to the first that still asks (the screen shows the front one). Switching
    /// to full left "allow bash?" on screen, waiting for a yes full would have given (the user, 2026-11-24).
    pub fn answer_for_mode(&mut self) -> Vec<(f64, Value)> {
        let mut out = vec![];
        while let Some(req) = self.pending.front() {
            let r = &req["request"];
            let tool = r["tool_name"].as_str().unwrap_or_default().to_string();
            let theirs = !matches!(tool.as_str(), "AskUserQuestion" | "ExitPlanMode") && r["from_text"] != true;
            let free = theirs && match self.permission_mode.as_str() {
                "bypassPermissions" => true,
                "auto" => r["outside"] != true && self.risky(req).0.is_none(),
                _ => false,
            };
            if !free {
                break;
            }
            let req = self.pending.pop_front().expect("the front, read above");
            self.respond(&req, true, None);
            out.push(self.record(json!({"type": "harness_permission_done", "tool": tool, "allow": true, "always": false, "mode": self.permission_mode})));
        }
        out
    }

    /// Switches the permission mode (also mid-run). Before the process starts it only sets the starting mode.
    pub fn set_mode(&mut self, mode: &str) -> (f64, Value) {
        self.reviewer = mode == "review";
        let shown = mode;
        let mode = if self.reviewer { "acceptEdits" } else { mode };
        self.permission_mode = mode.to_string();
        // auto is tazuna's: claude runs as acceptEdits under it (`turn_mode`), the own loop knows it by name
        let mode = if mode == "auto" && self.provider.is_none() { "acceptEdits" } else { mode };
        // a handed-over turn keeps its own (`turn_mode`): the session's applies to the process `hand_back` starts anew
        if self.handoff.is_none() && let Some(stdin) = self.stdin.as_mut() {
            self.requests_sent += 1;
            let req = json!({"type": "control_request", "request_id": format!("mode-{}", self.requests_sent), "request": {"subtype": "set_permission_mode", "mode": mode}});
            let _ = writeln!(stdin, "{req}").and_then(|_| stdin.flush());
        }
        self.record(json!({"type": "harness_mode", "mode": shown}))
    }

    /// The reviewer's answer to the oldest question, if it is still request `id`: allowed with `input` (the command in its
    /// sandbox), or refused with `why`. None: the question was answered meanwhile.
    #[inline(never)]
    pub fn answer_reviewed(&mut self, id: &str, command: &str, allow: bool, input: Value, why: &str) -> Option<(f64, Value)> {
        // the own loop's request ids start again at perm-1 in every engine process: a verdict on an earlier process's
        // question must not answer a later one with the same id (a hand-off's, above all: review mode never answers there)
        let front = self.pending.front()?;
        if self.handing_off() || front["request_id"].as_str() != Some(id) || front["request"]["input"]["command"].as_str().unwrap_or_default() != command {
            return None;
        }
        let req = self.pending.pop_front()?;
        let answer = if allow {
            json!({"behavior": "allow", "updatedInput": input})
        } else {
            json!({"behavior": "deny", "message": format!("tazuna's reviewer refused this: {why}. Do it another way, or ask the user.")})
        };
        let line = json!({"type": "control_response", "response": {"subtype": "success", "request_id": req["request_id"], "response": answer}});
        if let Some(stdin) = self.stdin.as_mut() {
            let _ = writeln!(stdin, "{line}").and_then(|_| stdin.flush());
        }
        Some(self.record(json!({"type": "harness_permission_done", "tool": req["request"]["tool_name"], "allow": allow, "always": false, "mode": self.permission_mode})))
    }

    /// Rewinds to just before the user message `checkpoints[i]`: the file changes made since (`files`, claude's
    /// `rewind_files`) and/or the conversation from there on (`conversation`: claude resumes at the last reply before
    /// it, in a forked session). Only while idle and on claude; the own loop keeps no checkpoints.
    pub fn rewind(&mut self, i: usize, files: bool, conversation: bool) -> Result<(f64, Value)> {
        // the own loop keeps where each typed message begins in its saved history (the conversation goes back by that), but
        // no file checkpoints: the desktop puts files back from tazuna's own turn snapshots
        if self.provider.is_some() && files {
            bail!("the own loop (--provider) can rewind the conversation, not files: claude keeps the file checkpoints");
        }
        if !self.idle() {
            bail!("wait until the run has finished (or stop it with esc), then rewind");
        }
        let cp = self.checkpoints.get(i).cloned().context("no such message")?;
        if files {
            if self.child.is_none() {
                self.spawn()?;
            }
            let stdin = self.stdin.as_mut().context("claude stdin closed")?;
            // a dry run first: its answer names the files (the real rewind's answer does not)
            for (dry, tag) in [(true, "rewind-dry"), (false, "rewind")] {
                self.requests_sent += 1;
                let req = json!({"type": "control_request", "request_id": format!("{tag}-{}", self.requests_sent),
                    "request": {"subtype": "rewind_files", "user_message_id": cp.uuid, "dry_run": dry}});
                writeln!(stdin, "{req}")?;
                stdin.flush()?;
            }
        }
        if conversation {
            self.checkpoints.truncate(i);
            self.back_before(&cp);
            if let Some(j) = self.conv.iter().rposition(|c| c.0 == cp.text) {
                self.conv.truncate(j);
            }
            if !files {
                self.resume_before();
            } // with files: once claude has answered the file rewind (its checkpoints live in that process)
        }
        let what = match (files, conversation) {
            (true, true) => "files and conversation",
            (true, false) => "files",
            _ => "conversation",
        };
        Ok(self.record(json!({"type": "harness_notice", "text": format!("── rewound {what} to before: {} ──", crate::session::shown(&cp.text, 60)), "error": false})))
    }

    /// The window's Retry (main.rs app_retry): the last message, whose turn ended without an answer, sent again, its
    /// images too, with no second copy of it in the history or the transcript (it was there twice, the failed try between).
    /// The conversation goes back to before it as "edit from here" does (claude at the reply before it, in a fork; the own
    /// loop's history cut before it); the new session continues the old one, so the lists keep one conversation. Records
    /// `harness_retry`, which takes the failed try off the screen (Session::apply).
    #[inline(never)]
    pub fn retry(&mut self, text: &str) -> Result<(f64, Value)> {
        if !self.idle() {
            bail!("wait until the run has finished (or stop it), then retry");
        }
        // its checkpoint: a message this process sent to the model (one sent before the window opened the conversation, or
        // handed to the agent in a turn of its own, has none: the history could not go back before it)
        let cp = self.checkpoints.last().filter(|c| c.text == text).cloned().context("that message cannot be sent again from here \
            (it was sent before this window opened the conversation, or handed to the agent): send it again")?;
        self.within_cap()?;
        self.checkpoints.pop();
        self.back_before(&cp);
        self.resume_before();
        // its message stays in `conv`, unanswered, for this try's answer
        self.start_run(text, cp.attach, None)?;
        Ok(self.record(json!({"type": "harness_retry"})))
    }

    /// Back to before message `cp` (an edit from it, a retry of it), for the next spawn (`resume_before`). One of this
    /// session's: claude at the reply before it, the own loop's history cut before it, in a fork. One from before a switch to
    /// a model that could not take the history: that history is not this session's, so a fresh session, the conversation
    /// before it carried as text (a Claude reply's id went to the own loop as `--resume-before`, review 12). Either way the
    /// next session continues this one.
    #[inline(never)]
    fn back_before(&mut self, cp: &Checkpoint) {
        let fresh = cp.epoch != self.switches;
        self.pending_resume = Some(match (fresh, self.provider.is_some()) {
            (true, _) => None,
            (false, true) => Some(cp.uuid.clone()),
            (false, false) => cp.before.clone(),
        });
        // back before a switch's first message: the next one carries the conversation before it again
        let upto = self.conv.iter().rposition(|c| c.0 == cp.text).unwrap_or(self.conv.len());
        self.carry = if fresh { carried(&self.conv[..upto]) } else { cp.carry.clone() };
        if let Some(id) = self.session_id.clone() {
            self.continue_from(id);
        }
    }

    /// The next session continues `id` (`continues`, `earlier`).
    fn continue_from(&mut self, id: String) {
        if self.earlier.last() != Some(&id) {
            self.earlier.push(id.clone());
        }
        self.continues = Some(id);
    }

    /// Every earlier session the conversation continues from, oldest first (the view's `earlier_sessions`).
    pub fn earlier_sessions(&self) -> &[String] {
        &self.earlier
    }

    /// A conversation resumed (main.rs restart_session): the sessions it continues from, oldest first.
    pub fn set_earlier(&mut self, ids: &[String]) {
        self.earlier = ids.to_vec();
    }

    /// Ends the process so the next message resumes the conversation at the reply `pending_resume` names (or starts a
    /// fresh one when the rewind went back before the first message).
    fn resume_before(&mut self) {
        let Some(before) = self.pending_resume.take() else { return };
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        self.stdin = None;
        self.generation += 1;
        match before {
            Some(at) => {
                // the reply it resumes at is the last one again: a message sent next goes back there (a second retry, an
                // edit of it); None started the next session fresh (review 12)
                self.last_assistant = self.provider.is_none().then(|| at.clone());
                self.resume_at = Some(at);
            }
            None => (self.session_id, self.last_assistant, self.mapped) = (None, None, false),
        }
    }

    /// What the explore tool's answers cost so far (`--explorer MODEL@URL`; claude's own figure does not include them).
    pub fn explorer_cost(&self) -> f64 {
        std::fs::read_to_string(&self.explorer_costs).unwrap_or_default().lines().filter_map(|l| l.trim().parse::<f64>().ok()).sum()
    }

    /// Asks claude for the latest output of shell and Monitor tasks still running, and once more right after one ends
    /// (subagents have no output to read). The answers arrive through `poll`.
    pub fn read_task_outputs(&mut self) {
        let ids: Vec<String> = self.bg.iter()
            .filter(|t| t.kind != "local_agent" && t.ended.map_or(true, |e| e.elapsed() < Duration::from_secs(3)))
            .map(|t| t.id.clone()).collect();
        let Some(stdin) = self.stdin.as_mut() else { return };
        for id in ids {
            self.requests_sent += 1;
            let req = json!({"type": "control_request", "request_id": format!("out-{}-{id}", self.requests_sent), "request": {"subtype": "get_task_output", "task_id": id}});
            let _ = writeln!(stdin, "{req}").and_then(|_| stdin.flush());
        }
    }

    /// Stops one background task (claude's `stop_task`); the run and other tasks go on.
    pub fn stop_task(&mut self, id: &str) -> Option<(f64, Value)> {
        let stdin = self.stdin.as_mut()?;
        self.requests_sent += 1;
        let req = json!({"type": "control_request", "request_id": format!("stop-{}", self.requests_sent), "request": {"subtype": "stop_task", "task_id": id}});
        writeln!(stdin, "{req}").and_then(|_| stdin.flush()).ok()?;
        Some(self.record(json!({"type": "harness_info", "text": format!("stopping background task {id}")})))
    }

    pub fn interrupt(&mut self) -> Option<(f64, Value)> {
        // a stop also answers open questions: no
        while let Some(e) = self.answer(false, false) {
            self.outbox.push(e);
        }
        if self.verifying {
            // claude is idle while tests run: nothing to interrupt there, the run ends when the tests return
            self.runs.front_mut()?.interrupted();
            return Some(self.record(json!({"type": "harness_verify_cancel"})));
        }
        // a second esc or ctrl-c while the first waits for an answer: stop now. Each press sent the stop again and started
        // the 3 s wait over, so pressing on kept everything waiting (about 10 s on a free model, 2026-11-11)
        if self.interrupting.is_some() {
            return Some(self.kill("stopped"));
        }
        let stdin = self.stdin.as_mut()?;
        self.interrupts_sent += 1;
        let req = json!({"type": "control_request", "request_id": format!("interrupt-{}", self.interrupts_sent), "request": {"subtype": "interrupt"}});
        if writeln!(stdin, "{req}").and_then(|_| stdin.flush()).is_err() {
            return Some(self.kill("stopped"));
        }
        self.interrupting = Some(Instant::now());
        if let Some(run) = self.runs.front_mut() {
            run.interrupted();
        }
        Some(self.record(json!({"type": "harness_interrupt"})))
    }

    /// Last resort: kill the process (the next message resumes the session).
    #[inline(never)]
    fn kill(&mut self, why: &str) -> (f64, Value) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.stdin = None;
        self.generation += 1;
        self.interrupting = None;
        // its background commands went with it, and their end will never be reported (the old generation's messages are
        // dropped): a held hand-off waiting for them would wait forever
        self.tasks.clear();
        self.tasks_done = None;
        self.end_runs();
        self.record(json!({"type": "harness_notice", "text": format!("{why} · send a message to continue"), "error": false}))
    }

    /// Events that arrived since the last call, timestamped and recorded.
    pub fn poll(&mut self) -> Vec<(f64, Value)> {
        let mut out = std::mem::take(&mut self.outbox);
        while let Ok((generation, msg)) = self.rx.try_recv() {
            if generation != self.generation {
                continue;
            }
            match msg {
                Msg::Line(l) => {
                    let Ok(mut e) = serde_json::from_str::<Value>(&l) else { continue };
                    if e["type"] == "system" && e["subtype"] == "init" {
                        // who pays claude's runs (models::claude_payer): its key's source, and the billing variables set in
                        // the environment it runs in (this one), written into the init by name for the session and its replays
                        if self.provider.is_none() {
                            let set: Vec<&str> = crate::models::CLAUDE_BILLING_ENV.into_iter().filter(|k| std::env::var(k).is_ok_and(|v| !v.is_empty() && v != "0")).collect();
                            self.claude_bill = crate::models::claude_payer(e["apiKeySource"].as_str(), |k| set.contains(&k));
                            e["tazuna_env"] = json!(set);
                        }
                        self.session_id = e["session_id"].as_str().map(String::from);
                        self.claude_version = e["claude_code_version"].as_str().map(String::from);
                        if self.mode != "headless" {
                            // claude's commands and skills here, for the menu of the next conversation in this folder
                            // before its first turn (the own loop's list is empty: nothing to keep)
                            let cmds: Vec<String> = e["slash_commands"].as_array().into_iter().flatten().filter_map(|c| c.as_str().map(String::from)).collect();
                            if !cmds.is_empty() && let Ok(d) = crate::labels::dir() {
                                save_claude_commands(&d.join(CLAUDE_COMMANDS), &self.cwd.to_string_lossy(), &cmds);
                            }
                            let state = crate::labels::dir().map(|d| d.join("claude-version")).unwrap_or_default();
                            for text in crate::versions::notices(self.claude_version.as_deref(), e["model"].as_str(), &state, self.opened) {
                                out.push(self.record(json!({"type": "harness_info", "text": text})));
                            }
                        }
                    }
                    // claude's echo of a message as it takes it (`--replay-user-messages`). Taken while a turn runs, claude
                    // folds it into that turn at its next tool call, and the turn ends with one result for both: its run
                    // joins the one running. Counted as its own run, it waited for a result that never came: the run log
                    // put later costs on the wrong message and the session never went idle (tried on Haiku 5.5, 2026-11-24).
                    // Taken after the result, it is its own turn, and already the front run. Only counted, never shown.
                    if e["type"] == "user" && e["isReplay"] == true {
                        let c = &e["message"]["content"];
                        let said = c.as_str().map(String::from).unwrap_or_else(|| c.as_array().into_iter().flatten().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("\n"));
                        if let Some(i) = self.runs.iter().skip(1).position(|r| !r.prompt.trim().is_empty() && said.contains(r.prompt.trim())) {
                            let later = self.runs.remove(i + 1).expect("found above");
                            self.runs.front_mut().expect("a run before it").join(later);
                        }
                        continue;
                    }
                    if e["type"] == "control_request" && e["request"]["subtype"] == "can_use_tool" {
                        // a tool call the own loop recovered from the model's text is asked every time: no "always"
                        // covers it (a page could hold such text, security review 2026-11-12)
                        let from_text = e["request"]["from_text"] == true;
                        // nor an edits-mode write outside the project (engine.rs `edits_freely`): each one could run
                        // something later (a shell profile, a hook)
                        let outside = e["request"]["outside"] == true;
                        // nor in a turn a chat handed over, where none is given either (`answer`): the session's "always"
                        // answers were given to its own work, not to what a fetched page may ask for
                        if self.handoff.is_some() {
                            e["request"]["handoff"] = json!(true);
                        }
                        let risky = self.risky(&e).0;
                        // a turn a chat handed over runs as every turn does in full and auto: full never asks (0.1.216: "i
                        // thought full access means basically near if not full access to everything"), auto only before a
                        // destructive command (0.1.219: a chat in ask got 17 questions; the user chose this over the
                        // 2026-11-21 rule that it asked before everything but the web, whatever the mode). The other modes
                        // keep its once-only questions.
                        let handoff_free = self.handoff.is_some() && !matches!(e["request"]["tool_name"].as_str(), Some("AskUserQuestion" | "ExitPlanMode"))
                            && match self.permission_mode.as_str() { "bypassPermissions" => true, "auto" => risky.is_none(), _ => false };
                        if !from_text && !outside && (handoff_free || self.handoff.is_none() && (self.allowed.contains(&always_key(&e)) || self.auto_allows(&e, risky.is_none()))) {
                            self.respond(&e, true, None);
                        } else if self.mode == "headless" {
                            // nobody is there to answer: a question (AskUserQuestion, or any tool in ask mode) would wait forever
                            self.respond(&e, false, None);
                        } else {
                            let tool = e["request"]["tool_name"].as_str().unwrap_or_default().to_string();
                            let scope = if from_text {
                                "once only: the model wrote this call as text"
                            } else if outside {
                                "once only: outside the project"
                            } else if self.handoff.is_some() {
                                HANDOFF_ONCE
                            } else {
                                always_scope(&tool)
                            };
                            // why auto asks this one, for the question's note ("auto asks: rm outside the project")
                            if self.permission_mode == "auto" && let Some(why) = risky {
                                e["request"]["risky"] = json!(why);
                            }
                            out.push(self.record(json!({"type": "harness_permission", "tool": tool, "input": e["request"]["input"], "scope": scope})));
                            self.pending.push_back(e);
                        }
                        continue;
                    }
                    if e["type"] == "assistant" && e["parent_tool_use_id"].is_null() {
                        if let Some(u) = e["uuid"].as_str() {
                            self.last_assistant = Some(u.to_string()); // where a conversation rewind resumes
                        }
                    }
                    // answers to /rewind's file requests
                    if e["type"] == "control_response" {
                        let rid = e["response"]["request_id"].as_str().unwrap_or_default();
                        let r = &e["response"]["response"];
                        if rid.starts_with("rewind-dry-") {
                            let files: Vec<String> = r["filesChanged"].as_array().into_iter().flatten()
                                .filter_map(|f| f.as_str()).map(|f| f.rsplit('/').next().unwrap_or(f).to_string()).collect();
                            let text = if r["canRewind"] == true && files.is_empty() {
                                "no file changes recorded since that message (claude checkpoints only its own edit tools; the lean profile edits through tazuna's)".to_string()
                            } else if r["canRewind"] == true {
                                format!("restoring {} file(s): {} (+{} −{})", files.len(), files.join(", "), r["insertions"].as_i64().unwrap_or(0), r["deletions"].as_i64().unwrap_or(0))
                            } else {
                                format!("files cannot be rewound: {}", r["error"].as_str().or(e["response"]["error"].as_str()).unwrap_or("no checkpoint"))
                            };
                            out.push(self.record(json!({"type": "harness_info", "text": text})));
                            continue;
                        }
                        if rid.starts_with("rewind-") {
                            let ok = e["response"]["subtype"] == "success" && r["canRewind"] == true;
                            let text = if ok { "files restored".to_string() } else { format!("file rewind failed: {}", r["error"].as_str().or(e["response"]["error"].as_str()).unwrap_or("unknown")) };
                            out.push(self.record(json!({"type": "harness_info", "text": text})));
                            self.resume_before();
                            continue;
                        }
                    }
                    // a background task's output, asked for by the ctrl-b view: no event of its own
                    if e["type"] == "control_response" {
                        let id = e["response"]["request_id"].as_str().and_then(|r| r.strip_prefix("out-")).and_then(|r| r.split_once('-')).map(|(_, id)| id);
                        if let Some(t) = id.and_then(|id| self.bg.iter_mut().find(|t| t.id == id)) {
                            t.output = e["response"]["response"]["output"].as_str().unwrap_or_default().to_string();
                            continue;
                        }
                    }
                    match (e["type"].as_str(), e["subtype"].as_str(), e["task_id"].as_str()) {
                        (Some("system"), Some("task_started"), Some(id)) => {
                            let owner = e["parent_task_id"].as_str().filter(|_| e["owned_by_subagent"] == true).map(String::from);
                            self.tasks.insert(id.to_string(), owner);
                            // an agent continued with SendMessage starts again under its id: running again, not the ended
                            // row it left (a resumed implementation agent worked for minutes with no sign of it, 2026-11-10)
                            let call = e["tool_use_id"].as_str().map(|c| (c.to_string(), e["prompt"].as_str().unwrap_or_default().to_string()));
                            if let Some(t) = self.bg.iter_mut().find(|t| t.id == id) {
                                (t.status, t.started, t.ended, t.doing) = ("running".into(), Instant::now(), None, String::new());
                                if let Some(d) = e["description"].as_str().filter(|d| !d.is_empty()) {
                                    t.description = d.to_string();
                                }
                                if let Some(c) = call.filter(|c| !t.calls.iter().any(|(id, _)| *id == c.0)) {
                                    t.calls.push(c);
                                }
                            } else {
                                self.bg.push(BgTask { id: id.to_string(), kind: e["task_type"].as_str().unwrap_or("task").to_string(),
                                    description: e["description"].as_str().unwrap_or_default().to_string(), status: "running".into(),
                                    started: Instant::now(), ended: None, output: String::new(),
                                    agent: e["subagent_type"].as_str().unwrap_or_default().to_string(), tokens: 0, tools: 0, doing: String::new(),
                                    calls: call.into_iter().collect() });
                            }
                        }
                        (Some("system"), Some("task_progress"), Some(id)) => {
                            if let Some(t) = self.bg.iter_mut().find(|t| t.id == id) {
                                t.tokens = e["usage"]["total_tokens"].as_u64().unwrap_or(t.tokens);
                                t.tools = e["usage"]["tool_uses"].as_u64().unwrap_or(t.tools);
                                t.doing = e["description"].as_str().unwrap_or_default().to_string();
                            }
                        }
                        (Some("system"), Some("task_updated"), Some(id)) => {
                            if let (Some(t), Some(st)) = (self.bg.iter_mut().find(|t| t.id == id), e["patch"]["status"].as_str()) {
                                t.status = st.to_string();
                                if st != "running" {
                                    t.ended.get_or_insert_with(Instant::now);
                                }
                            }
                        }
                        (Some("system"), Some("task_notification"), Some(id)) => {
                            let before = self.tasks.len();
                            self.tasks.retain(|t, owner| t != id && owner.as_deref() != Some(id));
                            if self.tasks.len() < before && self.tasks.is_empty() {
                                self.tasks_done = Some(Instant::now());
                            }
                            if let Some(t) = self.bg.iter_mut().find(|t| t.id == id) {
                                t.status = e["status"].as_str().unwrap_or("ended").to_string();
                                t.ended.get_or_insert_with(Instant::now);
                            }
                        }
                        _ => {}
                    }
                    if self.runs.is_empty() && !self.verifying && starts_turn(&e) {
                        // claude woke up without a message from us (a background command it started has finished):
                        // its turns and cost get a run of their own instead of going unlogged
                        let run = Run::new(BACKGROUND_PROMPT, self.elapsed_ms(), &self.cwd);
                        self.runs.push_back(run);
                        self.tasks_done = None;
                    }
                    if let Some(run) = self.runs.front_mut() {
                        run.observe(&e);
                    }
                    let is_result = e["type"] == "result";
                    if is_result && self.provider.is_some() {
                        e["tazuna_paid"] = json!(true); // the own loop spends money; claude's cost is on the plan
                    }
                    if is_result {
                        self.interrupting = None;
                        let total = e["total_cost_usd"].as_f64().unwrap_or(self.proc_cost);
                        if let Some(run) = self.runs.front_mut() {
                            // a process that started over (/clear, /new) reports from $0: its total is all new (the run
                            // log had -$16.26 for a /clear, 2026-11-12)
                            run.add_cost(if total >= self.proc_cost { total - self.proc_cost } else { total });
                        }
                        self.proc_cost = total;
                        // the message it answers, for the conversation's exchanges (`track`)
                        if let Some(run) = self.runs.front().filter(|r| r.prompt != BACKGROUND_PROMPT) {
                            e["tazuna_prompt"] = json!(run.prompt);
                        }
                        // a chat's answer or the agent's in its place, with its question (`heard`)
                        if let Some(run) = self.runs.front().filter(|_| self.profile == Profile::Chat || self.handoff.is_some()) {
                            e["tazuna_q"] = json!(run.prompt);
                            if self.handoff.is_some() {
                                e["tazuna_handoff"] = json!(true);
                            }
                        }
                        self.last_result = Some(e.clone());
                        // a background task that finished during this turn wakes claude after its result: settle from here
                        if self.tasks_done.is_some() {
                            self.tasks_done = Some(Instant::now());
                        }
                    }
                    out.push(self.record(e));
                    if is_result {
                        out.extend(self.after_result());
                    }
                }
                Msg::Verified(o) => out.extend(self.after_verify(o)),
                Msg::Reviewed(r) => out.extend(self.after_review(r)),
                Msg::Exited(stderr) => {
                    // the process and its commands are gone
                    self.tasks.clear();
                    for t in self.bg.iter_mut().filter(|t| t.ended.is_none()) {
                        (t.status, t.ended) = ("lost".into(), Some(Instant::now())); // the process that ran it is gone
                    }
                    let code = self.child.take().and_then(|mut c| c.wait().ok()).and_then(|st| st.code());
                    self.stdin = None;
                    self.end_runs();
                    let tail: Vec<&str> = stderr.lines().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
                    // tazuna's own loop says why it stopped in one line ("Error: 127.0.0.1:8768 lists mock-model without a
                    // price. …"): that line alone, not "claude exited (code 1): Error: …"
                    let said = stderr.lines().find_map(|l| l.strip_prefix("Error: ")).filter(|_| self.provider.is_some());
                    let text = match said {
                        Some(why) => why.to_string(),
                        None => format!("{} exited ({}){}", if self.provider.is_some() { "the model's process" } else { "claude" },
                            code.map_or("signal".into(), |c| format!("code {c}")),
                            if tail.is_empty() { String::new() } else { format!(": {}", tail.join(" · ")) }),
                    };
                    out.push(self.record(json!({"type": "harness_notice", "text": text, "error": code != Some(0)})));
                }
            }
        }
        if self.interrupting.is_some_and(|t| t.elapsed().as_secs() >= 3) {
            let who = if self.provider.is_some() { "the model" } else { "claude" };
            out.push(self.kill(&format!("stopped ({who} did not respond, process restarted)")));
        }
        // a hand-off that waited for the agent's background commands to end (`send_held`)
        if self.runs.is_empty() && !self.verifying && !self.held.is_empty() {
            out.extend(self.send_held());
        }
        out
    }

    /// claude says the front run is done: verify it, or end it.
    fn after_result(&mut self) -> Vec<(f64, Value)> {
        let success = self.last_result.as_ref().is_some_and(|r| r["subtype"] == "success" && r["is_error"] != true);
        // the chat replied [[agent]]: its message goes to the agent (main.rs does the same for the router's chat answers)
        let reply = self.last_result.as_ref().and_then(|r| r["result"].as_str()).unwrap_or_default();
        if self.profile == Profile::Chat && self.mode != "headless" && success && hands_off(reply) {
            // its question, read before `finish_front` starts the next message: the agent takes it with the exchanges the
            // chat saw, before the messages sent after it (and after any hand-off waiting before it). The chat's conversation
            // goes on as it is, the marker in it; its next message carries the agent's answer (`start_run`)
            let q = self.runs.front().map_or_else(|| self.task.clone(), |r| r.prompt.clone());
            let sent = with_earlier(&q, self.earlier());
            let mut out = vec![];
            match self.within_cap() {
                Ok(()) => {
                    let at = self.held.iter().take_while(|h| h.2.is_some()).count();
                    self.held.insert(at, (q, false, Some((sent, HANDOFF_NEEDS))));
                }
                Err(e) => out.push(self.record(json!({"type": "harness_info", "text": format!("{e:#}"), "error": true, "ended": true}))),
            }
            out.extend(self.finish_front());
            return out;
        }
        // a turn a chat handed over is a lookup, not a coding task: no review, tests or stronger model after it
        if self.handoff.is_some() {
            return self.finish_front();
        }
        // cascade: the cheap model on the own loop stopped with an error or changed nothing, so there is nothing to test. Not
        // for a chat's answer: a question changes nothing (each went on to Opus, 2026-11-21)
        if let (Some(_), Some((m, e)), false, false) = (&self.provider, self.escalate.clone(), self.escalated, self.profile == Profile::Chat) {
            let gave_up = self.runs.front().is_some_and(|r| !r.is_interrupted() && (!success || r.changed_files(&self.cwd).is_empty()));
            if gave_up {
                self.escalated = true;
                if let Some(run) = self.runs.front_mut() {
                    run.escalated();
                }
                let mut out = vec![self.record(json!({"type": "harness_escalate", "model": m, "effort": e}))];
                self.respawn(m, e);
                (self.provider, self.session_id) = (None, None);
                let why = if success { "changed no files" } else { "stopped with an error" };
                let msg = format!("{}\n\n---\nA cheaper model tried this task first and {why}; any changes it made are in the working tree (`git diff`). Do the task.", self.task);
                if let Err(e) = self.write_user(&msg) {
                    out.push(self.record(json!({"type": "harness_notice", "text": format!("could not hand the task over: {e:#}"), "error": true})));
                    out.extend(self.finish_front());
                }
                return out;
            }
        }
        let Some(run) = self.runs.front_mut() else { return vec![] };
        if !success || run.is_interrupted() {
            return self.finish_front();
        }
        let changed = run.changed_files(&self.cwd);
        if std::env::var_os("TAZUNA_ANSWER_REVIEW").is_some() && !run.reviewed && changed.is_empty() && self.profile != Profile::Chat {
            run.reviewed = true;
            return match self.write_user(ANSWER_REVIEW) {
                Ok(()) => vec![self.record(json!({"type": "harness_review"}))],
                Err(e) => {
                    let mut out = vec![self.record(json!({"type": "harness_notice", "text": format!("could not send the answer review: {e:#}"), "error": true}))];
                    out.extend(self.finish_front());
                    out
                }
            };
        }
        if self.self_review && !run.reviewed && !changed.is_empty() {
            // review before verification: its fixes get tested too
            run.reviewed = true;
            return match self.write_user(REVIEW) {
                Ok(()) => vec![self.record(json!({"type": "harness_review"}))],
                Err(e) => {
                    let mut out = vec![self.record(json!({"type": "harness_notice", "text": format!("could not send the self-review: {e:#}"), "error": true}))];
                    out.extend(self.finish_front());
                    out
                }
            };
        }
        // not while background tasks still run (boss mode's subagents): claude reports a result each time it waits for
        // them, and a review then sees half a change; the result after the last task ends triggers it instead
        if let Some((model, effort)) = self.review_model.clone().filter(|_| !self.task_reviewed && !changed.is_empty() && self.tasks.is_empty()) {
            // the reviewer gets the task and the diff in a fresh context; verification waits for its verdict
            self.task_reviewed = true;
            self.verifying = true;
            let (tx, g, cwd) = (self.tx.clone(), self.generation, self.cwd.clone());
            // with --lessons the reviewer checks against the repo's list too
            let task = match self.lessons.then(|| crate::lessons::load(&self.cwd)).flatten() {
                Some(l) => format!("{}\n\n{l}", self.task),
                None => self.task.clone(),
            };
            let (m, e, k) = (model.clone(), effort.clone(), self.review_key.clone());
            std::thread::spawn(move || {
                let _ = tx.send((g, Msg::Reviewed(review::run(&cwd, &task, &m, &e, k.as_deref()))));
            });
            return vec![self.record(json!({"type": "harness_review_start", "model": model, "effort": effort}))];
        }
        let Some(run) = self.runs.front_mut() else { return vec![] };
        let tests = verify::command(&self.verify, &self.cwd, &changed);
        // the baseline stays with the run, so fix rounds compare against the state before the run too. Waited for on
        // verification's thread: here it froze the screen until the type check ended, after every reply (2026-11-11)
        let types = verify::typecheck_command(&self.verify, &self.cwd).filter(|_| verify::touches_types(&changed)).zip(run.types_before.clone());
        if tests.is_none() && types.is_none() {
            return self.finish_front();
        }
        self.verifying = true;
        let round = run.failed_rounds() + 1;
        let cmd = [tests.clone(), types.as_ref().map(|t| t.0.clone())].into_iter().flatten().collect::<Vec<_>>().join(" + ");
        let (tx, g, cwd) = (self.tx.clone(), self.generation, self.cwd.clone());
        std::thread::spawn(move || {
            let types = types.and_then(|(cmd, b)| Some((cmd, b.lock().ok()?.get()?)));
            let _ = tx.send((g, Msg::Verified(verify::check(&cwd, tests, types))));
        });
        vec![self.record(json!({"type": "harness_verify_start", "cmd": cmd, "round": round}))]
    }

    /// The reviewer is back: findings go to claude once; otherwise the run moves on to verification.
    fn after_review(&mut self, r: review::Review) -> Vec<(f64, Value)> {
        self.verifying = false;
        let Some(run) = self.runs.front_mut() else { return vec![] };
        run.add_cost(r.cost);
        run.review = Some(json!({"ok": r.ok, "issues": r.issues.len(), "cost_usd": (r.cost * 1e6).round() / 1e6, "secs": (r.secs * 10.0).round() / 10.0, "error": r.error}));
        let interrupted = run.is_interrupted();
        let mut out = vec![self.record(json!({"type": "harness_review_done", "ok": r.ok, "issues": r.issues, "cost": r.cost, "secs": r.secs, "error": r.error}))];
        if r.ok || interrupted {
            out.extend(self.after_result());
            return out;
        }
        match self.write_user(&review::feedback(&r)) {
            Ok(()) => {}
            Err(e) => {
                out.push(self.record(json!({"type": "harness_notice", "text": format!("could not send the review: {e:#}"), "error": true})));
                out.extend(self.finish_front());
            }
        }
        out
    }

    /// Tests are back: pass ends the run; a failure goes back to claude while rounds remain.
    fn after_verify(&mut self, o: verify::Outcome) -> Vec<(f64, Value)> {
        self.verifying = false;
        if o.no_tests || o.timed_out || o.cannot_run {
            // the runner found nothing to run, the tests outran the time limit, or the command is not on this machine: not
            // verified, not failed
            let mut out = vec![self.record(json!({"type": "harness_verify_done", "cmd": o.cmd, "ok": o.no_tests, "no_tests": o.no_tests,
                "timed_out": o.timed_out, "cannot_run": o.cannot_run, "secs": o.secs, "output": "", "final": true}))];
            // the run log and a headless result say which (they read "claimed", as if nothing testable had changed)
            if let (Some(run), false) = (self.runs.front_mut(), o.no_tests) {
                run.no_verify(&o.cmd, if o.cannot_run { "could_not_run" } else { "timed_out" });
            }
            out.extend(self.finish_front());
            return out;
        }
        let Some(run) = self.runs.front_mut() else { return vec![] };
        run.add_verify(&o.cmd, o.ok, o.secs);
        let retry = !o.ok && !run.is_interrupted() && run.failed_rounds() <= verify::MAX_ROUNDS;
        let mut out = vec![self.record(json!({
            "type": "harness_verify_done", "cmd": o.cmd, "ok": o.ok, "code": o.code, "secs": o.secs, "output": o.output, "final": !retry,
        }))];
        if !o.preexisting.is_empty() {
            let text = format!("tests: {} fail on the last commit too, without this change: not counted against it", o.preexisting.join(", "));
            out.push(self.record(json!({"type": "harness_info", "text": text})));
        }
        if retry {
            let round = self.runs.front().map_or(1, |r| r.failed_rounds());
            if let Some((m, e)) = self.escalate.clone().filter(|_| !self.escalated) {
                // the cheap lane failed its tests: fix rounds run on the stronger model
                self.escalated = true;
                if let Some(run) = self.runs.front_mut() {
                    run.escalated();
                }
                out.push(self.record(json!({"type": "harness_escalate", "model": m, "effort": e})));
                self.respawn(m, e);
            }
            // own loop -> claude: a fresh claude session, which needs the task and what the cheap model left behind
            let cross = self.escalated && self.provider.take().is_some();
            if cross {
                self.session_id = None;
            }
            let feedback = verify::feedback(&o, round);
            let msg = if cross {
                format!("{}\n\n---\nA cheaper model already worked on this task; its changes are in the working tree (`git diff` shows them). Its result does not pass yet. Continue from there: fix what is wrong, or redo it.\n\n{feedback}", self.task)
            } else {
                feedback
            };
            match self.write_user(&msg) {
                Ok(()) => out.push(self.record(json!({"type": "harness_verify_feedback", "round": round, "max": verify::MAX_ROUNDS}))),
                Err(e) => {
                    out.push(self.record(json!({"type": "harness_notice", "text": format!("could not send test failures: {e:#}"), "error": true})));
                    out.extend(self.finish_front());
                }
            }
        } else {
            out.extend(self.finish_front());
        }
        out
    }

    /// Logs the front run, then sends any message typed while it was busy.
    fn finish_front(&mut self) -> Vec<(f64, Value)> {
        if let Some(run) = self.runs.pop_front() {
            let line = run.finish(self.last_result.as_ref(), self.elapsed_ms(), &self.meta(), &self.cwd);
            self.log(line);
        }
        self.send_held()
    }

    /// Nothing runs: the session's own conversation again after a turn a chat handed over, then what was held meanwhile, in
    /// order: a message, or a hand-off (`hand_off`).
    #[inline(never)]
    fn send_held(&mut self) -> Vec<(f64, Value)> {
        let mut out: Vec<(f64, Value)> = (self.runs.is_empty() && !self.verifying).then(|| self.hand_back()).flatten().into_iter().collect();
        while self.runs.is_empty() && !self.verifying {
            // a hand-off replaces the agent's process: not while its background commands run or may still wake it (`poll`
            // comes back for it)
            let busy = !self.tasks.is_empty() || self.tasks_done.is_some_and(|t| t.elapsed().as_secs() < 5);
            // a hand-off waits for them; a message behind it does not (with a dev server running in the background, every
            // later message waited until tazuna was restarted)
            let next = if busy { self.held.iter().position(|h| h.2.is_none()) } else { Some(0) };
            let Some((text, attach, agent)) = next.and_then(|i| self.held.remove(i)) else { break };
            let started = match agent {
                // the chat's question under its line; its process gives way to the agent's
                Some((sent, line)) => self.start_handoff(&text, &sent).map(|()| json!({"type": "harness_handoff", "text": line, "q": text})),
                // its message was shown when it was sent; its work begins now
                None => self.start_run(&text, attach, None).map(|()| json!({"type": "harness_held_start"})),
            };
            match started {
                Ok(e) => out.push(self.record(e)),
                Err(e) => {
                    out.extend(self.hand_back());
                    out.push(self.record(json!({"type": "harness_notice", "text": format!("{e:#}"), "error": true})));
                }
            }
        }
        out
    }

    /// A hand-off's turn begins (`hand_off`): the lean agent in a process and conversation of its own.
    fn start_handoff(&mut self, text: &str, sent: &str) -> Result<()> {
        self.handoff = Some((self.profile, self.session_id.take(), self.last_assistant.take()));
        self.profile = Profile::Lean;
        self.respawn(self.model.clone(), self.effort.clone());
        self.start_run(text, false, Some(sent))
    }

    /// The process is gone: runs without a result end here (stopped if the user asked, otherwise crashed).
    fn end_runs(&mut self) {
        // questions from a process that is gone can no longer be answered
        if !self.pending.is_empty() {
            self.pending.clear();
            let e = self.record(json!({"type": "harness_permission_cleared"}));
            self.outbox.push(e);
        }
        // ponytail: a dead process reports no cost, so these runs log $0; the session's estimate is in the recording
        while let Some(run) = self.runs.pop_front() {
            let line = run.finish(None, self.elapsed_ms(), &self.meta(), &self.cwd);
            self.log(line);
        }
        self.proc_cost = 0.0;
        self.verifying = false;
        let out = self.send_held();
        self.outbox.extend(out);
    }

    fn meta(&self) -> Value {
        json!({
            "model": self.model, "effort": self.effort, "profile": "cc-mid", "version": env!("CARGO_PKG_VERSION"), "mode": self.mode,
            "verify": match &self.verify { verify::Mode::Auto => "auto", verify::Mode::Command(_) => "command", verify::Mode::Off => "off" },
            "bash_cap": self.bash_cap, "diagnostics": self.diagnostics, "repo_map": self.repo_map, "explorer": self.explorer, "tester": self.tester, "cache_ttl": self.ttl(), "self_review": self.self_review, "repo_tools": if self.repo_tools_core { "core" } else { "all" }, "rtk": self.rtk, "batch_calls": self.batch_calls, "test_scope": self.test_scope, "lessons": self.lessons, "history": self.history, "boss_effort": self.boss_effort, "boss_pick": self.boss_pick, "boss_escalate": self.boss_escalate, "review_model": self.review_model.as_ref().map(|(m, e)| format!("{m} {e}")),
            "profile": self.profile.name(), "setting_sources": self.setting_sources(),
            "project_instructions": {"bytes": self.instructions_used.0, "files": self.instructions_used.1},
            "lane": self.lane.0, "lane_reason": self.lane.1, "escalated": self.escalated,
            "claude_version": self.claude_version, "cwd": self.cwd, "session_id": self.session_id(), "recording": self.rec_path,
            "host": host(),
            // which API the own loop ran on, or who billed claude (none on the plan): the usage page names it. `plan`: the
            // run log called every claude run plan, so the start page counted an API key's runs as nothing paid
            "provider": self.provider.as_ref().map(|(u, k)| crate::models::provider_name(u, k)).or_else(|| self.claude_bill.1.filter(|_| !self.claude_bill.0).map(String::from)),
            // the key's variable: two providers can share an API (Mistral and Mistral Vibe), and a resume goes on on the same
            "key_env": self.provider.as_ref().map(|(_, k)| k),
            "plan": self.provider.is_none() && self.claude_bill.0,
            // the session this one continues: the lists show them as one conversation (main.rs sessions_of)
            "continues": self.continues,
        })
    }

    fn log(&mut self, line: Value) {
        // a slash command alone (/cost, /clear, /mcp) is no agent work: it stays out of the run log, where it counted as a
        // run, stopped or at a negative cost (9 of 517 live runs, 2026-11-12)
        if line["prompt"].as_str().is_some_and(is_command) {
            return;
        }
        self.last_run = line["id"].as_str().map(String::from);
        self.last_log = Some(line.clone());
        self.logs.push(line.clone());
        // one write per line: parallel tazuna runs append to the same log without interleaving
        let appended = std::fs::OpenOptions::new().create(true).append(true).open(&self.runs_log).and_then(|mut f| f.write_all(format!("{line}\n").as_bytes()));
        if let Err(e) = appended {
            eprintln!("run log: {e}");
        }
    }

    /// Records an event from outside claude's stream (a plan reading by /usage or at the start), so the next session opens
    /// with it: `session::last_limits` reads the recordings.
    pub fn note(&mut self, e: Value) -> (f64, Value) {
        self.record(e)
    }

    fn record(&mut self, e: Value) -> (f64, Value) {
        self.heard(&e);
        let t = self.elapsed_ms();
        let _ = writeln!(self.rec, "{}", json!({"t": t as u64, "e": e}));
        (t, e)
    }

    /// Which settings files claude loads (recorded in the run log).
    /// Prompt cache lifetime: `--cache-ttl`, else 5m for headless runs without boss mode (turns come seconds apart and
    /// writes cost 1.25× instead of 2×: −10% per run, same judge, 0.0.20), else 1h. Live: a pause over 5 minutes would
    /// re-write it all. Boss mode: the lead waits on its workers past 5 minutes, and 1h also reads Claude Code's shared
    /// prefix on the first call (5m changes ScheduleWakeup's text, so it misses); 313 recorded runs re-priced: −3.2% on
    /// the boss baseline, +3% without boss (2026-11-09).
    fn ttl(&self) -> String {
        self.cache_ttl.clone().unwrap_or_else(|| if self.mode == "headless" && self.boss.is_none() { "5m" } else { "1h" }.into())
    }

    fn setting_sources(&self) -> &'static str {
        match (self.profile, self.user_settings) {
            (Profile::Lean | Profile::Chat, _) => "",
            (Profile::Repo | Profile::RepoLean, false) => "project,local",
            (Profile::Repo | Profile::RepoLean, true) => "user,project,local",
        }
    }

    fn today(&self) -> String {
        crate::platform::shell("date +%Y-%m-%d", &[]).output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default()
    }

    fn mid_prompt(&self) -> String {
        let date = self.today();
        let mut p = MID_PROMPT.replace("{cwd}", &self.cwd.to_string_lossy()).replace("{date}", &date);
        if self.self_review {
            p.push_str("\n- ");
            p.push_str(FINISH_RULES);
        }
        p
    }

    fn spawn(&mut self) -> Result<()> {
        if let Some((url, key_env)) = self.provider.clone() {
            // the engine asks before commands (ask, edits, plan) and edits (ask); plan refuses edits
            // the own loop: lean prompt and tools, the repo's instructions in the prompt
            // the own loop has no Grep/Glob tools: its runs searched with grep in bash anyway, and cut file reads at the
            // bash cap (2026-10-24)
            // with --search-tools the prompt must say the same as the tools (it said "grep in bash" through the whole
            // 2026-11-04 test of those tools)
            if self.profile == Profile::Chat {
                let prompt = chat_prompt(&self.today());
                let mut cmd = Command::new(std::env::current_exe().context("locating the tazuna binary")?);
                cmd.args(["engine", "--url", &url, "--key-env", &key_env, "--model", &self.model, "--effort", &self.effort, "--permission-mode", &self.permission_mode])
                    .env("TAZUNA_SYSTEM_PROMPT", &prompt)
                    .env("TAZUNA_NO_TOOLS", "1");
                if let Some(p) = &self.price {
                    cmd.env("TAZUNA_PRICE", p);
                }
                if std::env::var_os(&key_env).is_none() && let Some(k) = crate::models::key(&key_env) {
                    cmd.env(&key_env, k);
                }
                // no TAZUNA_HEADLESS: its nudge after a turn without a change made the own loop write general-set
                // answers into files
                self.own_resume(&mut cmd);
                return self.start(cmd);
            }
            let search = if std::env::var_os("TAZUNA_SEARCH_TOOLS").is_some() { "Search with the grep and glob tools, not in bash; read files with read." } else { "Search with grep -rn in bash; read whole files with read." };
            let mut prompt = self.mid_prompt().replace("Use Grep/Glob to search.", search);
            prompt.push_str(if std::env::var_os("TAZUNA_API_SCOPE").is_some() { API_SCOPE_RULE } else { API_RULE });
            prompt.push_str(PLAN_RULE);
            // experiment (TAZUNA_OWN_TEST_FOCUS): covering tests while working, and the changed packages' suites once by the
            // model before it finishes. The own loop's test runs still took 17-38 min of a hugo run (2026-11-11); the
            // rule below, which leaves the wider run to verify, ran faster and lost 0.83 judge (v1127)
            if std::env::var_os("TAZUNA_OWN_TEST_FOCUS").is_some() {
                prompt.push_str("\n- While working, run only the tests that cover your change (in Go: `go test -run 'TestA|TestB' ./pkg/`), not whole packages or the whole repository. Once, before you finish, run the full tests of every package you changed and fix what your change broke.");
            }
            // experiment (TAZUNA_OWN_TEST_SCOPE): the Claude path's test rule. DeepSeek spent about half of each k6 and hugo
            // run in `go test`, whole packages and `./...` included (650 s once), tests tazuna's verify runs anyway after
            // "run the wider suite once at the end"-style habits (2026-11-10)
            if std::env::var_os("TAZUNA_OWN_TEST_SCOPE").is_some() && self.test_scope && verify::covers(&self.verify, &self.cwd) {
                prompt.push_str("\n- ");
                prompt.push_str(TEST_SCOPE_RULE);
            }
            // the Claude path's 0.1.96 rule, on the own loop by default since 0.1.103: DeepSeek V4.1 Flash, 3 tasks x 3 runs,
            // judge 5.56 against 5.11 and hidden tests 44/54 against 39/54, at the same calls (2026-11-11).
            // TAZUNA_NO_COVER_ALL turns it off here too
            if std::env::var_os("TAZUNA_NO_COVER_ALL").is_none() {
                prompt.push_str("\n- ");
                prompt.push_str(COVER_ALL_RULE);
            }
            // experiment (TAZUNA_ANSWER_RULES, 2026-11-13): the rules above are written for a change; the suite's own-loop
            // round scored tazuna lowest on plans (it called the maintainers' targeted fix a symptom fix, twice) and reviews
            // (approved, missing defects outside the diff)
            if std::env::var_os("TAZUNA_DIRECT_ANSWERS").is_some() {
                prompt.push_str("\n- ");
                prompt.push_str(DIRECT_RULE);
            }
            if std::env::var_os("TAZUNA_ANSWER_RULES").is_some() {
                prompt.push_str("\n- ");
                prompt.push_str(ANSWER_RULE);
            }
            if std::env::var_os("TAZUNA_PRECISE_ANSWERS").is_some() {
                prompt.push_str("\n- ");
                prompt.push_str(PRECISE_ANSWER_RULE);
            }
            // neither in a handed-over turn, whose fetch goes out without a question (nothing of the project goes with it)
            if self.project_instructions && self.handoff.is_none() {
                let (text, bytes, sources) = project_instructions(&self.cwd, &["CLAUDE.md", ".claude/CLAUDE.md", "AGENTS.md"]);
                prompt.push_str(&text);
                self.instructions_used = (bytes, sources);
            }
            // the skills claude would have (behave.rs): listed, for the model to read when a task fits
            if self.handoff.is_none() {
                prompt.push_str(&crate::behave::skills_prompt(&crate::behave::skills(&self.cwd, self.behave_scope())));
            }
            if std::env::var_os("TAZUNA_NO_IDENTITY").is_none() {
                prompt = identity(&self.model, &url) + &prompt;
            }
            let mut cmd = Command::new(std::env::current_exe().context("locating the tazuna binary")?);
            cmd.args(["engine", "--url", &url, "--key-env", &key_env, "--model", &self.model, "--effort", &self.effort, "--permission-mode", self.turn_mode().0])
                .env("TAZUNA_SYSTEM_PROMPT", &prompt)
                .env("LEAN_BASH_CAP", self.bash_cap.to_string());
            if let Some(p) = &self.price {
                cmd.env("TAZUNA_PRICE", p);
            }
            // the managed SubRouter key lives in a file, not in tazuna's environment: the engine gets it as the variable it
            // reads, and takes it out of its own environment before any tool runs (engine.rs `connect`)
            if std::env::var_os(&key_env).is_none()
                && let Some(k) = crate::models::key(&key_env)
            {
                cmd.env(&key_env, k);
            }
            if self.connectors {
                cmd.env("TAZUNA_MCP", "1");
            }
            // the route's other checked providers as fallbacks, unless a bench run sets its own
            if std::env::var_os("TAZUNA_ROUTES").is_none() {
                let fb = crate::labels::dir().map(|d| crate::models::fallbacks(&d.join("routes.json"), &key_env)).unwrap_or_default();
                if !fb.is_empty() {
                    // a fallback's key may be saved in tazuna rather than set: the engine reads it from its environment
                    for var in fb.split("; ").filter_map(|r| r.split_once(' ').map(|(_, v)| v)) {
                        if std::env::var_os(var).is_none() && let Some(k) = crate::models::key(var) {
                            cmd.env(var, k);
                        }
                    }
                    cmd.env("TAZUNA_ROUTES", fb);
                }
            }
            if self.mode == "headless" {
                cmd.env("TAZUNA_HEADLESS", "1"); // a task, not a conversation: stopping without a change gets a nudge
            }
            self.own_resume(&mut cmd);
            return self.start(cmd);
        }
        let mut cmd = Command::new("claude");
        // model providers' keys stay out of claude's environment, which its Bash tool's commands inherit (models.rs
        // provider_key_vars); the explorer's own key stays, since claude starts the explorer
        let explorer_key = self.explorer_provider.as_ref().map(|e| e.1.clone());
        for k in crate::models::provider_key_vars() {
            if Some(&k) != explorer_key.as_ref() {
                cmd.env_remove(k);
            }
        }
        if self.artifacts {
            cmd.env("CLAUDE_CODE_ARTIFACT", "1");
        }
        // the user's own setting wins; otherwise the mode decides
        if std::env::var_os("CLAUDE_CODE_PROMPT_CACHE_TTL").is_none() {
            let ttl = self.ttl();
            cmd.env("CLAUDE_CODE_PROMPT_CACHE_TTL", ttl);
        }
        // --replay-user-messages: claude says when it takes each message, so one it folds into a running turn is counted
        // with that turn (poll)
        cmd.args(["-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose", "--include-partial-messages", "--replay-user-messages"])
            .args(["--model", &self.model, "--effort", &self.effort])
            // claude asks tazuna (stdio) instead of a terminal it does not have; "full" never asks
            .args(["--permission-prompt-tool", "stdio", "--allow-dangerously-skip-permissions", "--permission-mode", self.turn_mode().0])
            .args(["--setting-sources", self.setting_sources()]);
        let allowed = self.turn_mode().1;
        if !allowed.is_empty() {
            cmd.args(["--allowedTools", allowed]);
        }
        // the user's commands and skills made with /create (behave.rs), as a plugin where claude runs them itself
        if self.claude_commands() && let Some(d) = crate::behave::plugin_dir() {
            cmd.arg("--plugin-dir").arg(d);
        }
        // benchmarks (TAZUNA_NO_WEB): Claude's web search runs on Anthropic's servers, past any local network jail; OMP
        // found a held-out task's fix PR with it (2026-11-14)
        if std::env::var_os("TAZUNA_NO_WEB").is_some() {
            cmd.args(["--disallowedTools", "WebSearch,WebFetch"]);
        }
        // `--connectors` lets them in (2026-11-06): Claude Code now defers MCP tools (names only until one is used), and
        // the user's set (Jira, Slack, Claude Docs, 8 waiting for a login) measured 22.3k tokens a call against 18.3k
        if !matches!(self.profile, Profile::Lean | Profile::Chat) && !self.user_settings && !self.connectors {
            // MCP servers from the repo's .mcp.json only: without this, the user's claude.ai connectors (they come with the
            // account, not a settings file) load too, ~40k tokens of tool schemas per turn in a personal setup
            cmd.arg("--strict-mcp-config");
            let mcp = self.cwd.join(".mcp.json");
            if std::fs::symlink_metadata(&mcp).is_ok_and(|m| m.file_type().is_file()) {
                cmd.arg("--mcp-config").arg(&mcp);
            }
        }
        match self.profile {
            Profile::Repo => {
                // Claude Code's own prompt and tools; it loads the repo's CLAUDE.md (nested files, @imports), settings,
                // hooks and skills itself. Per-machine prompt sections move to the first message for cross-session caching.
                cmd.arg("--exclude-dynamic-system-prompt-sections");
                if self.repo_tools_core {
                    cmd.args(["--tools", &self.with_artifact(REPO_CORE_TOOLS)]);
                }
                let boss = self.boss.is_some();
                if let Some((url, key_env, model)) = self.explorer_provider.as_ref().filter(|_| !boss) {
                    let exe = std::env::current_exe().context("locating the tazuna binary")?;
                    let server = json!({"mcpServers": {"explore": {"type": "stdio", "command": exe,
                        "args": ["explore-mcp", "--url", url, "--key-env", key_env, "--model", model, "--effort", "max"],
                        "env": {"TAZUNA_EXPLORER_COSTS": self.explorer_costs}}}});
                    cmd.arg("--mcp-config").arg(server.to_string());
                }
                let strict = std::env::var_os("TAZUNA_EXPLORER_STRICT").is_some();
                let explore_rule = self.explorer_provider.as_ref().map(|_| EXPLORE_TOOL_RULE)
                    .or(self.explorer.as_ref().map(|_| if strict { EXPLORER_STRICT_RULE } else { EXPLORER_RULE }));
                // only where verify can test the repo: the rule tells claude that tazuna runs the tests at the end
                let scoped = self.test_scope && verify::covers(&self.verify, &self.cwd);
                let pick = if boss && self.boss_pick {
                    Some(boss_pick_rule(self.boss.as_deref().unwrap_or("haiku"), BOSS_STRONG))
                } else {
                    (boss && self.boss_escalate).then(|| boss_escalate_rule(BOSS_STRONG))
                };
                let lessons = self.lessons.then(|| crate::lessons::load(&self.cwd)).flatten();
                // experiment (TAZUNA_READ_CALLERS): Opus at low read about half of what Claude Code read and missed code
                // paths (k6's archive and cloud commands, hugo's template boundaries), 2026-11-10
                let callers = std::env::var_os("TAZUNA_READ_CALLERS").is_some().then_some(CALLERS_RULE);
                // experiment (TAZUNA_WRITE_TESTS): with the test rule, Opus at medium wrote a third less test code than
                // Claude Code (1,743 against 2,620 lines over 18 runs) and the judge's tests score was 0.50 lower, the
                // bulk of the gap (2026-11-10): the rule may read as "tazuna does the testing"
                let write_tests = (scoped && std::env::var_os("TAZUNA_WRITE_TESTS").is_some()).then_some(WRITE_TESTS_RULE);
                // every command, flag or API the task names, and their error cases (0.1.96): Claude Code tested each one
                // the k6 task names and hugo's error case in every run, tazuna missed `k6 cloud run` in all; with the rule,
                // six tasks at Opus medium: judge 6.67 like Claude Code's (6.39 before), cost +9%, still 25% under Claude
                // Code's (2026-11-11). TAZUNA_NO_COVER_ALL turns it off (bench baselines)
                let cover_all = std::env::var_os("TAZUNA_NO_COVER_ALL").is_none().then_some(COVER_ALL_RULE);
                let direct = std::env::var_os("TAZUNA_DIRECT_ANSWERS").is_some().then_some(DIRECT_RULE);
                let narrow = (!scoped && std::env::var_os("TAZUNA_WIDE_CHECKS").is_none()).then_some(NARROW_CHECKS_RULE);
                let quick = (!scoped && std::env::var_os("TAZUNA_QUICK_BUILD").is_some()).then_some(QUICK_BUILD_RULE);
                let append: Vec<&str> = [direct, callers, self.self_review.then_some(FINISH_RULES), boss.then_some(BOSS_RULE), pick.as_deref(), explore_rule.filter(|_| !boss), self.tester.as_ref().filter(|_| !boss).map(|_| TESTER_RULE), self.batch_calls.then_some(BATCH_RULE), scoped.then_some(TEST_SCOPE_RULE), narrow, quick, write_tests, cover_all, lessons.as_deref(), (self.history && has_history(&self.cwd)).then_some(HISTORY_RULE)]
                    .into_iter()
                    .flatten()
                    .collect();
                if !append.is_empty() {
                    cmd.args(["--append-system-prompt", &append.join("\n")]);
                }
                if let Some(m) = self.boss.as_deref() {
                    // the workers at medium unless told otherwise: at the session's xhigh they cost 46% more than no boss
                    // at all and scored no better; medium was the cheapest and best (2026-11-02)
                    let strong = (self.boss_pick || self.boss_escalate).then_some(BOSS_STRONG);
                    cmd.args(["--agents", &subagents(Some(m), Some(m), Some(m), Some(self.boss_effort.as_deref().unwrap_or(BOSS_EFFORT)), strong, scoped)]);
                } else if self.explorer.is_some() || self.tester.is_some() {
                    // the explorer and tester at medium too: without an effort of their own they ran at the session's (xhigh
                    // in the user's sessions), where the boss's Haiku workers cost 2.3x medium for no better score
                    cmd.args(["--agents", &subagents(self.explorer.as_deref(), self.tester.as_deref(), None, Some(BOSS_EFFORT), None, scoped)]);
                }
            }
            Profile::RepoLean => {
                // Claude Code loads CLAUDE.md (and its @imports) itself, but not AGENTS.md: that one goes in the prompt
                let mut prompt = self.mid_prompt();
                prompt.push_str(CHECK_RULES);
                let imported = ["CLAUDE.md", ".claude/CLAUDE.md"].iter().any(|f| std::fs::read_to_string(self.cwd.join(f)).is_ok_and(|t| t.contains("@AGENTS.md")));
                if self.project_instructions && !imported {
                    let (text, bytes, sources) = project_instructions(&self.cwd, &["AGENTS.md"]);
                    prompt.push_str(&text);
                    self.instructions_used = (bytes, sources);
                }
                let tools = if self.connectors { "Bash,Read,Edit,Write,Grep,Glob,Skill,ToolSearch" } else { "Bash,Read,Edit,Write,Grep,Glob,Skill" };
                cmd.arg("--system-prompt-file").arg(prompt_file(&prompt)?).args(["--tools", &self.with_artifact(tools)]);
            }
            Profile::Chat => {
                // the general set's "Opus alone": a one-line prompt, no tools, no settings, MCP or skills
                cmd.args(["--strict-mcp-config", "--disable-slash-commands", "--tools", &self.with_artifact("")])
                    .arg("--system-prompt")
                    .arg(chat_prompt(&self.today()));
            }
            Profile::Lean => {
                let mut prompt = self.mid_prompt();
                // neither in a handed-over turn, whose fetch goes out without a question (nothing of the project goes with it)
                if self.project_instructions && self.handoff.is_none() {
                    let (text, bytes, sources) = project_instructions(&self.cwd, &["CLAUDE.md", ".claude/CLAUDE.md", "AGENTS.md"]);
                    prompt.push_str(&text);
                    self.instructions_used = (bytes, sources);
                }
                // its slash commands are off: the user's skills listed here, their commands expanded by tazuna (behave.rs)
                if self.handoff.is_none() {
                    prompt.push_str(&crate::behave::skills_prompt(&crate::behave::skills(&self.cwd, self.behave_scope())));
                }
                // the lean tools are this binary's `mcp` mode
                let exe = std::env::current_exe().context("locating the tazuna binary")?;
                // a handed-over turn may need the web (a chat's question about live data): the lean tools' fetch
                let mut env = json!({"LEAN_BASH_CAP": self.bash_cap.to_string()});
                if self.handoff.is_some() {
                    env["LEAN_FETCH"] = json!("1");
                }
                let mcp = json!({"mcpServers": {"lean": {"type": "stdio", "command": exe, "args": ["mcp"], "env": env}}}).to_string();
                // cc-mid: lean prompt and tools, no skills or other MCP servers
                if !self.connectors {
                    cmd.arg("--strict-mcp-config");
                }
                cmd.arg("--disable-slash-commands")
                    .arg("--system-prompt-file")
                    .arg(prompt_file(&prompt)?)
                    .args(["--tools", &self.with_artifact(match (self.handoff.is_some(), self.connectors) {
                        // a handed-over turn: claude's web search, and no Grep or Glob, which read the folder without a question
                        (true, false) => "WebSearch",
                        (true, true) => "WebSearch,ToolSearch",
                        (false, false) => "Grep,Glob",
                        (false, true) => "Grep,Glob,ToolSearch",
                    }), "--mcp-config", &mcp]);
            }
        }
        // `/mcp on`: the user's own servers too (tazuna's list, for every model); remote ones through `tazuna mcp-proxy`.
        // Without ToolSearch claude sends every MCP tool's full definition: the lean profiles add it while MCP is on
        // (+150 tokens; claude then sends a large set as names only, as in the repo profile).
        if let Some(c) = self.connectors.then(crate::mcpc::claude_config).flatten() {
            cmd.arg("--mcp-config").arg(c);
        }
        let mut settings = json!({});
        if self.diagnostics && self.profile != Profile::Chat {
            if self.diag_dir.is_none() {
                self.diag_dir = Some(diag::prepare(&self.cwd)?);
            }
            let exe = std::env::current_exe().context("locating the tazuna binary")?;
            settings = serde_json::from_str(&diag::settings(&exe, self.diag_dir.as_ref().expect("set above")))?;
        }
        if self.boss.is_some() && self.profile == Profile::Repo {
            let exe = std::env::current_exe().context("locating the tazuna binary")?;
            let q = crate::platform::sh_quote(&exe);
            settings["hooks"]["PreToolUse"] = json!([{"matcher": "", "hooks": [{"type": "command", "command": format!("{q} boss-hook")}]}]);
        }
        if self.rtk && !matches!(self.profile, Profile::Lean | Profile::Chat) {
            // the hook rtk's own `rtk init` installs for Claude Code: it rewrites a Bash command to run through rtk
            let mut pre = settings["hooks"]["PreToolUse"].as_array().cloned().unwrap_or_default();
            pre.push(json!({"matcher": "Bash", "hooks": [{"type": "command", "command": "rtk hook claude"}]}));
            settings["hooks"]["PreToolUse"] = json!(pre);
        }
        if let Some(m) = &self.advisor {
            settings["advisorModel"] = json!(m);
        }
        // in auto (and full, which a switch can turn into auto) every command comes to tazuna: an allow rule in the
        // project's claude settings ("Bash(git push:*)") would let a force push run unasked (review, 2026-11-23). Ask
        // rules win over allow rules; the other modes keep the user's rules as claude reads them.
        // ponytail: a process started in ask, edits or plan and switched to auto keeps those rules; restart it if that matters
        if matches!(self.permission_mode.as_str(), "auto" | "bypassPermissions") && self.mode != "headless" && self.handoff.is_none() {
            settings["permissions"]["ask"] = json!(["Bash", "mcp__lean__bash"]);
        }
        if settings != json!({}) {
            cmd.args(["--settings", &settings.to_string()]);
        }
        if let Some(id) = &self.session_id {
            cmd.args(["--resume", id]);
            // after a conversation rewind: the session up to that reply, as a fork (the original stays as it was)
            if let Some(at) = self.resume_at.take() {
                cmd.args(["--resume-session-at", &at, "--fork-session"]);
            } else if std::mem::take(&mut self.fork_next) {
                cmd.arg("--fork-session");
            }
        }
        if self.mode != "headless" {
            cmd.env("CLAUDE_CODE_ENABLE_SDK_FILE_CHECKPOINTING", "1"); // /rewind's file checkpoints
        }
        self.start(cmd)
    }

    /// The own loop's engine on a resumed session (`-c`, `--resume`, or started again mid-session): it reads the history it
    /// saved (until 0.1.119 it was never passed, and an own-loop session always started empty). The chat profile's engine
    /// too: it resumed without the two below, so an edit or a retry there kept the message it went back before (review 12).
    fn own_resume(&mut self, cmd: &mut Command) {
        if let Some(id) = &self.session_id {
            cmd.args(["--resume", id]);
            // "edit from here", a retry: the history up to before that message, in a fork (the original stays as it was)
            if let Some(at) = self.resume_at.take() {
                cmd.args(["--resume-before", &at, "--fork-session"]);
            } else if std::mem::take(&mut self.fork_next) {
                cmd.arg("--fork-session"); // a side chat
            }
        }
    }

    /// Starts the agent process and the threads that read its output.
    fn start(&mut self, mut cmd: Command) -> Result<()> {
        // without the terminal, so nothing the agent's commands start prints over the screen (platform::detach)
        let mut child = crate::platform::detach(&mut cmd)
            .current_dir(&self.cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("starting claude (is it installed and on PATH?)")?;
        let (stdout, mut stderr) = (child.stdout.take().context("stdout")?, child.stderr.take().context("stderr")?);
        let g = self.generation;
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send((g, Msg::Line(line))).is_err() {
                    return;
                }
            }
        });
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let mut err = String::new();
            let _ = stderr.read_to_string(&mut err);
            let _ = tx.send((g, Msg::Exited(err)));
        });
        self.stdin = child.stdin.take();
        self.child = Some(child);
        self.shell_dir = Some(self.cwd.clone());
        Ok(())
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        if let Some(d) = self.diag_dir.take() {
            let _ = std::fs::remove_dir_all(d);
        }
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

/// The machine's name, in each run's log row: runs from several machines (synced to one for analysis) stay apart.
fn host() -> &'static str {
    static HOST: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    HOST.get_or_init(|| {
        Command::new("hostname").output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).filter(|h| !h.is_empty())
            .unwrap_or_else(|| "unknown".into())
    })
}

fn data_dir() -> Result<PathBuf> {
    let d = crate::platform::home().context("HOME not set")?.join(".local/share/tazuna");
    std::fs::create_dir_all(&d)?;
    Ok(d)
}

/// More than this much instruction text is cut (it is re-sent, cached, on every turn).
const MAX_INSTRUCTIONS: usize = 80_000;

/// The repo's own instructions for agents: root CLAUDE.md, .claude/CLAUDE.md and AGENTS.md in full, nested ones listed so
/// the agent reads them when it works in that directory (like Claude Code, which loads nested files lazily).
/// Returns (text for the system prompt, bytes included, files included).
// ponytail: no `@file` import expansion; add it when a repo relies on imports
/// The prompt as a file for `claude --system-prompt-file`, named by its content, in `~/.cache/tazuna/prompts` (0700): on
/// the command line, the repo's instructions in it made the process a match for an agent's `pkill -f`; in a shared temp
/// dir (/tmp on Linux) another user could plant the file or a symlink at the guessable name.
// ponytail: one small file per distinct prompt is kept; prune by age if the folder ever grows
fn prompt_file(prompt: &str) -> Result<PathBuf> {
    use std::hash::{Hash, Hasher};
    let dir = crate::platform::home().context("HOME")?.join(".cache/tazuna/prompts");
    crate::platform::private_dir(&dir, true).context("creating the prompt folder")?;
    crate::platform::make_private(&dir); // also when it existed
    let mut h = std::collections::hash_map::DefaultHasher::new();
    prompt.hash(&mut h);
    let path = dir.join(format!("{:016x}.md", h.finish()));
    if std::fs::read_to_string(&path).ok().as_deref() != Some(prompt) {
        // written beside and renamed: a reader never sees half a prompt
        let tmp = dir.join(format!("{:016x}.{}.tmp", h.finish(), std::process::id()));
        let mut f = crate::platform::private_file(&tmp, false).context("writing the prompt file")?;
        std::io::Write::write_all(&mut f, prompt.as_bytes())?;
        std::fs::rename(&tmp, &path).context("writing the prompt file")?;
    }
    Ok(path)
}

pub fn project_instructions(cwd: &Path, names: &[&str]) -> (String, usize, Vec<String>) {
    let mut body = String::new();
    let mut files = Vec::new();
    let root = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
    for &name in names {
        // regular files that really live inside the repo: a symlinked file or directory (e.g. `.claude` -> ~/.ssh) could
        // otherwise put any file on the machine into the prompt
        let path = cwd.join(name);
        let inside = path.canonicalize().is_ok_and(|real| real.starts_with(&root) && real == root.join(name));
        if !inside || !std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_file()) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        if text.trim().is_empty() || body.contains(text.trim()) {
            continue;
        }
        body.push_str(&format!("\n\n## From {name}\n\n{}", text.trim()));
        files.push(name.to_string());
    }
    let mut patterns: Vec<String> = names.iter().map(|n| format!("*{}", n.rsplit('/').next().unwrap_or(n))).collect();
    patterns.dedup();
    let nested: Vec<String> = Command::new("git")
        .args(["ls-files", "--"])
        .args(&patterns)
        .current_dir(cwd)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().filter(|l| l.contains('/') && !l.starts_with(".claude/")).take(30).map(String::from).collect())
        .unwrap_or_default();
    if files.is_empty() && nested.is_empty() {
        return (String::new(), 0, files);
    }
    if body.len() > MAX_INSTRUCTIONS {
        let cut = body.char_indices().map(|(i, _)| i).take_while(|&i| i <= MAX_INSTRUCTIONS).last().unwrap_or(0);
        body.truncate(cut);
        body.push_str("\n\n[tazuna: instructions cut here; read the rest of the file if you need it]");
    }
    let mut text = format!("\n\n# Project instructions\n\nThe repository's own instructions for coding agents. Follow them.{body}");
    if !nested.is_empty() {
        text.push_str(&format!("\n\nMore instruction files in this repo; read the one for a directory before you edit there: {}", nested.join(", ")));
    }
    let bytes = text.len();
    (text, bytes, files)
}

/// Live mode only runs where every change can be reviewed and undone.
pub fn in_git_repo() -> bool {
    Command::new("git").args(["rev-parse", "--is-inside-work-tree"]).output().is_ok_and(|o| o.status.success())
}

/// The own loop's first line: the model by name. A repo's CLAUDE.md ("This file provides guidance to Claude Code") made
/// DeepSeek answer "I'm Claude, made by Anthropic" (2026-11-05): with no name given, a model takes the one the prompt
/// suggests. TAZUNA_NO_IDENTITY leaves it out (the A/B).
fn identity(model: &str, url: &str) -> String {
    let host = url.split("//").nth(1).unwrap_or(url).split('/').next().unwrap_or_default();
    format!("You are {model}, run by tazuna through {host}. Instructions below that address Claude or Claude Code apply to you as this repo's coding agent.\n\n")
}

#[cfg(test)]
mod tests {
    use super::words;

    #[test]
    fn a_quoted_or_escaped_path_with_spaces_is_one_word() {
        assert_eq!(words(r#"see "/tmp/Screenshot 2026 at 10.00.png" and @'/a b.jpg' please"#),
            ["see", "/tmp/Screenshot 2026 at 10.00.png", "and", "@/a b.jpg", "please"]);
        assert_eq!(words(r"look at ~/Desktop/Screenshot\ 1.png now"), ["look", "at", "~/Desktop/Screenshot 1.png", "now"]);
        assert_eq!(words("it's fine, don't split"), ["it's", "fine,", "don't", "split"]);
        assert_eq!(words(r"C:\Users\me\shot.png"), [r"C:\Users\me\shot.png"]);
    }

    #[test]
    fn claude_commands_are_kept_by_folder() {
        let file = std::env::temp_dir().join(format!("tazuna-cmds-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&file);
        assert_eq!(super::claude_commands(&file, "/a"), None);
        super::save_claude_commands(&file, "/a", &["compact".into(), "my-skill".into()]);
        super::save_claude_commands(&file, "/b", &["init".into()]);
        assert_eq!(super::claude_commands(&file, "/a").unwrap(), ["compact", "my-skill"]);
        assert_eq!(super::claude_commands(&file, "/b").unwrap(), ["init"]);
        assert_eq!(super::claude_commands(&file, "/c"), None);
        std::fs::write(&file, "not json").unwrap();
        super::save_claude_commands(&file, "/a", &["clear".into()]);
        assert_eq!(super::claude_commands(&file, "/a").unwrap(), ["clear"]);
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn message_ids_are_version_4_uuids() {
        let (a, b) = (super::new_uuid(), super::new_uuid());
        assert_ne!(a, b);
        let parts: Vec<&str> = a.split('-').collect();
        assert_eq!(parts.iter().map(|p| p.len()).collect::<Vec<_>>(), [8, 4, 4, 4, 12]);
        assert!(parts[2].starts_with('4') && "89ab".contains(&parts[3][..1]));
    }

    use super::*;

    #[test]
    fn attaches_named_images() {
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
        let dir = std::env::temp_dir().join(format!("tazuna-img-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("shot.png"), b"\x89PNG").unwrap();
        let c = user_content("what is wrong in @shot.png ?", &dir);
        assert_eq!(c[0]["type"], "image");
        assert_eq!(c[0]["source"]["data"], base64(b"\x89PNG"));
        assert_eq!(c[1]["text"], "what is wrong in @shot.png ?");
        assert_eq!(user_content("no image here.png", &dir), json!("no image here.png"), "a path that is not a file stays text");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn prompt_goes_to_a_file_named_by_content() {
        let a = prompt_file("rules\n# AGENTS.md\nintegration tests").unwrap();
        assert_eq!(std::fs::read_to_string(&a).unwrap(), "rules\n# AGENTS.md\nintegration tests");
        assert_eq!(prompt_file("rules\n# AGENTS.md\nintegration tests").unwrap(), a, "the same prompt reuses its file");
        assert_ne!(prompt_file("other").unwrap(), a);
        assert!(!a.to_string_lossy().contains("integration"), "the path carries none of the text");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(a.parent().unwrap()).unwrap().permissions().mode() & 0o777, 0o700, "a private folder");
        }
    }

    #[test]
    fn boss_hook_blocks_only_the_lead() {
        assert!(boss_denies(&json!({"tool_name": "Read"})).is_some());
        assert!(boss_denies(&json!({"tool_name": "Bash"})).is_some());
        assert!(boss_denies(&json!({"tool_name": "Agent"})).is_none());
        assert!(boss_denies(&json!({"tool_name": "Read", "agent_id": "a1", "agent_type": "explorer"})).is_none());
    }

    #[test]
    fn history_rule_only_where_there_is_history() {
        let dir = std::env::temp_dir().join(format!("tazuna-hist-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let git = |a: &[&str]| assert!(Command::new("git").args(a).current_dir(&dir).output().unwrap().status.success());
        git(&["init", "-q"]);
        assert!(!has_history(&dir), "no commit");
        let commit = || git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "c"]);
        commit();
        assert!(!has_history(&dir), "one commit, as a benchmark's copy");
        commit();
        assert!(has_history(&dir));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn subagents_only_the_ones_on() {
        let both: Value = serde_json::from_str(&subagents(Some("haiku"), Some("sonnet"), None, None, None, false)).unwrap();
        assert_eq!((both["explorer"]["model"].as_str(), both["tester"]["model"].as_str()), (Some("haiku"), Some("sonnet")));
        let one: Value = serde_json::from_str(&subagents(None, Some("haiku"), None, None, None, false)).unwrap();
        assert!(both["explorer"].get("effort").is_none(), "no effort: the session's");
        let boss: Value = serde_json::from_str(&subagents(Some("haiku"), Some("haiku"), Some("haiku"), Some("high"), Some("sonnet"), true)).unwrap();
        assert!(["explorer", "implementer", "tester", "implementer-strong"].iter().all(|a| boss[*a]["effort"] == "high"));
        assert_eq!(boss["implementer-strong"]["model"], "sonnet");
        let scoped = |a: &str| boss[a]["prompt"].as_str().unwrap().ends_with(WORKER_TEST_SCOPE);
        assert!(scoped("implementer") && scoped("implementer-strong") && scoped("tester") && !scoped("explorer"), "test scope for the workers that run tests");
        assert!(!both["tester"]["prompt"].as_str().unwrap().contains("tazuna runs"));
        let rule = boss_pick_rule("haiku", BOSS_STRONG);
        assert!(rule.contains("haiku: low, medium, high, xhigh, max (default medium)") && rule.contains("sonnet: low, medium, high, xhigh, max (default high)"), "{rule}");
        assert!(one.get("explorer").is_none() && one["tester"]["tools"][0] == "Bash");
    }

    #[test]
    fn spots_turns_claude_starts_itself() {
        assert!(starts_turn(&json!({"type": "stream_event", "event": {"type": "message_start"}})));
        assert!(starts_turn(&json!({"type": "assistant", "message": {}})));
        assert!(!starts_turn(&json!({"type": "result"})));
        assert!(!starts_turn(&json!({"type": "system", "subtype": "init"})));
    }

    #[test]
    fn claudes_commands_come_before_its_first_turn_on_claude_only() {
        let mut live = Live::new("", "").unwrap();
        let own: Vec<String> = crate::session::OWN_COMMANDS.map(String::from).to_vec();
        // on Claude before its init: claude's too (kept or common ones), "compact" among them either way
        assert!(live.commands(&own).len() > own.len());
        // once claude listed its own, the session's list stands
        let listed = [own.clone(), vec!["my-skill".to_string()]].concat();
        assert_eq!(live.commands(&listed).len(), listed.len());
        // another provider: claude's commands do not run there
        live.provider = Some(("https://api.example".into(), "KEY".into()));
        assert_eq!(live.commands(&own), { let mut o = own.clone(); o.sort(); o });
    }

    #[test]
    fn mcp_switches_on_every_path() {
        let mut live = Live::new("", "").unwrap();
        for profile in [Profile::Lean, Profile::Repo] {
            live.profile = profile;
            assert!(live.set_connectors(true).is_ok() && live.connectors);
            assert!(live.set_connectors(false).is_ok() && !live.connectors);
        }
        live.provider = Some(("https://api.example".into(), "KEY".into()));
        assert!(live.set_connectors(true).is_ok() && live.connectors, "the own loop too");
    }

    #[test]
    fn profile_switches_and_remembers_where_chat_came_from() {
        let mut live = Live::new("", "").unwrap();
        live.profile = Profile::Repo;
        live.boss = Some("haiku".into());
        let (_, e) = live.set_profile(Profile::Chat).unwrap();
        assert_eq!(e["profile"], "chat");
        assert!(live.profile == Profile::Chat && live.prev_profile == Some(Profile::Repo));
        assert!(live.boss.is_none() && e["text"].as_str().unwrap().contains("boss mode off"), "boss needs the repo profile");
        let (_, again) = live.set_profile(Profile::Chat).unwrap();
        assert!(again["text"].as_str().unwrap().starts_with("already"));
        live.set_profile(Profile::Lean).unwrap();
        assert!(live.profile == Profile::Lean && live.prev_profile == Some(Profile::Chat));
    }

    #[test]
    fn cache_ttl_is_5m_only_headless_without_boss() {
        let mut live = Live::new("", "").unwrap();
        live.mode = "headless";
        assert_eq!(live.ttl(), "5m");
        live.boss = Some("haiku".into());
        assert_eq!(live.ttl(), "1h", "the lead waits on its workers past 5 minutes");
        live.mode = "live";
        live.boss = None;
        assert_eq!(live.ttl(), "1h");
        live.cache_ttl = Some("5m".into());
        assert_eq!(live.ttl(), "5m", "--cache-ttl wins");
    }

    #[test]
    fn a_shell_stops_counting_when_its_subagent_ends() {
        let mut live = Live::new("", "").unwrap();
        let line = |e: Value| (0, Msg::Line(e.to_string()));
        for e in [
            json!({"type": "system", "subtype": "task_started", "task_id": "agent", "task_type": "local_agent"}),
            json!({"type": "system", "subtype": "task_started", "task_id": "search", "task_type": "local_bash", "owned_by_subagent": true, "parent_task_id": "agent"}),
            json!({"type": "system", "subtype": "task_started", "task_id": "server", "task_type": "local_bash"}),
            json!({"type": "system", "subtype": "task_notification", "task_id": "agent", "status": "completed"}),
        ] {
            live.tx.send(line(e)).unwrap();
        }
        live.poll();
        assert_eq!(live.tasks.keys().collect::<Vec<_>>(), ["server"], "the subagent's shell is dropped, claude's own stays");
        live.tx.send(line(json!({"type": "system", "subtype": "task_notification", "task_id": "server", "status": "completed"}))).unwrap();
        live.poll();
        assert!(live.tasks.is_empty() && live.tasks_done.is_some());
        // an agent continued with SendMessage starts again under the same id: running again, in the count the UI shows
        live.tx.send(line(json!({"type": "system", "subtype": "task_started", "task_id": "agent", "task_type": "local_agent", "description": "again"}))).unwrap();
        live.poll();
        let agent = live.bg.iter().find(|t| t.id == "agent").unwrap();
        assert!(agent.ended.is_none() && agent.status == "running" && agent.description == "again", "{} {:?}", agent.status, agent.ended);
        std::fs::remove_file(&live.rec_path).ok();
    }

    #[test]
    fn a_tool_call_written_as_text_is_asked_every_time() {
        let mut live = Live::new("", "").unwrap();
        // ask mode: in full and auto tazuna answers a real call itself (`auto_allows`)
        (live.mode, live.permission_mode) = ("live", "default".into());
        let ask = |id: &str, from_text: bool| (0, Msg::Line(json!({"type": "control_request", "request_id": id,
            "request": {"subtype": "can_use_tool", "tool_name": "bash", "input": {"command": "uptime"}, "from_text": from_text}}).to_string()));
        live.tx.send(ask("p1", true)).unwrap();
        live.poll();
        assert_eq!(live.pending.len(), 1, "asked");
        live.answer(true, true); // "always", on a text-written call
        live.tx.send(ask("p2", true)).unwrap();
        live.poll();
        assert_eq!(live.pending.len(), 1, "asked again: no always for it");
        live.answer(true, false);
        // the same command as a real call is not covered either: nothing was stored
        live.tx.send(ask("p3", false)).unwrap();
        live.poll();
        assert_eq!(live.pending.len(), 1);
        std::fs::remove_file(&live.rec_path).ok();
    }

    #[test]
    fn a_reply_does_not_wait_for_the_type_check_baseline() {
        let mut live = Live::new("", "").unwrap();
        let mut run = Run::new("question", 0.0, &live.cwd);
        // the baseline of a slow type check, still running when the reply comes
        let slow = std::thread::spawn(|| {
            std::thread::sleep(Duration::from_secs(3));
            String::new()
        });
        run.types_before = Some(std::sync::Arc::new(std::sync::Mutex::new(crate::runlog::Baseline::Pending(slow))));
        live.runs.push_back(run);
        live.tx.send((0, Msg::Line(json!({"type": "result", "subtype": "success", "is_error": false}).to_string()))).unwrap();
        let t = Instant::now();
        live.poll();
        // under the baseline's 3 s with room to spare: at 1 s the run's own git snapshot of this repo, with the whole test
        // suite running beside it, failed it about once in two full runs (2026-11-24)
        assert!(t.elapsed() < Duration::from_millis(2500), "the screen froze {:?} for the baseline", t.elapsed());
        std::fs::remove_file(&live.rec_path).ok();
    }

    #[test]
    fn a_test_command_that_is_not_installed_ends_the_run_without_a_fix_round() {
        let mut live = Live::new("", "").unwrap();
        live.runs_log = std::env::temp_dir().join(format!("tazuna-test-runs-{}.jsonl", std::process::id()));
        live.escalate = Some(("opus".into(), "high".into()));
        live.verifying = true;
        live.runs.push_back(Run::new("change it", 0.0, &live.cwd));
        let o = verify::Outcome { cmd: "cargo test --quiet".into(), ok: false, no_tests: false, timed_out: false, cannot_run: true, code: Some(127), secs: 0.1,
            output: "sh: cargo: command not found".into(), failed: vec![], preexisting: vec![] };
        let events: Vec<Value> = live.after_verify(o).into_iter().map(|(_, e)| e).collect();
        let kinds: Vec<&str> = events.iter().filter_map(|e| e["type"].as_str()).collect();
        assert_eq!(kinds, ["harness_verify_done"], "no escalation, no feedback to the model");
        assert!(events[0]["cannot_run"] == true && events[0]["final"] == true && events[0]["ok"] == false);
        assert!(live.runs.is_empty() && !live.escalated && !live.verifying, "the run ended");
        let line = live.logs.last().expect("logged");
        assert_eq!((line["verify"]["status"].as_str(), line["verify"]["cmd"].as_str(), line["verify"]["rounds"].as_u64()), (Some("could_not_run"), Some("cargo test --quiet"), Some(0)));
        std::fs::remove_file(&live.runs_log).ok();
        std::fs::remove_file(&live.rec_path).ok();
    }

    const ALL: &[&str] = &["CLAUDE.md", ".claude/CLAUDE.md", "AGENTS.md"];

    #[test]
    fn repo_lean_adds_only_agents_md() {
        let dir = std::env::temp_dir().join(format!("tazuna-agents-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("CLAUDE.md"), "claude rules").unwrap();
        std::fs::write(dir.join("AGENTS.md"), "agent rules").unwrap();
        let (text, _, files) = project_instructions(&dir, &["AGENTS.md"]);
        assert!(text.contains("agent rules") && !text.contains("claude rules"), "Claude Code loads CLAUDE.md itself");
        assert_eq!(files, vec!["AGENTS.md"]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_own_loop_names_its_model() {
        let line = super::identity("deepseek-v4.1-flash-1017", "https://subrouter.ai/v1");
        assert!(line.starts_with("You are deepseek-v4.1-flash-1017, run by tazuna through subrouter.ai."), "{line}");
    }

    #[test]
    fn reads_project_instructions() {
        let dir = std::env::temp_dir().join(format!("tazuna-instr-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("app")).unwrap();
        let git = |args: &[&str]| assert!(Command::new("git").args(args).current_dir(&dir).output().unwrap().status.success());
        git(&["init", "-q"]);
        assert_eq!(project_instructions(&dir, ALL).1, 0, "no instruction files, nothing added");
        std::fs::write(dir.join("CLAUDE.md"), "Use npm run test:unit -- <file>.\n").unwrap();
        std::fs::write(dir.join("AGENTS.md"), "Use npm run test:unit -- <file>.\n").unwrap();
        std::fs::write(dir.join("app/CLAUDE.md"), "App rules.\n").unwrap();
        git(&["add", "."]);
        let (text, bytes, files) = project_instructions(&dir, ALL);
        assert_eq!(files, vec!["CLAUDE.md"], "identical AGENTS.md is not repeated");
        assert!(text.contains("test:unit") && text.contains("app/CLAUDE.md") && !text.contains("App rules"), "{text}");
        assert_eq!(bytes, text.len());
        std::fs::write(dir.join("CLAUDE.md"), "x".repeat(MAX_INSTRUCTIONS + 5000)).unwrap();
        assert!(project_instructions(&dir, ALL).0.contains("instructions cut here"));
        std::fs::remove_file(dir.join("CLAUDE.md")).unwrap();
        std::fs::remove_file(dir.join("AGENTS.md")).unwrap();
        #[cfg(unix)] // symlinks need admin rights on Windows
        {
            std::fs::write(dir.join("secret.txt"), "TOP SECRET").unwrap();
            std::os::unix::fs::symlink(dir.join("secret.txt"), dir.join("CLAUDE.md")).unwrap();
            assert!(!project_instructions(&dir, ALL).0.contains("TOP SECRET"), "symlinked instruction files are never read");
            std::fs::remove_file(dir.join("CLAUDE.md")).unwrap();
            let outside = std::env::temp_dir().join(format!("tazuna-outside-{}", std::process::id()));
            std::fs::create_dir_all(&outside).unwrap();
            std::fs::write(outside.join("CLAUDE.md"), "OUTSIDE SECRET").unwrap();
            std::os::unix::fs::symlink(&outside, dir.join(".claude")).unwrap();
            assert!(!project_instructions(&dir, ALL).0.contains("OUTSIDE SECRET"), "a symlinked .claude directory is not followed");
            std::fs::remove_dir_all(&outside).ok();
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_chat_reply_hands_off_only_when_it_starts_with_the_marker() {
        assert!(hands_off("[[agent]]") && hands_off("  \n[[agent]]\n ") && hands_off("[[agent]] It needs today's forecast."));
        assert!(!hands_off("Berlin is mild in May; for today's weather the [[agent]] can look."), "mentioned later: an answer");
        assert!(!hands_off("") && !hands_off("  ") && !hands_off("[[agen"));
    }

    #[test]
    fn the_hand_off_line_is_in_the_chat_prompt_unless_switched_off() {
        let with = |v: Option<&str>| {
            // SAFETY: only this test reads or writes TAZUNA_CHAT_HANDOFF
            unsafe {
                match v {
                    Some(v) => std::env::set_var("TAZUNA_CHAT_HANDOFF", v),
                    None => std::env::remove_var("TAZUNA_CHAT_HANDOFF"),
                }
            }
            chat_prompt("2026-11-21")
        };
        for v in [None, Some("1"), Some("2"), Some("")] {
            let p = with(v);
            assert!(p.starts_with("You are a helpful assistant. Today is 2026-11-21. One exception") && p.contains("your whole reply is [[agent]]"), "{v:?}: {p}");
        }
        assert_eq!(with(Some("0")), "You are a helpful assistant. Today is 2026-11-21.", "benchmarks of the plain model");
        with(None);
    }

    #[test]
    fn a_handed_over_turn_allows_only_the_web() {
        let mut live = Live::new("", "").unwrap();
        live.profile = Profile::Lean;
        live.permission_mode = "bypassPermissions".into();
        assert_eq!(live.turn_mode(), ("bypassPermissions", "mcp__lean__read,mcp__lean__write,mcp__lean__edit"), "bash comes to tazuna");
        live.permission_mode = "auto".into();
        assert_eq!(live.turn_mode(), ("acceptEdits", "mcp__lean__read,mcp__lean__write,mcp__lean__edit"), "the same tools as full: a switch changes none");
        live.permission_mode = "bypassPermissions".into();
        live.handoff = Some((Profile::Chat, None, None));
        // claude, whatever the session's mode: ask mode, the lean fetch and claude's web search allowed (a read asks)
        assert_eq!(live.turn_mode(), ("default", "mcp__lean__fetch,WebSearch"));
        live.permission_mode = "acceptEdits".into();
        assert_eq!(live.turn_mode(), ("default", "mcp__lean__fetch,WebSearch"));
        // the own loop: its handoff mode (engine.rs asks)
        live.provider = Some(("https://api.example".into(), "KEY".into()));
        assert_eq!(live.turn_mode(), ("handoff", ""));
        live.handoff = None;
        assert_eq!(live.turn_mode(), ("acceptEdits", ""), "every other turn: the session's own mode");
        std::fs::remove_file(&live.rec_path).ok();
    }

    /// claude's start for a handed-over turn from each profile: the lean agent, alone, in ask mode with only the web allowed;
    /// claude's own Read, Grep and Glob (which read the folder without a question) left out; no subagents, boss hook or
    /// plugins; the web at hand.
    #[cfg(unix)]
    #[test]
    fn a_handed_over_turn_on_claude_is_the_lean_agent_with_only_the_web_allowed() {
        for (profile, connectors) in [(Profile::Chat, false), (Profile::Lean, true), (Profile::Repo, false), (Profile::RepoLean, false)] {
            let (mut live, dir) = stub::live(&format!("handoff-tools-{}", profile.name()));
            (live.profile, live.permission_mode, live.connectors) = (profile, "bypassPermissions".into(), connectors);
            (live.boss, live.explorer) = (Some("haiku".into()), Some("sonnet".into()));
            live.hand_off("weather in Berlin now?", "weather in Berlin now?", HANDOFF_NEEDS).unwrap();
            let a = stub::started(&dir);
            let f = |name: &str| stub::flag(&a, name);
            assert_eq!((f("--permission-mode").as_deref(), f("--allowedTools").as_deref()), (Some("default"), Some("mcp__lean__fetch,WebSearch")), "{profile:?}");
            assert_eq!(f("--tools").as_deref(), Some(if connectors { "WebSearch,ToolSearch" } else { "WebSearch" }), "{profile:?}: no Read, Grep or Glob");
            assert!(f("--mcp-config").is_some_and(|c| c.contains("LEAN_FETCH")) && f("--resume").is_none(), "{profile:?}: the lean tools with fetch, a fresh conversation: {a:?}");
            assert!(!a.iter().any(|x| x == "--agents" || x == "--plugin-dir" || x.contains("boss-hook") || x == "--append-system-prompt"), "{profile:?}: one plain agent: {a:?}");
            assert!(stub::sent(&dir, "weather in Berlin now?"));
            drop(live);
            std::fs::remove_dir_all(&dir).ok();
        }
    }

    /// The session's own "always" answers and its repo's settings (their allow rules and hooks) do not reach a handed-over
    /// turn: what a fetched page asks for is asked.
    #[cfg(unix)]
    #[test]
    fn a_handed_over_turn_ignores_always_answers_and_the_repos_settings() {
        let (mut live, dir) = stub::live("handoff-always");
        (live.profile, live.permission_mode, live.user_settings) = (Profile::Repo, "default".into(), true);
        live.allowed.insert("mcp__lean__read".into());
        let ask = |live: &Live, id: &str| stub::feed(live, json!({"type": "control_request", "request_id": id,
            "request": {"subtype": "can_use_tool", "tool_name": "mcp__lean__read", "input": {"path": "/Users/x/.ssh/id_ed25519"}}}));
        live.hand_off("weather in Berlin now?", "weather in Berlin now?", HANDOFF_NEEDS).unwrap();
        assert_eq!(stub::flag(&stub::started(&dir), "--setting-sources").as_deref(), Some(""), "no allow rules or hooks from any settings file");
        ask(&live, "perm-1");
        let out = live.poll();
        assert!(out.iter().any(|(_, e)| e["type"] == "harness_permission") && live.pending.len() == 1, "asked, though always allowed: {out:?}");
        live.answer(false, false);
        stub::feed(&live, json!({"type": "result", "subtype": "success", "is_error": false, "result": "12 °C", "total_cost_usd": 0.0}));
        live.poll();
        // the session's own work again: its answers hold
        assert!(!live.handing_off());
        live.send_typed("go on").unwrap();
        assert_eq!(stub::flag(&stub::started(&dir), "--setting-sources").as_deref(), Some("user,project,local"));
        ask(&live, "perm-2");
        assert!(!live.poll().iter().any(|(_, e)| e["type"] == "harness_permission") && live.pending.is_empty());
        drop(live);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A chat's events on the stand-in: `r` as the result of the run in progress.
    #[cfg(unix)]
    fn result(live: &Live, r: &str) {
        stub::feed(live, json!({"type": "result", "subtype": "success", "is_error": false, "result": r, "total_cost_usd": 0.0}));
    }

    /// A message sent while a turn runs: claude's echo before that turn's result means it folded the message into the turn
    /// (one result for both), so its run joins the running one; an echo after the result is a turn of its own.
    #[cfg(unix)]
    #[test]
    fn a_message_claude_folds_into_the_running_turn_joins_its_run() {
        let echo = |live: &Live, text: &str| stub::feed(live, json!({"type": "user", "isReplay": true, "message": {"role": "user", "content": text}}));
        let (mut live, dir) = stub::live("replay-join");
        live.send_typed("run the tests").unwrap();
        assert!(stub::started(&dir).iter().any(|a| a == "--replay-user-messages"), "claude says when it takes each message");
        echo(&live, "run the tests");
        live.send_typed("and the linter too").unwrap();
        assert_eq!(live.runs.len(), 2);
        echo(&live, "and the linter too");
        live.poll();
        assert_eq!((live.runs.len(), live.runs[0].prompt.as_str()), (1, "run the tests\n\nand the linter too"), "folded into the turn: one run");
        result(&live, "Both pass.");
        live.poll();
        assert!(live.runs.is_empty() && live.idle(), "the turn's one result ends it");
        // taken after the result: a turn and a run of its own
        live.send_typed("thanks").unwrap();
        live.send_typed("one more").unwrap();
        result(&live, "You're welcome.");
        echo(&live, "one more");
        live.poll();
        assert_eq!((live.runs.len(), live.runs[0].prompt.as_str()), (1, "one more"));
        result(&live, "Done.");
        live.poll();
        assert!(live.runs.is_empty());
        drop(live);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// After a hand-off the chat's own conversation goes on as it was, the marker in it (no fork, no rewind: one
    /// conversation), and its next message carries the question and the agent's answer, as its own earlier answer.
    #[cfg(unix)]
    #[test]
    fn after_a_hand_off_the_chat_goes_on_in_its_own_conversation_with_the_agents_answer() {
        let (mut live, dir) = stub::live("handoff-chat");
        (live.profile, live.permission_mode) = (Profile::Chat, "bypassPermissions".into());
        live.resume("chat-1");
        live.last_assistant = Some("reply-0".into());
        live.send_typed("weather in Berlin now?").unwrap();
        assert_eq!(stub::flag(&stub::started(&dir), "--resume").as_deref(), Some("chat-1"));
        result(&live, "[[agent]]");
        assert_eq!(live.poll().last().map(|(_, e)| e["text"].clone()), Some(json!(HANDOFF_NEEDS)));
        assert_eq!(live.handed(), None, "a chat's: the main loops ask on every result");
        stub::started(&dir);
        result(&live, "12 °C and rain");
        live.poll();
        assert!(live.profile == Profile::Chat && !live.handing_off());
        std::fs::remove_file(dir.join("stdin")).ok();
        live.send_typed("and in Fahrenheit?").unwrap();
        let a = stub::started(&dir);
        assert_eq!(stub::flag(&a, "--resume").as_deref(), Some("chat-1"));
        assert!(!a.iter().any(|x| x == "--fork-session" || x == "--resume-session-at"), "the same conversation: {a:?}");
        assert!(stub::sent(&dir, "Earlier in this chat you were asked: weather in Berlin now?\nYou answered: 12 °C and rain\n\nThe new message: and in Fahrenheit?"));
        // both messages are points /rewind can go back to
        assert_eq!(live.checkpoints.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(), ["weather in Berlin now?", "and in Fahrenheit?"]);
        // the chat's answer to it: its conversation holds the agent's answer now
        result(&live, "54 °F");
        live.poll();
        assert_eq!(live.told, live.said.len());
        drop(live);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A follow-up the chat hands over ("and tomorrow?") reaches the agent with the chat's exchanges before it, so the agent
    /// knows what it is about (it got the bare message, review 2026-11-22).
    #[cfg(unix)]
    #[test]
    fn a_follow_up_hand_off_gives_the_agent_the_chats_earlier_exchanges() {
        let (mut live, dir) = stub::live("handoff-follow-up");
        (live.profile, live.permission_mode) = (Profile::Chat, "bypassPermissions".into());
        live.resume("chat-1");
        live.send_typed("I fly to Berlin on Friday").unwrap();
        stub::started(&dir);
        result(&live, "Have a good trip!");
        live.poll();
        live.send_typed("weather there now?").unwrap();
        result(&live, "[[agent]]");
        live.poll();
        stub::started(&dir);
        assert!(stub::sent(&dir, "Earlier in this chat you were asked: I fly to Berlin on Friday\nYou answered: Have a good trip!\n\nThe new message: weather there now?"));
        result(&live, "12 °C and rain");
        live.poll();
        live.send_typed("and tomorrow?").unwrap();
        stub::started(&dir);
        result(&live, "[[agent]]");
        live.poll();
        stub::started(&dir);
        std::thread::sleep(Duration::from_millis(100));
        let sent = std::fs::read_to_string(dir.join("stdin")).unwrap();
        let agent: Value = serde_json::from_str(sent.lines().last().unwrap()).unwrap();
        assert_eq!(agent["message"]["content"], "Earlier in this chat you were asked: I fly to Berlin on Friday\nYou answered: Have a good trip!\n\n\
            Earlier in this chat you were asked: weather there now?\nYou answered: 12 °C and rain\n\nThe new message: and tomorrow?");
        drop(live);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A hand-off that cannot start now waits its turn and runs in order, never refused: the chat hands a message over while
    /// another waits behind it (the waiting one started first, and the hand-off failed as "still at work"), and /agent while
    /// the chat is at work.
    #[cfg(unix)]
    #[test]
    fn a_hand_off_that_cannot_start_now_waits_its_turn() {
        let (mut live, dir) = stub::live("handoff-queue");
        (live.profile, live.permission_mode) = (Profile::Chat, "bypassPermissions".into());
        live.resume("chat-1");
        live.send_agent("weather in Berlin now?").unwrap();
        stub::started(&dir);
        live.send_typed("and in Paris?").unwrap();
        live.send_typed("tell me a joke").unwrap();
        result(&live, "12 °C and rain");
        live.poll();
        stub::started(&dir);
        // the chat hands "and in Paris?" over while "tell me a joke" waits: the hand-off goes first
        result(&live, "[[agent]]");
        let out = live.poll();
        assert!(!out.iter().any(|(_, e)| e["error"] == true), "{out:?}");
        assert!(out.iter().any(|(_, e)| e["type"] == "harness_handoff" && e["q"] == "and in Paris?") && live.handing_off() && live.held.len() == 1, "{out:?}");
        assert_eq!(stub::flag(&stub::started(&dir), "--tools").as_deref(), Some("WebSearch"));
        assert!(stub::sent(&dir, "The new message: and in Paris?"));
        result(&live, "9 °C");
        live.poll();
        // then the joke, in the chat's own conversation, with the answer it does not hold yet
        assert_eq!(stub::flag(&stub::started(&dir), "--resume").as_deref(), Some("chat-1"));
        assert!(stub::sent(&dir, "You answered: 9 °C\n\nThe new message: tell me a joke"));
        // /agent while the chat is at work: shown at once, taken once it is done
        let (_, e) = live.send_agent("how full is my disk?").unwrap();
        assert!(e["type"] == "harness_user" && !live.handing_off());
        result(&live, "A joke.");
        assert!(live.poll().iter().any(|(_, e)| e["type"] == "harness_handoff" && e["q"] == "how full is my disk?") && live.handing_off());
        drop(live);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A hand-off waiting for background commands (a dev server) lets the messages behind it run, and a stop forgets those
    /// commands: with both, every later message waited until tazuna was restarted (review of 0.1.194).
    #[cfg(unix)]
    #[test]
    fn a_hand_off_waiting_for_background_commands_does_not_hold_the_messages_behind_it() {
        let (mut live, dir) = stub::live("handoff-busy");
        (live.profile, live.permission_mode) = (Profile::Chat, "bypassPermissions".into());
        live.resume("chat-1");
        live.send_typed("start the dev server").unwrap();
        stub::started(&dir);
        // while it runs: a hand-off is queued, then a message behind it
        live.held.push_back(("weather in Berlin now?".into(), false, Some(("weather in Berlin now?".into(), "answered by the agent".into()))));
        live.send_typed("is it up?").unwrap();
        assert_eq!(live.held.len(), 2);
        // the turn ends with the dev server still running in the background: the message goes, the hand-off waits
        live.tasks.insert("dev".into(), None);
        result(&live, "Started.");
        live.poll();
        assert!(live.held.len() == 1 && live.held[0].2.is_some() && !live.runs.is_empty() && !live.handing_off(), "the message runs, the hand-off waits");
        // with only the waiting hand-off held and nothing running, a new message starts at once
        result(&live, "It is up.");
        live.poll();
        let (_, e) = live.send_typed("and the logs?").unwrap();
        assert!(e["type"] == "harness_user" && !live.runs.is_empty() && live.held.len() == 1);
        result(&live, "Clean.");
        live.poll();
        live.kill("stopped");
        assert!(live.tasks.is_empty(), "a stop forgets the background commands");
        // and the waiting hand-off starts as the stop ends the run
        assert!(live.handing_off() && live.held.is_empty() && live.poll().iter().any(|(_, e)| e["type"] == "harness_handoff"));
        drop(live);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A mode change while a hand-off runs (shift-tab, the window's menu, a side chat's plan mode) is kept for the session:
    /// the hand-off's process never gets it, so it keeps asking; the session's next process starts in it.
    #[cfg(unix)]
    #[test]
    fn a_mode_change_during_a_hand_off_waits_for_the_session() {
        let (mut live, dir) = stub::live("handoff-mode");
        (live.profile, live.permission_mode) = (Profile::Chat, "default".into());
        live.resume("chat-1");
        live.send_agent("how full is my disk?").unwrap();
        stub::started(&dir);
        live.set_mode("bypassPermissions");
        assert!(!stub::sent(&dir, "set_permission_mode"), "the hand-off's process keeps asking");
        result(&live, "40% used");
        live.poll();
        live.send_typed("thanks").unwrap();
        assert_eq!(stub::flag(&stub::started(&dir), "--permission-mode").as_deref(), Some("bypassPermissions"), "the session's own process: the new mode");
        drop(live);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Auto runs claude as acceptEdits and answers its questions itself, except a destructive command (with the rule it
    /// met) and the user's own questions; full is claude's bypassPermissions, and a switch reaches the running process.
    #[cfg(unix)]
    #[test]
    fn auto_answers_all_but_the_destructive_commands() {
        let (mut live, dir) = stub::live("auto-guard");
        live.set_mode("auto");
        live.send_typed("tidy up").unwrap();
        let args = stub::started(&dir);
        assert_eq!(stub::flag(&args, "--permission-mode").as_deref(), Some("acceptEdits"));
        // every command comes to tazuna: an allow rule in the project's claude settings cannot let one past
        assert!(stub::flag(&args, "--settings").is_some_and(|x| x.contains(r#""ask":["Bash","mcp__lean__bash"]"#)), "{args:?}");
        let ask = |live: &Live, id: &str, tool: &str, cmd: &str| stub::feed(live, json!({"type": "control_request", "request_id": id,
            "request": {"subtype": "can_use_tool", "tool_name": tool, "input": {"command": cmd}}}));
        ask(&live, "r-1", "Bash", "ls -la && rm -rf target");
        assert!(live.poll().iter().all(|(_, e)| e["type"] != "harness_permission") && live.pending.is_empty(), "runs without a question");
        // what claude was sent, as the stub copies it in: a moment after the write, so it is waited for
        let got = |want: &str| (0..80).any(|_| std::fs::read_to_string(dir.join("stdin")).is_ok_and(|s| s.contains(want)) || { std::thread::sleep(Duration::from_millis(25)); false });
        assert!(got(r#""request_id":"r-1""#) && got(r#""behavior":"allow""#));
        ask(&live, "r-2", "Bash", "git push --force");
        assert!(live.poll().iter().any(|(_, e)| e["type"] == "harness_permission"));
        assert_eq!(live.pending.front().unwrap()["request"]["risky"], "git push --force: rewrites the remote's history");
        ask(&live, "r-3", "AskUserQuestion", "");
        live.poll();
        assert_eq!(live.pending.len(), 2, "the user's own question waits for the user");
        // the lean bash keeps its folder: after an allowed `cd` out of the project (this one is in the temporary folder),
        // a relative removal is outside it
        live.pending.clear();
        ask(&live, "r-4", "mcp__lean__bash", "cd /usr");
        live.poll();
        ask(&live, "r-5", "mcp__lean__bash", "rm -rf other-project");
        live.poll();
        assert_eq!(live.pending.front().map(|r| r["request_id"].clone()), Some(json!("r-5")), "{:?}", live.pending);
        live.set_mode("bypassPermissions");
        assert!(got(r#""mode":"bypassPermissions""#), "full: claude's own");
        live.set_mode("auto");
        assert!(got(r#""mode":"acceptEdits""#), "auto reaches claude as acceptEdits");
        drop(live);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A switch to full answers the questions waiting; to auto, all but a destructive command, which stays on screen.
    #[cfg(unix)]
    #[test]
    fn a_mode_switch_answers_the_questions_it_would_not_have_asked() {
        let (mut live, dir) = stub::live("mode-answers");
        live.permission_mode = "default".into();
        live.send_typed("check the disk").unwrap();
        stub::started(&dir);
        let ask = |live: &Live, id: &str, cmd: &str| stub::feed(live, json!({"type": "control_request", "request_id": id,
            "request": {"subtype": "can_use_tool", "tool_name": "Bash", "input": {"command": cmd}}}));
        ask(&live, "q-1", "df -h");
        ask(&live, "q-2", "git push --force");
        live.poll();
        assert_eq!(live.pending.len(), 2, "ask mode: both wait");
        live.set_mode("auto");
        let done = live.answer_for_mode();
        assert_eq!((done.len(), live.pending.len()), (1, 1), "auto: df answered, the force push still asks");
        assert_eq!(live.pending.front().unwrap()["request_id"], "q-2");
        live.set_mode("bypassPermissions");
        assert_eq!((live.answer_for_mode().len(), live.pending.len()), (1, 0), "full: it goes too");
        let sent = (0..80).find_map(|_| std::fs::read_to_string(dir.join("stdin")).ok().filter(|t| t.contains("q-2")).or_else(|| { std::thread::sleep(Duration::from_millis(25)); None })).unwrap();
        assert!(sent.contains(r#""request_id":"q-1""#) && sent.matches(r#""behavior":"allow""#).count() == 2, "{sent}");
        drop(live);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A chat outside git keeps auto; leaving it for an agent profile there drops to ask, as starting in one would have.
    #[cfg(unix)]
    #[test]
    fn leaving_a_chat_outside_git_for_an_agent_drops_to_ask() {
        let dir = std::env::temp_dir().join(format!("tazuna-nogit-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut live = stub::in_dir(&dir, "rec.jsonl");
        (live.profile, live.permission_mode) = (Profile::Chat, "auto".into());
        live.set_profile(Profile::Lean).unwrap();
        assert_eq!(live.permission_mode, "default");
        assert!(live.poll().iter().any(|(_, e)| e["type"] == "harness_mode" && e["mode"] == "default"), "the screen hears of it");
        live.set_profile(Profile::Chat).unwrap();
        live.permission_mode = "auto".into();
        let git = std::env::temp_dir().join(format!("tazuna-git-{}", std::process::id()));
        std::fs::create_dir_all(&git).unwrap();
        assert!(Command::new("git").args(["init", "-q"]).current_dir(&git).status().unwrap().success());
        let mut repo = stub::in_dir(&git, "rec.jsonl");
        (repo.profile, repo.permission_mode) = (Profile::Chat, "auto".into());
        repo.set_profile(Profile::Lean).unwrap();
        assert_eq!(repo.permission_mode, "auto", "in a repo: as it was");
        std::fs::remove_dir_all(&dir).ok();
        std::fs::remove_dir_all(&git).ok();
    }

    /// A turn a chat handed over: full never asks, auto only before a destructive command, the other modes ask.
    #[cfg(unix)]
    #[test]
    fn a_hand_off_asks_as_its_mode_does() {
        for (mode, cmd, asked) in [("bypassPermissions", "git push --force", false), ("auto", "df -h", false), ("auto", "git push --force", true),
            ("default", "df -h", true)] {
            let (mut live, dir) = stub::live(&format!("handoff-mode-{mode}-{}", cmd.len()));
            live.permission_mode = mode.into();
            live.hand_off("how full is my disk?", "how full is my disk?", HANDOFF_SENT).unwrap();
            stub::started(&dir);
            stub::feed(&live, json!({"type": "control_request", "request_id": "perm-1",
                "request": {"subtype": "can_use_tool", "tool_name": "mcp__lean__bash", "input": {"command": cmd}}}));
            live.poll();
            assert_eq!(!live.pending.is_empty(), asked, "{mode} {cmd}");
            drop(live);
            std::fs::remove_dir_all(&dir).ok();
        }
    }

    /// No "always" in a handed-over turn: not offered (the question says once only), and an "always" answer allows it once.
    #[cfg(unix)]
    #[test]
    fn no_always_during_a_hand_off() {
        let (mut live, dir) = stub::live("handoff-once");
        live.permission_mode = "default".into();
        let ask = |live: &Live, id: &str| stub::feed(live, json!({"type": "control_request", "request_id": id,
            "request": {"subtype": "can_use_tool", "tool_name": "mcp__lean__bash", "input": {"command": "df -h"}}}));
        live.hand_off("how full is my disk?", "how full is my disk?", HANDOFF_SENT).unwrap();
        stub::started(&dir);
        ask(&live, "perm-1");
        assert!(live.poll().iter().any(|(_, e)| e["type"] == "harness_permission" && e["scope"] == HANDOFF_ONCE));
        assert_eq!(live.pending.front().map(|r| r["request"]["handoff"].clone()), Some(json!(true)));
        live.answer(true, true);
        assert!(live.allowed.is_empty(), "nothing kept for later turns");
        result(&live, "40% used");
        live.poll();
        live.send_typed("and now?").unwrap();
        stub::started(&dir);
        ask(&live, "perm-2");
        assert!(live.poll().iter().any(|(_, e)| e["type"] == "harness_permission" && e["scope"] == "this command"), "asked again");
        drop(live);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A handed-over turn's prompt holds no project instructions and no skills list: its fetch goes out without a question.
    #[cfg(unix)]
    #[test]
    fn a_handed_over_turn_gets_no_project_instructions_or_skills() {
        let (mut live, dir) = stub::live("handoff-prompt");
        std::fs::write(dir.join("CLAUDE.md"), "PROJECT-ONLY rules").unwrap();
        (live.profile, live.user_settings) = (Profile::Lean, true);
        let prompt = |a: &[String]| std::fs::read_to_string(stub::flag(a, "--system-prompt-file").unwrap()).unwrap();
        live.send_typed("hi").unwrap();
        assert!(prompt(&stub::started(&dir)).contains("PROJECT-ONLY"), "the session's own turns have them");
        result(&live, "hello");
        live.poll();
        live.hand_off("weather in Berlin now?", "weather in Berlin now?", HANDOFF_NEEDS).unwrap();
        let p = prompt(&stub::started(&dir));
        assert!(!p.contains("PROJECT-ONLY") && !p.contains("Skills: instructions"), "{p}");
        drop(live);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// The run log (what `tazuna -c` and the conversation lists read) and the window know the session's own conversation
    /// while a handed-over turn runs, never that turn's thrown-away one.
    #[cfg(unix)]
    #[test]
    fn a_handed_over_turn_is_logged_under_the_sessions_own_conversation() {
        let (mut live, dir) = stub::live("handoff-log");
        live.profile = Profile::Repo;
        live.resume("proj-1");
        live.hand_off("weather in Berlin now?", "weather in Berlin now?", HANDOFF_NEEDS).unwrap();
        stub::started(&dir);
        stub::feed(&live, json!({"type": "system", "subtype": "init", "session_id": "handoff-9"}));
        live.poll();
        assert_eq!(live.session_id(), Some("proj-1"));
        stub::feed(&live, json!({"type": "result", "subtype": "success", "is_error": false, "result": "12 °C", "total_cost_usd": 0.0}));
        live.poll();
        let log = std::fs::read_to_string(dir.join("runs.jsonl")).unwrap();
        let last: Value = serde_json::from_str(log.lines().last().unwrap()).unwrap();
        assert_eq!((last["prompt"].as_str(), last["meta"]["session_id"].as_str()), (Some("weather in Berlin now?"), Some("proj-1")));
        assert_eq!(live.session_id(), Some("proj-1"));
        drop(live);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A message sent while a handed-over turn runs waits, and goes to the session's own conversation after it.
    #[cfg(unix)]
    #[test]
    fn a_message_sent_during_a_hand_off_waits_for_the_sessions_conversation() {
        let (mut live, dir) = stub::live("handoff-held");
        (live.profile, live.permission_mode) = (Profile::Chat, "bypassPermissions".into());
        live.resume("chat-1");
        live.send_agent("weather in Berlin now?").unwrap();
        stub::started(&dir);
        assert!(stub::sent(&dir, "weather in Berlin now?"));
        let (_, e) = live.send_typed("and a joke about rain?").unwrap();
        assert!(e["type"] == "harness_user" && !live.idle());
        std::thread::sleep(Duration::from_millis(200));
        assert!(!stub::sent(&dir, "joke"), "not into the turn's conversation");
        std::fs::remove_file(dir.join("stdin")).ok();
        stub::feed(&live, json!({"type": "result", "subtype": "success", "is_error": false, "result": "12 °C and rain", "total_cost_usd": 0.0}));
        let kinds: Vec<_> = live.poll().iter().map(|(_, e)| e["type"].as_str().unwrap_or_default().to_string()).collect();
        assert_eq!(kinds, ["harness_handoff", "result", "harness_process_end", "harness_held_start"]);
        let a = stub::started(&dir);
        assert_eq!((stub::flag(&a, "--resume").as_deref(), stub::flag(&a, "--tools").as_deref()), (Some("chat-1"), Some("")), "the chat's own: {a:?}");
        assert!(stub::sent(&dir, "You answered: 12 °C and rain\n\nThe new message: and a joke about rain?"));
        drop(live);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A paid model's handed-over turn stops at the spend cap like any message (the router's skipped it).
    #[test]
    fn a_hand_off_stops_at_the_spend_cap() {
        if spend_cap().is_none() {
            return; // the user's cap is off here: nothing to stop at
        }
        let mut live = Live::new("m", "high").unwrap();
        live.runs_log = std::env::temp_dir().join(format!("tazuna-handoff-cap-{}.jsonl", std::process::id()));
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        std::fs::write(&live.runs_log, format!("{}\n", json!({"ts": now, "cost_usd": 1e9, "meta": {"claude_version": null}}))).unwrap();
        (live.provider, live.price) = (Some(("http://127.0.0.1:9/v1".into(), "KEY".into())), Some("1,1,1".into()));
        let err = live.hand_off("weather in Berlin now?", "weather in Berlin now?", HANDOFF_NEEDS).unwrap_err();
        assert!(format!("{err:#}").contains("at your cap") && !live.handing_off() && live.idle(), "{err:#}");
        std::fs::remove_file(&live.runs_log).ok();
        std::fs::remove_file(&live.rec_path).ok();
    }

    /// A handed-over turn is a lookup: no tests, review or stronger model after it, whatever the session runs.
    #[cfg(unix)]
    #[test]
    fn a_handed_over_turn_runs_no_checks_after_it() {
        let (mut live, dir) = stub::live("handoff-checks");
        live.verify = verify::Mode::Command("false".into());
        (live.self_review, live.review_model) = (true, Some(("claude-opus-5-5".into(), "high".into())));
        live.set_escalate("claude-opus-5-5".into(), "high".into());
        live.hand_off("weather in Berlin now?", "weather in Berlin now?", HANDOFF_NEEDS).unwrap();
        stub::started(&dir);
        stub::feed(&live, json!({"type": "result", "subtype": "success", "is_error": false, "result": "12 °C", "total_cost_usd": 0.0}));
        let kinds: Vec<_> = live.poll().iter().map(|(_, e)| e["type"].as_str().unwrap_or_default().to_string()).collect();
        assert_eq!(kinds, ["harness_handoff", "result", "harness_process_end"]);
        assert!(live.idle() && !live.escalated);
        drop(live);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Through a session on scripted events: the chat replies [[agent]], the lean agent takes the message in a fresh
    /// conversation, and the chat's own comes back after it.
    /// A switch between claude and the own loop: the new model gets the conversation so far as text in front of its first
    /// message (each could not read the other's history, and started with none until review 12), and its session says it
    /// continues the earlier one, for the lists.
    #[cfg(unix)]
    #[test]
    fn a_switch_carries_the_conversation_and_its_session_continues_the_earlier_one() {
        let (mut live, dir) = stub::live("carry");
        // an own-loop conversation, as recorded (its engine is not started here)
        live.provider = Some(("http://127.0.0.1:1/v1".into(), "LM_STUDIO_API_KEY".into()));
        live.resume("own-1");
        for e in [json!({"type": "harness_user", "text": "what is a haiku?"}), json!({"type": "result", "subtype": "success", "result": "A short poem.", "tazuna_prompt": "what is a haiku?"}),
            json!({"type": "harness_user", "text": "this one will fail"})] {
            live.heard_before(&e);
        }
        let (_, e) = live.switch("claude-opus-5-5".into(), "high".into(), None).unwrap();
        assert_eq!(e["text"], "model: Opus 5.5 · high: the conversation continues on Opus 5.5");
        live.send("and a limerick?").unwrap();
        stub::started(&dir);
        assert!(stub::sent(&dir, "Earlier in this chat you were asked: what is a haiku?\nYou answered: A short poem."));
        assert!(stub::sent(&dir, "asked: this one will fail\nYou answered: (no answer)\n\nThe new message: and a limerick?"));
        stub::feed(&live, json!({"type": "system", "subtype": "init", "session_id": "claude-2", "model": "claude-opus-5-5"}));
        stub::feed(&live, json!({"type": "result", "subtype": "success", "result": "There once was…", "total_cost_usd": 0.01}));
        live.poll();
        let log = std::fs::read_to_string(dir.join("runs.jsonl")).unwrap();
        let line: Value = serde_json::from_str(log.lines().last().unwrap()).unwrap();
        assert_eq!((line["meta"]["session_id"].as_str(), line["meta"]["continues"].as_str(), line["prompt"].as_str()), (Some("claude-2"), Some("own-1"), Some("and a limerick?")));
        // a long conversation: the latest of it, about 40k characters
        let long: Vec<(String, String)> = (0..30).map(|i| (format!("q{i}"), "a".repeat(2000))).collect();
        let c = carried(&long);
        assert!(c.len() == 19 && c.last().unwrap().0 == "q29", "{}", c.len());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Retry: the failed message again, the conversation back to before it, never a second copy in the history.
    #[cfg(unix)]
    #[test]
    fn a_retry_sends_the_last_message_again_from_before_it() {
        let (mut live, dir) = stub::live("retry");
        live.send_typed("what is a haiku?").unwrap();
        stub::started(&dir);
        stub::feed(&live, json!({"type": "system", "subtype": "init", "session_id": "c-1", "model": "claude-opus-5-5"}));
        stub::feed(&live, json!({"type": "assistant", "parent_tool_use_id": null, "uuid": "u-1", "message": {"content": [{"type": "text", "text": "A poem."}]}}));
        stub::feed(&live, json!({"type": "result", "subtype": "success", "result": "A poem.", "total_cost_usd": 0.01}));
        live.poll();
        live.send_typed("and a limerick?").unwrap();
        stub::feed(&live, json!({"type": "result", "subtype": "success", "is_error": true, "result": "API Error: 500", "total_cost_usd": 0.01}));
        live.poll();
        assert!(live.retry("something else").is_err(), "only the last message");
        let (_, e) = live.retry("and a limerick?").unwrap();
        assert_eq!(e["type"], "harness_retry");
        let a = stub::started(&dir);
        // claude at the reply before it, in a fork: the failed message is not in the history it resumes
        assert_eq!((stub::flag(&a, "--resume").as_deref(), stub::flag(&a, "--resume-session-at").as_deref()), (Some("c-1"), Some("u-1")));
        assert!(a.contains(&"--fork-session".to_string()) && live.checkpoints.len() == 2 && live.continues.as_deref() == Some("c-1"));
        // the retried message goes back to the same reply again: a second retry (or an edit of it) resumes there, not fresh
        assert_eq!(live.checkpoints[1].before.as_deref(), Some("u-1"));
        stub::feed(&live, json!({"type": "system", "subtype": "init", "session_id": "c-2", "model": "claude-opus-5-5"}));
        stub::feed(&live, json!({"type": "result", "subtype": "success", "is_error": true, "result": "API Error: 500", "total_cost_usd": 0.01}));
        live.poll();
        live.retry("and a limerick?").unwrap();
        let a = stub::started(&dir);
        assert_eq!((stub::flag(&a, "--resume").as_deref(), stub::flag(&a, "--resume-session-at").as_deref()), (Some("c-2"), Some("u-1")));
        assert_eq!(live.earlier_sessions(), ["c-1", "c-2"]);
        // tazuna's own text (create's check, `send`) is sent again as text, its paths never read as images
        live.checkpoints.last_mut().unwrap().attach = false;
        stub::feed(&live, json!({"type": "result", "subtype": "success", "is_error": true, "result": "API Error: 500", "total_cost_usd": 0.01}));
        live.poll();
        live.retry("and a limerick?").unwrap();
        assert!(!live.checkpoints.last().unwrap().attach);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A retry of a message from before a switch to a model that could not take the history: a fresh session with the
    /// conversation before it carried (the switch's carry was dropped, and a Claude reply's id went to the own loop, review
    /// 12); the message is not carried twice.
    #[cfg(unix)]
    #[test]
    fn a_retry_across_a_switch_starts_afresh_with_the_conversation_before_it() {
        let (mut live, dir) = stub::live("retry-switch");
        live.provider = Some(("http://127.0.0.1:1/v1".into(), "LM_STUDIO_API_KEY".into()));
        live.resume("own-1");
        for e in [json!({"type": "harness_user", "text": "what is a haiku?"}), json!({"type": "result", "subtype": "success", "result": "A short poem.", "tazuna_prompt": "what is a haiku?"}),
            json!({"type": "harness_user", "text": "and a limerick?"})] {
            live.heard_before(&e);
        }
        live.checkpoints.push(Checkpoint { uuid: "m-2".into(), text: "and a limerick?".into(), before: None, carry: vec![], attach: true, epoch: 0 });
        live.switch("claude-opus-5-5".into(), "high".into(), None).unwrap();
        live.retry("and a limerick?").unwrap();
        let a = stub::started(&dir);
        assert!(stub::flag(&a, "--resume").is_none() && !a.contains(&"--resume-session-at".to_string()), "{a:?}");
        assert!(stub::sent(&dir, "Earlier in this chat you were asked: what is a haiku?\nYou answered: A short poem.\n\nThe new message: and a limerick?"));
        assert!(!stub::sent(&dir, "asked: and a limerick?"));
        assert_eq!((live.continues.as_deref(), live.earlier_sessions()), (Some("own-1"), &["own-1".to_string()][..]));
        // the other way: a Claude message's reply id never reaches the own loop
        live.checkpoints.push(Checkpoint { uuid: "m-3".into(), text: "x".into(), before: Some("claude-reply".into()), carry: vec![], attach: true, epoch: 1 });
        live.switches = 2;
        live.provider = Some(("http://127.0.0.1:1/v1".into(), "LM_STUDIO_API_KEY".into()));
        let cp = live.checkpoints.last().cloned().unwrap();
        live.back_before(&cp);
        assert_eq!(live.pending_resume, Some(None));
        live.resume_before();
        assert!(live.resume_at.is_none() && live.session_id.is_none() && !live.mapped);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn a_chat_hands_its_message_to_the_lean_agent_and_takes_the_session_back() {
        let (mut live, dir) = stub::live("handoff");
        (live.profile, live.permission_mode) = (Profile::Chat, "bypassPermissions".into());
        live.resume("chat-1");
        let kinds = |out: &[(f64, Value)]| out.iter().map(|(_, e)| e["type"].as_str().unwrap_or_default().to_string()).collect::<Vec<_>>();
        // the chat's run on the question ends with the hand-off
        live.runs.push_back(Run::new("weather in Berlin now?", 0.0, &live.cwd));
        live.task = "weather in Berlin now?".into();
        stub::feed(&live, json!({"type": "result", "subtype": "success", "is_error": false, "result": " [[agent]]\n", "total_cost_usd": 0.001}));
        let out = live.poll();
        assert_eq!(kinds(&out), ["result", "harness_handoff"]);
        assert_eq!(out[1].1["text"], HANDOFF_NEEDS);
        assert!(live.profile == Profile::Lean && live.handing_off() && live.session_id.is_none() && live.runs.len() == 1 && live.checkpoints.is_empty());
        let a = stub::started(&dir);
        assert_eq!(stub::flag(&a, "--permission-mode").as_deref(), Some("default"), "asks, though the session runs in auto");
        // the agent's answer ends the turn: the chat again, its conversation resumed by the next message
        stub::feed(&live, json!({"type": "result", "subtype": "success", "is_error": false, "result": "12 °C and rain", "total_cost_usd": 0.02}));
        let out = live.poll();
        assert_eq!(kinds(&out), ["result", "harness_process_end"]);
        assert!(live.profile == Profile::Chat && !live.handing_off() && live.session_id.as_deref() == Some("chat-1") && live.idle());
        // an answer that mentions the marker later stays an answer
        live.runs.push_back(Run::new("what is [[agent]]?", 0.0, &live.cwd));
        stub::feed(&live, json!({"type": "result", "subtype": "success", "is_error": false, "result": "The text [[agent]] is tazuna's hand-off.", "total_cost_usd": 0.001}));
        assert_eq!(kinds(&live.poll()), ["result"]);
        assert!(live.profile == Profile::Chat && !live.handing_off());
        // /agent and "send to agent" there: the same agent, under its own line
        let (_, e) = live.send_agent("how full is my disk?").unwrap();
        assert!(e["type"] == "harness_user" && live.handing_off() && live.profile == Profile::Lean);
        assert_eq!(live.poll()[0].1["text"], HANDOFF_SENT);
        stub::started(&dir);
        // stopped: the chat again too
        let _ = live.kill("stopped");
        assert!(live.profile == Profile::Chat && !live.handing_off() && live.session_id.as_deref() == Some("chat-1"));
        assert_eq!(kinds(&live.poll()), ["harness_process_end"]);
        drop(live);
        std::fs::remove_dir_all(&dir).ok();
    }
}

/// A claude.ai artifact's link ("https://claude.ai/artifact/…", "…/code/artifact/…").
fn artifact_link(text: &str) -> bool {
    text.contains("claude.ai/artifact/") || text.contains("claude.ai/code/artifact/")
}

#[cfg(all(test, unix))]
#[test]
fn an_artifact_link_brings_claudes_artifact_tool() {
    for (name, text, tools, env) in [("artifact-none", "hello", "Grep,Glob", ""), ("artifact-link", "read https://claude.ai/artifact/TBgFC9m3 please", "Grep,Glob,Artifact", "1")] {
        let (mut live, dir) = stub::live(name);
        live.profile = Profile::Lean;
        live.artifacts = false; // whatever the test's own environment says
        live.send(text).unwrap();
        let a = stub::started(&dir);
        assert_eq!(stub::flag(&a, "--tools").as_deref(), Some(tools), "{name}");
        assert_eq!(std::fs::read_to_string(dir.join("artifact_env")).unwrap_or_default(), env, "{name}");
        drop(live);
        std::fs::remove_dir_all(&dir).ok();
    }
    let (mut live, dir) = stub::live("artifact-chat");
    (live.profile, live.artifacts) = (Profile::Chat, true);
    assert_eq!(live.with_artifact(""), "Artifact", "a chat's empty tool list gets it alone");
    drop(live);
    std::fs::remove_dir_all(&dir).ok();
}

/// Test support: a stand-in for claude (the real one is a paid call) and a session that runs it in a folder of its own.
#[cfg(all(test, unix))]
pub(crate) mod stub {
    use super::*;

    /// The stand-in, first on PATH for the whole test run and never removed (no test may reach the real claude): in a folder
    /// with a `stub` file it writes its arguments to `args` and what it is sent to `stdin` there. It says nothing itself:
    /// a test gives its events (`feed`).
    fn claude() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            use std::os::unix::fs::PermissionsExt;
            let bin = std::env::temp_dir().join(format!("tazuna-stub-claude-{}", std::process::id()));
            std::fs::create_dir_all(&bin).unwrap();
            std::fs::write(bin.join("claude"), "#!/bin/sh\n[ -e stub ] || exec cat > /dev/null\nprintf '%s' \"$CLAUDE_CODE_ARTIFACT\" > artifact_env\nprintf '%s\\n' \"$@\" > args.tmp && mv args.tmp args\nexec cat >> stdin\n").unwrap();
            std::fs::set_permissions(bin.join("claude"), std::fs::Permissions::from_mode(0o755)).unwrap();
            // SAFETY: set once, and only prepended: everything else is found as before
            unsafe { std::env::set_var("PATH", format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default())) };
        });
    }

    /// A session on Opus in a fresh folder of its own (`name`), where the stand-in writes, with its run log there.
    pub fn live(name: &str) -> (Live, PathBuf) {
        claude();
        let dir = std::env::temp_dir().join(format!("tazuna-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("stub"), "").unwrap();
        (in_dir(&dir, "rec.jsonl"), dir)
    }

    /// Another session in `dir` (a later tazuna there), recording to `rec` in it.
    pub fn in_dir(dir: &Path, rec: &str) -> Live {
        let mut live = Live::new("claude-opus-5-5", "high").unwrap();
        (live.cwd, live.runs_log, live.verify) = (dir.to_path_buf(), dir.join("runs.jsonl"), verify::Mode::Off);
        // the recording: in the folder too, not in the user's sessions
        let _ = std::fs::remove_file(&live.rec_path);
        live.rec_path = dir.join(rec);
        live.rec = File::create(&live.rec_path).unwrap();
        live
    }

    /// The arguments the stand-in last started with in `dir` (waited for), taken.
    pub fn started(dir: &Path) -> Vec<String> {
        for _ in 0..200 {
            if let Ok(a) = std::fs::read_to_string(dir.join("args")) && !a.is_empty() {
                let _ = std::fs::remove_file(dir.join("args"));
                return a.lines().map(String::from).collect();
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        panic!("claude did not start in {}", dir.display());
    }

    /// Whether the stand-ins in `dir` were sent `text` (waited for up to 2 s).
    pub fn sent(dir: &Path, text: &str) -> bool {
        let want = serde_json::to_string(text).unwrap();
        let want = &want[1..want.len() - 1];
        for _ in 0..80 {
            if std::fs::read_to_string(dir.join("stdin")).is_ok_and(|s| s.contains(want)) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        false
    }

    /// A line of claude's output, as the running process would write it.
    pub fn feed(live: &Live, e: Value) {
        live.tx.send((live.generation, Msg::Line(e.to_string()))).unwrap();
    }

    /// The value after `flag` in `args`.
    pub fn flag(args: &[String], flag: &str) -> Option<String> {
        args.iter().position(|x| x == flag).and_then(|i| args.get(i + 1).cloned())
    }
}
