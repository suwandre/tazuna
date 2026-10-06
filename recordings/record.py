#!/usr/bin/env python3
"""Record one real cc-mid run as timestamped stream-json for the UI prototype: {"t": ms_since_start, "e": event}, after a {"t": 0, "prompt", "effort"} header.

  ./record.py <task> <out.jsonl> [model] [effort]
"""
import importlib.util, json, os, subprocess, sys, tempfile, time
from pathlib import Path

RW = Path.home() / "Desktop/agent-chats/cc-lean/realwork"
spec = importlib.util.spec_from_file_location("rw", RW / "run.py"); rw = importlib.util.module_from_spec(spec); spec.loader.exec_module(rw)
task, out = RW / "tasks" / sys.argv[1], Path(sys.argv[2])
model, effort = (sys.argv[3] if len(sys.argv) > 3 else "claude-opus-5-5"), (sys.argv[4] if len(sys.argv) > 4 else "medium")
with tempfile.TemporaryDirectory(prefix="rec-") as d, tempfile.TemporaryDirectory() as cfg:
    w = Path(d).resolve()
    rw.setup_workdir(task, w)
    cmd = ["claude", "-p", (task / "prompt.txt").read_text(), "--model", model, "--effort", effort, "--output-format", "stream-json",
           "--verbose", "--include-partial-messages", "--no-session-persistence", "--dangerously-skip-permissions", *rw.cc_extra("cc-mid", w)]
    env = rw.env_for("cc-mid", CLAUDE_CONFIG_DIR=cfg, CLAUDE_CODE_OAUTH_TOKEN=open(os.path.expanduser("~/.config/cc-bench-token")).read().strip())
    t0 = time.monotonic()
    with subprocess.Popen(cmd, cwd=w, env=env, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, stdin=subprocess.DEVNULL, text=True) as p, out.open("w") as f:
        f.write(json.dumps({"t": 0, "prompt": (task / "prompt.txt").read_text(), "effort": effort}) + "\n")
        for line in p.stdout:
            try:
                e = json.loads(line)
            except ValueError:
                continue
            f.write(json.dumps({"t": int((time.monotonic() - t0) * 1000), "e": e}) + "\n")
print("recorded", out, f"{time.monotonic() - t0:.0f}s")
