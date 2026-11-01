#!/bin/bash
# HLE canary, 200 questions each, DeepSeek 1017 max (LingAI) then Sol xhigh (shared Pro pools), once the model round's last
# leg (mx3.sh) is done, so its spend does not count against the round's $5 stop. About $1.25 (2026-11-01). Started detached.
cd "$(dirname "$0")"; source ~/.subrouter-key
until grep -q MX3-DONE realwork/mx3.log 2>/dev/null; do sleep 60; done
echo "$(date +%H:%M) start" >> hle-run.log
python3 -I canary_hle.py run deepseek-v4.1-flash-1017 --key-env SUBROUTER_CANARY_DS_KEY --effort max --n 200 >> hle-run.log 2>&1
python3 -I canary_hle.py run gpt-6.1-sol --key-env SUBROUTER_CANARY_SOL_KEY --effort xhigh --n 200 >> hle-run.log 2>&1
echo "$(date +%H:%M) HLE-DONE" >> hle-run.log
