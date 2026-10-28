#!/bin/bash
# 2026-10-28: TAZUNA_API2 (API_RULE narrowed to breaking changes) on DeepSeek before it becomes the default: prometheus,
# the task API_RULE was written for, twice per arm (base v576/v578, narrowed v577/v579), then the four panel tasks once
# with the narrowed rule (v580-v583) against 0.0.97's DeepSeek runs. deepseek-v4.1-flash-1017 at max on foxrouter
# (tazuna-vet-4), tz-v98q, four at a time; then the prometheus pool judged (two passes). Stops at 15% battery.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> ds-api.log; }
caffeinate -ims -w $$ &
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUBROUTER_X_KEYENV=SUBROUTER_VET4_KEY
M=deepseek-v4.1-flash-1017 P=0.02,0.0004,0.08
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
[ -n "$U" ] && ./sub_guard.sh "$U" 300 $(seq -f "v%g" 576 583) >> sub-guard-ds.log 2>&1 &
low() { b=$(pmset -g batt | grep -oE '[0-9]+%' | tr -d %); [ -n "$b" ] && [ "$b" -le 15 ] && ! pmset -g batt | grep -q "AC Power"; }
tree() { local c; for c in $(ps -axo pid=,ppid= | awk -v p="$1" '$2==p{print $1}'); do tree "$c"; done; echo "$1"; }
running() { ps -axo command= | grep -E "^/bin/bash \./sub_run\.sh $M v5(7[6-9]|8[0-3]) " | grep -vc grep; }
while true; do
  if low; then log "battery at 15%: stopping"; for p in $(ps -axo pid=,command= | grep -E "v5(7[6-9]|8[0-3])@|sub_run.sh $M v5" | grep -v grep | awk '{print $1}'); do kill $(tree $p) 2>/dev/null; done; exit 1; fi
  j=$(head -1 ds-queue.txt)
  if [ -z "$j" ]; then [ "$(running)" -eq 0 ] && break; sleep 30; continue; fi
  [ "$(running)" -ge 4 ] && { sleep 30; continue; }
  sed -i '' 1d ds-queue.txt
  set -- $j
  SUB_SET=$1 SUB_HARNESS=$2 HARNESS_BIN=$HOME/bench-homes/tz-v98q/tazuna ./sub_run.sh $M v$4 $P 0.5 $3 &
  log "start v$4 $2 $3"
  sleep 5
done
log "runs over"
python3 -I leakscan.py > leakscan-ds.out 2>&1
log "leak scan: $(grep -E 'v5(7[6-9]|8[0-3])@' leakscan-ds.out | grep -vcE '(web|peer)-(blocked|listed)(,(web|peer)-(blocked|listed))*$') runs to look at"
ps -axo command= | grep -q "[g]uard.py" || (cd .. && nohup ./guard.py >> guard.log 2>&1 &)
ONLY="tazuna-ownsubx-v576@$M:max,tazuna-ownsubx-v578@$M:max,tazuna-ownsubxa2-v577@$M:max,tazuna-ownsubxa2-v579@$M:max"
BENCH_SET=tasks-oct JUDGE_TASKS=prometheus-exemplar-compaction JUDGE_OUT=judge.dsapi.tasks-oct.jsonl JUDGE_ONLY="$ONLY" python3 -I ./judge.py --passes 2 >> judge-dsapi.log 2>&1
log "DS-API-DONE"
