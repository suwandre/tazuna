from opslib import *
start()
branch("fix/gc-time-constant")
edit("packages/query-core/src/utils.ts", "// UTILS\n", "// UTILS\n\n/** How long unused cache entries are kept on the client by default. */\nexport const DEFAULT_GC_TIME = 5 * 60 * 1000\n\n")
edit("packages/query-core/src/removable.ts", "import { isValidTimeout } from './utils'", "import { DEFAULT_GCTIME, isValidTimeout } from './utils'")
edit("packages/query-core/src/removable.ts", "(isServerEnvironment() ? Infinity : 5 * 60 * 1000)", "(isServerEnvironment() ? Infinity : DEFAULT_GCTIME)")
commit("refactor(query-core): one constant for the default gcTime")
push("fix/gc-time-constant")
p = pr(77, "fix/gc-time-constant", "refactor(query-core): one constant for the default gcTime", "Replaces the inline 5 minute default with DEFAULT_GC_TIME.", checks="FAILURE")
LOG = """typecheck\tRun pnpm run test:types\t2026-11-12T09:14:03.118Z > tsc --build
typecheck\tRun pnpm run test:types\t2026-11-12T09:14:31.902Z src/removable.ts(3,10): error TS2724: '"./utils"' has no exported member named 'DEFAULT_GCTIME'. Did you mean 'DEFAULT_GC_TIME'?
typecheck\tRun pnpm run test:types\t2026-11-12T09:14:31.951Z  ELIFECYCLE  Command failed with exit code 2.
typecheck\tRun pnpm run test:types\t2026-11-12T09:14:32.004Z ##[error]Process completed with exit code 2."""
run = dict(databaseId=9001, name="CI", displayTitle=p["title"], headBranch="fix/gc-time-constant", status="completed", conclusion="failure", url=f"https://github.com/{REPO}/actions/runs/9001")
checks = [dict(name="typecheck", state="FAILURE", bucket="fail", link=f"https://github.com/{REPO}/actions/runs/9001/job/1"),
          dict(name="test", state="SUCCESS", bucket="pass", link=f"https://github.com/{REPO}/actions/runs/9001/job/2")]
entries = [dict(match=["pr", "checks", "77"], json=checks, text="typecheck\tfail\t31s\thttps://github.com/acme/query/actions/runs/9001/job/1\ntest\tpass\t2m4s\thttps://github.com/acme/query/actions/runs/9001/job/2", exit=1),
           dict(match=["run", "list"], json=[run], text="X  refactor(query-core): one constant for the default gcTime  CI  fix/gc-time-constant  pull_request  9001  31s  1h"),
           dict(match=["run", "view", "--log"], text=LOG), dict(match=["run", "view", "--log-failed"], text=LOG),
           dict(match=["run", "view"], json=dict(run, jobs=[dict(databaseId=1, name="typecheck", conclusion="failure"), dict(databaseId=2, name="test", conclusion="success")]),
                text="X fix/gc-time-constant CI · 9001\nJOBS\nX typecheck in 31s (ID 1)\n✓ test in 2m4s (ID 2)\n\nTo see the logs for the failed steps, try: gh run view --log-failed --job=1")]
finish(entries + pr_entries([p], 78), branch_head=git("rev-parse", "fix/gc-time-constant"))
