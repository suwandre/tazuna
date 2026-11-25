//! TokenRouter's public price catalog also tells us which API each model takes.
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::Value;

pub(crate) const URL: &str = "https://api.tokenrouter.com/v1";
pub(crate) const KEY: &str = "TOKENROUTER_API_KEY";
const PRICING_URL: &str = "https://api.tokenrouter.com/api/pricing";

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Wire {
    Chat,
    Responses,
    Messages,
}

pub(crate) fn fetch() -> Result<String> {
    fetch_from(PRICING_URL)
}

/// The catalog at `url`. Public: never send the user's key to the pricing endpoint, so no Authorization header at all.
fn fetch_from(url: &str) -> Result<String> {
    let http: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(15))).build().into();
    let mut r = http.get(url).call()?;
    Ok(r.body_mut().with_config().limit(4 << 20).read_to_string()?)
}

pub(super) fn models(list: &Value) -> impl Iterator<Item = (&str, &Value, Wire)> {
    list["data"].as_array().into_iter().flatten().filter_map(|m| {
        if !m["tags"].as_str()?.trim().eq_ignore_ascii_case("text") {
            return None;
        }
        let id = m["model_name"].as_str().filter(|id| !id.trim().is_empty())?;
        let endpoints = m["supported_endpoint_types"].as_array()?;
        let has = |endpoint: &str| endpoints.iter().any(|e| e == endpoint);
        let wire = if id.starts_with("openai/") && has("openai-response") {
            Wire::Responses
        } else if has("openai") {
            Wire::Chat
        } else if has("openai-response") {
            Wire::Responses
        } else if has("anthropic") || has("anthropic-compatible") {
            Wire::Messages
        } else {
            return None;
        };
        Some((id, m, wire))
    })
}

pub(crate) type Rates = (f64, f64, f64);

/// USD per million: input, output, cache reads, at the base context tier (tier 0) where the catalog tiers a model: the
/// highest tier read gpt-6.2-sol at $4/$15 for its $2/$10, 1.5-2x every run's cost and its $1 cap (review, 2026-11-25). A
/// tier without a usable price leaves the model unpriced. These are public rates, not a quote for the group on the user's
/// key.
/// ponytail: one rate per model; a prompt past the first tier's size is billed at the long-context tier's rates (sol: $4
/// in, $15 out), so pick the tier per call by its prompt size if long-context runs must cost exactly.
fn rates(list: &Value, id: &str, m: &Value) -> Option<Rates> {
    if m["quota_type"].as_u64()? != 0 {
        return None; // we cannot price per-request models with token rates
    }
    let rate = |kind: &str, base: f64| -> Option<f64> {
        let tier = &list["tiered_pricing"][id][kind];
        if !super::price_ok(base) {
            return None;
        }
        if tier.is_null() {
            return Some(base);
        }
        tier[0]["pricePerUnit"].as_f64().filter(|n| super::price_ok(*n))
    };
    // The catalog's model_ratio is relative to $2 per million input tokens; output and cache are relative to input.
    let input_base = 2.0 * m["model_ratio"].as_f64()?;
    let input = rate("input", input_base)?;
    let output = rate("output", input_base * m["completion_ratio"].as_f64()?)?;
    let cached = rate("cache_read", m["cache_ratio"].as_f64().map_or(input, |r| input_base * r))?;
    Some((input, output, cached))
}

/// A model's effort levels as models.dev lists them for the id after the slash, from its owner's catalog first
/// (openai/gpt-5.4: openai's, without max; gpt-5.5-pro without low); None where models.dev has no such id.
fn listed_levels(md: &str, id: &str) -> Option<Vec<String>> {
    let (owner, short) = id.split_once('/').unwrap_or(("", id));
    let (mut section, mut any) = ("", None);
    for line in md.lines() {
        if let Some(s) = line.strip_prefix('=') {
            section = s;
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 5 || f[0].rsplit('/').next() != Some(short) {
            continue;
        }
        if section == owner {
            any = Some(f[4]);
            break;
        }
        any = any.or(Some(f[4]));
    }
    any.map(|ls| match ls {
        "*" => super::LEVELS.iter().map(|e| e.to_string()).collect(),
        ls => ls.split_whitespace().map(String::from).collect(),
    })
}

pub(super) fn choices(l: &super::Login, list: &Value, md: &str, file: &Value, trusted: &impl Fn(&str) -> bool) -> Vec<super::Choice> {
    models(list).map(|(id, m, wire)| {
        // Claude on Messages: its own levels where it takes adaptive thinking (shape_messages strips effort elsewhere); the
        // rest as models.dev lists the id after the slash, else the levels these families take
        let short = id.rsplit('/').next().unwrap_or(id);
        let efforts: Vec<String> = match wire {
            Wire::Messages if short.starts_with("claude-") => if adaptive(id) { super::claude_levels(short).0.iter().map(|e| e.to_string()).collect() } else { vec![] },
            _ => listed_levels(md, id).unwrap_or_else(|| match (wire, id) {
                (Wire::Chat, id) if id.starts_with("deepseek/deepseek-v4") => vec!["high".into(), "max".into()],
                (Wire::Responses, id) if id.starts_with("openai/") => super::LEVELS.iter().map(|e| e.to_string()).collect(),
                _ => vec![],
            }),
        };
        let price = rates(list, id, m).map(|(i, o, c)| (i, o, Some(c)));
        let mut row = super::login_row(l, id, price, efforts, file, trusted);
        if !list["tiered_pricing"][id].is_null() && !row.price.is_empty() {
            row.note.push_str(" · priced at its base context tier: long prompts cost more");
        }
        row
    }).collect()
}

/// The engine reads the same catalog as the picker. A direct CLI run can fetch it even before /login. Ok(None): the
/// catalog was read and does not list `id` (not retried); an Err is a catalog that could not be read (retried).
pub(crate) fn model(id: &str) -> Result<Option<(Wire, Option<Rates>)>> {
    model_in(crate::labels::dir().ok().as_deref(), id, fetch)
}

/// `model` with the data folder and the fetch given: the cached catalog when it lists `id`, else a fresh one.
fn model_in(dir: Option<&Path>, id: &str, fetch: impl FnOnce() -> Result<String>) -> Result<Option<(Wire, Option<Rates>)>> {
    let find = |list: &Value| models(list).find(|(name, _, _)| *name == id)
        .map(|(_, m, wire)| (wire, rates(list, id, m)));
    if let Some(found) = dir.and_then(cached).as_ref().and_then(find) {
        return Ok(Some(found));
    }
    let list = serde_json::from_str(&fetch().context("could not load TokenRouter's model catalog")?)?;
    Ok(find(&list))
}

/// A Claude model that takes adaptive thinking and effort: a known version from 4.6 on (anthropic/claude-sonnet-4.6,
/// claude-opus-5.5, claude-opus-4-6). Anything else, an unknown name included, gets neither (an allow-list: the four
/// older names it had let a fifth one through, review 2026-11-25).
pub(crate) fn adaptive(id: &str) -> bool {
    let parts: Vec<&str> = id.rsplit('/').next().unwrap_or(id).split('-').collect();
    let Some(i) = parts.iter().position(|p| p.starts_with(|c: char| c.is_ascii_digit())) else { return false };
    let version = match parts[i].split_once('.') {
        Some((a, b)) => a.parse::<u32>().ok().zip(b.parse::<u32>().ok()),
        // "4-6" is 4.6; "4-20250514" is 4 with a date
        None => parts[i].parse::<u32>().ok().map(|a| (a, parts.get(i + 1).filter(|p| p.len() <= 2).and_then(|p| p.parse().ok()).unwrap_or(0))),
    };
    parts[0] == "claude" && version.is_some_and(|v| v >= (4, 6))
}

pub(crate) fn shape_messages(id: &str, body: &mut Value) {
    // Models before adaptive thinking: their default, without the newer thinking/effort fields.
    if !adaptive(id) && let Some(o) = body.as_object_mut() {
        o.remove("thinking");
        o.remove("output_config");
    }
}

fn cached(dir: &Path) -> Option<Value> {
    let l = super::LOGINS.iter().find(|l| l.url == URL)?;
    serde_json::from_str(&std::fs::read_to_string(dir.join(super::list_file(l))).ok()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tokenrouter_catalog_drives_picker_prices_and_model_apis() {
        let registration = super::super::LOGINS.iter().find(|l| l.vars == [KEY]).unwrap();
        assert_eq!((registration.name, registration.url), ("TokenRouter", URL));
        assert_eq!(registration.source, super::super::Source::List);
        assert_eq!(super::super::configured_keyed_lists(None, |var| var == KEY).iter().map(|(l, cached)| (l.name, *cached)).collect::<Vec<_>>(),
            [("TokenRouter", false)]);
        let list = json!({"data": [
            {"model_name": "deepseek/deepseek-v4.1-flash-1017-free", "tags": "Text", "quota_type": 0,
             "model_ratio": 0, "completion_ratio": 1, "supported_endpoint_types": ["openai", "openai-response"]},
            {"model_name": "deepseek/deepseek-v4.1-flash-1017", "tags": "Text", "quota_type": 0,
             "model_ratio": 0.0375, "completion_ratio": 4, "cache_ratio": 0.02, "supported_endpoint_types": ["openai", "openai-response"]},
            {"model_name": "openai/gpt-6.2-sol", "tags": "Text", "quota_type": 0, "model_ratio": 1, "completion_ratio": 5,
             "cache_ratio": 0.05, "supported_endpoint_types": ["openai-response"]},
            {"model_name": "openai/gpt-5.4-mini", "tags": "Text", "quota_type": 0, "model_ratio": 0.375, "completion_ratio": 6,
             "supported_endpoint_types": ["openai"]},
            {"model_name": "anthropic/claude-haiku-4.5", "tags": "Text", "quota_type": 0, "model_ratio": 0.5, "completion_ratio": 5,
             "supported_endpoint_types": ["anthropic-compatible"]},
            {"model_name": "google/gemini-3.7-flash", "tags": "Text", "supported_endpoint_types": ["gemini"]},
            {"model_name": "openai/gpt-5-image-mini", "tags": " Image", "supported_endpoint_types": ["openai"]},
            {"model_name": "invalid-price", "tags": "Text", "quota_type": 0, "model_ratio": -1, "completion_ratio": 2,
             "supported_endpoint_types": ["openai"]}
        ], "tiered_pricing": {"openai/gpt-6.2-sol": {
            "input": [{"pricePerUnit": 2}, {"pricePerUnit": 4}], "output": [{"pricePerUnit": 10}, {"pricePerUnit": 15}],
            "cache_read": [{"pricePerUnit": 0.1}, {"pricePerUnit": 0.2}]
        }}});
        assert_eq!(models(&list).map(|(_, _, wire)| wire).collect::<Vec<_>>(),
            [Wire::Chat, Wire::Chat, Wire::Responses, Wire::Chat, Wire::Messages, Wire::Chat]);

        // The same rows are consumed by /model and the desktop picker. Test with an isolated key source.
        super::super::ENV_AT.set(Some(vec![(KEY.into(), "test-key".into())]));
        let rows = choices(registration, &list, "", &json!({}), &|h| h == "api.tokenrouter.com");
        assert!(rows.iter().all(|r| r.state == super::super::State::Ready && super::super::provider_of(r) == "TokenRouter"));
        let price = |i: usize| rows[i].route.as_ref().unwrap().2.as_str();
        assert_eq!((rows[0].price.as_str(), price(0), price(1)), ("free", "0,0,0", "0.075,0.0015,0.3"));
        // a tiered model at its base tier: sol's $2/$10, not the long-context $4/$15
        assert_eq!((price(2), price(3), price(4), price(5)), ("2,0.1,10", "0.75,0.75,4.5", "1,1,5", ""));
        assert!(rows[2].note.contains("base context tier") && rows[5].note.contains("no price listed"));
        assert_eq!(choices(registration, &list, "", &json!({}), &|_| false)[0].state, super::super::State::Untrusted);
        super::super::ENV_AT.set(None);
    }

    #[test]
    fn tokenrouter_prices_only_what_it_can() {
        let list = json!({"data": [
            {"model_name": "per-request", "tags": "Text", "quota_type": 1, "model_ratio": 1, "completion_ratio": 2, "supported_endpoint_types": ["openai"]},
            {"model_name": "bad-base-tier", "tags": "Text", "quota_type": 0, "model_ratio": 1, "completion_ratio": 2, "supported_endpoint_types": ["openai"]},
            {"model_name": "bad-long-tier", "tags": "Text", "quota_type": 0, "model_ratio": 1, "completion_ratio": 2, "supported_endpoint_types": ["openai"]}
        ], "tiered_pricing": {
            "bad-base-tier": {"input": [{"pricePerUnit": -1}, {"pricePerUnit": 4}]},
            "bad-long-tier": {"input": [{"pricePerUnit": 2}, {"pricePerUnit": "x"}]}
        }});
        let rate = |id: &str| models(&list).find(|m| m.0 == id).and_then(|(id, m, _)| rates(&list, id, m));
        // a per-request model has no token rates; a base tier without a usable price leaves it unpriced, a later one's is not read
        assert_eq!((rate("per-request"), rate("bad-base-tier"), rate("bad-long-tier")), (None, None, Some((2.0, 4.0, 2.0))));
    }

    #[test]
    fn tokenrouter_levels_follow_models_dev_and_claude() {
        let list = json!({"data": [
            {"model_name": "openai/gpt-5.4", "tags": "Text", "supported_endpoint_types": ["openai-response"]},
            {"model_name": "openai/gpt-5.5-pro", "tags": "Text", "supported_endpoint_types": ["openai-response"]},
            {"model_name": "openai/gpt-7-unknown", "tags": "Text", "supported_endpoint_types": ["openai-response"]},
            {"model_name": "anthropic/claude-sonnet-4.6", "tags": "Text", "supported_endpoint_types": ["anthropic"]},
            {"model_name": "anthropic/claude-haiku-4.5", "tags": "Text", "supported_endpoint_types": ["anthropic"]}
        ]});
        // the owner's catalog first: kilo's line for gpt-5.4 says max, openai's does not
        let md = "=kilo\nopenai/gpt-5.4\t2.5\t15\t0.25\tlow medium high xhigh max\n=openai\ngpt-5.4\t2.5\t15\t0.25\tnone low medium high xhigh\ngpt-5.5-pro\t30\t180\t\tmedium high xhigh\n";
        let registration = super::super::LOGINS.iter().find(|l| l.vars == [KEY]).unwrap();
        let rows = choices(registration, &list, md, &json!({}), &|_| true);
        let levels: Vec<String> = rows.iter().map(|r| r.efforts.join(" ")).collect();
        assert_eq!(levels, ["none low medium high xhigh", "medium high xhigh", "low medium high xhigh max", "low medium high xhigh max", ""]);
    }

    #[test]
    fn tokenrouter_reads_its_cached_catalog_then_fetches() {
        let dir = std::env::temp_dir().join(format!("tazuna-tokenrouter-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let entry = |id: &str| json!({"data": [{"model_name": id, "tags": "Text", "quota_type": 0, "model_ratio": 1, "completion_ratio": 5, "supported_endpoint_types": ["openai"]}]}).to_string();
        std::fs::write(dir.join("tokenrouter-models.json"), entry("cached/m")).unwrap();
        let unused = || -> Result<String> { panic!("the cached catalog lists it: no fetch") };
        assert_eq!(model_in(Some(&dir), "cached/m", unused).unwrap(), Some((Wire::Chat, Some((2.0, 10.0, 2.0)))));
        // not in the cached copy: a fresh catalog; not there either: None (not listed, not retried)
        assert_eq!(model_in(Some(&dir), "new/m", || Ok(entry("new/m"))).unwrap().map(|f| f.0), Some(Wire::Chat));
        assert_eq!(model_in(Some(&dir), "gone/m", || Ok(entry("new/m"))).unwrap(), None);
        // a catalog that could not be read is an error, which the engine retries
        assert!(model_in(None, "new/m", || anyhow::bail!("timed out")).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn tokenrouter_catalog_goes_out_without_a_key_and_needs_a_text_model() {
        use std::io::{Read, Write};
        let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/api/pricing", server.local_addr().unwrap());
        let seen = std::thread::spawn(move || {
            let (mut c, _) = server.accept().unwrap();
            let mut buf = [0u8; 4096];
            let n = c.read(&mut buf).unwrap();
            let body = r#"{"data": []}"#;
            write!(c, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            String::from_utf8_lossy(&buf[..n]).to_lowercase()
        });
        super::super::ENV_AT.set(Some(vec![(KEY.into(), "sk-secret".into())]));
        assert_eq!(fetch_from(&url).unwrap(), r#"{"data": []}"#);
        let request = seen.join().unwrap();
        assert!(!request.contains("authorization") && !request.contains("sk-secret"), "{request}");
        super::super::ENV_AT.set(None);
        // kept only with a text model in it
        let l = super::super::LOGINS.iter().find(|l| l.vars == [KEY]).unwrap();
        let image = json!({"data": [{"model_name": "x/painter", "tags": "Image", "supported_endpoint_types": ["openai"]}]}).to_string();
        let text = json!({"data": [{"model_name": "x/m", "tags": "Text", "supported_endpoint_types": ["openai"]}]}).to_string();
        assert!(!super::super::list_ok(l, &image) && !super::super::list_ok(l, "not json") && super::super::list_ok(l, &text));
    }

    #[test]
    fn tokenrouter_rows_come_from_its_file_before_a_key_is_saved() {
        // the catalog file beside the routes: /model lists its models, priced, before the key (NoKey until then), each
        // routed to TokenRouter, not to OpenRouter's same id
        let dir = std::env::temp_dir().join(format!("tazuna-tokenrouter-rows-{}-{:?}", std::process::id(), std::thread::current().id()).replace(['(', ')'], ""));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        super::super::ENV_AT.set(Some(vec![]));
        super::super::KEYS_AT.set(Some(dir.join("keys.json")));
        std::fs::write(dir.join("routes.json"), r#"{"routes": {}}"#).unwrap();
        std::fs::write(dir.join("tokenrouter-models.json"), json!({"data": [{"model_name": "openai/gpt-6.2-sol", "tags": "Text", "quota_type": 0,
            "model_ratio": 1, "completion_ratio": 5, "cache_ratio": 0.05, "supported_endpoint_types": ["openai-response"]}]}).to_string()).unwrap();
        let cs = super::super::catalog(&dir.join("routes.json"), |_| true, &super::super::KeyModels::new(), u64::MAX / 2);
        let row = cs.iter().find(|c| c.place == "TokenRouter" && c.model == "openai/gpt-6.2-sol").expect("a TokenRouter row");
        assert_eq!((row.route.as_ref().map(|r| (r.0.as_str(), r.1.as_str(), r.2.as_str())), row.state), (Some((URL, KEY, "2,0.1,10")), super::super::State::NoKey));
        super::super::ENV_AT.set(None);
        super::super::KEYS_AT.set(None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn tokenrouter_messages_only_use_adaptive_thinking_where_supported() {
        let request = json!({"thinking": {"type": "adaptive"}, "output_config": {"effort": "high"}, "messages": []});
        let mut legacy = request.clone();
        shape_messages("anthropic/claude-haiku-4.5", &mut legacy);
        assert_eq!(legacy, json!({"messages": []}));
        let mut modern = request.clone();
        shape_messages("anthropic/claude-haiku-5.5", &mut modern);
        assert_eq!(modern, request);
        // an allow-list: known versions from 4.6 on keep them, anything else (a name it does not know) does not
        assert!(["anthropic/claude-sonnet-4.6", "anthropic/claude-opus-4-6", "claude-opus-5.5"].iter().all(|id| adaptive(id)));
        assert!(!["anthropic/claude-sonnet-4", "anthropic/claude-sonnet-4-20250514", "anthropic/claude-opus-4.5", "anthropic/claude-mystery", "openai/gpt-6.2-sol"].iter().any(|id| adaptive(id)));
    }
}
