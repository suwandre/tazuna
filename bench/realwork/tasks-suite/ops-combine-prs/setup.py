from opslib import *
start()
branch("fix/retry-delay-cap")
edit("packages/query-core/src/retryer.ts", "function defaultRetryDelay(failureCount: number) {\n  return Math.min(1000 * 2 ** failureCount, 30000)",
     "const MAX_RETRY_DELAY = 30_000\n\nfunction defaultRetryDelay(failureCount: number) {\n  return Math.min(1000 * 2 ** failureCount, MAX_RETRY_DELAY)")
commit("refactor(query-core): name the retry delay cap"); push("fix/retry-delay-cap")
branch("docs/gc-default")
edit("docs/framework/react/guides/important-defaults.md", "garbage collected after **5 minutes**.", "garbage collected after **5 minutes** (`gcTime: 5 * 60 * 1000`).")
commit("docs: show the gcTime default"); push("docs/gc-default")
prs = [pr(101, "fix/retry-delay-cap", "refactor(query-core): name the retry delay cap", "Names the 30s cap in defaultRetryDelay."),
       pr(102, "docs/gc-default", "docs: show the gcTime default", "Spells out the default gcTime in Important Defaults.")]
finish(pr_entries(prs, 103), heads={p["headRefName"]: git("rev-parse", p["headRefName"]) for p in prs})
