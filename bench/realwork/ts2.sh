#!/bin/bash
# 2026-11-05: --search-tools measured again after the prompt fix (8e7e7bc: the prompt named grep in bash while the tools
# were on), and o2oai (best of the 2026-11-04 provider screen) on the six Go tasks of tasks/. DeepSeek V4.1 Flash 1017
# xhigh, tazuna 0.1.55 frozen (tz-v155), two runs each, three arms:
#   ownsubx-v1551   the refreshed route (stableai + znu)          ownsubxs-v1551  the same with --search-tools
#   ownsubx-v1552   o2oai alone (key tazuna-route-smoke-o2oai)
# Judge: GPT-6.1 Sol xhigh (stands in for Opus, 2026-11-05), one call per task. SubRouter guard: stop past 400 fen (~$0.57).
cd "$(dirname "$0")"; source ~/.bench-token; source ~/.subrouter-key; export PATH="/opt/homebrew/bin:$PATH"
export HARNESS_BIN=$HOME/bench-homes/tz-v155/tazuna BENCH_TIMEOUT=5400 TAZUNA_MAX_USD=0.10
caffeinate -ims -w $$ &
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
echo "$(date +%H:%M) start counter $U" >> ts2.log
M=deepseek-v4.1-flash-1017
SUBROUTER_X_KEYENV=SUBROUTER_ROUTE_DEEPSEEK_V4_1_FLASH_1017_KEY TAZUNA_PRICE=0.0125,0.00125,0.05 ./run.py --runs 2 --workers 5 \
  --harness "tazuna-ownsubx-v1551@$M:xhigh,tazuna-ownsubxs-v1551@$M:xhigh" >> ts2.out 2>&1 &
SUBROUTER_X_KEYENV=SUBROUTER_SMOKE_O2OAI_KEY TAZUNA_PRICE=0.0159,0.00159,0.0635 ./run.py --runs 2 --workers 3 \
  --harness "tazuna-ownsubx-v1552@$M:xhigh" >> ts2.o2o.out 2>&1 &
sleep 30
./sub_guard.sh "$U" 400 ownsubx-v1551 ownsubxs-v1551 ownsubx-v1552 >> ts2.guard.log 2>&1 &
wait
echo "$(date +%H:%M) runs done" >> ts2.log
export JUDGE_KEY_ENV=SUBROUTER_ROUTE_GPT_6_1_SOL_KEY
JUDGE_ONLY="tazuna-ownsubx-v1551@$M:xhigh,tazuna-ownsubxs-v1551@$M:xhigh,tazuna-ownsubx-v1552@$M:xhigh" JUDGE_OUT=judge.ts2.sol.jsonl \
  python3 -u judge.py --model gpt-6.1-sol@https://subrouter.ai/v1 --effort xhigh >> ts2.judge.out 2>&1
echo "$(date +%H:%M) TS2-DONE" >> ts2.log
