#!/bin/bash
# 2026-10-28: the pi panel's remaining runs under the fixed jail (no network but localhost, no reads of other runs' work
# trees or of transcripts): pi terraform-import-foreach twice (v526, v527), caddy-tls-automate-names pi v528 and tazuna
# 0.0.97 v529; then caddy-retry-match-expr pi v530 and tazuna v531. deepseek-v4.1-flash-1017 at max on foxrouter
# (tazuna-vet-4), four conversations at most. Spend stop 600 fen.
cd "$(dirname "$0")"; source ~/.subrouter-key
caffeinate -ims -w $$ &
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUBROUTER_X_KEYENV=SUBROUTER_VET4_KEY
M=deepseek-v4.1-flash-1017 P=0.02,0.0004,0.08
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
[ -n "$U" ] && ./sub_guard.sh "$U" 600 $(seq -f "v%g" 526 531) >> sub-guard-pi5.log 2>&1 &
one() { SUB_SET=$1 SUB_HARNESS=$2 HARNESS_BIN=$HOME/bench-homes/tz-v97/tazuna ./sub_run.sh $M v$4 $P 0.5 $3 & J="$J $!"; }
J=""; one tasks pi-sub terraform-import-foreach 526; one tasks pi-sub terraform-import-foreach 527
one tasks-hard pi-sub caddy-tls-automate-names 528; one tasks-hard tazuna-ownsubx caddy-tls-automate-names 529; wait $J
J=""; one tasks-oct pi-sub caddy-retry-match-expr 530; one tasks-oct tazuna-ownsubx caddy-retry-match-expr 531; wait $J
