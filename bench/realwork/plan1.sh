#!/bin/bash
# 0.0.96 (plan written in the reply) against 0.0.95, tazuna only, deepseek-v4.1-flash-1017 at max on foxrouter
# (tazuna-vet-4), 2026-10-27: caddy-quoted-braces 0.0.96 v485 next to the running 0.0.95 v483; then, once the
# prometheus-duration pair and v483 are done, terraform-import-foreach 0.0.95 v486 and 0.0.96 v487. Logs to plan1.log.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUBROUTER_X_KEYENV=SUBROUTER_VET4_KEY SUB_SET=tasks SUB_HARNESS=tazuna-ownsubx
M=deepseek-v4.1-flash-1017 P=0.02,0.0004,0.08
log() { echo "$(date +%H:%M) $*" >> plan1.log; }
HARNESS_BIN=$HOME/bench-homes/tz-v96/tazuna ./sub_run.sh $M v485 $P 0.5 caddy-quoted-braces & A=$!; log "caddy-quoted-braces: 0.0.96 v485 (0.0.95 v483 running)"
while pgrep -f "sub_run.sh deepseek-v4.1-flash-1017 v4(79|80|83) " >/dev/null; do sleep 30; done
HARNESS_BIN=$HOME/bench-homes/tz-v95/tazuna ./sub_run.sh $M v486 $P 0.5 terraform-import-foreach & B=$!
HARNESS_BIN=$HOME/bench-homes/tz-v96/tazuna ./sub_run.sh $M v487 $P 0.5 terraform-import-foreach & C=$!
log "terraform-import-foreach: 0.0.95 v486, 0.0.96 v487"
wait $A $B $C; log "PLAN1-DONE"
