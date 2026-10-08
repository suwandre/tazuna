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
   default 6.67 (1st of 8; OMP 6.54, stock 6.38 within noise) at $0.41: −5% vs stock ($0.43), −20-25% vs OMP and Hermes.
   Lean tazuna 6.04 at −28%.
3. **Quality experiments**, one at a time, each against a same-day stock baseline:
   1. ~~Compiler errors in edit results~~ (dirge, OMP). Done 2026-10-08 in v0.0.13 as a `PostToolUse` hook that also
      fires after Bash edits; default on: same judge, −11% cost vs without.
   2. ~~Finish rules plus a self-review turn~~ (v0.0.14 `--self-review`, opt-in): +0.38 judge for +31% cost on two
      tickets; not worth the default. Possible follow-up: a cheaper review (Sonnet, or only the acceptance-criteria step).
   3. ~~A verifier in a fresh context~~ (v0.0.15 `--review-model`, opt-in). Gauge with Haiku 5.5 high implementing and
      Opus 5.5 medium reviewing: lower judge than Opus alone on both tasks, and the reviewer passed spec-divergent work.
      Not worth a full run as is; a stricter reviewer (high effort, acceptance criteria one by one) is the untested next try.
4. **Cost experiments:** stuck detection (stop when two attempts give identical results; back to planning after 3 failed
   fixes); hash-anchored edits in tazuna's own edit tool (OMP claims −61% output tokens).
5. **Footprint and UI:** ~~add RAM, CPU and startup time to Tier 0~~ (done 2026-10-08); ~~real multiline input~~ (done); ~~cache misses in the Cockpit sidebar~~ (done);
   ~~text selection under mouse capture~~ (done, ctrl-s); thinking text; check the edit-diff view in a real run; README with the company
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
- Cross-provider pairs: a cheap non-Anthropic implementer (GPT-6 Luna, MiMo 2.6 Pro, DeepSeek V4.1 Flash) with a Claude
  reviewer (`--review-model` already works with any implementer). Needs tazuna's own loop for the implementer. Only
  Haiku 5.5 high + Opus 5.5 medium reviewer has been tried (2026-10-08: worse than Opus alone); other pairs and a stricter
  reviewer are untested.
- `--fallback-model`, `--advisor`; a cross-family judge; real sandboxing; git tags and a fixed scoreboard.
- Smaller items: a verification shortcut, moving the benchmark kit into the repo; attribute result events that arrive
  after a run ended (claude waking up for a background command) to that run in the run log.

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
- 2026-10-07: cross-harness re-run (tazuna + stock, Go tasks, Sonnet high, one judge pool per task): quality near the top.
  (The "stock got 29% cheaper" reading here was the result-event bug below, not drift.)
- 2026-10-07: full cross-harness re-run at current versions: tazuna leads on quality (within noise of OMP and stock) and is
  cheaper than all Claude Code-based rivals. An apparent +37% cost vs stock was a benchmark bug: `claude -p` emits one
  result event per background-command wake-up and the runner kept only the last; it now sums them all.
- 2026-10-08: `--diagnostics` A/B on the company tickets: identical judge, −11% cost; made the default in v0.0.13.
- 2026-10-08: `--self-review` A/B (2 tickets): +0.38 judge, +31% cost; stays opt-in.
- 2026-10-08: working rule: big benchmarks run in night windows; daytime checks cap new runs at 40% of the 5-hour window.
- 2026-10-08: Haiku 5.5 high + Opus reviewer gauge (1 ticket + 1 Go task): worse than Opus medium alone; full run skipped.
