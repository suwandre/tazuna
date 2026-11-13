from opslib import *
start()
branch("feat/retry-jitter")
edit("packages/query-core/src/retryer.ts", "function defaultRetryDelay(failureCount: number) {\n  return Math.min(1000 * 2 ** failureCount, 30000)",
     "function defaultRetryDelay(failureCount: number) {\n  // up to 10% jitter, so clients that failed together do not retry together\n  const jitter = 1 + Math.random() * 0.1\n  return Math.min(1000 * 2 ** failureCount * jitter, 30000)")
commit("feat(query-core): add jitter to the default retry delay", author="dev")
edit("docs/framework/react/guides/important-defaults.md", "garbage collected after **5 minutes**.", "garbage collected after **5 minutes**.\n- Failed queries retry with an exponential delay that has up to 10% jitter.")
commit("docs: mention the retry jitter", author="dev")
p = pr(0, "feat/retry-jitter", "")
entries = [e for e in pr_entries([p], 120) if e["match"][:2] in (["pr", "create"], ["repo", "view"])]
entries.append(dict(match=["pr", "list"], json=[], text=""))
entries.append(dict(match=["pr", "view"], text="no pull requests found for branch \"feat/retry-jitter\"", exit=1))
finish(entries, on="feat/retry-jitter", head=git("rev-parse", "feat/retry-jitter"))
