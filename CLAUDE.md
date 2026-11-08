# tazuna

Read ROADMAP.md before proposing or starting work: it holds the two goals (small footprint with a clean UI; token
efficiency with output quality, measured against other harnesses), the order of work, and the ideas already tried or ruled
out. A change of direction gets a line in its decision log, with the reason. docs/WORKING.md holds the working habits
(cost, git, benches, machines).

- Never impersonate Claude Code: only the official `claude` CLI, no copied headers, no reuse of its OAuth token.
- Every change that can move a metric is measured; CHANGELOG.md records the numbers (only for versions that change a
  measurable metric). Tiers and noise thresholds are defined there.
- Benchmarks run with the usage guard (`bench/guard.py`; see bench/README.md): limits spill silently into paid extra usage.
- Commits and PRs carry no Claude attribution (no Co-Authored-By trailer, no "Generated with Claude Code" line).
- The repo is public (since 2026-11-08): nothing from the user's employer goes into it. No company or repo names, ticket
  keys, vendor or code names from company work; write "a company repo", T-123, a generic stand-in. Company bench material
  stays in the gitignored bench/company/. The original private history is in suwandre/tazuna-private-archive.
