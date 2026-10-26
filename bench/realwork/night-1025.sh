#!/bin/bash
# 2026-10-25 night: end of round7. Waits for the last prometheus runs (v375 0.0.84, v377 0.0.86; run.py stops them at
# 3 h), judges prometheus (one pool, two passes: 2 calls, run only if the dry run says 2; same anchors as the other six
# tasks), writes round7-summary.txt (per version over all seven tasks) and exits; Claude then writes the changelog,
# commits and touches night-1025.done. A second process (night-1025-sleep.sh) keeps the Mac awake and puts it to sleep
# on that file, 45 min after this script ends, at 04:00, or at 15% battery off power. Logs to night-1025.log.
cd "$(dirname "$0")"; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> night-1025.log; }
guard() { ps -axo command= | grep -q "[g]uard.py" || { (cd .. && nohup ./guard.py >> guard.log 2>&1 &); log "usage guard restarted"; }; }
while pgrep -f "sub_run.sh deepseek-v4.1-flash-1017 v37[57] " >/dev/null; do guard; sleep 60; done
log "prometheus runs over: $(grep -E 'v37[57]@' results-tasks-oct.jsonl | grep -oE '"turns": [0-9]+|"passed": [0-9]+' | tr '\n' ' ')"
guard; sleep 160  # a fresh meter reading for the judge
ONLY="$(seq -f "tazuna-ownsubx-v%g@deepseek-v4.1-flash-1017:max" 360 380 | tr '\n' ,)codex@gpt-6-luna:max,tazuna-repo-v34@claude-opus-5-5:medium"
J="BENCH_SET=tasks-oct JUDGE_TASKS=prometheus-exemplar-compaction JUDGE_OUT=judge.oct.round7p.jsonl"
n=$(env $J JUDGE_ONLY="$ONLY" python3 -I ./judge.py --passes 2 --dry-run 2>&1 | grep -oE "= [0-9]+ judge calls" | grep -oE "[0-9]+")
if [ "${n:-0}" = 2 ]; then
  env $J JUDGE_ONLY="$ONLY" python3 -I ./judge.py --passes 2 > judge-round7p.log 2>&1
  log "judged: $(grep -c . judge.oct.round7p.jsonl 2>/dev/null) rows"
else
  log "judge not run: dry run said ${n:-?} calls (expected 2)"
fi
python3 -I - > round7-summary.txt 2>&1 <<'PY'
import json, os, re
V = ["0.0.84", "0.0.85", "0.0.86"]
P = (0.02, 0.0004, 0.08)  # $/M uncached input, cached input, output: one fixed price so providers do not skew it
ver = lambda h: (m := re.search(r"ownsubx-v(3[6-8]\d)@", h)) and 360 <= int(m[1]) <= 380 and V[(int(m[1]) - 360) % 3]
jud = {(j["task"], j["harness"]): j["overall"] for f in ("judge.oct.round7.jsonl", "judge.oct.round7p.jsonl")
       if os.path.exists(f) for j in map(json.loads, open(f))}
rows = [r for r in map(json.loads, open("results-tasks-oct.jsonl")) if ver(r["harness"])]
print(f"{'task':28} " + "  ".join(f"{v:>26}" for v in V) + "   (turns, pass, judge)")
for t in sorted({r["task"] for r in rows}):
    c = {ver(r["harness"]): r for r in rows if r["task"] == t}
    print(f"{t:28} " + "  ".join(f"{c[v]['turns']:>8} {c[v]['passed']}/{c[v]['total']:<3} {jud.get((t, c[v]['harness']), '-'):>10}"
                                  if v in c else f"{'-':>26}" for v in V))
for skip in ((), ("caddy-unix-socket-close",)):
    print(f"\ntotals{' without ' + skip[0] if skip else ''}:")
    for v in V:
        rs = [r for r in rows if ver(r["harness"]) == v and r["task"] not in skip]
        s = {k: sum(r[k] for r in rs) for k in ("turns", "inp", "cache_r", "out", "passed", "total")}
        js = [jud[(r["task"], r["harness"])] for r in rs if (r["task"], r["harness"]) in jud]
        cost = (s["inp"] * P[0] + s["cache_r"] * P[1] + s["out"] * P[2]) / 1e6
        print(f"{v} runs {len(rs)} turns {s['turns']} uncached {s['inp']/1e6:.2f}M cached {s['cache_r']/1e6:.1f}M "
              f"({100*s['cache_r']/max(1, s['inp']+s['cache_r']):.1f}%) out {s['out']/1e6:.2f}M est ${cost:.3f} "
              f"hidden {s['passed']}/{s['total']} judge {sum(js)/max(1, len(js)):.2f} over {len(js)}")
PY
log "summary written; waiting for Claude (night-1025.done)"
