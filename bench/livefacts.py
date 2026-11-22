#!/usr/bin/env python3
"""Web search for the own loop's hand-off turn (2026-11-22): does `search` (Exa's hosted MCP endpoint, mcpc.rs web_search)
answer live questions better or cheaper than `fetch` alone? Each question of suite/live-facts.jsonl runs as the turn a chat
hands to the agent (`/agent TEXT` in a headless chat-profile run: that turn's tools, prompt and permissions; fetch and
search go without a question, anything that would ask is refused), on DeepSeek V4.1 Flash 1017 on SubRouter as
handoff.py's `ds`, in two arms: fetch (TAZUNA_NO_SEARCH=1) and search.

  livefacts.py run [--runs 2] [--cap-usd 0.08]   # results in livefacts/results.jsonl; done items are skipped
  livefacts.py judge                             # Opus 5.5 xhigh with web search, blind to the arm: livefacts/judge.jsonl
  livefacts.py summary

The judge runs on the Claude plan with the user's own login: start bench/guard.py first (its PAUSE file stops new judge
calls here, and a 5-hour window at or over --stop-at stops them too). One judge call per question grades its answers of
both arms together, shuffled and labelled A, B, ...: the facts are checked once for all of them.
"""
import argparse, collections, json, os, random, statistics, subprocess, tempfile, threading, time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
OUT = HERE / "livefacts" / "results.jsonl"
JUDGED = HERE / "livefacts" / "judge.jsonl"
BIN = HERE.parent / "target" / "release" / "tazuna"
SESSIONS = Path.home() / ".local/share/tazuna/sessions"
DS = ["--model", "deepseek-v4.1-flash-1017", "--effort", "high", "--provider", "https://subrouter.ai/v1", "--key-env",
      "SUBROUTER_API_KEY", "--data", "any", "--no-escalate", "--price", "0.017,0.00034,0.068"]
ARMS = ["fetch", "search"]
LIMITED = "Exa's free search limit is reached"
lock = threading.Lock()


def items():
    return [json.loads(l) for l in (HERE / "suite" / "live-facts.jsonl").read_text().splitlines() if l.strip()]


def rows(path):
    return [json.loads(l) for l in path.read_text().splitlines() if l.strip()] if path.exists() else []


def subrouter_key():
    for l in (Path.home() / ".subrouter-key").read_text().splitlines():
        if l.startswith("export SUBROUTER_API_KEY="):
            return l.split("=", 1)[1].strip().strip('"')


def balance():
    """SubRouter's balance in dollars now (account-wide), as models.rs subrouter_balance reads it."""
    import urllib.request
    get = lambda p: json.load(urllib.request.urlopen(urllib.request.Request(f"https://subrouter.ai{p}", headers={
        "Authorization": f"Bearer {subrouter_key()}", "User-Agent": "tazuna-bench"}), timeout=20))
    left = get("/v1/dashboard/billing/subscription")["hard_limit_usd"] - get("/v1/dashboard/billing/usage")["total_usage"] / 100
    st = get("/api/status")
    st = st.get("data", st) if isinstance(st, dict) else {}
    return left / st["usd_exchange_rate"] if st.get("quota_display_type") == "CNY" and st.get("usd_exchange_rate") else left


def recorded(folder):
    """The run's tool calls in order and whether a search met Exa's limit, from its session recording (named by its folder)."""
    names, limited = [], False
    for f in SESSIONS.glob(f"*-{folder}-*.jsonl"):
        for line in f.read_text().splitlines():
            try:
                e = json.loads(line).get("e") or {}
            except ValueError:
                continue
            content = (e.get("message") or {}).get("content")
            for b in content if isinstance(content, list) else []:
                if e.get("type") == "assistant" and b.get("type") == "tool_use":
                    names.append(b.get("name"))
                if e.get("type") == "user" and b.get("type") == "tool_result" and LIMITED in str(b.get("content")):
                    limited = True
    return names, limited


def one(arm, it, run):
    env = {k: v for k, v in os.environ.items() if not k.startswith(("TAZUNA_", "EXA_"))}  # Exa's free tier: no key
    env["SUBROUTER_API_KEY"] = subrouter_key()
    env["TAZUNA_ROUTES"] = ""  # SubRouter only: no fallback to another paid provider
    env["TAZUNA_MAX_USD"] = "0.02"  # a runaway question stops at 2 cents
    if arm == "fetch":
        env["TAZUNA_NO_SEARCH"] = "1"
    cwd = tempfile.mkdtemp(prefix="lf-")
    pf = Path(cwd) / "prompt.txt"
    pf.write_text("/agent " + it["text"])
    t = time.time()
    try:
        p = subprocess.run([str(BIN), "--run", str(pf), "--allow-non-git", "--profile", "chat", *DS], cwd=cwd, env=env,
                           capture_output=True, text=True, timeout=900)
        out, err, code = p.stdout, p.stderr, p.returncode
    except subprocess.TimeoutExpired:
        out, err, code = "", "timeout", -1
    secs = round(time.time() - t, 1)
    pf.unlink()
    os.rmdir(cwd)
    try:
        d = json.loads(out.strip().splitlines()[-1])
    except (IndexError, ValueError):
        d = {"is_error": True, "result": (err or out)[-300:]}
    u = d.get("usage") or {}
    names, limited = recorded(Path(cwd).name)
    r = {"arm": arm, "id": it["id"], "kind": it["kind"], "run": run, "error": bool(d.get("is_error") or code),
         "result": d.get("result") or "", "input": u.get("input_tokens", 0), "cached": u.get("cache_read_input_tokens", 0),
         "written": u.get("cache_creation_input_tokens", 0), "output": u.get("output_tokens", 0),
         "cost": d.get("total_cost_usd") or 0, "secs": secs, "tools": names, "limited": limited, "at": time.time()}
    with lock, OUT.open("a") as f:
        f.write(json.dumps(r) + "\n")
    return r


def run(runs, workers, cap, only):
    OUT.parent.mkdir(exist_ok=True)
    have = {(r["arm"], r["id"], r["run"]) for r in rows(OUT) if not r["error"]}
    todo = [(a, it, i) for i in range(runs) for it in items() for a in ARMS if (a, it["id"], i) not in have
            and (not only or it["id"] in only)]
    print(f"{len(todo)} runs; SubRouter balance ${balance():.4f}", flush=True)
    spent = 0.0
    with ThreadPoolExecutor(workers) as ex:
        futs = [ex.submit(one, *t) for t in todo]
        for f in futs:
            r = f.result()
            spent += r["cost"]
            print(f"  {r['arm']:6} {r['id']:18} r{r['run']} {'ERROR ' if r['error'] else ''}{r['secs']:6}s ${r['cost']:.5f} "
                  f"{len(r['tools'])} tools {','.join(r['tools'])[:60]}{' 429' if r['limited'] else ''}", flush=True)
            if spent > cap:
                print(f"cap ${cap} passed (${spent:.4f} by tazuna's count): stopping", flush=True)
                for g in futs:
                    g.cancel()
                break
    print(f"done: ${spent:.4f} by tazuna's count; SubRouter balance ${balance():.4f}", flush=True)


SYSTEM = """You are a strict fact checker grading answers to a question about live, current facts. Today is {today}; the
answers were written between {start} and {end} UTC today. First check the facts yourself with WebSearch and WebFetch
(several sources when they disagree); judge each answer by what was true when it was written (a price or rate may move a
little within the day: within about 1-2% is correct). Grade every answer on its own:
- correct (2): the main fact right and current, nothing important wrong.
- partly (1): the right fact with a real error or an important omission, stale but close, or vague where precision was asked.
- wrong (0): the main fact wrong or outdated (an older release, an earlier match, last week's news), or made up.
- none (0): no answer: a refusal, "I cannot browse", or only a pointer to where to look.
Do not reward length or confidence; a confident wrong answer is wrong.

Reply with only a JSON object, one key per answer label:
{{"A": {{"verdict": "correct" | "partly" | "wrong" | "none", "score": 0 | 1 | 2, "note": "<one line>"}}, ...}}"""


def judge_one(it, answers, window, stop_at):
    while (HERE / "PAUSE").exists():
        time.sleep(30)
    state = json.loads((HERE / "guard.state.json").read_text())
    if time.time() - state["at"] > 600 or state.get("five_hour", state["util"]) >= stop_at:
        raise SystemExit(f"guard reading stale or 5h window at {state.get('five_hour')}%: not judging {it['id']}")
    labels = [chr(ord("A") + i) for i in range(len(answers))]
    prompt = f"Question: {it['text']}\n\n" + "\n\n".join(f"Answer {l}:\n{a['result'].strip() or '(empty)'}" for l, a in zip(labels, answers))
    system = SYSTEM.format(today=time.strftime("%Y-%m-%d", time.gmtime()), start=window[0], end=window[1])
    with tempfile.TemporaryDirectory() as cwd:
        # the user's own claude login: no CLAUDE_CONFIG_DIR
        cmd = ["claude", "-p", "--model", "claude-opus-5-5", "--effort", "xhigh", "--output-format", "json",
               "--no-session-persistence", "--setting-sources", "", "--strict-mcp-config", "--disable-slash-commands",
               "--exclude-dynamic-system-prompt-sections", "--tools", "WebSearch,WebFetch", "--allowedTools", "WebSearch,WebFetch",
               "--system-prompt", system]
        env = {k: v for k, v in os.environ.items() if k not in ("CLAUDE_CONFIG_DIR", "CLAUDE_CODE_OAUTH_TOKEN", "ANTHROPIC_API_KEY")}
        p = subprocess.run(cmd, cwd=cwd, input=prompt, capture_output=True, text=True, timeout=900, env=env)
    d = json.loads(p.stdout.strip().splitlines()[-1])
    if d.get("is_error"):
        raise RuntimeError(d.get("result"))
    t = d["result"]
    v = json.JSONDecoder().raw_decode(t[t.find("{"):])[0]
    out = []
    for l, a in zip(labels, answers):
        g = v.get(l) or {}
        out.append({"arm": a["arm"], "id": a["id"], "run": a["run"], "verdict": g.get("verdict"), "score": g.get("score"),
                    "note": g.get("note")})
    with lock, JUDGED.open("a") as f:
        f.write(json.dumps({"id": it["id"], "grades": out, "cost": d.get("total_cost_usd") or 0, "turns": d.get("num_turns")}) + "\n")
    return it["id"], out, d.get("total_cost_usd") or 0


def judge(workers, stop_at):
    res = [r for r in rows(OUT) if not r["error"]]
    done = {j["id"] for j in rows(JUDGED)}
    stamp = lambda t: time.strftime("%H:%M", time.gmtime(t))
    window = (stamp(min(r["at"] - r["secs"] for r in res)), stamp(max(r["at"] for r in res)))
    todo = []
    for it in items():
        answers = [r for r in res if r["id"] == it["id"]]
        if it["id"] not in done and answers:
            random.shuffle(answers)
            todo.append((it, answers))
    print(f"{len(todo)} judge calls (one per question, {sum(len(a) for _, a in todo)} answers)", flush=True)
    with ThreadPoolExecutor(workers) as ex:
        for i, out, cost in ex.map(lambda t: judge_one(*t, window, stop_at), todo):
            print(f"  {i}: " + ", ".join(f"{g['arm']}r{g['run']}={g['score']}" for g in out) + f" (${cost:.2f} at API prices)", flush=True)


def summary():
    res = {(r["arm"], r["id"], r["run"]): r for r in rows(OUT) if not r["error"]}
    grades = {(g["arm"], g["id"], g["run"]): g for j in rows(JUDGED) for g in j["grades"]}
    print(f"{'arm':7}{'runs':>5}{'score':>7}{'corr':>6}{'part':>6}{'wrong':>6}{'none':>6}{'in tok':>9}{'cached':>8}{'out':>7}"
          f"{'tools':>7}{'search':>7}{'fetch':>6}{'secs':>7}{'cost $':>9}{'429':>5}")
    for arm in ARMS:
        rs = [r for k, r in res.items() if k[0] == arm]
        gs = [grades[k] for k in res if k[0] == arm and k in grades]
        if not rs:
            continue
        v = collections.Counter(g["verdict"] for g in gs)
        m = lambda f: statistics.mean(f(r) for r in rs)
        print(f"{arm:7}{len(rs):>5}{sum(g['score'] or 0 for g in gs):>4}/{2 * len(gs):<3}{v['correct']:>5}{v['partly']:>6}{v['wrong']:>6}{v['none']:>6}"
              f"{m(lambda r: r['input'] + r['cached'] + r['written']):>9.0f}{m(lambda r: r['cached']):>8.0f}{m(lambda r: r['output']):>7.0f}"
              f"{m(lambda r: len(r['tools'])):>7.1f}{m(lambda r: r['tools'].count('search')):>7.1f}{m(lambda r: r['tools'].count('fetch')):>6.1f}"
              f"{m(lambda r: r['secs']):>7.1f}{sum(r['cost'] for r in rs):>9.4f}{sum(r['limited'] for r in rs):>5}")
    print("\nper question (score of run 0 + run 1, of 4):")
    for it in items():
        s = {a: sum(grades.get((a, it["id"], n), {}).get("score") or 0 for n in range(2)) for a in ARMS}
        t = {a: statistics.mean(len(res[(a, it["id"], n)]["tools"]) for n in range(2) if (a, it["id"], n) in res) for a in ARMS
             if any((a, it["id"], n) in res for n in range(2))}
        print(f"  {it['id']:18} fetch {s['fetch']}  search {s['search']}   tools/run fetch {t.get('fetch', 0):.1f} search {t.get('search', 0):.1f}")
    print(f"\njudge: ${sum(j['cost'] for j in rows(JUDGED)):.2f} at API prices, {len(rows(JUDGED))} calls")


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("what", choices=["run", "judge", "summary"])
    ap.add_argument("--runs", type=int, default=2)
    ap.add_argument("--workers", type=int, default=6)
    ap.add_argument("--cap-usd", type=float, default=0.08)
    ap.add_argument("--only", default="", help="question ids, comma-separated (a smoke run)")
    ap.add_argument("--stop-at", type=float, default=85, help="no judge call once the 5-hour window is at this percentage")
    a = ap.parse_args()
    if a.what == "run":
        run(a.runs, a.workers, a.cap_usd, set(filter(None, a.only.split(","))))
    elif a.what == "judge":
        judge(min(a.workers, 2), a.stop_at)
    else:
        summary()
