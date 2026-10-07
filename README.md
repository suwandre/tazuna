# tazuna

手綱. A lean terminal harness around coding models, built for token efficiency and output quality.

tazuna drives the official `claude` CLI headlessly (`stream-json` in and out) with a lean profile, so Claude runs on your own
subscription through first-party Claude Code: no token reuse, no impersonation. Other providers will get their own agent loop.

Goals, order of work and the ideas already tried or parked: [ROADMAP.md](ROADMAP.md).

## Status

Early. What works today:

- **Live mode**: run `tazuna` in a git repo, type a task. One `claude` process per conversation (warm prompt cache),
  clean interrupts (`ctrl-c`), follow-ups, copy last reply (`ctrl-y`), mouse scrolling.
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
the repo at the PR's parent commit, twice. Hidden tests are the PR's own tests. A blind Opus 5.5 judge scores all 26
candidates for a task together (1-10), twice in shuffled order; every row below was judged in the same pools on
2026-10-07. Every harness runs Claude Sonnet 5.5 at high effort; cost is tokens × Sonnet list price for all of them.

| Harness | Run on | Judge | All hidden tests pass | Hidden tests passed | Median cost | Median time | Claude billing |
|---|---|---|---|---|---|---|---|
| Hermes | Oct 3-6 | 6.88 | 8/12 | 87% | $0.45 | 410s | routes as Claude Code (extra usage) |
| OMP | Oct 3-6 | 6.67 | 8/12 | 85% | $0.47 | 339s | direct claude.ai OAuth |
| OMO | Oct 3-6 | 6.67 | 9/12 | 90% | $0.56 | 290s | official Agent SDK |
| **tazuna 0.0.11 (default)** | Oct 7 | **6.58** | **8/12** | **84%** | **$0.41** | **411s** | **first-party (`claude` CLI)** |
| Claude Code (stock) | Oct 7 | 6.21 | 8/12 | 83% | $0.30 | 420s | first-party |
| Empryo | Oct 3-6 | 6.21 | 8/12 | 85% | $0.48 | 419s | subscription via its own client |
| tazuna 0.0.11 (`--profile lean`) | Oct 7 | 6.08 | 8/12 | 84% | $0.31 | 381s | first-party (`claude` CLI) |
| Claude Code (stock) | Oct 3-6 | 6.08 | 7/12 | 84% | $0.42 | 322s | first-party |
| tazuna 0.0.1 (lean) | Oct 3-6 | 6.08 | 8/12 | 83% | $0.31 | 240s | first-party (`claude` CLI) |
| pi | Oct 3-6 | 6.00 | 7/12 | 80% | $0.25 | 201s | extra usage (pay per token) |

How to read it: the top four are within 0.3 judge points of each other, inside the noise. tazuna's default (the repo
profile in the five repos that carry a `CLAUDE.md`/`AGENTS.md`, lean in terraform) scores +0.37 over same-day stock Claude
Code but costs 37% more there: it took more turns (24 vs 16.5 median). Stock Claude Code itself cost 29% less on Oct 7 than
on Oct 3-6 for the same tasks, so cost comparisons across days are unreliable; judge scores are comparable because all rows
were judged together. The other harnesses were not re-run.

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
| Input tokens per turn before any work (system prompt + tools) | 2.9k | 21k |
| App binary / memory / CPU while animating | 2.3 MB / ~7 MB / ~1% | — |
| Tool server memory | 1.8 MB (built in) | built in |

tazuna runs on top of the `claude` CLI (~200 MB, ~1 s to start).

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
