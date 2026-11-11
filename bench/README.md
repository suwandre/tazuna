# Benchmarks

- `tier0.sh`: every change. Binary size, replay render time, tokens per turn, startup, UI CPU and memory.
- `agent_footprint.py <binary> [flags]`: peak RAM and CPU of a whole headless agent run (tazuna plus the claude CLI or the
  own loop's engine), on one small fix; compares the claude path with the own loop.
- `realwork/`: real-work tasks with hidden tests from the merged PR. `tasks/` (6 Go fixes), `tasks-hard/` (4), `tasks-oct/`
  (7 merged 1-10 October 2026, after DeepSeek V4.1's release). Each task: `prompt.txt`, `git.json` (repo and base commit,
  cloned at run time), `hidden/` tests, `solution.patch` (the reference fix).
  - `run.py --harness H@MODEL:EFFORT --tasks a,b --runs N --workers 8` runs the agents. `BENCH_SET=tasks-oct` picks the set.
    Results go to `results[-SET].jsonl` and diffs to `runs[-SET]/`.
  - `judge.py`: blind Fable 5.2 high judge over the saved diffs (Opus 5.5 xhigh until 2026-11-11; scores compare within a round). One pass by default; `--passes 2` for results a default
    rests on. `--dry-run` first: it prints the number of judge calls. `JUDGE_ONLY=harness[-rN]`, `JUDGE_NOREF=1`.
  - `calibrate.py`: does a cheap judge rank like Opus (Experiential Labs balance, no Claude usage).
- `model_check.py`: the daily model check (ROADMAP Next 5). `roles.json` holds who does what, why, and how each kind of
  role weighs benchmark categories. Sources: Artificial Analysis (key in `ARTIFICIAL_ANALYSIS_API_KEY`) and Epoch AI (no
  key, CC BY 4.0), ~1,000 models and ~80 benchmarks; each benchmark is scaled from random (0) to ceiling (1) and put in a
  category (agentic coding, coding, tool use, instruction following, long context, reasoning, math, knowledge; saturated
  or unrelated ones are ignored on purpose). A candidate is compared with the pick only on shared benchmarks, at the
  pick's effort (cheap models at max), at most 10% dearer per task (our own runs in `realwork/results*.jsonl` first,
  then Epoch's cost-per-task columns, then the token price: MiMo V2.6 Pro was 1.3x DeepSeek on Epoch's numbers and 33x
  on our loop); a flag needs 3 shared benchmarks and a lead over 0.03. A flag
  starts a gauge run on our benchmark; only that may change a pick. `--why NAME` shows a model's scores.
  `model_check_daily.sh install` runs it daily at 9:00 (launchd, from a copy in `~/.local/share/tazuna/model-check`,
  since macOS keeps background jobs out of ~/Desktop; rerun install after changes) with a notification on new flags.
  `test_model_check.py` checks the scoring offline.
- `guard.py`: start it before any run. It polls the subscription meter, pauses new runs at `GUARD_PAUSE_AT`, kills agents
  at `GUARD_KILL_AT` and on any rise in extra-usage credits. The judge refuses to run without a fresh guard reading.

Tokens stay outside the repo: `~/.bench-token` (`CLAUDE_CODE_OAUTH_TOKEN`), `~/.ci-key` (CheaperInference),
`~/.el-key` (Experiential Labs). Company tickets (`company/`) and cloned plugins (`addons/`) are never committed.
