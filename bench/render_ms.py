"""Median wall time (ms) to replay and render the sample recording, after one warm-up run.  usage: render_ms.py <binary>"""
import statistics, subprocess, sys, time

cmd = [sys.argv[1], "recordings/caddy-opus-medium.jsonl", "--snapshot", "100000", "--size", "120x40"]
subprocess.run(cmd, capture_output=True, check=True)
times = []
for _ in range(7):
    t = time.perf_counter()
    subprocess.run(cmd, capture_output=True, check=True)
    times.append((time.perf_counter() - t) * 1000)
print(round(statistics.median(times)))
