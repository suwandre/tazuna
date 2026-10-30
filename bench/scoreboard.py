#!/usr/bin/env python3
"""One table of model performance over time: public benchmark scores (model_check.py's daily snapshots) and our own bench
runs (hidden tests, judge, cost, minutes), in bench/.model-check/scoreboard.sqlite (rebuilt from the sources each time;
outside git). The base for drift checks (does a model or a provider get worse?) and for routing by task category.

  scoreboard.py                 rebuild, print row counts
  scoreboard.py drift MODEL     a model's public scores per day, and its own-bench pass rate and judge per week
"""
import gzip, json, re, sqlite3, sys, time
from collections import defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
STATE, REAL = HERE / ".model-check", HERE / "realwork"
DB = STATE / "scoreboard.sqlite"
SESSIONS = Path.home() / "bench-homes/cc/.local/share/tazuna/sessions"


def label_dates():
    """Run label number -> unix time, from tazuna's session files (their names start with the start time). Rows written
    before run.py stamped `ts` (2026-10-30) get the date of the nearest label."""
    known = {}
    for f in SESSIONS.glob("*.jsonl"):
        m = re.match(r"(\d{9,10})-rw-.*?-v(\d+)@", f.name)
        if m:
            known.setdefault(int(m.group(2)), int(m.group(1)))
    return sorted(known.items())


def date_of(v, dates):
    if not dates or v is None:
        return None
    best = min(dates, key=lambda kv: abs(kv[0] - v))
    return best[1]


def build():
    STATE.mkdir(exist_ok=True)
    DB.unlink(missing_ok=True)
    db = sqlite3.connect(DB)
    db.executescript("""
        create table public(date text, model text, effort text, benchmark text, score real, creator text, price real);
        create table runs(date text, set_name text, task text, harness text, base text, model text, effort text,
                          passed int, total int, regress int, judge real, cost real, minutes real, turns int, key_env text);
    """)
    # the daily launchd job keeps its own state (model_check_daily.sh); a hand run keeps bench/.model-check
    snaps = {f.name: f for d in (Path.home() / ".local/share/tazuna/model-check/state/history", STATE / "history")
             for f in d.glob("public-*.jsonl.gz")}
    for f in (snaps[k] for k in sorted(snaps)):
        day = f.name[7:17]
        with gzip.open(f, "rt") as rows:
            db.executemany("insert into public values (?,?,?,?,?,?,?)",
                           ((day, r["model"], r["effort"], r["benchmark"], r["score"], r["creator"], r["price"]) for r in map(json.loads, rows)))
    judge = defaultdict(list)
    for f in REAL.glob("judge*.jsonl"):
        for line in f.open():
            try:
                j = json.loads(line)
            except ValueError:
                continue
            if "overall" in j and "harness" in j:
                judge[(j["task"], j["harness"])].append(j["overall"])
    dates = label_dates()
    for f in REAL.glob("results*.jsonl"):
        set_name = "tasks" if f.name == "results.jsonl" else f.name[8:-6]
        for r in map(json.loads, f.open()):
            h = r["harness"]
            m = re.search(r"-v(\d+)@", h)
            ts = r.get("ts") or date_of(int(m.group(1)) if m else None, dates)
            j = judge.get((r["task"], h))
            db.execute("insert into runs values (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)", (
                time.strftime("%Y-%m-%d", time.localtime(ts)) if ts else None, set_name, r["task"], h,
                re.sub(r"-v\d+$", "", h.split("@")[0]), r.get("model"), r.get("effort"), r.get("passed"), r.get("total"),
                r.get("regress"), sum(j) / len(j) if j else None, r.get("cost"), round((r.get("secs") or 0) / 60, 1),
                r.get("turns"), r.get("key_env")))
    db.commit()
    n = {t: db.execute(f"select count(*) from {t}").fetchone()[0] for t in ("public", "runs")}
    days = db.execute("select count(distinct date) from public").fetchone()[0]
    print(f"{DB}: {n['public']} public scores over {days} day(s), {n['runs']} own runs")
    return db


def drift(db, model):
    like = f"%{model.lower()}%"
    plain = "%" + re.sub(r"[^a-z0-9]", "", model.lower()) + "%"  # model_check.norm keeps letters and digits only
    print(f"public scores for models matching {model!r} (per day):")
    for row in db.execute("select model, effort, benchmark, group_concat(date || ' ' || round(score, 3), ' | ') from public "
                          "where model like ? group by model, effort, benchmark order by model, effort, benchmark limit 60", (plain,)):
        print("  ", *row)
    print(f"own bench runs with models matching {model!r} (per week):")
    for row in db.execute("select strftime('%Y-W%W', date), model, effort, count(*), round(avg(1.0 * passed / total), 2), "
                          "round(avg(judge), 2), round(avg(cost), 3), round(avg(minutes), 1) from runs where model like ? and total > 0 "
                          "group by 1, 2, 3 order by 1, 2, 3", (like,)):
        print("   week {} {}:{} runs {} pass {} judge {} cost {} min {}".format(*row))


if __name__ == "__main__":
    db = build()
    if len(sys.argv) > 2 and sys.argv[1] == "drift":
        drift(db, sys.argv[2])
