#!/usr/bin/env python3
"""Replay: can a decision model (TypeSafe's Jev) tell which own-loop diffs need a review? 113 of 114 failed DeepSeek runs
ended on their own, sure they were done (decide_data.py), so the failures are silent: a reviewer catches them (Opus
medium named the cause in 5-6 of 7), a stuck-run rule does not. If Jev can pick the diffs that miss something, the review
runs only on those.

Each run with a saved diff: state = the task, a short summary and the diff (cut to MAX_DIFF); one `noul` question. Truth:
the run's hidden tests (all passed or not). Scored against simple rules on the same diffs: the share of failures caught when
reviewing 30/50/70% of diffs, overall and within tasks (the same task's runs ranked against each other). Answers are kept
in decide-jev.jsonl, so a rerun asks only what is missing. Key: TYPESAFE_API_KEY (from ~/.typesafe-key); the diffs are of
open-source repositories only.

  decide_jev.py --dry-run | decide_jev.py [--limit N]
"""
import json, os, re, sys, threading, urllib.request
from collections import defaultdict
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
OUT, lock = HERE / "decide-jev.jsonl", threading.Lock()
URL, MODEL, MAX_DIFF = "https://api.typesafe.ai/v1/systemone", os.environ.get("JEV_MODEL", "jev-latest"), 24000
QUESTION = {"type": "noul", "instructions": "Does this change fail to fully do what the task asks: a case the task implies "
            "that the change misses, an incomplete fix, or a wrong approach, so that a careful reviewer would find a real defect?",
            "criteria": {"true": "the change misses or gets wrong something the task asks for",
                         "false": "the change does everything the task asks, correctly"}}


def items():
    """(key, task, harness, state, ok, features) for each DeepSeek own-loop run with a diff and an outcome."""
    runs = {}
    for line in (HERE / "decide-stuck.jsonl").open():
        s = json.loads(line)
        if "deepseek" in s["model"]:
            runs[(s["task"], s["harness"])] = s
    sets = {t.parent.name: t.parent.parent.name for t in HERE.glob("tasks*/*/prompt.txt")}
    for (task, h), s in sorted(runs.items()):
        bset = sets.get(task)
        d = HERE / ("runs" if bset == "tasks" else f"runs-{bset}") / task / f"{h}-r0.diff"
        if not bset or not d.exists():
            continue
        diff = d.read_text(errors="replace")
        files = re.findall(r"^\+\+\+ b/(\S+)", diff, re.M)
        tests = [f for f in files if "_test." in f or "/testdata/" in f]
        add = sum(1 for l in diff.splitlines() if l.startswith("+") and not l.startswith("+++"))
        rem = sum(1 for l in diff.splitlines() if l.startswith("-") and not l.startswith("---"))
        test_lines = 0
        cur = False
        for l in diff.splitlines():
            if l.startswith("+++ "):
                cur = "_test." in l or "/testdata/" in l
            elif cur and l.startswith("+") and not l.startswith("+++"):
                test_lines += 1
        feats = dict(files=len(files), test_files=len(tests), added=add, removed=rem, test_lines=test_lines,
                     code_lines=add + rem - test_lines, calls=s["total_calls"])
        state = (f"# Task\n{(HERE / bset / task / 'prompt.txt').read_text()[:6000]}\n\n# The change\n"
                 f"{len(files)} files ({len(tests)} test files), +{add} -{rem} lines\n```diff\n{diff[:MAX_DIFF]}\n```")
        yield f"{task}|{h}", task, h, state, s["ok"], feats


def ask(state):
    body = json.dumps({"model": MODEL, "state": state, "questions": {"needs_review": QUESTION}}).encode()
    q = urllib.request.Request(URL, data=body, headers={"Authorization": f"Bearer {os.environ['TYPESAFE_API_KEY']}",
                                                        "Content-Type": "application/json"})
    with urllib.request.urlopen(q, timeout=120) as r:
        d = json.load(r)
    return d["answers"]["needs_review"]["noul"], d.get("usage", {}), d.get("model")


def caught(ranked, budget):
    """Share of the failures among the top `budget` share of diffs, ranked by score (highest first)."""
    n = max(1, round(len(ranked) * budget))
    fails = sum(1 for _, ok in ranked if not ok)
    return sum(1 for _, ok in sorted(ranked, key=lambda x: -x[0])[:n] if not ok) / max(fails, 1)


def evaluate(rows):
    scores = {
        "Jev": lambda r: r["p"],
        "rule: fewer test lines": lambda r: -r["f"]["test_lines"],
        "rule: more code lines": lambda r: r["f"]["code_lines"],
        "rule: more model calls": lambda r: r["f"]["calls"],
        "random (expected)": None,
    }
    fails = sum(1 for r in rows if not r["ok"])
    print(f"{len(rows)} diffs, {fails} failed a hidden test ({100 * fails // len(rows)}%)")
    print(f"{'ranking':26} failures caught when reviewing 30% / 50% / 70% of diffs | within-task (mean of tasks with both outcomes)")
    by_task = defaultdict(list)
    for r in rows:
        by_task[r["task"]].append(r)
    mixed = [rs for rs in by_task.values() if 0 < sum(not r["ok"] for r in rs) < len(rs)]
    for name, f in scores.items():
        if f is None:
            print(f"{name:26} {30:3}% / {50:3}% / {70:3}%")
            continue
        ranked = [(f(r), r["ok"]) for r in rows]
        within = [sum(caught([(f(r), r["ok"]) for r in rs], b) for rs in mixed) / max(len(mixed), 1) for b in (0.3, 0.5, 0.7)]
        print(f"{name:26} " + " / ".join(f"{100 * caught(ranked, b):3.0f}%" for b in (0.3, 0.5, 0.7))
              + " | " + " / ".join(f"{100 * w:3.0f}%" for w in within) + f"  ({len(mixed)} tasks)")


def main():
    all_items = list(items())
    done = {json.loads(l)["key"]: json.loads(l) for l in OUT.open()} if OUT.exists() else {}
    todo = [i for i in all_items if i[0] not in done]
    if "--limit" in sys.argv:
        todo = todo[: int(sys.argv[sys.argv.index("--limit") + 1])]
    chars = sum(len(i[3]) for i in todo)
    print(f"{len(all_items)} diffs, {len(done)} answered, {len(todo)} to ask: ~{chars // 4 / 1e6:.2f}M input tokens "
          f"(~${chars / 4 / 1e6 * 0.042:.3f} at $0.042/M)", flush=True)
    if "--dry-run" in sys.argv:
        return

    def one(i):
        key, task, h, state, ok, feats = i
        try:
            p, usage, model = ask(state)
        except Exception as e:
            print(f"{key}: {e}", file=sys.stderr, flush=True)
            return
        with lock, OUT.open("a") as f:
            f.write(json.dumps(dict(key=key, task=task, harness=h, ok=ok, p=p, f=feats, usage=usage, model=model)) + "\n")

    with ThreadPoolExecutor(8) as ex:
        list(ex.map(one, todo))
    rows = [json.loads(l) for l in OUT.open()]
    evaluate(rows)


if __name__ == "__main__":
    main()
