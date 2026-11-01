#!/bin/bash
# 2026-11-01: DeepSeek 1017 on znu, the provider the gated route refresh put first (long context 88% vs AA 83%, listed
# ~47% under LingAI): the same four tasks x two runs as xh.sh's LingAI runs (v768-v782, "xhigh", which DeepSeek runs as
# high), at high, v1101-v1108, all at once. SubRouter only, so no Claude gate. Spend stop: account-wide $1.15 from now,
# i.e. $0.25 for these runs over the route scan's $0.90 still running. Started detached.
cd "$(dirname "$0")"; source ~/.subrouter-key
log() { echo "$(date +%H:%M) $*" >> znu.log; }
caffeinate -ims -w $$ &
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
./sub_guard.sh "$U" 805 $(seq -f "v%g" 1101 1108) >> sub-guard-znu.log 2>&1 &
log "start; SubRouter used $U"
while read -r set task v; do
  SUB_GATE= SUB_SET=$set SUB_EFFORT=high SUB_HARNESS=tazuna-ownsubx SUBROUTER_X_KEYENV=SUBROUTER_ROUTE_ZNU_KEY SUB_TIMEOUT=3600 HARNESS_BIN=$HOME/bench-homes/tz-v108/tazuna \
    ./sub_run.sh deepseek-v4.1-flash-1017 "v$v" 0.0118,0.0012,0.047 0.3 "$task" < /dev/null &
  log "start v$v $task"; sleep 5
done < znu.jobs
wait $(jobs -p | grep -v "^$(pgrep -P $$ caffeinate)$") 2>/dev/null
for v in $(seq 1101 1108); do log "  v$v: $(grep -h ' r0:\|ERROR' bench.v$v.log 2>/dev/null | tail -1 | cut -c1-120)"; done
log "ZNU-DONE"
