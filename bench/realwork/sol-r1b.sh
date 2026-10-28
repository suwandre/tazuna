#!/bin/bash
# 2026-10-28: sol-r1.sh again, with pi terraform rerun as v557 (v537 finished but the old parser threw it away over one
# failed call pi had retried), queued after v541-v544; judged with the rest of round 1.



cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> sol-panel.log; }
caffeinate -ims -w $$ &
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUBROUTER_X_KEYENV=SUBROUTER_VET1_KEY
M=gpt-6.1-sol P=0.04,0.002,0.2
running() { ps -axo command= | grep -E "^/bin/bash \./sub_run\.sh $M v5(3[3-9]|4[0-4]|57) " | grep -vc grep; }
for j in \
         "tasks-oct tazuna-ownsubx caddy-retry-match-expr 541" \
         "tasks-hard codexsub caddy-tls-automate-names 542" "tasks-hard pi-sub caddy-tls-automate-names 543" "tasks-hard tazuna-ownsubx caddy-tls-automate-names 544" "tasks pi-sub terraform-import-foreach 557"; do
  while [ "$(running)" -ge 4 ]; do sleep 30; done
  set -- $j
  SUB_SET=$1 SUB_HARNESS=$2 HARNESS_BIN=$HOME/bench-homes/tz-v97/tazuna ./sub_run.sh $M v$4 $P 0.5 $3 &
  log "start v$4 $2 $3"
  sleep 5
done
while [ "$(running)" -gt 0 ]; do sleep 30; done
log "round 1 over"
python3 -I leakscan.py > leakscan-sol.out 2>&1
log "leak scan: $(grep -E 'v(53[3-9]|54[0-4]|557)@' leakscan-sol.out | grep -vcE '(web|peer)-blocked(,(web|peer)-blocked)?$') runs to look at"
ps -axo command= | grep -q "[g]uard.py" || (cd .. && nohup ./guard.py >> guard.log 2>&1 &)
ONLY="$(for v in $(seq 533 544) 557; do for h in codexsub pi-sub tazuna-ownsubx; do echo -n "$h-v$v@$M:max,"; done; done)"
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
