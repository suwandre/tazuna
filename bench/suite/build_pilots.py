#!/usr/bin/env python3
"""Pilot tasks for the answer kinds (DESIGN.md): investigate, question and plan, drawn at random (seeded) from
candidates.jsonl. Public GitHub data only; writes task folders into ../realwork/tasks-suite/.

  investigate  a bug issue closed by a fix PR: the issue is the prompt, the tree is the fix's parent, the fix is the reference
  question     "explain this PR": the PR as the last commit (as for a review), its description and diff the reference
  plan         "how should we fix this": a bug issue as the prompt, the maintainers' fix as the reference approach
"""
import json, random, subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
OUT = HERE.parent / "realwork" / "tasks-suite"


def api(path):
    return json.loads(subprocess.run(["gh", "api", path], capture_output=True, text=True, check=True).stdout)


def diff(repo, n, limit=6000):
    d = subprocess.run(["gh", "pr", "diff", str(n), "-R", repo], capture_output=True, text=True).stdout
    # the non-test part is what explains the change; tests follow if there is room
    files = d.split("diff --git ")
    main = [f for f in files if f and not any(t in f.split("\n", 1)[0] for t in (".test.", ".spec.", "/test", "_test."))]
    text = "".join("diff --git " + f for f in main)
    return text[:limit] + ("\n[diff truncated]" if len(text) > limit else "")


def base_of(repo, merge):
    return api(f"repos/{repo}/commits/{merge}")["parents"][0]["sha"]


def write(tid, git, prompt, reference, meta):
    d = OUT / tid
    d.mkdir(parents=True, exist_ok=True)
    (d / "git.json").write_text(json.dumps(git, indent=2) + "\n")
    (d / "prompt.txt").write_text(prompt)
    (d / "reference.md").write_text(reference)
    (d / "meta.json").write_text(json.dumps(meta, indent=2) + "\n")
    print(tid, meta["kind"], meta["source"])


def main():
    rows = [json.loads(l) for l in open(HERE / "candidates.jsonl")]
    rnd = random.Random(20261112)
    inv = [r for r in rows if "investigate" in r["kinds"] and r["bug_issues"]]
    rnd.shuffle(inv)
    picks = {"investigate": inv[:3], "plan": inv[3:6]}
    qs = [r for r in rows if "change" in r["kinds"] and 80 <= r["size"] <= 600]
    rnd.shuffle(qs)
    by_repo, question = set(), []
    for r in qs:  # one per repo, for spread
        if r["repo"] not in by_repo:
            by_repo.add(r["repo"]); question.append(r)
        if len(question) == 3:
            break
    picks["question"] = question
    for kind, rs in picks.items():
        for r in rs:
            repo, n = r["repo"], r["number"]
            pr = api(f"repos/{repo}/pulls/{n}")
            merge = pr["merge_commit_sha"]
            name = repo.split("/")[1]
            slug = f"{kind}-{name}-{n}"
            git = dict(repo=f"/Users/suwandre/bench-repos/{name}", pr=pr["html_url"], setup="true")
            meta = dict(kind=kind, lang="python" if name == "pydantic" else "typescript", split="dev",
                        size=pr["additions"] + pr["deletions"], files=pr["changed_files"], merged_at=pr["merged_at"])
            if kind in ("investigate", "plan"):
                issue = api(f"repos/{repo}/issues/{r['bug_issues'][0]}")
                git["base"] = base_of(repo, merge)
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
                git.update(base=base_of(repo, merge), pr_head=merge, pr_title=f"{pr['title']} (#{n})")
                prompt = ("A teammate asks you about the pull request in this repository's last commit (`git show HEAD`; the commit "
                          "before it is the base branch):\n\nTitle: " + pr["title"] + "\n\nTL;DR this PR: what does it change, why, and what "
                          "could it break? Keep it short. Do not change any files.\n")
                ref = (f"The PR as its author described it ({pr['html_url']}):\n{(pr.get('body') or '').strip()[:2500]}\n\n"
                       f"Its changes (non-test files):\n```diff\n{diff(repo, n)}\n```\n"
                       "A good answer says what changes and why in a few lines, is true to the diff (no invented behaviour), "
                       "and names a real risk or says plainly that it sees none.\n")
                meta.update(source=f"PR {n}", difficulty="easy")
            write(slug, git, prompt, ref, meta)


if __name__ == "__main__":
    main()
