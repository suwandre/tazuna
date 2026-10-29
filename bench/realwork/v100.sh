#!/bin/bash
# 2026-10-29: tazuna 0.1.0 (the model's reasoning no longer goes back with the history by default) against last night's
# 0.0.99 runs: MiniMax-M3.1-Flash at max on foxrouter (tazuna-vet-4) caddy-quoted twice (v616, v619), caddy-retry (v617),
# terraform (v618, where 0.0.99 died at the context limit); DeepSeek 1017 on foxrouter (tazuna-vet-2) caddy-quoted (v620)
# and caddy-retry (v621). Four at a time (queue v100-queue.txt: model keyenv price set task label); then MiniMax's
# caddy-quoted and caddy-retry judged with last night's runs. Spend stop 600 fen.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> v100.log; }
caffeinate -ims -w $$ &
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
[ -n "$U" ] && ./sub_guard.sh "$U" 600 v616 v617 v618 v619 v620 v621 >> sub-guard-v100.log 2>&1 &
running() { ps -axo command= | grep -E "^/bin/bash \./sub_run\.sh \S+ v6(1[6-9]|2[01]) " | grep -vc grep; }
while true; do
  j=$(head -1 v100-queue.txt)
  if [ -z "$j" ]; then [ "$(running)" -eq 0 ] && break; sleep 30; continue; fi
  [ "$(running)" -ge 4 ] && { sleep 30; continue; }
  sed -i '' 1d v100-queue.txt
  set -- $j
  SUBROUTER_X_KEYENV=$2 SUB_SET=$4 SUB_HARNESS=tazuna-ownsubx HARNESS_BIN=$HOME/bench-homes/tz-v100/tazuna ./sub_run.sh $1 v$6 $3 0.5 $5 < /dev/null &
  log "start v$6 $1 $5"; sleep 5
done
log "runs over"
python3 -I leakscan.py > leakscan-v100.out 2>&1
ps -axo command= | grep -q "[g]uard.py" || (cd .. && nohup ./guard.py >> guard.log 2>&1 &)
X=minimax-m3.1-flash
ONLY="tazuna-ownsubx-v604@$X:max,tazuna-ownsubx-v610@$X:max,tazuna-ownsubx-v616@$X:max,tazuna-ownsubx-v617@$X:max,tazuna-ownsubx-v619@$X:max,cc-sub-v605@$X:max,cc-sub-v611@$X:max,pi-sub-v612@$X:max"
BENCH_SET=tasks JUDGE_TASKS=caddy-quoted-braces JUDGE_OUT=judge.v100.mm.tasks.jsonl JUDGE_ONLY="$ONLY" python3 -I ./judge.py --passes 2 >> judge-v100.log 2>&1
BENCH_SET=tasks-oct JUDGE_TASKS=caddy-retry-match-expr JUDGE_OUT=judge.v100.mm.tasks-oct.jsonl JUDGE_ONLY="$ONLY" python3 -I ./judge.py --passes 2 >> judge-v100.log 2>&1
log "V100-DONE"
