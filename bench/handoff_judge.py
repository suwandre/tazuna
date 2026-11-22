#!/usr/bin/env python3
"""Part B of handoff.py: does the hand-off line make ordinary chat answers worse? For each model, task of PART_B and run,
the answer with the line (from part A) and without it (part B), side by side in random order, judged in one call by
Opus 5.5 xhigh on the Claude plan against the task's reference.md, with judge.py's general rubric. A pair where the
line's answer handed off instead is counted apart (that is part A's "wrong hand-off"), not judged.

  handoff_judge.py [--models ds,ernie,sol,opus]   # results in handoff/judge.jsonl; done pairs are skipped
  handoff_judge.py summary

Start bench/guard.py first: its PAUSE file stops new judge calls here.
"""
import argparse, collections, json, os, random, statistics, subprocess, tempfile, time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
RES = HERE / "handoff" / "results.jsonl"
ARM = os.environ.get("HANDOFF_ARM", "line")  # the wording compared with none: line (first) or line2 (second)
OUT = HERE / "handoff" / ("judge.jsonl" if ARM == "line" else f"judge-{ARM}.jsonl")
GENERAL = HERE / "realwork" / "tasks-general"

SYSTEM = """You are a strict expert grading two anonymous answers to one request outside coding (math, science, law,
writing, spreadsheet formulas or software architecture). The reference gives the correct result or the points a good
answer covers, and any constraints the request set. Do not reward length or confident tone; a wrong final answer or a
broken constraint (length, form) costs heavily, and a confident wrong claim is worse than saying it is unsure.
Grade each answer on its own first (overall 1-10: correctness, coverage, precision, clarity, fit), then say which is
better, or "tie" when neither is clearly better.

Reply with only a JSON object: {"A": n, "B": n, "better": "A" | "B" | "tie", "note": "<one sentence>"}"""


def judge(prompt):
    while (HERE / "PAUSE").exists():
        time.sleep(30)
    # the user's own claude login (a fresh CLAUDE_CONFIG_DIR has none: the first try sat waiting for one)
    with tempfile.TemporaryDirectory() as cwd:
        cmd = ["claude", "-p", "--model", "claude-opus-5-5", "--effort", "xhigh", "--output-format", "json",
               "--no-session-persistence", "--setting-sources", "", "--strict-mcp-config", "--disable-slash-commands",
               "--exclude-dynamic-system-prompt-sections", "--tools", "", "--system-prompt", SYSTEM]
        out = subprocess.run(cmd, cwd=cwd, input=prompt, capture_output=True, text=True, timeout=600).stdout
    d = json.loads(out.strip().splitlines()[-1])
    if d.get("is_error"):
        raise RuntimeError(d.get("result"))
    t = d["result"]
    v = json.JSONDecoder().raw_decode(t[t.find("{"):])[0]
    return v, d.get("total_cost_usd") or 0


def pairs(models):
    rows = [json.loads(l) for l in RES.read_text().splitlines()]
    got = {(r["model"], r["arm"], r["id"], r["run"]): r for r in rows if r["got"] != "error"}
    seen = {(j["model"], j["id"], j["run"]) for j in map(json.loads, OUT.read_text().splitlines())} if OUT.exists() else set()
    for (m, arm, i, n), none in got.items():
        if arm != "none" or m not in models or (m, i, n) in seen:
            continue
        line = got.get((m, ARM, i, n))
        if line:
            yield m, i, n, line, none


def one(m, i, n, line, none):
    task = i.split(":", 1)[1]
    ref = (GENERAL / task / "reference.md").read_text()
    if line["got"] == "agent":
        r = {"model": m, "id": i, "run": n, "handed_off": True}
    else:
        flip = random.random() < 0.5
        a, b = (none, line) if flip else (line, none)
        prompt = (f"Request:\n{(GENERAL / task / 'prompt.txt').read_text()}\n\nReference:\n{ref}\n\n"
                  f"Answer A:\n{a['result']}\n\nAnswer B:\n{b['result']}")
        v, cost = judge(prompt)
        line_is = "B" if flip else "A"
        none_is = "A" if flip else "B"
        better = v.get("better")
        r = {"model": m, "id": i, "run": n, "handed_off": False, "line": v.get(line_is), "none": v.get(none_is),
             "better": "tie" if better == "tie" else ("line" if better == line_is else "none"), "note": v.get("note"), "cost": cost}
    with OUT.open("a") as f:
        f.write(json.dumps(r) + "\n")
    return r


def summary():
    rows = [json.loads(l) for l in OUT.read_text().splitlines()]
    by = collections.defaultdict(list)
    for r in rows:
        by[r["model"]].append(r)
    for m, rs in by.items():
        j = [r for r in rs if not r["handed_off"]]
        d = [r["line"] - r["none"] for r in j if r.get("line") is not None and r.get("none") is not None]
        c = collections.Counter(r["better"] for r in j)
        mean = statistics.mean(d) if d else 0
        sd = statistics.stdev(d) if len(d) > 1 else 0
        ci = 1.96 * sd / len(d) ** 0.5 if d else 0
        print(f"{m}: {len(j)} pairs judged ({len(rs) - len(j)} handed off); with the line {statistics.mean([r['line'] for r in j]):.2f}, "
              f"without {statistics.mean([r['none'] for r in j]):.2f}; difference {mean:+.2f} (CI {mean - ci:+.2f} .. {mean + ci:+.2f}); "
              f"better: line {c['line']}, without {c['none']}, tie {c['tie']}; judge ${sum(r.get('cost', 0) for r in rs):.2f} at API prices")


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("what", nargs="?", default="run", choices=["run", "summary"])
    ap.add_argument("--models", default="ds,ernie,sol,opus")
    a = ap.parse_args()
    if a.what == "summary":
        summary()
    else:
        todo = list(pairs(a.models.split(",")))
        print(f"{len(todo)} pairs", flush=True)
        with ThreadPoolExecutor(3) as ex:
            for r in ex.map(lambda p: one(*p), todo):
                print(f"  {r['model']} {r['id']} r{r['run']}: {'handed off' if r['handed_off'] else r['better']}", flush=True)
