#!/bin/bash
# 2026-11-05: the caddy part of luna.sh again (the first try waited on the Claude guard's PAUSE file: BENCH_GATE=state now, own-loop runs use no Claude usage). GPT-6 Luna smoke on SubRouter providers (user: Luna again on the OpenAI Pro / Plus pools and a few others,
# against DeepSeek 1017). Of six probed (../luna-probe-1105.jsonl) four answer: kriora, shared-openai-pro-52,
# shared-openai-plus-51 (Chat and Responses) and xcgx (Responses only); pro-50 and foxrouter 503. Per provider: canary.py
# (30 AIME + 100 MMLU-Pro, xhigh; Chat route, so not xcgx) and one caddy-quoted-braces run in the own loop (xhigh, a real
# GPT level), tazuna 0.1.55 frozen, then judged with the DeepSeek caddy runs of 2026-11-04 in one pool.
# Tags: v1561 kriora, v1562 pro-52, v1563 plus-51, v1564 xcgx. SubRouter guard: stop past 200 fen (~$0.29).
cd "$(dirname "$0")" || exit 1
source ~/.bench-token
source ~/.subrouter-key
export BENCH_GATE=state PATH="/opt/homebrew/bin:$PATH" HARNESS_BIN=$HOME/bench-homes/tz-v155/tazuna BENCH_TIMEOUT=2400 TAZUNA_MAX_USD=0.10
caffeinate -ims -w $$ &
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
echo "$(date +%H:%M) start counter $U" >> luna.log
run() {  # tag, key suffix, input price, output price
  SUBROUTER_X_KEYENV="SUBROUTER_SMOKE_$2_KEY" TAZUNA_PRICE="$3,$(python3 -c "print(round($3/10,5))"),$4" ./run.py --runs 1 --workers 1 \
    --tasks caddy-quoted-braces --harness "tazuna-ownsubx-v$1@gpt-6-luna:xhigh" > "luna.$1.out" 2>&1
  echo "$(date +%H:%M) v$1 $2: $(grep -E "v$1@.*r0:" "luna.$1.out" | cut -c40-220)" >> luna.log
}
run 1561 KRIORA 0.0156 0.0782 &
run 1562 SHARED_OPENAI_PRO_52 0.003 0.015 &
run 1563 SHARED_OPENAI_PLUS_51 0.004 0.02 &
run 1564 XCGX 0.0081 0.0404 &
sleep 30
./sub_guard.sh "$U" 200 ownsubx-v1561 ownsubx-v1562 ownsubx-v1563 ownsubx-v1564 >> luna.guard.log 2>&1 &
wait
echo "$(date +%H:%M) LUNA2-DONE" >> luna.log
