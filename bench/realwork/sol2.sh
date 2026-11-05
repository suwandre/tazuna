#!/bin/bash
# 2026-11-05: GPT-6.1 Sol on SubRouter providers, smoke test (user: Sol and other providers after Luna). All 11 probed
# answered on the Responses route (../sol-probe-1105.jsonl); one caddy-quoted-braces run in the own loop (xhigh) on eight,
# tazuna 0.1.55 frozen, own-loop runs gated on the meter only (BENCH_GATE=state). Judged afterwards with Sol in one pool
# with the DeepSeek and Luna caddy runs; the canary on the best three. SubRouter guard: stop past 420 fen (~$0.60).
# Tags: v1571 fxpayback, v1572 kriora, v1573 cheaprouter, v1574 pro-50, v1575 a6aa, v1576 pro-52, v1577 tokenx, v1578 xcgx.
cd "$(dirname "$0")" || exit 1
source ~/.bench-token
source ~/.subrouter-key
export BENCH_GATE=state PATH="/opt/homebrew/bin:$PATH" HARNESS_BIN=$HOME/bench-homes/tz-v155/tazuna BENCH_TIMEOUT=2400 TAZUNA_MAX_USD=0.15
caffeinate -ims -w $$ &
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
echo "$(date +%H:%M) start counter $U" >> sol.log
run() {  # tag, key suffix, input price, output price
  SUBROUTER_X_KEYENV="SUBROUTER_SMOKE_$2_KEY" TAZUNA_PRICE="$3,$(python3 -c "print(round($3/10,5))"),$4" ./run.py --runs 1 --workers 1 \
    --tasks caddy-quoted-braces --harness "tazuna-ownsubx-v$1@gpt-6.1-sol:xhigh" > "sol.$1.out" 2>&1
  echo "$(date +%H:%M) v$1 $2: $(grep -E "v$1@.*r0:" "sol.$1.out" | cut -c40-220)" >> sol.log
}
run 1571 FXPAYBACK 0.0049 0.0243 &
run 1572 KRIORA 0.0343 0.1714 &
run 1573 CHEAPROUTER 0.0478 0.2388 &
run 1574 SHARED_OPENAI_PRO_50 0.06 0.30 &
run 1575 A6AA 0.05 0.25 &
run 1576 SHARED_OPENAI_PRO_52 0.06 0.30 &
run 1577 TOKENX 0.0574 0.2868 &
run 1578 XCGX 0.0381 0.1904 &
sleep 30
./sub_guard.sh "$U" 420 ownsubx-v1571 ownsubx-v1572 ownsubx-v1573 ownsubx-v1574 ownsubx-v1575 ownsubx-v1576 ownsubx-v1577 ownsubx-v1578 >> sol.guard.log 2>&1 &
wait
echo "$(date +%H:%M) SOL-DONE" >> sol.log
