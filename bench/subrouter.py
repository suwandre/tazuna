#!/usr/bin/env python3
"""SubRouter account API for the bench (2026-10-25): what SubRouter's console and CSV exports showed by hand.

    subrouter.py log [MINUTES] [--model M]   calls per 10 minutes: provider, cached share, cost (default: last 180 min)
    subrouter.py providers MODEL             the model's smart-routing providers and their list prices
    subrouter.py keys                        the account's model API keys: id, name, model and provider limits (never the key)

Credentials: ~/.subrouter-account (SUBROUTER_ACCESS_TOKEN, SUBROUTER_USER_ID; mode 600). The access token controls the
account: it is sent only to subrouter.ai, never printed; this script only reads, except `keys` writes added later on
purpose. One smart key picks the cheapest subscribed provider per call (a6aa, cheapest for DeepSeek 1017, cached 13-22%;
PixStudio cost 5x), and a key can be limited per model to providers (`subrouter_model_providers`): the routes tazuna
switches between (docs/ROUTES.md).
"""
import json, os, sys, time, urllib.request
from collections import defaultdict

BASE = "https://subrouter.ai"
QUOTA_PER_USD = 500_000


def creds():
    env = {}
    for line in open(os.path.expanduser("~/.subrouter-account")):
        k, _, v = line.strip().partition("=")
        env[k] = v
    return env["SUBROUTER_ACCESS_TOKEN"], env["SUBROUTER_USER_ID"]


def api(path):
    token, user = creds()
    req = urllib.request.Request(BASE + path, headers={"Authorization": f"Bearer {token}", "New-Api-User": user,
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


if __name__ == "__main__":
    a = sys.argv[1:] or ["log"]
    model = a[a.index("--model") + 1] if "--model" in a else None
    {"log": lambda: log(int(a[1]) if len(a) > 1 and a[1].isdigit() else 180, model),
     "providers": lambda: providers(a[1]), "keys": keys}[a[0]]()
