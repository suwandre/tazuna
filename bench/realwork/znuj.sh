#!/bin/bash
# The judge for znu.sh: one pool per task, znu's two runs (v1101-v1108) and LingAI's two from xh.sh (v768-v782), Opus
# xhigh, two passes: 8 calls, under the usage guard (now on the claude CLI's own meter). Starts when znu.sh is done.
cd "$(dirname "$0")"; source ~/.bench-token
until grep -q ZNU-DONE znu.log 2>/dev/null; do sleep 60; done
python3 -I leakscan.py > leakscan-znu.out 2>&1
M=deepseek-v4.1-flash-1017
for s in "tasks caddy-quoted-braces 1101 1102 772 774" "tasks terraform-import-foreach 1103 1104 768 770" \
         "tasks-oct caddy-retry-match-expr 1105 1106 776 778" "tasks-oct rclone-archive-symlinks 1107 1108 780 782"; do
  set -- $s
  ONLY="tazuna-ownsubx-v$3@$M:high,tazuna-ownsubx-v$4@$M:high,tazuna-ownsubx-v$5@$M:xhigh,tazuna-ownsubx-v$6@$M:xhigh"
  BENCH_SET=$1 JUDGE_TASKS=$2 JUDGE_OUT=judge.znu.$2.jsonl JUDGE_ONLY="$ONLY" python3 -I ./judge.py --passes 2 >> judge-znu.log 2>&1
  echo "$(date +%H:%M) judged $2" >> znu.log
done
echo "$(date +%H:%M) ZNUJ-DONE" >> znu.log
