#!/usr/bin/env python3
"""SubRouter account API for the bench (2026-10-25): what SubRouter's console and CSV exports showed by hand.

    subrouter.py log [MINUTES] [--model M]   calls per 10 minutes: provider, cached share, cost (default: last 180 min)
    subrouter.py providers MODEL             the model's smart-routing providers and their list prices
    subrouter.py keys                        the account's model API keys: id, name, model and provider limits (never the key)
    subrouter.py vet MODEL [--all]           check each provider of MODEL (up to 3x the cheapest input price) through the
                                             key tazuna-vet, record it in subrouter-vetted.json and limit the route key
                                             tazuna-route-MODEL to those that pass (--all: also those checked within 3 days)
    subrouter.py watch MODEL [--key NAME --var ENV]  during runs on the route key (or the key NAME, its value in ENV):
                                             drop a provider from it when its last 15 warm calls cached under 70% (runs
                                             until killed). NOT RELIABLE with several runs on one key: the account log
                                             does not say which run a call belongs to, and replaying 2026-10-25's log
                                             it would still drop LingAI at 42% (88% over the day). Not used by the bench;
                                             the per-run cache rate inside tazuna (0.0.79 routes) is the sound signal.

Credentials: ~/.subrouter-account (SUBROUTER_ACCESS_TOKEN, SUBROUTER_USER_ID; mode 600). The access token controls the
account: it is sent only to subrouter.ai, never printed. Writes touch only keys named tazuna-vet and tazuna-route-*;
their values go to ~/.subrouter-key (SUBROUTER_VET_KEY, SUBROUTER_ROUTE_<MODEL>_KEY), never to the screen. One smart key picks the cheapest subscribed provider per call (a6aa, cheapest for DeepSeek 1017, cached 13-22%;
PixStudio cost 5x), and a key can be limited per model to providers (`subrouter_model_providers`): the routes tazuna
switches between (docs/ROUTES.md).
"""
import json, os, re, subprocess, sys, time, urllib.request
from collections import defaultdict
from pathlib import Path

BASE = "https://subrouter.ai"
QUOTA_PER_USD = 500_000


def creds():
    env = {}
    for line in open(os.path.expanduser("~/.subrouter-account")):
        k, _, v = line.strip().partition("=")
        env[k] = v
    return env["SUBROUTER_ACCESS_TOKEN"], env["SUBROUTER_USER_ID"]


def api(path, method="GET", body=None):
    token, user = creds()
    req = urllib.request.Request(BASE + path, method=method, data=None if body is None else json.dumps(body).encode(),
                                 headers={"Authorization": f"Bearer {token}", "New-Api-User": user, "Content-Type": "application/json",
                                          "Accept": "application/json", "User-Agent": "tazuna-bench/1"})
    with urllib.request.urlopen(req, timeout=30) as r:
        d = json.load(r)
    if not d.get("success"):  # some business errors come back as HTTP 200
        sys.exit(f"{path}: {d.get('message')}")
    return d["data"]


def calls(minutes, model=None):
    """Consumption log entries of the last `minutes`, oldest first."""
    since, out, page = int(time.time()) - minutes * 60, [], 1
    while True:
        q = f"/api/log/self?type=2&p={page}&page_size=100&start_timestamp={since}" + (f"&model_name={model}" if model else "")
        items = api(q)["items"] or []
        out += [i for i in items if int(i["created_at"]) >= since]
        if len(items) < 100 or int(items[-1]["created_at"]) < since:
            return sorted(out, key=lambda i: int(i["created_at"]))
        page += 1


def log(minutes=180, model=None):
    win = defaultdict(lambda: [0, 0, 0, 0, 0])  # calls, input, cached, output, quota
    for c in calls(minutes, model):
        o = json.loads(c.get("other") or "{}")
        t = time.strftime("%H:%M", time.localtime(int(c["created_at"]) // 600 * 600))
        w = win[(t, c["model_name"], o.get("provider_slug") or c.get("group", ""))]
        for i, v in enumerate((1, int(c["prompt_tokens"]), o.get("cache_tokens") or 0, int(c["completion_tokens"]), int(c["quota"]))):
            w[i] += v
    for (t, m, p), (n, i, cr, out, q) in sorted(win.items()):
        print(f"{t}  {m:28} {p:14} {n:4} calls  {100 * cr // max(i, 1):3}% cached  {out // 1000:5}K out  "
              f"${q / QUOTA_PER_USD:.3f}  ${q / QUOTA_PER_USD / n:.5f}/call")


def providers(model):
    m = next((m for m in api("/api/user/self/subrouter/models") if m["model_name"] == model), None)
    if not m:
        sys.exit(f"{model}: not a smart-routing model")
    print(f"{model}: {m['provider_count']} providers, prices in {m['price_currency']} per M (input / output)")
    for p in sorted(m["providers"], key=lambda p: (p["min_input_price"], p["min_output_price"])):
        print(f"  {p['provider_slug']:16} {p['provider_name'][:24]:24} {p['min_input_price']:8.4f} / {p['min_output_price']:8.4f}")


def keys():
    d = api("/api/token/?p=1&page_size=100")
    for k in (d["items"] if isinstance(d, dict) else d):
        limits = k.get("model_limits") if k.get("model_limits_enabled") else "all models"
        print(f"  {k['id']:8} {k['name'][:36]:36} status {k.get('status')}  models: {str(limits)[:50]}  "
              f"providers: {k.get('subrouter_model_providers') or '-'}")


HERE = Path(__file__).resolve().parent
VETTED = HERE / "subrouter-vetted.json"
KEYFILE = Path.home() / ".subrouter-key"


def env_name(model):
    return "SUBROUTER_ROUTE_" + re.sub(r"[^A-Z0-9]+", "_", model.upper()).strip("_") + "_KEY"


def key(name, model, slugs, var, cache_weight=True):
    """Creates the key `name` (one model, these providers) or updates its providers; its value goes to ~/.subrouter-key as
    `var`. Only keys this script names are touched."""
    assert re.fullmatch(r"tazuna-vet(-\d)?", name) or name.startswith("tazuna-route-")
    d = api("/api/token/?p=1&page_size=100")
    old = next((k for k in (d["items"] if isinstance(d, dict) else d) if k["name"] == name), None)
    limits = {"model_limits_enabled": True, "model_limits": model,
              "subrouter_model_providers": json.dumps({model: slugs}), "subrouter_providers": ",".join(slugs),
              "include_official_channels": False, "include_shared_subscriptions": False, "include_cache_hit_rate": cache_weight}
    if old is None:
        api("/api/token/", "POST", {"name": name, "expired_time": -1, "unlimited_quota": True, **limits})
        d = api("/api/token/?p=1&page_size=100")
        old = next(k for k in (d["items"] if isinstance(d, dict) else d) if k["name"] == name)
    else:
        api("/api/token/", "PUT", {**api(f"/api/token/{old['id']}"), **limits})
    value = "sk-" + api(f"/api/token/{old['id']}")["key"]
    lines = [l for l in KEYFILE.read_text().splitlines() if not l.startswith(f"export {var}=")] if KEYFILE.exists() else []
    # written owner-only from the start (no window with default permissions), then moved over the old file
    tmp = KEYFILE.with_name(KEYFILE.name + ".tmp")
    fd = os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, "w") as f:
        f.write("\n".join(lines + [f"export {var}={value}"]) + "\n")
    os.replace(tmp, KEYFILE)
    os.environ[var] = value
    return old["id"]


def vet(model, again=False):
    """Each provider through tazuna-vet with uno_route.py's check without its cache test (ROUTE_CACHE_BLIND): an answer,
    a long reasoning call that comes back, identity; then the account log for where the calls really went and what they
    cost. The cache is judged on real calls by `watch`: the check's 6 short calls were wrong both ways (FoxRouter 99% in
    the check and 40-73% in the run that followed; LingAI 15% in the check, 78-88% in runs; 2026-10-24/25)."""
    m = next(m for m in api("/api/user/self/subrouter/models") if m["model_name"] == model)
    cheapest = min(p["min_input_price"] for p in m["providers"])
    done = json.loads(VETTED.read_text()) if VETTED.exists() else {}
    seen = done.setdefault(model, {})
    todo = [p for p in sorted(m["providers"], key=lambda p: p["min_input_price"]) if p["min_input_price"] <= 3 * cheapest
            and (again or seen.get(p["provider_slug"], {}).get("verdict") in ("error", None)
                 or seen[p["provider_slug"]].get("check") != "blind" or seen[p["provider_slug"]].get("price") != p["min_input_price"]
                 or time.time() - seen[p["provider_slug"]].get("at", 0) >= 3 * 86400)]
    lock = __import__("threading").Lock()

    def one(i, p):
        # four test keys (tazuna-vet-1..4) check four providers at once: a check takes 2-30 minutes
        slug, last, var = p["provider_slug"], seen.get(p["provider_slug"], {}), f"SUBROUTER_VET{i}_KEY"
        with lock:
            vid = key(f"tazuna-vet-{i}", model, [slug], var)
        t0 = int(time.time())
        r = subprocess.run([sys.executable, "-I", str(HERE / "uno_route.py"), model, "1"], capture_output=True, text=True,
                           env={**os.environ, "ROUTE_API": BASE, "ROUTE_KEY_ENV": var, "ROUTE_CACHE_BLIND": "1"}, timeout=1800)
        out = r.stdout + r.stderr
        time.sleep(20)  # the log lags the calls a little
        cs = [c for c in calls((int(time.time()) - t0) // 60 + 2, model) if int(c["token_id"]) == vid and int(c["created_at"]) >= t0]
        served = sorted({json.loads(c.get("other") or "{}").get("provider_slug", "?") for c in cs})
        # only uno_route.py's own success line counts (anchored), never text a reply could contain
        cache = re.search(r"^    cache (\d+)% .*, identity ok$", r.stdout, re.M)
        ok = r.returncode == 0 and cache and not re.search(r"^\s+no: ", r.stdout, re.M)
        verdict = "pass" if ok and served == [slug] else "fail"
        # a transport error says nothing about the provider: checked again on the next run (a night of resets and 400s
        # that a rerun the next morning did not show, 2026-10-25)
        if verdict == "fail" and re.search(r"Connection reset|HTTP Error (400|429|5\d\d)|timed out|Remote end closed", out):
            verdict = "error"
        with lock:
            seen[slug] = {**last, "at": t0, "check": "blind", "price": p["min_input_price"], "out_price": p["min_output_price"],
                          "verdict": verdict, "served_by": served, "calls": len(cs),
                          "usd": round(sum(int(c["quota"]) for c in cs) / QUOTA_PER_USD, 4),
                          "why": next((l.strip() for l in out.splitlines() if "no:" in l), "")[:160]}
            VETTED.write_text(json.dumps(done, indent=1, sort_keys=True) + "\n")
            print(f"{slug:14} {verdict}  served by {served}  ${seen[slug]['usd']}  {seen[slug]['why']}", flush=True)

    from concurrent.futures import ThreadPoolExecutor
    with ThreadPoolExecutor(4) as ex:
        free = __import__("queue").Queue()
        for i in range(1, 5):
            free.put(i)

        def run(p):
            i = free.get()
            try:
                one(i, p)
            finally:
                free.put(i)
        list(ex.map(run, todo))
    good = [s for s, v in sorted(seen.items(), key=lambda kv: kv[1].get("price", 1e9)) if v.get("verdict") == "pass"]
    if good:
        key(f"tazuna-route-{model}", model, good, env_name(model))
    print(f"route key tazuna-route-{model}: {good or 'unchanged, no provider passed'} (in ~/.subrouter-key as {env_name(model)})")


def watch(model, floor=70, window=15, every=60, name=None, var=None, keep=3):
    """Reads the account log once a minute for the route key's calls; a provider whose last `window` warm calls cached
    under `floor`% leaves the key (never below `keep` providers), and SubRouter routes the next call to another one. Only
    warm calls count: one the same provider served the previous turn of (an earlier call within 10 minutes whose prompt
    is up to 40K tokens smaller). Counting every call dropped 9 of 10 providers in an hour (2026-10-25): six runs share
    the key, and after each move every run's first call on the new provider is cold. The real rates go into
    subrouter-vetted.json ("live"); a dropped provider is marked so the next vet checks it again."""
    name, var = name or f"tazuna-route-{model}", var or env_name(model)
    d = api("/api/token/?p=1&page_size=100")
    route = next(k for k in (d["items"] if isinstance(d, dict) else d) if k["name"] == name)
    since = int(time.time())
    print(f"{time.strftime('%H:%M')} watching {name}: {json.loads(route['subrouter_model_providers'] or '{}').get(model)}", flush=True)
    while True:
        time.sleep(every)
        slugs = json.loads(api(f"/api/token/{route['id']}")["subrouter_model_providers"] or "{}").get(model, [])
        per = defaultdict(list)
        for c in calls((int(time.time()) - since) // 60 + 2, model):
            if int(c["token_id"]) == route["id"] and int(c["created_at"]) >= since:
                o = json.loads(c.get("other") or "{}")
                p, t, n = o.get("provider_slug", "?"), int(c["created_at"]), int(c["prompt_tokens"])
                warm = any(t - t2 <= 600 and n - 40_000 <= n2 <= n for t2, n2, _ in per[p])
                per[p].append((t, n, (o.get("cache_tokens") or 0) if warm else None))
        done = json.loads(VETTED.read_text()) if VETTED.exists() else {}
        seen = done.setdefault(model, {})
        for slug, cs in per.items():
            last = [(n, c) for _, n, c in cs if c is not None][-window:]
            rate = 100 * sum(c for _, c in last) // max(1, sum(n for n, _ in last))
            seen.setdefault(slug, {})["live"] = {"at": int(time.time()), "calls": len(cs), "warm": len(last), f"cached_last_{window}": rate}
            if len(last) >= window and rate < floor and slug in slugs and len(slugs) > keep:
                slugs = [s for s in slugs if s != slug]
                key(name, model, slugs, var, route.get("include_cache_hit_rate", True))
                seen[slug].update(verdict="dropped", at=int(time.time()))
                print(f"{time.strftime('%H:%M')} dropped {slug}: {rate}% cached over its last {window} calls; route now {slugs}", flush=True)
        VETTED.write_text(json.dumps(done, indent=1, sort_keys=True) + "\n")


if __name__ == "__main__":
    a = sys.argv[1:] or ["log"]
    model = a[a.index("--model") + 1] if "--model" in a else None
    {"log": lambda: log(int(a[1]) if len(a) > 1 and a[1].isdigit() else 180, model),
     "providers": lambda: providers(a[1]), "keys": keys, "vet": lambda: vet(a[1], "--all" in a),
     "watch": lambda: watch(a[1], name=a[a.index("--key") + 1] if "--key" in a else None,
                            var=a[a.index("--var") + 1] if "--var" in a else None)}[a[0]]()
