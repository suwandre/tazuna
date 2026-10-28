#!/bin/bash
# 2026-10-28: TAZUNA_API2 on gpt-6.1-sol, caddy-retry-match-expr (v572-v575, tz-v98q): API_RULE narrowed to breaking
# changes ("adding a new exported function, or a method on a concrete type, is fine"). Under API_RULE Sol ruled out the
# exported method every passing run added and broke the upstream tests in 3 of 4 runs. Against tazuna's base runs on
# the same task (v560, v565, v566, v568, v570). Four at a time with ab-sol.sh's runs; then one judge pool, two passes.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> ab-sol.log; }
caffeinate -ims -w $$ &
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUBROUTER_X_KEYENV=SUBROUTER_VET1_KEY
M=gpt-6.1-sol P=0.04,0.002,0.2
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
[ -n "$U" ] && ./sub_guard.sh "$U" 400 v572 v573 v574 v575 >> sub-guard-sol.log 2>&1 &
running() { ps -axo command= | grep -E "^/bin/bash \./sub_run\.sh $M v5(6[6-9]|7[0-5]) " | grep -vc grep; }
mine() { ps -axo command= | grep -E "^/bin/bash \./sub_run\.sh $M v57[2-5] " | grep -vc grep; }
while true; do
  j=$(head -1 api-queue.txt)
  if [ -z "$j" ]; then [ "$(mine)" -eq 0 ] && break; sleep 30; continue; fi
  [ "$(running)" -ge 4 ] && { sleep 30; continue; }
  sed -i '' 1d api-queue.txt
  set -- $j
  SUB_SET=$1 SUB_HARNESS=$2 HARNESS_BIN=$HOME/bench-homes/tz-v98q/tazuna ./sub_run.sh $M v$4 $P 0.5 $3 &
  log "start v$4 $2"
  sleep 5
done
while grep -q "AB-SOL-DONE" ab-sol.log; [ $? -ne 0 ]; do sleep 30; done  # the other judge first: one claude at a time
log "api2 runs over"
python3 -I leakscan.py > leakscan-api.out 2>&1
log "leak scan: $(grep -E 'v57[2-5]@' leakscan-api.out | grep -vcE '(web|peer)-(blocked|listed)(,(web|peer)-(blocked|listed))*$') runs to look at"
ONLY="$(for v in 560 565 566 568 570; do echo -n "tazuna-ownsubx-v$v@$M:max,"; done)$(for v in 572 573 574 575; do echo -n "tazuna-ownsubxa2-v$v@$M:max,"; done)"
BENCH_SET=tasks-oct JUDGE_TASKS=caddy-retry-match-expr JUDGE_OUT=judge.abapi.tasks-oct.jsonl JUDGE_ONLY="$ONLY" python3 -I ./judge.py --passes 2 >> judge-abapi.log 2>&1
log "AB-API-DONE"
