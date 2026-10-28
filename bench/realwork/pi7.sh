#!/bin/bash
# 2026-10-28: pi terraform-import-foreach again (v532) for v527, which SubRouter 503s ended after pi's 11 retries; next to
# pi6.sh's caddy-retry pair (three conversations). foxrouter, tazuna-vet-4. Spend stop 200 fen.
cd "$(dirname "$0")"; source ~/.subrouter-key
caffeinate -ims -w $$ &
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUBROUTER_X_KEYENV=SUBROUTER_VET4_KEY
M=deepseek-v4.1-flash-1017 P=0.02,0.0004,0.08
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
[ -n "$U" ] && ./sub_guard.sh "$U" 200 v532 >> sub-guard-pi7.log 2>&1 &
SUB_SET=tasks SUB_HARNESS=pi-sub ./sub_run.sh $M v532 $P 0.5 terraform-import-foreach
