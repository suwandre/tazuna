#!/bin/bash
# 2026-10-28: pi with tazuna's retry patience (run.py run_pi_sub) after stock pi lost three runs to SubRouter 503s:
# terraform-import-foreach (pi v514), caddy-retry-match-expr (pi v515), and caddy-tls-automate-names next to tazuna 0.0.97
# (pi v516, tazuna v517). deepseek-v4.1-flash-1017 at max on foxrouter (tazuna-vet-4). Spend stop 400 fen.
cd "$(dirname "$0")"; source ~/.subrouter-key
caffeinate -ims -w $$ &
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUBROUTER_X_KEYENV=SUBROUTER_VET4_KEY
M=deepseek-v4.1-flash-1017 P=0.02,0.0004,0.08
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
[ -n "$U" ] && ./sub_guard.sh "$U" 400 v514 v515 v516 v517 >> sub-guard-pi3.log 2>&1 &
SUB_SET=tasks SUB_HARNESS=pi-sub ./sub_run.sh $M v514 $P 0.5 terraform-import-foreach & J="$J $!"
SUB_SET=tasks-oct SUB_HARNESS=pi-sub ./sub_run.sh $M v515 $P 0.5 caddy-retry-match-expr & J="$J $!"
SUB_SET=tasks-hard SUB_HARNESS=pi-sub ./sub_run.sh $M v516 $P 0.5 caddy-tls-automate-names & J="$J $!"
SUB_SET=tasks-hard SUB_HARNESS=tazuna-ownsubx HARNESS_BIN=$HOME/bench-homes/tz-v97/tazuna ./sub_run.sh $M v517 $P 0.5 caddy-tls-automate-names & J="$J $!"
wait $J
