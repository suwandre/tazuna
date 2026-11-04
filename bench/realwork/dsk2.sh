#!/bin/bash
# 2026-11-04 night (off-peak), follow-up to dsk.sh: the other five tasks of tasks/ x one run x both arms (caddy has two
# runs per arm from dsk.sh). Same as the Harness: DeepSeek's official API, deepseek-flash (V4.1 Flash 1017) at xhigh; reasoning
# sent back (owndskr, TAZUNA_SEND_REASONING) or not (owndsk). tazuna 0.1.54 + search-prompt fix frozen in tz-v154s.
# Balance guard: stop everything past CNY 3.20 spent (granted credit, CNY 3.58 at start).
cd "$(dirname "$0")"; source ~/.bench-token; source ~/.deepseek-key; export PATH="/opt/homebrew/bin:$PATH"
export TAZUNA_PRICE=0.15,0.003,0.60 TAZUNA_MAX_USD=0.12 HARNESS_BIN=$HOME/bench-homes/tz-v154s/tazuna BENCH_TIMEOUT=2400
caffeinate -ims -w $$ &
bal() { curl -s https://api.deepseek.com/user/balance -H "Authorization: Bearer $DEEPSEEK_API_KEY" | python3 -c 'import sys,json; print(next(b["total_balance"] for b in json.load(sys.stdin)["balance_infos"] if b["currency"]=="CNY"))'; }
start=$(bal); echo "$(date +%H:%M) start balance $start" >> dsk2.log
./run.py --runs 1 --workers 8 --tasks hugo-section-slug,hugo-template-return,k6-run-once,prometheus-duration-parens,terraform-import-foreach --harness "tazuna-owndsk-v1541@deepseek-flash:xhigh,tazuna-owndskr-v1541@deepseek-flash:xhigh" >> dsk2.out 2>&1 &
runpid=$!
while kill -0 $runpid 2>/dev/null; do
  sleep 30; now=$(bal)
  if python3 -c "import sys; sys.exit(0 if $start-$now>3.2 else 1)"; then echo "$(date +%H:%M) GUARD stop at $now" >> dsk2.log; kill $runpid; pkill -f 'tz-v154s/tazuna'; fi
done
echo "$(date +%H:%M) DSK2-DONE balance $(bal)" >> dsk2.log
