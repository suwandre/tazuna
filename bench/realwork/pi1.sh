#!/bin/bash
# 2026-10-28: first pi run on SubRouter (stock pi, pi's own DeepSeek V4.1 Flash settings), next to tazuna 0.0.97 at the
# same time, caddy-quoted-braces, deepseek-v4.1-flash-1017 at max on foxrouter (tazuna-vet-4). Spend stop 150 fen.
cd "$(dirname "$0")"; source ~/.subrouter-key
caffeinate -ims -w $$ &
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUBROUTER_X_KEYENV=SUBROUTER_VET4_KEY SUB_SET=tasks
M=deepseek-v4.1-flash-1017 P=0.02,0.0004,0.08
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
[ -n "$U" ] && ./sub_guard.sh "$U" 150 v508 v509 >> sub-guard-pi1.log 2>&1 &
SUB_HARNESS=pi-sub ./sub_run.sh $M v508 $P 0.5 caddy-quoted-braces & A=$!
SUB_HARNESS=tazuna-ownsubx HARNESS_BIN=$HOME/bench-homes/tz-v97/tazuna ./sub_run.sh $M v509 $P 0.5 caddy-quoted-braces & B=$!
wait $A $B
