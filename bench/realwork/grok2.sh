#!/bin/bash
# 2026-10-29: Grok 4.7 (SubRouter, miletechx) on tazuna 0.1.5, after 0.1.4's read loop (offset "100,200" read as line 1:
# v667 and v670 asked 134+ times without an edit and were stopped). rclone against the October run (v161, judge 6), and
# caddy-quoted and terraform with DeepSeek 1017's no-review runs as the anchor; one run each (v680-v682, grok2.jobs), key
# tazuna-vet-1; then a judge pool per task, two passes. Spend stop 300 fen (account-wide: the Kimi round's last Grok run counts).
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> grok2.log; }
caffeinate -ims -w $$ &
export SUB_EFFORT=max SUB_KEYENV=SUBROUTER_MODELS_KEY GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
[ -n "$U" ] && ./sub_guard.sh "$U" 300 $(seq -f "v%g" 680 682) >> sub-guard-grok2.log 2>&1 &
ps -axo command= | grep -q "[g]uard.py" || (cd .. && nohup ./guard.py >> guard.log 2>&1 &)
running() { ps -axo command= | grep -E "^/bin/bash \./sub_run\.sh \S+ v68[0-2] " | grep -vc grep; }
while read -r k set h m task v; do
  SUBROUTER_X_KEYENV=$k SUB_SET=$set SUB_HARNESS=$h HARNESS_BIN=$HOME/bench-homes/tz-v105/tazuna ./sub_run.sh $m v$v 0.011,0.003,0.033 1.0 $task < /dev/null &
  log "start v$v $h $m $task ($k)"; sleep 5
done < grok2.jobs
while [ "$(running)" -gt 0 ]; do sleep 30; done
log "runs over"
python3 -I leakscan.py > leakscan-grok2.out 2>&1
G=grok-4.7 D=deepseek-v4.1-flash-1017
for s in "tasks-oct rclone-archive-symlinks tazuna-ownsubx-v680@$G:max,tazuna-ownsubgp-v161@$G:high,tazuna-ownsubx-v665@kimi-k3.1-pro:max" \
         "tasks caddy-quoted-braces tazuna-ownsubx-v681@$G:max,tazuna-ownsubx-v648@$D:max" \
         "tasks terraform-import-foreach tazuna-ownsubx-v682@$G:max,tazuna-ownsubx-v651@$D:max"; do
  set -- $s
  BENCH_SET=$1 JUDGE_TASKS=$2 JUDGE_OUT=judge.grok2.$1.jsonl JUDGE_ONLY="$3" python3 -I ./judge.py --passes 2 >> judge-grok2.log 2>&1
  log "judged $2"
done
log "GROK2-DONE"
