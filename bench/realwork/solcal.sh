#!/bin/bash
# 2026-11-05: GPT-6.1 Sol (SubRouter route, xhigh) as a stand-in for the Opus judge: the same candidate pools Opus judged
# (dsk, kiro, sl, bts: tasks/, one call per task each, same shuffles), compared afterwards per candidate and per arm.
# Then the 13 SubRouter provider caddy runs of 2026-11-04 plus the four official-API caddy runs, judged by Sol only.
# Costs SubRouter money only (~$0.10-0.20), no Claude usage.
cd "$(dirname "$0")"; source ~/.bench-token; source ~/.subrouter-key; export PATH="/opt/homebrew/bin:$PATH"
export JUDGE_KEY_ENV=SUBROUTER_ROUTE_GPT_6_1_SOL_KEY
SOL=gpt-6.1-sol@https://subrouter.ai/v1
for f in judge.kiro.tasks.jsonl judge.sl.tasks.jsonl judge.bts.tasks.jsonl; do
  only=$(python3 -c "import json;print(','.join(sorted({r['harness']+'-r'+str(r['run']) for r in map(json.loads,open('$f'))})))")
  tasks=$(python3 -c "import json;print(','.join(sorted({r['task'] for r in map(json.loads,open('$f'))})))")
  JUDGE_ONLY=$only JUDGE_TASKS=$tasks JUDGE_OUT=${f%.jsonl}.sol.jsonl python3 -u judge.py --model $SOL --effort xhigh 2>&1 | grep --line-buffered -v '^[A-Z][A-Z]' 
  echo "$(date +%H:%M) $f done"
done
only=$(python3 -c "
import json,re
s=set()
for r in map(json.loads,open('results.jsonl')):
    if r['task']=='caddy-quoted-braces' and (re.search(r'ownsubx-v16\d\d@',r['harness']) or 'v1541@' in r['harness']): s.add(r['harness']+'-r'+str(r['run']))
print(','.join(sorted(s)))")
JUDGE_ONLY=$only JUDGE_TASKS=caddy-quoted-braces JUDGE_OUT=judge.prov.sol.jsonl python3 -u judge.py --model $SOL --effort xhigh 2>&1 | grep --line-buffered -v '^[A-Z][A-Z]'
echo "$(date +%H:%M) SOLCAL-DONE"
