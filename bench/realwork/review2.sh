#!/bin/bash
# 2026-10-29: review.sh's end, taken over: v655 (Astra reviewer, caddy-retry) was killed by sub_run's 30-minute hang watch
# while its review call stalled; rerun as v660 with the reviewer capped at 15 minutes (tz-v101c). Waits for v658 too, then
# the leak scan and the judge over v648-v660 (one pool per task, two passes).
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> review.log; }
caffeinate -ims -w $$ &
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUBROUTER_REVIEW_KEYENV=SUBROUTER_VET1_KEY
M=deepseek-v4.1-flash-1017 P=0.022,0.0022,0.088
SUBROUTER_X_KEYENV=SUBROUTER_VET3_KEY SUB_SET=tasks-oct SUB_HARNESS=tazuna-ownsubxrev HARNESS_BIN=$HOME/bench-homes/tz-v101c/tazuna ./sub_run.sh $M v660 $P 0.5 caddy-retry-match-expr < /dev/null &
log "start v660 tazuna-ownsubxrev caddy-retry-match-expr (rerun of v655, reviewer capped at 15 min)"
running() { ps -axo command= | grep -E "^/bin/bash \./sub_run\.sh $M v6(5[89]|60) " | grep -vc grep; }
sleep 10; while [ "$(running)" -gt 0 ]; do sleep 30; done
log "runs over"
python3 -I leakscan.py > leakscan-review.out 2>&1
ps -axo command= | grep -q "[g]uard.py" || (cd .. && nohup ./guard.py >> guard.log 2>&1 &)
ONLY="$(for v in $(seq 648 660); do for h in tazuna-ownsubx tazuna-ownsubxrev tazuna-ownsubxopus; do echo -n "$h-v$v@$M:max,"; done; done)"
for s in "tasks caddy-quoted-braces,terraform-import-foreach" "tasks-hard caddy-tls-automate-names" "tasks-oct caddy-retry-match-expr"; do
  set -- $s
  BENCH_SET=$1 JUDGE_TASKS=$2 JUDGE_OUT=judge.review.$1.jsonl JUDGE_ONLY="$ONLY" python3 -I ./judge.py --passes 2 >> judge-review.log 2>&1
  log "judged $2"
done
log "REVIEW-DONE"
