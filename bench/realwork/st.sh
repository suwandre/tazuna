#!/bin/bash
# 2026-11-04: own-loop levers from DeepSeek's own harness, DeepSeek V4.1 Flash 1017 (xhigh) on SubRouter's route
# (tazuna-route-deepseek-v4-1-flash-1017), tasks/ (six Go tasks) x two runs x three arms run together: baseline
# (ownsubx), grep/glob tools (ownsubxs, --search-tools), tools plus the few-calls rule (ownsubxsb, --batch-own); tazuna
# 0.1.51 frozen in ~/bench-homes/tz-v151. Metrics: minutes, calls, uncached input, hidden tests (the judge after the weekly
# reset). SubRouter guard: stop past 560 fen (~$0.80) from counter 25547.047839999996.
cd "$(dirname "$0")"; source ~/.bench-token; source ~/.subrouter-key; export PATH="/opt/homebrew/bin:$PATH"
export TAZUNA_PRICE=0.01103,0.0011,0.04412 TAZUNA_MAX_USD=0.15 HARNESS_BIN=$HOME/bench-homes/tz-v151/tazuna SUBROUTER_X_KEYENV=SUBROUTER_ROUTE_DEEPSEEK_V4_1_FLASH_1017_KEY BENCH_TIMEOUT=5400
caffeinate -ims -w $$ &
echo "$(date +%H:%M) start" >> st.log
./run.py --runs 2 --workers 6 --harness "tazuna-ownsubx-v1416@deepseek-v4.1-flash-1017:xhigh,tazuna-ownsubxs-v1416@deepseek-v4.1-flash-1017:xhigh,tazuna-ownsubxsb-v1416@deepseek-v4.1-flash-1017:xhigh" >> st.out 2>&1 &
sleep 30
./sub_guard.sh 25547.047839999996 560 ownsubx-v1416 ownsubxs-v1416 ownsubxsb-v1416 >> st.guard.log 2>&1 &
wait
echo "$(date +%H:%M) ST-DONE" >> st.log
