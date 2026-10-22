#!/usr/bin/env python3
"""Usage guard for subscription benchmarks. Polls the account meter and gates the runners.

  PAUSE file  -> runners stop starting new jobs (5h window >= threshold, or meter unreadable)
  ABORT file  -> extra-usage credits rose: runners exit, guard kills in-flight `claude -p` children

Reads the meter with pi's bench login (pi refreshes it without a model call).
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
PI_HOME = Path.home() / "bench-homes/pi"


def meter():
    tok = subprocess.run(["pi", "auth", "print-bearer-token", "--provider", "anthropic", "--min-expiry", "10m"],
                         env={**os.environ, "HOME": str(PI_HOME)}, capture_output=True, text=True,
                         stdin=subprocess.DEVNULL, timeout=60).stdout.strip().splitlines()[-1]
    r = subprocess.run(["curl", "-s", "-m", "30", "https://api.anthropic.com/api/oauth/usage", "-H", f"Authorization: Bearer {tok}",
                        "-H", "anthropic-beta: oauth-2025-04-20", "-H", "User-Agent: tazuna-guard/1"],
                       capture_output=True, text=True, timeout=60)
    d = json.loads(r.stdout)
    # the tighter of the 5h window and the weekly limit: either one running out spills into extra usage
    meters = [d[k] for k in ("five_hour", "seven_day") if isinstance(d.get(k), dict) and d[k].get("utilization") is not None]
    top = max(meters, key=lambda m: float(m["utilization"]))
    meter.each = {k: float(d[k]["utilization"]) for k in ("five_hour", "seven_day") if isinstance(d.get(k), dict) and d[k].get("utilization") is not None}
    return float(top["utilization"]), top["resets_at"], float(d["extra_usage"]["used_credits"])


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
