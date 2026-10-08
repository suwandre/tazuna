"""Footprint of the tazuna binary itself (not the claude CLI under it). Prints one JSON object.  usage: footprint.py <binary>

- startup_ms: median wall time to load the sample recording and render its first frame headlessly (7 runs after a warm-up)
- ui_cpu_pct / ui_rss_mb: the replay UI animating in a 120x40 pseudo-terminal for 10 s (CPU time over wall time, peak RSS)
- tool_server_rss_mb: `tazuna mcp` after the MCP handshake
"""
import fcntl, json, os, pty, select, signal, statistics, struct, subprocess, sys, termios, time

BIN, REC = sys.argv[1], "recordings/caddy-opus-medium.jsonl"


def startup_ms():
    cmd = [BIN, REC, "--snapshot", "0", "--size", "120x40"]
    subprocess.run(cmd, capture_output=True, check=True)
    times = []
    for _ in range(7):
        t = time.perf_counter()
        subprocess.run(cmd, capture_output=True, check=True)
        times.append((time.perf_counter() - t) * 1000)
    return round(statistics.median(times), 1)


def ui(seconds=10):
    pid, fd = pty.fork()
    if pid == 0:
        os.execv(BIN, [BIN, REC, "--speed", "1"])
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))
    os.kill(pid, signal.SIGWINCH)
    t0 = time.monotonic()
    while time.monotonic() - t0 < seconds:  # drain the screen output so the UI never blocks on a full pty
        if select.select([fd], [], [], 0.1)[0]:
            try:
                os.read(fd, 65536)
            except OSError:
                break
    wall = time.monotonic() - t0
    os.kill(pid, signal.SIGTERM)
    _, _, ru = os.wait4(pid, 0)
    os.close(fd)
    return round((ru.ru_utime + ru.ru_stime) / wall * 100, 2), round(ru.ru_maxrss / 1e6, 1)  # macOS: bytes


def tool_server_rss():
    p = subprocess.Popen([BIN, "mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    assert p.stdin and p.stdout
    p.stdin.write(b'{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"fp","version":"0"}}}\n')
    p.stdin.flush()
    p.stdout.readline()
    time.sleep(0.5)
    rss_kb = int(subprocess.run(["ps", "-o", "rss=", "-p", str(p.pid)], capture_output=True, text=True).stdout.strip())
    p.kill()
    return round(rss_kb / 1024, 1)


cpu, rss = ui()
print(json.dumps({"startup_ms": startup_ms(), "ui_cpu_pct": cpu, "ui_rss_mb": rss, "tool_server_rss_mb": tool_server_rss()}))
