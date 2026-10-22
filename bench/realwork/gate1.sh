#!/bin/bash
# Five-lens gate, first test: DeepSeek V4.1 Flash 1017 at max on SubRouter's default key, alone (tazuna-ownsubm) and with an
# Opus 5.5 high five-lens review on the subscription (tazuna-ownsubgate), side by side per task; 3 rounds of rclone,
# caddy-unix-socket-close and caddy-retry-match-expr (6 runs a round, labels v229-v246 (v211-v228: a first try, ten runs stopped by an agent's pkill, 0.0.76)), rounds 1 and 2 together. All
# stop at 1500 fen (~$2) on SubRouter; the Claude review waits for the usage guard (pause at 85% of 5h, 95% of the week).  Logs to gate1.log.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> gate1.log; }
used() { curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])"; }
( cd .. && ROUTE_API=https://subrouter.ai ROUTE_KEY_ENV=SUBROUTER_MODELS_KEY python3 -I -u uno_route.py deepseek-v4.1-flash-1017 1 > realwork/check-gate1.log 2>&1 )
grep -q "identity ok" check-gate1.log || { log "1017 check failed: $(grep -E 'no:|Error' check-gate1.log | head -1)"; exit 1; }
log "1017 check: $(grep -oE 'cache [0-9]+%|long call [^,]*' check-gate1.log | tr '\n' ' ')"
round() {  # ROUND FIRST_LABEL: 3 tasks x (alone, gate), labels FIRST..FIRST+5
  L=$2; labels=""
  for t in rclone-archive-symlinks caddy-unix-socket-close caddy-retry-match-expr; do
    SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_HARNESS=tazuna-ownsubm SUB_EFFORT=max ./sub_run.sh deepseek-v4.1-flash-1017 v$L 0.02,0.0004,0.08 0.5 $t &
    GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_HARNESS=tazuna-ownsubgate SUB_EFFORT=max ./sub_run.sh deepseek-v4.1-flash-1017 v$((L+1)) 0.02,0.0004,0.08 0.5 $t &
    log "round $1 $t: alone v$L, gate v$((L+1))"; labels="$labels v$L v$((L+1))"; L=$((L+2))
  done
  wait; log "round $1 done:$labels"
}
# rounds 1 and 2 together (12 runs; 18 Go builds at once is too much for the disk), round 3 after both; one guard for all
U=$(used); [ -n "$U" ] || U=$(used); [ -n "$U" ] || { log "no SubRouter counter reading: not starting"; exit 1; }
./sub_guard.sh "$U" 1500 $(seq -f "v%g" 229 246) >> sub-guard-gate1.log 2>&1 &
round 1 229 & round 2 235 & wait
round 3 241
log "GATE1-DONE"
