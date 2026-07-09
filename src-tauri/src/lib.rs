mod commands;
mod config;
mod menu;
mod watcher;

use std::path::PathBuf;
use std::sync::Mutex;

use commands::AppState;

/// One window, one file, one process. No single-instance plugin: `yap a.md` and
/// `yap b.md` are two independent editors, which is what a window manager and a
/// `%F` desktop entry both expect.
pub fn run(initial_path: Option<PathBuf>, start_untitled: bool) {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            initial_path: Mutex::new(initial_path),
            start_untitled: Mutex::new(start_untitled),
            watcher: Mutex::new(None),
        })
        .menu(|app| menu::build_menu(app, false))
        .on_menu_event(|app, event| menu::on_menu_event(app, event.id().as_ref()))
        .invoke_handler(tauri::generate_handler![
            commands::get_initial_file,
            commands::get_start_untitled,
            commands::read_file,
            commands::write_file_atomic,
            commands::delete_file,
            commands::rename_file,
            commands::start_watch,
            config::get_config,
            config::set_theme,
            config::record_recent,
            menu::refresh_menu,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
