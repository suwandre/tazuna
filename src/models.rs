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
/// Haiku 5.5 and Fable 5.2 take all five; Opus 5.5 and Haiku 5.5 default to medium, the others to high). Claude Code
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
    [("claude-opus-5-5", "Opus 5.5"), ("claude-sonnet-5-5", "Sonnet 5.5"), ("claude-haiku-5-5", "Haiku 5.5"), ("claude-fable-5-2", "Fable 5.2")];
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
    let key = key(var)?;
    let http: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(15))).build().into();
    let mut r = http.get("https://subrouter.ai/v1/models").header("Authorization", format!("Bearer {}", key.trim())).call().ok()?;
    let d: Value = serde_json::from_str(&r.body_mut().read_to_string().ok()?).ok()?;
    let ms: std::collections::HashSet<String> = d["data"].as_array()?.iter().filter_map(|m| m["id"].as_str().map(String::from)).collect();
    (!ms.is_empty()).then_some(ms)
}

/// The provider a row belongs to in `/model`'s first level (OMP-style: providers, then their models): Claude for the
/// subscription, else the route's host by its name.
pub fn provider_of(c: &Choice) -> String {
    match c.route.as_ref().map(|r| host_of(&r.0)) {
        None => "Claude".into(),
        Some("subrouter.ai") => "SubRouter".into(),
        Some("mortera.cloud") => "Mortera".into(),
        Some("api.mistral.ai") => "Mistral".into(),
        // a machine of the user's (an address, not a name): the route's own name for it ("qwen3.8-flash-next@pc": pc)
        Some(h) if h == "localhost" || h.parse::<std::net::IpAddr>().is_ok() => c.label.rsplit_once(" · ").map_or(h.to_string(), |(_, n)| n.to_string()),
        Some(h) => h.to_string(),
    }
}

/// `/model`'s providers in their order (Claude, SubRouter, Mortera, Mistral, then the rest as they come), each with its
/// rows' indices.
pub fn providers(cs: &[Choice]) -> Vec<(String, Vec<usize>)> {
    let mut out: Vec<(String, Vec<usize>)> = vec![];
    for (i, c) in cs.iter().enumerate() {
        let p = provider_of(c);
        match out.iter_mut().find(|(n, _)| *n == p) {
            Some((_, v)) => v.push(i),
            None => out.push((p, vec![i])),
        }
    }
    let rank = |n: &str| ["Claude", "SubRouter", "Mortera", "Mistral"].iter().position(|x| *x == n).unwrap_or(9);
    out.sort_by_key(|(n, _)| rank(n));
    out
}

/// Why a row cannot be picked as it is, in words that say what to do; None when it can.
pub fn why_not(c: &Choice) -> Option<String> {
    let (url, var, _) = c.route.as_ref()?;
    let host = host_of(url);
    match c.state {
        State::NoKey if host == "subrouter.ai" => Some(format!("no SubRouter key. Two ways: `tazuna subrouter --manage` makes \
            one in your account with tested providers for each model (needs your account's access token); or bring your own \
            key: /login, or set {var} (its provider settings decide: a model without a list on it can land on any \
            provider)")),
        State::NoKey => Some(format!("no key: /login, or set the environment variable {var} before starting tazuna{}", match host {
            "api.mistral.ai" => " (a Mistral API key, or one made under Code › Vibe CLI)",
            _ => "",
        })),
        State::Untrusted => Some(format!("not trusted: the code would go to {host}; /login allows it, or add the line {host} to {}",
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
    // who chose the providers behind SUBROUTER_API_KEY: tazuna (its managed key) or the user (their own key)
    let key_note = if key_is_managed() {
        " · on tazuna's managed key"
    } else {
        " · your own key decides the providers: tazuna subrouter --apply pins these on it, --manage makes a key with them"
    };
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
        let state_of = |var: &str| if key(var).is_none() {
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
            // a route added without our checks (Mortera's models, 2026-11-09) says so instead of a cache share and a date
            note: if r["unchecked"] == true {
                format!("not checked yet: prices from {}'s list; effort levels, cache and quality not measured", r["via"].as_str().unwrap_or(host))
            } else {
                format!("cached {}% · checked {}{levels}{}", r["cached"].as_u64().unwrap_or(0), match days {
                    0 => "today".to_string(),
                    1 => "yesterday".to_string(),
                    d => format!("{d} days ago"),
                }, if shipped { key_note } else { "" })
            },
            // the built-in list is as old as the release, and `tazuna upgrade` renews it
            stale: days > STALE_DAYS && !builtin,
            state,
            bench: scores(&file, r["model"].as_str().unwrap_or(name)).0,
            aa: scores(&file, r["model"].as_str().unwrap_or(name)).1,
            pins,
        });
    }
    // Mortera's own list (2026-11-09): every model that answers in text, from the copy `refresh_mortera` keeps
    // beside the routes file: the data folder in use, a test's own folder in tests
    if let Some(list) = routes.parent().and_then(|d| std::fs::read_to_string(d.join("mortera-models.json")).ok()) {
        out.extend(mortera_choices(&serde_json::from_str(&list).unwrap_or_default(), &file, &trusted));
    } else if key("MORTERA_API_KEY").is_none() {
        // no key, so no list yet: one greyed row, so Mortera is in /model with what it needs (it vanished on a PC whose
        // shell had no MORTERA_API_KEY, 2026-11-09)
        out.push(Choice {
            model: "mortera".into(),
            label: "Mortera's models".into(),
            route: Some(("https://mortera.cloud/v1".into(), "MORTERA_API_KEY".into(), "0,0,0".into())),
            place: "Mortera".into(),
            efforts: vec![],
            price: String::new(),
            note: "tazuna loads Mortera's model list at start once MORTERA_API_KEY is set".into(),
            stale: false,
            state: State::NoKey,
            bench: String::new(),
            aa: String::new(),
            pins: vec![],
        });
    }
    // every other SubRouter model with a public score (the list also holds image and video models), best first. One model
    // once: SubRouter lists some under two spellings ("claude-opus-5.5" beside the subscription's "claude-opus-5-5",
    // "DeepSeek-V4-Flash-0731" beside "deepseek-v4-flash-0731"), and a pick of the copy ran Opus through SubRouter instead
    // of the plan (a 403 on a key without it, 2026-11-08); the lower-case spelling stays
    // a model on another provider (Mortera, Mistral) does not hide SubRouter's own row for it
    let mut seen: std::collections::HashSet<String> =
        out.iter().filter(|c| c.route.as_ref().is_none_or(|r| host_of(&r.0) == "subrouter.ai")).map(|c| slug(&c.model)).collect();
    let mut rest: Vec<(f64, &String, &Value)> = file["catalog"].as_object().into_iter().flatten()
        .filter_map(|(m, c)| file["aa"][slug(m)].as_f64().map(|a| (a, m, c))).collect();
    rest.sort_by(|a, b| b.0.total_cmp(&a.0).then((*a.1 != a.1.to_lowercase()).cmp(&(*b.1 != b.1.to_lowercase()))).then(a.1.cmp(b.1)));
    // Claude runs on the subscription, never through SubRouter (with Fable 5.2 in the list, SubRouter's Fable 5.1 would
    // have come in as a row of its own, 2026-11-10)
    rest.retain(|(_, m, _)| !m.to_lowercase().starts_with("claude-") && seen.insert(slug(m)));
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
            state: if self::key(key).is_none() {
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

/// Mortera's models as picker rows: those with text output, minus embeddings, image makers and Claude (Claude runs on the
/// subscription; Mortera left its prompts uncached). Prices from its list, in rupiah; none of them checked by us yet.
fn mortera_choices(list: &Value, file: &Value, trusted: &impl Fn(&str) -> bool) -> Vec<Choice> {
    let mut rows: Vec<Choice> = list["data"].as_array().into_iter().flatten().filter_map(|m| {
        let id = m["id"].as_str()?;
        let out = m["architecture"]["output_modalities"].as_array();
        let text = out.is_none_or(|o| o.iter().any(|x| x == "text"));
        if !text || id.contains("embedding") || id.contains("image") || id.contains("banana") || id.starts_with("claude-") {
            return None;
        }
        let rp = &m["pricing"]["payg_per_1m_idr"];
        let usd = |k: &str| rp[k].as_f64().map(|r| r / crate::engine::IDR_PER_USD);
        let (input, output) = (usd("in")?, usd("out")?);
        Some(Choice {
            model: id.to_string(),
            label: id.to_string(),
            route: Some(("https://mortera.cloud/v1".into(), "MORTERA_API_KEY".into(), route_price(input, output, usd("cache")))),
            place: "Mortera".into(),
            efforts: LEVELS.iter().map(|e| e.to_string()).collect(),
            price: format!("{} / {}", per_m(input), per_m(output)),
            note: "not checked yet: prices from Mortera's list; effort levels, cache and quality not measured".into(),
            stale: false,
            state: if key("MORTERA_API_KEY").is_none() {
                State::NoKey
            } else if !trusted("mortera.cloud") {
                State::Untrusted
            } else {
                State::Ready
            },
            bench: scores(file, id).0,
            aa: scores(file, id).1,
            pins: vec![],
        })
    }).collect();
    rows.sort_by(|a, b| a.model.cmp(&b.model));
    rows
}

/// At start, with MORTERA_API_KEY set: Mortera's model list into the data folder, where `/model` reads it (a slow or
/// absent answer leaves the last copy).
pub fn refresh_mortera() {
    let Some(k) = key("MORTERA_API_KEY") else { return };
    let http: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(15))).build().into();
    let Ok(mut r) = http.get("https://mortera.cloud/v1/models").header("Authorization", format!("Bearer {}", k.trim())).call() else { return };
    let Ok(body) = r.body_mut().with_config().limit(20 << 20).read_to_string() else { return };
    let ok = serde_json::from_str::<Value>(&body).is_ok_and(|v| v["data"].as_array().is_some_and(|d| !d.is_empty()));
    if let (true, Ok(dir)) = (ok, crate::labels::dir()) {
        let tmp = dir.join("mortera-models.json.tmp");
        if std::fs::write(&tmp, body).is_ok() {
            let _ = std::fs::rename(&tmp, dir.join("mortera-models.json"));
        }
    }
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
        let mut others: Vec<(f64, &str)> = pins.into_iter().filter(|(_, k)| *k != key_env && key(k).is_some()).collect();
        others.sort_by(|a, b| a.0.total_cmp(&b.0));
        return others.iter().map(|(_, k)| format!("{url} {k}")).collect::<Vec<_>>().join("; ");
    }
    String::new()
}

pub fn subrouter_balance() -> Option<f64> {
    let key = key("SUBROUTER_API_KEY")
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

/// The key tazuna makes and keeps in a user's SubRouter account (`tazuna subrouter --manage`).
const MANAGED_NAME: &str = "tazuna (managed)";

fn managed_file() -> Option<std::path::PathBuf> {
    crate::platform::home().map(|h| h.join(".config/tazuna/subrouter-key"))
}

/// The managed key's value, when tazuna made one.
pub fn managed_key() -> Option<String> {
    std::fs::read_to_string(managed_file()?).ok().map(|k| k.trim().to_string()).filter(|k| !k.is_empty())
}

/// Keys saved in tazuna (`/model`: enter on a provider without one), by variable name, readable by the user only.
fn keys_file() -> Option<std::path::PathBuf> {
    crate::platform::home().map(|h| h.join(".config/tazuna/keys.json"))
}

/// Saves a provider's key under its variable name (a file that does not parse is left alone: an error, not a reset).
pub fn save_key(var: &str, value: &str) -> anyhow::Result<std::path::PathBuf> {
    let p = keys_file().ok_or_else(|| anyhow::anyhow!("HOME not set"))?;
    let mut v: Value = match std::fs::read_to_string(&p) {
        Ok(t) => serde_json::from_str(&t).map_err(|e| anyhow::anyhow!("{} does not parse ({e}): fix or remove it", p.display()))?,
        Err(_) => serde_json::json!({}),
    };
    let Some(m) = v.as_object_mut() else { anyhow::bail!("{} is not a JSON object: fix or remove it", p.display()) };
    m.insert(var.to_string(), value.trim().into());
    crate::mcpc::write_private(&p, &v)?;
    Ok(p)
}

fn saved_key(var: &str) -> Option<String> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(keys_file()?).ok()?).ok()?;
    v[var].as_str().map(|k| k.trim().to_string()).filter(|k| !k.is_empty())
}

fn env_key(var: &str) -> Option<String> {
    std::env::var(var).ok().filter(|k| !k.trim().is_empty())
}

/// The user's own key for a variable: the environment's, else the one saved in tazuna.
fn own_key(var: &str) -> Option<String> {
    env_key(var).or_else(|| saved_key(var))
}

/// The key a route's variable names: the environment's, else one saved in tazuna, else for SUBROUTER_API_KEY tazuna's
/// managed key. Saved and managed keys never go into tazuna's environment, where every child (claude, the agent's
/// commands) would inherit them; only the own loop's engine gets its key, and takes it out of its environment before any
/// tool runs.
pub fn key(var: &str) -> Option<String> {
    own_key(var).or_else(|| (var == "SUBROUTER_API_KEY").then(managed_key).flatten())
}

/// A provider `/login` offers: its name, the key's variable, its host (it sees the code), where keys are made, and an
/// example of a key's start. None of them has a login for third-party tools (no OAuth), so an API key it is.
pub struct Login {
    pub name: &'static str,
    /// The variables its routes read the key from (the first is the usual one): a login saves it under each.
    pub vars: &'static [&'static str],
    pub host: &'static str,
    pub page: &'static str,
    pub place: &'static str,
    pub example: &'static str,
}

pub const LOGINS: [Login; 3] = [
    Login { name: "SubRouter", vars: &["SUBROUTER_API_KEY"], host: "subrouter.ai", page: "https://subrouter.ai/console/keys",
            place: "SubRouter's console", example: "sk-..." },
    Login { name: "Mortera", vars: &["MORTERA_API_KEY"], host: "mortera.cloud", page: "https://mortera.cloud",
            place: "your Mortera dashboard", example: "" },
    // a Vibe CLI key works on the same API, and a user's routes read it from MISTRAL_VIBE_API_KEY (2026-11-10)
    Login { name: "Mistral", vars: &["MISTRAL_API_KEY", "MISTRAL_VIBE_API_KEY"], host: "api.mistral.ai", page: "https://console.mistral.ai/api-keys",
            place: "Mistral's console (or Code › Vibe CLI)", example: "" },
];

/// Where a provider's key comes from, for `/login`'s list; empty: none.
pub fn key_source(vars: &[&str]) -> &'static str {
    if vars.iter().any(|v| env_key(v).is_some()) {
        "key in the environment"
    } else if vars.iter().any(|v| saved_key(v).is_some()) {
        "logged in"
    } else if vars.contains(&"SUBROUTER_API_KEY") && managed_key().is_some() {
        "managed key (tazuna subrouter --manage)"
    } else {
        ""
    }
}

/// True when SUBROUTER_API_KEY means tazuna's managed key (the user set no key of their own), so tazuna chose its
/// providers.
pub fn key_is_managed() -> bool {
    managed_key().is_some_and(|m| own_key("SUBROUTER_API_KEY").is_none_or(|k| k == m))
}

/// SubRouter's account API with the account's access token (~/.subrouter-account: SUBROUTER_ACCESS_TOKEN=,
/// SUBROUTER_USER_ID=). Used for the user's keys only.
struct Account {
    token: String,
    user: String,
    http: ureq::Agent,
}

impl Account {
    fn load() -> anyhow::Result<Account> {
        use anyhow::Context;
        let path = crate::platform::home().context("HOME")?.join(".subrouter-account");
        let how = format!("{}: needs SUBROUTER_ACCESS_TOKEN=... and SUBROUTER_USER_ID=... (SubRouter: your profile, \
            security, access token; the user id is beside it)", path.display());
        let acct = std::fs::read_to_string(&path).context(how.clone())?;
        let get = |k: &str| acct.lines().find_map(|l| l.trim().strip_prefix(k)?.strip_prefix('=').map(|v| v.trim().to_string())).filter(|v| !v.is_empty());
        let (Some(token), Some(user)) = (get("SUBROUTER_ACCESS_TOKEN"), get("SUBROUTER_USER_ID")) else { anyhow::bail!(how) };
        let http = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(30))).http_status_as_error(false).build().into();
        Ok(Account { token, user, http })
    }

    fn api(&self, method: &str, path: &str, body: Option<&Value>) -> anyhow::Result<Value> {
        use anyhow::Context;
        let url = format!("https://subrouter.ai{path}");
        let (auth, user) = (format!("Bearer {}", self.token), self.user.as_str());
        let mut resp = match (method, body) {
            ("PUT" | "POST", Some(b)) => {
                let r = if method == "PUT" { self.http.put(&url) } else { self.http.post(&url) };
                r.header("Authorization", &auth).header("New-Api-User", user).header("Content-Type", "application/json").send(&b.to_string())?
            }
            ("DELETE", _) => self.http.delete(&url).header("Authorization", &auth).header("New-Api-User", user).call()?,
            _ => self.http.get(&url).header("Authorization", &auth).header("New-Api-User", user).call()?,
        };
        let d: Value = serde_json::from_str(&resp.body_mut().read_to_string()?).with_context(|| format!("{path}: not JSON"))?;
        if d["success"] != true {
            anyhow::bail!("{path}: {}", d["message"].as_str().unwrap_or("refused"));
        }
        Ok(d["data"].clone())
    }

    /// Every key of the account (id and name), every page: the user's own account had 141 (bench keys) and its main key
    /// was on page 2.
    fn keys(&self) -> anyhow::Result<Vec<Value>> {
        let mut all = Vec::new();
        for p in 1.. {
            let page = self.api("GET", &format!("/api/token/?p={p}&page_size=100"), None)?;
            let got: Vec<Value> = page["items"].as_array().or(page.as_array()).cloned().unwrap_or_default();
            let last = got.len() < 100;
            all.extend(got);
            if last {
                break;
            }
        }
        Ok(all)
    }

    /// Sets the per-model provider lists on a key (`replace`: only these lists, else added to the key's own) and the
    /// shared pools a list names; nothing else on the key changes.
    fn set_lists(&self, key: &mut Value, lists: &serde_json::Map<String, Value>, replace: bool) -> anyhow::Result<usize> {
        let mut map: serde_json::Map<String, Value> =
            if replace { Default::default() } else { key["subrouter_model_providers"].as_str().and_then(|s| serde_json::from_str(s).ok()).unwrap_or_default() };
        map.extend(lists.clone());
        key["subrouter_model_providers"] = Value::String(Value::Object(map.clone()).to_string());
        // the shared OpenAI pools serve a key only when it allows them
        let shared = map.values().flat_map(|v| v.as_array().cloned().unwrap_or_default()).any(|p| p.as_str().is_some_and(|p| p.starts_with("shared-")));
        if shared && key["include_shared_subscriptions"] != true {
            key["include_shared_subscriptions"] = Value::Bool(true);
            println!("(allowing shared subscription pools on the key: some lists name them)");
        }
        self.api("PUT", "/api/token/", Some(key))?;
        Ok(map.len())
    }
}

/// `tazuna subrouter [--apply | --manage]`: each checked model's providers, and two ways to use them. A key holds a provider
/// list per model (`subrouter_model_providers`) and SubRouter keeps each model's calls on its list, while models without
/// one go anywhere (bench/onekey.py, 2026-11-06): on a key with no list for DeepSeek every call of a bench run went to a
/// provider that cached 11% of the input, against 99% on the checked ones (2026-11-09).
/// - `--manage`: tazuna makes a key of its own in the account ("tazuna (managed)"), or renews it, with exactly the
///   checked lists, and keeps its value in ~/.config/tazuna/subrouter-key; it is used when SUBROUTER_API_KEY is not set.
/// - `--apply`: the lists go onto the user's own key in SUBROUTER_API_KEY, beside its own; nothing else on it changes
///   (its flat provider list, filled, limits every model: 503s for the rest, so it is left as it is).
pub fn subrouter_cli(args: &[String]) -> anyhow::Result<()> {
    use anyhow::Context;
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
    let manage = args.iter().any(|a| a == "--manage");
    if !manage && !args.iter().any(|a| a == "--apply") {
        println!("\nThese are the providers tazuna's checks passed for each model. Two ways to use them:\n\
                  - tazuna subrouter --manage: tazuna makes a key in your SubRouter account with exactly these lists and \
                  keeps it current when you run it again (needs your account's access token in ~/.subrouter-account: \
                  SUBROUTER_ACCESS_TOKEN=..., SUBROUTER_USER_ID=...). New to SubRouter: this one.\n\
                  - your own key in SUBROUTER_API_KEY: you choose its providers (https://subrouter.ai/console/keys, edit the \
                  key, smart routing per model). tazuna subrouter --apply puts these lists on it for you. A model with no \
                  list on the key can go to any provider, also ones that serve another model or barely cache.");
        return Ok(());
    }
    let acct = Account::load()?;
    if manage {
        println!("\ntazuna makes (or renews) the key \"{MANAGED_NAME}\" in your SubRouter account: each checked model's calls go only to \
                  the providers above, which served the real model, cached well and answered in tazuna's checks. Other models \
                  work too, where SubRouter picks the provider. The key has no spend cap of its own: your balance is the \
                  limit, and tazuna stops a task at $1. Your other keys are not touched. Your own choice instead: a key you \
                  set up yourself in SUBROUTER_API_KEY, which always wins over this one.");
        let find = |acct: &Account| -> anyhow::Result<Option<u64>> {
            Ok(acct.keys()?.iter().find(|k| k["name"] == MANAGED_NAME).and_then(|k| k["id"].as_u64()))
        };
        let id = match find(&acct)? {
            Some(id) => id,
            None => {
                acct.api("POST", "/api/token/", Some(&serde_json::json!({"name": MANAGED_NAME, "expired_time": -1, "unlimited_quota": true,
                    "model_limits_enabled": false, "model_limits": "", "subrouter_model_providers": "{}", "subrouter_providers": "",
                    "include_cache_hit_rate": true})))?;
                find(&acct)?.context("SubRouter did not keep the new key")?
            }
        };
        let mut key = acct.api("GET", &format!("/api/token/{id}"), None)?;
        let n = acct.set_lists(&mut key, &lists, true)?;
        let value = format!("sk-{}", key["key"].as_str().context("SubRouter did not show the key")?.trim_start_matches("sk-"));
        let path = managed_file().context("HOME")?;
        crate::platform::private_dir(path.parent().context("no folder")?, true)?;
        std::io::Write::write_all(&mut crate::platform::private_file(&path, false)?, value.as_bytes())?;
        println!("\nready: \"{MANAGED_NAME}\" with {n} checked models, kept in {} (readable by you only). tazuna uses it whenever \
                  SUBROUTER_API_KEY is not set. Run this again after `tazuna update` to bring new checks in.", path.display());
        if own_key("SUBROUTER_API_KEY").is_some_and(|k| k != value) {
            println!("note: SUBROUTER_API_KEY is set (or saved in tazuna) to another key, and it wins: remove it to use the managed key");
        }
        return Ok(());
    }
    let mine = own_key("SUBROUTER_API_KEY").context("SUBROUTER_API_KEY: the key to set the providers on")?;
    let mine = mine.trim().trim_start_matches("sk-");
    let mut key = None;
    for k in acct.keys()? {
        let Some(id) = k["id"].as_u64() else { continue };
        let k = acct.api("GET", &format!("/api/token/{id}"), None)?;
        if k["key"].as_str() == Some(mine) {
            key = Some(k);
            break;
        }
    }
    let mut key = key.context("SUBROUTER_API_KEY is not a key of this account")?;
    let n = acct.set_lists(&mut key, &lists, false)?;
    println!("\nset on your key \"{}\": {n} models; other models and settings unchanged", key["name"].as_str().unwrap_or("?"));
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

    /// A second variable that is always set, standing in for a key beside PATH: HOME on Unix, USERPROFILE on Windows, which
    /// has no HOME on GitHub's runners (the tests that used HOME failed there, 2026-11-08).
    const SET: &str = if cfg!(windows) { "USERPROFILE" } else { "HOME" };

    #[test]
    fn a_route_falls_back_to_its_other_providers() {
        let f = std::env::temp_dir().join(format!("tazuna-fallbacks-{}.json", std::process::id()));
        std::fs::write(&f, r#"{"routes": {"m": {"url": "https://subrouter.ai/v1", "key_env": "ROUTE_KEY",
            "pinned": {"a6aa": {"key_env": "PATH", "price": [0.012, 0.048]}, "znu": {"key_env": "HOME", "price": [0.0118, 0.047]},
                       "gone": {"key_env": "TAZUNA_TEST_NO_SUCH_KEY", "price": [0.001, 0.01]}}}}}"#.replace("HOME", SET)).unwrap();
        assert_eq!(fallbacks(&f, "ROUTE_KEY"), format!("https://subrouter.ai/v1 {SET}; https://subrouter.ai/v1 PATH"), "cheapest first, unset keys left out");
        assert_eq!(fallbacks(&f, "PATH"), format!("https://subrouter.ai/v1 {SET}"), "a pinned provider falls back to the others");
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
        let cs: Vec<Choice> = catalog(&f, |_| true, &none, 0).into_iter().filter(|c| c.route.is_some() && c.place != "Mortera").collect();
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
        let why = why_not(&c).unwrap();
        assert!(why.contains("tazuna subrouter --manage") && why.contains("/login, or set SUBROUTER_API_KEY"), "both ways: {why}");
        c.state = State::Untrusted;
        let why = why_not(&c).unwrap();
        assert!(why.contains("add the line subrouter.ai to") && why.contains("trusted-providers"), "{why}");
        c.state = State::Unchecked;
        assert_eq!(why_not(&c), None);
        std::fs::remove_file(&f).ok();
    }

    fn catalog_claude_row() -> Choice {
        Choice { model: "claude-opus-5-5".into(), label: "Opus 5.5".into(), route: None, place: "Claude · subscription".into(), efforts: vec![],
                 price: "plan".into(), note: String::new(), stale: false, state: State::Ready, bench: String::new(), aa: String::new(), pins: vec![] }
    }

    #[test]
    fn mortera_rows_are_its_text_models() {
        let list = serde_json::json!({"data": [
            {"id": "deepseek-v4.1-flash-1017-uncensored", "architecture": {"output_modalities": ["text"]}, "pricing": {"payg_per_1m_idr": {"in": 391, "out": 1630, "cache": 39}}},
            {"id": "muse-spark-1.3", "pricing": {"payg_per_1m_idr": {"in": 1630, "out": 3260}}},
            {"id": "gpt-image-2", "architecture": {"output_modalities": ["image"]}, "pricing": {"payg_per_1m_idr": {"in": 1, "out": 1}}},
            {"id": "claude-opus-5-5", "pricing": {"payg_per_1m_idr": {"in": 1, "out": 1}}},
            {"id": "text-embedding-3-large", "pricing": {"payg_per_1m_idr": {"in": 1, "out": 1}}},
            {"id": "no-price"}]});
        let rows = mortera_choices(&list, &serde_json::json!({}), &|h: &str| h == "mortera.cloud");
        assert_eq!(rows.iter().map(|c| c.model.as_str()).collect::<Vec<_>>(), ["deepseek-v4.1-flash-1017-uncensored", "muse-spark-1.3"]);
        let order: Vec<String> = providers(&[rows[0].clone(), catalog_claude_row()]).into_iter().map(|(n, _)| n).collect();
        assert_eq!(order, ["Claude", "Mortera"], "Claude first in /model's providers");
        let r = rows[1].route.as_ref().unwrap();
        assert_eq!((r.0.as_str(), r.1.as_str(), r.2.as_str()), ("https://mortera.cloud/v1", "MORTERA_API_KEY", "0.1,0.01,0.2"), "rupiah at 16,300 to the dollar");
        assert!(rows[0].note.starts_with("not checked yet"));
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
                           "kriora": {"key_env": "HOME", "price": [0.0387, 0.1935]}}}}}"#.replace("HOME", SET)).unwrap();
        let now = 1000000 + 9 * 86_400;
        let mut d: Value = serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        d["catalog"] = serde_json::json!({"glm-5.3": {"price": [0.1, 0.4], "providers": 3}, "seadance-video": {"price": [1, 1], "providers": 1},
                                          "gpt-6.1-sol": {"price": [0.03, 0.2], "providers": 40}});
        d["aa"] = serde_json::json!({"glm-5-3": 41.0, "gpt-6-1-sol": 54.5});
        d["bench"] = serde_json::json!({"gpt-6.1-sol": {"judge": 6.6, "runs": 51, "minutes": 19}});
        std::fs::write(&f, d.to_string()).unwrap();
        let c: Vec<Choice> = catalog(&f, |h| h == "subrouter.ai", &KeyModels::new(), now).into_iter().filter(|c| c.place != "Mortera").collect();
        assert_eq!(c.iter().map(|c| c.label.as_str()).collect::<Vec<_>>(),
                   ["Opus 5.5", "Sonnet 5.5", "Haiku 5.5", "Fable 5.2", "deepseek-v4.1-flash-1017", "deepseek-v4.1-flash-1017 · fast", "gpt-6.1-sol", "glm-5.3"],
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
        assert_eq!((k.label.as_str(), k.route.as_ref().unwrap().1.as_str(), k.state), ("gpt-6.1-sol · kriora", SET, State::Ready));
        let s = pinned(&c[5], 2);
        assert_eq!((s.price.as_str(), s.route.as_ref().unwrap().2.as_str(), s.state), ("0.05 / 0.25", "0.05,0.005,0.25", State::NoKey));
        assert_eq!(pinned(&c[4], 1).label, c[4].label, "no pins: the route as it is");
        std::fs::remove_dir_all(&dir).ok();
    }
}
