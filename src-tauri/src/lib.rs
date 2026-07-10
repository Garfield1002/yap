mod commands;
mod config;
mod marp;
mod menu;
mod plugins;
mod spell;
mod watcher;

use std::path::PathBuf;
use std::sync::Mutex;

use commands::AppState;

#[cfg(target_os = "macos")]
use tauri::Manager;

#[cfg(target_os = "macos")]
fn handle_run_event(app_handle: &tauri::AppHandle, event: tauri::RunEvent) {
    if let tauri::RunEvent::Opened { urls } = event {
        let state = app_handle.state::<AppState>();
        for path in urls.into_iter().filter_map(|url| url.to_file_path().ok()) {
            if let Err(path) = commands::set_unclaimed_initial_file(&state, path) {
                // Finder sends later document opens back to the running app.
                // A fresh process keeps each document isolated in its own window.
                if let Ok(executable) = std::env::current_exe() {
                    let _ = std::process::Command::new(executable).arg(path).spawn();
                }
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn handle_run_event(_app_handle: &tauri::AppHandle, _event: tauri::RunEvent) {}

/// One window, one file, one process. No single-instance plugin: `bulletmd a.md`
/// and `bulletmd b.md` are two independent editors, which is what a window manager and a
/// `%f` desktop entry both expect.
pub fn run(initial_path: Option<PathBuf>, start_untitled: bool) {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            initial_path: Mutex::new(initial_path),
            initial_file_claimed: Mutex::new(false),
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
            config::set_dot_opacity,
            config::record_recent,
            config::set_plugins_enabled,
            menu::new_window,
            marp::marp_export,
            plugins::list_plugins,
            plugins::install_plugin,
            plugins::read_plugin_data,
            plugins::write_plugin_data,
            spell::spell_languages,
            spell::spell_check,
            spell::spell_suggest,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(handle_run_event);
}
