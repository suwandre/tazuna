#!/bin/bash
# 2026-11-02, after bh.sh: the lean profile (tazuna's short prompt and 4 tools, ~3k tokens a call) on Opus 5.5 medium
# without boss mode, against the same day's repo-profile runs (rt.sh): last measured in October (Sonnet: -28% cost, judge
# -0.34). The six tasks/ tasks x two runs (v1293-v1304, ln.jobs), eight at a time, tazuna 0.1.32 (tz-v132). Waits for
# bh.sh's BH-DONE. Was: boss mode's Haiku workers at medium, high and xhigh effort (`--boss-effort`; unset they ran at the
# session's, so a live session at xhigh ran them at xhigh), Opus 5.5 medium leading, the repo profile, the six tasks/
# tasks x two runs per arm (v1257-v1292, bh.jobs, arms interleaved), eight at a time, tazuna 0.1.32 frozen in
# ~/bench-homes/tz-v132. Today's no-boss all-tools runs (rt.sh) are the baseline for boss against no boss. No new run once
# the 5-hour window is at 70% (BENCH_GATE_EXIT: skipped, not held; start bh.sh again later for what was skipped); week 95%;
# none below 15 GB free disk. No judge: by hand, calls counted first. Below 5% battery unplugged it stops the runs and
# sleeps the Mac. Started with nohup.
cd "$(dirname "$0")"; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> ln.log; }
until grep -q BH-DONE bh.log 2>/dev/null; do sleep 60; done
caffeinate -ims -w $$ &
log "start; claude $(claude --version 2>/dev/null | head -1); $(df -g ~ | awk 'NR==2{print $4}') GB free"
export BENCH_GATE=state BENCH_GATE_EXIT=1 GUARD_PAUSE_AT=70 GUARD_WEEK_AT=95 BENCH_TIMEOUT=10800 HARNESS_BIN=$HOME/bench-homes/tz-v132/tazuna
ps -axo command= | grep -q "[g]uard.py" || (cd .. && GUARD_PAUSE_AT=85 GUARD_KILL_AT=99 nohup ./guard.py >> guard.log 2>&1 &)
M=claude-opus-5-5
running() { ps -axo command= | grep -E "run\.py --runs 1 --workers 1 --tasks \S+ --harness tazuna-leanopus-v1(29[3-9]|30[0-4])@" | grep -vc grep; }
tree() { local c; for c in $(ps -axo pid=,ppid= | awk -v p="$1" '$2==p{print $1}'); do tree "$c"; done; echo "$1"; }
low_battery() { local b; b=$(pmset -g batt | sed -n 2p); echo "$b" | grep -q discharging && [ "$(echo "$b" | grep -o '[0-9]*%' | tr -d %)" -lt 5 ]; }
stop_sleep() {
  log "$1: stopping the runs, then sleep"
  for p in $(pgrep -f "run\.py --runs 1 --workers 1 --tasks \S+ --harness tazuna-leanopus"); do kill $(tree "$p") 2>/dev/null; done
  sleep 10; pmset sleepnow; exit 0
}
while read -r set task v arm; do
  grep -q "\"harness\": \"tazuna-$arm-v$v@" results.jsonl 2>/dev/null && continue  # done on an earlier start
  pgrep -f "harness tazuna-$arm-v$v@" > /dev/null && continue  # still running from an earlier start
  while [ "$(running)" -ge 8 ]; do low_battery && stop_sleep "battery under 5%"; sleep 30; done
  while [ "$(df -g ~ | awk 'NR==2{print $4}')" -lt 15 ]; do log "under 15 GB free: waiting"; low_battery && stop_sleep "battery under 5%"; sleep 300; done
  BENCH_SET=$set ./run.py --runs 1 --workers 1 --tasks "$task" --harness "tazuna-$arm-v$v@$M:medium" > "bench.v$v.log" 2>&1 &
  log "start v$v $arm $task"; sleep 5
done < ln.jobs
while [ "$(running)" -gt 0 ]; do low_battery && stop_sleep "battery under 5%"; sleep 30; done
log "runs over: $(for v in $(seq 1293 1304); do grep -h ' r0:\|ERROR' bench.v$v.log | tail -1 | awk '{print $3, $5}' | sed "s/@$M:medium//"; done | tr '\n' ';')"
python3 -I leakscan.py > leakscan-ln.out 2>&1
log "LN-DONE ($(df -g ~ | awk 'NR==2{print $4}') GB free)"
