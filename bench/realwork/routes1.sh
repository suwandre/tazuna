#!/bin/bash
# Route test 1 (2026-10-25): deepseek-v4.1-flash-1017 at max, tazuna 0.0.84, SubRouter's default smart key (the
# cheapest subscribed provider per call, tazuna-ownsubm) against the route key tazuna-route-deepseek-v4.1-flash-1017
# (only providers that passed `subrouter.py vet`, tazuna-ownsubx) with `subrouter.py watch` dropping a provider whose real
# calls cache under 70% over 15 calls; side by side on rclone, caddy-unix, caddy-retry, labels v341-v346 (odd: default,
# even: route key). Stops at 1000 fen (~$1.40). Logs to routes1.log, the watcher to routes1-watch.log.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 HARNESS_BIN=$HOME/bench-homes/tz-v84/tazuna
export SUBROUTER_X_KEYENV=SUBROUTER_ROUTE_DEEPSEEK_V4_1_FLASH_1017_KEY
M=deepseek-v4.1-flash-1017 P=0.02,0.0004,0.08 L=341
log() { echo "$(date +%H:%M) $*" >> routes1.log; }
used() { curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])"; }
[ -n "${!SUBROUTER_X_KEYENV}" ] || { log "no route key in ~/.subrouter-key: run subrouter.py vet first"; exit 1; }
U=$(used); [ -n "$U" ] || U=$(used); [ -n "$U" ] || { log "no SubRouter counter reading: not starting"; exit 1; }
./sub_guard.sh "$U" 1000 $(seq -f "v%g" $L $((L + 5))) >> sub-guard-routes1.log 2>&1 &
python3 -u ../subrouter.py watch $M >> routes1-watch.log 2>&1 & W=$!
for t in rclone-archive-symlinks caddy-unix-socket-close caddy-retry-match-expr; do
  SUB_HARNESS=tazuna-ownsubm ./sub_run.sh $M v$L $P 0.5 $t &
  SUB_HARNESS=tazuna-ownsubx ./sub_run.sh $M v$((L+1)) $P 0.5 $t &
  log "$t: default key v$L, route key v$((L+1))"; L=$((L+2))
done
wait $(jobs -p | tail -6); kill $W; log "ROUTES1-DONE"
