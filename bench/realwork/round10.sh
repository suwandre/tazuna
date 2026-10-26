#!/bin/bash
# 0.0.89 (tazuna's own test check reports a run past its 600 s limit as no result, not a failure) against 0.0.88, on
# prometheus, where round9's runs got false failures from that limit and spent hours on bugs that were not there
# (2026-10-26). Two each side by side, four at once as in round9 (v396-v399, even: 0.0.88, odd: 0.0.89).
# deepseek-v4.1-flash-1017 at max on the vetted route key. Keeps the Mac awake while it runs. Stops at 500 fen (~$0.70).
# Logs to round10.log.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
caffeinate -ims -w $$ &
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95
M=deepseek-v4.1-flash-1017 P=0.02,0.0004,0.08 T=prometheus-exemplar-compaction
log() { echo "$(date +%H:%M) $*" >> round10.log; }
used() { curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])"; }
U=$(used); [ -n "$U" ] || U=$(used); [ -n "$U" ] || { log "no SubRouter counter reading: not starting"; exit 1; }
./sub_guard.sh "$U" 500 v396 v397 v398 v399 >> sub-guard-round10.log 2>&1 &
run() { HARNESS_BIN=$HOME/bench-homes/tz-$1/tazuna ./sub_run.sh $M v$2 $P 0.5 $T & log "$T: $1 v$2"; }
run v88 396; run v89 397; run v88 398; run v89 399
wait $(jobs -p | tail -4); log "ROUND10-DONE"
