#!/usr/bin/env python3
"""Real use as a running score (bench/suite/DESIGN.md "Real use as a running score"): the user's own live tazuna runs,
scored from what happened next, with no labels asked for. Reads tazuna's data folder (runs.jsonl, sessions/, labels.jsonl)
and prints totals per version and per lane; `--rows FILE` writes one row per run (keep that file private: it holds the
user's prompts).

  bench/realuse.py [--since 0.1.100] [--rows FILE]

Per run, from the session recording and the next message the user sent:
  strong  the run was stopped or rewound, ended in an error or crash, git shows its change discarded, the router's chat
          answer was sent to the agent after all (`/agent`), the next message says it failed ("doesn't work", "wrong", "CI failed", "has an error", "don't ...") or asks the same again
  soft    pushback or waiting: "wait, ...", "why create ...?", "can't we ...", "progress?"
Checked against 390 runs labelled by hand (2026-11-12): strong and soft together caught 12 of the 14 bad runs; 4 of
231 good runs were flagged (all "wait - ..." clarifications, now soft).
"""
import argparse, collections, glob, json, os, re, statistics

DATA = os.path.expanduser("~/.local/share/tazuna")
NEG_START = re.compile(r"^\s*(no\b(?!\s+(need|worries|problem|rush))|nope|wrong|stop\b|undo|revert|don'?t\b|do not\b|that'?s not|"
                       r"not what|this is wrong|incorrect|you missed|you forgot|you didn'?t|why did you|why didn'?t)", re.I)
NEG_ANY = re.compile(r"(doesn'?t work|didn'?t work|still (not|fail|broken|the same|wrong|doesn)|same (error|issue|problem)|"
                     r"ci (failed|fails|is failing|broke)|(has|have|got|getting|threw|throws) an? error|error though|fails? (again|still)|"
                     r"it broke|broke (the|it)|not (working|correct|right)|that'?s wrong|wasn'?t (asked|what))", re.I)
SOFT_START = re.compile(r"^\s*(wait\b|hold on|why (create|make|add|open|use|is it|are you|not)|can'?t we|shouldn'?t (we|it|you))", re.I)
HANG = re.compile(r"^\s*(progress\??|any progress\??|still (running|working)\??|status\??|hello\??|are you (there|stuck)\??)\s*$", re.I)
SKIP_CWD = re.compile(r"/(bench|bench-homes|tmp|private/var|T/)|/rw-|looptest|livetest|mcp-e2e|rv-probe")


def words(t):
    return set(re.findall(r"[a-z0-9]{3,}", (t or "").lower()))


def vkey(v):
    return tuple(int(x) for x in re.findall(r"\d+", v or "0"))


def messages():
    """Each recording's typed messages in order, with the next one, the gap, and whether the run was stopped or rewound."""
    out = {}
    for f in glob.glob(f"{DATA}/sessions/*.jsonl"):
        ev = []
        for line in open(f, errors="ignore"):
            try:
                d = json.loads(line)
            except ValueError:
                continue
            if "e" in d:
                ev.append((d.get("t", 0), d["e"]))
        users = [(i, t, e.get("text", "")) for i, (t, e) in enumerate(ev) if e.get("type") == "harness_user"]
        for k, (i, t, text) in enumerate(users):
            nxt = users[k + 1] if k + 1 < len(users) else None
            span = ev[i:nxt[0]] if nxt else ev[i:]
            out.setdefault((f, (text or "").splitlines()[0][:200] if text else ""), []).append(dict(
                text=text, next=nxt[2] if nxt else "", gap=(nxt[1] - t) / 1000 if nxt else None,
                stopped=any(e.get("type") == "harness_interrupt" for _, e in span),
                rewound=any(e.get("type") in ("harness_rewind", "harness_rewind_start") for _, e in span)))
    return out


def signals(r):
    strong, soft = [], []
    if r["stopped"]: strong.append("stopped")
    if r["rewound"]: strong.append("rewound")
    if r["stop"] in ("error", "crashed"): strong.append(r["stop"])
    if r["verdict"] == "discarded": strong.append("discarded")
    head = (r["next"] or "").strip()[:240]
    # 0.1.127's router answered it as a chat and the user sent it to the agent after all
    if head.startswith("/agent "): strong.append("misrouted")
    if head and (NEG_START.search(head) or NEG_ANY.search(head)): strong.append("correction")
    if head and r["gap"] is not None and r["gap"] < 1800:
        a, b = words(r["prompt"]), words(head)
        if len(a) >= 4 and len(b) >= 4 and len(a & b) / len(a | b) >= 0.6: strong.append("re-ask")
    if head and SOFT_START.search(head): soft.append("pushback")
    if head and HANG.search(head): soft.append("waited")
    return strong, soft


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--since", default="0")
    ap.add_argument("--rows")
    a = ap.parse_args()
    msgs = messages()
    verdicts = {}
    if os.path.exists(f"{DATA}/labels.jsonl"):
        for line in open(f"{DATA}/labels.jsonl"):
            try:
                d = json.loads(line)
                verdicts[d["run"]] = d.get("verdict")
            except (ValueError, KeyError):
                pass
    rows = []
    for line in open(f"{DATA}/runs.jsonl"):
        try:
            run = json.loads(line)
        except ValueError:
            continue
        m = run.get("meta", {})
        prompt = run.get("prompt") or ""
        if m.get("mode") != "live" or SKIP_CWD.search(m.get("cwd") or "") or vkey(m.get("version")) < vkey(a.since):
            continue
        if prompt.startswith("(claude continued") or (prompt.startswith("/") and len(prompt.split()) <= 2):
            continue  # wake-ups and tazuna's own commands
        cands = msgs.get((m.get("recording") or "", prompt.splitlines()[0][:200] if prompt else ""), [])
        full = cands.pop(0) if cands else {}
        model = m.get("model") or ""
        r = dict(ts=run.get("ts"), version=m.get("version"), model=model, lane="Claude" if model.startswith("claude") or not m.get("provider") and not model else "own loop",
                 profile=m.get("profile"), stop=run.get("stop"), secs=run.get("secs"), cost=run.get("cost_usd") or 0, verdict=verdicts.get(run.get("id")),
                 prompt=full.get("text") or prompt, next=full.get("next") or "", gap=full.get("gap"), stopped=bool(full.get("stopped")), rewound=bool(full.get("rewound")))
        r["strong"], r["soft"] = signals(r)
        # a run with no model work (under 3 s, nothing spent: a local command, a start that never reached the model)
        if (r["secs"] or 0) < 3 and not r["cost"] and not r["strong"]:
            continue
        rows.append(r)
    if a.rows:
        with open(a.rows, "w") as f:
            for r in rows:
                f.write(json.dumps(r, ensure_ascii=False) + "\n")

    def table(key, title):
        g = collections.defaultdict(list)
        for r in rows:
            g[key(r)].append(r)
        print(f"\n{title:<24} {'runs':>5} {'strong':>8} {'soft':>7} {'median s':>9} {'$/run':>7}")
        for k in sorted(g, key=str):
            rs = g[k]
            st = sum(bool(r["strong"]) for r in rs) / len(rs)
            so = sum(bool(r["soft"]) and not r["strong"] for r in rs) / len(rs)
            print(f"{str(k):<24} {len(rs):>5} {100 * st:>7.0f}% {100 * so:>6.0f}% {statistics.median([r['secs'] or 0 for r in rs]):>9.0f} {statistics.mean([r['cost'] for r in rs]):>7.3f}")
    print(f"{len(rows)} live runs since {a.since} (strong: a clear miss; soft: pushback or waiting, not counted as strong)")
    import datetime
    table(lambda r: datetime.date.fromtimestamp(r["ts"] or 0).strftime("%Y-W%W"), "week")
    table(lambda r: r["lane"], "lane")
    table(lambda r: (r["model"] or "(router)").split("@")[0][:24], "model")
    why = collections.Counter(s for r in rows for s in r["strong"] + r["soft"])
    print("\nsignals:", dict(why.most_common()))


if __name__ == "__main__":
    main()
