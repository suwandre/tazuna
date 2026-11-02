#!/bin/bash
# 2026-11-02, 19:15: the rest of bh.sh, then ln.sh, now (the user stopped work for the day: up to 99% of the 5-hour
# window; runs start below 88% so the eight in flight stay under the guard's 99% kill). Started with nohup.
cd "$(dirname "$0")"
./bh.sh
./ln.sh
