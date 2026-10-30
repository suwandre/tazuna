#!/bin/bash
# 2026-10-30: stock oh-my-pi (omp 18.14.0) and OpenCode 2.2.3 on SubRouter with DeepSeek 1017 max (LingAI), one run per task
# (v721-v728, hx.jobs; omp tazuna-vet-1, OpenCode tazuna-vet-4), four at a time beside the tests-nudge A/B; then one judge
# pool per task with that A/B's current-prompt tazuna runs (same day, same model and provider), two passes. Command Code
# left out: it needs its own account login even with your own key. Spend stop 1600 fen, account-wide with the A/B's.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> hx.log; }
caffeinate -ims -w $$ &
export SUB_EFFORT=max SUB_KEYENV=SUBROUTER_MODELS_KEY GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95
M=deepseek-v4.1-flash-1017 P=0.022,0.0022,0.088
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
[ -n "$U" ] && ./sub_guard.sh "$U" 1600 $(seq -f "v%g" 721 728) >> sub-guard-hx.log 2>&1 &
ps -axo command= | grep -q "[g]uard.py" || (cd .. && nohup ./guard.py >> guard.log 2>&1 &)
running() { ps -axo command= | grep -E "^/bin/bash \./sub_run\.sh $M v72[1-8] " | grep -vc grep; }
while read -r k set h task v; do
  while [ "$(running)" -ge 4 ]; do sleep 30; done
  SUBROUTER_X_KEYENV=$k SUB_SET=$set SUB_HARNESS=$h ./sub_run.sh $M v$v $P 0.5 $task < /dev/null &
  log "start v$v $h $task ($k)"; sleep 5
done < hx.jobs
while [ "$(running)" -gt 0 ]; do sleep 30; done
log "runs over; waiting for the A/B's runs"
until grep -q "runs over" tn.log; do sleep 60; done
python3 -I leakscan.py > leakscan-hx.out 2>&1
pool() { echo -n "omp-sub-v$1@$M:max,opencode-sub-v$2@$M:max,tazuna-ownsubx-v$3@$M:max,tazuna-ownsubx-v$4@$M:max"; }
for s in "tasks terraform-import-foreach $(pool 721 722 705 707)" "tasks caddy-quoted-braces $(pool 723 724 709 711)" \
         "tasks-oct caddy-retry-match-expr $(pool 725 726 713 715)" "tasks-oct rclone-archive-symlinks $(pool 727 728 717 719)"; do
  set -- $s
  BENCH_SET=$1 JUDGE_TASKS=$2 JUDGE_OUT=judge.hx.$1.jsonl JUDGE_ONLY="$3" python3 -I ./judge.py --passes 2 >> judge-hx.log 2>&1
  log "judged $2"
done
log "HX-DONE"
