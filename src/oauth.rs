//! Sign-in logins for the own loop (2026-11-22): subscription plans reached through an OAuth sign-in instead of an API
//! key. ChatGPT through OpenAI's own "Sign in with ChatGPT" for open-source apps (developers.openai.com/siwc); GitHub
//! Copilot and SuperGrok send another app's identity (its OAuth client, its headers), so they stay opt-in behind the
//! warning on their row (`models::Login::note`), confirmed once. Their endpoints, client ids and headers as omp
//! (oh-my-pi, MIT, github.com/can1357/oh-my-pi) has them, reimplemented here. Claude is never one: it runs on the claude
//! CLI's own login only.
//! - One record per login, ~/.config/tazuna/logins/ID.json (0600, written whole through a rename): its tokens, when they
//!   run out, the models it offers, and what outlives a sign-out (ChatGPT's issued client and this machine's host id, the
//!   warning confirmed).
//! - Tokens never go into an environment: the engine reads the record and renews the access token itself, one process at
//!   a time (a lock file beside the record), since ChatGPT's refresh token is replaced at every renewal.
//! - A model of a sign-in is never picked by tazuna on its own: its rows are in /model, and only the user picks one.

use crate::mcpc::{b64url, enc, now, read_json, sha256, short_agent, wait_callback, write_private};
use crate::models::Login;
use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};
use std::{net::TcpListener, path::PathBuf, time::Duration};

const OPENAI_AUTH: &str = "https://auth.openai.com";
const OPENAI_API: &str = "https://api.openai.com/v1";
const CHATGPT_SCOPES: &str = "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct";
/// The scope that lets requests use the ChatGPT plan; a sign-in without it is an identity only.
const PLAN_SCOPE: &str = "chatgpt.tokens.use.direct";
/// OpenCode's GitHub app (a narrow read:user grant), as omp signs in to Copilot.
const GITHUB_CLIENT: &str = "Ov23li8tweQw6odWQebz";
/// Copilot CLI's identity on Copilot's API (omp's `COPILOT_CAPI_IDENTITY_HEADERS`).
const COPILOT_VERSION: &str = "copilot/1.0.82";
/// xAI's Grok CLI client, as omp (from hermes-agent) signs in to SuperGrok.
const XAI_CLIENT: &str = "b1a00492-073a-47ea-816f-4c329264a828";
const XAI_SCOPES: &str = "openid profile email offline_access grok-cli:access api:access";

#[cfg(test)]
thread_local! {
    /// Tests: every endpoint on a stand-in server (same path), the records in a folder of their own, a fixed nonce.
    pub(crate) static STAND_IN: std::cell::RefCell<Option<(String, PathBuf)>> = const { std::cell::RefCell::new(None) };
}

/// An endpoint's address (in tests on the stand-in server, same path).
fn at(url: &str) -> String {
    #[cfg(test)]
    if let Some((base, _)) = STAND_IN.with_borrow(|s| s.clone()) {
        let rest = url.split_once("://").map_or(url, |x| x.1);
        return format!("{base}/{}", rest.split_once('/').map_or("", |x| x.1));
    }
    url.to_string()
}

fn dir() -> Result<PathBuf> {
    // tests never read the machine's own records: a stand-in's folder, else one that stays empty
    #[cfg(test)]
    return Ok(STAND_IN.with_borrow(|s| s.clone()).map_or_else(|| std::env::temp_dir().join(format!("tazuna-no-logins-{}", std::process::id())), |s| s.1));
    #[cfg(not(test))]
    Ok(crate::platform::home().context("HOME not set")?.join(".config/tazuna/logins"))
}

fn path(l: &Login) -> Result<PathBuf> {
    Ok(dir()?.join(format!("{}.json", l.sign_in())))
}

/// The login's record (`{}` when there is none).
fn load(l: &Login) -> Value {
    path(l).map(|p| read_json(&p)).unwrap_or_else(|_| json!({}))
}

fn save(l: &Login, rec: &Value) -> Result<()> {
    write_private(&path(l)?, rec)
}

/// Signed in: a token is stored.
#[inline(never)]
pub fn signed_in(l: &Login) -> bool {
    load(l)["access_token"].is_string()
}

/// Signed in to ChatGPT with plan usage declined at the consent page: the identity alone, no model to offer. Kept through
/// a sign-out, so the next sign-in asks for consent again.
pub fn plan_declined(l: &Login) -> bool {
    load(l)["plan_declined"] == true
}

/// Why a sign-in's models cannot be picked now, for /model and the window (None: they can).
pub fn why_not(l: &Login) -> Option<String> {
    let rec = load(l);
    if !rec["access_token"].is_string() {
        Some(format!("not signed in: /login signs in to {}", l.name))
    } else if rec["plan_declined"] == true {
        Some("ChatGPT plan use was not allowed at sign-in: /login, enter, and allow it".into())
    } else if !rec["models"].as_array().is_some_and(|m| !m.is_empty()) {
        Some(format!("{}'s model list has not loaded yet: tazuna asks again at its next start", l.name))
    } else {
        None
    }
}

/// The warning on its row confirmed once (it stays through a sign-out).
#[inline(never)]
pub fn confirmed(l: &Login) -> bool {
    load(l)["confirmed"] == true
}

#[inline(never)]
pub fn confirm(l: &Login) -> Result<()> {
    update(l, |r| r["confirmed"] = json!(true))
}

/// A change to the record under the lock: a renewal running in another process at the same moment is neither lost nor
/// written over with the tokens it replaced.
fn update(l: &Login, f: impl FnOnce(&mut Value)) -> Result<()> {
    locked(l, || {
        let mut rec = load(l);
        f(&mut rec);
        save(l, &rec)
    })
}

/// The models the sign-in offers, as its record keeps them: `{"id", "name", "api"}`.
pub fn models(l: &Login) -> Vec<Value> {
    load(l)["models"].as_array().cloned().unwrap_or_default()
}

/// What `/data` says is stored: each signed-in login and its file (names, never values).
#[inline(never)]
pub fn stored() -> Vec<String> {
    crate::models::LOGINS.iter().filter(|l| !l.sign_in().is_empty() && signed_in(l))
        .filter_map(|l| Some(format!("{} (tokens in {}, readable by you only)", l.name, path(l).ok()?.display()))).collect()
}

/// A form POST answered with JSON, any status (OAuth endpoints answer errors as 400 with a JSON body).
fn post_form(url: &str, form: &[(&str, &str)]) -> Result<(u16, Value)> {
    let mut r = short_agent().post(url).header("Accept", "application/json").send_form(form.iter().copied())?;
    let status = r.status().as_u16();
    Ok((status, serde_json::from_str(&r.body_mut().read_to_string()?).unwrap_or_default()))
}

fn get_json(url: &str, auth: &str, headers: &[(&str, &str)]) -> Result<Value> {
    let mut req = short_agent().get(url).header("Accept", "application/json");
    if !auth.is_empty() {
        req = req.header("Authorization", auth);
    }
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let mut r = req.call()?;
    let status = r.status().as_u16();
    let text = r.body_mut().with_config().limit(20 << 20).read_to_string()?;
    if status != 200 {
        bail!("HTTP {status}");
    }
    Ok(serde_json::from_str(&text)?)
}

/// An OAuth error body in a few words, never a token: "invalid_grant: ...".
fn oauth_error(v: &Value) -> String {
    let e = if v["error"].is_object() { &v["error"] } else { v };
    let code = e["code"].as_str().or(e["error"].as_str()).or(v["error"].as_str()).unwrap_or("refused");
    let text = e["message"].as_str().or(v["error_description"].as_str()).unwrap_or_default();
    format!("{code}{}{}", if text.is_empty() { "" } else { ": " }, text.chars().take(160).collect::<String>())
}

/// A token answer's fields into the record: access token, a replacement refresh token, when it runs out, its scopes.
fn take_tokens(rec: &mut Value, t: &Value) {
    crate::mcpc::store(rec, t);
    if let Some(s) = t["scope"].as_str() {
        rec["scopes"] = json!(s.split_whitespace().collect::<Vec<_>>());
    }
    rec["saved_at"] = json!(now());
}

/// One process changes a login's record at a time: the system's lock on a file beside it (released when the holder
/// ends, a crash included; the file stays, since a removed lock file lets two holders lock two files). Two engines
/// renewing ChatGPT's rotating refresh token at once would leave one of them signed out.
fn locked<T>(l: &Login, f: impl FnOnce() -> Result<T>) -> Result<T> {
    let d = dir()?;
    let _ = crate::platform::private_dir(&d, true);
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create(true).truncate(false);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600);
    let file = o.open(d.join(format!("{}.lock", l.sign_in())))?;
    let until = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        match file.try_lock() {
            Ok(()) => break,
            Err(std::fs::TryLockError::WouldBlock) if std::time::Instant::now() < until => std::thread::sleep(Duration::from_millis(50)),
            Err(std::fs::TryLockError::WouldBlock) => bail!("{}: another tazuna is renewing the sign-in; try again", l.name),
            Err(std::fs::TryLockError::Error(e)) => return Err(e.into()),
        }
    }
    let r = f();
    let _ = file.unlock();
    r
}

/// The access token for a call: the stored one while it has 5 minutes left, else renewed. `stale`: the one a provider
/// just refused (401), renewed unless another process already did.
pub fn access_token(l: &Login, stale: Option<&str>) -> Result<String> {
    let fresh = |r: &Value| r["access_token"].as_str().filter(|t| Some(*t) != stale && r["expires_at"].as_u64().is_none_or(|e| e > now() + 300)).map(String::from);
    let rec = load(l);
    if !rec["access_token"].is_string() {
        bail!("not signed in to {}: /login signs in", l.name);
    }
    if let Some(t) = fresh(&rec) {
        return Ok(t);
    }
    let renewed = locked(l, || {
        let mut rec = load(l);
        if let Some(t) = fresh(&rec) {
            return Ok(t);
        }
        renew(l, &mut rec)?;
        rec["access_token"].as_str().map(String::from).context("no access token")
    });
    // renewing failed for a passing reason (a server error, the network, a lock held too long): the stored token while it
    // still has time left and was not just refused; a sign-in that is gone stays an error
    renewed.or_else(|e| {
        let r = load(l);
        r["access_token"].as_str().filter(|t| Some(*t) != stale && r["expires_at"].as_u64().is_some_and(|x| x > now() + 10)).map(String::from).ok_or(e)
    })
}

/// A new access token from the refresh token; a refusal that no retry mends signs the login out (its registration and
/// the confirmed warning stay).
fn renew(l: &Login, rec: &mut Value) -> Result<()> {
    // nothing to renew with (Copilot's GitHub token): a refused token is a sign-out, not a 401 on every call
    let Some(refresh) = rec["refresh_token"].as_str().map(String::from) else {
        forget_tokens(rec);
        save(l, rec)?;
        bail!("{} refused the sign-in: sign in again (/login)", l.name);
    };
    let (url, client) = match l.sign_in() {
        "chatgpt" => (at(&format!("{OPENAI_AUTH}/api/accounts/oauth/token")), rec["client_id"].as_str().unwrap_or_default().to_string()),
        _ => (rec["token_endpoint"].as_str().map(at).context("no token endpoint: sign in again")?, XAI_CLIENT.to_string()),
    };
    let mut form = vec![("grant_type", "refresh_token"), ("refresh_token", refresh.as_str()), ("client_id", client.as_str())];
    if l.sign_in() == "chatgpt" {
        form.push(("resource", OPENAI_API)); // no scope: the grant stays as it was
    }
    let (status, t) = post_form(&url, &form)?;
    if t["access_token"].is_string() {
        take_tokens(rec, &t);
        return save(l, rec);
    }
    let why = oauth_error(&t);
    // a network or server failure keeps the tokens; a refresh token that is gone for good does not
    if (400..500).contains(&status) && ["invalid_grant", "invalid_refresh_token", "token_expired", "refresh_token_"].iter().any(|c| why.contains(c)) {
        forget_tokens(rec);
        save(l, rec)?;
        bail!("{} signed tazuna out ({why}): sign in again (/login)", l.name);
    }
    bail!("{}: renewing the sign-in failed (HTTP {status}: {why})", l.name)
}

fn forget_tokens(rec: &mut Value) {
    if let Some(o) = rec.as_object_mut() {
        for k in ["access_token", "refresh_token", "id_token", "expires_at", "scopes", "saved_at", "models_at"] {
            o.remove(k);
        }
    }
}

/// Signs out: ChatGPT's renewable session is revoked at OpenAI first (tried three times); the tokens leave the record,
/// and what a later sign-in reuses stays (ChatGPT's registration and host id, the confirmed warning).
#[inline(never)]
pub fn sign_out(l: &Login) -> Result<String> {
    locked(l, || {
        let mut rec = load(l);
        let said = format!("signed out of {}{}", l.name, revoke(l, &rec));
        forget_tokens(&mut rec);
        rec.as_object_mut().map(|o| o.remove("models"));
        save(l, &rec)?;
        Ok(said)
    })
}

/// The renewable session revoked at the provider (three tries): ChatGPT's, SuperGrok's at the endpoint xAI's discovery
/// named. What to add to the sign-out's message: where a grant stays that tazuna cannot end.
fn revoke(l: &Login, rec: &Value) -> String {
    // ponytail: ChatGPT's revocation address as its discovery document names it (2026-11-22), fixed here; read the
    // document at sign-out if OpenAI moves it
    let (url, client) = match l.sign_in() {
        "chatgpt" => (Some(at(&format!("{OPENAI_AUTH}/api/accounts/oauth/revoke"))), rec["client_id"].as_str().unwrap_or_default()),
        "xai" => (rec["revocation_endpoint"].as_str().map(at), XAI_CLIENT),
        // GitHub ends an app's token only with the app's secret, which tazuna does not have
        _ => return "; the grant stays at GitHub until you revoke it at https://github.com/settings/applications".into(),
    };
    let (Some(url), Some(t)) = (url, rec["refresh_token"].as_str()) else { return String::new() };
    let form = [("token", t), ("token_type_hint", "refresh_token"), ("client_id", client)];
    let revoked = (0..3).any(|i| {
        std::thread::sleep(Duration::from_millis(500 * i));
        post_form(&url, &form).is_ok_and(|(s, _)| s == 200)
    });
    match (revoked, l.sign_in()) {
        (true, _) => String::new(),
        (false, "chatgpt") => "; OpenAI did not confirm the revocation: disconnect tazuna in ChatGPT's settings to be sure".into(),
        (false, _) => format!("; {} did not confirm the revocation", l.name),
    }
}

/// ChatGPT with another account: the session revoked and the registration forgotten (this machine's host id stays), so the
/// next sign-in registers anew. One account at a time.
#[inline(never)]
pub fn forget_account(l: &Login) -> Result<()> {
    locked(l, || {
        let mut rec = load(l);
        revoke(l, &rec);
        forget_tokens(&mut rec);
        if let Some(o) = rec.as_object_mut() {
            for k in ["models", "client_id", "subject", "email", "issuer", "plan_declined"] {
                o.remove(k);
            }
        }
        save(l, &rec)
    })
}

/// A sign-in, start to end (the browser, a code to type, minutes of waiting: off the screen's thread). What it did, and
/// whether this was the first sign-in that uses the ChatGPT plan (its welcome shows once).
#[inline(never)]
pub fn sign_in(l: &'static Login, say: &dyn Fn(String)) -> Result<(String, bool)> {
    if l.warns() && !confirmed(l) {
        bail!("{}: confirm its warning first", l.name);
    }
    let _busy = Busy::take(l)?;
    let open = |link: &str| crate::platform::open_url(link).is_ok();
    match l.sign_in() {
        "chatgpt" => chatgpt(l, say, &open),
        "copilot" => copilot(l, say, &open).map(|t| (t, false)),
        "xai" => xai(l, say, &open).map(|t| (t, false)),
        other => bail!("no sign-in {other}"),
    }
}

/// The sign-ins under way in this process: a second one of the same login waits for the first to end.
static BUSY: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());

struct Busy(&'static str);

impl Busy {
    fn take(l: &'static Login) -> Result<Busy> {
        let mut b = BUSY.lock().unwrap_or_else(|e| e.into_inner());
        if b.contains(&l.sign_in()) {
            bail!("a sign-in to {} is already waiting (5 minutes at most)", l.name);
        }
        b.push(l.sign_in());
        Ok(Busy(l.sign_in()))
    }
}

impl Drop for Busy {
    fn drop(&mut self) {
        BUSY.lock().unwrap_or_else(|e| e.into_inner()).retain(|x| *x != self.0);
    }
}

/// The models list once more, in the background at start, at most once a day (a plan's models change; every window's
/// core starts this, and a token renewed on the way is kept).
pub fn refresh_models() {
    for l in crate::models::LOGINS.iter().filter(|l| !l.sign_in().is_empty() && signed_in(l) && !plan_declined(l)) {
        if load(l)["models_at"].as_u64().is_some_and(|t| t + 86_400 > now()) {
            continue;
        }
        if let Ok(t) = access_token(l, None) {
            let ms = fetch_models(l, &t, &load(l));
            // an empty answer is asked again at the next start
            if !ms.is_empty() {
                let _ = update(l, |r| (r["models"], r["models_at"]) = (json!(ms), json!(now())));
            }
        }
    }
}

/// The browser at `link`, or the link on the clipboard or the screen where none opens.
fn send_to_browser(name: &str, link: &str, open: &dyn Fn(&str) -> bool, say: &dyn Fn(String), what: &str) {
    if open(link) {
        say(format!("{name}: {what} in the browser tab that just opened (5 minutes)"));
    } else if clip(link) {
        say(format!("{name}: no browser opened; the link is on the clipboard: paste it into a browser (5 minutes)"));
    } else {
        say(format!("{name}: open this link (5 minutes): {link}"));
    }
}

/// A random UUID (v4) as a URN: this machine's `ext_agent_host_id`, made once.
fn host_id() -> String {
    let mut b = crate::platform::random::<16>();
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("urn:uuid:{}-{}-{}-{}-{}", &h[..8], &h[8..12], &h[12..16], &h[16..20], &h[20..])
}

fn nonce() -> String {
    #[cfg(test)]
    if STAND_IN.with_borrow(|s| s.is_some()) {
        return "n1".into();
    }
    b64url(&crate::platform::random::<16>())
}

/// Sign in with ChatGPT, as OpenAI documents it for open-source apps: the first time a registration of its own
/// (`dynamic_agent_client`, named tazuna, this machine's host id), later the issued client again; PKCE, state and nonce;
/// the code through a listener on 127.0.0.1; the ID token checked against OpenAI's keys; the plan's scope checked.
fn chatgpt(l: &Login, say: &dyn Fn(String), open: &dyn Fn(&str) -> bool) -> Result<(String, bool)> {
    if !load(l)["ext_agent_host_id"].is_string() {
        // kept before the first sign-in, as the docs ask
        update(l, |r| if !r["ext_agent_host_id"].is_string() { r["ext_agent_host_id"] = json!(host_id()) })?;
    }
    let mut rec = load(l);
    let host = rec["ext_agent_host_id"].as_str().unwrap_or_default().to_string();
    let saved = rec["client_id"].as_str().map(String::from);
    // the port OpenAI's docs show, else any: only the port may change between sign-ins
    let listener = TcpListener::bind("127.0.0.1:1455").or_else(|_| TcpListener::bind("127.0.0.1:0"))?;
    let redirect = format!("http://127.0.0.1:{}/auth/callback", listener.local_addr()?.port());
    let (verifier, state, nonce) = (b64url(&crate::platform::random::<32>()), b64url(&crate::platform::random::<16>()), nonce());
    let mut q: Vec<(&str, String)> = vec![("client_id", saved.clone().unwrap_or_else(|| "dynamic_agent_client".into()))];
    if saved.is_none() {
        q.push(("agent_name_hint", "tazuna".into()));
    }
    q.push(("ext_agent_host_id", host));
    if saved.is_some() && let Some(e) = rec["email"].as_str() {
        q.push(("login_hint", e.into()));
    }
    // plan usage declined at an earlier sign-in: the consent page again, as OpenAI's docs ask (not on every sign-in)
    if rec["plan_declined"] == true {
        q.push(("prompt", "consent".into()));
    }
    q.extend([("response_type", "code".into()), ("redirect_uri", redirect.clone()), ("scope", CHATGPT_SCOPES.into()), ("resource", OPENAI_API.into()),
        ("state", state.clone()), ("nonce", nonce.clone()), ("code_challenge_method", "S256".into()), ("code_challenge", b64url(&sha256(verifier.as_bytes())))]);
    let link = |q: &[(&str, String)]| format!("{}?{}", at(&format!("{OPENAI_AUTH}/api/accounts/authorize")),
        q.iter().map(|(k, v)| format!("{k}={}", enc(v))).collect::<Vec<_>>().join("&"));
    // the retained ID token names the account only to the browser: a link that may land on the clipboard or the screen
    // goes without it
    let hint = rec["id_token"].as_str().filter(|_| saved.is_some()).map(String::from);
    let opened = hint.as_ref().is_some_and(|h| {
        let mut with = q.clone();
        with.push(("id_token_hint", h.clone()));
        open(&link(&with))
    });
    if opened {
        say(format!("{}: continue in the browser tab that just opened (5 minutes)", l.name));
    } else {
        send_to_browser(l.name, &link(&q), open, say, "continue");
    }
    let back = wait_callback(&listener, &state, l.name, "/login")?;
    let got = |k: &str| back.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone());
    let client = match (&saved, got("client_id")) {
        (None, Some(c)) if c != "dynamic_agent_client" => c,
        (None, _) => bail!("{}: the registration did not finish (no client id came back); sign in again", l.name),
        (Some(s), Some(c)) if c != *s => bail!("{}: the sign-in came back for another registration; refused", l.name),
        (Some(s), _) => s.clone(),
    };
    let code = got("code").context("no code")?;
    let (status, t) = post_form(&at(&format!("{OPENAI_AUTH}/api/accounts/oauth/token")), &[("grant_type", "authorization_code"), ("client_id", &client),
        ("code", &code), ("code_verifier", &verifier), ("redirect_uri", &redirect), ("resource", OPENAI_API)])?;
    if !t["access_token"].is_string() {
        bail!("{}: the sign-in was refused (HTTP {status}: {})", l.name, oauth_error(&t));
    }
    let id_token = t["id_token"].as_str().context("no ID token came back")?;
    let jwks = get_json(&at(&format!("{OPENAI_AUTH}/.well-known/jwks.json")), "", &[]).context("OpenAI's signing keys")?;
    let claims = check_id_token(id_token, &client, &nonce, &jwks, now()).with_context(|| format!("{}: the ID token does not check out; refused", l.name))?;
    let sub = claims["sub"].as_str().context("no subject in the ID token")?;
    if saved.is_some() && rec["subject"].as_str().is_some_and(|s| s != sub) {
        bail!("{}: another account came back than this registration's; refused", l.name);
    }
    rec["client_id"] = json!(client);
    rec["issuer"] = json!(OPENAI_AUTH);
    rec["subject"] = json!(sub);
    rec["email"] = claims["email"].clone();
    rec["id_token"] = json!(id_token);
    take_tokens(&mut rec, &t);
    // the token answer's granted scopes decide (the callback's where it names none)
    let granted = t["scope"].as_str().map(String::from).or(got("scope")).unwrap_or_default();
    if !granted.split_whitespace().any(|x| x == PLAN_SCOPE) {
        // the identity stays (OpenAI's docs); requests would not be paid by the plan, so no model is offered, and the next
        // sign-in asks for consent again
        (rec["models"], rec["plan_declined"]) = (json!([]), json!(true));
        locked(l, || save(l, &rec))?;
        bail!("{}: signed in, but ChatGPT plan use was not allowed; to use its models, sign in again (/login, enter) and allow it", l.name);
    }
    rec.as_object_mut().map(|o| o.remove("plan_declined"));
    let ms = fetch_models(l, rec["access_token"].as_str().unwrap_or_default(), &rec);
    rec["models"] = json!(ms);
    if !ms.is_empty() {
        rec["models_at"] = json!(now());
    }
    let first = rec["welcomed"] != true;
    rec["welcomed"] = json!(true);
    locked(l, || save(l, &rec))?;
    let who = rec["email"].as_str().map_or(String::new(), |e| format!(" as {e}"));
    let models = if ms.is_empty() { "its model list did not load; /login again later".to_string() } else { format!("{} models in /model", ms.len()) };
    Ok((format!("signed in to ChatGPT{who}: requests on its models use your ChatGPT plan ({models}); manage usage at https://chatgpt.com/settings/usage"), first))
}

/// base64url without padding, decoded.
fn unb64url(s: &str) -> Result<Vec<u8>> {
    let (mut out, mut acc, mut bits) = (vec![], 0u32, 0);
    for c in s.trim_end_matches('=').bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            _ => bail!("not base64url"),
        };
        acc = (acc << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Ok(out)
}

/// An ID token as OpenAI's docs ask: RS256 under a key of OpenAI's published set, its issuer, its audience the issued
/// client, not expired, this attempt's nonce. Its claims when it checks out.
fn check_id_token(token: &str, client: &str, nonce: &str, jwks: &Value, now: u64) -> Result<Value> {
    let [h, p, s] = token.split('.').collect::<Vec<_>>()[..] else { bail!("not a JWT") };
    let head: Value = serde_json::from_slice(&unb64url(h)?)?;
    let claims: Value = serde_json::from_slice(&unb64url(p)?)?;
    if head["alg"] != "RS256" {
        bail!("signed with {}, not RS256", head["alg"]);
    }
    let key = jwks["keys"].as_array().into_iter().flatten().find(|k| k["kid"] == head["kid"] && k["kty"] == "RSA").context("signed with a key OpenAI does not publish")?;
    let (n, e) = (unb64url(key["n"].as_str().unwrap_or_default())?, unb64url(key["e"].as_str().unwrap_or_default())?);
    ring::signature::RsaPublicKeyComponents { n: &n, e: &e }
        .verify(&ring::signature::RSA_PKCS1_2048_8192_SHA256, format!("{h}.{p}").as_bytes(), &unb64url(s)?)
        .map_err(|_| anyhow!("its signature does not verify"))?;
    if claims["iss"] != OPENAI_AUTH {
        bail!("issued by {}", claims["iss"]);
    }
    if !(claims["aud"] == client || claims["aud"].as_array().is_some_and(|a| a.iter().any(|x| x == client))) {
        bail!("made for another client");
    }
    if claims["exp"].as_u64().is_none_or(|e| e + 60 < now) {
        bail!("expired");
    }
    if claims["nonce"] != nonce {
        bail!("not this sign-in's (nonce)");
    }
    Ok(claims)
}

/// A device-code sign-in's wait: the provider's token endpoint asked every `interval` seconds until the user typed the
/// code (RFC 8628: pending, slow down, refused, expired).
fn device_wait(name: &str, url: &str, form: &[(&str, &str)], interval: u64, expires: u64) -> Result<Value> {
    let (end, mut wait) = (now() + expires.clamp(60, 1800), interval.clamp(1, 60));
    loop {
        #[cfg(not(test))]
        std::thread::sleep(Duration::from_secs(wait));
        let (_, t) = post_form(url, form)?;
        if t["access_token"].is_string() {
            return Ok(t);
        }
        match oauth_error(&t).split(':').next().unwrap_or_default() {
            "authorization_pending" => {}
            "slow_down" => wait += 5,
            _ => bail!("{name}: the sign-in was refused ({})", oauth_error(&t)),
        }
        if now() > end {
            bail!("{name}: no sign-in within {} minutes: /login to try again", expires / 60);
        }
    }
}

/// The code to type on the provider's page: the page opened in the browser, the code on the clipboard where it can go.
fn show_code(name: &str, code: &str, page: &str, open: &dyn Fn(&str) -> bool, say: &dyn Fn(String)) {
    let opened = if open(page) { "the page just opened in your browser" } else { "open it in your browser" };
    let clip = if clip(code) { "; the code is on the clipboard" } else { "" };
    say(format!("{name}: enter the code {code} at {page} ({opened}{clip})"));
}

/// Text on the clipboard (never in tests: the machine's own clipboard).
fn clip(text: &str) -> bool {
    !cfg!(test) && crate::platform::copy_text(text).is_ok()
}

/// GitHub Copilot: GitHub's device flow as OpenCode's GitHub app (read:user), then the plan's own API address; the GitHub
/// token is the bearer (it does not run out; a 401 means a new sign-in).
fn copilot(l: &Login, say: &dyn Fn(String), open: &dyn Fn(&str) -> bool) -> Result<String> {
    let (_, d) = post_form(&at("https://github.com/login/device/code"), &[("client_id", GITHUB_CLIENT), ("scope", "read:user")])?;
    let (Some(device), Some(code), Some(page)) = (d["device_code"].as_str(), d["user_code"].as_str(), d["verification_uri"].as_str()) else {
        bail!("{}: GitHub did not start the sign-in ({})", l.name, oauth_error(&d));
    };
    show_code(l.name, code, page, open, say);
    let t = device_wait(l.name, &at("https://github.com/login/oauth/access_token"), &[("client_id", GITHUB_CLIENT), ("device_code", device),
        ("grant_type", "urn:ietf:params:oauth:grant-type:device_code")], d["interval"].as_u64().unwrap_or(5), d["expires_in"].as_u64().unwrap_or(900))?;
    let token = t["access_token"].as_str().unwrap_or_default();
    // individual, business or enterprise: the plan's API host; https and Copilot's own domain only
    let user = get_json(&at("https://api.github.com/copilot_internal/user"), &format!("token {token}"), &[("User-Agent", COPILOT_VERSION)]).ok();
    let api = user.as_ref().and_then(|u| u["endpoints"]["api"].as_str()).map(|u| u.trim_end_matches('/').to_string())
        .filter(|u| u.starts_with("https://") && crate::models::host_of(u).ends_with(".githubcopilot.com"))
        .unwrap_or_else(|| l.url.to_string());
    let mut rec = load(l);
    rec["access_token"] = json!(token);
    rec["api_url"] = json!(api);
    rec["saved_at"] = json!(now());
    let ms = fetch_models(l, token, &rec);
    rec["models"] = json!(ms);
    if !ms.is_empty() {
        rec["models_at"] = json!(now());
    }
    locked(l, || save(l, &rec))?;
    if ms.is_empty() {
        bail!("{}: signed in, but Copilot listed no models for this account (is Copilot on for it?)", l.name);
    }
    Ok(format!("signed in to {}: {} models in /model; they use your Copilot plan's requests", l.name, ms.len()))
}

/// SuperGrok: xAI's device flow as the Grok CLI, its endpoints from xAI's discovery document (x.ai hosts over https only:
/// every later refresh token goes there).
fn xai(l: &Login, say: &dyn Fn(String), open: &dyn Fn(&str) -> bool) -> Result<String> {
    let conf = get_json(&at("https://auth.x.ai/.well-known/openid-configuration"), "", &[]).context("xAI's sign-in endpoints")?;
    let ours = |k: &str| conf[k].as_str().filter(|u| {
        let h = crate::models::host_of(u);
        u.starts_with("https://") && (h == "x.ai" || h.ends_with(".x.ai"))
    }).map(String::from).with_context(|| format!("xAI's {k} is not an x.ai address over https; refused"));
    let (device_at, token_at) = (ours("device_authorization_endpoint")?, ours("token_endpoint")?);
    let (_, d) = post_form(&at(&device_at), &[("client_id", XAI_CLIENT), ("scope", XAI_SCOPES)])?;
    let (Some(device), Some(code), Some(page)) = (d["device_code"].as_str(), d["user_code"].as_str(), d["verification_uri"].as_str()) else {
        bail!("{}: xAI did not start the sign-in ({})", l.name, oauth_error(&d));
    };
    // the page with the code in it, where xAI gives one
    show_code(l.name, code, d["verification_uri_complete"].as_str().unwrap_or(page), open, say);
    let t = device_wait(l.name, &at(&token_at), &[("grant_type", "urn:ietf:params:oauth:grant-type:device_code"), ("device_code", device),
        ("client_id", XAI_CLIENT)], d["interval"].as_u64().unwrap_or(5), d["expires_in"].as_u64().unwrap_or(900))?;
    let mut rec = load(l);
    take_tokens(&mut rec, &t);
    rec["token_endpoint"] = json!(token_at);
    if let Ok(r) = ours("revocation_endpoint") {
        rec["revocation_endpoint"] = json!(r);
    }
    let ms = fetch_models(l, rec["access_token"].as_str().unwrap_or_default(), &rec);
    rec["models"] = json!(ms);
    if !ms.is_empty() {
        rec["models_at"] = json!(now());
    }
    locked(l, || save(l, &rec))?;
    Ok(format!("signed in to {}: {} models in /model; they use your SuperGrok plan", l.name, ms.len()))
}

/// Copilot's API headers: Copilot CLI's identity, and whether the user asked (a premium request) or the agent goes on
/// after a tool's result (not one), from the request's last message.
fn copilot_headers(body: &Value, list: bool) -> Vec<(String, String)> {
    let last = body["messages"].as_array().or(body["input"].as_array()).and_then(|m| m.last()).cloned().unwrap_or_default();
    let agent = last["role"] == "tool" || last["type"] == "function_call_output";
    [("User-Agent", COPILOT_VERSION), ("Editor-Version", COPILOT_VERSION),
     ("Copilot-Integration-Id", if list { "copilot-developer-cli" } else { "copilot-chat" }), ("Copilot-Harness-Id", "copilot-sdk"),
     ("Openai-Intent", "conversation-agent"), ("X-GitHub-Api-Version", "2026-08-01"), ("X-Initiator", if agent { "agent" } else { "user" })]
        .iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

/// The models a sign-in offers, from its provider's list with the new token (none: the list did not answer). Claude
/// stays out (the claude CLI only), as do models that cannot chat.
fn fetch_models(l: &Login, token: &str, rec: &Value) -> Vec<Value> {
    let bearer = format!("Bearer {token}");
    let row = |id: &str, name: &str, api: &str| json!({"id": id, "name": name, "api": api});
    let ok = |id: &str| !id.to_lowercase().contains("claude") && !["embed", "tts", "imagine", "image", "audio", "realtime", "transcribe"].iter().any(|w| id.contains(w));
    match l.sign_in() {
        // OpenAI's list for this account: `models[]` (slug, display_name, visibility "list"), in its order
        "chatgpt" => get_json(&at(&format!("{OPENAI_API}/models")), &bearer, &[]).map(|v| {
            v["models"].as_array().into_iter().flatten().filter(|m| m["visibility"] == "list").filter_map(|m| {
                let id = m["slug"].as_str()?;
                ok(id).then(|| row(id, m["display_name"].as_str().unwrap_or(id), "responses"))
            }).collect()
        }).unwrap_or_default(),
        // Copilot's picker models; Responses for the families Copilot serves only there (omp's api-routes)
        "copilot" => {
            let base = rec["api_url"].as_str().unwrap_or(l.url);
            let hs = copilot_headers(&json!({}), true);
            let hs: Vec<(&str, &str)> = hs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
            get_json(&at(&format!("{base}/models")), &bearer, &hs).map(|v| {
                v["data"].as_array().into_iter().flatten()
                    .filter(|m| m["capabilities"]["type"].as_str().is_none_or(|t| t == "chat") && m["model_picker_enabled"] != false)
                    .filter_map(|m| {
                        let id = m["id"].as_str()?;
                        let eps: Vec<&str> = m["supported_endpoints"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                        let responses = if eps.is_empty() { ["gpt-5", "gpt-6", "grok-4.", "oswe", "mai-"].iter().any(|p| id.starts_with(p)) }
                            else { !eps.contains(&"/chat/completions") && eps.contains(&"/responses") };
                        ok(id).then(|| row(id, m["name"].as_str().unwrap_or(id), if responses { "responses" } else { "chat" }))
                    }).collect()
            }).unwrap_or_default()
        }
        _ => get_json(&at("https://api.x.ai/v1/models"), &bearer, &[]).map(|v| {
            v["data"].as_array().into_iter().flatten().filter_map(|m| m["id"].as_str()).filter(|id| ok(id)).map(|id| row(id, id, "responses")).collect()
        }).unwrap_or_default(),
    }
}

/// How the engine reaches a signed-in plan's model: the API's address (Copilot's plan host) and its wire.
pub fn route(l: &Login, model: &str) -> (String, &'static str) {
    let rec = load(l);
    let api = rec["models"].as_array().into_iter().flatten().find(|m| m["id"] == model).and_then(|m| m["api"].as_str())
        .map_or(if l.api.is_empty() { "chat" } else { l.api }, |a| if a == "responses" { "responses" } else { "chat" });
    (rec["api_url"].as_str().unwrap_or(l.url).to_string(), api)
}

/// Extra headers for one call (Copilot's); none for the others.
pub fn headers(l: &Login, body: &Value) -> Vec<(String, String)> {
    if l.sign_in() == "copilot" { copilot_headers(body, false) } else { vec![] }
}

/// A request in the shape the plan's route takes. ChatGPT's preview takes function tools grouped in a namespace
/// (OpenAI's preview limits); `flat` after it refused that once.
pub fn shape(l: &Login, body: &Value, flat: bool) -> Value {
    let mut b = body.clone();
    if l.sign_in() == "chatgpt" && !flat && let Some(tools) = b["tools"].as_array().filter(|t| !t.is_empty()).cloned() {
        b["tools"] = json!([{"type": "namespace", "name": "tazuna", "description": "The coding agent's tools.", "tools": tools}]);
    }
    b
}

/// A plan's refusal in words that say what to do (OpenAI's documented codes for ChatGPT plan usage).
pub fn explain(l: &Login, e: anyhow::Error) -> anyhow::Error {
    let t = format!("{e:#}");
    let say = if t.contains("subscription_sharing_usage_limit_exceeded") {
        "ChatGPT plan usage limit reached (your plan's, or the one set for tazuna): Manage usage at https://chatgpt.com/settings/usage"
    } else if t.contains("subscription_sharing_user_not_eligible") {
        "ChatGPT plan usage is not available for this account or workspace"
    } else if t.contains("subscription_sharing_invalid_user") || t.contains("chatpass_v2_") {
        "ChatGPT did not accept this sign-in for plan usage: sign out and in again (/login)"
    } else {
        return e;
    };
    anyhow!("{}: {say} ({})", l.name, t.chars().take(200).collect::<String>())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::{Read, Write};

    pub(crate) const JWT: &str = "eyJhbGciOiJSUzI1NiIsImtpZCI6ImsxIiwidHlwIjoiSldUIn0.eyJpc3MiOiJodHRwczovL2F1dGgub3BlbmFpLmNvbSIsImF1ZCI6Im9haWFwcF90ZXN0Iiwic3ViIjoidXNlci10ZXN0IiwiZW1haWwiOiJzb21lb25lQGV4YW1wbGUuY29tIiwiZXhwIjo0MTAyNDQ0ODAwLCJpYXQiOjE3OTUwMDAwMDAsIm5vbmNlIjoibjEifQ.mZkHGCWhwyRHTeDUdt5y-5jqIUHILOAmvYGrTZsZnqQ1h93B51YlS-1RZIp7nCRe0Z21LYOr-NamoByB5ue2Ui9Lw5a4pjJQ_ceSMtbW_YY_VejASVJ7B1qM1Q_4PsJ1AvUXLNvUKhPa1AmxIgkPV___RnvPMMTQLqVCME92PLbqR7n1ns-ZufLbUersOfDQwoUtZ4KH4wdiuMD8A4c5lo5ROf_4-C4Uofp5pFDy0xwjdNOzpFgEm5Ca3Dk2mBMMBqYg5W8InQQ0ZtjU73CSc1PrJDcxlRzS-ImqJ-cMvLwl7Y-kT3_8xCIa9l5LfqaZgoGpGCYueBiXZoys37nRvQ";
    /// The test key's public half (made with openssl for these tests; its private half was thrown away).
    pub(crate) fn jwks() -> Value {
        json!({"keys": [{"kty": "RSA", "kid": "k1", "alg": "RS256", "e": "AQAB", "n": "s5eMAmG7aZIVy5eyKPeZTYXZ3utGUl8rtjV1uG5a-6nVS9LS6AUFkyr07siTcq5F-JkXZ_GWufZaOznTWQMAnTGPcaqEBJExzDoAOC9FZu8jNhMZZIInWfVtqJ5KQFmkju3Kr4k-LV3_jDS6E0pwWnmuJdBxDnn6DnIABaj4ppK2cJjd_2s8sG0lVSfkLgA4dt99G5pbOQsQqP37c7sYEILuOqUizjrSJWR1j14ijtup9Y3ucnXtXvYB-RobxTi6cwpUT_G0zzW6kn9ccFEk_AUxmBO9uHUgyyOq5YzfkUQcozXcxekP4kP0oMVtItA8pMO0D48KY6PNnstJ2ltWXQ"}]})
    }

    pub(crate) fn login(id: &str) -> &'static Login {
        crate::models::LOGINS.iter().find(|l| l.sign_in() == id).unwrap()
    }

    /// A stand-in for every endpoint: each request as (method and path, headers, body) to `answer`, whose (status, body)
    /// goes back (a body starting with "data:" as an event stream). Runs until the test ends.
    pub(crate) fn stand_in(answer: impl Fn(&str, &str, &str) -> (u16, String) + Send + 'static) -> (String, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", server.local_addr().unwrap());
        let seen = std::sync::Arc::new(std::sync::Mutex::new(vec![]));
        let log = seen.clone();
        std::thread::spawn(move || {
            for c in server.incoming() {
                let Ok(mut c) = c else { continue };
                let (mut req, mut buf) = (vec![], [0u8; 65536]);
                let text = loop {
                    let n = c.read(&mut buf).unwrap_or(0);
                    req.extend_from_slice(&buf[..n]);
                    let t = String::from_utf8_lossy(&req).to_string();
                    if let Some((head, body)) = t.split_once("\r\n\r\n") {
                        let len = head.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0))).unwrap_or(0);
                        if body.len() >= len || n == 0 {
                            break t;
                        }
                    } else if n == 0 {
                        break t;
                    }
                };
                let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
                let line = head.lines().next().unwrap_or_default().rsplitn(2, ' ').nth(1).unwrap_or_default().to_string();
                log.lock().unwrap().push(format!("{line}\n{head}\n\n{body}"));
                let (status, out) = answer(&line, head, body);
                let kind = if out.starts_with("data:") { "text/event-stream" } else { "application/json" };
                let _ = c.write_all(format!("HTTP/1.1 {status} X\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len()).as_bytes());
            }
        });
        (base, seen)
    }

    pub(crate) fn put(l: &Login, rec: Value) {
        save(l, &rec).unwrap();
    }

    pub(crate) fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tazuna-{name}-{}-{:?}", std::process::id(), std::thread::current().id()).replace(['(', ')'], ""));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn an_id_token_checks_against_the_published_key_and_this_sign_in() {
        let now = 1_795_000_100;
        let ok = check_id_token(JWT, "oaiapp_test", "n1", &jwks(), now).unwrap();
        assert_eq!((ok["sub"].as_str(), ok["email"].as_str()), (Some("user-test"), Some("someone@example.com")));
        let no = |r: Result<Value>, why: &str| assert!(format!("{:#}", r.unwrap_err()).contains(why), "{why}");
        no(check_id_token(JWT, "oaiapp_other", "n1", &jwks(), now), "another client");
        no(check_id_token(JWT, "oaiapp_test", "n2", &jwks(), now), "nonce");
        no(check_id_token(JWT, "oaiapp_test", "n1", &jwks(), 4_102_444_800 + 61), "expired");
        // a changed claim breaks the signature; a key not in the set is refused
        let parts: Vec<&str> = JWT.split('.').collect();
        let forged = format!("{}.{}.{}", parts[0], b64url(br#"{"iss":"https://auth.openai.com","aud":"oaiapp_test","sub":"someone-else","exp":4102444800,"nonce":"n1"}"#), parts[2]);
        no(check_id_token(&forged, "oaiapp_test", "n1", &jwks(), now), "signature");
        let mut other = jwks();
        other["keys"][0]["kid"] = json!("k2");
        no(check_id_token(JWT, "oaiapp_test", "n1", &other, now), "does not publish");
        assert_eq!(unb64url("AQAB").unwrap(), vec![1, 0, 1]);
    }

    #[test]
    fn sign_in_with_chatgpt_registers_checks_and_stores_owner_only_then_renews_and_signs_out() {
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let n = calls.clone();
        let (base, seen) = stand_in(move |line, _, body| match line {
            "GET /.well-known/jwks.json" => (200, jwks().to_string()),
            "POST /api/accounts/oauth/token" if body.contains("grant_type=authorization_code") => (200, json!({"access_token": "at1", "refresh_token": "rt1",
                "id_token": JWT, "token_type": "Bearer", "expires_in": 3600, "scope": "chatgpt.tokens.use.direct email offline_access openid profile resource.invoke"}).to_string()),
            "POST /api/accounts/oauth/token" => {
                let i = n.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if i == 0 { (200, json!({"access_token": "at2", "refresh_token": "rt2", "expires_in": 3600, "scope": CHATGPT_SCOPES}).to_string()) }
                else { (400, json!({"error": "invalid_grant", "error_description": "refresh_token_reused"}).to_string()) }
            }
            "GET /v1/models" => (200, json!({"models": [{"slug": "gpt-6.1-sol", "display_name": "GPT-6.1 Sol", "visibility": "list"},
                {"slug": "gpt-hidden", "visibility": "hide"}]}).to_string()),
            "POST /api/accounts/oauth/revoke" => (200, String::new()),
            _ => (404, "{}".into()),
        });
        let dir = temp("siwc");
        STAND_IN.set(Some((base, dir.clone())));
        let l = login("chatgpt");
        assert!(!l.warns(), "OpenAI's own flow: no warning to confirm");
        // the browser: the authorize link is checked, then the callback gets the code and the issued client
        let link = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let got = link.clone();
        let open = move |u: &str| {
            *got.lock().unwrap() = u.to_string();
            let q = crate::mcpc::query(u);
            let v = |k: &str| q.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone()).unwrap_or_default();
            let cb = format!("{}?code=c1&state={}&client_id=oaiapp_test&scope=openid", v("redirect_uri"), v("state"));
            std::thread::spawn(move || { let _ = ureq::get(&cb).call(); });
            true
        };
        let (said, first) = chatgpt(l, &|_| {}, &open).unwrap();
        assert!(said.contains("someone@example.com") && said.contains("1 models") && first, "{said}");
        let u = link.lock().unwrap().clone();
        for want in ["/api/accounts/authorize?client_id=dynamic_agent_client", "agent_name_hint=tazuna", "ext_agent_host_id=urn%3Auuid%3A",
                     "redirect_uri=http%3A%2F%2F127.0.0.1%3A", "%2Fauth%2Fcallback", "scope=openid%20profile%20email%20offline_access%20resource.invoke%20chatgpt.tokens.use.direct",
                     "resource=https%3A%2F%2Fapi.openai.com%2Fv1", "nonce=n1", "code_challenge_method=S256"] {
            assert!(u.contains(want), "{want} in {u}");
        }
        let rec = load(l);
        assert_eq!((rec["client_id"].as_str(), rec["access_token"].as_str(), rec["subject"].as_str()), (Some("oaiapp_test"), Some("at1"), Some("user-test")));
        assert_eq!(models(l), vec![json!({"id": "gpt-6.1-sol", "name": "GPT-6.1 Sol", "api": "responses"})]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(path(l).unwrap()).unwrap().permissions().mode() & 0o777, 0o600, "owner only");
        }
        let exchange = seen.lock().unwrap().iter().find(|r| r.contains("authorization_code")).cloned().unwrap();
        assert!(exchange.contains("client_id=oaiapp_test") && exchange.contains("code_verifier=") && exchange.contains("resource=https%3A%2F%2Fapi.openai.com%2Fv1"), "{exchange}");
        // a token refused by the API: renewed with the issued client, the rotated refresh token kept
        assert_eq!(access_token(l, None).unwrap(), "at1");
        assert_eq!(access_token(l, Some("at1")).unwrap(), "at2");
        assert_eq!(load(l)["refresh_token"], "rt2");
        let renewal = seen.lock().unwrap().iter().rev().find(|r| r.contains("grant_type=refresh_token")).cloned().unwrap();
        assert!(renewal.contains("refresh_token=rt1") && renewal.contains("client_id=oaiapp_test") && !renewal.contains("scope="), "{renewal}");
        // a refresh token gone for good signs out, and the registration stays for the next sign-in
        let e = format!("{:#}", access_token(l, Some("at2")).unwrap_err());
        assert!(e.contains("sign in again") && !e.contains("rt2") && !signed_in(l), "{e}");
        assert_eq!(load(l)["client_id"], "oaiapp_test");
        // signed in again: the issued client, no name hint; signed out: revoked, tokens gone, registration kept
        chatgpt(l, &|_| {}, &open).unwrap();
        let u = link.lock().unwrap().clone();
        assert!(u.contains("client_id=oaiapp_test") && !u.contains("agent_name_hint") && u.contains("login_hint=someone%40example.com"), "{u}");
        let said = sign_out(l).unwrap();
        assert!(said == "signed out of ChatGPT" && !signed_in(l) && load(l)["client_id"] == "oaiapp_test" && load(l)["ext_agent_host_id"].is_string(), "{said}");
        assert!(seen.lock().unwrap().iter().any(|r| r.starts_with("POST /api/accounts/oauth/revoke") && r.contains("token_type_hint=refresh_token")));
        STAND_IN.set(None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn device_sign_ins_wait_for_the_code_and_keep_each_plans_route() {
        let polls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let p = polls.clone();
        let (base, seen) = stand_in(move |line, head, _| match line {
            "POST /login/device/code" => (200, json!({"device_code": "d1", "user_code": "ABCD-1234", "verification_uri": "https://github.com/login/device", "interval": 1, "expires_in": 900}).to_string()),
            "POST /login/oauth/access_token" if p.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 => (200, json!({"error": "authorization_pending"}).to_string()),
            "POST /login/oauth/access_token" => (200, json!({"access_token": "gho_test", "token_type": "bearer"}).to_string()),
            "GET /copilot_internal/user" => (200, json!({"endpoints": {"api": "https://api.individual.githubcopilot.com"}}).to_string()),
            "GET /models" if head.contains("Bearer gho_test") && head.to_lowercase().contains("copilot-integration-id: copilot-developer-cli") => (200, json!({"data": [
                {"id": "gpt-6.1-sol", "name": "GPT-6.1 Sol", "capabilities": {"type": "chat"}, "model_picker_enabled": true, "supported_endpoints": ["/responses"]},
                {"id": "gemini-3.7-pro", "name": "Gemini 3.7 Pro", "capabilities": {"type": "chat"}, "model_picker_enabled": true, "supported_endpoints": ["/chat/completions"]},
                {"id": "grok-4.7", "name": "Grok 4.7", "capabilities": {"type": "chat"}, "model_picker_enabled": true},
                {"id": "claude-opus-5.5", "name": "Claude Opus 5.5", "capabilities": {"type": "chat"}, "model_picker_enabled": true},
                {"id": "text-embedding-3", "capabilities": {"type": "embeddings"}}]}).to_string()),
            "GET /.well-known/openid-configuration" => (200, json!({"device_authorization_endpoint": "https://auth.x.ai/oauth2/device/code", "token_endpoint": "https://auth.x.ai/oauth2/token",
                "revocation_endpoint": "https://auth.x.ai/oauth2/revoke"}).to_string()),
            "POST /oauth2/revoke" => (200, String::new()),
            "POST /oauth2/device/code" => (200, json!({"device_code": "x1", "user_code": "GROK-1", "verification_uri": "https://accounts.x.ai/device", "interval": 1, "expires_in": 600}).to_string()),
            "POST /oauth2/token" => (200, json!({"access_token": "xat", "refresh_token": "xrt", "expires_in": 3600}).to_string()),
            "GET /v1/models" if head.contains("Bearer xat") => (200, json!({"data": [{"id": "grok-4.7"}, {"id": "grok-imagine-image"}]}).to_string()),
            _ => (404, "{}".into()),
        });
        let dir = temp("device");
        STAND_IN.set(Some((base.clone(), dir.clone())));
        let copilot_l = login("copilot");
        assert!(copilot_l.warns(), "another app's identity: a warning to confirm");
        assert!(format!("{:#}", sign_in(copilot_l, &|_| {}).unwrap_err()).contains("confirm its warning first"));
        confirm(copilot_l).unwrap();
        let said = std::cell::RefCell::new(vec![]);
        let done = copilot(copilot_l, &|t| said.borrow_mut().push(t), &|_| false).unwrap();
        assert!(said.borrow()[0].contains("enter the code ABCD-1234 at https://github.com/login/device") && done.contains("3 models"), "{:?} {done}", said.borrow());
        // Claude stays out; Responses where Copilot serves a model only there
        let route_of = |m: &str| route(copilot_l, m);
        assert_eq!(models(copilot_l).iter().map(|m| m["id"].as_str().unwrap()).collect::<Vec<_>>(), ["gpt-6.1-sol", "gemini-3.7-pro", "grok-4.7"]);
        assert_eq!((route_of("gpt-6.1-sol").1, route_of("gemini-3.7-pro").1, route_of("grok-4.7").1), ("responses", "chat", "responses"));
        assert_eq!(route_of("gemini-3.7-pro").0, "https://api.individual.githubcopilot.com");
        assert_eq!((load(copilot_l)["access_token"].as_str(), load(copilot_l)["api_url"].as_str()), (Some("gho_test"), Some("https://api.individual.githubcopilot.com")));
        assert!(seen.lock().unwrap().iter().any(|r| r.starts_with("POST /login/device/code") && r.contains("client_id=Ov23li8tweQw6odWQebz") && r.contains("scope=read%3Auser")));
        // a Copilot token does not renew: a refusal signs it out (not a 401 on every call) and asks for a new sign-in
        assert!(format!("{:#}", access_token(copilot_l, Some("gho_test")).unwrap_err()).contains("sign in again"));
        assert!(!signed_in(copilot_l) && confirmed(copilot_l));
        // SuperGrok: the discovered endpoints, the Grok CLI's client, models that chat
        let x = login("xai");
        confirm(x).unwrap();
        let said = xai(x, &|_| {}, &|_| false).unwrap();
        assert!(said.contains("1 models"), "{said}");
        assert_eq!((load(x)["token_endpoint"].as_str(), models(x)), (Some("https://auth.x.ai/oauth2/token"), vec![json!({"id": "grok-4.7", "name": "grok-4.7", "api": "responses"})]));
        assert!(seen.lock().unwrap().iter().any(|r| r.starts_with("POST /oauth2/device/code") && r.contains(&format!("client_id={XAI_CLIENT}")) && r.contains("grok-cli%3Aaccess")));
        // signed out: its session revoked at xAI; the confirmation outlives it
        assert_eq!(sign_out(x).unwrap(), "signed out of SuperGrok");
        assert!(confirmed(x) && !signed_in(x));
        assert!(seen.lock().unwrap().iter().any(|r| r.starts_with("POST /oauth2/revoke") && r.contains("token=xrt")));
        let said = sign_out(copilot_l).unwrap();
        assert!(said.contains("github.com/settings/applications"), "{said}");
        // one sign-in of a login at a time in a process
        let first = Busy::take(x).unwrap();
        assert!(Busy::take(x).is_err() && Busy::take(copilot_l).is_ok());
        drop(first);
        assert!(Busy::take(x).is_ok());
        STAND_IN.set(None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_declined_plan_offers_no_model_and_the_next_sign_in_asks_again() {
        let grant = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let g = grant.clone();
        let (base, seen) = stand_in(move |line, _, body| match line {
            "GET /.well-known/jwks.json" => (200, jwks().to_string()),
            "POST /api/accounts/oauth/token" if body.contains("authorization_code") => {
                let scope = if g.load(std::sync::atomic::Ordering::SeqCst) { CHATGPT_SCOPES } else { "openid profile email offline_access" };
                (200, json!({"access_token": "at1", "refresh_token": "rt1", "id_token": JWT, "expires_in": 3600, "scope": scope}).to_string())
            }
            "GET /v1/models" => (200, json!({"models": [{"slug": "gpt-6.1-sol", "display_name": "GPT-6.1 Sol", "visibility": "list"}]}).to_string()),
            "POST /api/accounts/oauth/revoke" => (200, String::new()),
            _ => (404, "{}".into()),
        });
        let dir = temp("declined");
        STAND_IN.set(Some((base, dir.clone())));
        let l = login("chatgpt");
        let link = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let got = link.clone();
        let open = move |u: &str| {
            *got.lock().unwrap() = u.to_string();
            let q = crate::mcpc::query(u);
            let v = |k: &str| q.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone()).unwrap_or_default();
            let cb = format!("{}?code=c1&state={}&client_id=oaiapp_test", v("redirect_uri"), v("state"));
            std::thread::spawn(move || { let _ = ureq::get(&cb).call(); });
            true
        };
        let e = format!("{:#}", chatgpt(l, &|_| {}, &open).unwrap_err());
        assert!(e.contains("plan use was not allowed") && signed_in(l) && plan_declined(l) && models(l).is_empty(), "{e}");
        assert!(why_not(l).unwrap().contains("not allowed"), "a greyed row with the reason, no model to pick");
        assert!(!seen.lock().unwrap().iter().any(|r| r.starts_with("GET /v1/models")), "no model list on an identity-only token");
        refresh_models();
        assert!(!seen.lock().unwrap().iter().any(|r| r.starts_with("GET /v1/models")), "nor at start");
        // signed out and in again: the consent page asked for again, and once allowed the models come
        sign_out(l).unwrap();
        assert!(plan_declined(l));
        grant.store(true, std::sync::atomic::Ordering::SeqCst);
        chatgpt(l, &|_| {}, &open).unwrap();
        assert!(link.lock().unwrap().contains("prompt=consent"));
        assert!(!plan_declined(l) && models(l).len() == 1 && why_not(l).is_none());
        // another account: the session revoked, the registration forgotten, the next sign-in registers anew
        forget_account(l).unwrap();
        assert!(!signed_in(l) && load(l)["client_id"].is_null() && load(l)["ext_agent_host_id"].is_string());
        chatgpt(l, &|_| {}, &open).unwrap();
        assert!(link.lock().unwrap().contains("client_id=dynamic_agent_client") && !link.lock().unwrap().contains("prompt=consent"));
        STAND_IN.set(None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn renewals_take_turns_and_a_passing_failure_keeps_the_token_that_still_works() {
        let (base, _) = stand_in(|line, _, _| match line {
            "POST /api/accounts/oauth/token" => (503, json!({"error": "temporarily_unavailable"}).to_string()),
            _ => (404, "{}".into()),
        });
        let dir = temp("renew");
        STAND_IN.set(Some((base.clone(), dir.clone())));
        let l = login("chatgpt");
        put(l, json!({"access_token": "at1", "refresh_token": "rt1", "expires_at": now() + 120, "client_id": "oaiapp_test"}));
        // 2 minutes left, renewal down: the stored token; refused by the API: an error, and the tokens stay
        assert_eq!(access_token(l, None).unwrap(), "at1");
        assert!(format!("{:#}", access_token(l, Some("at1")).unwrap_err()).contains("HTTP 503") && signed_in(l));
        // the system lock: a second holder waits for the first (two threads, two open files, as two processes)
        let (tx, rx) = std::sync::mpsc::channel();
        locked(l, || {
            let (b, d, tx2) = (base.clone(), dir.clone(), tx.clone());
            let other = std::thread::spawn(move || {
                STAND_IN.set(Some((b, d)));
                locked(login("chatgpt"), || { tx2.send("second").unwrap(); Ok(()) }).unwrap();
            });
            std::thread::sleep(Duration::from_millis(300));
            tx.send("first").unwrap();
            Ok(other)
        }).unwrap().join().unwrap();
        assert_eq!(rx.iter().take(2).collect::<Vec<_>>(), ["first", "second"]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(dir.join("chatgpt.lock")).unwrap().permissions().mode() & 0o777, 0o600);
        }
        STAND_IN.set(None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn copilot_says_who_asked_and_chatgpt_groups_its_tools() {
        let h = |b: Value| copilot_headers(&b, false).into_iter().find(|(k, _)| k == "X-Initiator").unwrap().1;
        assert_eq!(h(json!({"messages": [{"role": "user", "content": "hi"}]})), "user");
        assert_eq!(h(json!({"messages": [{"role": "tool", "content": "ok"}]})), "agent");
        assert_eq!(h(json!({"input": [{"type": "function_call_output", "output": "ok"}]})), "agent");
        let body = json!({"tools": [{"type": "function", "name": "bash"}], "input": []});
        let b = shape(login("chatgpt"), &body, false);
        assert_eq!((b["tools"][0]["type"].as_str(), b["tools"][0]["tools"][0]["name"].as_str()), (Some("namespace"), Some("bash")));
        assert_eq!(shape(login("chatgpt"), &body, true), body);
        assert_eq!(shape(login("xai"), &body, false), body);
        assert!(host_id().starts_with("urn:uuid:") && host_id().len() == 45 && host_id().as_bytes()[23] == b'4');
        let e = explain(login("chatgpt"), anyhow!("HTTP 429: limit (subscription_sharing_usage_limit_exceeded)"));
        assert!(format!("{e:#}").contains("Manage usage at https://chatgpt.com/settings/usage"));
    }
}
