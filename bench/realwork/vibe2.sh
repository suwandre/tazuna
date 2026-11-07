#!/bin/bash
# 2026-11-07 evening, after vibe.sh: tazuna's three missing runs of that round on the route without a6aa (bangai,
# lingai, kriora): both hugo tasks (a6aa's content filter refused them at the first call) and k6-run-once (60-minute
# timeout). Then Sol judges those three tasks' pools again with all three arms (a pool of its own file, so the scores
# compare within it). SubRouter guard: 150 fen (~$0.21, account-wide).
cd "$(dirname "$0")" || exit 1
source ~/.subrouter-key
export PATH="/opt/homebrew/bin:$PATH" HARNESS_BIN=$HOME/bench-homes/tz-v171/tazuna BENCH_TIMEOUT=3600 TAZUNA_MAX_USD=0.15
export SUBROUTER_X_KEYENV=SUBROUTER_ROUTE_DEEPSEEK_V4_1_FLASH_1017_KEY TAZUNA_PRICE=0.0153,0.00153,0.0612
caffeinate -ims -w $$ &
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
echo "$(date +%H:%M) start counter $U" >> vibe2.log
M=deepseek-v4.1-flash-1017
T=hugo-section-slug,hugo-template-return,k6-run-once
./sub_guard.sh "$U" 150 ownsubx-v1711 >> vibe2.guard.log 2>&1 &
./run.py --runs 1 --workers 3 --tasks "$T" --harness "tazuna-ownsubx-v1711@$M:high" >> vibe2.out 2>&1
echo "$(date +%H:%M) runs done" >> vibe2.log
export JUDGE_KEY_ENV=SUBROUTER_ROUTE_GPT_6_1_SOL_KEY
JUDGE_TASKS="$T" JUDGE_ONLY="vibe@$M:high,vibe@$M:max,tazuna-ownsubx-v1711@$M:high" JUDGE_OUT=judge.vibe2.sol.jsonl \
  python3 -u judge.py --model gpt-6.1-sol@https://subrouter.ai/v1 --effort xhigh >> vibe2.judge.out 2>&1
echo "$(date +%H:%M) VIBE2-DONE" >> vibe2.log
