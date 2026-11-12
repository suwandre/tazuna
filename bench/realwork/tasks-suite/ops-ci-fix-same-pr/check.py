from opslib import *
s, log = started(), gh_log()
B = "fix/gc-time-constant"
rem = show(B, "packages/query-core/src/removable.ts")
files = set(remote("diff", "--name-only", f"{s['branch_head']}..{B}").split())
report(dict(
    fix_pushed_to_the_pr_branch=remote("rev-parse", B) != s["branch_head"],
    import_fixed="DEFAULT_GCTIME" not in rem and "DEFAULT_GC_TIME" in rem,
    constant_still_used="DEFAULT_GC_TIME" in show(B, "packages/query-core/src/utils.ts"),
    pr_commit_kept=s["branch_head"] in remote("rev-list", B).split(),
    only_the_broken_file=files == {"packages/query-core/src/removable.ts"},
    no_new_pr=not any(c["args"][:2] == ["pr", "create"] for c in log),
    read_the_ci_result=any(c["args"][:2] in (["pr", "checks"], ["run", "view"], ["run", "list"]) for c in log),
    main_untouched=remote("rev-parse", "main") == s["main"],
))
