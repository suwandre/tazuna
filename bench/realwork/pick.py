#!/usr/bin/env python3
"""Offline check of "two cheap attempts, Opus picks one": for each pair of finished runs of a task, Opus (on the
subscription, `claude -p`, no tools, no reference) sees the task and both diffs in random order and picks the one it would
merge; the pick is then compared with the judge's scores of the two runs.

    BENCH_SET=tasks-oct pick.py [--dry-run] TASK:HARNESS,HARNESS,... [TASK:...]

Writes pick.oct.jsonl. Needs a fresh usage-guard reading under 85% of the 5-hour window before each call.
"""
import glob, itertools, json, os, random, subprocess, sys, time
from pathlib import Path

HERE = Path(__file__).resolve().parent
RUNS, TASKS, OUT = HERE / "runs-tasks-oct", HERE / "tasks-oct", HERE / "pick.oct.jsonl"
STATE = HERE.parent / "guard.state.json"
SYSTEM = """You review two candidate patches for the same task, as the maintainer who has to merge one of them. Pick the
one you would merge: it does what the task asks and handles the cases the task implies, it is correct, and it changes no
more than the task needs, in the style of the codebase. Extra features, rewrites of code the task did not touch and
speculative hardening count against a patch, not for it. Answer with JSON only: {"pick": "A" or "B", "why": "<one sentence>"}"""


def judged():
    """Latest judge score per (task, harness) from the October judge files."""
    out = {}
    for f in sorted(glob.glob(str(HERE / "judge.oct.*.jsonl")), key=os.path.getmtime):
        for line in open(f):
            if line.strip().startswith("{"):
                j = json.loads(line)
                if "overall" in j:
                    out[(j["task"], j["harness"])] = j["overall"]
    return out


def guard_ok():
    s = json.loads(STATE.read_text())
    return time.time() - s["at"] < 600 and s.get("five_hour", s["util"]) < 85, s


def pick(task, a, b):
    prompt = (f"# Task\n\n{(TASKS / task / 'prompt.txt').read_text()}\n\n# Patch A\n\n```diff\n{a[:60000]}\n```\n\n"
              f"# Patch B\n\n```diff\n{b[:60000]}\n```")
    r = subprocess.run(["claude", "-p", "--model", "claude-opus-5-5", "--effort", "high", "--output-format", "json",
                        "--no-session-persistence", "--setting-sources", "", "--strict-mcp-config", "--disable-slash-commands",
                        "--tools", "", "--system-prompt", SYSTEM], input=prompt, capture_output=True, text=True, timeout=900)
    res = json.loads(r.stdout)
    text = res.get("result", "")
    p = json.loads(text[text.find("{"):text.rfind("}") + 1])
    return p["pick"].strip().upper()[:1], p.get("why", ""), res.get("total_cost_usd")


def main():
    args = [a for a in sys.argv[1:] if a != "--dry-run"]
    pairs = []
    for spec in args:
        task, hs = spec.split(":", 1)
        pairs += [(task, x, y) for x, y in itertools.combinations(hs.split(","), 2)]
    print(f"{len(pairs)} pairs = {len(pairs)} Opus calls (claude-opus-5-5 high, no tools)", flush=True)
    if "--dry-run" in sys.argv:
        return
    scores, rng = judged(), random.Random(7)
    for task, x, y in pairs:
        ok, s = guard_ok()
        if not ok:
            sys.exit(f"guard: stale or over 85% (5h {s.get('five_hour')}%): stopping")
        first, second = (x, y) if rng.random() < 0.5 else (y, x)  # the order shown, against position bias
        diff = lambda h: (RUNS / task / f"{h}-r0.diff").read_text()
        p, why, cost = pick(task, diff(first), diff(second))
        chosen, other = (first, second) if p == "A" else (second, first)
        row = dict(task=task, shown_a=first, shown_b=second, pick=chosen, other=other, why=why, api_usd=cost,
                   pick_score=scores.get((task, chosen)), other_score=scores.get((task, other)))
        with OUT.open("a") as f:
            f.write(json.dumps(row) + "\n")
        print(f"{task[:12]} {chosen.split('@')[0]} ({row['pick_score']}) over {other.split('@')[0]} ({row['other_score']}): {why[:110]}"
              f" | 5h {guard_ok()[1].get('five_hour')}%", flush=True)


if __name__ == "__main__":
    main()
