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

