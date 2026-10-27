#!/bin/bash
# The fast panel again with 0.0.94 (DeepSeek's reasoning read from foxrouter's `reasoning` field and sent back), all at
# once (2026-10-27): batch 1's two tasks with tazuna only (v467 caddy-quoted-braces, v468 caddy-tls-automate-names),
# against Claude Code's runs at 15:09 (v460, v462); batch 2 as pairs: prometheus-duration-parens (tazuna v469, Claude Code
# v470) and terraform-import-foreach (v471, v472). deepseek-v4.1-flash-1017 at max on tazuna-vet-4 (foxrouter).
# Logs to ccpanel2.log.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUBROUTER_X_KEYENV=SUBROUTER_VET4_KEY
M=deepseek-v4.1-flash-1017 P=0.02,0.0004,0.08
log() { echo "$(date +%H:%M) $*" >> ccpanel2.log; }
tz() { SUB_SET=$1 SUB_HARNESS=tazuna-ownsubx HARNESS_BIN=$HOME/bench-homes/tz-v94/tazuna ./sub_run.sh $M v$3 $P 0.5 $2 & log "$2 ($1): tazuna 0.0.94 v$3"; }
cc() { SUB_SET=$1 SUB_HARNESS=cc-sub ./sub_run.sh $M v$3 $P 0.5 $2 & log "$2 ($1): Claude Code v$3"; }
tz tasks caddy-quoted-braces 467; tz tasks-hard caddy-tls-automate-names 468
tz tasks prometheus-duration-parens 469; cc tasks prometheus-duration-parens 470
tz tasks terraform-import-foreach 471; cc tasks terraform-import-foreach 472
wait; log "CCPANEL2-DONE"
