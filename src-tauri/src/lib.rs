mod commands;
mod watcher;

use std::path::PathBuf;
use std::sync::Mutex;

use commands::AppState;

/// One window, one file, one process. No single-instance plugin: `yap a.md` and
/// `yap b.md` are two independent editors, which is what a window manager and a
/// `%F` desktop entry both expect.
pub fn run(initial_path: Option<PathBuf>) {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            initial_path: Mutex::new(initial_path),
            watcher: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_initial_file,
            commands::read_file,
            commands::write_file_atomic,
            commands::start_watch,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
