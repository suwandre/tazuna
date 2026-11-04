#!/bin/bash
# Kiro credits including runs still going: the credits Kiro writes per turn into its session files (messages.jsonl)
# since START (epoch seconds). Stops every Kiro benchmark run once they pass LIMIT. kiro.sh's own counter read only
# finished runs and showed 10 credits while 160 were spent (2026-11-04).
START=$1 LIMIT=$2
tree() { local c; for c in $(ps -axo pid=,ppid= | awk -v p="$1" '$2==p{print $1}'); do tree "$c"; done; echo "$1"; }
used() { /opt/homebrew/bin/python3 -I -c "
import pathlib,re
print(round(sum(float(x) for f in (pathlib.Path.home()/'.kiro/sessions').rglob('messages.jsonl') if f.stat().st_mtime>=$START
    for x in re.findall(r'\"unit\":\"credit\",\"unitPlural\":\"credits\",\"usage\":([0-9.]+)', f.read_text(errors='replace'))),2))"; }
while pgrep -f 'harness kiro-v14' > /dev/null; do
  u=$(used); echo "$(date +%H:%M) kiro credits since start $u" >> kiro_guard.log
  if python3 -c "import sys; sys.exit(0 if $u > $LIMIT else 1)"; then
    echo "$(date +%H:%M) over $LIMIT: stopping" >> kiro_guard.log
    for p in $(pgrep -f 'harness kiro-v14'); do kill $(tree $p) 2>/dev/null; done; break
  fi
  sleep 60
done
