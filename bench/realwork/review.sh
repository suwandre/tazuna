#!/bin/bash
# 2026-10-29: a reviewer behind a cheap implementer. tazuna 0.1.1, deepseek-v4.1-flash-1017 at max on LingAI implementing in
# every arm (its own key per arm: tazuna-vet-2 none, -3 with GPT-6 Astra reviewing through SubRouter's shared Pro pools on
# tazuna-vet-1, -4 with Opus 5.5 reviewing through claude), four tasks (v648-v659, review.jobs: keyenv set harness task
# label), four at a time; then one judge pool per task, two passes. Spend stop 500 fen.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> review.log; }
caffeinate -ims -w $$ &
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUBROUTER_REVIEW_KEYENV=SUBROUTER_VET1_KEY
M=deepseek-v4.1-flash-1017 P=0.022,0.0022,0.088
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
[ -n "$U" ] && ./sub_guard.sh "$U" 500 $(seq -f "v%g" 648 659) >> sub-guard-review.log 2>&1 &
ps -axo command= | grep -q "[g]uard.py" || (cd .. && nohup ./guard.py >> guard.log 2>&1 &)
running() { ps -axo command= | grep -E "^/bin/bash \./sub_run\.sh $M v6(4[89]|5[0-9]) " | grep -vc grep; }
while read -r k set h task v; do
  while [ "$(running)" -ge 4 ]; do sleep 30; done
  SUBROUTER_X_KEYENV=$k SUB_SET=$set SUB_HARNESS=$h HARNESS_BIN=$HOME/bench-homes/tz-v101/tazuna ./sub_run.sh $M v$v $P 0.5 $task < /dev/null &
  log "start v$v $h $task ($k)"; sleep 5
done < review.jobs
while [ "$(running)" -gt 0 ]; do sleep 30; done
log "runs over"
python3 -I leakscan.py > leakscan-review.out 2>&1
ONLY="$(for v in $(seq 648 659); do for h in tazuna-ownsubx tazuna-ownsubxrev tazuna-ownsubxopus; do echo -n "$h-v$v@$M:max,"; done; done)"
for s in "tasks caddy-quoted-braces,terraform-import-foreach" "tasks-hard caddy-tls-automate-names" "tasks-oct caddy-retry-match-expr"; do
  set -- $s
  BENCH_SET=$1 JUDGE_TASKS=$2 JUDGE_OUT=judge.review.$1.jsonl JUDGE_ONLY="$ONLY" python3 -I ./judge.py --passes 2 >> judge-review.log 2>&1
  log "judged $2"
done
log "REVIEW-DONE"
