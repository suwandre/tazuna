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
  routes.py tiers [--write]       DeepSeek's providers from the gauge (provgauge.jsonl, canary): up to 4 of the cheapest that
                                  passed every check and up to 2 proven ones at up to 3x the floor; --write sets the route
  routes.py set MODEL SLUG,...    a route's providers by hand (the route key, routes.json, a pinned key each): `tiers` proposes
  routes.py ship                  src/routes.json, built into tazuna: the checked routes without keys, for users with one key
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
EFFORTS = {"deepseek-v4.1-flash-1017": ["off", "high", "max"], "kimi-k3.1-pro": []}  # DeepSeek: off ("none"), high (low to xhigh alike), max (2026-11-06)


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


LINES = [f"Line {i}: Die Quellcode-Überprüfung #{i * 7919} prüft {i * 104729} Zeilen, émojis 🙂🚀, 中文字符 {i}, snake_case_{i}, CamelCase{i}.\n"
         for i in range(10)]


def slope(model, slug):
    """Input tokens a line of distinct text through one provider (test key tazuna-vet-1): a model's tokenizer counts it
    the same on every honest provider, whatever prompt a provider adds in front. 2 and 10 lines, each sent twice, the
    smaller count of each size (some providers mix backends with and without a hidden prompt). Also the hidden prompt it
    implies. fxpayback's "Sol" counted 58 a line where Sol counts 43, and scored 29/100 on HLE against 45 (2026-11-05)."""
    import os, urllib.request
    k = s.api(f"/api/token/{s.key('tazuna-vet-1', model, [slug], 'SUBROUTER_VET1_KEY')}")
    if bool(k.get("include_shared_subscriptions")) != slug.startswith("shared-"):
        s.api("/api/token/", "PUT", {**k, "include_shared_subscriptions": slug.startswith("shared-")})

    def count(text):
        body = {"model": model, "messages": [{"role": "user", "content": text}], "max_tokens": 16, "reasoning_effort": "low"}
        q = urllib.request.Request("https://subrouter.ai/v1/chat/completions", data=json.dumps(body).encode(), headers={
            "Authorization": f"Bearer {os.environ['SUBROUTER_VET1_KEY']}", "Content-Type": "application/json", "User-Agent": "curl/8"})
        with urllib.request.urlopen(q, timeout=180) as r:
            return json.load(r)["usage"]["prompt_tokens"]

    try:
        two, ten = min(count("".join(LINES[:2])) for _ in range(2)), min(count("".join(LINES)) for _ in range(2))
    except Exception as e:  # noqa: BLE001
        print(f"  {slug:22} tokenizer not measured: {e!r:.80}")
        return None, None
    per = (ten - two) / 8
    return round(per, 1), round(two - 2 * per)


def tokenizer_check(model, good, routes):
    """Leaves out a provider whose tokens a line differ by over 10% from most providers' (another model under the name),
    when its count is consistent (its implied hidden prompt not negative: a mixed-backend provider can measure oddly)."""
    if len(good) < 2:
        return good
    seen = routes.setdefault("checked", {}).setdefault(model, {})
    m = {g: slope(model, g) for g in good}
    for g, (per, hidden) in m.items():
        if per is not None:
            seen.setdefault(g, {}).update(tokens_a_line=per, hidden_prompt=hidden)
    clean = [per for per, hidden in m.values() if per is not None and hidden is not None and hidden >= -10]
    if not clean:
        return good
    mode = max(set(round(x) for x in clean), key=lambda v: sum(abs(x - v) <= 0.1 * v for x in clean))
    keep = []
    for g in good:
        per, hidden = m[g]
        if per is not None and hidden is not None and hidden >= -10 and abs(per - mode) > 0.1 * mode:
            print(f"  {g:22} left out: {per} tokens a line against {mode} on most providers (another tokenizer, another model)", flush=True)
            continue
        if hidden and hidden > 1000:
            print(f"  {g:22} adds a ~{hidden}-token hidden prompt to every call (billed as input)", flush=True)
        keep.append(g)
    return keep


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
        good = tokenizer_check(model, good, routes)
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


def tiers(model="deepseek-v4.1-flash-1017", write=False):
    """The user's rule (2026-11-06): a few cheap providers and one or two proven ones that cost more. SubRouter sends a
    key's calls to the cheapest of its providers (weighed by cache) and moves on when one fails or caches badly, so the
    proven ones carry the calls only then. It cannot see a provider that answers with another model, so every provider
    here passed the gauge's checks in every recent round: same tokenizer as the rest (tokens a line), the hard problem
    right in most rounds, nearly every call answered, and the canary's AIME at 26/30 or more where measured."""
    import statistics as st
    rows = [json.loads(l) for l in (HERE / "provgauge.jsonl").open() if l.strip().startswith("{")]
    rows = [r for r in rows if r.get("at", "") >= time.strftime("%Y-%m-%dT%H", time.gmtime(time.time() - 2 * 86400))]
    ref = st.median([r["tokens_a_line"] for r in rows if r.get("tokens_a_line")])
    aime = {}
    for l in (HERE / ".model-check" / "canary.jsonl").open():
        c = json.loads(l)
        g = c["route"].split("_1017_")[-1].removesuffix("_KEY").lower().replace("_", "-") if "_1017_" in c["route"] else None
        if g and c["set"] == "aime25" and c["model"] == model:
            aime.setdefault((g, c["date"]), []).append((bool(c.get("ok")), bool(c.get("error")), c.get("out_tokens") or 0))
    latest, length = {}, {}
    for (g, d), res in sorted(aime.items()):
        # a run its provider mostly refused says nothing about quality (kriora: 503 on all 130 questions, 2026-11-06)
        if len(res) == 30 and sum(e for _, e, _ in res) <= 5:
            latest[g] = sum(ok for ok, _, _ in res)
            length[g] = st.median([n for ok, _, n in res if ok and n] or [0])
    # the real model at high writes 3-4k tokens a solved AIME problem; the providers that scored 20-25 that evening wrote
    # 7-11k and took 5-10x as long (another model or other settings, 2026-11-07): twice the usual length is out
    usual = st.median([n for n in length.values() if n]) if any(length.values()) else 0
    price = {p["provider_slug"]: p["min_input_price"] for m in s.api("/api/user/self/subrouter/models") if m["model_name"] == model
             for p in m["providers"]}
    per = {}
    for r in rows:
        per.setdefault(r["slug"], []).append(r)
    passed = []
    for g, rs in per.items():
        toks = [r["tokens_a_line"] for r in rs if r.get("tokens_a_line")]
        right = [r["depth_right"] for r in rs if r.get("depth_right") is not None]
        ok = sum(r["ok"] for r in rs) / (4 * len(rs))
        why = []
        if len(rs) < 3:
            why.append(f"{len(rs)} rounds")
        if ok < 0.9:
            why.append(f"answered {ok:.0%}")
        # another model reads far off (36.5, 56); 46.6 and 51.4 came from bangai and lingai between 49s, and every
        # provider had an off round within two days (2026-11-07), so 80% of the readings must match, not all
        real = [x for x in toks if abs(x - ref) <= 3]
        if not toks or len(real) < 0.8 * len(toks):
            why.append(f"tokens a line {len(real)}/{len(toks)} like the rest ({sorted(set(toks)) or 'never measured'})")
        if len(right) < 2 or sum(right) < len(right) * 2 / 3:
            why.append(f"hard problem {sum(right)}/{len(right)}")
        if latest.get(g, 30) < 26:
            why.append(f"AIME {latest[g]}/30")
        if usual and length.get(g, 0) > 2 * usual:
            why.append(f"{length[g]:.0f} tokens a solved AIME problem (usual {usual:.0f})")
        if g not in price:
            why.append("not listed now")
        cache = st.median([r["cached"] for r in rs])
        # what a call costs: agent calls are mostly input, and cached input costs a tenth (lingai at 99% cached is ~4x cheaper
        # a call than stableai at 33% for a lower list price)
        eff = price.get(g, 0) * (1 - 0.9 * cache / 100)
        tps = st.median([r["tps"] for r in rs if r.get("tps")] or [0])
        line = dict(slug=g, price=price.get(g), eff=eff, ok=ok, cache=cache, aime=latest.get(g), rounds=len(rs), tps=tps)
        print(f"  {g:12} {'pass' if not why else 'out: ' + '; '.join(why)}"
              f"  (${price.get(g, 0):.4f}/M, ${eff:.4f} with its cache, {len(rs)} rounds, answered {ok:.0%}, cache {cache:.0f}%, AIME {latest.get(g, '-')})")
        if not why:
            passed.append(line)
    passed.sort(key=lambda x: x["eff"])
    floor = passed[0]["eff"] if passed else 0
    cheap = [x for x in passed if x["eff"] <= 2 * floor][:4]
    least = min((x["price"] for x in passed), default=0)
    # the fallback: a clean record and quick answers over cache (bangai: AIME 30/30, never a fake reading, 65% cached)
    proven = sorted((x for x in passed if x not in cheap and x["ok"] >= 0.95 and x["cache"] >= 60 and x["tps"] >= 100 and x["price"] <= 3 * least),
                    key=lambda x: (-(x["aime"] or 0), -x["cache"], x["eff"]))[:2]
    pick = [x["slug"] for x in cheap + proven]
    print(f"{model}: cheap {[x['slug'] for x in cheap]} + proven {[x['slug'] for x in proven]}")
    if write and pick:
        routes = json.loads(OUT.read_text())
        r = routes["routes"][model]
        s.key("tazuna-route-" + re.sub(r"[^a-z0-9]+", "-", model).strip("-"), model, pick, r["key_env"])
        r["providers"], r["price"] = pick, [round(min(price[g] for g in pick), 5), r["price"][1]]
        r["why"] = (f"{time.strftime('%Y-%m-%d')} routes.py tiers: cheap {', '.join(x['slug'] for x in cheap)}; proven "
                    f"{', '.join(x['slug'] for x in proven) or 'none'} (every check passed over the gauge's last two days)")
        r["vetted_at"] = int(time.time())
        OUT.write_text(json.dumps(routes, indent=1, sort_keys=True) + "\n")
        print("route key and routes.json updated")


def set_route(model, slugs, why):
    """A route's providers by hand: the route key limited to them, routes.json, and a pinned key per provider."""
    routes = json.loads(OUT.read_text())
    r = routes["routes"][model]
    listed = {p["provider_slug"]: p for m in s.api("/api/user/self/subrouter/models") if m["model_name"] == model for p in m["providers"]}
    missing = [g for g in slugs if g not in listed]
    if missing:
        sys.exit(f"{model}: not listed now: {', '.join(missing)}")
    s.key("tazuna-route-" + re.sub(r"[^a-z0-9]+", "-", model).strip("-"), model, slugs, r["key_env"])
    cheapest = min(slugs, key=lambda g: listed[g]["min_input_price"])
    r["providers"], r["price"] = slugs, [round(listed[cheapest]["min_input_price"], 5), round(listed[cheapest]["min_output_price"], 5)]
    r["why"], r["vetted_at"] = f"{time.strftime('%Y-%m-%d')}: {why}", int(time.time())
    r["pinned"] = {g: p for g, p in r.get("pinned", {}).items() if g in slugs}
    pin(model, r, {g: (round(p["min_input_price"], 5), round(p["min_output_price"], 5)) for g, p in listed.items()})
    save(routes, model)
    print(f"{model}: {', '.join(slugs)} (pinned: {', '.join(r.get('pinned', {}))})")


def ship():
    """The checked routes as tazuna's built-in list (src/routes.json): what a user with one SubRouter key needs to pick a
    model and to set its providers on that key (`tazuna subrouter`). No key names: their key is SUBROUTER_API_KEY."""
    cur = json.loads(OUT.read_text())
    keep = ("url", "model", "providers", "price", "cached", "efforts", "vetted_at", "why", "via")
    routes = {n: {k: v for k, v in r.items() if k in keep} for n, r in cur["routes"].items() if "@" not in n}
    slug = lambda m: m.lower().replace(".", "-")
    out = {"routes": routes, "aa": {k: v for k, v in cur.get("aa", {}).items() if k in {slug(r.get("model", n)) for n, r in routes.items()}}}
    (HERE.parent / "src" / "routes.json").write_text(json.dumps(out, indent=1, sort_keys=True) + "\n")
    print(f"src/routes.json: {', '.join(routes)}")


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
    elif a[:1] == ["tiers"]:
        tiers(write="--write" in a)
    elif a[:1] == ["ship"]:
        ship()
    elif a[:1] == ["set"] and len(a) >= 3:
        set_route(a[1], a[2].split(","), " ".join(a[3:]) or "set by hand")
    else:
        sys.exit(__doc__)
