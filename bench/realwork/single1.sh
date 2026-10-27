#!/bin/bash
# A single conversation on a cheap provider (2026-10-27): the capacity probe's failures came with four conversations at
# once; with one, fate86 answered 31/31 calls, 100% cached, at ~1/10 of LingAI's input cost. 0.0.93 (streamed) on
# caddy-retry: fate86 alone through tazuna-route-test-1017 (v441, no fallback) and LingAI on the route key (v442), at the
# same time. Logs to single1.log.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUB_HARNESS=tazuna-ownsubx
M=deepseek-v4.1-flash-1017 P=0.02,0.0004,0.08 T=caddy-retry-match-expr
log() { echo "$(date +%H:%M) $*" >> single1.log; }
SUBROUTER_X_KEYENV=SUBROUTER_ROUTE_TEST_KEY HARNESS_BIN=$HOME/bench-homes/tz-v93/tazuna ./sub_run.sh $M v441 $P 0.5 $T & A=$!
SUBROUTER_X_KEYENV=SUBROUTER_ROUTE_DEEPSEEK_V4_1_FLASH_1017_KEY HARNESS_BIN=$HOME/bench-homes/tz-v93/tazuna ./sub_run.sh $M v442 $P 0.5 $T & B=$!
log "$T: fate86 v441, LingAI v442 (0.0.93, streamed)"
wait $A $B; log "SINGLE1-DONE"
