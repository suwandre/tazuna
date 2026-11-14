from opslib import *
s, log = started(), gh_log()
B = "docs/retry-delay"
creates = [c for c in log if c["args"][:2] == ["pr", "create"]]
c = creates[0] if creates else {"args": []}
a = c["args"]
head = a[a.index("--head") + 1] if "--head" in a else a[a.index("-H") + 1] if "-H" in a else B  # gh's default: the current branch
base = a[a.index("--base") + 1] if "--base" in a else a[a.index("-B") + 1] if "-B" in a else "main"
text = body(c).lower()
report(dict(
    # pushed with its commits (one more, such as a changeset, is fine: the repo's convention)
    branch_pushed=s["head"] in remote("rev-list", B).split(),
    one_pr_created=len(creates) == 1,
    pr_from_the_branch_to_main=head == B and base == "main",
    description_says_what_changes="retr" in text and any(w in text for w in ("delay", "cap", "30")),
    main_untouched=remote("rev-parse", "main") == s["main"],
))
