#!/bin/bash
# Best of two vs one run, side by side: gpt-6-luna at max on the 6 Luna key (OpenAI Pro + FoxRouter; tazuna 0.0.78, which
# drops prompt_cache_key when SubRouter refuses it). Per round and task: one run (tazuna-ownsubx) and best of two
# (tazuna-ownsubxbest2: two attempts, Opus on the subscription picks with their test results). 3 rounds of rclone,
# caddy-unix-socket-close and caddy-retry-match-expr (labels v257-v274), one round at a time (9 Luna processes). All
# stopped at 1500 fen (~$2) on SubRouter; the picks wait for the usage guard (85% of 5h, 95% of the week). Logs: best2test.log
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
export SUBROUTER_X_KEYENV=SUBROUTER_LUNA3_KEY SUB_KEYENV=SUBROUTER_LUNA3_KEY HARNESS_BIN=$HOME/bench-homes/tz-v78/tazuna GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUB_EFFORT=max
log() { echo "$(date +%H:%M) $*" >> best2test.log; }
used() { curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])"; }
( cd .. && ROUTE_API=https://subrouter.ai ROUTE_KEY_ENV=SUBROUTER_LUNA3_KEY python3 -I -u uno_route.py gpt-6-luna 1 > realwork/check-best2test.log 2>&1 )
grep -q "identity ok" check-best2test.log || { log "route check failed: $(grep -E 'no:|Error' check-best2test.log | head -1)"; exit 1; }
log "route check: $(grep -oE 'refused prompt_cache_key|cache [0-9]+%|long call [0-9]+ s' check-best2test.log | tr '\n' ' ')"
U=$(used); [ -n "$U" ] || U=$(used); [ -n "$U" ] || { log "no SubRouter counter reading: not starting"; exit 1; }
./sub_guard.sh "$U" 1500 $(seq -f "v%g" 257 274) >> sub-guard-best2test.log 2>&1 &
L=257
for r in 1 2 3; do
  for t in rclone-archive-symlinks caddy-unix-socket-close caddy-retry-match-expr; do
    SUB_HARNESS=tazuna-ownsubx ./sub_run.sh gpt-6-luna v$L 0.0082,0.0008,0.041 0.5 $t &
    SUB_HARNESS=tazuna-ownsubxbest2 ./sub_run.sh gpt-6-luna v$((L+1)) 0.0082,0.0008,0.041 0.5 $t &
    log "round $r $t: one run v$L, best of two v$((L+1))"; L=$((L+2))
  done
  wait $(jobs -p | tail -6); log "round $r done"
done
log "BEST2TEST-DONE"
