#!/bin/bash
# Clean retest of best of two against one run (2026-10-23 night, early morning in China): gpt-6-luna at max on the LingAI
# key (LUNA4) only, tazuna 0.0.78. Per round the three tasks at once, each as one run (tazuna-ownsubx) and best of two
# (tazuna-ownsubxbest2, both attempts scored on their own as ...atta/attb). 2 rounds, labels v275-v286. Stopped at 1500
# fen (~$2); the picks wait for the usage guard. Logs to best2retest.log.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
export SUBROUTER_X_KEYENV=SUBROUTER_LUNA4_KEY SUB_KEYENV=SUBROUTER_LUNA4_KEY HARNESS_BIN=$HOME/bench-homes/tz-v78/tazuna GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUB_EFFORT=max
log() { echo "$(date +%H:%M) $*" >> best2retest.log; }
used() { curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])"; }
( cd .. && ROUTE_API=https://subrouter.ai ROUTE_KEY_ENV=SUBROUTER_LUNA4_KEY python3 -I -u uno_route.py gpt-6-luna 1 > realwork/check-best2retest.log 2>&1 )
grep -q "identity ok" check-best2retest.log || { log "route check failed: $(grep -E 'no:|Error' check-best2retest.log | head -1)"; exit 1; }
log "route check: $(grep -oE 'refused prompt_cache_key|cache [0-9]+%|long call [0-9]+ s' check-best2retest.log | tr '\n' ' ')"
U=$(used); [ -n "$U" ] || U=$(used); [ -n "$U" ] || { log "no SubRouter counter reading: not starting"; exit 1; }
./sub_guard.sh "$U" 1500 $(seq -f "v%g" 275 286) >> sub-guard-best2retest.log 2>&1 &
L=275
for r in 1 2; do
  for t in rclone-archive-symlinks caddy-unix-socket-close caddy-retry-match-expr; do
    SUB_HARNESS=tazuna-ownsubx ./sub_run.sh gpt-6-luna v$L 0.02,0.002,0.1 0.5 $t &
    SUB_HARNESS=tazuna-ownsubxbest2 ./sub_run.sh gpt-6-luna v$((L+1)) 0.02,0.002,0.1 0.5 $t &
    log "round $r $t: one run v$L, best of two v$((L+1))"; L=$((L+2))
  done
  wait $(jobs -p | tail -6); log "round $r done"
done
log "BEST2RETEST-DONE"
