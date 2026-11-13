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
- **Cascade (code that may leave the machine; any `--provider`; `--preset reviewed` since 0.1.4: DeepSeek 1017 on
  SubRouter plus the Opus medium reviewer, replacing `--preset cascade`).** tazuna's own loop with DeepSeek
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
2. **Cheap `claude` levers** before the own loop: ~~5-minute prompt cache~~ (v0.0.20: −10%, same judge; headless default; boss mode back to 1 hour in 0.1.78),
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
     verification fails. ~~Tried 2026-10-10~~ (`plan-haiku`, `plan-ds`: 1.6× the default's cost, lower judge; dropped). On
     the own loop the reviewer does this job cheaper: Opus medium after DeepSeek, not a plan before it (2026-10-29).
   - ~~**Cheap explorer for Claude:**~~ an MCP tool backed by a DeepSeek sub-loop (0.0.48, `--explorer MODEL@URL`): +9% cost,
     same quality, slower; Opus kept reading itself. Opt-in.
6. **Stock Claude Code with popular efficiency plugins vs tazuna** (user idea, 2026-10-09; done 2026-10-11 for ECC,
   superpowers, caveman, graphify and GStack, none beat stock: decision log; only headroom left): caveman, headroom (context
   compression between Claude Code and the API), superpowers, ECC (affaan-m/ECC: discipline hooks, memory), GStack
   (garrytan/gstack: role team, /review, /qa), graphify alone (only tested with ponytail and agent-skills); ponytail + graphify + agent-skills already measured (−1.2
   judge, 2× slower). Same Go tasks and company tickets, 2 runs each, Opus xhigh judge; one plugin per 5-hour window.

## Next main item (2026-10-14): beat each model's own harness with that model

Goal (user): tazuna's own loop beats other harnesses, the model maker's own one included, with any model, not only
DeepSeek. So far the own loop was only measured with DeepSeek, and never against another harness running the same model.
Without subscriptions, pay per token:
1. DeepSeek V4.1 Flash: own loop vs Claude Code (official `claude` CLI with `ANTHROPIC_BASE_URL` on the provider's
   Anthropic-compatible route; DeepSeek, GLM and MiniMax recommend it) vs one open harness (pi or opencode). October
   tasks, 2 runs each: hidden tests, judge, tokens, time, cost.
2. If tazuna loses: find the cause in the transcripts, then a per-model profile (the tool names and formats the model
   knows, its thinking settings, a tuned prompt).
3. Model by model: GPT (Codex CLI), GLM and MiniMax (Claude Code), Kimi (Kimi CLI, 1 task: $3/$15 per M).
First data (0.0.53, rclone): GPT-6 Luna in Codex 3/3, judge 8, 453 s, 4.7M input; in tazuna's own loop 3/3, judge 7,
1451 s, 14.6M. A gap with GPT: find where the 3x input goes (tool output size, file reads, repeats) before more runs.
0.0.54 (read window): Luna's input -37% at the same result; judged again: Codex 8, tazuna 8 (input 6.3M vs 4.7M).
0.0.55: a headless task that stops without any change gets a nudge (two gateway runs had ended empty). 0.0.56:
streamed calls, empty replies asked again. Luna on BeatAPI vs Codex: 8 and 5 vs 8: tazuna can match Codex but is less
consistent; more runs and a second judge pass are needed before calling it. Next: the DeepSeek A/B on a steadier
provider. DeepSeek in the own loop: judge 9 (best in the pool). BeatAPI's `deepseek-v4.1-flash-hc` is a second provider on par
with CheaperInference (3/3, judge 9, $0.027, 641 s).
0.0.57, Haiku 5.5 on rclone: Claude Code (subscription) 2/3, 118 turns, 47.6M input; own loop on BeatAPI 3/3, 1/3, 2/3
at $0.03-0.06 per run (the 1/3 ended in empty replies: thinking lost in the OpenAI format); UnoRouter 2/3.
0.0.58-0.0.61 (see CHANGELOG): Claude on the Messages API (no more empty replies; costly at max effort on BeatAPI), GPT
with `prompt_cache_key` (BeatAPI cache 18-47% -> 96-97%: Sol 3/3 $0.39, Luna 3/3 twice at $0.08-0.11), server-side
clearing of old tool results for Claude, no more cut streams. DeepSeek 0.0.57 beats 0.0.53 (tests 23/24 vs 17/24, cost
-42%). Server-side clearing of tool results removed again in 0.0.62 (Haiku re-read and looped: 163 turns, 0 edits).
0.0.63: UnoRouter caches only unstreamed calls; with that, Luna on its si group ran rclone in 7-9 minutes at 92-96%
cached for ~2 cents (3/3, 2/3); MiMo V2.6 Flash there 3/3 at 98.5% cached for ~3 cents (on BeatAPI it never edited a
file); DeepSeek V4.1 Flash there 3/3 in 30 minutes at 97.4% cached for ~3.2 cents (174 turns, 18.7M input).
Judged 2026-10-17 (CHANGELOG): DeepSeek on CheaperInference 9 stays the best quality per dollar; Sol 8 on BeatAPI and
UnoRouter; Luna 7 on BeatAPI and Codex but 2-4 on UnoRouter; MiMo 7; Haiku 3-8; tazuna 0.0.57 beats 0.0.53 (8.33 vs
6.83). Through UnoRouter the same model scores lower (likely less reasoning upstream): cheap there is not better.
0.0.65 (2026-10-17): Luna on a vetted UnoRouter group 6 (slower and dearer than BeatAPI's 7); DeepSeek on UnoRouter's
default key 9, 7, 4, 9, 9, 9 against CheaperInference 8-9, but costlier than estimated (dearer groups) and caching less.
Haiku 5.5 high on tazuna's Claude path 7-8 (3/3 x3, ~1% of the 5-hour window per run); through UnoRouter a group that
answered in Chinese, 1-3. Sol through UnoRouter: 54% cached, dearer than BeatAPI, group offline; the $5 balance is used up.
UnoRouter so far: cheap and good for DeepSeek and MiMo at some hours; unsafe for Claude and GPT (cut reasoning, a
substituted model, prices that moved 2-30x in a day).
With the full check (`bench/uno_route.py`: price incl. output vs OpenRouter, cache, long reasoning with tools, identity)
DeepSeek on a pinned group (a7-burngate-5609) ran rclone 3/3 at 91% cached for 1.9 cents in 27 minutes, judge 8
(CheaperInference: 8 at 2.4-5.2 cents).

**UnoRouter config uno-1 (2026-10-18); only runs made with it count** (`bench/realwork/uno-runs.jsonl`, written by
`uno_run.sh`; `leaderboard.py` reads it). tazuna 0.0.65 or newer (no streaming on UnoRouter, 413 or empty 200 sent again as
a stream, `max` retried at `xhigh`, cache key for GPT, Messages API for Claude, `TAZUNA_HEADER` for the group); before each
run `uno_route.py MODEL` (price incl. output vs OpenRouter, at least 50% cached on a grown conversation, a long reasoning
call that comes back (asked again without tools if the model calls one), an English answer near the right number,
PIN <group>); at the start `--price` (live price, cap 5x the estimated run, refused when the group is gone or over budget);
during the run a stop when the group goes, its input price rises over 1.5x or under 50% is cached after 15 calls; effort
max; tasks rclone, caddy-unix, caddy-retry. Default model on UnoRouter: the cheapest with 3+ runs, a judge floor (mean
minus one standard error) of 7 or more and at most one failure in five. A config change is a new version (uno-2) with its
own runs. Earlier runs measured our mistakes (streaming without cache, cut calls, unpinned groups) and do not count.

**Group switching in the own loop: built in 0.0.68-0.0.69 (dropouts, upstream 400s, failing cache; prices still watched
outside); config uno-2.** DeepSeek on three tasks: judge 9, 9, 7, 98% cached, ~0.4 cents a run, no failure: the first
default under the rule (CheaperInference: 8-9 at 2.4-5 cents). Still open: a live group switch, more runs and models. uno-1 (one pinned
group) for DeepSeek: judged 8.5, 9, 4 and two runs lost to vanished groups.
**The design it follows** (the price log, 29 hours, CHANGELOG 2026-10-18: DeepSeek's cheapest group vanished within the
hour half the time, groups listed for 3 hours one time in ten; prices of a group that stays moved little within a run). Today a run pins one group (`TAZUNA_HEADER`); when it drops out, the calls
fail for ~15 minutes and the task ends. Instead, per model a dated list of groups that passed the check, ranked by the
live price of a whole run (input, cache, output; the price list is public and free), and at each call the group to use:
- switch at once to the next group on `get_channel_failed`, repeated 5xx/429, or an empty reply after the stream fallback;
- switch when the last 10 calls cached under 70%;
- prices: read before each run (groups listed 3 hours or more first) and every few minutes during it, but only to stop
  a run whose whole cost (input, cache, output) rose over 1.5x; no switch for a lower price (a fraction of a cent against
  a cache miss);
- no listed group left: a fixed provider (CheaperInference, BeatAPI or OpenRouter) or stop and ask;
- new groups enter the list only through the check, in the background, with a daily cent cap.
Measured as usual: the same tasks with switching vs one pinned group, cost, cache, judge. Then: `uno_route.py` becomes a
provider-neutral check (base URL + model) for any new pay-as-you-go provider; stability round 2 for DeepSeek on the list;
Haiku at high effort once BeatAPI is topped up; CheaperInference top-up for the cascade.

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

- **DeepSeek Harness (`dsh`) as a rival for the own loop** (own-loop phase, after the Claude engine). DeepSeek's official
  harness, 0.2.1-rc.5 (MIT, developer preview, signed by Hangzhou DeepSeek), installed by the user. Plugins on Cordis:
  subagents (spawn and fork, up to 8 live), plan mode, goals with a goal-round driver (keeps working until the goal is
  met: a likely source of the reported higher quality at more time and cost), web search through DeepSeek. The CLI runs
  the desktop app's own core, so its numbers stand for the app. Benchmarkable: `dsh headless --json "task"` (events as
  NDJSON). Default model `deepseek-flash` on `deepseek-official`; other providers through `llm-pi-ai` (a config patch in
  `~/.dsh/profiles/headless/cordis.patch.yml`). Claims found: no benchmark numbers, anecdotes only. Plan: tasks/ six x
  two with DeepSeek V4.1 Flash 1017 on SubRouter (the own loop's model) against the own loop, judged together; then, if
  it wins, find which plugin does it and borrow the idea. Never with company code (it calls DeepSeek's API). Cost:
  likely above the $0.50 weekly SubRouter cap; estimate first.
- Hand-off to a fresh session with a structured summary instead of compaction (Amp, pi): not needed yet (decision log
  2026-10-29).
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
- In-loop five-lens review (`--review-model`, 0.0.51): first run on 5 company tickets inconclusive (3 of 5 reviews ran
  before boss mode's subagents finished; fixed). Rerun those 3 with the fix, defect check and a judge pass, after the
  weekly reset; adopt only if it removes defects at a cost below the no-review run's +65%.
- The 5 company PRs with a peer's round-1 findings as ground truth (bench/company, t-838/841/842/845/851) are the
  better ticket set: candidate standard set for Tier 1 on company work.
- Smaller items: a verification shortcut.
- sgrep (Context.dev, local ripgrep plus static-embedding rerank; -21% cost claimed on SWE-bench Verified with GPT-6 Luna,
  70 vs 68 of 100): a nearby smart-search tool (jevgrep) cost judge on Opus. If tested, then as the own loop's `search`
  on the long October tasks (turns, tokens, time), after reading its code.

## Tried or ruled out (do not retry without a new reason)

| Idea | Result | Reason |
|---|---|---|
| jevgrep in cc-mid | −0.7 to −0.8 judge | no saving on Opus medium |
| Own loop: covering tests while working, the changed packages once at the end (`TAZUNA_OWN_TEST_FOCUS`, 2026-11-12) | hugo and k6, 3 runs each, Fable judge: 5.00 vs 4.33 and 6.00 vs 5.67 (within noise), but 20-79 min vs 16-43 | meant to save test time, it ran longer; the speed gap had closed anyway (0.1.104-0.1.106) |
| Claude path at Opus low instead of medium (tazuna 0.1.106, 2026-11-12) | six tasks x 3 runs, Fable judge: 5.83 vs 6.83, hidden 60 vs 63/72, $6.29 vs $12.43, 4 vs 7 min | lower on 5 of 6 tasks, a point beyond noise: half the price is not worth it; medium stays |
| Own loop: what already fails on the last commit, told with the first test run (`TAZUNA_BASELINE`, 2026-11-11) | hugo 3+3 runs (Fable judge): 4.00 vs 3.33, hidden 14 vs 13/24, but 56-64 min vs 39-67, more calls | the note arrived in every run and test time stayed (33-38 min); the background suite competes for the CPU |
| Own loop: exported-API rule only for code outside the repo (`TAZUNA_API_SCOPE`, 2026-11-11) | hugo 3+3 runs: judge 4.33 vs 5.00 (Opus; Fable re-judge 4.33 vs 4.33), hidden 15/24 both, −18% calls | the design faults the judge names (template boundary, editing the fork) stayed; not the rule's doing |
| ponytail + graphify + agent-skills | −1.2 judge, 2× slower | always-on skill/memory layers cost quality |
| Per-turn model switching | rejected | breaks the prompt cache (per subagent is fine) |
| LongHarness (arXiv 2610.13075: open questions steer reading, an evidence ledger) | not tested | document QA, not coding; plan tool and elision (nearby ideas) already failed; its backbones did not finish our tasks |
| Ouro (a harness that writes its own loop via eval) | rejected | thought experiment without guardrails; self-modifying code is against tazuna's safety rules |
| Reasoning window (only the last 8-24 turns' DeepSeek reasoning back, 0.0.86) | judge -0.1 to -1, cost +27% against all of it | each step breaks the cached history; with less of its own reasoning the model writes more |
| LobeHub self-improving harness on Kimi K3 | not adopted | preliminary (selected retries); Kimi K3 is $3/$15 at 41 tok/s with a lower Terminal-Bench than DeepSeek; automated tuning on our few tasks would overfit |
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
| `--repo-tools core` (14 of 32 Claude Code tools, 2026-11-02) | judge -0.50 (6.38 vs 6.88), tokens -13%, cost +9% | under noise both ways, lower on 4 of 6 tasks; fewer tokens did not make it cheaper: opt-in |
| Boss mode's workers at the session's effort (xhigh in the user's sessions, 2026-11-02) | Haiku xhigh $1.02 a run vs medium $0.45, judge 6.55 vs 6.79 | the dearest and slowest for no gain: workers default to medium (0.1.36) |
| Lean profile on Opus 5.5 medium (2026-11-02) | judge 6.71 vs 6.75, cost +16%, 24% faster | 42% more calls with four plain tools eat the smaller prompt: opt-in |
| `--batch-calls` (one prompt rule: independent tool calls in one turn, 2026-11-02) | judge -0.17 (6.62 vs 6.79), calls 22.4 vs 21.9, tokens +13%, cost +9% | Claude Code already batches where it can; the rule did not cut calls: opt-in |
| `--rtk` (rtk's Claude hook, 2026-11-02) | judge +0.12 (7.00), tokens +15%, cost +6% | command output -53%, but the agent made more calls: no saving in tazuna: opt-in |
| `--boss-pick` (the boss's lead picks each worker call's effort, Sonnet for hard changes, 2026-11-03) | tasks/ judge 6.62 vs 7.00 at +53% cost; tasks-hard 7.12 vs 6.19 at 2.1x cost, +34% time | the lead sent the main change to Sonnet in 17 of 21 runs, also on easy tasks; the hard-task gain is mostly one task: opt-in |
| Splitting a long run into a fresh conversation with a summary (2026-11-03, simulated on 75 recorded Opus runs, 15 with 40+ calls, up to 99) | best split point saves 0% on a typical run, 3% median on the 40+ call runs, none over 15% | Opus 5.5 reads its cache at 0.05x, so a long context costs little next to a hand-off (new prefix written, files read again). Model: agents ≈ calls × √(new tokens per call ÷ 2 × hand-off cost), ~1 at 20 calls, ~2 at 50. Only quality or time could justify splitting; untested |
| Sonnet 5.5 medium as the default's lead (2026-11-03) | judge 5.18 vs 7.54, hidden 35/48 vs 43/48, cost -55% | stops sooner and misses parts of the task |
| Opus 5.5 low effort as the lead (2026-11-03) | judge 5.79 vs 7.54, hidden 35/48 vs 43/48, cost -34% | same: fewer calls, less finished; medium is the floor for the lead |
| `--search-tools` / `--batch-own` in the own loop (DeepSeek, from DeepSeek's own harness, 2026-11-04) | tools: -13% time, -5% calls, hidden 30/39 vs 33/39; plus the few-calls rule: +22% calls, +22% time | inside noise or worse; the harness's speed comes from elsewhere (its prompt, a test subagent, effort high): opt-in |
| Repo map in the first message | +3% cost, same judge | the agent still reads the files it changes in full |
| Sonnet explorer subagent (`--explorer sonnet`) | +12% cost, −35% turns, same judge | the reading moves, it does not shrink |
| Haiku tester subagent (`--tester haiku`) | −14% cost, judge −0.40 | Fable judge agrees (lower, −0.21 of 5 vs Opus −0.32); the gap is one no-change run on T-946, even without it; saving at the noise line: opt-in |
| Earlier compaction (at ~80k) | at most −9% reads, before the summary's cost | median run peaks at 58k; only 8.8% of read tokens sit above 80k (313 runs, no benchmark) |
| Fable 5.1 xhigh reviewer (`--review-model`) | 3.4× cost, same judge (1 ticket) | the review costs twice the run; it talked Opus out of the root-cause fix |
| GPT-6 Astra or Opus 5.5 max reviewer behind DeepSeek 1017 (`tazuna-ownsubxrev`, `-ownsubxopus`) | judge 7.17 / 7.67 vs 7.00 alone, hidden 10/10 vs 9/10 (3 tasks × 1, 2026-10-29) | both caught terraform's gap; out-of-task findings cost −2.0 once each; Astra ~$0.016 a review but stalls at max |
| Best of two cheap attempts, Opus picks with their test results (`tazuna-ownsubxbest2`) | judge 7.08 vs 6.92 one run (Luna, 3 tasks x 2, 2026-10-24); the pick took the better attempt 4 of 5 times | +0.16 within noise for twice the calls and the slower attempt's time; at most an opt-in for hard tasks |
| Five-lens review + one fix round on the own loop (DeepSeek 1017 implementing, Opus 5.5 high reviewing, `tazuna-ownsubgate`) | judge 6.6 vs 8.2 alone, hidden tests 5/9 vs 9/9, +46% turns (3 tasks × 3 runs, 2026-10-22; 5 of those gate runs saw the reference fix). Clean redo on 0.0.83 (2026-10-25, 1 round): 6.5 vs 7.3 alone, same hidden tests, turns 355 vs 414 | findings in 3 of 9 runs; every fixed run judged lower (the fix widens the patch); the cheap model alone beat Opus on these tasks, and still does with clean runs |
| Haiku explorer subagent (`--explorer haiku`) | −28% cost, −41% turns, judge −0.25 | default since 0.0.25 (price to performance) |
| t3code as a rival (0.1.7, 2026-11-08, one-line fix, Opus 5.5 medium) | first call 34.7k tokens vs 17.6k for stock `claude -p`, ~725 MB RAM vs 250 MB (its server alone 418 MB idle) | on Claude it runs the `claude` CLI through the Agent SDK with Claude Code's full prompt plus its own Artifact tool (33.7k chars); nothing leaner to borrow. Its idle compaction before the cache expires is parked |

## Decision log

- 2026-11-13: the Claude path's explorer takes broad searches only (0.1.121); the main model reads what it can find
  itself. "Send every question to the explorer" made each short ask wait on Haiku, and the suite showed tazuna at about
  1.4x Claude Code's time; the lighter rule is level with it at 23% less cost. Also: benchmarks compare tazuna as the
  user runs it (`--no-boss`): `--run` on Opus defaults to boss mode, which the interactive app never uses, and a first
  batch measured that instead (1.8x Claude Code's time).
- 2026-11-12: every bench run gets a Go build cache of its own. The hidden tests compiled into the shared one after each
  run, and DeepSeek agents (tazuna, pi, OMP) read their names out of it with `strings`; that round's flagged pairs are
  left out and the round runs again.
- 2026-11-11: the own loop sends DeepSeek its earlier reasoning back on SubRouter (0.1.106), reversing 0.0.96's "no
  provider passes it on": some now do, and it ran faster on hugo, caddy and k6 at a judge within noise. The known
  failures told up front (`TAZUNA_BASELINE`) saved no time and stay off.
- 2026-11-11: the bench judge is Fable 5.2 at high (the user's choice), after Opus 5.5 xhigh judged every round before.
  Re-judging the six hugo candidates of that day: equal on four, one point lower on two, same order. Rounds are compared
  within themselves (each has its baseline arm), not across the change. Fable's own weekly cap is not in the plan meter
  the guard reads: a judge call that hits it fails rather than spilling into extra usage.
- 2026-11-11: bench agents get no web at all (the proxy allows model APIs and logins only), and leakscan flags any
  transcript holding two of a task's new hidden test names. Reason: OMP read hugo's and k6's merged fixes, hidden tests
  included, from cdn.jsdelivr.net and a Gitea mirror (4 of its 6 runs there; 4 runs of OMP and pi on 2026-11-09), past
  a denylist of GitHub and the Go proxies. With them out, the own loop is level with OMP, OpenCode and pi; its gap is
  time (2-4x on hugo and k6). The own loop gets the cover-all rule by default (0.1.103).
- 2026-11-11: on Claude, tazuna's gap to Claude Code was in coverage, not in the fix: fewer of the task's commands and
  error cases changed and tested (0.1.96 asks for all of them; level with Claude Code at -25% cost over six tasks).
  Kept as is after measuring: the Haiku explorer (mixed by task), the test rule (without it -0.17 at +36% cost),
  Opus as the model (Fable 5.2 medium the same judge at +40% cost), medium as the default effort (low is 1.8x cheaper
  at -0.2, a candidate once more task kinds are measured). The bench's sandbox proxy had tilted k6 against tazuna since
  the VM came in (0.1.93: verify no longer sends back failures that predate the change).
- 2026-11-09: MCP servers belong to tazuna, not to one model (0.1.80). The user's own list (`/mcp add`, Claude Code's
  format) goes to claude and, through a new MCP client, to the own loop; remote servers get tazuna's own OAuth login.
  Reason: claude.ai connectors only reach the `claude` CLI, so a DeepSeek session had no Jira at all, and an all-in-one
  harness should not need Claude to set up a tool. The own loop gets one `mcp` tool (names in its description, parameters
  on first use) so its tool list and prompt cache never change. Off by default: no measured metric moves.
- 2026-11-09: headless boss mode writes a 1-hour prompt cache (0.1.78). The 5-minute cache lost the lead's cache while it
  waited on its workers and missed Claude Code's cross-session prefix; 313 recorded runs re-priced: −3.2% on the boss
  baseline (CHANGELOG). Found while comparing t3code.
- 2026-11-08: CI builds every platform, macOS too, and a release needs no Mac (`./release.sh` checks, tags and pushes).
  Reason: the Mac built macOS because macOS runners counted 10x against a private repo's Actions minutes; the repo is
  public now, where standard runners cost nothing. Installs the usual way too: one-line installers and `tazuna update`.
  Next: a Homebrew tap (a tap repo and a token for CI to update it); npm, scoop and winget when users ask.
- 2026-11-08: a user without our bench sees what the bench's machines see. The built-in list ships SubRouter's picker
  list and Mistral's checked routes (on `MISTRAL_API_KEY`), never a route on a private host or the bench's scores;
  `/model` greys out what the user's SubRouter key may not use and drops SubRouter's copies of subscription models.
  Reason: a new install on the Windows PC listed 4 SubRouter models and no Mistral (the Mac 34 rows), and a pick of
  SubRouter's "claude-opus-5.5" ran Opus off the plan and failed with a 403 on a key without it.
- 2026-10-31: the Claude path ties stock Claude Code 2.1.314 on Opus medium (6.88 against 6.94, hidden 20/26 each) at
  −44% cost but 1.8x the time (boss mode). Bug (fixed in 0.1.8): a headless run that passed its final check hung while a
  shell a finished subagent had left running was still counted as work claude would wake up for. Weekly canary script
  ready (DeepSeek 1017 and Sol at xhigh; the user installs the launchd schedule).
- 2026-10-31: without the boss (the interactive default) the Claude path is level with stock Claude Code too (6.81 against
  6.88, hidden 20/26 each) at −24% cost and 1.3x the time (1.15x against Claude Code run at the same hour: the API is
  slower in the afternoon; the Haiku explorer is the largest part tazuna adds); with the boss −44% and 1.8x. Defaults stay (boss headless, none
  interactive). Real-use quality comes from stored sessions, git outcomes and a later judge: the user does not label runs.
- 2026-11-06: other users bring one SubRouter key, not one per model and provider. One key can hold a provider list per
  model (`subrouter_model_providers`): a test key allowing DeepSeek 1017 only on lingai and Sol only on bangai sent 6/6
  calls of each to its own provider (Sol is cheaper on lingai, so a union of the lists would have drifted;
  bench/onekey.py, under $0.01). On a key open to every model the lists hold too, as long as the flat
  `subrouter_providers` field stays empty (filled, it limits every other model to those providers: 503s). Plan: the vetted lists ship with tazuna and go on the user's one key, by hand from
  printed steps or, opt-in, by tazuna with the account's access token; per-provider pinned keys stay for the bench.
- 2026-11-05: GPT-6.1 Sol judges like Opus for comparisons (114 runs rejudged: 92% of decided pairs in the same order,
  every arm decision the same, scores ~0.6 higher, $0.095): bench rounds may use Sol as the judge while the weekly limit
  is tight, one judge per comparison. Best DeepSeek provider of the screen: o2oai (5.0 min, 29 calls, $0.003 billed,
  Sol 9), then znu (6.7 min, $0.0025, Sol 8, now in the route); o2oai is left out of the route until its Nov 1 failure
  ages out (Nov 8).
- 2026-11-05: SubRouter provider screen for DeepSeek 1017 (17 probed, 13 ran caddy once, $0.15): all ~2x DeepSeek's
  own per-call time; the spread comes from calls (providers that drop reasoning take 2-3x the calls) and caching. o2oai
  and znu look best. The route key still names bob-team, which left SubRouter: refresh it (`routes.py refresh`) before
  the next SubRouter runs. Review test built (bench/company/review.py, ten company PRs, Sol grades against the peer
  reviewers' findings; smoke: 2 caught, 2 partly of 6): runs after the Nov 7 reset.
- 2026-11-04 (night): judged on five tasks, DeepSeek's official API and SubRouter's route give the own loop the same
  quality (7.25 vs 7.00 on four tasks, 6.60 each on five); the official API is 2.5x faster with half the calls and costs
  twice as much. Provider choice is the user's trade (time vs price), not a tazuna default. Reasoning sent back: less
  uncached input on every task and nothing worse, so default on DeepSeek's own API (0.1.55), opt-in elsewhere. DeepSeek credit used up (¥0.33 left).
- 2026-11-04 (evening): the DeepSeek Harness's two-times speed was the provider, not the harness. tazuna's own loop on
  DeepSeek's official API (same model, effort xhigh, caddy-quoted-braces, four runs): 2.5-4.1 minutes and 22-41 calls
  against the Harness's 3.2-4.4 and 43-59, half its tokens and 35-55% of its cost less, 5/5 hidden tests every run; on SubRouter's route the same
  loop took 5.5-8.8 minutes and 56-73 calls with 2-6x the uncached input. SubRouter's route stays 4x cheaper per run, so
  it is a trade (time for price), not a harness fix. Sending reasoning back (TAZUNA_SEND_REASONING) halved the uncached
  input on two runs; too few runs to change the default (¥3.58 of the credits left).
- 2026-11-04: own loop on the company tickets (DeepSeek alone): 5.25 against Opus's 6.08, same share of review findings,
  2% of the cost, 4x the time, one run with regressions (the hand-over to Opus would have caught it). For the user's work
  on the subscription, Opus stays the default; DeepSeek with the hand-over is the cheap path for unattended or
  plan-limited work. DeepSeek Harness on one Go task: as good as the own loop, twice as fast, a quarter of the uncached
  input: study its call and cache pattern before more runs (¥4.16 of the user's credits left).
- 2026-11-03: built for the Nov 7 test (after the weekly reset): `tazuna lessons` (a repo's recurring review findings,
  any GitHub repo, stored in tazuna's data folder), `--lessons`, `--history`, the reviewer reading the lessons. Plan: the
  ten company tickets x one run x four arms (default, lessons, history, five-lens review), on history copies of the
  templates, scored by reviewer findings (`fcheck.py`) and the judge; all three together only if two or more help.
  Then the own loop.
- 2026-11-03: company tickets, batch 2 (five PRs by other authors): judge 5.70 against the merged PRs; against the
  authors' own first versions 5.19 vs 6.62 (partly design anchoring); tazuna's versions repeat 76% of the reviewers'
  applicable findings (Critical 2 of 4, Major 19 of 28). Next Claude-engine candidate with a measurable target: a review
  step (the five-lens prompt caught the most findings in the October recall test), scored by findings caught on both
  batches, not by the judge alone; cost bar: below the no-review run's +65%.
- 2026-11-03: company tickets, batch 1 (five of the user's PRs, two runs): today's default scores 6.00 against the
  user's own Claude Code first round 6.20 and October's boss runs 5.80 (all noise), no new type errors or failing tests,
  15.8 minutes and $2.77 a ticket (October's boss: 36 minutes, $1.78). Batch 2 (five PRs by other authors, #1198, #1190,
  #1375, #1167, #1309, later-round findings as a second ground truth) prepared in bench/company/tasks2.
- 2026-11-03: test scope confirmed on the hard tasks (same judge and hidden tests, -19% cost, 2x faster, arms
  interleaved). Cheaper leads ruled out: Sonnet 5.5 medium and Opus 5.5 low both lose 1.8-2.4 judge points and 8 hidden
  tests on tasks/. The Claude engine's levers are now measured except the boss-only items and company tickets; Opus
  medium with a Haiku explorer at medium and test scope is the default.
- 2026-11-03: worker count settled for the Claude engine: default = Opus + Haiku explorer (live), boss = Opus + three
  Haiku workers (headless); boss pays by moving reading, editing and test output to a cheaper model (its lead reads 248k
  tokens a run against 786k), not by splitting, and trades time for cost. Boss-only items left, none measured: escalation
  through the boss hook; keeping the lead's cache warm with wake-ups; two workers instead of three (the implementer runs
  the checks itself; the tester hand-off costs ~2 minutes a run). Efforts: lead medium (low queued), Haiku workers medium
  (measured), the default's explorer now medium too (0.1.47).
- 2026-11-03: offline check of a per-task decider (stored, judged runs; config chosen on one half of the runs, scored on
  the other): within the Claude engine no headroom. tasks/ (12 configs): picking per task 7.21 against the best fixed
  config 7.27 (default 6.92); tasks-hard (3 configs): 7.22 against 7.12. Per-task winners flip between halves: the
  configs differ by noise per task. The one choice with a payoff is the engine (own-loop DeepSeek at ~$0.02 a run, in
  October's shared pools ~1 judge point above Opus, ~3x slower; company code stays on Claude), and that is a rule that
  exists (`--data`, cascade with escalation on failing tests). The decision model stays parked until a task set shows
  config differences above noise.
- 2026-11-03: `--boss-escalate` (Sonnet only redoes what Haiku got wrong) deferred: asked by rule, the lead never escalated
  (12 runs); it needs the boss hook to route second attempts, and with ~30% of runs needing a redo, 8 runs per arm cannot
  show the expected ~+0.3. It improves boss mode only. Output of the default (tasks/, 11.7k tokens a run, ~41% of its
  cost): thinking 3.7k, file edits 2.1k+ (Python heredoc scripts: the explorer rule keeps the lead from Read, so Edit
  cannot be used), text 0.9k (final message 0.5k), the rest tokenization. No cheap lever: effort is already medium (low
  ruled out), the edit scripts are compact.
- 2026-11-03: test scope reaches the boss's workers (0.1.42): same judge (6.62 vs 6.92, noise), same cost, 29% faster
  (their whole-package test minutes 5.2 to 0.6). Per-role analysis (CHANGELOG): the expensive model is the cost and the
  cheap one the time in every config; worker reports are 2-5% of the lead's reads; the boss's lead loses its cache on
  waits over five minutes. Next candidates: Haiku first and Sonnet on the redo; a cheaper or conditional own-loop reviewer.
- 2026-11-03: test scope becomes the default (0.1.40): Claude runs only the tests covering its change, verify runs the
  changed packages' tests at the end. tasks/ six x two: judge 6.92 vs 6.58, cost -17%, time -58% (test commands 7.0 to
  1.7 minutes a run; the baseline ran a day earlier in the afternoon, so 40-58%). Go tasks only; the workers in boss mode
  do not get the rule yet.
- 2026-11-03: `--boss-pick` measured (tasks/ six x two, tasks-hard four x two): the lead varies effort as asked (testers
  low, explorers medium) but hands the main change to the Sonnet worker in 17 of 21 runs. Same score at +53% cost on
  ordinary tasks, +0.93 at 2.1x cost on hard ones (mostly one task): a trade, opt-in. If revisited: Sonnet only after a
  Haiku attempt failed its check, so ordinary tasks keep Haiku's price.
- 2026-11-02: boss mode's Haiku workers default to medium effort (they ran at the session's; at xhigh $1.02 a run
  against $0.45, no better judge). Boss with medium workers: same judge as no boss, 36% cheaper, 12% slower: a trade,
  so it stays opt-in for live sessions (`/boss`), on by default headless as before. Lean on Opus: same judge, dearer.
- 2026-11-02: `--batch-calls` (a prompt rule to batch independent tool calls) measured against the same day's default:
  no fewer calls (22.4 vs 21.9), 13% more tokens, judge level. Claude Code already batches what it can; with this and
  the tool and output levers ruled out, what is left is per-session (a fresh conversation for unrelated work, the /new
  hint) and per-role (boss mode's cheaper workers).
- 2026-11-02: two token levers measured on tasks/ (six tasks x two runs each, Opus 5.5 medium, repo profile without the
  boss, one Opus xhigh judge pool per task, two passes): `--repo-tools core` sent 13% fewer tokens but scored 0.50 lower
  and was not cheaper; `--rtk` cut command output by half yet sent 15% more tokens (more calls). Both inside noise on
  judge and cost: neither becomes the default, both stay opt-in.
- 2026-11-01: eight models x high/xhigh/max on two unused tasks, then Luna xhigh against DeepSeek on four more: DeepSeek
  xhigh stays the own loop's default (Luna's telegraf lead was one lucky run; DeepSeek xhigh 6.25 vs Luna 4.88 over four
  tasks). Sol max and Sonnet max reached the top judge on the hard task at higher time; Terra was strong but the dearest;
  Grok 4.7 cannot edit files in the own loop. SubRouter routes match Artificial Analysis on long-context reasoning (LCR)
  and roughly on HLE: no sign of degraded providers. Canaries now compare against AA's LCR and HLE.
- 2026-11-01: the own loop's cheap model stays DeepSeek 1017 high on LingAI (`--preset reviewed`): against GPT-6.1 Sol xhigh
  over four tasks it had the better judge (7.00 vs 6.50), hidden tests (24 vs 22 of 26) and time (8 vs 12 min) at a third
  of the cost. Routes are now re-checked weekly with a quality gate (long-context test against AA) and billed-cost check;
  DeepSeek on znu is the cheapest checked route (level judge, slower).
- 2026-10-31: own-loop time is 86% the model. DeepSeek 1017 at xhigh: same score as max (7.06 against 7.19), 41% faster,
  32% cheaper; candidate for `--preset reviewed` (waits on the user: their rule was max for cheap models). jevgrep: 10%
  faster at the same score, not adopted. Benchmark history: daily public snapshots, scoreboard.py, and a canary (130 fixed
  items) per model and route; first: DeepSeek 1017 on LingAI, AIME 2025 28/30 (2 cut by HTTP 524, fixed), MMLU-Pro 91/100.
- 2026-10-30: decision models in the own loop, replayed offline: run state does not predict failure (113 of 114
  failures are silent), and Jev cannot tell from the task and the diff which diffs miss something (no better than random,
  213 diffs, $0.06). Review-or-not stays a rule (`--preset reviewed` reviews every diff). The Jev router stays parked for
  routing tasks by their text, after real-use labels.
- 2026-10-30: the "thorough" workflow prompt (omp's ideas in tazuna's words) moved nothing on DeepSeek (7.06 against
  7.62, tests 4.44 against 4.62): dropped, not kept as a preset. Prompt wording is exhausted as a lever for the tests gap;
  where tazuna stands: ~93% of omp's score at ~1/3 its cost and faster, ahead of OpenCode, Command Code, pi and Claude Code.
  Next candidates are structural (what omp's runs do: more reading, more test runs, LSP diagnostics), each judged by score,
  cost and time per run together.
- 2026-10-30: omp's lead confirmed only in part: two runs a task, 7.81 against 7.25 (inside noise), but tests 4.81
  against 3.94 again. Next lever: a workflow prompt for tests and completeness ("thorough"), judged by score, cost and
  time per run together; kept as a preset if it buys score with cost, made the default if it dominates.
- 2026-10-30: on DeepSeek 1017, oh-my-pi (omp) beat tazuna by 1.1 (8.62 against 7.50, one run a task, tests 5.00 against
  4.38), at ~4x tazuna's cost; Command Code 6.88, OpenCode 5.12. First rival that clearly leads tazuna on its main model:
  confirm with more runs, then study what omp does differently. A prompt lever and a tests nudge, tried the same day for
  tazuna's tests gap, moved nothing.
- 2026-10-30: tazuna trails other harnesses by 0.32 on the same model (106 pairs), mostly in tests (−0.26, ~18% fewer
  test lines) and code quality; it leads on scope and on cost. A prompt lever aimed at both (focused change, remove what
  a change leaves unused, edge-case tests, API rule limited to code outside the repo) moved nothing (6.50 against 6.57,
  +18% time): dropped. The gap is inside run-to-run spread; closing it needs a better lever than wording.
- 2026-10-30: Kimi K3.1 Pro: tazuna level with Moonshot's Kimi Code on quality (6.88 against 7.25, same hidden tests) at
  2.5x less and 30% faster; step 3 of the main item done for Kimi. Grok 4.7 loops on today's SubRouter routes (reasons
  little inside agent runs; October's pool gone): not usable, but it found two tazuna bugs (0.1.5 read ranges, 0.1.6 repeat
  note). `--preset reviewed` as shipped: 6.88 against 6.50 without review, inside noise; the hand-over never fires when the
  repo's own tests pass.
- 2026-10-29: `--preset cascade` retired into `--preset reviewed` (0.1.4): same hand-over to Opus on failing tests,
  DeepSeek 1017 on SubRouter instead of V4.1 Flash on CheaperInference, the Opus medium reviewer, and the trusted-provider
  check instead of `--data any`. Review and hand-over together are not measured yet (the reviewer arms ran without it).
- 2026-10-29: hand-off to a fresh session instead of compaction: not built. 680 own-loop sessions: peak context per
  call p50 96K, p90 193K, 60 over 200K; two hit a context limit, MiMo (dropped) and MiniMax before 0.1.0 (its resent
  reasoning, 910K). DeepSeek tops out near 540K with 96% of its input cached, so a cut saves cents and risks the thread
  of the work. Revisit if a kept model hits its limit or long runs score worse than short ones on the same task.
- 2026-10-29: housekeeping. The caddy port-collision guess (2026-10-28) ruled out: caddy-retry's and caddy-unix's
  hidden tests passed four parallel runs each on the reference fix. roles.json gains the reviewer (Opus medium) and a
  second judge from another family (Sol xhigh). Own-loop results record the key variables they billed to.
- 2026-10-29: Sonnet 5.5 as reviewer (medium/xhigh/max) on the recall set: causes named 2, 4 and 2 of 7 against Opus
  medium's 5, at 1-2x the cost (xhigh, max) and, at max, half the reviews past 15 minutes. A higher public score did not
  carry over to diff review; Opus medium stays the reviewer.
- 2026-10-29: reviewer recall on 13 earlier DeepSeek diffs (graded by Sol against the reference): Opus medium named
  the cause of 6 of 7 test failures in 31-157 s, Astra xhigh 3 of 7 with 6 of 13 reviews past the 15-minute cap; both
  together 7 of 7. `--preset reviewed` (DeepSeek implements, Opus medium reviews) is the setup to use; Astra as a second
  reviewer only once it answers reliably (smaller input or a faster effort).
- 2026-10-29: reviews replayed on the same diffs (0.1.2 `--review-only`): Opus 5.5 max flags nothing on good diffs
  and finds the real gap; Astra xhigh finds the gap too, answers inside 15 minutes, costs ~$0.014 a review, but also
  flags points beyond the task. Holding findings to the task (prompt lever) dropped: no change on Opus, mixed on Astra.
  Reviewer default when one is wanted: Opus through the subscription at medium (the default effort: same findings as
  max on these diffs at ~1/15 the cost, under a minute); Astra xhigh as the paid fallback.
- 2026-10-29: a max-effort reviewer behind DeepSeek (0.1.1): GPT-6 Astra 7.17 and Opus 5.5 7.67 against 7.00 alone
  (3 tasks, one run each; hidden 10/10 against 9/10). Both catch the implementer's blind spot (terraform +3.5/+4.5) and
  both cost quality when their finding goes beyond the task (−2.0 each once). Astra costs ~$0.016 a review against
  ~$2.30 for Opus but stalls at max (one of five reviews under 15 minutes). Stays opt-in; next Astra at xhigh and
  findings held to the task's acceptance criteria.
- 2026-10-29: Codex's own usage report covers about a fifteenth of its real traffic (one key per harness, account
  log): on GPT-6 Astra it costs ~4x tazuna and ~5x pi per run; its judge lead (7.50 against 6.75) stands. Harness costs
  come from the account log from now on, one key per harness. SubRouter balance ran out ($0.03).
- 2026-10-29: 0.1.0 stops sending the model's reasoning back by default (MiniMax thought ever longer on its own
  thinking: 70 to 9 min on caddy-quoted; DeepSeek unchanged). Step 3 so far: tazuna ahead of or level with each model's
  own harness on quality everywhere, faster and cheaper on DeepSeek, Sol and GLM; on GPT-6 Astra Codex is leaner (half
  the input, faster). MiniMax dropped as a target (cost and time for its result). Next lever: tazuna's input per run on
  the Responses route (whole-file reads, its test check's extra rounds) against Codex.
- 2026-10-28: 0.0.99 narrows the exported-API rule to breaking changes: Sol read the old line as "add nothing exported"
  and lost caddy-retry by it (judge 5.30 to 7.75 with the new line, level with pi); DeepSeek unchanged on prometheus and
  the panel. Lesson: a prompt rule tuned on one model is checked on the next one before it is trusted.
- 2026-10-28: step 3, GPT leg (gpt-6.1-sol at max on SubRouter's shared OpenAI Pro pools): tazuna level with stock Codex
  on quality (judge 5.69 vs 6.31, inside the line; hidden 9/13 vs 10/13) at ~0.7x the cost; pi leads (6.62, 10/13, half
  Codex's cost). tazuna's gap is caddy-retry, where its Sol runs duplicated code and edited existing tests while on
  DeepSeek it was best there: the next lever is what in tazuna's prompt or loop pushes Sol that way.
- 2026-10-28: pi joins the DeepSeek panel (stock, pi's own DeepSeek settings, retry patience raised): quality level across
  all three (hidden 79% tazuna, 77% Claude Code, 73% pi; judge 6.83/6.75/6.25, inside the 0.75 line); tazuna 9 min and
  ~$0.008 a run against 28 min/$0.018 (Claude Code) and 20 min/$0.023 (pi). Step 1 for DeepSeek: tazuna matches both on
  quality at a third to a half of the time and cost; it does not beat them on quality. Next: a second model (GLM or
  MiniMax), where the gap to find is quality, not cost.
- 2026-10-28: main item step 1 met for DeepSeek: tazuna 0.0.97 beats stock Claude Code with the same model on the same
  provider (4 tasks, 2 runs a side: judge 7.44 vs 6.62, hidden 19/22 vs 17/22, 2.8x cheaper, 2.3x faster). The key was
  0.0.97: providers drop the model's earlier reasoning, so tazuna keeps the end of long reasoning steps as a note. Next:
  pi in the same panel, then GLM or MiniMax. Corrected the same day: tazuna's caddy-tls pass (v504) had read Claude
  Code's work tree; without it hidden 18/21 vs 17/22, judge 7.14 vs 6.62 (under the 0.75 line), 3.1x cheaper and faster
  per run, Claude Code ahead on caddy-tls. Step 1 holds on hidden tests, cost and time, not on a judge margin. The bench
  jail now blocks other runs' trees, transcripts and any network but the proxy.

- 2026-10-27: SubRouter unstreamed (0.0.91) kept: LingAI caches the same either way and costs a little less a call;
  a6aa is cheap only at low load (87% cached with three runs, 22% and ~100 s calls with seven). LingAI stays the route
  key's provider; route switching now judges the cache from call 20 (0.0.92). Next: per-provider capacity in the cache
  probe (several conversations at once), then a cheap route for single sessions.

- 2026-10-26: 0.0.88 cleared by a replay of the design turn (both versions plan the gc signature change 7-9 times in
  10; a rule against it does not stop it, it only halves the riskier exported-interface change). 0.0.89 kept: a test
  check past its limit ends unverified instead of a false failure (both runs 3/3, no fix loop). Next: the API rule as
  its own A/B on all seven tasks.

- 2026-10-26: bash timeout up to 1800 s (0.0.88) kept: polling gone, prometheus -32% turns and -37% cost, judge within
  noise pooled (6.0 vs 6.35 over ten runs) but -2 on prometheus, where 3 of 4 runs changed a signature the tests call.
  Next: a measured prompt line against changing existing signatures; tazuna's verify treats its own timeout as no result.

- 2026-10-26: all of DeepSeek's reasoning goes back (0.0.85, again as 0.0.87): on seven tasks same judge as without it
  outside caddy-unix's coin flip, -31% turns, -55% cost; the window (0.0.86) was worse on both. Next: the background
  test poll loop (one turn per `sleep; tail`).

- 2026-10-24: best of two retested on one route (Luna on OpenAI Pro, both attempts scored): +0.16, not a default. OpenCode's
  free DeepSeek judged 9 or 1 in a fifth of tazuna's turns; next: compare its transcripts with tazuna's on caddy-unix and
  rclone for where tazuna's own loop spends its extra turns, then measure route switching (0.0.79).
- 2026-10-22: offline check of best-of-two with an Opus picker (30 picks from judged runs, ~4% of the 5-hour window):
  +0.17 on a pool judged 7-9, +0.77 on Luna's 2-8.5 pool; the run judged 2 never picked; one run with an extra guard won
  every pair. Next for the cheap route: build it with each attempt's test results shown to the picker, then measure.
- 2026-10-22: the cheap route's quality gate (Opus five-lens review, the cheap model fixes the findings) measured on
  rclone and both caddy tasks, 3 runs each: 6.6 against 8.2 for DeepSeek 1017 alone. Not adopted; a reviewer that asks
  a cheap model for fixes widens its patches. Next candidate for the floor: two cheap attempts, Opus picks one (no fix).
  2026-10-25: redone clean (leak guards, 0.0.83): 6.5 against 7.3 alone; the verdict stands.
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
- 2026-10-14: providers for the own loop: apinex bills every re-sent token in full (no cache discount) and allows 5
  requests per minute: rclone unfinished after 115 turns and $0.47 (CheaperInference: done in 39-89 turns for ~$0.02).
  Ruled out. BeatAPI caches, but only 39% of a run's input hit the cache and rclone came out 1/3 at $0.10 (5x
  CheaperInference): ruled out. OpenGateway (an "ultrafast" DeepSeek route) is unknown until funded.
- 2026-10-14: tazuna's default against the user's own Claude Code (Opus high, interactive, with the user's guidance) on 5
  of the user's company PRs: same base commit, the ticket text as it was before work started, no review step. A peer's
  round-1 Critical/Major findings on the user's PRs (19): tazuna's versions carry 9 of the 16 that apply (3 concern parts
  tazuna did not build), the user's all 19. Judge against the merged PRs (1 pass, full diffs): tazuna 6.6, user 5.6
  (better on 3 of 5; the 2 losses are where the merged PR kept a design the ticket did not ask for). No new type errors
  or failing tests in any tazuna run; $1.78 per ticket, 13-50 minutes. Defects left in both: retries and idempotency,
  other writers of the same data, erasure of new keys: what the five-lens review catches.
- 2026-10-14: own-loop speed by provider (DeepSeek V4.1 Flash max, no hand-over, 3 October tasks, both at once).
  Experiential Labs: rclone 593 s 3/3 $0.086, caddy-unix 2446 s 7/7 $0.204, caddy-retry stopped at the 90-minute bench
  limit. CheaperInference: caddy-retry 2665 s 2/2 $0.103; rclone and caddy-unix hit an empty balance (earlier runs:
  rclone 694-1575 s, caddy-unix 578-2644 s, ~$0.02 each). Experiential Labs is not reliably faster (one quick run, one
  timeout) at 4-10x the cost: CheaperInference stays the cascade's provider. Its balance needs a top-up first.
- 2026-10-16: Claude models on the own loop speak Anthropic's Messages API (thinking kept, effort set). Stripping older
  thinking to keep the context small was ruled out: Anthropic requires every thinking block of a tool-use turn, and an
  edit before a block invalidates it. Server-side context editing is the sanctioned trim (0.0.60), but clearing tool
  results made Haiku re-read and loop, so it was removed in 0.0.62.
- 2026-10-16: gateways pool upstream accounts (BeatAPI full cache misses seconds after a hit; UnoRouter names its
  account groups). Prompt caches and Haiku/Sonnet 5.5 thinking are bound to one account, so a gateway's cache rate and
  thinking continuity are the gateway's property: probe a few calls before a paid run. UnoRouter is used only through a
  group that caches, and only for open-source tasks. Its groups change price by the hour: `bench/uno_route.py` picks.
- 2026-10-16: UnoRouter caches only calls that are not streamed, so tazuna does not stream there (0.0.63).
- 2026-10-13: Gemini 4 Argon (free for a while on Experiential Labs, 832,500 input tokens per hour; paid $2/$10 per M).
  As own-loop implementer: unusable on the free tier (4 parallel tasks used the hourly allowance in 5 minutes; one task
  needs 1.6-10M tokens; free answers carry an appended ad). As judge, 63 October patches vs Opus: r 0.90, same order
  within a task 66% (Luna 64%), ~1 point more lenient. No better than Luna at 20x Luna's price: not adopted.
- 2026-10-13: reviewer recall test. Ground truth: a peer's round-1 Critical/Major findings on 5 of the user's company PRs
  (19 findings), each reviewer in a fresh repo copy at the reviewed commit, the ticket verbatim, read-only. Caught: the
  user's own reviewer prompt (precision-first, "few high-confidence findings") 4 on Sonnet high ($1.61 for 5 PRs) and 6 on
  Opus high ($5.78); a five-lens prompt (`bench/review-lens.txt`: each criterion with a counterexample, every other
  reader/writer of the touched data, partial failure and retries, lifecycle of new state, actors and variants; report
  everything with a scenario) 7 on Sonnet ($2.58) and 13 on Opus ($11.69). Held-out PRs (prompt not written from
  them, 8 findings): 1 / 3 / 3 / 5. It also caught several issues the peer only raised in later rounds. Next: the lens
  prompt as tazuna's `--review-model` prompt, measured on tickets and Go tasks (the earlier generic reviewer added scope,
  not quality).
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
