#!/bin/bash
# 2026-10-26 night: round12 (the cheap route against today's), then a leak scan, one judge round (7 tasks x 2 passes:
# 14 calls, run only if the dry run says 14 and before 06:00), round12-summary.txt (per arm: judge, hidden, turns; per
# key, provider and stream mode: calls, cached, cost from the account log), a commit of the bench data, and sleep.
# Stops waiting at 15% battery off power or 06:30 (the runs are stopped then). Logs to night-1026.log.
cd "$(dirname "$0")"; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> night-1026.log; }
caffeinate -ims -w $$ &
log "caffeinate on"
late() { b=$(pmset -g batt | grep -oE '[0-9]+%' | tr -d %); { [ -n "$b" ] && [ "$b" -le 15 ] && ! pmset -g batt | grep -q "AC Power"; } || { [ "$(date +%H%M)" -ge 0630 ] && [ "$(date +%H%M)" -lt 1200 ]; }; }
guard() { ps -axo command= | grep -q "[g]uard.py" || { (cd .. && nohup ./guard.py >> guard.log 2>&1 &); log "usage guard restarted"; }; }
tree() { local c; for c in $(ps -axo pid=,ppid= | awk -v p="$1" '$2==p{print $1}'); do tree "$c"; done; echo "$1"; }
./round12.sh & R=$!
while kill -0 $R 2>/dev/null; do
  guard
  if late; then log "battery or 06:30: stopping round12"; kill $(tree $R) 2>/dev/null; break; fi
  sleep 60
done
log "round12 over: $(tail -1 round12.log)"
python3 -I leakscan.py > leakscan-round12.out 2>&1; log "leak scan: $(grep -E 'v4(2[0-9]|3[0-3])@' leakscan-round12.out | grep -vc 'web-blocked$') unrefused"
guard; sleep 160
export BENCH_SET=tasks-oct JUDGE_OUT=judge.oct.round12.jsonl JUDGE_ONLY="$(for v in $(seq 420 433); do echo -n "tazuna-ownsubx-v$v@deepseek-v4.1-flash-1017:max,"; done)codex@gpt-6-luna:max,tazuna-repo-v34@claude-opus-5-5:medium"
n=$(python3 -I ./judge.py --passes 2 --dry-run 2>&1 | grep -oE "= [0-9]+ judge calls" | grep -oE "[0-9]+")
if [ "${n:-0}" = 14 ] && [ "$(date +%H%M)" -lt 0600 -o "$(date +%H%M)" -ge 1200 ]; then
  python3 -I ./judge.py --passes 2 > judge-round12.log 2>&1; log "judged: $(grep -c . judge.oct.round12.jsonl 2>/dev/null) rows"
else
  log "judge not run: dry run said ${n:-?} calls (expected 14), or too late"
fi
python3 -I - > round12-summary.txt 2>&1 <<'PY'
import json, os, re, sys, time
sys.path.insert(0, ".."); import subrouter as s
from collections import defaultdict
P = (0.02, 0.0004, 0.08)
arm = lambda v: "0.0.90 LingAI streamed" if v % 2 == 0 else "0.0.91 a6aa unstreamed"
rows = {int(m[1]): r for r in map(json.loads, open("results-tasks-oct.jsonl")) if (m := re.search(r"ownsubx-v(4[23]\d)@", r["harness"])) and 420 <= int(m[1]) <= 433}
jud = {(j["task"], j["harness"]): j["overall"] for j in map(json.loads, open("judge.oct.round12.jsonl"))} if os.path.exists("judge.oct.round12.jsonl") else {}
print(f"{'task':28} {'0.0.90 LingAI streamed':>34} {'0.0.91 a6aa unstreamed':>34}   (judge, hidden, turns)")
for t in sorted({r["task"] for r in rows.values()}):
    c = {arm(v): r for v, r in rows.items() if r["task"] == t}
    print(f"{t:28} " + " ".join(f"{str(jud.get((t, c[a]['harness']), '-')):>10} {c[a]['passed']}/{c[a]['total']:<3} {c[a]['turns']:>6} turns" if a in c else f"{'-':>34}" for a in ("0.0.90 LingAI streamed", "0.0.91 a6aa unstreamed")))
for a in ("0.0.90 LingAI streamed", "0.0.91 a6aa unstreamed"):
    rs = [r for v, r in rows.items() if arm(v) == a]
    js = [jud[(r["task"], r["harness"])] for r in rs if (r["task"], r["harness"]) in jud]
    print(f"{a}: runs {len(rs)} judge {sum(js)/max(1,len(js)):.2f} over {len(js)} hidden {sum(r['passed'] for r in rs)}/{sum(r['total'] for r in rs)} turns {sum(r['turns'] for r in rs)} cached {100*sum(r['cache_r'] for r in rs)/max(1,sum(r['cache_r']+r['inp'] for r in rs)):.0f}%")
keys = {int(k["id"]): k["name"] for k in (lambda d: d["items"] if isinstance(d, dict) else d)(s.api("/api/token/?p=1&page_size=100"))}
start = os.path.getmtime("round12.sh")
agg = defaultdict(lambda: [0, 0, 0, 0])
for c in s.calls(int((time.time() - start) // 60) + 5, "deepseek-v4.1-flash-1017"):
    o = json.loads(c.get("other") or "{}")
    k = (keys.get(int(c["token_id"]), str(c["token_id"]))[:28], o.get("provider_slug"), "stream" if c.get("is_stream") else "unstreamed")
    a = agg[k]; a[0] += 1; a[1] += int(c["prompt_tokens"]); a[2] += o.get("cache_tokens") or 0; a[3] += int(c["quota"])
print("\naccount log since round12 started (key, provider, stream mode):")
for (k, p, st), (n, i, cr, q) in sorted(agg.items()):
    print(f"  {k:28} {str(p):10} {st:10} {n:5} calls {100*cr//max(i,1):3}% cached  ${q/s.QUOTA_PER_USD:.4f}  ${q/s.QUOTA_PER_USD/n:.5f}/call")
sw = 0
for f in os.listdir(os.path.expanduser("~/bench-homes/cc/.local/share/tazuna/sessions")):
    if re.search(r"ownsubx-v4(2[1357]|3[13])@", f):
        sw += open(os.path.expanduser(f"~/bench-homes/cc/.local/share/tazuna/sessions/{f}")).read().count("route_switch")
print(f"\nroute switches in the a6aa arm: {sw}")
PY
log "summary written"
cd ../.. && git add bench/realwork/round12.sh bench/realwork/night-1026.sh bench/realwork/round12-summary.txt bench/realwork/results-tasks-oct.jsonl bench/realwork/leaks.jsonl bench/realwork/judge.oct.round12.jsonl 2>/dev/null
git commit -q -m "bench: round12 (a6aa unstreamed with LingAI behind it against LingAI streamed), judged overnight" && log "committed $(git log --oneline -1 | cut -c1-60)"
cd bench/realwork; log "sleeping"; sleep 30; pmset sleepnow
