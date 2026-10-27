#!/bin/bash
# Single sessions per provider (2026-10-27): one conversation at a time on each provider (two tasks one after the
# other: caddy-retry, then gitea), the five providers side by side: kriora, a6aa, relai (tazuna-vet-1..3), fate86
# (tazuna-route-test-1017) and LingAI (the route key). 0.0.93, streamed, no fallback route. v443-v452 (per provider:
# caddy-retry, gitea). Logs to single2.log.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUB_HARNESS=tazuna-ownsubx
M=deepseek-v4.1-flash-1017 P=0.02,0.0004,0.08
log() { echo "$(date +%H:%M) $*" >> single2.log; }
one() {  # provider, key variable, first label
  for t in caddy-retry-match-expr gitea-actions-cancelling; do
    SUBROUTER_X_KEYENV=$2 HARNESS_BIN=$HOME/bench-homes/tz-v93/tazuna ./sub_run.sh $M v$3 $P 0.5 $t; log "$1 $t v$3 done"; set -- "$1" "$2" $(($3 + 1))
  done
}
one kriora SUBROUTER_VET1_KEY 443 & K1=$!
one a6aa SUBROUTER_VET2_KEY 445 & K2=$!
one relai SUBROUTER_VET3_KEY 447 & K3=$!
one fate86 SUBROUTER_ROUTE_TEST_KEY 449 & K4=$!
one lingai SUBROUTER_ROUTE_DEEPSEEK_V4_1_FLASH_1017_KEY 451 & K5=$!
log "started: kriora v443-444, a6aa v445-446, relai v447-448, fate86 v449-450, lingai v451-452"
wait $K1 $K2 $K3 $K4 $K5; log "SINGLE2-DONE"
