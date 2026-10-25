#!/bin/bash
# A/B of 0.0.85 (DeepSeek's reasoning back in the history by default) against 0.0.84 (2026-10-25): deepseek-v4.1-flash-1017
# at max on the vetted route key (sub_run.sh's default) with `subrouter.py watch`, the three tasks at once per round,
# 2 rounds, labels v348-v359 (even: 0.0.84, odd: 0.0.85). Stops at 1000 fen (~$1.40). Logs to reason1.log.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95
M=deepseek-v4.1-flash-1017 P=0.02,0.0004,0.08 L=348
log() { echo "$(date +%H:%M) $*" >> reason1.log; }
used() { curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])"; }
U=$(used); [ -n "$U" ] || U=$(used); [ -n "$U" ] || { log "no SubRouter counter reading: not starting"; exit 1; }
./sub_guard.sh "$U" 1000 $(seq -f "v%g" 348 359) >> sub-guard-reason1.log 2>&1 &
python3 -u ../subrouter.py watch $M >> reason1-watch.log 2>&1 & W=$!
for r in 1 2; do
  for t in rclone-archive-symlinks caddy-unix-socket-close caddy-retry-match-expr; do
    HARNESS_BIN=$HOME/bench-homes/tz-v84/tazuna ./sub_run.sh $M v$L $P 0.5 $t &
    HARNESS_BIN=$HOME/bench-homes/tz-v85/tazuna ./sub_run.sh $M v$((L+1)) $P 0.5 $t &
    log "round $r $t: 0.0.84 v$L, 0.0.85 v$((L+1))"; L=$((L+2))
  done
  wait $(jobs -p | tail -6); log "round $r done"
done
kill $W; log "REASON1-DONE"
