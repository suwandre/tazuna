#!/bin/bash
# tazuna against Claude Code with the same model on the same provider (2026-10-27, the roadmap's main item, step 1):
# deepseek-v4.1-flash-1017 at max through tazuna-vet-4 limited to foxrouter (LingAI does not serve SubRouter's Anthropic
# route: 404 "API endpoint not found"; foxrouter, kriora, a6aa, relai and fate86 do), caddy-retry, side by side: tazuna
# 0.0.93's own loop (v455) and stock Claude Code on SubRouter's Anthropic route (cc-sub, v456). Logs to ccsub2.log.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUBROUTER_X_KEYENV=SUBROUTER_VET4_KEY
M=deepseek-v4.1-flash-1017 P=0.02,0.0004,0.08 T=caddy-retry-match-expr
log() { echo "$(date +%H:%M) $*" >> ccsub2.log; }
SUB_HARNESS=tazuna-ownsubx HARNESS_BIN=$HOME/bench-homes/tz-v93/tazuna ./sub_run.sh $M v455 $P 0.5 $T & A=$!
SUB_HARNESS=cc-sub ./sub_run.sh $M v456 $P 0.5 $T & B=$!
log "$T on foxrouter: tazuna 0.0.93 v455, Claude Code v456"
wait $A $B; log "CCSUB2-DONE"
