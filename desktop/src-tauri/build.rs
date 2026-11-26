fn main() {
    // the console reader's one command (main.rs, `report`) is an inline plugin's: its permission, console:allow-report, is
    // granted to the console windows alone (capabilities/console.json); an app command would have put every command of
    // the main window under the ACL too (main.rs main_only checks every app command's caller instead)
    let console = tauri_build::InlinedPlugin::new().commands(&["report"]);
    tauri_build::try_build(tauri_build::Attributes::new().plugin("console", console)).expect("tauri-build failed");
}
