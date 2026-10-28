#!/bin/bash
# 2026-10-28 night: tazuna 0.0.97 against stock Claude Code, two runs a side on four tasks, deepseek-v4.1-flash-1017 at
# max on foxrouter (tazuna-vet-4), two pairs (four conversations) at a time:
#   B1 caddy-quoted-braces r1 (tazuna v492, Claude Code v493), terraform-import-foreach r1 (v494, v495)
#   B2 caddy-quoted-braces r2 (v496, v497), terraform-import-foreach r2 (v498, v499)
#   B3 caddy-tls-automate-names r1 (v500, v501), caddy-retry-match-expr r1 (v502, v503)
#   B4 caddy-tls-automate-names r2 (v504, v505), caddy-retry-match-expr r2 (v506, v507)
# then a leak scan, the judge (one pool per task, two passes: 8 calls, only if each dry run says what it should and before
# 06:30), night-1028-summary.txt, a commit, and sleep. Keeps the Mac awake throughout; stops the runs at 15% battery off
# power or 07:00 and sleeps anyway. Spend stop at 700 fen (~$1). Logs to night-1028.log.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> night-1028.log; }
caffeinate -ims -w $$ &
log "caffeinate on"
late() { b=$(pmset -g batt | grep -oE '[0-9]+%' | tr -d %); { [ -n "$b" ] && [ "$b" -le 15 ] && ! pmset -g batt | grep -q "AC Power"; } || { [ "$(date +%H%M)" -ge 0700 ] && [ "$(date +%H%M)" -lt 1200 ]; }; }
guard() { ps -axo command= | grep -q "[g]uard.py" || { (cd .. && nohup ./guard.py >> guard.log 2>&1 &); log "usage guard restarted"; }; }
tree() { local c; for c in $(ps -axo pid=,ppid= | awk -v p="$1" '$2==p{print $1}'); do tree "$c"; done; echo "$1"; }
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUBROUTER_X_KEYENV=SUBROUTER_VET4_KEY
M=deepseek-v4.1-flash-1017 P=0.02,0.0004,0.08
used() { curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])"; }
U=$(used); [ -n "$U" ] || U=$(used)
[ -n "$U" ] && ./sub_guard.sh "$U" 700 $(seq -f "v%g" 492 507) >> sub-guard-night1028.log 2>&1 &
START=$(date +%s)
pair() {  # set task tazuna-label cc-label
  SUB_SET=$1 SUB_HARNESS=tazuna-ownsubx HARNESS_BIN=$HOME/bench-homes/tz-v97/tazuna ./sub_run.sh $M v$3 $P 0.5 $2 & J="$J $!"
  SUB_SET=$1 SUB_HARNESS=cc-sub ./sub_run.sh $M v$4 $P 0.5 $2 & J="$J $!"
  log "$2 ($1): tazuna 0.0.97 v$3, Claude Code v$4"
}
batch() {  # waits for this batch's runs only (a bare wait would also wait for caffeinate)
  while true; do
    alive=0; for p in $J; do kill -0 $p 2>/dev/null && alive=1; done
    [ $alive = 0 ] && break
    guard
    if late; then log "battery or 07:00: stopping the runs"; for p in $J; do kill $(tree $p) 2>/dev/null; done; return 1; fi
    sleep 60
  done
  log "batch done"
}
J=""; pair tasks caddy-quoted-braces 492 493; pair tasks terraform-import-foreach 494 495; batch && \
{ J=""; pair tasks caddy-quoted-braces 496 497; pair tasks terraform-import-foreach 498 499; batch; } && \
{ J=""; pair tasks-hard caddy-tls-automate-names 500 501; pair tasks-oct caddy-retry-match-expr 502 503; batch; } && \
{ J=""; pair tasks-hard caddy-tls-automate-names 504 505; pair tasks-oct caddy-retry-match-expr 506 507; batch; }
log "runs over"
python3 -I leakscan.py > leakscan-night1028.out 2>&1
log "leak scan: $(grep -E 'v(49[2-9]|50[0-7])@' leakscan-night1028.out | grep -vc 'web-blocked$') unrefused"
guard; sleep 160
ONLY="$(for v in 492 494 496 498 500 502 504 506; do echo -n "tazuna-ownsubx-v$v@deepseek-v4.1-flash-1017:max,"; done)$(for v in 493 495 497 499 501 503 505 507; do echo -n "cc-sub-v$v@deepseek-v4.1-flash-1017:max,"; done)tazuna-repo-v34@claude-opus-5-5:medium"
judge() {  # set tasks expected-calls
  n=$(BENCH_SET=$1 JUDGE_TASKS=$2 JUDGE_OUT=judge.night1028.$1.jsonl JUDGE_ONLY="$ONLY" python3 -I ./judge.py --passes 2 --dry-run 2>&1 | grep -oE "= [0-9]+ judge calls" | grep -oE "[0-9]+")
  if [ "${n:-0}" = "$3" ] && { [ "$(date +%H%M)" -lt 0630 ] || [ "$(date +%H%M)" -ge 1200 ]; }; then
    BENCH_SET=$1 JUDGE_TASKS=$2 JUDGE_OUT=judge.night1028.$1.jsonl JUDGE_ONLY="$ONLY" python3 -I ./judge.py --passes 2 >> judge-night1028.log 2>&1
    log "judged $1: $(grep -c . judge.night1028.$1.jsonl 2>/dev/null) rows"
  else
    log "judge $1 not run: dry run said ${n:-?} (expected $3), or too late"
  fi
}
judge tasks caddy-quoted-braces,terraform-import-foreach 4
judge tasks-hard caddy-tls-automate-names 2
judge tasks-oct caddy-retry-match-expr 2
python3 -I - > night-1028-summary.txt 2>&1 <<PY
import json, os, re, statistics as st, sys, time
sys.path.insert(0, ".."); import subrouter as s
from collections import defaultdict
runs, jud = [], {}
for f in ("results.jsonl", "results-tasks-hard.jsonl", "results-tasks-oct.jsonl"):
    for r in map(json.loads, open(f)):
        m = re.search(r"-v(\d+)@", r["harness"])
        if m and 492 <= int(m[1]) <= 507: runs.append(r)
for f in ("judge.night1028.tasks.jsonl", "judge.night1028.tasks-hard.jsonl", "judge.night1028.tasks-oct.jsonl"):
    if os.path.exists(f):
        for j in map(json.loads, open(f)): jud[(j["task"], j["harness"])] = j["overall"]
who = lambda h: "Claude Code" if h.startswith("cc-sub") else "tazuna 0.0.97"
by = defaultdict(list)
for r in runs: by[(r["task"], who(r["harness"]))].append(r)
print(f"{'task':28} {'harness':14} runs  hidden  judge  turns  minutes  input M  output K")
for (t, h), rs in sorted(by.items()):
    js = [jud[(t, r["harness"])] for r in rs if (t, r["harness"]) in jud]
    print(f"{t:28} {h:14} {len(rs):4}  {sum(r['passed'] for r in rs)}/{sum(r['total'] for r in rs):<5} {st.mean(js) if js else float('nan'):5.2f}  "
          f"{st.mean(r['turns'] for r in rs):5.0f}  {st.mean(r['secs'] for r in rs)/60:7.1f}  {st.mean(r['inp']+r['cache_r']+r.get('cache_w',0) for r in rs)/1e6:7.2f}  {st.mean(r['out'] for r in rs)/1e3:8.0f}")
for h in ("tazuna 0.0.97", "Claude Code"):
    rs = [r for r in runs if who(r["harness"]) == h]; js = [jud[(r["task"], r["harness"])] for r in rs if (r["task"], r["harness"]) in jud]
    print(f"\n{h}: {len(rs)} runs, hidden {sum(r['passed'] for r in rs)}/{sum(r['total'] for r in rs)}, judge {st.mean(js) if js else float('nan'):.2f} over {len(js)}, "
          f"{sum(r['secs'] for r in rs)/3600:.1f} h, input {sum(r['inp']+r['cache_r']+r.get('cache_w',0) for r in rs)/1e6:.1f}M, output {sum(r['out'] for r in rs)/1e3:.0f}K")
keys = {int(k["id"]): k["name"] for k in (lambda d: d["items"] if isinstance(d, dict) else d)(s.api("/api/token/?p=1&page_size=100"))}
agg = defaultdict(lambda: [0, 0])
for x in s.calls(int((time.time() - $START) // 60) + 5, "deepseek-v4.1-flash-1017"):
    if keys.get(int(x["token_id"])) != "tazuna-vet-4" or int(x["created_at"]) < $START: continue
    k = "Claude Code" if json.loads(x.get("other") or "{}").get("request_path", "").endswith("messages") else "tazuna 0.0.97"
    agg[k][0] += 1; agg[k][1] += int(x["quota"])
print("\nreal cost from the account log (foxrouter key, this night):")
for k, (n, q) in agg.items(): print(f"  {k}: {n} calls, \${q / s.QUOTA_PER_USD:.4f}")
PY
log "summary written"
cd ../.. && git add bench/realwork/night-1028.sh bench/realwork/night-1028-summary.txt bench/realwork/results.jsonl bench/realwork/results-tasks-hard.jsonl bench/realwork/results-tasks-oct.jsonl bench/realwork/leaks.jsonl bench/realwork/judge.night1028.*.jsonl 2>/dev/null
git commit -q -m "bench: tazuna 0.0.97 against Claude Code, two runs a side on four tasks, judged overnight" && log "committed $(git log --oneline -1 | cut -c1-60)"
cd bench/realwork; log "sleeping"; sleep 30; pmset sleepnow
