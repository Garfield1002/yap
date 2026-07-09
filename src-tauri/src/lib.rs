mod commands;
mod config;
mod menu;
mod plugins;
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
        // No native menus at all: the frontend draws its own title bar and
        // dropdown menus. New Window is the one process-level action left.
        .invoke_handler(tauri::generate_handler![
            commands::get_initial_file,
            commands::get_start_untitled,
            commands::read_file,
            commands::write_file_atomic,
            commands::delete_file,
            commands::rename_file,
            commands::start_watch,
            commands::save_pasted_image,
            config::get_config,
            config::set_theme,
            config::record_recent,
            config::set_plugins_enabled,
            menu::new_window,
            plugins::list_plugins,
            plugins::read_plugin_data,
            plugins::write_plugin_data,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
