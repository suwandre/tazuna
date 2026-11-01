# tazuna

手綱. A lean terminal harness around coding models, built for token efficiency and output quality.

tazuna drives the official `claude` CLI headlessly (`stream-json` in and out) with a lean profile, so Claude runs on your own
subscription through first-party Claude Code: no token reuse, no impersonation. Other providers will get their own agent loop.

Goals, order of work and the ideas already tried or parked: [ROADMAP.md](ROADMAP.md).

## Status

Early. What works today:

- **Live mode**: run `tazuna` in a git repo, type a task. One `claude` process per conversation (warm prompt cache),
  clean interrupts (`esc` or `ctrl-c`), follow-ups, readline-style editing (`←→`, `ctrl-a/e/u/k/w`) and history (`↑↓`,
  kept across sessions), `ctrl-o` to expand tool output, multi-line input (`\` then enter, or `ctrl-j`; shift/alt-enter where the terminal
  reports it), mouse scrolling, drag over the transcript to select and copy its text (never the side panel), copy the last
  reply (`ctrl-y`), `ctrl-s` hands the mouse to the terminal.
  `tazuna -c` continues this repo's latest session (`--resume <id>` a given one), with the earlier transcript on screen.
  Slash commands go to claude (built-ins like `/compact`, `/cost`, `/clear`, `/model`, and the repo's own commands and
  skills); `tab` completes them and `@` file paths. Claude's to-do list shows as a checklist. Images named in a message
  (`@shot.png`, a dragged-in path, or `ctrl-v` for the clipboard) go along as images; `!cmd` runs a command and hands
  its output to claude.
  Permission modes (`--mode auto|ask|edits|plan`, `shift-tab` cycles): never ask (default), ask before edits and commands,
  ask before commands only, or plan read-only first and approve the plan.
- **Profiles**: repos with agent config (`CLAUDE.md`, `AGENTS.md`, `.claude/`) run the **repo profile**: Claude Code's own
  prompt and tools with the repo's settings, hooks and skills (your personal `~/.claude` setup stays out unless
  `--user-settings`; that includes claude.ai connectors). Repos whose config runs code ask once "Trust this repo?";
  headless runs need `--trust`. `--profile repo-lean` keeps the repo's config with the lean prompt (~10k instead of ~22k
  tokens per turn). Other repos, tiny repos and `--profile lean` use the lean profile below.
- **Lean profile "cc-mid"**: ~10-line system prompt, `Grep`/`Glob` plus four lean tools (bash/read/write/edit) served by tazuna
  itself (`tazuna mcp`), and none of the user's settings, skills, hooks or MCP servers. Benchmarked as the best cost/quality profile.
- **Four UI designs** (Ratatui + tachyonfx): aurora, cockpit (default: live context/cache gauges, tokens per turn, cost,
  tools, files), zen, timeline. `ctrl-t` switches.
- **Recordings**: every session is saved to `~/.local/share/tazuna/sessions/` and replays like a video.
- **Headless mode** for benchmarks: `tazuna --run prompt.txt` prints claude's result JSON.
- **Verified, not claimed**: when claude says it is done, tazuna runs the tests for what changed (and the repo's
  `typecheck` script, failing only on type errors the run added) and sends failures back
  (up to 2 rounds). `--verify "<cmd>"` overrides the detected command, `--no-verify` turns it off.
- **Diagnostics after each edit**: type/compile errors the agent just added come back to it at once (the repo's
  `typecheck` script, or `go vet`), not only at the end. Same quality, −11% cost on the company tickets.
  `--no-diagnostics` turns it off.
- **Router**: without `--model`/`--effort`, tiny repos (≤ 50 files) start on Sonnet 5.5 medium and escalate to Opus 5.5 high
  if their tests fail; everything else runs Opus 5.5 medium. On tiny-repo tasks: −67% cost, same hidden-test results.
- **Project instructions**: the repo's `CLAUDE.md` / `AGENTS.md` go into the prompt (cached); nested ones are listed.
  `--no-project-instructions` turns it off.
- **Bash output cap**: long command output is shrunk to its start and end (passing-test chatter removed first); the full
  output is saved where claude can read it. `--bash-cap N` sets the limit (default 8000 chars, `0` = off).
- **Labels**: rate runs with `/good` or `/bad` (UI) or `tazuna --verdict`; `tazuna --label-git` marks runs whose output you
  committed (`kept`) or threw away (`discarded`). This is the data for learning which model fits which task.
- **Run log**: one line per run in `~/.local/share/tazuna/runs.jsonl` (stop reason, cost, per-turn cache use, tools,
  files changed). `tazuna --log` shows the latest runs.

Parity check (6 real Go tickets × 2 runs, Opus 5.5 medium): same hidden-test pass rate and judge score as `claude -p` with
the same profile, identical per-turn overhead.

## Benchmarks (v0.0.11)

Changes per version are tracked in [CHANGELOG.md](CHANGELOG.md).

**Real work: 6 recent merged PRs** in hugo, k6, terraform, prometheus and caddy (Go). Each harness gets the issue text and
the repo at the PR's parent commit, twice. Hidden tests are the PR's own tests. A blind Opus 5.5 judge scores all 16
candidates for a task together (1-10), twice in shuffled order. All harnesses at their current versions, run on
2026-10-07 on an M5 Pro, Claude Sonnet 5.5 at high effort; cost is tokens × Sonnet list price for all of them.

| Harness | Version | Judge | All hidden tests pass | Hidden tests passed | Median cost | Median time | Claude billing |
|---|---|---|---|---|---|---|---|
| **tazuna (default)** | **0.0.11** | **6.67** | **8/12** | **84%** | **$0.41** | **411s** | **first-party (`claude` CLI)** |
| OMP | 18.8.3 | 6.54 | 8/12 | 86% | $0.51 | 401s | direct claude.ai OAuth |
| Claude Code (stock) | 2.1.292 | 6.38 | 8/12 | 83% | $0.43 | 420s | first-party |
| Hermes | 0.21.5 (865ba90) | 6.21 | 7/12 | 81% | $0.55 | 453s | routes as Claude Code (extra usage) |
| pi | 1.0.4 | 6.08 | 8/12 | 85% | $0.28 | 458s | extra usage (pay per token) |
| tazuna (`--profile lean`) | 0.0.11 | 6.04 | 8/12 | 84% | $0.31 | 381s | first-party (`claude` CLI) |
| OMO | 5.1.23 | 5.38 | 7/12 | 78% | $0.49 | 307s | official Agent SDK |
| Empryo | 3.8.7-beta | 5.12 | 8/12 | 84% | $0.50 | 464s | subscription via its own client |

How to read it: the top three are within 0.3 judge points, inside the noise. tazuna's default (the repo profile in the five
repos that carry a `CLAUDE.md`/`AGENTS.md`, lean in terraform) leads on quality at 5% below stock Claude Code's cost and
20-25% below OMP and Hermes; lean tazuna is 28% cheaper than stock for 0.34 lower. (Stock Claude Code's cost includes every
result event of a run: when it waits on a background command, `claude -p` reports each wake-up separately. Counting only
the last one had made three of its runs look nearly free; fixed 2026-10-07.)

On real company work (five tickets in a TypeScript monorepo, Opus 5.5 medium) tazuna lean matched stock Claude Code's
quality at 19% lower cost; see CHANGELOG 0.0.10-0.0.11.

**Model and effort matter more than the harness** (same tasks, tazuna's profile, cost as billed):

| Model · effort | Judge | Median cost |
|---|---|---|
| Sonnet 5.5 · low / medium / high | 4.04 / 5.00 / 6.21 | $0.15 / $0.17 / $0.36 |
| **Opus 5.5 · medium (tazuna default)** | **7.00** | **$0.44** |
| Opus 5.5 · high / xhigh | 7.04 / 7.00 | $0.74 / $3.10 |

On the default (Opus 5.5 medium), tazuna scored 7.25 vs 6.67-6.71 for the same profile through `claude -p`, at equal cost
(within noise; 12 runs each, judged together).

**One-shot tasks** (9 easy-to-hard Python tasks, 2 runs each, Sonnet 5.5 high): every harness passes nearly all hidden tests;
cost is what differs. tazuna's profile (cc-mid) scored 8.11 at $0.061 per task, vs 8.19 at $0.096 for stock Claude Code,
7.83 at $0.064 for pi, and 7.67-7.92 at $0.096-0.173 for OMO/OMP. (Measured with the profile through `claude -p`; tazuna
itself matched that profile on the real-work tasks.)

**Footprint**

| | tazuna | stock Claude Code |
|---|---|---|
| Input tokens per turn before any work (system prompt + tools) | 3.0k (lean) / ~22k (repo profile) | 21k |
| App binary | 2.6 MB | — |
| UI memory / CPU (120x40 terminal) | 7 MB / 1.6% while animating | 190 MB / 2.2% idle (first 12 s) |
| UI start to first frame | 6 ms | 0.45 s |
| Tool server memory (lean profile) | 2.1 MB (built in) | built in |

tazuna's UI runs on top of a headless `claude` process, which needs about as much memory as Claude Code's own UI: the win is
the UI layer, not the total. Measured 2026-10-08 on an M5 Pro with `bench/footprint.py` (every version, in Tier 0).

**How to read these numbers**: one run varies by about ±1.2 judge points and ±$0.40. Re-running the same setup a day later
moved median cost by 24% (mean 9%), and two arms that behaved identically scored 0.67 apart. We treat judge differences
under 0.75 and cost differences under 15% as noise (12 runs per arm), and compare against a same-day baseline.

## Usage

```sh
cargo build --release
cd <some git repo>
/path/to/tazuna/target/release/tazuna                         # Opus 5.5, medium effort
tazuna --model claude-sonnet-5-5 --effort high
tazuna ~/.local/share/tazuna/sessions/<file>.jsonl           # replay a session (space pause, ←/→ speed, 1-4 design)
```

Requires the `claude` CLI (logged in). Nothing else: the lean tools are built into the tazuna binary.

On Windows (cmd or PowerShell) tazuna needs Git for Windows: like Claude Code there, it runs commands in Git Bash
(`TAZUNA_BASH` or `CLAUDE_CODE_GIT_BASH_PATH` point elsewhere). Install Claude Code with its native installer (`claude.exe`;
the npm package's `claude.cmd` is not found). Build with `cargo install --path .` (Rust's MSVC toolchain), or take
`tazuna.exe` from the windows workflow's last run (`gh run download -n tazuna-windows`). Not on Windows yet: desktop
notifications, and reaping background processes an agent left running.

tazuna only runs inside a git repo by default: the agent edits without asking, so `git diff` is the review step.

Repo instructions are trusted: `CLAUDE.md` / `AGENTS.md` go into the prompt of an agent that runs commands without asking
(Claude Code does the same after its "trust this folder?" prompt; tazuna has no such prompt). In repos you did not write, use
`--no-project-instructions`. Symlinked instruction files are never read.

## Layout

```
src/main.rs      modes (live, replay, snapshot, headless) and the event loop
src/live.rs      the claude process: spawn, stream-json, interrupt, recording, verification
src/mcp.rs       the lean tool server (`tazuna mcp`)
src/verify.rs    test detection and running for "verified, not claimed"
src/runlog.rs    the run log
bench/           tier 0 metrics script and history
src/session.rs   session state built from stream-json events
src/replay.rs    recorded sessions on a virtual clock
src/ui/          shared transcript renderer + the four designs
recordings/      sample recording and the recorder used to make it
```
