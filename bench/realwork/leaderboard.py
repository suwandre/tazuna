#!/usr/bin/env python3
"""Which model to use by default on UnoRouter: every run of one config (uno-runs.jsonl, written by uno_run.sh), no others.

    ./leaderboard.py [config, default uno-1]

Per model: runs, hidden tests, judge (mean, spread, worst), a quality floor, billed cost, cache, time, and failures split
into quality (judge 4 or less, or a hidden test failing) and operational (the run was stopped or errored: group gone,
price rose, cache failed). Judge scores from separate calls are put on one scale with the reference runs every pool
carries (Codex with Luna, tazuna with DeepSeek on CheaperInference): a pool that scored them a point low is lifted a point.
The floor is the mean minus one standard error, with the judge's own noise (about 1.5 points per pass) as the least spread,
so a model with one run or erratic runs ranks lower. Default: the cheapest model with at least 3 runs, a floor of 7 or
more (Codex's level) and at most one failure in five runs.
"""
import glob, json, statistics, sys
from collections import defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
ANCHORS = ("codex@gpt-6-luna:max", "tazuna-own-v38@deepseek-v4.1-flash:max")
JUDGE_NOISE = 1.5


def judge_scores():
    """{(task, harness, run): judge score on one scale} over every judge pool file."""
    pools = []
    for f in glob.glob(str(HERE / "judge.oct*.jsonl")):
        rows = [json.loads(l) for l in open(f) if l.strip()]
        pools.append({(r["task"], r["harness"], r["run"]): (r.get("scores") or r)["overall"] for r in rows if (r.get("scores") or r).get("overall") is not None})
    def anchors(p):
        return {k: v for k, v in p.items() if k[1] in ANCHORS and k[2] == 0}
    seen = defaultdict(list)
    for p in pools:
        for k, v in anchors(p).items():
            seen[k].append(v)
    base = {k: statistics.mean(v) for k, v in seen.items()}
    out = defaultdict(list)
    for p in pools:
        a = anchors(p)
        offset = statistics.mean(v - base[k] for k, v in a.items()) if a else 0.0
        for k, v in p.items():
            out[k].append(v - offset)
    return {k: statistics.mean(v) for k, v in out.items()}


def main(config):
    runs = [json.loads(l) for l in open(HERE / "uno-runs.jsonl") if l.strip()]
    runs = [r for r in runs if r["config"] == config]
    results = {(r["task"], r["harness"], r["run"]): r for r in map(json.loads, open(HERE / "results-tasks-oct.jsonl"))}
    judged = judge_scores()
    by_model = defaultdict(list)
    for r in runs:
        res = results.get((r["task"], r["harness"], 0), {})
        inp = res.get("inp", 0) + res.get("cache_r", 0) + res.get("cache_w", 0)
        by_model[r["model"]].append(dict(
            ok=not r["status"].startswith(("stopped", "error")), judge=judged.get((r["task"], r["harness"], 0)),
            tests=(res.get("passed"), res.get("total")), cents=r.get("billed_cents"),
            cache=res.get("cache_r", 0) / inp if inp else None, mins=res.get("secs", 0) / 60 if res else None))
    print(f"{config}: {len(runs)} runs\n")
    print(f"{'model':22} {'n':>2} {'tests':>7} {'judge':>11} {'worst':>5} {'floor':>5} {'cents':>9} {'cache':>5} {'min':>4} {'fail q/op':>9}")
    best = []
    for model, rs in sorted(by_model.items()):
        js = [r["judge"] for r in rs if r["judge"] is not None]
        mean = statistics.mean(js) if js else None
        sd = max(statistics.stdev(js) if len(js) > 1 else 0, JUDGE_NOISE)
        floor = mean - sd / len(js) ** 0.5 if js else None
        passed = sum(r["tests"][0] or 0 for r in rs if r["tests"][1]), sum(r["tests"][1] or 0 for r in rs if r["tests"][1])
        q_fail = sum(1 for r in rs if r["ok"] and ((r["judge"] is not None and r["judge"] <= 4) or (r["tests"][1] and r["tests"][0] < r["tests"][1])))
        op_fail = sum(1 for r in rs if not r["ok"])
        cents = [r["cents"] for r in rs if r["cents"] is not None]
        med = statistics.median(cents) if cents else None
        cache = [r["cache"] for r in rs if r["cache"] is not None]
        mins = [r["mins"] for r in rs if r["mins"]]
        fmt = lambda v, f: f.format(v) if v is not None else "-"
        print(f"{model:22} {len(rs):>2} {passed[0]:>3}/{passed[1]:<3} {fmt(mean, '{:.1f}'):>5} ±{fmt(statistics.stdev(js) if len(js) > 1 else None, '{:.1f}'):>4} "
              f"{fmt(min(js) if js else None, '{:.1f}'):>5} {fmt(floor, '{:.1f}'):>5} {fmt(med, '{:.1f}'):>5}"
              f"{fmt(max(cents) if cents else None, '/{:.1f}'):>4} {fmt(statistics.median(cache) if cache else None, '{:.0%}'):>5} "
              f"{fmt(statistics.median(mins) if mins else None, '{:.0f}'):>4} {q_fail:>4}/{op_fail:<4}")
        if len(rs) >= 3 and floor is not None and floor >= 7 and (q_fail + op_fail) <= len(rs) / 5 and med is not None:
            best.append((med, model))
    print("\ncents: median/maximum billed per run; floor: judge mean minus one standard error (spread at least the judge's noise)")
    print(f"default: {min(best)[1]} (cheapest with 3+ runs, floor >= 7, at most 1 failure in 5)" if best else
          "default: none yet (no model has 3+ runs with a floor >= 7 and at most 1 failure in 5)")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "uno-1")
