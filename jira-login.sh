#!/bin/bash
# Sets up tazuna's /jira in one go: opens Atlassian's API token page, takes the pasted token (hidden), writes
# ~/.atlassian-token (url=, email=, token=; readable by you only) and checks the login with one read-only call.
# Site and email come from the existing file, or are asked for. Run it again to replace an expired token.
set -euo pipefail
f=~/.atlassian-token
get() { sed -n "s/^$1=//p" "$f" 2>/dev/null | head -1; }
url=$(get url)
email=$(get email)
[ -n "$url" ] || read -rp "Atlassian site (https://<name>.atlassian.net): " url
[ -n "$email" ] || read -rp "Atlassian email: " email
url=${url%/}

echo "Site $url, email $email."
echo "On the page that opens: Create API token (the plain one, not \"with scopes\"), any name, then Copy."
open "https://id.atlassian.com/manage-profile/security/api-tokens" 2>/dev/null || echo "Open https://id.atlassian.com/manage-profile/security/api-tokens"
read -rsp "Paste the token here (hidden), then enter: " token
echo
token=$(printf '%s' "$token" | tr -d '[:space:]')
[ -n "$token" ] || { echo "No token given; nothing changed."; exit 1; }

# the token goes to curl on stdin, not on its command line (where ps would show it)
body=$(mktemp)
trap 'rm -f "$body"' EXIT
code=$(printf 'user = "%s:%s"\n' "$email" "$token" | curl -s --config - -o "$body" -w '%{http_code}' -H 'Accept: application/json' "$url/rest/api/2/myself") || code=000
if [ "$code" != 200 ]; then
  echo "Jira answered HTTP $code: the token was not saved. 401: check the email and that the whole token was pasted; 000: the site is not reachable."
  exit 1
fi

umask 077
printf 'url=%s\nemail=%s\ntoken=%s\n' "$url" "$email" "$token" > "$f.new"
mv "$f.new" "$f"
chmod 600 "$f"
name=$(sed -n 's/.*"displayName":"\([^"]*\)".*/\1/p' "$body")
echo "Saved. Jira knows you as ${name:-$email}. In tazuna: /jira ABC-123 [what to do]"
