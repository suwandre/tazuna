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

/// SubRouter's public price list (/api/pricing) as the catalog routes.json carries: per model its cheapest provider's
/// input and output price, the provider count, and "chat" for a model that answers chat requests (by the endpoint
/// types it lists: no image, video, speech or embedding endpoint). None for an empty or unreadable list.
fn subrouter_catalog(list: &Value) -> Option<Value> {
    const NOT_CHAT: [&str; 6] = ["openai-video", "image-generation", "image-edit", "audio-speech", "embeddings", "gemini-embedding"];
    let mut out = serde_json::Map::new();
    for m in list["data"].as_array()? {
        let (Some(name), Some(ps)) = (m["model_name"].as_str(), m["provider_prices"].as_array()) else { continue };
        if ps.is_empty() {
            continue;
        }
        let low = |k: &str| ps.iter().filter_map(|p| p[k].as_f64()).fold(f64::INFINITY, f64::min);
        let types: Vec<&str> = m["supported_endpoint_types"].as_array().into_iter().flatten().filter_map(|t| t.as_str()).collect();
        let chat = types.iter().any(|t| ["openai", "anthropic"].contains(t)) && !types.iter().any(|t| NOT_CHAT.contains(t));
        out.insert(name.to_string(), serde_json::json!({"price": [low("input_price"), low("output_price")], "providers": ps.len(),
                                                       "category": if chat { "chat" } else { "other" }}));
    }
    (!out.is_empty()).then_some(Value::Object(out))
}

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
    if on_omniroute(c) {
        return "OmniRoute".into();
    }
    if let Some(l) = c.route.as_ref().and_then(|r| LOCAL.iter().find(|l| l.1 == r.0)) {
        return l.0.into();
    }
    if let Some(l) = login_of(c) {
        return l.name.into();
    }
    match c.route.as_ref() {
        None => "Claude".into(),
        // a machine of the user's (an address, not a name): the route's own name for it ("qwen3.8-flash-next@pc": pc)
        Some(r) if matches!(host_of(&r.0), "localhost") || host_of(&r.0).parse::<std::net::IpAddr>().is_ok() => {
            c.label.rsplit_once(" · ").map_or(host_of(&r.0).to_string(), |(_, n)| n.to_string())
        }
        Some(r) => provider_name(&r.0, &r.1),
    }
}

/// A provider by its API's address and key variable: "SubRouter" for https://subrouter.ai/v1 (the run log records it since
/// 0.1.182, so the usage page can say where an API run ran), "MiniMax Plan" for MiniMax's URL with the plan's key (the URL
/// alone named it "MiniMax", and Fire Pass "Fireworks").
pub fn provider_name(url: &str, var: &str) -> String {
    if url == OMNIROUTE {
        return "OmniRoute".into();
    }
    let url = url.trim_end_matches('/');
    if let Some(l) = LOCAL.iter().find(|l| l.1 == url) {
        return l.0.into();
    }
    // the provider whose API and key these are, else whose host it is
    login_at(url, var).or_else(|| LOGINS.iter().find(|l| l.host() == host_of(url)))
        .map_or_else(|| host_of(url).to_string(), |l| l.name.to_string())
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
    let rank = |n: &str| ["Claude", "SubRouter", "Mortera", "Mistral", "OpenRouter", "Kenari", "Cheaper Inference", "OmniRoute"].iter().position(|x| *x == n).unwrap_or(9);
    // some 70 since 0.1.196: Claude, the ones with a key (or on this machine), then the rest, each part in that order
    let keyless = |is: &[usize]| is.iter().all(|&i| cs[i].state == State::NoKey);
    out.sort_by_key(|(n, is)| (rank(n) > 0, keyless(is), rank(n)));
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

/// Who a turn on this API is paid to, for its receipt: the provider's name, or None where a turn pays nobody per token (a
/// machine of the user's, or OmniRoute's accounts, which are subscriptions).
pub fn paid_to(url: &str, var: &str) -> Option<String> {
    (url != OMNIROUTE && !own_machine(host_of(url))).then(|| provider_name(url, var))
}

/// The variables that, set where claude runs, bill its runs elsewhere than the Claude plan: tazuna writes the ones it sees
/// set into claude's init (`tazuna_env`, the names alone) for `claude_payer`.
pub const CLAUDE_BILLING_ENV: [&str; 5] = ["CLAUDE_CODE_USE_BEDROCK", "CLAUDE_CODE_USE_VERTEX", "ANTHROPIC_BASE_URL", "ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_API_KEY"];

/// Whether a claude run is on the Claude plan, and who is paid when it is not (None: not known): from its init's
/// `apiKeySource` ("none": the subscription's login) and which of CLAUDE_BILLING_ENV are `set` where it runs. The plan only
/// when nothing says otherwise: "none" read as the plan under Bedrock, Vertex or a gateway too, and an API key's run counted
/// as plan in the header and the run log (design review 12). The session (receipt, header) and the run log (usage and start
/// pages) ask this alone.
#[inline(never)]
pub fn claude_payer(key_source: Option<&str>, set: impl Fn(&str) -> bool) -> (bool, Option<&'static str>) {
    if set("CLAUDE_CODE_USE_BEDROCK") {
        (false, Some("Bedrock"))
    } else if set("CLAUDE_CODE_USE_VERTEX") {
        (false, Some("Vertex"))
    } else if set("ANTHROPIC_BASE_URL") || set("ANTHROPIC_AUTH_TOKEN") {
        (false, None)
    } else if key_source.map_or(set("ANTHROPIC_API_KEY"), |k| !k.is_empty() && k != "none") {
        (false, Some("Anthropic API"))
    } else {
        (true, None)
    }
}

/// A host that is the user's own: this machine, one on their network (a private, link-local or carrier-shared address,
/// IPv6's too; a `.local`, `.lan` or `.internal` name, a Tailscale one; a name without a dot), so a turn there pays nobody.
/// It named `gpu-box.local` and `[` (of `[::1]`) as providers (design review 11), and `gpu.tail1234.ts.net` (12).
fn own_machine(host: &str) -> bool {
    use std::net::IpAddr;
    let h = host.to_ascii_lowercase();
    let v4 = |a: std::net::Ipv4Addr| a.is_loopback() || a.is_private() || a.is_link_local() || a.is_unspecified() || (a.octets()[0] == 100 && a.octets()[1] & 0xc0 == 64);
    match h.parse::<IpAddr>() {
        Ok(IpAddr::V4(a)) => v4(a),
        Ok(IpAddr::V6(a)) => a.is_loopback() || a.is_unspecified() || a.segments()[0] & 0xfe00 == 0xfc00 || a.segments()[0] & 0xffc0 == 0xfe80 || a.to_ipv4_mapped().is_some_and(v4),
        Err(_) => !h.contains('.') || [".local", ".lan", ".internal", ".localhost", ".home.arpa", ".ts.net"].iter().any(|s| h.ends_with(s)),
    }
}

/// A URL's host: without the scheme, a user name, the port and the path; an IPv6 address without its brackets.
fn host_of(url: &str) -> &str {
    let rest = url.split("://").nth(1).unwrap_or(url).split(['/', '?', '#']).next().unwrap_or_default();
    let rest = rest.rsplit('@').next().unwrap_or(rest);
    match rest.strip_prefix('[') {
        Some(v6) => v6.split(']').next().unwrap_or_default(),
        None => rest.split(':').next().unwrap_or_default(),
    }
}

/// Claude's models, then the checked routes, then the unchecked SubRouter models that have a public score; `trusted` says
/// whether a provider host may see the code, `keys` which models a key may use.
pub fn catalog(routes: &Path, trusted: impl Fn(&str) -> bool, keys: &KeyModels, now: u64) -> Vec<Choice> {
    SAVED.set(Some(saved_keys()));
    let out = catalog_rows(routes, trusted, keys, now);
    SAVED.set(None);
    out
}

fn catalog_rows(routes: &Path, trusted: impl Fn(&str) -> bool, keys: &KeyModels, now: u64) -> Vec<Choice> {
    let (mut file, builtin) = routes_file(routes);
    // SubRouter's live price list, when tazuna has fetched it, over the copy from the release or the bench
    if let Some(c) = routes.parent().and_then(|d| std::fs::read_to_string(d.join("subrouter-pricing.json")).ok())
        .and_then(|t| subrouter_catalog(&serde_json::from_str(&t).unwrap_or_default())) {
        file["catalog"] = c;
    }
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
    // OpenRouter's list (2026-11-11), from the copy `refresh_openrouter` keeps beside the routes file
    if let Some(list) = routes.parent().and_then(|d| std::fs::read_to_string(d.join("openrouter-models.json")).ok()) {
        out.extend(openrouter_choices(&serde_json::from_str(&list).unwrap_or_default(), &file, &trusted));
    }
    // kenari's list (2026-11-11), the same way (`refresh_kenari`)
    if let Some(list) = routes.parent().and_then(|d| std::fs::read_to_string(d.join("kenari-models.json")).ok()) {
        out.extend(kenari_choices(&serde_json::from_str(&list).unwrap_or_default(), &file, &trusted));
    }
    // CheaperInference's list (2026-11-16), kept like Mortera's (it needs the key)
    if let Some(list) = routes.parent().and_then(|d| std::fs::read_to_string(d.join("cheaperinference-models.json")).ok()) {
        out.extend(cheaper_choices(&serde_json::from_str(&list).unwrap_or_default(), &file, &trusted));
    }
    // OmniRoute's, on this machine (2026-11-17), the same way
    if let Some(list) = routes.parent().and_then(|d| std::fs::read_to_string(d.join("omniroute-models.json")).ok()) {
        out.extend(omniroute_choices(&serde_json::from_str(&list).unwrap_or_default(), &file, &trusted));
    }
    // every other SubRouter model with a public score (the list also holds image and video models), best first. One model
    // once: SubRouter lists some under two spellings ("claude-opus-5.5" beside the subscription's "claude-opus-5-5",
    // "DeepSeek-V4-Flash-0731" beside "deepseek-v4-flash-0731"), and a pick of the copy ran Opus through SubRouter instead
    // of the plan (a 403 on a key without it, 2026-11-08); the lower-case spelling stays
    // a model on another provider (Mortera, Mistral) does not hide SubRouter's own row for it
    let mut seen: std::collections::HashSet<String> =
        out.iter().filter(|c| c.route.as_ref().is_none_or(|r| host_of(&r.0) == "subrouter.ai")).map(|c| slug(&c.model)).collect();
    // every chat model, by SubRouter's own category (image, video, audio and embedding models stay out), those with a
    // public score first; a list from before categories (no "category") keeps only the scored ones. Until 0.1.117 only the
    // scored ones were listed: about 20 of SubRouter's 311 chat models (2026-11-12)
    let mut rest: Vec<(f64, &String, &Value)> = file["catalog"].as_object().into_iter().flatten()
        .filter_map(|(m, c)| {
            let aa = file["aa"][slug(m)].as_f64();
            (c["category"] == "chat" || aa.is_some()).then_some((aa.unwrap_or(f64::NEG_INFINITY), m, c))
        }).collect();
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
    // the providers OMP lists (2026-11-22): models.dev's catalog for most (the data folder's copy, else the built-in one),
    // their own /models for a few, then the servers found on this machine at start
    let dir = routes.parent();
    out.extend(modelsdev_choices(&modelsdev_tsv(dir), &file, &trusted));
    for l in LOGINS.iter().filter(|l| l.source == Source::List) {
        match dir.and_then(|d| std::fs::read_to_string(d.join(list_file(l))).ok()) {
            Some(list) => out.extend(list_choices(l, &serde_json::from_str(&list).unwrap_or_default(), &file, &trusted)),
            // no key, so no list: one greyed row, so the provider is in /model with what it needs (as Mortera's)
            None if l.vars.iter().all(|v| self::key(v).is_none()) => out.push(Choice { model: list_file(l).trim_end_matches(".json").to_string(),
                label: format!("{}'s models", l.name), note: format!("tazuna loads {}'s model list at start once {} is set", l.name, l.vars[0]),
                ..login_row(l, "", None, vec![], &file, &trusted) }),
            None => {}
        }
    }
    if let Some(list) = dir.and_then(|d| std::fs::read_to_string(d.join(LOCAL_FILE)).ok()) {
        out.extend(local_choices(&serde_json::from_str(&list).unwrap_or_default(), &file));
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
        let rp = crate::engine::mortera_idr(m);
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

/// OpenRouter's models as picker rows: those that take tools (an agent needs them), without Claude (it runs on the
/// subscription) and without the `:batch` copies. Newest free models first, then the newest paid ones (the user's order,
/// 2026-11-11). Free ones are rate-limited by OpenRouter and their providers may keep or train on prompts: their note
/// says so. Prices from the list, per token in USD.
fn openrouter_choices(list: &Value, file: &Value, trusted: &impl Fn(&str) -> bool) -> Vec<Choice> {
    let mut rows: Vec<(bool, i64, Choice)> = list["data"].as_array().into_iter().flatten().filter_map(|m| {
        let id = m["id"].as_str()?;
        let params: Vec<&str> = m["supported_parameters"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        if id.starts_with("anthropic/") || id.ends_with(":batch") || !params.contains(&"tools") {
            return None;
        }
        let per_m_usd = |k: &str| m["pricing"][k].as_str().and_then(|p| p.parse::<f64>().ok()).map(|p| p * 1e6);
        let (input, output) = (per_m_usd("prompt")?, per_m_usd("completion")?);
        let free = input == 0.0 && output == 0.0;
        let efforts = if params.iter().any(|p| *p == "reasoning" || *p == "reasoning_effort") { LEVELS.iter().map(|e| e.to_string()).collect() } else { vec![] };
        Some((free, m["created"].as_i64().unwrap_or(0), Choice {
            model: id.to_string(),
            label: id.to_string(),
            route: Some(("https://openrouter.ai/api/v1".into(), "OPENROUTER_API_KEY".into(), route_price(input, output, per_m_usd("input_cache_read")))),
            place: "OpenRouter".into(),
            efforts,
            price: format!("{} / {}", per_m(input), per_m(output)),
            note: if free {
                "free on OpenRouter: rate-limited (a few dozen calls a minute and a daily cap, which one agent task can use up), and its providers may keep or train on your prompts".into()
            } else {
                "not checked yet: prices from OpenRouter's list; effort levels, cache and quality not measured".into()
            },
            stale: false,
            state: if key("OPENROUTER_API_KEY").is_none() {
                State::NoKey
            } else if !trusted("openrouter.ai") {
                State::Untrusted
            } else {
                State::Ready
            },
            bench: scores(file, id.rsplit('/').next().unwrap_or(id)).0,
            aa: scores(file, id.rsplit('/').next().unwrap_or(id)).1,
            pins: vec![],
        }))
    }).collect();
    rows.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    rows.into_iter().map(|(_, _, c)| c).collect()
}

/// kenari's chat models that take tools, as picker rows (a gateway in Indonesia, the user's pick, 2026-11-11): one key and
/// balance for many providers, the OpenAI and Anthropic APIs both. Free ones first, then the paid ones, each in kenari's
/// order (its list has no dates); Claude too, billed by kenari, for a user without the subscription (the user, 2026-11-11).
/// Left out: image, voice and video models, and chat models without tool calls (an agent cannot work without them).
/// Prices are micro-rupiah per 1M tokens; the effort levels are each model's own.
fn kenari_choices(list: &Value, file: &Value, trusted: &impl Fn(&str) -> bool) -> Vec<Choice> {
    let mut rows: Vec<(bool, Choice)> = list["data"].as_array().into_iter().flatten().filter_map(|m| {
        let id = m["id"].as_str()?;
        let chat = m["endpoints"].as_array().is_some_and(|e| e.iter().any(|e| e == "chat"));
        if !chat || m["tool_call"] != true {
            return None;
        }
        let p = &m["pricing"];
        let usd = |k: &str| p[k].as_f64().map(|v| v / 1e6 / crate::engine::IDR_PER_USD);
        let free = p["free"] == true;
        // a free model's prices are null, not 0
        let (input, output) = if free { (usd("input").unwrap_or(0.0), usd("output").unwrap_or(0.0)) } else { (usd("input")?, usd("output")?) };
        let efforts = m["supported_reasoning_levels"].as_array().into_iter().flatten().filter_map(Value::as_str).map(String::from).collect();
        Some((free, Choice {
            model: id.to_string(),
            label: m["name"].as_str().filter(|n| !n.is_empty() && *n != id).map_or(id.to_string(), |n| format!("{n} · {id}")),
            route: Some(("https://kenari.id/v1".into(), "KENARI_API_KEY".into(), route_price(input, output, usd("cache_read")))),
            place: "Kenari".into(),
            efforts,
            price: if free { "free".into() } else { format!("{} / {}", per_m(input), per_m(output)) },
            note: if free {
                "free on kenari: 5 calls a minute and 50 a day without a top-up (one agent task can use that up), 1,000 a day after a Rp 10,000 top-up".into()
            } else {
                "not checked yet: prices from kenari's list (rupiah); cache and quality not measured".into()
            },
            stale: false,
            state: if key("KENARI_API_KEY").is_none() {
                State::NoKey
            } else if !trusted("kenari.id") {
                State::Untrusted
            } else {
                State::Ready
            },
            bench: scores(file, id).0,
            aa: scores(file, id).1,
            pins: vec![],
        }))
    }).collect();
    rows.sort_by(|a, b| b.0.cmp(&a.0)); // stable: kenari's own order within free and paid
    rows.into_iter().map(|(_, c)| c).collect()
}

/// OmniRoute's endpoint: a gateway the user runs on this machine.
const OMNIROUTE: &str = "http://localhost:20128/v1";

/// OmniRoute's model list into the data folder, each time the picker opens (and at start): it is on this machine and
/// answers in well under a second, and what it lists follows the accounts the user logs in and out of there (a list
/// read only at start showed Antigravity's models after a switch to Antigravity CLI, 2026-11-17). No key, or OmniRoute
/// not running: nothing, at once.
pub fn refresh_omniroute() {
    let Some(k) = key("OMNIROUTE_API_KEY") else { return };
    let http: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(3))).build().into();
    let Ok(mut r) = http.get(&format!("{OMNIROUTE}/models")).header("Authorization", format!("Bearer {}", k.trim())).call() else { return };
    let Ok(body) = r.body_mut().with_config().limit(20 << 20).read_to_string() else { return };
    let ok = serde_json::from_str::<Value>(&body).is_ok_and(|v| v["data"].is_array());
    if let (true, Ok(dir)) = (ok, crate::labels::dir()) {
        let tmp = dir.join("omniroute-models.json.tmp");
        if std::fs::write(&tmp, body).is_ok() {
            let _ = std::fs::rename(&tmp, dir.join("omniroute-models.json"));
        }
    }
}

/// Effort levels as OmniRoute spells them at the end of a model's id (agy/gemini-4-argon-high), lowest first.
const TIERS: [&str; 6] = ["minimal", "low", "medium", "high", "xhigh", "max"];

/// An OmniRoute id split into the model and its level: agy/gemini-4-argon-high → (agy/gemini-4-argon, high).
pub fn tier_of(id: &str) -> Option<(&str, &str)> {
    id.rsplit_once('-').filter(|(_, t)| TIERS.contains(t))
}

/// A row on OmniRoute's endpoint.
pub fn on_omniroute(c: &Choice) -> bool {
    c.route.as_ref().is_some_and(|r| r.0 == OMNIROUTE)
}

/// The id a pick runs: an OmniRoute model with levels runs as its id for that level (Gemini 4 Argon at high:
/// agy/gemini-4-argon-high); every other row as its own id.
pub fn resolve(c: &Choice, effort: &str) -> String {
    if on_omniroute(c) && c.efforts.iter().any(|e| e == effort) { format!("{}-{effort}", c.model) } else { c.model.clone() }
}

/// Whether `model` (what the session runs) is this row: its id, or one of its levels' ids on OmniRoute.
pub fn same_model(c: &Choice, model: &str) -> bool {
    c.model == model || (on_omniroute(c) && tier_of(model).is_some_and(|(base, t)| base == c.model && c.efforts.iter().any(|e| e == t)))
}

/// A model's name from its id: claude-opus-5-5 → Claude Opus 5.5, gemini-4-argon → Gemini 4 Argon, gpt-oss-120b →
/// GPT OSS 120B.
fn display_name(id: &str) -> String {
    let mut out: Vec<String> = vec![];
    for w in id.split('-').filter(|w| !w.is_empty()) {
        let digit = |x: &str| x.len() == 1 && x.chars().all(|c| c.is_ascii_digit());
        // 5-5 is version 5.5
        if digit(w) && out.last().is_some_and(|l| l.chars().all(|c| c.is_ascii_digit() || c == '.') && !l.is_empty()) {
            let l = out.pop().unwrap_or_default();
            out.push(format!("{l}.{w}"));
            continue;
        }
        out.push(match w {
            "gpt" | "oss" | "glm" | "ai" | "vl" | "lm" => w.to_ascii_uppercase(),
            _ if w.chars().any(|c| c.is_ascii_digit()) && w.chars().any(|c| c.is_ascii_alphabetic()) => w.to_ascii_uppercase(),
            _ => w.chars().enumerate().map(|(i, c)| if i == 0 { c.to_ascii_uppercase() } else { c }).collect(),
        });
    }
    out.join(" ")
}

/// Where an OmniRoute model runs, by its owner (or its id's first part): the account or service connected there.
fn omniroute_source(owner: &str) -> String {
    match owner {
        "agy" => "Antigravity CLI",
        "antigravity" => "Antigravity",
        "claude" | "cc" => "your Claude subscription, not through the claude CLI",
        "codex" | "cx" => "Codex",
        "codex-app-server" | "cxa" => "Codex app server",
        "devin-cli-agentic" | "devin-cli" | "dva" | "dv" => "Devin CLI",
        "auggie" | "aug" => "Augment",
        "cloudflare-playground" | "cfp" => "Cloudflare Playground",
        "zcode" | "zc" => "Z Code",
        "uncloseai" | "unc" => "UncloseAI",
        "github" | "gh" => "GitHub Copilot",
        "kiro" | "kr" => "Kiro",
        "cursor" | "cu" => "Cursor",
        "kimi-coding" | "kmc" => "Kimi Code",
        "combo" | "auto" => "a combo: OmniRoute picks",
        other => other,
    }.to_string()
}

/// OmniRoute's models as picker rows: the accounts and API keys the user connected there, one endpoint. One row a model,
/// named (Gemini 4 Argon), placed (OmniRoute · Antigravity CLI), its levels as efforts where OmniRoute lists one id per
/// level (agy/gemini-4-argon-high, -medium, -low): until 0.1.172 each level was a row of its own, and tazuna's effort
/// on top read "argon-high at medium" (2026-11-18). Every route shows: the user chooses (ROADMAP's decision log, 2026-11-17).
/// tazuna itself still runs Claude only through the official CLI; OmniRoute's Claude Code route (provider claude, alias
/// cc) runs the subscription through OmniRoute's own client, and its rows say so. What a call costs is OmniRoute's to know
/// (a subscription reads $0 there, an upstream API key bills): tazuna counts it as free, so the spend cap does not see it.
fn omniroute_choices(list: &Value, file: &Value, trusted: &impl Fn(&str) -> bool) -> Vec<Choice> {
    // the Claude subscription's routes, by owner and by any part of the id (OmniRoute also lists no-think/cc/claude-...)
    let claude = |m: &Value| {
        let id = m["id"].as_str().unwrap_or_default().to_ascii_lowercase();
        let owner = m["owned_by"].as_str().unwrap_or_default();
        id.split('/').any(|p| p == "cc" || p == "claude") || owner == "claude" || owner == "cc"
    };
    // each model the agent can use, by its id without a level: (owner, its levels, whether the id without one is served too)
    let mut groups: Vec<(String, String, Vec<String>, bool)> = vec![];
    for m in list["data"].as_array().into_iter().flatten() {
        let Some(id) = m["id"].as_str() else { continue };
        // an agent needs text out and tool calls: image and video models (179 of 733 on 2026-11-17) are not for it
        let other_kind = m["kind"].as_str().or(m["type"].as_str()).is_some_and(|k| !matches!(k, "chat" | "text" | "llm" | "model"))
            || m["output_modalities"].as_array().is_some_and(|o| !o.iter().any(|x| x == "text"))
            || m["capabilities"]["tool_calling"] == false;
        if other_kind {
            continue;
        }
        let owner = if claude(m) { "claude".to_string() } else { m["owned_by"].as_str().unwrap_or_default().to_string() };
        let (base, tier) = tier_of(id).map_or((id, None), |(b, t)| (b, Some(t)));
        match groups.iter_mut().find(|g| g.0 == base) {
            Some(g) => match tier {
                Some(t) => g.2.push(t.to_string()),
                None => g.3 = true,
            },
            None => groups.push((base.to_string(), owner, tier.map(String::from).into_iter().collect(), tier.is_none())),
        }
    }
    // OmniRoute lists the Claude subscription twice (cc/ and claude/, its alias and its id): one row a model
    let mut seen = std::collections::HashSet::new();
    groups.retain(|g| {
        let first = g.0.split('/').next().unwrap_or_default();
        let rest = g.0.trim_start_matches(&format!("{first}/"));
        !(g.1 == "claude" && (first == "claude" || first == "cc")) || seen.insert(rest.to_string())
    });
    let mut rows: Vec<Choice> = groups.into_iter().map(|(base, owner, mut tiers, bare)| {
        tiers.sort_by_key(|t| TIERS.iter().position(|x| x == t));
        tiers.dedup();
        // levels only where there are two: one level alone is just the model (and the id without one stays its own model)
        let levelled = tiers.len() >= 2;
        let model = if levelled || bare || tiers.is_empty() { base.clone() } else { format!("{base}-{}", tiers[0]) };
        let parts: Vec<&str> = base.split('/').collect();
        let combo = owner == "combo" || parts.first() == Some(&"auto");
        let no_think = parts.first() == Some(&"no-think");
        let source = omniroute_source(if owner.is_empty() { parts.first().copied().unwrap_or_default() } else { &owner });
        let sub = owner == "claude";
        let name = parts.last().copied().unwrap_or(&base);
        Choice {
            label: if combo { format!("auto: {name}") } else { format!("{}{}", display_name(name), if no_think { " (no thinking)" } else { "" }) },
            model,
            route: Some((OMNIROUTE.into(), "OMNIROUTE_API_KEY".into(), route_price(0.0, 0.0, None))),
            place: format!("OmniRoute · {source}"),
            efforts: if levelled { tiers } else { vec![] },
            price: "via OmniRoute".into(),
            note: if sub {
                "your Claude subscription through OmniRoute's own client, not the official claude CLI: outside Anthropic's terms for the subscription, and accounts have been suspended for it; tazuna's own Claude rows use the CLI".into()
            } else {
                "through OmniRoute on this machine: it picks the account or key behind the model; prices and quality not measured, and the spend cap does not count it".into()
            },
            stale: false,
            state: if key("OMNIROUTE_API_KEY").is_none() {
                State::NoKey
            } else if !trusted("localhost") {
                State::Untrusted
            } else {
                State::Ready
            },
            bench: scores(file, name).0,
            aa: scores(file, name).1,
            pins: vec![],
        }
    }).collect();
    // combos first (OmniRoute's own choice among the rest), then by where they run and by name
    rows.sort_by_key(|c| (!c.label.starts_with("auto: "), c.place.clone(), c.label.clone()));
    rows
}

/// CheaperInference's text models that take tools, as picker rows (a gateway: 266 of its 573 models on 2026-11-16), by
/// name, without Claude (it runs on the subscription). Prices from its list, USD per million (strings); each model with
/// reasoning gets the effort levels. Free ones say so.
fn cheaper_choices(list: &Value, file: &Value, trusted: &impl Fn(&str) -> bool) -> Vec<Choice> {
    let mut rows: Vec<Choice> = list["data"].as_array().into_iter().flatten().filter_map(|m| {
        let id = m["id"].as_str()?;
        let tools = m["features"].as_array().is_some_and(|f| f.iter().any(|x| x == "tool-use"));
        if m["type"] != "text" || !tools || id.starts_with("claude-") {
            return None;
        }
        let p = &m["pricing"];
        let usd = |k: &str| p[k].as_str().and_then(|v| v.parse::<f64>().ok()).or(p[k].as_f64());
        let (input, output) = (usd("input_per_million")?, usd("output_per_million")?);
        let free = m["is_free"] == true || (input == 0.0 && output == 0.0);
        Some(Choice {
            model: id.to_string(),
            label: id.to_string(),
            route: Some(("https://api.cheaperinference.com/v1".into(), "CHEAPER_INFERENCE_API_KEY".into(), route_price(input, output, usd("cache_read_input_per_million")))),
            place: format!("Cheaper Inference · {}", m["provider"].as_str().unwrap_or("")).trim_end_matches(" · ").to_string(),
            efforts: if m["capabilities"]["reasoning"] == true { LEVELS.iter().map(|e| e.to_string()).collect() } else { vec![] },
            price: if free { "free".into() } else { format!("{} / {}", per_m(input), per_m(output)) },
            note: "not checked yet: prices from Cheaper Inference's list; effort levels, cache and quality not measured".into(),
            stale: false,
            state: if key("CHEAPER_INFERENCE_API_KEY").is_none() {
                State::NoKey
            } else if !trusted("api.cheaperinference.com") {
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

/// At start: OpenRouter's and kenari's model lists and SubRouter's price list (public, no key needed) into the data
/// folder, where `/model` reads them (a slow or absent answer leaves the last copy). SubRouter's came only with a
/// release until 0.1.118: a model it added, or a provider's new price, waited for the next tazuna.
pub fn refresh_openrouter() {
    for (url, name) in [("https://openrouter.ai/api/v1/models", "openrouter-models.json"), ("https://kenari.id/v1/models", "kenari-models.json"),
                        ("https://subrouter.ai/api/pricing", "subrouter-pricing.json")] {
        let http: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(15))).build().into();
        let Ok(mut r) = http.get(url).call() else { continue };
        let Ok(body) = r.body_mut().with_config().limit(40 << 20).read_to_string() else { continue };
        let ok = serde_json::from_str::<Value>(&body).is_ok_and(|v| v["data"].as_array().is_some_and(|d| !d.is_empty()));
        if let (true, Ok(dir)) = (ok, crate::labels::dir()) {
            let tmp = dir.join(format!("{name}.tmp"));
            if std::fs::write(&tmp, body).is_ok() {
                let _ = std::fs::rename(&tmp, dir.join(name));
            }
        }
    }
    // models.dev's catalog after the price lists: a slow models.dev held them up for up to 60 s (review 11)
    refresh_modelsdev();
}

/// At start, for each provider whose list needs its key and has one set (Mortera, CheaperInference, the `Source::List`
/// ones): its model list into the data folder, where `/model` reads it (a slow or absent answer leaves the last copy).
pub fn refresh_mortera() {
    refresh_omniroute();
    refresh_lists(&[]);
}

/// The model lists that need a key, each whose variable is in `only` (all of them when empty): a key just saved fetches
/// its provider's list alone.
pub fn refresh_lists(only: &[&str]) {
    let lists = LOGINS.iter().filter(|l| l.source == Source::List).map(|l| (l.vars[0], format!("{}/models", l.url), list_file(l)));
    for (var, url, name) in [("MORTERA_API_KEY", "https://mortera.cloud/v1/models".to_string(), "mortera-models.json".to_string()),
                             ("CHEAPER_INFERENCE_API_KEY", "https://api.cheaperinference.com/v1/models".into(), "cheaperinference-models.json".into())].into_iter().chain(lists)
                             .filter(|(var, ..)| only.is_empty() || only.contains(var)) {
        let Some(k) = key(var) else { continue };
        let http: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(15))).build().into();
        let Ok(mut r) = http.get(&url).header("Authorization", format!("Bearer {}", k.trim())).call() else { continue };
        let Ok(body) = r.body_mut().with_config().limit(20 << 20).read_to_string() else { continue };
        let ok = serde_json::from_str::<Value>(&body).is_ok_and(|v| v["data"].as_array().is_some_and(|d| !d.is_empty()));
        if let (true, Ok(dir)) = (ok, crate::labels::dir()) {
            let tmp = dir.join(format!("{name}.tmp"));
            if std::fs::write(&tmp, body).is_ok() {
                let _ = std::fs::rename(&tmp, dir.join(name));
            }
        }
    }
}

/// Where a `Source::List` provider's /models is kept in the data folder.
fn list_file(l: &Login) -> String {
    format!("{}-models.json", l.name.to_lowercase().replace(' ', "-"))
}

/// models.dev's catalog (MIT), trimmed: `trim_modelsdev`'s lines for the providers in LOGINS. The built-in copy is the data
/// folder's modelsdev.tsv as a release found it (`gzip -9n < ~/.local/share/tazuna/modelsdev.tsv > src/modelsdev.tsv.gz`);
/// the data folder's is renewed at most once a day (`refresh_modelsdev`).
const MODELSDEV_URL: &str = "https://models.dev/api.json";
const MODELSDEV: &str = "modelsdev.tsv";
const MODELSDEV_BUILTIN: &[u8] = include_bytes!("modelsdev.tsv.gz");

/// A gzip file's deflate data (RFC 1952: the header and its optional fields skipped).
fn gz_data(b: &[u8]) -> Option<&[u8]> {
    if b.len() < 18 || b[..3] != [0x1f, 0x8b, 8] {
        return None;
    }
    let flags = b[3];
    let mut i = 10;
    if flags & 4 != 0 {
        i += 2 + u16::from_le_bytes([*b.get(i)?, *b.get(i + 1)?]) as usize;
    }
    for bit in [8, 16] {
        if flags & bit != 0 {
            i += b.get(i..)?.iter().position(|&c| c == 0)? + 1;
        }
    }
    if flags & 2 != 0 {
        i += 2;
    }
    b.get(i..)
}

/// A gzip file's contents (at most 64 MB).
fn gunzip(b: &[u8]) -> Option<Vec<u8>> {
    miniz_oxide::inflate::decompress_to_vec_with_limit(gz_data(b)?, 64 << 20).ok()
}

/// Deflate data read as it inflates: the refresh parses models.dev's catalog from it without holding the 6.5 MB it
/// inflates to (0.1.196's refresh peaked 13 MB over the start's, review 11).
struct Inflate<'a> {
    data: &'a [u8],
    state: Box<miniz_oxide::inflate::stream::InflateState>,
    end: bool,
}

impl std::io::Read for Inflate<'_> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        use miniz_oxide::{MZFlush, MZStatus, inflate::stream::inflate};
        while !self.end && !out.is_empty() {
            let r = inflate(&mut self.state, self.data, out, MZFlush::None);
            self.data = &self.data[r.bytes_consumed..];
            match r.status {
                Ok(MZStatus::StreamEnd) => self.end = true,
                Ok(_) if r.bytes_written > 0 => {}
                Ok(_) if r.bytes_consumed > 0 => continue,
                _ => return Err(std::io::Error::other("a broken or cut gzip stream")),
            }
            return Ok(r.bytes_written);
        }
        Ok(0)
    }
}

/// What the picker reads of a models.dev model; the rest of its entry is skipped as it is read.
#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct MdModel {
    tool_call: Value,
    reasoning: Value,
    reasoning_options: Value,
    modalities: Value,
    cost: Value,
}

/// models.dev's api.json, read from `r` as it comes: for each provider in LOGINS its lines (`model_lines`), made as soon as
/// the provider is read; the other providers are skipped unread. Holding every listed provider's models until the end
/// peaked 9 MB over the start's.
fn modelsdev_providers(r: &mut dyn std::io::Read) -> Option<std::collections::HashMap<String, String>> {
    use std::collections::{BTreeMap, HashMap};
    struct Listed(HashMap<String, String>);
    impl<'de> serde::Deserialize<'de> for Listed {
        fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            struct Each;
            impl<'de> serde::de::Visitor<'de> for Each {
                type Value = Listed;
                fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                    f.write_str("models.dev's providers")
                }
                fn visit_map<A: serde::de::MapAccess<'de>>(self, mut m: A) -> Result<Listed, A::Error> {
                    #[derive(serde::Deserialize, Default)]
                    #[serde(default)]
                    struct Provider {
                        models: BTreeMap<String, MdModel>,
                    }
                    let mut out = HashMap::new();
                    while let Some(id) = m.next_key::<String>()? {
                        if LOGINS.iter().any(|l| matches!(l.source, Source::ModelsDev(x) if x == id)) {
                            out.insert(id, model_lines(&m.next_value::<Provider>()?.models));
                        } else {
                            m.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                    Ok(Listed(out))
                }
            }
            d.deserialize_map(Each)
        }
    }
    serde_json::from_reader::<_, Listed>(std::io::BufReader::new(r)).ok().map(|l| l.0)
}

/// A provider's models as `trim_modelsdev` lines: each chat model that calls tools (Claude left out: it runs through the
/// claude CLI), with its input, output and cached-input price per M (empty where none is listed, or below 0 or above
/// $1,000: not a price) and its effort levels ("*": it reasons, levels not named).
fn model_lines(ms: &std::collections::BTreeMap<String, MdModel>) -> String {
    let mut out = String::new();
    for (m, v) in ms {
        let text = v.modalities["output"].as_array().is_none_or(|o| o.iter().any(|x| x == "text"));
        if v.tool_call != true || !text || m.to_lowercase().contains("claude") || m.contains(char::is_whitespace) {
            continue;
        }
        let n = |k: &str| v.cost[k].as_f64().filter(|x| price_ok(*x)).map(|x| x.to_string()).unwrap_or_default();
        let named: Vec<&str> = v.reasoning_options.as_array().into_iter().flatten().filter(|o| o["type"] == "effort")
            .flat_map(|o| o["values"].as_array().into_iter().flatten().filter_map(Value::as_str)).collect();
        let levels = if !named.is_empty() { named.join(" ") } else if v.reasoning == true { "*".into() } else { String::new() };
        out.push_str(&format!("{m}\t{}\t{}\t{}\t{levels}\n", n("input"), n("output"), n("cache_read")));
    }
    out
}

/// models.dev's api.json down to what the picker reads, for the providers in LOGINS: an "=id" line per provider, then its
/// `model_lines`. 6.5 MB becomes ~130 KB. None for a file that is not models.dev's (too few providers in it).
fn trim_modelsdev(r: &mut dyn std::io::Read) -> Option<String> {
    let all = modelsdev_providers(r)?;
    let mut out = String::new();
    let mut found = vec![];
    for l in LOGINS {
        let Source::ModelsDev(id) = l.source else { continue };
        let Some(lines) = all.get(id).filter(|_| !found.contains(&id)) else { continue };
        found.push(id);
        out.push_str(&format!("={id}\n{lines}"));
    }
    (found.len() >= 10).then_some(out)
}

/// A price per M tokens that can be one: none below 0 or above $1,000 (a negative one made every call earn money against
/// the caps).
fn price_ok(x: f64) -> bool {
    (0.0..=1000.0).contains(&x)
}

/// When a catalog copy was fetched (its "#at" line, Unix seconds; 0: an older copy without one).
fn fetched_at(tsv: &str) -> u64 {
    tsv.lines().take_while(|l| l.starts_with('#')).find_map(|l| l.strip_prefix("#at ")?.parse().ok()).unwrap_or(0)
}

/// The catalog /model reads: the data folder's copy where it is newer than the binary's own (after an upgrade the binary's
/// is), with every provider it lacks from the binary's (one renamed upstream vanished, review 11).
fn modelsdev_tsv(dir: Option<&Path>) -> String {
    let builtin = gunzip(MODELSDEV_BUILTIN).and_then(|b| String::from_utf8(b).ok()).unwrap_or_default();
    let Some(cache) = dir.and_then(|d| std::fs::read_to_string(d.join(MODELSDEV)).ok()).filter(|t| fetched_at(t) > fetched_at(&builtin)) else {
        return builtin;
    };
    let have: std::collections::HashSet<&str> = cache.lines().filter(|l| l.starts_with('=')).collect();
    let mut keep = false;
    let missing: String = builtin.lines().filter(|l| {
        if l.starts_with('=') {
            keep = !have.contains(l);
        }
        keep
    }).map(|l| format!("{l}\n")).collect();
    cache + &missing
}

/// models.dev's catalog into the data folder, at most once a day, in the background: with its ETag, so an unchanged
/// catalog answers 304 and costs nothing but the request; gzipped on the wire (~650 KB for 6.5 MB), inflated as it is
/// read. A failed or odd answer leaves the last copy, and without one the built-in.
pub fn refresh_modelsdev() {
    if let Ok(dir) = crate::labels::dir() {
        refresh_modelsdev_at(MODELSDEV_URL, &dir);
    }
}

fn refresh_modelsdev_at(url: &str, dir: &Path) {
    let path = dir.join(MODELSDEV);
    let fresh = std::fs::metadata(&path).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|e| e.as_secs() < 86_400);
    if fresh {
        return;
    }
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    let etag = old.lines().next().and_then(|l| l.strip_prefix("#etag ")).unwrap_or_default();
    let http: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(60))).http_status_as_error(false).build().into();
    let mut req = http.get(url).header("Accept-Encoding", "gzip");
    if !etag.is_empty() {
        req = req.header("If-None-Match", etag);
    }
    let Ok(mut r) = req.call() else { return };
    match r.status().as_u16() {
        // unchanged: the copy counts as fresh for another day
        304 => {
            let _ = std::fs::File::options().append(true).open(&path).and_then(|f| f.set_modified(std::time::SystemTime::now()));
        }
        200 => {
            let tag = r.headers().get("etag").and_then(|v| v.to_str().ok()).unwrap_or_default().to_string();
            let gz = r.headers().get("content-encoding").is_some_and(|v| v == "gzip");
            let body = r.body_mut().with_config().limit(64 << 20);
            let t = if gz {
                let Ok(bytes) = body.read_to_vec() else { return };
                gz_data(&bytes).and_then(|data| trim_modelsdev(&mut Inflate { data, state: miniz_oxide::inflate::stream::InflateState::new_boxed(miniz_oxide::DataFormat::Raw), end: false }))
            } else {
                // one reader type for both: each is a parser's worth of binary
                trim_modelsdev(&mut body.reader())
            };
            let Some(t) = t else { return };
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
            let tmp = dir.join(format!("{MODELSDEV}.tmp"));
            if std::fs::write(&tmp, format!("#etag {tag}\n#at {now}\n{t}")).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        }
        _ => {}
    }
}

/// The model servers tazuna looks for on this machine at start (OMP's local engines): name, API, the key variable a server
/// that wants one reads (none needed otherwise), and what its list must name as owner where others use the port too
/// (8080, 8000). Price 0, trusted: the code stays on this machine.
const LOCAL: [(&str, &str, &str, &str); 4] = [
    ("Ollama", "http://127.0.0.1:11434/v1", "OLLAMA_LOCAL_API_KEY", ""),
    ("LM Studio", "http://127.0.0.1:1234/v1", "LM_STUDIO_API_KEY", ""),
    ("llama.cpp", "http://127.0.0.1:8080/v1", "LLAMA_CPP_API_KEY", "llamacpp"),
    ("vLLM", "http://127.0.0.1:8000/v1", "VLLM_API_KEY", "vllm"),
];
const LOCAL_FILE: &str = "local-models.json";

/// The local servers found at start (`refresh_local`), by name.
pub fn local_running() -> Vec<String> {
    let found: Value = crate::labels::dir().ok().and_then(|d| std::fs::read_to_string(d.join(LOCAL_FILE)).ok()).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    found.as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default()
}

/// One of the local servers' APIs (what `--data trusted` lets through: the code does not leave the machine).
pub fn local_server(url: &str) -> bool {
    LOCAL.iter().any(|l| l.1 == url.trim_end_matches('/'))
}

/// At start, in the background: which local servers answer and what they list, into the data folder (a refused port
/// answers at once; one that hangs waits 1.5 s). Rewritten each start, so a stopped server leaves the picker.
pub fn refresh_local() {
    let found = probe_local(&LOCAL);
    if let Ok(dir) = crate::labels::dir() {
        let tmp = dir.join(format!("{LOCAL_FILE}.tmp"));
        if std::fs::write(&tmp, found.to_string()).is_ok() {
            let _ = std::fs::rename(&tmp, dir.join(LOCAL_FILE));
        }
    }
}

/// Each of `servers` that answers with a list (and names the owner it must): {name: {url, data}}.
/// No key goes with the probe: whatever answers on those ports gets none before it is known to be the server (a server that
/// wants one is not found, and its models are set up by hand). Ollama's models that run elsewhere (a cloud model, or one
/// built FROM one: its /api/tags names a remote host or model) are left out, as not on this machine.
fn probe_local(servers: &[(&str, &str, &str, &str)]) -> Value {
    let http: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_millis(1500))).build().into();
    let get = |url: String| http.get(url).call().ok().and_then(|mut r| r.body_mut().read_to_string().ok()).and_then(|t| serde_json::from_str::<Value>(&t).ok());
    let mut found = serde_json::Map::new();
    for (name, url, _, owner) in servers {
        let Some(mut v) = get(format!("{url}/models")) else { continue };
        if *name == "Ollama" {
            let tags = get(format!("{}/api/tags", url.trim_end_matches("/v1"))).unwrap_or_default();
            let remote: Vec<&str> = tags["models"].as_array().into_iter().flatten()
                .filter(|m| m["remote_host"].as_str().is_some_and(|h| !h.is_empty()) || m["remote_model"].as_str().is_some_and(|r| !r.is_empty()))
                .filter_map(|m| m["name"].as_str().or(m["model"].as_str())).collect();
            if let Some(d) = v["data"].as_array_mut() {
                d.retain(|m| !m["id"].as_str().is_some_and(|id| remote.contains(&id)));
            }
        }
        if v["data"].as_array().is_some_and(|d| !d.is_empty() && (owner.is_empty() || d.iter().any(|m| m["owned_by"] == *owner))) {
            found.insert(name.to_string(), serde_json::json!({"url": url, "data": v["data"]}));
        }
    }
    Value::Object(found)
}

/// The local servers' models as picker rows: what each lists, but Ollama's cloud models (…:cloud, …-cloud run on
/// ollama.com) and embedding models.
fn local_choices(list: &Value, file: &Value) -> Vec<Choice> {
    let mut out = vec![];
    for (name, url, var, _) in LOCAL {
        for m in list[name]["data"].as_array().into_iter().flatten() {
            let Some(id) = m["id"].as_str() else { continue };
            if id.ends_with("cloud") || id.contains("embed") {
                continue;
            }
            out.push(Choice {
                model: id.to_string(),
                label: id.to_string(),
                route: Some((url.to_string(), var.to_string(), route_price(0.0, 0.0, None))),
                place: format!("{name} on this machine"),
                efforts: vec![],
                price: "free".into(),
                note: format!("{name} on this machine: the code stays here; tools, cache and quality not measured"),
                stale: false,
                state: State::Ready,
                bench: scores(file, id).0,
                aa: scores(file, id).1,
                pins: vec![],
            });
        }
    }
    out
}

/// A row of a provider in LOGINS: the first of its key variables that holds a key, its price ("plan" on a subscription,
/// "free" at 0, empty where none is listed: the engine reads one from the provider's list, else --price sets one), the
/// line on its terms first in the note.
fn login_row(l: &Login, id: &str, price: Option<(f64, f64, Option<f64>)>, efforts: Vec<String>, file: &Value, trusted: &impl Fn(&str) -> bool) -> Choice {
    let var = l.vars.iter().copied().find(|v| key(v).is_some()).unwrap_or(l.vars[0]);
    let (route, shown, what) = match price {
        _ if l.plan => (route_price(0.0, 0.0, None), "plan".to_string(), format!("on your {} subscription", l.name)),
        Some((i, o, c)) => (route_price(i, o, c), if i == 0.0 && o == 0.0 { "free".into() } else { format!("{} / {}", per_m(i), per_m(o)) }, String::new()),
        None => (String::new(), String::new(), "no price listed: --price IN,CACHED,OUT (USD per M) sets one for the $1 cap".into()),
    };
    let from = if matches!(l.source, Source::ModelsDev(_)) { "models.dev" } else { "its list" };
    let checked = format!("not checked yet: {}effort levels, cache and quality not measured", if l.plan { String::new() } else { format!("prices from {from}; ") });
    let name = id.rsplit('/').next().unwrap_or(id);
    Choice {
        model: id.to_string(),
        label: id.to_string(),
        route: Some((l.url.to_string(), var.to_string(), route)),
        place: l.name.to_string(),
        efforts,
        price: shown,
        note: [l.note, &what, &checked].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" · "),
        stale: false,
        state: if key(var).is_none() {
            State::NoKey
        } else if !trusted(l.host()) {
            State::Untrusted
        } else {
            State::Ready
        },
        bench: scores(file, name).0,
        aa: scores(file, name).1,
        pins: vec![],
    }
}

/// The models.dev providers' rows from `trim_modelsdev`'s lines.
fn modelsdev_choices(tsv: &str, file: &Value, trusted: &impl Fn(&str) -> bool) -> Vec<Choice> {
    let mut out = vec![];
    let mut at: Vec<&Login> = vec![];
    for line in tsv.lines().filter(|l| !l.starts_with('#')) {
        if let Some(id) = line.strip_prefix('=') {
            at = LOGINS.iter().filter(|l| matches!(l.source, Source::ModelsDev(m) if m == id)).collect();
            continue;
        }
        let [id, input, output, cached, levels] = line.split('\t').collect::<Vec<_>>()[..] else { continue };
        let n = |x: &str| x.parse::<f64>().ok().filter(|x| price_ok(*x));
        // 0 / 0 is no price listed (a gateway's router, Kilo's openrouter/auto, NanoGPT's auto-model, bills the model it
        // picks), unless the id says free; a plan's rows are "plan" whatever is listed
        let free = id.contains(":free") || id.contains("-free");
        let zero = n(input) == Some(0.0) && n(output) == Some(0.0);
        // OpenCode Zen's free models are OpenCode's own (its terms): its paid ones stay
        if (zero || free) && at.iter().any(|l| l.name == "OpenCode Zen") {
            continue;
        }
        let price = n(input).zip(n(output)).filter(|_| !zero || free).map(|(i, o)| (i, o, n(cached)));
        let efforts: Vec<String> = match levels {
            "*" => LEVELS.iter().map(|e| e.to_string()).collect(),
            l => l.split_whitespace().map(String::from).collect(),
        };
        out.extend(at.iter().map(|l| login_row(l, id, price, efforts.clone(), file, trusted)));
    }
    out
}

/// A `Source::List` provider's rows from its /models: each id, no price of its own (the engine reads the list's, if any).
fn list_choices(l: &Login, list: &Value, file: &Value, trusted: &impl Fn(&str) -> bool) -> Vec<Choice> {
    list["data"].as_array().into_iter().flatten().filter_map(|m| m["id"].as_str())
        .filter(|id| !id.to_lowercase().contains("claude") && !id.contains("embed"))
        .map(|id| login_row(l, id, None, vec![], file, trusted)).collect()
}

/// The API a provider's models all take ("chat", "responses", "messages"), by its URL; None: by the model's name.
pub fn api_of(url: &str) -> Option<&'static str> {
    let url = url.trim_end_matches('/');
    if local_server(url) {
        return Some("chat");
    }
    LOGINS.iter().find(|l| l.url == url && !l.api.is_empty()).map(|l| l.api)
}

/// The provider a row's route belongs to, by its URL and key variable (Fireworks and Fire Pass share a URL).
pub fn login_of(c: &Choice) -> Option<&'static Login> {
    let (url, var, _) = c.route.as_ref()?;
    login_at(url, var)
}

/// The provider whose API and key these are: by URL and key variable (MiniMax Plan and MiniMax share a URL, Fire Pass and
/// Fireworks), else by URL alone.
fn login_at(url: &str, var: &str) -> Option<&'static Login> {
    let url = url.trim_end_matches('/');
    LOGINS.iter().find(|l| l.url == url && l.vars.contains(&var)).or_else(|| LOGINS.iter().find(|l| l.url == url))
}

/// SubRouter's balance in USD, read with an API key (SUBROUTER_API_KEY, else any route key; none set: nothing is
/// sent), never the account login. Its billing endpoints count in the unit its public status names: on 2026-11-02 that
/// was CNY at 6.8 to the dollar ((granted - used) / 6.8 matched the account page to the cent); by 2026-11-21 it was USD
/// (`quota_display_type`), and the same division showed $0.046 for the user's $0.32. So the status decides, and without
/// it the value stands as the endpoints give it.
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
    Some(in_usd(granted - used / 100.0, &get("/api/status").unwrap_or_default()))
}

/// A SubRouter billing figure in dollars, by the unit its status (`/api/status`) says the figures are in.
fn in_usd(v: f64, status: &Value) -> f64 {
    let d = if status["data"].is_object() { &status["data"] } else { status };
    match (d["quota_display_type"].as_str(), d["usd_exchange_rate"].as_f64()) {
        (Some("CNY"), Some(rate)) if rate > 0.0 => v / rate,
        _ => v,
    }
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

fn saved_keys() -> Value {
    keys_file().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

thread_local! {
    /// keys.json as `catalog` read it once for all its rows (some 3,000 since 0.1.196, each asking `key`); None outside it.
    static SAVED: std::cell::RefCell<Option<Value>> = const { std::cell::RefCell::new(None) };
}

fn saved_key(var: &str) -> Option<String> {
    let of = |v: &Value| v[var].as_str().map(|k| k.trim().to_string()).filter(|k| !k.is_empty());
    SAVED.with_borrow(|s| s.as_ref().map(of)).unwrap_or_else(|| of(&saved_keys()))
}

fn env_key(var: &str) -> Option<String> {
    std::env::var(var).ok().filter(|k| !k.trim().is_empty())
}

/// The first of these variables set in this process's environment: what `/login` saves for every folder when the user
/// types nothing.
pub fn env_value(vars: &[&str]) -> Option<String> {
    vars.iter().find_map(|v| env_key(v))
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
        // a local server that wants no key gets a stand-in (the engine sends one)
        .or_else(|| LOCAL.iter().any(|l| l.2 == var).then(|| "local".to_string()))
}

/// A provider `/login` offers: its name, the key's variables, its API (whose host sees the code), where keys are made, an
/// example of a key's start, where its models come from, the API its models take, and a line on its terms where they put
/// the account at risk. None of them has a login for third-party tools here (no OAuth), so an API key it is.
pub struct Login {
    pub name: &'static str,
    /// The variables its routes read the key from (the first is the usual one): a login saves it under each.
    pub vars: &'static [&'static str],
    /// The API's base URL, to which tazuna adds /chat/completions, /responses or /messages.
    pub url: &'static str,
    pub page: &'static str,
    pub place: &'static str,
    pub example: &'static str,
    pub source: Source,
    /// "chat", "responses" or "messages": the API every model there takes; empty: by the model's name (claude- Messages,
    /// gpt- Responses, the rest Chat Completions), as before 0.1.196.
    pub api: &'static str,
    /// The provider's own terms in one line, where using it from tazuna can cost the user the plan or the account; shown on
    /// its row in /model, /login and the desktop. Empty: nothing to weigh.
    pub note: &'static str,
    /// A subscription: its models show "plan" and cost the $1 cap nothing.
    pub plan: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Source {
    /// tazuna's own list for it (SubRouter's routes, Mortera's, OpenRouter's... above).
    Own,
    /// models.dev's catalog under this id: the built-in snapshot, renewed once a day (`refresh_modelsdev`).
    ModelsDev(&'static str),
    /// The provider's /models with the key, read at start: ids without prices (the engine reads one from it if it has one).
    List,
}

impl Login {
    /// The host that sees the code.
    pub fn host(&self) -> &'static str {
        host_of(self.url)
    }

    /// Its note says the account (or the plan, the key) may be at risk, or that the provider may refuse tazuna: shown in the
    /// warn colour, the other notes quietly.
    pub fn warns(&self) -> bool {
        ["at risk", "refuse", "suspended", "blocked", "revoked"].iter().any(|w| self.note.contains(w))
    }
}

const P: Login = Login { name: "", vars: &[], url: "", page: "", place: "its dashboard (API keys)", example: "", source: Source::List, api: "chat", note: "", plan: false };
const OWN: Login = Login { source: Source::Own, api: "", ..P };
use Source::ModelsDev as Md;

/// The providers, each once (OMP's list, 2026-11-22, but its logins: Claude runs through the claude CLI, and ChatGPT,
/// Copilot, Gemini CLI, Qwen and the other OAuth plans are decided apart). Region variants with their own URL and key
/// are providers of their own; a China region's key variable ends in _CN_ where the provider names one variable for both.
/// URLs from the providers' docs (2026-11-22), else as OMP and models.dev both have them.
pub const LOGINS: &[Login] = &[
    Login { name: "SubRouter", vars: &["SUBROUTER_API_KEY"], url: "https://subrouter.ai/v1", page: "https://subrouter.ai/console/keys",
            place: "SubRouter's console", example: "sk-...", ..OWN },
    Login { name: "Mortera", vars: &["MORTERA_API_KEY"], url: "https://mortera.cloud/v1", page: "https://mortera.cloud",
            place: "your Mortera dashboard", ..OWN },
    Login { name: "OpenRouter", vars: &["OPENROUTER_API_KEY"], url: "https://openrouter.ai/api/v1", page: "https://openrouter.ai/keys",
            place: "your OpenRouter account", example: "sk-or-v1-...", ..OWN },
    Login { name: "Kenari", vars: &["KENARI_API_KEY"], url: "https://kenari.id/v1", page: "https://kenari.id/login?next=/keys",
            place: "your kenari dashboard (Keys)", ..OWN },
    // a Vibe CLI key works on the same API, and a user's routes read it from MISTRAL_VIBE_API_KEY (2026-11-10)
    Login { name: "Mistral", vars: &["MISTRAL_API_KEY", "MISTRAL_VIBE_API_KEY"], url: "https://api.mistral.ai/v1", page: "https://console.mistral.ai/api-keys",
            place: "Mistral's console (or Code › Vibe CLI)", ..OWN },
    // the own loop's first provider (2026-10-09); its list needs the key, so its models show once one is saved (2026-11-16)
    Login { name: "Cheaper Inference", vars: &["CHEAPER_INFERENCE_API_KEY"], url: "https://api.cheaperinference.com/v1",
            page: "https://platform.cheaperinference.com/dashboard", place: "your Cheaper Inference dashboard (API keys)", ..OWN },
    // a gateway on this machine (2026-11-17): the accounts and keys connected there behind one endpoint
    Login { name: "OmniRoute", vars: &["OMNIROUTE_API_KEY"], url: OMNIROUTE, page: "http://localhost:20128",
            place: "your OmniRoute dashboard (Endpoints)", ..OWN },
    // the model makers' own APIs
    Login { name: "DeepSeek", vars: &["DEEPSEEK_API_KEY"], url: "https://api.deepseek.com", page: "https://platform.deepseek.com/api_keys",
            place: "DeepSeek's platform (API keys)", example: "sk-...", source: Md("deepseek"), ..P },
    Login { name: "Moonshot", vars: &["MOONSHOT_API_KEY"], url: "https://api.moonshot.ai/v1", page: "https://platform.moonshot.ai/console/api-keys",
            place: "Moonshot's platform (API keys)", example: "sk-...", source: Md("moonshotai"), ..P },
    // Kimi's coding subscription: api.kimi.ai abroad, api.kimi.com in China (Kimi Code's models page, 2026-11-22)
    Login { name: "Kimi Code", vars: &["KIMI_API_KEY"], url: "https://api.kimi.ai/coding/v1", page: "https://www.kimi.com/code",
            place: "your Kimi Code membership (API keys)", source: Md("kimi-code-plan-global"), plan: true,
            note: "Kimi may refuse tazuna as a client (it named its listed coding agents in 2025): check before buying the plan", ..P },
    Login { name: "Kimi Code China", vars: &["KIMI_CN_API_KEY"], url: "https://api.kimi.com/coding/v1", page: "https://www.kimi.com/code",
            place: "your Kimi Code membership (API keys)", source: Md("kimi-code-plan-cn"), plan: true,
            note: "Kimi may refuse tazuna as a client (it named its listed coding agents in 2025): check before buying the plan", ..P },
    Login { name: "Z.ai", vars: &["ZAI_API_KEY"], url: "https://api.z.ai/api/paas/v4", page: "https://z.ai/manage-apikey/apikey-list",
            place: "Z.ai's API keys page", source: Md("zai"), ..P },
    // the same key as pay-as-you-go: the endpoint decides what it bills
    Login { name: "Z.ai Coding Plan", vars: &["ZAI_API_KEY"], url: "https://api.z.ai/api/coding/paas/v4", page: "https://z.ai/manage-apikey/apikey-list",
            place: "Z.ai's API keys page", source: Md("zai-coding-plan"), plan: true,
            note: "Z.ai allows the Coding Plan only in its listed tools and tazuna is not listed: the account may be at risk", ..P },
    Login { name: "Zhipu Coding Plan", vars: &["ZHIPU_API_KEY"], url: "https://open.bigmodel.cn/api/coding/paas/v4",
            page: "https://bigmodel.cn/usercenter/proj-mgmt/apikeys", place: "BigModel's API keys page", source: Md("zhipuai-coding-plan"), plan: true,
            note: "Zhipu allows the Coding Plan only in its listed tools and tazuna is not listed: the account may be at risk", ..P },
    // MiniMax's own advice: its Anthropic API (thinking kept across tool calls); China's host is api.minimax.cn
    Login { name: "MiniMax", vars: &["MINIMAX_API_KEY"], url: "https://api.minimax.io/anthropic/v1", page: "https://platform.minimax.io/user-center/basic-information/interface-key",
            place: "MiniMax's platform (API keys)", api: "messages", source: Md("minimax"), ..P },
    Login { name: "MiniMax China", vars: &["MINIMAX_CN_API_KEY"], url: "https://api.minimax.cn/anthropic/v1", page: "https://platform.minimax.cn",
            place: "MiniMax's China platform (API keys)", api: "messages", source: Md("minimax-cn"), ..P },
    // a plan key of its own ("cannot be used interchangeably" with pay-as-you-go); OMP's variable names
    Login { name: "MiniMax Plan", vars: &["MINIMAX_CODE_API_KEY"], url: "https://api.minimax.io/anthropic/v1", page: "https://platform.minimax.io/user-center/basic-information/interface-key",
            place: "MiniMax's platform (your plan's key)", api: "messages", source: Md("minimax-coding-plan"), plan: true, ..P },
    Login { name: "MiniMax Plan China", vars: &["MINIMAX_CODE_CN_API_KEY"], url: "https://api.minimax.cn/anthropic/v1", page: "https://platform.minimax.cn",
            place: "MiniMax's China platform (your plan's key)", api: "messages", source: Md("minimax-cn-coding-plan"), plan: true, ..P },
    // Model Studio: keys are per region ("They cannot be used across regions")
    Login { name: "Alibaba Model Studio", vars: &["DASHSCOPE_API_KEY"], url: "https://dashscope-intl.aliyuncs.com/compatible-mode/v1",
            page: "https://modelstudio.console.alibabacloud.com/?tab=playground#/api-key", place: "Model Studio's console (Singapore, API keys)",
            example: "sk-...", source: Md("alibaba"), ..P },
    Login { name: "Alibaba Model Studio China", vars: &["DASHSCOPE_CN_API_KEY"], url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
            page: "https://bailian.console.aliyun.com/?tab=model#/api-key", place: "Bailian's console (Beijing, API keys)", example: "sk-...",
            source: Md("alibaba-cn"), ..P },
    // the Qwen Coding Plan (the user, 2026-11-22)
    Login { name: "Alibaba Coding Plan", vars: &["ALIBABA_CODING_PLAN_API_KEY"], url: "https://coding-intl.dashscope.aliyuncs.com/v1",
            page: "https://modelstudio.console.alibabacloud.com/?tab=coding-plan", place: "Model Studio's Coding Plan page (Qwen Coding Plan)",
            example: "sk-sp-...", source: Md("alibaba-coding-plan"), plan: true,
            note: "Alibaba allows the Coding Plan only for interactive use in coding tools, never scripts or benchmarks; it may refuse tazuna", ..P },
    Login { name: "Alibaba Coding Plan China", vars: &["ALIBABA_CODING_PLAN_CN_API_KEY"], url: "https://coding.dashscope.aliyuncs.com/v1",
            page: "https://bailian.console.aliyun.com/?tab=coding-plan", place: "Bailian's Coding Plan page", example: "sk-sp-...",
            source: Md("alibaba-coding-plan-cn"), plan: true,
            note: "Alibaba allows the Coding Plan only for interactive use in coding tools, never scripts or benchmarks; it may refuse tazuna", ..P },
    Login { name: "Alibaba Token Plan", vars: &["ALIBABA_TOKEN_PLAN_API_KEY", "BAILIAN_TOKEN_PLAN_API_KEY"],
            url: "https://token-plan.ap-southeast-1.maas.aliyuncs.com/compatible-mode/v1", page: "https://modelstudio.console.alibabacloud.com/?tab=token-plan",
            place: "Model Studio's Token Plan page", example: "sk-sp-...", source: Md("alibaba-token-plan"), plan: true,
            note: "Alibaba allows the Token Plan only for interactive use, not scripts or app backends: the key may be suspended", ..P },
    Login { name: "Alibaba Token Plan China", vars: &["ALIBABA_TOKEN_PLAN_CN_API_KEY"], url: "https://token-plan.cn-beijing.maas.aliyuncs.com/compatible-mode/v1",
            page: "https://bailian.console.aliyun.com/?tab=token-plan", place: "Bailian's Token Plan page", example: "sk-sp-...",
            source: Md("alibaba-token-plan-cn"), plan: true,
            note: "Alibaba allows the Token Plan only for interactive use, not scripts or app backends: the key may be suspended", ..P },
    // xAI: Responses is "the recommended way", Chat Completions a legacy endpoint
    Login { name: "xAI", vars: &["XAI_API_KEY"], url: "https://api.x.ai/v1", page: "https://console.x.ai/team/default/api-keys",
            place: "the xAI console (API keys)", example: "xai-...", api: "responses", source: Md("xai"), ..P },
    Login { name: "OpenAI", vars: &["OPENAI_API_KEY"], url: "https://api.openai.com/v1", page: "https://platform.openai.com/api-keys",
            place: "OpenAI's platform (API keys)", example: "sk-...", api: "responses", source: Md("openai"), ..P },
    // Gemini's OpenAI-compatible endpoint (Chat Completions with tools; no Responses)
    Login { name: "Google Gemini", vars: &["GEMINI_API_KEY"], url: "https://generativelanguage.googleapis.com/v1beta/openai",
            page: "https://aistudio.google.com/apikey", place: "Google AI Studio (API keys)", example: "AIza...", source: Md("google"), ..P },
    Login { name: "Meta", vars: &["META_API_KEY"], url: "https://api.meta.ai/v1", page: "https://developer.meta.com/ai/",
            place: "Meta's Model API dashboard", api: "responses", source: Md("meta"), ..P },
    Login { name: "Xiaomi MiMo", vars: &["XIAOMI_API_KEY"], url: "https://api.xiaomimimo.com/v1", page: "https://platform.xiaomimimo.com",
            place: "Xiaomi MiMo's platform (API keys)", source: Md("xiaomi"), ..P },
    Login { name: "Xiaomi Token Plan", vars: &["XIAOMI_TOKEN_PLAN_SGP_API_KEY"], url: "https://token-plan-sgp.xiaomimimo.com/v1",
            page: "https://platform.xiaomimimo.com/console/plan-manage", place: "Xiaomi MiMo's plan page (Singapore key)", example: "tp-...",
            source: Md("xiaomi-token-plan-sgp"), plan: true,
            note: "Xiaomi allows the Token Plan only in coding tools, not scripts or backends: the key may be blocked", ..P },
    Login { name: "Xiaomi Token Plan Europe", vars: &["XIAOMI_TOKEN_PLAN_AMS_API_KEY"], url: "https://token-plan-ams.xiaomimimo.com/v1",
            page: "https://platform.xiaomimimo.com/console/plan-manage", place: "Xiaomi MiMo's plan page (Europe key)", example: "tp-...",
            source: Md("xiaomi-token-plan-ams"), plan: true,
            note: "Xiaomi allows the Token Plan only in coding tools, not scripts or backends: the key may be blocked", ..P },
    Login { name: "Xiaomi Token Plan China", vars: &["XIAOMI_TOKEN_PLAN_CN_API_KEY"], url: "https://token-plan-cn.xiaomimimo.com/v1",
            page: "https://platform.xiaomimimo.com/console/plan-manage", place: "Xiaomi MiMo's plan page (China key)", example: "tp-...",
            source: Md("xiaomi-token-plan-cn"), plan: true,
            note: "Xiaomi allows the Token Plan only in coding tools, not scripts or backends: the key may be blocked", ..P },
    Login { name: "StepFun", vars: &["STEPFUN_API_KEY"], url: "https://api.stepfun.ai/v1", page: "https://platform.stepfun.ai/interface-key",
            place: "StepFun's platform (API keys)", source: Md("stepfun"), ..P },
    Login { name: "Inception", vars: &["INCEPTION_API_KEY"], url: "https://api.inceptionlabs.ai/v1", page: "https://platform.inceptionlabs.ai/dashboard/api-keys",
            place: "Inception's dashboard (API keys)", source: Md("inception"), ..P },
    Login { name: "Sakana AI", vars: &["SAKANA_API_KEY"], url: "https://api.sakana.ai/v1", page: "https://console.sakana.ai/api-keys",
            place: "Sakana AI's console (API keys)", source: Md("sakana"), ..P },
    // hosts of open models
    Login { name: "Groq", vars: &["GROQ_API_KEY"], url: "https://api.groq.com/openai/v1", page: "https://console.groq.com/keys",
            place: "the Groq console (API keys)", example: "gsk_...", source: Md("groq"), ..P },
    Login { name: "Cerebras", vars: &["CEREBRAS_API_KEY"], url: "https://api.cerebras.ai/v1", page: "https://cloud.cerebras.ai/platform/",
            place: "the Cerebras cloud console (API keys)", example: "csk-...", source: Md("cerebras"), ..P },
    Login { name: "SambaNova", vars: &["SAMBANOVA_API_KEY"], url: "https://api.sambanova.ai/v1", page: "https://cloud.sambanova.ai/apis",
            place: "SambaNova Cloud (APIs)", source: Md("sambanova"), ..P },
    Login { name: "Together", vars: &["TOGETHER_API_KEY"], url: "https://api.together.ai/v1", page: "https://api.together.ai/settings/api-keys",
            place: "Together's settings (API keys)", source: Md("togetherai"), ..P },
    Login { name: "Fireworks", vars: &["FIREWORKS_API_KEY"], url: "https://api.fireworks.ai/inference/v1", page: "https://app.fireworks.ai/settings/users/api-keys",
            place: "Fireworks' settings (API keys)", example: "fw_...", source: Md("fireworks-ai"), ..P },
    Login { name: "Fire Pass", vars: &["FIREPASS_API_KEY"], url: "https://api.fireworks.ai/inference/v1", page: "https://app.fireworks.ai/settings/users/api-keys",
            place: "Fireworks' settings (a Fire Pass key)", example: "fpk_...", plan: true,
            note: "Fireworks allows Fire Pass for personal coding with agent tools, not production work: the pass may be revoked", ..P },
    Login { name: "DeepInfra", vars: &["DEEPINFRA_API_KEY"], url: "https://api.deepinfra.com/v1/openai", page: "https://deepinfra.com/dash/api_keys",
            place: "DeepInfra's dashboard (API keys)", source: Md("deepinfra"), ..P },
    Login { name: "Novita", vars: &["NOVITA_API_KEY"], url: "https://api.novita.ai/openai", page: "https://novita.ai/settings/key-management",
            place: "Novita's settings (key management)", source: Md("novita-ai"), ..P },
    Login { name: "SiliconFlow", vars: &["SILICONFLOW_API_KEY"], url: "https://api.siliconflow.com/v1", page: "https://cloud.siliconflow.com/account/ak",
            place: "SiliconFlow's console (API keys)", example: "sk-...", source: Md("siliconflow"), ..P },
    Login { name: "SiliconFlow China", vars: &["SILICONFLOW_CN_API_KEY"], url: "https://api.siliconflow.cn/v1", page: "https://cloud.siliconflow.cn/account/ak",
            place: "SiliconFlow's China console (API keys)", example: "sk-...", source: Md("siliconflow-cn"), ..P },
    Login { name: "Chutes", vars: &["CHUTES_API_KEY"], url: "https://llm.chutes.ai/v1", page: "https://chutes.ai/app/settings/api-keys",
            place: "Chutes' settings (API keys)", example: "cpk_...",  source: Md("chutes"),
            note: "Chutes' terms: interactive and low-volume use, not large automated runs", ..P },
    Login { name: "Baseten", vars: &["BASETEN_API_KEY"], url: "https://inference.baseten.co/v1", page: "https://app.baseten.co/settings/api_keys",
            place: "Baseten's settings (API keys)", source: Md("baseten"), ..P },
    Login { name: "Hugging Face", vars: &["HF_TOKEN", "HUGGINGFACE_HUB_TOKEN"], url: "https://router.huggingface.co/v1", page: "https://huggingface.co/settings/tokens",
            place: "Hugging Face's settings (a token that may call Inference Providers)", example: "hf_...", source: Md("huggingface"), ..P },
    Login { name: "NVIDIA", vars: &["NVIDIA_API_KEY"], url: "https://integrate.api.nvidia.com/v1", page: "https://build.nvidia.com/settings/api-keys",
            place: "NVIDIA's build site (API keys)", example: "nvapi-...", source: Md("nvidia"), ..P },
    Login { name: "Scaleway", vars: &["SCW_SECRET_KEY"], url: "https://api.scaleway.ai/v1", page: "https://console.scaleway.com/iam/api-keys",
            place: "Scaleway's console (its secret key)", source: Md("scaleway"), ..P },
    Login { name: "Modular", vars: &["MODULAR_API_KEY"], url: "https://api.modular.com/v1", page: "https://console.modular.com/api_keys",
            place: "the Modular console (API keys)", source: Md("modular"), ..P },
    Login { name: "GMI Cloud", vars: &["GMI_API_KEY"], url: "https://api.gmi-serving.com/v1", page: "https://console.gmicloud.ai",
            place: "the GMI Cloud console", source: Md("gmicloud"), ..P },
    Login { name: "ai&", vars: &["AIAND_API_KEY"], url: "https://api.aiand.com/v1", page: "https://console.aiand.com/api-keys",
            place: "the ai& console (API keys)", source: Md("aiand"), ..P },
    Login { name: "CoralBricks", vars: &["CORAL_API_KEY"], url: "https://inference.coralbricks.ai/v1", page: "https://www.coralbricks.ai/api-keys",
            place: "CoralBricks' dashboard (API keys)", example: "cb_...", source: Md("coralbricks"), ..P },
    Login { name: "Abliteration", vars: &["ABLITERATION_API_KEY", "ABLIT_KEY"], url: "https://api.abliteration.ai/v1", page: "https://abliteration.ai/console",
            place: "the Abliteration console", example: "ak_...", source: Md("abliteration-ai"), ..P },
    Login { name: "Venice", vars: &["VENICE_API_KEY"], url: "https://api.venice.ai/api/v1", page: "https://venice.ai/settings/api",
            place: "Venice's settings (API)", source: Md("venice"), ..P },
    Login { name: "Wafer", vars: &["WAFER_SERVERLESS_API_KEY"], url: "https://pass.wafer.ai/v1", page: "https://app.wafer.ai",
            place: "the Wafer dashboard (API keys)", example: "wfr_...", source: Md("wafer.ai"), ..P },
    Login { name: "Ollama Cloud", vars: &["OLLAMA_API_KEY"], url: "https://ollama.com/v1", page: "https://ollama.com/settings/keys",
            place: "your ollama.com settings (keys)", source: Md("ollama-cloud"), ..P },
    Login { name: "Synthetic", vars: &["SYNTHETIC_API_KEY"], url: "https://api.synthetic.new/openai/v1", page: "https://synthetic.new",
            place: "your Synthetic account (API keys)", source: Md("synthetic"), ..P },
    Login { name: "Umans Coding Plan", vars: &["UMANS_AI_CODING_PLAN_API_KEY"], url: "https://api.code.umans.ai/v1", page: "https://app.umans.ai",
            place: "the Umans dashboard (API keys)", example: "sk-...", source: Md("umans-ai-coding-plan"), plan: true, ..P },
    // gateways: many makers' models behind one key (Claude among them stays out: it runs through the claude CLI)
    Login { name: "Vercel AI Gateway", vars: &["AI_GATEWAY_API_KEY"], url: "https://ai-gateway.vercel.sh/v1", page: "https://vercel.com/d?to=%2F%5Bteam%5D%2F%7E%2Fai-gateway%2Fapi-keys",
            place: "Vercel's dashboard (AI Gateway, API keys)", example: "vck_...", source: Md("vercel"), ..P },
    Login { name: "NanoGPT", vars: &["NANO_GPT_API_KEY"], url: "https://nano-gpt.com/api/v1", page: "https://nano-gpt.com/api",
            place: "NanoGPT's API page", source: Md("nano-gpt"), ..P },
    Login { name: "AIML API", vars: &["AIMLAPI_API_KEY"], url: "https://api.aimlapi.com/v1", page: "https://aimlapi.com/app/keys",
            place: "AIML API's dashboard (keys)", source: Md("aimlapi"), ..P },
    Login { name: "ZenMux", vars: &["ZENMUX_API_KEY"], url: "https://zenmux.ai/api/v1", page: "https://zenmux.ai/settings/keys",
            place: "ZenMux's settings (keys)", example: "sk-...", source: Md("zenmux"), ..P },
    Login { name: "Kilo Gateway", vars: &["KILO_API_KEY"], url: "https://api.kilo.ai/api/gateway", page: "https://app.kilo.ai",
            place: "your Kilo account (API keys)", source: Md("kilo"), ..P },
    // Responses for GPT, Chat Completions for the rest, by the model's name
    Login { name: "OpenCode Zen", vars: &["OPENCODE_API_KEY"], url: "https://opencode.ai/inference/openai/v1", page: "https://opencode.ai/auth",
            place: "the OpenCode console (API keys)", example: "sk-...", source: Md("opencode"), api: "",
            note: "OpenCode's terms keep Zen's free models for OpenCode itself; its paid models work from other tools", ..P },
    Login { name: "OpenCode Go", vars: &["OPENCODE_API_KEY"], url: "https://opencode.ai/inference/go/openai/v1", page: "https://opencode.ai/auth",
            place: "the OpenCode console (API keys)", example: "sk-...", source: Md("opencode-go"), api: "",
            note: "OpenCode Go asks coding agents for a session header tazuna does not send; OpenCode watches its traffic for abuse", ..P },
    Login { name: "ClinePass", vars: &["CLINE_API_KEY"], url: "https://api.cline.bot/api/v1", page: "https://app.cline.bot/dashboard/account",
            place: "the Cline dashboard (Settings › API Keys)", example: "sk_...", source: Md("cline-pass"), ..P },
    Login { name: "Command Code", vars: &["COMMAND_CODE_API_KEY"], url: "https://api.commandcode.ai/provider/v1", page: "https://commandcode.ai/studio",
            place: "Command Code Studio (Provider API keys)", example: "user_...", source: Md("commandcode"), api: "", ..P },
    Login { name: "Charm Hyper", vars: &["CHARM_HYPER_API_KEY", "HYPER_API_KEY"], url: "https://hyper.charm.land/v1", page: "https://hyper.charm.land/",
            place: "the Charm Hyper dashboard", example: "sk-hyper-...", source: Md("hyper"), ..P },
    // not in models.dev: their models from their own /models once the key is set
    Login { name: "Qianfan", vars: &["QIANFAN_API_KEY"], url: "https://qianfan.baidubce.com/v2", page: "https://console.bce.baidu.com/qianfan/ais/console/apiKey",
            place: "Baidu Qianfan's console (API keys)", example: "bce-v3/ALTAK-...", ..P },
    Login { name: "Helmcode", vars: &["HELMCODE_API_KEY"], url: "https://api.helmcode.com/v1", page: "https://cloud.helmcode.com",
            place: "the Helmcode dashboard", example: "sk-...", ..P },
    Login { name: "Otari", vars: &["OTARI_API_KEY"], url: "https://api.otari.ai/api/v1", page: "https://app.otari.ai/settings/api-keys",
            place: "Otari's settings (API keys)", example: "tk_...", ..P },
    Login { name: "Yolo-Auto", vars: &["YOLO_AUTO_API_KEY"], url: "https://yolo-auto.com/v1", page: "https://yolo-auto.com/app",
            place: "the Yolo-Auto app", example: "yolo_...", ..P },
    Login { name: "SingularityAPI", vars: &["SINGULARITYAPI_DEV_API_KEY"], url: "https://api.singularityapi.dev/v1", page: "https://app.singularityapi.dev",
            place: "the SingularityAPI dashboard", example: "sk-sapi-...", ..P },
    Login { name: "SingularityAPI Lanes", vars: &["SINGULARITYAPI_TECH_API_KEY"], url: "https://api.singularityapi.tech/v1", page: "https://app.singularityapi.tech/compute/billing",
            place: "the SingularityAPI lanes dashboard (a booked slot)", example: "sk-...", ..P },
];

/// The key variables of the route this process runs on (engine.rs `connect`, the fallbacks' too): out of the agent's
/// commands as well.
static IN_USE: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// Marks a key variable as the one this process's model runs on (`provider_key_vars` strips it).
pub fn strip_too(var: &str) {
    if let Ok(mut v) = IN_USE.lock() {
        v.push(var.to_string());
    }
}

/// Environment variables that hold model providers' keys: tazuna's own providers' (the logins before 0.1.196), every
/// `SUBROUTER_...` variable, the key variables the routes name, and the key of the route in use (`strip_too`). The agent's
/// commands never need them, and one that printed the environment sent them to the model (`env | grep` in a DeepSeek chat
/// showed SUBROUTER_*_KEY, 2026-11-16), so they are taken out of what the agent's commands see. A user's other variables
/// stay, provider keys among them: OPENAI_API_KEY, HF_TOKEN or SCW_SECRET_KEY are what their tests and tools read (0.1.196
/// stripped every login's, review 11). TAZUNA_KEEP_KEYS keeps them all (a project whose own code reads one).
pub fn provider_key_vars() -> Vec<String> {
    strip_list(&IN_USE.lock().map(|v| v.clone()).unwrap_or_default())
}

fn strip_list(in_use: &[String]) -> Vec<String> {
    if std::env::var_os("TAZUNA_KEEP_KEYS").is_some() {
        return vec![];
    }
    let mut v: Vec<String> = LOGINS.iter().filter(|l| l.source == Source::Own).flat_map(|l| l.vars.iter().map(|s| s.to_string())).collect();
    v.extend(in_use.iter().cloned());
    v.push("EXA_API_KEY".into()); // the own loop's web search (mcpc.rs `web_search`)
    v.extend(std::env::vars_os().filter_map(|(k, _)| k.into_string().ok()).filter(|k| k.starts_with("SUBROUTER_")));
    // every "key_env" in routes.json, at any depth (a route's own and its pinned providers')
    if let Some(routes) = crate::labels::dir().ok().and_then(|d| std::fs::read_to_string(d.join("routes.json")).ok()).and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
        let mut todo = vec![&routes];
        while let Some(x) = todo.pop() {
            match x {
                Value::Object(m) => {
                    v.extend(m.get("key_env").and_then(Value::as_str).map(String::from));
                    todo.extend(m.values());
                }
                Value::Array(a) => todo.extend(a.iter()),
                _ => {}
            }
        }
    }
    v.retain(|k| !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
    v.sort();
    v.dedup();
    v
}

/// Where a provider's key comes from, for `/login`'s list; empty: none.
pub fn key_source(vars: &[&str]) -> &'static str {
    // a variable set in one terminal is seen by tazuna started there only (a Mistral key showed in one folder's terminal
    // and not another's, 2026-11-12): /login saves it for every folder
    if vars.iter().any(|v| env_key(v).is_some()) && !vars.iter().any(|v| saved_key(v).is_some()) {
        "key in this terminal only"
    } else if vars.iter().any(|v| env_key(v).is_some()) {
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

/// The effort a row starts at: the session's own when it is that model, else medium on Claude, the provider's "default"
/// where it lists one (Groq's Qwen: "none default", which started at none: no thinking), and the model's high (its default
/// where it has few levels) on a route.
pub fn start_effort(c: &Choice, current_model: &str, current_effort: &str) -> usize {
    let pick = if same_model(c, current_model) {
        current_effort
    } else if c.route.is_none() || on_omniroute(c) {
        "medium"
    } else if c.efforts.iter().any(|e| e == "default") {
        "default"
    } else {
        "high"
    };
    c.efforts.iter().position(|e| e == pick).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A second variable that is always set, standing in for a key beside PATH: HOME on Unix, USERPROFILE on Windows, which
    /// has no HOME on GitHub's runners (the tests that used HOME failed there, 2026-11-08).
    const SET: &str = if cfg!(windows) { "USERPROFILE" } else { "HOME" };

    #[test]
    fn modelsdev_trims_to_tool_models_and_lists_them_by_provider() {
        // models.dev's shape: every provider tazuna lists, one plain model each, and the cases that matter
        let mut all = serde_json::Map::new();
        for l in LOGINS {
            if let Source::ModelsDev(id) = l.source {
                all.insert(id.into(), serde_json::json!({"id": id, "env": ["X"], "models": {"plain": {"tool_call": true, "reasoning": false,
                    "modalities": {"input": ["text"], "output": ["text"]}, "cost": {"input": 1, "output": 2}}}}));
            }
        }
        all["deepseek"]["models"] = serde_json::json!({
            "deepseek-flash": {"tool_call": true, "reasoning": true, "reasoning_options": [{"type": "toggle"}, {"type": "effort", "values": ["low", "high", "max"]}],
                "modalities": {"output": ["text"]}, "cost": {"input": 0.15, "output": 0.6, "cache_read": 0.003}},
            "thinker": {"tool_call": true, "reasoning": true, "cost": {"input": 0, "output": 0}},
            "thinker:free": {"tool_call": true, "reasoning": true, "cost": {"input": 0, "output": 0}},
            "minus": {"tool_call": true, "cost": {"input": -1, "output": -1}},
            "absurd": {"tool_call": true, "cost": {"input": 5000, "output": 2}},
            "no-price": {"tool_call": true},
            "no-tools": {"tool_call": false, "cost": {"input": 1, "output": 1}},
            "painter": {"tool_call": true, "modalities": {"output": ["image"]}, "cost": {"input": 1, "output": 1}},
            "anthropic/claude-fable-5-2": {"tool_call": true, "cost": {"input": 5, "output": 25}}});
        // OpenCode Zen's free models are OpenCode's own: left out, its paid one stays
        all["opencode"]["models"]["big-pickle"] = serde_json::json!({"tool_call": true, "cost": {"input": 0, "output": 0}});
        all["opencode"]["models"]["glm-5-free"] = serde_json::json!({"tool_call": true, "cost": {"input": 0, "output": 0}});
        all.insert("not-listed".into(), serde_json::json!({"models": {"x": {"tool_call": true}}}));
        let tsv = trim_modelsdev(&mut Value::Object(all).to_string().as_bytes()).expect("a models.dev file");
        assert!(tsv.contains("=deepseek\nabsurd\t\t2\t\t\ndeepseek-flash\t0.15\t0.6\t0.003\tlow high max\nminus\t\t\t\t\nno-price\t\t\t\t\nthinker\t0\t0\t\t*\nthinker:free\t0\t0\t\t*\n=moonshotai\nplain\t1\t2\t\t\n"),
                "below 0 or above $1,000 is no price: {tsv}");
        assert!(!tsv.contains("claude") && !tsv.contains("painter") && !tsv.contains("no-tools") && !tsv.contains("not-listed"));
        assert!(trim_modelsdev(&mut r#"{"deepseek": {"models": {}}}"#.as_bytes()).is_none(), "a file with too few providers is not models.dev's");
        let rows = modelsdev_choices(&format!("#etag W/\"x\"\n{tsv}"), &serde_json::json!({}), &|h: &str| h == "api.deepseek.com");
        let ds: Vec<&Choice> = rows.iter().filter(|c| c.place == "DeepSeek").collect();
        assert_eq!(ds.iter().map(|c| c.model.as_str()).collect::<Vec<_>>(), ["absurd", "deepseek-flash", "minus", "no-price", "thinker", "thinker:free"]);
        let (f, r) = (ds[1], ds[1].route.as_ref().unwrap());
        assert_eq!((r.0.as_str(), r.1.as_str(), r.2.as_str(), f.price.as_str()), ("https://api.deepseek.com", "DEEPSEEK_API_KEY", "0.15,0.003,0.6", "0.15 / 0.6"));
        assert_eq!((f.efforts.join(","), ds[3].efforts.len(), ds[4].efforts.len()), ("low,high,max".into(), 0, 5), "levels as listed; reasoning alone: all five");
        // no price listed (none, below 0, above $1,000, or 0 / 0 on a model not marked free: a router's price is the model it
        // picks): the list's, else --price, and the caps apply; a model marked free is free
        for c in [ds[0], ds[2], ds[3], ds[4]] {
            assert!(c.price.is_empty() && c.route.as_ref().unwrap().2.is_empty() && c.note.contains("--price"), "{}: no price, not free: {c:?}", c.model);
        }
        assert_eq!((ds[5].price.as_str(), ds[5].route.as_ref().unwrap().2.as_str()), ("free", "0,0,0"));
        // a copy written before these checks reads the same: a negative or absurd price is none at parse time too
        let old = modelsdev_choices("=deepseek\nneg\t-1\t-2\t\t\nhuge\t1\t2000\t\t\n", &serde_json::json!({}), &|_: &str| true);
        assert!(old.len() == 2 && old.iter().all(|c| c.price.is_empty() && c.route.as_ref().unwrap().2.is_empty()), "{old:?}");
        let zen: Vec<&str> = rows.iter().filter(|c| c.place == "OpenCode Zen").map(|c| c.model.as_str()).collect();
        assert_eq!(zen, ["plain"]);
        assert_eq!((provider_of(f).as_str(), f.state), ("DeepSeek", if key("DEEPSEEK_API_KEY").is_some() { State::Ready } else { State::NoKey }));
        // a plan: "plan", nothing for the cap, its terms first in the note; the same model under each provider that lists it
        let plan = rows.iter().find(|c| c.place == "Alibaba Coding Plan").unwrap();
        assert_eq!((plan.price.as_str(), plan.route.as_ref().unwrap().2.as_str(), plan.route.as_ref().unwrap().0.as_str()), ("plan", "0,0,0", "https://coding-intl.dashscope.aliyuncs.com/v1"));
        assert!(plan.note.starts_with("Alibaba allows the Coding Plan only for interactive use"), "{}", plan.note);
        assert_eq!(login_of(plan).map(|l| l.note), Some(plan.note.split(" · ").next().unwrap()));
        let n = LOGINS.iter().filter(|l| matches!(l.source, Source::ModelsDev(_))).count();
        assert_eq!(rows.iter().filter(|c| c.model == "plain").count(), n - 1, "every models.dev provider but DeepSeek's own fixture");
    }

    #[test]
    fn the_built_in_catalog_is_small_and_reads() {
        // the snapshot in the binary: gzipped (~21 KB for ~60 providers), and gunzip reads a header with a file name too
        assert!(MODELSDEV_BUILTIN.len() < 40_000, "{} bytes", MODELSDEV_BUILTIN.len());
        let tsv = String::from_utf8(gunzip(MODELSDEV_BUILTIN).unwrap()).unwrap();
        let rows = modelsdev_choices(&tsv, &serde_json::json!({}), &|_: &str| true);
        assert!(rows.len() > 1000 && tsv.lines().filter(|l| l.starts_with('=')).count() >= 50, "{} rows", rows.len());
        assert!(rows.iter().all(|c| !c.model.to_lowercase().contains("claude")), "Claude runs through the claude CLI");
        let mut named = vec![0x1f, 0x8b, 8, 8, 0, 0, 0, 0, 0, 3];
        named.extend(b"modelsdev.tsv\0");
        named.extend(&MODELSDEV_BUILTIN[10..]);
        assert_eq!(gunzip(&named).unwrap(), tsv.as_bytes());
        assert!(gunzip(b"not gzip at all, but long enough").is_none());
        // every provider once by name; each URL with a host; the region variants apart
        let mut names: Vec<&str> = LOGINS.iter().map(|l| l.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), LOGINS.len());
        assert!(LOGINS.iter().all(|l| l.host().contains('.') || l.host() == "localhost") && LOGINS.iter().all(|l| !l.vars.is_empty()));
        let url = |n: &str| LOGINS.iter().find(|l| l.name == n).unwrap().url;
        assert_eq!([url("Z.ai"), url("Z.ai Coding Plan"), url("Alibaba Token Plan China"), url("Kimi Code")],
                   ["https://api.z.ai/api/paas/v4", "https://api.z.ai/api/coding/paas/v4", "https://token-plan.cn-beijing.maas.aliyuncs.com/compatible-mode/v1", "https://api.kimi.ai/coding/v1"]);
    }

    #[test]
    fn a_cached_catalog_counts_only_when_newer_than_the_binarys() {
        let dir = std::env::temp_dir().join(format!("tazuna-mdcache-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let builtin = String::from_utf8(gunzip(MODELSDEV_BUILTIN).unwrap()).unwrap();
        let at = fetched_at(&builtin);
        assert!(at > 1_790_000_000, "the binary's snapshot says when it was fetched: {at}");
        assert_eq!(modelsdev_tsv(Some(&dir)), builtin, "no copy: the binary's");
        let section = |t: &str, id: &str| t.split(&format!("={id}\n")).nth(1).map(|r| r.split("\n=").next().unwrap_or_default().to_string());
        // a copy from before this binary (an upgrade): the binary's own, newer catalog
        std::fs::write(dir.join(MODELSDEV), format!("#etag W/\"o\"\n#at {}\n=deepseek\nold-model\t1\t2\t\t\n", at - 1)).unwrap();
        assert_eq!(modelsdev_tsv(Some(&dir)), builtin);
        std::fs::write(dir.join(MODELSDEV), "#etag W/\"o\"\n=deepseek\nold-model\t1\t2\t\t\n").unwrap();
        assert_eq!(modelsdev_tsv(Some(&dir)), builtin, "a copy without a date is older");
        // a newer copy: its providers as it has them, and each it lacks (renamed upstream) from the binary's
        std::fs::write(dir.join(MODELSDEV), format!("#etag W/\"n\"\n#at {}\n=deepseek\nnew-model\t1\t2\t\t\n", at + 1)).unwrap();
        let t = modelsdev_tsv(Some(&dir));
        assert_eq!(section(&t, "deepseek").as_deref(), Some("new-model\t1\t2\t\t"), "the copy's own providers as it has them");
        assert_eq!(t.matches("=deepseek\n").count(), 1);
        assert_eq!(section(&t, "moonshotai"), section(&builtin, "moonshotai"), "a provider the copy lacks, from the binary's");
        let rows = modelsdev_choices(&t, &serde_json::json!({}), &|_: &str| true);
        assert!(rows.iter().any(|c| c.model == "new-model") && rows.iter().any(|c| c.place == "Moonshot"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_catalog_refresh_keeps_a_good_copy() {
        use std::io::{Read, Write};
        // a stub models.dev, answering each request in turn; it hands back what each request said
        let mut all = serde_json::Map::new();
        for l in LOGINS {
            if let Source::ModelsDev(id) = l.source {
                all.insert(id.into(), serde_json::json!({"models": {"m": {"tool_call": true, "cost": {"input": 1, "output": 2}}}}));
            }
        }
        all.insert("unlisted".into(), serde_json::json!({"models": {"x": {"tool_call": true}}}));
        let body = Value::Object(all).to_string();
        let mut gz = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 3];
        gz.extend(miniz_oxide::deflate::compress_to_vec(body.as_bytes(), 6));
        gz.extend([0; 8]);
        let ok = |etag: &str, gzip: bool, b: Vec<u8>| format!("HTTP/1.1 200 OK\r\nETag: {etag}\r\n{}Content-Length: {}\r\nConnection: close\r\n\r\n",
            if gzip { "Content-Encoding: gzip\r\n" } else { "" }, b.len()).into_bytes().into_iter().chain(b).collect::<Vec<u8>>();
        let answers = vec![ok("W/\"1\"", true, gz.clone()), b"HTTP/1.1 304 Not Modified\r\nConnection: close\r\n\r\n".to_vec(),
            b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
            ok("W/\"2\"", false, b"<html>not json</html>".to_vec()), ok("W/\"3\"", true, gz[..gz.len() / 2].to_vec()), ok("W/\"4\"", false, body.into_bytes())];
        let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/api.json", server.local_addr().unwrap());
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for (c, answer) in server.incoming().zip(answers) {
                let mut c = c.unwrap();
                let mut req = vec![];
                let mut buf = [0u8; 1024];
                while !req.ends_with(b"\r\n\r\n") {
                    let n = c.read(&mut buf).unwrap();
                    req.extend(&buf[..n]);
                }
                let _ = c.write_all(&answer);
                tx.send(String::from_utf8_lossy(&req).to_lowercase()).unwrap();
            }
        });
        let dir = std::env::temp_dir().join(format!("tazuna-mdrefresh-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join(MODELSDEV);
        let stale = || std::fs::File::options().append(true).open(&file).unwrap().set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(2 * 86_400)).unwrap();
        let read = || std::fs::read_to_string(&file).unwrap();
        // 200, gzipped: inflated as it is read, trimmed to the listed providers, with its ETag and the time it came
        refresh_modelsdev_at(&url, &dir);
        let req = rx.recv().unwrap();
        assert!(req.contains("accept-encoding: gzip") && !req.contains("if-none-match"), "{req}");
        let first = read();
        assert!(first.starts_with("#etag W/\"1\"\n#at ") && first.contains("\n=deepseek\nm\t1\t2\t\t\n") && !first.contains("unlisted"), "{first}");
        assert!(fetched_at(&first) + 60 > std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());
        // fresh (under a day): no request at all
        refresh_modelsdev_at(&url, &dir);
        // a day on: asked with its ETag; 304 keeps the copy and makes it fresh again
        stale();
        refresh_modelsdev_at(&url, &dir);
        assert!(rx.recv().unwrap().contains("if-none-match: w/\"1\""), "the ETag goes with the request");
        assert_eq!(read(), first);
        assert!(std::fs::metadata(&file).unwrap().modified().unwrap().elapsed().unwrap().as_secs() < 60, "fresh for another day");
        // failures leave the copy: a 500, a page that is not models.dev's, a cut gzip stream
        for _ in 0..3 {
            stale();
            refresh_modelsdev_at(&url, &dir);
            rx.recv().unwrap();
            assert_eq!(read(), first);
        }
        // and a plain (not gzipped) answer reads as it comes
        stale();
        refresh_modelsdev_at(&url, &dir);
        rx.recv().unwrap();
        assert!(read().starts_with("#etag W/\"4\"\n#at ") && read().contains("\n=deepseek\nm\t1\t2\t\t\n"));
        assert!(rx.try_recv().is_err(), "one request each, none when fresh");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn each_provider_takes_its_documented_api() {
        assert_eq!([api_of("https://api.x.ai/v1"), api_of("https://api.openai.com/v1"), api_of("https://api.minimax.io/anthropic/v1/"), api_of("https://api.deepseek.com"),
                    api_of("https://opencode.ai/inference/openai/v1"), api_of("http://127.0.0.1:11434/v1"), api_of("https://subrouter.ai/v1")],
                   [Some("responses"), Some("responses"), Some("messages"), Some("chat"), None, Some("chat"), None],
                   "OpenCode, Command Code and tazuna's own providers: by the model's name, as before");
    }

    #[test]
    fn the_discretion_notes_sit_on_the_providers_whose_terms_need_them() {
        let note = |n: &str| LOGINS.iter().find(|l| l.name == n).unwrap().note;
        assert!(note("Alibaba Coding Plan").contains("interactive use in coding tools") && note("Alibaba Coding Plan").contains("benchmarks"));
        assert!(note("Z.ai Coding Plan").contains("tazuna is not listed") && note("Z.ai Coding Plan").contains("at risk"));
        for n in ["Alibaba Coding Plan China", "Alibaba Token Plan", "Zhipu Coding Plan", "Kimi Code", "Xiaomi Token Plan", "Fire Pass", "Chutes", "OpenCode Go", "OpenCode Zen"] {
            assert!(!note(n).is_empty(), "{n}");
        }
        for n in ["DeepSeek", "Z.ai", "MiniMax", "Groq", "OpenAI", "SubRouter", "Umans Coding Plan", "ClinePass"] {
            assert!(note(n).is_empty(), "{n}: no note where none is needed");
        }
        assert!(LOGINS.iter().all(|l| l.note.len() < 130 && !l.note.contains('\n')), "one short line each");
        // Kimi may refuse a client that is not its listed coding agents (403, 2025): said before the plan is bought
        assert!(note("Kimi Code").contains("may refuse tazuna") && note("Kimi Code").contains("before buying"));
        // the warn colour only where the account may be at risk or the provider may refuse tazuna (review 11, UI-1)
        let warns = |n: &str| LOGINS.iter().find(|l| l.name == n).unwrap().warns();
        assert!(["Z.ai Coding Plan", "Zhipu Coding Plan", "Alibaba Coding Plan", "Kimi Code", "Alibaba Token Plan", "Xiaomi Token Plan", "Fire Pass"].iter().all(|n| warns(n)));
        assert!(!["Chutes", "OpenCode Go", "OpenCode Zen", "DeepSeek"].iter().any(|n| warns(n)));
        // the provider's row in /model and the desktop picker read it through login_of
        let row = Choice { model: "glm-5".into(), label: "glm-5".into(), route: Some(("https://api.z.ai/api/coding/paas/v4".into(), "ZAI_API_KEY".into(), "0,0,0".into())),
            place: String::new(), efforts: vec![], price: "plan".into(), note: String::new(), stale: false, state: State::Ready, bench: String::new(), aa: String::new(), pins: vec![] };
        assert_eq!((login_of(&row).unwrap().name, provider_of(&row).as_str()), ("Z.ai Coding Plan", "Z.ai Coding Plan"));
        let mut paygo = row.clone();
        paygo.route = Some(("https://api.z.ai/api/paas/v4".into(), "ZAI_API_KEY".into(), "1,0.1,2".into()));
        assert_eq!((provider_of(&paygo).as_str(), login_of(&paygo).unwrap().note), ("Z.ai", ""), "the same key, another endpoint");
    }

    #[test]
    fn local_servers_are_found_by_their_lists() {
        use std::io::{Read, Write};
        // a stub answering /v1/models and /api/tags as Ollama does, one on another port that is not llama.cpp, and a closed
        // port; each stub keeps the requests it saw
        let stub = |models: &'static str, tags: &'static str| {
            let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}/v1", server.local_addr().unwrap());
            let seen = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
            let log = seen.clone();
            std::thread::spawn(move || {
                for c in server.incoming().take(2) {
                    let mut c = c.unwrap();
                    let mut buf = [0u8; 4096];
                    let n = c.read(&mut buf).unwrap();
                    let req = String::from_utf8_lossy(&buf[..n]).to_string();
                    let body = if req.starts_with("GET /api/tags ") { tags } else { models };
                    log.lock().unwrap().push_str(&req);
                    let _ = c.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
                }
            });
            (url, seen)
        };
        // my-coder is built FROM a cloud model: /v1/models lists it like a local one, /api/tags names its remote host
        let (ollama, asked) = stub(r#"{"object":"list","data":[{"id":"qwen3-coder:30b","owned_by":"library"},{"id":"gpt-oss:120b-cloud","owned_by":"library"},{"id":"nomic-embed-text:latest","owned_by":"library"},{"id":"glm-4.6:cloud","owned_by":"library"},{"id":"my-coder:latest","owned_by":"library"}]}"#,
            r#"{"models":[{"name":"qwen3-coder:30b","model":"qwen3-coder:30b"},{"name":"my-coder:latest","model":"my-coder:latest","remote_model":"glm-4.6","remote_host":"https://ollama.com:443"}]}"#);
        let (other, _) = stub(r#"{"data":[{"id":"gpt-6","owned_by":"openai"}]}"#, "{}");
        let closed = { let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap(); format!("http://{}/v1", l.local_addr().unwrap()) };
        let t = std::time::Instant::now();
        // a key variable that is set (HOME): the probe still sends no key to whatever answers on the port
        let found = probe_local(&[("Ollama", Box::leak(ollama.into_boxed_str()), SET, ""),
                                  ("llama.cpp", Box::leak(other.into_boxed_str()), SET, "llamacpp"),
                                  ("vLLM", Box::leak(closed.into_boxed_str()), "TAZUNA_TEST_NO_SUCH_KEY", "vllm")]);
        assert!(t.elapsed() < std::time::Duration::from_secs(2), "no waiting on a closed port");
        assert_eq!(found.as_object().unwrap().keys().collect::<Vec<_>>(), ["Ollama"], "a gateway on llama.cpp's port is not llama.cpp");
        let asked = asked.lock().unwrap().to_lowercase();
        assert!(asked.contains("get /v1/models ") && asked.contains("get /api/tags ") && !asked.contains("authorization"), "{asked}");
        // the rows: Ollama's real models on its fixed address, free, ready and trusted without a key; its cloud models (by
        // name or by /api/tags) and embedders left out (they run on ollama.com, or do not chat)
        let rows = local_choices(&found, &serde_json::json!({}));
        assert_eq!(rows.iter().map(|c| c.model.as_str()).collect::<Vec<_>>(), ["qwen3-coder:30b"]);
        let r = rows[0].route.as_ref().unwrap();
        assert_eq!((r.0.as_str(), r.2.as_str(), rows[0].price.as_str(), rows[0].state, provider_of(&rows[0]).as_str()),
                   ("http://127.0.0.1:11434/v1", "0,0,0", "free", State::Ready, "Ollama"));
        assert!(key(&r.1).is_some() && local_server(&r.0) && paid_to(&r.0, "").is_none() && api_of(&r.0) == Some("chat"));
    }

    #[test]
    fn a_row_starts_at_its_providers_default_level() {
        // Groq's Qwen lists "none default": it started at none (no thinking); a row with high starts there, as before
        let row = |levels: &[&str]| Choice { model: "qwen/qwen3.6-27b".into(), label: String::new(), route: Some(("https://api.groq.com/openai/v1".into(), "GROQ_API_KEY".into(), String::new())),
            place: String::new(), efforts: levels.iter().map(|e| e.to_string()).collect(), price: String::new(), note: String::new(), stale: false, state: State::Ready,
            bench: String::new(), aa: String::new(), pins: vec![] };
        assert_eq!([start_effort(&row(&["none", "default"]), "claude-opus-5-5", "medium"), start_effort(&row(&["none", "low", "high"]), "claude-opus-5-5", "medium"),
                    start_effort(&row(&["none", "default"]), "qwen/qwen3.6-27b", "none")], [1, 2, 0], "the session's own level on its own model");
    }

    #[test]
    fn the_providers_with_a_key_come_first() {
        let row = |url: &str, var: &str, state: State| Choice { model: "m".into(), label: "m".into(), route: Some((url.into(), var.into(), String::new())), place: String::new(),
            efforts: vec![], price: String::new(), note: String::new(), stale: false, state, bench: String::new(), aa: String::new(), pins: vec![] };
        let mut claude = row("", "", State::Ready);
        claude.route = None;
        let cs = [row("https://subrouter.ai/v1", "SUBROUTER_API_KEY", State::NoKey), row("https://api.groq.com/openai/v1", "GROQ_API_KEY", State::NoKey),
                  row("https://api.deepseek.com", "DEEPSEEK_API_KEY", State::Untrusted), claude, row("http://127.0.0.1:11434/v1", "OLLAMA_LOCAL_API_KEY", State::Ready),
                  row("https://mortera.cloud/v1", "MORTERA_API_KEY", State::Ready)];
        assert_eq!(providers(&cs).iter().map(|p| p.0.as_str()).collect::<Vec<_>>(), ["Claude", "Mortera", "DeepSeek", "Ollama", "SubRouter", "Groq"]);
    }

    #[test]
    fn the_agents_commands_keep_the_users_own_keys() {
        // a default Claude session: tazuna's own providers' keys leave the agent's commands, a user's generic ones stay
        // (their tests and tools read them; 0.1.196 stripped every login's)
        let v = strip_list(&[]);
        for k in ["OPENAI_API_KEY", "HF_TOKEN", "HUGGINGFACE_HUB_TOKEN", "SCW_SECRET_KEY", "GEMINI_API_KEY", "NVIDIA_API_KEY", "AI_GATEWAY_API_KEY", "GROQ_API_KEY"] {
            assert!(!v.iter().any(|x| x == k), "{k} taken from the user's commands");
        }
        for k in ["SUBROUTER_API_KEY", "MORTERA_API_KEY", "OPENROUTER_API_KEY", "KENARI_API_KEY", "MISTRAL_VIBE_API_KEY", "CHEAPER_INFERENCE_API_KEY", "OMNIROUTE_API_KEY", "EXA_API_KEY"] {
            assert!(v.iter().any(|x| x == k), "{k}: stripped as before");
        }
        // an own-loop session on Scaleway: its key leaves too, the others stay
        let v = strip_list(&["SCW_SECRET_KEY".into()]);
        assert!(v.iter().any(|x| x == "SCW_SECRET_KEY") && !v.iter().any(|x| x == "OPENAI_API_KEY") && !v.iter().any(|x| x == "HF_TOKEN"));
        // the engine marks its own (engine.rs `connect`), and the agent's commands lose it
        strip_too("TAZUNA_TEST_ROUTE_IN_USE_KEY");
        assert!(provider_key_vars().iter().any(|x| x == "TAZUNA_TEST_ROUTE_IN_USE_KEY"));
    }

    #[test]
    fn a_receipt_names_who_was_paid_or_nobody() {
        assert_eq!(paid_to("https://subrouter.ai/v1", "").as_deref(), Some("SubRouter"));
        assert_eq!(paid_to("https://api.example.com/v1", "").as_deref(), Some("api.example.com"));
        // providers sharing a URL are told apart by the key: a plan's run is the plan's in the run log and the receipt
        assert_eq!([paid_to("https://api.minimax.io/anthropic/v1", "MINIMAX_CODE_API_KEY"), paid_to("https://api.minimax.io/anthropic/v1", "MINIMAX_API_KEY"),
                    paid_to("https://api.fireworks.ai/inference/v1", "FIREPASS_API_KEY"), paid_to("https://api.fireworks.ai/inference/v1/", "FIREWORKS_API_KEY")].map(Option::unwrap),
                   ["MiniMax Plan", "MiniMax", "Fire Pass", "Fireworks"]);
        assert!(paid_to(OMNIROUTE, "").is_none() && paid_to("http://localhost:8080/v1", "").is_none() && paid_to("http://192.168.1.20:8080/v1", "").is_none());
        // the user's own machines pay nobody: by address (IPv6 in its brackets too), by a local name, by a bare name
        for own in ["http://127.0.0.1:11434/v1", "http://10.0.0.5/v1", "http://172.20.1.1:8000/v1", "http://169.254.3.4/v1", "http://100.100.20.3:8080/v1", "http://[::1]:8080/v1",
            "http://[fd12:3456::1]/v1", "http://[fe80::1]:1234/v1", "http://[::ffff:192.168.1.2]/v1", "http://gpu-box.local:11434/v1", "http://nas.lan/v1", "http://host.docker.internal:8080/v1",
            "http://gpu-box:11434/v1", "http://user:pw@localhost:1/v1", "http://0.0.0.0:8000/v1"] {
            assert!(paid_to(own, "").is_none(), "{own}");
        }
        // an address out on the internet is somebody's
        assert!(paid_to("http://8.8.8.8/v1", "").is_some() && paid_to("http://[2001:4860::1]/v1", "").is_some() && paid_to("https://user@api.example.com:443/v1?x=1", "").as_deref() == Some("api.example.com"));
        assert_eq!((host_of("http://[::1]:8080/v1"), host_of("https://a.b:1/x"), host_of("a.b/x")), ("::1", "a.b", "a.b"));
    }

    #[test]
    fn a_tailscale_name_is_the_users_own_machine() {
        // like its 100.64/10 addresses: a machine on the user's tailnet pays nobody
        assert!(paid_to("http://gpu.tail1234.ts.net:11434/v1", "").is_none() && paid_to("http://100.101.1.2:11434/v1", "").is_none());
        assert_eq!(paid_to("https://ts.net.example.com/v1", "").as_deref(), Some("ts.net.example.com"));
    }

    #[test]
    fn a_claude_run_is_on_the_plan_only_when_nothing_says_otherwise() {
        let run = |key: Option<&str>, env: &[&str]| claude_payer(key, |k| env.contains(&k));
        assert_eq!(run(Some("none"), &[]), (true, None), "the subscription's login");
        assert_eq!(run(None, &[]), (true, None), "a recording whose init does not say");
        assert_eq!(run(Some("ANTHROPIC_API_KEY"), &[]), (false, Some("Anthropic API")));
        assert_eq!(run(None, &["ANTHROPIC_API_KEY"]), (false, Some("Anthropic API")), "a claude that does not say, with a key set");
        // claude says "none" under Bedrock, Vertex and a gateway too
        assert_eq!(run(Some("none"), &["CLAUDE_CODE_USE_BEDROCK"]), (false, Some("Bedrock")));
        assert_eq!(run(Some("none"), &["CLAUDE_CODE_USE_VERTEX"]), (false, Some("Vertex")));
        assert_eq!(run(Some("none"), &["ANTHROPIC_AUTH_TOKEN"]), (false, None), "a gateway: billed, to someone not known");
        assert_eq!(run(Some("ANTHROPIC_API_KEY"), &["ANTHROPIC_BASE_URL"]), (false, None), "the key goes to the gateway, not to Anthropic");
    }

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
        let own = |c: &Choice| login_of(c).is_none_or(|l| l.source == Source::Own);
        let cs: Vec<Choice> = catalog(&f, |_| true, &none, 0).into_iter().filter(|c| c.route.is_some() && c.place != "Mortera" && own(c)).collect();
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
    fn a_provider_shows_by_its_name_not_its_host() {
        let row = |url: &str| Choice { model: "m".into(), label: "m".into(), route: Some((url.into(), "K".into(), "0,0,0".into())), place: String::new(),
            efforts: vec![], price: String::new(), note: String::new(), stale: false, state: State::Ready, bench: String::new(), aa: String::new(), pins: vec![] };
        assert_eq!(provider_of(&row("https://api.cheaperinference.com/v1")), "Cheaper Inference");
        assert_eq!(provider_of(&row("https://kenari.id/v1")), "Kenari");
    }

    #[test]
    fn cheaperinference_rows_are_its_text_models_with_tools() {
        let list = serde_json::json!({"data": [
            {"id": "deepseek-v4.1-flash", "type": "text", "provider": "DeepSeek", "features": ["reasoning", "tool-use"], "capabilities": {"reasoning": true},
             "pricing": {"input_per_million": "0.150000", "cache_read_input_per_million": "0.003", "output_per_million": "0.600000"}},
            {"id": "no-tools", "type": "text", "features": ["reasoning"], "capabilities": {}, "pricing": {"input_per_million": "1", "output_per_million": "2"}},
            {"id": "claude-opus-5-5", "type": "text", "features": ["tool-use"], "capabilities": {}, "pricing": {"input_per_million": "5", "output_per_million": "25"}},
            {"id": "an-embedder", "type": "embedding", "features": ["tool-use"], "capabilities": {}, "pricing": {"input_per_million": "0.01", "output_per_million": "0"}},
            {"id": "glyph-cluster", "type": "text", "is_free": true, "features": ["tool-use"], "capabilities": {}, "pricing": {"input_per_million": "0", "output_per_million": "0"}}]});
        let omni = serde_json::json!({"data": [{"id": "auto/thrifty", "owned_by": "combo"}, {"id": "antigravity/gemini-3-pro", "owned_by": "antigravity"},
            {"id": "cc/claude-opus-5-5", "owned_by": "claude"}, {"id": "glm/glm-5.2", "owned_by": "glm"}, {"id": "openai/text-embedding-4", "owned_by": "openai", "kind": "embedding"},
            {"id": "no-think/cc/claude-opus-5", "owned_by": "x"}, {"id": "aihorde/flux", "owned_by": "aihorde", "output_modalities": ["image"]},
            {"id": "dva/old", "owned_by": "dva", "capabilities": {"tool_calling": false}}]});
        let ids = |l: &Value| omniroute_choices(l, &serde_json::json!({}), &|_: &str| true).into_iter().map(|c| c.model).collect::<Vec<_>>();
        assert_eq!(ids(&omni), ["auto/thrifty", "antigravity/gemini-3-pro", "glm/glm-5.2", "no-think/cc/claude-opus-5", "cc/claude-opus-5-5"],
            "every route the agent can use (the user chooses), not the image, embedding or tool-less ones");
        let all = omniroute_choices(&omni, &serde_json::json!({}), &|_: &str| true);
        assert!(all.iter().filter(|c| c.place.contains("Claude subscription")).map(|c| c.model.as_str()).eq(["no-think/cc/claude-opus-5", "cc/claude-opus-5-5"]),
            "the subscription's routes say what they are");
        // one row a model, its levels as efforts, named and placed (a level each was a row until 0.1.172)
        let levels = serde_json::json!({"data": [{"id": "agy/gemini-4-argon-high", "owned_by": "agy"}, {"id": "agy/gemini-4-argon-low", "owned_by": "agy"},
            {"id": "agy/gemini-4-argon-medium", "owned_by": "agy"}, {"id": "cc/claude-opus-5-5", "owned_by": "claude"}, {"id": "claude/claude-opus-5-5", "owned_by": "claude"},
            {"id": "agy/gpt-oss-120b-medium", "owned_by": "agy"}]});
        let rows = omniroute_choices(&levels, &serde_json::json!({}), &|_: &str| true);
        let argon = rows.iter().find(|c| c.model == "agy/gemini-4-argon").unwrap();
        assert_eq!((argon.label.as_str(), argon.place.as_str(), argon.efforts.join(",")), ("Gemini 4 Argon", "OmniRoute · Antigravity CLI", "low,medium,high".into()));
        assert_eq!((resolve(argon, "high").as_str(), same_model(argon, "agy/gemini-4-argon-low"), same_model(argon, "agy/gemini-4-argonx")), ("agy/gemini-4-argon-high", true, false));
        assert_eq!(rows.iter().filter(|c| c.label == "Claude Opus 5.5").count(), 1, "cc/ and claude/ are one route");
        let gpt = rows.iter().find(|c| c.label == "GPT OSS 120B").unwrap();
        assert!(gpt.model == "agy/gpt-oss-120b-medium" && gpt.efforts.is_empty(), "a model with one level runs as listed");
        let row = &all[1];
        assert_eq!((provider_of(row).as_str(), row.route.as_ref().unwrap().0.as_str()), ("OmniRoute", "http://localhost:20128/v1"));
        let rows = cheaper_choices(&list, &serde_json::json!({}), &|h: &str| h == "api.cheaperinference.com");
        assert_eq!(rows.iter().map(|c| c.model.as_str()).collect::<Vec<_>>(), ["deepseek-v4.1-flash", "glyph-cluster"], "text models with tools, no Claude");
        let r = &rows[0];
        assert_eq!((r.route.as_ref().unwrap().0.as_str(), r.route.as_ref().unwrap().2.as_str()), ("https://api.cheaperinference.com/v1", "0.15,0.003,0.6"));
        assert_eq!((r.place.as_str(), r.efforts.len(), rows[1].price.as_str(), rows[1].efforts.len()), ("Cheaper Inference · DeepSeek", 5, "free", 0));
    }

    #[test]
    fn kenari_rows_free_first_with_each_models_efforts() {
        let m = |id: &str, free: bool, tools: bool, levels: serde_json::Value| serde_json::json!({"id": id, "name": id.to_uppercase(), "endpoints": ["chat"], "tool_call": tools,
            "supported_reasoning_levels": levels, "pricing": {"free": free, "input": if free { serde_json::Value::Null } else { serde_json::json!(3_250_000_000.0) }, "output": if free { serde_json::Value::Null } else { serde_json::json!(6_000_000_000.0) }, "cache_read": null}});
        let list = serde_json::json!({"data": [m("deepseek-v4-1-flash", false, true, serde_json::json!(["low", "high", "max"])), m("hy4:free", true, true, serde_json::json!(null)),
            m("no-tools:free", true, false, serde_json::json!(null)), m("claude-fable-5", false, true, serde_json::json!(null)),
            serde_json::json!({"id": "kokoro-tts", "endpoints": ["speech"], "tool_call": false, "pricing": {"free": true, "input": 0.0, "output": 0.0}})]});
        let rows = kenari_choices(&list, &serde_json::json!({}), &|h: &str| h == "kenari.id");
        assert_eq!(rows.iter().map(|c| c.model.as_str()).collect::<Vec<_>>(), ["hy4:free", "deepseek-v4-1-flash", "claude-fable-5"], "chat models with tools, free first, Claude billed by kenari too");
        assert_eq!(rows[1].efforts, ["low", "high", "max"]);
        assert!(rows[0].efforts.is_empty() && rows[0].price == "free" && rows[0].note.contains("50 a day"));
        assert_eq!(rows[1].route.as_ref().unwrap().0, "https://kenari.id/v1");
        assert!(rows[1].price.starts_with("0.199"), "Rp 3,250 per 1M at Rp 16,300 a dollar: {}", rows[1].price);
    }

    #[test]
    fn openrouter_rows_newest_free_first_then_newest_paid() {
        let m = |id: &str, created: i64, price: &str, params: &[&str]| serde_json::json!({"id": id, "created": created,
            "pricing": {"prompt": price, "completion": price}, "supported_parameters": params});
        let list = serde_json::json!({"data": [
            m("old/paid", 100, "0.000001", &["tools"]), m("new/paid", 300, "0.000002", &["tools", "reasoning"]),
            m("old/free:free", 50, "0", &["tools"]), m("new/free:free", 200, "0", &["tools"]),
            m("anthropic/claude-fable-5.2", 400, "0.000008", &["tools"]), m("new/paid:batch", 300, "0.000001", &["tools"]),
            m("chat/only", 500, "0.000001", &["temperature"])]});
        let rows = openrouter_choices(&list, &serde_json::json!({}), &|h: &str| h == "openrouter.ai");
        assert_eq!(rows.iter().map(|c| c.model.as_str()).collect::<Vec<_>>(), ["new/free:free", "old/free:free", "new/paid", "old/paid"],
                   "free first, newest first within each; no Claude, no batch copies, no model without tools");
        assert_eq!((rows[2].price.as_str(), rows[2].efforts.len(), rows[3].efforts.len()), ("2 / 2", 5, 0));
        assert!(rows[0].note.contains("rate-limited") && rows[2].note.starts_with("not checked yet"));
        assert_eq!(provider_of(&rows[0]), "OpenRouter");
    }

    #[test]
    fn subrouter_price_list_gives_cheapest_prices_and_chat_models() {
        let list = serde_json::json!({"data": [
            {"model_name": "kimi-k4", "supported_endpoint_types": ["openai"], "provider_prices": [
                {"input_price": 0.5, "output_price": 2.0}, {"input_price": 0.3, "output_price": 2.5}]},
            {"model_name": "seadance-3", "supported_endpoint_types": ["openai", "openai-video"], "provider_prices": [{"input_price": 1, "output_price": 1}]},
            {"model_name": "no-provider", "supported_endpoint_types": ["openai"], "provider_prices": []}]});
        let c = super::subrouter_catalog(&list).unwrap();
        assert_eq!(c["kimi-k4"], serde_json::json!({"price": [0.3, 2.0], "providers": 2, "category": "chat"}), "each price at its cheapest provider");
        assert_eq!(c["seadance-3"]["category"], "other", "a video model is not listed as chat");
        assert!(c.get("no-provider").is_none(), "a model nobody serves is left out");
        assert!(super::subrouter_catalog(&serde_json::json!({"data": []})).is_none());
    }

    #[test]
    fn mortera_rows_are_its_text_models() {
        let list = serde_json::json!({"data": [
            {"id": "deepseek-v4.1-flash-1017-uncensored", "architecture": {"output_modalities": ["text"]}, "pricing": {"payg_per_1m_idr": {"in": 391, "out": 1630, "cache": 39}}},
            {"id": "muse-spark-1.3", "pricing": {"price_per_1m_idr": {"in": 1630, "out": 3260}}},
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
        // both names of the price field (renamed 2026-11-21: every model had left /model)
        assert!(rows[1].price.contains('/'), "a row priced from `price_per_1m_idr`");
    }

    #[test]
    fn subrouter_balance_is_in_the_unit_its_status_names() {
        let usd = serde_json::json!({"data": {"quota_display_type": "USD", "usd_exchange_rate": 6.8}});
        let cny = serde_json::json!({"data": {"quota_display_type": "CNY", "usd_exchange_rate": 6.8}});
        assert!((in_usd(0.315, &usd) - 0.315).abs() < 1e-9, "the user's $0.32 read as $0.046 with the old division");
        assert!((in_usd(6.8, &cny) - 1.0).abs() < 1e-9);
        assert!((in_usd(0.315, &serde_json::json!(null)) - 0.315).abs() < 1e-9, "no status: as the endpoints give it");
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
                                          "gpt-6.1-sol": {"price": [0.03, 0.2], "providers": 40},
                                          "kimi-k3": {"price": [0.5, 2], "providers": 2, "category": "chat"},
                                          "flux-pro": {"price": [1, 1], "providers": 1, "category": "image"}});
        d["aa"] = serde_json::json!({"glm-5-3": 41.0, "gpt-6-1-sol": 54.5});
        d["bench"] = serde_json::json!({"gpt-6.1-sol": {"judge": 6.6, "runs": 51, "minutes": 19}});
        std::fs::write(&f, d.to_string()).unwrap();
        let c: Vec<Choice> = catalog(&f, |h| h == "subrouter.ai", &KeyModels::new(), now).into_iter()
            .filter(|c| c.place != "Mortera" && login_of(c).is_none_or(|l| l.source == Source::Own)).collect();
        assert_eq!(c.iter().map(|c| c.label.as_str()).collect::<Vec<_>>(),
                   ["Opus 5.5", "Sonnet 5.5", "Haiku 5.5", "Fable 5.2", "deepseek-v4.1-flash-1017", "deepseek-v4.1-flash-1017 · fast", "gpt-6.1-sol", "glm-5.3", "kimi-k3"],
                   "routed models once; then every chat model, scored ones first (no image or video model)");
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
