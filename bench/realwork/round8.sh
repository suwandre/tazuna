#!/bin/bash
# 0.0.88 (bash takes a timeout up to 1800 s) against 0.0.87 (fixed 120 s; tz-v85's binary, same code), 2026-10-26:
# prometheus twice each side by side (v381-v384, odd: 0.0.87, even: 0.0.88), where 0.0.85-0.0.86 polled background test
# runs 33-47 times a run; and 0.0.88 once on the other six tasks (v385-v390) against round7's 0.0.85 runs, for
# regressions. deepseek-v4.1-flash-1017 at max on the vetted route key, all ten at once. Stops at 1000 fen (~$1.40).
# Logs to round8.log.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95
M=deepseek-v4.1-flash-1017 P=0.02,0.0004,0.08
log() { echo "$(date +%H:%M) $*" >> round8.log; }
used() { curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])"; }
U=$(used); [ -n "$U" ] || U=$(used); [ -n "$U" ] || { log "no SubRouter counter reading: not starting"; exit 1; }
./sub_guard.sh "$U" 1000 $(seq -f "v%g" 381 390) >> sub-guard-round8.log 2>&1 &
run() { HARNESS_BIN=$HOME/bench-homes/tz-$1/tazuna ./sub_run.sh $M v$2 $P 0.5 $3 & log "$3: $1 v$2"; }
run v85 381 prometheus-exemplar-compaction; run v88 382 prometheus-exemplar-compaction
run v85 383 prometheus-exemplar-compaction; run v88 384 prometheus-exemplar-compaction
L=385
for t in rclone-archive-symlinks caddy-unix-socket-close caddy-retry-match-expr gitea-actions-cancelling \
         lazygit-mnemonic-prefix telegraf-snmp-reconnect; do run v88 $L $t; L=$((L+1)); done
wait $(jobs -p | tail -10); log "ROUND8-DONE"
