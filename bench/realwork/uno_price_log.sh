#!/bin/bash
# UnoRouter prices every 10 minutes for 48 hours (public /api/pricing, no key; uno_route.py puts groups listed for 3 hours first):
# per model the input price per group, plus the output and cache-read multipliers (DeepSeek output went from 4x to 120x)
OUT="$(dirname "$0")/uno-prices.jsonl"
for i in $(seq 1 288); do
  curl -s --max-time 30 https://api.unorouter.com/api/pricing | python3 -I -c "
import json,sys,time
d=json.load(sys.stdin); gr=d['group_ratio']
models=('gpt-6-luna','gpt-6.1-sol','deepseek-v4.1-flash','mimo-v2.6-flash','claude-haiku-5-5','glm-5.3','grok-4.7')
row={'t': int(time.time())}
for m in d['data']:
    if m['model_name'] in models:
        row[m['model_name']] = {'out_x': m['completion_ratio'], 'cache_x': m.get('cache_ratio'),
                                'in': {g: round(m['model_ratio']*2*gr[g], 5) for g in m['enable_groups'] if g in gr}}
print(json.dumps(row))" >> "$OUT" 2>/dev/null
  sleep 600
done
