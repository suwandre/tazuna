#!/bin/bash
# One run on UnoRouter over groups that passed uno_route.py, at the live price:
#   uno_run.sh MODEL GROUPS LABEL BUDGET_USD [TASK]      (GROUPS: a,b,c cheapest first, from its GROUPS line)
# tazuna pins the first and moves along the list when a group drops out or stops caching (TAZUNA_GROUPS, 0.0.68).
# Refuses to start when no listed group serves the model or a run would cost more than the budget; stops the run when all
# listed groups are gone, the cheapest one left costs over 1.5x the start price, or under 30% is cached after 15 calls (the
# cost cap guards the money; a cheap group at 47% still cost less than CheaperInference).
MODEL=$1 GROUP_LIST=$2 LABEL=$3 BUDGET=$4 TASK=${5:-rclone-archive-symlinks}
cd "$(dirname "$0")" || exit 1
source ~/.uno-key
LOG=uno-$LABEL.log
# the price of the cheapest listed group that still serves the model (the others' prices only matter after a switch)
price() { for g in ${GROUP_LIST//,/ }; do python3 -I ../uno_route.py "$MODEL" --price "$g" --budget "$1" && return 0; done; return 1; }
ENV=$(price "$BUDGET") || { echo "$(date +%H:%M) not started: no listed group serves $MODEL within \$$BUDGET ($GROUP_LIST)" | tee "$LOG"; exit 1; }
START_IN=$(echo "$ENV" | sed -E 's/.*TAZUNA_PRICE=([0-9.]+),.*/\1/')
used() { curl -s --max-time 15 https://api.unorouter.com/v1/dashboard/billing/usage -H "Authorization: Bearer $UNOROUTER_API_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])"; }
echo "$(date +%H:%M) start $MODEL on $GROUP_LIST: $ENV; used-cents $(used)" > "$LOG"
env $ENV TAZUNA_GROUPS="$GROUP_LIST" BENCH_GATE=state GUARD_PAUSE_AT=101 GUARD_WEEK_AT=101 BENCH_SET=tasks-oct BENCH_TIMEOUT=10800 \
  HARNESS_BIN=/tmp/tz-v68/tazuna ./run.py --runs 1 --workers 1 --tasks "$TASK" --harness "tazuna-ownuno-$LABEL@$MODEL:max" > "bench.$LABEL.log" 2>&1 &
R=$!
# only this run's own process tree (matching by model killed a parallel run of the same model and one of a model whose
# name starts the same)
tree() { for c in $(pgrep -P "$1"); do tree "$c"; done; kill -TERM "$1" 2>/dev/null; }
stop() { echo "$(date +%H:%M) stopping: $1" >> "$LOG"; tree $R; }
sleep 120
while kill -0 $R 2>/dev/null; do
  NOW=$(price 100) || { stop "all listed groups gone ($GROUP_LIST)"; break; }
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
    [ "${1:-0}" -ge 15 ] && python3 -c "import sys;sys.exit(0 if float('$2')<0.3 else 1)" && { stop "cache $2 after $1 calls"; break; }
  fi
  sleep 120
done
wait
sleep 60
echo "$(date +%H:%M) end: used-cents $(used)" >> "$LOG"
# the uno-1 registry: only runs made this way count on the leaderboard (leaderboard.py)
python3 - "$LOG" "bench.$LABEL.log" "$MODEL" "$GROUP_LIST" "$LABEL" "$TASK" <<'PY'
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
