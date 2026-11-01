#!/bin/bash
# The night of 2026-11-01 (the user asleep): keeps the Mac awake while lx.sh (Luna vs DeepSeek), the gitea judge
# (mxj-later.sh) and the HLE canary (hle_after_round.sh) run. Sleeps it once all three are done (only if nobody touched it
# for 10 minutes), or earlier when the battery runs below 5% unplugged: then it stops them first (lx.sh skips finished
# jobs and the HLE canary skips answered questions, so both continue when started again). Started detached.
cd "$(dirname "$0")"
log() { echo "$(date +%H:%M) $*" >> night-1101.log; }
caffeinate -ims -w $$ &
tree() { local c; for c in $(ps -axo pid=,ppid= | awk -v p="$1" '$2==p{print $1}'); do tree "$c"; done; echo "$1"; }
stop_all() {
  for p in $(pgrep -f "lx\.sh|mxj\.sh|mxj-later\.sh|hle_after_round\.sh|canary_hle\.py|sub_run\.sh \S+ v10[0-2][0-9] "); do kill $(tree "$p") 2>/dev/null; done
}
log "watching: lx.sh, gitea judge, HLE"
while :; do
  b=$(pmset -g batt | sed -n 2p); pct=$(echo "$b" | grep -o '[0-9]*%' | tr -d %)
  if echo "$b" | grep -q discharging && [ "${pct:-100}" -lt 5 ]; then
    log "battery ${pct}% unplugged: stopping the runs, then sleep (start lx.sh and hle_after_round.sh again to continue)"
    stop_all; sleep 10; pmset sleepnow; exit 0
  fi
  if grep -q LX-DONE lx.log 2>/dev/null && grep -q "MXJ-DONE gitea" mxj.log 2>/dev/null && grep -q HLE-DONE ../hle-run.log 2>/dev/null; then
    idle=$(( $(ioreg -c IOHIDSystem | awk '/HIDIdleTime/ {print $NF; exit}') / 1000000000 ))
    if [ "$idle" -gt 600 ]; then log "all done, idle ${idle}s: sleeping"; pmset sleepnow; else log "all done, in use (idle ${idle}s): not sleeping"; fi
    exit 0
  fi
  sleep 60
done
