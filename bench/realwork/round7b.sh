#!/bin/bash
# round7.sh's second wave started early (2026-10-25): its first wave waited 100+ minutes on one 0.0.84 rclone run (v360).
# The main round7.sh was stopped; its spend guard (v360-v380) and balance pauser go on. Starts lazygit, prometheus and
# telegraf on 0.0.84/0.0.85/0.0.86 (v372-v380), waits for them and v360, then ends the pauser. Logs to round7.log.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95
M=deepseek-v4.1-flash-1017 P=0.02,0.0004,0.08 L=372 PAUSER=$1
log() { echo "$(date +%H:%M) $*" >> round7.log; }
for t in lazygit-mnemonic-prefix prometheus-exemplar-compaction telegraf-snmp-reconnect; do
  for v in v84 v85 v86; do
    HARNESS_BIN=$HOME/bench-homes/tz-$v/tazuna ./sub_run.sh $M v$L $P 0.5 $t &
    log "$t: $v v$L (wave 2, started early)"; L=$((L+1))
  done
done
wait
while pgrep -f "sub_run.sh $M v360 " >/dev/null; do sleep 30; done
kill $PAUSER 2>/dev/null; log "ROUND7-DONE"
