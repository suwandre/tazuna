#!/usr/bin/env python3
"""Key-free latency at the app-server and own-loop stdio boundaries.

From the repo root: python3 bench/local_latency.py target/release/tazuna [another-binary ...]
"""
import argparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import math
import os
from pathlib import Path
import statistics
import subprocess
import tempfile
import threading
import time


def fixture(root, repo):
    home = root / "home"
    base = home / ".local/share/tazuna"
    recs = base / "sessions"
    recs.mkdir(parents=True)
    (base / "openrouter-models.json").write_text("{}")
    dirs = [repo] + [root / f"project-{i}" for i in range(19)]
    for d in dirs[1:]:
        d.mkdir()
    env = {"HOME": str(home), "USERPROFILE": str(home), "PATH": os.environ["PATH"],
           "TAZUNA_PRICE": "0,0,0", "LOCAL_BENCH_KEY": "unused"}
    rec = recs / "1700000000-long.jsonl"
    answer = ("Analysis of the source file and its tests. " * 30).strip()
    sid = "local-bench-long"
    with rec.open("w") as f:
        def event(t, e):
            f.write(json.dumps({"t": t, "e": e}) + "\n")

        event(0, {"type": "system", "subtype": "init", "session_id": sid})
        for i in range(1000):
            event(i * 3 + 1, {"type": "harness_user", "text": "Explain this module"})
            event(i * 3 + 2, {"type": "assistant", "message": {
                "content": [{"type": "text", "text": answer}]}})
            event(i * 3 + 3, {"type": "result", "subtype": "success", "total_cost_usd": 0})
    shorter = (json.dumps({"t": 1, "e": {"type": "harness_user", "text": answer}}) + "\n") * 250
    for i in range(100):
        (recs / f"{1700000001 + i}-local.jsonl").write_text(shorter)
    with (base / "runs.jsonl").open("w") as f:
        for i in range(6000):
            f.write(json.dumps({"ts": 1700000000 + i, "prompt": answer[:500], "meta": {
                "cwd": str(dirs[i % len(dirs)]), "session_id": f"local-bench-{i // 100}-{i % len(dirs)}",
                "model": "claude-sonnet-4-6", "provider": "Claude", "profile": "lean"},
                "tokens": {"input": 2600, "output": 400, "cache_read": 27000}}) + "\n")
        f.write(json.dumps({"ts": 1700006000, "prompt": "Explain this module", "meta": {
            "cwd": str(repo), "session_id": sid, "recording": str(rec)}}) + "\n")
    histories = base / "own-sessions"
    histories.mkdir()
    (histories / "own-bench.json").write_text(json.dumps({"api": "Chat", "msgs": [
        {"role": "assistant", "content": answer} for _ in range(1600)]}))
    return env, dirs, sid


def receive(p, wanted, accept=lambda _: True):
    for line in p.stdout:
        end = time.perf_counter_ns()
        data = json.loads(line)
        if data.get("type") == wanted and accept(data):
            return end
    raise RuntimeError(f"tazuna exited without emitting {wanted}")


class StubModel(BaseHTTPRequestHandler):
    def do_POST(self):
        self.rfile.read(int(self.headers["Content-Length"]))
        body = json.dumps({"choices": [{"message": {"content": "ok"}, "finish_reason": "stop"}],
                           "usage": {"prompt_tokens": 20, "completion_tokens": 1}}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_):
        pass


def measure(binary, repo, env, dirs, sid, samples, url):
    def start(args):
        return subprocess.Popen([binary, *args], cwd=repo, env=env, stdin=subprocess.PIPE,
                                stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)

    def cmd(p, wanted, accept=lambda _: True, **args):
        began = time.perf_counter_ns()
        p.stdin.write(json.dumps(args).encode() + b"\n")
        p.stdin.flush()
        return (receive(p, wanted, accept) - began) / 1e6

    def stop(p):
        p.stdin.close()
        p.wait(timeout=5)
        p.stdout.close()

    for _ in range(samples + 2):  # warm up page cache and freshly built binaries
        began = time.perf_counter_ns()
        p = start(["app-server", "--allow-non-git", "--profile", "lean"])
        first = receive(p, "view", lambda d: not d.get("blocks"))
        values = {"sidecar first empty view (populated home)": (first - began) / 1e6}
        for n in (1, 5, 20):
            values[f"sidebar sessions ({n} folder{'s' if n != 1 else ''})"] = cmd(
                p, "sessions", lambda d: len(d["dirs"]) == n and all(r["list"] for r in d["dirs"]),
                op="sessions", dirs=list(map(str, dirs[:n])))
        values["sidecar file completion"] = cmd(p, "complete", lambda d: bool(d["files"]), op="complete")
        values["sidecar reopen ~3000-event transcript"] = cmd(
            p, "view", lambda d: len(d.get("blocks", [])) > 1000, op="resume", id=sid)
        stop(p)

        for args, label in [([], "own-loop init"), (["--resume", "own-bench"], "own-loop init + ~2 MB history")]:
            began = time.perf_counter_ns()
            p = start(["engine", "--url", url, "--key-env", "LOCAL_BENCH_KEY",
                       "--model", "local-bench", *args])
            values[label] = (receive(p, "system", lambda d: d.get("subtype") == "init" and (
                not args or d["session_id"] == "own-bench")) - began) / 1e6
            if not args:
                values["own-loop turn (instant localhost reply)"] = cmd(
                    p, "result", lambda d: d.get("result") == "ok", type="user",
                    message={"content": [{"type": "text", "text": "Say ok"}]})
            stop(p)
        if _ >= 2:
            yield values


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binaries", nargs="+", type=lambda s: Path(s).resolve())
    parser.add_argument("--samples", type=int, default=30)
    args = parser.parse_args()
    repo = Path.cwd()
    with tempfile.TemporaryDirectory(prefix="tazuna-local-latency-") as root, ThreadingHTTPServer(
            ("127.0.0.1", 0), StubModel) as server:
        threading.Thread(target=server.serve_forever, daemon=True).start()
        url = f"http://127.0.0.1:{server.server_port}/v1"
        env, dirs, sid = fixture(Path(root), repo)
        results = list(zip(*(measure(b, repo, env, dirs, sid, args.samples, url) for b in args.binaries)))
        for binary, data in zip(args.binaries, zip(*results)):
            print(f"{binary} ({args.samples} samples; median / p95 ms)")
            ranked = [(statistics.median(ms), sorted(ms)[math.ceil(0.95 * len(ms)) - 1], key)
                      for key in data[0] for ms in [[d[key] for d in data]]]
            for median, p95, key in sorted(ranked, reverse=True):
                print(f"{key:42} {median:8.2f} / {p95:7.2f}")


if __name__ == "__main__":
    main()
