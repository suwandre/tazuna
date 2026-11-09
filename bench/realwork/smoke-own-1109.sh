#!/bin/bash
# 2026-11-09 smoke test before an own-loop harness round: DeepSeek V4.1 Flash 1017 at high on SubRouter (its levels are off, high and max; xhigh runs as high, CHANGELOG 0.1.66) (one route,
# one key for every harness), one task, one run each. Does each harness run, edit and report its tokens? Starts once the
# Claude gauge is done (no CPU contention in the times). Stops at $1 spent (sub_guard.sh, 700 fen).
# Versions: tazuna 0.1.83, pi 1.5.0, omp 18.18.3, opencode 2.2.6, Command Code 1.88.5, Kimi Code 2.5.2, Codex 0.167.0.
# Route check just before: price fine, caching 37% (under its 50% bar): cheap either way, the same for every harness.
set -u
cd "$(dirname "$0")"
source ~/.subrouter-key
# every harness on tazuna's checked DeepSeek key (bangai, kriora, lingai: 99% cached in the route check). The first try
# used the general key, which pins no provider for DeepSeek: SubRouter sent every call to RelAI, 11% cached (stopped).
export SUBROUTER_X_KEYENV=SUBROUTER_ROUTE_DEEPSEEK_V4_1_FLASH_1017_KEY
log() { echo "$(date +%H:%M) $*" >> smoke-own-1109.log; }
used() { curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])"; }
M=deepseek-v4.1-flash-1017:high
L="tazuna-ownsubx-v1110@$M,pi-sub-v1110@$M,omp-sub-v1110@$M,opencode-sub-v1110@$M,cmd-sub-v1110@$M,kimi-sub-v1110@$M,codexsub-v1110@$M"
U=$(used); log "start (used $U fen)"
python3 -u run.py --harness "$L" --tasks caddy-quoted-braces --runs 1 --workers 8 > smoke-own-1109.out 2>&1 &
RUN=$!
./sub_guard.sh "$U" 700 v1110 >> smoke-own-1109.log 2>&1 &
GUARD=$!
wait $RUN
kill $GUARD 2>/dev/null
log "end (used $(used) fen, start $U)"
log "SMOKE-DONE"
