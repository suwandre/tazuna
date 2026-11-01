#!/bin/bash
# The judge for mx*.sh (2026-11-01): one pool per task and effort (high, xhigh, max: eight runs each, every model at that
# effort; Grok left out, its diffs are empty), Opus xhigh, two passes. TASKS: which tasks to judge now (telegraf first,
# gitea once its last run is in). Under the usage guard (judge.py stops at its PAUSE).
cd "$(dirname "$0")"; source ~/.bench-token
export BENCH_SET=tasks-oct
for t in ${TASKS:-telegraf-snmp-reconnect}; do
  for e in high xhigh max; do
    ONLY=$(python3 -I - "$t" "$e" <<'PY'
import json, re, sys
t, e = sys.argv[1:]
hs = set()
for r in map(json.loads, open("results-tasks-oct.jsonl")):
    m = re.search(r"-v(\d+)@(.*):(high|xhigh|max)$", r["harness"])
    if m and r["task"] == t and m.group(3) == e and "grok" not in m.group(2):
        v = int(m.group(1))
        if 823 <= v <= 852 or 901 <= v <= 996:
            hs.add(r["harness"])
print(",".join(sorted(hs)))
PY
)
    echo "$(date +%H:%M) $t $e: $(echo "$ONLY" | tr ',' '\n' | wc -l | tr -d ' ') runs" >> mxj.log
    JUDGE_TASKS=$t JUDGE_OUT=judge.mx.$t.$e.jsonl JUDGE_ONLY="$ONLY" python3 -I ./judge.py --passes 2 >> judge-mx.log 2>&1
    echo "$(date +%H:%M) judged $t $e" >> mxj.log
  done
done
echo "$(date +%H:%M) MXJ-DONE $TASKS" >> mxj.log
