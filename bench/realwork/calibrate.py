#!/usr/bin/env python3
"""Does a cheap judge rank like Opus? Re-grades patches Opus already judged (judge.oct.*.jsonl, with reference, one
candidate per call as there) with another model on Experiential Labs, then compares the overall scores.

  EXPERIENTIAL_API_KEY=... ./calibrate.py [--model gpt-6-luna] [--max-usd 1.0]
  ./calibrate.py --report

No Claude usage: the cheap judge runs on the Experiential Labs balance; --max-usd stops it.
"""
import argparse, glob, json, os, re, statistics, threading, urllib.request
from concurrent.futures import ThreadPoolExecutor

os.environ.setdefault("BENCH_SET", "tasks-oct")
import judge  # noqa: E402  (paths, SYSTEM prompt, reference fix, MAX_DIFF)

URL = "https://api.experientiallabs.ai/v1/responses"


def opus_rows():
    rows = {}
    for f in glob.glob(str(judge.HERE / "judge.oct.*.jsonl")):
        if ".noref." in f:
            continue
        for l in open(f):
            r = json.loads(l)
            if r.get("judge") == "claude-opus-5-5":
                rows[(r["task"], r["harness"], r["run"])] = r
    return rows


def prompt(task, harness, run):
    c = [x for x in judge.candidates(task) if x["harness"] == harness and x["run"] == run][0]
    diff = c["diff"] if len(c["diff"]) <= judge.MAX_DIFF else c["diff"][:judge.MAX_DIFF] + "\n[diff truncated]"
    reg = c["r"].get("regress")
    facts = f"hidden PR tests {c['r']['passed']}/{c['r']['total']}" + ("" if reg is None else (", package suite FAILED" if reg else ", package suite passed"))
    t = judge.TASKS / task
    return "\n\n".join([f"# Task\n{(t / 'prompt.txt').read_text()}",
                        f"# Reference fix\n```diff\n{judge.reference(t)[:judge.MAX_DIFF]}\n```",
                        f"# Candidate C1 ({facts})\n```diff\n{diff or '(no changes)'}\n```"])


def ask(model, effort, text, key):
    body = json.dumps({"model": model, "instructions": judge.SYSTEM, "input": [{"role": "user", "content": text}],
                       "reasoning": {"effort": effort}, "store": False}).encode()
    req = urllib.request.Request(URL, body, {"Authorization": f"Bearer {key}", "Content-Type": "application/json"})
    d = json.load(urllib.request.urlopen(req, timeout=600))
    out = "".join(c.get("text", "") for i in d.get("output", []) if i.get("type") == "message" for c in i.get("content", []))
    m = re.search(r"\{.*\}", out, re.S)
    if not m:
        raise RuntimeError("no JSON in the answer: " + out[:200])
    return json.loads(m.group(0))["C1"], d.get("usage", {}).get("cost") or 0.0


def report(path):
    rows = opus_rows()
    pairs = [(rows[k], json.loads(l)) for l in open(path) for k in [tuple(json.loads(l)["key"])] if k in rows]
    o = [p[0]["overall"] for p in pairs]
    c = [p[1]["score"]["overall"] for p in pairs]
    if len(o) < 3:
        return print("too few pairs")
    diffs = [abs(a - b) for a, b in zip(o, c)]
    # within a task, do both judges order the candidates the same way?
    agree = total = 0
    by_task = {}
    for (op, ch) in pairs:
        by_task.setdefault(op["task"], []).append((op["overall"], ch["score"]["overall"]))
    for xs in by_task.values():
        for i in range(len(xs)):
            for j in range(i + 1, len(xs)):
                if xs[i][0] != xs[j][0]:
                    total += 1
                    agree += (xs[i][0] - xs[j][0]) * (xs[i][1] - xs[j][1]) > 0
    print(f"{len(pairs)} patches  pearson r {statistics.correlation(o, c):.2f}  mean |diff| {statistics.mean(diffs):.2f}  "
          f"within 1 point {sum(d <= 1 for d in diffs) / len(diffs):.0%}  mean opus {statistics.mean(o):.2f} vs {statistics.mean(c):.2f}")
    print(f"pairwise order within a task: {agree}/{total} = {agree / max(1, total):.0%} agree (ties at Opus left out)")
    worst = sorted(zip(diffs, pairs), key=lambda x: -x[0])[:5]
    for d, (op, ch) in worst:
        print(f"  off by {d:.1f}: {op['task']} {op['harness']} r{op['run']}  opus {op['overall']}  cheap {ch['score']['overall']}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--model", default="gpt-6-luna")
    ap.add_argument("--effort", default="high")
    ap.add_argument("--max-usd", type=float, default=1.0)
    ap.add_argument("--report", action="store_true")
    a = ap.parse_args()
    out = judge.HERE / f"calib.{a.model}.{a.effort}.jsonl"
    if a.report:
        return report(out)
    key = os.environ["EXPERIENTIAL_API_KEY"]
    done = {tuple(json.loads(l)["key"]) for l in open(out)} if out.exists() else set()
    # a model never grades its own family's patches (self-preference)
    family = a.model.split("-")[0]
    todo = [k for k in opus_rows() if k not in done and family not in k[1]]
    spent, lock = [0.0], threading.Lock()
    print(f"{len(todo)} patches to grade with {a.model} {a.effort}, budget ${a.max_usd}")

    def one(k):
        if spent[0] >= a.max_usd:
            return
        try:
            score, cost = ask(a.model, a.effort, prompt(*k), key)
        except Exception as e:
            return print(f"{k}: ERROR {e}")
        with lock:
            spent[0] += cost
            with open(out, "a") as f:
                f.write(json.dumps({"key": list(k), "score": score, "cost": cost}) + "\n")

    with ThreadPoolExecutor(6) as ex:
        list(ex.map(one, todo))
    print(f"spent ${spent[0]:.3f}")
    report(out)


if __name__ == "__main__":
    main()
