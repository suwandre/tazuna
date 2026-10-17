#!/bin/bash
# One rclone-set run on UnoRouter pinned to a group that passed uno_route.py, at the group's live price:
#   uno_run.sh MODEL GROUP LABEL BUDGET_USD [TASK]
# Refuses to start when the group is gone or a run would cost more than the budget; stops the run when the group's
# input price rises over 1.5x the start price, the group disappears, or under 50% is cached after 15 calls.
MODEL=$1 GROUP=$2 LABEL=$3 BUDGET=$4 TASK=${5:-rclone-archive-symlinks}
cd "$(dirname "$0")" || exit 1
source ~/.uno-key
ENV=$(python3 -I ../uno_route.py "$MODEL" --price "$GROUP" --budget "$BUDGET") || { echo "not started: $ENV"; exit 1; }
START_IN=$(echo "$ENV" | sed -E 's/.*TAZUNA_PRICE=([0-9.]+),.*/\1/')
LOG=uno-$LABEL.log
used() { curl -s --max-time 15 https://api.unorouter.com/v1/dashboard/billing/usage -H "Authorization: Bearer $UNOROUTER_API_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])"; }
echo "$(date +%H:%M) start $MODEL on $GROUP: $ENV; used-cents $(used)" > "$LOG"
env $ENV TAZUNA_HEADER="X-Group: $GROUP" BENCH_GATE=state GUARD_PAUSE_AT=101 GUARD_WEEK_AT=101 BENCH_SET=tasks-oct BENCH_TIMEOUT=10800 \
  HARNESS_BIN=/tmp/tz-v65/tazuna ./run.py --runs 1 --workers 1 --tasks "$TASK" --harness "tazuna-ownuno-$LABEL@$MODEL:max" > "bench.$LABEL.log" 2>&1 &
R=$!
stop() { echo "$(date +%H:%M) stopping: $1" >> "$LOG"; kill $R; for g in $(ps -axo pgid,command | grep "tazuna --run" | grep -v grep | awk '{print $1}' | sort -u); do ps -o command= -g $g | grep -q -- "--model $MODEL" && kill -TERM -$g; done; }
sleep 120
while kill -0 $R 2>/dev/null; do
  NOW=$(python3 -I ../uno_route.py "$MODEL" --price "$GROUP" --budget 100) || { stop "group gone ($NOW)"; break; }
  NOW_IN=$(echo "$NOW" | sed -E 's/.*TAZUNA_PRICE=([0-9.]+),.*/\1/')
  python3 -c "import sys; sys.exit(0 if $NOW_IN > 1.5 * $START_IN else 1)" && { stop "input price rose from \$$START_IN to \$$NOW_IN per M"; break; }
  f=$(ls -t ~/bench-homes/cc/.local/share/tazuna/sessions/ | grep "$LABEL@$MODEL" | head -1)
  if [ -n "$f" ]; then
    set -- $(python3 - "$HOME/bench-homes/cc/.local/share/tazuna/sessions/$f" <<'PY'
import json,sys
n=i=c=0
for l in open(sys.argv[1]):
    e=json.loads(l).get("e",{})
    if e.get("type")=="stream_event" and e["event"]["type"]=="message_delta":
        u=e["event"]["usage"]; n+=1; i+=u["input_tokens"]; c+=u["cache_read_input_tokens"]
print(n, f"{c/max(i+c,1):.2f}")
PY
)
    [ "${1:-0}" -ge 15 ] && python3 -c "import sys;sys.exit(0 if float('$2')<0.5 else 1)" && { stop "cache $2 after $1 calls"; break; }
  fi
  sleep 120
done
wait
sleep 60
echo "$(date +%H:%M) end: used-cents $(used)" >> "$LOG"
# the uno-1 registry: only runs made this way count on the leaderboard (leaderboard.py)
python3 - "$LOG" "bench.$LABEL.log" "$MODEL" "$GROUP" "$LABEL" "$TASK" <<'PY'
import json, re, sys, time
log, bench, model, group, label, task = sys.argv[1:]
lines = open(log).read().splitlines()
used = [float(m.group(1)) for l in lines for m in [re.search(r"used-cents ([0-9.]+)", l)] if m]
stop = next((l.split("stopping: ", 1)[1] for l in lines if "stopping: " in l), None)
result = next((l for l in open(bench) if " r0:" in l), "")
rec = {"config": "uno-1", "t": int(time.time()), "model": model, "group": group, "label": label, "task": task,
       "harness": f"tazuna-ownuno-{label}@{model}:max", "price": lines[0].split(": ", 1)[1].split("; used")[0],
       "billed_cents": round(used[-1] - used[0], 3) if len(used) > 1 else None,
       "status": "stopped: " + stop if stop else ("error" if "ERROR" in result else "finished"), "result": result.strip()[-200:]}
open("uno-runs.jsonl", "a").write(json.dumps(rec) + "\n")
PY
