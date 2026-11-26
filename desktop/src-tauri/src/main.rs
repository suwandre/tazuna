// tazuna desktop: the window around tazuna's own core. Each open conversation (a "thread", by an id the window picks)
// runs its own `tazuna app-server` (the same session model the terminal UI renders), so one can keep working while
// another is open. The relay: each session's views to the frontend as `tazuna-view` events tagged with their thread, the
// user's input back as JSON lines. docs/DESKTOP.md.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::borrow::Cow;
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
async fn start(app: AppHandle, core: State<'_, Core>, tid: String, cwd: Option<String>, profile: Option<String>, saved_model: Option<serde_json::Value>) -> Result<String, String> {
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
    // the app has no console, so Windows would open a new one for the core: CREATE_NO_WINDOW
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000);
    let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()
        .map_err(|e| format!("could not start {} app-server: {e}", tazuna_bin()))?;
    eprintln!("tazuna desktop: app-server running (pid {})", child.id());
    let mut stdin = child.stdin.take().ok_or("no stdin")?;
    // The saved pick goes in before the window can send a message to this thread.
    if let Some(m) = saved_model.filter(|m| m["model"].is_string() && m["url"].is_string() && m["key_env"].is_string()) {
        let req = serde_json::json!({"op": "model", "restore": true, "model": m["model"], "url": m["url"], "key_env": m["key_env"]});
        writeln!(stdin, "{req}").map_err(|e| e.to_string())?;
    }
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
    if let Some((_, child, stdin)) = gone {
        quit_core(child, stdin);
    }
}

/// Asks a core to quit and waits for it, at most 3 seconds, then ends it. A core in the middle of a model call or a test
/// run reads the quit only when that ends, and the window waited for it without limit: an update's "Installing… tazuna
/// restarts by itself" stayed on screen until the reply was done (2026-11-18).
// ponytail: the core's own claude runs in a session of its own (platform::detach), so a killed core leaves it to end at
// its stdin's close; kill its tree from the core's side if that ever costs a reply's worth of tokens
fn quit_core(mut child: Child, mut stdin: ChildStdin) {
    let _ = writeln!(stdin, "{}", serde_json::json!({"op": "quit"}));
    drop(stdin);
    let end = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while std::time::Instant::now() < end {
        if !matches!(child.try_wait(), Ok(None)) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    eprintln!("tazuna desktop: app-server {} did not quit in 3 s: ended", child.id());
    let _ = child.kill();
    let _ = child.wait();
}

/// Every core at once (the window closing, an update's restart): their 3 seconds side by side, not one after another.
fn stop_all(core: &Core) {
    let all: Vec<(Child, ChildStdin)> = core.threads.lock().unwrap().drain().map(|(_, (_, c, i))| (c, i)).collect();
    let quits: Vec<_> = all.into_iter().map(|(c, i)| std::thread::spawn(move || quit_core(c, i))).collect();
    for q in quits {
        let _ = q.join();
    }
}

/// `agent`: past the router, straight to the agent ("send to agent" on a message answered as a chat).
#[tauri::command]
fn send(core: State<Core>, tid: String, text: String, agent: bool, draft: Option<serde_json::Value>) -> Result<(), String> {
    write(&core, &tid, serde_json::json!({"op": "send", "text": text, "agent": agent, "draft": draft}))
}

/// The user's yes to trusting the open folder's settings (hooks, skills); the window starts the core again after it.
#[tauri::command]
fn trust(core: State<Core>, tid: String) -> Result<(), String> {
    write(&core, &tid, serde_json::json!({"op": "trust"}))
}

/// A provider's page for making a key, in the browser: only https pages of the providers tazuna knows.
#[tauri::command]
fn open_key_page(url: String) -> Result<(), String> {
    const HOSTS: [&str; 6] = ["subrouter.ai", "mortera.cloud", "openrouter.ai", "kenari.id", "console.mistral.ai", "platform.cheaperinference.com"];
    let host = url.strip_prefix("https://").and_then(|r| r.split(['/', '?', '#']).next()).unwrap_or_default();
    // OmniRoute's dashboard is on this machine, over plain http: that one address exactly
    if (!HOSTS.contains(&host) && url != "http://localhost:20128") || url.chars().any(|c| c.is_whitespace() || c == '"') {
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

/// The editors "Open in" offers where they are installed (design review 14), by id: the macOS app's name (in /Applications or
/// ~/Applications), the command on the PATH elsewhere, and on Windows the program under %LOCALAPPDATA%\Programs.
const EDITORS: [(&str, &str, &[&str], &str); 4] = [
    ("vscode", "Visual Studio Code", &["code"], "Microsoft VS Code\\Code.exe"),
    ("cursor", "Cursor", &["cursor"], "cursor\\Cursor.exe"),
    ("zed", "Zed", &["zed", "zeditor"], "Zed\\Zed.exe"),
    ("windsurf", "Windsurf", &["windsurf"], "Windsurf\\Windsurf.exe"),
];
/// The terminals tried on Linux, in order: the first on the PATH opens in the folder (its working directory).
#[cfg(all(unix, not(target_os = "macos")))]
const TERMINALS: [&str; 9] = ["x-terminal-emulator", "gnome-terminal", "konsole", "xfce4-terminal", "kitty", "alacritty", "wezterm", "foot", "xterm"];
/// Package folders macOS opens as one thing (`open Calculator.app` launches it): never a project folder.
#[cfg(target_os = "macos")]
const PACKAGES: [&str; 12] = ["app", "bundle", "framework", "plugin", "kext", "pkg", "mpkg", "xpc", "appex", "prefpane", "qlgenerator", "saver"];

/// The PATH an opened app gets: the login shell's (as the cores get it) or the app's own, without its relative entries
/// ("." or an empty one), so nothing in a project's folder stands in for a program or a helper a launcher script runs.
#[inline(never)]
fn absolute_path_var() -> Option<std::ffi::OsString> {
    #[cfg(unix)]
    let path = PATH.get_or_init(login_path).clone().or_else(|| std::env::var("PATH").ok())?;
    #[cfg(not(unix))]
    let path = std::env::var("PATH").ok()?;
    std::env::join_paths(std::env::split_paths(&path).filter(|d| d.is_absolute())).ok()
}

/// A program on that PATH, by name, as an absolute path.
#[cfg(all(unix, not(target_os = "macos")))]
#[inline(never)]
fn on_path(name: &str) -> Option<std::path::PathBuf> {
    std::env::split_paths(&absolute_path_var()?).map(|d| d.join(name)).find(|p| p.is_file())
}

/// How app `id` starts on this machine (the program, by its absolute path, and its arguments before the folder), None where
/// it is not here. Never a bare name: the terminal runs in the project's folder, and one could be found there first (a
/// repository with its own cmd.bat; security review of design review 14).
#[inline(never)]
fn open_in_command(id: &str) -> Option<(String, Vec<String>)> {
    #[cfg(target_os = "macos")]
    {
        let s = |x: &str| x.to_string();
        if id == "files" { return Some((s("/usr/bin/open"), vec![])); }
        if id == "terminal" { return Some((s("/usr/bin/open"), vec![s("-a"), s("Terminal")])); }
        let (_, app, _, _) = EDITORS.iter().find(|e| e.0 == id)?;
        let home = std::env::var("HOME").unwrap_or_default();
        let found = [std::path::PathBuf::from("/Applications"), std::path::Path::new(&home).join("Applications")].iter().any(|d| d.join(format!("{app}.app")).is_dir());
        found.then(|| (s("/usr/bin/open"), vec![s("-a"), s(app)]))
    }
    #[cfg(target_os = "windows")]
    {
        let s = |x: &str| x.to_string();
        let root = std::env::var("SystemRoot").unwrap_or_else(|_| s(r"C:\Windows"));
        if id == "files" { return Some((format!(r"{root}\explorer.exe"), vec![])); }
        // the shell itself in a console of its own (open_in), no `start`: the folder is its working directory
        if id == "terminal" { return Some((std::env::var("ComSpec").unwrap_or_else(|_| format!(r"{root}\System32\cmd.exe")), vec![])); }
        let (_, _, _, exe) = EDITORS.iter().find(|e| e.0 == id)?;
        let p = std::path::Path::new(&std::env::var("LOCALAPPDATA").ok()?).join("Programs").join(exe);
        p.is_file().then(|| (p.to_string_lossy().into_owned(), vec![]))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if id == "files" { return on_path("xdg-open").map(|p| (p.to_string_lossy().into_owned(), vec![])); }
        if id == "terminal" { return TERMINALS.iter().find_map(|t| on_path(t)).map(|p| (p.to_string_lossy().into_owned(), vec![])); }
        let (_, _, names, _) = EDITORS.iter().find(|e| e.0 == id)?;
        names.iter().find_map(|n| on_path(n)).map(|p| (p.to_string_lossy().into_owned(), vec![]))
    }
}

/// A project's folder in the file manager, a terminal or an editor ("Open in", design review 14). Without `app`: the apps
/// there are on this machine, by id ("files", "terminal", then the editors installed). With it: only an app of that list,
/// only an existing folder given whole (an absolute path, so never read as an option; on macOS no package), started
/// without a shell, with a PATH of absolute entries; only the terminal starts in the folder, the others are given it.
#[tauri::command]
fn open_in(path: Option<String>, app: Option<String>) -> Result<Vec<String>, String> {
    let Some(app) = app else {
        return Ok(["files", "terminal"].into_iter().chain(EDITORS.iter().map(|e| e.0)).filter(|id| open_in_command(id).is_some()).map(str::to_string).collect());
    };
    let dir = std::path::PathBuf::from(path.unwrap_or_default());
    if !dir.is_absolute() || !dir.is_dir() {
        return Err(format!("not a folder: {}", dir.display()));
    }
    #[cfg(target_os = "macos")]
    if dir.extension().and_then(|e| e.to_str()).is_some_and(|e| PACKAGES.contains(&e.to_ascii_lowercase().as_str())) {
        return Err(format!("not a project folder: {}", dir.display()));
    }
    let Some((program, args)) = open_in_command(&app) else { return Err(format!("not an app to open in: {app}")) };
    let terminal = app == "terminal";
    let mut cmd = Command::new(program);
    cmd.args(args);
    if let Some(p) = absolute_path_var() {
        cmd.env("PATH", p);
    }
    if terminal {
        // its working directory is the folder (on Windows and Linux its only way in); its console is its own
        cmd.current_dir(&dir);
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
            cmd.creation_flags(CREATE_NEW_CONSOLE);
        }
    } else {
        cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    }
    if !terminal || cfg!(target_os = "macos") {
        cmd.arg(&dir);
    }
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    // reaped off this thread: a terminal or an editor runs on
    std::thread::spawn(move || { let _ = child.wait(); });
    Ok(vec![])
}

/// A line from the page on stderr ("loaded", a script error): where the window stops shows in a terminal or a CI log.
#[tauri::command]
fn ui_log(msg: String) {
    eprintln!("tazuna desktop (page): {}", msg.chars().take(500).collect::<String>());
}

fn home_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_else(|_| ".".into()))
}

/// The desktop app's own folder: ~/.config/tazuna/desktop (under the user's profile on Windows).
fn desktop_dir() -> std::path::PathBuf {
    home_dir().join(".config/tazuna/desktop")
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
            return Some(serde_json::json!({"id": id, "kind": "plugin", "title": title, "state": state, "print": print, "imported": e.path().join(".imported").exists(),
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

/// The files a shared panel or plugin carries (docs/PLUGINS.md, Sharing): nothing else is written on import.
const SHARED: [&str; 3] = ["manifest.json", "index.html", "plugin.js"];

/// A panel or plugin as one file to share, `{"tazuna_plugin": 1, "id", "files": {name: text}}`, saved where the user
/// picks; its path, or None when the dialog was cancelled.
#[tauri::command]
async fn plugins_export(app: AppHandle, id: String) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    if !widget_id_ok(&id) {
        return Err(format!("not a plugin id: {id}"));
    }
    let dir = widgets_dir().join(&id);
    let files: serde_json::Map<String, serde_json::Value> = SHARED.iter()
        .filter_map(|n| std::fs::read_to_string(dir.join(n)).ok().map(|t| (n.to_string(), serde_json::json!(t)))).collect();
    let text = serde_json::to_string_pretty(&serde_json::json!({"tazuna_plugin": 1, "id": id, "files": files})).map_err(|e| e.to_string())?;
    let Some(path) = app.dialog().file().set_file_name(format!("{id}.tazuna.json")).blocking_save_file().and_then(|p| p.into_path().ok()) else { return Ok(None) };
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

/// A shared panel or plugin (the shared file's text, from a file or the gallery) into the plugins folder under a free id:
/// only its manifest, index.html and plugin.js, as text, 1 MB together at most. A plugin arrives off: it runs only once the
/// user has read its code and enabled it (Settings), like one the agent made; an approval left under the same id goes.
#[tauri::command]
fn plugins_import(text: String) -> Result<String, String> {
    import_into(&widgets_dir(), &text)
}

fn import_into(root: &std::path::Path, text: &str) -> Result<String, String> {
    let v: serde_json::Value = serde_json::from_str(text).map_err(|_| "not a shared tazuna panel or plugin".to_string())?;
    let files = v["files"].as_object().filter(|_| v["tazuna_plugin"] == 1).ok_or("not a shared tazuna panel or plugin")?;
    if let Some(odd) = files.keys().find(|k| !SHARED.contains(&k.as_str())) {
        return Err(format!("it carries {odd}: a shared panel or plugin holds only manifest.json and index.html or plugin.js"));
    }
    let texts: Vec<(&str, &str)> = files.iter().map(|(k, t)| t.as_str().map(|t| (k.as_str(), t)).ok_or(format!("{k} is not text"))).collect::<Result<_, _>>()?;
    if texts.iter().map(|(_, t)| t.len()).sum::<usize>() > 1 << 20 {
        return Err("it is over 1 MB".into());
    }
    let manifest = files.get("manifest.json").and_then(|m| m.as_str()).and_then(|m| serde_json::from_str::<serde_json::Value>(m).ok()).filter(|m| m.is_object());
    if manifest.is_none() || !(files.contains_key("index.html") || files.contains_key("plugin.js")) {
        return Err("it needs a manifest.json and an index.html or a plugin.js".into());
    }
    let base = v["id"].as_str().filter(|i| widget_id_ok(i) && i.len() <= 56).unwrap_or("shared").to_string();
    let id = (1..100).map(|n| if n == 1 { base.clone() } else { format!("{base}-{n}") }).find(|i| !root.join(i).exists()).ok_or("no free id")?;
    let dir = root.join(&id);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    for (name, t) in texts {
        std::fs::write(dir.join(name), t).map_err(|e| e.to_string())?;
    }
    // tazuna's own mark (no shared file can carry it): Settings warns harder before enabling what came from someone else
    std::fs::write(dir.join(".imported"), "").map_err(|e| e.to_string())?;
    if root == widgets_dir() {
        let mut a = approvals();
        if a.remove(&id).is_some() {
            save_approvals(&a)?;
        }
    }
    Ok(id)
}

/// The file picker for a shared panel or plugin, then plugins_import; None when cancelled.
#[tauri::command]
async fn plugins_import_file(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let Some(path) = app.dialog().file().add_filter("tazuna panel or plugin", &["json"]).blocking_pick_file().and_then(|p| p.into_path().ok()) else { return Ok(None) };
    if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 2 << 20 {
        return Err("it is over 2 MB".into());
    }
    plugins_import(std::fs::read_to_string(&path).map_err(|e| e.to_string())?).map(Some)
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

/// A newer tazuna desktop, from the desktop releases' latest.json (desktop.yml writes it): its version and notes, or
/// null. Each build it names is signed with tazuna's updater key; the app checks that signature before installing.
#[tauri::command]
async fn update_check(app: AppHandle) -> Result<Option<serde_json::Value>, String> {
    use tauri_plugin_updater::UpdaterExt;
    let u = match app.updater().map_err(|e| e.to_string()) {
        Ok(up) => up.check().await.map_err(|e| e.to_string()),
        Err(e) => Err(e),
    };
    eprintln!("tazuna desktop: update check: {}", match &u {
        Ok(Some(u)) => format!("{} available", u.version),
        Ok(None) => "up to date".into(),
        Err(e) => format!("failed: {e}"),
    });
    let u = u?;
    Ok(u.map(|u| serde_json::json!({"version": u.version, "notes": u.body})))
}

/// The update downloaded and its signature checked, then every conversation's core stopped (they stay resumable), the
/// new version installed and started.
#[tauri::command]
async fn update_install(app: AppHandle, core: State<'_, Core>) -> Result<(), String> {
    use tauri_plugin_updater::UpdaterExt;
    // the download's progress for the window ("update-progress"), and an end to it: GitHub's release storage served the
    // 6 MB at 80-200 KB/s (2026-11-18), and the window said only "Installing…" for over a minute
    let updater = app.updater_builder().timeout(std::time::Duration::from_secs(300)).build().map_err(|e| e.to_string())?;
    let u = updater.check().await.map_err(|e| e.to_string())?.ok_or("no update to install")?;
    let (window, mut got, mut said) = (app.clone(), 0u64, 0u64);
    let bytes = u.download(move |n, total| {
        got += n as u64;
        if got - said >= 64 * 1024 || total == Some(got) {
            said = got;
            let _ = window.emit("update-progress", serde_json::json!({"got": got, "total": total}));
        }
    }, || {}).await.map_err(|e| e.to_string())?;
    let _ = app.emit("update-progress", serde_json::json!({"installing": true}));
    stop_all(&core);
    eprintln!("tazuna desktop: installing {}", u.version);
    u.install(bytes).map_err(|e| e.to_string())?;
    app.restart()
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

/// An image file as a data URL, for an attachment's thumbnail and an image an answer names (the page's own policy lets it
/// load nothing from disk). Images only, by their extension, and up to 8 MB; `cwd` is the folder a relative path is from.
#[tauri::command]
async fn image_data(path: String, cwd: Option<String>) -> Result<String, String> {
    let p = std::path::Path::new(&path);
    let p = if p.is_absolute() { p.to_path_buf() } else { std::path::Path::new(&cwd.ok_or("a relative path needs its folder")?).join(p) };
    let mime = match p.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("png") => "image/png", Some("jpg" | "jpeg") => "image/jpeg", Some("gif") => "image/gif", Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml", Some("bmp") => "image/bmp", Some("avif") => "image/avif",
        _ => return Err("not an image".into()),
    };
    if std::fs::metadata(&p).map_err(|e| e.to_string())?.len() > 8 << 20 { return Err("larger than 8 MB".into()); }
    Ok(format!("data:{mime};base64,{}", base64(&std::fs::read(&p).map_err(|e| e.to_string())?)))
}

fn paste_bytes(body: &tauri::ipc::InvokeBody) -> Result<Cow<'_, [u8]>, String> {
    use tauri::ipc::InvokeBody::{Json, Raw};

    match body {
        Raw(bytes) => Ok(Cow::Borrowed(bytes)),
        Json(serde_json::Value::Array(bytes)) => bytes.iter().map(|byte| byte.as_u64().and_then(|n| u8::try_from(n).ok())
            .ok_or_else(|| "no image in the paste".into())).collect::<Result<_, _>>().map(Cow::Owned),
        Json(_) => Err("no image in the paste".into()),
    }
}

/// A pasted image (Cmd-V of a screenshot, or of an image file copied in Finder) saved as a file the message can name: the
/// window has its bytes, not a path. Tauri delivers raw bytes or a JSON byte array via its fallback IPC; the kind is in
/// an `x-ext` header. Saves up to 20 MB in the app's own temp folder, readable by this user alone.
#[tauri::command]
fn paste_image(request: tauri::ipc::Request<'_>) -> Result<String, String> {
    let bytes = paste_bytes(request.body())?;
    let ext = match request.headers().get("x-ext").and_then(|v| v.to_str().ok()).map(str::to_ascii_lowercase).as_deref() {
        Some(e @ ("png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "avif")) => e.to_string(),
        _ => "png".into(),
    };
    save_pasted_image(&bytes, &ext)
}

/// WebKitGTK can deliver an empty paste event even when the system clipboard holds an image.
#[tauri::command]
async fn paste_clipboard_image(_app: AppHandle) -> Result<Option<String>, String> {
    #[cfg(target_os = "linux")]
    {
        let (tx, mut rx) = tauri::async_runtime::channel(1);
        _app.run_on_main_thread(move || {
            gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD).request_image(move |_, image| {
                let bytes = image.map(|p| p.save_to_bufferv("png", &[]).map_err(|e| e.to_string())).transpose();
                let _ = tx.try_send(bytes);
            });
        }).map_err(|e| e.to_string())?;
        let bytes = rx.recv().await.ok_or("could not read the clipboard")??;
        bytes.map(|b| save_pasted_image(&b, "png")).transpose()
    }
    #[cfg(not(target_os = "linux"))]
    Ok(None)
}

fn save_pasted_image(bytes: &[u8], ext: &str) -> Result<String, String> {
    if bytes.is_empty() || bytes.len() > 20 << 20 { return Err("an image up to 20 MB".into()); }
    let dir = std::env::temp_dir().join("tazuna-desktop-images");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    std::fs::set_permissions(&dir, std::os::unix::fs::PermissionsExt::from_mode(0o700)).map_err(|e| e.to_string())?;
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos();
    let path = dir.join(format!("paste-{nanos}.{ext}"));
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

/// Standard base64 with padding (the one use here does not earn a dependency).
fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            out.push(if i <= c.len() { ABC[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    out
}

#[cfg(test)]
#[test]
fn base64_pads() {
    assert_eq!([base64(b""), base64(b"f"), base64(b"fo"), base64(b"foo"), base64(b"foob")], ["", "Zg==", "Zm8=", "Zm9v", "Zm9vYg=="]);
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
    if !is_window_op(&msg) {
        return Err(format!("not a command the window sends: {msg}"));
    }
    write(&core, &tid, msg)
}

fn is_window_op(msg: &serde_json::Value) -> bool {
    msg.get("type").is_none() && matches!(msg["op"].as_str(), Some("models" | "refresh_model_list" | "model" | "effort" | "sessions" | "resume" | "mode" | "answer" | "diff" | "undo" | "settings" | "login" | "data" | "route" | "full" | "rename" | "hide" | "rewind" | "mcp" | "mcp_on" | "mcp_add" | "mcp_remove" | "mcp_check" | "hook" | "setup" | "gallery" | "gallery_get" | "accounts" | "review" | "piece" | "allow_root" | "fork" | "usage" | "spend_cap" | "worktree_new" | "worktrees" | "worktree_remove" | "pr" | "create" | "create_enable" | "create_discard" | "retry" | "complete" | "agent" | "task_stop" | "usage_sources" | "usage_track" | "usage_allowance"))
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

// The console reader (usage sources, review 14; experimental, off by default): a provider's own usage page, read in a window
// of tazuna's with the user's own sign-in, for the providers whose API tells no usage. The window's initialization script
// (console.js) watches the JSON the page itself gets and reports through one command (`report`) that checks everything again
// and writes under ~/.local/share/tazuna, where the core reads it: console-learn/<provider>.json (shapes only, from the first
// visit) and console/<provider>.json (the numbers). tazuna keeps no password: the sign-in lives in the window's own store.

/// A provider whose usage page tazuna may read: its name as the core says it, the page its window opens on (its LOGINS
/// page), the exact hosts of its console (capabilities/console.json lists the same; sign-in pages elsewhere get no script
/// and no command), and its
/// extractor: a JS function (path, json) => {left, total, unit, used_pct, resets} | null, written from its learn file
/// (Phase 2; until then none, and nothing is read in the background).
struct Console {
    name: &'static str,
    start: &'static str,
    sites: &'static [&'static str],
    read: Option<&'static str>,
}

const CONSOLES: [Console; 3] = [
    Console { name: "Qwen Cloud Token Plan", start: "https://home.qwencloud.com/api-keys?region=ap-southeast-1", sites: &["home.qwencloud.com"], read: None },
    Console { name: "Mistral Vibe", start: "https://chat.mistral.ai/code/extensions", sites: &["chat.mistral.ai", "console.mistral.ai", "admin.mistral.ai"], read: None },
    Console { name: "Caffeine", start: "https://caffeine.ai/inference/api-keys", sites: &["caffeine.ai", "www.caffeine.ai"], read: None },
];

const CONSOLE_JS: &str = include_str!("console.js");

/// The console windows' state: whether a hidden reading runs (one at a time), and who waits for its report.
#[derive(Default)]
struct Consoles {
    hidden: std::sync::atomic::AtomicBool,
    waiting: Mutex<Option<(String, std::sync::mpsc::Sender<&'static str>)>>,
    /// held around a learn file's read, merge and write: reports arrive on several threads at once
    learning: Mutex<()>,
}

fn console_of(provider: &str) -> Option<&'static Console> {
    CONSOLES.iter().find(|c| c.name == provider)
}

/// A provider's name in a file or window name, as the core names its credits files: "Qwen-Cloud-Token-Plan".
fn slug(name: &str) -> String {
    name.replace(|c: char| !c.is_ascii_alphanumeric(), "-")
}

fn label_of(name: &str) -> String {
    format!("console-{}", slug(name))
}

/// tazuna's data, as the core has it: ~/.local/share/tazuna.
fn share_dir() -> std::path::PathBuf {
    home_dir().join(".local/share/tazuna")
}

/// A URL over https on one of `sites`, exactly (no host under it: any subdomain could otherwise write a reading).
fn on_site(sites: &[&str], url: &str) -> bool {
    let Ok(u) = tauri::Url::parse(url) else { return false };
    u.scheme() == "https" && u.port().is_none() && sites.contains(&u.host_str().unwrap_or_default())
}

/// A report comes from a window opened for that provider: `console-<provider>`, or its background reading's
/// `console-<provider>-read` (a label of its own: a sign-in window opening never waits on one closing).
fn caller_ok(label: &str, provider: &str) -> bool {
    console_of(provider).is_some() && (label == label_of(provider) || label == format!("{}-read", label_of(provider)))
}

// no "token": accessToken is a sign-in, not usage
const USAGE_WORDS: [&str; 9] = ["quota", "usage", "remain", "balance", "credit", "limit", "used", "total", "plan"];

/// A name that may be a value (console.js valueLike has the same rules): a UUID, five digits or sixteen hex digits in a row, a
/// run of 8+ letters and digits mixed or of 8+ with a third of it capitals, a prefixed id ("cus_NffrFeUf", "sk-AbCdEf").
fn value_like(s: &str) -> bool {
    let run = |f: fn(char) -> bool| s.split(|c: char| !f(c)).map(str::len).max().unwrap_or(0);
    let uuid = s.split(|c: char| !(c.is_ascii_hexdigit() || c == '-')).any(|r| {
        let g: Vec<&str> = r.split('-').collect();
        g.windows(5).any(|w| w.iter().map(|x| x.len()).eq([8, 4, 4, 4, 12]))
    });
    let mixed = s.split(|c: char| !c.is_ascii_alphanumeric()).any(|t| {
        let caps = t.chars().filter(char::is_ascii_uppercase).count();
        t.len() >= 8 && ((t.chars().any(|c| c.is_ascii_digit()) && t.chars().any(|c| c.is_ascii_alphabetic()))
            || (t.chars().any(|c| c.is_ascii_lowercase()) && caps >= 2 && caps * 3 >= t.len()))
    });
    // [a-z]{2,6}[_-] after a start or a non-alphanumeric, then 6+ letters and digits with a capital or a digit
    let b = s.as_bytes();
    let prefixed = (0..b.len()).filter(|&i| b[i] == b'_' || b[i] == b'-').any(|i| {
        let pre = b[..i].iter().rev().take_while(|c| c.is_ascii_lowercase()).count();
        let before_ok = i == pre || !b[i - pre - 1].is_ascii_alphanumeric();
        let rest: Vec<u8> = b[i + 1..].iter().copied().take_while(u8::is_ascii_alphanumeric).collect();
        (2..=6).contains(&pre) && before_ok && rest.len() >= 6 && rest.iter().any(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
    });
    uuid || mixed || prefixed || run(|c| c.is_ascii_digit()) >= 5 || run(|c| c.is_ascii_hexdigit()) >= 16
}

/// A field name as console.js keeps it: "*" (one that may be a value), or a letter, "_" or "$" and then those and digits (a
/// dot, a dash or an @ makes it a value: john.smith, acme-prod).
fn key_ok(k: &str) -> bool {
    k == "*" || (k.len() <= 64 && k.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_' || c == '$')
        && k.chars().all(|c| c.is_ascii_alphanumeric() || "_$".contains(c)) && !value_like(k))
}

/// A path as console.js makes it: "/a/:id/b", its query's keys alone ("?region&lang"); a dot only in a file's name (api.json).
fn path_ok(p: &str) -> bool {
    let (path, query) = p.split_once('?').unwrap_or((p, ""));
    let file = |s: &str| s.split_once('.').is_some_and(|(n, ext)| n.len() <= 41 && n.starts_with(|c: char| c.is_ascii_alphabetic())
        && n.chars().all(|c| c.is_ascii_alphabetic() || c == '-') && ["json", "php", "html", "htm", "asp", "aspx", "do"].contains(&ext));
    let part = |s: &str| s.is_empty() || s == ":id" || (s.len() <= 64 && s.starts_with(|c: char| c.is_ascii_alphabetic())
        && (s.chars().all(|c| c.is_ascii_alphanumeric() || "_-".contains(c)) || file(s)) && !value_like(s));
    p.len() <= 512 && path.starts_with('/') && path.split('/').all(part) && (query.is_empty() || query.split('&').all(key_ok))
}

/// A shape and nothing more: type names as leaves ("string", "number", "boolean", "null"), objects of field names, arrays
/// of one item at most. A number, a boolean, null or any other string is a value and fails.
fn shape_ok(v: &serde_json::Value, depth: usize) -> bool {
    use serde_json::Value as V;
    depth <= 12 && match v {
        V::String(s) => matches!(s.as_str(), "string" | "number" | "boolean" | "null"),
        V::Array(a) => a.len() <= 1 && a.iter().all(|x| shape_ok(x, depth + 1)),
        V::Object(o) => o.len() <= 100 && o.iter().all(|(k, x)| key_ok(k) && shape_ok(x, depth + 1)),
        _ => false,
    }
}

/// The usage-like field names in a shape.
fn usage_keys(v: &serde_json::Value, out: &mut Vec<String>) {
    match v {
        serde_json::Value::Array(a) => a.iter().for_each(|x| usage_keys(x, out)),
        serde_json::Value::Object(o) => for (k, x) in o {
            let low = k.to_ascii_lowercase();
            if USAGE_WORDS.iter().any(|w| low.contains(w)) && !out.contains(k) {
                out.push(k.clone());
            }
            usage_keys(x, out);
        },
        _ => {}
    }
}

/// What a console page reported, checked.
#[derive(Debug, PartialEq)]
enum Report {
    /// a response's shape for the learn file: {path, method, status, shape, usage, page?}
    Learn(serde_json::Value),
    /// the numbers for the reading: {left, total, unit, used_pct, resets}, each there (null where not known)
    Read(serde_json::Value),
    SignedOut,
    /// the learn bar's Done: the window closes
    Done,
}

/// A console page's report, checked: it is the provider's page talking, and any script on it can call the command. From its
/// own window (`console-<provider>`) on that provider's sites, 64 KB at most, and only a shape (learn: field names and type
/// names, a cleaned path, a page on its sites only where usage-like fields appeared) or numbers (read: finite numbers or
/// null, one unit of four).
fn check_report(label: &str, url: &str, provider: &str, kind: &str, payload: &serde_json::Value) -> Result<Report, String> {
    let c = console_of(provider).ok_or("not a console reader's provider")?;
    if !caller_ok(label, provider) {
        return Err(format!("not {provider}'s console window: {label}"));
    }
    if !on_site(c.sites, url) {
        return Err(format!("not on {provider}'s console: {url}"));
    }
    if payload.to_string().len() > 64 * 1024 {
        return Err("a report over 64 KB".into());
    }
    let fields = |allowed: &[&str]| payload.as_object().filter(|o| o.keys().all(|k| allowed.contains(&k.as_str())));
    match kind {
        "learn" => {
            let o = fields(&["path", "method", "status", "shape", "page"]).ok_or("not a learn report")?;
            let path = o.get("path").and_then(|p| p.as_str()).filter(|p| path_ok(p)).ok_or("not a cleaned path")?;
            let method = o.get("method").and_then(|m| m.as_str())
                .filter(|m| matches!(*m, "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS")).ok_or("not a method")?;
            let status = o.get("status").and_then(|s| s.as_u64()).filter(|s| (100..600).contains(s)).ok_or("not a status")?;
            let shape = o.get("shape").filter(|s| s.is_object() || s.is_array()).filter(|s| shape_ok(s, 0)).ok_or("not a shape")?;
            let mut keys = vec![];
            usage_keys(shape, &mut keys);
            let page = match o.get("page") {
                None | Some(serde_json::Value::Null) => None,
                Some(p) => {
                    let p = p.as_str().filter(|p| on_site(c.sites, p)).ok_or("not a page on its console")?;
                    let path = tauri::Url::parse(p).map(|u| u.path().to_string()).unwrap_or_default();
                    if p.contains(['?', '#']) || !path_ok(&path) || keys.is_empty() {
                        return Err("not a usage page".into());
                    }
                    Some(p)
                }
            };
            let mut entry = serde_json::json!({"path": path, "method": method, "status": status, "shape": shape, "usage": !keys.is_empty()});
            if let Some(p) = page {
                entry["page"] = serde_json::json!(p);
            }
            Ok(Report::Learn(entry))
        }
        "read" if c.read.is_none() => Err(format!("no reader for {provider} yet")),
        "read" => check_numbers(payload),
        "signed_out" | "done" if payload.is_null() => Ok(if kind == "done" { Report::Done } else { Report::SignedOut }),
        _ => Err(format!("not a report: {kind}")),
    }
}

/// A reading's numbers: the five fields, finite numbers or null, one unit of four, at least one number.
fn check_numbers(payload: &serde_json::Value) -> Result<Report, String> {
    let fields = |allowed: &[&str]| payload.as_object().filter(|o| o.keys().all(|k| allowed.contains(&k.as_str())));
    {
        {
            let n = fields(&["numbers"]).and_then(|o| o.get("numbers")).and_then(|n| n.as_object())
                .filter(|n| n.keys().all(|k| ["left", "total", "unit", "used_pct", "resets"].contains(&k.as_str()))).ok_or("not a reading")?;
            // the core's own limits (src/accounts.rs console_check): finite and under 1e15, a total over 0, a share 0-1000, a
            // reset in epoch seconds (1e9 to 1e10: not milliseconds, not seconds from now)
            let num = |k: &str| match n.get(k) {
                None | Some(serde_json::Value::Null) => Ok(serde_json::Value::Null),
                Some(v) if v.as_f64().is_some_and(|f| f.is_finite() && f.abs() < 1e15) => Ok(v.clone()),
                Some(_) => Err(format!("{k} is not a number")),
            };
            let (left, total, used_pct, resets) = (num("left")?, num("total")?, num("used_pct")?, num("resets")?);
            if total.as_f64().is_some_and(|t| t <= 0.0) || used_pct.as_f64().is_some_and(|p| !(0.0..=1000.0).contains(&p))
                || resets.as_f64().is_some_and(|r| !(1e9..1e10).contains(&r)) {
                return Err("a total, a share or a reset out of range".into());
            }
            let unit = n.get("unit").and_then(|u| u.as_str()).filter(|u| matches!(*u, "usd" | "tokens" | "credits" | "percent")).ok_or("not a unit")?;
            if left.is_null() && total.is_null() && used_pct.is_null() {
                return Err("a reading with no number".into());
            }
            Ok(Report::Read(serde_json::json!({"left": left, "total": total, "unit": unit, "used_pct": used_pct, "resets": resets})))
        }
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// A file written whole or not at all (the core may read it at any moment).
fn write_atomic(path: &std::path::Path, v: &serde_json::Value) -> Result<(), String> {
    std::fs::create_dir_all(path.parent().ok_or("no folder")?).map_err(|e| e.to_string())?;
    static N: AtomicU64 = AtomicU64::new(0);
    let tmp = path.with_extension(format!("tmp{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
    std::fs::write(&tmp, serde_json::to_vec_pretty(v).unwrap_or_default()).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

/// The learn file with one more response: one entry per method and path (the newest), the usage page where the most
/// usage-like fields appeared (a chat page that mentions a plan does not replace /usage), 1 MB at most (the oldest go first).
fn merge_learn(mut old: serde_json::Value, entry: serde_json::Value, now: u64) -> serde_json::Value {
    let mut list: Vec<serde_json::Value> = old["responses"].as_array().cloned().unwrap_or_default();
    list.retain(|e| !(e["path"] == entry["path"] && e["method"] == entry["method"]));
    if entry["page"].is_string() {
        let mut keys = vec![];
        usage_keys(&entry["shape"], &mut keys);
        if keys.len() as u64 >= old["page_fields"].as_u64().unwrap_or(0) {
            old["page"] = entry["page"].clone();
            old["page_fields"] = serde_json::json!(keys.len());
        }
    }
    list.push(entry);
    while list.len() > 1 && (list.len() > 200 || serde_json::to_string(&list).map_or(0, |s| s.len()) > 1 << 20) {
        list.remove(0);
    }
    serde_json::json!({"at": now, "page": old["page"], "page_fields": old["page_fields"], "responses": list})
}

/// What the first visit found, for Settings: the usage page's path, the usage-like field names on it, when.
fn learned_summary(l: &serde_json::Value) -> serde_json::Value {
    let mut keys = vec![];
    for e in l["responses"].as_array().into_iter().flatten().filter(|e| e["usage"] == true) {
        usage_keys(&e["shape"], &mut keys);
    }
    let page = l["page"].as_str().and_then(|p| tauri::Url::parse(p).ok()).map(|u| u.path().to_string());
    serde_json::json!({"page": page, "fields": keys.len(), "at": l["at"]})
}

fn learn_file(root: &std::path::Path, provider: &str) -> std::path::PathBuf {
    root.join("console-learn").join(format!("{}.json", slug(provider)))
}

fn read_json(path: &std::path::Path) -> Option<serde_json::Value> {
    std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok())
}

/// A reading for the core: {"at", "status": "ok" | "signed_out" | "error", "numbers" (null unless ok), "source": "console"}.
fn write_reading(root: &std::path::Path, provider: &str, status: &str, numbers: serde_json::Value) -> Result<(), String> {
    let v = serde_json::json!({"at": now_secs(), "status": status, "numbers": numbers, "source": "console"});
    write_atomic(&root.join("console").join(format!("{}.json", slug(provider))), &v)
}

/// The console reader's one command, plugin:console|report (capabilities/console.json grants it to the console windows on
/// their providers' sites, and nothing else to them): the report checked, then written, the waiting reading told, or the
/// window closed (Done).
#[tauri::command]
async fn report(app: AppHandle, webview: tauri::Webview, provider: String, kind: String, payload: serde_json::Value) -> Result<(), String> {
    let url = webview.url().map_err(|e| e.to_string())?;
    let r = check_report(webview.label(), url.as_str(), &provider, &kind, &payload);
    if let Err(e) = &r {
        eprintln!("tazuna desktop: console report refused: {e}");
    }
    take_report(&app, &webview, &provider, r?)
}

#[inline(never)]
fn take_report(app: &AppHandle, webview: &tauri::Webview, provider: &str, r: Report) -> Result<(), String> {
    let root = share_dir();
    let tell = |kind: &'static str| {
        if let Some((p, tx)) = app.state::<Consoles>().waiting.lock().unwrap().as_ref() {
            if p == provider {
                let _ = tx.send(kind);
            }
        }
    };
    match r {
        Report::Learn(entry) => {
            let file = learn_file(&root, provider);
            let consoles = app.state::<Consoles>();
            let _one = consoles.learning.lock().unwrap();
            write_atomic(&file, &merge_learn(read_json(&file).unwrap_or_default(), entry, now_secs()))
        }
        Report::Read(numbers) => write_reading(&root, provider, "ok", numbers).map(|_| tell("read")),
        Report::SignedOut => write_reading(&root, provider, "signed_out", serde_json::Value::Null).map(|_| tell("signed_out")),
        Report::Done => webview.window().destroy().map_err(|e| e.to_string()),
    }
}

/// The macOS data store of a provider's console window (macOS 14 on; before it, WebKit's one default store): a fixed id per
/// provider, FNV-1a of its name (std's hasher may change between Rust releases, and the sign-in would be lost).
#[cfg(target_os = "macos")]
fn store_id(name: &str) -> [u8; 16] {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in name.bytes() {
        h = (h ^ b as u64).wrapping_mul(0x0100_0000_01b3);
    }
    let mut id = *b"tazuna-c\0\0\0\0\0\0\0\0";
    id[8..].copy_from_slice(&h.to_be_bytes());
    id
}

/// Elsewhere, its data folder under the app's own: …/console/<provider>.
#[cfg(not(target_os = "macos"))]
fn console_store(app: &AppHandle, c: &Console) -> Result<std::path::PathBuf, String> {
    Ok(app.path().app_data_dir().map_err(|e| e.to_string())?.join("console").join(slug(c.name)))
}

/// macOS 14 or later: before it WebKit has no store of a window's own, and a console window would share the main window's.
#[cfg(target_os = "macos")]
fn own_stores() -> Result<(), String> {
    let v = Command::new("/usr/bin/sw_vers").arg("-productVersion").output().ok().and_then(|o| String::from_utf8(o.stdout).ok()).unwrap_or_default();
    if v.split('.').next().and_then(|m| m.trim().parse::<u32>().ok()).unwrap_or(0) < 14 {
        return Err("Reading a provider's console needs macOS 14 or later.".into());
    }
    Ok(())
}

/// A provider's console window on `url`: its own store, the script only on its sites, https only.
#[inline(never)]
fn console_window(app: &AppHandle, c: &Console, mode: &str, url: &str, visible: bool) -> Result<tauri::WebviewWindow, String> {
    let label = if visible { label_of(c.name) } else { format!("{}-read", label_of(c.name)) };
    #[cfg(target_os = "macos")]
    own_stores()?;
    let cfg = serde_json::json!({"provider": c.name, "mode": mode, "sites": c.sites});
    let script = format!("{CONSOLE_JS}(window, {cfg}, {});", c.read.unwrap_or("null"));
    let url = tauri::Url::parse(url).map_err(|e| e.to_string())?;
    let b = tauri::WebviewWindowBuilder::new(app, label, tauri::WebviewUrl::External(url))
        .title(format!("{} usage — tazuna", c.name))
        .inner_size(1100.0, 780.0)
        .visible(visible)
        .initialization_script(script)
        .on_navigation(|u| u.scheme() == "https" || u.as_str() == "about:blank");
    // a sign-in in a popup (window.open: Google's, Internet Identity's): a window of its own, outside console-* (no report),
    // on the opener's webview configuration (its store), https only
    let (handle, owner) = (app.clone(), c.name);
    let b = b.on_new_window(move |url, features| popup(&handle, owner, url, features));
    #[cfg(target_os = "macos")]
    let b = b.data_store_identifier(store_id(c.name));
    #[cfg(not(target_os = "macos"))]
    let b = b.data_directory(console_store(app, c)?);
    b.build().map_err(|e| e.to_string())
}

#[inline(never)]
fn popup(app: &AppHandle, owner: &str, url: tauri::Url, features: tauri::webview::NewWindowFeatures) -> tauri::webview::NewWindowResponse<tauri::Wry> {
    static N: AtomicU64 = AtomicU64::new(0);
    if url.scheme() != "https" {
        return tauri::webview::NewWindowResponse::Deny;
    }
    let label = format!("popup-{}-{}", slug(owner), N.fetch_add(1, Ordering::SeqCst));
    let built = tauri::WebviewWindowBuilder::new(app, label, tauri::WebviewUrl::External("about:blank".parse().expect("about:blank")))
        .window_features(features)
        .title(format!("{owner} sign-in — tazuna"))
        .on_navigation(|u| u.scheme() == "https" || u.as_str() == "about:blank")
        .build();
    match built {
        Ok(window) => tauri::webview::NewWindowResponse::Create { window },
        Err(_) => tauri::webview::NewWindowResponse::Deny,
    }
}

/// "Sign in…" (Settings › Providers): the provider's console in a window of its own, learning (the first visit: its bar,
/// shapes only) or reading. One already open comes to the front.
#[tauri::command]
async fn console_open(app: AppHandle, provider: String, learn: bool) -> Result<(), String> {
    let c = console_of(&provider).ok_or_else(|| format!("no console reader for {provider}"))?;
    if let Some(w) = app.get_webview_window(&label_of(c.name)) {
        return w.set_focus().map_err(|e| e.to_string());
    }
    console_window(&app, c, if learn { "learn" } else { "read" }, c.start, true).map(|_| ())
}

async fn pause(ms: u64) {
    let _ = tauri::async_runtime::spawn_blocking(move || std::thread::sleep(std::time::Duration::from_millis(ms))).await;
}

/// "Forget sign-in": the window closed, its store deleted (its cookies, its sign-in) and its reading with it. The learn file
/// stays: shapes only.
#[tauri::command]
async fn console_forget(app: AppHandle, consoles: State<'_, Consoles>, provider: String) -> Result<(), String> {
    let c = console_of(&provider).ok_or_else(|| format!("no console reader for {provider}"))?;
    for label in [label_of(c.name), format!("{}-read", label_of(c.name))] {
        if let Some(w) = app.get_webview_window(&label) {
            let _ = w.destroy();
        }
    }
    // a background reading ends first (it is told nothing: its window is gone; at most its 30 s)
    for _ in 0..130 {
        if !consoles.hidden.load(Ordering::SeqCst) {
            break;
        }
        pause(250).await;
    }
    // the webview lets go of its store a moment after its window: a few tries
    let mut last = Err(String::new());
    for _ in 0..4 {
        pause(400).await;
        #[cfg(target_os = "macos")]
        {
            own_stores()?;
            last = app.remove_data_store(store_id(c.name)).await.map_err(|e| e.to_string());
        }
        #[cfg(not(target_os = "macos"))]
        {
            last = match std::fs::remove_dir_all(console_store(&app, c)?) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
                _ => Ok(()),
            };
        }
        if last.is_ok() {
            break;
        }
    }
    match std::fs::remove_file(share_dir().join("console").join(format!("{}.json", slug(c.name)))) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.to_string()),
        _ => {}
    }
    last
}

/// A reading in a hidden window (D4: the window asks at most every 15 minutes while a provider is tracked by its console):
/// the learned usage page, until a report or 30 s; no report in that time is "error" (a sign-in page says "signed_out" itself).
/// Only where the provider has an extractor and a usage page was learned, never beside an open console window or another
/// reading. Says what it did: "read", "signed_out", "error", or "skipped".
#[tauri::command]
async fn console_refresh(app: AppHandle, consoles: State<'_, Consoles>, provider: String) -> Result<String, String> {
    let c = console_of(&provider).ok_or_else(|| format!("no console reader for {provider}"))?;
    let page = read_json(&learn_file(&share_dir(), c.name)).and_then(|l| l["page"].as_str().map(str::to_string)).filter(|p| !p.contains(":id"));
    let Some(page) = page.filter(|_| c.read.is_some()) else { return Ok("skipped".into()) };
    if app.get_webview_window(&label_of(c.name)).is_some() || consoles.hidden.swap(true, Ordering::SeqCst) {
        return Ok("skipped".into());
    }
    let (tx, rx) = std::sync::mpsc::channel();
    *consoles.waiting.lock().unwrap() = Some((c.name.to_string(), tx));
    let got = match console_window(&app, c, "read", &page, false) {
        Ok(w) => {
            let got = tauri::async_runtime::spawn_blocking(move || rx.recv_timeout(std::time::Duration::from_secs(30)).ok()).await.ok().flatten();
            let _ = w.destroy();
            Ok(got)
        }
        Err(e) => Err(e),
    };
    *consoles.waiting.lock().unwrap() = None;
    consoles.hidden.store(false, Ordering::SeqCst);
    let got = got?;
    if got.is_none() {
        write_reading(&share_dir(), c.name, "error", serde_json::Value::Null)?;
    }
    Ok(got.unwrap_or("error").into())
}

/// What the first visit found, for Settings ("found usage data on /usage (3 fields)"), and whether the window's store (its
/// sign-in) is there, for Forget: {page, fields, at, store}, or null with neither.
#[tauri::command]
async fn console_learned(app: AppHandle, provider: String) -> Result<Option<serde_json::Value>, String> {
    let Some(c) = console_of(&provider) else { return Ok(None) };
    #[cfg(target_os = "macos")]
    let store = own_stores().is_ok() && app.fetch_data_store_identifiers().await.is_ok_and(|ids| ids.contains(&store_id(c.name)));
    #[cfg(not(target_os = "macos"))]
    let store = console_store(&app, c).is_ok_and(|d| d.exists());
    let learned = read_json(&learn_file(&share_dir(), c.name)).map(|l| learned_summary(&l));
    Ok(match learned {
        Some(mut l) => { l["store"] = serde_json::json!(store); Some(l) }
        None if store => Some(serde_json::json!({"page": null, "fields": 0, "at": null, "store": true})),
        None => None,
    })
}

/// A window gone: the main one takes the cores and the console windows with it (until the console reader, any window's end
/// stopped every core); a console window's end tells Settings, which shows what was learned.
#[inline(never)]
fn window_gone(w: &tauri::Window) {
    if w.label() == "main" {
        stop_all(&w.state::<Core>());
        for (label, cw) in w.app_handle().webview_windows() {
            if label.starts_with("console-") || label.starts_with("popup-") {
                let _ = cw.destroy();
            }
        }
    } else if let Some(c) = CONSOLES.iter().find(|c| w.label().strip_suffix("-read").unwrap_or(w.label()) == label_of(c.name)) {
        let _ = w.app_handle().emit_to("main", "console-closed", c.name);
    }
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
            &P::about(app, None, None)?, &MenuItem::with_id(app, "update", "Check for Updates…", true, None::<&str>)?, &sep()?,
            &item("settings", "Settings…", "CmdOrCtrl+,")?, &sep()?, &P::services(app, None)?, &sep()?,
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
        .register_uri_scheme_protocol("tzwidget", |ctx, req| {
            // the main window's alone (a panel's frame is inside it): a console page gets nothing
            if ctx.webview_label() != "main" {
                return tauri::http::Response::builder().status(403).body(Vec::new()).unwrap_or_default();
            }
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
        .register_uri_scheme_protocol("tzplugin", |ctx, req| {
            let (status, body) = if ctx.webview_label() == "main" { plugin_code(req.uri().path()) } else { (403, String::new()) };
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
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri::plugin::Builder::<tauri::Wry, ()>::new("console").invoke_handler(tauri::generate_handler![report]).build())
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
        .manage(Consoles::default())
        .invoke_handler(tauri::generate_handler![start, stop, send, op, trust, pick_folder, pick_files, image_data, paste_image, paste_clipboard_image, open_key_page, open_link, open_in, notify, ui_log, widgets_list, widgets_new, widgets_path, widgets_remove, plugins_enable, plugins_code, plugins_export, plugins_import, plugins_import_file, hotkey, update_check, update_install, interrupt, new_chat, set_profile, console_open, console_forget, console_refresh, console_learned])
        .on_window_event(|w, e| {
            if let tauri::WindowEvent::Destroyed = e {
                window_gone(w);
            }
        })
        .run(tauri::generate_context!())
        .expect("tazuna desktop failed to start");
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_window_can_request_completions() {
        assert!(super::is_window_op(&serde_json::json!({"op": "complete"})));
        assert!(!super::is_window_op(&serde_json::json!({"type": "complete", "op": "complete"})));
    }

    /// Every request the window's code sends (lib/tazuna.ts) passes: a missing one was refused without a word, and its
    /// feature looked stuck (the `@` menu until 0.1.112; a subagent's Watch, "Reading what it has done so far…" for good,
    /// and its Stop until 0.1.117).
    #[test]
    fn every_op_the_window_sends_passes() {
        let ts = include_str!("../../src/lib/tazuna.ts");
        let ops: Vec<&str> = ts.match_indices("msg: { op: \"").map(|(i, m)| &ts[i + m.len()..]).map(|r| &r[..r.find('"').unwrap()]).collect();
        assert!(ops.len() > 20, "{ops:?}");
        for op in ops {
            assert!(super::is_window_op(&serde_json::json!({"op": op})), "the window sends {op:?} and the bridge refuses it");
        }
    }

    /// "Open in" takes an existing folder given whole and an app of its own list, nothing else.
    #[test]
    fn open_in_takes_only_a_folder_and_an_app_it_knows() {
        // whatever this machine has (a Linux box without xdg-open lists no "files"), only from the list
        let apps = super::open_in(None, None).unwrap();
        assert!(apps.iter().all(|a| ["files", "terminal", "vscode", "cursor", "zed", "windsurf"].contains(&a.as_str())), "{apps:?}");
        let here = std::env::temp_dir().to_string_lossy().into_owned();
        assert!(super::open_in(Some(here.clone()), Some("sh".into())).is_err());
        assert!(super::open_in(Some(here), Some("vscode; rm -rf ~".into())).is_err());
        assert!(super::open_in(Some("relative/dir".into()), Some("files".into())).is_err());
        assert!(super::open_in(Some("/no/such/folder".into()), Some("files".into())).is_err());
        // a macOS package is a folder `open` launches, not a project
        #[cfg(target_os = "macos")]
        {
            let pkg = std::env::temp_dir().join("tz-open-in-test.app");
            std::fs::create_dir_all(&pkg).unwrap();
            let r = super::open_in(Some(pkg.to_string_lossy().into_owned()), Some("files".into()));
            let _ = std::fs::remove_dir(&pkg);
            assert!(r.is_err(), "{r:?}");
        }
        // the PATH it gives has no relative entry
        if let Some(p) = super::absolute_path_var() {
            assert!(std::env::split_paths(&p).all(|d| d.is_absolute()), "{p:?}");
        }
        // never a bare name, which the project's folder could shadow
        for a in &apps {
            let (program, _) = super::open_in_command(a).unwrap();
            assert!(std::path::Path::new(&program).is_absolute(), "{a}: {program}");
        }
    }

    #[test]
    fn the_window_can_retry_a_model_list_but_cannot_signal_completion() {
        assert!(super::is_window_op(&serde_json::json!({"op": "refresh_model_list", "provider": "Mortera"})));
        assert!(!super::is_window_op(&serde_json::json!({"type": "tazuna_list_finished", "op": "refresh_model_list"})));
        assert!(!super::is_window_op(&serde_json::json!({"op": "tazuna_list_finished"})));
    }

    #[test]
    #[cfg(unix)]
    fn a_core_that_does_not_quit_is_ended_after_three_seconds() {
        use std::process::{Command, Stdio};
        // reads nothing, quits on nothing: a core busy in a long model call
        let mut child = Command::new("sleep").arg("60").stdin(Stdio::piped()).spawn().unwrap();
        let stdin = child.stdin.take().unwrap();
        let t = std::time::Instant::now();
        super::quit_core(child, stdin);
        assert!(t.elapsed() < std::time::Duration::from_secs(5), "{:?}", t.elapsed());
    }

    #[test]
    fn pasted_image_bytes_can_arrive_as_raw_or_json() {
        use tauri::ipc::InvokeBody::{Json, Raw};

        let png_signature = [137, 80, 78, 71, 13, 10, 26, 10];
        for body in [Raw(png_signature.to_vec()), Json(serde_json::json!(png_signature))] {
            assert_eq!(super::paste_bytes(&body).unwrap().as_ref(), &png_signature);
        }
        assert_eq!(super::paste_bytes(&Json(serde_json::json!([256]))).unwrap_err(), "no image in the paste");
        let empty = Json(serde_json::json!([]));
        assert_eq!(super::save_pasted_image(&super::paste_bytes(&empty).unwrap(), "png").unwrap_err(), "an image up to 20 MB");
    }

    #[test]
    fn a_shared_plugin_is_imported_with_its_own_files_only_under_a_free_id() {
        let root = std::env::temp_dir().join(format!("tazuna-import-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("cost-bar")).unwrap();
        let bundle = |files: serde_json::Value| serde_json::json!({"tazuna_plugin": 1, "id": "cost-bar", "files": files}).to_string();
        let ok = bundle(serde_json::json!({"manifest.json": "{\"title\": \"Cost\"}", "plugin.js": "export default function () {}"}));
        assert_eq!(super::import_into(&root, &ok).unwrap(), "cost-bar-2", "a taken id gets a number");
        assert!(root.join("cost-bar-2/plugin.js").is_file() && root.join("cost-bar-2/.imported").is_file());
        assert!(super::import_into(&root, &bundle(serde_json::json!({"manifest.json": "{}", "plugin.js": "x", "../evil.sh": "x"}))).unwrap_err().contains("../evil.sh"));
        assert!(super::import_into(&root, &bundle(serde_json::json!({"plugin.js": "x"}))).unwrap_err().contains("manifest"));
        let odd = serde_json::json!({"tazuna_plugin": 1, "id": "../../x", "files": {"manifest.json": "{}", "index.html": "<p>"}}).to_string();
        assert_eq!(super::import_into(&root, &odd).unwrap(), "shared", "an id that is not a folder name is not used");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A console page's report: from its own window, on its provider's sites, a shape or numbers and nothing else.
    #[test]
    fn a_console_report_is_checked() {
        use super::{check_report, Report};
        use serde_json::json;
        let (q, label, url) = ("Qwen Cloud Token Plan", "console-Qwen-Cloud-Token-Plan", "https://home.qwencloud.com/usage");
        let shape = json!({"data": {"remaining": "number", "totalQuota": "number", "plan": "string", "items": [{"name": "string"}], "*": "boolean"}, "ok": "boolean"});
        let learn = json!({"path": "/api/usage/:id?region&lang", "method": "GET", "status": 200, "shape": shape, "page": "https://home.qwencloud.com/usage"});
        let Ok(Report::Learn(e)) = check_report(label, url, q, "learn", &learn) else { panic!("{:?}", check_report(label, url, q, "learn", &learn)) };
        assert_eq!((&e["usage"], &e["page"], &e["shape"]), (&json!(true), &json!("https://home.qwencloud.com/usage"), &shape));
        // exactly what console.js reports for review/console.cjs's response full of values (SHOW=1 prints it)
        let from_js: serde_json::Value = serde_json::from_str(r#"{"path":"/api/usage?region","method":"GET","status":200,"shape":{"user":{"email":"string","id":"number","token":"string"},"quota":{"remaining":"number","total":"number","resetAt":"string","plan":"string"},"items":[{"name":"string","used":"number"}],"*":{"x":"number"},"flags":[],"nothing":"null"},"page":"https://home.qwencloud.com/usage/:id"}"#).unwrap();
        assert!(matches!(check_report(label, url, q, "learn", &from_js), Ok(Report::Learn(_))), "{:?}", check_report(label, url, q, "learn", &from_js));
        // the label: another provider's window, the main window, a provider without a console
        assert!(super::caller_ok(label, q) && !super::caller_ok("console-Caffeine", q) && !super::caller_ok("main", q) && !super::caller_ok("console-OpenRouter", "OpenRouter"));
        assert!(super::caller_ok("console-Qwen-Cloud-Token-Plan-read", q) && !super::caller_ok("popup-Qwen-Cloud-Token-Plan-0", q));
        for l in ["console-Caffeine", "main", "console-Qwen-Cloud-Token-Plan-2", "popup-Qwen-Cloud-Token-Plan-0"] {
            assert!(check_report(l, url, q, "learn", &learn).is_err(), "{l}");
        }
        // the site: another one, plain http, a look-alike host
        for u in ["https://evil.example/usage", "http://home.qwencloud.com/usage", "https://qwencloud.com.evil.example/", "https://evilqwencloud.com/", "tauri://localhost/"] {
            assert!(check_report(label, u, q, "learn", &learn).is_err(), "{u}");
        }
        assert!(super::on_site(&["chat.mistral.ai"], "https://chat.mistral.ai/code/usage") && super::on_site(&["caffeine.ai"], "https://caffeine.ai/"));
        assert!(!super::on_site(&["caffeine.ai"], "https://evil.caffeine.ai/") && !super::on_site(&["caffeine.ai"], "https://caffeine.ai:8443/"), "exact hosts only");
        // values the security review found kept: ids, keys, names in field names and paths; real field names stay
        let uuid_l = "e29b41d4-a716-4466-8554-446655440000";
        for k in [uuid_l, "acme-prod", "john.smith", "key_2aBcD9eFgH1jK", "cus_NffrFeUfNV2Hib", "AbCdEfGhIjKl", "user9f8e7d6c", "a@b"] {
            assert!(!super::key_ok(k), "{k}");
        }
        for k in ["used_tokens", "totalQuota", "remaining", "plan_limit", "resetAt", "v1", "creditsRemaining", "*"] {
            assert!(super::key_ok(k), "{k}");
        }
        for p in ["/v1/customers/cus_NffrFeUfNV2Hib/keys/x", "/keys/sk-AbCdEfGhIjKlMnOp", &format!("/u/{uuid_l}"), "/users/john.smith/usage"] {
            assert!(!super::path_ok(p), "{p}");
        }
        assert!(super::path_ok("/v1/customers/:id/keys/:id/:id/api.json") && super::path_ok("/api-keys/token-plan"));
        // a value anywhere in the shape, or a key that may be one
        for bad in [json!({"a": 12}), json!({"a": "sk-secret"}), json!({"a": true}), json!({"a": null}), json!({"a": ["string", "number"]}),
            json!({"someone@example.com": "string"}), json!({"1234567": "string"}), json!({"user_123456": "string"}), json!({"a b": "string"}), json!("string")] {
            let r = check_report(label, url, q, "learn", &json!({"path": "/api", "method": "GET", "status": 200, "shape": bad}));
            assert!(r.is_err(), "{bad}");
        }
        // a path with a query value or an id; an odd method or status; a field it does not know
        for (k, v) in [("path", json!("/api/usage?region=sg")), ("path", json!("/api/users/1234567/usage")), ("path", json!("api/usage")), ("path", json!("/a/b@c")),
            ("method", json!("get; rm")), ("status", json!(42)), ("cookie", json!("string"))] {
            let mut p = learn.clone();
            p[k] = v;
            assert!(check_report(label, url, q, "learn", &p).is_err(), "{k}");
        }
        // a page: only on its sites, without a query, and only where usage-like fields came
        for page in ["https://evil.example/usage", "https://home.qwencloud.com/usage?token=abc", "https://home.qwencloud.com/u/12345678"] {
            let mut p = learn.clone();
            p["page"] = json!(page);
            assert!(check_report(label, url, q, "learn", &p).is_err(), "{page}");
        }
        assert!(check_report(label, url, q, "learn", &json!({"path": "/api/me", "method": "GET", "status": 200, "shape": {"name": "string"}, "page": url})).is_err());
        // 64 KB at most
        let big: serde_json::Map<String, serde_json::Value> = (0..100).map(|i| (format!("k{i}_{}", "x".repeat(50)), json!([{"a": "string"}]))).collect();
        let deep = (0..15).fold(json!("string"), |s, _| json!({"a": s}));
        assert!(check_report(label, url, q, "learn", &json!({"path": "/", "method": "GET", "status": 200, "shape": deep})).is_err(), "too deep");
        let huge = json!({"path": "/", "method": "GET", "status": 200, "shape": (0..20).map(|i| (format!("g{i}"), json!(big))).collect::<serde_json::Map<_, _>>()});
        assert!(huge.to_string().len() > 64 * 1024 && check_report(label, url, q, "learn", &huge).unwrap_err().contains("64 KB"));
        // a reading: finite numbers or null in its five fields, one unit of four, at least one number
        let read = json!({"numbers": {"left": 1500.25, "total": 2000, "unit": "credits", "used_pct": 25, "resets": 1764500000}});
        // no reading while the provider has no extractor (Phase 2 writes them)
        assert!(check_report(label, url, q, "read", &read).unwrap_err().contains("no reader"));
        let check_report = |_: &str, _: &str, _: &str, _: &str, p: &serde_json::Value| super::check_numbers(p);
        assert_eq!(check_report(label, url, q, "read", &read), Ok(Report::Read(read["numbers"].clone())));
        assert_eq!(check_report(label, url, q, "read", &json!({"numbers": {"used_pct": 30, "unit": "percent"}})),
            Ok(Report::Read(json!({"left": null, "total": null, "unit": "percent", "used_pct": 30, "resets": null}))));
        for bad in [json!({"numbers": {"left": "1500", "unit": "usd"}}), json!({"numbers": {"left": 1, "unit": "EUR"}}), json!({"numbers": {"left": 1}}),
            json!({"numbers": {"unit": "usd"}}), json!({"numbers": {"left": 1, "unit": "usd", "email": "a@b"}}), json!({"numbers": {"used_pct": 1400, "unit": "percent"}}), json!({"numbers": {"total": 0, "unit": "usd"}}), json!({"numbers": {"left": 1e15, "unit": "usd"}}),
            json!({"numbers": {"left": 1, "unit": "usd", "resets": 1764500000000u64}}), json!({"numbers": {"left": 1, "unit": "usd", "resets": 3600}}),
            json!({"numbers": {"left": 1, "unit": "usd"}, "html": "x"}), json!([1, 2])] {
            assert!(check_report(label, url, q, "read", &bad).is_err(), "{bad}");
        }
        let check_report = super::check_report;
        assert_eq!(check_report(label, url, q, "signed_out", &json!(null)), Ok(Report::SignedOut));
        assert_eq!(check_report(label, url, q, "done", &json!(null)), Ok(Report::Done));
        assert!(check_report(label, url, q, "signed_out", &json!({"x": 1})).is_err() && check_report(label, url, q, "write", &json!(null)).is_err());
    }

    /// The learn file keeps one entry per call, the usage page, and its size; Settings reads what it found from it; a reading
    /// is written whole where the core reads it.
    #[test]
    fn a_learn_file_keeps_shapes_and_says_what_it_found() {
        use serde_json::json;
        let usage = json!({"path": "/api/usage", "method": "GET", "status": 200, "usage": true, "shape": {"data": {"remain": "number", "total": "number", "usedPct": "number", "name": "string"}}, "page": "https://home.qwencloud.com/usage"});
        let me = json!({"path": "/api/me", "method": "GET", "status": 200, "usage": false, "shape": {"name": "string"}});
        let l = super::merge_learn(json!(null), usage.clone(), 10);
        let l = super::merge_learn(l, me.clone(), 11);
        let l = super::merge_learn(l, me, 11);
        // a page with fewer usage fields later (a chat page naming a plan) keeps /usage
        let l = super::merge_learn(l, json!({"path": "/api/chat", "method": "GET", "status": 200, "usage": true, "shape": {"planName": "string"}, "page": "https://home.qwencloud.com/chat/:id"}), 12);
        assert_eq!((l["responses"].as_array().unwrap().len(), &l["page"], &l["at"]), (3, &json!("https://home.qwencloud.com/usage"), &json!(12)));
        assert_eq!(super::learned_summary(&l), json!({"page": "/usage", "fields": 4, "at": 12}));
        assert_eq!(super::learned_summary(&super::merge_learn(json!(null), json!({"path": "/x", "method": "GET", "usage": false, "shape": {}}), 5)), json!({"page": null, "fields": 0, "at": 5}));
        let many = (0..250).fold(json!(null), |l, i| super::merge_learn(l, json!({"path": format!("/p{i}"), "method": "GET", "shape": {}}), i));
        assert_eq!((many["responses"].as_array().unwrap().len(), &many["responses"][0]["path"]), (200, &json!("/p50")));
        let root = std::env::temp_dir().join(format!("tazuna-console-{}", std::process::id()));
        super::write_reading(&root, "Qwen Cloud Token Plan", "signed_out", json!(null)).unwrap();
        let r = super::read_json(&root.join("console/Qwen-Cloud-Token-Plan.json")).unwrap();
        assert_eq!((&r["status"], &r["numbers"], &r["source"]), (&json!("signed_out"), &json!(null), &json!("console")));
        assert!(super::learn_file(&root, "Mistral Vibe").ends_with("console-learn/Mistral-Vibe.json"));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The console windows reach one command, from their providers' sites alone, and the main window's capability is its own.
    #[test]
    fn the_console_capability_reaches_only_the_consoles() {
        use serde_json::json;
        let cap: serde_json::Value = serde_json::from_str(include_str!("../capabilities/console.json")).unwrap();
        let want: Vec<String> = super::CONSOLES.iter().flat_map(|c| c.sites.iter().map(|s| format!("https://{s}"))).collect();
        assert_eq!((&cap["windows"], &cap["local"], &cap["permissions"], &cap["remote"]["urls"]), (&json!(["console-*"]), &json!(false), &json!(["console:allow-report"]), &json!(want)));
        let main: serde_json::Value = serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
        assert_eq!(main["windows"], json!(["main"]));
        for c in &super::CONSOLES {
            assert!(super::on_site(c.sites, c.start), "{}", c.name);
            assert!(super::label_of(c.name).chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-'), "a window label: {}", c.name);
        }
        // the script ends in the function main.rs calls
        assert!(super::CONSOLE_JS.trim_end().ends_with("})") && super::CONSOLE_JS.contains("plugin:console|report"));
    }

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
