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
    // Asked over the CLI's stream and taken as soon as its line is there: about 1.2 s. As `-p /usage --output-format json`
    // the same page came only once the CLI had closed down, 4.9 s (both measured 2026-11-20; the user: a refresh "takes
    // like a few seconds"). The question is the only input (stdin is closed after it), so a CLI that cannot answer ends
    // as before; its closing down is waited for off this thread.
    use std::io::{BufRead, Write};
    use std::process::Stdio;
    let mut child = std::process::Command::new("claude")
        .args(["-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose", "--no-session-persistence", "--setting-sources", "", "--strict-mcp-config"])
        .current_dir(crate::platform::home()?).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    let asked = child.stdin.take().and_then(|mut i| writeln!(i, "{}", json!({"type": "user", "message": {"role": "user", "content": "/usage"}})).ok());
    let page = asked.and(child.stdout.take()).and_then(|o| {
        std::io::BufReader::new(o).lines().map_while(Result::ok).filter_map(|l| serde_json::from_str::<Value>(&l).ok())
            .find(|e| e["type"] == "result").and_then(|e| e["result"].as_str().map(String::from))
    });
    std::thread::spawn(move || { let _ = child.wait(); });
    parse_plan(&page?)
}

/// Whether the official CLI is signed in, as `claude auth status` says ("loggedIn" in its JSON, ~0.15 s; it reads no
/// model and tazuna never reads its credentials): None when claude is not there or said nothing readable within 5 s. From
/// the home folder, as `claude_plan`. Kept for the process (`claude_logged_in`): an installed, logged-out claude was ticked
/// "Claude on your subscription" and its models offered as on the plan (review 12).
#[inline(never)]
pub fn refresh_claude_login() -> Option<bool> {
    let read = || -> Option<bool> {
        use std::io::Read;
        let mut child = std::process::Command::new("claude").args(["auth", "status"]).current_dir(crate::platform::home()?)
            .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).spawn().ok()?;
        let mut out = child.stdout.take()?;
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut s = String::new();
            let _ = out.read_to_string(&mut s);
            let _ = tx.send(s);
        });
        let text = rx.recv_timeout(Duration::from_secs(5)).ok();
        if text.is_none() {
            let _ = child.kill();
        }
        std::thread::spawn(move || { let _ = child.wait(); });
        // logged out it exits 1, with the same JSON
        serde_json::from_str::<Value>(text?.trim()).ok()?["loggedIn"].as_bool()
    };
    // tests never start the user's claude
    let now = if cfg!(test) { None } else { read() };
    *CLAUDE_LOGIN.lock().unwrap_or_else(|e| e.into_inner()) = Some(now);
    now
}

static CLAUDE_LOGIN: std::sync::Mutex<Option<Option<bool>>> = std::sync::Mutex::new(None);

/// `refresh_claude_login` as last read in this process; None before the first read. Never read here: a claude that hangs
/// (a keychain prompt) held the window's server and the terminal's screen up to 5 s each time /model opened (review 12).
#[inline(never)]
pub fn claude_logged_in() -> Option<bool> {
    CLAUDE_LOGIN.lock().unwrap_or_else(|e| e.into_inner()).flatten()
}

/// Reads it again off the caller's thread; `changed` runs when it differs from `shown` (what the caller showed with
/// `claude_logged_in`), to show it again.
#[inline(never)]
pub fn recheck_claude_login(shown: Option<bool>, changed: impl FnOnce() + Send + 'static) {
    std::thread::spawn(move || {
        if refresh_claude_login() != shown {
            changed();
        }
    });
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
    // each account's own page at once (each asks its provider: four in a row took over a second)
    let one = |p: &Value| {
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
    };
    let accounts: Vec<&Value> = list["providers"].as_array().into_iter().flatten().collect();
    std::thread::scope(|s| accounts.iter().map(|p| s.spawn(|| one(p))).collect::<Vec<_>>().into_iter().filter_map(|h| h.join().ok()).collect())
}

/// All of it, for the desktop's usage page (the app server's `accounts` op).
pub fn all() -> Value {
    all_then(|_| {})
}

/// The same, with the plan handed to `early` as soon as it is read (about a second): the credits and OmniRoute's accounts
/// ask other servers and take as long again, and the plan is what a refresh is looked at for. The three are read at once,
/// so the whole takes as long as the slowest, not their sum.
pub fn all_then(early: impl FnOnce(&Value)) -> Value {
    std::thread::scope(|s| {
        // claude's login read again too (the pickers' Claude rows follow it)
        let (c, o, _) = (s.spawn(credits), s.spawn(omniroute), s.spawn(refresh_claude_login));
        let plan = claude_plan();
        let mut v = json!({
            "plan": plan.as_ref().map(|(rows, _)| rows.iter().map(|r| json!({"label": r.label, "used": r.used, "resets": r.resets})).collect::<Vec<_>>()),
            "plan_notes": plan.as_ref().map(|(_, n)| n.clone()).unwrap_or_default(),
        });
        early(&v);
        v["credits"] = json!(c.join().unwrap_or_default());
        v["omniroute"] = json!(o.join().unwrap_or_default());
        v
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

/// This machine's offset from UTC in seconds at `now`: the C library's local time on unix; elsewhere none, and /usage
/// keeps claude's own words for its resets.
fn local_offset(now: i64) -> Option<i64> {
    #[cfg(unix)]
    {
        // SAFETY: localtime_r writes only into `tm`, which lives here
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        let t = now as libc::time_t;
        if unsafe { libc::localtime_r(&t, &mut tm) }.is_null() {
            return None;
        }
        Some(tm.tm_gmtoff as i64)
    }
    #[cfg(not(unix))]
    {
        let _ = now;
        None
    }
}

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// (year, month 1-12, day) of a day count since 1970-01-01 (Howard Hinnant's civil_from_days).
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let (d, m) = (doy - (153 * mp + 2) / 5 + 1, if mp < 10 { mp + 3 } else { mp - 9 });
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// When a window resets, as the desktop app says it (lib/health.ts whenReset): "in 2 h 14 min" within a day, "Sat 3am"
/// within the week, "Nov 26" later; `at` and `now` in seconds since 1970, `off` the local offset (design review 7).
fn when(at: i64, now: i64, off: i64) -> String {
    let mins = (at - now + 30) / 60;
    if mins <= 0 {
        return "now".into();
    }
    if mins < 60 {
        return format!("in {mins} min");
    }
    if mins < 24 * 60 {
        return if mins % 60 == 0 { format!("in {} h", mins / 60) } else { format!("in {} h {} min", mins / 60, mins % 60) };
    }
    let local = at + off;
    let days = local.div_euclid(86_400);
    let (_, m, d) = civil(days);
    if mins < 7 * 24 * 60 {
        let (h, mi) = (local.rem_euclid(86_400) / 3600, local.rem_euclid(3600) / 60);
        let weekday = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"][days.rem_euclid(7) as usize];
        let time = format!("{}{}{}", if h % 12 == 0 { 12 } else { h % 12 }, if mi > 0 { format!(":{mi:02}") } else { String::new() }, if h < 12 { "am" } else { "pm" });
        return format!("{weekday} {time}");
    }
    format!("{} {d}", MONTHS[(m - 1) as usize])
}

/// claude's /usage reset in your own time ("Nov 21 at 2:59am", "5:29pm") as seconds since 1970, given the local offset.
fn claude_reset(text: &str, now: i64, off: i64) -> Option<i64> {
    let t = text.trim();
    let (date, time) = match t.split_once(" at ") { Some((d, tm)) => (Some(d), tm), None => (None, t) };
    let (clock, pm) = if let Some(c) = time.strip_suffix("pm") { (c, true) } else { (time.strip_suffix("am")?, false) };
    let (h, mi) = match clock.split_once(':') { Some((h, m)) => (h.parse::<i64>().ok()?, m.parse::<i64>().ok()?), None => (clock.parse::<i64>().ok()?, 0) };
    let h = h % 12 + if pm { 12 } else { 0 };
    let today = (now + off).div_euclid(86_400);
    let day = match date {
        Some(d) => {
            let (mon, dd) = d.split_once(' ')?;
            let m = MONTHS.iter().position(|x| x.eq_ignore_ascii_case(&mon[..3.min(mon.len())]))? as i64 + 1;
            let (y, ..) = civil(today);
            let at = |y: i64| iso_secs(&format!("{y:04}-{m:02}-{:02}T00:00:00Z", dd.parse::<i64>().ok()?)).map(|s| s / 86_400);
            let first = at(y)?;
            if first < today - 1 { at(y + 1)? } else { first }
        }
        None => if h * 3600 + mi * 60 < (now + off).rem_euclid(86_400) { today + 1 } else { today },
    };
    Some(day * 86_400 + h * 3600 + mi * 60 - off)
}

/// claude's window names, shorter: "Current session" is the 5-hour one, "Current week (Fable)" the week on Fable.
fn window_name(label: &str) -> String {
    let l = label.trim_start_matches("Current ");
    match l.split_once(" (") {
        _ if l == "session" => "5-hour session".into(),
        Some((w, rest)) => format!("{}{}, {}", w[..1].to_uppercase(), &w[1..], rest.trim_end_matches(')')),
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
    let off = local_offset(now);
    // a reset as the desktop says it where the time zone is known, else as claude or OmniRoute gave it
    let reset_claude = |text: &str| off.and_then(|o| claude_reset(text, now, o).map(|at| when(at, now, o))).unwrap_or_else(|| text.to_string());
    let reset_iso = |iso: &str| match off { Some(o) => iso_secs(iso).map(|at| when(at, now, o)), None => until(iso, now) };
    // a meter's row: its name from `indent`, its bar at one column for all (column 35: "Claude & GPT Models, weekly" fits)
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
                push(meter(2, &window_name(r["label"].as_str().unwrap_or_default()), used, &reset_claude(resets)), if used >= 90.0 { Bad } else { Plain }, None);
            }
            // what claude says contributes, for the last day (its last 7 days repeat the same advice)
            let notes: Vec<&str> = v["plan_notes"].as_array().into_iter().flatten().filter_map(Value::as_str)
                .take_while(|n| !n.trim_start().starts_with("Last 7d")).filter(|n| !n.starts_with("Approximate")).collect();
            if !notes.is_empty() {
                push(String::new(), Plain, None);
                push("  What's contributing (approximate, this machine only)".into(), Dim, None);
                // claude's own lines, its middle dots as commas ("Last 24h, 1179 requests, 7 sessions")
                for n in notes {
                    push(format!("    {}", n.trim().replace(" · ", ", ")), Dim, None);
                }
            }
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
        push("  no balance from their APIs: their page is a click away".into(), Dim, None);
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
            let plan = a["plan"].as_str().map_or(String::new(), |p| format!(", {p}"));
            let status = if a["status"] == "valid" { String::new() } else { format!(", login {}", a["status"].as_str().unwrap_or("?")) };
            let provider = a["provider"].as_str().unwrap_or("?");
            let provider = if provider.starts_with("openai-compatible") { "OpenAI-compatible" } else { provider };
            push(format!("  {provider}, {}{plan}{status}", a["name"].as_str().unwrap_or_default()), Plain, None);
            for w in a["windows"].as_array().into_iter().flatten() {
                let what = [w["group"].as_str().unwrap_or_default(), w["window"].as_str().unwrap_or_default()].iter()
                    .filter(|x| !x.is_empty()).copied().collect::<Vec<_>>().join(", ");
                if w["unlimited"] == true {
                    push(format!("    {what:<30} unlimited"), Plain, None);
                    continue;
                }
                let used = w["used"].as_f64().unwrap_or(0.0);
                let resets = w["resets"].as_str().and_then(reset_iso).unwrap_or_default();
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
        assert!(row("5-hour session").0.contains("14%") && row("5-hour session").0.contains("resets ") && !row("5-hour session").0.contains("Europe"), "the zone dropped: {:?}", row("5-hour session"));
        assert_eq!(row("Week, Fable").1, Bad, "a window at 90% or more is red");
        assert_eq!((row("Mortera").2.as_deref(), &row("Mortera").0[LINK_FROM..]), (Some("https://mortera.cloud"), "mortera.cloud"), "its page, a click away");
        assert_eq!(row("OpenRouter").1, Bad, "below zero");
        let g = row("Gemini Models, weekly");
        assert!(g.1 == Plain && g.0.contains("resets ") && g.0.chars().position(|c| c == '░') == row("5-hour session").0.chars().position(|c| c == '█'), "bars line up across sections: {g:?}");
        assert_eq!(until("2026-11-25T11:03:10.000Z", iso_secs("2026-11-18T10:00:00Z").unwrap()).as_deref(), Some("in 7 d 1 h"));
        // the desktop's words for a reset, at UTC+1 on Thursday 2026-11-19 15:00 local
        let (off, now) = (3600, iso_secs("2026-11-19T14:00:00Z").unwrap());
        assert_eq!([when(now + 42 * 60, now, off), when(now + 134 * 60, now, off), when(iso_secs("2026-11-21T02:00:00Z").unwrap(), now, off), when(iso_secs("2026-11-26T16:13:00Z").unwrap(), now, off)],
            ["in 42 min", "in 2 h 14 min", "Sat 3am", "Nov 26"].map(String::from));
        assert_eq!(claude_reset("Nov 21 at 2:59am", now, off), iso_secs("2026-11-21T01:59:00Z"));
        assert_eq!(claude_reset("5:29pm", now, off), iso_secs("2026-11-19T16:29:00Z"), "a time alone is today's, or tomorrow's once past");
        assert_eq!((plan_name("default_claude_max_5x"), plan_name("Ultra")), ("Claude Max 5x".into(), "Ultra".into()));
        assert_eq!(window_name("Current week (all models)"), "Week, all models");
    }
}
