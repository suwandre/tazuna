#!/usr/bin/env python3
"""leakscan.peer on the cases that happened (2026-10-28): python3 -I test_leakscan.py"""
import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent))
import leakscan as L

CASES = {  # transcript line, own run number, expected
    "cd and git diff in the run beside it (v504)": ('{"type":"tool_use","input":{"command":"cd /T/rw-caddy-tls-automate-names-cc-sub-v505@m:max-kmo90buw && git diff"}}', "peer"),
    "a path in a variable, then cp (v504)": ('{"type":"tool_use","input":{"command":"d=\\"/T/rw-caddy-tls-automate-names-cc-sub-v462@m:max-ri5gz_kt/x\\"; cp \\"$d/a\\" b/"}}', "peer"),
    "python over earlier transcripts (v518)": ('{"type":"message_end","message":{"content":[{"type":"toolCall","arguments":{"command":"python3 - <<PY\\nfor f in glob.glob(\'/Users/s/bench-homes/pi-sub/transcripts/*.jsonl\'):\\n  open(f)\\nPY"}}]}}', "peer"),
    "a read tool on another tree": ('{"type":"tool_use","name":"Read","input":{"file_path":"/T/rw-x-cc-sub-v462@m:max-abc/a.go"}}', "peer"),
    "listing a tree": ('{"type":"tool_use","input":{"command":"ls -la /T/rw-caddy-tls-automate-names-cc-sub-v505@m:max-k/"}}', "peer-listed"),
    "listing transcripts": ('{"type":"toolCall","arguments":{"command":"find /Users/s/bench-homes/pi-sub/transcripts -type f | head"}}', "peer-listed"),
    "a grep pattern over a listing": ('{"type":"tool_use","input":{"command":"ls /T/ | grep rw-caddy-tls-automate-names-cc-sub-v505@m"}}', None),
    "Codex reading another tree": ('{"type":"item.completed","item":{"type":"command_execution","command":"bash -lc \'cat /T/rw-x-cc-sub-v505@m:max-k/a.go\'"}}', "peer"),
    "its own tree, mistyped": ('{"type":"tool_use","input":{"command":"cd /T/rw-caddy-tls-automate-names-tazuna-ash-ownsubx-v504@m:max-x && ls"}}', None),
}
bad = [n for n, (t, want) in CASES.items() if L.peer(t, "504") != want]
print("ok" if not bad else f"FAILED: {bad}")
sys.exit(bool(bad))
