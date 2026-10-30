#!/bin/bash
# 2026-10-30: own-loop time. 86% of a DeepSeek run's wall time is the model (timesplit.py), ~1,000 output tokens a call at
# max: DeepSeek 1017 at xhigh against max, tazuna 0.1.6, SubRouter LingAI, a key per arm (vet-3 max, vet-2 xhigh), four tasks x
# two runs (v767-v782, xh.jobs), eight at a time; then one judge pool per task with all four runs, two passes. Judged on score,
# cost and minutes per run together. Spend stop 800 fen (nothing else on the account). Started with nohup.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> xh.log; }
caffeinate -ims -w $$ &
export SUB_KEYENV=SUBROUTER_MODELS_KEY GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95
M=deepseek-v4.1-flash-1017 P=0.022,0.0022,0.088
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
[ -n "$U" ] && ./sub_guard.sh "$U" 800 $(seq -f "v%g" 767 782) >> sub-guard-xh.log 2>&1 &
ps -axo command= | grep -q "[g]uard.py" || (cd .. && GUARD_PAUSE_AT=85 GUARD_KILL_AT=99 nohup ./guard.py >> guard.log 2>&1 &)
running() { ps -axo command= | grep -E "^/bin/bash \./sub_run\.sh $M v(76[7-9]|77[0-9]|78[0-2]) " | grep -vc grep; }
while read -r k set eff task v; do
  while [ "$(running)" -ge 8 ]; do sleep 30; done
  SUBROUTER_X_KEYENV=$k SUB_SET=$set SUB_HARNESS=tazuna-ownsubx SUB_EFFORT=$eff HARNESS_BIN=$HOME/bench-homes/tz-v106/tazuna ./sub_run.sh $M v$v $P 0.5 $task < /dev/null &
  log "start v$v $eff $task ($k)"; sleep 5
done < xh.jobs
while [ "$(running)" -gt 0 ]; do sleep 30; done
log "runs over"
python3 -I leakscan.py > leakscan-xh.out 2>&1
until [ ! -e ../PAUSE ] && [ ! -e ../ABORT ]; do sleep 60; done
ONLY="$(for v in $(seq 767 782); do for e in max xhigh; do echo -n "tazuna-ownsubx-v$v@$M:$e,"; done; done)"
for s in "tasks terraform-import-foreach,caddy-quoted-braces" "tasks-oct caddy-retry-match-expr,rclone-archive-symlinks"; do
  set -- $s
  BENCH_SET=$1 JUDGE_TASKS=$2 JUDGE_OUT=judge.xh.$1.jsonl JUDGE_ONLY="$ONLY" python3 -I ./judge.py --passes 2 >> judge-xh.log 2>&1
  log "judged $2"
done
log "XH-DONE"
