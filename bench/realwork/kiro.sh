#!/bin/bash
# 2026-11-04: Kiro CLI (stock settings, no web tools) on the six tasks/ Go tasks x two runs: Claude Opus 5.5 medium (the
# same model as tazuna's default) and Claude Haiku 5.5 medium, on the user's Kiro Pro credits (1,418). Probe: Haiku on
# caddy-quoted-braces 0.36 credits. Stops every Kiro run once the credits of these runs pass 800.
cd "$(dirname "$0")"; source ~/.bench-token; export PATH="/opt/homebrew/bin:$PATH"
log() { echo "$(date +%H:%M) $*" >> kiro.log; }
caffeinate -ims -w $$ &
tree() { local c; for c in $(ps -axo pid=,ppid= | awk -v p="$1" '$2==p{print $1}'); do tree "$c"; done; echo "$1"; }
spent() { python3 -I -c "
import json
print(round(sum(json.loads(l).get('credits') or 0 for l in open('results.jsonl') if '\"kiro-v14' in l), 2))"; }
log "start ($(spent) credits so far)"
./run.py --runs 2 --workers 4 --harness "kiro-v1418@claude-opus-5.5:medium,kiro-v1419@claude-haiku-5.5:medium" >> kiro.out 2>&1 &
main=$!
while kill -0 $main 2>/dev/null; do
  s=$(spent)
  if python3 -c "import sys; sys.exit(0 if $s > 800 else 1)"; then log "credits $s > 800: stopping"; kill $(tree $main) 2>/dev/null; break; fi
  sleep 60
done
wait $main 2>/dev/null
log "KIRO-DONE: $(spent) credits"
