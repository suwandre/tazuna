#!/bin/bash
# 2026-11-07 night: tazuna's own loop on Mistral's API (the user's $15 a month API credit, $14 left), DeepSeek V4.1 Flash
# 1017 at high: the same hosting as Vibe's runs that evening, so this arm separates Vibe's agent from Mistral's hosting.
# tazuna 0.1.73 frozen (tz-v173: reads Mistral's content blocks). MIS_TASKS picks the tasks (caddy first, to price a run:
# Mistral publishes no price for it). Per-run cap $0.60 at an assumed $0.25 / $0.025 cached / $1.00 output per M.
cd "$(dirname "$0")" || exit 1
source ~/.mistral-key
export PATH="/opt/homebrew/bin:$PATH" HARNESS_BIN=$HOME/bench-homes/tz-v173/tazuna BENCH_TIMEOUT=3600 TAZUNA_MAX_USD=0.6
export TAZUNA_PRICE=0.25,0.025,1.0
caffeinate -ims -w $$ &
echo "$(date +%H:%M) start ${MIS_TASKS:-all}" >> mis.log
./run.py --runs 1 --workers ${MIS_WORKERS:-3} ${MIS_TASKS:+--tasks "$MIS_TASKS"} --harness "tazuna-ownmis-v1731@deepseek-v4-1-flash-1017:high" >> mis.out 2>&1
echo "$(date +%H:%M) MIS-DONE ${MIS_TASKS:-all}" >> mis.log
# after the remaining tasks: Sol judges all four arms on all six tasks, one pool per task (MIS_JUDGE=1)
if [ -n "$MIS_JUDGE" ]; then
  source ~/.subrouter-key
  M=deepseek-v4.1-flash-1017
  export JUDGE_KEY_ENV=SUBROUTER_ROUTE_GPT_6_1_SOL_KEY
  JUDGE_ONLY="vibe@$M:high,vibe@$M:max,tazuna-ownsubx-v1711@$M:high,tazuna-ownmis-v1731@deepseek-v4-1-flash-1017:high" JUDGE_OUT=judge.vibe3.sol.jsonl \
    python3 -u judge.py --model gpt-6.1-sol@https://subrouter.ai/v1 --effort xhigh >> mis.judge.out 2>&1
  echo "$(date +%H:%M) MIS-JUDGED" >> mis.log
fi
