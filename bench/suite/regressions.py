#!/usr/bin/env python3
"""Review tasks with the strongest ground truth: a merged PR that a later PR fixed ("regression from #N", "introduced in
#N", "broke in #N"). The earlier PR as merged is what a review should have caught; the later fix says what was wrong.
Public GitHub data only (`gh api graphql`); writes regressions.jsonl beside this file.  regressions.py [owner/repo ...]"""
import json, re, subprocess, sys
from pathlib import Path
REPOS = ["n8n-io/n8n", "withastro/astro", "TanStack/query", "pydantic/pydantic"]
Q = """query($q: String!) { search(query: $q, type: ISSUE, first: 50) { nodes { ... on PullRequest {
  number title url body mergedAt additions deletions } } } }"""
REF = re.compile(r"(?:regress\w*|introduced|broke|broken|caused|follow[- ]up)[^.\n]{0,80}?#(\d{3,6})|#(\d{3,6})[^.\n]{0,60}?(?:regress\w*|introduced|broke)", re.I)

def search(q):
    r = subprocess.run(["gh", "api", "graphql", "-f", f"query={Q}", "-f", f"q={q}"], capture_output=True, text=True, check=True)
    return [n for n in json.loads(r.stdout)["data"]["search"]["nodes"] if n]

def pr(repo, n):
    r = subprocess.run(["gh", "api", f"repos/{repo}/pulls/{n}", "--jq", "{number, title, merged_at, additions, deletions, changed_files}"],
                       capture_output=True, text=True)
    return json.loads(r.stdout) if r.returncode == 0 else None

out = Path(__file__).with_name("regressions.jsonl")
with open(out, "w") as f:
    for repo in (sys.argv[1:] or REPOS):
        fixes = {}
        for word in ("regression", "introduced in", "broke", "follow-up"):
            for p in search(f'repo:{repo} is:pr is:merged merged:>=2026-10-20 "{word}"'):
                fixes[p["number"]] = p
        found = 0
        for p in fixes.values():
            refs = {int(a or b) for a, b in REF.findall(p.get("body") or "") + REF.findall(p.get("title") or "")} - {p["number"]}
            for n in sorted(refs):
                orig = pr(repo, n)
                if not orig or not orig.get("merged_at") or orig["merged_at"] < "2026-10-15":
                    continue  # not a merged PR, or older than the models
                found += 1
                f.write(json.dumps(dict(repo=repo, review_pr=n, review_title=orig["title"], review_size=orig["additions"] + orig["deletions"],
                                        review_files=orig["changed_files"], fix_pr=p["number"], fix_title=p["title"],
                                        fix_size=p["additions"] + p["deletions"], fix_url=p["url"])) + "\n")
        print(f"{repo}: {len(fixes)} fix-like PRs, {found} pairs with a merged PR from after 2026-10-15", flush=True)
print("written to", out)
