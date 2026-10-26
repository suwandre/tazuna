#!/bin/bash
# a6aa on real runs (2026-10-26): the provider cache probe had it at 98% cached with no warm miss and ~1/10 of LingAI's
# cost per input token (two days before it cached 13-22%). 0.0.89 on rclone, caddy-retry and telegraf through
# tazuna-route-test-1017 limited to a6aa (v414-v416), alongside round11's 0.0.89 runs on LingAI (v400, v404, v412).
# Logs to a6aa1.log.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
caffeinate -ims -w $$ &
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUB_HARNESS=tazuna-ownsubx SUBROUTER_X_KEYENV=SUBROUTER_ROUTE_TEST_KEY
M=deepseek-v4.1-flash-1017 P=0.02,0.0004,0.08 L=414
log() { echo "$(date +%H:%M) $*" >> a6aa1.log; }
for t in rclone-archive-symlinks caddy-retry-match-expr telegraf-snmp-reconnect; do
  HARNESS_BIN=$HOME/bench-homes/tz-v89/tazuna ./sub_run.sh $M v$L $P 0.5 $t & log "$t: v89 on a6aa v$L"; L=$((L+1))
done
wait; log "A6AA1-DONE"
