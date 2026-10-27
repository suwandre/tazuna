#!/usr/bin/env python3
"""Which SubRouter provider keeps its cache over a long run? (2026-10-26)

LingAI, the provider the route key picks for deepseek-v4.1-flash-1017, missed the cache on ~15% of round9's large
calls, and those misses made ~71% of the round's cost (two upstream backends with their own caches). The vetting check
sends 6 short calls and was wrong both ways. This replays a real run's history the way an agent grows it (every second
turn, from ~30K to ~110K tokens) through one provider per test key (tazuna-vet-1..4, four providers at once), then reads
the account log for what each call cached and cost. Output is capped (max_tokens) so the calls cost input only.

CACHE_CONC=4 (2026-10-27): each provider gets four conversations at once, from four different runs, with replies up to
2000 tokens: a6aa cached 98% with one conversation and 22%, at ~100 s a call, under seven real runs at once. Reports
seconds per call and output tokens per second too.

    cacheprobe.py PROVIDER [PROVIDER ...]   -> cacheprobe.jsonl, and a summary per provider
"""
import concurrent.futures as cf, json, os, sys, time, urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE)); sys.path.insert(0, str(HERE / "realwork"))
import subrouter as sr  # noqa: E402
import sigprobe as sp  # noqa: E402

MODEL, FIRST, LAST, STEP = "deepseek-v4.1-flash-1017", 20, 80, 2
CONC = int(os.environ.get("CACHE_CONC", "1"))
RUNS = ["v395", "v396", "v397", "v399"][:max(CONC, 1)]  # prometheus runs of 83-112 turns: one conversation each
MAX_OUT = 2000 if CONC > 1 else 64


def body(run, cut):
    cwd, prompt, hist = sp.history(run, cut)
    return {"model": MODEL, "messages": [{"role": "system", "content": sp.system(cwd)}, {"role": "user", "content": prompt}, *hist],
            "tools": sp.tools(sp.BASH88, True), "reasoning_effort": "max", "max_tokens": MAX_OUT}


def conversation(var, bodies):
    rows = []
    for cut, b in bodies:
        req = urllib.request.Request("https://subrouter.ai/v1/chat/completions", json.dumps(b).encode(),
                                     {"Authorization": f"Bearer {os.environ[var]}", "Content-Type": "application/json", "User-Agent": "curl/8"})
        t = time.time()
        try:
            with urllib.request.urlopen(req, timeout=600) as r:
                u = json.load(r).get("usage") or {}
            rows.append(dict(cut=cut, secs=round(time.time() - t, 1), prompt=u.get("prompt_tokens"), out=u.get("completion_tokens"),
                             cached=(u.get("prompt_tokens_details") or {}).get("cached_tokens", u.get("prompt_cache_hit_tokens"))))
        except Exception as e:
            rows.append(dict(cut=cut, secs=round(time.time() - t, 1), error=str(e)[:200]))
    return rows


def probe(slot, slug, convs):
    var = f"SUBROUTER_VET{slot}_KEY"
    kid = sr.key(f"tazuna-vet-{slot}", MODEL, [slug], var)
    t0 = int(time.time())
    with cf.ThreadPoolExecutor(len(convs)) as ex:
        rows = [r for rs in ex.map(lambda bs: conversation(var, bs), convs) for r in rs]
    time.sleep(30)  # the account log lags the calls
    log = [c for c in sr.calls((int(time.time()) - t0) // 60 + 2, MODEL) if int(c["token_id"]) == kid and int(c["created_at"]) >= t0]
    served = sorted({json.loads(c.get("other") or "{}").get("provider_slug", "?") for c in log})
    inp = sum(int(c["prompt_tokens"]) for c in log)
    cached = sum(json.loads(c.get("other") or "{}").get("cache_tokens") or 0 for c in log)
    warm = log[len(convs):]  # each conversation's first call is cold everywhere
    misses = sum(1 for c in warm if (json.loads(c.get("other") or "{}").get("cache_tokens") or 0) < 0.5 * int(c["prompt_tokens"]))
    quota = sum(int(c["quota"]) for c in log)
    ok = sorted(r["secs"] for r in rows if "error" not in r)
    tps = sum(r.get("out") or 0 for r in rows if "error" not in r) / max(sum(ok), 1)
    return dict(provider=slug, conc=len(convs), served=served, calls=len(log), errors=sum("error" in r for r in rows), cached_pct=round(100 * cached / max(inp, 1)),
                warm_misses=misses, median_secs=ok[len(ok) // 2] if ok else None, out_tok_per_s=round(tps, 1),
                usd=round(quota / sr.QUOTA_PER_USD, 4), usd_per_mtok=round(quota / sr.QUOTA_PER_USD / max(inp, 1) * 1e6, 4), rows=rows)


def main():
    slugs = sys.argv[1:]
    convs = [[(c, body(run, c)) for c in range(FIRST, LAST + 1, STEP)] for run in RUNS]
    out = HERE / "cacheprobe.jsonl"
    with cf.ThreadPoolExecutor(4) as ex:
        # four test keys: provider i goes to key (i % 4) + 1, one provider per key at a time
        slots = {s: i % 4 + 1 for i, s in enumerate(slugs)}
        pending = list(slugs)
        running = {}
        while pending or running:
            for s in list(pending):
                if slots[s] not in [slots[r] for r in running]:
                    running[s] = ex.submit(probe, slots[s], s, convs); pending.remove(s)
            done, _ = cf.wait(list(running.values()), return_when=cf.FIRST_COMPLETED)
            for s, f in list(running.items()):
                if f in done:
                    r = f.result(); del running[s]
                    with out.open("a") as fh:
                        fh.write(json.dumps({**r, "at": time.strftime("%Y-%m-%d %H:%M")}) + "\n")
                    print(f"{r['provider']:10} x{r['conc']} served {r['served']} calls {r['calls']} errors {r['errors']} cached {r['cached_pct']}% "
                          f"warm misses {r['warm_misses']} median {r['median_secs']} s, {r['out_tok_per_s']} tok/s, cost ${r['usd']} (${r['usd_per_mtok']}/M input)", flush=True)


if __name__ == "__main__":
    main()
