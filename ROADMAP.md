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

## Where we stand (2026-10-08, v0.0.17)

- **Quality: at or above stock Claude Code, never proven below.** Go tasks, all harnesses at current versions judged
  together: tazuna default 6.67 (1st of 8; OMP 6.54, stock 6.38). Company tickets (5 tickets, Opus medium): tazuna lean 5.95,
  repo profile 5.95, stock 6.00 (same day); in the latest two-ticket pool the default (repo + diagnostics) 7.12 vs stock
  6.62. All within noise.
- **Cost:** default −5% vs stock on Go tasks, −2% on the company tickets; lean −19% to −28% (no repo hooks or skills);
  diagnostics −11% vs without. Cheaper than OMP and Hermes by 20-25%.
- **Footprint:** UI 7 MB and 6 ms to first frame vs Claude Code's 190 MB and 0.45 s; the `claude` process underneath is
  the same size as Claude Code's own, so the total only shrinks with tazuna's own loop.
- **Shipped:** live UI (four designs, Cockpit default), headless `--run`, recordings and replay, run log and labels,
  router (tiny repos to Sonnet), repo / repo-lean / lean profiles with trust, verification (tests + type check, new errors
  only), diagnostics after each edit (default), `--self-review`, `--review-model`, background-command handling,
  multi-line input, select mode, cache-miss display, Tier 0 footprint metrics.

## Done

1. ~~Benchmark repo-lean~~ (2026-10-07; v0.0.17 then lifted it to stock quality at stock cost: stays opt-in).
2. ~~Re-run the cross-harness table~~ (2026-10-07, README).
3. ~~Quality experiments~~: diagnostics after each edit (kept, default); self-review turn (+0.38 judge, +31% cost: opt-in);
   fresh-context reviewer with Haiku 5.5 implementing (worse than Opus alone: opt-in).
4. ~~Cost experiments~~: stuck detection and hash-anchored edits ruled out by 210 recorded runs (no loops; edits ~0.2% of
   cost).
5. ~~Footprint and UI~~: Tier 0 measures startup, UI CPU and memory; multi-line input; select mode (ctrl-s); cache misses in
   the Cockpit; Claude Code's own edits shown as diffs. Thinking text: not possible (the CLI streams only token counts).

## Next

1. **Daily-driver gaps** (the user's bar for real use: "not comfortable yet"). Done in 0.0.18: permission modes and plan
   mode, input editing and history, resume, subagents under their call. Done after 0.0.18: slash commands (claude runs
   them in stream mode; tazuna shows their output, marks /clear and /compact, completes them with tab), `@` file
   completion, the to-do list as a checklist, images (`@shot.png`, dragged paths, `ctrl-v`), `!` shell mode. Left (nice to
   have): external editor, stash, vim mode, a background-task view, rewind.
2. **Cheap `claude` levers** before the own loop: ~~5-minute prompt cache~~ (v0.0.20: −10%, same judge; headless default),
   ~~a lower Bash output limit~~ (+30%: ruled out), ~~repo map~~ (v0.0.21: no gain, the agent reads as much),
   ~~Sonnet explorer subagent~~ (v0.0.23: −35% turns but +12% cost), ~~Haiku explorer~~ (v0.0.24: −28% cost, judge −0.25;
   default since 0.0.25), Haiku test-runner worker (v0.0.26: −14% cost, judge −0.40; a second judge agrees: opt-in),
   ~~earlier compaction~~ (ruled out from 313 recorded runs: at most ~9% of reads, see below). Each an A/B against the
   default.
3. **Own agent loop for other providers** (design: [docs/OWN_LOOP.md](docs/OWN_LOOP.md)): tazuna calls provider APIs directly (OpenAI-compatible, Anthropic
   with an API key), runs its own tools and reuses the event format, so the UI, run log, diagnostics, verification and
   reviewer keep working. Unlocks cheap non-Anthropic implementers with a Claude reviewer and the footprint floor (no
   `claude` process). Claude on the subscription stays on the CLI path. Needs an API key and a spending limit: first tests on a $15 prepaid
   CheaperInference test account (90+ models; the prepaid balance is the limit).
4. **Real use and labels** once 1 is good enough: rate runs (`/good`, `/bad`, `--label-git`); the Jev router needs ~100-200.
5. **Model picker per role** (after 3; the front end of the learning router, "Where tazuna can lead" 1). New models beat
   old ones in one category every few weeks; re-benchmarking by hand does not keep up. Three layers:
   - **Shortlist from public data:** models from a whitelist (or what a login exposes; OpenRouter lists 600+) ranked per
     role from Artificial Analysis (has an API), vals.ai, Terminal-Bench, SWE-style and domain benchmarks. Implementer:
     coding score, tokens per task, speed. Reviewer: strongest coding score. Explorer: cheapest with good long context.
   - **Our gauge decides:** a model entering a shortlist gets a 1-ticket A/B against the current pick (same judge, usage
     guard); it replaces the pick only if it wins on price to performance. Public scores alone would have picked wrong
     here: Fable 5.1 xhigh as reviewer (3.4× cost, same judge), Sonnet explorer (+12% cost), Haiku + Opus reviewer.
   - **Real use refines it:** labels (Next 4) adjust the per-role pick.
   First step: a hand-written role table and a weekly Artificial Analysis check that flags a whitelisted model beating the
   current pick; the flag starts the gauge. Scraping more sites waits until that loop proves useful (check their terms).
7. **Tiers instead of a swarm** (2026-10-09, after the own-loop benchmark: model choice is worth ~10×, splitting 10-30%).
   Each tier is an A/B against Opus alone and DeepSeek alone on the Go tasks and the hard tasks:
   - **Cascade:** the cheap model on the own loop first; tests, type check and diagnostics decide; on failure the task goes
     to Opus with the cheap attempt (cross-engine escalation, reusing the router's `escalate`).
   - **Planner → executor:** Opus plans in plan mode (files, exact changes, how to verify), a cheap model executes the plan
     (DeepSeek on the own loop; Haiku for code that must stay on the subscription), Opus is called again only when
     verification fails. Untested so far; differs from the failed Haiku + Opus *reviewer* run, which reviewed after the fact.
   - **Cheap explorer for Claude:** an MCP tool backed by a DeepSeek sub-loop, so Opus on the claude CLI can hand off
     reading to a non-Claude model.
6. **Stock Claude Code with popular efficiency plugins vs tazuna** (user idea, 2026-10-09): caveman, headroom (context
   compression between Claude Code and the API), superpowers; ponytail + graphify + agent-skills already measured (−1.2
   judge, 2× slower). Same Go tasks and company tickets, 2 runs each, Opus xhigh judge; one plugin per 5-hour window.

## Where tazuna can lead (gaps no other harness fills, 2026-10-08)

1. **A router that learns from the user's own outcomes** (`/good`, `/bad`, git kept/discarded): model and effort per kind
   of task, cheaper and better with use. Needs real use and labels (Next 4) and the Jev router (parked).
2. **The Claude subscription and cheap API models in one harness, without impersonation:** Claude through the official CLI
   as planner or reviewer, other models implementing through the own loop (Next 3).
3. **Measured by default:** per-version metrics, A/B tests on the user's repos, cache health and cost per turn, shown to
   the user; the CHANGELOG with numbers is part of the product.
4. **Agent config that travels:** `CLAUDE.md` and `AGENTS.md`, the repo's hooks and skills applied to any model, not only
   the one whose vendor defined the format.
5. **Session replay** as a review tool.

Worth borrowing: an OS sandbox for commands (Codex), checkpoints and rewind (parked).

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
- A cheaper or stricter review (Sonnet, or acceptance criteria one by one at high effort).
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
| Stuck detection | not built | 0 of 184 tazuna runs repeated a call with the same failure |
| Hash-anchored edits | not built | old text in edits is 0.6% of output tokens, ~0.2% of cost |
| Bash output capped at 8,000 chars | +30% cost | the agent re-runs commands to see what was cut |
| Repo map in the first message | +3% cost, same judge | the agent still reads the files it changes in full |
| Sonnet explorer subagent (`--explorer sonnet`) | +12% cost, −35% turns, same judge | the reading moves, it does not shrink |
| Haiku tester subagent (`--tester haiku`) | −14% cost, judge −0.40 | Fable judge agrees (lower, −0.21 of 5 vs Opus −0.32); the gap is one no-change run on T-946, even without it; saving at the noise line: opt-in |
| Earlier compaction (at ~80k) | at most −9% reads, before the summary's cost | median run peaks at 58k; only 8.8% of read tokens sit above 80k (313 runs, no benchmark) |
| Fable 5.1 xhigh reviewer (`--review-model`) | 3.4× cost, same judge (1 ticket) | the review costs twice the run; it talked Opus out of the root-cause fix |
| Haiku explorer subagent (`--explorer haiku`) | −28% cost, −41% turns, judge −0.25 | default since 0.0.25 (price to performance) |

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
- 2026-10-08: repo-lean's gap traced to reading less (short prompt + Claude's tools); two check lines brought it to stock
  quality at stock cost (0.0.17). The default repo profile with diagnostics stays best in its pool (7.12 at stock cost).
- 2026-10-08: 5-minute prompt cache for headless runs (−10%, same judge); Bash output cap ruled out (+30%).
- 2026-10-08: repo map A/B: no gain (reads unchanged); stays opt-in.
- 2026-10-08: Sonnet explorer A/B: fewer turns, +12% cost; stays opt-in. Next cheap try: a Haiku explorer.
- 2026-10-08: Haiku explorer on all 5 tickets: −28% cost, judge within noise but lower on 3 of 5; opt-in until a third run.
- 2026-10-08: the measure is price to performance, not quality alone (user): a small judge drop for a clear saving is
  worth it, a strong reviewer can buy quality back. Haiku explorer made the default (0.0.25).
- 2026-10-08: Haiku tester A/B: −14% cost, judge −0.40, not lower on every ticket: inconclusive (user), opt-in for now,
  retest later with a second judge model. Fixed headless runs ending
  while claude was about to wake for a finished background explorer.
  Headless runs now deny permission questions (one run hung 25 min on AskUserQuestion).
- 2026-10-09: own loop (0.0.28) with DeepSeek V4.1 Flash on CheaperInference vs tazuna's default on Opus 5.5, six Go tasks
  × 1: judge 8.33 vs 7.17, hidden tests 94% vs 87%, cost $0.53 vs $4.26 (−88%), 4.5× the wall time (provider wait). Needs a
  second run before any default changes; the result mixes model and loop.
- 2026-10-09: second run each: Opus 7.17 / 7.08, DeepSeek V4.1 Flash (own loop) 8.33 / 8.20 on finished runs, max effort 8.00,
  GPT-6 Luna max 5.75. DeepSeek ≈ Opus or better on routine Go fixes at ~1/10 the cost; medium confidence (6 tasks, gap mostly
  one task). Next: engine call timeout and retry (2 of 18 runs hung), then harder tasks.
- 2026-10-09: model picker per role added as Next 5 (user idea): public benchmarks shortlist, our gauge decides, labels
  refine. Public scores alone would have chosen wrong on three of our levers.
- 2026-10-09: Haiku tester rejudged by Fable 5.1 high (same diffs): tester lower on both judges (dimension mean −0.21 vs
  Opus −0.32 of 5), driven by T-946 (one tester run made no change); even on the other four. −14% cost is at the noise
  line, so it stays opt-in. Fable as judge leaves out "overall" on large prompts; compare dimension means.
- 2026-10-09: earlier compaction ruled out from the run log (313 runs): peak context p50 58k, p90 155k; 8.8% of read
  tokens above 80k, so ~3-5% net after the summary's cost, inside noise. Cheap `claude` levers (Next 2) done.
- 2026-10-09: Fable 5.1 xhigh reviewer gauge (T-872, 0.0.27): same judge at 3.4× cost; full run skipped, stays opt-in.
