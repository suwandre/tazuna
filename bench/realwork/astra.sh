#!/bin/bash
# 2026-10-29: gpt-6-astra at max on SubRouter's shared OpenAI Pro pools (tazuna-vet-1, Pro 50 and 52; probe: 92% cached
# over four conversations, no errors, ~$0.001 a call): tazuna 0.1.0 against stock Codex and pi, one run a task
# (v623-v634, astra.jobs), four at a time; then one judge pool per task, two passes. Spend stop 1500 fen.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> astra.log; }
caffeinate -ims -w $$ &
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUBROUTER_X_KEYENV=SUBROUTER_VET1_KEY
M=gpt-6-astra P=0.2,0.02,1.0
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
[ -n "$U" ] && ./sub_guard.sh "$U" 1500 $(seq -f "v%g" 623 634) >> sub-guard-astra.log 2>&1 &
running() { ps -axo command= | grep -E "^/bin/bash \./sub_run\.sh $M v6(2[3-9]|3[0-4]) " | grep -vc grep; }
while read -r set h task v; do
  while [ "$(running)" -ge 4 ]; do sleep 30; done
  SUB_SET=$set SUB_HARNESS=$h HARNESS_BIN=$HOME/bench-homes/tz-v100/tazuna ./sub_run.sh $M v$v $P 0.5 $task < /dev/null &
  log "start v$v $h $task"; sleep 5
done < astra.jobs
while [ "$(running)" -gt 0 ]; do sleep 30; done
log "runs over"
python3 -I leakscan.py > leakscan-astra.out 2>&1
ps -axo command= | grep -q "[g]uard.py" || (cd .. && nohup ./guard.py >> guard.log 2>&1 &)
ONLY="$(for v in $(seq 623 634); do for h in tazuna-ownsubx codexsub pi-sub; do echo -n "$h-v$v@$M:max,"; done; done)"
for s in "tasks caddy-quoted-braces,terraform-import-foreach" "tasks-hard caddy-tls-automate-names" "tasks-oct caddy-retry-match-expr"; do
  set -- $s
  BENCH_SET=$1 JUDGE_TASKS=$2 JUDGE_OUT=judge.astra.$1.jsonl JUDGE_ONLY="$ONLY" python3 -I ./judge.py --passes 2 >> judge-astra.log 2>&1
  log "judged $2"
done
log "ASTRA-DONE"
