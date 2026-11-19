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

/// An account's plan as a reader says it: OmniRoute's "default_claude_max_5x" is "Claude Max 5x"; "Ultra" stays.
fn plan_name(id: &str) -> String {
    id.trim_start_matches("default_").split('_').filter(|w| !w.is_empty())
        .map(|w| { let mut c = w.chars(); c.next().map_or(String::new(), |f| f.to_uppercase().chain(c).collect()) }).collect::<Vec<_>>().join(" ")
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
                // spent past its credit: "$0.17 over", where it read "-$0.17 left of $0.00" (design review 4)
                Some(if left < 0.0 { format!("${:.2} over its credit", -left) } else { format!("${left:.2} left of ${total:.2}") })
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
        json!({"provider": p["provider"], "name": p["name"], "plan": detail["plan"].as_str().map(plan_name), "status": p["tokenStatus"], "windows": windows})
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

/// Seconds since 1970 of an ISO time in UTC ("2026-11-25T11:03:10.000Z"; days from the civil date, as in Howard
/// Hinnant's algorithm).
fn iso_secs(iso: &str) -> Option<i64> {
    let n = |a: usize, b: usize| iso.get(a..b)?.parse::<i64>().ok();
    let (y, m, d, hh, mm, ss) = (n(0, 4)?, n(5, 7)?, n(8, 10)?, n(11, 13)?, n(14, 16)?, n(17, 19)?);
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let days = era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468;
    Some(days * 86_400 + hh * 3600 + mm * 60 + ss)
}

/// "in 6 d 23 h", "in 3 h 12 min", "in 4 min" from `now` (seconds since 1970) to an ISO time: no time zone to get wrong.
fn until(iso: &str, now: i64) -> Option<String> {
    let s = (iso_secs(iso)? - now).max(0);
    let (d, h, m) = (s / 86_400, s % 86_400 / 3600, s % 3600 / 60);
    Some(if d > 0 { format!("in {d} d {h} h") } else if h > 0 { format!("in {h} h {m} min") } else { format!("in {m} min") })
}

/// claude's window names, shorter: "Current session" is the 5-hour one, "Current week (Fable)" the week on Fable.
fn window_name(label: &str) -> String {
    let l = label.trim_start_matches("Current ");
    match l.split_once(" (") {
        _ if l == "session" => "5-hour session".into(),
        Some((w, rest)) => format!("{}{} · {}", w[..1].to_uppercase(), &w[1..], rest.trim_end_matches(')')),
        None => format!("{}{}", l[..1].to_uppercase(), &l[1..]),
    }
}

/// Where a provider's page link starts in its row: after the two-space indent and the name's column.
pub const LINK_FROM: usize = 22;

/// The terminal's /usage panel: a section per kind of account (a heading, a line across), the meters' bars aligned,
/// notes dimmed; a row's link (a provider's page), opened by a click.
pub fn lines(v: &Value) -> Vec<(String, crate::ui::Tone, Option<String>)> {
    use crate::ui::Tone::{Bad, Dim, Head, Plain, Rule};
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    let mut out: Vec<(String, crate::ui::Tone, Option<String>)> = vec![];
    let mut push = |text: String, tone, link: Option<String>| out.push((text, tone, link));
    // a meter's row: its name from `indent`, its bar at one column for all (column 35: "Claude & GPT Models · weekly" fits)
    let meter = |indent: usize, name: &str, used: f64, resets: &str| {
        format!("{:indent$}{name:<w$} {} {used:>4.0}%{}", "", bar(used, 20), if resets.is_empty() { String::new() } else { format!("   resets {resets}") }, w = 34 - indent)
    };
    push("Claude plan".into(), Head, None);
    push(String::new(), Rule, None);
    match v["plan"].as_array() {
        Some(rows) => {
            for r in rows {
                let used = r["used"].as_f64().unwrap_or(0.0);
                // "Nov 18 at 5:29pm (Europe/Berlin)": the zone is the user's own
                let resets = r["resets"].as_str().unwrap_or_default().split(" (").next().unwrap_or_default();
                push(meter(2, &window_name(r["label"].as_str().unwrap_or_default()), used, resets), if used >= 90.0 { Bad } else { Plain }, None);
            }
            // what claude says contributes, for the last day (its last 7 days repeat the same advice)
            let notes: Vec<&str> = v["plan_notes"].as_array().into_iter().flatten().filter_map(Value::as_str)
                .take_while(|n| !n.trim_start().starts_with("Last 7d")).filter(|n| !n.starts_with("Approximate")).collect();
            if !notes.is_empty() {
                push(String::new(), Plain, None);
                push("  What's contributing (approximate, this machine only)".into(), Dim, None);
                for n in notes {
                    push(format!("    {}", n.trim()), Dim, None);
                }
            }
            push(String::new(), Plain, None);
            push("  from claude's own /usage: no model call".into(), Dim, None);
        }
        None => push("  no reading: is claude installed and logged in to a subscription?".into(), Dim, None),
    }
    let credits = v["credits"].as_array().cloned().unwrap_or_default();
    if !credits.is_empty() {
        push(String::new(), Plain, None);
        push("Providers' credits".into(), Head, None);
        push(String::new(), Rule, None);
        for c in &credits {
            let name = c["provider"].as_str().unwrap_or_default();
            match (c["left"].as_str(), c["page"].as_str()) {
                (Some(left), _) => push(format!("  {name:<20}{left}"), if left.contains(" over ") { Bad } else { Plain }, None),
                (None, Some(page)) => {
                    let host = page.trim_start_matches("https://").trim_start_matches("http://");
                    push(format!("  {name:<20}{host}"), Plain, Some(page.to_string()));
                }
                (None, None) => push(format!("  {name:<20}no balance to read"), Dim, None),
            }
        }
        push("  without a balance API: their page, a click away".into(), Dim, None);
    }
    let accounts = v["omniroute"].as_array().cloned().unwrap_or_default();
    if !accounts.is_empty() {
        push(String::new(), Plain, None);
        push("OmniRoute accounts".into(), Head, None);
        push(String::new(), Rule, None);
        for (i, a) in accounts.iter().enumerate() {
            if i > 0 {
                push(String::new(), Plain, None);
            }
            let plan = a["plan"].as_str().map_or(String::new(), |p| format!(" · {p}"));
            let status = if a["status"] == "valid" { String::new() } else { format!(" · login {}", a["status"].as_str().unwrap_or("?")) };
            let provider = a["provider"].as_str().unwrap_or("?");
            let provider = if provider.starts_with("openai-compatible") { "OpenAI-compatible" } else { provider };
            push(format!("  {provider} · {}{plan}{status}", a["name"].as_str().unwrap_or_default()), Plain, None);
            for w in a["windows"].as_array().into_iter().flatten() {
                let what = [w["group"].as_str().unwrap_or_default(), w["window"].as_str().unwrap_or_default()].iter()
                    .filter(|x| !x.is_empty()).copied().collect::<Vec<_>>().join(" · ");
                if w["unlimited"] == true {
                    push(format!("    {what:<30} unlimited"), Plain, None);
                    continue;
                }
                let used = w["used"].as_f64().unwrap_or(0.0);
                let resets = w["resets"].as_str().and_then(|r| until(r, now)).unwrap_or_default();
                push(meter(4, &what, used, &resets), if used >= 90.0 { Bad } else { Plain }, None);
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
        let shown = lines(&json!({"plan": [{"label": "Current session", "used": 14.0, "resets": "5:29pm (Europe/Berlin)"}, {"label": "Current week (Fable)", "used": 95.0, "resets": "Nov 21"}],
            "credits": [{"provider": "Mortera", "left": null, "page": "https://mortera.cloud"}, {"provider": "OpenRouter", "left": "$0.17 over its credit", "page": "https://openrouter.ai/keys"}],
            "omniroute": [{"provider": "agy", "name": "a", "plan": "Ultra", "status": "valid", "windows": [{"group": "Gemini Models", "window": "weekly", "used": 1.4, "resets": "2099-11-25T11:03:10.000Z"}]}]}));
        let row = |t: &str| shown.iter().find(|l| l.0.contains(t)).unwrap_or_else(|| panic!("no row with {t}: {shown:?}"));
        use crate::ui::Tone::{Bad, Head, Plain, Rule};
        assert_eq!((row("Claude plan").1, shown[1].1), (Head, Rule), "a section: its heading, a line across");
        assert!(row("5-hour session").0.contains("14%") && row("5-hour session").0.ends_with("resets 5:29pm"), "the zone dropped: {:?}", row("5-hour session"));
        assert_eq!(row("Week · Fable").1, Bad, "a window at 90% or more is red");
        assert_eq!((row("Mortera").2.as_deref(), &row("Mortera").0[LINK_FROM..]), (Some("https://mortera.cloud"), "mortera.cloud"), "its page, a click away");
        assert_eq!(row("OpenRouter").1, Bad, "below zero");
        let g = row("Gemini Models · weekly");
        assert!(g.1 == Plain && g.0.contains(" in ") && g.0.chars().position(|c| c == '░') == row("5-hour session").0.chars().position(|c| c == '█'), "bars line up across sections: {g:?}");
        assert_eq!(until("2026-11-25T11:03:10.000Z", iso_secs("2026-11-18T10:00:00Z").unwrap()).as_deref(), Some("in 7 d 1 h"));
        assert_eq!((plan_name("default_claude_max_5x"), plan_name("Ultra")), ("Claude Max 5x".into(), "Ultra".into()));
        assert_eq!(window_name("Current week (all models)"), "Week · all models");
    }
}
