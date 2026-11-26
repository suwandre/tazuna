//! `/usage`: what each account has left, in one place (2026-11-18). Claude's plan from the official CLI's own `/usage`
//! page (`claude -p /usage`: no model call, $0; until now tazuna made a Haiku call to read the plan from its rate-limit
//! event), the providers' credits or plan windows where their API tells (`reading`; the others' pages are linked), and
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
    // logged out it exits 1, with the same JSON
    let read = || cli_json("claude", &["auth", "status"], 5)?["loggedIn"].as_bool();
    // tests never start the user's claude
    let now = if cfg!(test) { None } else { read() };
    *CLAUDE_LOGIN.lock().unwrap_or_else(|e| e.into_inner()) = Some(now);
    now
}

/// What a CLI prints as JSON, run from the home folder with no shell and no input; None when it is not there, prints nothing
/// readable, or takes over `secs`. Then it is stopped with all it started: on unix it runs in a process group of its own,
/// and the group is killed (an npm launcher that starts node and does not exec into it left node behind).
fn cli_json(bin: &str, args: &[&str], secs: u64) -> Option<Value> {
    use std::io::Read;
    let mut cmd = std::process::Command::new(bin);
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
    let mut child = cmd.args(args).current_dir(crate::platform::home()?)
        .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).spawn().ok()?;
    let mut out = child.stdout.take()?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut s = String::new();
        let _ = out.read_to_string(&mut s);
        let _ = tx.send(s);
    });
    let text = rx.recv_timeout(Duration::from_secs(secs)).ok();
    if text.is_none() {
        // SAFETY: kill only sends a signal; the group is the child's own (process_group(0) above), not tazuna's
        #[cfg(unix)]
        unsafe {
            libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL);
        }
        let _ = child.kill();
    }
    std::thread::spawn(move || { let _ = child.wait(); });
    serde_json::from_str(text?.trim()).ok()
}

static CLAUDE_LOGIN: std::sync::Mutex<Option<Option<bool>>> = std::sync::Mutex::new(None);

/// Whether the official CLI is there to start: `claude` on PATH. Without it a new user saw Claude ready in /model, and every
/// message failed with "No such file or directory" (design review 13). Tests count it there (their stand-in claude is on
/// PATH only once a test has made it).
#[inline(never)]
pub fn claude_installed() -> bool {
    #[cfg(test)]
    return !NO_CLAUDE.get();
    #[cfg(not(test))]
    crate::platform::on_path("claude", &std::env::var_os("PATH").unwrap_or_default())
}

#[cfg(test)]
thread_local! {
    /// A test's machine without claude (`claude_installed`), on its own thread only.
    pub static NO_CLAUDE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

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

/// GET `url` with `auth` as the whole Authorization header: the status and the JSON body, whatever the status (a 403's
/// body says why).
fn fetch(agent: &ureq::Agent, url: &str, auth: &str) -> Option<(u16, Value)> {
    let mut r = agent.get(url).header("Authorization", auth).config().http_status_as_error(false).build().call().ok()?;
    Some((r.status().as_u16(), serde_json::from_str(&r.body_mut().read_to_string().ok()?).ok()?))
}

fn get(agent: &ureq::Agent, url: &str, key: &str) -> Option<Value> {
    fetch(agent, url, &format!("Bearer {}", key.trim())).filter(|(s, _)| (200..300).contains(s)).map(|(_, v)| v)
}

/// Each provider with a key (only `only`, where named: after a run, the provider it ran on): its row (`row`), from the
/// reading every tazuna process on this machine shares (`cached`: asked once a minute at most). Each reader on a thread of
/// its own; those not done within 12 s (a reader makes up to three requests of 10 s each, the Qwen CLI two starts) give
/// their last good row, and finish into the shared reading for the next look. A reading that fails is no reading, never
/// an error.
#[inline(never)]
pub fn credits(only: Option<&str>) -> Vec<Value> {
    let keyed: Vec<(&'static crate::models::Login, String)> = crate::models::LOGINS.iter()
        .filter(|l| l.name != "OmniRoute" && only.is_none_or(|o| o == l.name))
        .filter_map(|l| Some((l, l.vars.iter().find_map(|v| crate::models::key(v))?))).collect();
    let dir = crate::labels::dir().ok().map(|d| d.join("credits"));
    let (tx, rx) = std::sync::mpsc::channel();
    for (i, (l, key)) in keyed.iter().cloned().enumerate() {
        let (tx, dir) = (tx.clone(), dir.clone());
        std::thread::spawn(move || {
            let _ = tx.send((i, cached(dir.as_deref(), l, &key, crate::session::unix_now(), || reading(l, &key))));
        });
    }
    drop(tx);
    let rows = gather(&rx, keyed.len(), Duration::from_secs(12));
    keyed.iter().zip(rows).map(|((l, key), r)| r.unwrap_or_else(|| kept_row(&kept(dir.as_deref(), l, key), crate::session::unix_now()).unwrap_or_else(|| row(l, None)))).collect()
}

/// The rows of `n` readers as they arrive through `rx`, until all are in or `within` has passed: one that is late is None.
fn gather(rx: &std::sync::mpsc::Receiver<(usize, Value)>, n: usize, within: Duration) -> Vec<Option<Value>> {
    let deadline = std::time::Instant::now() + within;
    let mut rows = vec![None; n];
    while let Some(wait) = deadline.checked_duration_since(std::time::Instant::now()) && let Ok((i, row)) = rx.recv_timeout(wait) {
        rows[i] = Some(row);
    }
    rows
}

/// The shared reading's file for `l` (under ~/.local/share/tazuna/credits/), and what it holds when it was read with this
/// key (`kh`, a hash of it: a new key is read again at once); else null.
fn kept(dir: Option<&std::path::Path>, l: &crate::models::Login, key: &str) -> Value {
    let Some(f) = dir.map(|d| d.join(format!("{}.json", l.name.replace(|c: char| !c.is_ascii_alphanumeric(), "-")))) else { return Value::Null };
    let v: Value = std::fs::read_to_string(&f).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    if v["kh"] == key_hash(key) { v } else { Value::Null }
}

fn key_hash(key: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    key.trim().hash(&mut h);
    format!("{:016x}", h.finish())
}

/// The last good row of a shared reading, up to a day old.
fn kept_row(k: &Value, now: u64) -> Option<Value> {
    (k["row"]["left"].is_string() && k["good_at"].as_u64().is_some_and(|g| now.saturating_sub(g) < 86_400)).then(|| k["row"].clone())
}

/// `l`'s row from the reading shared by every tazuna process here: four conversations and the desktop asked each provider
/// at each start and after every run, about nine times a minute (Ollama allows ten; review of the balances, 2026-11-26).
/// Read again (`read`) once it is a minute old; marked as being read first, so another process meanwhile takes the kept
/// row; a read that fails keeps the last good row. Without `dir`, read each time.
fn cached(dir: Option<&std::path::Path>, l: &crate::models::Login, key: &str, now: u64, read: impl FnOnce() -> Option<Reading>) -> Value {
    let k = kept(dir, l, key);
    if k["at"].as_u64().is_some_and(|at| now.saturating_sub(at) < 60) && k["row"].is_object() {
        return k["row"].clone();
    }
    let file = dir.map(|d| d.join(format!("{}.json", l.name.replace(|c: char| !c.is_ascii_alphanumeric(), "-"))));
    // written whole and moved into place: another process never reads half a file
    let save = |v: &Value| if let Some(f) = &file {
        let tmp = f.with_extension(format!("{}.tmp", std::process::id()));
        let _ = std::fs::create_dir_all(f.parent().unwrap_or(f)).and_then(|_| std::fs::write(&tmp, v.to_string())).and_then(|_| std::fs::rename(&tmp, f));
    };
    let kh = key_hash(key);
    save(&json!({"kh": kh, "at": now, "good_at": k["good_at"], "row": k["row"]}));
    match read() {
        Some(r) => {
            let v = row(l, Some(r));
            save(&json!({"kh": kh, "at": now, "good_at": now, "row": v}));
            v
        }
        None => {
            let v = kept_row(&k, now).unwrap_or_else(|| row(l, None));
            save(&json!({"kh": kh, "at": now, "good_at": k["good_at"], "row": v}));
            v
        }
    }
}

/// A provider's reading: the line the usage pages show, a short one for narrow places (the cockpit's 38-column cost box,
/// the window's status bar), a plan's windows (OmniRoute's shape: % used, resets), the money left (amount, currency:
/// "USD", "CNY", "IDR"), a state the reader knows better than the numbers ("needs_scope", free requests), and whether the
/// money is a key's own limit rather than the account's credit.
#[derive(Default)]
struct Reading {
    left: String,
    short: String,
    windows: Vec<Value>,
    money: Option<(f64, &'static str)>,
    state: Option<&'static str>,
    key_limit: bool,
    /// today's free requests, not credit: they come back the next day
    free_requests: bool,
}

/// Credit under this many dollars is low (the desktop's threshold, health.ts); yuan, with no rate in the code, about as
/// much. ponytail: fixed thresholds; one from the user's own daily spend when that matters
const LOW_USD: f64 = 2.0;
const LOW_CNY: f64 = 14.0;

/// A provider's row: `provider`, `left` (the line to show; null: no reading, and its `page` to look), `short`, `windows`
/// where a plan has them, and the numbers the windows decide by rather than the words: `usd_left` (dollars, converted
/// where the code has a rate: rupiah at engine.rs's), `max_used_pct` (the fullest window), and `state`: "ok", "low" (under
/// $2, or a window at 75%), "out" (none left, or a window at 90%: the meters' red, the desktop's byPct), "over" (spent past
/// it), "needs_scope" (the key may not read it), "unknown"; `key_limit` when the money is a key's own limit,
/// `free_requests` when the reading is today's free requests.
fn row(l: &crate::models::Login, r: Option<Reading>) -> Value {
    let Some(r) = r else {
        return json!({"provider": l.name, "left": null, "short": null, "page": l.page, "usd_left": null, "max_used_pct": null, "state": "unknown"});
    };
    let usd = r.money.and_then(|(x, cur)| match cur { "USD" => Some(x), "IDR" => Some(x / crate::engine::IDR_PER_USD), _ => None });
    let max = r.windows.iter().filter_map(|w| w["used"].as_f64()).max_by(f64::total_cmp);
    let state = r.state.unwrap_or(match (r.money, max) {
        (Some((x, _)), _) if x < 0.0 => "over",
        (Some((x, _)), _) => match usd.unwrap_or(x) {
            v if v < 0.01 => "out",
            v if v < if usd.is_some() { LOW_USD } else { LOW_CNY } => "low",
            _ => "ok",
        },
        (None, Some(u)) if u >= 90.0 => "out",
        (None, Some(u)) if u >= 75.0 => "low",
        (None, Some(_)) => "ok",
        (None, None) => "unknown",
    });
    let mut v = json!({"provider": l.name, "left": r.left, "short": r.short, "page": l.page, "usd_left": usd, "max_used_pct": max, "state": state});
    if !r.windows.is_empty() {
        v["windows"] = json!(r.windows);
    }
    if r.key_limit {
        v["key_limit"] = json!(true);
    }
    if r.free_requests {
        v["free_requests"] = json!(true);
    }
    v
}

/// A provider's reading from its official API (read-only GETs with its key). None: its API tells nothing, or did not
/// answer.
fn reading(l: &crate::models::Login, key: &str) -> Option<Reading> {
    let a = http(10);
    let get = |url: &str| get(&a, url, key);
    match l.name {
        "SubRouter" => crate::models::subrouter_balance().map(|b| money("USD", b)),
        "TokenRouter" => crate::models::newapi_balance(crate::models::tokenrouter::URL.trim_end_matches("/v1"), key).map(|b| money("USD", b)),
        // both: a key's own limit can run out before the account's credit, and the account's before the key's limit
        "OpenRouter" => openrouter(get("https://openrouter.ai/api/v1/key").as_ref(), get("https://openrouter.ai/api/v1/credits").as_ref()),
        "Mortera" => mortera(&get("https://mortera.cloud/v1/credits")?),
        "Cheaper Inference" => cheaper(fetch(&a, "https://api.cheaperinference.com/v1/account/balance", &format!("Bearer {}", key.trim()))?),
        "DeepSeek" => deepseek(&get("https://api.deepseek.com/user/balance")?),
        "Moonshot" => Some(money("USD", num(&get(&format!("{}/users/me/balance", l.url))?["data"]["available_balance"])?)),
        "SiliconFlow" | "SiliconFlow China" => Some(money(if l.name.ends_with("China") { "CNY" } else { "USD" }, num(&get(&format!("{}/user/info", l.url))?["data"]["totalBalance"])?)),
        "Ollama Cloud" => ollama(&get("https://ollama.com/api/balance")?),
        "Synthetic" => synthetic(&get("https://api.synthetic.new/v2/quotas")?),
        "Venice" => venice(&get("https://api.venice.ai/api/v1/api_keys/rate_limits")?),
        // the URL in MiniMax's Token Plan FAQ (platform.minimax.io/docs/token-plan/faq, checked 2026-11-26)
        "MiniMax Plan" => minimax(&get("https://www.minimax.io/v1/token_plan/remains")?),
        // the plan key as the whole header, as Z.ai's own tools send it (a Bearer works too)
        "Z.ai Coding Plan" | "Zhipu Coding Plan" => {
            let host = l.url.split("/api/").next()?;
            zai(&fetch(&a, &format!("{host}/api/monitor/usage/quota/limit"), key.trim()).filter(|(s, _)| (200..300).contains(s))?.1)
        }
        "Kimi Code" | "Kimi Code China" => kimi(&get(&format!("{}/usages", l.url))?),
        "Qwen Cloud Token Plan" => qwencloud_usage().and_then(|(account, usage)| qwencloud(&usage, account.as_deref())),
        _ => None,
    }
}

/// A number given as a number or as a string ("110.00"); not NaN or infinite.
fn num(v: &Value) -> Option<f64> {
    v.as_f64().or_else(|| v.as_str()?.trim().parse().ok()).filter(|x: &f64| x.is_finite())
}

/// Whole units with their thousands grouped: 48000 is "48,000".
fn grouped(x: f64) -> String {
    let d = format!("{:.0}", x.abs());
    d.chars().enumerate().fold(String::new(), |mut out, (i, c)| {
        if i > 0 && (d.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
        out
    })
}

/// An amount without its sign, in its currency's way: "$1,234.50", "¥30.00", "Rp 48,000".
fn amount(cur: &str, x: f64) -> String {
    if cur == "IDR" {
        return format!("Rp {}", grouped(x));
    }
    let cents = (x.abs() * 100.0).round() as i64;
    format!("{}{}.{:02}", if cur == "CNY" { "¥" } else { "$" }, grouped((cents / 100) as f64), cents % 100)
}

/// Money left: "$4.20 left"; spent past it, "$0.17 over its credit" (it read "-$0.17 left of $0.00", design review 4), and
/// short "$0.17 below zero" (the desktop's words: "over" read as over a limit, design review 12).
fn money(cur: &'static str, x: f64) -> Reading {
    let a = amount(cur, x);
    let (left, short) = if x < 0.0 { (format!("{a} over its credit"), format!("{a} below zero")) } else { (format!("{a} left"), format!("{a} left")) };
    Reading { left, short, money: Some((x, cur)), ..Default::default() }
}

/// OpenRouter's /key and the account's /credits: whichever runs out first (a key past its own limit fails with 402 while
/// the account has credit); on the free tier today's free requests, unless the account is past zero (the user's: free
/// tier, $0.17 past it, 2026-11-26); else what the key spent this month.
fn openrouter(key: Option<&Value>, credits: Option<&Value>) -> Option<Reading> {
    let k = key.map(|k| &k["data"]);
    let account = credits.and_then(|c| Some((num(&c["data"]["total_credits"])?, num(&c["data"]["total_usage"])?))).map(|(t, u)| (t - u, t));
    if let Some(k) = k && let Some(r) = num(&k["limit_remaining"]) && account.is_none_or(|(a, _)| r < a) {
        let a = amount("USD", r);
        let (left, short) = if r < 0.0 { (format!("{a} over its key limit"), "key limit used up".to_string()) }
            else { (format!("{a} left of its {} key limit", amount("USD", num(&k["limit"]).unwrap_or(r))), format!("{a} left (key)")) };
        return Some(Reading { left, short, money: Some((r, "USD")), key_limit: true, ..Default::default() });
    }
    if let Some((a, _)) = account.filter(|(a, _)| *a < 0.0) {
        return Some(money("USD", a));
    }
    if let Some(k) = k.filter(|k| k["is_free_tier"] == true) && let (Some(r), Some(n)) = (k["free_model_daily_requests"]["remaining"].as_u64(), k["free_model_daily_requests"]["limit"].as_u64()) {
        return Some(Reading { left: format!("{r} of {n} free requests left today"), short: format!("{r}/{n} free today"), state: Some(if r == 0 { "out" } else { "ok" }), free_requests: true, ..Default::default() });
    }
    if let Some((a, total)) = account {
        return Some(Reading { left: format!("{} left of {}", amount("USD", a), amount("USD", total)), ..money("USD", a) });
    }
    let u = amount("USD", num(&k?["usage_monthly"])?);
    Some(Reading { left: format!("{u} used this month"), short: format!("{u} this month"), ..Default::default() })
}

/// Mortera's /v1/credits, in rupiah (`available_idr`; `held_idr` set aside for calls under way): "Rp 48,000 left", and
/// about how many dollars at the rate its prices are read at (engine.rs IDR_PER_USD, a fixed rate).
fn mortera(v: &Value) -> Option<Reading> {
    let d = if v["data"].is_object() { &v["data"] } else { v };
    let avail = num(&d["available_idr"]).or_else(|| num(&d["balance_idr"]))?;
    let held = num(&d["held_idr"]).filter(|h| *h > 0.0).map_or(String::new(), |h| format!(", {} held", amount("IDR", h)));
    let r = money("IDR", avail);
    Some(Reading { left: format!("{}{held} (about {})", r.left, amount("USD", avail / crate::engine::IDR_PER_USD)), ..r })
}

/// Cheaper Inference's /v1/account/balance (`available_usd`, `reserved_usd`), which needs the key's account:read scope: a
/// key without it gets a 403 `insufficient_scope`, said as what to do.
fn cheaper((status, v): (u16, Value)) -> Option<Reading> {
    if status == 403 && v.to_string().contains("insufficient_scope") {
        return Some(Reading { left: "turn on account:read for this key".into(), short: "needs account:read".into(), state: Some("needs_scope"), ..Default::default() });
    }
    let d = if v["data"].is_object() { &v["data"] } else { &v };
    let avail = num(&d["available_usd"]).or_else(|| num(&d["balance_usd"])).filter(|_| (200..300).contains(&status))?;
    let reserved = num(&d["reserved_usd"]).filter(|r| *r > 0.0).map_or(String::new(), |r| format!(", {} reserved", amount("USD", r)));
    let r = money("USD", avail);
    Some(Reading { left: format!("{}{reserved}", r.left), ..r })
}

/// DeepSeek's /user/balance: each currency's `total_balance` (a string), "$4.20 left" or "¥30.00 left"; the first decides.
fn deepseek(v: &Value) -> Option<Reading> {
    let each: Vec<Reading> = v["balance_infos"].as_array()?.iter().filter_map(|b| {
        let cur = match b["currency"].as_str()? { "USD" => "USD", "CNY" => "CNY", _ => return None };
        Some(money(cur, num(&b["total_balance"])?))
    }).collect();
    let left = each.iter().map(|r| r.left.as_str()).collect::<Vec<_>>().join(", ");
    Some(Reading { left, ..each.into_iter().next()? })
}

/// Ollama Cloud's /api/balance: the plan's included credit (`balance_usd` of `allowance_usd`) and the purchased credit;
/// on older plans the session and weekly windows (`remaining_percent`, `resets_at`). Neither: no reading.
fn ollama(v: &Value) -> Option<Reading> {
    let (inc, bought) = (&v["included"], num(&v["purchased"]["balance_usd"]));
    if let Some(b) = num(&inc["balance_usd"]) {
        let of = num(&inc["allowance_usd"]).map_or(String::new(), |a| format!(" of {}", amount("USD", a)));
        let p = bought.unwrap_or(0.0);
        let r = money("USD", b + p);
        return Some(Reading { left: format!("{} ({}{of} included, {} purchased)", r.left, amount("USD", b), amount("USD", p)), ..r });
    }
    let windows: Vec<Value> = ["session", "weekly"].iter()
        .filter_map(|k| Some(window(k, 100.0 - num(&inc[*k]["remaining_percent"])?, inc[*k]["resets_at"].as_str().map(String::from)))).collect();
    let r = plan(windows)?;
    let p = bought.filter(|p| *p > 0.0).map_or(String::new(), |p| format!(", {} purchased", amount("USD", p)));
    Some(Reading { left: format!("{}{p}", r.left), ..r })
}

/// Synthetic's /v2/quotas: the subscription's `requests` of its `limit` until it renews (`renewsAt`).
fn synthetic(v: &Value) -> Option<Reading> {
    let s = &v["subscription"];
    let (limit, used) = (num(&s["limit"]).filter(|l| *l > 0.0)?, num(&s["requests"])?);
    let r = plan(vec![window("requests", used / limit * 100.0, s["renewsAt"].as_str().map(String::from))])?;
    let n = (limit - used).max(0.0);
    Some(Reading { left: format!("{n:.0} of {limit:.0} requests left"), short: format!("{n:.0}/{limit:.0} left"), ..r })
}

/// Venice's /api_keys/rate_limits balances: USD and bundled credit (in USD), the dollars it can spend, said first as one
/// sum (the usage page bolds the line's first amount: "$0.40" read as low beside $25.00 bundled), and DIEM (a stake that
/// pays a daily allowance: shown, not counted).
fn venice(v: &Value) -> Option<Reading> {
    let b = &v["data"]["balances"];
    let (usd, bundled, diem) = (num(&b["USD"]), num(&b["BUNDLED_CREDITS"]).filter(|x| *x > 0.0), num(&b["DIEM"]).filter(|x| *x > 0.0));
    let dollars = (usd.is_some() || bundled.is_some()).then(|| usd.unwrap_or(0.0) + bundled.unwrap_or(0.0)).filter(|d| *d != 0.0 || diem.is_none());
    let parts = match (usd.filter(|u| *u != 0.0), bundled) {
        (Some(u), Some(x)) => format!(" ({} + {} bundled)", amount("USD", u), amount("USD", x)),
        (None, Some(_)) => " (bundled)".into(),
        _ => String::new(),
    };
    let each: Vec<String> = [dollars.map(|d| format!("{}{parts}", money("USD", d).left)), diem.map(|x| format!("{x:.2} DIEM"))].into_iter().flatten().collect();
    if each.is_empty() {
        return None;
    }
    let r = dollars.map_or_else(|| Reading { short: format!("{:.2} DIEM", diem.unwrap_or(0.0)), ..Default::default() }, |d| money("USD", d));
    Some(Reading { left: each.join(", "), ..r })
}

/// A plan's window as OmniRoute's accounts carry them: its name, % used, when it resets (ISO, UTC).
fn window(name: &str, used: f64, resets: Option<String>) -> Value {
    json!({"group": "", "window": name, "used": used.clamp(0.0, 100.0), "resets": resets, "unlimited": false})
}

/// The windows in the row's line: "5-hour 12%, weekly 3% used".
fn summary(windows: &[Value]) -> String {
    let each: Vec<String> = windows.iter().map(|w| format!("{} {:.0}%", w["window"].as_str().unwrap_or_default(), w["used"].as_f64().unwrap_or(0.0))).collect();
    format!("{} used", each.join(", "))
}

/// A plan's windows as a reading: the line their summary, the short one the fullest ("40% used"). None without one.
fn plan(windows: Vec<Value>) -> Option<Reading> {
    let top = windows.iter().filter_map(|w| w["used"].as_f64()).max_by(f64::total_cmp)?;
    Some(Reading { left: summary(&windows), short: format!("{top:.0}% used"), windows, ..Default::default() })
}

/// A reading from an endpoint the provider does not document (the shape as its own or others' tools read it): said so.
const EXPERIMENTAL: &str = " (experimental)";

fn experimental(windows: Vec<Value>) -> Option<Reading> {
    plan(windows).map(|r| Reading { left: format!("{}{EXPERIMENTAL}", r.left), ..r })
}

/// An ISO time in UTC of milliseconds since 1970.
fn iso_ms(ms: f64) -> String {
    let s = (ms / 1000.0) as i64;
    let ((y, m, d), t) = (civil(s.div_euclid(86_400)), s.rem_euclid(86_400));
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", t / 3600, t % 3600 / 60, t % 60)
}

/// MiniMax's Token Plan /token_plan/remains (its fields not documented): only the remaining percents of its `general`
/// bucket (another is the video models'), the 5-hour and weekly windows, with their ends (ms).
fn minimax(v: &Value) -> Option<Reading> {
    if v["base_resp"]["status_code"].as_i64().is_some_and(|c| c != 0) {
        return None;
    }
    let b = v["model_remains"].as_array()?.iter().find(|b| b["model_name"] == "general")?;
    experimental([("5-hour", "current_interval_remaining_percent", "end_time"), ("weekly", "current_weekly_remaining_percent", "weekly_end_time")].iter()
        .filter_map(|(name, pct, end)| Some(window(name, 100.0 - num(&b[*pct])?, num(&b[*end]).map(iso_ms)))).collect())
}

/// Z.ai's and Zhipu's Coding Plan quota (/api/monitor/usage/quota/limit, not documented): `data.limits[]`, each a type
/// (TOKENS_LIMIT the plan's window, `number` of `unit`: 3 hours, 6 weeks, 1 days; TIME_LIMIT its MCP tools), `percentage`
/// used, `nextResetTime` (ms).
fn zai(v: &Value) -> Option<Reading> {
    experimental(v["data"]["limits"].as_array()?.iter().filter_map(|l| {
        let n = l["number"].as_u64().unwrap_or(1);
        let name = match (l["type"].as_str()?, l["unit"].as_u64()) {
            ("TIME_LIMIT", _) => "MCP tools".to_string(),
            (_, Some(3)) => format!("{n}-hour"),
            (_, Some(6)) if n == 1 => "weekly".into(),
            (_, Some(6)) => format!("{n}-week"),
            (_, Some(1)) => format!("{n}-day"),
            ("TOKENS_LIMIT" | "CREDIT_LIMIT", _) => "tokens".into(),
            _ => return None,
        };
        Some(window(&name, num(&l["percentage"])?, num(&l["nextResetTime"]).map(iso_ms)))
    }).collect())
}

/// Kimi Code's /usages (not documented; as Kimi's own CLI reads it): `limits[]` the shorter windows (`window.duration`
/// in its `timeUnit`), `usage` the week; each `used` (or `remaining`) of `limit`, reset at `resetTime`.
fn kimi(v: &Value) -> Option<Reading> {
    let one = |d: &Value, name: &str| -> Option<Value> {
        let limit = num(&d["limit"]).filter(|l| *l > 0.0)?;
        let used = num(&d["used"]).or_else(|| Some(limit - num(&d["remaining"])?))?;
        let resets = ["resetTime", "reset_at", "resetAt", "reset_time"].iter().find_map(|k| d[*k].as_str()).map(String::from);
        Some(window(name, used / limit * 100.0, resets))
    };
    let mut windows: Vec<Value> = v["limits"].as_array().into_iter().flatten().filter_map(|l| {
        let (n, unit) = (num(&l["window"]["duration"])?, l["window"]["timeUnit"].as_str().unwrap_or_default());
        let mins = if unit.contains("MINUTE") { n } else if unit.contains("HOUR") { n * 60.0 } else if unit.contains("DAY") { n * 1440.0 } else { return None };
        let name = if mins % 60.0 == 0.0 { format!("{}-hour", mins / 60.0) } else { format!("{mins}-minute") };
        one(if l["detail"].is_object() { &l["detail"] } else { l }, &name)
    }).collect();
    windows.extend(one(&v["usage"], "weekly"));
    experimental(windows)
}

/// The Qwen Cloud account's use as Qwen's official CLI says it (`qwencloud usage summary --format json`), with the account
/// the CLI is signed in to (`qwencloud auth status`; it may not be the one the key belongs to, so the row names it). Only
/// when the CLI is on PATH and signed in: tazuna never signs it in, nor reads its credentials.
fn qwencloud_usage() -> Option<(Option<String>, Value)> {
    // tests never start the user's CLI
    if cfg!(test) || !crate::platform::on_path("qwencloud", &std::env::var_os("PATH").unwrap_or_default()) {
        return None;
    }
    let status = cli_json("qwencloud", &["auth", "status", "--format", "json"], 5).filter(|s| s["authenticated"] == true)?;
    let account = ["account", "email", "username", "user", "accountName"].iter().find_map(|k| status[*k].as_str()).or(status["user"]["email"].as_str()).map(String::from);
    Some((account, cli_json("qwencloud", &["usage", "summary", "--format", "json"], 8)?))
}

/// Its Token Plan (`token_plan`: `remainingCredits` of `totalCredits`, shown at the precision given; `usedPct`;
/// `resetDate`). None when not subscribed.
fn qwencloud(v: &Value, account: Option<&str>) -> Option<Reading> {
    let t = if v["token_plan"].is_object() { &v["token_plan"] } else { &v["data"]["token_plan"] };
    if t["subscribed"] == false {
        return None;
    }
    let (rem, total) = (num(&t["remainingCredits"]), num(&t["totalCredits"]).filter(|x| *x > 0.0));
    let used = match (rem, total) { (Some(r), Some(n)) => Some((1.0 - r / n) * 100.0), _ => num(&t["usedPct"]) }?;
    let shown = |x: &Value| x.as_str().map_or_else(|| x.to_string(), String::from);
    let text = if rem.is_some() && total.is_some() { format!("{} of {} credits left", shown(&t["remainingCredits"]), shown(&t["totalCredits"])) } else { format!("{:.0}% of credits left", 100.0 - used) };
    let r = plan(vec![window("credits", used, t["resetDate"].as_str().map(String::from))])?;
    Some(Reading { left: format!("{text}{}", account.map_or(String::new(), |a| format!(" (qwencloud signed in as {a})"))), ..r })
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
        let (c, o, _) = (s.spawn(|| credits(None)), s.spawn(omniroute), s.spawn(refresh_claude_login));
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

/// Where a provider's page link starts in its row: after the two-space indent and the name's column, which is as wide as
/// the longest name and two spaces (at least 20): at a fixed 20, "Qwen Cloud Token Plan" ran into its link (2026-11-24).
pub fn link_from(v: &Value) -> usize {
    let longest = v["credits"].as_array().into_iter().flatten().filter_map(|c| c["provider"].as_str()).map(|n| n.chars().count()).max().unwrap_or(0);
    2 + (longest + 2).max(20)
}

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
    // the meters' names, for their bars' column: one for all, just past the longest name (a fixed 35 left "5-hour session"
    // 20 columns of space, and its reset was cut at 80 columns, design review 13)
    let window = |w: &Value| [w["group"].as_str().unwrap_or_default(), w["window"].as_str().unwrap_or_default()].iter()
        .filter(|x| !x.is_empty()).copied().collect::<Vec<_>>().join(", ");
    let col = v["plan"].as_array().into_iter().flatten().map(|r| 2 + window_name(r["label"].as_str().unwrap_or_default()).chars().count())
        .chain(["omniroute", "credits"].iter().flat_map(|k| v[*k].as_array().into_iter().flatten())
            .flat_map(|a| a["windows"].as_array().into_iter().flatten().map(|w| 4 + window(w).chars().count())))
        .max().unwrap_or(0) + 1;
    // a meter's row: its name from `indent`, its bar at that column (the panel shrinks the bars where a row is too wide:
    // ui.rs draw_list)
    let meter = |indent: usize, name: &str, used: f64, resets: &str| {
        format!("{:indent$}{name:<w$}{} {used:>4.0}%{}", "", bar(used, 20), if resets.is_empty() { String::new() } else { format!("   resets {resets}") }, w = col - indent)
    };
    // a quota window's row (an OmniRoute account's, a plan's under its provider)
    let window_row = |w: &Value| {
        let what = window(w);
        if w["unlimited"] == true {
            return (format!("    {what:<30} unlimited"), Plain);
        }
        let used = w["used"].as_f64().unwrap_or(0.0);
        let resets = w["resets"].as_str().and_then(reset_iso).unwrap_or_default();
        (meter(4, &what, used, &resets), if used >= 90.0 { Bad } else { Plain })
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
    let col = link_from(v) - 2;
    if !credits.is_empty() {
        push(String::new(), Plain, None);
        push("Providers' credits".into(), Head, None);
        push(String::new(), Rule, None);
        for c in &credits {
            let name = c["provider"].as_str().unwrap_or_default();
            // a plan's windows as meters under its name, which carries the rest of its line ("108 of 135 requests left",
            // "$25.00 purchased", "(experimental)"; the windows' summary is the meters)
            if let (Some(left), Some(ws)) = (c["left"].as_str(), c["windows"].as_array().filter(|w| !w.is_empty())) {
                let rest = left.strip_prefix(summary(ws).as_str()).unwrap_or(left).trim_start_matches(',').trim();
                push(format!("  {name:<col$}{rest}").trim_end().to_string(), Plain, None);
                for w in ws {
                    let (text, tone) = window_row(w);
                    push(text, tone, None);
                }
                continue;
            }
            match (c["left"].as_str(), c["page"].as_str()) {
                (Some(left), _) => push(format!("  {name:<col$}{left}"), if matches!(c["state"].as_str(), Some("over" | "out")) { Bad } else { Plain }, None),
                (None, Some(page)) => {
                    let host = page.trim_start_matches("https://").trim_start_matches("http://");
                    push(format!("  {name:<col$}{host}"), Plain, Some(page.to_string()));
                }
                (None, None) => push(format!("  {name:<col$}no balance to read"), Dim, None),
            }
        }
        if credits.iter().any(|c| c["left"].is_null()) {
            push("  no balance from their APIs: their page is a click away".into(), Dim, None);
        }
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
                let (text, tone) = window_row(w);
                push(text, tone, None);
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
            "credits": [{"provider": "Mortera", "left": null, "page": "https://mortera.cloud"}, {"provider": "OpenRouter", "left": "$0.17 over its credit", "page": "https://openrouter.ai/keys", "state": "over"}],
            "omniroute": [{"provider": "agy", "name": "a", "plan": "Ultra", "status": "valid", "windows": [{"group": "Gemini Models", "window": "weekly", "used": 1.4, "resets": "2099-11-25T11:03:10.000Z"}]}]}));
        let row = |t: &str| shown.iter().find(|l| l.0.contains(t)).unwrap_or_else(|| panic!("no row with {t}: {shown:?}"));
        use crate::ui::Tone::{Bad, Head, Plain, Rule};
        assert_eq!((row("Claude plan").1, shown[1].1), (Head, Rule), "a section: its heading, a line across");
        assert!(row("5-hour session").0.contains("14%") && row("5-hour session").0.contains("resets ") && !row("5-hour session").0.contains("Europe"), "the zone dropped: {:?}", row("5-hour session"));
        assert_eq!(row("Week, Fable").1, Bad, "a window at 90% or more is red");
        assert_eq!((row("Mortera").2.as_deref(), &row("Mortera").0[22..]), (Some("https://mortera.cloud"), "mortera.cloud"), "its page, a click away");
        // a name longer than the column's 20: the column widens, two spaces stay between it and its link
        let long = json!({"credits": [{"provider": "Qwen Cloud Token Plan", "left": null, "page": "https://home.qwencloud.com/api-keys"}, {"provider": "Mortera", "left": null, "page": "https://mortera.cloud"}]});
        let rows = lines(&long);
        let qwen = rows.iter().find(|l| l.0.contains("Qwen")).unwrap();
        assert_eq!((link_from(&long), &qwen.0[link_from(&long)..]), (25, "home.qwencloud.com/api-keys"), "{qwen:?}");
        assert!(qwen.0.contains("Plan  home"), "{qwen:?}");
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

    /// A reading as its row says it: (left, short, state, dollars to the cent).
    fn shown(r: Option<Reading>) -> (String, String, String, Option<f64>) {
        let v = row(&crate::models::LOGINS[0], r);
        let s = |k: &str| v[k].as_str().unwrap_or("-").to_string();
        (s("left"), s("short"), s("state"), v["usd_left"].as_f64().map(|x| (x * 100.0).round() / 100.0))
    }

    fn sh(left: &str, short: &str, state: &str, usd: Option<f64>) -> (String, String, String, Option<f64>) {
        (left.into(), short.into(), state.into(), usd)
    }

    #[test]
    fn balances_read_from_each_providers_documented_shape() {
        // OpenRouter's /key and the account's /credits: whichever runs out first
        let key = |data: Value| json!({"data": data});
        let credits = |t: f64, u: f64| json!({"data": {"total_credits": t, "total_usage": u}});
        let lim = |r: f64| key(json!({"label": "sk-or-v1-au7...890", "limit": 5.0, "limit_remaining": r, "usage": 25.5, "usage_monthly": 1.8, "is_free_tier": false,
            "free_model_daily_requests": {"used": 12, "limit": 1000, "remaining": 988}}));
        assert_eq!(shown(openrouter(Some(&lim(3.2)), Some(&credits(20.0, 10.0)))), sh("$3.20 left of its $5.00 key limit", "$3.20 left (key)", "ok", Some(3.2)));
        assert_eq!(shown(openrouter(Some(&lim(3.2)), Some(&credits(5.0, 5.17)))), sh("$0.17 over its credit", "$0.17 below zero", "over", Some(-0.17)),
            "the account past zero binds, though the key has $3.20 of its limit");
        let past = openrouter(Some(&lim(-0.4)), Some(&credits(20.0, 10.0)));
        assert!(past.as_ref().is_some_and(|r| r.key_limit), "the desktop says to raise the key's limit, not to add credit");
        assert_eq!(shown(past), sh("$0.40 over its key limit", "key limit used up", "over", Some(-0.4)));
        assert_eq!(shown(openrouter(Some(&lim(1.0)), None)).2, "low", "/credits failed: the key's limit alone");
        let free = key(json!({"limit": null, "limit_remaining": null, "usage_monthly": 0.0, "is_free_tier": true, "free_model_daily_requests": {"used": 12, "limit": 50, "remaining": 38}}));
        assert_eq!(shown(openrouter(Some(&free), Some(&credits(0.0, 0.0)))), sh("38 of 50 free requests left today", "38/50 free today", "ok", None));
        let none_left = key(json!({"limit_remaining": null, "is_free_tier": true, "free_model_daily_requests": {"used": 50, "limit": 50, "remaining": 0}}));
        let fr = row(&crate::models::LOGINS[0], openrouter(Some(&none_left), None));
        assert_eq!((fr["state"].as_str(), fr["free_requests"].as_bool()), (Some("out"), Some(true)), "free requests, not credit: the desktop says they come back tomorrow");
        assert_eq!(shown(openrouter(Some(&free), Some(&json!({"data": {"total_credits": 0, "total_usage": 0.165348734}})))).0, "$0.17 over its credit", "past zero says so, free tier or not");
        let paid = key(json!({"limit": null, "limit_remaining": null, "usage_monthly": 1.25, "is_free_tier": false}));
        assert_eq!(shown(openrouter(Some(&paid), Some(&credits(1234.5, 0.0)))), sh("$1,234.50 left of $1,234.50", "$1,234.50 left", "ok", Some(1234.5)));
        assert_eq!(shown(openrouter(None, Some(&credits(5.0, 5.17)))).0, "$0.17 over its credit", "/key failed: /credits");
        assert_eq!(shown(openrouter(Some(&paid), None)), sh("$1.25 used this month", "$1.25 this month", "unknown", None), "spent is not left: never flagged low");
        assert!(openrouter(None, None).is_none());
        // Mortera, in rupiah, its dollars at engine.rs's rate: low under $2
        assert_eq!(shown(mortera(&json!({"data": {"balance_idr": 796.943078, "held_idr": 0, "available_idr": 796.943078}}))), sh("Rp 797 left (about $0.05)", "Rp 797 left", "low", Some(0.05)));
        assert_eq!(shown(mortera(&json!({"balance_idr": 50000, "held_idr": 2000, "available_idr": 48000}))), sh("Rp 48,000 left, Rp 2,000 held (about $2.94)", "Rp 48,000 left", "ok", Some(2.94)));
        // Cheaper Inference: a key without account:read is told so, and needs you
        assert_eq!(shown(cheaper((200, json!({"object": "account.balance", "currency": "USD", "balance_usd": 12.5, "available_usd": 12.0, "reserved_usd": 0.5})))), sh("$12.00 left, $0.50 reserved", "$12.00 left", "ok", Some(12.0)));
        assert_eq!(shown(cheaper((403, json!({"error": {"code": "insufficient_scope", "message": "requires account:read"}})))), sh("turn on account:read for this key", "needs account:read", "needs_scope", None));
        assert!(cheaper((401, json!({"error": "invalid key"}))).is_none());
        // DeepSeek: strings, per currency; yuan has no rate here, so no dollars, but is low under ¥14
        assert_eq!(shown(deepseek(&json!({"is_available": true, "balance_infos": [{"currency": "CNY", "total_balance": "12.00", "granted_balance": "2.00", "topped_up_balance": "10.00"}]}))), sh("¥12.00 left", "¥12.00 left", "low", None));
        assert_eq!(shown(deepseek(&json!({"is_available": true, "balance_infos": [{"currency": "USD", "total_balance": "4.2"}]}))), sh("$4.20 left", "$4.20 left", "ok", Some(4.2)));
        // Moonshot and SiliconFlow (their parse is in `reading`: the fields as `num` takes them)
        assert_eq!(num(&json!({"code": 0, "data": {"available_balance": 49.58894, "voucher_balance": 46.58893, "cash_balance": 3.00001}})["data"]["available_balance"]), Some(49.58894));
        assert_eq!(num(&json!({"code": 20000, "data": {"balance": "0.88", "chargeBalance": "88.00", "totalBalance": "88.88"}})["data"]["totalBalance"]), Some(88.88));
        assert_eq!(shown(Some(money("USD", -1.5))).2, "over");
        assert_eq!((num(&json!("NaN")), num(&json!("inf")), num(&json!(" 2.5 "))), (None, None, Some(2.5)), "no \"$NaN left\"");
        // Ollama Cloud: included and purchased credit (numbers or strings), or an older plan's windows; nothing else
        let ol = json!({"included": {"balance_usd": "72.5", "allowance_usd": 100, "period": {"from": "2026-09-15T09:30:00Z", "until": "2026-10-15T09:30:00Z"}}, "purchased": {"balance_usd": 25}});
        assert_eq!(shown(ollama(&ol)), sh("$97.50 left ($72.50 of $100.00 included, $25.00 purchased)", "$97.50 left", "ok", Some(97.5)));
        let old = ollama(&json!({"included": {"session": {"remaining_percent": 80, "resets_at": "2026-11-26T15:00:00Z"}, "weekly": {"remaining_percent": 95, "resets_at": "2026-11-30T00:00:00Z"}}, "purchased": {"balance_usd": 3}})).unwrap();
        assert_eq!((old.left.as_str(), old.windows[0]["resets"].as_str()), ("session 20%, weekly 5% used, $3.00 purchased", Some("2026-11-26T15:00:00Z")));
        assert!(ollama(&json!({"included": {"balance": 72.5}, "purchased": {"balance_usd": 25}})).is_none(), "a renamed field is no reading, not \"$25.00 left\"");
        assert_eq!(shown(synthetic(&json!({"subscription": {"limit": 135, "requests": 27, "renewsAt": "2025-09-21T14:36:14.288Z"}}))), sh("108 of 135 requests left", "108/135 left", "ok", None));
        let ve = |b: Value| shown(venice(&json!({"data": {"accessPermitted": true, "apiTier": {"id": "paid", "isCharged": true}, "balances": b}})));
        assert_eq!(ve(json!({"USD": 0.4, "DIEM": 100.023, "BUNDLED_CREDITS": 25})), sh("$25.40 left ($0.40 + $25.00 bundled), 100.02 DIEM", "$25.40 left", "ok", Some(25.4)), "bundled credit is spendable, and said first: not low");
        assert_eq!(ve(json!({"USD": 0, "DIEM": 0, "BUNDLED_CREDITS": 25})).0, "$25.00 left (bundled)");
        assert_eq!(ve(json!({"USD": 50.23})).0, "$50.23 left");
        assert_eq!(ve(json!({"USD": 0, "DIEM": 2.5, "BUNDLED_CREDITS": 0})), sh("2.50 DIEM", "2.50 DIEM", "unknown", None), "no $0.00 ahead of the DIEM");
        assert_eq!(ve(json!({"USD": 0, "DIEM": 0, "BUNDLED_CREDITS": 0})), sh("$0.00 left", "$0.00 left", "out", Some(0.0)));
        assert_eq!(shown(None), sh("-", "-", "unknown", None));
        assert_eq!(crate::models::LOGINS.iter().filter(|l| ["Ollama Cloud", "Synthetic", "Venice", "DeepSeek", "Moonshot", "SiliconFlow", "MiniMax Plan", "Kimi Code"].contains(&l.name)).count(), 8, "each reader's provider is in LOGINS by that name");
        assert!(super::credits(Some("no such provider")).is_empty(), "only the provider named");
    }

    #[test]
    fn plan_windows_read_as_omniroutes_and_say_experimental() {
        let at = |w: &Value| (w["window"].as_str().unwrap_or_default().to_string(), w["used"].as_f64().unwrap_or(-1.0), w["resets"].as_str().map(String::from));
        // MiniMax: only the remaining percents, of the general bucket alone
        let general = json!({"model_name": "general", "current_interval_remaining_percent": 88, "current_interval_total_count": 0, "start_time": 1780347600000u64, "end_time": 1780365600000u64,
             "current_weekly_remaining_percent": 97, "weekly_start_time": 1780243200000u64, "weekly_end_time": 1780848000000u64});
        let video = json!({"model_name": "video", "current_interval_remaining_percent": 10});
        let mm = minimax(&json!({"model_remains": [video, general], "base_resp": {"status_code": 0, "status_msg": "success"}})).unwrap();
        assert_eq!((mm.left.as_str(), mm.short.as_str()), ("5-hour 12%, weekly 3% used (experimental)", "12% used"));
        assert_eq!(at(&mm.windows[0]), ("5-hour".into(), 12.0, Some(iso_ms(1780365600000.0))));
        assert_eq!(iso_secs(&iso_ms(1780365600000.0)), Some(1780365600), "ms to ISO and back");
        assert!(minimax(&json!({"model_remains": [video], "base_resp": {"status_code": 0}})).is_none(), "the video bucket is not the plan's");
        assert!(minimax(&json!({"model_remains": [], "base_resp": {"status_code": 1004, "status_msg": "invalid key"}})).is_none());
        // Z.ai / Zhipu: TOKENS_LIMIT over 5 hours, a weekly one, TIME_LIMIT the MCP tools; the fullest decides the state
        let z = zai(&json!({"code": 200, "msg": "success", "success": true, "data": {"limits": [
            {"type": "TOKENS_LIMIT", "unit": 3, "number": 5, "usage": 800000, "currentValue": 96000, "remaining": 704000, "percentage": 12, "nextResetTime": 1764166000000u64},
            {"type": "TOKENS_LIMIT", "unit": 6, "number": 1, "percentage": 95, "nextResetTime": 1764500000000u64},
            {"type": "TIME_LIMIT", "unit": 5, "number": 1, "usage": 1000, "currentValue": 30, "remaining": 970, "percentage": 3}, {"type": "SOMETHING_NEW", "percentage": 50}]}})).unwrap();
        assert_eq!(z.left, "5-hour 12%, weekly 95%, MCP tools 3% used (experimental)");
        assert_eq!(at(&z.windows[2]).2, None, "no reset given, none made up");
        let zr = row(&crate::models::LOGINS[0], Some(z));
        assert_eq!((zr["max_used_pct"].as_f64(), zr["state"].as_str(), zr["short"].as_str()), (Some(95.0), Some("out"), Some("95% used")), "a plan at 95%: red, as its meter");
        let at_pct = |p: f64| shown(plan(vec![window("weekly", p, None)])).2;
        assert_eq!([at_pct(74.0), at_pct(75.0), at_pct(89.0), at_pct(90.0)], ["ok", "low", "low", "out"].map(String::from), "the meters' limits, 75 and 90 (desktop byPct)");
        assert!(zai(&json!({"code": 200, "data": {"limits": []}})).is_none());
        assert_eq!(shown(zai(&json!({"data": {"limits": [{"type": "TOKENS_LIMIT", "unit": 3, "number": 5, "percentage": 100}]}}))).2, "out");
        // Kimi Code: the 5-hour window (300 minutes) and the week
        let k = kimi(&json!({"usage": {"limit": "100", "used": "20", "remaining": "80", "resetTime": "2026-11-30T05:24:18.443553353Z"},
            "limits": [{"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"}, "detail": {"limit": "50", "remaining": "45", "resetTime": "2026-11-26T15:00:00Z"}}]})).unwrap();
        assert_eq!((k.left.as_str(), k.short.as_str()), ("5-hour 10%, weekly 20% used (experimental)", "20% used"));
        assert_eq!(at(&k.windows[1]), ("weekly".into(), 20.0, Some("2026-11-30T05:24:18.443553353Z".into())));
        assert!(kimi(&json!({})).is_none());
        // Qwen Cloud's CLI: the Token Plan's Credits as given, named with the account the CLI is signed in to
        let usage = json!({"period": {"from": "2026-11-01", "to": "2026-11-26"}, "token_plan": {"subscribed": true, "planName": "Pro", "status": "valid",
            "totalCredits": "2000", "remainingCredits": "1500.25", "usedPct": 24.99, "resetDate": "2026-12-01T00:00:00Z"}, "coding_plan": {"subscribed": false}});
        let q = qwencloud(&usage, Some("someone@example.com")).unwrap();
        assert_eq!((q.left.as_str(), q.short.as_str(), at(&q.windows[0]).0), ("1500.25 of 2000 credits left (qwencloud signed in as someone@example.com)", "25% used", "credits".to_string()));
        assert!(qwencloud(&json!({"token_plan": {"subscribed": false}}), None).is_none());
        assert_eq!(qwencloud(&json!({"token_plan": {"subscribed": true, "usedPct": 30}}), None).map(|r| r.left).as_deref(), Some("70% of credits left"));
        // the short lines fit the cockpit's cost box (34 columns inside) with the provider's name beside them
        for r in [&mm.short, &k.short, &q.short, &shown(synthetic(&json!({"subscription": {"limit": 1350, "requests": 27}}))).1, &shown(openrouter(Some(&json!({"data": {"limit": 5000.0, "limit_remaining": 1234.5}})), None)).1, &shown(openrouter(Some(&json!({"data": {"limit": 50.0, "limit_remaining": -1234.5}})), None)).1] {
            assert!(r.chars().count() <= 20, "{r}");
        }
        // the terminal: a plan's windows as meters under its name, which carries the rest of its line; no page footer when all read
        let syn = row(&crate::models::LOGINS[0], synthetic(&json!({"subscription": {"limit": 135, "requests": 27, "renewsAt": "2099-09-21T14:36:14.288Z"}})));
        let zrow = row(&crate::models::LOGINS[0], zai(&json!({"data": {"limits": [{"type": "TIME_LIMIT", "unit": 5, "number": 1, "percentage": 3}]}})));
        let shown = lines(&json!({"credits": [{"provider": "Z.ai Coding Plan", "left": zrow["left"], "page": "https://z.ai", "windows": zrow["windows"], "state": "ok"},
            {"provider": "Synthetic", "left": syn["left"], "page": "https://synthetic.new", "windows": syn["windows"], "state": "ok"},
            {"provider": "Mortera", "left": "Rp 797 left (about $0.05)", "page": "https://mortera.cloud", "state": "low"}]}));
        let row = |t: &str| shown.iter().find(|l| l.0.contains(t)).unwrap_or_else(|| panic!("no row with {t}: {shown:?}"));
        assert!(row("Z.ai Coding Plan").0.ends_with("(experimental)") && row("MCP tools").0.contains('█'), "{shown:?}");
        assert!(row("Synthetic").0.ends_with("108 of 135 requests left") && row("20%").0.contains("requests"), "{shown:?}");
        assert!(!shown.iter().any(|l| l.0.contains("a click away")), "{shown:?}");
    }

    #[test]
    fn a_reading_is_shared_for_a_minute_and_a_failed_one_keeps_the_last() {
        let dir = std::env::temp_dir().join(format!("tazuna-credits-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let l = crate::models::LOGINS.iter().find(|l| l.name == "Ollama Cloud").unwrap();
        let calls = std::cell::Cell::new(0);
        let read = |r: Option<Reading>| { calls.set(calls.get() + 1); r };
        let t0 = 1_000_000;
        assert_eq!(cached(Some(&dir), l, "k1", t0, || read(Some(money("USD", 5.0))))["left"], "$5.00 left");
        assert_eq!(cached(Some(&dir), l, "k1", t0 + 30, || read(Some(money("USD", 4.0))))["left"], "$5.00 left", "within the minute: kept, not asked");
        assert_eq!(calls.get(), 1);
        assert_eq!(cached(Some(&dir), l, "k1", t0 + 61, || read(None))["left"], "$5.00 left", "a failed read keeps the last good row");
        assert_eq!(cached(Some(&dir), l, "k1", t0 + 130, || read(Some(money("USD", 4.0))))["left"], "$4.00 left");
        assert_eq!(cached(Some(&dir), l, "k2", t0 + 131, || read(None))["left"], Value::Null, "another key: read at once, none of the old key's");
        assert_eq!(cached(Some(&dir), l, "k2", t0 + 200, || read(None))["state"], "unknown");
        assert_eq!(cached(Some(&dir), l, "k1", t0 + 100_000, || read(None))["left"], Value::Null, "a day on: the old row is gone");
        assert_eq!(calls.get(), 6);
        let k = std::fs::read_to_string(dir.join("Ollama-Cloud.json")).unwrap();
        assert!(!k.contains("k1") && !k.contains("k2"), "only a hash of the key on disk: {k}");
        assert_eq!(cached(None, l, "k1", t0, || read(Some(money("USD", 1.0))))["state"], "low", "no folder: read each time");
        let _ = std::fs::remove_dir_all(&dir);
        // a reader that does not answer in time gives way: the others' rows come at once
        let (tx, rx) = std::sync::mpsc::channel();
        let late = tx.clone();
        tx.send((0, json!({"provider": "a"}))).unwrap();
        drop(tx);
        let t = std::time::Instant::now();
        let rows = gather(&rx, 2, Duration::from_millis(300));
        assert!(rows[0].is_some() && rows[1].is_none() && t.elapsed() < Duration::from_secs(2), "{rows:?}");
        drop(late);
    }

    #[cfg(unix)]
    #[test]
    fn a_cli_that_hangs_is_stopped_with_what_it_started() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("tazuna-cli-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (script, pid) = (dir.join("launcher"), dir.join("pid"));
        // a launcher that starts its program and waits, as an npm shim that does not exec
        std::fs::write(&script, format!("#!/bin/sh\nsleep 30 &\necho $! > {}\nwait\n", pid.display())).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(cli_json(script.to_str().unwrap(), &[], 1).is_none());
        std::thread::sleep(Duration::from_millis(300));
        let child: i32 = std::fs::read_to_string(&pid).unwrap().trim().parse().unwrap();
        // SAFETY: signal 0 only asks whether the process is there
        assert!(unsafe { libc::kill(child, 0) } != 0, "the launcher's child was left running");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
