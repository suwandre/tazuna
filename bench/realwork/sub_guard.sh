#!/bin/bash
# Stops runs once the account has spent LIMIT since START:  sub_guard.sh START LIMIT LABEL...
# SubRouter (default): its usage counter, in fen (about 700 per USD). GUARD_SITE=mortera: minus Mortera's balance, in IDR
# (16300 per USD; START is then minus the balance at the start). The counter is account-wide: runs started together share
# one guard. GUARD_EVERY: seconds between reads (60).
START=$1 LIMIT=$2; shift 2
# a failed counter read at launch once left START empty: LIMIT took the first label and the guard never stopped anything
python3 -c "float('$START'); float('$LIMIT')" 2>/dev/null || { echo "$(date +%H:%M) bad START/LIMIT '$START' '$LIMIT': not guarding $*"; exit 2; }
RE="-($(IFS='|'; echo "$*"))@"
if [ "$GUARD_SITE" = mortera ]; then
  source ~/.mortera-key; UNIT=IDR PER_USD=16300
  used() { curl -s -A curl/8 --max-time 15 https://mortera.cloud/v1/credits -H "Authorization: Bearer $MORTERA_API_KEY" | python3 -I -c "import json,sys;print(-json.load(sys.stdin)['data']['balance_idr'])"; }
else
  source ~/.subrouter-key; UNIT=fen PER_USD=700
  used() { curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])"; }
fi
pids() { ps -axo pid=,command= | grep -E -- "$RE" | grep -v grep | awk '{print $1}'; }
sleep 30
while [ -n "$(pids)" ]; do
  u=$(used)
  if [ -n "$u" ]; then
    s=$(python3 -c "print(round($u - $START, 2))")
    echo "$(date +%H:%M) $* spent $s $UNIT (\$$(python3 -c "print(round($s / $PER_USD, 3))"))"
    python3 -c "import sys; sys.exit(0 if $s > $LIMIT else 1)" && { echo "$(date +%H:%M) LIMIT $LIMIT: stopping $*"; pids | xargs kill 2>/dev/null; break; }
  fi
  sleep ${GUARD_EVERY:-60}
done
echo "$(date +%H:%M) DONE $*"
