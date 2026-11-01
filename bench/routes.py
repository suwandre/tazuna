#!/usr/bin/env python3
"""Fresh SubRouter routes for the models tazuna uses. Provider prices change from day to day (GPT-6.1 Sol's cheapest went
from the shared OpenAI Pro pools to Kriora within days, 2026-11-01), so a route vetted once goes stale. For each model:
today's providers and prices, the cheapest few checked like a real run (chatprobe.py: four agent-like conversations; GPT
models on the Responses route with respprobe.py), the route key tazuna-route-<model> set to every one that passes
(cached 70%+, at most two errors, calls served where asked); SubRouter then picks the cheapest of them call by call. The
result goes to ~/.local/share/tazuna/routes.json, which tazuna's model picker reads (no credentials in it).

SubRouter labels some price lists CNY, but its billing counts those numbers as USD (DeepSeek on LingAI, listed 0.0882
"CNY" per M output, billed $0.09 per M, 2026-11-01), so they are taken as USD here.

  routes.py refresh [MODEL ...] [--budget USD]   default: MODELS, $0.40; stops spending at the budget
  routes.py pin [ROUTE ...]       a key per checked provider of each route, for pinning one in the picker (no model calls)
  routes.py catalog               the picker's list of every SubRouter model, bench scores, AA index (no model calls)
  routes.py show                  routes.json as a table
"""
import json, re, subprocess, sys, time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import subrouter as s  # noqa: E402

MODELS = ["deepseek-v4.1-flash-1017", "gpt-6.1-sol", "gpt-6-luna", "gpt-6.1-terra"]
# The quality gate: about QUALITY_N long-context questions (canary_lcr.py: whole document sets, so most of the input is
# read from cache; ~42 questions in 7 sets) through a provider that passed the run
# checks, at the effort AA measured; it passes at AA's LCR score minus QUALITY_MARGIN (40 questions: +-15 points). Without
# it the first refresh swapped DeepSeek's checked LingAI route for a provider listed at $0 and for RelAI, which caps max
# and got fewer answers right (2026-11-01). A provider's verdict holds VERIFIED_DAYS.
QUALITY_N, QUALITY_MARGIN, VERIFIED_DAYS = 40, 0.15, 30
AA_EFFORT = {"deepseek-v4.1-flash-1017": "max"}  # AA lists DeepSeek 1017 at max only; GPT models at each level (xhigh used)
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
    # what the account log billed for the check's calls: the same conversations on every provider, so comparable
    billed = re.findall(r"account log: (\d+) calls, .*\$([0-9.]+)", out)
    per_call = float(billed[-1][1]) / max(1, int(billed[-1][0])) if billed else None
    # a shared pool is served as "openai-pro" or "openai-plus" in the account log
    where = served == [slug] or (slug.startswith("shared-") and len(served) == 1 and served[0].startswith("openai-"))
    return dict(slug=slug, ok=ok, errors=errors, cached=cached, served=served, per_call=per_call,
                passed=ok > 0 and errors <= 2 and cached >= 70 and where)


def quality(model, slug, routes):
    """The long-context gate for one provider, through the test key tazuna-vet-1; remembered in routes.json."""
    seen = routes.setdefault("verified", {}).setdefault(model, {})
    v = seen.get(slug)
    if v and time.time() - v["at"] < VERIFIED_DAYS * 86400:
        return v["pass"]
    import canary_lcr
    effort = AA_EFFORT.get(model, "xhigh")
    aa = canary_lcr.aa_scores("lcr")
    base = model.replace(".", "-")
    ref = aa.get(f"{base}-{effort}") or aa.get(base)
    if not ref:
        print(f"  {slug}: no public LCR score for {model}: quality not checked, left out")
        return False
    vid = s.key("tazuna-vet-1", model, [slug], "SUBROUTER_VET1_KEY")
    k = s.api(f"/api/token/{vid}")
    if bool(k.get("include_shared_subscriptions")) != slug.startswith("shared-"):
        s.api("/api/token/", "PUT", {**k, "include_shared_subscriptions": slug.startswith("shared-")})
    rows = canary_lcr.run(model, "SUBROUTER_VET1_KEY", effort, QUALITY_N)
    acc = sum(r["ok"] for r in rows) / max(1, len(rows))
    seen[slug] = dict(at=int(time.time()), lcr=round(acc, 3), aa=ref, n=len(rows), pass_=None)
    seen[slug]["pass"] = acc >= ref - QUALITY_MARGIN
    del seen[slug]["pass_"]
    print(f"  {slug:22} quality {'pass' if seen[slug]['pass'] else 'FAIL'}: long context {100 * acc:.0f}% of {len(rows)} (AA {100 * ref:.0f}%)", flush=True)
    return seen[slug]["pass"]


def save(routes, model):
    """Into routes.json without losing what others wrote there since this refresh began (a pinned route like
    deepseek-v4.1-flash-1017@fast went missing once): this model's route and the check records are merged in."""
    cur = json.loads(OUT.read_text()) if OUT.exists() else {"routes": {}}
    if model:
        cur.setdefault("routes", {})[model] = routes["routes"][model]
    for k in ("verified", "checked"):
        for m, v in routes.get(k, {}).items():
            cur.setdefault(k, {}).setdefault(m, {}).update(v)
    cur["updated"] = int(time.time())
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(cur, indent=1, sort_keys=True) + "\n")


def pin(name, route, price):
    """A key of its own for each checked provider of a route with several (tazuna-route-<route>-<provider>, one provider
    each), so the picker can pin one without touching the route's key, which other sessions may be using. Keys cost
    nothing; their values go to ~/.subrouter-key like the route's."""
    route.pop("pinned", None)
    if len(route["providers"]) < 2:
        return
    base = "tazuna-route-" + re.sub(r"[^a-z0-9]+", "-", name).strip("-")
    route["pinned"] = {}
    for g in route["providers"]:
        var = s.env_name(f"{name}-{g}")
        kid = s.key(f"{base}-{re.sub(r'[^a-z0-9]+', '-', g).strip('-')}", route.get("model", name), [g], var)
        if g.startswith("shared-"):  # a shared subscription pool serves only keys that allow them
            s.api("/api/token/", "PUT", {**s.api(f"/api/token/{kid}"), "include_shared_subscriptions": True})
        route["pinned"][g] = dict(key_env=var, price=list(price.get(g, route["price"])))


def spent_fen():
    """The account's usage counter (fen, ~700 a dollar; sub_guard.sh reads the same)."""
    import os, urllib.request
    q = urllib.request.Request("https://subrouter.ai/v1/dashboard/billing/usage", headers={"Authorization": f"Bearer {os.environ['SUBROUTER_MODELS_KEY']}", "User-Agent": "curl/8"})
    return float(json.load(urllib.request.urlopen(q, timeout=30))["total_usage"])


def refresh(models, budget):
    """Checks providers listed under the route's cheapest verified one (all the way down, four at a time), keeps the
    verified ones that still pass, and stops spending at `budget` dollars: a quality test whose estimate (40 questions,
    ~3.8M input tokens at the listed price) would pass it is left for the next refresh."""
    routes = json.loads(OUT.read_text()) if OUT.exists() else {"routes": {}}
    listed = {m["model_name"]: m for m in s.api("/api/user/self/subrouter/models")}
    start = spent_fen()
    left = lambda: budget - (spent_fen() - start) / 700
    for model in models:
        if model not in listed:
            print(f"{model}: not on SubRouter")
            continue
        now = time.time()
        verdicts = routes.setdefault("verified", {}).get(model, {})
        ok_before = {g for g, v in verdicts.items() if v["pass"] and now - v["at"] < VERIFIED_DAYS * 86400}
        failed = {g for g, v in verdicts.items() if not v["pass"] and now - v["at"] < VERIFIED_DAYS * 86400}
        recent_fail = {g for g, v in routes.setdefault("checked", {}).setdefault(model, {}).items() if not v["passed"] and now - v["at"] < 7 * 86400}
        ranked = sorted(listed[model]["providers"], key=lambda p: (p["min_input_price"], p["min_output_price"]))
        price = {p["provider_slug"]: (round(p["min_input_price"], 5), round(p["min_output_price"], 5)) for p in ranked}
        ref = min((price[g][0] for g in ok_before if g in price), default=None)
        # cheaper than the cheapest verified provider (all of them if none is verified yet), plus the verified ones
        todo = [p["provider_slug"] for p in ranked if (ref is None or p["min_input_price"] < ref) and p["provider_slug"] not in failed | recent_fail]
        todo = (todo if ref is not None else todo[:2 * CANDIDATES]) + sorted(g for g in ok_before if g in price)
        todo = list(dict.fromkeys(todo))  # once each: LingAI listed at the verified price came in twice (2026-11-01)
        print(f"{model}: {len(todo)} providers to check (${left():.2f} of the budget left)", flush=True)
        checks = []
        for i in range(0, len(todo), 4):
            if left() <= 0.02:
                print(f"  budget spent: {', '.join(todo[i:])} wait for the next refresh")
                break
            with ThreadPoolExecutor(4) as ex:  # one test key (tazuna-vet-1..4) each
                batch = list(ex.map(lambda ip: probe(model, ip[1], ip[0] + 1), enumerate(todo[i:i + 4])))
            for c in batch:
                routes["checked"][model][c["slug"]] = dict(at=int(time.time()), passed=c["passed"], cached=c["cached"], errors=c["errors"])
                print(f"  {c['slug']:22} {'pass' if c['passed'] else 'fail'}  cached {c['cached']}%  errors {c['errors']}  served by {c['served']}"
                      f"  ${price[c['slug']][0]}/${price[c['slug']][1]} per M", flush=True)
            checks += batch
        good = [c["slug"] for c in checks if c["passed"]]
        # billed, not listed: cybersol listed DeepSeek at $0 and billed ~4x LingAI a call (2026-11-01). A provider whose
        # check cost over 1.5x the cheapest passing one's is left out (SubRouter would pick it by its listing)
        cost = {c["slug"]: c["per_call"] for c in checks if c["passed"] and c["per_call"]}
        if cost:
            floor = min(cost.values())
            for g in [g for g in good if cost.get(g, floor) > 1.5 * floor]:
                print(f"  {g:22} left out: billed ${cost[g]:.5f} a call, {cost[g] / floor:.1f}x the cheapest passing provider")
            good = [g for g in good if cost.get(g, floor) <= 1.5 * floor]
        passed = []
        for g in good:
            fresh = verdicts.get(g) and now - verdicts[g]["at"] < VERIFIED_DAYS * 86400
            # ~4M input tokens, about a sixth of them uncached once a set's documents are cached; ~120k output
            estimate = 0.7 * price[g][0] + 3.3 * price[g][0] * 0.1 + 0.12 * price[g][1]
            if not fresh and (price[g][0] == 0 or estimate > left()):
                print(f"  {g:22} quality test left for later ({'no listed price' if price[g][0] == 0 else f'~${estimate:.2f}, ${left():.2f} left'})")
                continue
            if quality(model, g, routes):
                passed.append(g)
        if not passed:
            print(f"  no provider passed: the route key keeps its providers")
            continue
        name, var = "tazuna-route-" + re.sub(r"[^a-z0-9]+", "-", model).strip("-"), s.env_name(model)
        kid = s.key(name, model, passed, var)
        k = s.api(f"/api/token/{kid}")
        shared = any(g.startswith("shared-") for g in passed)
        if bool(k.get("include_shared_subscriptions")) != shared:
            s.api("/api/token/", "PUT", {**k, "include_shared_subscriptions": shared})
        cheapest = min(passed, key=lambda g: cost.get(g) or price[g][0])
        routes["routes"][model] = dict(via="subrouter", url="https://subrouter.ai/v1", key_env=var, providers=passed,
                                       price=list(price[cheapest]), cached=max(c["cached"] for c in checks if c["slug"] in passed),
                                       efforts=EFFORTS.get(model, ["low", "medium", "high", "xhigh", "max"]), vetted_at=int(time.time()))
        pin(model, routes["routes"][model], price)
        print(f"  route {name} ({var}): {passed}", flush=True)
        save(routes, model)
    save(routes, None)
    catalog()
    print(f"spent ${(spent_fen() - start) / 700:.2f} of ${budget:.2f}")
    show()


def catalog():
    """What the model picker shows beside the checked routes, all into routes.json (no model calls): every model SubRouter
    offers with its cheapest listed price and provider count; our bench's judge and minutes a run per model over the last
    30 days (scoreboard.sqlite, rebuilt first); Artificial Analysis' intelligence index (model_check.py's aa.json)."""
    import sqlite3
    cur = json.loads(OUT.read_text()) if OUT.exists() else {"routes": {}}
    cur["catalog"] = {m["model_name"]: dict(price=[round(min(p["min_input_price"] for p in m["providers"]), 5),
                                                 round(min(p["min_output_price"] for p in m["providers"]), 5)],
                                          providers=len(m["providers"]))
                      for m in s.api("/api/user/self/subrouter/models") if m.get("providers")}
    subprocess.run([sys.executable, "-I", str(HERE / "scoreboard.py")], capture_output=True, timeout=600)
    db = sqlite3.connect(HERE / ".model-check" / "scoreboard.sqlite")
    cur["bench"] = {m: dict(judge=j, runs=n, minutes=mi) for m, n, j, mi in db.execute(
        "select model, count(*), round(avg(judge), 1), round(avg(minutes)) from runs where judge is not null "
        "and date >= date('now', '-30 day') group by model")}
    aa = HERE / ".model-check" / "aa.json"
    rows = json.loads(aa.read_text())["data"] if aa.exists() else []
    cur["aa"] = {m["slug"]: (m.get("evaluations") or {}).get("artificial_analysis_intelligence_index") for m in rows}
    cur["aa"] = {k: v for k, v in cur["aa"].items() if v is not None}
    OUT.write_text(json.dumps(cur, indent=1, sort_keys=True) + "\n")
    print(f"catalog: {len(cur['catalog'])} SubRouter models, bench for {len(cur['bench'])}, AA index for {len(cur['aa'])}")


def show():
    routes = json.loads(OUT.read_text())["routes"] if OUT.exists() else {}
    for model, r in sorted(routes.items()):
        age = (time.time() - r["vetted_at"]) / 86400
        print(f"{model:28} {','.join(r['providers'])[:40]:40} in ${r['price'][0]}/M out ${r['price'][1]}/M  cached {r['cached']}%"
              f"  efforts {'/'.join(r['efforts']) or 'none'}  vetted {age:.1f} days ago")


if __name__ == "__main__":
    a = sys.argv[1:]
    if a[:1] == ["refresh"]:
        budget = float(a[a.index("--budget") + 1]) if "--budget" in a else 0.40
        names = [x for x in a[1:] if not x.startswith("--") and x != str(a[a.index("--budget") + 1] if "--budget" in a else "")]
        refresh(names or MODELS, budget)
    elif a[:1] == ["pin"]:
        routes = json.loads(OUT.read_text())
        listed = {m["model_name"]: m for m in s.api("/api/user/self/subrouter/models")}
        for name, r in routes["routes"].items():
            if a[1:] and name not in a[1:]:
                continue
            ps = listed.get(r.get("model", name), {}).get("providers", [])
            pin(name, r, {p["provider_slug"]: (round(p["min_input_price"], 5), round(p["min_output_price"], 5)) for p in ps})
            save(routes, name)
            print(f"{name}: {', '.join(f'{g} ({p['key_env']})' for g, p in r.get('pinned', {}).items()) or 'one provider, nothing to pin'}")
    elif a[:1] == ["catalog"]:
        catalog()
    elif a[:1] == ["show"]:
        show()
    else:
        sys.exit(__doc__)
