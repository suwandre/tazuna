#!/bin/bash
# Forced switch (2026-10-25): one rclone run, tazuna 0.0.84, deepseek-v4.1-flash-1017 at max, on a test key that allows
# a6aa (cheapest, cached 23% in a check) and LingAI, cache weighting off so SubRouter starts on a6aa; `subrouter.py watch`
# must drop a6aa after 15 calls under 70% cached and the run go on on LingAI. Label v347. Stops at 300 fen (~$0.43).
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
M=deepseek-v4.1-flash-1017
log() { echo "$(date +%H:%M) $*" >> forced1.log; }
python3 -I -c "import sys; sys.path.insert(0, '..'); import subrouter as s; s.key('tazuna-route-test-1017', '$M', ['a6aa', 'lingai'], 'SUBROUTER_ROUTE_TEST_KEY', cache_weight=False)"
source ~/.subrouter-key
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 HARNESS_BIN=$HOME/bench-homes/tz-v84/tazuna
export SUBROUTER_X_KEYENV=SUBROUTER_ROUTE_TEST_KEY
used() { curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])"; }
U=$(used); [ -n "$U" ] || { log "no SubRouter counter reading: not starting"; exit 1; }
./sub_guard.sh "$U" 300 v347 >> sub-guard-forced1.log 2>&1 &
python3 -u ../subrouter.py watch $M --key tazuna-route-test-1017 --var SUBROUTER_ROUTE_TEST_KEY >> forced1-watch.log 2>&1 & W=$!
log "rclone on tazuna-route-test-1017 (a6aa, lingai): v347"
SUB_HARNESS=tazuna-ownsubx ./sub_run.sh $M v347 0.02,0.0004,0.08 0.5 rclone-archive-symlinks
kill $W; log "FORCED1-DONE"
