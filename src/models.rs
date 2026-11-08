//! The model picker's list (`/model`): Claude's models through the `claude` CLI, and the own loop's routes that
//! bench/routes.py checked (~/.local/share/tazuna/routes.json: providers, price, cache share, the effort levels that differ
//! (every level can be picked), when checked), then every other SubRouter model with a public score, unchecked (SubRouter picks its
//! provider). Each row carries our bench's judge where we have runs and Artificial Analysis' intelligence index. A route is
//! ready when its key is in the environment and its host on the trusted list; otherwise it is listed, greyed out or
//! marked, with the reason.

use serde_json::Value;
use std::path::Path;

pub const LEVELS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

/// The effort levels a Claude model takes and its default (Anthropic's effort docs, 2026-11-02: Opus 5.5, Sonnet 5.5,
/// Haiku 5.5 and Fable 5.1 take all five; Opus 5.5 and Haiku 5.5 default to medium, the others to high). Claude Code
/// names them by family ("haiku", "sonnet") as well. The own loop's models list theirs per route in routes.json: the
/// levels that differ (DeepSeek V4.1 Flash: high and max; Kimi: none).
pub fn claude_levels(model: &str) -> (&'static [&'static str], &'static str) {
    let m = model.trim_start_matches("claude-");
    let default = if m.starts_with("opus") || m.starts_with("haiku") { "medium" } else { "high" };
    (&LEVELS, default)
}
/// Whether the picker lists this Claude model (a newer one runs too, but without its effort levels).
pub fn known_claude(model: &str) -> bool {
    CLAUDE.iter().any(|(id, _)| model.starts_with(id))
}

const CLAUDE: [(&str, &str); 4] =
    [("claude-opus-5-5", "Opus 5.5"), ("claude-sonnet-5-5", "Sonnet 5.5"), ("claude-haiku-5-5", "Haiku 5.5"), ("claude-fable-5-1", "Fable 5.1")];
/// A route checked longer ago than this is marked stale: provider prices move by the day (Sol's cheapest provider changed
/// within days, 2026-11-01).
const STALE_DAYS: u64 = 7;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum State {
    Ready,
    /// The route's key variable is not set.
    NoKey,
    /// The provider is not on the trusted list: the code would go there.
    Untrusted,
    /// Usable, but no route was checked: SubRouter picks the provider (cache, price and quality unknown).
    Unchecked,
    /// The key is set, but SubRouter does not let it use this model (HTTP 403).
    NotOnKey,
}

/// The models a key may use, by its variable (SubRouter's /v1/models for that key); a key not in here is not checked.
pub type KeyModels = std::collections::HashMap<String, std::collections::HashSet<String>>;

#[derive(Clone, Debug)]
pub struct Choice {
    pub model: String,
    pub label: String,
    /// None: claude on the subscription. Some: the own loop on (provider URL, key variable, price "in,cached,out" per M).
    pub route: Option<(String, String, String)>,
    pub place: String,
    pub efforts: Vec<String>,
    pub price: String,
    pub note: String,
    pub stale: bool,
    pub state: State,
    /// Our bench's judge and its number of runs ("6.8 (308)"), Artificial Analysis' intelligence index ("32"); empty where
    /// unknown.
    pub bench: String,
    pub aa: String,
    /// The route's checked providers, each with a key of its own (bench/routes.py pin): one can be pinned in the picker.
    pub pins: Vec<Pin>,
}

#[derive(Clone, Debug)]
pub struct Pin {
    pub provider: String,
    pub key_env: String,
    /// Input and output, USD per M tokens.
    pub price: (f64, f64),
    pub state: State,
}

/// The cached-input price is not in the provider list: a tenth of input, the usual discount.
/// TAZUNA_PRICE for a route: input, cached, output per million. Cached input is a tenth of input unless the route names
/// its own: Mistral bills DeepSeek 1017's at $0.007 against $0.30 (2.3%), GLM 5.3 Flash's at a fifth (2026-11-08).
fn route_price(input: f64, output: f64, cached: Option<f64>) -> String {
    format!("{input},{},{output}", cached.unwrap_or((input / 10.0 * 1e6).round() / 1e6))
}

/// The choice with its route pinned to its `i`-th checked provider (1..): that provider's key, price and state. 0: the
/// route as it is, SubRouter picking among the checked providers call by call.
pub fn pinned(c: &Choice, i: usize) -> Choice {
    let mut c = c.clone();
    let Some(p) = i.checked_sub(1).and_then(|i| c.pins.get(i)).cloned() else {
        if !c.pins.is_empty() {
            c.place.push_str(" · tab: pin one");
        }
        return c;
    };
    if let Some(r) = c.route.as_mut() {
        r.1 = p.key_env.clone();
        r.2 = route_price(p.price.0, p.price.1, None);
    }
    c.label = format!("{} · {}", c.label, p.provider);
    c.place = format!("SubRouter · {} only (pinned; tab: next)", p.provider);
    c.price = format!("{} / {}", per_m(p.price.0), per_m(p.price.1));
    c.state = p.state;
    c
}

/// Artificial Analysis' slug for a model name: lower case, dots as dashes.
fn slug(model: &str) -> String {
    model.to_lowercase().replace('.', "-")
}

fn scores(file: &Value, model: &str) -> (String, String) {
    let bench = file["bench"][model]["judge"].as_f64().map(|j| format!("{j:.1} ({})", file["bench"][model]["runs"].as_u64().unwrap_or(0)));
    (bench.unwrap_or_default(), file["aa"][slug(model)].as_f64().map(|a| format!("{a:.0}")).unwrap_or_default())
}

/// A price per M tokens in three significant digits, without the dollar sign: 0.01296 is 0.013, 1.17647 is 1.18.
fn per_m(x: f64) -> String {
    if x <= 0.0 {
        return "0".into();
    }
    let s = format!("{x:.*}", (2 - x.log10().floor() as i32).max(0) as usize);
    if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s }
}

/// The list built into tazuna (bench/routes.py ship): what a user gets when there is no routes.json from our bench. The
/// checked SubRouter routes without key names (2026-11-06: the user's one SubRouter key, which carries each model's
/// providers, `tazuna subrouter`), the routes on public providers with that provider's usual key variable (Mistral), and
/// every SubRouter model the picker lists (0.1.77: until then a new install listed 4 SubRouter models and no Mistral, the
/// bench's machines 34 rows).
const SHIPPED: &str = include_str!("routes.json");

/// The bench's routes.json, else the built-in list; true for the built-in one.
fn routes_file(routes: &Path) -> (Value, bool) {
    match std::fs::read_to_string(routes).ok().and_then(|t| serde_json::from_str(&t).ok()) {
        Some(v) => (v, false),
        None => (serde_json::from_str(SHIPPED).unwrap_or_default(), true),
    }
}

/// The models SubRouter lets the key in `var` use: its /v1/models follows the key's model limits (a key limited to 18
/// models listed only those and answered 403 for another, 2026-11-08). None when the key is unset or SubRouter does not
/// answer.
pub fn key_models(var: &str) -> Option<std::collections::HashSet<String>> {
    let key = std::env::var(var).ok()?;
    let http: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(15))).build().into();
    let mut r = http.get("https://subrouter.ai/v1/models").header("Authorization", format!("Bearer {}", key.trim())).call().ok()?;
    let d: Value = serde_json::from_str(&r.body_mut().read_to_string().ok()?).ok()?;
    let ms: std::collections::HashSet<String> = d["data"].as_array()?.iter().filter_map(|m| m["id"].as_str().map(String::from)).collect();
    (!ms.is_empty()).then_some(ms)
}

/// Why a row cannot be picked as it is, in words that say what to do; None when it can.
pub fn why_not(c: &Choice) -> Option<String> {
    let (url, var, _) = c.route.as_ref()?;
    let host = host_of(url);
    match c.state {
        State::NoKey => Some(format!("no key: set the environment variable {var} before starting tazuna{}", match host {
            "subrouter.ai" => " (a SubRouter key: https://subrouter.ai/console/keys)",
            "api.mistral.ai" => " (a Mistral API key, or one made under Code › Vibe CLI)",
            _ => "",
        })),
        State::Untrusted => Some(format!("not trusted: the code would go to {host}; add the line {host} to {} to allow it",
            crate::platform::home().unwrap_or_default().join(".config/tazuna/trusted-providers").display())),
        State::NotOnKey => Some(format!("not on your key: SubRouter does not let {var} use {} (the key's models: \
            https://subrouter.ai/console/keys)", c.model)),
        State::Ready | State::Unchecked => None,
    }
}

fn host_of(url: &str) -> &str {
    url.split("://").nth(1).unwrap_or(url).split(['/', ':']).next().unwrap_or_default()
}

/// Claude's models, then the checked routes, then the unchecked SubRouter models that have a public score; `trusted` says
/// whether a provider host may see the code, `keys` which models a key may use.
pub fn catalog(routes: &Path, trusted: impl Fn(&str) -> bool, keys: &KeyModels, now: u64) -> Vec<Choice> {
    let (file, builtin) = routes_file(routes);
    let on_key = |var: &str, model: &str| keys.get(var).is_none_or(|ms| ms.contains(model));
    let mut out: Vec<Choice> = CLAUDE.iter().map(|(m, l)| Choice {
        model: m.to_string(),
        label: l.to_string(),
        route: None,
        place: "Claude · subscription".into(),
        efforts: LEVELS.iter().map(|e| e.to_string()).collect(),
        price: "plan".into(),
        note: String::new(),
        stale: false,
        state: State::Ready,
        bench: scores(&file, m).0,
        aa: scores(&file, m).1,
        pins: vec![],
    }).collect();
    let mut rs: Vec<(&String, &Value)> = file["routes"].as_object().map(|o| o.iter().collect()).unwrap_or_default();
    rs.sort_by_key(|(m, _)| m.to_string());
    for (name, r) in rs {
        let Some(url) = r["url"].as_str() else { continue };
        let host = host_of(url);
        // a built-in SubRouter route names no key: the user's one SubRouter key, which carries the providers if they set them
        let shipped = builtin && host == "subrouter.ai";
        let Some(var) = r["key_env"].as_str().or((host == "subrouter.ai").then_some("SUBROUTER_API_KEY")) else { continue };
        // a second route for one model ("deepseek-v4.1-flash-1017@fast": another provider set) names its model
        let model = r["model"].as_str().map(String::from).unwrap_or_else(|| name.clone());
        let label = name.split_once('@').map_or(name.clone(), |(m, v)| format!("{m} · {v}"));
        let (input, output) = (r["price"][0].as_f64().unwrap_or(0.0), r["price"][1].as_f64().unwrap_or(0.0));
        let days = now.saturating_sub(r["vetted_at"].as_u64().unwrap_or(0)) / 86_400;
        let providers: Vec<&str> = r["providers"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        // the picker offers only the levels that differ (a user saw "medium" and "xhigh" for DeepSeek, which acts at
        // either as at high, 2026-11-06); a route that lists none ignores the effort
        let distinct: Vec<&str> = r["efforts"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        let levels = if distinct.is_empty() { " · effort is ignored".to_string() } else { String::new() };
        let state_of = |var: &str| if std::env::var_os(var).is_none() {
            State::NoKey
        } else if !trusted(host) {
            State::Untrusted
        } else if !on_key(var, &model) {
            State::NotOnKey
        } else {
            State::Ready
        };
        let state = state_of(var);
        let mut pins: Vec<Pin> = r["pinned"].as_object().into_iter().flatten().filter_map(|(g, p)| Some(Pin {
            provider: g.clone(),
            key_env: p["key_env"].as_str()?.to_string(),
            price: (p["price"][0].as_f64()?, p["price"][1].as_f64()?),
            state: state_of(p["key_env"].as_str()?),
        })).collect();
        pins.sort_by(|a, b| a.price.0.total_cmp(&b.price.0).then(a.provider.cmp(&b.provider)));
        out.push(Choice {
            model,
            label,
            route: Some((url.to_string(), var.to_string(), route_price(input, output, r["price"][2].as_f64()))),
            // another host (Mistral's API on a Vibe-plan key, 2026-11-07) is named by its host, not as SubRouter
            place: if host == "subrouter.ai" { format!("SubRouter · {}", providers.join(", ")) } else { host.to_string() },
            // only the levels that differ (DeepSeek: off, high, max); a route that names none offers them all
            efforts: if distinct.is_empty() { LEVELS.iter().map(|e| e.to_string()).collect() } else { distinct.iter().map(|e| e.to_string()).collect() },
            price: format!("{} / {}", per_m(input), per_m(output)),
            note: format!("cached {}% · checked {}{levels}{}", r["cached"].as_u64().unwrap_or(0), match days {
                0 => "today".to_string(),
                1 => "yesterday".to_string(),
                d => format!("{d} days ago"),
            }, if shipped { " · these providers on your key: tazuna subrouter" } else { "" }),
            // the built-in list is as old as the release, and `tazuna upgrade` renews it
            stale: days > STALE_DAYS && !builtin,
            state,
            bench: scores(&file, r["model"].as_str().unwrap_or(name)).0,
            aa: scores(&file, r["model"].as_str().unwrap_or(name)).1,
            pins,
        });
    }
    // every other SubRouter model with a public score (the list also holds image and video models), best first. One model
    // once: SubRouter lists some under two spellings ("claude-opus-5.5" beside the subscription's "claude-opus-5-5",
    // "DeepSeek-V4-Flash-0731" beside "deepseek-v4-flash-0731"), and a pick of the copy ran Opus through SubRouter instead
    // of the plan (a 403 on a key without it, 2026-11-08); the lower-case spelling stays
    let mut seen: std::collections::HashSet<String> = out.iter().map(|c| slug(&c.model)).collect();
    let mut rest: Vec<(f64, &String, &Value)> = file["catalog"].as_object().into_iter().flatten()
        .filter_map(|(m, c)| file["aa"][slug(m)].as_f64().map(|a| (a, m, c))).collect();
    rest.sort_by(|a, b| b.0.total_cmp(&a.0).then((*a.1 != a.1.to_lowercase()).cmp(&(*b.1 != b.1.to_lowercase()))).then(a.1.cmp(b.1)));
    rest.retain(|(_, m, _)| seen.insert(slug(m)));
    let key = "SUBROUTER_API_KEY";
    for (_, model, c) in rest {
        let (input, output) = (c["price"][0].as_f64().unwrap_or(0.0), c["price"][1].as_f64().unwrap_or(0.0));
        out.push(Choice {
            model: model.clone(),
            label: model.clone(),
            route: Some(("https://subrouter.ai/v1".into(), key.into(), route_price(input, output, None))),
            place: format!("SubRouter · {} providers", c["providers"].as_u64().unwrap_or(0)),
            efforts: LEVELS.iter().map(|e| e.to_string()).collect(),
            price: format!("{} / {}", per_m(input), per_m(output)),
            note: "unchecked: SubRouter picks the provider (the price is the cheapest one's)".into(),
            stale: false,
            state: if std::env::var_os(key).is_none() {
                State::NoKey
            } else if !trusted("subrouter.ai") {
                State::Untrusted
            } else if !on_key(key, model) {
                State::NotOnKey
            } else {
                State::Unchecked
            },
            bench: scores(&file, model).0,
            aa: scores(&file, model).1,
            pins: vec![],
        });
    }
    out
}

/// SubRouter's balance in USD, read with an API key (SUBROUTER_API_KEY, else any route key; none set: nothing is
/// sent), never the account login. Its billing endpoints count in CNY at 6.8 to the dollar: (granted - used) / 6.8
/// matched the account page to the cent (2026-11-02).
/// The other checked providers of the route `key_env` belongs to, cheapest first, as `TAZUNA_ROUTES` ("url key-env; ...",
/// only keys that are set): a live own-loop session goes on there when its provider refuses or fails. SubRouter keeps a
/// route's calls on one provider and does not forward a refusal: a6aa's content filter refused "when were you developed"
/// twice and the session just failed, while znu in the same route answered (2026-11-05).
pub fn fallbacks(routes: &Path, key_env: &str) -> String {
    let file: Value = std::fs::read_to_string(routes).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    for r in file["routes"].as_object().into_iter().flatten().map(|(_, r)| r) {
        let pins: Vec<(f64, &str)> = r["pinned"].as_object().into_iter().flatten()
            .filter_map(|(_, p)| Some((p["price"][0].as_f64().unwrap_or(0.0), p["key_env"].as_str()?))).collect();
        if r["key_env"].as_str() != Some(key_env) && !pins.iter().any(|(_, k)| *k == key_env) {
            continue;
        }
        let url = r["url"].as_str().unwrap_or_default();
        let mut others: Vec<(f64, &str)> = pins.into_iter().filter(|(_, k)| *k != key_env && std::env::var_os(k).is_some()).collect();
        others.sort_by(|a, b| a.0.total_cmp(&b.0));
        return others.iter().map(|(_, k)| format!("{url} {k}")).collect::<Vec<_>>().join("; ");
    }
    String::new()
}

pub fn subrouter_balance() -> Option<f64> {
    let key = std::env::var("SUBROUTER_API_KEY").ok()
        .or_else(|| std::env::vars().find(|(k, _)| k.starts_with("SUBROUTER_ROUTE_") && k.ends_with("_KEY")).map(|(_, v)| v))?;
    let http: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(15))).build().into();
    let get = |path: &str| -> Option<Value> {
        let mut r = http.get(format!("https://subrouter.ai{path}")).header("Authorization", format!("Bearer {key}")).call().ok()?;
        serde_json::from_str(&r.body_mut().read_to_string().ok()?).ok()
    };
    let granted = get("/v1/dashboard/billing/subscription")?["hard_limit_usd"].as_f64()?;
    let used = get("/v1/dashboard/billing/usage")?["total_usage"].as_f64()?;
    Some((granted - used / 100.0) / 6.8)
}

/// `tazuna subrouter [--apply]`: each checked model's providers, for the user's one SubRouter key. A key holds a
/// provider list per model (`subrouter_model_providers`) and SubRouter keeps each model's calls on its list, while models
/// without one go anywhere (bench/onekey.py, 2026-11-06). Without --apply it prints the lists and where to set them;
/// --apply sets them itself with the account's access token (~/.subrouter-account: SUBROUTER_ACCESS_TOKEN=,
/// SUBROUTER_USER_ID=), on the key in SUBROUTER_API_KEY, and touches nothing else on it: its flat provider list, filled,
/// limits every model (503s for the rest), so it is left as it is.
pub fn subrouter_cli(args: &[String]) -> anyhow::Result<()> {
    use anyhow::{bail, Context};
    let (file, _) = routes_file(&crate::labels::dir()?.join("routes.json"));
    let mut lists = serde_json::Map::new();
    for (name, r) in file["routes"].as_object().into_iter().flatten() {
        if name.contains('@') || !r["url"].as_str().unwrap_or_default().contains("subrouter.ai") {
            continue;
        }
        let ps = r["providers"].as_array().cloned().unwrap_or_default();
        let shown: Vec<&str> = ps.iter().filter_map(Value::as_str).collect();
        println!("{:28} {}", r["model"].as_str().unwrap_or(name), shown.join(", "));
        lists.insert(r["model"].as_str().unwrap_or(name).to_string(), Value::Array(ps));
    }
    if !args.iter().any(|a| a == "--apply") {
        println!("\nSet these on your SubRouter key (https://subrouter.ai/console/keys, edit the key, smart routing: per model, \
                  only these providers; leave its other settings). Or let tazuna do it: `tazuna subrouter --apply`, with \
                  your account's access token in ~/.subrouter-account (SUBROUTER_ACCESS_TOKEN=..., SUBROUTER_USER_ID=...).");
        return Ok(());
    }
    let path = crate::platform::home().context("HOME")?.join(".subrouter-account");
    let acct = std::fs::read_to_string(&path).with_context(|| format!("{}: needs SUBROUTER_ACCESS_TOKEN= and SUBROUTER_USER_ID=", path.display()))?;
    let get = |k: &str| acct.lines().find_map(|l| l.trim().strip_prefix(k)?.strip_prefix('=').map(|v| v.trim().to_string())).filter(|v| !v.is_empty());
    let (Some(token), Some(user)) = (get("SUBROUTER_ACCESS_TOKEN"), get("SUBROUTER_USER_ID")) else {
        bail!("{}: needs SUBROUTER_ACCESS_TOKEN= and SUBROUTER_USER_ID=", path.display());
    };
    let mine = std::env::var("SUBROUTER_API_KEY").context("SUBROUTER_API_KEY: the key to set the providers on")?;
    let mine = mine.trim().trim_start_matches("sk-");
    let http: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(30))).http_status_as_error(false).build().into();
    let api = |method: &str, path: &str, body: Option<&Value>| -> anyhow::Result<Value> {
        let url = format!("https://subrouter.ai{path}");
        let auth = |r: ureq::RequestBuilder<ureq::typestate::WithoutBody>| r.header("Authorization", format!("Bearer {token}")).header("New-Api-User", &user);
        let mut resp = match (method, body) {
            ("PUT", Some(b)) => http.put(&url).header("Authorization", format!("Bearer {token}")).header("New-Api-User", &user)
                .header("Content-Type", "application/json").send(&b.to_string())?,
            _ => auth(http.get(&url)).call()?,
        };
        let d: Value = serde_json::from_str(&resp.body_mut().read_to_string()?).with_context(|| format!("{path}: not JSON"))?;
        if d["success"] != true {
            bail!("{path}: {}", d["message"].as_str().unwrap_or("refused"));
        }
        Ok(d["data"].clone())
    };
    // every page of the account's keys: the user's own account had 141 (bench keys), and its main key was on page 2
    let mut ids: Vec<u64> = Vec::new();
    for p in 1.. {
        let page = api("GET", &format!("/api/token/?p={p}&page_size=100"), None)?;
        let got: Vec<u64> = page["items"].as_array().or(page.as_array()).into_iter().flatten().filter_map(|k| k["id"].as_u64()).collect();
        let last = got.len() < 100;
        ids.extend(got);
        if last {
            break;
        }
    }
    let mut key = None;
    for id in ids {
        let k = api("GET", &format!("/api/token/{id}"), None)?;
        if k["key"].as_str() == Some(mine) {
            key = Some(k);
            break;
        }
    }
    let mut key = key.context("SUBROUTER_API_KEY is not a key of this account")?;
    let mut map: serde_json::Map<String, Value> = key["subrouter_model_providers"].as_str().and_then(|s| serde_json::from_str(s).ok()).unwrap_or_default();
    map.extend(lists);
    key["subrouter_model_providers"] = Value::String(Value::Object(map.clone()).to_string());
    // the shared OpenAI pools serve a key only when it allows them
    let shared = map.values().flat_map(|v| v.as_array().cloned().unwrap_or_default()).any(|p| p.as_str().is_some_and(|p| p.starts_with("shared-")));
    if shared && key["include_shared_subscriptions"] != true {
        key["include_shared_subscriptions"] = Value::Bool(true);
        println!("(allowing shared subscription pools on the key: some lists name them)");
    }
    api("PUT", "/api/token/", Some(&key))?;
    println!("\nset on your key \"{}\": {} models; other models and settings unchanged", key["name"].as_str().unwrap_or("?"), map.len());
    if key["subrouter_providers"].as_str().is_some_and(|s| !s.trim().is_empty()) {
        println!("note: the key also has a flat provider list ({}), which limits every model to those; left as it is",
                 key["subrouter_providers"].as_str().unwrap_or_default());
    }
    Ok(())
}

/// The effort a row starts at: the session's own when it is that model, else medium on Claude and the model's high (its
/// default where it has few levels) on a route.
pub fn start_effort(c: &Choice, current_model: &str, current_effort: &str) -> usize {
    let pick = if c.model == current_model { current_effort } else if c.route.is_none() { "medium" } else { "high" };
    c.efforts.iter().position(|e| e == pick).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_route_falls_back_to_its_other_providers() {
        let f = std::env::temp_dir().join(format!("tazuna-fallbacks-{}.json", std::process::id()));
        std::fs::write(&f, r#"{"routes": {"m": {"url": "https://subrouter.ai/v1", "key_env": "ROUTE_KEY",
            "pinned": {"a6aa": {"key_env": "PATH", "price": [0.012, 0.048]}, "znu": {"key_env": "HOME", "price": [0.0118, 0.047]},
                       "gone": {"key_env": "TAZUNA_TEST_NO_SUCH_KEY", "price": [0.001, 0.01]}}}}}"#).unwrap();
        assert_eq!(fallbacks(&f, "ROUTE_KEY"), "https://subrouter.ai/v1 HOME; https://subrouter.ai/v1 PATH", "cheapest first, unset keys left out");
        assert_eq!(fallbacks(&f, "PATH"), "https://subrouter.ai/v1 HOME", "a pinned provider falls back to the others");
        assert_eq!(fallbacks(&f, "OTHER"), "");
        std::fs::remove_file(&f).ok();
    }

    #[test]
    fn a_route_can_name_its_cached_price() {
        assert_eq!(route_price(0.3, 1.2, Some(0.007)), "0.3,0.007,1.2", "Mistral's DeepSeek: cached at 2.3%");
        assert_eq!(route_price(0.02, 0.08, None), "0.02,0.002,0.08", "otherwise a tenth of input");
    }

    #[test]
    fn without_routes_json_the_built_in_routes_use_the_one_key() {
        let cs = catalog(Path::new("/nonexistent/routes.json"), |_| true, &KeyModels::new(), u64::MAX / 2);
        let ds = cs.iter().find(|c| c.model == "deepseek-v4.1-flash-1017").expect("DeepSeek is built in");
        assert_eq!(ds.route.as_ref().unwrap().1, "SUBROUTER_API_KEY");
        assert!(ds.note.contains("tazuna subrouter") && !ds.stale, "{}", ds.note);
        assert_eq!(ds.efforts, ["off", "high", "max"]);
        // a new install lists what the bench's machines list (0.1.77; four SubRouter models until then): Mistral's routes
        // on Mistral's usual key variable, never stale, and SubRouter's other models; nothing on a private host
        let mistral: Vec<&Choice> = cs.iter().filter(|c| c.place == "api.mistral.ai").collect();
        assert!(!mistral.is_empty() && mistral.iter().all(|c| c.route.as_ref().unwrap().1 == "MISTRAL_API_KEY" && !c.stale && !c.note.contains("tazuna subrouter")));
        assert!(cs.iter().filter(|c| c.place.starts_with("SubRouter")).count() > 10, "SubRouter's catalog ships");
        assert!(cs.iter().all(|c| c.route.as_ref().is_none_or(|r| !host_of(&r.0).starts_with(|ch: char| ch.is_ascii_digit()) && host_of(&r.0) != "localhost")));
        // SubRouter's other models once each, and none a copy of a subscription model (the list has "claude-opus-5.5")
        let claude: Vec<String> = CLAUDE.iter().map(|(m, _)| slug(m)).collect();
        let mut more: Vec<String> = cs.iter().filter(|c| c.note.starts_with("unchecked")).map(|c| slug(&c.model)).collect();
        assert!(more.iter().all(|m| !claude.contains(m)), "{more:?}");
        let n = more.len();
        more.sort();
        more.dedup();
        assert_eq!(more.len(), n);
    }

    #[test]
    fn one_row_per_model_and_what_the_key_may_use() {
        let f = std::env::temp_dir().join(format!("tazuna-keymodels-{}.json", std::process::id()));
        // PATH stands in for a key variable that is set
        std::fs::write(&f, r#"{"routes": {"gpt-x": {"url": "https://subrouter.ai/v1", "key_env": "PATH", "price": [1, 2], "vetted_at": 0}},
            "catalog": {"claude-opus-5.5": {"price": [5, 25], "providers": 3}, "DeepSeek-V4-Flash-0731": {"price": [0.1, 0.4], "providers": 2},
                        "deepseek-v4-flash-0731": {"price": [0.1, 0.4], "providers": 4}, "glm-5.3": {"price": [0.2, 0.8], "providers": 5}},
            "aa": {"claude-opus-5-5": 70, "deepseek-v4-flash-0731": 50, "glm-5-3": 45}}"#).unwrap();
        let none = KeyModels::new();
        let cs: Vec<Choice> = catalog(&f, |_| true, &none, 0).into_iter().filter(|c| c.route.is_some()).collect();
        assert_eq!(cs.iter().map(|c| c.model.as_str()).collect::<Vec<_>>(), ["gpt-x", "deepseek-v4-flash-0731", "glm-5.3"],
                   "the subscription's Opus not again through SubRouter (a pick ran it there, 2026-11-08); the lower-case spelling once");
        assert_eq!(cs[0].state, State::Ready, "a key not looked up is not held against a model");
        let keys = |ms: &[&str]| KeyModels::from([("PATH".to_string(), ms.iter().map(|m| m.to_string()).collect())]);
        let route = |k: &KeyModels| catalog(&f, |_| true, k, 0).into_iter().find(|c| c.model == "gpt-x").unwrap();
        let off = route(&keys(&["glm-5.3"]));
        assert_eq!(off.state, State::NotOnKey);
        assert!(why_not(&off).unwrap().contains("does not let PATH use gpt-x"), "{:?}", why_not(&off));
        assert_eq!(route(&keys(&["gpt-x"])).state, State::Ready);
        // the reasons name what to do
        let mut c = cs[2].clone();
        c.state = State::NoKey;
        assert!(why_not(&c).unwrap().contains("set the environment variable SUBROUTER_API_KEY"));
        c.state = State::Untrusted;
        let why = why_not(&c).unwrap();
        assert!(why.contains("add the line subrouter.ai to") && why.contains("trusted-providers"), "{why}");
        c.state = State::Unchecked;
        assert_eq!(why_not(&c), None);
        std::fs::remove_file(&f).ok();
    }

    #[test]
    fn lists_claude_then_checked_routes_with_their_state() {
        let dir = std::env::temp_dir().join(format!("tazuna-models-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("routes.json");
        std::fs::write(&f, r#"{"routes": {
            "deepseek-v4.1-flash-1017": {"url": "https://subrouter.ai/v1", "key_env": "TAZUNA_TEST_NO_SUCH_KEY", "providers": ["lingai"],
                "price": [0.022, 0.088], "cached": 81, "efforts": ["high", "max"], "vetted_at": 1000000},
            "deepseek-v4.1-flash-1017@fast": {"model": "deepseek-v4.1-flash-1017", "url": "https://subrouter.ai/v1", "key_env": "PATH",
                "providers": ["lingai"], "price": [0.022, 0.088], "cached": 89, "efforts": ["high", "max"], "vetted_at": 1000000},
            "gpt-6.1-sol": {"url": "https://subrouter.ai/v1", "key_env": "PATH", "providers": ["kriora", "shared-openai-pro-50"],
                "price": [0.0387, 0.1935], "cached": 91, "efforts": ["low", "high", "xhigh", "max"], "vetted_at": 1000000,
                "pinned": {"shared-openai-pro-50": {"key_env": "TAZUNA_TEST_NO_SUCH_KEY", "price": [0.05, 0.25]},
                           "kriora": {"key_env": "HOME", "price": [0.0387, 0.1935]}}}}}"#).unwrap();
        let now = 1000000 + 9 * 86_400;
        let mut d: Value = serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        d["catalog"] = serde_json::json!({"glm-5.3": {"price": [0.1, 0.4], "providers": 3}, "seadance-video": {"price": [1, 1], "providers": 1},
                                          "gpt-6.1-sol": {"price": [0.03, 0.2], "providers": 40}});
        d["aa"] = serde_json::json!({"glm-5-3": 41.0, "gpt-6-1-sol": 54.5});
        d["bench"] = serde_json::json!({"gpt-6.1-sol": {"judge": 6.6, "runs": 51, "minutes": 19}});
        std::fs::write(&f, d.to_string()).unwrap();
        let c = catalog(&f, |h| h == "subrouter.ai", &KeyModels::new(), now);
        assert_eq!(c.iter().map(|c| c.label.as_str()).collect::<Vec<_>>(),
                   ["Opus 5.5", "Sonnet 5.5", "Haiku 5.5", "Fable 5.1", "deepseek-v4.1-flash-1017", "deepseek-v4.1-flash-1017 · fast", "gpt-6.1-sol", "glm-5.3"],
                   "routed models once; unchecked ones only with a public score (no video model)");
        assert_eq!((c[6].bench.as_str(), c[6].aa.as_str(), c[6].price.as_str()), ("6.6 (51)", "54", "0.0387 / 0.194"));
        assert_eq!((c[7].route.as_ref().unwrap().1.as_str(), c[7].price.as_str()), ("SUBROUTER_API_KEY", "0.1 / 0.4"));
        assert_eq!([0.01296, 0.02206, 1.17647, 0.003, 12.5, 120.0].map(per_m), ["0.013", "0.0221", "1.18", "0.003", "12.5", "120"]);
        let c: Vec<Choice> = c.into_iter().filter(|c| c.label != "glm-5.3").collect();
        assert_eq!((c[5].model.as_str(), c[5].place.as_str()), ("deepseek-v4.1-flash-1017", "SubRouter · lingai"));
        let c: Vec<Choice> = c.into_iter().filter(|c| !c.label.ends_with("fast")).collect();
        assert!(c[0].route.is_none() && c[0].state == State::Ready);
        assert_eq!(c[4].state, State::NoKey, "no key in the environment");
        assert_eq!((c[5].state, c[5].stale), (State::Ready, true), "key set (PATH stands in), host trusted, checked 9 days ago");
        assert_eq!(c[4].efforts, ["high", "max"], "only the levels that differ");
        assert_eq!(c[5].efforts, ["low", "high", "xhigh", "max"], "Sol: its four levels that differ");
        assert_eq!(c[5].route.as_ref().unwrap().2, "0.0387,0.00387,0.1935");
        assert_eq!(catalog(&f, |_| false, &KeyModels::new(), now)[5].state, State::Untrusted);
        assert_eq!(c[4].efforts[start_effort(&c[4], "claude-opus-5-5", "medium")], "high");
        assert_eq!(c[0].efforts[start_effort(&c[0], "claude-opus-5-5", "xhigh")], "xhigh");
        // pinning: the cheapest provider first, its own key and price; a pinned key that is not set says so
        assert_eq!(c[5].pins.iter().map(|p| p.provider.as_str()).collect::<Vec<_>>(), ["kriora", "shared-openai-pro-50"]);
        assert!(pinned(&c[5], 0).place.ends_with("tab: pin one") && pinned(&c[5], 0).route == c[5].route);
        let k = pinned(&c[5], 1);
        assert_eq!((k.label.as_str(), k.route.as_ref().unwrap().1.as_str(), k.state), ("gpt-6.1-sol · kriora", "HOME", State::Ready));
        let s = pinned(&c[5], 2);
        assert_eq!((s.price.as_str(), s.route.as_ref().unwrap().2.as_str(), s.state), ("0.05 / 0.25", "0.05,0.005,0.25", State::NoKey));
        assert_eq!(pinned(&c[4], 1).label, c[4].label, "no pins: the route as it is");
        std::fs::remove_dir_all(&dir).ok();
    }
}
