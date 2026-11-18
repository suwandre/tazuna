//! `/usage`: what each account has left, in one place (2026-11-18). Claude's plan from the official CLI's own `/usage`
//! page (`claude -p /usage`: no model call, $0; until now tazuna made a Haiku call to read the plan from its rate-limit
//! event), the providers' credits where their API tells (SubRouter, OpenRouter; the others' pages are linked), and
//! OmniRoute's connected accounts with their quota windows (its /api/usage endpoints, with tazuna's OmniRoute key).

use serde_json::{Value, json};
use std::time::Duration;

/// One of the plan's windows: "Current session", 14 (% used), "Nov 18 at 5:29pm (Europe/Berlin)".
pub struct PlanRow {
    pub label: String,
    pub used: f64,
    pub resets: String,
}

/// Claude's plan as `claude -p /usage` says it: its windows and what it says contributes to the usage. None: the CLI is
/// not there, not logged in to a subscription, or said nothing readable.
pub fn claude_plan() -> Option<(Vec<PlanRow>, Vec<String>)> {
    // no project's settings or MCP servers (their hooks and commands would run), from the home folder: on Linux the
    // temporary folder is shared, and another user could leave a .claude/ or .mcp.json there (security review, 2026-11-18)
    let out = std::process::Command::new("claude")
        .args(["-p", "/usage", "--output-format", "json", "--no-session-persistence", "--setting-sources", "", "--strict-mcp-config"])
        .current_dir(crate::platform::home()?).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).output().ok()?;
    let v: Value = serde_json::from_slice(&out.stdout).ok()?;
    parse_plan(v["result"].as_str()?)
}

fn parse_plan(text: &str) -> Option<(Vec<PlanRow>, Vec<String>)> {
    let mut rows = vec![];
    for l in text.lines().map(str::trim) {
        let Some((label, rest)) = l.split_once(": ").filter(|(k, _)| k.starts_with("Current ")) else { continue };
        let Some(used) = rest.split('%').next().and_then(|n| n.trim().parse::<f64>().ok()) else { continue };
        let resets = rest.split_once("resets ").map_or(String::new(), |(_, r)| r.trim().to_string());
        rows.push(PlanRow { label: label.to_string(), used, resets });
    }
    let notes: Vec<String> = text.split_once("What's contributing to your usage?")
        .map(|(_, rest)| rest.lines().map(|l| l.trim_end().to_string()).filter(|l| !l.trim().is_empty()).collect())
        .unwrap_or_default();
    (!rows.is_empty()).then_some((rows, notes))
}

/// The plan's 5-hour and weekly use as claude's rate-limit event carries them, for the meters a session shows.
pub fn rate_event(rows: &[PlanRow]) -> Option<Value> {
    let find = |p: &str| rows.iter().find(|r| r.label.to_lowercase().contains(p)).map(|r| r.used / 100.0);
    Some(json!({"type": "rate_limit_event", "rate_limit_info": {"unifiedWindows": {
        "five_hour": {"utilization": find("session")?}, "seven_day": {"utilization": find("all models").or_else(|| find("week"))?}}}}))
}

fn http(secs: u64) -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(secs))).build().into()
}

fn get(agent: &ureq::Agent, url: &str, key: &str) -> Option<Value> {
    let mut r = agent.get(url).header("Authorization", format!("Bearer {}", key.trim())).call().ok()?;
    serde_json::from_str(&r.body_mut().read_to_string().ok()?).ok()
}

/// Each provider with a key: what is left where its API says (SubRouter, OpenRouter), else its page to look.
pub fn credits() -> Vec<Value> {
    let mut out = vec![];
    for l in crate::models::LOGINS.iter().filter(|l| l.name != "OmniRoute") {
        let Some(key) = l.vars.iter().find_map(|v| crate::models::key(v)) else { continue };
        let left = match l.name {
            "SubRouter" => crate::models::subrouter_balance().map(|b| format!("${b:.2} left")),
            "OpenRouter" => get(&http(15), "https://openrouter.ai/api/v1/credits", &key).and_then(|v| {
                let (total, used) = (v["data"]["total_credits"].as_f64()?, v["data"]["total_usage"].as_f64()?);
                let left = total - used;
                Some(format!("{}${:.2} left of ${total:.2}", if left < 0.0 { "-" } else { "" }, left.abs()))
            }),
            _ => None,
        };
        out.push(json!({"provider": l.name, "left": left, "page": l.page}));
    }
    out
}

/// OmniRoute's connected accounts: each one's provider, name, plan and quota windows (used %, when they reset). Empty
/// without its key, or when OmniRoute is not running.
pub fn omniroute() -> Vec<Value> {
    let Some(key) = crate::models::key("OMNIROUTE_API_KEY") else { return vec![] };
    let agent = http(25);
    let Some(list) = get(&agent, "http://localhost:20128/api/usage/quota", &key) else { return vec![] };
    list["providers"].as_array().into_iter().flatten().map(|p| {
        let id = p["connectionId"].as_str().unwrap_or_default();
        let detail = (!id.is_empty()).then(|| get(&agent, &format!("http://localhost:20128/api/usage/{id}"), &key)).flatten().unwrap_or_default();
        let mut windows = vec![];
        for g in detail["quotaGroups"].as_array().into_iter().flatten() {
            for (_, w) in g["windows"].as_object().into_iter().flatten() {
                let used = w["remainingPercentage"].as_f64().map(|r| 100.0 - r)
                    .or_else(|| Some(w["used"].as_f64()? / w["total"].as_f64().filter(|t| *t > 0.0)? * 100.0));
                windows.push(json!({"group": g["displayName"].as_str().or(g["id"].as_str()), "window": w["window"], "used": used,
                    "resets": w["resetAt"], "unlimited": w["unlimited"]}));
            }
        }
        if windows.is_empty() && let Some(r) = p["percentRemaining"].as_f64() {
            windows.push(json!({"group": "", "window": "quota", "used": 100.0 - r, "resets": p["resetAt"], "unlimited": false}));
        }
        json!({"provider": p["provider"], "name": p["name"], "plan": detail["plan"], "status": p["tokenStatus"], "windows": windows})
    }).collect()
}

/// All of it, for the desktop's usage page (the app server's `accounts` op).
pub fn all() -> Value {
    let plan = claude_plan();
    json!({
        "plan": plan.as_ref().map(|(rows, _)| rows.iter().map(|r| json!({"label": r.label, "used": r.used, "resets": r.resets})).collect::<Vec<_>>()),
        "plan_notes": plan.as_ref().map(|(_, n)| n.clone()).unwrap_or_default(),
        "credits": credits(),
        "omniroute": omniroute(),
    })
}

/// A bar of `width` cells for `used` %.
fn bar(used: f64, width: usize) -> String {
    let full = ((used.clamp(0.0, 100.0) / 100.0) * width as f64).round() as usize;
    format!("{}{}", "█".repeat(full), "░".repeat(width - full))
}

/// The same as lines, for the terminal's /usage panel.
pub fn lines(v: &Value) -> Vec<String> {
    let mut out = vec!["Claude plan (claude's own /usage, no model call)".to_string()];
    match v["plan"].as_array() {
        Some(rows) => {
            for r in rows {
                let used = r["used"].as_f64().unwrap_or(0.0);
                out.push(format!("  {:<28} {} {:>3.0}% used · resets {}", r["label"].as_str().unwrap_or_default(), bar(used, 20), used, r["resets"].as_str().unwrap_or("?")));
            }
            for n in v["plan_notes"].as_array().into_iter().flatten().filter_map(Value::as_str).take(8) {
                out.push(format!("  {n}"));
            }
        }
        None => out.push("  no reading: is claude installed and logged in to a subscription?".into()),
    }
    let credits = v["credits"].as_array().cloned().unwrap_or_default();
    if !credits.is_empty() {
        out.push(String::new());
        out.push("Providers' credits".into());
        for c in &credits {
            let left = c["left"].as_str().map_or(format!("see {}", c["page"].as_str().unwrap_or("its page")), String::from);
            out.push(format!("  {:<18} {left}", c["provider"].as_str().unwrap_or_default()));
        }
    }
    let accounts = v["omniroute"].as_array().cloned().unwrap_or_default();
    if !accounts.is_empty() {
        out.push(String::new());
        out.push("OmniRoute accounts".into());
        for a in &accounts {
            let plan = a["plan"].as_str().map_or(String::new(), |p| format!(" · {p}"));
            out.push(format!("  {} · {}{plan}{}", a["provider"].as_str().unwrap_or("?"), a["name"].as_str().unwrap_or_default(),
                if a["status"] == "valid" { String::new() } else { format!(" · login {}", a["status"].as_str().unwrap_or("?")) }));
            for w in a["windows"].as_array().into_iter().flatten() {
                let used = w["used"].as_f64().unwrap_or(0.0);
                let what = format!("{} {}", w["group"].as_str().unwrap_or_default(), w["window"].as_str().unwrap_or_default()).trim().to_string();
                let resets = w["resets"].as_str().map_or(String::new(), |r| format!(" · resets {}", r.replace('T', " ").trim_end_matches(".000Z").trim_end_matches('Z')));
                out.push(format!("    {what:<26} {} {used:>3.0}% used{resets}", if w["unlimited"] == true { "unlimited           ".into() } else { bar(used, 20) }));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_usage_page_reads_as_windows_and_notes() {
        let text = "You are currently using your subscription to power your Claude Code usage\n\nCurrent session: 14% used · resets Nov 18 at 5:29pm (Europe/Berlin)\nCurrent week (all models): 78% used · resets Nov 21 at 2:59am (Europe/Berlin)\nCurrent week (Fable): 64% used · resets Nov 21 at 2:59am (Europe/Berlin)\n\nWhat's contributing to your usage?\nApproximate · sessions on this machine only\n\n  95%  Start fresh when you switch tasks · run /clear\n";
        let (rows, notes) = parse_plan(text).unwrap();
        assert_eq!(rows.iter().map(|r| (r.label.as_str(), r.used)).collect::<Vec<_>>(), [("Current session", 14.0), ("Current week (all models)", 78.0), ("Current week (Fable)", 64.0)]);
        assert_eq!(rows[0].resets, "Nov 18 at 5:29pm (Europe/Berlin)");
        assert!(notes.iter().any(|n| n.contains("Start fresh")));
        let e = rate_event(&rows).unwrap();
        assert_eq!((e["rate_limit_info"]["unifiedWindows"]["five_hour"]["utilization"].as_f64(), e["rate_limit_info"]["unifiedWindows"]["seven_day"]["utilization"].as_f64()), (Some(0.14), Some(0.78)));
        let shown = lines(&json!({"plan": [{"label": "Current session", "used": 14.0, "resets": "5:29pm"}], "credits": [{"provider": "Mortera", "left": null, "page": "https://mortera.cloud"}],
            "omniroute": [{"provider": "agy", "name": "a", "plan": "Ultra", "status": "valid", "windows": [{"group": "Gemini Models", "window": "weekly", "used": 1.4, "resets": "2026-11-25T11:03:10.000Z"}]}]}));
        assert!(shown.iter().any(|l| l.contains("Current session") && l.contains("14% used")) && shown.iter().any(|l| l.contains("see https://mortera.cloud")));
        assert!(shown.iter().any(|l| l.contains("Gemini Models weekly") && l.contains("resets 2026-11-25 11:03:10")));
    }
}
