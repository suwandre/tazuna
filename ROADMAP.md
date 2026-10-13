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

## Where we stand (2026-10-12, v0.0.41)

Two paths, chosen by where the code may go (`--data trusted|any`):

- **Claude path (default; code that must stay with Anthropic, e.g. company work).** Official `claude` CLI on the
  subscription; router (tiny repos Sonnet, else Opus 5.5 medium), Haiku explorer, diagnostics after edits, verification
  (tests + type check, two fix rounds); `--run` on Opus directs from a boss by default (Haiku subagents do the work, 0.0.49).
  Company tickets, one judge pool, 10 runs each: stock Claude Code 6.15 at $2.67 per ticket, tazuna without boss 5.90 at
  $2.18, with boss 5.25 at $1.38 (judge within noise, cost −37%). Go tasks: tazuna without boss 7.46 (judge without
  reference) at $3.97 for 6 tasks vs stock 7.58 at $5.46; boss 7.33 at $2.70. No plugin (ECC, superpowers,
  caveman, graphify, GStack) beat stock; the Sonnet + Opus advisor stays opt-in (−0.6 judge on tickets).
- **Cascade (code that may leave the machine; `--preset cascade`, or any `--provider`).** tazuna's own loop with DeepSeek
  V4.1 Flash at max effort on CheaperInference, no time limit, $1 per task; a task it cannot finish goes to Opus 5.5 medium
  on claude (high measured: 3-10x the cost, no gain). October tasks (merged after DeepSeek's release, 14 runs): 13/14 fully
  solved, judge 7.79 / 8.32 (without / with reference), $8.36, vs tazuna on Opus 9/14, 6.79 / 6.93, $38.90. Best quality
  per dollar measured in every set where it is allowed. Other cheap models (GPT-6 Luna, Qwen3.8 Flash, GLM-5.3 Flash) and
  own-loop ideas (elision, plan tool, few-turns prompt) did not beat it.
- **Footprint:** binary 3.35 MB (rustls, strip, LTO); UI 7.3 MB RAM, 7 ms to first frame, ~1.5% CPU; tool server 2.1 MB
  (Tier 0 at 0.0.50). A whole agent run (`bench/agent_footprint.py`, one small fix): **own loop 21-22 MB peak RAM**
  (engine 18-19 MB, UI 3 MB; 27 MB at 0.0.41), 0.1 s CPU; **claude path 262-282 MB** (the `claude` CLI 248-267 MB, tazuna 7 MB, tool server 7 MB), 0.6-0.7 s CPU.
  The own loop needs ~10x less memory; on that task it took 29-69 s against 5-6 s, all of it waiting for the provider.
- **Measurement:** the benchmark kit lives in `bench/` (runner, blind Opus judge with and without reference, usage guard,
  cheap-judge calibration). Judge rounds run one pass by default and print their call count first; the guard blocks judging
  without a fresh meter reading after a chained judge round spent extra usage (2026-10-12).
- **Shipped:** live UI (four designs), headless `--run`, recordings and replay, run log and labels (`/good`, `/bad`,
  `--label-git`), router, repo / repo-lean / lean profiles, verification, diagnostics, `--self-review`, `--review-model`,
  permission modes and plan mode, resume, slash commands, `@` files, images, `!` shell mode, explorer/tester/boss subagents,
  `--advisor`, presets, data policy with a trusted-provider list, own loop for OpenAI-compatible providers (Chat Completions
  and Responses; CheaperInference and Experiential Labs price lists), cascade hand-over.

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
   have): all done. Own loop ask and edits modes (0.0.42; plan mode claude-only), `ctrl-g` external editor (0.0.43),
   `ctrl-b` background-task view with stop (0.0.44), `/rewind` files and conversation (0.0.45; claude path, Claude Code's
   own edit tools), `ctrl-x` stash and `/vim` (0.0.46).
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
   current pick; the flag starts the gauge. Built (2026-10-12): `bench/roles.json` and `bench/model_check.py` over
   Artificial Analysis and Epoch AI (~1,000 models, ~80 benchmarks, scaled and weighted per role, compared at the pick's
   effort and price), daily at 9:00 with a notification. First run: 7 flags, the strongest for the cascade's cheap step
   MiMo V2.6 Pro (+0.08), Ling 3.1 Flash and Haiku 5.5 max (all on Experiential Labs). Gauged MiMo (2026-10-13):
   same 7/7 and judge 9 as DeepSeek on caddy-unix at ~30x the cost; not adopted. Public scores miss tokens per task, so
   the check now compares cost per task: our own runs first (MiMo 33x), then Epoch's cost columns (MiMo 1.3x: public
   cost data understates it on our loop), then the token price. Other sources
   checked: SWE-bench's and Aider's own leaderboards stopped in Feb 2026 / Oct 2025 (Epoch carries current SWE-bench
   and Aider numbers); LMArena, vals.ai and tbench.ai offer HTML only (Terminal-Bench is in both sources). Scraping more sites waits until that loop proves useful (check their terms).
8. ~~**Profiles with a model and effort per role**~~ (0.0.36 `--preset`; 0.0.35 `--data trusted|any`) (user, 2026-10-10): main model, explorer, implementer, tester and the
   cascade's cheap and strong model each set per profile (for example a cheap default, an expensive "max" profile with Opus
   at max effort, a ZDR-only profile), overridable by hand. The model picker (5) fills them later.
7. **Tiers instead of a swarm** (2026-10-09, after the own-loop benchmark: model choice is worth ~10×, splitting 10-30%).
   Each tier is an A/B against Opus alone and DeepSeek alone on the Go tasks and the hard tasks:
   - **Cascade:** the cheap model on the own loop first; tests, type check and diagnostics decide; on failure the task goes
     to Opus with the cheap attempt (cross-engine escalation, reusing the router's `escalate`).
   - **Planner → executor:** Opus plans in plan mode (files, exact changes, how to verify), a cheap model executes the plan
     (DeepSeek on the own loop; Haiku for code that must stay on the subscription), Opus is called again only when
     verification fails. Untested so far; differs from the failed Haiku + Opus *reviewer* run, which reviewed after the fact.
   - ~~**Cheap explorer for Claude:**~~ an MCP tool backed by a DeepSeek sub-loop (0.0.48, `--explorer MODEL@URL`): +9% cost,
     same quality, slower; Opus kept reading itself. Opt-in.
6. **Stock Claude Code with popular efficiency plugins vs tazuna** (user idea, 2026-10-09): caveman, headroom (context
   compression between Claude Code and the API), superpowers, ECC (affaan-m/ECC: discipline hooks, memory), GStack
   (garrytan/gstack: role team, /review, /qa), graphify alone (only tested with ponytail and agent-skills); ponytail + graphify + agent-skills already measured (−1.2
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

- **Monthly API credits on Max/Team plans** (Anthropic, 2026-10-07: $100 Max 5x, $200 Max 20x, up to $500 Team; Claude
  API, Agent SDK and `claude -p` with an API key; not Claude Code). Usage with a key from the linked Console organization
  draws from the credits, not the 5h/weekly limits, and stops when they run out (never extra usage). Uses: benchmarks and
  the judge on credits; the claude path on an API key as an option; later Claude in the own loop through the Messages API
  (27 MB instead of ~270 MB). The impersonation rule is unchanged: the update allows the official channels, not reusing
  Claude Code's OAuth token. Waiting on the user (plan, claimed credits, key in `~/.anthropic-key`).

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
- Smaller items: a verification shortcut.

## Tried or ruled out (do not retry without a new reason)

| Idea | Result | Reason |
|---|---|---|
| jevgrep in cc-mid | −0.7 to −0.8 judge | no saving on Opus medium |
| ponytail + graphify + agent-skills | −1.2 judge, 2× slower | always-on skill/memory layers cost quality |
| Per-turn model switching | rejected | breaks the prompt cache (per subagent is fine) |
| Boss + Opus high reviewer (tickets) | same judge as boss (5.20), +56% cost | fixes widened the patches (scope −0.4); 0.0.49 follow-up |
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
- 2026-10-11: plugins (ECC, superpowers, caveman, graphify, GStack, alone and all together): none beats stock Claude Code on
  quality or cost; plugin comparison (Next 6) answered for these five, headroom left. Sonnet 5.5 + Opus advisor: Opus-level
  judge at −45% cost (1 run) — a candidate claude-path default next to `--boss`, after a second run.
- 2026-10-11: advisor second run: Go 7.08 mean (−0.4 vs Opus default, within noise) at −44% cost, but company tickets 5.10
  vs 5.70 (Opus default) and 6.15 (stock) at $0.61 vs $2.18 per run. Stays opt-in, not the default: the drop shows on the
  work that matters most. Boss stays the cheaper Opus-quality option.
- 2026-10-11: own-loop ideas from arXiv 2609.20804 (eliding old tool outputs, a plan tool): no gain on DeepSeek, dropped.
  Calls are bound by the provider's wait, not prompt size; plan 4/7 vs 4/7 solved. Fixed on the way: the own loop's budget
  now counts per task (follow-ups had reset it) and network outages get ~4 minutes of retries.
- 2026-10-12: own loop without a time limit by default, $1 per task (0.0.39): cheap models are slow but cost cents;
  `--max-minutes` for speed. Cascade on October tasks: 13/14 solved, judge 7.79 / 8.32, $8.36 for 14 runs vs $38.90 for
  tazuna on Opus (9/14, 6.79 / 6.93). Time per task varies more by run than by setting (lazygit: 10 min or 2.7 h).
- 2026-10-12: Qwen3.8 Flash, GLM-5.3 Flash (CheaperInference, Experiential Labs) and free Qwen3.8 Flash Next: none beats
  DeepSeek on CheaperInference on price or speed. A "few turns" prompt cut DeepSeek's turns 21% and tokens 38% but not cost
  (more output). DeepSeek on CheaperInference stays the cheap step. Judge rounds now skip empty tasks and need the guard
  after a chained judge round spent extra usage.
- 2026-10-12: judge rounds cost less: one pass by default (two only for results a default rests on), with reference only,
  and `--dry-run` prints the number of Opus calls first. A cheap judge (GPT-6 Luna max, 51 patches Opus had graded): r 0.89,
  88% within one point, but the same order as Opus for two patches of one task only 64% of the time (high 66%, xhigh 62%).
  Luna max screens out clear losers (2+ points behind); Opus stays the judge for decisions.
- 2026-10-12: cascade by default with `--provider` (0.0.41). Open: the hand-over runs Opus 5.5 at medium, a default never
  measured; it only sees the tasks the cheap model failed, where effort matters most (terraform: medium 1/3 in every
  harness, max 3/3). Next with weekly headroom: the October hand-over tasks (prometheus, lazygit, telegraf) at high vs medium.
- 2026-10-12: hand-over effort measured (DeepSeek hands over after 30 min; 1 run each): prometheus medium 2/3 hidden tests,
  suite intact, $0.63; high 0/3, suite broken, $6.12. Telegraf: judge 6 both (medium broke the suite, high added an
  unrelated change), $0.44 vs $1.42. Lazygit: DeepSeek solved it alone in the high run. Medium stays: high costs 3-10x for
  no gain here.
- 2026-10-12: footprint of a whole agent run measured for the first time (`bench/agent_footprint.py`): own loop 27 MB peak
  RAM and 0.1 s CPU vs the claude path 262-282 MB and 0.6-0.7 s, on the same small fix. Goal 1's "real footprint wins come
  with the own loop" holds: ~10x less memory. Wall time goes the other way (provider wait), so speed is the own loop's gap.
- 2026-10-13: boss + Opus high reviewer on tickets: 5.20, same as boss, at $2.15 (the old default's price). Not adopted.
  The boss gap on tickets held in a second pool (−0.70 vs 5.90): consistent, just inside noise. Boss stays the `--run`
  default; `--no-boss` for tickets where quality matters more than cost.
- 2026-10-13: the system's TLS (0.0.28) cost every tazuna process ~4.4 MB at start, unnoticed because the roadmap quoted
  Tier 0 from 0.0.20. rustls plus a size-tuned release build (0.0.50): UI 11.6 → 7.3 MB, tool server 6.3 → 2.1 MB, binary
  4.04 → 3.35 MB. Lesson: compare Tier 0 against the last recorded line on every release, not against the roadmap.
- 2026-10-13: boss mode becomes the default for `--run` on Opus (0.0.49). Company tickets, 10 runs: judge 5.25 vs 5.90
  (−0.65, within the ±0.75 threshold) at $1.38 vs $2.18 (−37%); Go −0.13 at −32%. Quality per dollar decides. Watch:
  the ticket drop is near the threshold and sits in edge cases; a reviewer pass on the boss's diff is the next lever if
  it holds. Live sessions keep the old default (boss there is unmeasured; `--preset boss` turns it on).
- 2026-10-13: cheap explorer for Claude measured (DeepSeek V4.1 Flash via `--explorer MODEL@URL`, 6 Go tasks): judge 7.33 vs
  7.13, cost +9%, runs up to 3x longer. Opus asked 2-3 questions per task and read on its own otherwise; a slow explorer is
  not worth waiting for. Boss mode remains the claude-path lever ($2.70 vs $3.97 for the same quality).
- 2026-10-12: the benchmark kit moved into the repo (`bench/`: run, judge, guard, calibration, task definitions, results and
  diffs). Company tickets and cloned plugins stay out.
- 2026-10-11: clean benchmark (7 tasks merged after DeepSeek's release, 2 runs, judged without reference): cascade +0.7
  judge at ~1/8 the cost of tazuna on Opus. The earlier ~2.4-point lead on older tasks was mostly the judge's reference
  anchoring; the real lead is smaller but holds. Cascade stays the plan for code that may leave the machine (`--data any`).
- 2026-10-10: cascade (0.0.33, DeepSeek max first, Opus on failure): 8 of 8 tasks fully solved for $0.17, no escalation
  needed; best quality per dollar measured so far for code that may leave the machine. Next: a time budget for the cheap
  model (prometheus), a second run, then the cascade as the own-loop default.
- 2026-10-10: `--boss` on two tickets, two runs: boss + Haiku about the default's quality at 0-24% less (Go: −32%);
  boss + Sonnet no gain at higher cost (dropped). Boss + Haiku stays opt-in; quality holds, the saving on tickets is modest.
- 2026-10-10: `--boss haiku` (0.0.32, Opus only delegates, a hook blocks its file tools): Go −32% cost at the same judge,
  tickets −45% for −0.30 judge (1 run). Best quality per dollar on the Claude path so far; opt-in until a second ticket run,
  then a default candidate. Planner → executor dropped (1.6× the default's cost, lower judge). Explorer stays (off: +0.5
  judge at 2.4× cost). tazuna default vs stock Claude Code: Go −27% same judge; tickets −18%, judge −0.25.
- 2026-10-09: model picker per role added as Next 5 (user idea): public benchmarks shortlist, our gauge decides, labels
  refine. Public scores alone would have chosen wrong on three of our levers.
- 2026-10-09: Haiku tester rejudged by Fable 5.1 high (same diffs): tester lower on both judges (dimension mean −0.21 vs
  Opus −0.32 of 5), driven by T-946 (one tester run made no change); even on the other four. −14% cost is at the noise
  line, so it stays opt-in. Fable as judge leaves out "overall" on large prompts; compare dimension means.
- 2026-10-09: earlier compaction ruled out from the run log (313 runs): peak context p50 58k, p90 155k; 8.8% of read
  tokens above 80k, so ~3-5% net after the summary's cost, inside noise. Cheap `claude` levers (Next 2) done.
- 2026-10-09: Fable 5.1 xhigh reviewer gauge (T-872, 0.0.27): same judge at 3.4× cost; full run skipped, stays opt-in.
