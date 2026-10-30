#!/bin/bash
# 2026-10-30, continues hx.sh (stopped before its judge): OpenCode rerun with its own sealed data folder (the jail had closed
# its usual one and its standalone server would not start: v722/v724/v726 failed at once) and stock Command Code 1.88 (BYOK,
# local-only, the user's login in a sealed per-run HOME, the relay adding reasoning_effort max), DeepSeek 1017 max on LingAI,
# one run per task (v729-v736, hx2.jobs; OpenCode tazuna-vet-4, Command Code tazuna-vet), three at a time. Then one judge pool
# per task: omp (hx.sh's v721-v727), OpenCode, Command Code and the tests-nudge A/B's two current-prompt tazuna runs, two passes.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> hx.log; }
caffeinate -ims -w $$ &
export SUB_EFFORT=max SUB_KEYENV=SUBROUTER_MODELS_KEY GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95
M=deepseek-v4.1-flash-1017 P=0.022,0.0022,0.088
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
[ -n "$U" ] && ./sub_guard.sh "$U" 1600 $(seq -f "v%g" 729 736) >> sub-guard-hx2.log 2>&1 &
ps -axo command= | grep -q "[g]uard.py" || (cd .. && nohup ./guard.py >> guard.log 2>&1 &)
mine() { ps -axo command= | grep -E "^/bin/bash \./sub_run\.sh $M v7(29|3[0-6]) " | grep -vc grep; }
omp() { ps -axo command= | grep -E "^/bin/bash \./sub_run\.sh $M v72[1357] " | grep -vc grep; }
while read -r k set h task v; do
  while [ "$(mine)" -ge 3 ]; do sleep 30; done
  SUBROUTER_X_KEYENV=$k SUB_SET=$set SUB_HARNESS=$h ./sub_run.sh $M v$v $P 0.5 $task < /dev/null &
  log "start v$v $h $task ($k)"; sleep 5
done < hx2.jobs
while [ "$(mine)" -gt 0 ] || [ "$(omp)" -gt 0 ]; do sleep 30; done
log "runs over; waiting for the A/B's runs"
until grep -q "runs over" tn.log; do sleep 60; done
python3 -I leakscan.py > leakscan-hx.out 2>&1
pool() { echo -n "omp-sub-v$1@$M:max,opencode-sub-v$2@$M:max,cmd-sub-v$3@$M:max,tazuna-ownsubx-v$4@$M:max,tazuna-ownsubx-v$5@$M:max"; }
for s in "tasks terraform-import-foreach $(pool 721 729 733 705 707)" "tasks caddy-quoted-braces $(pool 723 730 734 709 711)" \
         "tasks-oct caddy-retry-match-expr $(pool 725 731 735 713 715)" "tasks-oct rclone-archive-symlinks $(pool 727 732 736 717 719)"; do
  set -- $s
  BENCH_SET=$1 JUDGE_TASKS=$2 JUDGE_OUT=judge.hx.$1.jsonl JUDGE_ONLY="$3" python3 -I ./judge.py --passes 2 >> judge-hx.log 2>&1
  log "judged $2"
done
log "HX-DONE"
