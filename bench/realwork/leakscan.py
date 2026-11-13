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
BLOCKED = re.compile(r"Blocked by the benchmark|Transport error \(GET|Web search cancelled|CONNECT tunnel failed, response 403|Tunnel connection failed: 403|non 2xx status code \(403"
                     r"|\[exit 56\]|exit:56\b|Operation not permitted|command not found|content:,is_error|returned error: 403|content:/usr/bin/curl\s*,is_error|proxy refused the connection|\(no output\)"
                     # the Linux jail (bubblewrap) lays an empty tmpfs over the bench repos: git finds no repository there, a
                     # listing shows only . and .. (`ls -la`: an empty tmpfs directory, 2 links and 40 bytes), a file is missing
                     # (tazuna and pi on the VM, 2026-11-10: flagged, refused)
                     r"|up to mount point /home/bench/bench-repos|No such file or directory|content:\.\s\.\.\s*,is_error"
                     r"|\s2 \S+ \S+\s+40 [^\n]* \.\n")  # exit 56: a silent curl refused by the proxy; pi's empty result: (no output); `timeout` missing: the fetch never ran; an empty result (or only `command -v curl`'s line): nothing came back; curl -f's 403


# labels invalid as a whole: OpenCode v287/v288 ran with the bench directory as their project (no PWD), where the tasks'
# reference fixes and hidden tests lie, whether or not a transcript shows them read
# tazuna v504 read the work tree of the Claude Code run beside it and of a killed one in TMPDIR (2026-10-28; the jail now
# blocks both, and this scan does not look for it)
INVALID = ("opencode-v287@", "opencode-v288@", "tazuna-ownsubx-v504@")


def leaked():
    """(task, harness) pairs that saw the fix, from the last scan; summaries and the judge leave them out."""
    p = HERE / "leaks.jsonl"
    found = {(d["task"], d["harness"]) for d in map(json.loads, p.read_text().splitlines())
             if any(not c.endswith(("-blocked", "-listed")) for c in d["channels"])} if p.exists() else set()
    return found | {(r["task"], r["harness"]) for f in HERE.glob("results*.jsonl")
                    for r in map(json.loads, f.read_text().splitlines()) if r["harness"].startswith(INVALID)}


# Another run's work tree (rw-<task>-<harness>-vN@...) or a harness's logs. tazuna v504 copied tests from a killed Claude
# Code run's tree and ran `git diff` in the one beside it; pi v518 mined an earlier pi transcript (2026-10-28). A command
# that only lists them (ls, find, a scandir) is "peer-listed", not a leak.
JAILED_FROM = 526  # first run under run.py's jail_for (2026-10-28)
RUNDIR = re.compile(r"rw-[a-z0-9-]+?-[a-z][\w-]*?-v(\d+)@[^\s'\"\\]*")
LOGDIR = re.compile(r"bench-homes/[^\s'\"\\]*?/(?:transcripts|tazuna/sessions|tazuna/own-sessions|tazuna/runs\.jsonl|\.claude/projects|opencode|\.codex/sessions)\b")
READS = re.compile(r"^\s*(?:\w+=\S*\s+)*(?:cd|cat|sed|head|tail|less|more|diff|git|cp|rsync|grep|rg|awk|cut|xxd|od|strings|"
                   r"python3?|patch|jq|bat|vim?|nano|tar|zip|unzip|base64|go|for|while)\b|open\(|read_text|\.read\(|glob\(")


def tool_calls(text):
    """The arguments of every tool call in a transcript: tazuna's and Claude Code's tool_use, pi's toolCall, Codex's
    command_execution and file_change items, Kimi Code's OpenAI-style tool_calls."""
    out = []
    for line in text.splitlines():
        if not any(t in line for t in ('"tool_use"', '"toolCall"', '"command_execution"', '"file_change"', '"tool_calls"')):
            continue
        try:
            stack = [json.loads(line)]
        except json.JSONDecodeError:
            continue
        while stack:
            x = stack.pop()
            if isinstance(x, dict):
                if x.get("type") in ("tool_use", "toolCall"):  # OpenCode's tool_use event keeps it in part.state.input
                    out.append(x.get("input") or x.get("arguments") or ((x.get("part") or {}).get("state") or {}).get("input") or {})
                elif x.get("type") == "command_execution" and isinstance(x.get("command"), str):
                    out.append({"command": x["command"]})
                elif x.get("type") == "function" and isinstance(x.get("function"), dict):  # Kimi Code: OpenAI tool_calls
                    try:
                        a = json.loads(x["function"].get("arguments") or "{}")
                    except json.JSONDecodeError:
                        a = {"raw": x["function"].get("arguments")}
                    out.append(a if isinstance(a, dict) else {"raw": a})
                elif x.get("type") == "file_change":
                    out.append({"paths": [c.get("path") for c in x.get("changes") or []]})
                else:
                    stack.extend(x.values())
            elif isinstance(x, list):
                stack.extend(x)
    return out


def peer(text, own):
    """'peer' when a tool call reads another run's tree or a harness log, 'peer-listed' when one only lists them."""
    seen = None
    for a in tool_calls(text):
        cmd = a.get("command") if isinstance(a.get("command"), str) else None
        if cmd:  # Codex wraps each command in `bash -lc '...'`
            cmd = re.sub(r"^\s*(?:/bin/)?(?:ba|z)?sh\s+-l?c\s+(['\"])(.*)\1\s*$", r"\2", cmd, flags=re.S)
        blob = cmd if cmd is not None else json.dumps(a)
        # as a path (after a slash), not a grep pattern filtering a listing
        hits = [m for m in [*(m for m in RUNDIR.finditer(blob) if m.group(1) != own), *LOGDIR.finditer(blob)]
                if m.start() and blob[m.start() - 1] == "/"]
        if not hits:
            continue
        if cmd is None:  # read, grep, glob, edit tools name what they open
            return "peer"
        names = {v for v in re.findall(r"(\w+)=[\"']?[^\s\"';]*rw-|(\w+)=[\"']?[^\s\"';]*bench-homes", cmd) for v in v if v}
        for seg in re.split(r"&&|\|\||;|\||\n", cmd):  # a path held in a shell variable counts where the variable is used
            if (any(h.group(0) in seg for h in hits) or any(f"${n}" in seg or f"${{{n}}}" in seg for n in names)) and READS.search(seg):
                return "peer"
        seen = "peer-listed"
    return seen


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
    # an ops task (bench/suite) is a scripted scenario with no upstream PR: nothing on GitHub to fetch it from
    owner, repo = re.match(r"https://github.com/([^/]+)/([^/]+)/", g["pr"]).groups() if g.get("pr") else ("-", "-")  # type: ignore[union-attr]
    repo = g["repo"] if os.path.exists(g["repo"]) else str(Path.home() / "bench-repos" / Path(g["repo"]).name)  # the Linux VM
    mod = subprocess.run(["git", "-C", repo, "show", f"{g['base']}:go.mod"], capture_output=True,
                         text=True).stdout.split("\n", 1)[0].removeprefix("module ").strip()
    rules = {
        "local": re.compile(r"bench-repos/" + re.escape(Path(g["repo"]).name) + r"\b|bench/realwork/tasks[\w-]*/"
                            + re.escape(tdir.name) + r"/(solution\.patch|hidden|expected)"),
        # a fetch, not an import path or a doc link: curl/wget/WebFetch url within one line (quotes removed first)
        # the repo's own git history (`git show HEAD:bench/realwork/tasks/...`): the jails hid the directory, not .git,
        # until 2026-11-11 (a tazuna run on the VM read k6's hidden tests that way, v1142)
        "local-git": re.compile(r"\bgit\b[^\n]{0,300}?\b(?:show|cat-file|log|archive|grep|checkout|restore|diff)\b[^\n]{0,300}?"
                                r"bench/realwork/tasks[\w-]*/" + re.escape(tdir.name) + r"/"),
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


def drop_raw_input(line):
    try:
        e = json.loads(line)
    except json.JSONDecodeError:
        return line
    stack = [e]
    while stack:
        x = stack.pop()
        if isinstance(x, dict):
            x.pop("rawInput", None)
            stack.extend(x.values())
        elif isinstance(x, list):
            stack.extend(x)
    return json.dumps(e)


def new_tests(tdir):
    """Test names the PR's hidden tests add: not in the repo before the fix, not in the prompt. A transcript that holds
    three of them (or all, if fewer) saw the upstream tests, from whatever host or copy (OMP read hugo's and k6's from
    cdn.jsdelivr.net and a Gitea mirror, 2026-11-11, past every rule above). One or two can be the model's own guesses."""
    g = json.loads((tdir / "git.json").read_text())
    repo = g["repo"] if os.path.exists(g["repo"]) else str(Path.home() / "bench-repos" / Path(g["repo"]).name)
    prompt = (tdir / "prompt.txt").read_text(errors="ignore") if (tdir / "prompt.txt").exists() else ""
    names = {n for f in (tdir / "hidden").rglob("*") if f.is_file() for n in re.findall(r"func (Test\w{6,})", f.read_text(errors="ignore"))}
    return {n for n in names if n not in prompt
            and subprocess.run(["git", "-C", repo, "grep", "-q", "-w", n, g["base"]], capture_output=True).returncode != 0}


def transcripts():
    """(name, text) of every transcript: JSON(L) files, and OpenCode's sessions (sqlite, one text per session)."""
    import sqlite3
    for root, dirs, files in os.walk(HOMES):
        dirs[:] = [d for d in dirs if d not in ("mod", "gomod", "go-build", "node_modules")]
        for f in files:
            p = Path(root) / f
            if f.endswith((".jsonl", ".json")):
                try:
                    # the model's own reasoning (tazuna 0.0.95 records it) is thinking, not a fetch or a read: left out; so are
                    # pi's message_update events, partial copies of a message that its message_end repeats in full (they put
                    # a refused fetch's result beyond the window, v518)
                    lines = [l for l in p.read_text(errors="ignore").splitlines()
                             if '"e":{"type":"reasoning"' not in l and not l.startswith('{"type":"message_update"')]
                    if "/opencode-sub/transcripts/" in str(p):  # OpenCode repeats each call's input as rawInput after its result
                        lines = [drop_raw_input(l) for l in lines]
                    yield str(p), "\n".join(lines)
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
    added = {name: new_tests(t) for d in tdirs for t in sorted(d.iterdir()) if (t / "git.json").exists() for name in [t.name]}
    label = re.compile(r"rw-(" + "|".join(map(re.escape, tasks)) + r")-(\S+?@[\w.-]+(?::[a-z]+)?)-[a-z0-9_]{8}")
    found = {}
    for f, s in transcripts():
        m = label.search(f) or label.search(s)
        v = re.search(r"-v(\d+)@", m.group(2)) if m else None
        p = peer(s, v.group(1)) if v and "company-review" not in f else None
        if p == "peer" and int(v.group(1)) >= JAILED_FROM:
            p = "peer-blocked"  # the jail refuses those reads (pi v530 tried 62 times: "Operation not permitted")
        if p:
            found.setdefault((m.group(1), m.group(2)), set()).add(p)
        # the commands the agent ran: a fetch counts only there, not in a file it read (CONTRIBUTING.md's `git clone
        # https://github.com/...` flagged six Sol runs, 2026-10-28); a transcript with no parsed calls is read whole
        ran = "\n".join(json.dumps(a) for a in tool_calls(s)).replace('\\"', "").replace('"', "").replace("\\n", "\n")
        # JSON escapes undone so a match cannot run across lines (Go's `net/url` and a GitHub link in a comment below it
        # once read as a fetch)
        s = s.replace('\\"', "").replace('"', "").replace("\\n", "\n")
        if True:
            m = label.search(f) or label.search(s)
            if not m or "company-review" in f:
                continue
            hits = {k for k, r in tasks[m.group(1)][0].items() if any(k != "web" or not ran or w.group(0) in ran for w in r.finditer(s))}
            for ch in ("web", "local"):
                # Claude Code's stream-json puts more between a call and its result than tazuna's transcript does; its
                # WebFetch reports "proxy refused the connection"; OpenCode's names the URL inside its error ("Transport error (GET <url>)"), so
                # the search starts just before the match
                win = 20000 if "/transcripts/" in f else 1500
                if ch in hits and all(BLOCKED.search(s, max(0, w.start() - 40), w.end() + win) for w in tasks[m.group(1)][0][ch].finditer(s)
                                      if ch != "web" or not ran or w.group(0) in ran):
                    hits.discard(ch)
                    hits.add(ch + "-blocked")  # tried, refused by the proxy or the sandbox
            # the Linux jail (bubblewrap, run.py since 2026-11-08; the VM's labels from v1100) lays an empty tmpfs over the
            # bench repos and the tasks: a local read finds nothing there. pi's hugo run was still flagged for its own
            # thinking ("maybe bench-repos/hugo ... It's empty"), 2026-11-10
            v = re.search(r"-v(\d+)@", m.group(2))
            if sys.platform.startswith("linux") and v and int(v.group(1)) >= 1100 and "local" in hits:
                hits.discard("local")
                hits.add("local-blocked")
            # three names (or all, for a task with fewer): two were one model's own good guesses (OpenCode named its test
            # TestRunOnceRejectsMultipleScenarios beside TestRunOnce, k6, 2026-11-12); every real leak showed 3 to 9
            new = added[m.group(1)]
            if len(new) >= 2 and len({n for n in new if n in s}) >= min(3, len(new)):
                hits.add("hidden-tests")
            if hits:
                found.setdefault((m.group(1), m.group(2)), set()).update(hits)
    with open(HERE / "leaks.jsonl", "w") as out:
        for (t, h), ch in sorted(found.items()):
            out.write(json.dumps({"task": t, "harness": h, "channels": sorted(ch)}) + "\n")
            print(f"{t:28} {h:60} {','.join(sorted(ch))}")
    print(f"{len(found)} task/harness pairs saw the fix; written to leaks.jsonl")


if __name__ == "__main__":
    main(sys.argv[1:])
