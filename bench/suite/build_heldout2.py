#!/usr/bin/env python3
"""Replacement held-out tasks (2026-11-15): five held-out answer tasks were read to explain a gap, so they are dev now.
Draws, seeded, from candidates.jsonl past every PR any task already uses: 2 investigate, 1 plan (bug issue + fix PR) and
2 question (a PR to explain), at least two from pydantic. Built as build_pilots.py builds its tasks; split held-out.
Prints only the task names, so whoever runs it stays blind to the tasks."""
import json, random, re
from pathlib import Path
from build_pilots import api, base_of, diff, write, OUT


def used():
    nums = set()
    for d in OUT.iterdir():
        for f in ("meta.json", "git.json"):
            p = d / f
            if p.exists():
                nums |= {int(n) for n in re.findall(r"(?:PR |pull/|issue )(\d+)", p.read_text())}
        nums |= {int(n) for n in re.findall(r"-(\d{4,6})$", d.name)}
    return nums


def main():
    rows = [json.loads(l) for l in open(Path(__file__).parent / "candidates.jsonl")]
    taken = used()
    rnd = random.Random(20261115)
    bug = [r for r in rows if "investigate" in r["kinds"] and r["bug_issues"] and r["number"] not in taken and not set(r["bug_issues"]) & taken]
    rnd.shuffle(bug)
    py = [r for r in bug if r["repo"] == "pydantic/pydantic"]
    other = [r for r in bug if r["repo"] != "pydantic/pydantic"]
    picks = {"investigate": [py[0], other[0]], "plan": [other[1]]}
    seen = {r["number"] for v in picks.values() for r in v}
    qs = [r for r in rows if "change" in r["kinds"] and 80 <= r["size"] <= 600 and r["number"] not in taken | seen]
    rnd.shuffle(qs)
    qpy = [r for r in qs if r["repo"] == "pydantic/pydantic"][:1]
    qother = [r for r in qs if r["repo"] != "pydantic/pydantic"][:1]
    picks["question"] = qpy + qother
    for kind, rs in picks.items():
        for r in rs:
            repo, n = r["repo"], r["number"]
            name = repo.split("/")[1]
            pr = api(f"repos/{repo}/pulls/{n}")
            git = dict(repo=f"/Users/suwandre/bench-repos/{name}", pr=pr["html_url"], setup="true")
            meta = dict(kind=kind, lang="python" if name == "pydantic" else "typescript", split="held-out",
                        size=pr["additions"] + pr["deletions"], files=pr["changed_files"], merged_at=pr["merged_at"])
            if kind in ("investigate", "plan"):
                issue = api(f"repos/{repo}/issues/{r['bug_issues'][0]}")
                git["base"] = base_of(repo, pr["merge_commit_sha"])
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
                meta.update(source=f"issue {r['bug_issues'][0]} + fix PR {n}", difficulty="medium")
            else:
                git.update(base=base_of(repo, pr["merge_commit_sha"]), pr_head=pr["merge_commit_sha"], pr_title=f"{pr['title']} (#{n})")
                prompt = ("A teammate asks you about the pull request in this repository's last commit (`git show HEAD`; the commit "
                          "before it is the base branch):\n\nTitle: " + pr["title"] + "\n\nTL;DR this PR: what does it change, why, and what "
                          "could it break? Keep it short. Do not change any files.\n")
                ref = (f"The PR as its author described it ({pr['html_url']}):\n{(pr.get('body') or '').strip()[:2500]}\n\n"
                       f"Its changes (non-test files):\n```diff\n{diff(repo, n)}\n```\n"
                       "A good answer says what changes and why in a few lines, is true to the diff (no invented behaviour), "
                       "and names a real risk or says plainly that it sees none.\n")
                meta.update(source=f"PR {n}", difficulty="easy")
            slug = f"{kind}-{name}-{n}"
            if (OUT / slug).exists():
                continue
            d = OUT / slug
            d.mkdir(parents=True)
            (d / "git.json").write_text(json.dumps(git, indent=2) + "\n")
            (d / "prompt.txt").write_text(prompt)
            (d / "reference.md").write_text(ref)
            (d / "meta.json").write_text(json.dumps(meta, indent=2) + "\n")
            print(slug)


if __name__ == "__main__":
    main()
