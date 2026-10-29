#!/bin/bash
# 2026-10-29: --preset reviewed as shipped (tazuna 0.1.4): DeepSeek 1017 max on SubRouter (LingAI, tazuna-vet-3), the Opus
# medium reviewer, and the hand-over to Opus medium when tests still fail (on: no --no-escalate). The four reviewer tasks
# (v661-v664, reviewed.jobs), all at once after the 5-hour window resets; then one judge pool per task with the earlier
# no-review (v648/651/654/657) and Opus-max-review arms (v650/653/656/659), two passes. Spend stop 300 fen.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> reviewed.log; }
caffeinate -ims -w $$ &
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95
M=deepseek-v4.1-flash-1017 P=0.022,0.0022,0.088
ps -axo command= | grep -q "[g]uard.py" || (cd .. && nohup ./guard.py >> guard.log 2>&1 &)
while [ -e ../PAUSE ] || [ -e ../ABORT ]; do sleep 60; done
log "window clear"
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
[ -n "$U" ] && ./sub_guard.sh "$U" 300 $(seq -f "v%g" 661 664) >> sub-guard-reviewed.log 2>&1 &
running() { ps -axo command= | grep -E "^/bin/bash \./sub_run\.sh $M v66[1-4] " | grep -vc grep; }
while read -r k set h task v; do
  SUBROUTER_X_KEYENV=$k SUB_SET=$set SUB_HARNESS=$h HARNESS_BIN=$HOME/bench-homes/tz-v104/tazuna ./sub_run.sh $M v$v $P 0.5 $task < /dev/null &
  log "start v$v $h $task ($k)"; sleep 5
done < reviewed.jobs
while [ "$(running)" -gt 0 ]; do sleep 30; done
log "runs over"
python3 -I leakscan.py > leakscan-reviewed.out 2>&1
ONLY="$(for v in 648 651 654 657 650 653 656 659 661 662 663 664; do for h in tazuna-ownsubx tazuna-ownsubxopus tazuna-reviewed; do echo -n "$h-v$v@$M:max,"; done; done)"
for s in "tasks caddy-quoted-braces,terraform-import-foreach" "tasks-hard caddy-tls-automate-names" "tasks-oct caddy-retry-match-expr"; do
  set -- $s
  BENCH_SET=$1 JUDGE_TASKS=$2 JUDGE_OUT=judge.reviewed.$1.jsonl JUDGE_ONLY="$ONLY" python3 -I ./judge.py --passes 2 >> judge-reviewed.log 2>&1
  log "judged $2"
done
log "REVIEWED-DONE"
