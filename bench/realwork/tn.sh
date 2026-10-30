#!/bin/bash
# 2026-10-30: the tests nudge (TAZUNA_TEST_NUDGE: a headless run that ends with a code change and no test file is asked once to add
# a test that fails without the change) against none, tazuna 0.1.6 (tz-v106t, the lever behind an env var), DeepSeek 1017 max on SubRouter LingAI, a key per arm (vet-3 current, vet-2 lever). Four tasks x
# two runs per arm (v705-v720, tn.jobs; started with nohup), eight at a time; then one judge pool per task with all four runs, two passes.
# Spend stop 800 fen (nothing else on the account).
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> tn.log; }
caffeinate -ims -w $$ &
export SUB_EFFORT=max SUB_KEYENV=SUBROUTER_MODELS_KEY GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95
M=deepseek-v4.1-flash-1017 P=0.022,0.0022,0.088
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
[ -n "$U" ] && ./sub_guard.sh "$U" 800 $(seq -f "v%g" 705 720) >> sub-guard-tn.log 2>&1 &
ps -axo command= | grep -q "[g]uard.py" || (cd .. && nohup ./guard.py >> guard.log 2>&1 &)
running() { ps -axo command= | grep -E "^/bin/bash \./sub_run\.sh $M v(70[5-9]|71[0-9]|720) " | grep -vc grep; }
while read -r k set h task v; do
  while [ "$(running)" -ge 8 ]; do sleep 30; done
  SUBROUTER_X_KEYENV=$k SUB_SET=$set SUB_HARNESS=$h HARNESS_BIN=$HOME/bench-homes/tz-v106t/tazuna ./sub_run.sh $M v$v $P 0.5 $task < /dev/null &
  log "start v$v $h $task ($k)"; sleep 5
done < tn.jobs
while [ "$(running)" -gt 0 ]; do sleep 30; done
log "runs over"
python3 -I leakscan.py > leakscan-tn.out 2>&1
ONLY="$(for v in $(seq 705 720); do for h in tazuna-ownsubx tazuna-ownsubxtn; do echo -n "$h-v$v@$M:max,"; done; done)"
for s in "tasks terraform-import-foreach,caddy-quoted-braces" "tasks-oct caddy-retry-match-expr,rclone-archive-symlinks"; do
  set -- $s
  BENCH_SET=$1 JUDGE_TASKS=$2 JUDGE_OUT=judge.tn.$1.jsonl JUDGE_ONLY="$ONLY" python3 -I ./judge.py --passes 2 >> judge-tn.log 2>&1
  log "judged $2"
done
log "TN-DONE"
