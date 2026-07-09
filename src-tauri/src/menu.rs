//! The window menus: File, Edit, Settings.
//!
//! There is no persistent menu bar. The frontend draws its own themed title bar
//! with File / Edit / Settings buttons and calls `popup_menu` to drop the native
//! submenu under the button that was clicked. Each popup is rebuilt from the
//! current config, so Open Recent, the path-only items' enabled state, and the
//! theme tick are always current.
//!
//! Most items forward their id to the frontend as a `menu-action` event -- the
//! frontend owns the editor, the path, and the dialogs. The exceptions:
//! `new_window` spawns a second process, and Cut/Copy/Paste are Tauri predefined
//! items (CodeMirror fills the clipboard with document source on the native copy
//! event, so a hidden-markup selection still copies correct markdown).

use std::path::Path;

use serde::Serialize;
use tauri::menu::{CheckMenuItemBuilder, ContextMenu, MenuItemBuilder, Submenu, SubmenuBuilder};
use tauri::{AppHandle, Emitter, Runtime};

use crate::config;

/// Event carrying a menu selection to the frontend.
#[derive(Clone, Serialize)]
struct MenuAction {
    /// The item id, e.g. `"save"`, or `"open-path"` for a recent file.
    name: String,
    /// The file path, only for `"open-path"`.
    path: Option<String>,
}

fn basename(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_owned())
}

fn file_submenu<R: Runtime>(app: &AppHandle<R>, has_path: bool) -> tauri::Result<Submenu<R>> {
    let cfg = config::load();

    // Open Recent, filtered to files that still exist.
    let mut recent = SubmenuBuilder::new(app, "Open Recent");
    let live: Vec<&String> = cfg.recent.iter().filter(|p| Path::new(p).exists()).collect();
    if live.is_empty() {
        recent = recent
            .item(&MenuItemBuilder::with_id("recent_none", "No recent files").enabled(false).build(app)?);
    } else {
        for path in live {
            recent = recent
                .item(&MenuItemBuilder::with_id(format!("recent:{path}"), basename(path)).build(app)?);
        }
    }
    let recent = recent.build()?;

    SubmenuBuilder::new(app, "File")
        .item(&MenuItemBuilder::with_id("new", "New").accelerator("CmdOrCtrl+N").build(app)?)
        .item(
            &MenuItemBuilder::with_id("new_window", "New Window")
                .accelerator("CmdOrCtrl+Shift+N")
                .build(app)?,
        )
        .item(&MenuItemBuilder::with_id("open", "Open…").accelerator("CmdOrCtrl+O").build(app)?)
        .item(&recent)
        .separator()
        .item(&MenuItemBuilder::with_id("save", "Save").accelerator("CmdOrCtrl+S").build(app)?)
        .item(&MenuItemBuilder::with_id("rename", "Rename…").enabled(has_path).build(app)?)
        .item(&MenuItemBuilder::with_id("delete", "Delete").enabled(has_path).build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("copy_path", "Copy Path").enabled(has_path).build(app)?)
        .item(
            &MenuItemBuilder::with_id("open_location", "Open File Location")
                .enabled(has_path)
                .build(app)?,
        )
        .separator()
        .item(&MenuItemBuilder::with_id("quit", "Quit").accelerator("CmdOrCtrl+Q").build(app)?)
        .build()
}

fn edit_submenu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Submenu<R>> {
    SubmenuBuilder::new(app, "Edit")
        // Undo/Redo route to the editor's own history, not the webview's.
        .item(&MenuItemBuilder::with_id("undo", "Undo").build(app)?)
        .item(&MenuItemBuilder::with_id("redo", "Redo").build(app)?)
        .separator()
        .cut()
        .copy()
        .item(&MenuItemBuilder::with_id("copy_html", "Copy HTML").build(app)?)
        .paste()
        .build()
}

fn settings_submenu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Submenu<R>> {
    let cfg = config::load();
    SubmenuBuilder::new(app, "Settings")
        .item(
            &CheckMenuItemBuilder::with_id("theme_light", "Light Theme")
                .checked(cfg.theme.as_deref() == Some("light"))
                .build(app)?,
        )
        .item(
            &CheckMenuItemBuilder::with_id("theme_dark", "Dark Theme")
                .checked(cfg.theme.as_deref() == Some("dark"))
                .build(app)?,
        )
        .build()
}

/// Pop up one of the menus as a context menu, under the button that asked for
/// it. `has_path` gates the File items that only apply to a saved file.
#[tauri::command]
pub fn popup_menu<R: Runtime>(
    app: AppHandle<R>,
    window: tauri::Window<R>,
    which: String,
    has_path: bool,
) -> Result<(), String> {
    let submenu = match which.as_str() {
        "file" => file_submenu(&app, has_path),
        "edit" => edit_submenu(&app),
        "settings" => settings_submenu(&app),
        other => return Err(format!("unknown menu: {other}")),
    }
    .map_err(|e| e.to_string())?;
    submenu.popup(window).map_err(|e| e.to_string())
}

fn spawn_new_window() {
    if let Ok(exe) = std::env::current_exe() {
        // `--new` tells the fresh process to open an untitled buffer rather
        // than the file picker it would show on a bare launch.
        let _ = std::process::Command::new(exe).arg("--new").spawn();
    }
}

/// Dispatch a menu selection. Handled here when it is process-level; forwarded
/// to the frontend otherwise.
pub fn on_menu_event<R: Runtime>(app: &AppHandle<R>, id: &str) {
    if id == "new_window" {
        spawn_new_window();
        return;
    }
    if let Some(path) = id.strip_prefix("recent:") {
        let _ = app.emit(
            "menu-action",
            MenuAction { name: "open-path".into(), path: Some(path.to_owned()) },
        );
        return;
    }
    if id == "recent_none" {
        return; // inert placeholder
    }
    let _ = app.emit("menu-action", MenuAction { name: id.to_owned(), path: None });
}
