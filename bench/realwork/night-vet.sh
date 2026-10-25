#!/bin/bash
# 2026-10-25 night: keep the Mac awake until `subrouter.py vet deepseek-v4.1-flash-1017` has checked every provider and
# limited the route key, commit the results (no secrets in them), then sleep the Mac. Stops the vetting early at 15%
# battery off power or at 07:00.
cd "$(dirname "$0")"
log() { echo "$(date +%H:%M) $*" >> night-vet.log; }
caffeinate -i -s -w $$ &
log "caffeinate on; waiting for the vetting run"
tree() { local c; for c in $(ps -axo pid=,ppid= | awk -v p="$1" '$2==p{print $1}'); do tree "$c"; done; echo "$1"; }
deadline() { b=$(pmset -g batt | grep -oE '[0-9]+%' | tr -d %); { [ -n "$b" ] && [ "$b" -le 15 ] && ! pmset -g batt | grep -q "AC Power"; } || { [ "$(date +%H%M)" -ge 0700 ] && [ "$(date +%H%M)" -lt 1200 ]; }; }
while pgrep -f "subrouter.py vet" >/dev/null; do
  deadline && { log "battery or 07:00: stopping the vetting"; for p in $(pgrep -f "subrouter.py vet"); do kill $(tree $p) 2>/dev/null; done; break; }
  sleep 60
done
log "vet log: $(tr '\n' '|' < vet-1017.log | cut -c1-1500)"
cd ../.. && git add bench/subrouter-vetted.json bench/realwork/night-vet.sh \
  && git commit -q -m "bench: first vetting of deepseek-v4.1-flash-1017's SubRouter providers (overnight)" && log "committed $(git log --oneline -1 | cut -c1-60)"
cd bench/realwork; sleep 60; log "sleeping"; pmset sleepnow
