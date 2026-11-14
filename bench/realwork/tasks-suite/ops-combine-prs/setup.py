from opslib import *
start()
branch("fix/retry-delay-cap")
edit("packages/query-core/src/retryer.ts", "function defaultRetryDelay(failureCount: number) {\n  return Math.min(1000 * 2 ** failureCount, 30000)",
     "const MAX_RETRY_DELAY = 30_000\n\nfunction defaultRetryDelay(failureCount: number) {\n  return Math.min(1000 * 2 ** failureCount, MAX_RETRY_DELAY)")
commit("refactor(query-core): name the retry delay cap"); push("fix/retry-delay-cap")
# both PRs are about the retry delay's cap (2026-11-14: the first version paired it with an unrelated gcTime doc line, which
# the repo's CONTRIBUTING.md forbids combining; agents that stopped to ask were right and failed the check)
branch("docs/retry-delay-cap")
edit("docs/framework/react/guides/query-retries.md", "with each attempt, but not exceed 30 seconds:", "with each attempt, up to a cap of 30 seconds:")
commit("docs: state the retry delay cap"); push("docs/retry-delay-cap")
prs = [pr(101, "fix/retry-delay-cap", "refactor(query-core): name the retry delay cap", "Names the 30s cap in defaultRetryDelay."),
       pr(102, "docs/retry-delay-cap", "docs: state the retry delay cap", "Says plainly that the default retry delay is capped at 30 seconds.")]
finish(pr_entries(prs, 103), heads={p["headRefName"]: git("rev-parse", p["headRefName"]) for p in prs})
