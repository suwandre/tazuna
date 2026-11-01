#!/bin/bash
# The $5 stop for mx.sh, mx2.sh and mx3.sh together: sub_guard.sh ends when no labelled run is left (between two blocks),
# so it is started again while any of the three scripts lives. Spend counts from mx.sh's start (20128.85272 fen).
cd "$(dirname "$0")"
while pgrep -f "mx[23]?\.sh" > /dev/null; do
  ./sub_guard.sh 20128.85272 3500 $(seq -f "v%g" 823 990) >> sub-guard-mx.log 2>&1
  sleep 20
done
