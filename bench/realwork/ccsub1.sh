#!/bin/bash
# tazuna against Claude Code with the same model (2026-10-27, the roadmap's main item, step 1): deepseek-v4.1-flash-1017
# at max on the LingAI route key, caddy-retry, side by side: tazuna 0.0.93's own loop (v453) and stock Claude Code, the
# official CLI on SubRouter's Anthropic route (cc-sub, v454). One task first, to decide on the full round. Logs to ccsub1.log.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUBROUTER_X_KEYENV=SUBROUTER_ROUTE_DEEPSEEK_V4_1_FLASH_1017_KEY
M=deepseek-v4.1-flash-1017 P=0.02,0.0004,0.08 T=caddy-retry-match-expr
log() { echo "$(date +%H:%M) $*" >> ccsub1.log; }
SUB_HARNESS=tazuna-ownsubx HARNESS_BIN=$HOME/bench-homes/tz-v93/tazuna ./sub_run.sh $M v453 $P 0.5 $T & A=$!
SUB_HARNESS=cc-sub ./sub_run.sh $M v454 $P 0.5 $T & B=$!
log "$T: tazuna 0.0.93 v453, Claude Code v454"
wait $A $B; log "CCSUB1-DONE"
