#!/bin/bash
# Releases the desktop app's version (src-tauri/tauri.conf.json, package.json and src-tauri/Cargo.toml must agree): tags
# desktop-vX.Y.Z and pushes the tag. CI (.github/workflows/desktop.yml) builds macOS, Linux and Windows, signs the update
# packages and writes latest.json, which installed apps read (they offer the update in the sidebar). Run on main,
# committed and pushed.
set -euo pipefail
cd "$(dirname "$0")"
v=$(python3 -c 'import json; print(json.load(open("src-tauri/tauri.conf.json"))["version"])')
[ "$(node -p 'require("./package.json").version')" = "$v" ] || { echo "package.json is not at $v"; exit 1; }
grep -q "^version = \"$v\"" src-tauri/Cargo.toml || { echo "src-tauri/Cargo.toml is not at $v"; exit 1; }
[ "$(git branch --show-current)" = main ] || { echo "release from main"; exit 1; }
[ -z "$(git status --porcelain -- . ../src ../Cargo.toml ../Cargo.lock)" ] || { echo "the app or the core has uncommitted changes"; exit 1; }
git fetch -q origin main
[ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] || { echo "push main first"; exit 1; }
git ls-remote --exit-code --tags origin "refs/tags/desktop-v$v" >/dev/null && { echo "desktop-v$v is tagged already: bump the version"; exit 1; }
git tag "desktop-v$v" && git push -q origin "desktop-v$v"
echo "desktop-v$v pushed: CI builds, signs and publishes it (gh run watch); installed apps offer it within the hour, or Check for Updates"
