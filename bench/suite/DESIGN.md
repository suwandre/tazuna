# A suite that mirrors real use (design, 2026-11-12)

The real-work bench so far is one kind of task: implement a merged Go PR, hidden tests decide. An analysis of 517 live
tazuna runs (the user's own, 390 messages after Claude's self-wake-ups) found that kind in about 2% of messages. The rest:

| kind | share of messages | what it is |
|---|---|---|
| follow-up steering | 22% | "sure, go ahead", "post them as inline comments", "same PR, not a new one" after a review or plan |
| question | 16% | how or why the code does something, TLDR of a PR, "is this script still used?" |
| review | 12% (36% of spend) | is this PR good to approve, blockers, are a colleague's comments right |
| plan | 9% | trade-offs and a recommendation, without changing code |
| ops | 8% | open or combine PRs, rebase, CI and deploy checks, tickets |
| investigate | 6% | why CI is slow or failing, which step regressed, is a reported bug real |
| change | 2% asked directly, ~8% of runs edit | fixes and features, often CI YAML, Dockerfiles, infra, in a TypeScript monorepo |

Claude-path runs went badly (corrected, stopped, errored) in 4% of messages; own-loop runs in 24%, mostly free models
hanging or erroring. The suite has to measure all of it, so that tazuna is at least as good as other harnesses on any
ask, not tuned to one task.

## Shape

- **Mix** (by task count): review 25%, question 20%, change 15%, investigate 15%, plan 10%, ops 10%, clarifying
  follow-up 5%. About a quarter of tasks get a scripted second turn (approve, redirect, narrow the scope).
- **Difficulty**: 50% easy, 40% medium, 10% hard, plus a few improbable asks (ambiguous, impossible, should ask or say
  no) graded on whether the agent noticed.
- **Languages**: TypeScript first (where the real work is), then Python, Go and Rust.
- **Later, general chat**: today tazuna runs from the CLI for work only; a desktop version for everyday chat is planned,
  and then non-coding conversations (explain, write, look something up) join the mix.
- **Two tiers**:
  - *public*: open-source repos, PRs merged after the models' release, results go into the README;
  - *private*: tasks replayed from the user's own sessions on the work repo at the commit they were asked at, in the
    gitignored `bench/company/` (company tickets and reviews already live there). Never published; the best mirror of
    real use.
- **Size**: about 36 public tasks to start, one run per harness per task (differences between tasks outweigh repeats of
  one), more runs only where a comparison is close.

## Grading per kind

| kind | ground truth | how it is scored |
|---|---|---|
| change | the PR's own tests, hidden | hidden tests, then the blind judge for scope and quality (as now) |
| review | a PR whose review or follow-up commits found real defects (location + what) | findings matched against the known ones: recall, precision, false blockers count against |
| question | an answer checked against the code (files and lines it rests on) | judge with the reference answer; a confident wrong claim scores below "not sure" |
| investigate | a real failing CI run or bug with its fixing commit | root cause named and located; judge with the reference |
| plan | the maintainers' chosen approach and the trade-offs in the PR or issue discussion | rubric (options, trade-offs, a recommendation that fits the repo) plus judge |
| ops | the end state | git state checked by a script; GitHub calls go to a recording `gh` stand-in, checked after |
| follow-up | the first turn's answer | consistency with it, and doing what the second turn asks (same PR, inline comments) |

The judge sees every candidate for a task together, blind, with the reference (Fable 5.2 high since 2026-11-11).

## Against tuning to the suite

- A third of the tasks are **held out**: nobody looks at them while improving tazuna. A change ships only if it helps on
  the rest and does not hurt on those.
- Results are reported **per kind**, never only pooled: a weak kind cannot hide in an average.
- Prompt rules that mention a task's specifics are not accepted; harness mechanics (speed, cost, streaming, stalls,
  provider errors, context, verification) are where tazuna should win on every kind.
- Leak checks as in the real-work bench: no web for agents, the repo history hidden, a build cache per run, leakscan's
  hidden-test names (and, for review and investigate tasks, the reference's distinctive strings).

## Real use as a running score

Beside the suite, the user's own sessions give a free, continuous measure per tazuna version and model, with no labels
asked for: runs stopped, rewound or crashed; the next message correcting the last answer ("no", "that's wrong", "not
what I asked"); a question asked again; edits reverted in git. It catches regressions the suite cannot see.

## Order

1. Pick the public repos (TypeScript monorepos with CI, active review and post-release PRs) and mine candidates per kind.
2. Write three pilot tasks per kind; one cheap own-loop run each to check the task works and the grading separates good
   from bad answers.
3. Fill to ~36, split dev / held-out; then the baseline pass across harnesses (own loop on SubRouter about $2-3; the
   Claude path and the judge on the plan, after a weekly reset).

## Pilots (2026-11-12)

One own-loop run each (DeepSeek V4.1 Flash high on SubRouter), Fable 5.2 high judging against reference.md.

| kind | tasks | scores (of 10) | run time | cost |
|---|---|---|---|---|
| review | 3 (n8n, pydantic; known defects from later fix PRs) | 7, 4, 3 | 10-27 min | a few cents |
| investigate | 3 (Astro bug issues, the fix PR as reference) | 9, 9, 6 | 1-3.5 min | $0.009 for the 3 |
| plan | 3 (Astro bug issues, the maintainers' fix as reference) | 9, 9, 7 | 1-5 min | $0.011 |
| question | 3 ("TL;DR this PR" in n8n, Astro, TanStack Query) | 8, 8, 8 | 1.5-2.5 min | $0.005 |
| ops | 3 (combine PRs, rebase with a conflict, fix CI in the same PR; local origin, gh stand-in) | 9/9, 9/9, 7/7 checks (0.1.116) | 1-7 min | $0.006 |
| follow-up | 3 (plan then "go ahead"; TL;DR then "so X, right?"; find why CI fails then "same PR"), two messages in one session | 1/1 test, 9/10, 7/7 checks | 7-41 min | $0.075 |
| change | 3 (Astro bug fixes; the PR's unit tests hidden, failing on the base and passing with the fix) | 2/2, 1/1, 2/7 tests | 30-41 min | $0.065 |

- The grading separates: the low scores name a concrete wrong claim (a loop said to add hashes that never runs) or a
  recommendation wider than what the maintainers shipped, not style.
- Answer kinds sit high for a cheap model (8-9): the suite needs its harder half (multi-turn, bug reports that do not
  name the cause, larger PRs) to leave room between harnesses. Review is already hard.
- The judge sees the reference, not the repo, and says so when it cannot check a cited file:line. A mechanical check
  of cited paths and lines against the tree would close that gap.
- Change tasks (build_change.py, validate_change.py, nodetest.py): 7 of 8 candidates had tests that fail on the base and
  pass with the fix. The 2/7 run rejected valid paths (an encoded `/`) that the PR keeps passing: the hidden tests caught
  a real regression. That run also "fixed" unrelated tests that fail only without network (three extra files): scope
  the judge should see. Runs were long (93-160 turns) because the agents ran Astro's integration tests.
- Ops tasks (opslib.py, gh_standin.py, a check.py per task) grade the end state: the remote's branches and the gh calls.
  The first run found a real gap: the own loop's prompt said "Do not commit", so "fix it in that PR" stopped at an
  uncommitted fix (fixed in 0.1.116). Checks count outcomes only: a changeset added beside a change is the repo's
  convention, and finding the cause without opening the CI log is fine. A harder variant should fail where reading the
  code cannot show why (a flaky test, an environment difference).
- Follow-up tasks send a second message into the same session (`tazuna --run P --then F`; run.py passes a task's
  followup.txt) and keep both replies for the judge. The clarifying run corrected a wrong premise in the question
  itself (the values are still made per item, only lazily): the reference was fixed to say so before judging. Other
  harnesses need the same two-message support before the baseline.
- Found in passing: every answer ended with the nudge's "DONE" reply (fixed in 0.1.115); one DeepSeek answer began
  with an unclosed `<thinking>` block written as text (a model quirk, 1 of 9; watch it).

## The suite now (2026-11-13)

34 public tasks in bench/realwork/tasks-suite: review 3, question 7, change 7, investigate 6, plan 4, ops 4, follow-up 3
(two-message). `split.py` holds out 8, drawn only from tasks nobody read closely: two change, two investigate, two
question, one plan, one ops. The pilot tasks are all dev.

- Review is short of its share (3 of a planned 9). The later-fix signal found 14 pairs, and most are not defects of the
  earlier PR (image pins, a docs link the docs team was about to move, a refactor that "does not change the behavior").
  It needs another source: review threads by people, not bots, whose point a later commit in the same PR addressed, on
  more repos and a longer window.
- Each new task gets one cheap own-loop run before it counts, to see that it works; judging waits for a weekly reset.
  suite-fill1 (0.1.119, DeepSeek V4.1 Flash high, $0.195 for 12 runs, no leaks): every answer task answered (1-23 min);
  change 4/4 and 1/1 tests, and 0/1 on change-astro-18511, whose test pins an internal function the issue does not name
  (a hard task, 62 min); ops-open-pr 4/5, where the check wanted the pushed branch to equal the original and the run
  added a commit before pushing: the check now asks that the branch holds the original commits. The gh stand-in also
  records a `--body-file`'s text now (a run used one for the PR description).

## Own-loop comparison, first pass (2026-11-13)

tazuna, OpenCode, pi and OMP on DeepSeek V4.1 Flash high through SubRouter, one run per task, Fable 5.2 high judging
each answer task's four answers together. Incomplete: the SubRouter balance capped the round, so OpenCode and OMP missed
the question and review tasks and pi three tasks; tazuna ran at 0.1.120 (its first pass at 0.1.119 spent its share on
three change tasks, below) and two of its runs read other runs' saved histories and were left out (a jail gap, fixed).

- Change (7, hidden tests), ops (4, end-state checks), code follow-ups (2): every harness passed nearly all; the hard
  change-astro-18651 got 1-3 of 7 from everyone. tazuna 0.1.120: 6 of 7 change tasks fully, ops 4/4, follow-ups 1/1
  and 6/7.
- The 9 answer tasks all four answered (5 investigate, 3 plan, 1 follow-up): OpenCode 8.11, tazuna 7.89, OMP 7.89,
  pi 7.78, within noise. tazuna against pi on the 17 they share: 7.65 to 7.41. Plan was tazuna's lowest kind (5.75 over
  4 against 6.25-7.50): it recommended a broader design than the maintainers' fix twice, as others did once each.
- Speed found the real gap: tazuna 0.1.119 took 26-57 min per Astro change task where the others took 2-11, running
  the whole monorepo's tests and chasing a network test's failure. 0.1.120 fixed that (173 to 71 min on the four dev
  change tasks).
- Leak-seeking: on both pydantic tasks tazuna's DeepSeek searched the whole home folder for the task's name and other
  runs' session ids; the jail hid the repos and transcripts but not 0.1.119's own-sessions folder.

## Own-loop comparison, complete (2026-11-13, evening)

All 34 tasks for every harness (DeepSeek V4.1 Flash high on SubRouter, tazuna 0.1.120), one run each; the 21 answer
tasks judged with all four answers together (Fable 5.2 high).

| kind | tazuna | OpenCode | pi | OMP |
|---|---|---|---|---|
| question (7) | 7.86 | 7.20 (5) | 6.86 | 7.71 |
| investigate (6) | 8.17 | 8.33 | 7.67 | 7.83 |
| follow-up (1) | 8.00 | 8.00 | 7.00 | 9.00 |
| plan (4) | 5.25 | 6.50 | 7.50 | 6.50 |
| review (3) | 3.67 | 5.33 | 3.67 | 5.00 (2) |
| the 18 all four answered | 6.89 | 7.33 | 7.06 | 7.28 |

Efficiency over the 33 tasks all four finished (cost at SubRouter's DeepSeek price, the same for every harness):

| | time | median | fresh input | cached input | output | cost |
|---|---|---|---|---|---|---|
| tazuna | 4.9 h | 5.5 min | 3.9M | 60M | 0.71M | $0.21 |
| OpenCode | 5.2 h | 4.7 min | 6.0M | 76M | 0.67M | $0.26 |
| pi | 5.5 h | 5.1 min | 5.6M | 80M | 0.67M | $0.26 |
| OMP | 5.5 h | 6.2 min | 6.0M | 75M | 0.86M | $0.27 |

tazuna: the cheapest and least total time, level on change, ops and code follow-ups, within noise on answers overall;
weakest on plan (twice: 5.75, 5.25) and review. The next own-loop work.

## Claude-path comparison, first batch (2026-11-13)

Claude Opus 5.5 high on the plan, six dev tasks, one run each: tazuna 0.1.120 (its Claude path, default profile), stock
Claude Code, and OMP (stock, its own Anthropic login).

| task | tazuna | Claude Code | OMP |
|---|---|---|---|
| question (judge) | 9 · 34 s | 8 · 22 s | 8 · 25 s |
| investigate | 9 · 151 s | 9 · 38 s | 8 · 47 s |
| plan | 9 · 75 s | 9 · 39 s | 9 · 36 s |
| review | 7 · 250 s | 8 · 267 s | 6 · 211 s |
| change (hidden tests) | 1/1 · 270 s | 1/1 · 79 s | 1/1 · 57 s |
| ops rebase (checks) | 9/9 · 89 s | 9/9 · 28 s | 9/9 · 37 s |
| judge average (4) | 8.50 | 8.50 | 7.75 |
| total time | 14.5 min | 7.9 min | 6.9 min |
| cost at API prices | $2.32 | $2.78 | $2.63 |

tazuna matches Claude Code on quality at 17% less cost but takes about 1.8x the time: its Claude path hands work to
helper subagents (1-2 minutes of Agent calls in the investigate run). Some of these times (tazuna's investigate and
change, Claude Code's investigate) ran while /tmp was full, so the gap is approximate. The next Claude-path work: is
the delegation worth its time on short asks.

## Explorer rule, hard tasks, and two experiments (2026-11-13, late)

- The Claude path's explorer now takes broad searches only (0.1.121; CHANGELOG has the 12-task numbers). On four hard
  tasks (Opus 5.5 high, judged with Claude Code): light rule judge 8.00, 18.3 min, $3.53; strict delegation 7.00, 18.7
  min, $2.15 (Haiku's summaries cost the pydantic review its defect: 5 against 8); boss 7.33, 20.0 min, $2.75; Claude Code
  7.67, 13.9 min, $4.07. One logic for all difficulties: delegation saves cost on hard tasks only by losing review detail.
- change-astro-18651, the hard change: Claude Code 4/7 once, then 1/7 twice (mean 2.0); tazuna 1, 1, 1; tazuna without
  the cover-all rule 3, 1, 3. No gap to Claude Code; a hint against cover-all on the Claude path, to test on more tasks.
  No run but one fixed the dev base middleware the report does not name.
- TAZUNA_ANSWER_RULES (own loop, a rule for requests that ask for no change), DeepSeek, six dev plan and review tasks x 2:
  plan 7.17 to 7.33, review 4.67 to 5.17, all 5.92 to 6.25. Within noise; stays off. The default's plans scored 7.17 here
  against 5.25 in the comparison round: that weakness was mostly noise. TAZUNA_EXPLORER_STRICT (0.1.120's rule) stays as
  an experiment for measuring delegation.

## Outside coding: the general set (2026-11-14)

bench/realwork/tasks-general: 12 tasks (math, science, law, writing, spreadsheet formulas, software architecture; two
each) in an empty repo, one run each, Fable 5.2 high judging all six answers per task together (SYSTEM_GENERAL).

| | judge | total time | cost (API prices) | input tokens |
|---|---|---|---|---|
| Opus 5.5 alone (`claude -p`, no tools) | 8.42 | 2.5 min | $0.30 | 5k |
| stock Claude Code | 8.33 | 2.8 min | $0.88 | 211k |
| OMP | 8.17 | 2.7 min | $0.57 | 152k |
| tazuna (Claude path) | 7.83 | 2.9 min | $0.41 | 44k |
| DeepSeek V4.1 Flash alone (one call) | 8.25 | 1.6 min | ~$0 | 1k |
| tazuna (own loop, DeepSeek) | 7.92 | 3.0 min | ~$0 | 74k |

- No harness beats the model alone outside coding, as expected; Claude Code comes closest (-0.09), tazuna the furthest
  (-0.59 on Opus, -0.33 on DeepSeek), at the lowest harness cost.
- Coding habits leak: the own loop wrote the essay into a file and answered with a diff (the headless no-change nudge;
  a bench artifact, live has no nudge) and added workspace notes to a GDPR answer; the Claude path ran furthest over
  length limits (600 words for "under 400"; a five-sentence explanation for "one or two"). Every harness broke the
  architecture tasks' word limits, the bare model least.
- For the desktop app's general chat: a chat mode without the coding rules and tools, and limits in the request kept.

