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

/// Each tracked provider (only `only`, where named: after a run, the provider it ran on), read its chosen way (`track`):
/// `local` and `console` from files here, the others (`api`, `api_scope`, `cli`) from the reading every tazuna process on
/// this machine shares (`cached`: asked once a minute at most), each reader on a thread of its own; those not done within
/// 12 s (a reader makes up to three requests of 10 s each, the Qwen CLI two starts) give their last good row, and finish
/// into the shared reading for the next look. A reading that fails is no reading, never an error.
#[inline(never)]
pub fn credits(only: Option<&str>) -> Vec<Value> {
    credits_in(&usage_conf(usage_file().as_deref()), only)
}

fn credits_in(conf: &Value, only: Option<&str>) -> Vec<Value> {
    let base = crate::labels::dir().ok();
    rows_of(&tracked_in(conf, only, &|v| crate::models::key(v)), conf, base.as_deref(), crate::session::unix_now(), reading)
}

/// The providers to read: tracked on, with a key where their method needs one (console and local know it by name).
fn tracked_in(conf: &Value, only: Option<&str>, key: &dyn Fn(&str) -> Option<String>) -> Vec<(&'static crate::models::Login, String, &'static str)> {
    crate::models::LOGINS.iter().filter(|l| l.name != "OmniRoute" && only.is_none_or(|o| o == l.name)).filter_map(|l| {
        let key = l.vars.iter().find_map(|v| key(v));
        let (on, method) = track(l, conf, key.is_some());
        (on && (key.is_some() || matches!(method, "console" | "local"))).then(|| (l, key.unwrap_or_default(), method))
    }).collect()
}

/// The rows of `tracked`, each by its method; `read` is the readers' (`reading`; a stand-in in tests).
fn rows_of(tracked: &[(&'static crate::models::Login, String, &'static str)], conf: &Value, base: Option<&std::path::Path>, now: u64,
    read: fn(&crate::models::Login, &str) -> Option<Reading>) -> Vec<Value> {
    let dir = base.map(|d| d.join("credits"));
    // the run log only when a provider is counted here
    let runs = if tracked.iter().any(|t| t.2 == "local") { base.map(crate::labels::runs).unwrap_or_default() } else { vec![] };
    let (tx, rx) = std::sync::mpsc::channel();
    for (i, (l, key, method)) in tracked.iter().cloned().enumerate() {
        if !matches!(method, "api" | "api_scope" | "cli") {
            continue;
        }
        let (tx, dir) = (tx.clone(), dir.clone());
        std::thread::spawn(move || {
            let _ = tx.send((i, cached(dir.as_deref(), l, &key, crate::session::unix_now(), || read(l, &key))));
        });
    }
    drop(tx);
    let got = gather(&rx, tracked.len(), Duration::from_secs(12));
    // ponytail: no local offset on Windows (the code has no call for it there): its periods start at UTC midnight
    let off = |t: i64| local_offset(t).unwrap_or(0);
    tracked.iter().zip(got).map(|((l, key, method), r)| match *method {
        "local" => local_row(l, allowance_of(conf, l.name).as_ref(), &runs, now as i64, off),
        // no reading and no first visit: the user has a step to take, not tazuna (the status bar said "no reading yet" for a
        // console never signed in to, the user 2026-11-27)
        "console" => match console_reading(base, l, now as i64) {
            None if !base.is_some_and(|b| b.join("console-learn").join(slug(l.name)).is_file()) =>
                row(l, Some(Reading { left: "sign in to its console in Settings".into(), short: "sign in from Settings".into(), ..Default::default() })),
            c => console_row(l, c, now as i64),
        },
        "api" | "api_scope" | "cli" => r.unwrap_or_else(|| kept_row(&kept(dir.as_deref(), l, key), now).unwrap_or_else(|| row(l, None))),
        _ => row(l, None),
    }).collect()
}

/// The providers with a key whose usage is not read (`track` off): /usage names them in one line.
fn untracked_in(conf: &Value, key: &dyn Fn(&str) -> Option<String>) -> Vec<&'static str> {
    crate::models::LOGINS.iter().filter(|l| l.name != "OmniRoute" && l.vars.iter().any(|v| key(v).is_some()) && !track(l, conf, true).0)
        .map(|l| l.name).collect()
}

/// The credits alone, read again after a usage choice (not Claude's plan nor OmniRoute): the app server's
/// `accounts_credits` line, merged into the accounts as `accounts_plan` is; rows and the untracked from one read of the
/// choices.
#[inline(never)]
pub fn credits_reading() -> Value {
    let conf = usage_conf(usage_file().as_deref());
    json!({"type": "accounts_credits", "credits": credits_in(&conf, None), "untracked": untracked_in(&conf, &|v| crate::models::key(v))})
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
    let Some(f) = dir.map(|d| d.join(slug(l.name))) else { return Value::Null };
    let v: Value = std::fs::read_to_string(&f).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    if v["kh"] == key_hash(key) { v } else { Value::Null }
}

/// A provider's file name: "Qwen Cloud Token Plan" is "Qwen-Cloud-Token-Plan.json".
fn slug(name: &str) -> String {
    format!("{}.json", name.replace(|c: char| !c.is_ascii_alphanumeric(), "-"))
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
    cached_within(dir, l, key, now, 60, read)
}

/// The same, read again once `within` seconds old (0: now, for Settings' Check again).
fn cached_within(dir: Option<&std::path::Path>, l: &crate::models::Login, key: &str, now: u64, within: u64, read: impl FnOnce() -> Option<Reading>) -> Value {
    let k = kept(dir, l, key);
    if k["at"].as_u64().is_some_and(|at| now.saturating_sub(at) < within) && k["row"].is_object() {
        return k["row"].clone();
    }
    let file = dir.map(|d| d.join(slug(l.name)));
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
            let mut v = kept_row(&k, now).unwrap_or_else(|| row(l, None));
            // a quiet row (Qwen's CLI without the plan) has no words to keep it by: its mark stays, or a CLI that timed out
            // would read as signed in
            if v["left"].is_null() && k["row"]["cli_plan"] == false {
                v["cli_plan"] = json!(false);
            }
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
    /// Qwen's CLI says its account has no Token Plan (CLI_NO_PLAN): a row with nothing in it
    cli_no_plan: bool,
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
    let quiet = || json!({"provider": l.name, "left": null, "short": null, "page": l.page, "usd_left": null, "max_used_pct": null, "state": "unknown"});
    let Some(r) = r else { return quiet() };
    if r.cli_no_plan {
        let mut v = quiet();
        v["cli_plan"] = json!(false);
        return v;
    }
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
    let account = qwencloud_signed_in().ok()?;
    Some((account, cli_json("qwencloud", &["usage", "summary", "--format", "json"], 8)?))
}

/// The account Qwen's CLI is signed in to (`qwencloud auth status`), or why it cannot read: not installed, not signed in.
fn qwencloud_signed_in() -> Result<Option<String>, &'static str> {
    // tests never start the user's CLI
    if cfg!(test) || !crate::platform::on_path("qwencloud", &std::env::var_os("PATH").unwrap_or_default()) {
        return Err(CLI_MISSING);
    }
    let status = cli_json("qwencloud", &["auth", "status", "--format", "json"], 5).filter(|s| s["authenticated"] == true).ok_or(CLI_SIGNED_OUT)?;
    Ok(cli_account(&status))
}

/// The account `qwencloud auth status` names: CLI 1.7.0 says `user.aliyunId`, masked as the CLI prints it
/// ("someone*****@example.com").
fn cli_account(status: &Value) -> Option<String> {
    ["account", "email", "username", "user", "accountName"].iter().find_map(|k| status[*k].as_str())
        .or(status["user"]["email"].as_str()).or(status["user"]["aliyunId"].as_str()).map(String::from)
}

/// Why Qwen's CLI cannot read, as the usage line says it (the method's `why`).
const CLI_MISSING: &str = "Not installed";
const CLI_SIGNED_OUT: &str = "Not signed in";
/// Signed in, and its usage says no Token Plan: CLI 1.7.0 says so for an account whose Individual plan the console shows
/// active (the user, 2026-11-27), so it claims nothing about the account; its console reads the plan.
const CLI_NO_PLAN: &str = "CLI doesn't show this plan";

/// Its Token Plan (`token_plan`: `remainingCredits` of `totalCredits`, shown at the precision given; `usedPct`;
/// `resetDate`); not subscribed, a quiet row (no value, no warning: the CLI may not see the plan) marked for the usage
/// line (`cli_plan` false). None when it says neither.
fn qwencloud(v: &Value, account: Option<&str>) -> Option<Reading> {
    let t = if v["token_plan"].is_object() { &v["token_plan"] } else { &v["data"]["token_plan"] };
    if t["subscribed"] == false {
        return Some(Reading { cli_no_plan: true, ..Default::default() });
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
    let conf = usage_conf(usage_file().as_deref());
    std::thread::scope(|s| {
        // claude's login read again too (the pickers' Claude rows follow it)
        let (c, o, _) = (s.spawn(|| credits_in(&conf, None)), s.spawn(omniroute), s.spawn(refresh_claude_login));
        let plan = claude_plan();
        let mut v = json!({
            "plan": plan.as_ref().map(|(rows, _)| rows.iter().map(|r| json!({"label": r.label, "used": r.used, "resets": r.resets})).collect::<Vec<_>>()),
            "plan_notes": plan.as_ref().map(|(_, n)| n.clone()).unwrap_or_default(),
        });
        early(&v);
        v["credits"] = json!(c.join().unwrap_or_default());
        v["omniroute"] = json!(o.join().unwrap_or_default());
        // the keyed providers whose usage is not read: /usage names them
        v["untracked"] = json!(untracked_in(&conf, &|v| crate::models::key(v)));
        v
    })
}

// Usage sources (2026-11-26): which providers' usage is read, and how; the user chooses on the settings page
// (out/review-14/usage-sources-spec.md).

/// The providers whose documented endpoint `reading` reads with their key alone.
const API: [&str; 16] = ["SubRouter", "TokenRouter", "OpenRouter", "Mortera", "DeepSeek", "Moonshot", "SiliconFlow", "SiliconFlow China", "Ollama Cloud",
    "Synthetic", "Venice", "MiniMax Plan", "Z.ai Coding Plan", "Zhipu Coding Plan", "Kimi Code", "Kimi Code China"];

/// Where Qwen's CLI is documented.
const QWENCLOUD_CLI: &str = "https://github.com/QwenCloud/qwencloud-cli";

/// The ways tazuna can read a provider's usage, best first: `api` (its documented endpoint, `reading`), `api_scope` (the
/// same once the key may read the account: Cheaper Inference's account:read), `admin_key` (Mistral's API tells this
/// month's spend only, to an Admin key: not read), `cli` (Qwen's official CLI), `console` (what the desktop's console
/// reader wrote), `local` (tazuna's own use against an allowance the user sets: any provider), and `none` where nothing
/// reads it automatically (its page).
pub fn methods(name: &str) -> Vec<&'static str> {
    let own: &[&'static str] = match name {
        "Cheaper Inference" => &["api_scope"],
        "Qwen Cloud Token Plan" => &["cli", "console"],
        "Mistral" => &["admin_key"],
        "Mistral Vibe" => &["admin_key", "console"],
        "Caffeine" => &["console"],
        n if API.contains(&n) => &["api"],
        _ => &[],
    };
    let mut m = own.to_vec();
    m.push("local");
    if !own.iter().any(|x| matches!(*x, "api" | "api_scope" | "cli" | "console")) {
        m.push("none");
    }
    m
}

/// A method's steps in plain words (what tazuna does, what the user does) and the page they need.
fn steps(l: &crate::models::Login, method: &str) -> (String, &'static str) {
    let n = l.name;
    match method {
        "api" => (format!("tazuna reads it from {n}'s API with your key, at most once a minute. Nothing to do once the key is saved."), l.page),
        "api_scope" => (format!("tazuna reads it from {n}'s API once the key may read the account: turn on account:read for this key on its dashboard, then check again."), l.page),
        "admin_key" => (format!("{n}'s API tells only this month's spend, and only to an Admin key; tazuna does not read it. Count it here instead, or look on its console."), l.page),
        "cli" => ("Install Qwen's CLI (npm install -g @qwencloud/qwencloud-cli) and sign in with the Token Plan's account (qwencloud auth login); tazuna reads qwencloud usage summary.".into(), QWENCLOUD_CLI),
        "console" => (format!("Sign in to {n}'s console once in tazuna's own window (Sign in…); tazuna reads its usage page there, at most every 15 minutes. Experimental: automated reading may break the provider's terms; your own account."), l.page),
        "local" => (format!("tazuna counts its own use of {n} against an allowance you enter (amount, unit, period). Use through other apps is not counted."), l.page),
        _ => (format!("{n} offers no way to read its usage: see its page."), l.page),
    }
}

/// The user's choices: ~/.config/tazuna/usage.json, `{"track": {provider: {"on", "method"}}, "allowance": {provider:
/// {"amount", "unit", "period", "reset"}}}`, beside the spend cap; a provider not in it takes the default (`track`).
fn usage_file() -> Option<std::path::PathBuf> {
    crate::platform::home().map(|h| h.join(".config/tazuna/usage.json"))
}

fn usage_conf(file: Option<&std::path::Path>) -> Value {
    file.and_then(|f| std::fs::read_to_string(f).ok()).and_then(|t| serde_json::from_str::<Value>(&t).ok()).filter(Value::is_object).unwrap_or_else(|| json!({}))
}

/// Whether `l`'s usage is read, and how: the user's choice where its method is one of `l`'s and can be on; else on, by
/// its endpoint or Qwen's CLI, for a provider with a key (as before tracking), off for the rest; the method then its first
/// that reads (never `admin_key` or `none`).
fn track(l: &crate::models::Login, conf: &Value, has_key: bool) -> (bool, &'static str) {
    let ms = methods(l.name);
    let t = &conf["track"][l.name];
    if let (Some(on), Some(m)) = (t["on"].as_bool(), t["method"].as_str().and_then(|m| ms.iter().find(|x| **x == m))) {
        return (on && !matches!(*m, "admin_key" | "none"), m);
    }
    let first = ms.iter().copied().find(|m| !matches!(*m, "admin_key" | "none")).unwrap_or("local");
    (has_key && matches!(first, "api" | "api_scope" | "cli"), first)
}

/// An allowance checked: `amount` above 0, `unit` tokens|usd|requests, `period` day|week|month, `reset` the weekday it
/// starts (0 Sunday to 6) for a week, the day of the month (1 to 31; a shorter month resets on its last) for a month, null
/// for a day.
pub fn allowance(v: &Value) -> Result<Value, &'static str> {
    let amount = v["amount"].as_f64().filter(|a| a.is_finite() && *a > 0.0 && *a <= 1e15).ok_or("the amount is a number above 0, at most 1e15")?;
    let unit = ["tokens", "usd", "requests"].into_iter().find(|u| v["unit"] == *u).ok_or("the unit is tokens, usd or requests")?;
    let period = ["day", "week", "month"].into_iter().find(|p| v["period"] == *p).ok_or("the period is day, week or month")?;
    let reset = match period {
        "day" => Value::Null,
        "week" => json!(v["reset"].as_u64().filter(|r| *r <= 6).ok_or("a week resets on a weekday, 0 (Sunday) to 6")?),
        _ => json!(v["reset"].as_u64().filter(|r| (1..=31).contains(r)).ok_or("a month resets on a day from 1 to 31")?),
    };
    Ok(json!({"amount": amount, "unit": unit, "period": period, "reset": reset}))
}

fn allowance_of(conf: &Value, name: &str) -> Option<Value> {
    allowance(&conf["allowance"][name]).ok()
}

/// A provider usage can be set for: in LOGINS, not OmniRoute (its accounts have their own section).
fn usage_login(name: &str) -> Result<&'static crate::models::Login, String> {
    crate::models::LOGINS.iter().find(|l| l.name == name && l.name != "OmniRoute").ok_or_else(|| format!("no provider named {name}"))
}

/// `{"provider", "on", "method"}` into the choices.
fn track_edit(conf: &mut Value, cmd: &Value) -> Result<(), String> {
    let l = usage_login(cmd["provider"].as_str().unwrap_or_default())?;
    let on = cmd["on"].as_bool().ok_or("on is true or false")?;
    let method = cmd["method"].as_str().filter(|m| methods(l.name).contains(m)).ok_or_else(|| format!("{} has no method {}", l.name, cmd["method"]))?;
    if on && matches!(method, "none" | "admin_key") {
        return Err(format!("{} cannot be read that way: choose another method", l.name));
    }
    conf["track"][l.name] = json!({"on": on, "method": method});
    Ok(())
}

/// `{"provider", "allowance": {...} | null}` into the choices (null: none).
fn allowance_edit(conf: &mut Value, cmd: &Value) -> Result<(), String> {
    let l = usage_login(cmd["provider"].as_str().unwrap_or_default())?;
    match &cmd["allowance"] {
        Value::Null => {
            conf["allowance"].as_object_mut().map(|m| m.shift_remove(l.name));
        }
        a => conf["allowance"][l.name] = allowance(a)?,
    }
    Ok(())
}

/// The choices read, edited by `edit` and written whole (moved into place: another process never reads half of it), under
/// a lock file (`usage.json.lock`, made only when absent: one writer at a time; one left over 10 s is a crashed writer's).
/// A file that is there but not a JSON object is never written over (it may hold the user's allowances): an error to show.
/// A `track` or `allowance` that is not an object (a hand edit) starts empty.
fn edit_conf(file: Option<&std::path::Path>, edit: impl FnOnce(&mut Value) -> Result<(), String>) -> Result<(), String> {
    let f = file.ok_or("no home folder")?;
    std::fs::create_dir_all(f.parent().unwrap_or(f)).map_err(|e| format!("the choice was not saved: {e}"))?;
    let lock = f.with_extension("json.lock");
    let mut tries = 0;
    while std::fs::OpenOptions::new().write(true).create_new(true).open(&lock).is_err() {
        let old = std::fs::metadata(&lock).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|e| e > Duration::from_secs(10));
        if old {
            let _ = std::fs::remove_file(&lock);
        } else if tries >= 40 {
            return Err("the choice was not saved: another tazuna is saving one; try again".into());
        } else {
            tries += 1;
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    let done = (|| {
        let mut conf = match std::fs::read_to_string(f) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({}),
            Err(e) => return Err(format!("the choice was not saved: {e}")),
            Ok(t) => serde_json::from_str::<Value>(&t).ok().filter(Value::is_object)
                .ok_or_else(|| format!("the choice was not saved: {} is not valid JSON; fix or delete it", f.display()))?,
        };
        for k in ["track", "allowance"] {
            if !conf[k].is_object() {
                conf[k] = json!({});
            }
        }
        edit(&mut conf)?;
        let tmp = f.with_extension(format!("{}.tmp", std::process::id()));
        std::fs::write(&tmp, serde_json::to_string_pretty(&conf).unwrap_or_default()).and_then(|_| std::fs::rename(&tmp, f))
            .map_err(|e| format!("the choice was not saved: {e}"))
    })();
    let _ = std::fs::remove_file(&lock);
    done
}

/// The app server's `usage_track` (whether to read a provider's usage, and how) and `usage_allowance` (its allowance for
/// local counting, or none): saved on the event loop, in the order they came; None for another op.
#[inline(never)]
pub fn usage_save(cmd: &Value) -> Option<Result<(), String>> {
    usage_save_in(cmd, usage_file().as_deref())
}

fn usage_save_in(cmd: &Value, file: Option<&std::path::Path>) -> Option<Result<(), String>> {
    match cmd["op"].as_str() {
        Some("usage_track") => Some(edit_conf(file, |c| track_edit(c, cmd))),
        Some("usage_allowance") => Some(edit_conf(file, |c| allowance_edit(c, cmd))),
        _ => None,
    }
}

/// `/usage track <provider>` (any case): read with its first method that can read now, else its first that reads at all.
/// Quick enough for the screen's thread: the CLI is looked for on PATH, not started.
#[inline(never)]
pub fn track_by_name(name: &str) -> Result<String, String> {
    let cli = if crate::platform::on_path("qwencloud", &std::env::var_os("PATH").unwrap_or_default()) { Ok(None) } else { Err(CLI_MISSING) };
    track_by_name_in(name, usage_file().as_deref(), crate::labels::dir().ok().as_deref(), &|v| crate::models::key(v), &cli)
}

fn track_by_name_in(name: &str, file: Option<&std::path::Path>, base: Option<&std::path::Path>, key: &dyn Fn(&str) -> Option<String>,
    cli: &Result<Option<String>, &'static str>) -> Result<String, String> {
    let l = crate::models::LOGINS.iter().find(|l| l.name.eq_ignore_ascii_case(name.trim()) && l.name != "OmniRoute").ok_or_else(|| format!("no provider named {}", name.trim()))?;
    let conf = usage_conf(file);
    let k = l.vars.iter().find_map(|v| key(v));
    let ms: Vec<&str> = methods(l.name).into_iter().filter(|m| !matches!(*m, "admin_key" | "none")).collect();
    let method = ms.iter().find(|m| why_not(l, m, k.as_deref(), &conf, base, cli).is_none()).or(ms.first()).copied().unwrap_or("local");
    edit_conf(file, |c| track_edit(c, &json!({"provider": l.name, "on": true, "method": method})))?;
    let wait = if k.is_none() && !matches!(method, "console" | "local") { ", once it has a key" } else { "" };
    Ok(format!("usage: {} tracked ({method}){wait}", l.name))
}

/// Why `method` cannot read `l`'s usage now, in the usage line's few words; None: it can. `cli` is Qwen's CLI as
/// `qwencloud_signed_in` found it; signed in, the last shared reading says whether it shows the Token Plan.
fn why_not(l: &crate::models::Login, method: &str, key: Option<&str>, conf: &Value, base: Option<&std::path::Path>, cli: &Result<Option<String>, &'static str>) -> Option<String> {
    let last = || kept_row(&kept(base.map(|b| b.join("credits")).as_deref(), l, key.unwrap_or_default()), crate::session::unix_now());
    match method {
        "api" | "api_scope" if key.is_none() => Some("No key saved".into()),
        // the last reading said the key may not read the account
        "api_scope" => last().is_some_and(|r| r["state"] == "needs_scope").then(|| "Needs account:read".into()),
        "api" => None,
        "cli" => match cli {
            Err(e) => Some(e.to_string()),
            // the row as last written (kept_row keeps only rows with words, and this one has none)
            Ok(_) => (kept(base.map(|b| b.join("credits")).as_deref(), l, key.unwrap_or_default())["row"]["cli_plan"] == false).then(|| CLI_NO_PLAN.into()),
        },
        // no reading is not signed out: the first visit may have found the page, and the reader comes later
        "console" => match console_reading(base, l, crate::session::unix_now() as i64) {
            None => Some("No reading yet".into()),
            Some(Err(())) => Some("Reading not valid".into()),
            Some(Ok(c)) if c["status"] == "signed_out" => Some("Signed out".into()),
            Some(Ok(c)) if c["status"] == "error" => Some("Last reading failed".into()),
            Some(Ok(_)) => None,
        },
        "local" => allowance_of(conf, l.name).is_none().then(|| "No allowance set".into()),
        "admin_key" => Some("tazuna does not read Admin keys".into()),
        _ => Some("No usage API".into()),
    }
}

/// What to run for Qwen's CLI now, by why it cannot read (the usage line's commands, each with Copy): none once it reads,
/// nor when it does not show the plan (the console is the way then).
fn cli_commands(why: Option<&str>) -> Vec<&'static str> {
    const LOGIN: &str = "qwencloud auth login";
    match why {
        Some(CLI_MISSING) => vec!["npm install -g @qwencloud/qwencloud-cli", LOGIN],
        Some(CLI_SIGNED_OUT) => vec![LOGIN],
        _ => vec![],
    }
}

/// The settings page's usage lines (the app server's `usage_sources`): each provider, whether it has a key, its methods
/// with their steps, link and whether each can read now (`ready`, else `why`), what is tracked, its allowance. Starts
/// Qwen's CLI (up to 5 s, and signed in its usage up to 8 s more where the shared reading is a minute old; the credits read
/// next take it) only where the op is about Qwen or about every provider (`usage_sources` with no provider: Settings
/// opening); a change or a Check for another provider says the CLI as last asked, and waits on nothing of Qwen's.
/// `usage_sources` for one provider is its Check again: its reading read again now, not the shared one of the last minute
/// (a key just given account:read went on saying it needs it). Off the event loop.
#[inline(never)]
pub fn sources(cmd: &Value) -> Value {
    static CLI_LAST: std::sync::Mutex<Option<Result<Option<String>, &'static str>>> = std::sync::Mutex::new(None);
    let one = cmd["provider"].as_str();
    let (check, ask_cli) = (cmd["op"] == "usage_sources" && one.is_some(), one.is_none_or(|p| p == QWEN));
    let last = CLI_LAST.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let cli = match last {
        Some(c) if !ask_cli => c,
        _ => {
            let c = qwencloud_signed_in();
            *CLI_LAST.lock().unwrap_or_else(|e| e.into_inner()) = Some(c.clone());
            c
        }
    };
    let (base, now) = (crate::labels::dir().ok(), crate::session::unix_now());
    let dir = base.as_ref().map(|b| b.join("credits"));
    let key_of = |l: &crate::models::Login| l.vars.iter().find_map(|v| crate::models::key(v));
    if let (true, Ok(account), Some(l)) = (ask_cli, &cli, usage_login(QWEN).ok()) {
        cached_within(dir.as_deref(), l, &key_of(l).unwrap_or_default(), now, if check { 0 } else { 60 },
            || qwencloud(&cli_json("qwencloud", &["usage", "summary", "--format", "json"], 8)?, account.as_deref()));
    }
    if let Some(l) = one.filter(|p| check && *p != QWEN).and_then(|p| usage_login(p).ok()).filter(|l| methods(l.name).iter().any(|m| matches!(*m, "api" | "api_scope"))) {
        if let Some(k) = key_of(l) {
            cached_within(dir.as_deref(), l, &k, now, 0, || reading(l, &k));
        }
    }
    sources_in(&usage_conf(usage_file().as_deref()), base.as_deref(), &|v| crate::models::key(v), &cli)
}

const QWEN: &str = "Qwen Cloud Token Plan";

/// The usage op's lines (the app server's `usage_sources`, `usage_track`, `usage_allowance`): the usage lines with the op's
/// `seq` and the window's own `req` said back (it knows which click a reply answers), and a refusal's `error`; after a
/// saved change, the credits read again with the same `seq`. `sources` and `credits` are the readers (stand-ins in tests).
pub fn usage_lines(cmd: &Value, seq: u64, saved: &Option<Result<(), String>>, sources: impl FnOnce() -> Value, credits: impl FnOnce() -> Value) -> Vec<Value> {
    let mut v = sources();
    v["seq"] = json!(seq);
    if !cmd["req"].is_null() {
        v["req"] = cmd["req"].clone();
    }
    if let Some(Err(e)) = saved {
        v["error"] = json!(e);
    }
    let mut lines = vec![v];
    if matches!(saved, Some(Ok(()))) {
        let mut c = credits();
        c["seq"] = json!(seq);
        lines.push(c);
    }
    lines
}

fn sources_in(conf: &Value, base: Option<&std::path::Path>, key: &dyn Fn(&str) -> Option<String>, cli: &Result<Option<String>, &'static str>) -> Value {
    let providers: Vec<Value> = crate::models::LOGINS.iter().filter(|l| l.name != "OmniRoute").map(|l| {
        let k = l.vars.iter().find_map(|v| key(v));
        let methods: Vec<Value> = methods(l.name).into_iter().map(|m| {
            let ((steps, link), why) = (steps(l, m), why_not(l, m, k.as_deref(), conf, base, cli));
            let mut v = json!({"id": m, "steps": steps, "link": link, "ready": why.is_none(), "why": why});
            // what to run now, and the method to use in its place where the CLI does not show the plan
            if m == "cli" {
                v["commands"] = json!(cli_commands(why.as_deref()));
                if why.as_deref() == Some(CLI_NO_PLAN) {
                    v["instead"] = json!("console");
                }
            }
            v
        }).collect();
        let (on, method) = track(l, conf, k.is_some());
        json!({"provider": l.name, "has_key": k.is_some(), "methods": methods, "track": {"on": on, "method": method}, "allowance": allowance_of(conf, l.name)})
    }).collect();
    json!({"type": "usage_sources", "providers": providers})
}

/// The allowance's current period: (start, next reset) in seconds since 1970, each at local midnight. `off` is the zone's
/// offset from UTC at an instant; midnight is placed by the offset at midnight itself (found in two steps), so the day
/// after a DST change starts at its own midnight, not an hour off.
fn period(a: &Value, now: i64, off: impl Fn(i64) -> i64) -> (i64, i64) {
    let today = (now + off(now)).div_euclid(86_400);
    let reset = a["reset"].as_i64().unwrap_or(1);
    let (from, to) = match a["period"].as_str() {
        Some("week") => {
            // 1970-01-01 was a Thursday (4)
            let back = ((today + 4).rem_euclid(7) - reset).rem_euclid(7);
            (today - back, today - back + 7)
        }
        Some("month") => {
            // the reset day, or the month's last where it is shorter
            let next = |y: i64, m: i64| if m == 12 { (y + 1, 1) } else { (y, m + 1) };
            let on = |y: i64, m: i64| {
                let (ny, nm) = next(y, m);
                days_from_civil(y, m, 1) + reset.min(days_from_civil(ny, nm, 1) - days_from_civil(y, m, 1)) - 1
            };
            let (y, m, _) = civil(today);
            let (y, m) = if today >= on(y, m) { (y, m) } else if m == 1 { (y - 1, 12) } else { (y, m - 1) };
            let (ny, nm) = next(y, m);
            (on(y, m), on(ny, nm))
        }
        _ => (today, today + 1),
    };
    let midnight = |day: i64| {
        let t = day * 86_400;
        t - off(t - off(t))
    };
    (midnight(from), midnight(to))
}

/// A run's provider and key variable (its `meta`, or an escalated run's `lane`) are `l`'s, as start_pick matches them.
fn ran_on(l: &crate::models::Login, m: &Value) -> bool {
    let (p, var) = (m["provider"].as_str().unwrap_or_default(), m["key_env"].as_str().unwrap_or_default());
    // logged as "Mistral" before Mistral Vibe was a provider (0.1.198): the Vibe key, as start_pick takes it
    let p = if var.is_empty() && p == "Mistral" { "Mistral Vibe" } else { p };
    p == l.name && (var.is_empty() || l.vars.contains(&var))
}

/// `l`'s part of a run in `unit`: the whole run on it (less the independent reviewer's cost, another model's), or the
/// cheaper lane's part of a run that went on to claude (`lane`, runlog.rs); else 0.
fn use_of(l: &crate::models::Login, r: &Value, unit: &str) -> f64 {
    let part = if ran_on(l, &r["meta"]) { r } else if ran_on(l, &r["lane"]) { &r["lane"] } else { return 0.0 };
    match unit {
        "usd" => num(&part["cost_usd"]).unwrap_or(0.0) - if std::ptr::eq(part, r) { num(&r["review"]["cost_usd"]).unwrap_or(0.0) } else { 0.0 },
        "requests" => part["turns"].as_f64().unwrap_or(0.0),
        _ => ["input", "cache_write", "output"].iter().filter_map(|k| part["tokens"][*k].as_f64()).sum(),
    }
}

/// The meters' limits (desktop byPct): 90% used is out, 75% low.
fn by_pct(used: f64) -> &'static str {
    if used >= 90.0 { "out" } else if used >= 75.0 { "low" } else { "ok" }
}

/// `l`'s row counted here: its allowance less tazuna's own use of it this period, from the run log. Tokens are what the
/// provider bills in full: input, cache writes and output; cache reads are left out (billed at a tenth or less, where at
/// all: a long session re-reads millions). usd is the run's logged cost; requests its model calls (`turns`: one per call).
/// ponytail: tazuna's own use on this machine only (not other apps, other machines, nor the explorer's calls), and a plan's
/// own way of counting (cache reads, credits) is not modelled.
fn local_row(l: &crate::models::Login, a: Option<&Value>, runs: &[Value], now: i64, off: impl Fn(i64) -> i64) -> Value {
    let Some(a) = a else {
        return row(l, Some(Reading { left: "no allowance set: Settings › Providers".into(), short: "no allowance".into(), ..Default::default() }));
    };
    let (from, to) = period(a, now, off);
    let (total, unit) = (a["amount"].as_f64().unwrap_or(1.0), a["unit"].as_str().unwrap_or("tokens"));
    let used: f64 = runs.iter().filter(|r| r["ts"].as_i64().is_some_and(|t| t >= from && t <= now)).map(|r| use_of(l, r, unit)).sum();
    let pct = used / total * 100.0;
    let left = (100.0 - pct).max(0.0);
    let of = match unit { "usd" => amount("USD", total), "requests" => format!("{} requests", grouped(total)), _ => format!("{} tokens", grouped(total)) };
    let name = match a["period"].as_str() { Some("day") => "daily", Some("week") => "weekly", _ => "monthly" };
    row(l, Some(Reading {
        left: format!("≈ {left:.0}% left of {of} (use through tazuna on this machine)"),
        short: format!("≈ {left:.0}% left"),
        windows: vec![window(name, pct, Some(iso_ms(to as f64 * 1000.0)))],
        money: (unit == "usd").then_some((total - used, "USD")),
        state: Some(by_pct(pct)),
        ..Default::default()
    }))
}

/// What the desktop's console reader last wrote for `l`: ~/.local/share/tazuna/console/<name>.json, the name as the shared
/// readings' ("Qwen-Cloud-Token-Plan.json"). None: no file; Err: not valid.
fn console_reading(base: Option<&std::path::Path>, l: &crate::models::Login, now: i64) -> Option<Result<Value, ()>> {
    use std::io::Read;
    let f = Some(base?.join("console").join(slug(l.name))).filter(|f| f.is_file())?;
    let mut text = String::new();
    let read = std::fs::File::open(f).and_then(|f| f.take(CONSOLE_MAX as u64 + 1).read_to_string(&mut text));
    Some(read.ok().and_then(|_| console_check(&text, now)).ok_or(()))
}

/// A console reading's file is at most this long.
const CONSOLE_MAX: usize = 4096;

/// A console reading checked strictly, as data from a web page: `at` (seconds, not in the future), `status`
/// ok|signed_out|error, `source` "console", and when ok `numbers`: `unit` usd|tokens|credits|percent, `left`, `total`,
/// `used_pct` each a finite number or null (total above 0, used 0 to 1000%), `resets` an ISO time or seconds or null. What
/// comes back is only these, rebuilt: nothing else of the file is shown or used.
fn console_check(text: &str, now: i64) -> Option<Value> {
    if text.len() > CONSOLE_MAX {
        return None;
    }
    let v: Value = serde_json::from_str(text).ok()?;
    let at = v["at"].as_f64().filter(|a| a.is_finite() && *a >= 1e9 && *a <= (now + 300) as f64)? as i64;
    let status = ["ok", "signed_out", "error"].into_iter().find(|s| v["status"] == *s)?;
    if v["source"] != "console" {
        return None;
    }
    if status != "ok" {
        return Some(json!({"at": at, "status": status}));
    }
    let n = &v["numbers"];
    let unit = ["usd", "tokens", "credits", "percent"].into_iter().find(|u| n["unit"] == *u)?;
    let opt = |k: &str| match &n[k] {
        Value::Null => Some(None),
        x => x.as_f64().filter(|f| f.is_finite() && f.abs() < 1e15).map(Some),
    };
    let (left, total, used) = (opt("left")?, opt("total")?, opt("used_pct")?);
    if total.is_some_and(|t| t <= 0.0) || used.is_some_and(|u| !(0.0..=1000.0).contains(&u)) {
        return None;
    }
    let secs = match &n["resets"] {
        Value::Null => None,
        Value::String(s) if s.len() <= 40 => Some(iso_secs(s)?),
        x @ Value::Number(_) => Some(x.as_f64().filter(|s| s.is_finite())? as i64),
        _ => return None,
    };
    let resets = match secs {
        Some(s) if (1_000_000_000..10_000_000_000).contains(&s) => Some(iso_ms(s as f64 * 1000.0)),
        Some(_) => return None,
        None => None,
    };
    Some(json!({"at": at, "status": status, "numbers": {"unit": unit, "left": left, "total": total, "used_pct": used, "resets": resets}}))
}

/// "4 min ago", "3 h ago", "2 d ago".
fn ago(at: i64, now: i64) -> String {
    let m = (now - at).max(0) / 60;
    if m < 60 { format!("{m} min ago") } else if m < 1440 { format!("{} h ago", m / 60) } else { format!("{} d ago", m / 1440) }
}

/// `l`'s row from its console reading (`console_reading`): "$4.20 left of $10.00 from its console", the state by the
/// meters' limits where a share used is known (else by the money, as an API's); over an hour old it is unknown, and says
/// when it was read.
fn console_row(l: &crate::models::Login, c: Option<Result<Value, ()>>, now: i64) -> Value {
    let say = |left: String, short: &str| row(l, Some(Reading { left, short: short.into(), ..Default::default() }));
    let c = match c {
        None => return say("no reading from its console yet".into(), "no reading yet"),
        Some(Err(())) => return say("its console reading is not valid".into(), "unreadable"),
        Some(Ok(c)) => c,
    };
    let at = c["at"].as_i64().unwrap_or(0);
    match c["status"].as_str() {
        Some("signed_out") => return say("sign in again in Settings".into(), "sign in again"),
        Some("error") => return say(format!("its console could not be read ({})", ago(at, now)), "console failed"),
        _ => {}
    }
    let n = &c["numbers"];
    let (left, total, unit) = (n["left"].as_f64(), n["total"].as_f64(), n["unit"].as_str().unwrap_or_default());
    let used = n["used_pct"].as_f64().or_else(|| Some((1.0 - left? / total?) * 100.0)).or_else(|| (unit == "percent").then_some(100.0 - left?));
    let text = match (unit, left, total) {
        ("usd", Some(x), _) if x < 0.0 => format!("{} below zero", amount("USD", x)),
        ("usd", Some(x), Some(t)) => format!("{} left of {}", amount("USD", x), amount("USD", t)),
        ("usd", Some(x), None) => format!("{} left", amount("USD", x)),
        ("percent", Some(x), _) => format!("{x:.0}% left"),
        (u, Some(x), Some(t)) => format!("{} of {} {u} left", grouped(x), grouped(t)),
        (u, Some(x), None) => format!("{} {u} left", grouped(x)),
        (_, None, _) => match used {
            Some(u) => format!("{u:.0}% used"),
            None => return say("its console gave no numbers".into(), "no numbers"),
        },
    };
    // over an hour old: no numbers to decide by (the window's meters and alerts read max_used_pct and the money before
    // the state), only what it said and when
    if now - at > 3600 {
        return say(format!("{text} from its console, last read {}", ago(at, now)), &format!("read {}", ago(at, now)));
    }
    let short = match (unit, left) {
        ("usd", Some(x)) => money("USD", x).short,
        _ => format!("{:.0}% used", used.unwrap_or(0.0)),
    };
    row(l, Some(Reading {
        left: format!("{text} from its console"),
        short,
        windows: used.map(|u| vec![window("usage", u, n["resets"].as_str().map(String::from))]).unwrap_or_default(),
        money: left.filter(|_| unit == "usd").map(|x| (x, "USD")),
        state: used.map(by_pct),
        ..Default::default()
    }))
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
    Some(days_from_civil(y, m, d) * 86_400 + hh * 3600 + mm * 60 + ss)
}

/// Days since 1970-01-01 of a civil date (Howard Hinnant's days_from_civil).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468
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
    let untracked: Vec<&str> = v["untracked"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    let col = link_from(v) - 2;
    if !credits.is_empty() || !untracked.is_empty() {
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
        if !untracked.is_empty() {
            push(format!("  not tracked: {} (Settings, or /usage track <provider>)", untracked.join(", ")), Dim, None);
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
        // CLI 1.7.0's own answers (2026-11-27), for an account whose Individual plan the console shows active: a quiet row
        // (no value, no warning, nothing claimed), marked for the usage line; the account as `auth status` masks it
        let real = json!({"period": {"from": "2026-11-01", "to": "2026-11-27"}, "free_tier": [], "coding_plan": {"subscribed": false},
            "token_plan": {"subscribed": false, "planName": "Token Plan", "totalCredits": 0, "remainingCredits": 0, "usedPct": 0}, "pay_as_you_go": {}});
        let none = row(login("Qwen Cloud Token Plan"), qwencloud(&real, Some("someone*****@example.com")));
        assert_eq!((&none["left"], &none["short"], none["state"].as_str(), &none["max_used_pct"], &none["cli_plan"]), (&Value::Null, &Value::Null, Some("unknown"), &Value::Null, &json!(false)));
        let status = json!({"authenticated": true, "server_verified": true, "auth_mode": "device_flow", "source": "keychain", "user": {"aliyunId": "someone*****@example.com"}});
        assert_eq!(cli_account(&status).as_deref(), Some("someone*****@example.com"));
        assert_eq!((cli_account(&json!({"email": "a@b.c"})).as_deref(), cli_account(&json!({"user": {"email": "x@y.z"}})).as_deref(), cli_account(&json!({}))), (Some("a@b.c"), Some("x@y.z"), None));
        assert!(qwencloud(&json!({"pay_as_you_go": {}}), None).is_none(), "no Token Plan said either way: no reading");
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

    fn login(name: &str) -> &'static crate::models::Login {
        crate::models::LOGINS.iter().find(|l| l.name == name).unwrap()
    }

    #[test]
    fn each_provider_has_its_ways_to_read_usage_and_the_choices_are_checked() {
        for l in crate::models::LOGINS.iter().filter(|l| l.name != "OmniRoute") {
            let ms = methods(l.name);
            assert!(ms.contains(&"local"), "any provider can be counted here: {}", l.name);
            for m in &ms {
                let (steps, link) = steps(l, m);
                assert!(!steps.is_empty() && link.starts_with("http"), "{} {m}: {link}", l.name);
            }
        }
        assert!(API.iter().all(|n| crate::models::LOGINS.iter().any(|l| l.name == *n)), "each reader's provider is in LOGINS by that name");
        assert_eq!(methods("Qwen Cloud Token Plan"), ["cli", "console", "local"]);
        assert_eq!(methods("Mistral"), ["admin_key", "local", "none"], "an Admin key reads nothing: no way to read it but here");
        assert_eq!(methods("Mistral Vibe"), ["admin_key", "console", "local"]);
        assert_eq!(methods("Cheaper Inference"), ["api_scope", "local"]);
        assert_eq!((methods("SubRouter"), methods("Groq")), (vec!["api", "local"], vec!["local", "none"]));
        assert!(steps(login("Qwen Cloud Token Plan"), "cli").0.contains("npm install -g @qwencloud/qwencloud-cli"));
        // the defaults: read by its endpoint where it has a key, as before tracking; the rest off
        let none = json!({});
        assert_eq!([track(login("SubRouter"), &none, true), track(login("SubRouter"), &none, false), track(login("Cheaper Inference"), &none, true),
            track(login("Qwen Cloud Token Plan"), &none, true), track(login("Caffeine"), &none, true), track(login("Groq"), &none, true)],
            [(true, "api"), (false, "api"), (true, "api_scope"), (true, "cli"), (false, "console"), (false, "local")]);
        assert_eq!((track(login("Qwen Cloud Token Plan"), &none, false), track(login("Mistral"), &none, true), track(login("Mistral Vibe"), &none, true)),
            ((false, "cli"), (false, "local"), (false, "console")), "Qwen's CLI read with a key, as before tracking; never admin_key by default");
        // usage_track: a known provider, one of its methods, on a boolean; none is never on
        let mut c = json!({});
        let t = |c: &mut Value, p: &str, on: Value, m: &str| track_edit(c, &json!({"provider": p, "on": on, "method": m}));
        assert!(t(&mut c, "Nobody", json!(true), "local").is_err());
        assert!(t(&mut c, "OmniRoute", json!(true), "local").is_err(), "its accounts have their own section");
        assert!(t(&mut c, "Caffeine", json!(true), "api").is_err(), "Caffeine has no API reader");
        assert!(t(&mut c, "Groq", json!(true), "none").is_err());
        assert!(t(&mut c, "Mistral", json!(true), "admin_key").is_err(), "an Admin key is read by nothing");
        c["track"]["Mistral"] = json!({"on": true, "method": "admin_key"});
        assert_eq!(track(login("Mistral"), &c, true), (false, "admin_key"), "a hand edit cannot turn it on");
        assert!(t(&mut c, "Groq", json!("yes"), "local").is_err());
        t(&mut c, "Caffeine", json!(true), "console").unwrap();
        t(&mut c, "SubRouter", json!(false), "api").unwrap();
        assert_eq!((track(login("Caffeine"), &c, true), track(login("SubRouter"), &c, true)), ((true, "console"), (false, "api")));
        c["track"]["Groq"] = json!({"on": true, "method": "cli"});
        assert_eq!(track(login("Groq"), &c, true), (false, "local"), "a method it does not have (a hand edit): the default");
        // usage_allowance: bounds, units, periods, resets
        let a = |c: &mut Value, al: Value| allowance_edit(c, &json!({"provider": "Caffeine", "allowance": al}));
        for bad in [json!({"amount": 0, "unit": "usd", "period": "month", "reset": 1}), json!({"amount": -5, "unit": "usd", "period": "month", "reset": 1}),
            json!({"amount": "10", "unit": "usd", "period": "month", "reset": 1}), json!({"amount": 10, "unit": "eur", "period": "month", "reset": 1}),
            json!({"amount": 10, "unit": "usd", "period": "year", "reset": 1}), json!({"amount": 10, "unit": "usd", "period": "month", "reset": 32}),
            json!({"amount": 10, "unit": "usd", "period": "month", "reset": 0}), json!({"amount": 10, "unit": "usd", "period": "week", "reset": 7}),
            json!({"amount": 10, "unit": "usd", "period": "week"}), json!({"amount": 1e16, "unit": "tokens", "period": "day"}), json!("ten dollars")] {
            assert!(a(&mut c, bad.clone()).is_err(), "{bad}");
        }
        a(&mut c, json!({"amount": 2_000_000, "unit": "tokens", "period": "day", "reset": 9})).unwrap();
        assert_eq!(allowance_of(&c, "Caffeine"), Some(json!({"amount": 2_000_000.0, "unit": "tokens", "period": "day", "reset": null})), "a day ignores its reset");
        a(&mut c, json!({"amount": 10, "unit": "usd", "period": "week", "reset": 0})).unwrap();
        assert_eq!(allowance_of(&c, "Caffeine").unwrap()["reset"], 0);
        a(&mut c, Value::Null).unwrap();
        assert_eq!(allowance_of(&c, "Caffeine"), None, "null: none");
        // the three ops: saved whole; a refused choice leaves the file as it was; each answers with the usage lines
        let dir = std::env::temp_dir().join(format!("tazuna-usage-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let f = dir.join("usage.json");
        let op = |cmd: Value| usage_save_in(&cmd, Some(&f));
        assert!(op(json!({"op": "usage_sources"})).is_none(), "nothing to save");
        op(json!({"op": "usage_track", "provider": "Caffeine", "on": true, "method": "local"})).unwrap().unwrap();
        assert!(op(json!({"op": "usage_track", "provider": "Caffeine", "on": true, "method": "api"})).unwrap().is_err());
        op(json!({"op": "usage_allowance", "provider": "Caffeine", "allowance": {"amount": 5, "unit": "usd", "period": "month", "reset": 31}})).unwrap().unwrap();
        assert!(op(json!({"op": "usage_allowance", "provider": "Caffeine", "allowance": {"amount": 5, "unit": "usd", "period": "year"}})).unwrap().is_err());
        assert_eq!(track(login("Caffeine"), &usage_conf(Some(&f)), true), (true, "local"));
        let keys = |v: &str| (v == "CAFFEINE_API_KEY").then(|| "k".to_string());
        let src = sources_in(&usage_conf(Some(&f)), Some(&dir), &keys, &Err(CLI_MISSING));
        let caf = src["providers"].as_array().unwrap().iter().find(|p| p["provider"] == "Caffeine").unwrap().clone();
        assert_eq!((&caf["has_key"], &caf["track"], &caf["allowance"]["reset"]), (&json!(true), &json!({"on": true, "method": "local"}), &json!(31)));
        assert_eq!(caf["methods"].as_array().unwrap().iter().map(|m| (m["id"].as_str().unwrap(), m["ready"].as_bool().unwrap())).collect::<Vec<_>>(), [("console", false), ("local", true)]);
        assert!(!src["providers"].as_array().unwrap().iter().any(|p| p["provider"] == "OmniRoute"));
        // odd shapes: a track or allowance that is not an object starts empty, never a crash
        std::fs::write(&f, r#"{"track": ["SubRouter"], "allowance": 3, "other": true}"#).unwrap();
        op(json!({"op": "usage_track", "provider": "Groq", "on": true, "method": "local"})).unwrap().unwrap();
        op(json!({"op": "usage_allowance", "provider": "Groq", "allowance": null})).unwrap().unwrap();
        let c2 = usage_conf(Some(&f));
        assert_eq!((&c2["track"], &c2["allowance"], &c2["other"]), (&json!({"Groq": {"on": true, "method": "local"}}), &json!({}), &json!(true)));
        // a file that is not JSON (the user's allowances in it) is never written over
        std::fs::write(&f, "{\"allowance\": {\"Groq\": ").unwrap();
        assert!(op(json!({"op": "usage_track", "provider": "Groq", "on": false, "method": "local"})).unwrap().unwrap_err().contains("not valid JSON"));
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "{\"allowance\": {\"Groq\": ");
        std::fs::write(&f, "[1]").unwrap();
        assert!(op(json!({"op": "usage_track", "provider": "Groq", "on": false, "method": "local"})).unwrap().is_err(), "not an object either");
        // another writer's lock: waited for, then refused; a crashed writer's (old) lock is taken over
        std::fs::write(&f, "{}").unwrap();
        let lock = f.with_extension("json.lock");
        std::fs::write(&lock, "").unwrap();
        assert!(op(json!({"op": "usage_track", "provider": "Groq", "on": true, "method": "local"})).unwrap().unwrap_err().contains("another tazuna"));
        std::fs::File::options().write(true).open(&lock).unwrap().set_modified(std::time::SystemTime::now() - Duration::from_secs(60)).unwrap();
        op(json!({"op": "usage_track", "provider": "Caffeine", "on": true, "method": "local"})).unwrap().unwrap();
        assert!(!lock.exists(), "the lock goes with the save");
        op(json!({"op": "usage_allowance", "provider": "Caffeine", "allowance": {"amount": 5, "unit": "usd", "period": "month", "reset": 31}})).unwrap().unwrap();
        // /usage track: by name in any case, its first method that reads now; a provider with no reader is counted here
        assert_eq!(track_by_name_in("caffeine", Some(&f), Some(&dir), &keys, &Err(CLI_MISSING)).unwrap(), "usage: Caffeine tracked (local)", "an allowance is set: local reads now");
        assert_eq!(track_by_name_in("Mistral", Some(&f), Some(&dir), &keys, &Err(CLI_MISSING)).unwrap(), "usage: Mistral tracked (local)", "never admin_key");
        assert_eq!(track_by_name_in("SubRouter", Some(&f), Some(&dir), &keys, &Err(CLI_MISSING)).unwrap(), "usage: SubRouter tracked (api), once it has a key");
        assert_eq!(track_by_name_in("Qwen Cloud Token Plan", Some(&f), Some(&dir), &keys, &Ok(None)).unwrap(), "usage: Qwen Cloud Token Plan tracked (cli), once it has a key");
        assert!(track_by_name_in("OmniRoute", Some(&f), Some(&dir), &keys, &Err(CLI_MISSING)).is_err() && track_by_name_in("nobody", Some(&f), Some(&dir), &keys, &Err(CLI_MISSING)).is_err());
        assert_eq!(track(login("Mistral"), &usage_conf(Some(&f)), false), (true, "local"));
        // the rows by method: console and local need no key; api by the shared reading (a stand-in reader here); untracked
        let conf = usage_conf(Some(&f));
        let mut both = conf.clone();
        both["track"]["Mortera"] = json!({"on": true, "method": "api"});
        both["track"]["Mistral Vibe"] = json!({"on": true, "method": "console"});
        both["track"]["SubRouter"] = json!({"on": false, "method": "api"});
        both["track"]["Qwen Cloud Token Plan"] = json!({"on": false, "method": "cli"});
        let tracked = tracked_in(&both, None, &keys);
        let names: Vec<(&str, &str)> = tracked.iter().map(|t| (t.0.name, t.2)).collect();
        assert_eq!(names, [("Mistral", "local"), ("Mistral Vibe", "console"), ("Caffeine", "local")], "Mortera has no key here: not read");
        // a console never visited asks for the sign-in; once visited (its learn file), it waits for its reading
        let rows = rows_of(&tracked, &both, Some(&dir), crate::session::unix_now(), |_, _| None);
        assert_eq!(rows.iter().map(|r| r["left"].as_str().unwrap_or("-")).collect::<Vec<_>>(),
            ["no allowance set: Settings › Providers", "sign in to its console in Settings", "≈ 100% left of $5.00 (use through tazuna on this machine)"]);
        assert_eq!(rows[1]["short"], "sign in from Settings");
        std::fs::create_dir_all(dir.join("console-learn")).unwrap();
        std::fs::write(dir.join("console-learn/Mistral-Vibe.json"), "{}").unwrap();
        let rows = rows_of(&tracked, &both, Some(&dir), crate::session::unix_now(), |_, _| None);
        assert_eq!(rows[1]["left"], "no reading from its console yet");
        let api = rows_of(&[(login("Mortera"), "k".into(), "api"), (login("Mistral"), "k".into(), "admin_key")], &both, Some(&dir), crate::session::unix_now(),
            |_, _| Some(money("IDR", 50_000.0)));
        assert_eq!((api[0]["left"].as_str(), api[1]["left"].as_str(), api[1]["page"].as_str()), (Some("Rp 50,000 left"), None, Some(login("Mistral").page)), "admin_key: not read, its page");
        assert_eq!(untracked_in(&both, &keys), Vec::<&str>::new(), "Caffeine, the one keyed, is tracked");
        assert_eq!(untracked_in(&json!({}), &keys), ["Caffeine"]);
        // why a method cannot read now
        let conf = usage_conf(Some(&f));
        let cli: Result<Option<String>, &str> = Err(CLI_MISSING);
        let why = |name: &str, m: &str, key: Option<&str>| why_not(login(name), m, key, &conf, Some(&dir), &cli);
        assert_eq!((why("SubRouter", "api", None).as_deref(), why("SubRouter", "api", Some("k"))), (Some("No key saved"), None));
        assert_eq!(why("Qwen Cloud Token Plan", "cli", Some("k")).as_deref(), Some(CLI_MISSING));
        assert_eq!(why("Groq", "local", Some("k")).as_deref(), Some("No allowance set"));
        // no reading yet is never "not signed in": the first visit may have been made (the user: Done went back to it)
        assert_eq!(why("Caffeine", "console", Some("k")).as_deref(), Some("No reading yet"));
        std::fs::create_dir_all(dir.join("console")).unwrap();
        let now = crate::session::unix_now() as i64;
        std::fs::write(dir.join("console/Caffeine.json"), json!({"at": now, "status": "signed_out", "source": "console"}).to_string()).unwrap();
        assert_eq!(why("Caffeine", "console", Some("k")).as_deref(), Some("Signed out"));
        std::fs::write(dir.join("console/Caffeine.json"), json!({"at": now, "status": "ok", "source": "console", "numbers": {"unit": "usd", "left": 3, "total": 10, "used_pct": null, "resets": null}}).to_string()).unwrap();
        assert_eq!(why("Caffeine", "console", Some("k")), None);
        assert!(why("Mistral", "admin_key", Some("k")).is_some() && why("Groq", "none", Some("k")).is_some());
        // Qwen's CLI: missing, signed out, signed in; signed in, its last reading without the plan (the console in its
        // place); and what each says to run
        let qwen = login("Qwen Cloud Token Plan");
        let cli_why = |cli: &Result<Option<String>, &'static str>| why_not(qwen, "cli", Some("k"), &conf, Some(&dir), cli);
        let me: Result<Option<String>, &'static str> = Ok(Some("someone*****@example.com".into()));
        assert_eq!((cli_why(&Err(CLI_SIGNED_OUT)).as_deref(), cli_why(&me)), (Some("Not signed in"), None));
        let t = crate::session::unix_now();
        let real = json!({"coding_plan": {"subscribed": false}, "token_plan": {"subscribed": false, "planName": "Token Plan", "totalCredits": 0, "remainingCredits": 0, "usedPct": 0}});
        cached(Some(&dir.join("credits")), qwen, "k", t, || qwencloud(&real, Some("someone*****@example.com")));
        assert_eq!((cli_why(&me).as_deref(), cli_why(&Ok(None)).as_deref()), (Some(CLI_NO_PLAN), Some(CLI_NO_PLAN)));
        assert_eq!([cli_commands(None), cli_commands(Some(CLI_MISSING)), cli_commands(Some(CLI_SIGNED_OUT)), cli_commands(Some(CLI_NO_PLAN))],
            [vec![], vec!["npm install -g @qwencloud/qwencloud-cli", "qwencloud auth login"], vec!["qwencloud auth login"], vec![]]);
        let src = sources_in(&conf, Some(&dir), &|v: &str| (v == "QWENCLOUD_TOKEN_PLAN_API_KEY").then(|| "k".to_string()), &me);
        let m = src["providers"].as_array().unwrap().iter().find(|p| p["provider"] == "Qwen Cloud Token Plan").unwrap()["methods"][0].clone();
        assert_eq!((&m["id"], &m["ready"], &m["why"], &m["commands"], &m["instead"]), (&json!("cli"), &json!(false), &json!(CLI_NO_PLAN), &json!([]), &json!("console")), "{m}");
        // a CLI that times out next keeps the quiet row's mark: never a false "Signed in"
        cached(Some(&dir.join("credits")), qwen, "k", t + 61, || None);
        assert_eq!(cli_why(&me).as_deref(), Some(CLI_NO_PLAN), "a failed read lost the mark");
        // Check again reads now (cached_within 0), not the shared reading of the last minute
        assert_eq!(cached_within(Some(&dir.join("credits")), qwen, "k", t + 62, 60, || panic!("read within the minute")).get("cli_plan"), Some(&json!(false)));
        // the plan bought: reads again
        cached_within(Some(&dir.join("credits")), qwen, "k", t + 63, 0, || qwencloud(&json!({"token_plan": {"subscribed": true, "usedPct": 30}}), None));
        assert_eq!(cli_why(&me), None);
        // the op's lines: its seq, the window's req said back, a refusal's error, the credits only after a saved change
        let src = || json!({"type": "usage_sources", "providers": []});
        let cr = || json!({"type": "accounts_credits", "credits": []});
        let lines = usage_lines(&json!({"op": "usage_track", "provider": "Groq", "req": 7}), 3, &Some(Ok(())), src, cr);
        assert_eq!((lines.len(), &lines[0]["seq"], &lines[0]["req"], &lines[1]["seq"], lines[1].get("req")), (2, &json!(3), &json!(7), &json!(3), None));
        let refused = usage_lines(&json!({"op": "usage_track", "req": 8}), 4, &Some(Err("not saved".into())), src, || panic!("no credits for a refusal"));
        assert_eq!((refused.len(), &refused[0]["req"], &refused[0]["error"]), (1, &json!(8), &json!("not saved")));
        let older = usage_lines(&json!({"op": "usage_sources"}), 5, &None, src, || panic!("nothing saved"));
        assert_eq!((older.len(), older[0].get("req")), (1, None), "no req asked: none said back");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_allowance_period_starts_at_local_midnight_across_dst() {
        let t = |iso: &str| iso_secs(iso).unwrap();
        // Berlin: CET +1, CEST +2 from 2026-03-29 01:00 UTC to 2026-10-25 01:00 UTC
        let (spring, autumn) = (t("2026-03-29T01:00:00Z"), t("2026-10-25T01:00:00Z"));
        let berlin = |at: i64| if (spring..autumn).contains(&at) { 7200 } else { 3600 };
        let a = |period: &str, reset: Value| json!({"amount": 1, "unit": "tokens", "period": period, "reset": reset});
        assert_eq!(period(&a("day", Value::Null), t("2026-10-25T12:00:00Z"), berlin), (t("2026-10-24T22:00:00Z"), t("2026-10-25T23:00:00Z")), "the autumn day has 25 hours");
        assert_eq!(period(&a("day", Value::Null), t("2026-03-29T12:00:00Z"), berlin), (t("2026-03-28T23:00:00Z"), t("2026-03-29T22:00:00Z")), "the spring day has 23");
        assert_eq!(period(&a("month", json!(25)), t("2026-10-25T12:00:00Z"), berlin), (t("2026-10-24T22:00:00Z"), t("2026-11-24T23:00:00Z")), "each end at its own offset");
        assert_eq!(period(&a("month", json!(1)), t("2026-11-30T23:30:00Z"), berlin).0, t("2026-11-30T23:00:00Z"), "already December 1 in Berlin");
        assert_eq!(period(&a("month", json!(10)), t("2026-01-05T12:00:00Z"), berlin), (t("2025-12-09T23:00:00Z"), t("2026-01-09T23:00:00Z")), "across the year");
        assert_eq!(period(&a("month", json!(28)), t("2026-11-26T12:00:00Z"), |_| 0), (t("2026-10-28T00:00:00Z"), t("2026-11-28T00:00:00Z")));
        // Thursday 2026-11-26; a week from Monday (1), or from Thursday (4): today
        assert_eq!(period(&a("week", json!(1)), t("2026-11-26T10:00:00Z"), berlin), (t("2026-11-22T23:00:00Z"), t("2026-11-29T23:00:00Z")));
        assert_eq!(period(&a("week", json!(4)), t("2026-11-26T10:00:00Z"), |_| 0).0, t("2026-11-26T00:00:00Z"));
        // a reset on the 31st: a shorter month resets on its last day (February 28, April 30)
        assert_eq!(period(&a("month", json!(31)), t("2026-02-15T12:00:00Z"), |_| 0), (t("2026-01-31T00:00:00Z"), t("2026-02-28T00:00:00Z")));
        assert_eq!(period(&a("month", json!(31)), t("2026-02-28T12:00:00Z"), |_| 0), (t("2026-02-28T00:00:00Z"), t("2026-03-31T00:00:00Z")));
        assert_eq!(period(&a("month", json!(30)), t("2028-02-29T12:00:00Z"), |_| 0).0, t("2028-02-29T00:00:00Z"), "a leap year's February");
    }

    #[test]
    fn local_counting_takes_the_providers_runs_this_period() {
        let now = iso_secs("2026-11-26T12:00:00Z").unwrap();
        let run = |ts: i64, provider: &str, var: Option<&str>, cost: f64, turns: u64| json!({"ts": ts, "meta": {"provider": provider, "key_env": var},
            "cost_usd": cost, "turns": turns, "tokens": {"input": 1000, "cache_write": 500, "cache_read": 1_000_000, "output": 500}});
        let runs = vec![
            run(now - 3600, "Mistral Vibe", Some("MISTRAL_VIBE_API_KEY"), 1.5, 10),
            run(now - 7200, "Mistral", None, 2.5, 20),  // logged before Mistral Vibe was a provider: the Vibe key
            run(now - 7200, "Mistral", Some("MISTRAL_API_KEY"), 9.0, 99),  // the API key's: not Vibe's
            run(now - 40 * 86_400, "Mistral Vibe", Some("MISTRAL_VIBE_API_KEY"), 9.0, 99),  // last month
            run(now + 60, "Mistral Vibe", Some("MISTRAL_VIBE_API_KEY"), 9.0, 99),  // a clock ahead: not yet
            run(now - 60, "SubRouter", Some("SUBROUTER_API_KEY"), 9.0, 99),
        ];
        let mut runs = runs;
        // the reviewer's cost (another model) is left out of the run's provider's dollars
        runs[0]["review"] = json!({"ok": true, "cost_usd": 0.5});
        runs[0]["cost_usd"] = json!(2.0);
        // a run the Vibe lane began and claude finished: the lane's part counts, claude's does not
        runs.push(json!({"ts": now - 60, "meta": {"provider": null, "key_env": null}, "cost_usd": 7.0, "turns": 50,
            "tokens": {"input": 9999, "cache_write": 0, "cache_read": 0, "output": 9999},
            "lane": {"provider": "Mistral Vibe", "key_env": "MISTRAL_VIBE_API_KEY", "turns": 0, "cost_usd": 0.0, "tokens": {"input": 0, "cache_write": 0, "cache_read": 0, "output": 0}}}));
        let vibe = login("Mistral Vibe");
        let al = |amount: f64, unit: &str| json!({"amount": amount, "unit": unit, "period": "month", "reset": 1});
        let r = local_row(vibe, Some(&al(10.0, "usd")), &runs, now, |_| 0);
        assert_eq!((r["left"].as_str(), r["short"].as_str(), r["usd_left"].as_f64(), r["max_used_pct"].as_f64(), r["state"].as_str()),
            (Some("≈ 60% left of $10.00 (use through tazuna on this machine)"), Some("≈ 60% left"), Some(6.0), Some(40.0), Some("ok")));
        let mut lane = runs.clone();
        lane.last_mut().unwrap()["lane"]["cost_usd"] = json!(1.0);
        lane.last_mut().unwrap()["lane"]["turns"] = json!(5);
        assert_eq!(local_row(vibe, Some(&al(10.0, "usd")), &lane, now, |_| 0)["usd_left"].as_f64(), Some(5.0), "the lane's $1.00, not the run's $7.00");
        assert_eq!(local_row(vibe, Some(&al(35.0, "requests")), &lane, now, |_| 0)["max_used_pct"].as_f64(), Some(100.0), "and its 5 calls");
        assert_eq!(r["windows"][0]["resets"], "2026-12-01T00:00:00Z");
        let r = local_row(vibe, Some(&al(5000.0, "tokens")), &runs, now, |_| 0);
        assert_eq!((r["max_used_pct"].as_f64(), r["state"].as_str(), r["usd_left"].as_f64()), (Some(80.0), Some("low"), None), "input, cache writes, output: 4,000; cache reads left out");
        assert_eq!(r["left"], "≈ 20% left of 5,000 tokens (use through tazuna on this machine)");
        let r = local_row(vibe, Some(&al(30.0, "requests")), &runs, now, |_| 0);
        assert_eq!((r["max_used_pct"].as_f64(), r["state"].as_str()), (Some(100.0), Some("out")), "30 model calls of 30");
        assert_eq!(local_row(vibe, Some(&al(25.0, "requests")), &runs, now, |_| 0)["left"], "≈ 0% left of 25 requests (use through tazuna on this machine)", "past it: none left, not less");
        let none = local_row(vibe, None, &runs, now, |_| 0);
        assert_eq!((none["left"].as_str(), none["state"].as_str()), (Some("no allowance set: Settings › Providers"), Some("unknown")));
    }

    #[test]
    fn a_console_reading_is_checked_strictly_and_ages() {
        let now = 1_795_000_000;
        let file = |at: i64, status: &str, numbers: Value| json!({"at": at, "status": status, "numbers": numbers, "source": "console"}).to_string();
        let usd = json!({"left": 4.2, "total": 10, "unit": "usd", "used_pct": null, "resets": "2026-12-01T00:00:00Z"});
        let c = console_check(&file(now - 60, "ok", usd.clone()), now).unwrap();
        assert_eq!(c["numbers"]["resets"], "2026-12-01T00:00:00Z");
        let l = login("Caffeine");
        let r = console_row(l, Some(Ok(c)), now);
        assert_eq!((r["left"].as_str(), r["short"].as_str(), r["usd_left"].as_f64(), r["state"].as_str()), (Some("$4.20 left of $10.00 from its console"), Some("$4.20 left"), Some(4.2), Some("ok")));
        assert_eq!(r["max_used_pct"].as_f64().map(f64::round), Some(58.0));
        let stale = console_row(l, Some(Ok(console_check(&file(now - 7200, "ok", usd.clone()), now).unwrap())), now);
        assert_eq!((stale["left"].as_str(), stale["short"].as_str(), stale["state"].as_str()), (Some("$4.20 left of $10.00 from its console, last read 2 h ago"), Some("read 2 h ago"), Some("unknown")));
        let red = console_row(l, Some(Ok(console_check(&file(now - 7200, "ok", json!({"left": null, "total": null, "unit": "percent", "used_pct": 95, "resets": null})), now).unwrap())), now);
        assert_eq!((&red["max_used_pct"], &red["usd_left"], red.get("windows"), red["state"].as_str()), (&Value::Null, &Value::Null, None, Some("unknown")), "a stale 95% is not red: {red}");
        let below = console_row(l, Some(Ok(console_check(&file(now, "ok", json!({"left": -4.2, "total": 10, "unit": "usd", "used_pct": null, "resets": null})), now).unwrap())), now);
        assert_eq!((below["left"].as_str(), below["short"].as_str(), below["state"].as_str()), (Some("$4.20 below zero from its console"), Some("$4.20 below zero"), Some("out")));
        let pct = console_check(&file(now, "ok", json!({"left": null, "total": null, "unit": "percent", "used_pct": 92, "resets": 1_796_000_000})), now).unwrap();
        assert_eq!(pct["numbers"]["resets"], iso_ms(1_796_000_000_000.0), "seconds as an ISO time");
        let r = console_row(l, Some(Ok(pct)), now);
        assert_eq!((r["left"].as_str(), r["state"].as_str()), (Some("92% used from its console"), Some("out")));
        let credits = console_check(&file(now, "ok", json!({"left": 1500, "total": 2000, "unit": "credits", "used_pct": null, "resets": null})), now).unwrap();
        assert_eq!(console_row(l, Some(Ok(credits)), now)["left"], "1,500 of 2,000 credits left from its console");
        let out = console_row(l, Some(Ok(console_check(&file(now, "signed_out", Value::Null), now).unwrap())), now);
        assert_eq!((out["left"].as_str(), out["state"].as_str()), (Some("sign in again in Settings"), Some("unknown")));
        assert_eq!(console_row(l, Some(Ok(console_check(&file(now - 300, "error", Value::Null), now).unwrap())), now)["left"], "its console could not be read (5 min ago)");
        assert_eq!(console_row(l, None, now)["left"], "no reading from its console yet");
        assert_eq!(console_row(l, Some(Err(())), now)["left"], "its console reading is not valid");
        // not valid: a wrong type, a number that is not finite, the future, another source, an unknown unit, a total of 0,
        // a reset that is no time, too long a file
        let n = |k: &str, v: Value| { let mut x = usd.clone(); x[k] = v; x };
        for bad in [file(now, "ok", n("left", json!("4.2"))), file(now, "ok", n("total", json!(0))), file(now, "ok", n("unit", json!("eur"))),
            file(now, "ok", n("used_pct", json!(-1))), file(now, "ok", n("resets", json!("soon"))), file(now, "ok", n("resets", json!(12))),
            file(now, "ok", n("resets", json!({"at": 1}))), file(now + 3600, "ok", usd.clone()), file(now, "fine", usd.clone()), file(now, "ok", Value::Null),
            json!({"at": now, "status": "ok", "numbers": usd, "source": "page"}).to_string(), json!({"at": now * 1000, "status": "ok", "numbers": usd, "source": "console"}).to_string(),
            "{\"at\": 1795000000, \"status\": \"ok\", \"source\": \"console\", \"numbers\": {\"unit\": \"usd\", \"left\": NaN}}".to_string(), "not json".into(),
            format!("{}{}", file(now, "ok", usd.clone()), " ".repeat(CONSOLE_MAX))] {
            assert!(console_check(&bad, now).is_none(), "{}", &bad[..bad.len().min(200)]);
        }
        // the file under the shared readings' name only
        let dir = std::env::temp_dir().join(format!("tazuna-console-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("console")).unwrap();
        std::fs::write(dir.join("console/Qwen Cloud Token Plan.json"), file(now, "ok", usd.clone())).unwrap();
        assert!(console_reading(Some(&dir), login("Qwen Cloud Token Plan"), now).is_none());
        std::fs::write(dir.join("console/Qwen-Cloud-Token-Plan.json"), file(now, "ok", usd.clone())).unwrap();
        assert!(console_reading(Some(&dir), login("Qwen Cloud Token Plan"), now).is_some_and(|r| r.is_ok()));
        std::fs::write(dir.join("console/Qwen-Cloud-Token-Plan.json"), "{}").unwrap();
        assert_eq!(console_reading(Some(&dir), login("Qwen Cloud Token Plan"), now), Some(Err(())));
        let _ = std::fs::remove_dir_all(&dir);
        // /usage: the untracked keyed providers in one line
        let shown = lines(&json!({"credits": [], "untracked": ["Caffeine", "Mistral Vibe"]}));
        assert!(shown.iter().any(|l| l.0 == "  not tracked: Caffeine, Mistral Vibe (Settings, or /usage track <provider>)"), "{shown:?}");
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
