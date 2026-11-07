#!/bin/bash
# 2026-11-08 after midnight: a second run of each Go task for tazuna on the Mistral Vibe-plan key (run index 1), then Sol
# judges five arms per task in one pool (the Vibe-key arm with both runs). Usage: ~$1.2 of the Vibe plan, ~$0.4 SubRouter.
cd "$(dirname "$0")" || exit 1
source ~/.mistral-vibe-key
export PATH="/opt/homebrew/bin:$PATH" HARNESS_BIN=$HOME/bench-homes/tz-v173/tazuna BENCH_TIMEOUT=3600 TAZUNA_MAX_USD=1.0
export TAZUNA_PRICE=0.25,0.025,1.0
caffeinate -ims -w $$ &
echo "$(date +%H:%M) start" >> misv2.log
./run.py --runs 2 --workers 3 --harness "tazuna-ownmisv-v1731@deepseek-v4-1-flash-1017:high" >> misv2.out 2>&1
echo "$(date +%H:%M) runs done" >> misv2.log
source ~/.subrouter-key
M=deepseek-v4.1-flash-1017
export JUDGE_KEY_ENV=SUBROUTER_ROUTE_GPT_6_1_SOL_KEY
JUDGE_ONLY="vibe@$M:high,vibe@$M:max,tazuna-ownsubx-v1711@$M:high,tazuna-ownmis-v1731@deepseek-v4-1-flash-1017:high,tazuna-ownmisv-v1731@deepseek-v4-1-flash-1017:high" \
  JUDGE_OUT=judge.vibe5.sol.jsonl python3 -u judge.py --model gpt-6.1-sol@https://subrouter.ai/v1 --effort xhigh >> misv2.judge.out 2>&1
echo "$(date +%H:%M) MISV2-JUDGED" >> misv2.log
