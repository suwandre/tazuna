#!/bin/bash
# One run on SubRouter, watched:  sub_run.sh MODEL LABEL PRICE CAP [TASK]
#   PRICE: tazuna's estimate in USD per M (input,cache,output); CAP: tazuna's own stop at that estimate (the real stop is
#   sub_guard.sh, on SubRouter's counter). SUB_KEYENV, SUB_HARNESS (tazuna-ownsubm), SUB_EFFORT (max), SUB_TIMEOUT (10800 s),
#   SUB_NOEDIT (240: stop after that many calls without an edit; 0 for free models, where only time counts), SUB_SITE (for the
#   log line; another site works too: SUB_HARNESS picks its key, SUB_KEYENV only the SubRouter counter in the log).
# The run also stops after 30 min without a reply. Lives in the repo, not /tmp: a restart wiped the /tmp copies mid-run.
MODEL=$1 LABEL=$2 PRICE=$3 CAP=$4 TASK=${5:-rclone-archive-symlinks} KEYENV=${SUB_KEYENV:-SUBROUTER_API_KEY} H=${SUB_HARNESS:-tazuna-ownsubm} EFF=${SUB_EFFORT:-max}
cd "$(dirname "$0")" || exit 1
source ~/.subrouter-key
# the model's vetted route key (subrouter.py vet) when there is one and no harness was chosen: the default smart key once
# picked PixStudio at 5x the price and a6aa at 13-22% cached (2026-10-24); SUB_ROUTE=0 keeps the default key
RK=SUBROUTER_ROUTE_$(echo "$MODEL" | tr '[:lower:]' '[:upper:]' | sed -E 's/[^A-Z0-9]+/_/g; s/^_+|_+$//g')_KEY
if [ -z "$SUB_HARNESS" ] && [ "${SUB_ROUTE:-1}" = 1 ] && [ -n "${!RK}" ]; then H=tazuna-ownsubx; export SUBROUTER_X_KEYENV=$RK; fi
SESSIONS=~/bench-homes/cc/.local/share/tazuna/sessions
used() { curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer ${!KEYENV}" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])"; }
# a process and everything under it: tazuna and its engine carry no label on their command line (0.0.76), so they
# are found through run.py, which does
tree() { local c; for c in $(ps -axo pid=,ppid= | awk -v p="$1" '$2==p{print $1}'); do tree "$c"; done; echo "$1"; }
stop() { echo "$(date +%H:%M) stopped: $1" >> "$LOG"; for p in $(ps -axo pid=,command= | grep -- "-$LABEL@" | grep -v grep | awk '{print $1}'); do kill $(tree "$p") 2>/dev/null; done; }
LOG=mrt-$LABEL.log
echo "$(date +%H:%M) start $MODEL on ${SUB_SITE:-SubRouter} $TASK price $PRICE cap $CAP effort $EFF; used $(used)" > "$LOG"
TAZUNA_PRICE=$PRICE TAZUNA_MAX_USD=$CAP BENCH_GATE=${SUB_GATE-state} GUARD_PAUSE_AT=${GUARD_PAUSE_AT:-101} GUARD_WEEK_AT=${GUARD_WEEK_AT:-101} BENCH_SET=tasks-oct BENCH_TIMEOUT=${SUB_TIMEOUT:-10800} \
  HARNESS_BIN=${HARNESS_BIN:-$HOME/bench-homes/tz-v76/tazuna} ./run.py --runs 1 --workers 1 --tasks "$TASK" --harness "$H-$LABEL@$MODEL:$EFF" > "bench.$LABEL.log" 2>&1 &
RUN=$!
while kill -0 $RUN 2>/dev/null; do
  sleep 60
  f=$(ls -t "$SESSIONS" | grep -- "-$LABEL@" | head -1); [ -z "$f" ] && continue  # no session before the gate opens
  # a pause (kill -STOP on the run and this loop, or a sleeping Mac) is not a hang: idle counts from the end of a gap
  t=$(date +%s); [ $((t - ${TICK:-$t})) -gt 300 ] && WOKE=$t; TICK=$t
  m=$(stat -f %m "$SESSIONS/$f"); idle=$(( t - (m > ${WOKE:-0} ? m : ${WOKE:-0}) ))
  [ $idle -gt 1800 ] && { stop "no reply for $idle s"; break; }
  [ "${SUB_NOEDIT:-240}" -gt 0 ] || continue
  # calls since the last edit: an edit tool, or a change in the work tree (DeepSeek edits through python in bash)
  read total since < <(python3 -I - "$SESSIONS/$f" <<'PY'
import json, sys
n = since = 0
for l in open(sys.argv[1]):
    e = json.loads(l).get("e", {})
    if e.get("type") == "stream_event" and e["event"]["type"] == "message_delta":
        n += 1; since += 1
    elif e.get("type") == "assistant" and any(b.get("type") == "tool_use" and b.get("name") in ("edit", "write", "apply_patch") for b in e["message"].get("content", [])):
        since = 0
print(n, since)
PY
)
  d=$(ls -d "${TMPDIR%/}"/rw-*"-$LABEL@"* 2>/dev/null | head -1)
  h=$([ -n "$d" ] && git -C "$d" diff 2>/dev/null | md5 -q)
  [ "$h" != "$LAST" ] && { LAST=$h; MARK=$total; }
  n=$(( since < total - ${MARK:-0} ? since : total - ${MARK:-0} ))
  [ "${n:-0}" -ge "${SUB_NOEDIT:-240}" ] && { stop "$n calls without an edit"; break; }
done
wait $RUN 2>/dev/null
sleep 60
echo "$(date +%H:%M) end: used $(used); $(grep ' r0:' bench.$LABEL.log | tail -1)" >> "$LOG"
