#!/bin/bash
# 2026-10-28 evening: Codex caddy-tls rerun (v562; v542 finished but the old parser threw it away, v561 stopped at the
# pause) and caddy-retry round 2 for all three (v563-v565), four at once, then caddy-tls and caddy-retry judged with every
# valid Sol run (one pool per task, two passes; judge.sol1028b.*). Spend stop 800 fen.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> sol-panel.log; }
caffeinate -ims -w $$ &
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUBROUTER_X_KEYENV=SUBROUTER_VET1_KEY
M=gpt-6.1-sol P=0.04,0.002,0.2
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
[ -n "$U" ] && ./sub_guard.sh "$U" 800 v562 v563 v564 v565 >> sub-guard-sol.log 2>&1 &
running() { ps -axo command= | grep -E "^/bin/bash \./sub_run\.sh $M v5[3-9][0-9] " | grep -vc grep; }
while true; do
  j=$(head -1 sol-queue.txt)
  if [ -z "$j" ]; then [ "$(running)" -eq 0 ] && [ -z "$(head -1 sol-queue.txt)" ] && break; sleep 30; continue; fi
  [ "$(running)" -ge 4 ] && { sleep 30; continue; }
  sed -i '' 1d sol-queue.txt
  set -- $j
  SUB_SET=$1 SUB_HARNESS=$2 HARNESS_BIN=$HOME/bench-homes/tz-v97/tazuna ./sub_run.sh $M v$4 $P 0.5 $3 &
  log "start v$4 $2 $3"
  sleep 5
done
log "evening runs over"
python3 -I leakscan.py > leakscan-sol.out 2>&1
log "leak scan: $(grep -E 'v5[3-9][0-9]@gpt-6.1-sol' leakscan-sol.out | grep -vcE '(web|peer)-blocked(,(web|peer)-blocked)?$') runs to look at"
ps -axo command= | grep -q "[g]uard.py" || (cd .. && nohup ./guard.py >> guard.log 2>&1 &)
ONLY="$(for v in 533 534 535 536 538 543 544 557 558 559 560 562 563 564 565; do for h in codexsub pi-sub tazuna-ownsubx; do echo -n "$h-v$v@$M:max,"; done; done)"
judge() {  # set tasks expected-calls
  n=$(BENCH_SET=$1 JUDGE_TASKS=$2 JUDGE_OUT=judge.sol1028b.$1.jsonl JUDGE_ONLY="$ONLY" python3 -I ./judge.py --passes 2 --dry-run 2>&1 | grep -oE "= [0-9]+ judge calls" | grep -oE "[0-9]+")
  if [ "${n:-0}" = "$3" ]; then
    BENCH_SET=$1 JUDGE_TASKS=$2 JUDGE_OUT=judge.sol1028b.$1.jsonl JUDGE_ONLY="$ONLY" python3 -I ./judge.py --passes 2 >> judge-sol1028.log 2>&1
    log "judged $1: $(grep -c . judge.sol1028.$1.jsonl 2>/dev/null) rows"
  else
    log "judge $1 not run: dry run said ${n:-?} (expected $3)"
  fi
}
judge tasks-hard caddy-tls-automate-names 2
judge tasks-oct caddy-retry-match-expr 2
log "SOL-EVENING-DONE"
