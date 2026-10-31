#!/usr/bin/env python3
"""Runs a command, passing its stdout through unchanged, and writes each JSON line of it with the milliseconds since the
start to a file in tazuna's session format ({"t": ms, "e": event}), so timesplit.py can read a stock Claude Code run.

  stamp.py OUT-FILE COMMAND...
"""
import json, subprocess, sys, time

out, cmd = sys.argv[1], sys.argv[2:]
t0 = time.time()
p = subprocess.Popen(cmd, stdout=subprocess.PIPE, text=True)
with open(out, "w") as f:
    for line in p.stdout:
        sys.stdout.write(line)
        try:
            e = json.loads(line)
        except ValueError:
            continue
        f.write(json.dumps({"t": int((time.time() - t0) * 1000), "e": e}) + "\n")
sys.exit(p.wait())
