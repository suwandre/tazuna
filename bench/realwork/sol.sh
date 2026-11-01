#!/bin/bash
# 2026-11-01: GPT-6.1 Sol at xhigh as the own loop's cheap model, against DeepSeek: the four tasks x two runs DeepSeek ran on
# LingAI (xh.sh, v768-v782) and on znu (znu.sh, v1101-v1108), v1201-v1208, all at once, on Sol's checked route (Kriora and
# the shared Pro pools; tazuna-route-gpt-6-1-sol). Spend stop $0.30 account-wide (nothing else running). Then one judge
# pool per task with all six runs, Opus xhigh, two passes: 8 calls under the usage guard. Started detached.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> sol.log; }
caffeinate -ims -w $$ &
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
./sub_guard.sh "$U" 210 $(seq -f "v%g" 1201 1208) >> sub-guard-sol.log 2>&1 &
log "start; SubRouter used $U"
while read -r set task v; do
  SUB_GATE= SUB_SET=$set SUB_EFFORT=xhigh SUB_HARNESS=tazuna-ownsubx SUBROUTER_X_KEYENV=SUBROUTER_ROUTE_GPT_6_1_SOL_KEY SUB_TIMEOUT=3600 HARNESS_BIN=$HOME/bench-homes/tz-v108/tazuna \
    ./sub_run.sh gpt-6.1-sol "v$v" 0.03869,0.0039,0.19347 0.5 "$task" < /dev/null &
  log "start v$v $task"; sleep 5
done < sol.jobs
wait $(jobs -p | grep -v "^$(pgrep -P $$ caffeinate)$") 2>/dev/null
for v in $(seq 1201 1208); do log "  v$v: $(grep -h ' r0:\|ERROR' bench.v$v.log 2>/dev/null | tail -1 | cut -c1-120)"; done
log "SOL-RUNS-DONE"
python3 -I leakscan.py > leakscan-sol.out 2>&1
D=deepseek-v4.1-flash-1017 S=gpt-6.1-sol
for s in "tasks caddy-quoted-braces 1201 1202 772 774 1101 1102" "tasks terraform-import-foreach 1203 1204 768 770 1103 1104" \
         "tasks-oct caddy-retry-match-expr 1205 1206 776 778 1105 1106" "tasks-oct rclone-archive-symlinks 1207 1208 780 782 1107 1108"; do
  set -- $s
  ONLY="tazuna-ownsubx-v$3@$S:xhigh,tazuna-ownsubx-v$4@$S:xhigh,tazuna-ownsubx-v$5@$D:xhigh,tazuna-ownsubx-v$6@$D:xhigh,tazuna-ownsubx-v$7@$D:high,tazuna-ownsubx-v$8@$D:high"
  BENCH_SET=$1 JUDGE_TASKS=$2 JUDGE_OUT=judge.sol.$2.jsonl JUDGE_ONLY="$ONLY" python3 -I ./judge.py --passes 2 >> judge-sol.log 2>&1
  log "judged $2"
done
log "SOL-DONE"
