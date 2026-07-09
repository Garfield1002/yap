//! Window-spawning, the one menu action that is process-level.
//!
//! yap no longer has any native menus: the frontend draws its own themed title
//! bar and dropdown menus (see `src/lib/ui/`), routing every action through the
//! webview. The lone exception is New Window, which starts a second process --
//! that cannot be done from JS, so it stays a Tauri command.

/// Spawn a second editor process showing an untitled buffer. `--new` tells the
/// fresh process to open a blank buffer rather than the file picker it would
/// show on a bare launch.
#[tauri::command]
pub fn new_window() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    std::process::Command::new(exe)
        .arg("--new")
        .spawn()
        .map_err(|e| format!("spawn: {e}"))?;
    Ok(())
}
