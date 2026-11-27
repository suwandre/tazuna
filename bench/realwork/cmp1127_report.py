#!/usr/bin/env python3
"""The compaction A/B's numbers (cmp-1127.sh): per arm and per task, from results-tasks-hard.jsonl (hidden tests, cost at
the plan's reference price, calls, time), the bench home's run log (each call's input: the peak context; compactions;
paired with a result by task, arm and duration) and judge.cmp1127.r*.jsonl (Opus 5.5 overall and dimensions)."""
import json, statistics as st
from pathlib import Path

HERE = Path(__file__).resolve().parent
ARMS = {"N (none)": "tazuna-ownmisv-v1235@deepseek-v4-1-flash-1017:high", "C (80k)": "tazuna-ownmisvcmp-v1235@deepseek-v4-1-flash-1017:high"}
TASKS = ["caddy-tls-automate-names", "prometheus-replay-mmap", "prometheus-wbl-checkpoint"]
LOG = Path.home() / "bench-homes/cc/.local/share/tazuna/runs.jsonl"


def rows():
    res = [json.loads(l) for l in (HERE / "results-tasks-hard.jsonl").read_text().splitlines() if l.strip()]
    res = [r for r in res if r["harness"] in ARMS.values() and r["task"] in TASKS]
    logs = []
    for l in LOG.read_text().splitlines():
        try:
            r = json.loads(l)
        except ValueError:
            continue
        cwd = str(r.get("meta", {}).get("cwd", ""))
        for arm in ARMS.values():
            for t in TASKS:
                if f"rw-{t}-{arm.replace(':', '~')}-" in cwd:
                    pt = r.get("per_turn") or []
                    logs.append(dict(task=t, harness=arm, secs=r.get("secs", 0), peak=max((x[0] + x[1] + x[2] for x in pt), default=0),
                                     compactions=r.get("compactions", 0), calls=len(pt), ts=r.get("ts", 0), used=False))
    judged = {}
    for f in HERE.glob("judge.cmp1127.r*.jsonl"):
        for l in f.read_text().splitlines():
            j = json.loads(l)
            judged[(j["task"], j["harness"], j["run"])] = j
    for r in res:
        # the run-log line of this run: same task and arm, the closest duration (run.py's time includes tazuna's start)
        cand = [x for x in logs if x["task"] == r["task"] and x["harness"] == r["harness"] and not x["used"]]
        m = min(cand, key=lambda x: abs(x["secs"] - r["secs"]), default=None)
        if m:
            m["used"] = True
        r["log"] = m
        r["judge"] = judged.get((r["task"], r["harness"], r["run"]))
    return res


def main():
    res = rows()
    for name, arm in ARMS.items():
        rs = [r for r in res if r["harness"] == arm]
        if not rs:
            continue
        j = [r["judge"]["overall"] for r in rs if r["judge"]]
        dims = {d: [r["judge"][d] for r in rs if r["judge"] and d in r["judge"]] for d in ["correctness", "edge_cases", "code_quality", "scope", "tests"]}
        peaks = [r["log"]["peak"] for r in rs if r["log"]]
        comp = [r["log"]["compactions"] for r in rs if r["log"]]
        print(f"{name}: runs {len(rs)}, judge {st.mean(j):.2f} ({len(j)} judged)" if j else f"{name}: runs {len(rs)}, judge -",
              f"| hidden {sum(r['passed'] for r in rs)}/{sum(r['total'] for r in rs)}, suite regressions {sum(1 for r in rs if r.get('regress'))}",
              f"| cost mean ${st.mean(r['reported_cost'] for r in rs):.4f} total ${sum(r['reported_cost'] for r in rs):.3f}",
              f"| calls mean {st.mean(r['turns'] for r in rs):.1f} | min mean {st.mean(r['secs'] for r in rs) / 60:.1f}",
              f"| peak median {st.median(peaks) / 1000:.0f}k max {max(peaks) / 1000:.0f}k" if peaks else "",
              f"| compactions {comp} (mean {st.mean(comp):.1f})" if comp else "")
        print("   dims:", {d: round(st.mean(v), 2) for d, v in dims.items() if v})
        for t in TASKS:
            for r in sorted((r for r in rs if r["task"] == t), key=lambda r: r["run"]):
                lg = r["log"] or {}
                print(f"   {t[:24]:24} r{r['run']} judge {r['judge']['overall'] if r['judge'] else '-':>4} hidden {r['passed']}/{r['total']} "
                      f"regress {r.get('regress')} ${r['reported_cost']:.4f} calls {r['turns']} {r['secs'] / 60:.0f} min "
                      f"peak {lg.get('peak', 0) / 1000:.0f}k compactions {lg.get('compactions', '-')}")


if __name__ == "__main__":
    main()
