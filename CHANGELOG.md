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

About $0.06 per million input tokens, but every re-sent token is billed in full: agent loops cost 20x and more. Not
used.

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
