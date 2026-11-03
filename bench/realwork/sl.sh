#!/bin/bash
# 2026-11-03: two leads for the default config (repo profile, no boss, test scope), queued behind hts.sh: Sonnet 5.5 at
# medium (tsson) and Opus 5.5 at low (tslow), the six tasks/ tasks x two runs each, arms interleaved (v1385-v1408,
# sl.jobs), eight at a time, tazuna 0.1.47 frozen in ~/bench-homes/tz-v147. Baseline: this morning's default with test
# scope (Opus medium, v1333-v1344, 09:58); the judge pools all three by hand, calls counted first. Waits for HTS-DONE.
# Cap: no new run once the 5-hour window is at 70% (skipped, not held); week 95%; none below 15 GB free disk. Below 5%
# battery unplugged it stops the runs and sleeps the Mac. Started with nohup.
cd "$(dirname "$0")"; source ~/.bench-token; export PATH="/opt/homebrew/bin:$PATH"  # Homebrew Python (run.py needs 3.10+)
log() { echo "$(date +%H:%M) $*" >> sl.log; }
log "waiting for HTS-DONE"
until grep -q "HTS-DONE" hts.log 2>/dev/null; do sleep 60; done
caffeinate -ims -w $$ &
log "start; claude $(claude --version 2>/dev/null | head -1); $(df -g ~ | awk 'NR==2{print $4}') GB free"
export BENCH_GATE=state BENCH_GATE_EXIT=1 GUARD_PAUSE_AT=70 GUARD_WEEK_AT=95 BENCH_TIMEOUT=10800 HARNESS_BIN=$HOME/bench-homes/tz-v147/tazuna
ps -axo command= | grep -q "[g]uard.py" || (cd .. && GUARD_PAUSE_AT=85 GUARD_KILL_AT=99 nohup ./guard.py >> guard.log 2>&1 &)
running() { ps -axo command= | grep -E "run\.py --runs 1 --workers 1 --tasks \S+ --harness tazuna-ts(son|low)-v1(38[5-9]|39[0-9]|40[0-8])@" | grep -vc grep; }
tree() { local c; for c in $(ps -axo pid=,ppid= | awk -v p="$1" '$2==p{print $1}'); do tree "$c"; done; echo "$1"; }
low_battery() { local b; b=$(pmset -g batt | sed -n 2p); echo "$b" | grep -q discharging && [ "$(echo "$b" | grep -o '[0-9]*%' | tr -d %)" -lt 5 ]; }
stop_sleep() {
  log "$1: stopping the runs, then sleep"
  for p in $(pgrep -f "run\.py --runs 1 --workers 1 --tasks \S+ --harness tazuna-ts(son|low)"); do kill $(tree "$p") 2>/dev/null; done
  sleep 10; pmset sleepnow; exit 0
}
while read -r set task v arm model; do
  grep -q "\"harness\": \"tazuna-$arm-v$v@" results.jsonl 2>/dev/null && continue  # done on an earlier start
  pgrep -f "harness tazuna-$arm-v$v@" > /dev/null && continue  # still running from an earlier start
  while [ "$(running)" -ge 8 ]; do low_battery && stop_sleep "battery under 5%"; sleep 30; done
  while [ "$(df -g ~ | awk 'NR==2{print $4}')" -lt 15 ]; do log "under 15 GB free: waiting"; low_battery && stop_sleep "battery under 5%"; sleep 300; done
  BENCH_SET=$set ./run.py --runs 1 --workers 1 --tasks "$task" --harness "tazuna-$arm-v$v@$model" > "bench.v$v.log" 2>&1 &
  log "start v$v $arm $model $task"; sleep 5
done < sl.jobs
while [ "$(running)" -gt 0 ]; do low_battery && stop_sleep "battery under 5%"; sleep 30; done
log "runs over: $(for v in $(seq 1385 1408); do grep -h ' r0:\|ERROR' bench.v$v.log | tail -1 | awk '{print $3, $5}'; done | tr '\n' ';')"
python3 -I leakscan.py > leakscan-sl.out 2>&1
log "SL-DONE ($(df -g ~ | awk 'NR==2{print $4}') GB free)"
