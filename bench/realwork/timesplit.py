#!/usr/bin/env python3
"""Where an own-loop run's wall time goes, from its session file (every event carries `t`, ms since the start): the model
(from a tool result, or the start, to the next reply), tools by kind (test or build commands, other shell, file reads and
edits), the harness's own test check and review. Over runs whose label matches a pattern.

  timesplit.py [LABEL-REGEX]      default: DeepSeek 1017 own-loop runs from v689 on (2026-10-30)
"""
import json, re, statistics as st, sys
from collections import defaultdict
from pathlib import Path

SESSIONS = Path.home() / "bench-homes/cc/.local/share/tazuna/sessions"
BUILD = re.compile(r"\bgo (test|build|vet)\b|cargo (test|build)|pytest|npm (test|run build)")


def split(path):
    t_prev, kinds, open_calls, out, last = 0, defaultdict(float), {}, {}, 0
    model_calls = 0
    for line in open(path, errors="replace"):
        try:
            d = json.loads(line)
        except ValueError:
            continue
        t, e = d.get("t", 0), d.get("e", {})
        typ = e.get("type")
        if typ == "assistant":
            kinds["model"] += t - t_prev
            model_calls += 1
            for b in e["message"].get("content", []):
                if b.get("type") == "tool_use":
                    cmd = (b.get("input") or {}).get("command", "") if b["name"] == "bash" else ""
                    open_calls[b["id"]] = (t, "tests/builds" if BUILD.search(cmd or "") else "shell" if b["name"] == "bash" else "file ops")
            t_prev = t
        elif typ == "user":
            for b in e["message"].get("content", []) if isinstance(e["message"].get("content"), list) else []:
                if b.get("type") == "tool_result" and b.get("tool_use_id") in open_calls:
                    start, kind = open_calls.pop(b["tool_use_id"])
                    kinds[kind] += t - start
            t_prev = t
        elif typ in ("harness_verify_start", "harness_review_start"):
            out[typ] = t
        elif typ == "harness_verify_done" and "harness_verify_start" in out:
            kinds["harness test check"] += t - out.pop("harness_verify_start")
            t_prev = t
        elif typ == "harness_review_done" and "harness_review_start" in out:
            kinds["review"] += t - out.pop("harness_review_start")
            t_prev = t
        last = t
    return kinds, last, model_calls


def main():
    pat = re.compile(sys.argv[1] if len(sys.argv) > 1 else r"tazuna-own[a-z]*-v(689|69\d|7[0-6]\d)@deepseek-v4.1-flash-1017")
    runs = []
    for f in SESSIONS.glob("*.jsonl"):
        if pat.search(f.name):
            kinds, total, calls = split(f)
            if total and calls:
                runs.append((f.name, kinds, total, calls))
    if not runs:
        sys.exit("no sessions match")
    total = sum(r[2] for r in runs)
    keys = ["model", "tests/builds", "shell", "file ops", "harness test check", "review"]
    print(f"{len(runs)} runs, {total / 60000 / len(runs):.1f} min a run on average, {st.mean(r[3] for r in runs):.0f} model calls a run")
    for k in keys:
        v = sum(r[1].get(k, 0) for r in runs)
        print(f"  {k:20} {100 * v / total:5.1f}%  {v / 60000 / len(runs):5.1f} min a run")
    other = total - sum(sum(r[1].values()) for r in runs)
    print(f"  {'(between events)':20} {100 * other / total:5.1f}%")
    secs_per_call = [r[1].get("model", 0) / 1000 / r[3] for r in runs]
    print(f"  model wait per call: median {st.median(secs_per_call):.1f} s, p90 {sorted(secs_per_call)[int(len(secs_per_call) * .9)]:.1f} s")


if __name__ == "__main__":
    main()
