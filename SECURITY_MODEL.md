# tazuna — security model

What tazuna defends, against whom, and what is out of scope. Written for security scans and reviewers (2026-11-23), from the code and the owner's answers. There is no separate infrastructure repository: tazuna has no server side, and its release pipeline is `.github/workflows/` in this repo. Edit it as the design changes.

## System overview and trust boundaries
tazuna is a local coding agent. A Rust CLI (`src/`) drives either the official `claude` CLI (`src/live.rs`) or tazuna's own agent loop against OpenAI-compatible providers (`src/engine.rs`). The Tauri desktop app (`desktop/`) runs the same binary as a sidecar (`tazuna app-server`, JSON lines over stdin/stdout, `src/main.rs`).

Trust boundaries:
1. **Model / content → agent actions**: everything the model reads (repo files, tool and web output, MCP results) is untrusted and can steer the commands the agent runs.
2. **Project folder → rest of the machine**: agent commands should stay in the project unless the user approves (permission modes `ask`/`edits`/`auto`/`full`).
3. **Repository config → code execution**: a cloned repo's hooks, skills, commands and MCP servers run only after the user trusts the folder (`src/trust.rs`).
4. **Desktop window ↔ plugins/panels**: panels run in a sandboxed frame (`tzwidget:`), plugins run in the window only when enabled (`tzplugin:`).
5. **Network → updates/installs**: releases from GitHub replace the binary (`tazuna upgrade`, `install.sh`, `install.ps1`, desktop updater).

## Components
| Component | Code |
|---|---|
| Agent core | `src/main.rs`, `src/live.rs`, `src/engine.rs`, `src/session.rs`, `src/looprun.rs`, `src/route.rs`, `src/ui/`, other `src/*.rs` not listed below |
| Command guards | `src/risky.rs`, `src/sandbox.rs`, `src/review.rs`, `src/verify.rs`, `src/worktree.rs` |
| Repo trust & extensions | `src/trust.rs`, `src/hooks.rs`, `src/mcpc.rs` (user MCP servers, `mcp-proxy`), skills/commands loading |
| MCP tool server | `src/mcp.rs` (`tazuna mcp`: bash/read/write/edit) |
| Credentials & sign-in | `src/oauth.rs`, `src/models.rs` (keys.json), `src/accounts.rs`, MCP OAuth in `src/mcpc.rs`, `src/platform.rs` private files |
| Desktop | `desktop/src-tauri/` (Tauri commands, URI schemes, CSP, updater), `desktop/src/` (React UI, `lib/plugins.ts`, `lib/widgets.ts`) |
| Release & update | `src/versions.rs` (`upgrade_cli`), `install.sh`, `install.ps1`, `release.sh`, `.github/workflows/` |
| Bench tooling | `bench/` (benchmark harness scripts run by the owner with provider keys) |

## Entry points and who can reach them
- Repo content the agent reads (any repository a user opens; anyone who can get a file, issue text or web page in front of the agent).
- Repo config files: `.claude/`, tazuna project settings, `.mcp.json`-style servers, skills/commands — gated by the trust prompt.
- `~/.config/tazuna/mcp.json` remote MCP servers (their responses and OAuth endpoints).
- Provider API responses (own loop) and the `claude` CLI stream.
- Desktop: Tauri commands from the window, `tzplugin:` / `tzwidget:` schemes, plugins in `~/.config/tazuna/desktop/plugins/`, agent-written plugins (creator mode, dry run first).
- `app-server` stdin protocol (local parent process only).
- GitHub releases / `latest.json` (updates), `install.sh` piped from raw.githubusercontent.com.

## Assets (owner: all equally important)
- Provider API keys and OAuth logins: `~/.config/tazuna/keys.json`, `logins/*.json`, `mcp-auth/*.json` (0600).
- The user's machine outside the project (files, credential stores, other repos).
- Release signing: updater key `TAURI_SIGNING_PRIVATE_KEY`, macOS certificate secrets, GitHub token in CI.
- The user's project code and git history (no unapproved destructive git ops, no exfiltration).

## Attacker profiles (owner's answers)
- **Users**: today only the owner, but the repo is public and anyone can install it — treat users as the general public (owner, 2026-11-23).
- The owner said all of these matter (2026-11-23):
  - Malicious repository content / prompt injection steering the agent.
  - Malicious or compromised MCP servers and desktop plugins.
  - Supply chain: releases, install scripts, auto-update, CI secrets.
  - Other local users or processes on the same machine.

## Existing controls
- Trust prompt per repo before its hooks/skills/commands/MCP servers run (`src/trust.rs`); hooks live only in the user's config (`src/hooks.rs`).
- `auto` mode asks before risky commands via a lexical parse (`src/risky.rs`; documented gaps: symlinks, aliases, scripts the agent wrote, PowerShell).
- Review mode on macOS runs commands in a Seatbelt sandbox: no network, writes only to project/temp, no credential stores, no writes to `.git` or run-later files, secret env vars unset (`src/sandbox.rs`).
- Credential files written 0600 via create-new + rename (`src/platform.rs`); OAuth tokens never put in the environment; MCP OAuth uses PKCE and binds tokens to server URL.
- Desktop CSP restricts scripts to `self` + `tzplugin:`; panels sandboxed without same origin; plugin code only served while the exact enabled code matches; dry run before enabling agent-written plugins.
- Desktop updates signed with the updater key (pubkey in `tauri.conf.json`); CI actions pinned by commit in places.

## Out of scope / accepted risk (owner)
- Nothing ruled out: everything, including `bench/`, `full` permission mode, Linux/Windows lacking the review sandbox, and the opt-in sign-ins using another app's OAuth identity, is in scope (owner, 2026-11-23).

## Open questions
- CLI self-update (`upgrade_cli`) and `install.sh`/`install.ps1`: is there any checksum/signature check on the CLI tarball, or only HTTPS from GitHub? (to verify in the code)
- Which provider endpoints are reachable over plain HTTP (e.g. local OmniRoute at `localhost:20128`) and whether any other local process can intercept keys sent there.
