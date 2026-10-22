#!/bin/bash
# Stops SubRouter runs once the account has spent LIMIT fen (about 700 per USD) since START:  sub_guard.sh START LIMIT LABEL...
# The counter is account-wide: runs started together share one guard. GUARD_EVERY: seconds between reads (60).
START=$1 LIMIT=$2; shift 2
RE="-($(IFS='|'; echo "$*"))@"
source ~/.subrouter-key
used() { curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])"; }
pids() { ps -axo pid=,command= | grep -E -- "$RE" | grep -v grep | awk '{print $1}'; }
sleep 30
while [ -n "$(pids)" ]; do
  u=$(used)
  if [ -n "$u" ]; then
    s=$(python3 -c "print(round($u - $START, 2))")
    echo "$(date +%H:%M) $* spent $s fen (\$$(python3 -c "print(round($s / 700, 3))"))"
    python3 -c "import sys; sys.exit(0 if $s > $LIMIT else 1)" && { echo "$(date +%H:%M) LIMIT $LIMIT: stopping $*"; pids | xargs kill 2>/dev/null; break; }
  fi
  sleep ${GUARD_EVERY:-60}
done
echo "$(date +%H:%M) DONE $*"
