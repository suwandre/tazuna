#!/bin/bash
# 2026-11-09 OMP's Claude arm, third try: v1109 could not reach api.anthropic.com from the jail; v1110 went through the
# relay, which a bug of mine broke (a variable named like the new `drop` option: every relay closed the connection).
# Fixed and checked in the jail (OMP answered through the relay). Opus 5.5 low, six tasks, two runs, as the other arms.
set -u
cd "$(dirname "$0")"
source ~/.bench-token
export BENCH_GATE=state GUARD_PAUSE_AT=70
echo "$(date +%H:%M) start" >> omp-claude-1111.log
python3 -u run.py --harness "omp-v1111@claude-opus-5-5:low" --runs 2 --workers 8 > omp-claude-1111.out 2>&1
echo "$(date +%H:%M) OMP-CLAUDE-DONE" >> omp-claude-1111.log
