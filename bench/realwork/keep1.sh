#!/bin/bash
# 0.0.97 (the end of each long reasoning step kept in the history as a note) against 0.0.96, tazuna only, side by side,
# deepseek-v4.1-flash-1017 at max on foxrouter (tazuna-vet-4), 2026-10-27: caddy-quoted-braces (0.0.97 v488, 0.0.96
# v489) and terraform-import-foreach (0.0.97 v490, 0.0.96 v491). Logs to keep1.log.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUBROUTER_X_KEYENV=SUBROUTER_VET4_KEY SUB_SET=tasks SUB_HARNESS=tazuna-ownsubx
M=deepseek-v4.1-flash-1017 P=0.02,0.0004,0.08
log() { echo "$(date +%H:%M) $*" >> keep1.log; }
J=""
run() { HARNESS_BIN=$HOME/bench-homes/tz-$1/tazuna ./sub_run.sh $M v$2 $P 0.5 $3 & J="$J $!"; log "$3: $1 v$2"; }
run v97 488 caddy-quoted-braces; run v96 489 caddy-quoted-braces
run v97 490 terraform-import-foreach; run v96 491 terraform-import-foreach
wait $J; log "KEEP1-DONE"
