#!/bin/bash
# 2026-10-28: pi against tazuna 0.0.97 at the same time, batch A: terraform-import-foreach (pi v510, tazuna v511) and
# caddy-retry-match-expr (pi v512, tazuna v513), deepseek-v4.1-flash-1017 at max on foxrouter (tazuna-vet-4). Spend stop 300 fen.
cd "$(dirname "$0")"; source ~/.subrouter-key
caffeinate -ims -w $$ &
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUBROUTER_X_KEYENV=SUBROUTER_VET4_KEY
M=deepseek-v4.1-flash-1017 P=0.02,0.0004,0.08
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
[ -n "$U" ] && ./sub_guard.sh "$U" 300 v510 v511 v512 v513 >> sub-guard-pi2.log 2>&1 &
pair() {  # set task pi-label tazuna-label
  SUB_SET=$1 SUB_HARNESS=pi-sub ./sub_run.sh $M v$3 $P 0.5 $2 & J="$J $!"
  SUB_SET=$1 SUB_HARNESS=tazuna-ownsubx HARNESS_BIN=$HOME/bench-homes/tz-v97/tazuna ./sub_run.sh $M v$4 $P 0.5 $2 & J="$J $!"
}
pair tasks terraform-import-foreach 510 511; pair tasks-oct caddy-retry-match-expr 512 513
wait $J
