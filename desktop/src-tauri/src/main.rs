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
    if msg.get("type").is_some() || !matches!(msg["op"].as_str(), Some("models" | "model" | "effort" | "sessions" | "resume" | "mode" | "answer" | "diff" | "undo" | "settings" | "login" | "data" | "route" | "full" | "rename" | "hide")) {
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
            &item("theme", "Light or Dark", "CmdOrCtrl+Shift+L")?, &sep()?, &P::fullscreen(app, None)?,
        ])?,
        &Submenu::with_items(app, "Window", true, &[&P::minimize(app, None)?, &P::maximize(app, None)?])?,
    ])
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
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
        .invoke_handler(tauri::generate_handler![start, stop, send, op, trust, pick_folder, pick_files, open_key_page, open_link, notify, ui_log, interrupt, new_chat, set_profile])
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
