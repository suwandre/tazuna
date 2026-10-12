"""Footprint of a whole agent run: tazuna plus everything it starts (claude, tool servers), not the agent's own commands.
Runs one tiny bug fix headlessly and samples the process tree every 0.25 s. Prints one JSON object.

  python3 bench/agent_footprint.py <binary> [tazuna flags...]
  e.g. own loop:    ... --data any --provider https://api.cheaperinference.com/v1 --key-env CHEAPER_INFERENCE_API_KEY \\
                        --model deepseek-v4.1-flash --effort max --no-escalate
       claude path: ... --model claude-haiku-5-5 --effort low

- peak_rss_mb: the largest sum of RSS over the harness processes at one moment; peak_rss_by_process_mb: per kind
- cpu_s: CPU seconds used by the harness processes (highest cumulative value seen per process, summed)
"""
import json, os, subprocess, sys, tempfile, time

BIN, FLAGS = os.path.abspath(sys.argv[1]), sys.argv[2:]
HARNESS = ("tazuna", "claude", "node")  # tazuna, its engine and tool server; the claude CLI and its MCP servers


def kind(cmd):
    exe = os.path.basename(cmd.split()[0]) if cmd else ""
    if exe == "tazuna":
        return "tazuna " + (cmd.split()[1] if len(cmd.split()) > 1 and cmd.split()[1] in ("engine", "mcp") else "main")
    return exe if exe in HARNESS else None


def cpu_seconds(t):  # ps TIME: [[dd-]hh:]mm:ss.ss
    parts = t.replace("-", ":").split(":")
    return sum(float(p) * m for p, m in zip(reversed(parts), (1, 60, 3600, 86400)))


def tree(root):
    rows = subprocess.run(["ps", "-axo", "pid=,ppid=,rss=,time=,command="], capture_output=True, text=True).stdout.splitlines()
    procs = {}
    for r in rows:
        f = r.split(None, 4)
        if len(f) == 5:
            procs[int(f[0])] = (int(f[1]), int(f[2]), cpu_seconds(f[3]), f[4])
    keep, todo = {}, [root]
    while todo:
        p = todo.pop()
        for pid, (ppid, rss, cpu, cmd) in procs.items():
            if ppid == p and pid not in keep:
                keep[pid] = (rss, cpu, cmd)
                todo.append(pid)
    if root in procs:
        keep[root] = procs[root][1:]
    return keep


with tempfile.TemporaryDirectory() as work:
    open(os.path.join(work, "calc.py"), "w").write("def add(a, b):\n    return a - b\n")
    open(os.path.join(work, "test_calc.py"), "w").write("from calc import add\n\n\ndef test_add():\n    assert add(2, 3) == 5\n")
    open(os.path.join(work, "prompt.txt"), "w").write("Fix the bug in calc.py so that `python3 -m pytest -q` passes. Change nothing else.\n")
    subprocess.run(["git", "init", "-q"], cwd=work)
    subprocess.run(["git", "add", "-A"], cwd=work)
    subprocess.run(["git", "-c", "user.email=b@b", "-c", "user.name=b", "commit", "-qm", "seed"], cwd=work)
    t0 = time.monotonic()
    p = subprocess.Popen([BIN, "--run", "prompt.txt", "--verify", "python3 -m pytest -q", *FLAGS], cwd=work,
                         stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
    peak_total, peak_by, cpu = 0, {}, {}
    while p.poll() is None:
        total = 0
        for pid, (rss, c, cmd) in tree(p.pid).items():
            k = kind(cmd)
            if not k:
                continue  # the agent's own commands (python, git, shells) are not the harness
            total += rss
            peak_by[k] = max(peak_by.get(k, 0), rss)
            cpu[pid] = max(cpu.get(pid, 0), c)
        peak_total = max(peak_total, total)
        time.sleep(0.25)
    wall = time.monotonic() - t0
    out = p.stdout.read().strip().splitlines() if p.stdout else []
    result = json.loads(out[-1]) if out else {}
    fixed = "a + b" in open(os.path.join(work, "calc.py")).read()

print(json.dumps({"peak_rss_mb": round(peak_total / 1024, 1),
                  "peak_rss_by_process_mb": {k: round(v / 1024, 1) for k, v in sorted(peak_by.items())},
                  "cpu_s": round(sum(cpu.values()), 1), "wall_s": round(wall, 1), "fixed": fixed,
                  "turns": result.get("num_turns"), "cost_usd": result.get("total_cost_usd")}))
