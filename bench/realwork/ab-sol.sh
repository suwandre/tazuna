#!/bin/bash
# 2026-10-28: A/B on gpt-6.1-sol, caddy-retry-match-expr: tazuna 0.0.98 (tz-v98p) as is (v566, v568, v570) against
# TAZUNA_PROMPT2 (v567, v569, v571): tests line "add tests for the new behavior; change existing tests or test helpers only
# where the task changes what they check", "reuse existing code rather than copying it", and no PLAN_RULE for gpt- models
# (their reasoning goes back encrypted). Four at a time on the shared Pro pools (tazuna-vet-1); then the six judged blind in
# one pool, two passes. Spend stop 500 fen.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> ab-sol.log; }
caffeinate -ims -w $$ &
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUBROUTER_X_KEYENV=SUBROUTER_VET1_KEY
M=gpt-6.1-sol P=0.04,0.002,0.2
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
[ -n "$U" ] && ./sub_guard.sh "$U" 500 v566 v567 v568 v569 v570 v571 >> sub-guard-sol.log 2>&1 &
running() { ps -axo command= | grep -E "^/bin/bash \./sub_run\.sh $M v5(6[6-9]|7[01]) " | grep -vc grep; }
while true; do
  j=$(head -1 ab-queue.txt)
  if [ -z "$j" ]; then [ "$(running)" -eq 0 ] && break; sleep 30; continue; fi
  [ "$(running)" -ge 4 ] && { sleep 30; continue; }
  sed -i '' 1d ab-queue.txt
  set -- $j
  SUB_SET=$1 SUB_HARNESS=$2 HARNESS_BIN=$HOME/bench-homes/tz-v98p/tazuna ./sub_run.sh $M v$4 $P 0.5 $3 &
  log "start v$4 $2"
  sleep 5
done
log "runs over"
python3 -I leakscan.py > leakscan-ab.out 2>&1
log "leak scan: $(grep -E 'v5(6[6-9]|7[01])@' leakscan-ab.out | grep -vcE '(web|peer)-(blocked|listed)(,(web|peer)-(blocked|listed))*$') runs to look at"
ps -axo command= | grep -q "[g]uard.py" || (cd .. && nohup ./guard.py >> guard.log 2>&1 &)
ONLY="$(for v in 566 568 570; do echo -n "tazuna-ownsubx-v$v@$M:max,"; done)$(for v in 567 569 571; do echo -n "tazuna-ownsubxp2-v$v@$M:max,"; done)"
BENCH_SET=tasks-oct JUDGE_TASKS=caddy-retry-match-expr JUDGE_OUT=judge.absol.tasks-oct.jsonl JUDGE_ONLY="$ONLY" python3 -I ./judge.py --passes 2 >> judge-absol.log 2>&1
log "AB-SOL-DONE"
