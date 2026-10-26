#!/bin/bash
# Keeps the Mac awake for night-1025.sh and puts it to sleep once night-1025.done exists, 45 min after the summary is
# written (Claude did not finish), at 04:00, or at 15% battery off power. Logs to night-1025.log.
cd "$(dirname "$0")"
log() { echo "$(date +%H:%M) $*" >> night-1025.log; }
caffeinate -i -s -w $$ &
log "caffeinate on"
low() { b=$(pmset -g batt | grep -oE '[0-9]+%' | tr -d %); [ -n "$b" ] && [ "$b" -le 15 ] && ! pmset -g batt | grep -q "AC Power"; }
until [ -e night-1025.done ]; do
  if grep -q "summary written" night-1025.log && [ -n "$(find night-1025.log -mmin +45)" ]; then log "no night-1025.done 45 min after the summary"; break; fi
  { low || { [ "$(date +%H%M)" -ge 0400 ] && [ "$(date +%H%M)" -lt 1200 ]; }; } && { log "battery or 04:00"; break; }
  sleep 60
done
log "sleeping"; sleep 30; pmset sleepnow
