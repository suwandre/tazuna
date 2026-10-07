# tazuna

Read ROADMAP.md before proposing or starting work: it holds the two goals (small footprint with a clean UI; token
efficiency with output quality, measured against other harnesses), the order of work, and the ideas already tried or ruled
out. A change of direction gets a line in its decision log, with the reason.

- Never impersonate Claude Code: only the official `claude` CLI, no copied headers, no reuse of its OAuth token.
- Every change that can move a metric is measured; CHANGELOG.md records the numbers (only for versions that change a
  measurable metric). Tiers and noise thresholds are defined there.
- Benchmarks run with the usage guard (`cc-lean/guard.py`): limits spill silently into paid extra usage.
- Commits and PRs carry no Claude attribution (no Co-Authored-By trailer, no "Generated with Claude Code" line).
