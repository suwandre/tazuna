from opslib import *
start()
# a docs-only branch on one topic, the retry delay (2026-11-14: the first version added jitter to defaultRetryDelay,
# which broke an existing exact-delay test and lacked the changeset the repo's CONTRIBUTING.md asks for; 5 of 6 runs
# rightly stopped to ask and failed the check)
branch("docs/retry-delay")
edit("docs/framework/react/guides/important-defaults.md", "with exponential backoff delay** before",
     "with exponential backoff delay** (doubling from 1 second, capped at 30 seconds) before")
commit("docs: spell out the default retry delay", author="dev")
edit("docs/framework/react/guides/query-retries.md", "with each attempt, but not exceed 30 seconds:", "with each attempt, up to a cap of 30 seconds:")
commit("docs: call the 30 second retry delay limit a cap", author="dev")
p = pr(0, "docs/retry-delay", "")
entries = [e for e in pr_entries([p], 120) if e["match"][:2] in (["pr", "create"], ["repo", "view"], ["auth", "status"])]
entries.append(dict(match=["pr", "list"], json=[], text=""))
entries.append(dict(match=["pr", "view"], text="no pull requests found for branch \"docs/retry-delay\"", exit=1))
finish(entries, on="docs/retry-delay", head=git("rev-parse", "docs/retry-delay"))
