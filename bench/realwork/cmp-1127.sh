#!/bin/bash
# 2026-11-27: the own loop's compaction A/B (out/review-14/compaction-design.md). DeepSeek V4.1 Flash 1017 on the Mistral
# Vibe plan (~/.mistral-vibe-key), effort high (DeepSeek's level that stands for xhigh), tazuna 0.1.235: forced compaction
# at 80k tokens (tazuna-ownmisvcmp: TAZUNA_COMPACT_AT_TOKENS=80000) against none (tazuna-ownmisv; the default 90% of the
# window never triggers on these runs), tasks-hard caddy-tls-automate-names, prometheus-replay-mmap and
# prometheus-wbl-checkpoint (own-loop peaks past 80k: caddy-tls p50 98k; the tsdb tasks' kin, prometheus-exemplar-compaction,
# p50 203k), 3 runs each, both arms interleaved in one round (same-day baseline). The plan pays: costs are at its reference
# price (TAZUNA_PRICE, API prices). No Claude use in the runs (--no-escalate); the judge runs apart (cmp-1127-judge.sh),
# after the Claude 5-hour window resets, reading the meter before each call. Start with nohup: session background
# commands die at 2 hours with their children.
cd "$(dirname "$0")" || exit 1
source ~/.mistral-vibe-key
export PATH="/opt/homebrew/bin:$PATH" HARNESS_BIN=$HOME/bench-homes/tz-v1235/tazuna BENCH_SET=tasks-hard BENCH_TIMEOUT=5400
export TAZUNA_PRICE=0.3,0.007,1.2
caffeinate -ims -w $$ &
echo "$(date +%H:%M) start ($(df -h / | tail -1 | awk '{print $4}') free)" >> cmp-1127.log
./run.py --runs 3 --workers 8 --tasks caddy-tls-automate-names,prometheus-replay-mmap,prometheus-wbl-checkpoint \
  --harness "tazuna-ownmisv-v1235@deepseek-v4-1-flash-1017:high,tazuna-ownmisvcmp-v1235@deepseek-v4-1-flash-1017:high" >> cmp-1127.out 2>&1 &
RUN=$!
# the run's own pid: a bare `wait` would wait for caffeinate too, which waits for this script
wait "$RUN"
echo "$(date +%H:%M) runs done (exit $?)" >> cmp-1127.log
