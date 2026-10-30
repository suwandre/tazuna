#!/bin/bash
# 2026-10-30: the PROMPT3 lever (TAZUNA_PROMPT3: "focused change" instead of "smallest change", remove what a change leaves
# unused, tests for edge cases, internal packages are not an API) against the current prompt, tazuna 0.1.6 (tz-v106p, the
# lever behind an env var), DeepSeek 1017 max on SubRouter LingAI, a key per arm (vet-3 current, vet-2 lever). Four tasks x
# two runs per arm (v689-v704, p3.jobs), eight at a time; then one judge pool per task with all four runs, two passes.
# Spend stop 400 fen (nothing else on the account).
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> p3.log; }
caffeinate -ims -w $$ &
export SUB_EFFORT=max SUB_KEYENV=SUBROUTER_MODELS_KEY GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95
M=deepseek-v4.1-flash-1017 P=0.022,0.0022,0.088
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
[ -n "$U" ] && ./sub_guard.sh "$U" 400 $(seq -f "v%g" 689 704) >> sub-guard-p3.log 2>&1 &
ps -axo command= | grep -q "[g]uard.py" || (cd .. && nohup ./guard.py >> guard.log 2>&1 &)
running() { ps -axo command= | grep -E "^/bin/bash \./sub_run\.sh $M v(689|69[0-9]|70[0-4]) " | grep -vc grep; }
while read -r k set h task v; do
  while [ "$(running)" -ge 8 ]; do sleep 30; done
  SUBROUTER_X_KEYENV=$k SUB_SET=$set SUB_HARNESS=$h HARNESS_BIN=$HOME/bench-homes/tz-v106p/tazuna ./sub_run.sh $M v$v $P 0.5 $task < /dev/null &
  log "start v$v $h $task ($k)"; sleep 5
done < p3.jobs
while [ "$(running)" -gt 0 ]; do sleep 30; done
log "runs over"
python3 -I leakscan.py > leakscan-p3.out 2>&1
ONLY="$(for v in $(seq 689 704); do for h in tazuna-ownsubx tazuna-ownsubxp3; do echo -n "$h-v$v@$M:max,"; done; done)"
for s in "tasks terraform-import-foreach" "tasks-oct caddy-retry-match-expr,prometheus-exemplar-compaction,rclone-archive-symlinks"; do
  set -- $s
  BENCH_SET=$1 JUDGE_TASKS=$2 JUDGE_OUT=judge.p3.$1.jsonl JUDGE_ONLY="$ONLY" python3 -I ./judge.py --passes 2 >> judge-p3.log 2>&1
  log "judged $2"
done
log "P3-DONE"
