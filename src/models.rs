//! The model picker's list (`/model`): Claude's models through the `claude` CLI, and the own loop's routes that
//! bench/routes.py checked (~/.local/share/tazuna/routes.json: providers, price, cache share, the effort levels the model
//! really has, when checked). A route is ready when its key is in the environment and its host on the trusted list;
//! otherwise it is listed, greyed out or marked, with the reason.

use serde_json::Value;
use std::path::Path;

pub const LEVELS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];
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
}

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
}

/// Claude's models, then the checked routes; `trusted` says whether a provider host may see the code.
pub fn catalog(routes: &Path, trusted: impl Fn(&str) -> bool, now: u64) -> Vec<Choice> {
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
    }).collect();
    let file: Value = std::fs::read_to_string(routes).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let mut rs: Vec<(&String, &Value)> = file["routes"].as_object().map(|o| o.iter().collect()).unwrap_or_default();
    rs.sort_by_key(|(m, _)| m.to_string());
    for (name, r) in rs {
        let (Some(url), Some(var)) = (r["url"].as_str(), r["key_env"].as_str()) else { continue };
        // a second route for one model ("deepseek-v4.1-flash-1017@fast": another provider set) names its model
        let model = r["model"].as_str().map(String::from).unwrap_or_else(|| name.clone());
        let label = name.split_once('@').map_or(name.clone(), |(m, v)| format!("{m} · {v}"));
        let (input, output) = (r["price"][0].as_f64().unwrap_or(0.0), r["price"][1].as_f64().unwrap_or(0.0));
        let host = url.split("://").nth(1).unwrap_or(url).split(['/', ':']).next().unwrap_or_default();
        let days = now.saturating_sub(r["vetted_at"].as_u64().unwrap_or(0)) / 86_400;
        let providers: Vec<&str> = r["providers"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        let efforts: Vec<String> = r["efforts"].as_array().into_iter().flatten().filter_map(|e| e.as_str().map(String::from)).collect();
        let state = if std::env::var_os(var).is_none() {
            State::NoKey
        } else if !trusted(host) {
            State::Untrusted
        } else {
            State::Ready
        };
        out.push(Choice {
            model,
            label,
            // the cached-input price is not in the provider list: a tenth of input, the usual discount
            route: Some((url.to_string(), var.to_string(), format!("{input},{},{output}", (input / 10.0 * 1e6).round() / 1e6))),
            place: format!("SubRouter · {}", providers.join(", ")),
            efforts,
            price: format!("${input} / ${output} per M"),
            note: format!("cached {}% · checked {}", r["cached"].as_u64().unwrap_or(0), match days {
                0 => "today".to_string(),
                1 => "yesterday".to_string(),
                d => format!("{d} days ago"),
            }),
            stale: days > STALE_DAYS,
            state,
        });
    }
    out
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
    fn lists_claude_then_checked_routes_with_their_state() {
        let dir = std::env::temp_dir().join(format!("tazuna-models-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("routes.json");
        std::fs::write(&f, r#"{"routes": {
            "deepseek-v4.1-flash-1017": {"url": "https://subrouter.ai/v1", "key_env": "TAZUNA_TEST_NO_SUCH_KEY", "providers": ["lingai"],
                "price": [0.022, 0.088], "cached": 81, "efforts": ["high", "max"], "vetted_at": 1000000},
            "deepseek-v4.1-flash-1017@fast": {"model": "deepseek-v4.1-flash-1017", "url": "https://subrouter.ai/v1", "key_env": "PATH",
                "providers": ["lingai"], "price": [0.022, 0.088], "cached": 89, "efforts": ["high", "max"], "vetted_at": 1000000},
            "gpt-6.1-sol": {"url": "https://subrouter.ai/v1", "key_env": "PATH", "providers": ["kriora"],
                "price": [0.0387, 0.1935], "cached": 91, "efforts": ["low", "high", "xhigh", "max"], "vetted_at": 1000000}}}"#).unwrap();
        let now = 1000000 + 9 * 86_400;
        let c = catalog(&f, |h| h == "subrouter.ai", now);
        assert_eq!(c.iter().map(|c| c.label.as_str()).collect::<Vec<_>>(),
                   ["Opus 5.5", "Sonnet 5.5", "Haiku 5.5", "Fable 5.1", "deepseek-v4.1-flash-1017", "deepseek-v4.1-flash-1017 · fast", "gpt-6.1-sol"]);
        assert_eq!((c[5].model.as_str(), c[5].place.as_str()), ("deepseek-v4.1-flash-1017", "SubRouter · lingai"));
        let c: Vec<Choice> = c.into_iter().filter(|c| !c.label.ends_with("fast")).collect();
        assert!(c[0].route.is_none() && c[0].state == State::Ready);
        assert_eq!(c[4].state, State::NoKey, "no key in the environment");
        assert_eq!((c[5].state, c[5].stale), (State::Ready, true), "key set (PATH stands in), host trusted, checked 9 days ago");
        assert_eq!(c[4].efforts, ["high", "max"]);
        assert_eq!(c[5].route.as_ref().unwrap().2, "0.0387,0.00387,0.1935");
        assert_eq!(catalog(&f, |_| false, now)[5].state, State::Untrusted);
        assert_eq!(c[4].efforts[start_effort(&c[4], "claude-opus-5-5", "medium")], "high");
        assert_eq!(c[0].efforts[start_effort(&c[0], "claude-opus-5-5", "xhigh")], "xhigh");
        std::fs::remove_dir_all(&dir).ok();
    }
}
