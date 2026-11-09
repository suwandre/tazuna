#!/bin/bash
# 2026-11-09 Claude round, low effort first (medium and high later): Opus 5.5 at low, the six Go tasks, two runs each.
# Arms: tazuna 0.1.83 with boss mode (what `--run` does on Opus) and without (a live session's default), stock Claude
# Code 2.1.319, OMP 18.18.3 (on the user's claude.ai login, copied into its bench home; the user's call). The gauge's
# caddy runs (r0) count: run.py skips jobs already in results.jsonl. Starts after the own-loop smoke test (no CPU
# contention in the times). The usage guard pauses new runs at 70% of the 5-hour window; they go on after its reset.
set -u
cd "$(dirname "$0")"
source ~/.bench-token
export BENCH_GATE=state GUARD_PAUSE_AT=70
log() { echo "$(date +%H:%M) $*" >> claude-low-1109.log; }
reading() { python3 -I -c "import json;s=json.load(open('../guard.state.json'));print(f\"5h {s['five_hour']}% 7d {s['seven_day']}% extra {s['extra']}\")"; }
until grep -q SMOKE-DONE smoke-own-1109.log 2>/dev/null; do sleep 30; done
log "start: $(reading)"
M=claude-opus-5-5:low
python3 -u run.py --harness "tazuna-boss-v183@$M,tazuna-noboss-v183@$M,cc@$M,omp-v1109@$M" --runs 2 --workers 8 > claude-low-1109.out 2>&1
sleep 200  # one more guard poll after the last run
log "end: $(reading)"
log "CLAUDE-LOW-DONE"
