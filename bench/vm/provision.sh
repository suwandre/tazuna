#!/bin/bash
# A bench machine from a fresh Ubuntu (26.04 LTS on Vultr VX1, 2026-11-08), run as root: `ssh root@IP 'bash -s' < provision.sh`.
# Same on any provider (Contabo, Hetzner ...): nothing here is tied to Vultr. Stage 1 (this file): updates, a `bench`
# user with the root login's SSH keys, key-only SSH without root, a firewall, automatic security updates, Tailscale.
# After `tailscale up` (a login link to approve once; sign in with the account the other devices use) SSH goes over
# Tailscale only: `provision.sh lockdown` below.
# Toolchains and the bench itself: provision-bench.sh, as the bench user.
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive
# `ssh bench@<tailscale-ip> 'sudo bash -s lockdown' < provision.sh`: once SSH over Tailscale works, the public port
# closes (only the tailscale0 interface stays open) and the hostname is set
if [ "${1:-}" = lockdown ]; then
  ufw allow in on tailscale0 >/dev/null
  ufw delete allow OpenSSH >/dev/null 2>&1 || true
  ufw delete allow 22/tcp >/dev/null 2>&1 || true
  hostnamectl set-hostname tazuna-bench
  ufw status
  exit 0
fi
apt-get update -qq
apt-get -y -qq -o Dpkg::Options::=--force-confnew upgrade
apt-get install -y -qq build-essential git curl jq unzip zip bubblewrap ufw unattended-upgrades python3 python3-venv \
  python3-pip ripgrep tmux rsync htop ca-certificates gnupg >/dev/null

# the bench user: sudo without a password (scripts run unattended), the same SSH keys as root
id bench >/dev/null 2>&1 || adduser --disabled-password --gecos "" bench >/dev/null
usermod -aG sudo bench
echo "bench ALL=(ALL) NOPASSWD:ALL" > /etc/sudoers.d/90-bench && chmod 440 /etc/sudoers.d/90-bench
install -d -m 700 -o bench -g bench /home/bench/.ssh
install -m 600 -o bench -g bench /root/.ssh/authorized_keys /home/bench/.ssh/authorized_keys

# key-only SSH, no root login (checked before the reload: a typo must not lock us out)
cat > /etc/ssh/sshd_config.d/10-bench.conf <<'CONF'
PermitRootLogin no
PasswordAuthentication no
KbdInteractiveAuthentication no
CONF
sshd -t && systemctl reload ssh

# firewall: nothing in but SSH (public until Tailscale is up, then Tailscale only)
ufw default deny incoming >/dev/null && ufw default allow outgoing >/dev/null
ufw allow OpenSSH >/dev/null && ufw --force enable >/dev/null
dpkg-reconfigure -f noninteractive unattended-upgrades

# Tailscale (its own installer; `tailscale up` prints the login link)
command -v tailscale >/dev/null || curl -fsSL https://tailscale.com/install.sh | sh >/dev/null
echo "stage 1 done: run 'sudo tailscale up --hostname tazuna-bench' as bench, approve the link"
