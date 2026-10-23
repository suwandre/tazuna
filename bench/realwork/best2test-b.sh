#!/bin/bash
# Rounds 2 and 3 of best2test.sh one task at a time (3 engines: the single run and both best-of-two attempts) instead of
# all three tasks at once (9 engines): in round 1 Luna's calls on the 6 Luna key slowed from 5-22 s to 50-90 s, some
# waiting 8-13 minutes; this shows whether the subscription pool throttles parallel requests. Labels v263-v274 as planned;
# the spend guard started by best2test.sh still covers them. Logs to best2test.log.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
export SUBROUTER_X_KEYENV=SUBROUTER_LUNA3_KEY SUB_KEYENV=SUBROUTER_LUNA3_KEY HARNESS_BIN=$HOME/bench-homes/tz-v78/tazuna GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUB_EFFORT=max
log() { echo "$(date +%H:%M) $*" >> best2test.log; }
while ps -axo command= | grep run.py | grep -qE "v2(5[7-9]|6[0-2])@"; do sleep 30; done
log "round 1 done (one task at a time from here)"
L=263
for r in 2 3; do
  for t in rclone-archive-symlinks caddy-unix-socket-close caddy-retry-match-expr; do
    log "round $r $t: one run v$L, best of two v$((L+1))"
    SUB_HARNESS=tazuna-ownsubx ./sub_run.sh gpt-6-luna v$L 0.0082,0.0008,0.041 0.5 $t &
    SUB_HARNESS=tazuna-ownsubxbest2 ./sub_run.sh gpt-6-luna v$((L+1)) 0.0082,0.0008,0.041 0.5 $t &
    wait; L=$((L+2))
  done
  log "round $r done"
done
log "BEST2TEST-DONE"
