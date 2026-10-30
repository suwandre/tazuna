#!/bin/bash
# 2026-10-30: the thorough workflow (TAZUNA_THOROUGH: reproduce, a failing-before/passing-after regression test, boundaries and precedence,
# every requirement, a clean cutover; in the spirit of oh-my-pi's prompt) against none, tazuna 0.1.6 (tz-v106thh, lever behind an env var), DeepSeek 1017 max on SubRouter LingAI, a key per arm (vet-3 current, vet-2 lever). Four tasks x
# two runs per arm (v751-v766, th.jobs; started with nohup), eight at a time; then one judge pool per task with all four runs, two passes.
# Spend stop 800 fen (nothing else on the account).
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> th.log; }
caffeinate -ims -w $$ &
export SUB_EFFORT=max SUB_KEYENV=SUBROUTER_MODELS_KEY GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95
M=deepseek-v4.1-flash-1017 P=0.022,0.0022,0.088
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
[ -n "$U" ] && ./sub_guard.sh "$U" 800 $(seq -f "v%g" 751 766) >> sub-guard-th.log 2>&1 &
ps -axo command= | grep -q "[g]uard.py" || (cd .. && nohup ./guard.py >> guard.log 2>&1 &)
running() { ps -axo command= | grep -E "^/bin/bash \./sub_run\.sh $M v(75[1-9]|76[0-6]) " | grep -vc grep; }
while read -r k set h task v; do
  while [ "$(running)" -ge 8 ]; do sleep 30; done
  SUBROUTER_X_KEYENV=$k SUB_SET=$set SUB_HARNESS=$h HARNESS_BIN=$HOME/bench-homes/tz-v106th/tazuna ./sub_run.sh $M v$v $P 0.5 $task < /dev/null &
  log "start v$v $h $task ($k)"; sleep 5
done < th.jobs
while [ "$(running)" -gt 0 ]; do sleep 30; done
log "runs over"
python3 -I leakscan.py > leakscan-th.out 2>&1
ONLY="$(for v in $(seq 751 766); do for h in tazuna-ownsubx tazuna-ownsubxth; do echo -n "$h-v$v@$M:max,"; done; done)"
for s in "tasks terraform-import-foreach,caddy-quoted-braces" "tasks-oct caddy-retry-match-expr,rclone-archive-symlinks"; do
  set -- $s
  BENCH_SET=$1 JUDGE_TASKS=$2 JUDGE_OUT=judge.th.$1.jsonl JUDGE_ONLY="$ONLY" python3 -I ./judge.py --passes 2 >> judge-th.log 2>&1
  log "judged $2"
done
log "TH-DONE"
