"""Helpers for ops tasks (DESIGN.md): a task's setup.py builds its git scenario in the run's tree (a bare `origin`
inside .git, branches, a gh stand-in scenario); its check.py reads the end state and the gh log and prints one
`go test -json` event per check, for run.py's hidden_score."""
import json, subprocess
from pathlib import Path

W = Path.cwd()
REPO = "acme/query"


def git(*a, check=True):
    return subprocess.run(["git", *a], cwd=W, capture_output=True, text=True, check=check).stdout.strip()


def gitdir():
    return Path(git("rev-parse", "--path-format=absolute", "--git-common-dir"))


def remote(*a):
    """git on the bare origin, as the agent's pushes left it."""
    return subprocess.run(["git", f"--git-dir={gitdir() / 'remote.git'}", *a], capture_output=True, text=True).stdout.strip()


# ---- setup

def start():
    git("branch", "-M", "main")
    git("config", "user.name", "Dev"); git("config", "user.email", "dev@example.com")
    subprocess.run(["git", "init", "-q", "--bare", "-b", "main", str(gitdir() / "remote.git")], check=True)
    git("remote", "add", "origin", str(gitdir() / "remote.git"))
    git("push", "-q", "-u", "origin", "main")


def edit(path, old, new):
    p = W / path
    s = p.read_text()
    assert old in s, (path, old)
    p.write_text(s.replace(old, new, 1))


def commit(msg, author="teammate"):
    git("add", "-A")
    git("-c", f"user.name={author}", "-c", f"user.email={author}@example.com", "commit", "-qm", msg)


def branch(name, base="main"):
    git("checkout", "-q", "-b", name, base)


def push(name):
    git("push", "-q", "-u", "origin", name)


def pr(n, head, title, body="", checks="SUCCESS"):
    return dict(number=n, title=title, body=body, headRefName=head, baseRefName="main", state="OPEN",
                url=f"https://github.com/{REPO}/pull/{n}", author=dict(login="teammate"), isDraft=False,
                statusCheckRollup=[dict(name="typecheck", conclusion=checks, status="COMPLETED")])


def pr_entries(prs, next_number):
    """gh answers for a set of open PRs: list, view, diff, checkout, create (the next number), close, comment."""
    e = [dict(match=["pr", "list"], json=prs, text="\n".join(f"{p['number']}\t{p['title']}\t{p['headRefName']}\tOPEN" for p in prs))]
    for p in prs:
        n = str(p["number"])
        e += [dict(match=["pr", "view", n], json=p, text=f"{p['title']} {REPO}#{n}\nOpen • teammate wants to merge into main from {p['headRefName']}\n\n{p['body']}\n\n{p['url']}"),
              dict(match=["pr", "diff", n], run=f"git --no-pager diff origin/main...origin/{p['headRefName']}"),
              dict(match=["pr", "checkout", n], run=f"git fetch -q origin && (git checkout -q {p['headRefName']} 2>/dev/null || git checkout -q -b {p['headRefName']} --track origin/{p['headRefName']})"),
              dict(match=["pr", "close", n], text=f"✓ Closed pull request {REPO}#{n} ({p['title']})"),
              dict(match=["pr", "comment", n], text=f"{p['url']}#issuecomment-1")]
    e += [dict(match=["pr", "create"], text=f"https://github.com/{REPO}/pull/{next_number}"),
          dict(match=["pr", "view"], json=prs[0], text="no pull requests found for the current branch", exit=1),
          dict(match=["repo", "view"], json=dict(nameWithOwner=REPO, defaultBranchRef=dict(name="main")), text=f"{REPO}\nTanStack Query fork")]
    return e


def finish(entries, on="main", **state):
    (gitdir() / "gh-scenario.json").write_text(json.dumps(entries, indent=1))
    git("checkout", "-q", on)
    git("tag", "-f", "bench-start")
    state["main"] = git("rev-parse", "main")
    (gitdir() / "ops-start.json").write_text(json.dumps(state))


# ---- check

def started():
    return json.loads((gitdir() / "ops-start.json").read_text())


def gh_log():
    p = gitdir() / "gh-log.jsonl"
    return [json.loads(l) for l in p.read_text().splitlines()] if p.exists() else []


def body(call):
    """The text a gh call carried: --body/-b/--comment/-c values, or what it read from stdin."""
    a = call["args"]
    vals = [a[i + 1] for i, x in enumerate(a[:-1]) if x in ("--body", "-b", "--comment", "-c")]
    return " ".join(vals + [call.get("stdin") or ""])


def show(ref, path):
    return remote("show", f"{ref}:{path}")


def report(checks):
    for name, ok in checks.items():
        print(json.dumps({"Test": name, "Action": "pass" if ok else "fail"}))
