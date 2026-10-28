#!/bin/bash
# 2026-10-28: second pi round next to tazuna 0.0.97 at the same time (pi with tazuna's retry patience), two pairs at a time:
#   C terraform-import-foreach (pi v518, tazuna v519), caddy-quoted-braces (pi v520, tazuna v521)
#   D caddy-tls-automate-names (pi v522, tazuna v523), caddy-retry-match-expr (pi v524, tazuna v525)
# deepseek-v4.1-flash-1017 at max on foxrouter (tazuna-vet-4). Spend stop 600 fen.
cd "$(dirname "$0")"; source ~/.subrouter-key
caffeinate -ims -w $$ &
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUBROUTER_X_KEYENV=SUBROUTER_VET4_KEY
M=deepseek-v4.1-flash-1017 P=0.02,0.0004,0.08
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
[ -n "$U" ] && ./sub_guard.sh "$U" 600 $(seq -f "v%g" 518 525) >> sub-guard-pi4.log 2>&1 &
pair() {  # set task pi-label tazuna-label
  SUB_SET=$1 SUB_HARNESS=pi-sub ./sub_run.sh $M v$3 $P 0.5 $2 & J="$J $!"
  SUB_SET=$1 SUB_HARNESS=tazuna-ownsubx HARNESS_BIN=$HOME/bench-homes/tz-v97/tazuna ./sub_run.sh $M v$4 $P 0.5 $2 & J="$J $!"
}
J=""; pair tasks terraform-import-foreach 518 519; pair tasks caddy-quoted-braces 520 521; wait $J
J=""; pair tasks-hard caddy-tls-automate-names 522 523; pair tasks-oct caddy-retry-match-expr 524 525; wait $J
