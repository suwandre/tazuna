#!/bin/bash
# 2026-11-07: DeepSeek V4.1 Flash 1017 in Mistral's Vibe CLI (stock, headless, the user's EDU plan; run_vibe in run.py)
# against tazuna's own loop on the SubRouter route (bangai, lingai, kriora, a6aa since 18:40), the six Go tasks, one run
# each: Vibe at high (5 tasks: caddy-quoted-braces ran as the probe at 17:08, 6.0 min, 61 calls, 5/5, ~$0.33 of Vibe
# usage) and at max (6), tazuna 0.1.71 frozen (tz-v171) at high (6); then Sol judges each task's pool. Vibe usage is
# the plan's ($150 a month; reported as $0 in export.json). SubRouter guard: stop past 300 fen (~$0.43, account-wide:
# the provider gauge counts too).
cd "$(dirname "$0")" || exit 1
source ~/.subrouter-key
export PATH="/opt/homebrew/bin:$PATH" HARNESS_BIN=$HOME/bench-homes/tz-v171/tazuna BENCH_TIMEOUT=3600 TAZUNA_MAX_USD=0.15
export SUBROUTER_X_KEYENV=SUBROUTER_ROUTE_DEEPSEEK_V4_1_FLASH_1017_KEY TAZUNA_PRICE=0.012,0.0012,0.048
caffeinate -ims -w $$ &
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
echo "$(date +%H:%M) start counter $U" >> vibe.log
M=deepseek-v4.1-flash-1017
ARMS="vibe@$M:high,vibe@$M:max,tazuna-ownsubx-v1711@$M:high"
./sub_guard.sh "$U" 300 ownsubx-v1711 >> vibe.guard.log 2>&1 &
./run.py --runs 1 --workers 6 --harness "$ARMS" --skip "caddy-quoted-braces/vibe@$M:high/0" >> vibe.out 2>&1
echo "$(date +%H:%M) runs done" >> vibe.log
export JUDGE_KEY_ENV=SUBROUTER_ROUTE_GPT_6_1_SOL_KEY
JUDGE_ONLY="$ARMS" JUDGE_OUT=judge.vibe.sol.jsonl python3 -u judge.py --model gpt-6.1-sol@https://subrouter.ai/v1 --effort xhigh >> vibe.judge.out 2>&1
echo "$(date +%H:%M) VIBE-DONE" >> vibe.log
