#!/bin/bash
# 2026-11-04 evening (off-peak): why the DeepSeek Harness is twice as fast as tazuna's own loop. Same model and provider
# as the Harness: DeepSeek's official API, deepseek-flash (V4.1 Flash 1017) at xhigh, caddy-quoted-braces x2, reasoning
# sent back (owndskr, TAZUNA_SEND_REASONING) or not (owndsk). tazuna 0.1.54 + search-prompt fix frozen in tz-v154s.
# Balance guard: stop everything past CNY 3.00 spent (granted credit, CNY 4.15 at start).
cd "$(dirname "$0")"; source ~/.bench-token; source ~/.deepseek-key; export PATH="/opt/homebrew/bin:$PATH"
export TAZUNA_PRICE=0.15,0.003,0.60 TAZUNA_MAX_USD=0.15 HARNESS_BIN=$HOME/bench-homes/tz-v154s/tazuna BENCH_TIMEOUT=2400
caffeinate -ims -w $$ &
bal() { curl -s https://api.deepseek.com/user/balance -H "Authorization: Bearer $DEEPSEEK_API_KEY" | python3 -c 'import sys,json; print(next(b["total_balance"] for b in json.load(sys.stdin)["balance_infos"] if b["currency"]=="CNY"))'; }
start=$(bal); echo "$(date +%H:%M) start balance $start" >> dsk.log
./run.py --runs 2 --workers 4 --tasks caddy-quoted-braces --harness "tazuna-owndsk-v1541@deepseek-flash:xhigh,tazuna-owndskr-v1541@deepseek-flash:xhigh" >> dsk.out 2>&1 &
runpid=$!
while kill -0 $runpid 2>/dev/null; do
  sleep 30; now=$(bal)
  if python3 -c "import sys; sys.exit(0 if $start-$now>3.0 else 1)"; then echo "$(date +%H:%M) GUARD stop at $now" >> dsk.log; pkill -f 'owndsk'; kill $runpid; fi
done
echo "$(date +%H:%M) DSK-DONE balance $(bal)" >> dsk.log
