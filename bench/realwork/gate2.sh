#!/bin/bash
# Five-lens gate, clean redo (2026-10-24): the first test (gate1.sh, v229-v246) rests partly on runs that saw the reference
# fix. tazuna 0.0.83, deepseek-v4.1-flash-1017 at max on SubRouter's default key, alone (tazuna-ownsubm) and with the
# Opus 5.5 high five-lens review on the subscription (tazuna-ownsubgate), side by side per task: rclone, caddy-unix,
# caddy-retry; ROUNDS rounds (default 1), labels from FIRST (v335). Proxy, sandbox and leak scan as in run.py. Stops at
# 1000 fen (~$1.40) on SubRouter; the Claude review waits for the usage guard (pause at 85% of 5h, 95% of the week).
# Logs to gate2.log.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 HARNESS_BIN=$HOME/bench-homes/tz-v83/tazuna
M=deepseek-v4.1-flash-1017 P=0.02,0.0004,0.08 FIRST=${FIRST:-335} ROUNDS=${ROUNDS:-1}
log() { echo "$(date +%H:%M) $*" >> gate2.log; }
used() { curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])"; }
( cd .. && ROUTE_API=https://subrouter.ai ROUTE_KEY_ENV=SUBROUTER_MODELS_KEY python3 -I -u uno_route.py $M 1 > realwork/check-gate2.log 2>&1 )
grep -q "identity ok" check-gate2.log || { log "route check failed: $(grep -E 'no:|Error' check-gate2.log | head -1)"; exit 1; }
log "route check: $(grep -oE 'cache [0-9]+%|long call [0-9]+ s' check-gate2.log | tr '\n' ' ')"
U=$(used); [ -n "$U" ] || U=$(used); [ -n "$U" ] || { log "no SubRouter counter reading: not starting"; exit 1; }
./sub_guard.sh "$U" 1000 $(seq -f "v%g" $FIRST $((FIRST + 6 * ROUNDS - 1))) >> sub-guard-gate2.log 2>&1 &
L=$FIRST
for r in $(seq 1 $ROUNDS); do
  for t in rclone-archive-symlinks caddy-unix-socket-close caddy-retry-match-expr; do
    SUB_HARNESS=tazuna-ownsubm ./sub_run.sh $M v$L $P 0.5 $t &
    SUB_HARNESS=tazuna-ownsubgate ./sub_run.sh $M v$((L+1)) $P 0.5 $t &
    log "round $r $t: alone v$L, gate v$((L+1))"; L=$((L+2))
  done
  wait $(jobs -p | tail -6); log "round $r done"
done
log "GATE2-DONE"
