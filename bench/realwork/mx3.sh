#!/bin/bash
# 2026-10-31: mx2.sh for gpt-6.1-terra, on shared OpenAI Pro 52 alone (Pro 50 answers 404 "not supported" for Terra; Pro 52: 88% cached in respprobe.py); (real ChatGPT Pro accounts, 88-92% cached in earlier
# rounds; a key listing both pools moves a conversation off one that goes down): high/xhigh/max on the same two tasks, one
# model at a time, from mx3.todo ("MODEL PRICE", re-read before each block). Starts once mx2.sh is done (MX2-DONE); labels
# from v961; key tazuna-route-mxt. Spend stop: what is left of mx.sh's $5. Started with nohup.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> mx3.log; }
caffeinate -ims -w $$ &
until grep -q MX2-DONE mx2.log 2>/dev/null; do sleep 60; done
export GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUB_SET=tasks-oct SUB_TIMEOUT=3600 HARNESS_BIN=$HOME/bench-homes/tz-v108/tazuna
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
LEFT=$(python3 -c "print(max(0, round(3500 - ($U - 20128.85272))))")
./sub_guard.sh "$U" "$LEFT" $(seq -f "v%g" 961 990) >> sub-guard-mx3.log 2>&1 &
TASKS="telegraf-snmp-reconnect gitea-actions-cancelling"
v=961
log "start; SubRouter used $U, $LEFT fen left of the \$5"
while :; do
  line=$(grep -v -x -F -f mx3.done mx3.todo | head -1)
  [ -z "$line" ] && break
  read -r M P <<< "$line"
  first=$v
  python3 -I -c "import sys; sys.path.insert(0, '..'); import subrouter as s; i = s.key('tazuna-route-mxt', '$M', ['shared-openai-pro-52'], 'SUBROUTER_ROUTE_MXT_KEY'); k = s.api(f'/api/token/{i}'); k.get('include_shared_subscriptions') or s.api('/api/token/', 'PUT', {**k, 'include_shared_subscriptions': True})" >> mx3.log 2>&1 || { log "key for $M failed: skipped"; echo "$line" >> mx3.done; continue; }
  source ~/.subrouter-key
  for e in high xhigh max; do
    for t in $TASKS; do
      SUB_EFFORT=$e SUB_HARNESS=tazuna-ownsubx SUBROUTER_X_KEYENV=SUBROUTER_ROUTE_MXT_KEY ./sub_run.sh "$M" "v$v" "$P" 0.5 "$t" < /dev/null &
      log "start v$v $M $e $t"; v=$((v + 1)); sleep 3
    done
  done
  wait $(jobs -p | grep -v "^$(pgrep -P $$ caffeinate)$") 2>/dev/null
  for i in $(seq $first $((v - 1))); do log "  v$i: $(grep -h ' r0:\|ERROR' bench.v$i.log 2>/dev/null | tail -1 | cut -c1-120)"; done
  echo "$line" >> mx3.done
  log "block $M done"
done
python3 -I leakscan.py > leakscan-mx3.out 2>&1
log "MX3-DONE"
