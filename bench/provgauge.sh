#!/bin/bash
# DeepSeek V4.1 Flash 1017 providers on SubRouter over the day (and Sol's and Luna's route candidates, probe only) (user, 2026-11-05: caching fell from ~90% at night to
# 0-33% in the morning; no provider should stay picked that serves poor quality). launchd (com.tazuna.provgauge) runs
# this every 4 hours while the Mac is awake: provsmoke.py on every listed provider up to $0.03/M input (speed, first
# token, cache, reasoning, errors) into provgauge.jsonl; every 12 hours also canary.py (30 AIME + 100 MMLU-Pro, high: the level "xhigh" ran as; the first round, 11:16 Nov 5, sent xhigh)
# per provider that answered, into .model-check/canary.jsonl (route = the provider's key). Ends itself after
# 2026-11-08 12:00 or past $2.50 spent since the first round (the account counter: other SubRouter use counts too).
cd "$(dirname "$0")" || exit 1
source ~/.subrouter-key
export PATH="/opt/homebrew/bin:$PATH"
M=deepseek-v4.1-flash-1017
log() { echo "$(date '+%m-%d %H:%M') $*" >> provgauge.log; }
used() { curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])"; }

if [ "$(date +%s)" -gt "$(date -j -f '%Y-%m-%d %H:%M' '2026-11-08 12:00' +%s)" ]; then
  log "past the end date; unload: launchctl unload ~/Library/LaunchAgents/com.tazuna.provgauge.plist"
  exit 0
fi
U=$(used) || exit 1
[ -f provgauge.start ] || echo "$U" > provgauge.start
if python3 -c "import sys; sys.exit(0 if $U - $(cat provgauge.start) > 1750 else 1)"; then
  log "spend cap \$2.50 reached: no more rounds"
  exit 0
fi

SLUGS=$(python3 -I provgauge_slugs.py "$M")
N=$(echo "$SLUGS" | wc -w | tr -d ' ')
python3 -I provsmoke.py "$M" $SLUGS >> provgauge.jsonl 2>> provgauge.err
source ~/.subrouter-key  # provsmoke.py adds keys for providers new to the list
log "probe: $N providers"
# Sol and Luna (user, 2026-11-05): their route candidates, probe only, on the Responses route tazuna uses for GPT, xhigh
PROVSMOKE_API=responses PROVSMOKE_EFFORT=xhigh python3 -I provsmoke.py gpt-6.1-sol xcgx shared-openai-pro-50 shared-openai-pro-52 kriora >> provgauge-sol.jsonl 2>> provgauge.err
PROVSMOKE_API=responses PROVSMOKE_EFFORT=xhigh python3 -I provsmoke.py gpt-6-luna kriora shared-openai-pro-52 shared-openai-plus-51 >> provgauge-luna.jsonl 2>> provgauge.err
source ~/.subrouter-key
log "probe: Sol 4, Luna 3 providers"

last=$(cat provgauge.canary 2>/dev/null || echo 0)
if [ $(( $(date +%s) - last )) -ge 43000 ]; then
  date +%s > provgauge.canary
  OK=$(tail -n "$N" provgauge.jsonl | python3 -I -c "import json,sys;print(' '.join(d['slug'] for d in map(json.loads,sys.stdin) if d.get('ok')==4))")
  for g in $OK; do python3 -I -c "import re,sys;print('SUBROUTER_SMOKE_'+re.sub(r'[^A-Z0-9]+','_',(sys.argv[1]+'_'+sys.argv[2]).upper()).strip('_')+'_KEY')" "$M" "$g"; done |
    xargs -P 4 -I{} python3 -I canary.py run "$M" --key-env {} --effort high >> provgauge.canary.out 2>&1
  log "canary: $OK"
fi
log "spent since the first round: \$$(python3 -c "print(round(($(used) - $(cat provgauge.start)) / 700, 3))")"
