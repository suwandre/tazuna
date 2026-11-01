#!/usr/bin/env python3
"""Fresh SubRouter routes for the models tazuna uses. Provider prices change from day to day (GPT-6.1 Sol's cheapest went
from the shared OpenAI Pro pools to Kriora within days, 2026-11-01), so a route vetted once goes stale. For each model:
today's providers and prices, the cheapest few checked like a real run (chatprobe.py: four agent-like conversations; GPT
models on the Responses route with respprobe.py), the route key tazuna-route-<model> set to every one that passes
(cached 70%+, at most two errors, calls served where asked); SubRouter then picks the cheapest of them call by call. The
result goes to ~/.local/share/tazuna/routes.json, which tazuna's model picker reads (no credentials in it).

SubRouter labels some price lists CNY, but its billing counts those numbers as USD (DeepSeek on LingAI, listed 0.0882
"CNY" per M output, billed $0.09 per M, 2026-11-01), so they are taken as USD here.

  routes.py refresh [MODEL ...]   default: MODELS; about $0.01-0.10 a provider
  routes.py show                  routes.json as a table
"""
import json, re, subprocess, sys, time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import subrouter as s  # noqa: E402

MODELS = ["deepseek-v4.1-flash-1017", "gpt-6.1-sol", "gpt-6-luna", "gpt-6.1-terra"]
CANDIDATES = 4  # the cheapest four by input price (agent calls are mostly input)
OUT = Path.home() / ".local/share/tazuna/routes.json"
# effort levels the model really has (bench/effortprobe.py, 2026-11-01); others take low ... max
EFFORTS = {"deepseek-v4.1-flash-1017": ["high", "max"], "kimi-k3.1-pro": []}


def probe(model, slug, i):
    gpt = model.startswith("gpt-")
    cmd = ["respprobe.py", slug, str(i), model] if gpt else ["chatprobe.py", model, slug, str(i)]
    out = subprocess.run([sys.executable, "-I", str(HERE / cmd[0]), *cmd[1:]], capture_output=True, text=True, timeout=3600).stdout
    m = re.search(r"(\d+) ok, (\d+) errors,? .*?cached (\d+)%", out)
    served = re.findall(r"served by \[([^\]]*)\]", out)
    served = [x.strip(" '") for x in served[-1].split(",")] if served else []
    ok, errors, cached = (int(g) for g in m.groups()) if m else (0, 99, 0)
    # a shared pool is served as "openai-pro" in the account log
    where = served == [slug] or (slug.startswith("shared-") and served == ["openai-pro"])
    return dict(slug=slug, ok=ok, errors=errors, cached=cached, served=served, passed=ok > 0 and errors <= 2 and cached >= 70 and where)


def refresh(models):
    routes = json.loads(OUT.read_text()) if OUT.exists() else {"routes": {}}
    listed = {m["model_name"]: m for m in s.api("/api/user/self/subrouter/models")}
    for model in models:
        if model not in listed:
            print(f"{model}: not on SubRouter")
            continue
        provs = sorted(listed[model]["providers"], key=lambda p: (p["min_input_price"], p["min_output_price"]))[:CANDIDATES]
        print(f"{model}: checking {', '.join(p['provider_slug'] for p in provs)}", flush=True)
        with ThreadPoolExecutor(4) as ex:  # one test key (tazuna-vet-1..4) each
            checks = list(ex.map(lambda ip: probe(model, ip[1]["provider_slug"], ip[0] + 1), enumerate(provs)))
        price = {p["provider_slug"]: (round(p["min_input_price"], 5), round(p["min_output_price"], 5)) for p in provs}
        for c in checks:
            print(f"  {c['slug']:22} {'pass' if c['passed'] else 'fail'}  cached {c['cached']}%  errors {c['errors']}  served by {c['served']}"
                  f"  ${price[c['slug']][0]}/${price[c['slug']][1]} per M", flush=True)
        good = [c["slug"] for c in checks if c["passed"]]
        if not good:
            print(f"  no provider passed: the route key keeps its providers")
            continue
        name, var = "tazuna-route-" + re.sub(r"[^a-z0-9]+", "-", model).strip("-"), s.env_name(model)
        kid = s.key(name, model, good, var)
        k = s.api(f"/api/token/{kid}")
        shared = any(g.startswith("shared-") for g in good)
        if bool(k.get("include_shared_subscriptions")) != shared:
            s.api("/api/token/", "PUT", {**k, "include_shared_subscriptions": shared})
        cheapest = min(good, key=lambda g: price[g])
        routes["routes"][model] = dict(via="subrouter", url="https://subrouter.ai/v1", key_env=var, providers=good,
                                       price=list(price[cheapest]), cached=max(c["cached"] for c in checks if c["passed"]),
                                       efforts=EFFORTS.get(model, ["low", "medium", "high", "xhigh", "max"]), vetted_at=int(time.time()))
        print(f"  route {name} ({var}): {good}", flush=True)
    routes["updated"] = int(time.time())
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(routes, indent=1, sort_keys=True) + "\n")
    show()


def show():
    routes = json.loads(OUT.read_text())["routes"] if OUT.exists() else {}
    for model, r in sorted(routes.items()):
        age = (time.time() - r["vetted_at"]) / 86400
        print(f"{model:28} {','.join(r['providers'])[:40]:40} in ${r['price'][0]}/M out ${r['price'][1]}/M  cached {r['cached']}%"
              f"  efforts {'/'.join(r['efforts']) or 'none'}  vetted {age:.1f} days ago")


if __name__ == "__main__":
    a = sys.argv[1:]
    if a[:1] == ["refresh"]:
        refresh(a[1:] or MODELS)
    elif a[:1] == ["show"]:
        show()
    else:
        sys.exit(__doc__)
