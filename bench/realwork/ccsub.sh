#!/bin/bash
# tazuna against stock Claude Code, one task at a time (2026-10-27): ccsub.sh TASK TAZUNA_LABEL CC_LABEL. Same model
# (deepseek-v4.1-flash-1017 at max), same provider (tazuna-vet-4, limited to foxrouter: LingAI does not serve SubRouter's
# Anthropic route), side by side, two runs only. Logs to ccsub.log.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUBROUTER_X_KEYENV=SUBROUTER_VET4_KEY
M=deepseek-v4.1-flash-1017 P=0.02,0.0004,0.08 T=$1
log() { echo "$(date +%H:%M) $*" >> ccsub.log; }
SUB_HARNESS=tazuna-ownsubx HARNESS_BIN=$HOME/bench-homes/tz-v93/tazuna ./sub_run.sh $M v$2 $P 0.5 $T & A=$!
SUB_HARNESS=cc-sub ./sub_run.sh $M v$3 $P 0.5 $T & B=$!
log "$T on foxrouter: tazuna 0.0.93 v$2, Claude Code v$3"
wait $A $B; log "DONE $T"
