#!/bin/bash
# 2026-10-28: pi5.sh's second batch, held back while run.py got the key relay and per-harness credentials: after v526-v529
# end, caddy-retry-match-expr pi v530 and tazuna 0.0.97 v531 (foxrouter, tazuna-vet-4; pi5's spend stop covers both).
cd "$(dirname "$0")"; source ~/.subrouter-key
caffeinate -ims -w $$ &
until [ $(grep -l " end:" mrt-v52[6-9].log 2>/dev/null | wc -l) -ge 4 ]; do sleep 30; done
export SUB_KEYENV=SUBROUTER_MODELS_KEY SUB_EFFORT=max GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUBROUTER_X_KEYENV=SUBROUTER_VET4_KEY
M=deepseek-v4.1-flash-1017 P=0.02,0.0004,0.08
SUB_SET=tasks-oct SUB_HARNESS=pi-sub ./sub_run.sh $M v530 $P 0.5 caddy-retry-match-expr & A=$!
SUB_SET=tasks-oct SUB_HARNESS=tazuna-ownsubx HARNESS_BIN=$HOME/bench-homes/tz-v97/tazuna ./sub_run.sh $M v531 $P 0.5 caddy-retry-match-expr & B=$!
wait $A $B
