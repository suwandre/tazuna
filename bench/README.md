# Benchmarks

- `tier0.sh`: every change. Binary size, replay render time, tokens per turn, startup, UI CPU and memory.
- `agent_footprint.py <binary> [flags]`: peak RAM and CPU of a whole headless agent run (tazuna plus the claude CLI or the
  own loop's engine), on one small fix; compares the claude path with the own loop.
- `realwork/`: real-work tasks with hidden tests from the merged PR. `tasks/` (6 Go fixes), `tasks-hard/` (4), `tasks-oct/`
  (7 merged 1-10 October 2026, after DeepSeek V4.1's release). Each task: `prompt.txt`, `git.json` (repo and base commit,
  cloned at run time), `hidden/` tests, `solution.patch` (the reference fix).
  - `run.py --harness H@MODEL:EFFORT --tasks a,b --runs N --workers 8` runs the agents. `BENCH_SET=tasks-oct` picks the set.
    Results go to `results[-SET].jsonl` and diffs to `runs[-SET]/`.
  - `judge.py`: blind Opus 5.5 xhigh judge over the saved diffs. One pass by default; `--passes 2` for results a default
    rests on. `--dry-run` first: it prints the number of judge calls. `JUDGE_ONLY=harness[-rN]`, `JUDGE_NOREF=1`.
  - `calibrate.py`: does a cheap judge rank like Opus (Experiential Labs balance, no Claude usage).
- `guard.py`: start it before any run. It polls the subscription meter, pauses new runs at `GUARD_PAUSE_AT`, kills agents
  at `GUARD_KILL_AT` and on any rise in extra-usage credits. The judge refuses to run without a fresh guard reading.

Tokens stay outside the repo: `~/.bench-token` (`CLAUDE_CODE_OAUTH_TOKEN`), `~/.ci-key` (CheaperInference),
`~/.el-key` (Experiential Labs). Company tickets (`company/`) and cloned plugins (`addons/`) are never committed.
