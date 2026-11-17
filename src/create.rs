//! `/create`: tazuna builds what the user asks for as one of three kinds of behaviour, in a folder of its own, checks it,
//! and the user turns it on after reading it.
//! - a command: a prompt the user runs with /name (behave.rs's commands);
//! - a skill: know-how the agent reads when a task fits (behave.rs's skills);
//! - a tool: a small MCP server the agent can call (tazuna's /mcp list, which reaches every model).
//!
//! It is made under ~/.config/tazuna/behave-new/<id>/ and goes live only on `/create enable`: commands and skills into the
//! creations folder (behave.rs), a tool into ~/.config/tazuna/behave/tools/<name>/ and the MCP list. Nothing made runs
//! before that, except a tool's server started once to list its tools when the session runs without questions (auto
//! mode), where the agent could have started it anyway.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// Where creations wait for the user.
pub fn staging() -> Option<PathBuf> {
    crate::platform::home().map(|h| h.join(".config/tazuna/behave-new"))
}

/// A new, empty folder for one creation.
pub fn new_dir() -> Result<PathBuf> {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
    let dir = staging().context("no home folder")?.join(format!("c{now:x}"));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// The message that asks the agent for it.
pub fn spec(ask: &str, dir: &Path) -> String {
    let d = dir.display();
    format!("Make what the user asks for below, as exactly one of these, inside the folder {d} (write nothing outside it):

- A command: a prompt the user runs by typing /name. File {d}/commands/<name>.md: a frontmatter with `description:` (one \
line) and, if it takes input, `argument-hint:`; then the prompt itself. $ARGUMENTS stands for what the user types after \
/name, $1, $2 for its words.
- A skill: know-how for a kind of task, which the agent reads by itself when a task fits. File {d}/skills/<name>/SKILL.md: \
a frontmatter with `name:` and `description:` (one line saying when to use it), then the instructions. Scripts it uses go \
beside it in that folder.
- A tool: a small MCP server (stdio, JSON-RPC 2.0, protocol 2025-06-18) whose tools the agent can call. Folder \
{d}/tools/<name>/ with the server (Python 3's standard library only, or Node.js without packages) and \
{d}/tools/<name>/tool.json: {{\"command\": \"python3\", \"args\": [\"server.py\"]}} (paths relative to that folder). It must \
answer initialize, tools/list and tools/call. Start it yourself and test those before you finish.

Pick the kind that fits: a prompt the user triggers is a command; know-how the agent should apply on its own is a skill; \
anything that needs code, live data or another service is a tool. <name>: lowercase letters, digits and -. No secret in \
any file: a tool reads a key from an environment variable and says which. Keep it small.

Last, write {d}/creation.json: {{\"kind\": \"command\" or \"skill\" or \"tool\", \"name\": \"<name>\", \"summary\": \"<one line: \
what it does>\"}}.

The user's request: {ask}")
}

/// What a check found ready.
#[derive(Debug, PartialEq)]
pub struct Made {
    pub kind: String,
    pub name: String,
    pub summary: String,
    /// a tool's tools, as its server listed them (empty when it was not started)
    pub tools: Vec<String>,
}

fn valid(name: &str) -> bool {
    !name.is_empty() && name.len() <= 40 && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// What looks like a key written into a file (sk-..., or a long token after key/token/secret =): a creation holds none.
fn holds_key(text: &str) -> bool {
    let lower = text.to_lowercase();
    let long_run = |s: &str| s.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').count() >= 24;
    lower.match_indices("sk-").any(|(i, _)| long_run(&text[i + 3..]))
        || ["key", "token", "secret"].iter().any(|w| lower.match_indices(w).any(|(i, _)| {
            let after = text[i + w.len()..].trim_start_matches(|c: char| c == '_' || c == '"' || c == '\'' || c.is_whitespace());
            after.strip_prefix('=').or_else(|| after.strip_prefix(':')).map(|v| v.trim_start().trim_start_matches(['"', '\''])).is_some_and(long_run)
        }))
}

/// The creation in `dir` checked: its creation.json, its files in the right place and form, no key in any file; a tool's
/// server started and its tools listed when `run_tool` (the session asks nothing). Err: what is wrong, for the agent.
pub fn check(dir: &Path, run_tool: bool) -> std::result::Result<Made, String> {
    let meta: Value = std::fs::read_to_string(dir.join("creation.json")).ok().and_then(|t| serde_json::from_str(&t).ok())
        .ok_or("there is no readable creation.json in the folder")?;
    let (kind, name, summary) = (meta["kind"].as_str().unwrap_or_default(), meta["name"].as_str().unwrap_or_default(), meta["summary"].as_str().unwrap_or_default());
    if !valid(name) {
        return Err(format!("creation.json's name \"{name}\" must be lowercase letters, digits and -"));
    }
    for e in walk(dir) {
        if std::fs::read_to_string(&e).is_ok_and(|t| holds_key(&t)) {
            return Err(format!("{} holds what looks like a key or token: read it from an environment variable instead", e.display()));
        }
    }
    let front = |p: &Path| std::fs::read_to_string(p).ok().map(|t| crate::behave::frontmatter(&t).0);
    let mut tools = vec![];
    match kind {
        "command" => {
            let f = front(&dir.join("commands").join(format!("{name}.md"))).ok_or(format!("commands/{name}.md is missing"))?;
            if crate::behave::field(&f, "description").is_empty() {
                return Err(format!("commands/{name}.md needs a frontmatter with description:"));
            }
        }
        "skill" => {
            let f = front(&dir.join("skills").join(name).join("SKILL.md")).ok_or(format!("skills/{name}/SKILL.md is missing"))?;
            if crate::behave::field(&f, "name").is_empty() || crate::behave::field(&f, "description").is_empty() {
                return Err(format!("skills/{name}/SKILL.md needs a frontmatter with name: and description:"));
            }
        }
        "tool" => {
            let spec = server(dir, name).map_err(|e| format!("{e:#}"))?;
            if run_tool {
                let mut c = crate::mcpc::Client::connect(name, &spec).map_err(|e| format!("the server did not start or answer initialize: {e:#}"))?;
                tools = c.tools().map_err(|e| format!("the server did not answer tools/list: {e:#}"))?.iter()
                    .filter_map(|t| t["name"].as_str().map(String::from)).collect();
                if tools.is_empty() {
                    return Err("the server lists no tools".into());
                }
            }
        }
        _ => return Err(format!("creation.json's kind \"{kind}\" must be command, skill or tool")),
    }
    Ok(Made { kind: kind.into(), name: name.into(), summary: summary.into(), tools })
}

/// A tool's MCP server as mcp.json takes it, its arguments made absolute (they are relative to its folder).
fn server(dir: &Path, name: &str) -> Result<Value> {
    let folder = dir.join("tools").join(name);
    let t: Value = serde_json::from_str(&std::fs::read_to_string(folder.join("tool.json")).with_context(|| format!("tools/{name}/tool.json is missing"))?)
        .with_context(|| format!("tools/{name}/tool.json is not JSON"))?;
    let command = t["command"].as_str().filter(|c| !c.is_empty()).with_context(|| format!("tools/{name}/tool.json needs a command"))?;
    let args: Vec<String> = t["args"].as_array().into_iter().flatten().filter_map(Value::as_str)
        .map(|a| if folder.join(a).exists() { folder.join(a).to_string_lossy().into_owned() } else { a.to_string() }).collect();
    Ok(json!({"command": command, "args": args}))
}

/// Every file under `dir` (to look for keys), at most a few hundred.
fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = vec![];
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() && !p.ends_with("node_modules") && !p.ends_with("__pycache__") {
                todo.push(p);
            } else if p.is_file() && out.len() < 500 {
                out.push(p);
            }
        }
    }
    out
}

/// The creation in `dir` turned on, its folder in staging removed; what the user can do with it now.
pub fn enable(dir: &Path) -> Result<String> {
    let made = check(dir, false).map_err(|e| anyhow::anyhow!(e))?;
    let live = crate::behave::creations().context("no home folder")?;
    let name = &made.name;
    let summary = made.summary.trim_end_matches('.');
    let said = match made.kind.as_str() {
        "command" => {
            std::fs::create_dir_all(live.join("commands"))?;
            std::fs::copy(dir.join("commands").join(format!("{name}.md")), live.join("commands").join(format!("{name}.md")))?;
            format!("/{name} works now, on every model: {summary}")
        }
        "skill" => {
            copy_dir(&dir.join("skills").join(name), &live.join("skills").join(name))?;
            format!("the skill {name} is on, for every model: {summary}")
        }
        "tool" => {
            let to = live.join("tools").join(name);
            copy_dir(&dir.join("tools").join(name), &to)?;
            let spec = server(&live, name)?;
            let mut target = vec![spec["command"].as_str().unwrap_or_default().to_string()];
            target.extend(spec["args"].as_array().into_iter().flatten().filter_map(Value::as_str).map(String::from));
            crate::mcpc::add(name, &target.iter().map(String::as_str).collect::<Vec<_>>())?;
            format!("the tool server {name} is in your MCP list (/mcp on reaches every model with it): {summary}")
        }
        k => bail!("unknown kind {k}"),
    };
    let _ = std::fs::remove_dir_all(dir);
    Ok(said)
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    if to.exists() {
        std::fs::remove_dir_all(to)?;
    }
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)?.flatten() {
        let (p, q) = (e.path(), to.join(e.file_name()));
        if e.file_type()?.is_symlink() {
            continue; // a creation's link could point anywhere: not carried over
        } else if p.is_dir() {
            copy_dir(&p, &q)?;
        } else {
            std::fs::copy(&p, &q)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_creation_is_checked_before_it_can_go_live() {
        let dir = std::env::temp_dir().join(format!("tazuna-create-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("commands")).unwrap();
        assert!(check(&dir, false).unwrap_err().contains("creation.json"));
        std::fs::write(dir.join("creation.json"), r#"{"kind": "command", "name": "standup", "summary": "today's standup"}"#).unwrap();
        assert!(check(&dir, false).unwrap_err().contains("commands/standup.md is missing"));
        std::fs::write(dir.join("commands/standup.md"), "Write my standup.\n").unwrap();
        assert!(check(&dir, false).unwrap_err().contains("description"));
        std::fs::write(dir.join("commands/standup.md"), "---\ndescription: Write today's standup\n---\nWrite my standup from git log.\n").unwrap();
        assert_eq!(check(&dir, false).unwrap().name, "standup");
        std::fs::write(dir.join("commands/notes.txt"), "api_key = \"sk-abcdefghijklmnopqrstuvwxyz123456\"\n").unwrap();
        assert!(check(&dir, false).unwrap_err().contains("looks like a key"), "no key in any file");
        std::fs::remove_file(dir.join("commands/notes.txt")).unwrap();
        std::fs::write(dir.join("creation.json"), r#"{"kind": "command", "name": "../evil", "summary": "x"}"#).unwrap();
        assert!(check(&dir, false).unwrap_err().contains("lowercase"));
        assert!(!holds_key("read the key from the environment: API_KEY") && !holds_key("token: ${GITHUB_TOKEN}"));
        // a tool: its server started and its tools listed
        #[cfg(unix)]
        {
            std::fs::create_dir_all(dir.join("tools/echo")).unwrap();
            std::fs::write(dir.join("tools/echo/server.sh"), r#"while read -r line; do
id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9]*\).*/\1/p')
case "$line" in
*initialize*) echo "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"protocolVersion\":\"2025-06-18\",\"capabilities\":{\"tools\":{}},\"serverInfo\":{\"name\":\"echo\",\"version\":\"1\"}}}" ;;
*tools/list*) echo "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"tools\":[{\"name\":\"say\",\"inputSchema\":{\"type\":\"object\"}}]}}" ;;
esac
done
"#).unwrap();
            std::fs::write(dir.join("tools/echo/tool.json"), r#"{"command": "sh", "args": ["server.sh"]}"#).unwrap();
            std::fs::write(dir.join("creation.json"), r#"{"kind": "tool", "name": "echo", "summary": "says it back"}"#).unwrap();
            assert_eq!(check(&dir, false).unwrap().tools, Vec::<String>::new(), "not started unless asked");
            assert_eq!(check(&dir, true).unwrap().tools, vec!["say"]);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
