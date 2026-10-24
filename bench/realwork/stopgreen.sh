#!/bin/bash
# A/B of 0.0.80's stop-after-green notes (2026-10-24): deepseek-v4.1-flash-1017 (a first start on Flash, v293-v298, was stopped: the key served it from PixStudio at ~5x the usual price) at max on SubRouter's default key, tazuna
# 0.0.79 against 0.0.80, the three tasks at once per round, 2 rounds, labels v305-v316 (odd: 0.0.79, even: 0.0.80); round 2 stopped at ~$0.90 spent (PixStudio). Then 0.0.81 (the note as
# its own user turn) against 0.0.79, one round, labels v317-v322, on whichever provider the key picks (a6aa at the smoke test).
# Then 0.0.79b (0.0.79 plus 0.0.83's tool-call id fix) against 0.0.83 (note, whole-file read, edit errors, hints), labels v329-v334.
# Clean environment: run.py's proxy and sandbox, leak scan after each run. Stopped at 1500 fen (~$2). Logs to stopgreen.log.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
export SUB_KEYENV=SUBROUTER_MODELS_KEY GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUB_EFFORT=max
M=deepseek-v4.1-flash-1017 P=0.02,0.0004,0.08
log() { echo "$(date +%H:%M) $*" >> stopgreen.log; }
used() { curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])"; }
( cd .. && ROUTE_API=https://subrouter.ai ROUTE_KEY_ENV=SUBROUTER_MODELS_KEY python3 -I -u uno_route.py $M 1 > realwork/check-stopgreen.log 2>&1 )
grep -q "identity ok" check-stopgreen.log || { log "route check failed: $(grep -E 'no:|Error' check-stopgreen.log | head -1)"; exit 1; }
log "route check: $(grep -oE 'refused prompt_cache_key|cache [0-9]+%|long call [0-9]+ s' check-stopgreen.log | tr '\n' ' ')"
U=$(used); [ -n "$U" ] || U=$(used); [ -n "$U" ] || { log "no SubRouter counter reading: not starting"; exit 1; }
./sub_guard.sh "$U" ${LIMIT:-1500} $(seq -f "v%g" ${FIRST:-305} $((${FIRST:-305} + 6 * ${ROUNDS:-2} - 1))) >> sub-guard-stopgreen.log 2>&1 &
L=${FIRST:-305}
for r in $(seq 1 ${ROUNDS:-2}); do
  for t in rclone-archive-symlinks caddy-unix-socket-close caddy-retry-match-expr; do
    HARNESS_BIN=$HOME/bench-homes/tz-${OLD:-v79}/tazuna ./sub_run.sh $M v$L $P 0.5 $t &
    HARNESS_BIN=$HOME/bench-homes/tz-${NEW:-v80}/tazuna ./sub_run.sh $M v$((L+1)) $P 0.5 $t &
    log "round $r $t: ${OLD:-v79} v$L, ${NEW:-v80} v$((L+1))"; L=$((L+2))
  done
  wait $(jobs -p | tail -6); log "round $r done"
done
log "STOPGREEN-DONE"
