from opslib import *
s, log = started(), gh_log()
B = "fix/gc-time-constant"
rem = show(B, "packages/query-core/src/removable.ts")
files = set(remote("diff", "--name-only", f"{s['branch_head']}..{B}").split())
# graded on the end state only: a run that found the typo by reading the branch, without asking gh for the CI log,
# fixed it all the same (v1155); the gh calls are kept in ops-checks.jsonl
report(dict(
    fix_pushed_to_the_pr_branch=remote("rev-parse", B) != s["branch_head"],
    import_fixed="DEFAULT_GCTIME" not in rem and "DEFAULT_GC_TIME" in rem,
    constant_still_used="DEFAULT_GC_TIME" in show(B, "packages/query-core/src/utils.ts"),
    pr_commit_kept=s["branch_head"] in remote("rev-list", B).split(),
    only_the_broken_file={f for f in files if not f.startswith(".changeset/")} == {"packages/query-core/src/removable.ts"},
    no_new_pr=not any(c["args"][:2] == ["pr", "create"] for c in log),
    main_untouched=remote("rev-parse", "main") == s["main"],
))
