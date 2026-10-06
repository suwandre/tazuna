# Changelog

Every change that can affect the agent is measured before it stays. Each entry lists what moved and by how much. Versions
that change no measurable metric (CPU, memory, binary size, tokens, cost, quality) get no metric table.

**Tiers**
- **Tier 0** (every change, free): tests, `bench/tier0.sh` (binary size, replay render time, input tokens per turn).
- **Tier 1** (changes to prompt, tools, routing or stop logic): 6 real-work Go tasks × 2 runs vs a same-day baseline. Judge
  score, hidden tests, cost, turns, time, cache hit rate.
- **Tier 2** (releases): Tier 1 plus the cross-harness table in the README.

**Noise thresholds**: judge ±0.75, cost ±15% (mean), turns ±20% at 12 runs per arm. Smaller differences are reported as
"within noise". (The judge threshold was ±0.5 until 0.0.3, whose two arms behaved identically and still scored 0.67 apart.)

Also recorded per measurement: the `claude` CLI version. Claude Code updates itself, and its built-in prompt changes token
counts (2.1.289 → 2.1.291 added ~104 input tokens per turn).

## 0.0.9 (2026-10-07)

**Repo config is respected by default.** In repos with agent config (`CLAUDE.md`, `AGENTS.md`, `.claude/`, `.mcp.json`),
tazuna runs the **repo profile**: Claude Code's own prompt and tools with the repo's settings, hooks and skills, and
Claude Code's own loading of `CLAUDE.md` (nested files and `@imports` included). The user's personal `~/.claude` setup stays
out unless `--user-settings`. Repos without config, tiny repos and `--profile lean` run the lean profile (cc-mid, ~30%
cheaper; the bash output cap only exists there).

**Trust.** When a repo's config can run code (any settings file, skills, commands, subagents, MCP servers), the first live
start asks "Trust this repo? [y/N]" and remembers the answer (`~/.local/share/tazuna/trusted.json`); headless runs need
`--trust`. Without trust the run is lean: nothing from the repo runs, and its `CLAUDE.md` still reaches the prompt. The check
fails closed: any settings file counts, not only ones with hooks (`statusLine`, `apiKeyHelper`, `env` and plugins can run
code too).

Real-work test on T-872 / PR #1068 (same task as 0.0.8), all 15 runs judged together, Opus 5.5 medium:

| Arm | Runs | Judge | Mean cost | New type errors | Verification |
|---|---|---|---|---|---|
| Stock Claude Code, repo hooks off | 3 | 5.67 | $1.91 | 0 | — |
| Stock Claude Code, repo hooks on | 2 | 5.50 | $2.02 | 0 | — |
| **tazuna 0.0.9 (repo profile)** | 3 | **5.17** (5.5, 5.0, 5.0) | **$1.79** | **0** | 3/3 verified |
| cc-mid via `claude -p` | 2 | 5.00 | $1.46 | 0 | — |
| tazuna 0.0.7 (lean) | 2 | 5.00 | $1.31 | 1 | 2/2 verified |
| tazuna 0.0.8 (lean + CLAUDE.md) | 3 | 4.83 | $1.37 | 1 | 3/3 verified |

The repo profile is within noise of stock Claude Code (−0.33 to −0.5) at 11% lower cost, with no new type errors and the
most consistent scores. Against lean it is +0.3 (within noise) at ~30% higher cost. Turning the repo's hooks off did not
lower stock Claude Code's score, so its earlier lead came from its prompt and tools, not the hooks. Across all 15 runs the
spread between arms (4.8-5.7) is small next to the spread within arms; only 2 runs found the backend root cause. One task:
an anecdote, not a benchmark.

Tier 0 (lean probe in a repo without config): 2,983 input tokens per turn (unchanged), binary
2.41 MB.

## 0.0.8 (2026-10-06)

**Project instructions.** The repo's own `CLAUDE.md`, `.claude/CLAUDE.md` and `AGENTS.md` are added to tazuna's system prompt
(duplicates skipped, cut at 80k chars); nested instruction files are listed so the agent reads them before working in that
directory. The text is the same every turn, so it is cached. Hooks and skills from the repo are still not loaded.
`--no-project-instructions` turns it off; the run log records which files were included.

First test on real company work: acme/app, T-872 / PR #1068 (TypeScript monorepo, 3k files; the agent gets
the Jira ticket plus the one log line, the PR's non-test diff is the judge's reference). All arms Opus 5.5 medium, judged
together:

| Arm | Runs | Judge | Mean cost | Mean time | New failing unit tests (of 17,812) | New type errors |
|---|---|---|---|---|---|---|
| Stock Claude Code (repo CLAUDE.md, hooks, skills) | 2 | 6.00 | $2.02 | 271 s | 0 | 0 |
| cc-mid via `claude -p` | 2 | 5.25 | $1.46 | 298 s | 0 | 0 |
| **tazuna 0.0.8 (with CLAUDE.md)** | 3 | 4.83 | $1.37 | 261 s | 0 | 0, 1, 0 |
| tazuna 0.0.7 | 2 | 4.50 | $1.31 | 376 s | 0 | 0, 1 |

Project instructions: +0.33 judge (within noise), +5% cost (CLAUDE.md is ~10k tokens, re-read from cache at 95-97% hit).
No arm found the backend root cause the PR fixed; all built a frontend workaround (correctness 3/5 for every run). Stock
Claude Code led in both judge passes; the repo's hooks (a comment linter after every Write/Edit) and Claude Code's full
prompt and tools are the likely reasons and are not yet separated. One task and 2-3 runs per arm is an anecdote, not a
benchmark.

Security: instruction files are only read when they are regular files whose real path is inside the repo (a symlinked
`CLAUDE.md` or `.claude` directory could otherwise put any file on the machine into the prompt). Repo instructions are trusted input; the README says to use `--no-project-instructions`
in repos you did not write.

Tier 0: 2,980 input tokens per turn in a repo without instruction files (unchanged), binary
2.35 MB.

## 0.0.7 (2026-10-06)

**Labels for runs**, the training data for routing by task type later. Kept in `~/.local/share/tazuna/labels.jsonl`
(append-only, next to the run log):
- `/good` or `/bad` (with an optional note) in the live UI rates the last finished run; it is never sent to claude.
- `tazuna --verdict good|bad [--note "..."] [--id RUN]` does the same from the shell (default: the latest run).
- `tazuna --label-git` labels older runs from git: `kept` when a run's output was later committed unchanged, `discarded`
  when a day has passed and it is in no commit and gone from the working tree. Runs in the current repo are labelled this
  way automatically when tazuna starts. A user verdict always wins over a git label.
- Run log lines now have an `id` and the content hash of every file the run changed (what the git check looks for).
  `tazuna --log` shows a verdict column.

No change to what claude sees or does. Binary 2.35 MB (0.0.6: 2.31 MB); other Tier 0 metrics
unchanged (2,980 input tokens per turn, 7 ms replay render).

## 0.0.6 (2026-10-06)

**Rules router v1.** Without `--model`/`--effort`, tazuna picks a lane per session from the number of tracked files:
tiny repos (≤ 50 files) start on Sonnet 5.5 medium, everything else on Opus 5.5 medium (unchanged default). If
verification fails on the cheap lane, tazuna switches the session to Opus 5.5 high (resuming it; one cache rebuild) and
runs the fix rounds there. The lane and reason show at the top of the transcript and go into the run log, with whether the
run escalated. `--model`/`--effort` turn routing off.

Why only repo size: in the cc-lean sweeps Sonnet medium matched Opus medium on easy and medium tasks in tiny repos at about a
third of the cost, but no cheap text signal separated hard tasks, and on real tickets in large repos Opus medium was the best
value. Routing by task type needs labelled outcomes from the run log (the Jev step).

Also: Python verification finds `test_*.py` files, uses pytest only when it is installed (else `uv run pytest`, else
`unittest`), and treats "no tests ran" (exit 5) as nothing to verify instead of a failure.

Tier 1 (4 tiny-repo tasks, 3 easy and 1 medium, × 3 runs; router vs always Opus 5.5 medium, judged together):

| Metric | Always Opus medium (= 0.0.5) | Router | Change |
|---|---|---|---|
| All hidden tests pass | 12/12 | 12/12 | same |
| Judge | 8.62 | 8.08 | −0.54, within noise (lower on 2 of 4 tasks) |
| Mean cost | $0.086 | $0.029 | **−67%** |
| Median time | 19 s | 11 s | −42% |
| Escalations | — | 0 of 12 | verification passed or found no tests |

Large repos route to Opus medium as before, so the real-work numbers are unchanged. Escalation is exercised by a live test
(Sonnet medium → failed verification → Opus high → verified), not yet by a benchmark task.

Tier 0:

| Metric | 0.0.5 | 0.0.6 | Change |
|---|---|---|---|
| Input tokens per turn before any work | 2,977 | 2,980 | within noise |
| Binary size | 2.27 MB | 2.31 MB | |
| Replay render | 7 ms | 7 ms | |

## 0.0.5 (2026-10-06)

**Bash output cap.** Bash output over 8,000 characters (~2k tokens) is shrunk before claude sees it: passing-test chatter
(`=== RUN`, `--- PASS`, `ok`) goes first, then the start (~2.5k chars) and the end (~5.5k chars) are kept with a note saying
how many lines were omitted and where the full output was saved (a private temp directory outside the repo; claude can read
any part of it). `--bash-cap N` sets the limit, `0` turns it off. `read` and short output are unchanged.

Tier 1 (6 real-work Go tasks × 2 runs, **Sonnet 5.5 high** to stay inside the 5h usage window, same binary with and without
`--bash-cap 0`, interleaved, judged together):

| Metric | Cap off (= 0.0.4) | Cap on | Change |
|---|---|---|---|
| Judge | 6.79 | 6.58 | within noise |
| All hidden tests pass | 8/12 | 8/12 | same |
| Hidden tests passed | 84% | 84% | same |
| Mean cost | $0.45 | $0.40 | −12%, within noise |
| Mean input tokens | 833k | 676k | −19% (mostly fewer turns: 17 vs 20) |
| Tool output chars per run | 61.5k | 58.8k | −4% |
| Outputs shrunk | — | 7 in 12 runs | |

Long bash output is rare on these tasks, so the direct saving is small (−4% of tool output). The cap costs no quality and
guards against blowups that would otherwise be re-sent every turn (live check: `seq 1 20000`, ~27k tokens, became ~2k, and
claude read the one line it needed from the saved file). Kept on by default.

Also: saved outputs go to a private directory (0700, unpredictable name) as new 0600 files, so on a shared /tmp nobody else
can read them or plant a symlink.

Tier 0:

| Metric | 0.0.4 | 0.0.5 | Change |
|---|---|---|---|
| Input tokens per turn before any work | 2,983 | 2,977 | within noise |
| Binary size | 2.27 MB | 2.27 MB | |
| Replay render | 8 ms | 7 ms | within noise |

## 0.0.4 (2026-10-06)

**Lean tool server in Rust.** The bash/read/write/edit tools are now `tazuna mcp`, a mode of the same binary, instead of a
bundled Node script. tazuna no longer needs Node. Tool names, descriptions and schemas are byte-identical, and replies match
the Node server's byte for byte on a captured request set (now a unit test). One behavior fix: a command that hits the
120 s timeout is killed with everything it started (process group); the Node server killed only `bash`, which left
children running (seen with graphify's 30-minute orphans in the addon benchmark).

| Metric | 0.0.3 | 0.0.4 | Change |
|---|---|---|---|
| Input tokens per turn before any work | 2,980 | 2,983 | within noise (same tools) |
| Tool server memory | 53 MB (Node) | 1.8 MB | −97% |
| Binary size | 2.20 MB | 2.27 MB | +3.2% |
| Replay render | 6 ms | 8 ms | within noise |
| Runtime dependencies | claude, node | claude | Node dropped |

Tier 1 not run: what claude sees (tool definitions and replies) is identical, so a Tier 1 run would measure only noise.
Remaining differences are error wording for failed file operations and the timeout fix.

## 0.0.3 (2026-10-06)

**Verified, not claimed.** When claude says a run is done, tazuna runs the tests for what the run changed (Go: `go test`
on the changed packages; Rust, Python and JS when set up; `--verify "<cmd>"` to override, `--no-verify` to disable).
Failures go back to claude as the next message, up to 2 extra rounds in the same process. The run log records
`verified`, `verify_failed`, `claimed` (nothing to test), `skipped` or `off`.

Tier 1 (6 real-work Go tasks × 2 runs, Opus 5.5 medium, same binary with and without `--no-verify`, interleaved, judged
together):

| Metric | Verification off (= 0.0.2) | Verification on | Change |
|---|---|---|---|
| Judge | 6.71 | 7.38 | +0.67, within noise (see below) |
| All hidden tests pass | 8/12 | 8/12 | same |
| Hidden tests passed | 87% | 90% | within noise |
| Mean cost | $0.67 | $0.71 | +5%, within noise |
| Median turns | 22 | 23.5 | within noise |
| Time added by verification | — | median 27 s (10-112 s) | zero tokens |

Verification passed on the first try in all 12 runs: claude already tests its own work on these tasks, so it never had to
send a failure back. Both arms saw identical inputs, which makes the +0.67 judge difference pure noise and is why the
judge threshold is now ±0.75. Verification stays on by default: it costs no tokens when tests pass, it is a safety net for
runs that skip or break tests, and `verified` vs `claimed` is useful data for routing. Possible follow-up: skip it when
claude's own last test run already covered the same packages after its last edit.

Also:
- Auto-detected test commands only include plain path characters, so a file name can't inject shell syntax.
- The run log and Tier 0 now record the `claude` CLI version.

Tier 0:

| Metric | 0.0.2 | 0.0.3 | Change |
|---|---|---|---|
| Input tokens per turn before any work | 2,873 (claude 2.1.289) | 2,980 (claude 2.1.291) | +104 from Claude Code's update, not tazuna |
| Binary size | 2.13 MB | 2.20 MB | +3.2% |
| Replay render | 5 ms | 6 ms | within noise |

## 0.0.2 (2026-10-06)

**Run log.** Every run (one message until claude's result) appends a line to `~/.local/share/tazuna/runs.jsonl`: why it
stopped (`completed`, `stopped`, `max_turns`, `budget`, `error`, `crashed`), cost, time, turns, per-turn tokens, cache hit
rate and the turns where the prompt cache missed, tool calls and failures, the files this run changed, and an empty
`verdict` to label later. Live and headless runs are both logged. `tazuna --log [N]` prints the last runs as a table.

Tier 0 only (no change to what claude sees or does):

| Metric | 0.0.1 | 0.0.2 | Change |
|---|---|---|---|
| Input tokens per turn before any work | 2,876 | 2,873 | within noise |
| Binary size | 2.02 MB | 2.13 MB | +5.4% |
| Replay render (median of 7) | 6 ms | 5 ms | within noise |

Measurement fix: replay render time is now the median of 7 warm runs. The first launch of a new binary includes a ~200 ms
macOS scan, which made the 0.0.1 figure (231 ms) wrong; re-measured at 6 ms.

## 0.0.1 (2026-10-06)

First version: live mode driving `claude` over stream-json with the cc-mid profile, replay, four UI designs, headless mode.

Baseline (no previous version to compare against):

| Metric | Value |
|---|---|
| Real work, Sonnet 5.5 high (12 runs) | judge 6.29 · all hidden tests 8/12 · median $0.31 · 240 s · 30 turns |
| Real work, Opus 5.5 medium (12 runs) | judge 7.25 · all hidden tests 8/12 · median $0.70 · 254 s · 29 turns · cache 94% |
| Parity vs `claude -p`, same profile | judge +0.55 (noise) · mean cost +3% (noise, same day) · per-turn overhead +58 tokens |
| Input tokens per turn before any work | 2,876 |
| Binary / replay render | 2.0 MB / 6 ms |

Fixes found while measuring:
- Exact cost was wrong after a resumed process (claude reports cost per process); now summed.
- `ctrl-c` used to kill and resume, which left a "No response requested." turn in history; now uses the stream-json interrupt.
- The auth token was visible to commands the agent runs; the tool server now removes it from its environment.
