#!/bin/bash
# 2026-10-29: gpt-6-astra at max on the shared Pro pools, one key per harness (Codex tazuna-vet-1, pi tazuna-vet-2, tazuna
# 0.1.0 tazuna-vet-3) so the account log splits model calls, prompt size per call, cache and cost by harness:
# caddy-quoted (v642-v644) and caddy-retry (v645-v647). Four at a time; no judge (a diagnostic). Spend stop 500 fen.
cd "$(dirname "$0")"; source ~/.subrouter-key
log() { echo "$(date +%H:%M) $*" >> perkey.log; }
caffeinate -ims -w $$ &
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95
M=gpt-6-astra P=0.2,0.02,1.0
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
[ -n "$U" ] && ./sub_guard.sh "$U" 500 $(seq -f "v%g" 642 647) >> sub-guard-perkey.log 2>&1 &
running() { ps -axo command= | grep -E "^/bin/bash \./sub_run\.sh $M v64[2-7] " | grep -vc grep; }
while read -r k set h task v; do
  while [ "$(running)" -ge 4 ]; do sleep 30; done
  SUBROUTER_X_KEYENV=$k SUB_SET=$set SUB_HARNESS=$h HARNESS_BIN=$HOME/bench-homes/tz-v100/tazuna ./sub_run.sh $M v$v $P 0.5 $task < /dev/null &
  log "start v$v $h $task ($k)"; sleep 5
done < perkey-queue.txt
while [ "$(running)" -gt 0 ]; do sleep 30; done
log "PERKEY-DONE"
