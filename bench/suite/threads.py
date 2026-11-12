#!/usr/bin/env python3
"""A PR's review threads for a review task's ground truth (threads.py owner/repo NUMBER [...]): who wrote each, where,
whether it was resolved, and the commit it was made on. Bots (CodeRabbit, Copilot, Cursor and the like) are marked: a
review task's known defects should come from people, or from a bot's finding that people acted on."""
import json, subprocess, sys
Q = """query($o: String!, $r: String!, $n: Int!) { repository(owner: $o, name: $r) { pullRequest(number: $n) {
  title baseRefOid headRefOid commits(first: 1) { nodes { commit { oid } } }
  reviewThreads(first: 60) { nodes { isResolved isOutdated path line originalLine
    comments(first: 3) { nodes { author { login __typename } body originalCommit { oid } } } } } } } }"""
BOTS = ("bot", "coderabbit", "copilot", "cursor", "sourcery", "codex", "claude", "greptile", "ellipsis")

def threads(repo, n):
    o, r = repo.split("/")
    d = json.loads(subprocess.run(["gh", "api", "graphql", "-f", f"query={Q}", "-F", f"o={o}", "-F", f"r={r}", "-F", f"n={n}"],
                                  capture_output=True, text=True, check=True).stdout)["data"]["repository"]["pullRequest"]
    out = []
    for t in d["reviewThreads"]["nodes"]:
        c = t["comments"]["nodes"][0] if t["comments"]["nodes"] else None
        if not c:
            continue
        who = (c["author"] or {}).get("login", "?")
        bot = (c["author"] or {}).get("__typename") == "Bot" or any(b in who.lower() for b in BOTS)
        replies = [x["body"][:120] for x in t["comments"]["nodes"][1:]]
        out.append(dict(path=t["path"], line=t["line"] or t["originalLine"], resolved=t["isResolved"], outdated=t["isOutdated"],
                        author=who, bot=bot, body=c["body"][:400], commit=(c["originalCommit"] or {}).get("oid", "")[:12], replies=replies))
    return d, out

if __name__ == "__main__":
    a = sys.argv[1:]
    for repo, n in zip(a[::2], a[1::2]):
        d, ts = threads(repo, int(n))
        human = [t for t in ts if not t["bot"]]
        acted = [t for t in ts if t["resolved"] or t["outdated"]]
        print(f"{repo}#{n} {d['title'][:60]} | threads {len(ts)}, by people {len(human)}, resolved or outdated {len(acted)}, "
              f"bots {sorted({t['author'] for t in ts if t['bot']})}")
