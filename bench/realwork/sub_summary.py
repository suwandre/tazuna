#!/usr/bin/env python3
"""Per-run summary of the SubRouter runs on rclone-archive-symlinks: tokens, real cost, routes, speed, hidden tests, judge.

    sub_summary.py USAGE_LOG.csv [MORE.csv ...] [LABEL ...]

Real cost comes from SubRouter's own usage log (the console's CSV export): the quota of every request for the run's model
between its start and end line in mrt-<label>.log, at 500,000 quota per USD (checked against the account counter on
v145, v148 and v156: within 5% of the counter at 7 yuan per USD). Tokens and speed come
from tazuna's session log of the run; hidden tests from results-tasks-oct.jsonl; judge scores from judge.oct.*.jsonl.
Writes sub-summary.json next to this file and prints a markdown table.
"""
import csv, glob, json, os, re, statistics, sys, collections
from datetime import datetime, timedelta

HERE = os.path.dirname(os.path.abspath(__file__))
SESSIONS = os.path.expanduser("~/bench-homes/cc/.local/share/tazuna/sessions")
QUOTA_PER_USD, UTC_OFFSET = 500_000, timedelta(hours=2)  # the log is in UTC, the run logs in CEST
DEFAULT = ["v129", "v131", "v137", "v140", "v141", "v143", "v145", "v148", "v150", "v151", "v153", "v154", "v156",
           "v157", "v158", "v159", "v160", "v161", "v163", "v165", "v166", "v167", "v168", "v169", "v170", "v172", "v174",
           "v175", "v176", "v177", "v178", "v179", "v180", "v181", "v182", "v183", "v184", "v185", "v186", "v189", "v196",
           "v197", "v200", "v201", "v202", "v203", "v204", "v205", "v207"]
LOG_MODEL = {"minimax-m3.1-flash-free": "minimax/minimax-m3.1-flash:free"}  # the run's model as SubRouter's log names it


def run_window(label):
    """(model, start, end, result line, effort) from mrt-<label>.log; the log's lines carry the time only, its creation the
    date; the effort is on the start line since 2026-10-22 (None before)."""
    path = os.path.join(HERE, f"mrt-{label}.log")
    lines = open(path).read().splitlines()
    m = re.match(r"(\d\d):(\d\d) start (\S+)", lines[0])
    end = next((l for l in lines if " end:" in l), None)
    eff = (re.search(r" effort (\w+)", lines[0]) or [None, None])[1]
    born = datetime.fromtimestamp(os.stat(path).st_birthtime)
    day = datetime(born.year, born.month, born.day)
    start = day + timedelta(hours=int(m[1]), minutes=int(m[2]))
    if end is None:  # still running
        return m[3], start, datetime.now(), "running", eff
    e = re.match(r"(\d\d):(\d\d)", end)
    stop = day + timedelta(hours=int(e[1]), minutes=int(e[2]) + 1)
    if stop < start:
        stop += timedelta(days=1)
    r0 = re.search(r"r0: (.*)", end)
    return m[3], start, stop, (r0[1] if r0 else end.split(" ", 1)[-1]), eff


def session(label):
    """Token totals, call count and wall seconds of the run from tazuna's session log."""
    files = [f for f in os.listdir(SESSIONS) if f"-{label}@" in f]
    if not files:
        return None
    f = max(files, key=lambda x: os.path.getmtime(os.path.join(SESSIONS, x)))
    s = collections.Counter(); last = 0
    for l in open(os.path.join(SESSIONS, f)):
        j = json.loads(l); last = max(last, j.get("t", 0)); e = j.get("e", {})
        if e.get("type") == "stream_event" and e["event"]["type"] == "message_delta":
            u = e["event"]["usage"]; s["calls"] += 1
            for k in ("input_tokens", "cache_read_input_tokens", "cache_creation_input_tokens", "output_tokens"):
                s[k] += u.get(k) or 0
        if e.get("type") == "assistant":
            s["edits"] += sum(b.get("type") == "tool_use" and b.get("name") in ("edit", "write", "apply_patch") for b in e["message"].get("content", []))
    s["secs"] = last / 1000
    return s


def usage(rows, model, start, stop, effort=None):
    """Real spend, billed tokens, routes with their prices (USD per M: the log's provider prices are USD per M, charged
    at 0.5 quota per price x token) and per-request speed of the run's requests in SubRouter's log."""
    q, routes, rates, frts, tok = 0, collections.Counter(), [], [], collections.Counter()
    prices = {}
    for r in rows:
        t = datetime.strptime(r[0], "%Y-%m-%d %H:%M:%S") + UTC_OFFSET
        if r[2] != LOG_MODEL.get(model, model) or not start <= t <= stop or r[1] != "消费":
            continue
        d = json.loads(r[14]) if r[14].startswith("{") else {}
        if effort and d.get("reasoning_effort") != effort:  # two efforts of one model run side by side (v196/v197)
            continue
        q += int(r[7] or 0); route = d.get("provider_name") or r[4]; routes[route] += 1
        cr, cw = int(d.get("cache_tokens") or 0), int(d.get("cache_creation_tokens") or 0)
        # Claude rows count input without the cache, OpenAI-style rows with it
        tok["input"] += int(r[5] or 0) - (0 if d.get("claude") else cr); tok["cache_read"] += cr; tok["cache_write"] += cw
        tok["output"] += int(r[6] or 0)
        out, secs, frt = int(r[6] or 0), float(r[8] or 0), (d.get("frt") or 0) / 1000
        if out > 50 and secs - frt > 0.5:
            rates.append(out / (secs - frt))
        if frt > 0:
            frts.append(frt)
        if d.get("provider_input_price") is not None:
            prices[route] = dict(input=d["provider_input_price"], cache_read=d.get("provider_cache_read_price"),
                                 cache_write=d.get("provider_cache_creation_price_5m"), output=d.get("provider_output_price"))
    return dict(usd=q / QUOTA_PER_USD, routes=dict(routes.most_common()), prices=prices, tokens=tok,
                gen_tok_s=round(statistics.median(rates), 1) if rates else None,
                first_token_s=round(statistics.median(frts), 1) if frts else None)


def judges():
    """Latest judge row per harness from the October judge files."""
    out = {}
    for f in sorted(glob.glob(os.path.join(HERE, "judge.oct.*.jsonl")), key=os.path.getmtime):
        for l in open(f):
            try:
                j = json.loads(l)
            except ValueError:
                continue
            if j.get("task") == "rclone-archive-symlinks" and "overall" in j:
                out[j["harness"]] = j
    return out


def main():
    files, labels = [a for a in sys.argv[1:] if a.endswith(".csv")], [a for a in sys.argv[1:] if not a.endswith(".csv")] or DEFAULT
    seen, rows = set(), []  # several exports may overlap: one row per request id
    for f in files:
        for r in list(csv.reader(open(f, encoding="utf-8-sig")))[1:]:
            if r[11] not in seen:
                seen.add(r[11]); rows.append(r)
    results = {r["harness"]: r for r in map(json.loads, open(os.path.join(HERE, "results-tasks-oct.jsonl"))) if r.get("task") == "rclone-archive-symlinks"}
    jd, out = judges(), []
    for label in labels:
        model, start, stop, end, effort = run_window(label)
        s = session(label) or collections.Counter()
        harness = next((h for h in results if f"-{label}@" in h), None)
        u = usage(rows, model, start, stop, effort)
        # billed tokens from SubRouter's log when the run is in it, else tazuna's own counts (no cache writes there)
        b = u["tokens"] if u["routes"] else {"input": s["input_tokens"], "cache_read": s["cache_read_input_tokens"], "cache_write": 0, "output": s["output_tokens"]}
        inp, cr, cw, o = b["input"], b["cache_read"], b["cache_write"], b["output"]
        tot = inp + cr + cw
        j = next((v for h, v in jd.items() if f"-{label}@" in h), None)
        out.append(dict(label=label, model=model, start=start.strftime("%m-%d %H:%M"), calls=s["calls"], edits=s["edits"],
                        input=inp, cache_read=cr, cache_write=cw, output=o, cached_share=round(cr / tot, 3) if tot else None,
                        usd=round(u["usd"], 4) if u["routes"] else None, usd_per_call=round(u["usd"] / s["calls"], 5) if u["routes"] and s["calls"] else None,
                        usd_per_m_tokens=round(u["usd"] / tot * 1e6, 4) if u["routes"] and tot else None, routes=u["routes"], route_prices=u["prices"],
                        wall_s=round(s["secs"]), run_tok_s=round(s["output_tokens"] / s["secs"], 1) if s["secs"] else None,
                        gen_tok_s=u["gen_tok_s"], first_token_s=u["first_token_s"],
                        hidden=f"{results[harness]['passed']}/3" if harness else None, outcome=end[:60],
                        # another run of the same model in the same window: the spend and tokens are both runs' (v175 with v177)
                        overlap=sum(u["routes"].values()) > 1.2 * s["calls"],
                        judge=j["overall"] if j else None))
    path = os.path.join(HERE, "sub-summary.json")  # runs not asked for keep their earlier rows (older exports)
    keep = [r for r in json.load(open(path)) if r["label"] not in labels] if os.path.exists(path) else []
    json.dump(sorted(keep + out, key=lambda r: int(r["label"][1:])), open(path, "w"), indent=1, ensure_ascii=False)
    print("| run | model | calls | in / cache read / write / out (K) | cached | real $ | $/call | $/M tok | run tok/s | gen tok/s | 1st tok s | hidden | judge | routes |")
    print("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|")
    for r in out:
        k = lambda n: f"{n / 1000:.0f}"
        print(f"| {r['label']} | {r['model']} | {r['calls']} | {k(r['input'])} / {k(r['cache_read'])} / {k(r['cache_write'])} / {k(r['output'])} | "
              f"{r['cached_share'] or 0:.0%} | {r['usd'] if r['usd'] is not None else '—'} | {r['usd_per_call'] or '—'} | {r['usd_per_m_tokens'] or '—'} | "
              f"{r['run_tok_s']} | {r['gen_tok_s'] or '—'} | {r['first_token_s'] or '—'} | {r['hidden'] or 'cut'} | {r['judge'] or '—'} | "
              f"{', '.join(f'{a} {b}' for a, b in list(r['routes'].items())[:3])} |")


if __name__ == "__main__":
    main()
