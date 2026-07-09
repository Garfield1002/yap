use std::ffi::OsString;
use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

/// Quiet period after the last event before we call back. Editors write a file
/// in several syscalls; without this we would reload halfway through.
const DEBOUNCE: Duration = Duration::from_millis(200);

/// Watch a single file for changes, calling `on_change` once per burst.
///
/// The watch is on the *parent directory*, not the file. A file-level watch
/// follows the inode, so it goes deaf the moment another editor saves by
/// writing a temp file and renaming it over the target -- which is exactly what
/// this program does, and what most editors do.
///
/// The returned watcher owns the watch: drop it and the background thread ends.
pub fn watch_file<F>(path: &Path, on_change: F) -> Result<RecommendedWatcher, String>
where
    F: Fn() + Send + 'static,
{
    let dir = path
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .ok_or_else(|| format!("{} has no parent directory", path.display()))?
        .to_path_buf();
    let name: OsString = path
        .file_name()
        .ok_or_else(|| format!("{} has no file name", path.display()))?
        .to_owned();

    let (tx, rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |res| {
        let _ = tx.send(res);
    })
    .map_err(|e| e.to_string())?;
    watcher
        .watch(&dir, RecursiveMode::NonRecursive)
        .map_err(|e| e.to_string())?;

    thread::spawn(move || {
        let mut pending = false;
        loop {
            // Block until something happens, then coalesce the burst.
            let received = if pending {
                match rx.recv_timeout(DEBOUNCE) {
                    Ok(res) => Some(res),
                    Err(RecvTimeoutError::Timeout) => {
                        pending = false;
                        on_change();
                        continue;
                    }
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            } else {
                match rx.recv() {
                    Ok(res) => Some(res),
                    Err(_) => break,
                }
            };

            if let Some(Ok(event)) = received {
                if event
                    .paths
                    .iter()
                    .any(|p| p.file_name() == Some(name.as_os_str()))
                {
                    pending = true;
                }
            }
        }
    });

    Ok(watcher)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::sync::mpsc::channel;

    /// Wait for one `on_change` callback, or give up.
    fn expect_change(rx: &mpsc::Receiver<()>) -> bool {
        rx.recv_timeout(Duration::from_secs(5)).is_ok()
    }

    #[test]
    fn fires_when_the_file_is_modified_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("note.md");
        fs::write(&target, "one").unwrap();

        let (tx, rx) = channel();
        let _watcher = watch_file(&target, move || {
            let _ = tx.send(());
        })
        .unwrap();

        fs::write(&target, "two").unwrap();
        assert!(expect_change(&rx), "in-place modification should notify");
    }

    #[test]
    fn fires_when_another_editor_renames_a_new_file_over_the_target() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("note.md");
        fs::write(&target, "one").unwrap();

        let (tx, rx) = channel();
        let _watcher = watch_file(&target, move || {
            let _ = tx.send(());
        })
        .unwrap();

        // The atomic-save dance: new inode, renamed over the old one. A
        // file-level watch would miss every change after this point.
        let mut tmp = tempfile::NamedTempFile::new_in(dir.path()).unwrap();
        tmp.write_all(b"two").unwrap();
        tmp.persist(&target).unwrap();

        assert!(expect_change(&rx), "rename-over should notify");
    }

    #[test]
    fn ignores_changes_to_other_files_in_the_directory() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("note.md");
        fs::write(&target, "one").unwrap();

        let (tx, rx) = channel();
        let _watcher = watch_file(&target, move || {
            let _ = tx.send(());
        })
        .unwrap();

        fs::write(dir.path().join("other.md"), "hello").unwrap();
        assert!(
            rx.recv_timeout(Duration::from_millis(800)).is_err(),
            "a sibling file must not trigger a reload"
        );
    }

    #[test]
    fn coalesces_a_burst_of_writes_into_one_callback() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("note.md");
        fs::write(&target, "one").unwrap();

        let (tx, rx) = channel();
        let _watcher = watch_file(&target, move || {
            let _ = tx.send(());
        })
        .unwrap();

        for i in 0..5 {
            fs::write(&target, format!("write {i}")).unwrap();
        }
        assert!(expect_change(&rx), "burst should notify at least once");
        assert!(
            rx.recv_timeout(Duration::from_millis(600)).is_err(),
            "burst should notify exactly once"
        );
    }
}
