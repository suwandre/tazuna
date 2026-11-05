#!/bin/bash
# 2026-11-05 evening: the own loop's identity line ("You are <model>, run by tazuna through <host>...") against none
# (TAZUNA_NO_IDENTITY), after DeepSeek answered "I'm Claude" under a repo's CLAUDE.md. The six Go tasks of tasks/ x one
# run x two arms, DeepSeek V4.1 Flash 1017 at high on its route (stableai + znu), tazuna 0.1.62 frozen (tz-v162); then
# Sol judges each task's pool. SubRouter guard: stop past 400 fen (~$0.57, account-wide: the provider gauge counts too).
cd "$(dirname "$0")" || exit 1
source ~/.bench-token
source ~/.subrouter-key
export BENCH_GATE=state PATH="/opt/homebrew/bin:$PATH" HARNESS_BIN=$HOME/bench-homes/tz-v162/tazuna BENCH_TIMEOUT=3600 TAZUNA_MAX_USD=0.15
export SUBROUTER_X_KEYENV=SUBROUTER_ROUTE_DEEPSEEK_V4_1_FLASH_1017_KEY TAZUNA_PRICE=0.0125,0.00125,0.05
caffeinate -ims -w $$ &
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
echo "$(date +%H:%M) start counter $U" >> ident.log
M=deepseek-v4.1-flash-1017
./sub_guard.sh "$U" 400 ownsubx-v1621 ownsubxni-v1621 >> ident.guard.log 2>&1 &
./run.py --runs 1 --workers 6 --harness "tazuna-ownsubx-v1621@$M:high,tazuna-ownsubxni-v1621@$M:high" >> ident.out 2>&1
echo "$(date +%H:%M) runs done" >> ident.log
export JUDGE_KEY_ENV=SUBROUTER_ROUTE_GPT_6_1_SOL_KEY
JUDGE_ONLY="tazuna-ownsubx-v1621@$M:high,tazuna-ownsubxni-v1621@$M:high" JUDGE_OUT=judge.ident.sol.jsonl \
  python3 -u judge.py --model gpt-6.1-sol@https://subrouter.ai/v1 --effort xhigh >> ident.judge.out 2>&1
echo "$(date +%H:%M) IDENT-DONE" >> ident.log
