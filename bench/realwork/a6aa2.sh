#!/bin/bash
# a6aa unstreamed (2026-10-26): the cache probe's unstreamed calls cached 98% on a6aa, the three streamed real runs
# (a6aa1.sh, v414-v416) 46%. Same three tasks and version with TAZUNA_STREAM=0 (v417-v419), at the same time.
# Logs to a6aa2.log.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
caffeinate -ims -w $$ &
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUB_HARNESS=tazuna-ownsubx SUBROUTER_X_KEYENV=SUBROUTER_ROUTE_TEST_KEY TAZUNA_STREAM=0
M=deepseek-v4.1-flash-1017 P=0.02,0.0004,0.08 L=417
log() { echo "$(date +%H:%M) $*" >> a6aa2.log; }
for t in rclone-archive-symlinks caddy-retry-match-expr telegraf-snmp-reconnect; do
  HARNESS_BIN=$HOME/bench-homes/tz-v89/tazuna ./sub_run.sh $M v$L $P 0.5 $t & log "$t: v89 on a6aa, unstreamed, v$L"; L=$((L+1))
done
wait; log "A6AA2-DONE"
