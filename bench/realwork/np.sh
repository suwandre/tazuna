#!/bin/bash
# 2026-10-31: the Claude path without boss mode (tazuna 0.1.8 --no-boss: Opus works itself, the Haiku explorer stays)
# against cp.sh's runs (0.1.6 default with the Haiku boss, stock Claude Code 2.1.314), claude-opus-5-5 medium on the
# subscription, the same four tasks x two runs (v807-v814, np.jobs), four at a time like cp.sh, gated on the usage guard
# (5h 85%, week 95%). Then one judge pool per task over all three arms, two passes. Started with nohup.
cd "$(dirname "$0")"; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> np.log; }
caffeinate -ims -w $$ &
log "start; claude $(claude --version 2>/dev/null | head -1)"
export BENCH_GATE=state GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 BENCH_TIMEOUT=10800 HARNESS_BIN=$HOME/bench-homes/tz-v108/tazuna
ps -axo command= | grep -q "[g]uard.py" || (cd .. && GUARD_PAUSE_AT=85 GUARD_KILL_AT=99 nohup ./guard.py >> guard.log 2>&1 &)
M=claude-opus-5-5
running() { ps -axo command= | grep -E "run\.py --runs 1 --workers 1 --tasks \S+ --harness tazuna-noboss-v(80[7-9]|81[0-4])@" | grep -vc grep; }
while read -r set task v; do
  while [ "$(running)" -ge 4 ]; do sleep 30; done
  BENCH_SET=$set ./run.py --runs 1 --workers 1 --tasks "$task" --harness "tazuna-noboss-v$v@$M:medium" > "bench.v$v.log" 2>&1 &
  log "start v$v $task"; sleep 5
done < np.jobs
while [ "$(running)" -gt 0 ]; do sleep 30; done
log "runs over: $(for v in $(seq 807 814); do grep -h ' r0:\|ERROR' bench.v$v.log | tail -1 | awk '{print $3, $5}' | sed "s/@$M:medium//"; done | tr '\n' ';')"
python3 -I leakscan.py > leakscan-np.out 2>&1
until [ ! -e ../PAUSE ] && [ ! -e ../ABORT ]; do sleep 60; done
ONLY="$(for v in $(seq 783 798); do for h in tazuna cc; do echo -n "$h-v$v@$M:medium,"; done; done)$(for v in $(seq 807 814); do echo -n "tazuna-noboss-v$v@$M:medium,"; done)"
for s in "tasks terraform-import-foreach,caddy-quoted-braces" "tasks-oct caddy-retry-match-expr,rclone-archive-symlinks"; do
  set -- $s
  BENCH_SET=$1 JUDGE_TASKS=$2 JUDGE_OUT=judge.np.$1.jsonl JUDGE_ONLY="$ONLY" python3 -I ./judge.py --passes 2 >> judge-np.log 2>&1
  log "judged $2"
done
log "NP-DONE"
