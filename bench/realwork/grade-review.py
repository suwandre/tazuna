#!/usr/bin/env python3
"""Grades the replayed reviews of the recall set (replay-review.py --set recall): per diff, one call to a judge with the
task, the reference fix, the diff, its hidden test result and every reviewer's findings (anonymous, shuffled). Each
finding: gap (a real defect the task asks to be handled), beyond (real, not asked for) or wrong; on a diff whose hidden
tests failed, whether the reviewer named the cause. GPT-6.1 Sol by default: Opus is one of the graded reviewers.

  grade-review.py [--dry-run]   -> grade-review.jsonl, then the table
"""
import json, os, random, sys, threading
from concurrent.futures import ThreadPoolExecutor
from collections import defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import judge

MODEL, EFFORT = os.environ.get("GRADE_MODEL", "gpt-6.1-sol@https://subrouter.ai/v1"), "xhigh"  # MODEL@URL only
OUT, lock = HERE / "grade-review.jsonl", threading.Lock()
SYSTEM = """You grade code reviews of one candidate patch. You get the task (a real GitHub issue), the reference fix (the merged PR's non-test changes; one valid solution, not the only one), the candidate patch, the candidate's hidden PR test result, and the findings of anonymous reviewers.

For each finding decide:
- gap: a real defect in the candidate that the task asks to be handled (the task states it, or the reference handles it as part of the fix)
- beyond: a real point, but one the task does not ask about
- wrong: not a real defect in the candidate (a misreading, or already handled)

If hidden tests failed, also say for each reviewer whether any of its findings names the cause of the failure.

Reply with only JSON: {"findings": {"<finding id>": {"class": "gap|beyond|wrong", "note": "<one sentence>"}, ...}, "cause": {"<reviewer id>": true|false, ...}}"""


def result(bset, task, diff):
    f = HERE / ("results.jsonl" if bset == "tasks" else f"results-{bset}.jsonl")
    h, run = diff.rsplit("-r", 1)
    return next(r for r in map(json.loads, f.open()) if r["task"] == task and r["harness"] == h and r["run"] == int(run))


def main():
    rows = [r for r in map(json.loads, (HERE / "replay-review.jsonl").open()) if r.get("set") == "recall"]
    by = defaultdict(list)
    for r in rows:
        by[(r["bset"], r["task"], r["diff"])].append(r)
    done = {(g["task"], g["diff"]) for g in map(json.loads, OUT.open())} if OUT.exists() else set()
    todo = [k for k in by if (k[1], k[2]) not in done]
    print(f"{len(todo)} diffs = {len(todo)} grading calls ({MODEL} {EFFORT})", flush=True)
    if "--dry-run" in sys.argv:
        return
    def one(key):
        bset, task, diff = key
        res, revs = result(bset, task, diff), by[(bset, task, diff)][:]
        random.Random(f"{task}-{diff}").shuffle(revs)
        names = {f"R{i + 1}": r["arm"] for i, r in enumerate(revs)}
        parts = [f"# Task\n{(HERE / bset / task / 'prompt.txt').read_text()}",
                 f"# Reference fix\n```diff\n{judge.reference(HERE / bset / task)[:judge.MAX_DIFF]}\n```",
                 f"# Candidate (hidden PR tests {res['passed']}/{res['total']}"
                 + ("" if res.get("regress") is None else (", package suite FAILED" if res["regress"] else ", package suite passed"))
                 + f")\n```diff\n{(HERE / ('runs' if bset == 'tasks' else f'runs-{bset}') / task / f'{diff}.diff').read_text()[:judge.MAX_DIFF]}\n```"]
        for rid, r in zip(names, revs):
            issues = r.get("issues") or []
            parts.append(f"# Reviewer {rid}\n" + ("\n".join(f"- {rid}-{j + 1}: {i}" for j, i in enumerate(issues)) or "(no findings: verdict ok)"))
        v = judge.verdict_json(judge.ask_api("\n\n".join(parts), MODEL, EFFORT, SYSTEM))
        g = dict(raw=v, task=task, diff=diff, passed=res["passed"], total=res["total"], regress=res.get("regress"), grader=MODEL,
                 reviewers={arm: dict(n=len(r.get("issues") or []), error=r.get("error"), cause=v.get("cause", {}).get(rid),
                                      classes=[v["findings"].get(f"{rid}-{j + 1}", {}).get("class") for j in range(len(r.get("issues") or []))],
                                      notes=[v["findings"].get(f"{rid}-{j + 1}", {}).get("note") for j in range(len(r.get("issues") or []))])
                            for rid, (arm, r) in zip(names, zip(names.values(), revs))})
        with lock, OUT.open("a") as f:
            f.write(json.dumps(g) + "\n")
        print(f"{task} {diff[:24]}: " + ", ".join(f"{a} {x['classes']} cause {x['cause']}" for a, x in g["reviewers"].items()), flush=True)
    def safe(key):  # one failed call leaves the others' grades (a rerun grades only what is missing)
        try:
            one(key)
        except Exception as e:
            print(f"{key[1]} {key[2][:24]}: ERROR {e}", file=sys.stderr, flush=True)
    with ThreadPoolExecutor(7) as ex:
        list(ex.map(safe, todo))
    table()


def table():
    gs = list(map(json.loads, OUT.open()))
    arms = sorted({a for g in gs for a in g["reviewers"]})
    for a in arms + ["both"]:
        gap = [g for g in gs if g["passed"] < g["total"]]
        good = [g for g in gs if g["passed"] == g["total"]]
        rv = lambda g: ({"n": sum(x["n"] for x in g["reviewers"].values()), "cause": any(x["cause"] for x in g["reviewers"].values()),
                         "classes": [c for x in g["reviewers"].values() for c in x["classes"]]} if a == "both" else g["reviewers"][a])
        cls = [c for g in gs for c in rv(g)["classes"]]
        print(f"{a:12} cause found {sum(bool(rv(g)['cause']) for g in gap)}/{len(gap)} gap diffs | findings on good diffs "
              f"{sum(rv(g)['n'] for g in good)} ({sum(c == 'gap' for g in good for c in rv(g)['classes'])} gap) | all findings "
              f"{len(cls)}: gap {cls.count('gap')}, beyond {cls.count('beyond')}, wrong {cls.count('wrong')}")


if __name__ == "__main__":
    table() if "--table" in sys.argv else main()
