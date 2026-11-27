#!/bin/bash
# 2026-11-27, 17:40: the compaction A/B's 80k arm again (cmp-1127.sh), with run.py fixed: its only_keys had dropped
# TAZUNA_COMPACT_AT_TOKENS (TOKEN in the name), so the first 80k runs ran without the threshold (moved to
# results-tasks-hard.cmp1127-nothreshold.jsonl and runs-tasks-hard-cmp1127-nothreshold/). Same binary, provider, tasks and
# day as the baseline arm, which the first round (cmp-1127.sh) finishes beside this one; 6 workers here so the two
# together stay near 8-12 agents. Start with nohup.
cd "$(dirname "$0")" || exit 1
source ~/.mistral-vibe-key
export PATH="/opt/homebrew/bin:$PATH" HARNESS_BIN=$HOME/bench-homes/tz-v1235/tazuna BENCH_SET=tasks-hard BENCH_TIMEOUT=5400
export TAZUNA_PRICE=0.3,0.007,1.2
caffeinate -ims -w $$ &
echo "$(date +%H:%M) 80k arm start ($(df -h / | tail -1 | awk '{print $4}') free)" >> cmp-1127.log
./run.py --runs 3 --workers 6 --tasks caddy-tls-automate-names,prometheus-replay-mmap,prometheus-wbl-checkpoint \
  --harness "tazuna-ownmisvcmp-v1235@deepseek-v4-1-flash-1017:high" >> cmp-1127c.out 2>&1 &
RUN=$!
wait "$RUN"
echo "$(date +%H:%M) 80k arm done (exit $?)" >> cmp-1127.log
