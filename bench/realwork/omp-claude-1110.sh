#!/bin/bash
# 2026-11-09 OMP's Claude arm again, after the low round: its first try (omp-v1109) failed on every task with "Couldn't
# connect to api.anthropic.com" (the jail allows only localhost and OMP's binary ignores HTTPS_PROXY); it now reaches
# Anthropic through a local relay that passes its own login on. Opus 5.5 low, six tasks, two runs, as the other arms.
set -u
cd "$(dirname "$0")"
source ~/.bench-token
export BENCH_GATE=state GUARD_PAUSE_AT=70
until grep -q CLAUDE-LOW-DONE claude-low-1109.log 2>/dev/null; do sleep 30; done
echo "$(date +%H:%M) start" >> omp-claude-1110.log
python3 -u run.py --harness "omp-v1110@claude-opus-5-5:low" --runs 2 --workers 8 > omp-claude-1110.out 2>&1
echo "$(date +%H:%M) OMP-CLAUDE-DONE" >> omp-claude-1110.log
