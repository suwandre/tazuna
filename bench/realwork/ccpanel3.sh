#!/bin/bash
# The fast panel as pairs on 0.0.94 (2026-10-27 evening, Chinese night: foxrouter held 4 conversations at 96% cached in
# the capacity probe, but only ~3 in the Chinese evening): two pairs at a time, each tazuna 0.0.94 against stock Claude
# Code on the same task, deepseek-v4.1-flash-1017 at max on tazuna-vet-4 (foxrouter).
# Batch A: terraform-import-foreach (tazuna v473, Claude Code v474), caddy-quoted-braces (v475, v476).
# Batch B: caddy-tls-automate-names (v477, v478), prometheus-duration-parens (v479, v480). Logs to ccpanel3.log.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUBROUTER_X_KEYENV=SUBROUTER_VET4_KEY
M=deepseek-v4.1-flash-1017 P=0.02,0.0004,0.08
log() { echo "$(date +%H:%M) $*" >> ccpanel3.log; }
pair() {
  SUB_SET=$1 SUB_HARNESS=tazuna-ownsubx HARNESS_BIN=$HOME/bench-homes/tz-v94/tazuna ./sub_run.sh $M v$3 $P 0.5 $2 & J="$J $!"
  SUB_SET=$1 SUB_HARNESS=cc-sub ./sub_run.sh $M v$4 $P 0.5 $2 & J="$J $!"
  log "$2 ($1): tazuna 0.0.94 v$3, Claude Code v$4"
}
J=""; pair tasks terraform-import-foreach 473 474; pair tasks caddy-quoted-braces 475 476
wait $J; log "batch A done"
J=""; pair tasks-hard caddy-tls-automate-names 477 478; pair tasks prometheus-duration-parens 479 480
wait $J; log "CCPANEL3-DONE"
