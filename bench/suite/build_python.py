#!/usr/bin/env python3
"""Python dev tasks (2026-11-15): two investigate and two plan tasks from pydantic bug fixes, built as build_pilots.py
builds its own (public GitHub data only), split dev. The held-out split had three Python answer tasks and the dev split
none of these kinds, so a Python weakness of a harness could not be seen on dev. 14065 (generics) is left out: close to a
held-out task's topic."""
import json
from build_pilots import api, base_of, diff, write, OUT

PICKS = {"investigate": [14115, 14051], "plan": [14159, 13953]}


def main():
    rows = {r["number"]: r for r in map(json.loads, open("candidates.jsonl")) if r["repo"] == "pydantic/pydantic"}
    for kind, ns in PICKS.items():
        for n in ns:
            r, repo = rows[n], "pydantic/pydantic"
            slug = f"{kind}-pydantic-{n}"
            if (OUT / slug).exists():
                continue
            pr = api(f"repos/{repo}/pulls/{n}")
            issue = api(f"repos/{repo}/issues/{r['bug_issues'][0]}")
            git = dict(repo="/Users/suwandre/bench-repos/pydantic", pr=pr["html_url"], setup="true", base=base_of(repo, pr["merge_commit_sha"]))
            body = (issue.get("body") or "").strip()[:4000]
            ask = ("Find the root cause: what in the code makes this happen, with file:line, and why. Do not fix it or change any files."
                   if kind == "investigate" else
                   "How should we fix this? Give the options you see, their trade-offs, and the one you recommend for this codebase, "
                   "with the files it would touch. Do not change any files.")
            prompt = f"A bug report came in for this repository:\n\nTitle: {issue['title']}\n\n{body}\n\n{ask}\n"
            ref = (f"The maintainers fixed it in {pr['html_url']} (\"{pr['title']}\").\n\nTheir description:\n{(pr.get('body') or '').strip()[:2500]}\n\n"
                   f"The fix (non-test files):\n```diff\n{diff(repo, n)}\n```\n"
                   + ("A good answer names this cause and where it is; another real cause, shown in the code, counts too.\n" if kind == "investigate"
                      else "A good answer includes this approach or an equally sound one, says why, and weighs at least one alternative.\n"))
            meta = dict(kind=kind, lang="python", split="dev", size=pr["additions"] + pr["deletions"], files=pr["changed_files"],
                        merged_at=pr["merged_at"], source=f"issue {r['bug_issues'][0]} + fix PR {n}", difficulty="medium")
            write(slug, git, prompt, ref, meta)


if __name__ == "__main__":
    main()
