#!/bin/bash
# 2026-11-07 late: tazuna's own loop on Mistral's API with a key made under Code > Vibe CLI (~/.mistral-vibe-key), which
# bills the plan's Vibe Code usage (caddy at 23:11: the user's Vibe Code meter moved, API usage did not). All six Go tasks
# (caddy done), cap $1.00 a run so the two large tasks finish (the API arm's stopped at $0.60), then Sol judges five arms
# on all six tasks in one pool per task. tazuna sends no Vibe identity of any kind.
cd "$(dirname "$0")" || exit 1
source ~/.mistral-vibe-key
export PATH="/opt/homebrew/bin:$PATH" HARNESS_BIN=$HOME/bench-homes/tz-v173/tazuna BENCH_TIMEOUT=3600 TAZUNA_MAX_USD=1.0
export TAZUNA_PRICE=0.25,0.025,1.0
caffeinate -ims -w $$ &
echo "$(date +%H:%M) start" >> misv.log
./run.py --runs 1 --workers 3 --harness "tazuna-ownmisv-v1731@deepseek-v4-1-flash-1017:high" >> misv.out 2>&1
echo "$(date +%H:%M) runs done" >> misv.log
source ~/.subrouter-key
M=deepseek-v4.1-flash-1017
export JUDGE_KEY_ENV=SUBROUTER_ROUTE_GPT_6_1_SOL_KEY
JUDGE_ONLY="vibe@$M:high,vibe@$M:max,tazuna-ownsubx-v1711@$M:high,tazuna-ownmis-v1731@deepseek-v4-1-flash-1017:high,tazuna-ownmisv-v1731@deepseek-v4-1-flash-1017:high" \
  JUDGE_OUT=judge.vibe4.sol.jsonl python3 -u judge.py --model gpt-6.1-sol@https://subrouter.ai/v1 --effort xhigh >> misv.judge.out 2>&1
echo "$(date +%H:%M) MISV-JUDGED" >> misv.log
