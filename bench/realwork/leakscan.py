#!/usr/bin/env python3
"""Find benchmark runs that saw the reference fix (2026-10-24).

Agents can read anything on this machine and the internet. Runs were found copying the merged upstream fix from three
places: the bench repos (full git history), the Go module cache (newer releases that include the fix) and GitHub
(raw files, commit API, .patch URLs). This scans every transcript under ~/bench-homes for those reads, per task, and
writes leaks.jsonl: one line per (task, harness label) with the channels seen. A module-cache copy counts only if the
reference fix is in it (checked with git apply), so reading an older release is not flagged.

usage: leakscan.py [tasks-dir ...]   (default: every tasks* set; run.py runs it after each batch)
"""
import json, os, re, subprocess, sys, tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
HOMES = Path.home() / "bench-homes"
SEEN = HERE / "leak-modcache.json"  # module-cache copies that contained a task's fix
MODS = Path(os.environ.get("GOMODCACHE", Path.home() / "go/pkg/mod"))


# refusals by run.py's proxy (web) and sandbox (local files): an attempt that got one of these is not a leak
BLOCKED = re.compile(r"Blocked by the benchmark|CONNECT tunnel failed, response 403|Tunnel connection failed: 403|non 2xx status code \(403"
                     r"|\[exit 56\]|exit:56\b|Operation not permitted|command not found|content:,is_error")  # exit 56: a silent curl refused by the proxy; `timeout` missing: the fetch never ran; an empty result: nothing came back (a silent curl piped into head)


# labels invalid as a whole: OpenCode v287/v288 ran with the bench directory as their project (no PWD), where the tasks'
# reference fixes and hidden tests lie, whether or not a transcript shows them read
INVALID = ("opencode-v287@", "opencode-v288@")


def leaked():
    """(task, harness) pairs that saw the fix, from the last scan; summaries and the judge leave them out."""
    p = HERE / "leaks.jsonl"
    found = {(d["task"], d["harness"]) for d in map(json.loads, p.read_text().splitlines())
             if any(not c.endswith("-blocked") for c in d["channels"])} if p.exists() else set()
    return found | {(r["task"], r["harness"]) for f in HERE.glob("results*.jsonl")
                    for r in map(json.loads, f.read_text().splitlines()) if r["harness"].startswith(INVALID)}


def has_fix(moddir, patch):
    """True when the reference patch is already applied in this module copy."""
    with tempfile.TemporaryDirectory() as t:
        subprocess.run(["cp", "-R", f"{moddir}/.", t], check=True)
        subprocess.run(["chmod", "-R", "u+w", t], check=True)
        subprocess.run(["git", "init", "-q"], cwd=t, check=True)
        return subprocess.run(["git", "apply", "--check", "-R", str(patch)], cwd=t,
                              capture_output=True).returncode == 0


def task_rules(tdir):
    g = json.loads((tdir / "git.json").read_text())
    owner, repo = re.match(r"https://github.com/([^/]+)/([^/]+)/", g["pr"]).groups()  # type: ignore[union-attr]
    mod = subprocess.run(["git", "-C", g["repo"], "show", f"{g['base']}:go.mod"], capture_output=True,
                         text=True).stdout.split("\n", 1)[0].removeprefix("module ").strip()
    rules = {
        "local": re.compile(r"bench-repos/" + re.escape(Path(g["repo"]).name) + r"\b|bench/realwork/tasks[\w-]*/"
                            + re.escape(tdir.name) + r"/(solution\.patch|hidden|expected)"),
        # a fetch, not an import path or a doc link: curl/wget/WebFetch url within one line (quotes removed first)
        "web": re.compile(r"(?:(curl|wget|urlopen|requests\.get|git clone|git ls-remote)[^\n]{0,300}?|\burl:\s*)https?://"
                          r"(github\.com|raw\.githubusercontent\.com|api\.github\.com/repos|codeload\.github\.com)/"
                          + re.escape(owner) + "/" + re.escape(repo) + r"\b"),
    }
    fixed = [d.name for d in (MODS / mod).parent.glob(Path(mod).name + "@*")
             if d.is_dir() and has_fix(d, tdir / "solution.patch")] if mod else []
    # copies found once are remembered: they get deleted from the cache, the old transcripts still name them
    seen = json.loads(SEEN.read_text()) if SEEN.exists() else {}
    fixed = sorted(set(fixed) | set(seen.get(tdir.name, [])))
    if fixed:
        SEEN.write_text(json.dumps(seen | {tdir.name: fixed}, indent=1, sort_keys=True) + "\n")
    if fixed:
        rules["modcache"] = re.compile("|".join(re.escape(f"{Path(mod).parent}/{v}/") for v in fixed))
    return rules, fixed


def transcripts():
    """(name, text) of every transcript: JSON(L) files, and OpenCode's sessions (sqlite, one text per session)."""
    import sqlite3
    for root, dirs, files in os.walk(HOMES):
        dirs[:] = [d for d in dirs if d not in ("mod", "gomod", "go-build", "node_modules")]
        for f in files:
            p = Path(root) / f
            if f.endswith((".jsonl", ".json")):
                try:
                    yield str(p), p.read_text(errors="ignore")
                except OSError:
                    pass
            elif f == "opencode.db":
                db = sqlite3.connect(f"file:{p}?mode=ro", uri=True)
                for sid, d in db.execute("select id, directory from session_v2"):
                    yield str(p), d + "\n" + "\n".join(r[0] for r in db.execute(
                        "select data from session_message where session_id = ? order by seq", (sid,)))
                db.close()


def main(sets=()):
    tdirs = [HERE / a for a in sets] or sorted(HERE.glob("tasks*/"))
    tasks = {t.name: task_rules(t) for d in tdirs for t in sorted(d.iterdir()) if (t / "git.json").exists()}
    for name, (_, fixed) in tasks.items():
        print(f"{name}: fix in module cache copies {fixed or 'none'}")
    label = re.compile(r"rw-(" + "|".join(map(re.escape, tasks)) + r")-(\S+?@[\w.-]+(?::[a-z]+)?)-[a-z0-9_]{8}")
    found = {}
    for f, s in transcripts():
        # JSON escapes undone so a match cannot run across lines (Go's `net/url` and a GitHub link in a comment below it
        # once read as a fetch)
        s = s.replace('\\"', "").replace('"', "").replace("\\n", "\n")
        if True:
            m = label.search(f) or label.search(s)
            if not m or "company-review" in f:
                continue
            hits = {k for k, r in tasks[m.group(1)][0].items() if r.search(s)}
            for ch in ("web", "local"):
                if ch in hits and all(BLOCKED.search(s, w.end(), w.end() + 1500) for w in tasks[m.group(1)][0][ch].finditer(s)):
                    hits.discard(ch)
                    hits.add(ch + "-blocked")  # tried, refused by the proxy or the sandbox
            if hits:
                found.setdefault((m.group(1), m.group(2)), set()).update(hits)
    with open(HERE / "leaks.jsonl", "w") as out:
        for (t, h), ch in sorted(found.items()):
            out.write(json.dumps({"task": t, "harness": h, "channels": sorted(ch)}) + "\n")
            print(f"{t:28} {h:60} {','.join(sorted(ch))}")
    print(f"{len(found)} task/harness pairs saw the fix; written to leaks.jsonl")


if __name__ == "__main__":
    main(sys.argv[1:])
