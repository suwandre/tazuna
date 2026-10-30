#!/usr/bin/env python3
"""Offline data for a decision model in the own loop (ROADMAP: Jev router, parked). From the bench's own-loop sessions and
their known outcomes:

  decide-stuck.jsonl  one row per run and checkpoint (every 20 model calls): what the run has done so far (calls, edits,
                      test runs and how the last one ended, calls since the last edit, repeated calls, output tokens) and
                      how it ended (hidden tests, judge, stopped). The decision to test: "will this run fail? hand over".
  decide-route.jsonl  one row per task: each arm's mean hidden pass rate, cost and minutes (DeepSeek own loop, Opus on
                      the claude path, DeepSeek with an Opus review), for routing.

Then the simple rules a decision model has to beat, scored on the checkpoints.
  decide_data.py
"""
import json, re, sys
from collections import defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from leakscan import leaked

SESSIONS = Path.home() / "bench-homes/cc/.local/share/tazuna/sessions"
SKIP = re.compile(r"rev|opus|reviewed|best2|gate|xp2|xa2|xr-|xp3|xtn|xth|plan|elide|batch|both")
TEST = re.compile(r"\bgo test\b|cargo test|pytest|npm test")


def results():
    rows = {}
    for f in HERE.glob("results*.jsonl"):
        for r in map(json.loads, f.open()):
            rows[(r["task"], r["harness"])] = r
    return rows


def judges():
    out = defaultdict(list)
    for f in HERE.glob("judge*.jsonl"):
        for line in f.open():
            try:
                j = json.loads(line)
            except ValueError:
                continue
            if "overall" in j and "harness" in j:
                out[(j["task"], j["harness"])].append(j["overall"])
    return {k: sum(v) / len(v) for k, v in out.items()}


def checkpoints(path, every=20):
    """The run's state after every `every` model calls."""
    calls = edits = tests = out = repeat = 0
    last_test, since_edit, last_call, rows = None, 0, None, []
    pending = {}
    for line in open(path, errors="replace"):
        try:
            e = json.loads(line).get("e", {})
        except ValueError:
            continue
        if e.get("type") == "stream_event" and e["event"].get("type") == "message_delta":
            calls += 1
            since_edit += 1
            out += e["event"]["usage"].get("output_tokens", 0)
            if calls % every == 0:
                rows.append(dict(calls=calls, edits=edits, tests=tests, last_test=last_test, since_edit=since_edit,
                                 repeat=repeat, out_k=out // 1000))
        elif e.get("type") == "assistant":
            for b in e["message"].get("content", []):
                if b.get("type") != "tool_use":
                    continue
                key = (b["name"], json.dumps(b.get("input"), sort_keys=True))
                repeat = repeat + 1 if key == last_call else 0
                last_call = key
                if b["name"] in ("edit", "write"):
                    edits += 1
                    since_edit = 0
                cmd = (b.get("input") or {}).get("command", "") if b["name"] == "bash" else ""
                if TEST.search(cmd or ""):
                    pending[b["id"]] = True
        elif e.get("type") == "user":
            for b in e["message"].get("content", []) if isinstance(e["message"].get("content"), list) else []:
                if b.get("type") == "tool_result" and pending.pop(b.get("tool_use_id"), False):
                    tests += 1
                    text = b.get("content") if isinstance(b.get("content"), str) else json.dumps(b.get("content"))
                    last_test = "fail" if re.search(r"FAIL|panic:|\[exit [1-9]", text or "") else "pass"
    return rows, calls


def main():
    rows, jd, bad = results(), judges(), leaked()
    stuck = []
    for f in SESSIONS.glob("*-rw-*-tazuna-own*.jsonl"):
        m = re.match(r"\d+-rw-(.+?)-(tazuna-own[^@]*@[^-]+(?:-[^-:]+)*:[a-z]+)-", f.name)
        if not m:
            continue
        task, h = m.group(1), m.group(2)
        r = rows.get((task, h))
        if not r or not r.get("total") or (task, h) in bad or SKIP.search(h.split("@")[0]):
            continue
        cps, total_calls = checkpoints(f)
        ok = r["passed"] == r["total"]
        for c in cps:
            stuck.append(dict(task=task, harness=h, model=h.split("@")[1].split(":")[0], **c, total_calls=total_calls,
                              passed=r["passed"], total=r["total"], ok=ok, judge=jd.get((task, h))))
    with open(HERE / "decide-stuck.jsonl", "w") as out:
        out.writelines(json.dumps(s) + "\n" for s in stuck)
    runs = {(s["task"], s["harness"]) for s in stuck}
    fails = {(s["task"], s["harness"]) for s in stuck if not s["ok"]}
    print(f"decide-stuck.jsonl: {len(stuck)} checkpoints from {len(runs)} runs ({len(fails)} failed a hidden test)")

    # the rules to beat: at each checkpoint, say "this run will fail"
    rules = {
        "60+ calls since the last edit": lambda s: s["since_edit"] >= 60,
        "last test run failed, 80+ calls": lambda s: s["last_test"] == "fail" and s["calls"] >= 80,
        "a call repeated 3+ times in a row": lambda s: s["repeat"] >= 2,
        "no edit by call 60": lambda s: s["edits"] == 0 and s["calls"] >= 60,
        "120+ calls": lambda s: s["calls"] >= 120,
    }
    print(f"{'rule (flags a run at its first matching checkpoint)':52} flagged  of which failed  failed runs caught  median call")
    for name, rule in rules.items():
        first = {}
        for s in stuck:
            k = (s["task"], s["harness"])
            if rule(s) and k not in first:
                first[k] = s["calls"]
        hit = [k for k in first if k in fails]
        calls = sorted(first.values())
        print(f"{name:52} {len(first):7}  {len(hit):7} ({100 * len(hit) // max(len(first), 1):3}%)  {len(hit):5}/{len(fails)}"
              f"        {calls[len(calls) // 2] if calls else '-'}")

    # routing: per task, each arm's outcome
    arm = lambda h, m: ("opus-claude" if "claude-opus" in m and h.startswith("tazuna") and "own" not in h else
                        "deepseek-own" if "deepseek" in m and h.startswith("tazuna-own") and not SKIP.search(h) else
                        "deepseek+opus-review" if "deepseek" in m and re.search(r"xopus|reviewed", h) else None)
    per = defaultdict(lambda: defaultdict(list))
    for (task, h), r in rows.items():
        a = arm(h.split("@")[0], h.split("@")[-1])
        if a and r.get("total") and (task, h) not in bad:
            per[task][a].append((r["passed"] / r["total"], r.get("cost") or 0, (r.get("secs") or 0) / 60))
    with open(HERE / "decide-route.jsonl", "w") as out:
        for task, d in sorted(per.items()):
            arms = {a: dict(n=len(x), pass_rate=round(sum(p for p, _, _ in x) / len(x), 3), cost=round(sum(c for _, c, _ in x) / len(x), 4),
                            minutes=round(sum(m for _, _, m in x) / len(x), 1)) for a, x in d.items()}
            out.write(json.dumps(dict(task=task, arms=arms)) + "\n")
    print(f"decide-route.jsonl: {len(per)} tasks, {sum(len(d) > 1 for d in per.values())} with two arms or more")


if __name__ == "__main__":
    main()
