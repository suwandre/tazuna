// tazuna desktop: the window around tazuna's own core. Each window runs `tazuna app-server` (the same session model the
// terminal UI renders) and relays: the session's views to the frontend as `tazuna-view` events, the user's input back as
// JSON lines. docs/DESKTOP.md.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

#[cfg(unix)]
static PATH: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();

#[derive(Default)]
struct Core {
    child: Mutex<Option<(Child, ChildStdin)>>,
    /// Bumped on each start, so an old core's exit is not reported as the new one's.
    started: AtomicU64,
}

/// The tazuna binary: TAZUNA_BIN, else `tazuna` on the PATH (a bundled sidecar later).
fn tazuna_bin() -> String {
    std::env::var("TAZUNA_BIN").unwrap_or_else(|_| "tazuna".into())
}

/// A window opened from the Finder or a desktop launcher gets a bare PATH (/usr/bin:/bin...), without the folders
/// where tazuna and the `claude` CLI usually live: take the PATH the user's login shell sets up.
#[cfg(unix)]
fn login_path() -> Option<String> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    let out = Command::new(shell).args(["-lic", "printf '\\n%s' \"$PATH\""]).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    String::from_utf8(out.stdout).ok()?.lines().last().map(str::to_string).filter(|p| !p.is_empty())
}

/// Starts (or restarts) the core in `cwd` (a project) or the home folder (a chat). Async: stopping the old core waits for
/// it, off the main thread.
#[tauri::command]
async fn start(app: AppHandle, core: State<'_, Core>, cwd: Option<String>, profile: Option<String>) -> Result<(), String> {
    stop_child(&core);
    let dir = cwd.or_else(|| std::env::var("HOME").ok()).unwrap_or_else(|| ".".into());
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
    let stdin = child.stdin.take().ok_or("no stdin")?;
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let run = core.started.fetch_add(1, Ordering::SeqCst) + 1;
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                let _ = app.emit("tazuna-view", v);
            }
        }
        if app.state::<Core>().started.load(Ordering::SeqCst) == run {
            let _ = app.emit("tazuna-exit", ());
        }
    });
    *core.child.lock().unwrap() = Some((child, stdin));
    Ok(())
}

fn write(core: &State<Core>, v: serde_json::Value) -> Result<(), String> {
    let mut guard = core.child.lock().unwrap();
    let (_, stdin) = guard.as_mut().ok_or("tazuna is not running")?;
    writeln!(stdin, "{v}").and_then(|_| stdin.flush()).map_err(|e| e.to_string())
}

fn stop_child(core: &State<Core>) {
    if let Some((mut child, mut stdin)) = core.child.lock().unwrap().take() {
        let _ = writeln!(stdin, "{}", serde_json::json!({"op": "quit"}));
        drop(stdin);
        let _ = child.wait();
    }
}

/// `agent`: past the router, straight to the agent ("send to agent" on a message answered as a chat).
#[tauri::command]
fn send(core: State<Core>, text: String, agent: bool) -> Result<(), String> {
    write(&core, serde_json::json!({"op": "send", "text": text, "agent": agent}))
}

/// The user's yes to trusting the open folder's settings (hooks, skills); the window starts the core again after it.
#[tauri::command]
fn trust(core: State<Core>) -> Result<(), String> {
    write(&core, serde_json::json!({"op": "trust"}))
}

/// The native folder picker; async so it does not block the main thread it needs.
#[tauri::command]
async fn pick_folder(app: AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    app.dialog().file().blocking_pick_folder().and_then(|p| p.into_path().ok()).map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
fn interrupt(core: State<Core>) -> Result<(), String> {
    write(&core, serde_json::json!({"op": "interrupt"}))
}

#[tauri::command]
fn new_chat(core: State<Core>) -> Result<(), String> {
    write(&core, serde_json::json!({"op": "new"}))
}

#[tauri::command]
fn set_profile(core: State<Core>, name: String) -> Result<(), String> {
    write(&core, serde_json::json!({"op": "profile", "name": name}))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Core::default())
        .invoke_handler(tauri::generate_handler![start, send, trust, pick_folder, interrupt, new_chat, set_profile])
        .on_window_event(|w, e| {
            if let tauri::WindowEvent::Destroyed = e {
                stop_child(&w.state::<Core>());
            }
        })
        .run(tauri::generate_context!())
        .expect("tazuna desktop failed to start");
}
