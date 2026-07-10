// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

fn main() {
    let arg = std::env::args_os().nth(1);
    // `--new` (spawned by File > New Window) means: no file, open untitled.
    let start_untitled = arg.as_deref().map(|a| a == "--new").unwrap_or(false);
    let initial_path = if start_untitled {
        None
    } else {
        arg.map(|a| resolve(PathBuf::from(a)))
    };
    bulletmd_lib::run(initial_path, start_untitled)
}

/// Resolve the CLI argument against the working directory *before* Tauri starts,
/// because the webview process does not inherit a meaningful cwd.
///
/// `canonicalize` fails on a path that does not exist yet, and `bulletmd new.md`
/// must still work, so fall back to the merely-absolute form.
fn resolve(path: PathBuf) -> PathBuf {
    let absolute = if path.is_absolute() {
        path
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    absolute.canonicalize().unwrap_or(absolute)
}
