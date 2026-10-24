#!/usr/bin/env python3
"""Find benchmark runs that saw the reference fix (2026-10-24).

Agents can read anything on this machine and the internet. Runs were found copying the merged upstream fix from three
places: the bench repos (full git history), the Go module cache (newer releases that include the fix) and GitHub
(raw files, commit API, .patch URLs). This scans every transcript under ~/bench-homes for those reads, per task, and
writes leaks.jsonl: one line per (task, harness label) with the channels seen. A module-cache copy counts only if the
reference fix is in it (checked with git apply), so reading an older release is not flagged.

usage: leakscan.py [tasks-dir ...]   (default: tasks-oct)
"""
import json, os, re, subprocess, sys, tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
HOMES = Path.home() / "bench-homes"
MODS = Path(os.environ.get("GOMODCACHE", Path.home() / "go/pkg/mod"))


def leaked():
    """(task, harness) pairs that saw the fix, from the last scan; summaries and the judge leave them out."""
    p = HERE / "leaks.jsonl"
    return {(d["task"], d["harness"]) for d in map(json.loads, p.read_text().splitlines())} if p.exists() else set()


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
        "web": re.compile(r"(curl|wget|\burl\b|urlopen|requests\.get|git clone)[^\n]{0,300}?https?://"
                          r"(github\.com|raw\.githubusercontent\.com|api\.github\.com/repos|codeload\.github\.com)/"
                          + re.escape(owner) + "/" + re.escape(repo) + r"\b"),
    }
    fixed = [d.name for d in (MODS / mod).parent.glob(Path(mod).name + "@*")
             if d.is_dir() and has_fix(d, tdir / "solution.patch")] if mod else []
    if fixed:
        rules["modcache"] = re.compile("|".join(re.escape(f"{Path(mod).parent}/{v}/") for v in fixed))
    return rules, fixed


def main():
    tdirs = [HERE / a for a in (sys.argv[1:] or ["tasks-oct"])]
    tasks = {t.name: task_rules(t) for d in tdirs for t in sorted(d.iterdir()) if (t / "git.json").exists()}
    for name, (_, fixed) in tasks.items():
        print(f"{name}: fix in module cache copies {fixed or 'none'}")
    label = re.compile(r"rw-(" + "|".join(map(re.escape, tasks)) + r")-(\S+?@[\w.-]+(?::[a-z]+)?)-[a-z0-9_]{8}")
    found = {}
    for root, dirs, files in os.walk(HOMES):
        dirs[:] = [d for d in dirs if d not in ("mod", "gomod", "go-build", "node_modules")]
        for f in files:
            if not f.endswith((".jsonl", ".json")):
                continue
            p = Path(root) / f
            try:
                s = p.read_text(errors="ignore").replace('\\"', "").replace('"', "")
            except OSError:
                continue
            m = label.search(f) or label.search(s)
            if not m or "company-review" in str(p):
                continue
            hits = {k for k, r in tasks[m.group(1)][0].items() if r.search(s)}
            if hits:
                found.setdefault((m.group(1), m.group(2)), set()).update(hits)
    with open(HERE / "leaks.jsonl", "w") as out:
        for (t, h), ch in sorted(found.items()):
            out.write(json.dumps({"task": t, "harness": h, "channels": sorted(ch)}) + "\n")
            print(f"{t:28} {h:60} {','.join(sorted(ch))}")
    print(f"{len(found)} task/harness pairs saw the fix; written to leaks.jsonl")


if __name__ == "__main__":
    main()
