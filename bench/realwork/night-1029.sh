#!/bin/bash
# 2026-10-28/29 night, in sequence: (1) tazuna 0.0.99 on gpt-6.1-sol, two runs a task (v584-v591), judged with the Sol
# panel's Codex and pi runs; (2) GLM-5.3 and (3) MiniMax-M3.1-Flash, tazuna 0.0.99 against stock Claude Code and pi, one run
# a task (phase files night-1029-glm.jobs / -mm.jobs, written after the probes), each judged per task. Four at a time.
# Spend stop per phase (sub_guard) and for the night (2500 fen); stops at 15% battery or 08:00; caffeinated throughout;
# summary, local commit and pmset sleepnow only when everything has ended.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> night-1029.log; }
caffeinate -ims -w $$ &
log "caffeinate on"
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95
used() { curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])"; }
START_USED=$(used); [ -n "$START_USED" ] || START_USED=$(used)
over() { u=$(used); [ -n "$u" ] && [ -n "$START_USED" ] && python3 -I -c "import sys; sys.exit(0 if $u - $START_USED >= 2500 else 1)"; }
low() { b=$(pmset -g batt | grep -oE '[0-9]+%' | tr -d %); { [ -n "$b" ] && [ "$b" -le 15 ] && ! pmset -g batt | grep -q "AC Power"; } || { [ "$(date +%H%M)" -ge 0800 ] && [ "$(date +%H%M)" -lt 1200 ]; }; }
tree() { local c; for c in $(ps -axo pid=,ppid= | awk -v p="$1" '$2==p{print $1}'); do tree "$c"; done; echo "$1"; }
guard() { ps -axo command= | grep -q "[g]uard.py" || { (cd .. && nohup ./guard.py >> guard.log 2>&1 &); log "usage guard restarted"; }; }
STOP=0
# phase MODEL KEYENV PRICE JOBFILE: lines "set harness task label"; four at a time
phase() {
  local M=$1 P=$3 jobs=$4; export SUBROUTER_X_KEYENV=$2
  local labels=$(awk '{print "v"$4}' "$jobs" | tr '\n' ' ')
  local U=$(used); [ -n "$U" ] && ./sub_guard.sh "$U" 1200 $labels >> sub-guard-night1029.log 2>&1 &
  local re="$(awk '{printf "%s|", $4}' "$jobs" | sed 's/|$//')"
  running() { ps -axo command= | grep -E "^/bin/bash \./sub_run\.sh $M v($re) " | grep -vc grep; }
  while read -r set h task v; do
    while [ "$(running)" -ge 4 ]; do sleep 30; guard; if low || over; then STOP=1; break; fi; done
    [ $STOP = 1 ] && break
    SUB_SET=$set SUB_HARNESS=$h HARNESS_BIN=$HOME/bench-homes/tz-v99/tazuna ./sub_run.sh $M v$v $P 0.5 $task < /dev/null &
    log "start v$v $h $task ($M)"; sleep 5
  done < "$jobs"
  while [ "$(running)" -gt 0 ]; do
    sleep 30; guard
    if [ $STOP = 1 ] || low || over; then STOP=1; log "stop: battery, 08:00 or night budget"; for p in $(ps -axo pid=,command= | grep -E "sub_run.sh $M v($re) " | grep -v grep | awk '{print $1}'); do kill $(tree $p) 2>/dev/null; done; for p in $(ps -axo pid=,command= | grep -E "\-v($re)@" | grep -v grep | awk '{print $1}'); do kill $(tree $p) 2>/dev/null; done; break; fi
  done
  log "phase $M over"
}
# judge SET TASKS ONLY OUT EXPECTED: only when the dry run counts the expected calls
judge() {
  local n=$(BENCH_SET=$1 JUDGE_TASKS=$2 JUDGE_OUT=$4 JUDGE_ONLY="$3" python3 -I ./judge.py --passes 2 --dry-run 2>&1 | grep -oE "= [0-9]+ judge calls" | grep -oE "[0-9]+")
  if [ "${n:-0}" = "$5" ] && [ $STOP = 0 ]; then guard; BENCH_SET=$1 JUDGE_TASKS=$2 JUDGE_OUT=$4 JUDGE_ONLY="$3" python3 -I ./judge.py --passes 2 >> judge-night1029.log 2>&1; log "judged $2 -> $4"
  else log "judge $2 not run: dry run ${n:-?} (expected $5), stop=$STOP"; fi
}
labels() { local h=$1 M=$2; shift 2; for v in "$@"; do echo -n "$h-v$v@$M:max,"; done; }

# (1) Sol, tazuna 0.0.99
S=gpt-6.1-sol
phase $S SUBROUTER_VET1_KEY 0.04,0.002,0.2 night-1029-sol.jobs
python3 -I leakscan.py > leakscan-night1029.out 2>&1
SOL_ONLY="$(labels codexsub $S 533 536 558 562 563)$(labels pi-sub $S 534 543 557 559 564)$(labels tazuna-ownsubx $S $(seq 584 591))"
judge tasks caddy-quoted-braces,terraform-import-foreach "$SOL_ONLY" judge.night1029.sol.tasks.jsonl 4
judge tasks-hard caddy-tls-automate-names "$SOL_ONLY" judge.night1029.sol.tasks-hard.jsonl 2
judge tasks-oct caddy-retry-match-expr "$SOL_ONLY" judge.night1029.sol.tasks-oct.jsonl 2

# (2) and (3): written after the probes (model, key, price, jobs, judge labels); absent file = phase skipped
for f in night-1029-glm.phase night-1029-mm.phase; do
  [ $STOP = 0 ] && [ -f "$f" ] && { log "phase file $f"; source "$f"; }
done
log "runs over"
python3 -I leakscan.py > leakscan-night1029.out 2>&1
log "leak scan done"
cd ../.. && git add bench/realwork/night-1029* bench/realwork/judge.night1029.*.jsonl bench/realwork/results*.jsonl bench/realwork/leaks.jsonl 2>/dev/null
git commit -q -m "bench: night 2026-10-29, Sol with 0.0.99, GLM-5.3 and MiniMax-M3.1-Flash against Claude Code and pi" && log "committed"
cd bench/realwork; log "sleeping"; sleep 30; pmset sleepnow
