#!/bin/bash
# 2026-11-04 night: one caddy-quoted-braces run per SubRouter provider of DeepSeek V4.1 Flash 1017 that answered all four
# calls of ../provsmoke.py (provsmoke-1104.jsonl), xhigh, tazuna 0.1.54 frozen (tz-v154s, same as the official-API runs),
# a key per provider (tazuna-route-smoke-<slug>), seven at a time. Version tags v1601-v1614 name the providers (below).
# SubRouter guard: stop past 400 fen (~$0.57) from the counter at the start.
cd "$(dirname "$0")"; source ~/.bench-token; source ~/.subrouter-key; export PATH="/opt/homebrew/bin:$PATH"
export HARNESS_BIN=$HOME/bench-homes/tz-v154s/tazuna BENCH_TIMEOUT=2400 TAZUNA_MAX_USD=0.10
caffeinate -ims -w $$ &
P="1601 kriora 0.0109 0.0436
1602 a6aa 0.0120 0.0479
1603 stableai 0.0130 0.0518
1604 znu 0.0118 0.0471
1605 o2oai 0.0159 0.0635
1606 x-loop 0.0191 0.0765
1607 bangai 0.0221 0.0882
1608 ydjs-ai 0.0176 0.0706
1609 getapi 0.0265 0.1059
1610 lingai 0.0221 0.0882
1611 model-share 0.0221 0.0882
1612 relai 0.0118 0.0471
1613 tokenus-net 0.0199 0.0796
1614 tokenx 0.0265 0.1059"
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
echo "$(date +%H:%M) start counter $U" >> provsmoke.log
one() {
  set -- $1
  v=$1 slug=$2; var=SUBROUTER_SMOKE_$(echo "$slug" | tr 'a-z-' 'A-Z_')_KEY
  SUBROUTER_X_KEYENV=$var TAZUNA_PRICE=$3,$(python3 -c "print(round($3/10,5))"),$4 ./run.py --runs 1 --workers 1 --tasks caddy-quoted-braces \
    --harness "tazuna-ownsubx-v$v@deepseek-v4.1-flash-1017:xhigh" > provsmoke.$slug.out 2>&1
  echo "$(date +%H:%M) $slug v$v: $(grep -E 'v'$v'@.*r0:' provsmoke.$slug.out | cut -c40-200)" >> provsmoke.log
}
export -f one
sleep 1
./sub_guard.sh "$U" 400 $(echo "$P" | awk '{print "ownsubx-v"$1}') >> provsmoke.guard.log 2>&1 &
echo "$P" | xargs -P 7 -I{} bash -c 'one "{}"'
echo "$(date +%H:%M) PROVSMOKE-DONE" >> provsmoke.log
