#!/bin/bash
# 0.0.84 (no reasoning back) against 0.0.85 (all of it) against 0.0.86 (the last 8-24 turns'), all seven tasks-oct tasks,
# deepseek-v4.1-flash-1017 at max on the vetted route key without the watcher (so a run keeps its provider and cache),
# one round in two waves (4 tasks, then 3; 12 and 9 runs at once), labels v360-v380 (per task: 0.0.84, 0.0.85, 0.0.86).
# Refuses to start under $2.50 on the account; stops at 1750 fen (~$2.50). Logs to round7.log. (2026-10-25)
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95
M=deepseek-v4.1-flash-1017 P=0.02,0.0004,0.08 L=360
log() { echo "$(date +%H:%M) $*" >> round7.log; }
B=$(python3 -I -c "import sys; sys.path.insert(0, '..'); import subrouter as s; print(int(s.api('/api/user/self')['quota']) / s.QUOTA_PER_USD)")
python3 -c "import sys; sys.exit(0 if float('$B') >= 2.5 else 1)" || { log "balance \$$B under \$2.50: not starting"; exit 1; }
used() { curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])"; }
U=$(used); [ -n "$U" ] || U=$(used); [ -n "$U" ] || { log "no SubRouter counter reading: not starting"; exit 1; }
./sub_guard.sh "$U" 1750 $(seq -f "v%g" 360 380) >> sub-guard-round7.log 2>&1 &
log "balance \$$B; start"
wave() {
  for t in "$@"; do
    for v in v84 v85 v86; do
      HARNESS_BIN=$HOME/bench-homes/tz-$v/tazuna ./sub_run.sh $M v$L $P 0.5 $t &
      log "$t: $v v$L"; L=$((L+1))
    done
  done
  wait $(jobs -p | grep -v "$GUARD"); log "wave done: $*"
}
GUARD=$(jobs -p | tail -1)
wave rclone-archive-symlinks caddy-unix-socket-close caddy-retry-match-expr gitea-actions-cancelling
wave lazygit-mnemonic-prefix prometheus-exemplar-compaction telegraf-snmp-reconnect
log "ROUND7-DONE"
