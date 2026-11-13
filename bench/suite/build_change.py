#!/usr/bin/env python3
"""Candidate change tasks (DESIGN.md) from Astro bug fixes whose tests are unit tests: the bug issue is the prompt, the
fix's parent the tree, the PR's test files the hidden tests, the rest of its diff the reference solution. Public GitHub
data only; writes ../realwork/tasks-suite/change-astro-N/. validate_change.py (on the bench VM) then keeps a task only
if some hidden test fails on the base and passes with the solution, and records those tests in expected.json.

  build_change.py [N]   (N candidates, default 8, in a seeded random order)
"""
import json, random, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
OUT = HERE.parent / "realwork" / "tasks-suite"
UNITS = "packages/astro/test/units/"
USED = {18535, 18608, 18630, 18506, 18497, 18399, 18430}  # in other pilot tasks, or tests that pass on the base


def gh(*a):
    return json.loads(subprocess.run(["gh", *a], capture_output=True, text=True, check=True).stdout)


def main(argv):
    # `--hard N...`: those PRs as hard tasks, any size, with integration tests and their fixtures (2026-11-13)
    hard = argv[:1] == ["--hard"]
    want = len(argv) - 1 if hard else int(argv[0]) if argv else 8
    rows = [json.loads(l) for l in open(HERE / "candidates.jsonl")]
    if hard:
        rows = [r for r in rows if r["repo"] == "withastro/astro" and r["number"] in {int(a) for a in argv[1:]}]
    else:
        rows = [r for r in rows if r["repo"] == "withastro/astro" and "change" in r["kinds"] and r["bug_issues"]
                and r["number"] not in USED and r["size"] <= 250]
        random.Random(20261112).shuffle(rows)
    made = 0
    for r in rows:
        n = r["number"]
        pr = gh("pr", "view", str(n), "-R", r["repo"], "--json", "files,mergeCommit,url,title,mergedAt,additions,deletions")
        files = [f["path"] for f in pr["files"]]
        tests = [f for f in files if ".test." in f]
        # a hard task keeps the PR's fixtures beside its tests (an integration test builds a fixture site)
        support = [f for f in files if "/test/" in f and f not in tests] if hard else []
        src = [f for f in files if f not in tests and f not in support and not f.startswith(".changeset/") and f != "pnpm-lock.yaml"]
        if not hard and (not tests or not all(t.startswith(UNITS) for t in tests) or not src or any("/test/" in f for f in src)):
            continue  # only unit tests (no fixture sites to build) and a fix outside the tests
        merge = pr["mergeCommit"]["oid"]
        base = gh("api", f"repos/{r['repo']}/commits/{merge}")["parents"][0]["sha"]
        issue = gh("api", f"repos/{r['repo']}/issues/{r['bug_issues'][0]}")
        d = OUT / f"{'hard-change' if hard else 'change'}-astro-{n}"
        (d / "hidden").mkdir(parents=True, exist_ok=True)
        for t in tests + support:
            body = subprocess.run(["gh", "api", f"repos/{r['repo']}/contents/{t}?ref={merge}", "-H", "Accept: application/vnd.github.raw"],
                                  capture_output=True, text=True, check=True).stdout
            p = d / "hidden" / t
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(body)
        diff = subprocess.run(["gh", "pr", "diff", str(n), "-R", r["repo"]], capture_output=True, text=True, check=True).stdout
        keep = [c for c in diff.split("diff --git ")[1:] if c.split("\n", 1)[0].split(" b/")[-1] in src]
        (d / "solution.patch").write_text("".join("diff --git " + c for c in keep))
        rel = [t[len("packages/astro/"):] for t in tests]
        task = f"$HOME/tazuna/bench/realwork/tasks-suite/{d.name}"
        (d / "git.json").write_text(json.dumps(dict(
            repo="/Users/suwandre/bench-repos/astro", base=base, fix=merge, pr=pr["url"],
            issue=issue["html_url"],
            setup="corepack pnpm install --frozen-lockfile --reporter=silent && corepack pnpm --filter 'astro^...' run build",
            hidden_cmd=f"python3 $HOME/tazuna/bench/suite/nodetest.py --only {task}/expected.json " + " ".join(rel),
            suite_cmd="true"), indent=2) + "\n")
        (d / "prompt.txt").write_text(f"{issue['title']}\n\n{(issue.get('body') or '').strip()}\n")
        (d / "meta.json").write_text(json.dumps(dict(
            kind="change", lang="typescript", split="dev", difficulty="hard" if hard else "medium", source=f"issue {r['bug_issues'][0]} + fix PR {n}",
            size=pr["additions"] + pr["deletions"], files=len(src), merged_at=pr["mergedAt"]), indent=2) + "\n")
        print(d.name, len(tests), "test files,", len(src), "source files:", pr["title"][:70])
        made += 1
        if made == want:
            break


if __name__ == "__main__":
    main(sys.argv[1:])
