#!/usr/bin/env python3
"""Best and average finished rclone run per model, effort and provider site, with every number we have.

    site_summary.py

Runs: results-tasks-oct.jsonl (finished runs only; a cut run has no row), plain provider harnesses only (feature variants
like cascade or self-review left out). Tokens: input (uncached), cache read, cache write, output, cached share. Speed: run
tok/s (output over the whole run, tools and tests included) and call tok/s (median output per call over the call's own
time in tazuna's session log, first token included). Cost: the real bill where we have it (SubRouter: sub-summary.json,
from SubRouter's usage log), else tazuna's estimate at the price the run was started with (TAZUNA_PRICE, set from each
site's price list; "est"). Judge: the latest judge row for the run, with the DeepSeek anchor of the same judge call.
Writes site-summary.json and site-summary.md next to this file.
"""
import glob, json, os, re, statistics, collections

HERE = os.path.dirname(os.path.abspath(__file__))
SESSIONS = os.path.expanduser("~/bench-homes/cc/.local/share/tazuna/sessions")
TASK = "rclone-archive-symlinks"
ANCHOR = "tazuna-own-v38@deepseek-v4.1-flash:max"
# real costs recorded in CHANGELOG.md from the sites' own counters (USD; yuan at 7 per USD; "paid" for Shannon's credit)
RECORDED = {"v98": 0.018, "v99": 0.033, "v106": 0.10, "v107": 0.012, "v108": 0.14, "v111": 0.0265, "v112": 0.069,
            "v114": 0.046, "v118": 0.026, "v119": 0.004, "v121": 0.0064, "v127": 0.009, "v129": 0.005, "v136": 0.17,
            "v138": 0.0044,
            # v167 and v168 ran together for $0.047 on the counter, split by tazuna's own estimates for each
            "v167": 0.026, "v168": 0.021}
SITE = {"tazuna-own": "CheaperInference", "tazuna-ownel": "Experiential Labs", "tazuna-ownbeat": "BeatAPI",
        "tazuna-ownuno": "UnoRouter", "tazuna-ownapx": "apinex", "tazuna-ownmrt": "Mortera", "tazuna-ownshn": "Shannon",
        "tazuna-ownshnds": "Shannon", "tazuna-ownsub": "SubRouter", "tazuna-ownsubm": "SubRouter",
        "tazuna-ownsubla": "SubRouter", "tazuna-ownsubl1": "SubRouter", "tazuna-ownsubl4": "SubRouter",
        "tazuna-ownsubhk": "SubRouter", "tazuna-ownsubgp": "SubRouter (Grok pool key)", "tazuna-ownsubmm": "SubRouter",
        "tazuna-ownsubx": "SubRouter", "codexsub": "SubRouter (Codex CLI)",
        "codex": "OpenAI plan (Codex CLI)", "tazuna-repo": "Anthropic plan"}


def base(h):
    return re.sub(r"-v\d+$", "", h.split("@")[0])


def call_speed(r):
    """Median output tok/s per call from the run's session log (the file whose output total is closest to the run's)."""
    files = [f for f in os.listdir(SESSIONS) if f"-{r['harness']}-" in f] if os.path.isdir(SESSIONS) else []
    best, rates = None, []
    for f in files:
        out, rs, t0 = 0, [], None
        for l in open(os.path.join(SESSIONS, f)):
            j = json.loads(l); e = j.get("e", {})
            if e.get("type") == "stream_event" and e["event"]["type"] == "message_start":
                t0 = j.get("t")
            elif e.get("type") == "stream_event" and e["event"]["type"] == "message_delta" and t0 is not None:
                o = e["event"]["usage"].get("output_tokens") or 0; out += o
                if o > 50 and j["t"] > t0:
                    rs.append(o / ((j["t"] - t0) / 1000))
                t0 = None
        if best is None or abs(out - r["out"]) < best:
            best, rates = abs(out - r["out"]), rs
    return round(statistics.median(rates), 1) if rates else None


def main():
    judge, anchor = {}, {}
    for f in sorted(glob.glob(os.path.join(HERE, "judge.oct.*.jsonl")), key=os.path.getmtime):
        rows = [json.loads(l) for l in open(f) if l.strip().startswith("{")]
        a = next((x["overall"] for x in rows if x.get("task") == TASK and x["harness"] == ANCHOR), None)
        for x in rows:
            if x.get("task") == TASK and "overall" in x:
                judge[(x["harness"], x.get("run", 0))], anchor[(x["harness"], x.get("run", 0))] = x["overall"], a
    real, billed = {}, {}
    if os.path.exists(os.path.join(HERE, "sub-summary.json")):
        # only runs the usage export covers whole (a logging gap cut v165-v168 short)
        subs = [s for s in json.load(open(os.path.join(HERE, "sub-summary.json")))
                if s.get("usd") is not None and sum(s["routes"].values()) >= 0.8 * s["calls"] and not s.get("overlap")]
        real = {s["label"]: s["usd"] for s in subs}
        billed = {s["label"]: s for s in subs}  # SubRouter's billed tokens (tazuna's own log has no cache writes)
    runs = []
    for l in open(os.path.join(HERE, "results-tasks-oct.jsonl")):
        r = json.loads(l)
        if r["task"] != TASK or base(r["harness"]) not in SITE:
            continue
        label = re.search(r"-(v\d+)@", r["harness"])
        if label and label[1] in billed:
            t = billed[label[1]]; r.update(inp=t["input"], cache_r=t["cache_read"], cache_w=t["cache_write"], out=t["output"])
        tot = r["inp"] + r["cache_r"] + r["cache_w"]
        if label and label[1] in real:
            usd, src = real[label[1]], "real"
        elif label and label[1] in RECORDED:
            usd, src = RECORDED[label[1]], "real"
        elif base(r["harness"]) == "codex":
            usd, src = 0.0, "plan"
        else:
            usd, src = r.get("reported_cost") or r.get("cost"), "est"
        k = (r["harness"], r["run"])
        runs.append(dict(site=SITE[base(r["harness"])], model=r["model"], effort=r.get("effort"), harness=r["harness"],
                         run=r["run"], tests=f"{r['passed']}/{r['total']}", passed=r["passed"], turns=r["turns"],
                         minutes=round(r["secs"] / 60, 1), input=r["inp"], cache_read=r["cache_r"], cache_write=r["cache_w"],
                         output=r["out"], cached=round(r["cache_r"] / tot, 3) if tot else None, usd=round(usd, 4), usd_source=src,
                         run_tok_s=round(r["out"] / r["secs"], 1) if r["secs"] else None, call_tok_s=call_speed(r),
                         judge=judge.get(k), anchor=anchor.get(k)))
    groups = collections.defaultdict(list)
    for r in runs:
        groups[(r["site"], r["model"], r["effort"])].append(r)  # max and xhigh apart: they judge differently
    out = []
    for (site, model, _), rs in groups.items():
        best = max(rs, key=lambda r: (r["judge"] or -1, r["passed"], -r["usd"]))
        mean = lambda k: round(statistics.mean(x[k] for x in rs if x[k] is not None), 3) if any(x[k] is not None for x in rs) else None
        out.append(dict(site=site, model=model, runs=len(rs), best=best,
                        mean={k: mean(k) for k in ("judge", "passed", "usd", "minutes", "turns", "cached", "run_tok_s", "call_tok_s",
                                                    "input", "cache_read", "cache_write", "output")}))
    out.sort(key=lambda g: (-(g["best"]["judge"] or -1), g["best"]["usd"]))
    json.dump(dict(runs=runs, groups=out), open(os.path.join(HERE, "site-summary.json"), "w"), indent=1, ensure_ascii=False)
    k = lambda n: f"{n / 1e6:.2f}M" if n >= 1e6 else f"{n / 1e3:.0f}K"
    lines = ["| site | model | effort | runs | best run | judge (anchor) | tests | $ | in / cache read / write / out | cached | min | turns | run tok/s | call tok/s | avg judge | avg $ | avg min |",
             "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|"]
    for g in out:
        b, m = g["best"], g["mean"]
        lines.append(f"| {g['site']} | {g['model']} | {b['effort']} | {g['runs']} | {re.sub(r'@.*', '', b['harness'])} r{b['run']} | "
                     f"{b['judge'] if b['judge'] is not None else '—'} ({b['anchor'] if b['anchor'] is not None else '—'}) | {b['tests']} | "
                     f"{'plan' if b['usd_source'] == 'plan' else format(b['usd'], '.3f') + ('' if b['usd_source'] == 'real' else ' est')} | {k(b['input'])} / {k(b['cache_read'])} / {k(b['cache_write'])} / {k(b['output'])} | "
                     f"{format(b['cached'], '.0%') if b['cached'] is not None else '—'} | {b['minutes']} | {b['turns']} | {b['run_tok_s']} | {b['call_tok_s'] or '—'} | "
                     f"{m['judge'] if m['judge'] is not None else '—'} | {m['usd']:.3f} | {m['minutes']} |")
    open(os.path.join(HERE, "site-summary.md"), "w").write("\n".join(lines) + "\n")
    print("\n".join(lines))


if __name__ == "__main__":
    main()
