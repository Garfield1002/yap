//! The native window menu: File, Edit, Settings.
//!
//! Most items just forward their id to the frontend as a `menu-action` event --
//! the frontend owns the editor view, the current path, and the dialogs, so it
//! is the natural place to act. The exceptions are handled here: `new_window`
//! spawns a second process, and the clipboard's Cut/Copy/Paste are Tauri's
//! predefined items (CodeMirror fills the clipboard with document source on the
//! native copy event, so these do the right thing without a round trip).
//!
//! The menu is rebuilt from scratch whenever the open document changes, which
//! is how the Open Recent submenu, and the enabled state of the path-only items,
//! stay current.

use std::path::Path;

use serde::Serialize;
use tauri::menu::{CheckMenuItemBuilder, Menu, MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::{AppHandle, Emitter, Manager, Runtime};

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

/// Build the whole menu. `has_path` gates the items that only make sense once
/// the buffer is backed by a file (Rename, Delete, Copy Path, Open Location).
pub fn build_menu<R: Runtime>(app: &AppHandle<R>, has_path: bool) -> tauri::Result<Menu<R>> {
    let cfg = config::load();

    // Open Recent, filtered to files that still exist.
    let mut recent = SubmenuBuilder::new(app, "Open Recent");
    let live: Vec<&String> = cfg.recent.iter().filter(|p| Path::new(p).exists()).collect();
    if live.is_empty() {
        recent = recent.item(
            &MenuItemBuilder::with_id("recent_none", "No recent files")
                .enabled(false)
                .build(app)?,
        );
    } else {
        for path in live {
            recent = recent.item(
                &MenuItemBuilder::with_id(format!("recent:{path}"), basename(path)).build(app)?,
            );
        }
    }
    let recent = recent.build()?;

    let file = SubmenuBuilder::new(app, "File")
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
        .build()?;

    let edit = SubmenuBuilder::new(app, "Edit")
        // Undo/Redo route to the editor's own history, not the webview's.
        .item(&MenuItemBuilder::with_id("undo", "Undo").build(app)?)
        .item(&MenuItemBuilder::with_id("redo", "Redo").build(app)?)
        .separator()
        .cut()
        .copy()
        .item(&MenuItemBuilder::with_id("copy_html", "Copy HTML").build(app)?)
        .paste()
        .build()?;

    // The ticked item reflects the current override; neither is ticked when
    // the theme follows the system.
    let settings = SubmenuBuilder::new(app, "Settings")
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
        .build()?;

    MenuBuilder::new(app).items(&[&file, &edit, &settings]).build()
}

/// Apply `menu` to every open window (menus are per-window on Linux).
fn apply<R: Runtime>(app: &AppHandle<R>, menu: Menu<R>) -> tauri::Result<()> {
    for (_, window) in app.webview_windows() {
        window.set_menu(menu.clone())?;
    }
    Ok(())
}

/// Rebuild and re-apply the menu. Called by the frontend after the open
/// document, the recent list, or the theme changes.
#[tauri::command]
pub fn refresh_menu<R: Runtime>(app: AppHandle<R>, has_path: bool) -> Result<(), String> {
    let menu = build_menu(&app, has_path).map_err(|e| e.to_string())?;
    apply(&app, menu).map_err(|e| e.to_string())
}

fn spawn_new_window<R: Runtime>(_app: &AppHandle<R>) {
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
        spawn_new_window(app);
        return;
    }
    if let Some(path) = id.strip_prefix("recent:") {
        let _ = app.emit(
            "menu-action",
            MenuAction { name: "open-path".into(), path: Some(path.to_owned()) },
        );
        return;
    }
    // recent_none and any unknown id are inert.
    if id == "recent_none" {
        return;
    }
    let _ = app.emit("menu-action", MenuAction { name: id.to_owned(), path: None });
}
