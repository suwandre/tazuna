#!/bin/bash
# tazuna against stock Claude Code on a fast panel (2026-10-27): four short tasks (6-11 min for tazuna before), two of them
# hard ones whose hidden tests split runs (caddy-tls-automate-names 3/5, terraform-import-foreach 3/24), so hidden tests
# decide quality without the judge; cost and time differ on any task. Same model (deepseek-v4.1-flash-1017 at max), same
# provider (tazuna-vet-4, foxrouter), two tasks (four runs) at a time. tazuna 0.0.93: v459 v461 v463 v465; Claude Code
# (cc-sub): v460 v462 v464 v466. Logs to ccpanel.log.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUBROUTER_X_KEYENV=SUBROUTER_VET4_KEY
M=deepseek-v4.1-flash-1017 P=0.02,0.0004,0.08
log() { echo "$(date +%H:%M) $*" >> ccpanel.log; }
pair() {  # set task tazuna-label cc-label
  SUB_SET=$1 SUB_HARNESS=tazuna-ownsubx HARNESS_BIN=$HOME/bench-homes/tz-v93/tazuna ./sub_run.sh $M v$3 $P 0.5 $2 &
  SUB_SET=$1 SUB_HARNESS=cc-sub ./sub_run.sh $M v$4 $P 0.5 $2 &
  log "$2 ($1): tazuna v$3, Claude Code v$4"
}
pair tasks caddy-quoted-braces 459 460; pair tasks-hard caddy-tls-automate-names 461 462
wait; log "batch 1 done"
pair tasks prometheus-duration-parens 463 464; pair tasks terraform-import-foreach 465 466
wait; log "CCPANEL-DONE"
