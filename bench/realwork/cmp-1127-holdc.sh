#!/bin/bash
# 2026-11-27: the running round's "80k" arm ran without its threshold (run.py's only_keys dropped TAZUNA_COMPACT_AT_TOKENS,
# a name with TOKEN in it): its agents are stopped as they start, while this run.py (pid in $1) finishes the baseline
# arm's jobs; the fixed round reruns the 80k arm.
cd "$(dirname "$0")" || exit 1
while kill -0 "$1" 2>/dev/null; do
  for p in $(pgrep -f "tz-v1235/tazuna --run"); do
    # only the old round's (its run.py is the parent): the fixed round's 80k agents run
    if [ "$(ps -o ppid= -p "$p" | tr -d ' ')" = "$1" ] && lsof -a -p "$p" -d cwd -Fn 2>/dev/null | grep -q ownmisvcmp; then
      pkill -P "$p"; kill "$p" 2>/dev/null; echo "$(date +%H:%M:%S) stopped $p (80k arm)" >> cmp-1127-holdc.log
    fi
  done
  sleep 5
done
echo "$(date +%H:%M:%S) run.py gone" >> cmp-1127-holdc.log
