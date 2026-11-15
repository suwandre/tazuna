// tazuna desktop: the window. The conversation itself will come from tazuna's own core (an app-server mode of the
// `tazuna` binary, the same events the terminal UI renders); this shell shows the design with demo data for now.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("tazuna desktop failed to start");
}
