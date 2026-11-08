#!/bin/bash
# Stage 2 of a bench machine, as the bench user: `ssh bench@<tailscale-ip> 'bash -s' < provision-bench.sh`. Toolchains at
# the Mac's versions (2026-11-08: Go 1.27.1, Rust stable, Node 26), the GitHub CLI, the official `claude` CLI (its own
# installer), tazuna from the public repo (built), and the open-source bench repos cloned from GitHub into ~/bench-repos.
# Keys, the Claude bench token and company material are copied over separately (never in this repo).
set -euo pipefail
GO=1.27.1 NODE=26.9.0
cd ~
if ! /usr/local/go/bin/go version 2>/dev/null | grep -q "go$GO "; then
  curl -fsSL "https://go.dev/dl/go$GO.linux-amd64.tar.gz" -o /tmp/go.tgz && sudo rm -rf /usr/local/go && sudo tar -C /usr/local -xzf /tmp/go.tgz && rm /tmp/go.tgz
fi
command -v rustup >/dev/null || curl -fsSL https://sh.rustup.rs | sh -s -- -y -q --profile minimal >/dev/null
if ! ~/.local/node/bin/node --version 2>/dev/null | grep -q "v$NODE"; then
  mkdir -p ~/.local/node && curl -fsSL "https://nodejs.org/dist/v$NODE/node-v$NODE-linux-x64.tar.xz" | tar -xJ -C ~/.local/node --strip-components=1
fi
if ! command -v gh >/dev/null; then
  sudo install -d -m 755 /etc/apt/keyrings
  curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg | sudo tee /etc/apt/keyrings/githubcli-archive-keyring.gpg >/dev/null
  echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" | sudo tee /etc/apt/sources.list.d/github-cli.list >/dev/null
  sudo apt-get update -qq && sudo apt-get install -y -qq gh >/dev/null
fi
command -v claude >/dev/null || curl -fsSL https://claude.ai/install.sh | bash >/dev/null
grep -q 'tazuna bench PATH' ~/.profile || cat >> ~/.profile <<'P'
# tazuna bench PATH
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$HOME/.local/node/bin:/usr/local/go/bin:$HOME/go/bin:$PATH"
P
. ~/.profile
[ -d ~/tazuna ] || git clone -q https://github.com/suwandre/tazuna.git ~/tazuna
(cd ~/tazuna && git pull -q && cargo build -q --release)
mkdir -p ~/bench-repos
for r in caddyserver/caddy go-gitea/gitea gohugoio/hugo grafana/k6 jesseduffield/lazygit prometheus/prometheus rclone/rclone influxdata/telegraf hashicorp/terraform; do
  [ -d ~/bench-repos/${r#*/} ] || git clone -q "https://github.com/$r.git" ~/bench-repos/${r#*/} &
done
wait
echo "go $(go version | cut -d' ' -f3) | $(rustc --version | cut -d' ' -f1-2) | node $(node --version) | $(gh --version | head -1 | cut -d' ' -f1-3) | claude $(claude --version 2>/dev/null) | $(~/tazuna/target/release/tazuna --version) | repos: $(ls ~/bench-repos | tr '\n' ' ')"
