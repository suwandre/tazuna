#!/bin/bash
# provgauge.sh every GAUGE_EVERY seconds (2 hours) from 10:00 to 23:59 until 2026-11-08 12:00, as a background loop
# started with nohup (a LaunchAgent was not allowed, 2026-11-05). No rounds from 00:00 to 09:59 (the user is asleep, so
# those hours are not worth measuring, 2026-11-05). Each daytime round keeps the Mac awake on AC power until the next one
# (caffeinate -s -t), so the Mac sleeps normally at night and on battery. Gone after a reboot or logout; a round that
# came due while the Mac slept runs on wake (wall-clock checks every 5 minutes: sleep(1) does not count sleep).
# Stop it with pkill -f provgauge_loop.sh.
cd "$(dirname "$0")" || exit 1
every=${GAUGE_EVERY:-7200}
end=$(date -j -f '%Y-%m-%d %H:%M' '2026-11-08 12:00' +%s)
next=$(( $(date +%s) + ${1:-$every} ))  # first round after $1 seconds
while [ "$(date +%s)" -lt "$end" ]; do
  if [ "$(date +%s)" -ge "$next" ]; then
    next=$(( $(date +%s) + every ))
    if [ "$(date +%H)" -ge 10 ]; then
      caffeinate -s -t $(( every + 300 )) &
      ./provgauge.sh
    fi
  fi
  sleep 300
done
echo "$(date '+%m-%d %H:%M') loop ended (end date)" >> provgauge.log
