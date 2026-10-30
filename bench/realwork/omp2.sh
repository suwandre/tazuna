#!/bin/bash
# 2026-10-30: omp's lead (8.62 against tazuna 7.50, one run a task) checked with a second omp run a task (v746-v749,
# omp2.jobs, tazuna-vet-1), all four at once; then one pool per task with both omp runs and tazuna's two, two passes.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> omp2.log; }
caffeinate -ims -w $$ &
export SUB_EFFORT=max SUB_KEYENV=SUBROUTER_MODELS_KEY GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95
M=deepseek-v4.1-flash-1017 P=0.022,0.0022,0.088
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
[ -n "$U" ] && ./sub_guard.sh "$U" 600 $(seq -f "v%g" 746 749) >> sub-guard-omp2.log 2>&1 &
ps -axo command= | grep -q "[g]uard.py" || (cd .. && GUARD_PAUSE_AT=85 GUARD_KILL_AT=99 nohup ./guard.py >> guard.log 2>&1 &)
busy() { ps -axo command= | grep -E "^/bin/bash \./sub_run\.sh $M v74[6-9] " | grep -vc grep; }
while read -r k set h task v; do
  SUBROUTER_X_KEYENV=$k SUB_SET=$set SUB_HARNESS=$h ./sub_run.sh $M v$v $P 0.5 $task < /dev/null &
  log "start v$v $h $task ($k)"; sleep 5
done < omp2.jobs
while [ "$(busy)" -gt 0 ]; do sleep 30; done
log "runs over: $(for v in 746 747 748 749; do grep -h ' r0:\|ERROR' bench.v$v.log | tail -1 | awk '{print $3, $5}' | sed "s/@$M:max//"; done | tr '\n' ';')"
python3 -I leakscan.py > leakscan-omp2.out 2>&1
until [ ! -e ../PAUSE ] && [ ! -e ../ABORT ]; do sleep 60; done
pool() { echo -n "omp-sub-v$1@$M:max,omp-sub-v$2@$M:max,tazuna-ownsubx-v$3@$M:max,tazuna-ownsubx-v$4@$M:max"; }
for s in "tasks terraform-import-foreach $(pool 721 746 705 707)" "tasks caddy-quoted-braces $(pool 742 747 709 711)" \
         "tasks-oct caddy-retry-match-expr $(pool 725 748 713 715)" "tasks-oct rclone-archive-symlinks $(pool 743 749 717 719)"; do
  set -- $s
  BENCH_SET=$1 JUDGE_TASKS=$2 JUDGE_OUT=judge.omp2.$1.jsonl JUDGE_ONLY="$3" python3 -I ./judge.py --passes 2 >> judge-omp2.log 2>&1
  log "judged $2"
done
log "OMP2-DONE"
