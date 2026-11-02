#!/bin/bash
# 2026-11-02, 16:00 (the user's start time): `--batch-calls` (claude told to make independent tool calls in one turn), the repo profile without boss
# mode, claude-opus-5-5 medium on the subscription, the six tasks/ tasks x two runs (v1245-v1256, rtb.jobs), eight at a
# time, tazuna 0.1.30 frozen in ~/bench-homes/tz-v130. Its baseline is the same day's all-tools arm (rt.sh, v1209-v1231
# odd); the judge pools both by hand, calls counted first. Waits until 16:00; gated on the usage guard (5h 85%, week
# 95%: a job over it waits); starts no run below 15 GB free disk. Keeps the Mac awake while it runs; below 5% battery
# unplugged it stops the runs and sleeps it (start rtb.sh again to continue: finished labels are skipped). nohup.
cd "$(dirname "$0")"; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> rtb.log; }
target=$(date -j -f "%Y-%m-%d %H:%M" "$(date +%F) 16:00" +%s)
log "waiting until 16:00"
while [ "$(date +%s)" -lt "$target" ]; do sleep 60; done
caffeinate -ims -w $$ &
log "start; claude $(claude --version 2>/dev/null | head -1); $(df -g ~ | awk 'NR==2{print $4}') GB free"
export BENCH_GATE=state GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 BENCH_TIMEOUT=10800 HARNESS_BIN=$HOME/bench-homes/tz-v130/tazuna
ps -axo command= | grep -q "[g]uard.py" || (cd .. && GUARD_PAUSE_AT=85 GUARD_KILL_AT=99 nohup ./guard.py >> guard.log 2>&1 &)
M=claude-opus-5-5
running() { ps -axo command= | grep -E "run\.py --runs 1 --workers 1 --tasks \S+ --harness tazuna-rtbatch-v12(4[5-9]|5[0-6])@" | grep -vc grep; }
tree() { local c; for c in $(ps -axo pid=,ppid= | awk -v p="$1" '$2==p{print $1}'); do tree "$c"; done; echo "$1"; }
low_battery() { local b; b=$(pmset -g batt | sed -n 2p); echo "$b" | grep -q discharging && [ "$(echo "$b" | grep -o '[0-9]*%' | tr -d %)" -lt 5 ]; }
stop_sleep() {
  log "$1: stopping the runs, then sleep"
  for p in $(pgrep -f "run\.py --runs 1 --workers 1 --tasks \S+ --harness tazuna-rtbatch"); do kill $(tree "$p") 2>/dev/null; done
  sleep 10; pmset sleepnow; exit 0
}
while read -r set task v arm; do
  grep -q "\"harness\": \"tazuna-$arm-v$v@" results.jsonl 2>/dev/null && continue  # done on an earlier start
  pgrep -f "harness tazuna-$arm-v$v@" > /dev/null && continue  # still running from an earlier start
  while [ "$(running)" -ge 8 ]; do low_battery && stop_sleep "battery under 5%"; sleep 30; done
  while [ "$(df -g ~ | awk 'NR==2{print $4}')" -lt 15 ]; do log "under 15 GB free: waiting"; low_battery && stop_sleep "battery under 5%"; sleep 300; done
  BENCH_SET=$set ./run.py --runs 1 --workers 1 --tasks "$task" --harness "tazuna-$arm-v$v@$M:medium" > "bench.v$v.log" 2>&1 &
  log "start v$v $arm $task"; sleep 5
done < rtb.jobs
while [ "$(running)" -gt 0 ]; do low_battery && stop_sleep "battery under 5%"; sleep 30; done
log "runs over: $(for v in $(seq 1245 1256); do grep -h ' r0:\|ERROR' bench.v$v.log | tail -1 | awk '{print $3, $5}' | sed "s/@$M:medium//"; done | tr '\n' ';')"
python3 -I leakscan.py > leakscan-rtb.out 2>&1
log "RTB-DONE ($(df -g ~ | awk 'NR==2{print $4}') GB free)"
