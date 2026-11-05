#!/bin/bash
# provgauge.sh every 4 hours until 2026-11-08 12:00, as a background loop started with nohup (2026-11-05: a LaunchAgent
# was not allowed). Gone after a reboot or logout; while the Mac sleeps nothing runs, and the round that came due runs on
# wake (wall-clock checks every 5 minutes, since sleep(1) does not count the time the Mac sleeps). Stop it with
# pkill -f provgauge_loop.sh.
cd "$(dirname "$0")" || exit 1
end=$(date -j -f '%Y-%m-%d %H:%M' '2026-11-08 12:00' +%s)
next=$(( $(date +%s) + ${1:-14400} ))  # first round after $1 seconds (default 4 hours)
while [ "$(date +%s)" -lt "$end" ]; do
  if [ "$(date +%s)" -ge "$next" ]; then
    next=$(( $(date +%s) + 14400 ))
    ./provgauge.sh
  fi
  sleep 300
done
echo "$(date '+%m-%d %H:%M') loop ended (end date)" >> provgauge.log
