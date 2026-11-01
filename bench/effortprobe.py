#!/usr/bin/env python3
"""Does a route honour the effort it is given? The same hard AIME problems at each effort, a few times each, through the
route in KEY-ENV: reasoning tokens, time and answers per effort, plus a made-up level ("ultra") to see what a value the
route does not know turns into. A route that maps max to something lower shows max at or under xhigh. 2026-11-01, after
DeepSeek and Sol scored no better at max than at xhigh in agent runs.

  effortprobe.py MODEL KEY-ENV [N]     N samples per effort and problem (default 2)
"""
import json, os, statistics as st, sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from canary import ask, grade  # noqa: E402

EFFORTS = ["low", "high", "xhigh", "max", "ultra"]
model, key_env = sys.argv[1], sys.argv[2]
n = int(sys.argv[3]) if len(sys.argv) > 3 else 2
items = [json.loads(l) for l in (HERE / "canary" / "set-v1.jsonl").open() if '"aime25"' in l]
items = [items[i] for i in (13, 21, 27)]  # three late, harder AIME 2025 problems, fixed
key = os.environ[key_env]


def one(job):
    effort, item = job
    try:
        text, usage = ask(model, key, effort, item["question"] + "\n\nSolve it. End with one line: Answer: <the integer>", max_tokens=64000)
    except Exception as e:
        return effort, None, None, False, str(e)[:120]
    details = usage.get("completion_tokens_details") or {}
    return effort, usage.get("completion_tokens"), details.get("reasoning_tokens"), grade(item, text)[0], None


jobs = [(e, it) for e in EFFORTS for it in items for _ in range(n)]
with ThreadPoolExecutor(8) as ex:
    out = list(ex.map(one, jobs))
print(f"{model} via {key_env}: {len(items)} AIME problems x {n} per effort")
for e in EFFORTS:
    rs = [r for r in out if r[0] == e]
    toks = [r[1] for r in rs if r[1]]
    reas = [r[2] for r in rs if r[2]]
    errs = [r[4] for r in rs if r[4]]
    print(f"  {e:6} completion tokens median {st.median(toks) if toks else '-':>7}  reasoning median {st.median(reas) if reas else '-':>7}"
          f"  right {sum(r[3] for r in rs)}/{len(rs)}" + (f"  errors {len(errs)}: {errs[0]}" if errs else ""))
