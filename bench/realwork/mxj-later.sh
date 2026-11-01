#!/bin/bash
# the gitea pools of mxj.sh, once Claude's 5-hour window has reset (01:20 on 2026-11-01)
cd "$(dirname "$0")"
until [ "$(date +%H%M)" -ge 0122 ] && [ "$(date +%H%M)" -lt 1200 ]; do sleep 30; done
TASKS=gitea-actions-cancelling ./mxj.sh
