#!/bin/bash
# 2026-11-01: is GPT-6 Luna at xhigh really ahead of DeepSeek 1017 (mx.sh: telegraf judge 7.5 against 5.0/4.5, one run
# each), or was that telegraf? Luna xhigh (shared OpenAI Pro pools), DeepSeek xhigh (the default) and DeepSeek max (LingAI)
# on four tasks mx.sh did not use, two runs each (lx.jobs, v1001-v1024), eight at a time; jobs that already have a result
# are skipped, so a rerun after a stop finishes the rest. Spend stop $2 (sub_guard, kept up while this runs). Then, once
# mx.sh's gitea pools are judged, one judge pool per task (six runs), Opus xhigh, two passes: 8 calls. Started detached.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> lx.log; }
caffeinate -ims -w $$ &
export GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUB_TIMEOUT=3600 HARNESS_BIN=$HOME/bench-homes/tz-v108/tazuna SUB_HARNESS=tazuna-ownsubx
python3 -I -c "
import sys; sys.path.insert(0, '..'); import subrouter as s
for name, model, slugs, var, shared in (('tazuna-route-mxg', 'gpt-6-luna', ['shared-openai-pro-50', 'shared-openai-pro-52'], 'SUBROUTER_ROUTE_MXG_KEY', True),
                                        ('tazuna-route-mx', 'deepseek-v4.1-flash-1017', ['lingai'], 'SUBROUTER_ROUTE_MX_KEY', False)):
    i = s.key(name, model, slugs, var); k = s.api(f'/api/token/{i}')
    if bool(k.get('include_shared_subscriptions')) != shared: s.api('/api/token/', 'PUT', {**k, 'include_shared_subscriptions': shared})
" >> lx.log 2>&1 || { log "route keys failed"; exit 1; }
source ~/.subrouter-key
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
LABELS=$(awk '{print $4}' lx.jobs | tr '\n' ' ')
( while kill -0 $$ 2>/dev/null; do ./sub_guard.sh "$U" 1400 $LABELS >> sub-guard-lx.log 2>&1; sleep 20; done ) &
log "start; SubRouter used $U"
running() { ps -axo command= | grep -E "^/bin/bash \./sub_run\.sh \S+ v10[0-2][0-9] " | grep -vc grep; }
label() { case $1 in (luna-xhigh) echo "tazuna-ownsubx-$2@gpt-6-luna:xhigh" ;; (ds-xhigh) echo "tazuna-ownsubx-$2@deepseek-v4.1-flash-1017:xhigh" ;; (ds-max) echo "tazuna-ownsubx-$2@deepseek-v4.1-flash-1017:max" ;; esac; }
while read -r set task arm v; do
  case $set in tasks) rs=results.jsonl ;; *) rs=results-$set.jsonl ;; esac
  grep -q -- "-$v@" "$rs" 2>/dev/null && continue
  while [ "$(running)" -ge 8 ]; do sleep 30; done
  case $arm in
    luna-xhigh) M=gpt-6-luna E=xhigh K=SUBROUTER_ROUTE_MXG_KEY P=0.003,0.0006,0.015 ;;
    ds-xhigh) M=deepseek-v4.1-flash-1017 E=xhigh K=SUBROUTER_ROUTE_MX_KEY P=0.022,0.0022,0.088 ;;
    ds-max) M=deepseek-v4.1-flash-1017 E=max K=SUBROUTER_ROUTE_MX_KEY P=0.022,0.0022,0.088 ;;
  esac
  SUB_SET=$set SUB_EFFORT=$E SUBROUTER_X_KEYENV=$K ./sub_run.sh "$M" "$v" "$P" 0.5 "$task" < /dev/null &
  log "start $v $arm $task"; sleep 5
done < lx.jobs
while [ "$(running)" -gt 0 ]; do sleep 30; done
log "runs over"
python3 -I leakscan.py > leakscan-lx.out 2>&1
until grep -q "MXJ-DONE gitea" mxj.log 2>/dev/null; do sleep 60; done
for st in "tasks-oct lazygit-mnemonic-prefix" "tasks-oct prometheus-exemplar-compaction" "tasks-oct caddy-unix-socket-close" "tasks k6-run-once"; do
  set -- $st
  ONLY=$(grep " $2 " lx.jobs | while read -r s t arm v; do echo -n "$(label $arm $v),"; done)
  BENCH_SET=$1 JUDGE_TASKS=$2 JUDGE_OUT=judge.lx.$2.jsonl JUDGE_ONLY="$ONLY" python3 -I ./judge.py --passes 2 >> judge-lx.log 2>&1
  log "judged $2"
done
log "LX-DONE"
