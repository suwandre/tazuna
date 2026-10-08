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
