#!/usr/bin/env python3
"""tazuna's build and tests on Linux, on an InstaBox machine (free plan: one machine, 4 vCPU, 8 GiB, pauses after 30 idle
minutes, at most 8 hours at a time). Only tazuna's own source goes there (`git archive HEAD`): no GitHub access, no
SubRouter or Claude keys, never company code. The key stays in ~/.instabox-key and goes only to api.instabox.sh.

  [INSTABOX_MACHINE=NAME] linux_ci.py   create or wake the machine, install Rust once, build and test HEAD; prints the tail and exits with
                       the tests' exit code
"""
import json, os, subprocess, sys, time, urllib.error, urllib.request, uuid
from pathlib import Path

# INSTABOX_MACHINE: an existing machine to use (the free plan allows one; the user's is andres-machine, 2026-10-31)
API, NAME = "https://api.instabox.sh/v1", os.environ.get("INSTABOX_MACHINE", "tazuna-ci")
KEY = (Path.home() / ".instabox-key").read_text().strip()
REPO = Path(__file__).resolve().parent.parent


def call(method, path, body=None, raw=None, timeout=120):
    h = {"X-API-Key": KEY}
    if body is not None:
        raw, h["Content-Type"] = json.dumps(body).encode(), "application/json"
    if method == "POST" and path == "/machines":
        h["Idempotency-Key"] = str(uuid.uuid4())
    q = urllib.request.Request(API + path, data=raw, method=method, headers=h)
    try:
        with urllib.request.urlopen(q, timeout=timeout) as r:
            data = r.read()
            return json.loads(data) if data else {}
    except urllib.error.HTTPError as e:
        sys.exit(f"{method} {path}: HTTP {e.code} {e.read()[:300]!r}")


def wait(op):
    while op.get("state") not in ("succeeded", "failed"):
        op = call("GET", f"/operations/{op['id']}?wait=25s", timeout=40)
    if op["state"] == "failed":
        sys.exit(f"operation {op.get('type')} failed: {op.get('error')}")
    return op


def sh(mid, cmd, timeout=600):
    return call("POST", f"/machines/{mid}/exec", {"cmd": cmd, "timeoutSeconds": timeout}, timeout=timeout + 30)


def machine():
    m = next((m for m in call("GET", "/machines")["machines"] if m.get("name") == NAME), None)
    if not m:
        print("creating the machine")
        return wait(call("POST", "/machines", {"name": NAME, "templateId": "ci"}))["machineId"]
    if m.get("state") != "running":
        print(f"machine is {m.get('state')}: waking it")
        op = call("POST", f"/machines/{m['id']}/wake", {})
        if op.get("id"):
            wait(op)
    return m["id"]


def main():
    mid = machine()
    r = sh(mid, "command -v cargo >/dev/null || ~/.cargo/bin/cargo -V >/dev/null 2>&1 || "
                "(curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal >/dev/null 2>&1 && echo installed rust)")
    print((r.get("stdout") or "").strip() or "rust present")
    src = subprocess.run(["git", "archive", "--format=tar.gz", "HEAD"], cwd=REPO, capture_output=True, check=True).stdout
    head = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=REPO, capture_output=True, text=True).stdout.strip()
    call("PUT", "/machines/{}/files?path=/data/home/tazuna.tgz".format(mid), raw=src)
    # longer than one exec may take (600 s): in the background, then polled
    sh(mid, "rm -rf /data/home/tazuna && mkdir -p /data/home/tazuna && tar xzf /data/home/tazuna.tgz -C /data/home/tazuna && "
            "rm -f /data/home/ci.exit && (cd /data/home/tazuna && nohup bash -lc 'source ~/.cargo/env 2>/dev/null; "
            "cargo build --release && cargo test; echo $? > /data/home/ci.exit' > /data/home/ci.log 2>&1 &)")
    print(f"building and testing {head} on Linux")
    t = time.time()
    while True:
        code = (sh(mid, "cat /data/home/ci.exit 2>/dev/null").get("stdout") or "").strip()
        if code:
            break
        time.sleep(20)
    tail = sh(mid, "grep -E '^(error|warning: unused)|test result|FAILED|panicked' /data/home/ci.log | tail -20; tail -3 /data/home/ci.log")
    print(tail.get("stdout", ""))
    print(f"{head}: exit {code} after {(time.time() - t) / 60:.1f} min")
    sys.exit(int(code))


if __name__ == "__main__":
    main()
