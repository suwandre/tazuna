#!/usr/bin/env python3
"""SubRouter account API for the bench (2026-10-25): what SubRouter's console and CSV exports showed by hand.

    subrouter.py log [MINUTES] [--model M]   calls per 10 minutes: provider, cached share, cost (default: last 180 min)
    subrouter.py providers MODEL             the model's smart-routing providers and their list prices
    subrouter.py keys                        the account's model API keys: id, name, model and provider limits (never the key)
    subrouter.py vet MODEL [--all]           check each provider of MODEL (up to 3x the cheapest input price) through the
                                             key tazuna-vet, record it in subrouter-vetted.json and limit the route key
                                             tazuna-route-MODEL to those that pass (--all: also those checked within 3 days)

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


def key(name, model, slugs, var):
    """Creates the key `name` (one model, these providers) or updates its providers; its value goes to ~/.subrouter-key as
    `var`. Only keys this script names are touched."""
    assert name == "tazuna-vet" or name.startswith("tazuna-route-")
    d = api("/api/token/?p=1&page_size=100")
    old = next((k for k in (d["items"] if isinstance(d, dict) else d) if k["name"] == name), None)
    limits = {"model_limits_enabled": True, "model_limits": model,
              "subrouter_model_providers": json.dumps({model: slugs}), "subrouter_providers": ",".join(slugs),
              "include_official_channels": False, "include_shared_subscriptions": False, "include_cache_hit_rate": True}
    if old is None:
        api("/api/token/", "POST", {"name": name, "expired_time": -1, "unlimited_quota": True, **limits})
        d = api("/api/token/?p=1&page_size=100")
        old = next(k for k in (d["items"] if isinstance(d, dict) else d) if k["name"] == name)
    else:
        api("/api/token/", "PUT", {**api(f"/api/token/{old['id']}"), **limits})
    value = "sk-" + api(f"/api/token/{old['id']}")["key"]
    lines = [l for l in KEYFILE.read_text().splitlines() if not l.startswith(f"export {var}=")] if KEYFILE.exists() else []
    KEYFILE.write_text("\n".join(lines + [f"export {var}={value}"]) + "\n")
    KEYFILE.chmod(0o600)
    os.environ[var] = value
    return old["id"]


def vet(model, again=False):
    """Each provider through tazuna-vet with uno_route.py's check (cache over a growing conversation, a long reasoning
    call, identity), then the account log for where the calls really went and what they cost."""
    m = next(m for m in api("/api/user/self/subrouter/models") if m["model_name"] == model)
    cheapest = min(p["min_input_price"] for p in m["providers"])
    done = json.loads(VETTED.read_text()) if VETTED.exists() else {}
    seen = done.setdefault(model, {})
    for p in sorted(m["providers"], key=lambda p: p["min_input_price"]):
        slug, last = p["provider_slug"], seen.get(p["provider_slug"], {})
        if p["min_input_price"] > 3 * cheapest:
            continue
        if not again and time.time() - last.get("at", 0) < 3 * 86400 and last.get("price") == p["min_input_price"]:
            continue
        vid = key("tazuna-vet", model, [slug], "SUBROUTER_VET_KEY")
        t0 = int(time.time())
        r = subprocess.run([sys.executable, "-I", str(HERE / "uno_route.py"), model, "1"], capture_output=True, text=True,
                           env={**os.environ, "ROUTE_API": BASE, "ROUTE_KEY_ENV": "SUBROUTER_VET_KEY"}, timeout=1800)
        out = r.stdout + r.stderr
        time.sleep(20)  # the log lags the calls a little
        cs = [c for c in calls((int(time.time()) - t0) // 60 + 2, model) if int(c["token_id"]) == vid]
        served = sorted({json.loads(c.get("other") or "{}").get("provider_slug", "?") for c in cs})
        cache = re.search(r"cache (\d+)%", out)
        verdict = "pass" if "identity ok" in out and cache and int(cache[1]) >= 70 and served == [slug] else "fail"
        seen[slug] = {"at": t0, "price": p["min_input_price"], "out_price": p["min_output_price"], "verdict": verdict,
                      "cache": int(cache[1]) if cache else None, "served_by": served, "calls": len(cs),
                      "usd": round(sum(int(c["quota"]) for c in cs) / QUOTA_PER_USD, 4),
                      "why": next((l.strip() for l in out.splitlines() if "no:" in l), "")[:160]}
        VETTED.write_text(json.dumps(done, indent=1, sort_keys=True) + "\n")
        print(f"{slug:14} {verdict}  cache {seen[slug]['cache']}%  served by {served}  ${seen[slug]['usd']}  {seen[slug]['why']}", flush=True)
    good = [s for s, v in sorted(seen.items(), key=lambda kv: kv[1]["price"]) if v["verdict"] == "pass"]
    if good:
        key(f"tazuna-route-{model}", model, good, env_name(model))
    print(f"route key tazuna-route-{model}: {good or 'unchanged, no provider passed'} (in ~/.subrouter-key as {env_name(model)})")


if __name__ == "__main__":
    a = sys.argv[1:] or ["log"]
    model = a[a.index("--model") + 1] if "--model" in a else None
    {"log": lambda: log(int(a[1]) if len(a) > 1 and a[1].isdigit() else 180, model),
     "providers": lambda: providers(a[1]), "keys": keys, "vet": lambda: vet(a[1], "--all" in a)}[a[0]]()
