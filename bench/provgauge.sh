#!/bin/bash
# The cheap models' SubRouter providers through the day (user, 2026-11-05): which stay fast, cached and honest whether
# China is busy (10:00 here = 17:00 there) or asleep (22:00 here = 05:00 there)? Caching fell from ~90% at night to
# 0-33% one morning, and a provider sold another model as Sol. provgauge_loop.sh runs this every 2 hours:
#   - DeepSeek V4.1 Flash 1017 on every provider listed at up to ~2x the floor price (provgauge_slugs.py; past that a
#     subscription is cheaper): provsmoke.py's probe (first token, speed, cache, errors) plus PROVSMOKE_DEEP's checks
#     (tokens a line: the same model or not; one hard AIME problem: thinking depth and right or wrong), effort high
#     (DeepSeek's real level) -> provgauge.jsonl
#   - Sol's and Luna's route candidates, probe only, Responses route, xhigh -> provgauge-sol.jsonl, provgauge-luna.jsonl
#   - in the 10:00-11:59, 16:00-17:59 and 22:00-23:59 windows (China busy, late evening, asleep), once a day each: canary.py (30 AIME + 100 MMLU-Pro, high) on the
#     DeepSeek shortlist -> .model-check/canary.jsonl
# Ends after 2026-11-08 12:00 or past $3.00 spent since 2026-11-05 20:00 (the account counter: other SubRouter use
# counts too). The first version (11:16-19:25) probed every 4 hours with the canary every 12 hours on all providers.
cd "$(dirname "$0")" || exit 1
source ~/.subrouter-key
export PATH="/opt/homebrew/bin:$PATH"
M=deepseek-v4.1-flash-1017
# Canary shortlist from 2026-11-06 18:40: the route (a6aa x-loop bangai lingai), o2oai, stableai, and the two that
# routes.py tiers picks next and no canary has measured in China's busy hours (kriora: 503 on all of that morning's
# questions; ydjs-ai: never). Dropped: redqueen (hard problem wrong 3 of 4, gone from the list), model-share (503 all
# morning), znu (another model by day: 18/30 AIME, 36.5 tokens a line). Before: "bangai a6aa o2oai x-loop lingai redqueen
# stableai", and "znu model-share bangai a6aa o2oai kriora" until 12:09.
SHORT="bangai a6aa o2oai x-loop lingai kriora ydjs-ai stableai"
log() { echo "$(date '+%m-%d %H:%M') $*" >> provgauge.log; }
used() { curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])"; }
keyvar() { python3 -I -c "import re,sys;print('SUBROUTER_SMOKE_'+re.sub(r'[^A-Z0-9]+','_',(sys.argv[1]+'_'+sys.argv[2]).upper()).strip('_')+'_KEY')" "$1" "$2"; }

if [ "$(date +%s)" -gt "$(date -j -f '%Y-%m-%d %H:%M' '2026-11-08 12:00' +%s)" ]; then
  log "past the end date"
  exit 0
fi
U=$(used) || exit 1
# an empty or broken answer read as 0 (18:09 on 2026-11-06 logged "$-38.34"), which would also skip the spend cap
case $U in ''|*[!0-9.]*) log "usage unreadable: round skipped"; exit 1 ;; esac
if python3 -c "import sys; sys.exit(0 if $U - $(cat provgauge.start2) > 2100 else 1)"; then
  log "spend cap \$3.00 reached: no more rounds"
  exit 0
fi

SLUGS=$(python3 -I provgauge_slugs.py "$M")
PROVSMOKE_DEEP=1 PROVSMOKE_EFFORT=high python3 -I provsmoke.py "$M" $SLUGS >> provgauge.jsonl 2>> provgauge.err
source ~/.subrouter-key  # provsmoke.py adds keys for providers new to the list
log "probe and checks: $(echo $SLUGS | wc -w | tr -d ' ') DeepSeek providers"
PROVSMOKE_API=responses PROVSMOKE_EFFORT=xhigh python3 -I provsmoke.py gpt-6.1-sol xcgx shared-openai-pro-50 shared-openai-pro-52 kriora >> provgauge-sol.jsonl 2>> provgauge.err
PROVSMOKE_API=responses PROVSMOKE_EFFORT=xhigh python3 -I provsmoke.py gpt-6-luna kriora shared-openai-pro-52 shared-openai-plus-51 >> provgauge-luna.jsonl 2>> provgauge.err
source ~/.subrouter-key
log "probe: Sol 4, Luna 3 providers"

case $(date +%H) in 10|11) W=busy ;; 16|17) W=evening ;; 22|23) W=asleep ;; *) W= ;; esac  # 17:00, 23:00, 05:00 in China
if [ -n "$W" ] && ! grep -q "^$(date +%F) $W$" provgauge.windows 2>/dev/null; then
  echo "$(date +%F) $W" >> provgauge.windows
  for g in $SHORT; do keyvar "$M" "$g"; done | xargs -P 3 -I{} python3 -I canary.py run "$M" --key-env {} --effort high >> provgauge.canary.out 2>&1
  log "canary ($W in China): $SHORT"
fi
U=$(used)
case $U in
  ''|*[!0-9.]*) log "spent: usage unreadable" ;;
  *) log "spent since 2026-11-05 20:00: \$$(python3 -c "print(round(($U - $(cat provgauge.start2)) / 700, 3))")" ;;
esac
