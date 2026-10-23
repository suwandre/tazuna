#!/bin/bash
# The rest of rounds 2 and 3 of the best-of-two test on the LingAI key (LUNA4), one task at a time: on the 6 Luna key
# a run stayed on one pool for its whole length, 8-16 s a call on one and 56-148 s on the other (OpenAI Team, still
# routed although switched off); LingAI ran its first 15-23 calls at 5-8 s. Round 2 rclone (v263/v264) already runs
# there. Labels v265-v274; a spend guard of its own (1000 fen, ~$1.40). Logs to best2test.log.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
export SUBROUTER_X_KEYENV=SUBROUTER_LUNA4_KEY SUB_KEYENV=SUBROUTER_LUNA4_KEY HARNESS_BIN=$HOME/bench-homes/tz-v78/tazuna GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUB_EFFORT=max
log() { echo "$(date +%H:%M) $*" >> best2test.log; }
used() { curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])"; }
U=$(used); [ -n "$U" ] || U=$(used); [ -n "$U" ] || { log "no SubRouter counter reading: not starting"; exit 1; }
while ps -axo command= | grep run.py | grep -qE "v26[34]@"; do sleep 30; done
./sub_guard.sh "$U" 1000 $(seq -f "v%g" 265 274) >> sub-guard-best2test.log 2>&1 &
L=265
for spec in 2:caddy-unix-socket-close 2:caddy-retry-match-expr 3:rclone-archive-symlinks 3:caddy-unix-socket-close 3:caddy-retry-match-expr; do
  r=${spec%%:*}; t=${spec#*:}
  log "round $r $t on LingAI: one run v$L, best of two v$((L+1))"
  SUB_HARNESS=tazuna-ownsubx ./sub_run.sh gpt-6-luna v$L 0.0082,0.0008,0.041 0.5 $t &
  SUB_HARNESS=tazuna-ownsubxbest2 ./sub_run.sh gpt-6-luna v$((L+1)) 0.0082,0.0008,0.041 0.5 $t &
  wait $(jobs -p | tail -2); L=$((L+2))
done
log "BEST2TEST-DONE"
