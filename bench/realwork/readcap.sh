#!/bin/bash
# 2026-10-29: TAZUNA_READ_MAX=12000 (a read stops at whole lines after 12K characters) against tazuna 0.1.0's whole-file
# reads: gpt-6-astra on the shared Pro pools, four tasks (v635-v638, against v623/v626/v629/v632), DeepSeek 1017 on LingAI
# caddy-quoted and caddy-retry (v639, v640) with a clean 0.1.0 caddy-retry beside them (v641; v622 is 0.1.0's
# caddy-quoted). Four at a time (readcap-queue.txt: model keyenv price set harness task label bindir); then the Astra
# pools judged with every Astra run. Spend stop 700 fen.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> readcap.log; }
caffeinate -ims -w $$ &
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
[ -n "$U" ] && ./sub_guard.sh "$U" 700 $(seq -f "v%g" 635 641) >> sub-guard-readcap.log 2>&1 &
running() { ps -axo command= | grep -E "^/bin/bash \./sub_run\.sh \S+ v6(3[5-9]|4[01]) " | grep -vc grep; }
while true; do
  j=$(head -1 readcap-queue.txt)
  if [ -z "$j" ]; then [ "$(running)" -eq 0 ] && break; sleep 30; continue; fi
  [ "$(running)" -ge 4 ] && { sleep 30; continue; }
  sed -i '' 1d readcap-queue.txt
  set -- $j
  SUBROUTER_X_KEYENV=$2 SUB_SET=$4 SUB_HARNESS=$5 HARNESS_BIN=$HOME/bench-homes/$8/tazuna ./sub_run.sh $1 v$7 $3 0.5 $6 < /dev/null &
  log "start v$7 $5 $1 $6"; sleep 5
done
log "runs over"
python3 -I leakscan.py > leakscan-readcap.out 2>&1
ps -axo command= | grep -q "[g]uard.py" || (cd .. && nohup ./guard.py >> guard.log 2>&1 &)
M=gpt-6-astra
ONLY="$(for v in $(seq 623 638); do for h in tazuna-ownsubx tazuna-ownsubxr codexsub pi-sub; do echo -n "$h-v$v@$M:max,"; done; done)"
for s in "tasks caddy-quoted-braces,terraform-import-foreach" "tasks-hard caddy-tls-automate-names" "tasks-oct caddy-retry-match-expr"; do
  set -- $s
  BENCH_SET=$1 JUDGE_TASKS=$2 JUDGE_OUT=judge.readcap.$1.jsonl JUDGE_ONLY="$ONLY" python3 -I ./judge.py --passes 2 >> judge-readcap.log 2>&1
  log "judged $2"
done
log "READCAP-DONE"
