# Roadmap

The north star for tazuna, the ideas we have tested or parked, and the order of work. Read this before proposing or
starting new work. A change of direction is fine, but it gets a line in the decision log below, with the reason.

## Goals

1. **Small footprint, clean UI/UX.** As little CPU, RAM and storage as possible without giving anything up, and a clean
   terminal UI.
2. **Token efficiency and output quality.** As few tokens (and dollars) as possible per task at the best output quality we
   can reach: "metric-maxxing", measured against the other harnesses (Claude Code, Codex, OMP, OMO, OMH, Hermes, pi, maki,
   dirge, opencode, kilocode, cline). Beat them, or at least match them.

**Method.** Start from the leanest setup, benchmark it, then add one thing at a time and keep it only if the numbers say
so (CHANGELOG.md records every measured change; tiers and noise thresholds are defined there).

**Constraint for Claude.** Use the official `claude` CLI on the user's subscription. No impersonation (no Claude Code
headers or user agent, no reuse of its OAuth token outside the CLI). Everything has to come from what the CLI exposes:
flags, system prompt, tools, MCP, settings, hooks. Other providers get tazuna's own agent loop later.

**Limits to keep in mind.** On Claude, the `claude` CLI underneath (~200 MB, ~1 s start) sets the floor for footprint;
tazuna's own part is small (2.4 MB binary, ~7 MB RAM, ~1% CPU). Real footprint wins come with tazuna's own loop.

## Where we stand (2026-10-07, v0.0.11)

- Five real company tickets (acme/app), 10 runs per arm, Opus 5.5 medium: tazuna lean 5.65 judge at −19% cost
  vs stock Claude Code 5.55; repo profile 5.35 at −5% (it is stock Claude Code's prompt plus tazuna's extras). Quality is
  tied across arms; cost is where tazuna wins.
- The cross-harness table in the README is from v0.0.1 and has not been re-run since.
- `repo-lean` (lean prompt + Claude's own tools + the repo's hooks, skills, CLAUDE.md) is built but not benchmarked.

## Order of work

1. ~~Benchmark repo-lean~~ (done 2026-10-07: −12% cost, −0.90 judge vs same-day stock; stays opt-in, repo profile stays
   the default).
2. ~~Re-run the cross-harness table~~ (done 2026-10-07, all current versions, one judge pool per task; README): tazuna
   default 6.67 (1st of 8; OMP 6.54, stock 6.38 within noise) at $0.41, +37% cost vs stock ($0.30), −20-25% vs OMP and
   Hermes. Lean tazuna 6.04 at stock's cost.
3. **Quality experiments**, one at a time, each against a same-day stock baseline:
   1. Compiler errors in edit results (dirge, OMP): after an edit, return the new type errors at once. For Claude's own
      tools, through a `PostToolUse` hook tazuna adds itself, so every profile gets it. It must also fire after `Bash`:
      on the company tickets every arm made most edits with `python3`/`sed` through Bash, not with Edit/Write.
   2. An instruction against ending early, plus a self-review turn before "done" (diff vs acceptance criteria, unrelated
      changes).
   3. A verifier in a fresh context that tries to break the work (a worker and its checker never share a context).
4. **Cost experiments:** stuck detection (stop when two attempts give identical results; back to planning after 3 failed
   fixes); hash-anchored edits in tazuna's own edit tool (OMP claims −61% output tokens).
5. **Footprint and UI:** add RAM, CPU and startup time to Tier 0; real multiline input; cache misses in the Cockpit sidebar;
   text selection under mouse capture; thinking text; check the edit-diff view in a real run; README with the company
   results.
6. **Real use and labels:** use tazuna for real work and rate runs (`/good`, `/bad`, `--label-git`). The Jev router needs
   ~100-200 labeled runs.

## Parked (not tested yet)

- Hand-off to a fresh session with a structured summary instead of compaction (Amp, pi).
- Checkpoints and rewind (OMP).
- A careful lane (phased effort): 7.17 vs 6.33 judge at 2.5-3× the cost of Opus medium, in an early test.
- A heavy profile with subagents (Haiku explorer, Opus planner, Sonnet implementer).
- Jev router beside the rules (needs labels; one call per decision, pinned version, falls back to the strong option, must
  beat a simple classifier first).
- Memory layers (jevmem, Memmy experiment).
- Own-loop rules for other providers: byte-stable prompt and tools, large outputs to disk, repeat the goal, every loop has
  done/check/stop/budget; Jev context trimming.
- Other providers: Codex as the OpenAI path, Kimi K3, MiniMax M3.
- `--fallback-model`, `--advisor`; a cross-family judge; real sandboxing; git tags and a fixed scoreboard.
- Smaller items: a verification shortcut, moving the benchmark kit into the repo.

## Tried or ruled out (do not retry without a new reason)

| Idea | Result | Reason |
|---|---|---|
| jevgrep in cc-mid | −0.7 to −0.8 judge | no saving on Opus medium |
| ponytail + graphify + agent-skills | −1.2 judge, 2× slower | always-on skill/memory layers cost quality |
| Per-turn model switching | rejected | breaks the prompt cache (per subagent is fine) |
| Impersonating Claude Code | rejected | ban risk; Hermes, OMP, Empryo do it |
| Multiple agents on sequential tickets | rejected | 39-70% worse in the cited data; ~7× tokens for swarms |
| xhigh / max effort | rejected | ~4× cost of medium for the same score |
| Haiku for coding, low effort on real work | rejected | quality drops |
| More verification work | neutral | Claude already tests its own work |
| Bash output cap | neutral | rarely triggers |
| Tool search, OmniRoute | skipped | no expected gain; dropped by the user |

## Decision log

- 2026-10-03: start lean (cc-mid profile) and measure everything; Opus 5.5 medium as default (best price to quality).
- 2026-10-06: the repo's own config (hooks, skills, CLAUDE.md/AGENTS.md) is respected by default, because "those configs
  exist for a reason". This gave up the lean cost advantage in repos with config; repo-lean is the attempt to get it back.
- 2026-10-07: the repo profile loaded the user's claude.ai connectors (~42k tokens per turn); fixed with
  `--strict-mcp-config` in v0.0.11.
- 2026-10-07: after ten versions, back to the two goals above; this file is the reference against drift.
- 2026-10-07: repo-lean benchmarked on five tickets: −12% cost but −0.90 judge vs same-day stock (beyond noise). Not the
  default. Open question: why the same short prompt does well in lean (6.00) and badly with Claude's tools and repo config.
- 2026-10-07: cross-harness re-run (tazuna + stock, Go tasks, Sonnet high, one judge pool per task): quality near the top,
  cost lead gone on these tasks (stock was 29% cheaper than on Oct 3-6). Cost claims need same-day baselines every time.
- 2026-10-07: full cross-harness re-run at current versions: tazuna leads on quality (within noise of OMP and stock); the gap
  to close is cost vs stock Claude Code on these tasks (more turns).
