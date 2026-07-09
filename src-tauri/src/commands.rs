use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

use notify::RecommendedWatcher;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tempfile::NamedTempFile;

use crate::watcher::watch_file;

/// Emitted when the file changes on disk. The frontend decides what to do:
/// ignore its own save echo, reload silently, or raise a conflict.
pub const FILE_CHANGED: &str = "file-changed";

#[derive(Default)]
pub struct AppState {
    /// The file named on the command line, if any. One window, one file.
    pub initial_path: Mutex<Option<PathBuf>>,
    /// Holding the watcher keeps the watch alive; dropping it ends the thread.
    pub watcher: Mutex<Option<RecommendedWatcher>>,
}

#[derive(Serialize)]
pub struct FileContents {
    text: String,
    mtime_ms: f64,
}

fn mtime_ms(path: &Path) -> f64 {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0)
}

#[tauri::command]
pub fn get_initial_file(state: State<AppState>) -> Option<String> {
    state
        .initial_path
        .lock()
        .ok()?
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned())
}

/// Reads the file. A path that does not exist yet is not an error: `yap new.md`
/// should open an empty buffer that saves into place.
#[tauri::command]
pub fn read_file(path: String) -> Result<FileContents, String> {
    let path = PathBuf::from(path);
    if !path.exists() {
        return Ok(FileContents {
            text: String::new(),
            mtime_ms: 0.0,
        });
    }
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mtime_ms = mtime_ms(&path);
    Ok(FileContents { text, mtime_ms })
}

/// Writes via a temp file in the *target's own directory*, then renames over
/// the target. Same directory is not a style choice: `rename(2)` is only atomic
/// within a single filesystem, and `/tmp` is usually a different one.
///
/// Returns the new mtime so the caller can keep its bookkeeping current.
#[tauri::command]
pub fn write_file_atomic(path: String, contents: String) -> Result<f64, String> {
    let target = PathBuf::from(path);
    let dir = target
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .ok_or_else(|| format!("{} has no parent directory", target.display()))?;

    let mut tmp = NamedTempFile::new_in(dir).map_err(|e| format!("temp file: {e}"))?;
    tmp.write_all(contents.as_bytes())
        .map_err(|e| format!("write: {e}"))?;
    // Get the bytes to disk before the rename, or a crash can leave the target
    // pointing at an inode whose contents never landed.
    tmp.as_file().sync_all().map_err(|e| format!("fsync: {e}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // NamedTempFile is 0600. Keep the target's mode, or use 0644 for a new
        // file, so saving never silently tightens permissions.
        let mode = fs::metadata(&target)
            .map(|m| m.permissions().mode() & 0o777)
            .unwrap_or(0o644);
        fs::set_permissions(tmp.path(), fs::Permissions::from_mode(mode))
            .map_err(|e| format!("chmod: {e}"))?;
    }

    tmp.persist(&target)
        .map_err(|e| format!("rename onto {}: {e}", target.display()))?;

    Ok(mtime_ms(&target))
}

#[tauri::command]
pub fn start_watch(app: AppHandle, state: State<AppState>, path: String) -> Result<(), String> {
    let mut slot = state.watcher.lock().map_err(|e| e.to_string())?;
    let handle = app.clone();
    // Replacing the old watcher drops it, which stops the previous watch.
    *slot = Some(watch_file(Path::new(&path), move || {
        let _ = handle.emit(FILE_CHANGED, ());
    })?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("note.md");
        let path = target.to_string_lossy().into_owned();

        write_file_atomic(path.clone(), "hello\n".into()).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "hello\n");

        write_file_atomic(path, "goodbye\n".into()).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "goodbye\n");
    }

    #[test]
    fn atomic_write_creates_a_new_file_with_sane_permissions() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("fresh.md");

        write_file_atomic(target.to_string_lossy().into_owned(), "x".into()).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&target).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o644, "a new file must not inherit the temp file's 0600");
        }
    }

    #[test]
    fn atomic_write_preserves_an_existing_files_mode() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("locked.md");
        fs::write(&target, "before").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();

            write_file_atomic(target.to_string_lossy().into_owned(), "after".into()).unwrap();

            let mode = fs::metadata(&target).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "saving must not widen permissions");
        }
    }

    #[test]
    fn atomic_write_leaves_no_temp_files_behind() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("note.md");
        write_file_atomic(target.to_string_lossy().into_owned(), "x".into()).unwrap();

        let entries: Vec<_> = fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(entries.len(), 1, "only the target should remain: {entries:?}");
    }

    #[test]
    fn reading_a_missing_file_yields_an_empty_buffer() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("does-not-exist.md");
        let contents = read_file(target.to_string_lossy().into_owned()).unwrap();
        assert_eq!(contents.text, "");
    }

    #[test]
    fn read_after_write_sees_the_written_text() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("note.md");
        let path = target.to_string_lossy().into_owned();

        write_file_atomic(path.clone(), "round trip".into()).unwrap();
        let contents = read_file(path).unwrap();
        assert_eq!(contents.text, "round trip");
        assert!(contents.mtime_ms > 0.0);
    }
}
