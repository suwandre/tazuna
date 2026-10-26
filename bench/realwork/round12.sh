#!/bin/bash
# The cheap route against today's (2026-10-26 night): 0.0.91 (SubRouter unstreamed) on a key limited to a6aa, with the
# LingAI route key behind it (TAZUNA_ROUTES; 0.0.79 switches when a route fails, slows down or caches under 70% over 10
# calls), against 0.0.90 streamed on the LingAI route key. All seven tasks once each side by side, fourteen at once,
# v420-v433 (even: 0.0.90 LingAI streamed, odd: 0.0.91 a6aa unstreamed). deepseek-v4.1-flash-1017 at max.
# Stops at 1000 fen (~$1.40). Logs to round12.log.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUB_HARNESS=tazuna-ownsubx
M=deepseek-v4.1-flash-1017 P=0.02,0.0004,0.08 RK=SUBROUTER_ROUTE_DEEPSEEK_V4_1_FLASH_1017_KEY
log() { echo "$(date +%H:%M) $*" >> round12.log; }
used() { curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])"; }
U=$(used); [ -n "$U" ] || U=$(used); [ -n "$U" ] || { log "no SubRouter counter reading: not starting"; exit 1; }
./sub_guard.sh "$U" 1000 $(seq -f "v%g" 420 433) >> sub-guard-round12.log 2>&1 &
L=420
for t in rclone-archive-symlinks caddy-unix-socket-close caddy-retry-match-expr gitea-actions-cancelling \
         lazygit-mnemonic-prefix prometheus-exemplar-compaction telegraf-snmp-reconnect; do
  SUBROUTER_X_KEYENV=$RK HARNESS_BIN=$HOME/bench-homes/tz-v90/tazuna ./sub_run.sh $M v$L $P 0.5 $t &
  SUBROUTER_X_KEYENV=SUBROUTER_ROUTE_TEST_KEY TAZUNA_ROUTES="https://subrouter.ai/v1 $RK" HARNESS_BIN=$HOME/bench-homes/tz-v91/tazuna ./sub_run.sh $M v$((L+1)) $P 0.5 $t &
  log "$t: v90 LingAI streamed v$L, v91 a6aa unstreamed v$((L+1))"; L=$((L+2))
done
wait $(jobs -p | tail -14); log "ROUND12-DONE"
