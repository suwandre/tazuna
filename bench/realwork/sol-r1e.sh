#!/bin/bash
# 2026-10-28: the Sol panel's round 1 with a queue file (sol-queue.txt: "set harness task label" per line), so reruns can
# be added while it runs: v537/v539 (finished, thrown away by the old parsers), v540/v541 (the 15:38-15:50 outage of
# shared Pro 50; the key now lists Pro 50 and 52). Four at a time; then the leak scan and the judge over every round-1
# run from v533 on that has a result row.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> sol-panel.log; }
caffeinate -ims -w $$ &
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUBROUTER_X_KEYENV=SUBROUTER_VET1_KEY
M=gpt-6.1-sol P=0.04,0.002,0.2
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
log "round 1 over"
python3 -I leakscan.py > leakscan-sol.out 2>&1
log "leak scan: $(grep -E 'v5[3-9][0-9]@gpt-6.1-sol' leakscan-sol.out | grep -vcE '(web|peer)-blocked(,(web|peer)-blocked)?$') runs to look at"
ps -axo command= | grep -q "[g]uard.py" || (cd .. && nohup ./guard.py >> guard.log 2>&1 &)
ONLY="$(for v in $(seq 533 599); do for h in codexsub pi-sub tazuna-ownsubx; do echo -n "$h-v$v@$M:max,"; done; done)"
judge() {  # set tasks expected-calls
  n=$(BENCH_SET=$1 JUDGE_TASKS=$2 JUDGE_OUT=judge.sol1028.$1.jsonl JUDGE_ONLY="$ONLY" python3 -I ./judge.py --passes 2 --dry-run 2>&1 | grep -oE "= [0-9]+ judge calls" | grep -oE "[0-9]+")
  if [ "${n:-0}" = "$3" ]; then
    BENCH_SET=$1 JUDGE_TASKS=$2 JUDGE_OUT=judge.sol1028.$1.jsonl JUDGE_ONLY="$ONLY" python3 -I ./judge.py --passes 2 >> judge-sol1028.log 2>&1
    log "judged $1: $(grep -c . judge.sol1028.$1.jsonl 2>/dev/null) rows"
  else
    log "judge $1 not run: dry run said ${n:-?} (expected $3)"
  fi
}
judge tasks caddy-quoted-braces,terraform-import-foreach 4
judge tasks-hard caddy-tls-automate-names 2
judge tasks-oct caddy-retry-match-expr 2
log "SOL-PANEL-DONE"
