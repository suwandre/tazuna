#!/bin/bash
# Route check, then one watched SubRouter run with its own spend guard:  sub_one.sh MODEL LABEL PRICE [LIMIT_FEN] [TASK]
#   SUB_KEYENV (SUBROUTER_MODELS_KEY), SUB_EFFORT (max) and the rest as for sub_run.sh; LIMIT_FEN 525 (~$0.75).
#   Logs to sub-one.log; the check to check-<LABEL>.log.
MODEL=$1 LABEL=$2 PRICE=$3 LIMIT=${4:-525} TASK=${5:-rclone-archive-symlinks}
cd "$(dirname "$0")"; source ~/.subrouter-key
export SUB_KEYENV=${SUB_KEYENV:-SUBROUTER_MODELS_KEY}
log() { echo "$(date +%H:%M) $*" >> sub-one.log; }
used() { curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])"; }
( cd .. && ROUTE_API=https://subrouter.ai ROUTE_KEY_ENV=$SUB_KEYENV python3 -I -u uno_route.py "$MODEL" 1 > "realwork/check-$LABEL.log" 2>&1 )
grep -q "identity ok" "check-$LABEL.log" || { log "$LABEL $MODEL check failed: $(grep -E 'no:|Error' "check-$LABEL.log" | head -1)"; exit 1; }
U=$(used); log "$LABEL $MODEL ${SUB_EFFORT:-max} on $SUB_KEYENV: $(grep -oE 'cache [0-9]+%|long call [0-9]+ s' "check-$LABEL.log" | tr '\n' ' ')started (used $U)"
./sub_run.sh "$MODEL" "$LABEL" "$PRICE" 0.5 "$TASK" &
./sub_guard.sh "$U" "$LIMIT" "$LABEL" >> sub-guard-one.log 2>&1
wait; log "$LABEL done: $(grep -h 'end:\|stopped' "mrt-$LABEL.log" | grep -oE 'r0: [0-9]/3|stopped.*' | head -1)"
