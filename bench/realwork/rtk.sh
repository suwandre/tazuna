#!/bin/bash
# 2026-11-02 day: `--rtk` (claude's Bash commands through rtk 0.49.0, shortened output), the repo profile without boss
# mode, claude-opus-5-5 medium on the subscription, the six tasks/ tasks x two runs (v1233-v1244, rtk.jobs), tazuna 0.1.25
# frozen in ~/bench-homes/tz-v125; its baselines (all tools, core tools) run tonight in rt.sh. Daytime cap: runs start only
# while the 5-hour window is under 53% (the ROADMAP's 40% of a window over this morning's 13%), so the user's own work keeps
# room; a run moves the window ~3%. Jobs over the cap wait for it to drop. No judge. Started with nohup.
cd "$(dirname "$0")"; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> rtk.log; }
caffeinate -ims -w $$ &
log "start; claude $(claude --version 2>/dev/null | head -1); $(rtk --version); $(df -g ~ | awk 'NR==2{print $4}') GB free"
export BENCH_GATE=state GUARD_PAUSE_AT=53 GUARD_WEEK_AT=95 BENCH_TIMEOUT=10800 HARNESS_BIN=$HOME/bench-homes/tz-v125/tazuna
ps -axo command= | grep -q "[g]uard.py" || (cd .. && GUARD_PAUSE_AT=85 GUARD_KILL_AT=99 nohup ./guard.py >> guard.log 2>&1 &)
M=claude-opus-5-5
running() { ps -axo command= | grep -E "run\.py --runs 1 --workers 1 --tasks \S+ --harness tazuna-rtk-v12(3[3-9]|4[0-4])@" | grep -vc grep; }
while read -r set task v arm; do
  grep -q "\"harness\": \"tazuna-$arm-v$v@" results.jsonl 2>/dev/null && continue
  while [ "$(running)" -ge 8 ]; do sleep 30; done
  while [ "$(df -g ~ | awk 'NR==2{print $4}')" -lt 15 ]; do log "under 15 GB free: waiting"; sleep 300; done
  BENCH_SET=$set ./run.py --runs 1 --workers 1 --tasks "$task" --harness "tazuna-$arm-v$v@$M:medium" > "bench.v$v.log" 2>&1 &
  log "start v$v $arm $task"; sleep 5
done < rtk.jobs
while [ "$(running)" -gt 0 ]; do sleep 30; done
log "runs over: $(for v in $(seq 1233 1244); do grep -h ' r0:\|ERROR' bench.v$v.log | tail -1 | awk '{print $3, $5}' | sed "s/@$M:medium//"; done | tr '\n' ';')"
python3 -I leakscan.py > leakscan-rtk.out 2>&1
log "RTK-DONE ($(df -g ~ | awk 'NR==2{print $4}') GB free)"
