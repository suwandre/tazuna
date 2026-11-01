#!/usr/bin/env python3
"""Usage guard for subscription benchmarks. Polls the account meter and gates the runners.

  PAUSE file  -> runners stop starting new jobs (5h window >= threshold, or meter unreadable)
  ABORT file  -> extra-usage credits rose: runners exit, guard kills in-flight `claude -p` children

Reads the meter from the official claude CLI's rate_limit_event (see meter()).
"""
import json, os, signal, subprocess, time
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
PAUSE, ABORT, STATE = HERE / "PAUSE", HERE / "ABORT", HERE / "guard.state.json"
PAUSE_AT = float(os.environ.get("GUARD_PAUSE_AT", 70))
KILL_AT = float(os.environ.get("GUARD_KILL_AT", 100))  # % of the tighter limit: kill the agents here (100: before paid usage)   # % of 5h window; leaves headroom for in-flight runs
POLL = int(os.environ.get("GUARD_POLL", 150))
EXTRA_ALLOW = float(os.environ.get("GUARD_EXTRA_ALLOW", 0))  # credits of extra usage a run may spend on purpose (pi, Hermes)


SESSIONS = [Path.home() / "bench-homes/cc/.local/share/tazuna/sessions", Path.home() / ".local/share/tazuna/sessions"]
FRESH = 600         # a rate_limit_event older than this (by its file's last write) is not used
PROBE_EVERY = 600   # with none fresh, one tiny `claude -p` call at most this often


def latest_event():
    """The newest rate_limit_event a claude process wrote into a tazuna session file lately (bench runs, live sessions)."""
    best, now = None, time.time()
    for d in SESSIONS:
        for f in (d.glob("*.jsonl") if d.exists() else []):
            m = f.stat().st_mtime
            if now - m > FRESH or (best and m <= best[0]):
                continue
            with open(f, "rb") as fh:
                fh.seek(max(0, f.stat().st_size - 400_000))
                tail = fh.read().decode(errors="replace")
            for line in reversed(tail.splitlines()):
                if '"rate_limit_event"' in line:
                    try:
                        best = (m, json.loads(line)["e"]["rate_limit_info"])
                    except (ValueError, KeyError):
                        continue
                    break
    return best


def probe():
    """One tiny call through the official claude CLI (Haiku, no tools, a one-line system prompt, no project settings):
    its stream carries the plan's rate_limit_event. The CLI's own bench login (~/.bench-token, `claude setup-token`)."""
    env = dict(os.environ)
    for line in (Path.home() / ".bench-token").read_text().splitlines():
        if line.startswith("export ") and "=" in line:
            k, v = line[7:].split("=", 1)
            env.setdefault(k, v.strip().strip('"'))
    with __import__("tempfile").TemporaryDirectory() as cwd:
        r = subprocess.run(["claude", "-p", "Reply with: ok", "--model", "claude-haiku-5-5", "--output-format", "stream-json", "--verbose",
                            "--no-session-persistence", "--setting-sources", "", "--strict-mcp-config", "--tools", "",
                            "--system-prompt", "Reply briefly."], cwd=cwd, env=env, capture_output=True, text=True,
                           stdin=subprocess.DEVNULL, timeout=180)
    for line in r.stdout.splitlines():
        if '"rate_limit_event"' in line:
            return json.loads(line)["rate_limit_info"]
    raise RuntimeError(f"no rate_limit_event from claude -p (exit {r.returncode}): {r.stderr[-160:]}")


def meter():
    """The plan meter from the official `claude` CLI's rate_limit_event (2026-11-01): until then the guard read Anthropic's
    usage endpoint with pi's subscription login, which stopped resolving (and third-party tools may no longer use a
    subscription login). The newest event in a session file if one is fresh, else a probe every PROBE_EVERY seconds,
    else the last reading. Returns (the tighter window %, its reset time, 1 when the plan runs on paid extra usage)."""
    got = latest_event()
    if got:
        meter.last = got[1]
    elif not getattr(meter, "last", None) or time.time() - getattr(meter, "probed", 0) >= PROBE_EVERY:
        meter.last, meter.probed = probe(), time.time()
    info = meter.last
    w = info["unifiedWindows"]
    meter.each = {k: round(100 * float(w[k]["utilization"]), 1) for k in ("five_hour", "seven_day") if k in w}
    top = max(meter.each, key=meter.each.get)
    reset = datetime.fromtimestamp(int(w[top]["resetsAt"]), tz=timezone.utc).isoformat()
    return meter.each[top], reset, 1.0 if info.get("isUsingOverage") else 0.0


# every agent a benchmark starts (and the judge, which is `claude -p` too); never an interactive session
AGENTS = ["claude -p", "tazuna --run", "omp -p", "pi -p", "hermes -z", "omo -p", "empryo --headless"]


def kill_runs():
    pids = sorted({p for pat in AGENTS for p in subprocess.run(["pgrep", "-f", pat], capture_output=True, text=True).stdout.split()})
    for p in pids:
        try:
            os.kill(int(p), signal.SIGTERM)
        except ProcessLookupError:
            pass
    return len(pids)


def log(msg):
    print(time.strftime("%H:%M:%S"), msg, flush=True)


def main():
    ABORT.unlink(missing_ok=True)
    baseline, last_ok, full, aborted = None, time.time(), False, False
    while True:
        try:
            util, reset, extra = meter()
            last_ok = time.time()
            if baseline is None:
                baseline = extra
                log(f"baseline extra credits {extra}")
            STATE.write_text(json.dumps(dict(util=util, reset=reset, extra=extra, baseline=baseline, at=time.time(), **getattr(meter, "each", {}))))
            # at 100% the next request is paid extra usage: stop the agents now instead of after the first charge
            if util >= KILL_AT and not full:
                full = True
                log(f"FULL: usage at {util}%; killed {kill_runs()} agent processes")
            elif util < KILL_AT:
                full = False
            # "reset" means the 5h window: the weekly meter can stay above PAUSE_AT - 50 for days, which kept a stale
            # PAUSE in place all week (2026-10-12)
            reset_low = getattr(meter, "each", {}).get("five_hour", util) < PAUSE_AT - 50 and util < PAUSE_AT
            if aborted:
                if reset_low:  # window has reset: resume with a fresh baseline
                    aborted = False
                    baseline = extra
                    ABORT.unlink(missing_ok=True); PAUSE.unlink(missing_ok=True)
                    log(f"RESUME: 5h at {util}%; new extra baseline {extra}")
                else:
                    log(f"ABORTED: still blocked, killed {kill_runs()} agent processes (5h {util}% extra {extra})")
            elif extra > baseline + EXTRA_ALLOW:
                aborted = True
                ABORT.touch(); PAUSE.touch()
                log(f"ABORT: extra usage rose {baseline} -> {extra}; killed {kill_runs()} claude -p")
            if util >= PAUSE_AT and not PAUSE.exists():
                PAUSE.touch(); log(f"PAUSE: usage at {util}% (resets {reset})")
            elif reset_low and PAUSE.exists():  # only after the window has reset
                PAUSE.unlink(); log(f"RESUME: 5h at {util}%")
            else:
                each = getattr(meter, "each", {})
                log(f"5h {each.get('five_hour', util)}%  week {each.get('seven_day', util)}%  extra {extra}  {'paused' if PAUSE.exists() else 'running'}")
        except Exception as e:
            log(f"meter read failed: {str(e)[:120]}")
            if time.time() - last_ok > 600 and not PAUSE.exists():
                PAUSE.touch(); log("PAUSE: meter unreadable for 10 min (fail-safe)")
        time.sleep(POLL)


if __name__ == "__main__":
    main()
