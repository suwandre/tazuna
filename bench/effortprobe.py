#!/usr/bin/env python3
"""Does a route honour the effort it is given? The same hard AIME problems at each effort, a few times each, through the
route in KEY-ENV: reasoning tokens, time and answers per effort, plus a made-up level ("ultra") to see what a value the
route does not know turns into. A route that maps max to something lower shows max at or under xhigh. 2026-11-01, after
DeepSeek and Sol scored no better at max than at xhigh in agent runs.

  effortprobe.py MODEL KEY-ENV [N] [--hle]   N samples per effort and problem (default 2); --hle: three HLE math questions
"""
import json, os, statistics as st, sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from canary import ask, grade  # noqa: E402

EFFORTS = ["low", "high", "xhigh", "max", "ultra"]
hle = "--hle" in sys.argv  # three HLE math questions instead: AIME was too easy to tell GPT's xhigh from max (2026-11-01)
args = [a for a in sys.argv[1:] if a != "--hle"]
model, key_env = args[0], args[1]
n = int(args[2]) if len(args) > 2 else 2
if hle:
    items = [q for q in map(json.loads, (HERE / ".model-check" / "hle" / "set-400.jsonl").open())
             if q["category"] == "Math" and q["answer_type"] == "exactMatch"][:3]
else:
    items = [json.loads(l) for l in (HERE / "canary" / "set-v1.jsonl").open() if '"aime25"' in l]
    items = [items[i] for i in (13, 21, 27)]  # three late, harder AIME 2025 problems, fixed
key = os.environ[key_env]


def one(job):
    effort, item = job
    try:
        tail = "\n\nEnd with one line: Answer: <your answer>" if hle else "\n\nSolve it. End with one line: Answer: <the integer>"
        text, usage = ask(model, key, effort, item["question"] + tail, max_tokens=64000)
    except Exception as e:
        return effort, None, None, False, str(e)[:120]
    details = usage.get("completion_tokens_details") or {}
    return effort, usage.get("completion_tokens"), details.get("reasoning_tokens"), (not hle) and grade(item, text)[0], None


jobs = [(e, it) for e in EFFORTS for it in items for _ in range(n)]
with ThreadPoolExecutor(8) as ex:
    out = list(ex.map(one, jobs))
print(f"{model} via {key_env}: {len(items)} {'HLE math questions (not graded)' if hle else 'AIME problems'} x {n} per effort")
for e in EFFORTS:
    rs = [r for r in out if r[0] == e]
    toks = [r[1] for r in rs if r[1]]
    reas = [r[2] for r in rs if r[2]]
    errs = [r[4] for r in rs if r[4]]
    print(f"  {e:6} completion tokens median {st.median(toks) if toks else '-':>7}  reasoning median {st.median(reas) if reas else '-':>7}"
          f"  right {sum(r[3] for r in rs)}/{len(rs)}" + (f"  errors {len(errs)}: {errs[0]}" if errs else ""))
