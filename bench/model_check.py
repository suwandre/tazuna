#!/usr/bin/env python3
"""Weekly model check (ROADMAP Next 5, first step): does a model we could use beat a current pick on its metric?

  ARTIFICIAL_ANALYSIS_API_KEY=... python3 bench/model_check.py          # report new flags since the last run
  python3 bench/model_check.py --all                                    # every flag, not only new ones

Data: Artificial Analysis (https://artificialanalysis.ai/, attribution required), cached for a day in
bench/.model-check/. Candidates per role (bench/roles.json): `anthropic` = Anthropic models; `cheap` = models one of our
providers serves (their /v1/models, keys from the environment) under the role's price cap. A flag is a lead, not a
decision: it starts a gauge run on our own benchmark, and only that may change a pick.
"""
import json, os, re, sys, time, urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
STATE = HERE / ".model-check"
AA_URL = "https://artificialanalysis.ai/api/v2/data/llms/models"
# words Artificial Analysis appends for effort or mode variants; tazuna runs its own effort, so they are matched away
VARIANT = re.compile(r"\((?:reasoning|thinking|non-reasoning|max|xhigh|high|medium|low|minimal|adaptive)[^)]*\)|\b(?:reasoning|thinking)\b", re.I)


def norm(s):
    """A comparable model name: variant words dropped, then only letters and digits ("DeepSeek V4.1 Flash (max)" and
    "deepseek-v4.1-flash" both become "deepseekv41flash")."""
    return re.sub(r"[^a-z0-9]", "", VARIANT.sub("", s.lower()))


def get_json(url, headers):
    with urllib.request.urlopen(urllib.request.Request(url, headers=headers), timeout=60) as r:
        return json.load(r)


def cached(name, fetch, max_age=86400):
    STATE.mkdir(exist_ok=True)
    f = STATE / f"{name}.json"
    if f.exists() and time.time() - f.stat().st_mtime < max_age:
        return json.loads(f.read_text())
    data = fetch()
    f.write_text(json.dumps(data))
    return data


def aa_models():
    key = os.environ.get("ARTIFICIAL_ANALYSIS_API_KEY")
    if not key:
        sys.exit("ARTIFICIAL_ANALYSIS_API_KEY not set (a free key from the Artificial Analysis Insights Platform)")
    return cached("aa", lambda: get_json(AA_URL, {"x-api-key": key})["data"])


def provider_models(providers):
    """{provider name: set of model ids} for the providers whose key is in the environment."""
    out = {}
    for name, p in providers.items():
        key = os.environ.get(p["key_env"])
        if key:
            out[name] = cached(f"models-{name}", lambda: [m["id"] for m in get_json(p["url"] + "/models", {"Authorization": f"Bearer {key}"})["data"]])
    return out


def score(m, metric):
    return (m.get("evaluations") or {}).get(f"artificial_analysis_{metric}")


def best_variant(aa, wanted_norm, metric):
    """The highest-scoring Artificial Analysis entry for a model (its effort variants count as one model)."""
    hits = [m for m in aa if norm(m.get("slug", "")) == wanted_norm or norm(m.get("name", "")) == wanted_norm]
    hits = [m for m in hits if score(m, metric) is not None]
    return max(hits, key=lambda m: score(m, metric) or 0, default=None)


def check(roles, aa, served, overrides):
    """Flags as (role, candidate name, candidate score, pick score, blended price, where it is served)."""
    flags = []
    served_norm = {norm(overrides.get(mid, mid)): (prov, mid) for prov, ids in served.items() for mid in ids}
    for r in roles:
        metric = r["metric"]
        pick = best_variant(aa, norm(overrides.get(r["pick"], r["pick"])), metric)
        if pick is None:
            flags.append((r["role"], f"(no data for {r['pick']})", None, None, None, ""))
            continue
        base = score(pick, metric)
        seen = set()
        for m in sorted(aa, key=lambda m: -(score(m, metric) or -1)):
            s, key = score(m, metric), norm(m.get("slug") or m.get("name", ""))
            if s is None or s <= base or key in seen or key == norm(pick.get("slug", "")):
                continue
            price = (m.get("pricing") or {}).get("price_1m_blended_3_to_1")
            if r["pool"] == "anthropic":
                where = "anthropic" if (m.get("model_creator") or {}).get("slug") == "anthropic" else None
            else:
                hit = served_norm.get(key) or served_norm.get(norm(m.get("name", "")))
                cap = r.get("max_blended_usd_per_m")
                where = f"{hit[0]}:{hit[1]}" if hit and (cap is None or (price is not None and price <= cap)) else None
            if where:
                seen.add(key)
                flags.append((r["role"], m.get("name"), s, base, price, where))
    return flags


def main():
    cfg = json.loads((HERE / "roles.json").read_text())
    overrides = {k: v for k, v in cfg.get("aa_names", {}).items() if not k.startswith("_")}
    flags = check(cfg["roles"], aa_models(), provider_models(cfg["providers"]), overrides)
    seen_file = STATE / "flagged.json"
    seen = set(json.loads(seen_file.read_text())) if seen_file.exists() else set()
    keys = {f"{f[0]}|{f[1]}" for f in flags}
    new = [f for f in flags if "--all" in sys.argv or f"{f[0]}|{f[1]}" not in seen]
    print(f"Model check {time.strftime('%Y-%m-%d')}: {len(new)} {'flags' if '--all' in sys.argv else 'new flags'} "
          f"(data: Artificial Analysis, https://artificialanalysis.ai/)")
    for role, name, s, base, price, where in new:
        if s is None:
            print(f"- {role}: {name}")
        else:
            cost = f", ${price:.2f}/M blended" if price is not None else ""
            print(f"- {role}: {name} scores {s:.1f} vs the pick's {base:.1f} ({where}{cost}) -> gauge run")
    seen_file.write_text(json.dumps(sorted(keys)))


if __name__ == "__main__":
    main()
