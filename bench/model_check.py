#!/usr/bin/env python3
"""Daily model check (ROADMAP Next 5, first step): does a model we could use beat a current pick?

  python3 bench/model_check.py           # new flags since the last run (keys from the environment)
  python3 bench/model_check.py --all     # every flag
  python3 bench/model_check.py --why X   # the benchmark evidence for one model

Sources, cached for a day in bench/.model-check/ (attribution required by both):
- Artificial Analysis (https://artificialanalysis.ai/), key in ARTIFICIAL_ANALYSIS_API_KEY: indices and ~15 benchmarks.
- Epoch AI (https://epoch.ai/benchmarks, CC BY 4.0), no key: ~85 benchmark files with random baseline and ceiling.
Scoring: every benchmark becomes 0 (random) .. 1 (ceiling); benchmarks fall into categories; each role weighs the
categories (bench/roles.json). A candidate is compared with the pick only on benchmarks both have, at the pick's effort
(cheap models at max), and may cost at most 10% more per task: from our own runs (results*.jsonl), else Epoch's
cost-per-task columns, else the token price. A flag is a lead, not a decision: it starts a gauge run on our benchmark, and only that may change a
pick.
"""
import csv, io, json, os, re, sys, time, urllib.request, zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
STATE = Path(os.environ.get("MODEL_CHECK_STATE") or HERE / ".model-check")
AA_URL = "https://artificialanalysis.ai/api/v2/data/llms/models"
EPOCH_URL = "https://epoch.ai/data/benchmark_data.zip"
EFFORTS = ["max", "xhigh", "high", "medium", "low", "minimal", "default"]
MIN_SHARED, MIN_LEAD = 3, 0.03

# benchmark name (normalized) -> category; anything else is guessed from its name or left unscored (and reported)
CATEGORY = {
    # agentic coding: whole tasks in a repository or terminal
    "swebenchverified": "agentic_coding", "deepswe": "agentic_coding", "frontierswe": "agentic_coding",
    "cursorbench": "agentic_coding", "terminalbench": "agentic_coding", "terminalbenchhard": "agentic_coding",
    "terminalbenchv21": "agentic_coding", "terminalbenchv40": "agentic_coding", "frontiercode": "agentic_coding",
    "gso": "agentic_coding", "theagentcompany": "agentic_coding", "apexagents": "agentic_coding",
    "metrtimehorizons": "agentic_coding", "mirrorcode": "agentic_coding", "posttrainbench": "agentic_coding",
    "artificialanalysiscodingindex": "agentic_coding",
    # coding: problems and programs
    "livecodebench": "coding", "scicode": "coding", "aiderpolyglot": "coding", "webdevarena": "coding",
    "alebench": "coding", "algotune": "coding", "weirdml": "coding", "weirdmlv3": "coding", "cybench": "coding",
    "exploitbench": "coding", "cadeval": "coding", "livebench": "coding",
    # tool use and long-running agents
    "tau2": "tool_use", "taubanking": "tool_use", "vendingbench2": "tool_use", "osworld": "tool_use",
    "osworld2": "tool_use", "rli": "tool_use", "deepresearchbench": "tool_use", "balrog": "tool_use",
    # instruction following, long context
    "ifbench": "instruction", "lcr": "long_context", "fictionlivebench": "long_context",
    # reasoning and knowledge
    "artificialanalysisintelligenceindex": "reasoning", "gpqa": "reasoning", "gpqadiamond": "reasoning", "hle": "reasoning",
    "arcagi": "reasoning", "arcagi2": "reasoning", "simplebench": "reasoning", "critpt": "reasoning",
    "mmlupro": "knowledge", "mmlu": "knowledge", "simpleqaverified": "knowledge",
    "aime": "math", "aime25": "math", "math500": "math", "mathlevel5": "math", "artificialanalysismathindex": "math",
    "otismockaime20242025": "math", "frontiermath": "math",
}
CATEGORY.update({
    "gsobench": "agentic_coding", "osworld20": "tool_use", "remotelaborindex": "tool_use", "gdpval": "tool_use",
    "proofbench": "math", "gsm8k": "math", "chesspuzzles": "reasoning", "mysterygamepuzzles": "reasoning",
    "clbench": "long_context", "clbenchlife": "long_context",
})
# not scored on purpose: saturated (no longer separates current models), unrelated to our roles, or content unclear
IGNORE = {"anli", "bbh", "hellaswag", "lambada", "superglue", "winogrande", "furnitureassembly", "vpct", "geobench",
          "surfaceevolverbench", "lechmazurwriting", "dtbench", "ebrbench", "lmca"}
# a candidate may cost at most this much more than the pick per task: quality per dollar, not quality at any price
MAX_PRICE_RATIO = 1.1
GUESS = [("swe", "agentic_coding"), ("terminal", "agentic_coding"), ("agent", "agentic_coding"), ("code", "coding"),
         ("frontiermath", "math"), ("math", "math"), ("aime", "math"), ("arc", "reasoning"), ("qa", "knowledge")]


def norm(s):
    return re.sub(r"[^a-z0-9]", "", s.lower())


def category(bench):
    n = norm(bench)
    if n in IGNORE:
        return None
    if n in CATEGORY:
        return CATEGORY[n]
    return next((c for k, c in GUESS if k in n), None)


def fetch(url, headers=None, timeout=120):
    with urllib.request.urlopen(urllib.request.Request(url, headers=headers or {}), timeout=timeout) as r:
        return r.read()


def cached(name, get, max_age=86400):
    STATE.mkdir(exist_ok=True)
    f = STATE / name
    if not (f.exists() and time.time() - f.stat().st_mtime < max_age):
        f.write_bytes(get())
    return f.read_bytes()


def split_effort(name):
    """'glm-5.3_max' / 'claude-opus-5-5-medium' / 'DeepSeek V4.1 Flash (Max)' -> (base key, effort)."""
    s = name.lower()
    m = re.search(r"[_\-\s(](max|xhigh|extra high|high|medium|low|minimal|non-reasoning|unknown)\)?\s*$", s)
    effort = "default"
    if m:
        effort = {"extra high": "xhigh", "non-reasoning": "minimal", "unknown": "default"}.get(m.group(1), m.group(1))
        s = s[: m.start()]
    return norm(re.sub(r"\(.*?\)", "", s)), effort


def load_aa(raw):
    """[(base, effort, benchmark, score 0..1, creator, blended price)] from the Artificial Analysis response."""
    out = []
    for m in json.loads(raw)["data"]:
        base, effort = split_effort(m["slug"])
        if effort == "default" and "(max" in m.get("name", "").lower():
            effort = "max"  # the bare slug is the max variant for models that come in efforts
        price = (m.get("pricing") or {}).get("price_1m_blended_3_to_1")
        creator = (m.get("model_creator") or {}).get("slug", "")
        for k, v in (m.get("evaluations") or {}).items():
            if isinstance(v, (int, float)):
                out.append((base, effort, k, v / 100 if "index" in k else v, creator, price))
    return out


def load_epoch(raw):
    """Same rows from Epoch's archive, each benchmark scaled by its random baseline and ceiling."""
    z = zipfile.ZipFile(io.BytesIO(raw))
    names = {n.rsplit("/", 1)[-1]: n for n in z.namelist()}
    meta = list(csv.DictReader(io.TextIOWrapper(z.open(names["benchmark_metadata.csv"]), encoding="utf-8")))
    out = []
    for b in meta:
        if b.get("superseded_by") or b["source_file"] not in names:
            continue
        lo, hi = float(b.get("random_baseline") or 0), float(b.get("score_ceiling") or b.get("scale") or 1)
        best = {}
        for r in csv.DictReader(io.TextIOWrapper(z.open(names[b["source_file"]]), encoding="utf-8")):
            try:
                v = float(r.get(b["score_column"]) or "")
            except ValueError:
                continue
            base, effort = split_effort(r.get("Model version") or r.get("model_version") or "")
            if base:
                scaled = max(0.0, min(1.0, (v - lo) / (hi - lo))) if hi > lo else v
                key = (base, effort)
                best[key] = max(best.get(key, 0.0), scaled)  # several agents or scorers per model: the best run
        org = ""
        out += [(base, effort, b["benchmark"], s, org, None) for (base, effort), s in best.items()]
    return out


# cost per task: what a run costs, not what a token costs (MiMo V2.6 Pro: cheaper per token than its benchmark lead
# suggested, 30x DeepSeek per task because it writes 15x the output)
COST_COLUMNS = ["Cost per task", "Mean cost (USD)", "Average cost (USD)", "Cost", "Cost per run", "Cost per test (USD)",
                "Total cost (USD)", "Average costs", "Estimated cost (USD)"]


def load_epoch_costs(raw):
    """{(base, effort): {benchmark file: cost per task}} from Epoch's files that report one."""
    z = zipfile.ZipFile(io.BytesIO(raw))
    out = {}
    for n in z.namelist():
        if not n.endswith(".csv"):
            continue
        rows = list(csv.DictReader(io.TextIOWrapper(z.open(n), encoding="utf-8")))
        col = next((c for c in COST_COLUMNS if rows and c in rows[0]), None)
        if not col:
            continue
        for r in rows:
            try:
                cost = float(r.get(col) or "")
            except ValueError:
                continue
            base, effort = split_effort(r.get("Model version") or "")
            if base and cost > 0:
                out.setdefault((base, effort), {})[n.rsplit("/", 1)[-1]] = cost
    return out


def load_own_costs(paths):
    """{base: {task: cost}} from our own benchmark runs (results*.jsonl): the cost on our loop, the strongest evidence."""
    out = {}
    for p in paths:
        for line in open(p):
            try:
                r = json.loads(line)
            except ValueError:
                continue
            h = r.get("harness", "")
            cost = r.get("reported_cost") or r.get("cost")  # reported: what the run cost (`cost` uses one price table)
            # plain runs of one model only: the cascade mixes two models, prompt experiments are not the default loop
            if not re.match(r"(tazuna-own|tazuna-ownel|tazuna-repo|cc)(-v\d+)?@", h):
                continue
            if isinstance(cost, (int, float)) and cost > 0 and r.get("passed") is not None:
                base, _ = split_effort(h.split("@", 1)[1].split(":")[0])
                out.setdefault(base, {}).setdefault(r["task"], []).append(cost)
    return {b: {t: sum(c) / len(c) for t, c in ts.items()} for b, ts in out.items()}


def geo_ratio(a, b):
    """Geometric mean of a[k] / b[k] over shared keys, and how many."""
    shared = [k for k in a if k in b]
    if not shared:
        return None, 0
    import math
    return math.exp(sum(math.log(a[k] / b[k]) for k in shared) / len(shared)), len(shared)


def cost_ratio(costs, own, cand_base, cand_eff, pick_base, pick_eff, price, pick_price):
    """(candidate cost / pick cost per task, source): our runs, else Epoch's cost-per-task columns, else token price."""
    r, n = geo_ratio(own.get(cand_base, {}), own.get(pick_base, {}))
    if r is not None:
        return r, f"our runs, {n} task{'s' if n > 1 else ''}"
    c, _ = at_effort(costs, cand_base, cand_eff)
    p, _ = at_effort(costs, pick_base, pick_eff)
    r, n = geo_ratio(c or {}, p or {})
    if r is not None:
        return r, f"Epoch, {n} benchmark{'s' if n > 1 else ''}"
    if price is not None and pick_price:
        return price / pick_price, "token price"
    return None, "no cost data"


def table(rows):
    """{(base, effort): {benchmark: score}} plus {base: (creator, price)}; Artificial Analysis wins a name clash."""
    scores, info = {}, {}
    for base, effort, bench, s, creator, price in rows:
        scores.setdefault((base, effort), {}).setdefault(bench, s)
        if creator or price is not None:
            old = info.get(base, ("", None))
            info[base] = (creator or old[0], price if price is not None else old[1])
    return scores, info


def at_effort(scores, base, effort):
    """The model's scores at `effort`, else the nearest effort it was measured at (higher first)."""
    order = EFFORTS[EFFORTS.index(effort):] + EFFORTS[: EFFORTS.index(effort)][::-1] if effort in EFFORTS else EFFORTS
    for e in order:
        if (base, e) in scores:
            return scores[(base, e)], e
    return None, None


def compare(cand, pick, weights):
    """Weighted lead of `cand` over `pick` on shared benchmarks: (lead, shared count, [(benchmark, delta)])."""
    by_cat = {}
    deltas = []
    for b, s in cand.items():
        if b in pick and (c := category(b)) in weights:
            by_cat.setdefault(c, []).append(s - pick[b])
            deltas.append((b, s - pick[b]))
    if not deltas:
        return 0.0, 0, []
    total = sum(weights[c] for c in by_cat)
    lead = sum(weights[c] * sum(d) / len(d) for c, d in by_cat.items()) / total
    return lead, len(deltas), sorted(deltas, key=lambda x: -x[1])


def is_free(m):
    """A provider's model-list entry priced at zero (Experiential Labs: nano-USD fields; CheaperInference: per million)."""
    p = m.get("pricing") or {}
    prices = [p.get(k) for k in ("input_nano_usd_per_million_tokens", "output_nano_usd_per_million_tokens", "input_per_million", "output_per_million")]
    known = [float(v) for v in prices if v not in (None, "")]
    return bool(known) and all(v == 0 for v in known)


def check(roles, profiles, scores, info, served, costs=None, own=None):
    """Flags: (role, candidate, effort, lead, shared, top evidence, where, cost ratio, cost source)."""
    costs, own = costs or {}, own or {}
    flags = []
    served_keys = {norm(re.sub(r"[_\-]?(max|xhigh|high|medium|low)$", "", mid)): (prov, mid) for prov, ids in served.items() for mid in ids}
    for r in roles:
        weights = profiles[r["profile"]]
        pbase, _ = split_effort(r["pick"])
        pick, _ = at_effort(scores, pbase, r["effort"])
        pick_price = info.get(pbase, ("", None))[1]
        if pick is None:
            flags.append((r["role"], f"(no data for {r['pick']})", "", 0, 0, [], "", None, ""))
            continue
        for base in sorted({b for b, _ in scores}):
            if base == pbase:
                continue
            creator, price = info.get(base, ("", None))
            if r["pool"] == "anthropic":
                where = "anthropic" if creator == "anthropic" or base.startswith("claude") else None
                eff = r["effort"]
            else:
                hit = served_keys.get(base)
                cap = r.get("max_blended_usd_per_m")
                where = f"{hit[0]}:{hit[1]}" if hit and (cap is None or price is None or price <= cap) else None
                eff = "max"
            if not where:
                continue
            cand, ceff = at_effort(scores, base, eff)
            if cand is None:
                continue
            lead, shared, ev = compare(cand, pick, weights)
            if shared < MIN_SHARED or lead <= MIN_LEAD:
                continue
            ratio, source = cost_ratio(costs, own, base, ceff, pbase, r["effort"], price, pick_price)
            if ratio is not None and ratio > MAX_PRICE_RATIO:
                continue  # better but dearer per task: the pick was chosen for its price
            flags.append((r["role"], base, ceff, lead, shared, ev[:3], where, ratio, source))
    return flags


def unscored(scores):
    return sorted({b for s in scores.values() for b in s if category(b) is None and norm(b) not in IGNORE})


def main():
    cfg = json.loads((HERE / "roles.json").read_text())
    rows = []
    key = os.environ.get("ARTIFICIAL_ANALYSIS_API_KEY")
    if key:
        rows += load_aa(cached("aa.json", lambda: fetch(AA_URL, {"x-api-key": key})))
    else:
        print("note: ARTIFICIAL_ANALYSIS_API_KEY not set, Epoch AI data only")
    epoch = cached("epoch.zip", lambda: fetch(EPOCH_URL))
    rows += load_epoch(epoch)
    scores, info = table(rows)
    costs = load_epoch_costs(epoch)
    own = load_own_costs(sorted((HERE / "realwork").glob("results*.jsonl")))
    served = {}
    for name, p in cfg["providers"].items():
        if k := os.environ.get(p["key_env"]):
            raw = cached(f"models-{name}.json", lambda: fetch(p["url"] + "/models", {"Authorization": f"Bearer {k}"}))
            # a model listed at $0 is a free tier: throttled so hard for free accounts that agent runs fall over (Qwen3.8
            # Flash Next, Ling 3.1 Flash on Experiential Labs, 2026-10-12/13), so it is no candidate
            served[name] = [m["id"] for m in json.loads(raw)["data"] if not is_free(m)]
    if "--why" in sys.argv:
        want = norm(sys.argv[sys.argv.index("--why") + 1])
        for (base, effort), s in sorted(scores.items()):
            if want in base:
                print(f"{base} @ {effort}: " + ", ".join(f"{b} {v:.2f}" for b, v in sorted(s.items())))
        return
    flags = check(cfg["roles"], cfg["profiles"], scores, info, served, costs, own)
    seen_file = STATE / "flagged.json"
    seen = set(json.loads(seen_file.read_text())) if seen_file.exists() else set()
    new = [f for f in flags if "--all" in sys.argv or f"{f[0]}|{f[1]}" not in seen]
    print(f"Model check {time.strftime('%Y-%m-%d')}: {len({b for b, _ in scores})} models, "
          f"{len({b for s in scores.values() for b in s})} benchmarks; {len(new)} {'flags' if '--all' in sys.argv else 'new flags'}. "
          "Data: Artificial Analysis (https://artificialanalysis.ai/), Epoch AI (https://epoch.ai/benchmarks, CC BY 4.0).")
    for role, name, eff, lead, shared, ev, where, ratio, source in new:
        if not where:
            print(f"- {role}: {name}")
            continue
        best = ", ".join(f"{b} {d:+.2f}" for b, d in ev)
        cost = f"cost per task x{ratio:.2f} ({source})" if ratio is not None else "cost per task unknown"
        print(f"- {role}: {name} @ {eff} leads by {lead:+.3f} on {shared} shared benchmarks ({best}); {cost}; {where} -> gauge run")
    if "--all" in sys.argv:
        print("unscored benchmarks (no category yet):", ", ".join(unscored(scores)) or "none")
    seen_file.write_text(json.dumps(sorted(f"{f[0]}|{f[1]}" for f in flags)))


if __name__ == "__main__":
    main()
