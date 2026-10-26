# Changelog

Every change that can affect the agent is measured before it stays. Each entry lists what moved and by how much. Versions
that change no measurable metric (CPU, memory, binary size, tokens, cost, quality) get no metric table.

**Tiers**
- **Tier 0** (every change, free): tests, `bench/tier0.sh` (binary size, replay render time, input tokens per turn, and since
  0.0.15 startup time, UI CPU and memory while animating, tool server memory).
- **Tier 1** (changes to prompt, tools, routing or stop logic): 6 real-work Go tasks × 2 runs vs a same-day baseline. Judge
  score, hidden tests, cost, turns, time, cache hit rate.
- **Tier 2** (releases): Tier 1 plus the cross-harness table in the README.

**Noise thresholds**: judge ±0.75, cost ±15% (mean), turns ±20% at 12 runs per arm. Smaller differences are reported as
"within noise". (The judge threshold was ±0.5 until 0.0.3, whose two arms behaved identically and still scored 0.67 apart.)

Also recorded per measurement: the `claude` CLI version. Claude Code updates itself, and its built-in prompt changes token
counts (2.1.289 → 2.1.291 added ~104 input tokens per turn).

## 0.0.88 (2026-10-26): bash takes a timeout up to 1800 s; polling gone, prometheus -32% turns, judge mixed (open)

At a fixed 120 s the model ran prometheus's slow tests in the background and polled them (`sleep 115; tail`), one turn
per poll with the whole history sent again: 33-47 polls and 4-5 timeouts a run in round7. `bash` now takes `timeout`
(seconds, default 120, up to 1800), its description says to give one for long builds and test runs, and a timeout says
to give a larger one.

`round8.sh` (v381-v390): prometheus 0.0.87 (tz-v85's binary, same code) against 0.0.88 twice each, and 0.0.88 once on
the other six tasks, judged in one pool with round7's 0.0.85 runs (`judge.oct.round8.jsonl`). `round9.sh` (v392-v395):
prometheus twice each again, after the bench fix below (`judge.oct.round9.jsonl`, all eight prometheus runs in one
pool). deepseek-v4.1-flash-1017 at max on the vetted route key; leak scans clean. Cost at the fixed round price.

| | 0.0.87 | 0.0.88 |
|---|---|---|
| prometheus, 4 runs each | judge 6.0 (6, 6.5, 3.5, 8), hidden 3/4 runs, 937 turns, $0.608, polls 10-25 a run | judge 4.0 (3, 2.5, 3.5, 7), hidden 1/4, 634 turns (-32%), $0.382 (-37%), no polls |
| six other tasks, 1 run each | (0.0.85, round7) judge 6.58, hidden 11/19 | judge 7.33 (+0.75), hidden 19/19 |

- Every prometheus failure is the same mistake: a changed `stripeSeries.gc` signature that the hidden tests (and the
  base tests) call. 0.0.88 made it 3 times in 4, 0.0.87 once, Opus v34 once in 2: p = 0.24 for 3/4 against 1/4, and
  nothing in the change touches that design choice (it is made before any long test run). The six other tasks gained
  +0.75 on runs from different days. Pooled over ten runs a side: 6.0 against 6.35, within noise; turns and cost lower.
  Kept, with prometheus as the open question: the next step is a measured prompt line against changing the signatures
  of existing functions the code and tests call (5 of 12 prometheus runs, DeepSeek and Opus, failed on it).
- Three round9 runs (v392-v394) hit the 3 h limit while tazuna's own check reran the whole tsdb package: under four
  runs at once it took over 600 s, and tazuna's verify counts its own 600 s kill as a failure, so the model hunted bugs
  that were not there (v393 and v394 twice). Scored by hand from their partial diffs; v393 had stashed its changes to
  test the base when stopped, its diff rebuilt by replaying its 38 edits (matches its last `git diff --stat`). Next for
  tazuna: a verify run killed at its limit reports "no result", not a failure.
- Bench fix: since the 2026-10-24 module-cache isolation `GOFLAGS=-mod=mod` made go refuse to run in the four prometheus
  tasks (workspace mode) until the agent deleted go.work; runs that kept it scored 0 hidden tests (v383, corrected to
  3/3), runs that deleted it were marked down for "unrelated go.work changes". Workspace repos now run without it.
- LingAI's cache: about 15% of large calls missed and made ~71% of round9's cost; the same history came back counted
  as ~135K or ~180K tokens, two upstream backends with their own caches. Not ours; a longer per-provider cache check is
  the next routing step.
- UnoRouter's a7-supplier group for 1017 (v391, caddy-retry): ~8x cheaper a turn than SubRouter but ~5x slower; its
  price rose 1.6x mid-run and uno_run.sh stopped it (81 turns, $0.0041). A fallback, not a default.

## 0.0.87 (2026-10-26): 0.0.85's reasoning back again (all of it); 0.0.86's window measured worse; 0.0.85 -55% cost on 7 tasks

0.0.86 sent back only the last 8-24 turns' reasoning, to cut the input that 0.0.85's full reasoning seemed to add. One
round of all seven tasks-oct tasks, 0.0.84 (no reasoning back) against 0.0.85 (all) against 0.0.86 (window)
(`round7.sh`, `round7b.sh`, v360-v380), deepseek-v4.1-flash-1017 at max on the vetted route key without the watcher, so
each run kept its provider. Leak scan clean (only GitHub tries the proxy refused). Judge `judge.oct.round7.jsonl` and
`judge.oct.round7p.jsonl` (two passes; anchors Opus v34, Codex Luna). Cost at one fixed price ($0.02/M uncached input,
$0.0004/M cached, $0.08/M output) so the provider does not skew it; the round spent $0.96 on the account.

| task | 0.0.84: judge, hidden, turns | 0.0.85 | 0.0.86 |
|---|---|---|---|
| rclone | 8, 3/3, 247 | 8, 3/3, 110 | 6, 3/3, 123 |
| caddy-unix | 8.5, 7/7, 63 | 4.5, 0/7, 66 | 4.5, 0/7, 107 |
| caddy-retry | 7, 2/2, 66 | 7.5, 2/2, 63 | 8, 2/2, 74 |
| gitea | 7, 2/2, 65 | 8, 2/2, 38 | 7.5, 2/2, 37 |
| lazygit | 7.5, 3/3, 179 | 7, 3/3, 176 | 6.5, 3/3, 89 |
| telegraf | 5, 1/2, 150 | 4, 1/2, 75 | 6, 2/2, 121 |
| prometheus | stopped at 3 h, 352 turns | 5.5, 3/3, 200 | 4.5, 3/3, 235 |
| 6 tasks (not prometheus) | judge 7.17, 770 turns, output 1.54M, input 6.69M uncached + 65.8M cached (90.8%), $0.283 | 6.50, 528, 0.49M, 3.66M + 38.5M (91.3%), $0.128 | 6.42, 551, 0.69M, 4.70M + 35.4M (88.3%), $0.163 |

- 0.0.84's judge lead is caddy-unix alone: its 7/7 is one of two passes in the last 15 caddy-unix runs, and 0.0.84
  failed it 0/7 twice the day before (v350, v356). The failing runs rebuild the listener's internal types, which the
  hidden white-box tests compile against. On the other five tasks: 6.90, 6.90, 6.80.
- 0.0.85 against 0.0.84: judge -0.67 (within noise, all of it caddy-unix), turns -31%, output -68%, cost -55% (-54%
  without rclone's 247-turn 0.0.84 run). 0.0.84 did not finish prometheus in 3 h; 0.0.85 did in 200 turns.
- Correction to 0.0.85's "+25% cost": that A/B ran with `subrouter.py watch`, whose provider moves cold-started the
  caches. Without it 0.0.85 caches as much as 0.0.84 and its fewer turns make it much cheaper.
- 0.0.86's window: each step changed the history and lost cache (88.3%), and output rose 40% over 0.0.85; judge -0.08 on
  six tasks and -1 on prometheus, cost +27%. Reverted: 0.0.87 is 0.0.85's code.

Seen on prometheus: all three versions ran the slow tests in the background and polled them (`sleep 115; tail ...`), one
turn per poll with the whole history sent again. Next to measure.

## 0.0.85 (2026-10-25): DeepSeek's reasoning back in the history; judge +1.5, turns -30%, output -63%, cost ~+25%

Where tazuna's turns before green went on rclone (clean 0.0.83/0.0.84 runs against OpenCode's): without its earlier
reasoning DeepSeek re-planned on every turn, 15-34 turns a run of 8-26K output tokens followed by one small edit, and
rewrote its own code (one file 7 times, 20 "final rewrite" announcements in a run); OpenCode keeps the reasoning, plans
once and edits each place once (73-82 turns against 168-217).

- The own loop sends `reasoning_content` back with the history by default (Chat Completions); a provider that refuses
  the field gets the history without it for the rest of the route (the reverse fallback, for one that demands it, stays).
  `TAZUNA_DROP_REASONING` turns it off.
- A failed `edit` without `old` says so; no "nothing changed" note when one command writes, runs and deletes a probe test.

A/B against 0.0.84 (`reason1.sh`, v348-v359), deepseek-v4.1-flash-1017 at max on the vetted route key, 2 rounds side by
side. v353 (0.0.85, caddy-retry round 1) read the reference patch from a git worktree of this repo left in ~/bench-homes:
excluded with its pair, the worktree removed, the sandbox now refuses any `bench/realwork` path and any `solution.patch`.
Judge `judge.oct.reason1.jsonl` (two passes; anchors Opus v34 4.5-9, Codex Luna 7).

| task | 0.0.84: judge, hidden, turns, output | 0.0.85: judge, hidden, turns, output |
|---|---|---|
| rclone r1 | 5.5, 2/3, 207, 454K | 8.5, 3/3, 236, 211K |
| caddy-unix r1 | 5, 0/7, 118, 318K | 5, 0/7, 47, 93K |
| rclone r2 | 4, 2/3, 145, 506K | 8, 3/3, 121, 178K |
| caddy-unix r2 | 4, 0/7, 172, 314K | 5, 0/7, 39, 89K |
| caddy-retry r2 | 8, 2/2, 75, 130K | 7.5, 2/2, 57, 57K |
| 5 pairs | judge 5.3, turns 717, output 1.72M, input 15.6M uncached + 50.1M cached | judge 6.8, turns 500, output 0.63M, input 26.5M + 58.0M |

The judge gain (+1.5, over the ±0.75 threshold) is rclone's: both 0.0.84 runs there passed 2 of 3 hidden tests and both
0.0.85 runs all 3. Input grows with the reasoning in the history and is cached less (69% against 76%): at LingAI's
prices (cache read assumed ~10% of input) about +25% a run, fractions of a cent; worth it for the quality. Next to try:
keeping only the last turns' reasoning, for the same planning at less input.

## SubRouter routes through the account API: vetted provider keys, live drops, 2026-10-25 (bench only)

SubRouter picks the provider per call from a key's settings (subscribed providers, price, its own stability and
cache-rate weighting); a request cannot name one. Its account API (system access token, `~/.subrouter-account`) can:
`bench/subrouter.py` reads the per-call log (provider, cached tokens, cost: what the CSV exports showed), the providers of
a model with prices, and creates keys limited per model to providers.

- **Vetting** (`subrouter.py vet MODEL`): each provider up to 3x the cheapest input price, through a test key limited to it
  (four at once): an answer, a long reasoning call, identity, and the log for who really served it. Without the cache
  test: its 6 short calls were wrong both ways (FoxRouter 99% in the check, 40-73% in the run after it; LingAI 15% in
  the check, 78-88% in runs). deepseek-v4.1-flash-1017: 10 of 13 pass (QuqiAI and GETAPI answer the long question with
  a tool message, ModelShare over 15 minutes); ~$0.03 for all. A first night's vetting lost 9 providers to transport
  errors (connection resets, 400s that a morning rerun did not show): errors are now retried, not failed.
- **Route key** `tazuna-route-<model>`: limited to the providers that passed; `subrouter.py watch` reads the log during
  runs and drops a provider whose last 15 calls cached under 70%.

Route test 1 (`routes1.sh`, v341-v346, 0.0.84, 1017 at max): default key against the route key, side by side, $0.27.
SubRouter sent both keys to LingAI only (548 calls, 94-95% cached): same tasks results (rclone 3/3 at 217 vs 201
turns, caddy-unix 0/7 at 116 vs 71, caddy-retry 2/2 at 83 vs 89), nothing to drop. A forced switch (v347: a test key
with a6aa and LingAI, cache weighting off, so the cheaper a6aa should come first) sent all 47 calls to LingAI as well:
SubRouter's own ordering (stability first) already avoided a6aa, so the run was stopped. Today the key limits matter
for keeping out providers SubRouter does pick (PixStudio at 5x the price, a6aa at 13-22% cached, both on 2026-10-24);
the live drop has yet to fire on a real run.

## Opus gate on 0.0.83, clean: still no better than DeepSeek alone, 2026-10-25 (bench only)

The first gate test (2026-10-22, judged 6.6 against 8.2) rested partly on runs that saw the reference fix, so it was
redone behind the leak guards (`bench/realwork/gate2.sh`): tazuna 0.0.83, deepseek-v4.1-flash-1017 at max on SubRouter's
default key (FoxRouter; its cache fell from 98-100% to 40-73% from 23:10, so the round cost $0.62 instead of ~$0.35),
alone (`tazuna-ownsubm`) and with the Opus 5.5 high five-lens review on the subscription (`tazuna-ownsubgate`), side by
side, one round (v335-v340); leak scan clean. Judge `judge.oct.gate2.jsonl` (two passes, the earlier 0.0.83 runs
v330/v332/v334 in the same pools, anchors Opus v34 5-9 and Codex Luna 6.5); Claude usage ~2% of the 5-hour window.

| task | alone (v335/v337/v339) | gate (v336/v338/v340) | 0.0.83 alone, earlier (v330/v332/v334) |
|---|---|---|---|
| rclone | 8, 3/3, 177 turns | 7, 3/3, 175 | 8 |
| caddy-unix | 5.5, 0/7, 172 | 5, 0/7, 138 | 4.5 |
| caddy-retry | 8.5, 2/2, 65 | 7.5, 2/2, 42 | 9 |
| mean | 7.33 | 6.5 | 7.17 |

The gate scores lower on every task (-0.83, just over the ±0.75 threshold, one round) and costs Opus usage; the earlier
verdict stands. On caddy-unix, 0.0.83's after-green note came at turn 35-36 in both arms, when a repro test written
before the fix first passed: the runs then went on for 100+ turns, rightly, since the fix was not done. The note
should wait for a change in non-test code as well (next version).

## 0.0.80-0.0.83 (2026-10-24): finish after green, whole-file reads, edit errors with context; -47% turns, judge +2.2

From the clean OpenCode/tazuna comparison on rclone (where tazuna's extra turns went: ~53 a run after its tests first
passed, re-reading, rerunning unchanged tests, builds and repros; reads cut at the 8K bash cap; python-heredoc edits
failing up to 17 times a run). Own loop only.

- **0.0.80**: when a test run passes and the change includes test files, the result gets "check the diff once, then
  finish; rerun only if code changes"; a passing rerun with the tree unchanged gets "nothing changed since the last
  passing run". Appended to the tool output, DeepSeek ignored it: 79 (rclone) and 95 (caddy-unix) more turns after it
  (one PixStudio round, v305-v310, stopped at $0.90).
- **0.0.81**: the same note as its own user turn.
- **0.0.82**: `read` returns the whole file (2000 lines, was 250; cut at a whole line under the output cap, with the
  offset to go on); a failed `edit` shows where the old text's first line is and the lines there; tool descriptions
  steer reads to `read` and changes to `edit`; the own loop's prompt no longer names Grep/Glob (tools it lacks).
- **0.0.83**: a tool call without an id (or with a repeated one) gets its own; LingAI sent 16 empty ids in one run and
  the provider refused the history at call 161.

A/B, deepseek-v4.1-flash-1017 at max on SubRouter's default key (round 1 LingAI, round 2 FoxRouter, 96-99% cached),
both arms side by side, leak scan clean (every attempt at GitHub or the bench repos was refused). Round 1: 0.0.79
against 0.0.81 (v323-v328, $0.36); round 2: 0.0.79b (0.0.79 plus the id fix) against 0.0.83 (v329-v334, $0.34). Judge
`judge.oct.stopgreen.jsonl`, one pool per task, two passes, anchors tazuna on Opus (v34) 3.5-8.5 and Codex Luna 6.

| task | 0.0.79 / 0.0.79b: judge, hidden, turns | 0.0.81 / 0.0.83: judge, hidden, turns |
|---|---|---|
| rclone r1 | 7, 3/3, 216 | 8, 3/3, 123 |
| rclone r2 | 8, 3/3, 231 | 8.5, 3/3, 168 |
| caddy-unix r1 | 4.5, 0/7, 108 | 8.5, 7/7, 74 |
| caddy-unix r2 | 3, 0/7, 175 | 4, 0/7, 72 |
| caddy-retry r1 | died at call 161 (empty ids) | 8.5, 2/2, 74 |
| caddy-retry r2 | 4.5, 2/2, 252 | 9, 2/2, 79 |
| 5 pairs | judge 5.4, turns 982, input 113M, output 2.5M | judge 7.6, turns 516 (-47%), input 41M (-64%), output 1.5M (-40%) |

Over the noise thresholds on every measure, at 5 pairs (fewer than the 12 runs per arm a Tier 1 asks). The note works
as a user turn: 4 of 5 runs given it ended 2-19 turns later (rclone 28-43). Long runs cost quality as well as tokens:
the baseline's 252-turn caddy-retry passed its tests and judged 4.5 (churned code), the 79-turn one 9. The hints move
DeepSeek part of the way: rclone r2 31 `edit` calls and 11 script edits, against 0 and 47 in the baseline. caddy-unix's
7/7 in round 1 is one lucky run: both arms fail it otherwise. 0.0.83 is the own loop's new baseline.

## OpenCode's free DeepSeek, clean: judges like tazuna's DeepSeek in fewer turns, 2026-10-24 (bench only)

Rerun behind the leak guards below (proxy, sandbox; leak scan clean for all six; times on a slow LTE link, not
comparable). OpenCode CLI 2.0.29, `opencode/deepseek-v4.1-flash-free` at its default effort and at `#max`. Judge:
`judge.oct.opencode-clean.jsonl`, one pool per task, two passes (6 calls, ~4% of a 5-hour window); anchors tazuna on Opus
(v34) and Codex Luna.

| task | default (hidden, turns) | `#max` (hidden, turns) | tazuna DeepSeek, clean runs (judge, median turns) |
|---|---|---|---|
| rclone | 6 (3/3, 73) | 8 (3/3, 82) | 6.6 (111); 1017: 7.1 (192) |
| caddy-unix | 4.5 (0/7, 60) | 5 (0/7, 63) | 4.0 (80); 1017: 4.75 (150) |
| caddy-retry | 9 (2/2, 73) | 9 (2/2, 67) | 9 (57); 1017: 7 (74) |
| mean | 6.5 | 7.33 | |

Same quality as tazuna's clean DeepSeek runs (from other pools, so only roughly comparable), in fewer turns on rclone
(73-82 against 111-192) and caddy-unix (60-63 against 80-150); caddy-retry is close. caddy-unix fails for both harnesses
and for Luna (0/7 in 7 clean runs): a model limit, not a harness one. Every OpenCode run tried GitHub or local copies
for the fix and was refused. Next: where tazuna's extra turns go on rclone (clean transcripts only).

## Correction: 91 of 263 tasks-oct runs saw the reference fix; DeepSeek's lead was mostly that, 2026-10-24 (bench only)

Comparing OpenCode's transcripts with tazuna's turned up runs that copied the merged upstream fix instead of writing one.
The agents can read anything on this machine and fetch from the internet, and the fix of every tasks-oct task is public.
Three channels, found with the new `bench/realwork/leakscan.py` (scans every transcript under `~/bench-homes`; writes
`leaks.jsonl`, one line per task and harness label):

- **GitHub** (66 task/label pairs): `curl` of `raw.githubusercontent.com/<repo>/master/...`, the commit API by path, and
  `github.com/.../commit/<fix>.patch`, then a diff against the work tree. DeepSeek did this most, but also Luna (v286
  caddy-retry), MiMo, Haiku and Opus via the `repo` profile on rclone (v34).
- **Go module cache** (20): `~/go/pkg/mod` held caddy v2.11.7 (contains the caddy-unix fix) and a 2026-10-17 pseudo-version
  (contains both caddy fixes), pulled in by earlier runs and shared by every run. The anchor run tazuna-own-v38 DeepSeek
  (judged ≈9, used as an anchor in every pool since) diffed its caddy-unix work against v2.11.7's `listen_unix.go`.
  rclone v1.75.1/v1.75.2 in the cache do not contain the rclone fix and are not counted.
- **The bench repos** (2, plus OpenCode): `git log`/`git show <fix>` in `~/bench-repos/<repo>` (full history); cascade-v38
  rclone and cascade-v34 caddy-retry applied the fix commit verbatim. OpenCode v287/v288 ran with the bench directory as
  their working directory (the harness passed no `PWD`), read `solution.patch` and the hidden tests and edited
  `~/bench-repos` directly: all six OpenCode rows are invalid, and the OpenCode table in the next entry is withdrawn.

Judged score, leaked runs against clean runs of the same task and model (all tasks-oct judges):

| task | model | leaked | clean |
|---|---|---|---|
| caddy-unix | deepseek-v4.1-flash | 8.67 (9) | 4.0 (1) |
| caddy-unix | deepseek-v4.1-flash-1017 | 8.0 (4) | 4.75 (2) |
| rclone | deepseek-v4.1-flash | 8.38 (12) | 6.57 (14) |
| rclone | deepseek-v4.1-flash-1017 | 8.33 (6) | 7.12 (12) |
| caddy-retry | deepseek-v4.1-flash-1017 | 8.4 (5) | 7.0 (1) |
| rclone | gpt-6-luna | - | 5.93 (43) |
| caddy-unix | gpt-6-luna | 8.0 (1) | 5.27 (11) |

What changes: "DeepSeek V4.1 Flash on the own loop judges ≈9" and the routes-by-day DeepSeek results are inflated; clean
DeepSeek runs judge like Luna (rclone 6.6-7.1, caddy-unix ~4-5). The Luna route conclusions (Pro pool against Team pool,
max over xhigh) stand: those runs are nearly all clean. The DeepSeek anchor (v38) is out of the anchor set. An earlier
note here that tazuna's DeepSeek runs "almost always" fail caddy-unix was also wrong: v231/v237/v243 passed 7/7 (v237
leaked; v231 fetched upstream files too).

Fixed in the harness (run.py): every command gets `PWD` set to its own working directory; each task gets its own Go
module cache (`~/bench-homes/gomod/<task>`, filled by the task's setup) and runs with `GOPROXY=off`, so `go` cannot fetch a
newer release mid-run. Then closed, same day:

- **GitHub and Go proxies**: agents and harnesses reach the internet through a CONNECT proxy inside run.py that refuses
  GitHub (incl. raw, API, codeload), the Go module proxies, pkg.go.dev, sourcegraph, gitlab and codeberg; model APIs and
  logins tunnel through. A clean OpenCode run (v290 caddy-retry) tried `git ls-remote`, webfetch and `go list -m
  -versions` against them and got 403 each time.
- **Local copies**: OpenCode v289 then took the fix commit from `~/bench-repos/caddy` instead. Agents now run under
  `sandbox-exec` with reads denied for the task repos, `bench/`, `~/go/pkg/mod` and the transcript stores (Claude, Codex,
  OpenCode); OpenCode and the Claude CLI were checked working inside it. The module-cache copies that held fixes (caddy
  v2.11.7 and the 10-17 pseudo-version, hugo v0.167.0, prometheus v0.315.0) and agents' upstream files in `/tmp` are
  deleted.
- **Detection**: `leakscan.py` runs after every batch, reads OpenCode's sqlite sessions too, remembers fixed module
  copies in `leak-modcache.json` and reports proxy-refused fetches as `web-blocked` (not a leak); the judge and the
  summaries skip leaked rows.

Over all task sets: 114 task/label pairs leaked, 109 of them DeepSeek V4.1 Flash (any variant), 2 Luna, 2 Sol, 1 MiMo.
Rows leaked: tasks-oct 89 of 266 (85 tazuna), tasks 22 of 374 (all tazuna own-loop DeepSeek), tasks-hard 5 of 20. The
Claude CLI's 165 transcripts show none; pi, omp, omo, hermes, empryo and Codex keep no transcripts and cannot be checked
(their rows are Opus/Haiku/Luna).

## Best of two, retested: +0.16, not worth twice the calls; OpenCode's free DeepSeek scores 9 or 1, 2026-10-23/24 (bench only)

(Leak check, 2026-10-24: v286 caddy-retry fetched upstream files; the OpenCode table below is invalid, see above.)

**Best of two** (`tazuna-ownsubxbest2`, bench prototype in run.py): the same own-loop setup twice in two copies of the
work tree, Opus 5.5 high on the subscription (no tools, no reference) picks one with each attempt's own test result.
Retest at night (23:21-01:39 CEST, early morning in China): gpt-6-luna at max on SubRouter's 4-providers key (it served
OpenAI Pro all night, 15-22 s a call in every run), tazuna 0.0.78, both arms side by side, 2 rounds of rclone,
caddy-unix and caddy-retry (v275-v286); both attempts of each pair scored on their own. Judge: `judge.oct.best2retest.jsonl`,
one pool per task, two passes (6 calls); anchors DeepSeek 9, Codex Luna 8; tazuna on Opus (v34) 4.5-9.

| task | one run | best of two: result (picked attempt / other) |
|---|---|---|
| rclone | 8, 8 | 5.5 (5.5 / 7), 7 (7 / 6) |
| caddy-unix | 4, 6 | 5 (5 / 5), 8 (8, 7/7 hidden / 5) |
| caddy-retry | 7.5, 8 | 8 (8 / 7.5), 9 (9 / 7.5) |
| mean | 6.92 | 7.08 |

The pick took the better attempt in 4 of the 5 pairs that differed; the miss was rclone again, where an extra guard
("drops archive members beneath a symlink") won as in the offline check. Best of two helps when one attempt is clearly
bad (caddy-unix: the only tazuna run of the day passing 7/7) and costs twice the model calls and the slower attempt's time
(33-70 minutes against 15-48). +0.16 is within noise: not made a default. The day before, a first try on the 6 Luna key
was spoiled by its OpenAI Team pool (56-148 s a call, a best-of-two attempt at the 3-hour limit): judged 6.0 against 6.5.

**OpenCode with its free DeepSeek** (`opencode` harness: the official OpenCode CLI, `opencode run --standalone --auto
--format json -m opencode/deepseek-v4.1-flash-free[#max]`, its own clean HOME). OpenCode Zen's free models answer only
from within OpenCode (403 "FreeTierError" to any other client), so they are measured in their own harness; effort is a
model variant (this one has `max` and `high`). Same pools as above:

| task | default effort (v287) | `#max` (v288) |
|---|---|---|
| rclone | 9 (3/3) | 1 (1/3) |
| caddy-unix | 9 (7/7) | 9 (7/7) |
| caddy-retry | 1 (1/2) | 1 (1/2) |

Each run judged 9 or 1, in 20-29 turns and 2-7 minutes (tazuna with paid DeepSeek 1017: 60-340 turns, 12-67 minutes,
judged 6-9). caddy-unix: 9 twice where tazuna's runs scored 4-6 and failed the hidden tests (they rewrite the socket
bookkeeping the PR's tests read). A lead for tazuna's own loop: the same model family needs far fewer turns there.

## 0.0.79 (2026-10-23)

- **Routes: the own loop moves to another provider or key when one fails, slows down or stops caching** (design:
  docs/ROUTES.md). `TAZUNA_ROUTES="url key-env; url key-env"` lists more routes for the run's model after `--provider`;
  their keys are read at start and leave the environment, and with `--data trusted` every route's host must be trusted.
  A route is left (`route_switch` in the run log, with the reason) on a call that fails for good (a refusal, or 3 failed
  tries while another route waits: no ~15 minutes of backing off), on 3 empty replies, or, from its own last 10 calls,
  under 70% cached or slow (median over 60 s, or over 40 s and 3x its first 10). On a switch Claude thinking and Responses
  reasoning (bound to the old upstream) leave the history and the per-route flags start over; the new route's cache
  starts cold. No probe calls: the run's own calls are the check, so a run whose first route works costs nothing extra.
  Why: on SubRouter one key put a Luna run on a pool at 56-148 s a call and another on one at 8-16 s for its whole length
  (2026-10-23), a key's routing refused the cache key for an hour, and a key without a caching provider cached 0%.
  Tests: route_left_when_it_fails_slows_or_stops_caching (each rule; a refusal finishing the turn on the next route).
  Not measured on runs yet.

## 0.0.78 (2026-10-23)

- **The cache key goes when a provider refuses it.** GPT models on the Responses API carry `prompt_cache_key` (the
  session id) since 0.0.61: on BeatAPI it lifted the cached share from 18-47% to 96-97%. On 2026-10-23 SubRouter answered
  every gpt-6-luna request with it 503 "该分组下模型无可用渠道" (no channel for the model in this group, `model_not_found`)
  and logged nothing, while the same request without it went through, so a Luna run on that key would have stopped at
  its first call. On that refusal the call goes again without the key, which stays off for the run; `TAZUNA_NO_CACHE_KEY`
  turns it off from the start. The route check (`uno_route.py`) does the same and takes `ROUTE_NO_CACHE_KEY`. A first
  check on the key showed the refusal comes and goes: its first call was taken with the key, the next refused.

## 0.0.77 (2026-10-22)

- **DeepSeek's reasoning goes back when a provider needs it.** On Chat Completions the own loop kept only text and tool
  calls (some providers reject reasoning fields); a gate run on SubRouter's default key ended at call 29 with HTTP 400
  "This reasoning model requires previous reasoning_content in multi-turn requests", most likely after the gateway moved
  it to another provider. Each assistant turn now keeps its `reasoning_content` (from the stream too), still left out of
  requests; on that 400 the call is sent again with it, and for the rest of the run. Memory: the reasoning text of the
  run stays in the history (a few MB at most on a 300-call run).

## Two cheap attempts, Opus picks one: +0.2 to +0.8, the worst run never picked, 2026-10-22 (bench only)

Offline, from runs already judged: for each pair of runs of a task, Opus 5.5 high (`claude -p` on the subscription, no
tools, no reference) sees the task and both diffs in random order and picks the one it would merge (`bench/realwork/pick.py`,
`pick.oct.jsonl`); its pick is then compared with the judge's scores. Thirty picks used ~4% of the 5-hour window (~0.15%
a pick, a third of a five-lens review).

| pool | pairs | picked the better / worse / tie | picked mean vs pair mean | lowest picked |
|---|---|---|---|---|
| DeepSeek 1017 alone, rclone and both caddy tasks (judged 7-9) | 9 | 5 / 2 / 2 | 8.39 vs 8.22 | 8 (pair minimums 7-7.5) |
| Luna, rclone, three routes (judged 2-8.5) | 21 | 14 / 5 / 2 | 6.98 vs 6.21 | 5 |

The run judged 2 lost all six of its pairs; the best (8.5) won five. All five wrong picks in the Luna pool were v181
(judged 5, 2/3 hidden tests), which won every pair with an extra guard (archive members under a symlink dropped: "blocks
the hostile-archive escape") although the picker's prompt counts hardening beyond the task against a patch. In tazuna each
attempt's own verification result would be at hand for the picker, which may have caught it. As a route, best of two
costs one more cheap run (~$0.01-0.04) and ~0.15% of the window for the pick, and raises the floor where single runs
vary most. Next: build it (two attempts in separate work trees, the pick with each attempt's test results) and measure
it end to end.

## Five-lens gate on the own loop: worse, not better, 2026-10-22 (bench only)

The cheap route with a quality gate: DeepSeek V4.1 Flash 1017 at max on SubRouter's default key implements; Opus 5.5 high
reviews the diff on the subscription (`claude -p`, the five-lens prompt, `--review-model`); its Critical/Major findings go
back once to DeepSeek as a fix round; then tazuna's verification. No hand-over (`--no-escalate`) to isolate the review.
Harness `tazuna-ownsubgate` against the same setup without the review (`tazuna-ownsubm`), side by side per task, 3 runs each
on rclone, caddy-unix and caddy-retry (v229-v247, `gate1.sh`; one gate run lost to a provider error, replaced by v247).
Judge: one pool per task, two passes (`judge.oct.gate1.jsonl`, `judge.oct.gate1r.jsonl`, six calls; anchors DeepSeek 8,
Codex Luna 7), with tazuna on Opus 5.5 medium (v34) for reference.

| task | alone | with the gate | tazuna on Opus (v34) |
|---|---|---|---|
| rclone | 8, 7.5, 8 (7.8) | 7, 7.5, 5 (6.5) | 4.5, 5 |
| caddy-unix | 9, 9, 8 (8.7) | 6, 4.5, 5 (5.2) | 4, 5 |
| caddy-retry | 7, 8.5, 9 (8.2) | 6.5, 9, 9 (8.2) | 8.5, 8.5 |
| all | 8.2, hidden tests 9 of 9 runs | 6.6, 5 of 9 | |

Turns 121 against 177, 30 against 46 minutes a run; $0.73 real on SubRouter for all 19 runs (~$0.04 a run); each review
~0.5% of the 5-hour window, 61-137 s (about $0.45 a review at API prices). Reviews found something in 3 runs of 9 (v230:
every zip member scanned the whole directory; v232: a socket registry pointing at a closed socket; v242: three symlink
cases) and the fixed runs judged 7, 6 and 5: the fix widened the patch away from what the task asked (v232's rewrite also
broke the hidden tests' build). Six reviews found nothing; two of those runs (v238, v244) were weak already, from
DeepSeek's own redesign of the socket bookkeeping (the hidden tests read a `count` field it removed: they do not compile,
0/7). With 3 runs an arm part of the gap is chance, but the gate never came out ahead. DeepSeek 1017 alone beat tazuna on
Opus on these three tasks (8.2 against ~6) at about a cent each. Not adopted: a review with a fix round by the cheap model
does not raise its floor (as with Claude implementers, 2026-10-08 and 2026-10-13).

## 0.0.76 (2026-10-22)

- **No prompt on a command line.** The own loop's engine got its system prompt, the repo's CLAUDE.md and AGENTS.md
  included, as an argument; in a benchmark round of 18 parallel runs one agent cleaned up its test servers with
  `pkill -f "integration.test"`, which matched caddy's AGENTS.md text ("integration tests") in every engine's arguments
  and stopped ten runs at once (in real use the same command would stop tazuna itself; the prompt was also readable by
  anyone running `ps`). The engine now reads it from `TAZUNA_SYSTEM_PROMPT` and drops it from its environment, so the
  agent's commands do not inherit it; the claude path's lean profiles pass `--system-prompt-file` (a file named by the
  prompt's hash in a private `~/.cache/tazuna/prompts`, 0700: not the shared temp dir, where on Linux another user could
  plant it); the reviewer reads the task and diff on stdin. Checked: the same `pkill` against a run in a repo
  whose AGENTS.md says "integration tests" leaves the engine running and the task finishes; a Haiku review reads its
  input from stdin (verdict in 4.9 s).

## Routes by day: Luna on the OpenAI Pro pool judges 8 for $0.005; UnoRouter DeepSeek over SubRouter, 2026-10-22 (bench only)

rclone, China evening (12:24-15:20 CEST); max and xhigh side by side on one key and route, SubRouter's log split by the
effort on each call (every call went out at the effort asked). Real cost: SubRouter's usage log, UnoRouter's counter.
Judge calls: `judge.oct.day6.jsonl` (the Sol, LingAI-Luna and UnoRouter runs), `judge.oct.luna-routes.jsonl` (all Luna
runs of 2026-10-21/22 in one pool) and `judge.oct.ds-sites.jsonl` (DeepSeek on both sites, both days), two each; anchors
DeepSeek 8-9, Codex Luna 7-7.5. Runs judged twice moved 0.5-1.

| run | key, route | effort | judge | tests | turns | time | real cost |
|---|---|---|---|---|---|---|---|
| gpt-6-luna (v205 / v204) | Luna pools key, OpenAI Pro | max / xhigh | 8 / 6 | 3/3, 3/3 | 67 / 72 | 21 / 18 min | $0.0050 / $0.0029 |
| gpt-6-luna (v201 / v200) | 4-provider key, LingAI | max / xhigh | 8 / 8.5 | 3/3, 3/3 | 105 / 63 | 35 / 17 min | $0.034 / $0.015 |
| gpt-6.1-sol (v197 / v196) | default, 子木 ai | max / xhigh | 7.5 / 4.5 | 3/3, 2/3 | 75 / 111 | 21 / 22 min | $0.054 / $0.073 |
| DeepSeek 1017 (v199) | UnoRouter, zlkpro | max | 8-9 | 3/3 | 117 | 19 min | $0.009 |
| DeepSeek V4.1 Flash (v198) | UnoRouter, ih-cb | max | 6.5 | 3/3 | 336 | 52 min | $0.016 |
| DeepSeek 1017 (v203) | default, Kriora | max | 6 | 3/3 | 133 | 42 min | $0.015 |
| DeepSeek V4.1 Flash (v202) | default, a6aa | max | 5.5 | 3/3 | 213 | 56 min | $0.057 |

Luna by route, all seven runs in one pool: LingAI 8.5/8, OpenAI Pro pool 8/6, FoxRouter (2026-10-21, early morning in
China) 6/5/2. The route moves Luna more than the effort does; max is ahead on two routes of three (mean 7.3 against 5.4 at
xhigh). The Pro pool is the cheapest good route: 8 for half a cent, a seventh of LingAI. The default key sent Luna to a6aa,
which cached nothing (0% in three checks, 13:14-13:25); a key with FoxRouter and the pools sent the check to FoxRouter,
also at 0%; the pools alone cached 99%. Sol: max over xhigh this time (7.5 against 4.5), the other way on 2026-10-21 (7
against 8.5): one run each does not settle it. DeepSeek: UnoRouter 1017 judged 8-9 in 19 min for under a cent; SubRouter's
DeepSeek judged 5.5-6 in the China evening against 8-8.5 the night before, V4.1 Flash on a6aa at 3.6x 1017's cost on
Kriora. UnoRouter 1017 needed four tries to find a group: its cheapest group took over 15 minutes for the long call, the
next cached 9% or answered 502/503.

Shannon again: of its GPT groups only 0.12x serves Luna and Sol (0.02x "upstream access forbidden" as on 2026-10-20, 0.19x
and 0.37x without the models, 0.27x Sol only); Luna there failed the long call (over 15 minutes), Sol passed (73% cached,
616 s long call). Luna on 0.12x and Sol on 0.27x bill ~4,400 input tokens for a one-line question (a hidden system prompt);
Sol on 0.12x 44.

Bench tooling: the SubRouter run, its watcher and the spend guard live in the repo (`bench/realwork/sub_run.sh`,
`sub_guard.sh`; a restart of the Mac at 12:10 cleared their `/tmp` copies, the tazuna binary the runs used and three runs
in flight). The no-edit stop counts a change in the work tree as an edit (DeepSeek edits through python in bash: 321 calls
without an edit tool, 618 lines changed). `sub_summary.py` reads the effort from the run's start line and splits two runs
of one model side by side by it; `uno_run.sh` takes `HARNESS_BIN` and no longer prints an arithmetic error on a run without
a group switch.

Later the same day (`judge.oct.sol-sites.jsonl`, `judge.oct.ds-flash.jsonl`, two calls each; anchors DeepSeek 8-9, Codex
Luna 7): Sol at max on Shannon 0.12x (v206) 3/3 in 16.5 minutes, 86% cached, ~$0.52 of credit (~$0.07 at the 2026-10-20
rate), judge 7, beside SubRouter's Sol runs 7.5-8 (v185, v167, v197) and Shannon 0.27x on 2026-10-20 at 4. DeepSeek V4.1
Flash on the default key with a6aa disabled went to Kriora (v207: 339 calls, 93% cached, $0.052, 67 minutes) and judged
6.5, as v202 on a6aa: the same quality and cost, a third less per call but more calls; UnoRouter's V4.1 Flash 7.5-9 for
$0.002-0.016. Mortera: Opus 5.5 left every prompt uncached (four calls on a 26K prefix marked for caching, each billed in
full; its streamed replies report 0 input tokens), so a run would cost ~$1.80 there; Sol at max (v208) spent $0.20 in 28
minutes unfinished, then was stopped by our own watcher after a pause (below) and is left out.

The run watcher counted an hour's `kill -STOP` as an hour without a reply and stopped v208 as it resumed; a gap of over
five minutes in its own loop (a pause or a sleeping Mac) now restarts the count. `sub_one.sh` runs the route check, then
one watched run with its own spend guard; `sub_guard.sh` also reads Mortera's balance (`GUARD_SITE=mortera`, IDR).

## Effort and time of day on SubRouter: Luna max over xhigh, Sol xhigh over max; MiniMax free passes, 2026-10-21/22 (bench only)

rclone, one run each unless noted; real cost from SubRouter's usage log (`sub_summary.py`), UnoRouter at tazuna's estimate.
The night round ran staggered (two rounds of three plus MiniMax, $0.75 cap per round) at 21:50-01:33 CEST; all calls of a
run went to one route. Judge: `judge.oct.night2.jsonl`, two passes (two calls, 5h meter +1%); anchors DeepSeek 8, Codex
Luna 7.5.

| run | route | effort | judge | tests | turns | time | real cost |
|---|---|---|---|---|---|---|---|
| DeepSeek V4.1 Flash (v187) | UnoRouter | max | 9 | 3/3 | 228 | 29 min | ~$0.002 est |
| DeepSeek 1017 (v182 / v183) | SubRouter, Kriora | xhigh | 8.5 / 7 | 3/3, 3/3 | 287 / 199 | 66 / 52 min | $0.051 / $0.026 |
| DeepSeek 1017 (v188) | UnoRouter | max | 8 | 3/3 | 184 | 54 min | ~$0.018 est |
| gpt-6.1-sol (v185 / v184) | SubRouter, 数游AI | xhigh | 8 / 5 | 3/3, 2/3 | 53 / 59 | 14 / 15 min | $0.085 / $0.088 |
| gpt-6-luna (v189) | SubRouter, FoxRouter | max | 6 | 3/3 | 87 | 16 min | $0.016 |
| gpt-6-luna (v181 / v180) | SubRouter, FoxRouter | xhigh | 5.5 / 2 | 2/3, 1/3 | 73 / 28 | 14 / 8 min | $0.015 / $0.003 |
| MiniMax M3.1 Flash free (v186) | SubRouter, XCGX | xhigh | 6 | 3/3 | 372 | 3.5 h | $0 |

Effort. SubRouter's log shows the effort each call went out with: v189 forwarded max on all 87 calls, the xhigh runs xhigh.
Luna on SubRouter (tazuna runs since 2026-10-20) judges 6.3 at max (7, 6, 6; tests 3, 2, 3 of 3) and 4.8 at xhigh (8, 6,
5.5, 4, 3.5, 2; tests 3, 3, 2, 2, 2, 1): max ahead by 1.5, over the noise threshold but on few runs, other days and routes.
Max does not think longer per call on FoxRouter (499 output tokens per call against 649-1018 at xhigh; the new effort check
below found no more reasoning at max than at xhigh there), it works longer (87 calls against 28-73). gpt-6.1-sol goes the other way:
max judged 7 against xhigh's 8.5 in a blind pair with v167 (`judge.oct.sol-max.jsonl`), at 3x the cost ($0.082 against
$0.026). Sol at xhigh: 8, 8, 8, 5, 4.

Time of day. The same Sol model, effort and key judged 8 in the evening (v167) and 4 at Chinese daytime (v174, 139 min against
16; `judge.oct.drift.jsonl`, UnoRouter DeepSeek v173 6.5 in the same pair). Daytime Luna and DeepSeek 1017 (v175 6, v176 6.5,
v177 on the pool key 3.5) against evening (v129 8, v131 8; `judge.oct.daypair.jsonl`): 1.5-4 points lower, 3-8x slower.

Route price. gpt-6.1-sol on 数游AI costs $0.085 a run, 3x the same model on LingAI or ChorAPI ($0.019-0.026) and 4x tazuna's
estimate; the log has no provider prices for it. MiniMax M3.1 Flash free changed no file in its first ~90 calls (1.2 min and
~5K output tokens each) and still passed 3/3 at 6 for $0: free models now run without the no-edit stop (time limit only).

Bench tooling: `uno_route.py MODEL --effort` asks a long question twice at high, xhigh and max (streamed, in parallel) and
prints the highest level that reasons more than the one below (`EFFORT <level>`, None when a level is cut or unmeasured);
`sub_summary.py` merges new runs into `sub-summary.json` and flags runs that shared a window with another run of the same
model (v174, v175, v177: their spend is not theirs alone), which `site_summary.py` then leaves at tazuna's estimate;
`site_summary.py` groups by effort too; `run.py` has harnesses for a key chosen per run (`ownsubx`) and MiniMax free
(`ownsubmm`). The usage guard sends its own User-Agent (`tazuna-guard/1`) instead of Claude Code's: the meter answers the same.

## SubRouter, more models: Gemini, Qwen, Opus, Grok pool, Sol family; real cost from the usage log, 2026-10-20/21 (bench only)

rclone, all on SubRouter. Real cost is SubRouter's own usage log (the console's CSV export: 500,000 quota per USD, within
5% of the account counter), read per run by `bench/realwork/sub_summary.py`, which also writes `sub-summary.json` (tokens
billed, routes and their prices, run and generation tok/s, first-token time) for ranking in the app later. Judge calls:
`judge.oct.sub4.jsonl` and `judge.oct.sol2.jsonl`, one each; DeepSeek anchor 9 in both, Codex Luna 9 and 7.

| run | route | judge | tests | turns | time | real cost |
|---|---|---|---|---|---|---|
| gpt-6.1-sol (v167) | ChorAPI | 7 | 3/3 | 44 | 16 min | $0.047 together with v168 |
| gpt-6-sol (v165) | (between the two exports) | 7 | 3/3 | 113 | 55 min | under $0.236 (that with v166) |
| Grok 4.7, X Premium+ pool, high (v161) | pool | 6 | 3/3 | 40 | 21 min | $0.020 |
| gpt-6-sol-plus (v170) | OrangeAPI | 6 | 3/3 | 125 | 83 min | $0.179 |
| gpt-5.6-sol (v163) | ModelShare | 5 | 2/3 | 81 | 18 min | $0.040 |
| gpt-6.1-sol-plus (v169) | 星球中转站 | 4 | 3/3 | 103 | 64 min | $0.096 |
| gpt-6.1-sol-new (v168) | ChorAPI | 4 | 2/3 | 31 | 12 min | see v167 |
| Opus 5.5 (v156) | fate86, viapi | 3 | 2/3 | 129 | 20 min | $0.467 |
| Opus 4.8 (v145) | BangAi | 1 | 0/3 | 73 | 11 min | $0.072 |
| for reference: DeepSeek 1017 / Luna / Sol / Astra (v131/v129/v137/v140) | | 8 | 3/3 | 33-67 | 8-16 min | $0.009 / ~$0.005 / $0.019 / $0.120 |

Cut before finishing (most at a $0.45-0.80 cap): Opus 4.7 (老六 reports ~200K tokens per call for ~35K of context,
~$0.008-0.009 per call; $0.48 and $0.52), Opus 5 (曲奇ai $0.475 at 224 calls; 老六 and the Claude shared pool at about 4x that
per call, the pool at 8.5x its token prices), Opus 4.8 on Stable AI ($0.47 and $0.67), Qwen 3.8 Max ($1.14 in 203 calls, at a6aa's $0.279 per M
input). Qwen 3.8 Flash (twice, 242 and 503 calls) and Gemini 3.8 Flash (257 calls) never changed a file.

What decides a SubRouter run is the route, not the model name: the same Opus cost $0.001-0.016 per call by provider.
Routes that drop the effort (a6aa and xcgx on Opus: reasoning_effort None in the log), add hidden prompts (老六 ~20-40K,
one-api ~68K), stop caching (BangAi on Opus 5 and later on Opus 4.8: 0-1%; LingAI's cache stalls at a fixed prefix) or
bill the official price (Claude 共享池, ratio 1.0) make a run 4-10x dearer or not a real xhigh run. A smart-routing key
also falls back to shared subscription pools and official channels when its providers fail (曲奇ai answered 503 "no
available accounts" all evening): the key settings "Include shared subscriptions" (now off) and "Maximum official-channel
price ratio" (now 0.2) close that; gpt-5.6-sol-fast is only served above the 0.2 ratio and did not run. Reasoning effort:
Gemini 3.8 Flash thinks at none and high, not at xhigh or max, Qwen 3.8 and Grok 4.7 reason the same at any level.

On this task the cheap models stay on top: DeepSeek 1017, Luna and gpt-6.1-sol at 7-8 for $0.005-0.03; Grok on the pool
passes the hidden tests for $0.02 but judges 6; the Opus models judge 1-3 at 20-50x the cost. One run per model: ±1-2.

Bench tooling: the route check (`uno_route.py`) now streams Claude calls as tazuna does (a provider left the cache fields
out of unstreamed replies, which read as 13 input tokens for a 27K conversation), counts cache writes as input, re-asks
without tools when Claude answers with a tool call, reads SubRouter's untiered price formulas, and takes
`ROUTE_CACHE_BLIND` where the bill was measured to ignore the cache; `run.py` keeps the work of a run that ends in an
error as `<run>.diff.partial` (the judge reads `*.diff` only) and has a harness for a second SubRouter key (`ownsubgp`).

## Shannon by day: DeepSeek 0.14x and Sol 0.27x, 2026-10-20 (bench only)

rclone, one judge call (`judge.oct.shn2.jsonl`):

| run | judge | tests | turns | time | cost |
|---|---|---|---|---|---|
| Sol, SubRouter one key (v137) | 8 | 3/3 | 33 | 14 min | ~$0.016 |
| DeepSeek, Shannon 0.14x by day (v135) | 7 | 3/3 | 142 | 39 min | |
| DeepSeek, Shannon 0.14x at night (v127) | 6 | 3/3 | 75 | 11 min | ~$0.009 paid |
| Sol, Shannon 0.27x (v136) | 4 | 2/3 | 103 | 99 min | ~$1.2 credit, ~$0.17 paid |
| anchors: tazuna own-v38 / Codex Luna | 9 / 7 | | | | |

The same Sol scored 4 on Shannon and 8 on SubRouter, at ten times the cost and seven times the time; the judge called the
Shannon run over-engineered (an O(N*M) pass dropping zip members, per-call directory walks). Of Shannon's GPT groups only
0.27x served Sol (0.02x forbidden, 0.12x "overloaded", 0.19x not listed, 0.37x unavailable). Shannon's DeepSeek scores
6-7 by day or night.

## GPT-6 Astra on the one SubRouter key; more runs, 2026-10-20 (bench only)

Astra's cheapest providers on SubRouter bill about 1% of its list price (老六 ¥0.595 in / ¥2.975 out / ¥0.0595 cached
per M, with a 21% "reference evaluation" against 53-56% for the next two). Check: 98% cached, the long question answered
exactly (571) after 518 s. rclone, one judge call (`judge.oct.astra.jsonl`), all on the shared key:

| run | judge | tests | turns | time | billed |
|---|---|---|---|---|---|
| Astra (v140, xhigh) | 8 | 3/3 | 38 | 14 min | 0.85 yuan (~$0.125) with its check |
| Luna (v129) | 8 | 3/3 | 67 | 8 min | ~$0.005 |
| DeepSeek 1017 (v131) | 8 | 3/3 | 62 | 16 min | ~$0.0095 |
| Sol (v137) | 7 | 3/3 | 33 | 14 min | ~$0.016 |
| anchors: tazuna own-v38 / Codex Luna | 9 / 6 | | | | |

On this task Astra scored no higher than Luna or DeepSeek at 13-25x their cost; Sol and Astra need about half the turns.
Also today: Shannon DeepSeek 0.14x by day (v135) 3/3 in 39 minutes at 55-63% cached (95% at night); GLM 5.3 Flash and
Muse Spark 1.3 on Mortera again read for an hour without changing a file (Muse stopped after 30 minutes without a reply,
GLM recovered from a 28-minute call); UnoRouter 1017: of the two cheapest groups one cached 10%, the other returned an
empty long answer after 31 minutes (the check's 15-minute limit was swallowed as an OSError; fixed).

## Is SubRouter's gpt-6-luna the real model? Codex on SubRouter, 2026-10-20 (bench only)

The same harness on two providers: Codex CLI (`codexsub`, a model provider pointed at SubRouter's Responses API with the
shared key) against the Codex reference run (Codex CLI on the user's ChatGPT plan, official gpt-6-luna). rclone, one
judge call (`judge.oct.codexsub.jsonl`):

| run | judge | tests | commands | time | billed |
|---|---|---|---|---|---|
| Codex, official gpt-6-luna (reference) | 7 | | | | ChatGPT plan |
| Codex, SubRouter gpt-6-luna (v138) | 7 | 3/3 | 162 | 11 min | 0.030 yuan (~$0.0044) |
| tazuna, SubRouter gpt-6-luna (v129) | 8 | 3/3 | 67 | 8 min | ~$0.005 |
| tazuna, SubRouter gpt-6-luna, cheapest provider (v119) | 7 | 3/3 | 115 | 15 min | ~$0.004 |
| anchor: tazuna own-v38 (DeepSeek) | 9 | | | | |

Same harness, same score: nothing points to a weaker model behind SubRouter's gpt-6-luna. The judge's notes on both
Codex runs name the same habits (double lookups in squashfs, symlink-specific offset handling in zip). tazuna on the same
provider scored a point higher with fewer than half the commands.

## GPT-6.1 Sol on the one SubRouter key, 2026-10-20 (bench only)

Sol on the shared models key: the check cached 98% and answered the long question exactly (571, 279 s). It bills at about
3% of the official list ($2 in, $0.10 cached, $10 out per M): 20 calls cost 0.079 yuan against $0.39 at list. The first
run (v134) was restarted after 20 calls because tazuna counted the list price and was about to reach its cap; with the
real price (0.06, 0.003, 0.30 per M):

| run | judge | tests | turns | time | billed |
|---|---|---|---|---|---|
| Sol, one key (v137, xhigh) | 8 | 3/3 | **33** | 14 min | 0.109 yuan (~$0.016) |
| Luna, one key (v129) | 8 | 3/3 | 67 | 8 min | ~$0.005 |
| DeepSeek 1017, one key (v131) | 7 | 3/3 | 62 | 16 min | ~$0.0095 |
| anchors: tazuna own-v38 / Codex Luna | 9 / 6 | | | | |

(`judge.oct.sol.jsonl`; v131 judged 8 the night before.) Sol solved the task in half the turns of any other model, with
"very thorough" tests, for under 2 cents.

## One SubRouter key for DeepSeek and Luna, 2026-10-20 (bench only)

The models-only key with its providers restricted to the ones that cache for DeepSeek 1017 (kriora, lingai, a6aa and one
more) and routing by stability, then cache-adjusted price. The check cached 0% on the first two calls (other providers),
then 7,424, 6,400, 9,600 and 10,624 tokens: 80% over the test, long call not cut.

| run | judge | tests | turns | time | cached | billed |
|---|---|---|---|---|---|---|
| DeepSeek 1017, one key (v131, xhigh) | 8 | 3/3 | 62 | 16 min | 97-98% | 0.065 yuan (~$0.0095) with its check |
| DeepSeek 1017, three providers (v114) | 9 | 3/3 | 113 | 37 min | ~87% | 0.32 yuan |
| DeepSeek 1017, three providers (v118) | 8 | 3/3 | 69 | 19 min | ~92% | <= 0.18 yuan |
| DeepSeek 1017, lingai + a6aa (v117) | 8 | 3/3 | 92 | 23 min | 96% | |
| anchors: tazuna own-v38 / Codex Luna | 8 / 7 | | | | | |

(`judge.oct.onekey.jsonl`.) With Luna on the same key (v129: 8, ~$0.005), one key serves both models at 8 for about a
cent a run or less. A key per model and provider is not needed: the key lists the models, allows the providers that
work for each, and routes by stability, then cache-adjusted price.

## One SubRouter key for all models: routing order, 2026-10-19 (bench only)

The models-only key (DeepSeek 1017, Luna, Haiku 5.5; no provider pinned) with its routing reordered: stability, then
price adjusted for cache hits, then first-token speed, credit, evaluation (before: stability, evaluation, credit,
first-token speed, price).
- Luna (v129, xhigh): **judge 8** in one pass with v119 (7, cheapest provider pinned), v125 (4, same key in the old
  order) and Codex with Luna (7); 3/3, 67 turns, 7.6 minutes, 3.7M input, ~0.033 yuan (~$0.005) with two checks.
- DeepSeek 1017: the check cached 0% twice, so no run. In the old order the same key put it on an official-price
  provider (v124, ~5x the fixed key). A new provider, manxueai (满血AI), joined 1017's list during the evening; one
  call that set a cookie for api.a6api.com went to a6aa. For DeepSeek the key has to allow only providers that cache
  (kriora, lingai, a6aa: judged 9 three times at ~$0.03 a run).
- Correction: the DeepSeek checks that "cached 0%" counted replies without usage as uncached; `uno_route.py` now prints
  each call and fails on a reply without usage. Re-run: with price first, the first call timed out (524) twice; with
  stability first and no provider restriction, all six calls answered with real usage and none cached (8,450 tokens
  sent twice, 0 cached), at 0.01 yuan for the check. Unrestricted, the key routes DeepSeek 1017 to a provider that
  does not cache.
- So one key can serve every model: allowed providers = the good ones per model, routing = stability, then
  cache-adjusted price.

## Provider and key comparison, night of 2026-10-19 (bench only)

rclone, one judge call (`judge.oct.night.jsonl`):

| run | judge | tests | turns | time | cost |
|---|---|---|---|---|---|
| DeepSeek 1017, SubRouter three providers (v118) | 9 | 3/3 | 69 | 19 min | <= 0.18 yuan |
| Luna, SubRouter cheapest provider (v119) | **8** | 3/3 | 115 | 15 min | ~$0.004 at the key's prices |
| DeepSeek V4.1 Flash, Shannon 0.14x at night (v127, xhigh) | 7 | 3/3 | 75 | 11 min | ~$0.06 credit, ~$0.009 paid |
| Luna, SubRouter 4 providers (v120) | 6 | 2/3 | 152 | 95 min | |
| Luna, SubRouter models-only key, smart routing (v125, xhigh) | 5 | 2/3 | 93 | 11 min | |
| Haiku 5.5 "mh-kiro", SubRouter (v121, xhigh) | 2 | 2/3 | 127 | 11 min | <= 0.045 yuan |
| anchors: tazuna own-v38 / Codex Luna | 9 / 8 | | | | |

- Luna on SubRouter is best on its single cheapest provider ($0.003 in, $0.0003 cached, $0.015 out per M): judged at
  Codex's level for about $0.004 a run; four providers or smart routing scored 5-6.
- The Kiro Haiku reports fake usage (1 token in, 0 out), thinks ~300 characters and answered the counting question wrong
  (2,411 and 2,439 for 571); its run judged 2. SubRouter's default Haiku 5.5 fails its streams after ~50 s without data,
  since its providers send nothing while Haiku thinks ("upstream stream read failed ... 等待响应超时").
- GLM 5.3 (kriora) and GLM 5.3 Flash (foxrouter) on SubRouter cached 15% and 31% in the check and 23-25% in runs;
  stopped after ~20 minutes at 0.67 yuan for both.
- Shannon at night (04:00 in China): DeepSeek 0.14x cached 100% in the check and ~95% over the run, 11 minutes against
  80 by day; Luna still waited 65-190 s for the first token; the 0.02x and 0.05x groups still answered 502/429.
- DeepSeek 1017 on a models-only SubRouter key (smart routing, v124) was stopped after ~90 calls: over 56 calls it billed
  0.87 yuan (~$0.13) for 5.1M input (98% cached) and 138K output, about the official DeepSeek price and roughly 5x the
  fixed three-provider key (v118: <= 0.18 yuan for a whole run). Smart routing ranks stability and evaluation before
  price, so for DeepSeek the providers have to be fixed in the key.
- The cost cap needs the price in real dollars: SubRouter's listed rates are a generic price (its 1017 list tripled during
  the day while real charges did not move), so `TAZUNA_PRICE` takes the provider's own price from the console.

## SubRouter: DeepSeek 1017 three times at 9, Luna at 7, 2026-10-19 (bench only)

rclone, one judge call (`judge.oct.sub3.jsonl`):

| run | judge | tests | turns | time | cached |
|---|---|---|---|---|---|
| 1017, three providers (v114) | 9 | 3/3 | 113 | 37 min | ~87% |
| 1017, three providers, price in real dollars (v118) | 9 | 3/3 | 69 | 19 min | ~92% |
| 1017, lingai + a6aa (v117) | 9 | 3/3 | 92 | 23 min | 96% |
| Luna, cheapest provider (v119) | 7 | 3/3 | 115 | 15 min | 98% |
| Luna, Mortera xhigh (v107) | 5 | 3/3 | 99 | 18 min | ~90% |
| anchors: tazuna own-v38 / Codex Luna | 9 / 7 | | | | |

- DeepSeek V4.1 Flash 1017 on SubRouter: three runs, all 9 in one pass, 19-37 minutes, about $0.03-0.05 each (one shared
  usage counter across the keys). The most consistent setup measured so far.
- Luna on SubRouter's cheapest provider matched Codex with Luna (7) in 15 minutes; Mortera's Luna at xhigh scored 5.
  SubRouter's Luna upstream adds about 4,400 tokens of `instructions` of its own to every call (cached).
- Effort on SubRouter's DeepSeek 1017 (reasoning tokens, streamed counting question): low 29.1K and 7.9K, xhigh 28.5K and
  32K+ (capped), max 23.2K and 17.4K: xhigh thought most both times, max below it, as on Mortera.

## Repeat runs, 2026-10-19 evening (bench only)

rclone, one judge call (`judge.oct.eve.jsonl`):

| run | judge | tests | turns | time | cached |
|---|---|---|---|---|---|
| 1017, SubRouter three providers (v114, afternoon) | 9 | 3/3 | 113 | 37 min | ~87% |
| 1017, SubRouter lingai + a6aa (v117) | 8 | 3/3 | 92 | 23 min | 96% |
| 1017, UnoRouter (v111) | 9 | 3/3 | 59 | 10 min | 57-78% |
| Luna, Mortera xhigh (v107, afternoon) | 5 | 3/3 | 99 | 18 min | ~90% |
| Luna, Mortera xhigh (v116) | 4 | 2/3 | 170 | 59 min | 94% |
| anchors: tazuna own-v38 / Codex Luna | 8 / 7 | | | | |

- DeepSeek V4.1 Flash 1017 holds at 8-9 on SubRouter whichever providers serve it; fixed to lingai + a6aa it was the
  fastest SubRouter run and cached 96%. Both SubRouter keys share one usage counter: v115, v117 and a check together
  billed 0.53 yuan (~$0.078).
- A second three-provider run (v115) was stopped by its own $0.50 cap at ~$0.07 of real spend: tazuna was given
  SubRouter's listed rates as dollars, about 6x its real charge (v114: tazuna $0.27, billed 0.32 yuan). For SubRouter,
  `TAZUNA_PRICE` should be about 0.17x the list (0.035, 0.0007, 0.14 per M), so the cap means real money.
- Luna at xhigh on Mortera: 5 and 4 in this pass, 2/3 hidden tests once; it varies a lot from run to run and stays
  below DeepSeek.
- Shannon's GPT groups again (18:35): 0.12x and 0.27x Pro answered the reasoning question right (109) after 67-141 s to
  the first token and ~12 minutes in all; short calls took 27-360 s, half of them empty; 0.19x and 0.37x still do not
  serve gpt-6-luna.

## SubRouter with three providers, 2026-10-19 (bench only)

The same 1017 run with the SubRouter key routed over kriora, lingai and a6aa (the site's own ranking: stability, then
reference score, provider credit, first-token speed, then price adjusted for cache hits), instead of kriora alone.
Judged in one call with the earlier 1017 runs (`judge.oct.sub2.jsonl`):

| run | judge | tests | turns | time | cached | billed |
|---|---|---|---|---|---|---|
| SubRouter, three providers (v114) | **9** | 3/3 | 113 | 37 min | ~87% at the end | 0.32 yuan, ~$0.047 |
| SubRouter, kriora only (v112) | 7 | 3/3 | 112 | 61 min | ~62% | 0.47 yuan, ~$0.069 |
| UnoRouter, zlkpro then hly (v111) | 9 | 3/3 | 59 | 10 min | 57-78% | 2.65 cents |
| anchors: tazuna own-v38 / Codex Luna | 8 / 7 | | | | | |

The pre-run check cached 55% with three providers (83% with kriora alone), yet over the run the cache rate rose to 87%:
the router's "adjust for cache hit rate" seems to keep a conversation where it caches. Faster and a third cheaper than
kriora alone, with no visible kriora 524s slowing it.

## DeepSeek V4.1 Flash 1017 on UnoRouter and SubRouter; Shannon's Luna groups, 2026-10-19 (bench only)

rclone, one judge call (`judge.oct.1017.jsonl`):

| run | judge | tests | turns | time | cached | billed |
|---|---|---|---|---|---|---|
| 1017, UnoRouter zlkpro then hly (v111, 0.0.75) | **9** | 3/3 | 59 | 10 min | 57-78% | 2.65 cents |
| 1017, SubRouter kriora (v112) | 6 | 3/3 | 112 | 61 min | ~62% | 0.47 yuan, ~$0.069 |
| V4.1 Flash, Mortera, default effort (v98) | 8 | 3/3 | 74 | 9 min | 97% | ~$0.018 |
| anchors: tazuna own-v38 / Codex Luna | 8 / 7 | | | | | |

- The first group switch in a real run: v111 left zlkpro at call 21 ("57% cached over the last 10 calls") for hly and
  finished there. UnoRouter billed it at almost the uncached price (2.65 cents against tazuna's 0.1-cent estimate), six
  times the regular V4.1 Flash runs (~0.4 cents). An earlier 1017 run on one checked group ended at call 43 when that
  group vanished (0.92 cents); the check script crashed on a group answering `"choices": null` (fixed).
- SubRouter (new-api, shown in yuan at 6.8 per dollar): the check passed (83% cached, long call not cut); the run cached
  ~62%. Some calls failed with a 524 that SubRouter logs as "json: cannot unmarshal number 1792415975.375 into Go struct
  field ... of type int64" (kriora sends a fractional timestamp); unbilled, retried by tazuna. The judge marked it down
  for reading every zip link target when listing.
- Shannon's GPT groups for gpt-6-luna, speed test (time to the first token at max effort): 0.12x 14 s to 601 s (a 504
  from its gateway), 0.27x Pro 180-248 s, 0.19x and 0.37x Fast do not serve gpt-6-luna at the moment (404, though on
  their price lists). A Luna run on 0.12x reached the 3-hour limit after ~50 calls.

## 0.0.74-0.0.75 (2026-10-19)

- **0.0.74: on Shannon the first token is waited for 600 s** (180 s elsewhere). Its GPT groups queue calls before the
  first token (gpt-6-luna on 0.12x: median 69 s, up to 396 s in its own usage export; "Say OK." took 14 s, 179 s and
  then a 504 from its gateway at 601 s), and a call tazuna gave up on still ran and was billed: the first request of a
  run went three times.
- **0.0.75: the group list can be a file** (`TAZUNA_GROUPS_FILE`), read again at each switch. `uno_run.sh` keeps the
  file: when at most one listed group still serves the model, it checks the groups not tried yet in the background
  (`uno_route.py` with `UNO_SKIP`) and adds those that pass; the run stops only when no group is left and no check is
  running. A DeepSeek V4.1 Flash 1017 run (UnoRouter, one checked group, 0.92 cents) ended at call 43 when that group
  vanished from the price list.

## Effort on Mortera, and Shannon runs, 2026-10-19 (0.0.72-0.0.73)

0.0.73: a set `TAZUNA_PRICE` skips the provider's model list (Shannon lists a model only while an account serving it is
online; gpt-6-luna came and went within the hour). rclone, one judge call (`judge.oct.xhigh.jsonl`; this pass scored the
anchors 1-2 points lower than earlier ones, so compare within it):

| run | effort | judge | tests | turns | time | cost |
|---|---|---|---|---|---|---|
| DeepSeek, Mortera (v98) | default (max ignored) | 8 | 3/3 | 74 | 9 min | $0.018 |
| DeepSeek, Mortera (v106) | xhigh | 8 | 3/3 | 214 | 48 min | ~$0.10 |
| DeepSeek, Shannon 0.14x (v108) | max | 7 | 3/3 | 203 | 80 min | $1.03 credit, ~$0.14 paid |
| Luna, Mortera (v99) | default | 4 | 2/3 | 150 | 37 min | $0.033 |
| Luna, Mortera (v107) | xhigh | 6 | 3/3 | 99 | 18 min | $0.012 |
| anchors: tazuna own-v38 / Codex Luna | | 8 / 6 | | | | |

xhigh lifted Luna two points at a third of the cost; DeepSeek scored the same at 5.7x the cost, so DeepSeek on Mortera
is best at its default effort (quality per dollar). Shannon's DeepSeek cached 67-77% over the run.

## 0.0.72 (2026-10-19)

On Mortera Cloud:
- **`max` effort is sent as `xhigh`.** Its sources take OpenRouter's effort levels, which end at `xhigh`; Mortera forwards
  the value untouched and accepts any string (a made-up "banana" passed on all four models). DeepSeek V4.1 Flash on a
  counting question, streamed, output tokens: `low` 12.2K and 10.6K, `xhigh` 21.0K and 25.0K, `max` 16.2K and 13.8K. The
  Mortera runs before this (DeepSeek 9, Haiku 7, Luna 6) ran at the model's default effort, not at its highest.
- **A `cache_affinity_key` per conversation** (a hash of its first messages): Mortera keeps the conversation on the same
  upstream source for up to an hour, for warm caches and no switch to an account that rejects earlier thinking blocks.
Haiku's effort is likely lost either way: Mortera carries thinking settings only on a native Anthropic source, Haiku's
usage comes back in OpenAI's shape, and through Chat Completions any effort field is refused (400 or 422).

Shannon (Sub2API 0.2.17, docs on Feishu: base URL `https://api.shannonapi.xyz/v1`, tools set to Chat Completions): the
DeepSeek 0.14x group (cache read $0.0028/M) cached 0-100% over ten six-call tests, about 48% on average, with or without
`session_id`/`conversation_id` headers, `prompt_cache_key` or `user` (one combination hit 100% once, then 53% and 0%):
about $0.065 a run in real money, 3.6x Mortera. Luna on the ChatGPT 0.12x group answered once (Responses, streamed,
`high`, no output cap: 311 s, 568) and otherwise came back empty or `502`. The two 0.02x/0.05x groups never answered.
Parked.

## 0.0.71 (2026-10-19)

- **A 400 on a conversation with thinking blocks drops them and goes again** (Messages API). A thinking block's
  signature holds only on the upstream account that wrote it; Mortera Cloud's failover moves a conversation to another
  account and then answers `400 "rejected the request: it is invalid"` (reproduced with a forged signature; the same
  request without thinking blocks goes through). Haiku 5.5 died at call 39 on 0.0.70; on 0.0.71 the drop fired 2 and 3
  times in two runs and each went on.
- **Mortera's `422 model_unavailable` is tried again like a 5xx**, though it says not to retry: it passed within a minute
  for Muse Spark and ended a Haiku run at call 38.

## 0.0.70 (2026-10-19)

**Mortera Cloud** (`https://mortera.cloud/v1`, rupiah pay-as-you-go, cache at cache rates): no top-level
`cache_control` there, and GPT unstreamed there.
- Mortera sets its own cache breakpoints and writes none when a request brings a top-level one: Haiku 5.5 0% cached over
  25 calls on 0.0.69, 92-96% on 0.0.70.
- Its streamed Responses carry no usage: GPT-6 Luna looked 0% cached and the cost cap counted nothing; unstreamed, Luna
  ran at 96% cached.

## Three more gateways, 2026-10-18/19 (bench only)

`uno_route.py` now checks any gateway (`ROUTE_API`, `ROUTE_KEY_ENV`; Apizio's time-of-day price formula, Mortera's
rupiah list, Shannon at the official price times the key's group rate) and streams its test calls off UnoRouter.

| gateway | model | check | run (rclone) |
|---|---|---|---|
| Apizio (new-api, $5 free) | DeepSeek V4.1 Flash (`n/`) | no reply in 100 s, 10 tries over 2 hours | - |
| Apizio | GLM 5.3 | cache passed; long call cut twice (549 s) | - |
| Mortera | GLM 5.3 Flash | cache 99%, long call 60 s, answer 566 | stopped: one call open 27 min after 27 calls |
| Mortera | DeepSeek V4.1 Flash | cache 99%, long call not cut | **judge 9**, 3/3, 74 turns, 9 min, 97% cached, ~$0.018 |
| Mortera | GPT-6 Luna | cache 79%, long call 102 s | judge 6, 2/3, 150 turns, 37 min, 96% cached, ~$0.033 |
| Mortera | Haiku 5.5 | cache 94%, long call 5 s | 0.0.69 uncached; 0.0.70 died on thinking (39 calls); 0.0.71 on a 422 (38), on Cloudflare 524 after an hour-long upstream outage (44); then, alone, **judge 7**, 3/3, 33 turns, 19 min, 144K output (its stream no longer reported input, so cache and cost only from the balance) |
| Mortera | Muse Spark 1.3 (free) | 422 in the cache test twice; streamed with an effort, "model unavailable" | - |
| Shannon (sub2api, pooled accounts) | DeepSeek (0.05x group), GPT-6 Astra (0.02x; Luna not in the group) | "upstream temporarily unavailable" / "access forbidden", 11 tries over 2 hours | - |

Judged in one pool with the anchors (`judge.oct.mrt.jsonl`; the second pool, `judge.oct.mrt2.jsonl`, with Haiku gave every other run the same score): Mortera DeepSeek 9 ("matches the reference fix"), tazuna
own-v38 9, Codex Luna 8, UnoRouter DeepSeek v91 7, Mortera Luna 6 ("an unrequested zip pre-scan ... fails that hidden
test"). Mortera spent Rp 2,074 (about $0.13) on 5 checks and 8 runs or attempts; Apizio 3.4 cents of free credit; Shannon
nothing. Five runs at once on one Mortera key: DeepSeek ran at 79 tokens/s (83.7 alone on UnoRouter); GLM's long calls
fell from about 50 to 12-24 tokens/s, unclear whether from the load or the provider.

## `--self-review` on the own loop, DeepSeek on UnoRouter, 2026-10-18 (no code change)

Weak DeepSeek runs are scope failures in the judge's notes: "over-engineered", "an unrelated DetachAfterHijack method",
"rewrites the internal types ... the hidden tests do not compile". `--self-review` (0.0.14: finish rules in the prompt,
one turn reviewing the diff when the model first stops, "revert changes the task did not ask for") already reaches the
own loop; measured only on Opus before. Three runs (harness `tazuna-ownunorev`, groups ih-cb then b2capi, no switch),
judged in one pool per task with the uno-2 runs and the anchors (`judge.oct.rev.jsonl`):

| task | uno-2 (no review) | with review | review turn |
|---|---|---|---|
| caddy-retry | 9, 2/2, 57 turns, 0.37 cents | 9, 2/2, 46 turns, 0.28 cents | 9 calls, 3 small edits |
| rclone | 6, 3/3, 61 turns, ~0.43 cents | 8, 3/3, 80 turns, 0.96 cents | 11 calls |
| caddy-unix | 9, 7/7, 57 turns, ~0.43 cents | 4, 0/7 (suite does not compile), 80 turns, 0.70 cents | 17 calls, no edit |

Mean 7.0 vs 8.0 at one run per task (judge noise 1.5 per run), billed +58% (1.94 vs 1.23 cents; the three runs shared
one counter, split by input tokens). The caddy-unix failure came before the review: the same redesign as v76 (a new
shared socket type, the map type and the `count` field changed). The review read the diff, called it all needed and
changed nothing: "revert what the task did not ask for" catches extra changes, not a fix larger than it needs to be.
Decision: stays opt-in; no evidence it lifts DeepSeek, and it does not touch the failure that costs most.

## UnoRouter: what 29 hours of prices say, 2026-10-18 (bench only)

The public price list read every 10 minutes from Saturday 14:25 to Sunday 19:18 (170 readings,
`bench/realwork/uno-prices.jsonl`, logger `uno_price_log.sh`). Per model, per hour:

| model | groups seen / listed >95% | groups appear / vanish | cheapest group changes | cheapest input, min-median-max $/M |
|---|---|---|---|---|
| DeepSeek V4.1 Flash | 50 / 26 | 3.1 / 3.1 | 2.4 | 0.0057-0.0084-0.0130 |
| GPT-6.1 Sol | 29 / 16 | 0.6 / 0.6 | 2.2 | 0.266-0.280-0.300 |
| Grok 4.7 | 15 / 8 | 0.5 / 0.5 | 3.6 | 0.0086-0.100-0.100 |
| MiMo V2.6 Flash | 17 / 6 | 0.7 / 0.6 | 1.4 | 0.024-0.044-0.121 |
| GLM 5.3 | 24 / 5 | 1.5 / 1.5 | 0.5 | 0.011-0.011-0.098 |
| GPT-6 Luna | 23 / 10 | 0.4 / 0.5 | 0.07 | 0.0125-0.021-0.021 |
| Haiku 5.5 | 7 / 4 | 0.1 / 0.1 | 0.14 | 0.043-0.043-0.050 |

Picking the cheapest group and keeping it: for DeepSeek it was gone within 30 minutes in 39% of cases, within an hour in
49%, within 12 hours in 90%; Luna's was gone within 12 hours in 11%. A group that stays rarely gets dearer by much within
an hour (at most 2x for DeepSeek, 1.2x for Sol). The one large move was model-wide: DeepSeek's output multiplier stood at
120x input for about four hours (Saturday 15:38-19:29; 3.8-4.9x otherwise), with the input price unchanged.

Groups that have been listed for a while stay: of DeepSeek's cheapest groups 43% vanished within the hour, of the cheapest
listed for 1, 3 and 6 hours 20%, 10% and 6%, at 1.10x, 1.18x and 1.25x the price (MiMo: 13% to 4% at 1.12x; Sol 8% to 2%
at 1.03x). At 0.4 cents a run the 18% costs nothing that matters, while three vanishing groups end the run. Changes:
- `uno_route.py` tries groups listed in every reading of the last 3 hours first (when the logger runs), cheapest first
  within each half;
- `uno_run.sh` stops a run when a typical run at the current price (input, cache and output) costs over 1.5x the
  estimate at the start; it compared only the input price and would have missed the 120x output.

How often to read prices: before every run (free, one request), and every few minutes during it only as a stop guard. A
daily list is stale (90% of DeepSeek's cheapest picks gone after 12 hours); a read before every call buys nothing (a group
that stays moves little within a run); switching groups mid-run for a lower price saves fractions of a cent and costs a
cache miss, so it is not built.

## UnoRouter uno-2: a first default, 2026-10-18 (no code change)

DeepSeek V4.1 Flash on UnoRouter with group switching (0.0.69; three groups passed the check: sinnode, zlkpro, zangin):

| task | tests | judge | turns | time | cached | billed |
|---|---|---|---|---|---|---|
| caddy-retry | 2/2 | 9 | 57 | 9 min | 98% | 0.37 cents |
| caddy-unix | 7/7 | 9 | 57 | 10 min | 97% | ~0.43 cents |
| rclone | 3/3 | 7 | 61 | 9 min | 98% | ~0.43 cents |

`leaderboard.py uno-2`: judge 8.4 +- 1.1, worst 7, floor 7.5, no failure: **DeepSeek V4.1 Flash is the first model to meet
the default rule** (3+ runs, floor 7 or more, at most one failure in five). For comparison, DeepSeek on CheaperInference
scored 8-9 at 2.4-5.2 cents a run. No group switch was needed in these three runs, so switching is so far tested only in
unit tests. The two runs of one launch share the billing counter; each is about half of the 0.86 cents logged.

## 0.0.69 (2026-10-18)

**A 400 that UnoRouter passes on from an upstream ("Provider returned error ... Baidu", `bad_response_status_code`) moves
to the next group** like a dropout, instead of ending the task. The first two runs with group switching (DeepSeek, rclone
and caddy-retry over ih-cb and burngate) both ended on such a 400: caddy-retry after the task was solved (its summary was
written; the 400 came in tazuna's follow-up, so the solution was not saved), rclone after 185 calls. **Group switches are
written to the run log** (`system` / `group_switch`); they went only to stderr, which no one kept.

## 0.0.68 (2026-10-18)

**UnoRouter group switching in the own loop.** `TAZUNA_GROUPS=a,b,c` (cheapest first, the groups that passed
`bench/uno_route.py`, which now prints up to three): calls go to the current group with an `X-Group` header; when it drops
out (`get_channel_failed`, "No permission to access this group", "no available provider") the same call goes at once to
the next, and when the last 10 calls on it cached under 70% the next call does. No listed group left: the task ends with
the error, as before (no fall back to automatic routing, which picked dear groups). Why: in one day four groups vanished
between check and run or mid-run; two of them ended working DeepSeek runs (99 and 146 calls, 20 and 15 edits) that a
switch would have kept going. Prices are not re-read during a run yet (`uno_run.sh` watches them). Also 0.0.67: Experiential
Labs' hourly free limit (`insufficient_quota` with code `free_limit_reached`) is waited out instead of read as an empty
balance; a flaky test server that answered before reading the whole request is fixed.

## 0.0.66 (2026-10-18)

**A rate limit is waited out for up to 70 more minutes** (the last try repeated every 5 minutes) instead of ending the
task after ~15 minutes of tries: Experiential Labs' free DeepSeek V4.1 Flash 1017 allows 166,500 output tokens an hour,
reset on the hour, and two runs hit it at once; BeatAPI's 15 requests a minute stalled runs the same way. A run can be
interrupted while it waits. Bench: `uno_run.sh` logs why it did not start (a DeepSeek group that passed the check was gone
by the start, the second group to vanish between check and run), and the launcher checks again in that case.

## UnoRouter: Grok 4.7 and GLM 5.3 through the check, 2026-10-18 (bench only)

- **Grok 4.7:** a7-bin-704 passed (100% cached, long call 527 s, 573) and was closed to the key a minute later (403 "No
  permission to access this group"); a7-bin-709 passed too (100%, 434 s, 570), but its price rose from $0.024 to $0.10
  per M input during the run (4.1 cents for 19 calls instead of ~1): stopped. Every Grok group costs $0.10 or more now
  (~20 cents a run). Since then `uno_route.py --price` reads the price again at the start and `uno_run.sh` refuses or
  stops a run on a price rise, a vanished group or a failing cache.
- **GLM 5.3 Flash:** four groups cache (80-100%; ~2-7 cents a run), but none finished a long reasoning call: empty after
  467-486 s, an error after 180-189 s, or held open for over 40 minutes. Not usable for hard steps on UnoRouter today.
- **GLM 5.3:** cache reads cost the full input price there; the groups that cache come to $0.28-0.46 a run, the
  cheapest group cached 20-32%.
- The check takes at least 50% cached now (the price at the measured rate decides above that), asks again without tools
  when a model calls one (GLM ignores tool_choice none), and gives a group at most 15 minutes.

## UnoRouter: the full check before a run, 2026-10-17 (bench only)

`bench/uno_route.py MODEL` now runs, for the cheapest groups by the price of a whole run, until one passes: the live
price (input, cache, output) against OpenRouter's public list; caching over a conversation grown like an agent's (at
least 80%); a long reasoning call with tools that must come back; an English answer near the right number. It prints
`PIN <group>`. DeepSeek V4.1 Flash: a7-sinnode-5969 (cheapest) came back empty after 296 s, a7-ipeenk-5188 cached 34%,
a7-burngate-5609 passed (0.46 cents for the check). OpenRouter's reference for a typical run: $0.12; UnoRouter's groups
$0.007-0.011. The run pinned to a7-burngate-5609: rclone 3/3, 111 turns, 27 minutes, 12.9M input, 91% cached,
**1.9 cents billed, judge 8**, the same as CheaperInference (8 and 8 in the same pool, $0.024-0.052); the references
scored as before (Codex 7; UnoRouter's default key 9 and 6).

## UnoRouter: Haiku and Sol, pinned groups, 2026-10-17 (no code change)

Group check first (long reasoning with tools): GLM 5.3 reasoned 0-156 tokens on its cheap groups and si answered 429
after 180 s, so it was left out; all four cheapest Haiku groups returned thinking blocks; one of five Sol groups
(a7-gpt-4848) finished, four cut the call at 180 s. Sol's price there had doubled ($0.30 / $1.50 per M).

| rclone, 3 runs each | tests | judge | turns | time | cost |
|---|---|---|---|---|---|
| Haiku 5.5 high, **tazuna's Claude path** (subscription) | 3/3, 3/3, 3/3 | **7, 8, 8** | 36-54 | 8-10 min | 5-hour window 7% -> 9% for all three |
| Haiku 5.5 high, UnoRouter pinned to a7-hiyo-codex-5544 | 0/3, 0/3, 1/3 | **1, 2, 3** | 43-52 | 7 min | ~6 cents together |
| Sol max, UnoRouter pinned to a7-gpt-4848 | none finished | | 25-28 calls | | ~$1.40 together |

The UnoRouter "Haiku" wrote its summaries in Chinese to an English task and scored 1-3: the group most likely serves
another model or adds its own prompt. The Sol runs cached 54% and cost ~$0.45 each by call 28 (BeatAPI's whole run:
$0.39); they emptied the $5 balance, then the group went offline (`get_channel_failed`). References in the same pool
scored as before (Codex 7, DeepSeek CheaperInference 9, Haiku BeatAPI 8, Haiku Claude Code at max 4).

## 0.0.65 (2026-10-17)

**`TAZUNA_HEADER="Name: value"` adds one header to every model call**, e.g. `X-Group: <group>` to pin an UnoRouter group
for a run without a key per group.

Why Luna scored 2-4 through UnoRouter: a long reasoning call with tools on the Responses API never returns on some groups
(CX-Sale-duck 605 s, ih-cx 992 s, a7-ai-router 1048 s: no output), so the cheap groups cut the hard steps; Chat
Completions is no way out (with tools Luna does not reason there: 0 reasoning tokens, wrong answer). Groups that finished
a long call: a7-misy-3451, a7-jisol-5564, a7-misy-5255. DeepSeek reasons fully through UnoRouter (92,801 reasoning tokens
against 99,911 on CheaperInference for the same question).

Judged with the same reference runs as the pass below (they scored within a point of it):

| rclone etc. | tests | judge | turns | time | cost |
|---|---|---|---|---|---|
| Luna, UnoRouter pinned to a7-jisol-5564 | 3/3 | 6 | 210 | 159 min | ~$0.20 counted |
| Luna, BeatAPI / Codex (reference) | 3/3 | 7 / 7 | | 19 / 7.5 min | $0.11 / plan |
| DeepSeek, UnoRouter default key, rclone x2 | 3/3, 3/3 | 9, 7 | 134-149 | 34-37 min | |
| DeepSeek, UnoRouter default key, caddy-unix x2 | **0/7**, 7/7 | **4**, 9 | 86-143 | | |
| DeepSeek, UnoRouter default key, caddy-retry x2 | 2/2, 2/2 | 9, 9 | 65-126 | | |
| DeepSeek, CheaperInference (control) | 3/3, 7/7, 2/2 | 8, 9, 8 | 60-77 | 12-15 min | $0.04-0.05 |
| DeepSeek, UnoRouter (earlier run) / CheaperInference, 0.0.64, rclone | 3/3, 3/3 | 8, 8 | 81, 77 | 17, 15 min | ~$0.01, $0.05 |

Luna on a vetted UnoRouter group works but is slower, dearer and no better than BeatAPI. DeepSeek on UnoRouter's default
key matched CheaperInference on 5 of 6 runs; one broke the package's tests. With automatic routing its runs cached less
(57-83% against 96% earlier) and took about twice the turns, and UnoRouter billed ~$1.27 for these 7 runs against ~$0.55
estimated at the cheapest group's prices: routing also used dearer groups. CheaperInference's balance ran out (402).

## Judge pass, 2026-10-17 (no code change)

One judge pool per task (Opus xhigh with the reference fix, 1 pass, so a score can swing by about 2), 3 calls: 5-hour
window 4% -> 6%, week 0% -> 1%, no extra usage.

rclone, latest run per model and provider (tests, judge, billed):

| model | provider | tests | judge | cost per run |
|---|---|---|---|---|
| DeepSeek V4.1 Flash | CheaperInference (0.0.38, anchor) | 3/3 | **9** | $0.024 |
| DeepSeek V4.1 Flash HC | BeatAPI, 0.0.57 | 3/3 | 8 | $0.18 |
| DeepSeek V4.1 Flash | UnoRouter (ih-cb) | 3/3 | 5 | ~$0.03 |
| GPT-6.1 Sol | BeatAPI | 3/3 | 8 | $0.39 |
| GPT-6.1 Sol | UnoRouter | 3/3 | 8 | $0.11 |
| GPT-6 Luna | BeatAPI, 2 runs | 3/3, 3/3 | 7, 7 | $0.08-0.11 |
| GPT-6 Luna | Codex CLI (ChatGPT plan) | 3/3 | 7 (8 in two earlier pools) | plan |
| GPT-6 Luna | UnoRouter si, xhigh / max | 3/3, 2/3 | **4, 2** | ~$0.02 |
| MiMo V2.6 Flash | UnoRouter | 3/3 | 7 | ~$0.03 |
| Claude Haiku 5.5 | BeatAPI, old route, 2 runs | 3/3, 2/3 | 8, 4 | ~$0.05-0.09 |
| Claude Haiku 5.5 | UnoRouter | 2/3 | 4 | $0.15 |
| Claude Haiku 5.5 | Claude Code (subscription) | 2/3 | 3 | plan |

The same model scores lower through UnoRouter than elsewhere (Luna 2-4 vs 7, DeepSeek 5 vs 8-9; Sol is the exception at
8), and UnoRouter reports a third of the output tokens: its upstreams likely give the model less reasoning, so part of its
low price is a weaker model. The cheap UnoRouter Luna runs are not a win.

DeepSeek HC on BeatAPI by tazuna version (rclone, caddy-unix, caddy-retry): 0.0.53 (two rounds) 6/8, 4/9, 9/5 = 6.83;
0.0.55 8, 7, 1 (no change on caddy-retry) = 5.33; **0.0.57 8, 9, 8 = 8.33**, the best of the three, with fewer turns and
less input (see 0.0.61).

## 0.0.64 (2026-10-17)

**An empty 200 to an unstreamed call is sent again as a stream** (UnoRouter): some of its upstreams cut a slow
unstreamed call at ~90 s with an empty 200 (0 tokens in and out) instead of the 413 handled in 0.0.63. (Written by
another assistant while this one was at its usage limit; reviewed, tested.)

GPT-6 Luna max on UnoRouter with automatic group routing (cheapest group of the hour, not si), 2026-10-16 evening: 1/3,
44 turns, 69 minutes, 3.6M input, 89% cached, ~2 cents, and no edit at all (four empty replies in a row above 105K, before
this fallback existed); a second run with the fallback edited and tested, then was stopped by its watch at 49% cached
after 40 calls (4.6 cents). On the si group earlier that day: 3/3 and 2/3 in 7-9 minutes. The group decides: pin a caching group
(`uno_route.py`) rather than trust automatic routing.

## 0.0.63 (2026-10-16)

**UnoRouter calls are no longer streamed** (a 413 for a long call switches that call to a stream): UnoRouter caches only
calls that are not streamed. Same conversation on its si group: streamed 0 / 0 / 0 tokens cached on calls 2-4, not
streamed everything before the new text; the billing log never showed a cache read for a streamed call. This explains
every UnoRouter GPT run so far (0-35% cached). BeatAPI caches streamed calls (97%), so streaming stays the default
elsewhere; `TAZUNA_STREAM=0/1` decides for any provider. **A call rejected for `max` effort goes again at `xhigh`**
instead of ending the task (one of si's upstreams accepts none ... xhigh only; the 400 is free and takes 1 s).

GPT-6 Luna, rclone, UnoRouter's si group (today's cheapest, $0.021 / $0.0021 cached / $0.105 per M):

| | tests | turns | time | input | cached | billed |
|---|---|---|---|---|---|---|
| 0.0.61, streamed, max | stopped by the 400 after 10 calls | | | | 0% | |
| 0.0.61, `TAZUNA_STREAM=0`, xhigh | 3/3 | 122 | 404 s | 3.7M | 92% | 1.6 cents |
| 0.0.63 defaults, max | 2/3 | 130 | 515 s | 5.9M | 96% | 1.9 cents |
| BeatAPI, cache key, max (0.0.60, 2 runs) | 3/3, 3/3 | 70, 102 | 17-19 min | 4.5-7.1M | 97% | 8-11 cents |

The fastest and cheapest Luna runs so far (not judged yet). UnoRouter reports few output tokens (12-16K per run against
42-50K on BeatAPI), though the model reasons. Group prices change by the hour (si was the dearest group the night
before): **`bench/uno_route.py MODEL`** lists a model's groups by today's price (UnoRouter's public `/api/pricing`) and
tests caching on the cheapest few with a growing conversation, through the `X-Group` header (no key per group).

Other models through UnoRouter's cheapest group (probe, then one rclone run at max):
- **MiMo V2.6 Flash:** UnoRouter 3/3, 211 turns, 105 minutes, 20.5M input, 98.5% cached, ~2.8 cents billed. BeatAPI
  ($0.098 / $0.00196 / $0.196 per M, caches streamed calls): 147 calls in 99 minutes without changing a file (searching
  and printing code only), stopped at ~15 cents. (The probe's 0% for MiMo, DeepSeek and GLM was a bug in the probe:
  UnoRouter's Chat Completions usage has `input_tokens: 0` next to `prompt_tokens`, and the probe read the first.)
- **DeepSeek V4.1 Flash:** UnoRouter 3/3, 174 turns, 30.3 minutes (1821 s), 18.7M input, 97.4% cached, ~3.2 cents billed ($0.021 counted). Both `ih-cb` and the `a7-*` groups cached ~100% in the probe once `uno_route.py` read `prompt_tokens` when `input_tokens` is `0` (`a7-*` returns `"input_tokens": 0` alongside `prompt_tokens`). **GLM 5.3** (si, $0.011 per M): 37%.

## 0.0.62 (2026-10-16)

**The server-side clearing of old tool results (0.0.60) is removed.** Haiku 5.5 lost code it had read, read it again and
never got to the change: at high effort on BeatAPI 163 turns, 0 edits, $0.45, stopped (60% cached: each clearing
rewrites the cache); at max 146 turns without finishing (~$0.8). Keeping the last 5 tool results is too few, and a
higher number would save little. Claude's context is left to grow; effort is the lever for its size.

**GPT-6 Luna through UnoRouter's a7-5053 group** (the best of five group keys in a 4-call probe: every repeat cached):
the real run cached 58% early and nothing after 10:20, calls took a median 77 s, and at 116 minutes and 37 turns the
group went offline (`get_channel_failed`), ending the run; 9.1 cents billed. Its log shows why: a miss is billed as a
cache write at 1.25x input (~10x a hit), and the input count fell five times between consecutive calls (100,236 ->
93,400) although the history only grows: the group's upstreams add hidden prompts of different sizes, so a switch is a
miss. A short probe cannot show this. GPT runs stay on BeatAPI with the cache key (97% cached, 17-19 minutes, $0.08-0.11).

## 0.0.61 (2026-10-16)

Four own-loop changes, each measured on rclone (October set) with the build named; not judged yet (weekly limit).

**0.0.58: Claude models (`claude-`) use Anthropic's Messages API.** Thinking goes back with the history (signature and
all, as Anthropic requires during tool use), `thinking: adaptive` with `output_config.effort` (so max works), a top-level
`cache_control` breakpoint that moves with the conversation, and all tool results of a turn in one user message. Through
a gateway's Chat Completions the thinking was dropped and effort rejected: one of three Haiku runs ended in nothing but
empty replies. Haiku 5.5 max on BeatAPI, 3 runs: no empty reply and no nudge in 139 calls, but kept thinking grew the
context to 143-334K, past BeatAPI's 100K line where Haiku costs 5x; 16 calls missed the cache entirely (after calls held
by BeatAPI's limit of 15 requests per minute per key, and some 12-22 s after a hit). About $3 billed for the three;
stopped at 43-48 turns.

**0.0.59: GPT models send `prompt_cache_key`** (the session id) on the Responses API. Probe on BeatAPI, same 10K-token
prompt 4 times: without it 31% cached and ~3,800 more input tokens per call; with it 99.97% from the second call.

| rclone, own loop, BeatAPI | tests | turns | input | cached | billed |
|---|---|---|---|---|---|
| GPT-6.1 Sol max, 0.0.58 | stopped | 29 | ~1.7M | 18% | $1.35 |
| GPT-6.1 Sol max, 0.0.59 | 3/3 | 42 | 3.1M | 96.5% | $0.39 |
| GPT-6 Luna max, 0.0.55, 2 runs | 3/3, 2/3 | | | 47% | ~$0.19 each |
| GPT-6 Luna max, 0.0.60, 2 runs | 3/3, 3/3 | 70, 102 | 4.5M, 7.1M | 97% | $0.08, $0.11 counted |

**0.0.60: Claude models clear old tool results on the server** (context editing, `clear_tool_uses_20250919`): above 80K
input tokens the oldest go, the last 5 stay, at least 20K at a time. The context fell from 80K back to ~22K each time,
but BeatAPI served 3 of the first 62 calls uncleared (106-198K): its upstreams differ. Haiku max still thought a lot
(1.23M output tokens in 146 turns) and hit the 3-hour limit without finishing: ~$0.8 billed, no result. Max effort is
not worth it for Haiku on this route.

**0.0.61: the 180 s call limit is on the reply's start only;** a started stream may run 15 minutes. Cut at 180 s,
UnoRouter's slow Luna calls (100-500 s) were billed anyway, with the input counted twice, and then sent again. Luna max
on UnoRouter's default group with the fix, 2 runs: both hit the 3-hour limit without finishing (~100 turns in the first 2
hours, 0% cached, calls of 100-500 s), $1.00 billed. BeatAPI did the same task in 17-19 minutes.

Also measured:
- **DeepSeek V4.1 Flash HC, 0.0.53 vs 0.0.57** (BeatAPI, same time, 3 tasks): tests 12/12 both, turns 528 vs 332,
  input 48.4M vs 21.8M, $0.68 vs $0.30. With the round before (0.0.53 vs 0.0.55): tests 17/24 vs 23/24, input -32%, cost
  -42%. Times not comparable (rate limit).
- **GPT-6.1 Sol on UnoRouter** (0.0.58): 3/3, 70 turns, 4.3M input, 95% cached, $0.11 billed.
- **UnoRouter** routes each model through groups of upstream accounts, set per API key. Luna probe, same 9K prompt 4
  times per group key: a7-5053 and ih-cx hit the cache on every repeat (~$0.010 and ~$0.012 per M input effective);
  CX-Sale-duck (the default) 1 in 3 (~$0.017); si (list price) 2 in 3 (~$0.068); a7-hlgbao1908-3844 never, and adds
  ~2,500 prompt tokens per call. Its Luna reasons (77-322 output tokens for a two-character answer) but reports no
  reasoning tokens; its Haiku returns no thinking at all.
- **BeatAPI:** 15 requests per minute per key across all models; more than 2-3 runs at once only add waiting. It bills
  cache writes at the 1-hour rate and settles minutes late.

## 0.0.57 (2026-10-15)

**Claude models on a gateway's Chat Completions route:** no `reasoning_effort` for `claude-` models (BeatAPI rejects
Haiku 5.5 with one: "requires adaptive thinking"; the model then runs at the gateway's default effort). **Streamed tool
calls are keyed by their `index`:** BeatAPI counts indexes across content blocks (text 0, tools 1, 2, ...), which left an
empty call at index 0 and failed the next request ("custom tool name is invalid").

Haiku 5.5 on rclone (October set; hidden tests, not judged yet):

| | tests | turns | time | input (cached) | output | cost |
|---|---|---|---|---|---|---|
| Claude Code through tazuna (subscription, max) | 2/3 | 118 | 3697 s | 47.6M (98%) | 441K | plan ($5.88 reported by the CLI) |
| own loop, UnoRouter, max | 2/3 | 91 | 2023 s | 6.4M (76%) | 174K | $0.15 billed |
| own loop, BeatAPI, default effort, 3 runs | 3/3, 1/3, 2/3 | 45, 19, 48 | 873-1353 s | 0.6-2.7M (73-95%) | 50-97K | $0.03-0.06 counted, billed ~1.5x |

On average the own loop matches Claude Code with Haiku at about 1/40 of the cost, but BeatAPI's runs vary. The 1/3 run
changed nothing: after 17 turns every reply came back empty, through 3 retries and both nudges. Likely cause: the
OpenAI-format translation drops Claude's thinking blocks, so a turn that ends in thinking alone arrives empty, and asking
again repeats it. Next: the own loop speaks Anthropic's Messages API for Claude models (thinking kept, effort set, cache
control).

## 0.0.56 (2026-10-15)

**The own loop streams its model calls** (`stream: true`; `TAZUNA_STREAM=0` turns it off) and puts the pieces back into a
whole reply. The answer is the same; gateways cap replies that are not streamed (UnoRouter: complete within 90 s, which a
130K-token prompt cannot meet: HTTP 413, and the task ended). Checked live on BeatAPI, Experiential Labs and UnoRouter.
**An empty reply (no text, no call) is asked again** up to 3 times (5, 15, 30 s) before the nudge; an error inside a
stream is retried like a 5xx. **Usage in either API's field names** (UnoRouter answers the Responses API with Chat
Completions usage).

GPT-6 Luna max on rclone, one judge pool (Opus xhigh with reference, 1 pass):

| | tests | judge | cost per run |
|---|---|---|---|
| Codex CLI (ChatGPT plan) | 3/3 | **8** (8 in the earlier pool too) | plan |
| tazuna own loop, BeatAPI, 0.0.55, 2 runs | 3/3, 2/3 | 8, 5 | $0.19 |
| tazuna own loop, BeatAPI, 0.0.54 | 3/3 | 6 (8 in the earlier pool) | |
| tazuna own loop, UnoRouter, 0.0.55, 2 runs | 1/3 (three empty replies), 2/3 | 1, 4 | $0.05 |

Codex is consistently at 8; tazuna with Luna reaches 8 but varies (5-8), and the same patch scored 6 and 8 in two pools,
so one judge pass swings by about 2 points. UnoRouter's Luna route returned empty replies and failed with "All accounts
failed. Tried 3 accounts": it pools accounts, so its GPT models are of the same unclear origin as apinex's. Not used for
GPT or Claude models.

## 0.0.55 (2026-10-15)

**A headless own-loop task no longer ends the moment the model answers without a tool call while nothing has changed.**
It gets up to two nudges ("you stopped without changing any file; continue, or reply DONE and say why"); an empty reply
is nudged in chats too. Before, any reply without a tool call counted as done: two rclone runs (BeatAPI's regular
DeepSeek route, UnoRouter) stopped after a plan or an empty reply, changed nothing, and were reported as successes; the
repo's own tests could not catch it, since they still pass with no change. Nudges show in the transcript.

**Luna vs Codex, again** (rclone, one judge pool, Opus xhigh with reference): Codex 8; tazuna 0.0.54 8 (input 6.3M vs
4.7M); 0.0.53 8; 0.0.52 (BeatAPI) 5; 0.0.38 (CheaperInference) 7. Quality equal now; input 1.3x Codex's.

**UnoRouter** (named German operator, OpenAI-compatible, `https://api.unorouter.com/v1`), DeepSeek V4.1 Flash max,
unpinned (`owned_by: openrouter`), rclone: before the fix 1/3 (stopped on an empty reply after 35 turns, no change,
$0.037); with it 3/3, 83 turns, 1138 s, 4.3M input, $0.16. No cached tokens reported; input $0.01/M, output $1.20/M:
6-8x CheaperInference (~$0.02) and BeatAPI HC ($0.027) for this task. Not used.

## 0.0.54 (2026-10-15)

**`read` returns at most 250 lines unless the model gives a limit,** with a note on the rest ("lines 1-250 of 900; read
with offset=251 for more"). **On the own loop, a repeat of an unchanged read points back to the earlier result** instead of
sending it again (nothing leaves the conversation, unlike the elision tried in 0.0.38). **Background processes no longer
outlive a task:** what a command leaves running (`server &`) is recorded (pid and start time) and killed when the task
ends, or by the next engine if this one was killed; a background process holding the output pipe no longer blocks the
tool call (2 s, then a hint to redirect). The benchmark runner kills the whole process tree at its time limit. Found
because timed-out runs had left a Caddy server, two test backends and an engine running for a day and a half.

A/B on BeatAPI, old (0.0.53) vs new, same time, one run each:

| | old: tests | old: turns / input | new: tests | new: turns / input |
|---|---|---|---|---|
| GPT-6 Luna max, rclone | 3/3 | 127 / 10.1M | 3/3 | **100 / 6.3M (-37%)** |
| DeepSeek V4.1 Flash HC max, caddy-retry | 2/2 | 90 / 5.3M | 1/2 | 31 / 0.8M |
| DeepSeek HC, caddy-unix | 0/7 | 139 / 8.4M | **7/7** | 133 / 7.9M |
| DeepSeek HC, rclone | 3/3 | 195 / 19.0M | 3/3 | 240 / 25.0M |

Luna: a third less input at the same result (Codex used 4.7M on this task). DeepSeek: tests 11/12 vs 5/12, tokens about
equal; BeatAPI was slow and erratic that day (rclone took 195-240 turns, 58 the night before), so the DeepSeek pairs are
noisy. In the failed caddy-retry run every read had an explicit range: the window was not the cause. No process was
left behind after these runs.

## 0.0.53 (2026-10-15)

**GPT models use the Responses API on any provider** (by model id `gpt-`; BeatAPI lists every model as "openai").
**Benchmark kit: Codex CLI as a harness** (`codex@MODEL:EFFORT`, the user's own ChatGPT plan through the official
client, a clean `CODEX_HOME` with only the login, a minimal environment).

First "own loop vs the model maker's harness" data point, rclone (October set), one run each, one judge pool (Opus xhigh,
with reference):

| | tests | judge | time | turns | input | cache | cost |
|---|---|---|---|---|---|---|---|
| GPT-6 Luna max, **Codex CLI** | 3/3 | **8** | **453 s** | 106 actions | **4.7M** | 96% | plan |
| GPT-6 Luna max, tazuna own loop, CheaperInference (0.0.38) | 3/3 | 7 | 1451 s | 119 | 14.6M | 99% | $0.133 |
| GPT-6 Luna max, tazuna own loop, BeatAPI | 2/3 | 5 | 728 s | 72 | 4.4M | 47% | $0.111 |
| DeepSeek V4.1 Flash max, own loop, CheaperInference | 3/3 | **9** | 1575 s | 77 | 4.4M | 98% | $0.024 |
| DeepSeek V4.1 Flash **HC** max, own loop, BeatAPI | 3/3 | **9** | 641 s | 58 | 3.2M | 97% | $0.027 |
| DeepSeek V4.1 Flash max, own loop, BeatAPI (regular) | 1/3 | 1 (no change) | 965 s | 39 | 1.8M | 39% | $0.102 |

With Luna, Codex is a point better at a third of the time and input tokens: the own loop has a real gap with GPT
models (next: where the 3x tokens go). With DeepSeek the own loop is the best result in the pool. BeatAPI's high-cache
route (`deepseek-v4.1-flash-hc`, $0.105/$0.0021/$0.42 per M, doubled on weekdays 09-12 and 14-18 Beijing time) matches
CheaperInference at about the same cost and faster; its regular route caches poorly and its run changed nothing.

## 0.0.52 (2026-10-14)

**`TAZUNA_PRICE=input,cached,output`** (USD per million tokens) for providers whose model list has no prices: the own
loop's dollar cap needs one.

First use, apinex (DeepSeek V4.1 Flash max, own loop, rclone from the October set; the provider reports no cached
tokens and bills two identical requests the same):

| | result | time | turns | cost |
|---|---|---|---|---|
| CheaperInference (earlier runs) | 3/3 | 352-1575 s | 39-89 | $0.010-0.026 |
| Experiential Labs | 3/3 | 593-1119 s | 60-91 | $0.086-0.204 |
| apinex free tier | stopped: 10M free tokens used up | 2354 s | 84 | $0 |
| apinex paid (5 requests/minute) | stopped by hand, unfinished | 2120 s | 115 | $0.465 |
| apinex `claude-opus-5.5` medium (own loop) | 2/3 | 391 s | 25 | $0.270 |
| BeatAPI DeepSeek (cache hits 39%) | 1/3 | 965 s | 39 | $0.102 |

DeepSeek there: about $0.06 per million input tokens, but every re-sent token is billed in full: agent loops cost 20x and
more. Its Opus 5.5: a flat ~$0.50 per million tokens (about a tenth of Anthropic's list price, so of unclear origin), fast
and few turns, but 2/3 tests at 10-25x DeepSeek's cost on CheaperInference. BeatAPI passes caching through, but only 39%
of input came from cache (97-99% elsewhere) and the run solved 1 of 3 (CheaperInference: 3 of 3 in every run). None is
used; CheaperInference stays the cascade's provider.

## 0.0.51 (2026-10-14)

**`--review-model` uses the five-lens review prompt** (bench/review-lens.txt: acceptance criteria with a counterexample,
every other reader and writer of the touched data, partial failure and retries, lifecycle of new state, actors and
variants). Only Critical and Major findings go back, each with a failure example, and the implementer is told to fix
with the smallest change. **Fixed:** with boss mode the review started at the first result, while subagents were
still working in the background (3 of 5 runs below), so it saw half a change; it now waits until no background task
runs.

First measurement (5 of the user's company tickets, boss + Opus 5.5 high reviewer, 1 run; 3 of the 5 reviews hit the
bug above): the peer's round-1 defects that apply, 9 of 16 with the review as without it; cost $14.68 vs $8.90 (+65%).
The reviews raised real issues the peer only found in later rounds (a gate a product swap never closes, mismatched
`anchorStepKey`), which this check does not count. Stays opt-in; a clean rerun of the 3 affected tickets and a judge
pass decide.

## 0.0.50 (2026-10-13)

**rustls instead of the system's TLS, and a size-tuned release build (strip, LTO, one codegen unit).** Linking the macOS
Security framework for the own loop (0.0.28) cost every tazuna process ~4.4 MB of memory at start, used or not: the UI
went from 7.2 to 11.6 MB and the tool server from 2.1 to 6.4 MB (bisected over 71 commits; 0.0.41's "UI 7 MB" was Tier 0
at 0.0.20). rustls is plain Rust code that stays out of memory until a provider is called. It made the binary 1.3 MB
larger; strip and LTO took that back and more.

| Tier 0 | 0.0.49 | 0.0.50 |
|---|---|---|
| binary | 4.04 MB | 3.35 MB (−17%) |
| UI memory (animating) | 11.6 MB | 7.3 MB (−37%) |
| tool server memory | 6.3 MB | 2.1 MB (−67%) |
| startup | 9.1 ms | 7.2 ms |
| UI CPU | 2.0% | 1.2-1.7% |
| replay render | 10 ms | 8 ms |

Own loop, one small fix (`bench/agent_footprint.py`): fixed on CheaperInference (22.2 MB peak, engine 19.3 MB) and on
Experiential Labs (20.8 MB), against 27 MB at 0.0.41. Root certificates now come from the bundled Mozilla list
(webpki-roots), not the system keychain: a provider behind a private CA would need its certificate added there. A clean
release build takes ~45 s.

## 0.0.49 (2026-10-13)

**Boss mode is the default for `--run` on Opus on the claude path.** Opus only directs; Haiku subagents read, edit and
run the tests (`--boss haiku`, 0.0.32). `--no-boss` turns it off; `--boss MODEL` picks another worker model. Interactive
sessions, the tiny-repo Sonnet lane and the own loop are unchanged (not measured with boss). Fixed on the way: `--boss`
switched an untrusted repo from the lean profile to the repo profile, so the repo's hooks and settings ran without trust.
Boss now turns itself off in an untrusted repo (with a warning when it was asked for).

Company tickets (5 company tickets × 2 runs, Opus 5.5 medium; one judge pool, Opus xhigh with reference; 698 and 870 from
the 0.0.37 pool with 2 passes, 872, 921 and 946 judged again with 1 pass to include the 3 new boss runs):

| | judge | cost per ticket | turns | time | new type errors / failing unit tests |
|---|---|---|---|---|---|
| stock Claude Code (cc) | 6.15 | $2.67 | 49 | 12 min | 0 / 0 |
| tazuna default until now (repo profile) | 5.90 | $2.18 | 33 | 31 min | 0 / 0 |
| tazuna `--boss haiku` | 5.25 | $1.38 (−37%) | 14 | 23 min | 0 / 0 |

Judge −0.65 against the old default, within the ±0.75 noise threshold; cost −37%, past it. On the Go tasks boss was −0.13
at −32% (0.0.48 table). By the quality-per-dollar rule this makes boss the default. The drop is close to the threshold
and concentrated in the 3 new runs (4, 5 and 2 on 872, 921 and 946, each the lowest on its ticket; the judge named missed
edge cases, e.g. a removed guard that re-opens T-619). Edge cases: 2.90 vs 3.45. If later tickets confirm a real
drop, the next lever is a reviewer pass on the boss's diff, not dropping boss. The 3 new boss runs cost $0.94-0.99 each
and finished in 15-18 minutes.

Benchmark note: `tazuna` harnesses in `bench/` now run with boss unless they pass `--no-boss`.

Follow-up, same day: **boss + an Opus 5.5 high reviewer** (`--review-model claude-opus-5-5 --review-effort high`, 1 run
per ticket) to buy the edge cases back. Fresh judge pool over all 5 tickets (1 pass, with reference, 7 candidates each):
stock 5.90 at $2.67, tazuna without boss 5.90 at $2.18, boss 5.20 at $1.38, boss + reviewer 5.20 at $2.15. The reviewer
found problems in 4 of 5 patches (review $0.11-1.12, mean $0.63); edge cases rose (3.2 vs 2.8) but scope fell (2.8 vs
3.2): the fixes it asked for widened the patches. Same score at the old default's price: not adopted. The boss gap held
in the second pool (−0.70), so on tickets boss is consistently ~0.7 lower; it stays the default for `--run` by the
quality-per-dollar rule, and `--no-boss` is the choice for a ticket where quality matters more than cost.

## 0.0.48 (2026-10-13)

**`--explorer MODEL@URL` (opt-in): Opus hands code questions to a cheap model on tazuna's own loop.** `tazuna
explore-mcp` serves one MCP tool, `explore`, answered by MODEL with read-only tools of its own (`read`, `search` = grep,
`list` = find; no shell, paths inside the repo only, so no key files or other projects reach the provider). The code
leaves the machine, so the host must be trusted or `--data any`. The explorer's cost lands in the result
(`explorer_cost_usd`, and in the total). A security review caught the first version, which gave the explorer bash behind a
word list (`rg --pre`, `git -c core.pager`, `sed 'w file'` and `sort -o` got through) and an unrestricted `read`.

Measured on the 6 Go tasks (Opus 5.5 medium, DeepSeek V4.1 Flash max as explorer, 1 run; judge 1 pass with reference):

| | hidden tests | judge (with ref) | cost (6 tasks) |
|---|---|---|---|
| tazuna default (Haiku explorer, 2 runs) | same | 7.13 | $3.97 |
| Opus + DeepSeek explorer | same (terraform 1/3, hugo-template 7/8, rest full) | 7.33 | $4.31 (+9%) |
| `--boss haiku` (2 runs) | same | 7.00 | $2.70 |

No saving: Opus asked the explorer 2-3 questions per task and read and searched on its own the rest of the time (25 own
calls against 2 questions on hugo-template-return); each DeepSeek answer took ~4 minutes on CheaperInference, so runs got
up to 3x longer. Stays opt-in. Opus only stops paying for reading when it cannot read at all, which is what `--boss` does.

## 0.0.47 (2026-10-13)

**A model call that times out gets twice the time on the next try** (180 s, then 360, 720, at most 15 minutes). At the
official speed (~30 tok/s), MiMo V2.6 Pro's reasoning answers take over 3 minutes; every try was cut off at 180 s, and a run
made 26 calls in 2 hours.

**Gauge run for the model check's first flag** (MiMo V2.6 Pro for the cascade's cheap step: +0.08 over DeepSeek V4.1 Flash
on public benchmarks). caddy-unix (Opus fails it, DeepSeek solves it), own loop at max, one Opus judge call:

| | hidden tests | judge | cost | time | output tokens |
|---|---|---|---|---|---|
| DeepSeek V4.1 Flash, CheaperInference (0.0.38) | 7/7 | 9 | $0.02 | 44 min | 37k |
| MiMo V2.6 Pro, Experiential Labs (156 tok/s) | 7/7 | 9 | $0.67 | 59 min | 555k |

Same result at ~30x the cost: MiMo writes 15x the output. DeepSeek stays the cheap step. Experiential Labs serves MiMo at
5x the speed Artificial Analysis measures for the official API, with no quality loss on this task. The other four tasks
were stopped when the Experiential Labs balance ran low; the CheaperInference run (official speed) hit the timeout above.

## 0.0.46 (2026-10-12)

**`ctrl-x` stashes the message:** it goes aside and the composer clears; after the next message is sent it comes back by
itself, and `ctrl-x` on an empty composer brings it back at once (with both, they swap). **`/vim`: modal editing in the
composer**, remembered across sessions: esc for normal mode (`h l w b e 0 ^ $ x X D C dd cc dw cw j k`), `i a I A o` to type
again; enter still sends, ctrl keys work as always, and esc in normal mode with nothing typed stops the run (it never clears
the text). The composer shows `-- NORMAL --` / `-- INSERT --` while typing. Checked in a pseudo-terminal: stash and unstash,
`/vim` on and off, and "fix the parser bug" + esc `0 w dw` became "fix parser bug" in normal mode.

## 0.0.45 (2026-10-12)

**`/rewind`: undo a message and everything after it.** It lists the messages sent so far; enter rewinds files and
conversation, `f` files only, `c` the conversation only. Files: claude's own file checkpoints
(`CLAUDE_CODE_ENABLE_SDK_FILE_CHECKPOINTING`, live sessions only) and its `rewind_files` request, with a dry run first that
names the files. Conversation: claude restarts at the last reply before that message (`--resume-session-at`, as a fork of
the session), and the message comes back into the composer to edit and send again. Each typed message carries an id
tazuna gives it, which is what both take. Checked in a pseudo-terminal against Haiku: after "create a.txt with one" and
"append two", `/rewind` + enter reported "restoring 1 file(s): a.txt (+1 −2)", a.txt held only "one" again, the second
message was back in the composer, and asked about its previous message claude named only the first. Limits: claude
checkpoints only its own edit tools, so in the lean profile there is nothing to restore (tazuna says so); not on the own
loop yet.

**Text from outside cannot drive the terminal.** Every transcript row now passes the same filter as permission questions
and the task view: escape sequences are dropped whole and other control or invisible characters show as `�`; the Cockpit's
file list and tool names too. Before, model replies, tool output and notices (a file name in the rewind notice, an error
text) reached the screen unfiltered, so a crafted file name or command output could have set the window title, written the
clipboard or cleared the screen (found by a security review of the rewind notice). Replay render time unchanged (9 ms).

## 0.0.44 (2026-10-12)

**`ctrl-b`: the background tasks of a session** (commands claude runs with `run_in_background`, Monitors, background
subagents): status, age and the latest output of the selected one, refreshed every second; `x` stops the selected task, the
run goes on. While one runs, the composer shows `[N background · ctrl-b]`. Built on claude's own stream: `task_started`,
`task_updated` and `task_notification` events, and the `get_task_output` and `stop_task` control requests (the same data as
Claude Code's `/tasks` view). Checked in a pseudo-terminal against Haiku: a 60-second loop showed as running with its
output ticking, and `x` stopped it. Only with Claude Code's own tools (repo profile); the lean profile's bash and the own
loop run every command to its end. Task output and descriptions pass through the same filter as permission questions: every
escape sequence is dropped whole (CSI, OSC up to BEL or ST, DCS/APC/PM/SOS) and any other control or invisible character
shows as `�`, so a command's output cannot set the window title, write the clipboard or move the cursor (a security
review caught the first version, which only removed colour codes).

## 0.0.43 (2026-10-12)

**`ctrl-g` opens the message in your editor** (`$VISUAL`, else `$EDITOR`, else `vi`; a command like `code --wait` works).
tazuna steps out of the screen while the editor runs; what you save comes back into the composer, and a failing editor
leaves the message as it was. Checked in a pseudo-terminal: the edited text is back in the composer and the screen redraws.
The temporary file is created exclusively with mode 0600 under an unguessable name (a security review flagged the first
version's predictable name: in a shared /tmp a planted symlink could have redirected the write).

## 0.0.42 (2026-10-12)

**The own loop asks before commands and edits.** With `--provider`, the ask mode now asks before every `bash`, `write` and
`edit` (reading stays free) and the edits mode before `bash`; shift-tab switches mid-run. The engine sends claude's own
`can_use_tool` request, so tazuna's question UI, "always" answers and an interrupt while a question is open work as on
claude. Checked against DeepSeek over the engine's protocol: it read the file, asked before its edit, the answer "no" left the
file unchanged, and the model explained the fix instead. Plan mode stays claude-only (it needs claude's plan approval);
shift-tab skips it with `--provider`. `--run` now says that `--mode` does not apply to headless runs instead of ignoring it.

## 0.0.41 (2026-10-12)

**The cascade is the own loop's default.** With `--provider`, a task the own loop cannot finish (tests still failing, an
error, no change, the budget spent) goes to Opus 5.5 medium on claude without `--escalate`; `--no-escalate` keeps the own
loop's result. The `cascade` preset no longer needs the flag. **Model calls time out after 180 s** (was 300): of 15,505
recorded DeepSeek and Luna calls, the median took 15 s, 99% under 83 s and 0.1% over 180 s, while a provider that accepts a
call and never answers stalled a run for an hour across the retries.

## 0.0.40 (2026-10-12)

**Own loop on Experiential Labs** (`--provider https://api.experientiallabs.ai/v1`): its model list gives prices in nano-USD
and names the upstream provider in `data_policy`; both are read now, so GPT-6 Luna there goes through the Responses API.
**Retries span ~15 minutes** (a free model "at capacity" ended all seven runs of a benchmark at 4 minutes), but a 429 that
waiting cannot fix (`insufficient_quota`, a model locked until a purchase) ends the run at once.

**Cheap models next to DeepSeek V4.1 Flash** (own loop, max effort, October tasks; no judge yet):

| | provider | runs | cache hits | speed | cost vs DeepSeek | result |
|---|---|---|---|---|---|---|
| Qwen3.8 Flash | CheaperInference | rclone, gitea (stopped at ~70 min, balance) | 82-95% | 25-40 tok/s | 2-3x | none finished; DeepSeek: 12-26 min |
| GLM-5.3 Flash | CheaperInference | same | 16-58% | 8-28 tok/s | 4-10x | none finished |
| GLM-5.3 Flash | Experiential Labs | rclone (call hung 20+ min), gitea (stopped at $0.20, 84 min) | 79-96% | 30-50 tok/s | 5-6x | none finished |
| Qwen3.8 Flash Next (free) | Experiential Labs | 7 tasks | - | - | free | "at capacity" for free accounts; unusable |
| Qwen3.8 Flash | Experiential Labs | - | - | - | - | locked until credits are bought |
| DeepSeek V4.1 Flash | Experiential Labs | rclone | 95% | 76 tok/s | 8-14x (list price 4x, more output) | 3/3, judge 9.5 / 7.5 |

None beats DeepSeek on CheaperInference on price or speed; DeepSeek stays the cascade's first step.

**"Few turns" prompt for the own loop, measured and kept off** (`TAZUNA_BATCH=1`: bundle independent tool calls, read whole
files, run targeted tests until the end; DeepSeek made 1.14 tool calls per response):

| task | turns | total tokens | cost | hidden tests |
|---|---|---|---|---|
| rclone | 39 vs 77 | 1.6M vs 4.5M | $0.010 vs $0.024 | 3/3 both |
| caddy-unix | 71 vs 60 | 3.7M vs 3.3M | $0.036 vs $0.020 | 7/7 both |
| telegraf | 100 vs 129 | 5.7M vs 9.9M | $0.051 vs $0.047 | 2/2 both; package suite failed with the prompt |
| gitea | stopped at 59 vs 99 | 5.0M vs 8.3M | $0.112 (stopped) vs $0.035 | - |

Turns -21% and tokens -38% on the three finished tasks, but cost rose: with fewer turns DeepSeek writes 2-4x more output,
and its output costs 200x a cache read. Worth retrying only with a model whose input is expensive relative to its output.

## 0.0.39 (2026-10-12)

**Own loop: no time limit by default, $1 per task.** A cheap model's extra hour costs cents, so the own loop now runs until
it is done or has spent $1 on the task (was 60 minutes and $0.50: on lazygit and prometheus, DeepSeek reached $0.28-0.49
after about 2 hours, still working). `--max-minutes N` brings a time limit back for whoever wants it fast (with
`--escalate`, the hand-over comes after N minutes); `TAZUNA_MAX_USD` still overrides the dollar cap. The turn backstop rises
to 2000 (200 had cut DeepSeek off after ~75 minutes). The provider's model list at startup is retried like model calls:
one run died when that request hung.

**Benchmark: cascade on the 7 October tasks** (DeepSeek V4.1 Flash max first, Opus 5.5 medium on failure; 2 runs; Opus
5.5 xhigh judge with / without reference):

| | fully solved | judge without ref | with ref | cost (14 runs) | hours (sum / longest run) | Opus hand-overs |
|---|---|---|---|---|---|---|
| tazuna on Opus 5.5 medium (0.0.34) | 9/14 | 6.79 | 6.93 | $38.90 | 16.8 / 8.6 | - |
| cascade 0.0.34 (60 min, reset by each follow-up) | 12/14 | 7.47 | 7.97 | $5.04 | 12.3 / 2.1 | 6 |
| cascade 0.0.38 (no time limit, $0.50) | 13/14 | 7.79 | 8.32 | $8.36 | 14.1 / 2.7 | 5 |
| cascade 0.0.39 ($1), lazygit and prometheus only | 3/4 | 5.50 / 8.00 | 6.25 / 8.25 | $0.85 (4 runs) | 4.1 / 2.0 | 0 |
| GPT-6 Luna max, own loop, no fallback (1 run) | 5/7 | 6.79 | 6.14 | $0.85 (7 runs) | 3.7 / 1.4 | - |

Without the time limit, quality and solved runs move within noise (+0.3 judge, +1 solved) and the sum of hours by +15%;
cost rose 65%, nearly all from one prometheus hand-over ($3.79). The $1 cap is untested so far: none of its four runs got
near $0.50. DeepSeek solved lazygit in 10-17 minutes in both runs here and needed 2.7 hours in both runs at 0.0.38, so a
single task's time swings far more than any setting does. One prometheus run claimed success with the visible tests green
and failed the hidden ones, a miss the cascade cannot catch: it hands over only on visible failures. Luna is quick (5 tasks
under 30 minutes) but judged ~1 point below the cascade, which has Opus behind it.

## 0.0.38 (2026-10-11)

**Own loop: the time and dollar budget count per task.** tazuna's follow-ups (test failures, review) used to start a fresh
hour and a fresh $0.50 each; one DeepSeek run passed the benchmark's 90-minute limit that way, and a cascade waited that
much longer before handing over to Opus. **Model calls retry for about 4 minutes** (was 40 s): a short network outage
ended a 38-minute run.

**Two own-loop ideas from arXiv 2609.20804, measured and dropped** (DeepSeek V4.1 Flash max, own loop, 7 October tasks):
- *Eliding old tool outputs* (stubs once outputs pass ~30k tokens). Prompts shrank to ~60k tokens per call from 100-118k,
  but calls were not faster (23-29 s vs 15-27 s: the provider's wait dominates, not prompt size), and with 99% cache hits
  it saves almost nothing. Eliding every old output made the model re-read one file 16 times; eliding only stale reads
  and old bash output fixed that, still without a speed gain.
- *A `plan` tool* (checklist the model writes and updates, plus one system-prompt line; without that line it was never
  called). 1 run each:

| task | own loop | own loop + plan |
|---|---|---|
| caddy-retry-match-expr | over 90 min (budget bug above) | 2/2 |
| caddy-unix-socket-close | 7/7 | stopped at 60 min |
| lazygit-mnemonic-prefix | stopped at 60 min | 3/3 |
| gitea-actions-cancelling | 2/2 | 2/2 |
| rclone-archive-symlinks | 3/3 | 3/3 |
| telegraf-snmp-reconnect | 2/2 | 1/2 |
| prometheus-exemplar-compaction | stopped at 60 min | stopped at 60 min |

4 of 7 fully solved either way, cost $0.02-0.09 per run either way: no effect, so the code is gone. Most useful side
result: DeepSeek alone runs out of its hour on 2-3 of these 7 tasks per run, which is the cascade's hand-over to Opus doing
its job.

## 0.0.37 (2026-10-11)

**`--advisor MODEL` (opt-in until benchmarked):** sets Claude Code's `advisorModel`, which gives the main model an
`advisor` tool to consult MODEL when it is stuck (checked: with Sonnet 5.5 as main model and `opus` as advisor the tool
appears). Idea from a thread suggesting Opus as advisor with cheaper models doing the work.

**Benchmark: popular plugins and the advisor** (6 Go tasks, 1 run each, Opus 5.5 xhigh judge, each run alone):

| | judge without ref | with ref | cost (6 tasks) |
|---|---|---|---|
| stock Claude Code (Opus 5.5 medium, 2 runs, earlier) | 7.58 / 7.58 | 6.83 / 7.08 | $5.46 avg |
| + ECC | 7.67 | 7.08 | $5.56 |
| + superpowers | 7.50 | 6.83 | $5.18 |
| + caveman | 7.75 | 7.17 | $6.35 |
| + graphify alone | 7.58 | 6.92 | $5.24 |
| + GStack (skills; browser binary not built) | 7.08 | 6.75 | $5.32 |
| + all five together | 7.42 | 7.17 | $5.36 |
| tazuna default (Opus 5.5 medium + Haiku explorer, earlier) | 7.42 / 7.50 | 7.17 / 7.08 | $3.97 avg |
| tazuna, Sonnet 5.5 medium + Haiku explorer | 6.33 | 6.17 | **$1.35** |
| **tazuna, Sonnet 5.5 + `--advisor claude-opus-5-5`** | **7.33** | 6.75 | **$2.17** |

No plugin, alone or all together, moves quality past noise or lowers cost (−5% to +16% vs stock). The advisor lifts Sonnet
by +1.0 judge to about Opus's level (−0.1 vs tazuna's Opus default) at −45% cost: about `--boss haiku`'s quality and cost
(7.33, $2.70). One run each; a second run decides.

**Second run** (Go tasks again; company tickets 5 × 2 runs, judged in one pool with stock and tazuna on Opus):

| | Go judge without ref | with ref | Go cost (6 tasks) | tickets judge | ticket cost per run |
|---|---|---|---|---|---|
| stock Claude Code, Opus 5.5 medium | 7.58 | 6.96 | $5.46 | 6.15 | $2.67 |
| tazuna default, Opus 5.5 medium | 7.46 | 7.13 | $3.97 | 5.70 | $2.18 |
| tazuna `--boss haiku` | 7.33 | | $2.70 | 5.79 (7 runs) | $1.56 |
| tazuna Sonnet 5.5 + Opus advisor (2 runs) | 7.33 / 6.83 | 6.75 / 6.33 | $2.17 / $2.29 | 5.10 | $0.61 |

On Go the advisor holds up (7.08 mean without reference, −0.4 vs the Opus default, within noise) at −44% cost. On the
company tickets it falls 0.6 below the Opus default and 1.05 below stock, at −72% cost; no new type errors or unit failures,
PR-test counts identical in both runs. It stays opt-in: cheap, but the tickets show a drop larger than noise relative to
stock. `--boss haiku` keeps Opus-level quality on both sets.

## 0.0.36 (2026-10-11)

**`--preset NAME`: a model and effort per role, bundled.** Built in: `default` (Opus 5.5 medium + Haiku explorer), `boss`
(`--boss haiku`), `max` (Opus 5.5 at max effort, opt-in: terraform 3/3 at about 5× the cost of medium), `cascade`
(DeepSeek V4.1 Flash max on CheaperInference with `--data any`, Opus 5.5 medium on failure). `~/.config/tazuna/presets`
adds or overrides presets (`name: --flag value ...`); flags on the command line win over the preset's.

## 0.0.35 (2026-10-11)

**`--data trusted|any` (default `trusted`).** With `trusted`, `--provider` only accepts hosts listed in
`~/.config/tazuna/trusted-providers` (one per line); Anthropic through the claude CLI is always allowed. Providers that may
store or use the code (for example DeepSeek routes) need `--data any` or an entry in the list. A strict zero-retention mode
was considered and left out: none of CheaperInference's 97 models offers a ZDR route today, and the Claude subscription
follows the account's own retention terms.

## 0.0.34 (2026-10-10)

**Own loop: one hour per run.** A run still going after 60 minutes ends with an error `result`; with `--escalate` the
task then moves to the stronger model on claude. DeepSeek on the prometheus hard tasks ran into the benchmark's 3-hour
limit at 15-24 s of provider wait per call; the cascade could not take over.

Second cascade run (0.0.34): the same 8 tasks all pass every hidden test again, none breaks a passing test, $0.11 in total,
no escalation. Judge (Opus 5.5 xhigh, each run alone): Go 10 / 10 / 10 / 9 / 10 / 9, **mean 9.67** (run 1: 9.42); hard
caddy-tls 10, hugo-eq 9. Prometheus hard tasks: the hour ran out (replay-mmap) or the tests failed (wbl-checkpoint), and
both moved to Opus as designed.

**The judge anchors on the reference fix.** Same runs judged again without the merged PR's code (`JUDGE_NOREF=1`, task,
diff and test facts only). Go task means, with → without reference:

| | with | without |
|---|---|---|
| stock Claude Code r0 / r1 | 6.83 / 7.08 | 7.58 / 7.58 |
| tazuna default r0 / r1 | 7.17 / 7.08 | 7.42 / 7.50 |
| `--boss haiku` r0 / r1 | 7.08 / 6.92 | 7.33 / 7.33 |
| DeepSeek, own loop, no effort r0 / r1 | 8.33 / 8.20 | 7.67 / 7.70 |
| **cascade (DeepSeek max) r1 / r2** | **9.42 / 9.67** | **8.17 / 8.17** |

DeepSeek's solutions resemble the merged fixes (reference: up, no reference: −0.5 to −1.5); the Opus setups' solutions
differ in design (no reference: +0.3 to +0.8). The cascade's lead on the Go tasks shrinks from ~2.4 to ~0.7. Hard tasks
(caddy-tls, hugo-eq; merged after DeepSeek's release), no reference: cascade 9.0 / 7.5, tazuna Opus 7.25 / 7.0. Also: Opus
5.5 medium on the own loop (one agent, four tools, like DeepSeek) got terraform 1/3, like every Opus setup on claude: the
loop is not what makes the difference.
Effort is: tazuna default with Opus 5.5 at **max** solved terraform 3/3 (judge 9 with and without reference; 79 turns,
50 min, $4.71), where Opus at medium gets 1/3 in every harness (judge 4-5). DeepSeek at max: 3/3 for $0.01. Opus max stays
an opt-in profile (about 5× the cost of medium on this task).

**Clean benchmark: 7 tasks merged 1-10 October 2026** (after DeepSeek's release; caddy ×2, gitea, lazygit, prometheus,
rclone, telegraf), 2 runs each, judge Opus 5.5 xhigh with and without the reference fix:

| | judge without ref r0 / r1 | judge with ref r0 / r1 | cost r0 / r1 (7 tasks) |
|---|---|---|---|
| **cascade** (DeepSeek V4.1 Flash max, Opus 5.5 medium on failure) | **6.86 / 8.07** | 7.29 / 8.64 | **$2.25 / $2.79** |
| tazuna default (Opus 5.5 medium + Haiku explorer) | 7.21 / 6.36 | 7.21 / 6.64 | $20.31 / $18.59 |

On average the cascade scores +0.7 (no reference) at about 1/8 of the cost; without the prometheus task (Opus there:
$10-12.7 per run, 5-8.6 h, part of it while the Mac slept) the default costs $7.6 / $8.6 and the cascade $0.82 / $0.91.
The cascade escalated on rclone, telegraf, gitea (once) and prometheus (twice). Opus failed caddy-unix-socket-close in both
runs (0/7 hidden tests, a passing test broken; judge 4) where DeepSeek solved it alone (7/7, judge 9-10). Prometheus split
both ways: cascade 2.5 / 8, default 8 / 3. Telegraf breaks a passing test in every run of both setups.

Benchmark fixes found on the way: the diff was taken against HEAD, so an agent that commits left an empty diff (Opus on
lazygit, two planner runs, one tester run were judged as empty); it is now taken against the base commit. The usage gate
skipped jobs on a stale meter reading after a guard restart; a stale reading now waits.

## 0.0.33 (2026-10-10)

**Cascade: `--escalate MODEL[:EFFORT]` with `--provider`.** The cheap model works on tazuna's own loop first; when its
tests still fail, or it stops with an error or changes nothing, the task moves to MODEL on the claude CLI in a fresh session
with the task, a note that a cheaper model's changes are in the working tree, and the test failures. Smoke: Gemma 3 4B
changed nothing, Haiku 5.5 took over, fixed and verified.

Benchmark (DeepSeek V4.1 Flash max first, Opus 5.5 medium on failure; 1 run): all 8 tasks pass every hidden test, none
breaks a passing test, $0.17 in total; no task needed the escalation (DeepSeek's own tests passed each time). Go: k6 5/5,
prometheus 1/1, terraform **3/3** (every Opus run 1/3), caddy 5/5, hugo-section 2/2, hugo-template-return 8/8. Hard
(merged after DeepSeek's release): caddy-tls 1/1 (every Opus setup 0/1), hugo-eq 4/4. Prometheus hard tasks left out
(DeepSeek runs out of time there; the cascade has no time budget yet).
Judge (Opus 5.5 xhigh, each run alone): Go 10 / 10 / 9 / 10 / 9 / 8.5, **mean 9.42** (Opus default 7.12, stock 6.96,
DeepSeek without effort 8.33 / 8.20); hard caddy-tls 10, hugo-eq 9.

Also (0.0.32 follow-up, T-698 and T-870, all setups judged in one pool): `--boss haiku` 6.5 / 6.0 vs the default
6.75 / 5.75 and stock 7.0 / 7.0, at $2.26 / $1.75 vs the default's $2.26 / $2.29 (two runs each; boss mode's cost varies
run to run, the earlier −45% was partly one lucky run). `--boss sonnet`: 7 / 6 at $3.54 / $1.98, no gain: dropped.

## 0.0.32 (2026-10-10)

**`--boss MODEL` (opt-in until benchmarked): the main model only directs.** Explorer, implementer and tester subagents on
MODEL do all reading, editing and test runs; a PreToolUse hook (`tazuna boss-hook`) denies the main model every tool but
delegation (subagent calls carry an `agent_id`, the main model's do not; `--tools`/`--disallowedTools` would block the
subagents too). Implies the repo profile. Why: in the default, Opus still did ~27% of the reading, all test runs and every
edit itself (T-872: Opus 19 reads, 11 runs; Haiku explorer 52 reads). Smoke (Python fixture, Opus medium + Haiku):
Opus 2 delegations, Haiku 3 Bash, 1 Read, 2 Edit; verified, $0.13.

**Benchmark (2026-10-09/10, Opus 5.5 medium everywhere, judge Opus 5.5 xhigh, 2 passes).**

Go tasks, 2 runs each, every run judged alone:

| | judge r0 / r1 | cost r0 / r1 (6 tasks) |
|---|---|---|
| stock Claude Code | 6.83 / 7.08 | $5.50 / $5.42 |
| tazuna default (Haiku explorer) | 7.17 / 7.08 | $4.26 / $3.68 |
| **tazuna `--boss haiku`** | **7.08 / 6.92** | **$2.36 / $3.04** |
| Opus plan → Haiku executes → Opus checks | 5.92 | $6.53 |
| DeepSeek V4.1 Flash, own loop (CheaperInference) | 8.33 / 8.20 | $0.53 / $0.40 |

Company tickets, one judge pool per ticket (all setups side by side):

| | judge | cost per run | runs |
|---|---|---|---|
| stock Claude Code | 5.75 | $2.67 | 10 |
| tazuna default | 5.50 | $2.18 | 10 |
| tazuna without the explorer | 6.00 | $5.34 | 4 (1 hit 90 min) |
| **tazuna `--boss haiku`** | **5.20** | **$1.19** | 5 (5 more hit 90 min under load) |

Boss mode: same quality on the Go tasks at −32% vs the default and −51% vs stock; on the company tickets −45% vs the
default and −55% vs stock for −0.30 / −0.55 judge (one run per ticket; within noise but lower on 3 of 5). Opus needs 4-17
turns instead of 12-40. The five first-batch ticket runs hit the 90-minute limit while 11 runs and their test suites shared
the machine; the second batch, alone, took 19-26 min. Stays opt-in until a second ticket run.

Also measured: without the explorer the tickets score +0.5 at 2.4× the cost (the explorer stays); Opus plan → cheap
executor costs more than Opus alone (planning at high effort alone cost $0.47-1.21) and a wrong plan steers the executor
wrong (caddy-tls: DeepSeek alone 1/1, DeepSeek following Opus's plan 0/1); hard tasks (merged after DeepSeek's release):
DeepSeek solved caddy-tls (judge 10, $0.017) where every Opus setup failed, but timed out on the prometheus tasks
(CheaperInference adds 15-24 s per call; the agents rerun a 10-minute suite).


## 0.0.31 (2026-10-09)

**Own loop: a model call that hangs is retried.** Two of 18 benchmark runs hung on a call that never answered (the engine
waited 600 s, then gave up; the benchmark's 90-minute limit ended the run). Calls now time out after 300 s, and timeouts and
dropped connections get the same three retries as rate limits and server errors (2, 8, 30 s apart).

## 0.0.30 (2026-10-09)

**Six Go tasks, every run judged alone (Opus 5.5 xhigh, 2 passes).** Opus 5.5 medium is tazuna's default on claude; the
others run the own loop on CheaperInference. A run that hit the benchmark's 90-minute limit counts 0.

| Task | Opus r0 | Opus r1 | DeepSeek V4.1 Flash r0 | DeepSeek r1 | DeepSeek max | GPT-6 Luna max (Responses) |
|---|---|---|---|---|---|---|
| caddy-quoted-braces | 9 | 8.5 | 10 | 9 | 7 | 9 |
| hugo-section-slug | 9 | 8 | 9 | 9 | timeout | 7 |
| hugo-template-return | 4.5 | 5 | 9 | 9 | 9.5 | 3 |
| k6-run-once | 7 | 7 | 7.5 | timeout | 10 | 7 |
| prometheus-duration-parens | 9 | 9 | 9.5 | 9 | 9.5 | 3.5 |
| terraform-import-foreach | 4.5 | 5 | 5 | 5 | 4 | 5 |
| **Judge (finished runs)** | **7.17** | **7.08** | **8.33** | **8.20** | **8.00** | **5.75** |
| Hidden tests | 87% | 87% | 94% | 93% | 87% | 66% |
| Cost (6 tasks) | $4.26 | $3.68 | $0.53 | $0.40 (5) | $0.39 (5) | $0.37 |

Opus is consistent across its two runs. DeepSeek scores at or above Opus on every finished task at ~1/10 the cost; most of
the gap is hugo-template-return, which no harness in this pool has fully solved before (cc, OMP, Hermes, pi: at most 88%
of hidden tests) and DeepSeek solved in all three runs. Possible causes beyond the model: 4-8× the turns, the own loop,
training-data overlap on public repos (unchecked). Max effort shows no clear gain over the default. Luna stays behind and
breaks passing tests on two tasks. Two of 18 DeepSeek runs hung on a model call that never answered (the engine waits
600 s and does not retry a timeout): fix before the next benchmark.

**OpenAI models go through the Responses API.** Through Chat Completions with tools, GPT-6 Luna does not reason at all
(0 reasoning tokens at `xhigh` and `max`, with or without `max_tokens`); through Responses it does (54-98 on the same
prompt) and keeps `max` (Chat Completions on CheaperInference turns it into `xhigh`). Stateless (`store: false`): reasoning
comes back encrypted and goes out again with the history. Chosen by the model's owner in `/v1/models`. Every 0.0.28 and
0.0.29 Luna result is void. Smoke: GPT-6 Luna max fixed the Python fixture in 7 turns, $0.0005.

## 0.0.29 (2026-10-09)

**The own loop sends `--effort` as `reasoning_effort`.** 0.0.28 sent none, so every own-loop run used the provider's
default. On a hard prompt DeepSeek V4.1 Flash reasoned 152 tokens without it, 130 at `low` and 2,439 at `max`: the 0.0.28
DeepSeek results are near its lowest effort. GPT-6 Luna barely changes (109 / 53 / 93).

## 0.0.28 (2026-10-09)

**Own agent loop, first slice (`--provider URL [--key-env VAR]`).** tazuna runs its own loop on an OpenAI-compatible API
instead of `claude`: `tazuna engine` speaks claude's stream-json, so the run log, verification and the reviewer work
unchanged. Chat Completions without streaming, the four lean tools in-process, prices from the provider's `/v1/models`,
a $0.50 cap per run (and 200 turns as a backstop). The key is removed from the environment before any tool runs. Design and scope:
docs/OWN_LOOP.md.

| Metric | 0.0.27 | 0.0.28 |
|---|---|---|
| Binary | 2.77 MB | 3.80 MB (ureq with the system's TLS; rustls would be 5.17 MB) |

Smoke (DeepSeek V4.1 Flash on CheaperInference, two-bug Python fixture): fixed and verified in 6 turns, $0.0007, 75% of
input tokens from cache.
Go task smoke (hugo-section-slug, 1 run): hidden tests 2/2, $0.043, 57 turns, 28 min. tazuna on Opus 5.5 medium (0.0.15):
2/2, $0.52-0.60, 16-22 turns. Not judged yet. CheaperInference adds 15-24 s before the first byte of every call (a 7-token
prompt with a 2-token answer took 18 s), so its times say nothing about the model.

Six Go tasks × 1 run, each run judged alone (Opus 5.5 xhigh, 2 passes, same day):

| Task | Opus 5.5 medium (claude, 0.0.28 default): hidden, cost, turns, judge | DeepSeek V4.1 Flash (own loop): hidden, cost, turns, judge |
|---|---|---|
| caddy-quoted-braces | 5/5, $0.43, 14, 9.0 | 5/5, $0.10, 108, 10.0 |
| hugo-section-slug | 2/2, $0.53, 13, 9.0 | 2/2, $0.04, 57, 9.0 |
| hugo-template-return | 7/8, $1.36, 40, 4.5 | 8/8, $0.08, 60, 9.0 |
| k6-run-once | 5/5, $0.79, 23, 7.0 | 5/5, $0.12, 116, 7.5 |
| prometheus-duration-parens | 1/1, $0.71, 19, 9.0 | 1/1, $0.08, 105, 9.5 |
| terraform-import-foreach | 1/3, $0.44, 28, 4.5 | 2/3, $0.10, 124, 5.0 |
| **Total** | 87% hidden, **$4.26**, judge **7.17**, 64 min | 94% hidden, **$0.53** (−88%), judge **8.33**, 287 min |

One run per side, so within noise per task, but DeepSeek is equal or higher on all six. It takes 4-5× the turns; the time
is mostly CheaperInference's 15-24 s wait per call. The comparison mixes model and loop (claude CLI with the Haiku explorer
vs the own loop with four tools).

Also: the own loop refuses the ask, edits and plan modes for now (it never asks before a tool runs; found by a security
review); cost is the provider's billed `usage.cost` when reported.

## 0.0.27 (2026-10-09)

**`--review-model` reviews once per task, against the real task.** A run that claude starts on its own after a background
command (the Haiku explorer, so nearly every run) counted as a new run: the reviewer ran again for each wake-up, with the
placeholder "(claude continued on its own…)" as its task instead of the user's message. The reviewer now gets the user's
last message and runs once per message. First Fable 5.1 xhigh reviewer gauge (0.0.26, with this bug) is void.

Fable 5.1 xhigh reviewer gauge (T-872, 1 run, same judge pool as the default's 2 runs): one review per run confirmed in
the run log. Judge 5.00 vs 5.00, cost $3.56 vs $1.04 (3.4×; the review alone $2.15, 209 s), PR tests 58/86 both. The review
made Opus revert its backend change, which the judge called the root cause. Full run skipped; `--review-model` stays opt-in.

## 0.0.26 (2026-10-08)

**Headless runs no longer end while claude is about to wake up.** One run ended after 8 turns with no change ("Still
waiting on the explorer…"): its last background explorer finished during a turn, the 5-second settle counted from that
moment and had passed when the turn's result came, but claude started another turn 70 ms later. The settle now restarts at
each result while a finished background task is pending.

**`--tester <model>`** (opt-in): a second subagent runs tests, type checks, builds and linters and reports only the
failures, so their output stays out of the main model's context. All 5 company tickets, 2 runs each, all 10 at once, Haiku
explorer and Haiku tester against 0.0.24's Haiku explorer alone, one judge pool per ticket:

| Ticket | Explorer: cost, judge | Explorer + tester: cost, judge |
|---|---|---|
| T-921 | $1.24, 6.75 | $1.09, 6.25 |
| T-870 | $1.80, 6.50 | $1.61, 6.75 |
| T-946 | $1.87, 4.00 | $1.41, 3.00 |
| T-698 | $2.09, 7.50 | $1.86, 6.75 |
| T-872 | $1.04, 5.00 | $0.97, 5.00 |
| **Mean per ticket** | **$1.61, 5.95** | **$1.39 (−14%), 5.55 (−0.40)** |

The run that ended early (T-946, judge 1.0) is left out. Cheaper on every ticket, but the saving sits at the noise line
and the judge drop is larger than the explorer's (lower on 3 of 5 tickets): inconclusive (judge up on T-870, equal on T-872).
Stays opt-in until a retest with a second judge model. Usage: 10 runs + judge ≈ 19% of the 5-hour window.

## 0.0.25 (2026-10-08)

**Haiku explorer on by default** (repo profile; `--no-explorer` turns it off, `--explorer <model>` picks another model). The
user's measure is price to performance, not quality alone: 0.0.24's −28% cost for a judge −0.25 (within noise) is a better
ratio, and a strong reviewer can buy quality back where it matters. The default's numbers are now 0.0.24's Haiku column
(mean per ticket $1.61, 26 turns, judge 5.70).

## 0.0.24 (2026-10-08)

**Headless runs answer permission questions themselves.** A benchmark run hung for 25 minutes: Opus called
`AskUserQuestion`, and with nobody to answer, tazuna waited forever. With `--run`, every `can_use_tool` request is now
denied at once with "No user is present in this run … decide with your best judgment and continue." Live mode is unchanged.

**Haiku 5.5 explorer** (`--explorer haiku`, opt-in): all 5 company tickets, Opus 5.5 medium, 5-minute cache, one judge pool
per ticket. Default runs: 0.0.21 on T-921/870, 0.0.23 on the other three (the 0.0.12 runs there predate the 5-minute
cache). 2 runs per arm and ticket; one Haiku run on T-946 hung on the bug above and is excluded.

| Ticket | Default: cost, turns, judge | Haiku explorer: cost, turns, judge |
|---|---|---|
| T-921 | $1.38, 29, 5.75 | $1.24, 21, 6.50 |
| T-870 | $2.20, 42, 6.75 | $1.80, 33, 6.25 |
| T-946 | $3.54, 78, 4.25 | $1.87, 30, 3.50 |
| T-698 | $2.75, 47, 7.25 | $2.09, 31, 7.50 |
| T-872 | $1.33, 28, 5.75 | $1.04, 17, 4.75 |
| **Mean per ticket** | **$2.24, 45, 5.95** | **$1.61 (−28%), 26 (−41%), 5.70** |

Cost down well past noise on every ticket; judge −0.25, within noise, though lower on 3 of 5 tickets. No run added type
errors or unit failures, and each pair passes the same PR tests (one test fewer in one T-872 Haiku run). Haiku answers
cost a fraction of Opus reading the same files, unlike the Sonnet explorer (+12%). Made the default in 0.0.25.
Usage for this batch: 12 runs + judge ≈ 28% of the 5-hour window.

## 0.0.23 (2026-10-08)

**`--explorer` rule made firm.** With the first wording Opus called the explorer once in 4 runs and kept exploring with
`cat`/`sed`/`grep`. Now: no exploring yourself, every code question goes to the explorer, read a file only right before
editing it. Opus then delegated (its own shell commands dropped to 0-31 per segment; the explorers run in the background,
so each return shows as a separate run in the run log).

T-921 + T-870, 2 runs each, Opus 5.5 medium, Sonnet 5.5 explorer, 5-minute cache, one pool per ticket:

| Arm | 921 | 870 | Judge avg | Cost (4 runs) | Turns |
|---|---|---|---|---|---|
| default (0.0.21) | 6.0, 6.0 | 7.0, 7.0 | 6.50 | $7.15 | 24-50 |
| explorer, first wording (0.0.22) | 5.5, 6.5 | 7.0, 6.0 | 6.25 | $7.66 (+7%) | 29-45 |
| **explorer, firm rule (0.0.23)** | 6.0, 7.0 | 6.0, 6.0 | **6.25** | **$8.03 (+12%)** | **19-31** |

Fewer turns on the main model, but more money: the explorer reads the same code on Sonnet (only ~40% cheaper per token
than Opus) and its answers come back into Opus's context. Quality within noise. Stays opt-in; a Haiku 5.5 explorer is the
untested cheaper variant.

## 0.0.22 (2026-10-08)

**`--explorer <model>`** (opt-in until benchmarked, repo profile): a custom subagent (`claude --agents`) on a cheaper model
reads and searches the code and answers in short (paths, line ranges, the lines that matter); one appended rule tells the
main model to ask it and to read a whole file itself only right before editing. Reading is most of a run's cost and the
repo map did not reduce it; this moves the reading off the expensive model and out of its context.

## 0.0.21 (2026-10-08)

**`--repo-map`** (opt-in until benchmarked): the session's first message starts with a map of the repo, the folders with
file counts and, for the source files whose path or definitions match the task's words, the names they define (top-level
definitions by plain line matching; tests ranked below the code they test; capped at 12,000 chars). `tazuna map <task-file>`
prints it. On the company tickets it lists the files the real fixes changed (T-921: 1st and 7th of 25), in 0.7 s.

A/B on T-921 + T-870 (2 runs each, Opus 5.5 medium, 5-minute cache in both arms, judged with the afternoon's runs):

| Arm | 921 | 870 | Judge avg | Cost (4 runs) | Turns | Code read (median) | Searches (median) |
|---|---|---|---|---|---|---|---|
| default | 6.0, 6.0 | 6.5, 7.0 | 6.38 | $7.15 | 24-50 | 150k chars | 23 |
| `--repo-map` | 6.5, 6.0 | 7.0, 6.0 | 6.38 | $7.38 (+3%) | 23-59 | 153k chars | 19 |

No gain: the agent searched a little less but read as much, because it reads the files it changes in full; knowing where
to look does not shrink what it reads. Stays opt-in (`tazuna map` is still handy for a look at a task's files).

## 0.0.20 (2026-10-08)

**Prompt cache lifetime by mode.** claude writes its prompt cache with a 1-hour lifetime, billed at 2× input; a 5-minute
cache is billed at 1.25×. Cache writes are ~30% of a run's cost, and within a run turns come seconds apart, so headless
runs (`--run`) now use 5 minutes; live sessions keep 1 hour, where a pause over 5 minutes would re-write the whole context.
`--cache-ttl 5m|1h` overrides; a `CLAUDE_CODE_PROMPT_CACHE_TTL` the user set always wins.

A/B on T-921 + T-870 (2 runs each, Opus 5.5 medium, repo profile, same hour, side pool judged together):

| Arm | 921 | 870 | Judge avg | Cost (4 runs) | Turns |
|---|---|---|---|---|---|
| default (1-hour cache) | 6.5, 7.0 | 6.5, 6.5 | 6.62 | $8.16 | 30-46 |
| **5-minute cache** | 7.0, 7.0 | 6.5, 6.5 | **6.75** | **$7.35 (−10%)** | 32-44 |
| Bash output capped at 8,000 chars (`BASH_MAX_OUTPUT_LENGTH`) | 8.0, 6.5 | 6.0, 7.0 | 6.88 | $10.59 (+30%) | 40-61 |
| stock Claude Code (reference, earlier) | 6.0, 7.0 | 6.5, 6.0 | 6.38 | | |

Every 5-minute run was cheaper than every default run, as predicted from the cost breakdown (−11%). Capping Bash output
backfired: the agent ran more commands to see what was cut (54-61 turns on T-870 vs 42-46); ruled out. Benchmarks of
tazuna from this version on run with the 5-minute cache.

## 0.0.19 (2026-10-08)

Daily-driver features, part 2 (headless runs unchanged).

- **Slash commands** reach claude, which runs them in stream mode: built-ins (`/compact`, `/cost`, `/clear`, `/model`,
  `/context`, `/init`) and the repo's own commands and skills. tazuna shows output that has no model turn (`/cost`),
  marks `/clear` and `/compact` in the transcript, and completes commands with `tab` from the list claude reports.
- **`@` file completion** from the repo's tracked files (`tab` cycles).
- **Claude's to-do list** (TodoWrite) shows as a checklist (☐ ◐ ☑).
- **Images:** a message the user types that names an image file (`@shot.png`, a dragged-in path) sends it as an image;
  `ctrl-v` saves the clipboard's image to a private temp folder and names it in the message. Only typed messages attach:
  after a security review, text tazuna sends on its own (test failures, reviewer findings, `!cmd` output, `--run`
  prompts) stays text, because claude's or a command's output could name any file and bypass the permission mode.
- **`!cmd`** runs a command in the repo, shows the output and hands it to claude.

## 0.0.18 (2026-10-08)

Daily-driver features (no benchmark change: headless runs keep the "auto" mode and behave as before).

- **Permission modes** (`--mode`, `shift-tab` cycles live): **auto** (never asks, the default), **ask** (before edits and
  commands), **edits** (edits go ahead, commands ask), **plan** (read-only; the plan appears in the transcript and
  approving it switches to edits). claude routes its questions to tazuna (`--permission-prompt-tool stdio`); the
  composer shows "allow Write: b.txt  (always = this tool)   y yes · a always · n no". A stop answers open questions with no.
  Hardened after a security review of the first version: the question owns the keyboard and ignores keys for 600 ms after
  it appears, so typing cannot approve by accident, and Enter no longer means yes; the whole action is shown (every line
  of a command, control characters made visible, cuts marked) so a first line cannot hide the rest; "always" covers one
  exact command for shells and the tool otherwise, and the question says which; questions from a process that died are
  dropped instead of being answered later. A second review pass: the question on screen is now drawn from the same queue
  the answer applies to (a stop or a dead process can no longer leave a stale question showing), bidirectional overrides
  and zero-width characters are made visible too (in the plan text as well), and ctrl-c/ctrl-d still stop or quit while a
  question is open (the stop answers it with no).
- **Input editing:** cursor movement, `ctrl-a/e/u/k/w`, `alt-backspace`, and message history on `↑↓`, kept across
  sessions. `esc` stops a run when nothing is typed. Expand moved from `ctrl-e` to `ctrl-o` (Claude Code's detail key).
- **Resume:** `tazuna -c` continues this repo's latest session, `--resume <id>` a given one, with the earlier transcript
  on screen.
- **Subagents:** their messages stay under the call that started them ("12 steps · Read src/a.ts") instead of mixing into
  the main transcript.

## 0.0.17 (2026-10-08)

**repo-lean reads before it relies on a name.** Recorded runs showed why repo-lean scored lowest: with Claude Code's own
tools and the short prompt it read ~25% less code than Claude Code's own prompt (150k vs ~197k chars, median over 10
company-ticket runs) and ran the type check less (2 vs 3); on T-921 it read 102k vs 118-167k and the judge flagged a
guessed database column. Two lines now go into repo-lean's prompt (~60 tokens): read the code you change, its callers and
the definition of every name you rely on; run the type check and the relevant tests before finishing.

T-921 + T-870, 2 runs each, side pool of 12 per ticket judged together, Opus 5.5 medium:

| Arm | 921 | 870 | Judge avg | Cost (2 tickets, medians) |
|---|---|---|---|---|
| tazuna repo profile + diagnostics (0.0.12, the default since 0.0.13) | 7.0, 7.0 | 6.5, 8.0 | **7.12** | $4.53 |
| tazuna repo profile, no diagnostics (0.0.12) | 6.5, 7.0 | 7.5, 7.0 | 7.00 | $5.42 |
| stock Claude Code | 6.0, 7.0 | 7.5, 6.0 | 6.62 | $4.63 |
| **tazuna repo-lean 0.0.17** | 6.5, 5.5 | 7.0, 7.0 | **6.50** | $4.62 |
| tazuna lean | 5.5, 6.0 | 6.5, 7.0 | 6.25 | $3.82 |
| tazuna repo-lean 0.0.11 | 6.0, 6.0 | 6.0, 5.5 | 5.88 | $3.55 |

The two lines lifted repo-lean by 0.62 to stock level, by reading more, and its cost rose to stock's. It no longer has an
edge: the default (repo profile with diagnostics) scored higher at about the same cost. repo-lean stays opt-in.

## 0.0.16 (2026-10-08)

**Background commands.** When claude starts a command in the background and ends its turn, it wakes up again once the
command finishes. tazuna now tracks claude's running commands: the session is not idle (and `--run` does not exit) until
they are done and claude has had 5 s to start the turn it wakes up for. That turn is logged as a run of its own (prompt
"(claude continued on its own …)") with its cost, and `--run` output sums every run of the session (`tazuna.runs`).
Before, `--run` exited at the first result, cutting claude off where `claude -p` would have continued; none of the 25
tazuna runs in the cross-harness table used a background command, so those numbers stand.

**UI.** Multi-line input (the composer grows to 6 rows; `ctrl-j` everywhere, shift-enter where the terminal reports it);
`ctrl-s` hands the mouse to the terminal to select text; cache misses next to the Cockpit's cache gauge; Claude Code's own
Edit/Write show as a diff like the lean tools (they showed raw JSON); the session panel shows the real profile.

**Tier 0** now also measures startup to first frame (6.4 ms), UI CPU and memory while animating (1.6%, 7.1 MB) and the
tool server's memory (2.1 MB); stock Claude Code's UI: 190 MB, 2.2% idle, 0.45 s to first output.

## 0.0.15 (2026-10-08)

**`--review-model M [--review-effort E]`: an independent reviewer** (opt-in until benchmarked). When claude first says it is
done and the run changed files, a separate `claude -p` on model M reads the task and the diff (new files in full) in a fresh
context, with read-only tools, and answers `ok` or a list of concrete problems. Problems go back to the implementer once;
then verification runs. The reviewer's cost is added to the run (`total_cost_usd` in `--run` output is now tazuna's own
total for the run, every process included). Meant for a cheap implementer with a strong reviewer, e.g. Haiku 5.5 implements
and Opus 5.5 reviews. Smoke test (Haiku 5.5 low + Opus 5.5 low, a two-function fix): review in 2-5 s for $0.006-0.027.

Gauge on one company ticket and one Go task (2 runs each, all arms the same hour, judged in side pools):

| Arm | T-921 judge | T-921 cost | hugo-section-slug judge | hugo cost | Turns |
|---|---|---|---|---|---|
| Opus 5.5 medium (default) | 5.75-6.75 (earlier same-day runs) | $1.4-2.6 | **8.00** | $0.56 | 16-45 |
| Haiku 5.5 high + Opus 5.5 medium reviewer | 5.00 | $1.06 | 6.75 | $0.71 | 60-71 |
| Haiku 5.5 high alone | 4.50 | $0.45 | 7.00 | $0.26 | 44-67 |

The reviewer flagged a problem in 1 of 4 runs and passed implementations the judge called "diverges most from the spec";
each review cost $0.25-0.48 (Opus reading code), more than a Haiku run. Haiku 5.5 needs 2-3× the turns of Opus and
~25 min on the company ticket. Verdict: Haiku + Opus review is not competitive with Opus medium alone (lower score on both
tasks; on hugo also more expensive). `--review-model` stays opt-in. Usage: 10 runs and both judges, 37% → 46% of the 5-hour
window.

## 0.0.14 (2026-10-08)

**`--self-review`** (opt-in until benchmarked): three sentences of finish rules in the system prompt (finish only when every
requirement is met and checked; do not stop to ask; read a name before relying on it), and one review turn when claude first
says it is done and the run changed files: read the whole diff, map every acceptance criterion to code, revert what the task
did not ask for, check unread names, re-run tests. The review comes before verification, so its fixes are tested too.

A/B on T-921 and T-870 (the two company tickets whose cause the ticket names), repo profile with diagnostics, Opus 5.5
medium, 2 runs per arm per ticket, both arms the same morning, each ticket's whole pool re-judged:

| Arm | 921 | 870 | Judge avg | Median cost (sum of 2 tickets) | Turns (median) |
|---|---|---|---|---|---|
| tazuna 0.0.14 `--self-review` | 7.5, 7.0 | 7.0, 6.5 | **7.00** | $5.36 (**+31%**) | 45 |
| tazuna 0.0.14 without | 6.0, 6.0 | 7.5, 7.0 | 6.62 | $4.10 | 38 |

+0.38 judge (within the 0.75 noise threshold) for +31% cost. On T-921 the review lifted correctness (4.5 vs 3.5-4: the
judge called both reviewed runs "close to the reference"); on T-870 it scored 0.5 lower, one run extending into frontend
code the ticket scoped out. Earlier arms in the same re-judged pools moved by up to 0.5. Decision: stays opt-in. The 8 runs
and judging used 25 points of the 5-hour window.

## 0.0.13 (2026-10-08)

**Diagnostics after each edit are on by default** (`--no-diagnostics` turns them off), after this A/B on the five company
tickets (acme/app, Opus 5.5 medium, repo profile, 2 runs per arm per ticket, both arms the same evening, every
ticket's runs judged as one pool):

| Arm | 872 | 921 | 946 | 870 | 698 | Judge avg | Total cost (median per ticket) | New type errors |
|---|---|---|---|---|---|---|---|---|
| **tazuna 0.0.12 `--diagnostics`** | 5.00 | 6.25 | 3.50 | 7.00 | 7.00 | **5.75** | **$12.66 (−11%)** | 0 / 10 |
| tazuna 0.0.12 without | 5.00 | 6.25 | 3.50 | 7.00 | 7.00 | 5.75 | $14.18 | 0 / 10 |
| stock Claude Code (previous afternoon) | 5.00 | 7.00 | 4.50 | 6.50 | 7.00 | 6.00 | $13.19 | 0 / 10 |

Identical judge scores on every ticket; 11% cheaper and ~12% fewer turns, below the 15% cost threshold, so suggestive rather
than proven, at no quality cost. No run broke the type check, so the gain is errors caught right after the edit instead of
by the agent's own later type-check runs. The 20 runs and judging took the 5-hour window from 45% to 100%.

Tier 0: binary 2.52 MB (+84 KB); input tokens per turn unchanged (2,983; the hook adds nothing to the prompt).

## 0.0.12 (2026-10-07)

**`--diagnostics`: compiler errors right after an edit** (opt-in until benchmarked). tazuna adds a `PostToolUse` hook to
claude (`--settings`, so it runs beside the repo's own hooks, in every profile). After any tool call that changed files,
including shell commands (agents make most edits with `python3`/`sed` through Bash), it runs the repo's `typecheck` script
or `go vet` on the changed Go packages and tells claude about errors the change added, once, while the edit is fresh.
Errors the repo already had when the session started are left out (same comparison as verification).

## 0.0.11 (2026-10-07)

**`--profile repo-lean`.** The lean prompt with Claude Code's own tools (`Bash`, `Read`, `Edit`, `Write`, `Grep`, `Glob`,
`Skill`) and the repo's settings, hooks, skills, plugins and `CLAUDE.md`: the repo's rules at close to lean cost. It uses
Claude's tools, not the lean MCP tools, because repo hooks match Claude's tool names (`Write|Edit`) and would never fire on
`mcp__lean__edit`; the bash output cap does not apply. `AGENTS.md`, which Claude Code does not read, goes into the prompt
unless `CLAUDE.md` imports it. Not the default yet: it needs the real-work benchmark first.

**Fix: the repo profile loaded the user's claude.ai connectors.** They come with the account, not a settings file, so
`--setting-sources project,local` did not keep them out: in a personal setup with Atlassian, Slack and Claude Docs
connected, every turn carried 76 extra tool schemas (~42k tokens), and the first tool call broke the prompt cache when they
finished connecting. Both repo profiles now pass `--strict-mcp-config` and load only the repo's `.mcp.json`
(`--user-settings` keeps everything). Benchmarks ran on an empty config and were not affected.

Input tokens per turn, probe repo with CLAUDE.md, one skill, one hook and a `.mcp.json` server (Haiku 4.5, claude 2.1.292):

| Profile | Tokens per turn | Hook fired | Skill used | Repo MCP server |
|---|---|---|---|---|
| repo, 0.0.10, personal setup | ~51,900 | yes | yes | — |
| repo, 0.0.11 | 21,763 | yes | yes | yes |
| **repo-lean, 0.0.11** | **9,724** | yes | yes | yes |
| lean | 2,977 | no | no | no |

Tier 0: binary 2.43 MB (+3 KB), lean 2,977 input tokens per turn.

**Real work, repo-lean vs a same-day stock baseline** (five acme/app tickets, 2 runs per arm per ticket, Opus
5.5 medium, all runs of a ticket judged as one pool; repo-lean and stock ran 15:00-16:10 the same day):

| Arm | 872 | 921 | 946 | 870 | 698 | Judge avg | Total cost (median per ticket) |
|---|---|---|---|---|---|---|---|
| stock Claude Code, same day | 5.50 | 6.75 | 5.00 | 6.75 | 7.75 | **6.35** | $13.20 |
| stock Claude Code, earlier | 5.00 | 6.25 | 4.50 | 6.75 | 7.50 | 6.00 | $13.38 |
| tazuna lean (0.0.9) | 4.25 | 6.25 | 5.00 | 7.00 | 7.50 | 6.00 | $10.88 |
| tazuna repo (0.0.9) | 5.00 | 6.25 | 4.50 | 6.00 | 7.00 | 5.75 | $12.66 |
| cc-mid via `claude -p` | 4.75 | 6.25 | 3.50 | 7.25 | 6.00 | 5.55 | $10.98 |
| **tazuna repo-lean (0.0.11)** | 5.50 | 5.00 | 4.50 | 6.25 | 6.00 | **5.45** | **$11.62 (−12% vs same day)** |

repo-lean is 12% cheaper than same-day stock (cheaper on 4 of 5 tickets) but scores 0.90 lower, beyond the 0.75 noise
threshold, and lowest of the tazuna arms. The judge's notes point at unchecked assumptions (a guessed database column in
T-921, swallowed lookup errors in T-946), not at repo rules. Lean, with the same prompt, scored 6.00, so the drop comes
from the combination of Claude's tools, its CLAUDE.md loading and the repo's hooks with the short prompt; not isolated yet.
Decision: repo-lean stays opt-in; the repo profile stays the default. Stock Claude Code itself moved 0.35 judge and up to
±20% cost per ticket between two batches on the same and the previous day. The 16 runs and judging used 26 points of the
5-hour window and 5 points of the weekly limit.

## 0.0.10 (2026-10-07)

**Verification checks types.** When the repo has a `typecheck` (or `type-check`) script, verification runs it beside the
tests and fails only on type errors the run added: the check runs once when the run starts and again at the end, and errors
are compared without line and column, so known errors that moved are not new. Test runners like `tsx` strip types, so
before this a run that broke the type-checked build still passed as "verified". Monorepos without a root test script now
run the tests of the packages whose files changed (`npm --prefix <pkg> test`) instead of skipping verification.

**Fixes.** Parallel runs garbled `runs.jsonl` (each line was written in pieces); every line is now one write, also in
`labels.jsonl`. Sessions started in the same second in same-named directories wrote one recording; the file name now ends in
the process id.

Real work, five acme/app tickets (T-872, 921, 946, 870, 698), 2 runs per arm per ticket, Opus 5.5 medium,
each ticket judged as one pool. Arms ran 0.0.9:

| Arm | Judge avg | Total cost (median per ticket) | New type errors |
|---|---|---|---|
| tazuna lean | 5.65 | $10.88 (−19%) | 0 / 10 runs |
| stock Claude Code | 5.55 | $13.38 | 0 / 10 |
| cc-mid via `claude -p` | 5.50 | $10.98 (−18%) | 7 in 1 / 10 |
| tazuna repo profile | 5.35 | $12.66 (−5%) | 0 / 10 |

Quality is tied (all within 0.3). The lean profiles are consistently ~18% cheaper; cc-mid was cheaper than stock on 5 of 5
tickets. On two tickets (T-872, T-946) no arm found the real root cause.

0.0.10 vs 0.0.9 on T-698 (all 12 runs re-judged together):

| Arm | Judge | Median cost | Verification time | New type errors |
|---|---|---|---|---|
| tazuna lean 0.0.9 | 6.75 | $2.79 | 97-101 s | 0 |
| **tazuna lean 0.0.10** | **7.00** | **$3.10** | **65-73 s** | **0** |
| tazuna repo 0.0.9 | 5.75 | $3.17 | 96-103 s | 0 |
| **tazuna repo 0.0.10** | **6.50** | **$3.08** | **72-75 s** | **0** |

All differences are within noise; the type check adds no time (it runs beside the tests; the lower times came from 4
parallel runs instead of 8). No 0.0.10 run broke the build, so the check had nothing to catch here. Replayed on cc-mid's
broken T-698 diff, it reports exactly the 7 new errors against the repo's 361 known ones. Re-judging moved the 0.0.9
arms by up to 0.75, the noise threshold.

Tier 0: binary 2.43 MB (+20 KB), 2,983 input tokens per turn (unchanged), claude 2.1.292.

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
