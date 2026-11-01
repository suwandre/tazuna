#!/bin/bash
# 2026-10-31: mx.sh's Haiku 5.5 block (subscription, Claude path) run beside the rest instead of after it (the user's call):
# high/xhigh/max on the same two tasks, v991-v996, six at once, usage guard. Started detached.
cd "$(dirname "$0")"; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> mxh.log; }
caffeinate -ims -w $$ &
export GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 BENCH_TIMEOUT=3600 BENCH_GATE=state BENCH_SET=tasks-oct HARNESS_BIN=$HOME/bench-homes/tz-v108/tazuna
v=991
for e in high xhigh max; do
  for t in telegraf-snmp-reconnect gitea-actions-cancelling; do
    ./run.py --runs 1 --workers 1 --tasks "$t" --harness "tazuna-v$v@claude-haiku-5-5:$e" > "bench.v$v.log" 2>&1 &
    log "start v$v claude-haiku-5-5 $e $t"; v=$((v + 1)); sleep 3
  done
done
wait $(jobs -p | grep -v "^$(pgrep -P $$ caffeinate)$") 2>/dev/null
for i in $(seq 991 996); do log "  v$i: $(grep -h ' r0:\|ERROR' bench.v$i.log 2>/dev/null | tail -1 | cut -c1-120)"; done
log "MXH-DONE"
