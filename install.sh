#!/bin/sh
# tazuna's installer for macOS and Linux: the newest release for this machine into ~/.local/bin (TAZUNA_INSTALL_DIR
# elsewhere). Later `tazuna update` installs newer ones.
#   curl -fsSL https://raw.githubusercontent.com/suwandre/tazuna/main/install.sh | sh
set -eu
repo=suwandre/tazuna
dir="${TAZUNA_INSTALL_DIR:-$HOME/.local/bin}"
case "$(uname -s)-$(uname -m)" in
  Darwin-arm64) target=aarch64-apple-darwin ;;
  Darwin-x86_64) target=x86_64-apple-darwin ;;
  Linux-x86_64) target=x86_64-unknown-linux-gnu ;;
  Linux-aarch64 | Linux-arm64) target=aarch64-unknown-linux-gnu ;;
  *)
    echo "no tazuna build for $(uname -s) $(uname -m): build it from source (cargo install --git https://github.com/$repo)" >&2
    echo "on Windows: irm https://raw.githubusercontent.com/$repo/main/install.ps1 | iex" >&2
    exit 1
    ;;
esac
mkdir -p "$dir"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
curl -fsSL "https://github.com/$repo/releases/latest/download/tazuna-$target.tar.gz" | tar -xz -C "$tmp"
chmod 755 "$tmp/tazuna"
# a rename, so a running tazuna keeps its old binary
mv -f "$tmp/tazuna" "$dir/tazuna"
echo "installed $("$dir/tazuna" --version) in $dir"
case ":$PATH:" in
  *":$dir:"*) ;;
  *) echo "$dir is not on your PATH: add this line to your shell profile (~/.zshrc, ~/.bashrc):  export PATH=\"$dir:\$PATH\"" ;;
esac
command -v claude >/dev/null 2>&1 || echo "tazuna drives the claude CLI: install Claude Code and log in (https://claude.com/claude-code)"
