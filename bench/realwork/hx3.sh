#!/bin/bash
# 2026-10-30: the five rival runs of hx.sh/hx2.sh lost to provider errors (SubRouter/LingAI: "do request failed", "499
# upstream stream read failed", 503 no channel; omp, OpenCode and Command Code retry briefly, tazuna lost none of its 16 that
# day) run again (v737-v741, hx3.jobs), all at once; with Command Code's rclone run (v736) they complete one run per harness
# and task. Then one judge pool per task: omp, OpenCode, Command Code, and the A/B's two current-prompt tazuna runs.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> hx.log; }
caffeinate -ims -w $$ &
export SUB_EFFORT=max SUB_KEYENV=SUBROUTER_MODELS_KEY GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95
M=deepseek-v4.1-flash-1017 P=0.022,0.0022,0.088
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
[ -n "$U" ] && ./sub_guard.sh "$U" 1000 v736 $(seq -f "v%g" 737 741) >> sub-guard-hx3.log 2>&1 &
ps -axo command= | grep -q "[g]uard.py" || (cd .. && nohup ./guard.py >> guard.log 2>&1 &)
busy() { ps -axo command= | grep -E "^/bin/bash \./sub_run\.sh $M v7(3[6-9]|4[01]) " | grep -vc grep; }
while read -r k set h task v; do
  SUBROUTER_X_KEYENV=$k SUB_SET=$set SUB_HARNESS=$h ./sub_run.sh $M v$v $P 0.5 $task < /dev/null &
  log "start v$v $h $task ($k), rerun"; sleep 5
done < hx3.jobs
while [ "$(busy)" -gt 0 ]; do sleep 30; done
log "reruns over"
python3 -I leakscan.py > leakscan-hx.out 2>&1
pool() { echo -n "omp-sub-v$1@$M:max,opencode-sub-v$2@$M:max,cmd-sub-v$3@$M:max,tazuna-ownsubx-v$4@$M:max,tazuna-ownsubx-v$5@$M:max"; }
for s in "tasks terraform-import-foreach $(pool 721 729 740 705 707)" "tasks caddy-quoted-braces $(pool 737 730 741 709 711)" \
         "tasks-oct caddy-retry-match-expr $(pool 725 739 735 713 715)" "tasks-oct rclone-archive-symlinks $(pool 738 732 736 717 719)"; do
  set -- $s
  BENCH_SET=$1 JUDGE_TASKS=$2 JUDGE_OUT=judge.hx.$1.jsonl JUDGE_ONLY="$3" python3 -I ./judge.py --passes 2 >> judge-hx.log 2>&1
  log "judged $2"
done
log "HX-DONE"
