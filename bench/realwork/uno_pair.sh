#!/bin/bash
# UnoRouter: DeepSeek V4.1 Flash and V4.1 Flash 1017 at max, each after its route check (up to 4 groups), side by side:
#   uno_pair.sh LABEL_FLASH LABEL_1017 [BUDGET_FLASH] [BUDGET_1017]     logs to uno-pair.log
cd "$(dirname "$0")"; source ~/.uno-key
log() { echo "$(date +%H:%M) $*" >> uno-pair.log; }
one() {  # MODEL LABEL BUDGET
  ( cd .. && python3 -I -u uno_route.py "$1" 4 > "realwork/check-$2.log" 2>&1 )
  g=$(grep '^GROUPS' "check-$2.log" | cut -d' ' -f2)
  [ -z "$g" ] && { log "$2 $1: no group passed ($(grep -E 'no:' "check-$2.log" | tail -1 | sed 's/^ *//'))"; return; }
  log "$2 $1 on $g, budget \$$3"; ./uno_run.sh "$1" "$g" "$2" "$3" > "uno-$2.out" 2>&1
  log "$2 done: $(grep -h 'end:' "uno-$2.log" | tail -1)"
}
one deepseek-v4.1-flash "$1" "${3:-0.03}" &
one deepseek-v4.1-flash-1017 "$2" "${4:-0.035}" &
wait; log "UNO-PAIR-DONE $1 $2"
