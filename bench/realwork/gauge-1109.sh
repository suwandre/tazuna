#!/bin/bash
# 2026-11-09 gauge before a fresh harness round: Claude Opus 5.5 at low, medium and high, one task, one run per arm.
# Arms: tazuna 0.1.83 as `--run` gives it (boss mode: Opus directs, Haiku workers at medium) and as a live session
# does (no boss), stock Claude Code, Hermes (bills extra usage, allowed up to $8 here). pi, OMO and OMP are out: their
# claude.ai logins no longer refresh (third-party subscription use is blocked since 2026-04-04). Measures plan usage per
# run (guard meter before and after) to size the full round: 6 tasks x 2 runs.
set -u
cd "$(dirname "$0")"
source ~/.bench-token
export BENCH_GATE=state GUARD_PAUSE_AT=70
log() { echo "$(date +%H:%M) $*" >> gauge-1109.log; }
reading() { python3 -I -c "import json;s=json.load(open('../guard.state.json'));print(f\"5h {s['five_hour']}% 7d {s['seven_day']}% extra {s['extra']}\")"; }
log "start: $(reading)"
H=""
for e in low medium high; do
  H="$H,tazuna-boss-v183@claude-opus-5-5:$e,tazuna-noboss-v183@claude-opus-5-5:$e,cc@claude-opus-5-5:$e,hermes@claude-opus-5-5:$e"
done
python3 -u run.py --harness "${H#,}" --tasks caddy-quoted-braces --runs 1 --workers 8 > gauge-1109.out 2>&1
sleep 200  # one more guard poll after the last run
log "end: $(reading)"
log "GAUGE-DONE"
