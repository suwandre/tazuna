#!/bin/bash
# 2026-10-30: the Claude path against stock Claude Code again (last on 2026-10-12, Claude Code has changed since): tazuna 0.1.6
# default (headless on Opus: the Haiku boss, the Haiku explorer) and stock Claude Code, both claude-opus-5-5 medium on the
# subscription, four tasks x two runs (v783-v798, cp.jobs), four at a time, gated on the usage guard (5h 85%, week 95%; the
# guard stops at 99%). Starts after xh.sh's judge; then one judge pool per task, two passes. Sleeps the Mac at the end only if
# nobody has touched it for 20 minutes. Started with nohup.
cd "$(dirname "$0")"; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> cp.log; }
caffeinate -ims -w $$ &
until grep -q "XH-DONE" xh.log 2>/dev/null; do sleep 60; done
log "after xh.sh; claude $(claude --version 2>/dev/null | head -1)"
export BENCH_GATE=state GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 BENCH_TIMEOUT=10800 HARNESS_BIN=$HOME/bench-homes/tz-v106c/tazuna
ps -axo command= | grep -q "[g]uard.py" || (cd .. && GUARD_PAUSE_AT=85 GUARD_KILL_AT=99 nohup ./guard.py >> guard.log 2>&1 &)
M=claude-opus-5-5
running() { ps -axo command= | grep -E "run\.py --runs 1 --workers 1 --tasks \S+ --harness (tazuna|cc)-v(78[3-9]|79[0-8])@" | grep -vc grep; }
while read -r set h task v; do
  while [ "$(running)" -ge 4 ]; do sleep 30; done
  BENCH_SET=$set ./run.py --runs 1 --workers 1 --tasks "$task" --harness "$h-v$v@$M:medium" > "bench.v$v.log" 2>&1 &
  log "start v$v $h $task"; sleep 5
done < cp.jobs
while [ "$(running)" -gt 0 ]; do sleep 30; done
log "runs over: $(for v in $(seq 783 798); do grep -h ' r0:\|ERROR' bench.v$v.log | tail -1 | awk '{print $3, $5}' | sed "s/@$M:medium//"; done | tr '\n' ';')"
python3 -I leakscan.py > leakscan-cp.out 2>&1
until [ ! -e ../PAUSE ] && [ ! -e ../ABORT ]; do sleep 60; done
ONLY="$(for v in $(seq 783 798); do for h in tazuna cc; do echo -n "$h-v$v@$M:medium,"; done; done)"
for s in "tasks terraform-import-foreach,caddy-quoted-braces" "tasks-oct caddy-retry-match-expr,rclone-archive-symlinks"; do
  set -- $s
  BENCH_SET=$1 JUDGE_TASKS=$2 JUDGE_OUT=judge.cp.$1.jsonl JUDGE_ONLY="$ONLY" python3 -I ./judge.py --passes 2 >> judge-cp.log 2>&1
  log "judged $2"
done
log "CP-DONE"
idle=$(( $(ioreg -c IOHIDSystem | awk '/HIDIdleTime/ {print $NF; exit}') / 1000000000 ))
if [ "$idle" -gt 1200 ]; then log "idle ${idle}s: sleeping"; pmset sleepnow; else log "in use (idle ${idle}s): not sleeping"; fi
