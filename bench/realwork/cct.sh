#!/bin/bash
# 2026-10-31: stock Claude Code 2.1.314 again, Opus 5.5 medium on the subscription, the same four tasks x two runs as np.sh
# (v815-v822, cct.jobs), four at a time, with a timestamped transcript of each run (BENCH_STAMP, ~/bench-homes/cc/stamped):
# where Claude Code's time goes next to tazuna's (timesplit.py), and whether cp.sh's midnight runs were faster than
# afternoon ones. No judge (the question is time). Gated on the usage guard. Started with nohup.
cd "$(dirname "$0")"; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> cct.log; }
caffeinate -ims -w $$ &
log "start; claude $(claude --version 2>/dev/null | head -1)"
export BENCH_GATE=state GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 BENCH_TIMEOUT=10800 BENCH_STAMP=1
ps -axo command= | grep -q "[g]uard.py" || (cd .. && GUARD_PAUSE_AT=85 GUARD_KILL_AT=99 nohup ./guard.py >> guard.log 2>&1 &)
M=claude-opus-5-5
running() { ps -axo command= | grep -E "run\.py --runs 1 --workers 1 --tasks \S+ --harness cc-v(81[5-9]|82[0-2])@" | grep -vc grep; }
while read -r set task v; do
  while [ "$(running)" -ge 4 ]; do sleep 30; done
  BENCH_SET=$set ./run.py --runs 1 --workers 1 --tasks "$task" --harness "cc-v$v@$M:medium" > "bench.v$v.log" 2>&1 &
  log "start v$v $task"; sleep 5
done < cct.jobs
while [ "$(running)" -gt 0 ]; do sleep 30; done
log "runs over: $(for v in $(seq 815 822); do grep -h ' r0:\|ERROR' bench.v$v.log | tail -1 | awk '{print $3, $5}' | sed "s/@$M:medium//"; done | tr '\n' ';')"
log "CCT-DONE"
