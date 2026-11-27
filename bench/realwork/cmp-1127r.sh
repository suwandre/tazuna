#!/bin/bash
# 2026-11-27, evening: the compaction A/B's runs that hit the 90-minute limit (prometheus-replay-mmap, both arms) while the
# two rounds ran side by side (up to 12 agents, each running the tsdb tests), again with only these on the machine. run.py
# runs only the (task, arm, run) left without a result. Same binary, provider and day. Start with nohup.
cd "$(dirname "$0")" || exit 1
source ~/.mistral-vibe-key
export PATH="/opt/homebrew/bin:$PATH" HARNESS_BIN=$HOME/bench-homes/tz-v1235/tazuna BENCH_SET=tasks-hard BENCH_TIMEOUT=5400
export TAZUNA_PRICE=0.3,0.007,1.2
caffeinate -ims -w $$ &
echo "$(date +%H:%M) reruns start ($(df -h / | tail -1 | awk '{print $4}') free)" >> cmp-1127.log
./run.py --runs 3 --workers 8 --tasks caddy-tls-automate-names,prometheus-replay-mmap,prometheus-wbl-checkpoint \
  --harness "tazuna-ownmisv-v1235@deepseek-v4-1-flash-1017:high,tazuna-ownmisvcmp-v1235@deepseek-v4-1-flash-1017:high" >> cmp-1127r.out 2>&1 &
RUN=$!
wait "$RUN"
echo "$(date +%H:%M) reruns done (exit $?)" >> cmp-1127.log
