#!/bin/bash
# 0.0.90 (own loop: "Do not change exported APIs ... unless the task asks") against 0.0.89, all seven tasks-oct tasks
# once each side by side (2026-10-26): does the rule cost anything on tasks that need API changes, and does it keep
# exported interfaces intact where it should (sigprobe: 4/9 replayed prometheus plans avoided the ExemplarStorage change
# with it, 1-2/10 without). v400-v413 (even: 0.0.89, odd: 0.0.90), fourteen at once. deepseek-v4.1-flash-1017 at max on
# the vetted route key. Keeps the Mac awake while it runs. Stops at 1000 fen (~$1.40). Logs to round11.log.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
caffeinate -ims -w $$ &
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95
M=deepseek-v4.1-flash-1017 P=0.02,0.0004,0.08
log() { echo "$(date +%H:%M) $*" >> round11.log; }
used() { curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])"; }
U=$(used); [ -n "$U" ] || U=$(used); [ -n "$U" ] || { log "no SubRouter counter reading: not starting"; exit 1; }
./sub_guard.sh "$U" 1000 $(seq -f "v%g" 400 413) >> sub-guard-round11.log 2>&1 &
run() { HARNESS_BIN=$HOME/bench-homes/tz-$1/tazuna ./sub_run.sh $M v$2 $P 0.5 $3 & log "$3: $1 v$2"; }
L=400
for t in rclone-archive-symlinks caddy-unix-socket-close caddy-retry-match-expr gitea-actions-cancelling \
         lazygit-mnemonic-prefix prometheus-exemplar-compaction telegraf-snmp-reconnect; do
  run v89 $L $t; run v90 $((L+1)) $t; L=$((L+2))
done
wait $(jobs -p | tail -14); log "ROUND11-DONE"
