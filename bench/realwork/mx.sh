#!/bin/bash
# 2026-10-31: models x effort (high, xhigh, max) on two tasks the recent rounds did not use (tasks-oct telegraf-snmp-reconnect,
# gitea-actions-cancelling), one model at a time (its six runs at once), one run per cell. SubRouter models on tazuna's own
# loop through one route key (tazuna-route-mx, set to the model's vetted provider before its block: output under $0.50 per M,
# cached 70%+ in chatprobe.py); Sonnet and Haiku also on the subscription (Claude path). Models come from mx.todo, re-read
# before each block, so routes vetted later join the queue; done ones go to mx.done. Spend stop $5 account-wide (3500 fen);
# usage guard for the subscription. No judge here (judged by hand afterwards). Started with nohup.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> mx.log; }
caffeinate -ims -w $$ &
export GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUB_SET=tasks-oct SUB_TIMEOUT=3600 BENCH_TIMEOUT=3600 HARNESS_BIN=$HOME/bench-homes/tz-v108/tazuna
ps -axo command= | grep -q "[g]uard.py" || (cd .. && GUARD_PAUSE_AT=85 GUARD_KILL_AT=99 nohup ./guard.py >> guard.log 2>&1 &)
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
./sub_guard.sh "$U" 3500 $(seq -f "v%g" 823 900) >> sub-guard-mx.log 2>&1 &
TASKS="telegraf-snmp-reconnect gitea-actions-cancelling"
v=823
log "start; claude $(claude --version 2>/dev/null | head -1); SubRouter used $U"
while :; do
  line=$(grep -v -x -F -f mx.done mx.todo | head -1)
  [ -z "$line" ] && break
  read -r M SLUG P KIND <<< "$line"
  first=$v
  if [ "$KIND" = sub ]; then
    python3 -I -c "import sys; sys.path.insert(0, '..'); import subrouter as s; i = s.key('tazuna-route-mx', '$M', ['$SLUG'], 'SUBROUTER_ROUTE_MX_KEY'); k = s.api(f'/api/token/{i}'); k.get('include_shared_subscriptions') and s.api('/api/token/', 'PUT', {**k, 'include_shared_subscriptions': False})" >> mx.log 2>&1 || { log "key for $M failed: skipped"; echo "$line" >> mx.done; continue; }
    source ~/.subrouter-key
  fi
  for e in high xhigh max; do
    for t in $TASKS; do
      if [ "$KIND" = sub ]; then
        SUB_EFFORT=$e SUB_HARNESS=tazuna-ownsubx SUBROUTER_X_KEYENV=SUBROUTER_ROUTE_MX_KEY ./sub_run.sh "$M" "v$v" "$P" 0.5 "$t" < /dev/null &
      else
        BENCH_GATE=state BENCH_SET=tasks-oct ./run.py --runs 1 --workers 1 --tasks "$t" --harness "tazuna-v$v@$M:$e" > "bench.v$v.log" 2>&1 &
      fi
      log "start v$v $M ($SLUG) $e $t"; v=$((v + 1)); sleep 3
    done
  done
  wait $(jobs -p | grep -v "^$(pgrep -P $$ caffeinate)$") 2>/dev/null
  for i in $(seq $first $((v - 1))); do log "  v$i: $(grep -h ' r0:\|ERROR' bench.v$i.log 2>/dev/null | tail -1 | cut -c1-120)"; done
  echo "$line" >> mx.done
  log "block $M done"
done
python3 -I leakscan.py > leakscan-mx.out 2>&1
log "MX-DONE"
