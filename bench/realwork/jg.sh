#!/bin/bash
# 2026-10-30: jevgrep in the own loop (`jg` on PATH, through the relay to TypeSafe; the task names the tool), DeepSeek 1017 max,
# tazuna 0.1.6, two runs a task (v799-v806, jg.jobs, tazuna-vet-1), four at a time beside xh.sh; judged in one pool per task
# with xh.sh's max runs (same day, same setup), two passes. Half of a run's time is spent before the first edit, exploring
# (timesplit.py): does a semantic search shorten that without costing quality? Spend stop 1600 fen (account-wide with xh.sh's).
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token; source ~/.typesafe-key
log() { echo "$(date +%H:%M) $*" >> jg.log; }
caffeinate -ims -w $$ &
export SUB_EFFORT=max SUB_KEYENV=SUBROUTER_MODELS_KEY GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95
M=deepseek-v4.1-flash-1017 P=0.022,0.0022,0.088
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
[ -n "$U" ] && ./sub_guard.sh "$U" 1600 $(seq -f "v%g" 799 806) >> sub-guard-jg.log 2>&1 &
running() { ps -axo command= | grep -E "^/bin/bash \./sub_run\.sh $M v(799|80[0-6]) " | grep -vc grep; }
while read -r set task v; do
  while [ "$(running)" -ge 4 ]; do sleep 30; done
  SUBROUTER_X_KEYENV=SUBROUTER_VET1_KEY SUB_SET=$set SUB_HARNESS=tazuna-ownsubxjg HARNESS_BIN=$HOME/bench-homes/tz-v106/tazuna ./sub_run.sh $M v$v $P 0.5 $task < /dev/null &
  log "start v$v jg $task"; sleep 5
done < jg.jobs
while [ "$(running)" -gt 0 ]; do sleep 30; done
log "runs over: $(for v in $(seq 799 806); do grep -h ' r0:\|ERROR' bench.v$v.log | tail -1 | awk '{print $3, $5}' | sed "s/@$M:max//"; done | tr '\n' ';')"
until grep -q "runs over" xh.log; do sleep 60; done
python3 -I leakscan.py > leakscan-jg.out 2>&1
until [ ! -e ../PAUSE ] && [ ! -e ../ABORT ]; do sleep 60; done
pool() { echo -n "tazuna-ownsubxjg-v$1@$M:max,tazuna-ownsubxjg-v$2@$M:max,tazuna-ownsubx-v$3@$M:max,tazuna-ownsubx-v$4@$M:max"; }
for s in "tasks terraform-import-foreach $(pool 799 800 767 769)" "tasks caddy-quoted-braces $(pool 801 802 771 773)" \
         "tasks-oct caddy-retry-match-expr $(pool 803 804 775 777)" "tasks-oct rclone-archive-symlinks $(pool 805 806 779 781)"; do
  set -- $s
  BENCH_SET=$1 JUDGE_TASKS=$2 JUDGE_OUT=judge.jg.$1.jsonl JUDGE_ONLY="$3" python3 -I ./judge.py --passes 2 >> judge-jg.log 2>&1
  log "judged $2"
done
log "JG-DONE"
