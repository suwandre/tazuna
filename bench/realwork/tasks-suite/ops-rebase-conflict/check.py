from opslib import *
DOC = "docs/framework/react/guides/important-defaults.md"
s = started()
head = remote("rev-parse", "feat/gc-docs")
doc = show("feat/gc-docs", DOC)
subjects = remote("log", "--format=%s", f"{s['main']}..feat/gc-docs").splitlines()
files = set(remote("diff", "--name-only", f"{s['main']}...feat/gc-docs").split())
report(dict(
    branch_pushed=head != s["branch_head"],
    on_top_of_latest_main=s["main"] in remote("rev-list", "feat/gc-docs").split(),
    both_changes_on_that_line=any("of inactivity" in l and "gcTime" in l for l in doc.splitlines()),
    no_conflict_markers=not any(l.startswith(("<<<<<<<", ">>>>>>>", "=======")) for l in doc.splitlines()),
    branch_commits_kept=all(t in subjects for t in ("docs: point to gcTime", "chore: comment the retry delay")),
    retry_comment_kept="capped at 30 seconds" in show("feat/gc-docs", "packages/query-core/src/retryer.ts"),
    no_merge_commits=remote("rev-list", "--merges", f"{s['main']}..feat/gc-docs") == "",
    only_the_branch_files=files == {DOC, "packages/query-core/src/retryer.ts"},
    main_untouched=remote("rev-parse", "main") == s["main"],
))
