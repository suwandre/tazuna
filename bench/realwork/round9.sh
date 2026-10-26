#!/bin/bash
# Prometheus rerun (2026-10-26): 0.0.88 (bash timeout up to 1800 s) against 0.0.87 (fixed 120 s; tz-v85's binary, same
# code), two each side by side (v392-v395, even: 0.0.87, odd: 0.0.88), now that run.py leaves prometheus's go.work
# working (round8's two 0.0.88 runs both changed stripeSeries.gc's signature and failed the hidden tests; the five other
# DeepSeek runs kept it). deepseek-v4.1-flash-1017 at max on the vetted route key. Keeps the Mac awake while it runs.
# Stops at 500 fen (~$0.70). Logs to round9.log.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
caffeinate -ims -w $$ &
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95
M=deepseek-v4.1-flash-1017 P=0.02,0.0004,0.08 T=prometheus-exemplar-compaction
log() { echo "$(date +%H:%M) $*" >> round9.log; }
used() { curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])"; }
U=$(used); [ -n "$U" ] || U=$(used); [ -n "$U" ] || { log "no SubRouter counter reading: not starting"; exit 1; }
./sub_guard.sh "$U" 500 v392 v393 v394 v395 >> sub-guard-round9.log 2>&1 &
run() { HARNESS_BIN=$HOME/bench-homes/tz-$1/tazuna ./sub_run.sh $M v$2 $P 0.5 $T & log "$T: $1 v$2"; }
run v85 392; run v88 393; run v85 394; run v88 395
wait $(jobs -p | tail -4); log "ROUND9-DONE"
