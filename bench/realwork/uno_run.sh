#!/bin/bash
# One run on UnoRouter over groups that passed uno_route.py, at the live price:
#   uno_run.sh MODEL GROUPS LABEL BUDGET_USD [TASK]      (GROUPS: a,b,c cheapest first, from its GROUPS line)
# UNO_HARNESS=tazuna-ownunorev: the same with --self-review (its runs go in the registry as config uno-2/tazuna-ownunorev)
# tazuna pins the first and moves along the list when a group drops out or stops caching (0.0.68). The list is a file
# (TAZUNA_GROUPS_FILE, 0.0.75): when at most one listed group still serves the model, the groups not tried yet are checked
# in the background and those that pass are added, so a run outlives its first groups (a 1017 run ended at call 43 when
# its only group vanished).
# Refuses to start when no listed group serves the model or a run would cost more than the budget; stops the run when all
# listed groups are gone, a run on the cheapest one left costs over 1.5x the start estimate (input, cache and output: DeepSeek's
# output went from 4x to 120x input for four hours while its input price held), or under 30% is cached after 15 calls (the
# cost cap guards the money; a cheap group at 47% still cost less than CheaperInference).
MODEL=$1 GROUP_LIST=$2 LABEL=$3 BUDGET=$4 TASK=${5:-rclone-archive-symlinks}
cd "$(dirname "$0")" || exit 1
H=${UNO_HARNESS:-tazuna-ownuno}
source ~/.uno-key
LOG=uno-$LABEL.log
GFILE=$PWD/uno-$LABEL.groups TRIED=$PWD/uno-$LABEL.tried
printf '%s\n' ${GROUP_LIST//,/ } > "$GFILE"; cp "$GFILE" "$TRIED"
# the price of the cheapest listed group that still serves the model (the others' prices only matter after a switch)
price() { for g in $(cat "$GFILE"); do python3 -I ../uno_route.py "$MODEL" --price "$g" --budget "$1" && return 0; done; return 1; }
# listed groups that still serve the model
serving() { n=0; for g in $(cat "$GFILE"); do python3 -I ../uno_route.py "$MODEL" --price "$g" --budget 100 >/dev/null && n=$((n+1)); done; echo $n; }
ENV=$(price "$BUDGET") || { echo "$(date +%H:%M) not started: no listed group serves $MODEL within \$$BUDGET ($GROUP_LIST)" | tee "$LOG"; exit 1; }
# a typical run (uno_route.py run_cost) at the TAZUNA_PRICE in, cached, out
cost() { echo "$1" | sed -E 's/.*TAZUNA_PRICE=([^ ]+).*/\1/' | python3 -I -c "i,c,o=map(float,input().split(','));print(6*(0.05*1.25*i+0.95*c)+0.08*o)"; }
START=$(cost "$ENV")
used() { curl -s --max-time 15 https://api.unorouter.com/v1/dashboard/billing/usage -H "Authorization: Bearer $UNOROUTER_API_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])"; }
echo "$(date +%H:%M) start $MODEL on $GROUP_LIST: $ENV; used-cents $(used)" > "$LOG"
env $ENV TAZUNA_GROUPS_FILE="$GFILE" BENCH_GATE=state GUARD_PAUSE_AT=101 GUARD_WEEK_AT=101 BENCH_SET=tasks-oct BENCH_TIMEOUT=10800 \
  HARNESS_BIN=/tmp/tz-v75/tazuna ./run.py --runs 1 --workers 1 --tasks "$TASK" --harness "$H-$LABEL@$MODEL:max" > "bench.$LABEL.log" 2>&1 &
R=$!
# only this run's own process tree (matching by model killed a parallel run of the same model and one of a model whose
# name starts the same)
tree() { for c in $(pgrep -P "$1"); do tree "$c"; done; kill -TERM "$1" 2>/dev/null; }
stop() { echo "$(date +%H:%M) stopping: $1" >> "$LOG"; tree $R; }
sleep 120
while kill -0 $R 2>/dev/null; do
  # at most one listed group left: check the groups not tried yet and add those that pass (one check at a time)
  if [ "$(serving)" -le 1 ] && ! kill -0 "${CHK:-0}" 2>/dev/null; then
    echo "$(date +%H:%M) checking more groups" >> "$LOG"
    ( UNO_SKIP=$(paste -sd, "$TRIED") python3 -I ../uno_route.py "$MODEL" 4 > "uno-$LABEL.recheck.log" 2>&1
      grep "^checking " "uno-$LABEL.recheck.log" | cut -d' ' -f2 >> "$TRIED"
      grep "^GROUPS " "uno-$LABEL.recheck.log" | cut -d' ' -f2 | tr ',' '\n' >> "$GFILE"
      echo "$(date +%H:%M) groups now: $(paste -sd, "$GFILE")" >> "$LOG" ) &
    CHK=$!
  fi
  NOW=$(price 100) || { kill -0 "${CHK:-0}" 2>/dev/null || { stop "all listed groups gone ($(paste -sd, "$GFILE"))"; break; }; sleep 120; continue; }
  NOW_COST=$(cost "$NOW")
  python3 -c "import sys; sys.exit(0 if $NOW_COST > 1.5 * $START else 1)" && { stop "a run now costs \$$NOW_COST, \$$START at the start"; break; }
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
# the registry (uno-2: group switching, 0.0.68+): only runs made this way count on the leaderboard (leaderboard.py)
python3 - "$LOG" "bench.$LABEL.log" "$MODEL" "$GROUP_LIST" "$LABEL" "$TASK" "$H" <<'PY'
import json, re, sys, time
log, bench, model, group, label, task, h = sys.argv[1:]
lines = open(log).read().splitlines()
used = [float(m.group(1)) for l in lines for m in [re.search(r"used-cents ([0-9.]+)", l)] if m]
stop = next((l.split("stopping: ", 1)[1] for l in lines if "stopping: " in l), None)
result = next((l for l in open(bench) if " r0:" in l), "")
rec = {"config": "uno-2" if h == "tazuna-ownuno" else "uno-2/" + h, "t": int(time.time()), "model": model, "group": group, "label": label, "task": task,
       "harness": f"{h}-{label}@{model}:max", "price": lines[0].split(": ", 1)[1].split("; used")[0],
       "billed_cents": round(used[-1] - used[0], 3) if len(used) > 1 else None,
       "status": "stopped: " + stop if stop else ("error" if "ERROR" in result else "finished"), "result": result.strip()[-200:]}
open("uno-runs.jsonl", "a").write(json.dumps(rec) + "\n")
PY
