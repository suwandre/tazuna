// tazuna desktop: the window around tazuna's own core. Each open conversation (a "thread", by an id the window picks)
// runs its own `tazuna app-server` (the same session model the terminal UI renders), so one can keep working while
// another is open. The relay: each session's views to the frontend as `tazuna-view` events tagged with their thread, the
// user's input back as JSON lines. docs/DESKTOP.md.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

#[cfg(unix)]
static PATH: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();

/// The running cores by thread: each with the start it came from (a number bumped on every start, so a core that was
/// replaced does not report its exit as its successor's), its process and its input.
#[derive(Default)]
struct Core {
    threads: Mutex<HashMap<String, (u64, Child, ChildStdin)>>,
    started: AtomicU64,
}

/// The tazuna binary: TAZUNA_BIN, else the one the app carries (bundle.externalBin, next to the app's own binary), else
/// `tazuna` on the PATH (a development build run with `tauri dev`).
fn tazuna_bin() -> String {
    if let Ok(b) = std::env::var("TAZUNA_BIN") {
        return b;
    }
    let carried = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join(if cfg!(windows) { "tazuna.exe" } else { "tazuna" })));
    carried.filter(|p| p.is_file()).map_or_else(|| "tazuna".into(), |p| p.to_string_lossy().into_owned())
}

/// A window opened from the Finder or a desktop launcher gets a bare PATH (/usr/bin:/bin...), without the folders
/// where tazuna and the `claude` CLI usually live: take the PATH the user's login shell sets up.
#[cfg(unix)]
fn login_path() -> Option<String> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    eprintln!("tazuna desktop: reading PATH from {shell}");
    let out = Command::new(shell).args(["-lic", "printf '\\n%s' \"$PATH\""]).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    String::from_utf8(out.stdout).ok()?.lines().last().map(str::to_string).filter(|p| !p.is_empty())
}

/// Starts (or restarts) thread `tid`'s core in `cwd` (a project) or the home folder (a chat). Async: stopping an old
/// core waits for it, off the main thread.
#[tauri::command]
async fn start(app: AppHandle, core: State<'_, Core>, tid: String, cwd: Option<String>, profile: Option<String>) -> Result<String, String> {
    stop_thread(&core, &tid);
    // a chat runs in the home folder: HOME, or USERPROFILE on Windows (which has no HOME, and "." is the app's own folder)
    let dir = cwd.or_else(|| std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).ok()).unwrap_or_else(|| ".".into());
    // one line each on stderr: where a start stops shows in a terminal or a CI log
    eprintln!("tazuna desktop: starting {} app-server in {dir}", tazuna_bin());
    let mut cmd = Command::new(tazuna_bin());
    cmd.arg("app-server").arg("--allow-non-git").current_dir(&dir);
    #[cfg(unix)]
    if let Some(p) = PATH.get_or_init(login_path) {
        cmd.env("PATH", p);
    }
    if let Some(p) = profile {
        cmd.args(["--profile", &p]);
    }
    let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()
        .map_err(|e| format!("could not start {} app-server: {e}", tazuna_bin()))?;
    eprintln!("tazuna desktop: app-server running (pid {})", child.id());
    let stdin = child.stdin.take().ok_or("no stdin")?;
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let run = core.started.fetch_add(1, Ordering::SeqCst) + 1;
    let t = tid.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                let _ = app.emit("tazuna-view", serde_json::json!({"tid": t, "msg": v}));
            }
        }
        let mine = app.state::<Core>().threads.lock().unwrap().get(&t).is_some_and(|(r, ..)| *r == run);
        if mine {
            let _ = app.emit("tazuna-exit", &t);
        }
    });
    core.threads.lock().unwrap().insert(tid, (run, child, stdin));
    Ok(dir)
}

/// Ends thread `tid`'s core (its conversation stays in the lists, resumable).
#[tauri::command]
async fn stop(core: State<'_, Core>, tid: String) -> Result<(), String> {
    stop_thread(&core, &tid);
    Ok(())
}

fn write(core: &State<Core>, tid: &str, v: serde_json::Value) -> Result<(), String> {
    let mut guard = core.threads.lock().unwrap();
    let (_, _, stdin) = guard.get_mut(tid).ok_or("that conversation is not running")?;
    writeln!(stdin, "{v}").and_then(|_| stdin.flush()).map_err(|e| e.to_string())
}

fn stop_thread(core: &State<Core>, tid: &str) {
    let gone = core.threads.lock().unwrap().remove(tid);
    if let Some((_, mut child, mut stdin)) = gone {
        let _ = writeln!(stdin, "{}", serde_json::json!({"op": "quit"}));
        drop(stdin);
        let _ = child.wait();
    }
}

/// `agent`: past the router, straight to the agent ("send to agent" on a message answered as a chat).
#[tauri::command]
fn send(core: State<Core>, tid: String, text: String, agent: bool) -> Result<(), String> {
    write(&core, &tid, serde_json::json!({"op": "send", "text": text, "agent": agent}))
}

/// The user's yes to trusting the open folder's settings (hooks, skills); the window starts the core again after it.
#[tauri::command]
fn trust(core: State<Core>, tid: String) -> Result<(), String> {
    write(&core, &tid, serde_json::json!({"op": "trust"}))
}

/// A provider's page for making a key, in the browser: only https pages of the providers tazuna knows.
#[tauri::command]
fn open_key_page(url: String) -> Result<(), String> {
    const HOSTS: [&str; 5] = ["subrouter.ai", "mortera.cloud", "openrouter.ai", "kenari.id", "console.mistral.ai"];
    let host = url.strip_prefix("https://").and_then(|r| r.split(['/', '?', '#']).next()).unwrap_or_default();
    if !HOSTS.contains(&host) || url.chars().any(|c| c.is_whitespace() || c == '"') {
        return Err(format!("not a provider page: {url}"));
    }
    #[cfg(target_os = "macos")]
    let r = Command::new("open").arg(&url).status();
    #[cfg(target_os = "windows")]
    let r = Command::new("explorer").arg(&url).status();
    #[cfg(all(unix, not(target_os = "macos")))]
    let r = Command::new("xdg-open").arg(&url).status();
    r.map(|_| ()).map_err(|e| e.to_string())
}

/// A link in an answer, in the browser: http and https only (an answer could hold any URL; a click is the user's).
#[tauri::command]
fn open_link(url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) || url.chars().any(|c| c.is_whitespace() || c.is_control() || c == '"') {
        return Err(format!("not a web link: {url}"));
    }
    #[cfg(target_os = "macos")]
    let r = Command::new("open").arg(&url).status();
    #[cfg(target_os = "windows")]
    let r = Command::new("explorer").arg(&url).status();
    #[cfg(all(unix, not(target_os = "macos")))]
    let r = Command::new("xdg-open").arg(&url).status();
    r.map(|_| ()).map_err(|e| e.to_string())
}

/// A line from the page on stderr ("loaded", a script error): where the window stops shows in a terminal or a CI log.
#[tauri::command]
fn ui_log(msg: String) {
    eprintln!("tazuna desktop (page): {}", msg.chars().take(500).collect::<String>());
}

/// The desktop app's own folder: ~/.config/tazuna/desktop (under the user's profile on Windows).
fn desktop_dir() -> std::path::PathBuf {
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_else(|_| ".".into());
    std::path::Path::new(&home).join(".config/tazuna/desktop")
}

/// The user's panels and plugins (docs/PLUGINS.md), one folder each: a panel has index.html, a plugin plugin.js.
fn widgets_dir() -> std::path::PathBuf {
    desktop_dir().join("plugins")
}

/// A plugin's code, fingerprinted: approval holds for this exact code (a change made outside creator mode asks again).
/// Not a cryptographic hash: whoever can write the plugin can also write the approvals file.
fn code_print(code: &[u8]) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    code.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// plugins.json: {"<id>": "<fingerprint of the code the user enabled>"}.
fn approvals() -> serde_json::Map<String, serde_json::Value> {
    std::fs::read_to_string(desktop_dir().join("plugins.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save_approvals(a: &serde_json::Map<String, serde_json::Value>) -> Result<(), String> {
    std::fs::create_dir_all(desktop_dir()).map_err(|e| e.to_string())?;
    std::fs::write(desktop_dir().join("plugins.json"), serde_json::to_string_pretty(a).unwrap_or_default()).map_err(|e| e.to_string())
}

/// A panel's id as a folder name: lowercase letters, digits and "-" only (nothing that leaves the folder).
fn widget_id_ok(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// The user's panels and plugins: each folder's id, kind (panel: index.html; plugin: plugin.js), manifest fields, and for
/// a plugin its state: "enabled" (the user enabled this exact code), "changed" (enabled, but the code changed since) or
/// "new". Code is served by the tzwidget: and tzplugin: schemes, not sent here.
#[tauri::command]
fn widgets_list() -> Vec<serde_json::Value> {
    let Ok(rd) = std::fs::read_dir(widgets_dir()) else { return vec![] };
    let ok = approvals();
    let mut out: Vec<serde_json::Value> = rd.flatten().filter_map(|e| {
        let id = e.file_name().into_string().ok().filter(|i| widget_id_ok(i))?;
        let manifest: serde_json::Value = std::fs::read_to_string(e.path().join("manifest.json")).ok()
            .and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| serde_json::json!({}));
        let title = manifest["title"].as_str().unwrap_or(&id).chars().take(60).collect::<String>();
        let code = std::fs::read(e.path().join("plugin.js")).ok().filter(|c| c.len() <= 1 << 20);
        if let Some(code) = code {
            let print = code_print(&code);
            let state = match ok.get(&id).and_then(|v| v.as_str()) {
                Some(p) if p == print => "enabled",
                Some(_) => "changed",
                None => "new",
            };
            return Some(serde_json::json!({"id": id, "kind": "plugin", "title": title, "state": state, "print": print,
                "about": manifest["description"].as_str().unwrap_or_default().chars().take(200).collect::<String>()}));
        }
        e.path().join("index.html").is_file().then_some(())?;
        Some(serde_json::json!({"id": id, "kind": "panel", "title": title, "slot": manifest["slot"].as_str().unwrap_or("rail")}))
    }).collect();
    out.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    out
}

/// A new panel's folder (creator mode works in it), created empty; its path.
#[tauri::command]
fn widgets_new(id: String) -> Result<String, String> {
    if !widget_id_ok(&id) {
        return Err(format!("not a panel id: {id}"));
    }
    let dir = widgets_dir().join(&id);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.to_string_lossy().into_owned())
}

/// Enables plugin `id` for its code as it is now (`print` from the list: what the user saw); off with `on` false.
#[tauri::command]
fn plugins_enable(id: String, print: String, on: bool) -> Result<(), String> {
    if !widget_id_ok(&id) {
        return Err(format!("not a plugin id: {id}"));
    }
    let mut a = approvals();
    if on {
        let code = std::fs::read(widgets_dir().join(&id).join("plugin.js")).map_err(|e| e.to_string())?;
        if code_print(&code) != print {
            return Err("the plugin changed since it was listed: look at it again".into());
        }
        a.insert(id, serde_json::json!(print));
    } else {
        a.remove(&id);
    }
    save_approvals(&a)
}

/// A plugin's code as text with the fingerprint of those same bytes: what the user reads is what Enable turns on
/// (plugins_enable refuses if the file changed since).
#[tauri::command]
fn plugins_code(id: String) -> Result<serde_json::Value, String> {
    if !widget_id_ok(&id) {
        return Err(format!("not a plugin id: {id}"));
    }
    let bytes = std::fs::read(widgets_dir().join(&id).join("plugin.js")).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"code": String::from_utf8_lossy(&bytes), "print": code_print(&bytes)}))
}

/// A panel's folder (for editing it with tazuna).
#[tauri::command]
fn widgets_path(id: String) -> Result<String, String> {
    widget_id_ok(&id).then(|| widgets_dir().join(&id).to_string_lossy().into_owned()).ok_or(format!("not a panel id: {id}"))
}

/// A panel off the window: its folder moves to widgets/.removed/ (kept, not deleted).
#[tauri::command]
fn widgets_remove(id: String) -> Result<(), String> {
    if !widget_id_ok(&id) {
        return Err(format!("not a panel id: {id}"));
    }
    let gone = widgets_dir().join(".removed");
    std::fs::create_dir_all(&gone).map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    std::fs::rename(widgets_dir().join(&id), gone.join(format!("{id}-{stamp}"))).map_err(|e| e.to_string())
}

/// The code tzplugin://localhost/<id> serves: plugin.js, only while the user has enabled exactly this code (the window
/// imports it as a module, which runs inside the window; docs/PLUGINS.md).
fn plugin_code(path: &str) -> (u16, String) {
    let id = path.trim_matches('/');
    if !widget_id_ok(id) {
        return (404, String::new());
    }
    let Ok(code) = std::fs::read(widgets_dir().join(id).join("plugin.js")) else { return (404, String::new()) };
    if code.len() > 1 << 20 || approvals().get(id).and_then(|v| v.as_str()) != Some(&code_print(&code)) {
        return (403, String::new());
    }
    (200, String::from_utf8_lossy(&code).into_owned())
}

/// tzwidget://localhost/__dryrun/<id>: a plugin's code run once in the sandboxed frame (no tazuna, no network, no
/// window), against a stand-in `ctx` that only records what it registers; the result goes to the window by message.
/// Creator mode's check, before the user enables anything (security review: agent-written code must not run in the
/// window unseen). The default export is found by turning `export default` into an assignment.
fn dry_run_page(id: &str) -> (u16, String) {
    if !widget_id_ok(id) || !widgets_dir().join(id).join("plugin.js").is_file() {
        return (404, String::new());
    }
    // the stand-in ctx records into a closure; the plugin's code loads from its own address (never pasted into this
    // page); the result is posted once. The plugin runs in this same page, so what it registers is self-reported: the
    // window says so and points to the code.
    let page = r#"<!doctype html><meta charset="utf-8"><script>
(function(){var rec={slots:[],hidden:[],palette:[],theme:0,css:0},sent=false,noop=function(){},
h=function(){return null},R={createElement:h,useState:function(v){return[v,noop]},useEffect:noop,useMemo:function(f){return f()},useRef:function(v){return{current:v}},useCallback:function(f){return f},Fragment:"f"};
window.__ctx={api:1,id:ID,React:R,h:h,useData:function(){return null},
slots:{register:function(s,e){rec.slots.push([String(s),String(e&&e.id)]);},hide:function(i){rec.hidden.push(String(i))},map:function(){return{}}},
actions:{send:noop,newChat:noop,setMode:noop,notify:noop,openLink:noop,review:noop},
theme:{set:function(){rec.theme++},css:function(){rec.css++}},palette:{add:function(a){rec.palette.push(String(a&&a.id))}},storage:{get:function(){return null},set:noop}};
var post=parent.postMessage.bind(parent);
window.__done=function(err){if(sent)return;sent=true;post({type:"tazuna-dryrun",id:ID,error:err,rec:JSON.parse(JSON.stringify(rec))},"*")};
window.addEventListener("error",function(e){window.__done(String(e.message||e))});})();
</script><script src="../__dryrun-code/IDRAW"></script><script>
try{if(typeof window.__plugin!=="function")throw new Error("plugin.js has no default export function(ctx)");var r=window.__plugin(window.__ctx);
Promise.resolve(r).then(function(){window.__done(null)},function(e){window.__done(String(e&&e.message||e))});}catch(e){window.__done(String(e&&e.message||e))}
</script>"#;
    (200, page.replace("IDRAW", id).replace("ID", &serde_json::json!(id).to_string()))
}

/// The dry-run page's policy: its own inline harness and the one script at its plugin's code address, nothing else (no
/// other address on the scheme, and on macOS and Linux no http://tzwidget.localhost, which is port 80 of this machine).
/// Windows' webview reaches the scheme over http.
fn dry_run_csp(id: &str) -> String {
    let id = if widget_id_ok(id) { id } else { "-" };
    let at = if cfg!(windows) { "http://tzwidget.localhost" } else { "tzwidget://localhost" };
    format!("default-src 'none'; script-src 'unsafe-inline' {at}/__dryrun-code/{id}")
}

/// tzwidget://localhost/__dryrun-code/<id>: a plugin's code for its dry run, as a classic script whose default export
/// becomes window.__plugin.
fn dry_run_code(path: &str) -> (u16, String) {
    let id = path.trim_matches('/').trim_start_matches("__dryrun-code/");
    if !widget_id_ok(id) {
        return (404, String::new());
    }
    match std::fs::read_to_string(widgets_dir().join(id).join("plugin.js")) {
        Ok(code) if code.len() <= 1 << 20 => (200, code.replacen("export default", "window.__plugin =", 1)),
        _ => (404, String::new()),
    }
}

/// The page tzwidget://localhost/<id> serves: the panel's index.html after a small preamble (the design tokens, filled
/// in by the window's first message), under its own policy: scripts and styles inline only, images and fonts from data:
/// URLs, no network. The window frames it sandboxed (no same origin), so it cannot reach tazuna either.
fn widget_page(path: &str) -> (u16, String) {
    if let Some(id) = path.trim_matches('/').strip_prefix("__dryrun/") {
        return dry_run_page(id);
    }
    if path.trim_matches('/').starts_with("__dryrun-code/") {
        return (404, String::new()); // served as JavaScript by widget_response
    }
    let id = path.trim_matches('/');
    if !widget_id_ok(id) {
        return (404, String::new());
    }
    let file = widgets_dir().join(id).join("index.html");
    match std::fs::metadata(&file) {
        Ok(m) if m.len() <= 1 << 20 => {}
        _ => return (404, String::new()),
    }
    let body = std::fs::read_to_string(file).unwrap_or_default();
    let pre = r#"<!doctype html><meta charset="utf-8"><style>
html,body{margin:0;background:transparent;color:var(--ink);font:12.5px/1.5 var(--font-ui,system-ui,sans-serif);overflow:hidden}
</style><script>
window.addEventListener("message",function(e){var d=e.data;if(!d||d.type!=="tazuna"||!d.tokens)return;
var r=document.documentElement.style;for(var k in d.tokens)r.setProperty(k,d.tokens[k]);});
</script>"#;
    (200, format!("{pre}
{body}"))
}

/// The quick-chat shortcut from anywhere (Settings; empty: none): the window comes to the front and opens a new chat. A
/// chord another app holds, or one that is not a chord, fails here and Settings says so.
#[tauri::command]
fn hotkey(app: AppHandle, keys: String) -> Result<(), String> {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
    let gs = app.global_shortcut();
    gs.unregister_all().map_err(|e| e.to_string())?;
    if keys.trim().is_empty() {
        return Ok(());
    }
    gs.on_shortcut(keys.trim(), |app, _, e| {
        if e.state == ShortcutState::Pressed {
            if let Some(w) = app.get_webview_window("main") {
                let _ = (w.unminimize(), w.show(), w.set_focus());
            }
            let _ = app.emit("quick-chat", ());
        }
    })
    .map_err(|e| {
        eprintln!("tazuna desktop: the quick-chat shortcut {keys}: {e}");
        e.to_string()
    })
}

/// A system notification (a conversation in the background finished or asks): plain text, shown by the system.
#[tauri::command]
fn notify(app: AppHandle, title: String, body: String) -> Result<(), String> {
    use tauri_plugin_notification::NotificationExt;
    app.notification().builder().title(title.chars().take(80).collect::<String>()).body(body.chars().take(200).collect::<String>())
        .show().map_err(|e| e.to_string())
}

/// The native file picker for attachments (images go to the model as images, other files as their paths).
#[tauri::command]
async fn pick_files(app: AppHandle) -> Vec<String> {
    use tauri_plugin_dialog::DialogExt;
    app.dialog().file().blocking_pick_files().unwrap_or_default().into_iter()
        .filter_map(|p| p.into_path().ok()).map(|p| p.to_string_lossy().into_owned()).collect()
}

/// The native folder picker; async so it does not block the main thread it needs.
#[tauri::command]
async fn pick_folder(app: AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    app.dialog().file().blocking_pick_folder().and_then(|p| p.into_path().ok()).map(|p| p.to_string_lossy().into_owned())
}

/// The picker's, the sidebar's and the composer's commands for the core ({"op":"models"}, "model", "effort", "sessions",
/// "resume", "mode", "answer", "diff", "undo");
/// nothing else passes (the core drops other lines too).
#[tauri::command]
fn op(core: State<Core>, tid: String, msg: serde_json::Value) -> Result<(), String> {
    if msg.get("type").is_some() || !matches!(msg["op"].as_str(), Some("models" | "model" | "effort" | "sessions" | "resume" | "mode" | "answer" | "diff" | "undo" | "settings" | "login" | "data" | "route" | "full" | "rename" | "hide" | "rewind" | "review" | "piece" | "allow_root" | "fork" | "usage")) {
        return Err(format!("not a command the window sends: {msg}"));
    }
    write(&core, &tid, msg)
}

#[tauri::command]
fn interrupt(core: State<Core>, tid: String) -> Result<(), String> {
    write(&core, &tid, serde_json::json!({"op": "interrupt"}))
}

#[tauri::command]
fn new_chat(core: State<Core>, tid: String) -> Result<(), String> {
    write(&core, &tid, serde_json::json!({"op": "new"}))
}

#[tauri::command]
fn set_profile(core: State<Core>, tid: String, name: String) -> Result<(), String> {
    write(&core, &tid, serde_json::json!({"op": "profile", "name": name}))
}

/// The macOS menu bar: the system's own items (Quit, Hide, the Edit menu that copy and paste need) and tazuna's (new chat,
/// open folder, the palette, the shortcuts, light or dark). tazuna's send a `menu` event with their id; their shortcuts
/// are the window's (docs/DESKTOP.md "Desktop habits").
#[cfg(target_os = "macos")]
fn menu(app: &AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem as P, Submenu};
    let item = |id: &str, text: &str, keys: &str| MenuItem::with_id(app, id, text, true, Some(keys));
    let sep = || P::separator(app);
    Menu::with_items(app, &[
        &Submenu::with_items(app, "tazuna", true, &[
            &P::about(app, None, None)?, &sep()?, &item("settings", "Settings…", "CmdOrCtrl+,")?, &sep()?, &P::services(app, None)?, &sep()?,
            &P::hide(app, None)?, &P::hide_others(app, None)?, &P::show_all(app, None)?, &sep()?, &P::quit(app, None)?,
        ])?,
        &Submenu::with_items(app, "File", true, &[
            &item("new-chat", "New Chat", "CmdOrCtrl+N")?, &item("open-folder", "Open Folder…", "CmdOrCtrl+O")?, &sep()?,
            &P::close_window(app, None)?,
        ])?,
        &Submenu::with_items(app, "Edit", true, &[
            &P::undo(app, None)?, &P::redo(app, None)?, &sep()?,
            &P::cut(app, None)?, &P::copy(app, None)?, &P::paste(app, None)?, &P::select_all(app, None)?,
        ])?,
        &Submenu::with_items(app, "View", true, &[
            &item("palette", "Command Palette", "CmdOrCtrl+K")?, &item("shortcuts", "Keyboard Shortcuts", "CmdOrCtrl+/")?,
            &item("theme", "Light or Dark", "CmdOrCtrl+Shift+L")?, &item("usage", "Usage", "CmdOrCtrl+Shift+U")?, &sep()?, &P::fullscreen(app, None)?,
        ])?,
        &Submenu::with_items(app, "Window", true, &[&P::minimize(app, None)?, &P::maximize(app, None)?])?,
    ])
}

fn main() {
    tauri::Builder::default()
        .register_uri_scheme_protocol("tzwidget", |_ctx, req| {
            let path = req.uri().path().to_string();
            let p = path.trim_matches('/');
            // a plugin's code for its dry run is JavaScript; the dry-run page may load it from this scheme; panels may not
            let (status, body, kind, csp) = if p.starts_with("__dryrun-code/") {
                let (st, b) = dry_run_code(&path);
                (st, b, "text/javascript; charset=utf-8", "default-src 'none'".into())
            } else if p.starts_with("__dryrun/") {
                let (st, b) = widget_page(&path);
                (st, b, "text/html; charset=utf-8", dry_run_csp(p.trim_start_matches("__dryrun/")))
            } else {
                let (st, b) = widget_page(&path);
                (st, b, "text/html; charset=utf-8", "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; font-src data:".into())
            };
            tauri::http::Response::builder().status(status).header("Content-Type", kind).header("Content-Security-Policy", csp)
                .body(body.into_bytes()).unwrap_or_default()
        })
        .register_uri_scheme_protocol("tzplugin", |_ctx, req| {
            let (status, body) = plugin_code(req.uri().path());
            tauri::http::Response::builder().status(status)
                .header("Content-Type", "text/javascript; charset=utf-8")
                // the window imports it from its own origin
                .header("Access-Control-Allow-Origin", "*")
                .header("Cache-Control", "no-store")
                .body(body.into_bytes()).unwrap_or_default()
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            {
                let handle = app.handle().clone();
                app.set_menu(menu(&handle)?)?;
                app.on_menu_event(|app, e| {
                    let _ = app.emit("menu", e.id().0.clone());
                });
            }
            let _ = app;
            Ok(())
        })
        .manage(Core::default())
        .invoke_handler(tauri::generate_handler![start, stop, send, op, trust, pick_folder, pick_files, open_key_page, open_link, notify, ui_log, widgets_list, widgets_new, widgets_path, widgets_remove, plugins_enable, plugins_code, hotkey, interrupt, new_chat, set_profile])
        .on_window_event(|w, e| {
            if let tauri::WindowEvent::Destroyed = e {
                let core = w.state::<Core>();
                let tids: Vec<String> = core.threads.lock().unwrap().keys().cloned().collect();
                for t in tids {
                    stop_thread(&core, &t);
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("tazuna desktop failed to start");
}

#[cfg(test)]
mod tests {
    /// The dry run's page for a plugin, written to TAZUNA_DRYRUN_OUT for a browser to run (the window's own check).
    #[test]
    fn dry_run_page_wraps_the_plugin() {
        let home = std::env::temp_dir().join(format!("tazuna-dry-{}", std::process::id()));
        let dir = home.join(".config/tazuna/desktop/plugins/demo");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("plugin.js"), std::env::var("TAZUNA_DRYRUN_CODE").unwrap_or_else(|_|
            "export default function (ctx) { ctx.slots.register('composer.dock', { id: 'bar', render: () => null }); ctx.slots.hide('rail.today'); }".into())).unwrap();
        // SAFETY: one test sets HOME before reading it; nothing else runs in this test binary
        unsafe { std::env::set_var("HOME", &home) };
        let (status, html) = super::dry_run_page("demo");
        assert_eq!(status, 200);
        assert!(html.contains(r#"<script src="../__dryrun-code/demo">"#) && !html.contains("ctx.slots.register('composer.dock'"), "the code is not pasted in: {html}");
        assert!(super::dry_run_code("/__dryrun-code/demo").1.starts_with("window.__plugin ="));
        assert_eq!(super::dry_run_page("../etc").0, 404);
        assert!(super::dry_run_csp("demo").ends_with("script-src 'unsafe-inline' tzwidget://localhost/__dryrun-code/demo") || cfg!(windows));
        assert!(super::dry_run_csp("a b; script-src *").ends_with("/__dryrun-code/-"), "an id that is not one stays out of the header");
        if let Ok(out) = std::env::var("TAZUNA_DRYRUN_OUT") {
            std::fs::write(out, &html).unwrap();
        }
        let _ = std::fs::remove_dir_all(&home);
    }
}
