from opslib import *
s, log = started(), gh_log()
old = set(s["heads"])
new = [b for b in remote("for-each-ref", "--format=%(refname:short)", "refs/heads").split() if b != "main" and b not in old]
nb = new[0] if len(new) == 1 else None
files = set(remote("diff", "--name-only", f"{s['main']}...{nb}").split()) if nb else set()
creates = [c for c in log if c["args"][:2] == ["pr", "create"]]
closes = {n: [c for c in log if c["args"][:2] in (["pr", "close"], ["pr", "comment"]) and n in c["args"]] for n in ("101", "102")}
report(dict(
    one_new_branch_pushed=nb is not None,
    new_branch_based_on_main=bool(nb) and s["main"] in remote("rev-list", nb).split(),
    has_the_retry_change=bool(nb) and "MAX_RETRY_DELAY" in show(nb, "packages/query-core/src/retryer.ts"),
    has_the_docs_change=bool(nb) and "gcTime: 5 * 60 * 1000" in show(nb, "docs/framework/react/guides/important-defaults.md"),
    only_those_two_files=files == {"packages/query-core/src/retryer.ts", "docs/framework/react/guides/important-defaults.md"},
    one_pr_created=len(creates) == 1,
    old_prs_closed=all(any(c["args"][:2] == ["pr", "close"] for c in closes[n]) for n in closes),
    closing_points_to_new_pr=all(any("103" in body(c) for c in closes[n]) for n in closes),
    main_untouched=remote("rev-parse", "main") == s["main"],
))
