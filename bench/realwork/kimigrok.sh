#!/bin/bash
# 2026-10-29: Kimi K3.1 Pro (SubRouter, znu) in tazuna 0.1.4 against Moonshot's own Kimi Code 2.4.0 (kimi-sub), and Grok 4.7
# (miletechx) in tazuna against its earlier run (v161, rclone, X Premium+ pool, judge 6). Four tasks, one run each, a key per
# arm (vet-1 tazuna/Kimi, vet-2 Kimi Code, vet-4 tazuna/Grok), eight at a time (v665-v676, kimigrok.jobs); then one judge
# pool per task with v161 on rclone and the no-review DeepSeek 1017 runs as the anchor elsewhere, two passes. Spend stop 1400 fen.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> kimigrok.log; }
caffeinate -ims -w $$ &
export SUB_EFFORT=max SUB_KEYENV=SUBROUTER_MODELS_KEY GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
[ -n "$U" ] && ./sub_guard.sh "$U" 1400 $(seq -f "v%g" 665 676) >> sub-guard-kimigrok.log 2>&1 &
ps -axo command= | grep -q "[g]uard.py" || (cd .. && nohup ./guard.py >> guard.log 2>&1 &)
running() { ps -axo command= | grep -E "^/bin/bash \./sub_run\.sh \S+ v6(6[5-9]|7[0-6]) " | grep -vc grep; }
while read -r k set h m task v; do
  while [ "$(running)" -ge 8 ]; do sleep 30; done
  case $m in kimi*) P=0.02,0.004,0.1 ;; *) P=0.011,0.003,0.033 ;; esac
  SUBROUTER_X_KEYENV=$k SUB_SET=$set SUB_HARNESS=$h HARNESS_BIN=$HOME/bench-homes/tz-v104/tazuna ./sub_run.sh $m v$v $P 1.0 $task < /dev/null &
  log "start v$v $h $m $task ($k)"; sleep 5
done < kimigrok.jobs
while [ "$(running)" -gt 0 ]; do sleep 30; done
log "runs over"
python3 -I leakscan.py > leakscan-kimigrok.out 2>&1
K=kimi-k3.1-pro G=grok-4.7 D=deepseek-v4.1-flash-1017
pool() { echo -n "tazuna-ownsubx-v$1@$K:max,kimi-sub-v$2@$K:max,tazuna-ownsubx-v$3@$G:max,$4"; }
for s in "tasks-oct rclone-archive-symlinks $(pool 665 666 667 tazuna-ownsubgp-v161@$G:high)" \
         "tasks caddy-quoted-braces $(pool 668 669 670 tazuna-ownsubx-v648@$D:max)" \
         "tasks terraform-import-foreach $(pool 671 672 673 tazuna-ownsubx-v651@$D:max)" \
         "tasks-oct caddy-retry-match-expr $(pool 674 675 676 tazuna-ownsubx-v654@$D:max)"; do
  set -- $s
  BENCH_SET=$1 JUDGE_TASKS=$2 JUDGE_OUT=judge.kimigrok.$1.jsonl JUDGE_ONLY="$3" python3 -I ./judge.py --passes 2 >> judge-kimigrok.log 2>&1
  log "judged $2"
done
log "KIMIGROK-DONE"
