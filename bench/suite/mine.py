#!/usr/bin/env python3
"""Candidate tasks for the suite (DESIGN.md) from public repos: PRs merged after the models' release, sorted by the
evidence each kind needs. Public GitHub data only, through `gh api graphql`; writes candidates.jsonl beside this file.

  mine.py [--since 2026-10-20] [owner/repo ...]

Kinds and their evidence:
  review       review threads on the PR and commits pushed after the first review: the threads are known defects
  change       tests added or changed, a moderate size: hidden tests can grade it
  investigate  the PR closes an issue labelled as a bug: the issue is the symptom, the PR the cause
  ci           .github/workflows or Docker files changed: CI and infra work, as in real use
"""
import json, subprocess, sys
from pathlib import Path

REPOS = ["n8n-io/n8n", "withastro/astro", "TanStack/query", "pydantic/pydantic"]
Q = """query($q: String!, $after: String) { search(query: $q, type: ISSUE, first: 50, after: $after) {
  pageInfo { hasNextPage endCursor }
  nodes { ... on PullRequest { number title url mergedAt additions deletions changedFiles
    labels(first: 10) { nodes { name } }
    reviews(first: 30) { nodes { state submittedAt } }
    reviewThreads(first: 1) { totalCount }
    commits(last: 30) { nodes { commit { committedDate } } }
    closingIssuesReferences(first: 3) { nodes { number title labels(first: 10) { nodes { name } } } }
    files(first: 100) { nodes { path } } } } } }"""


def search(repo, since):
    after, out = None, []
    while True:
        args = ["gh", "api", "graphql", "-f", f"query={Q}", "-f", f"q=repo:{repo} is:pr is:merged merged:>={since}"]
        if after:
            args += ["-f", f"after={after}"]
        d = json.loads(subprocess.run(args, capture_output=True, text=True, check=True).stdout)["data"]["search"]
        out += [n for n in d["nodes"] if n]
        if not d["pageInfo"]["hasNextPage"] or len(out) >= 300:
            return out
        after = d["pageInfo"]["endCursor"]


def kinds(pr):
    files = [f["path"] for f in pr["files"]["nodes"]]
    tests = [f for f in files if any(t in f.lower() for t in (".test.", ".spec.", "/__tests__/", "/test/", "/tests/", "_test."))]
    reviews = [r for r in pr["reviews"]["nodes"] if r["state"] in ("CHANGES_REQUESTED", "COMMENTED")]
    first_review = min((r["submittedAt"] for r in reviews if r["submittedAt"]), default=None)
    later_commits = sum(1 for c in pr["commits"]["nodes"] if first_review and c["commit"]["committedDate"] > first_review)
    size = pr["additions"] + pr["deletions"]
    bug_issue = [i for i in pr["closingIssuesReferences"]["nodes"]
                 if any("bug" in l["name"].lower() for l in i["labels"]["nodes"])]
    k = []
    if pr["reviewThreads"]["totalCount"] >= 2 and later_commits >= 1 and size <= 1500:
        k.append("review")
    if tests and len(files) - len(tests) >= 1 and pr["changedFiles"] <= 15 and size <= 800:
        k.append("change")
    if bug_issue and size <= 800:
        k.append("investigate")
    if any(f.startswith(".github/workflows/") or "dockerfile" in f.lower() or f.endswith((".yml", ".yaml")) and "docker" in f.lower() for f in files):
        k.append("ci")
    return k, dict(threads=pr["reviewThreads"]["totalCount"], later_commits=later_commits, size=size, files=pr["changedFiles"],
                   tests=len(tests), bug_issues=[i["number"] for i in bug_issue])


def main(argv):
    since = argv[argv.index("--since") + 1] if "--since" in argv else "2026-10-20"
    repos = [a for a in argv if "/" in a and not a.startswith("--")] or REPOS
    out = Path(__file__).with_name("candidates.jsonl")
    n = 0
    with open(out, "w") as f:
        for repo in repos:
            prs = search(repo, since)
            counts = {}
            for pr in prs:
                k, ev = kinds(pr)
                for x in k:
                    counts[x] = counts.get(x, 0) + 1
                if k:
                    n += 1
                    f.write(json.dumps(dict(repo=repo, number=pr["number"], title=pr["title"], url=pr["url"], merged=pr["mergedAt"][:10],
                                            kinds=k, **ev)) + "\n")
            print(f"{repo}: {len(prs)} merged PRs since {since}; candidates by kind {counts}", flush=True)
    print(f"{n} candidates written to {out}")


if __name__ == "__main__":
    main(sys.argv[1:])
