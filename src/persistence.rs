use std::{
    ffi::OsString,
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::mpsc::{self, RecvTimeoutError},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

const RECENT_MAX: usize = 12;
const DEBOUNCE: Duration = Duration::from_millis(200);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub recent: Vec<String>,
    #[serde(default)]
    pub theme: Option<String>,
    #[serde(default = "default_dot_opacity")]
    pub dot_opacity: f32,
    #[serde(default)]
    pub plugins_enabled: Vec<String>,
}

const fn default_dot_opacity() -> f32 {
    0.12
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            recent: Vec::new(),
            theme: None,
            dot_opacity: default_dot_opacity(),
            plugins_enabled: Vec::new(),
        }
    }
}

pub fn config_home() -> PathBuf {
    if let Some(home) = std::env::var_os("BULLETMD_HOME").filter(|home| !home.is_empty()) {
        return PathBuf::from(home);
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"));
    base.join("bulletmd")
}

#[must_use] 
pub fn load_config() -> AppConfig {
    fs::read_to_string(config_home().join("state.json"))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save_config(config: &AppConfig) -> Result<(), String> {
    let dir = config_home();
    fs::create_dir_all(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    let text = serde_json::to_string_pretty(config).map_err(|error| error.to_string())?;
    atomic_write(&dir.join("state.json"), &text)
}

pub fn save_pasted_image(bytes: &[u8], extension: &str) -> Result<PathBuf, String> {
    save_pasted_image_in(&config_home().join("assets"), bytes, extension)
}

fn save_pasted_image_in(dir: &Path, bytes: &[u8], extension: &str) -> Result<PathBuf, String> {
    let extension: String = extension
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_lowercase();
    let extension = if extension.is_empty() {
        "png"
    } else {
        &extension
    };
    fs::create_dir_all(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis());
    let path = dir.join(format!("paste-{millis}.{extension}"));
    fs::write(&path, bytes).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(path)
}

pub fn push_recent(config: &mut AppConfig, path: &Path) {
    let path = path.to_string_lossy().into_owned();
    config.recent.retain(|candidate| candidate != &path);
    config.recent.insert(0, path);
    config.recent.truncate(RECENT_MAX);
}

pub fn atomic_write(target: &Path, contents: &str) -> Result<(), String> {
    let dir = target
        .parent()
        .filter(|directory| !directory.as_os_str().is_empty())
        .ok_or_else(|| format!("{} has no parent directory", target.display()))?;
    fs::create_dir_all(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    let mut temporary =
        NamedTempFile::new_in(dir).map_err(|error| format!("temporary file: {error}"))?;
    temporary
        .write_all(contents.as_bytes())
        .map_err(|error| format!("write: {error}"))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| format!("fsync: {error}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(target)
            .map_or(0o644, |metadata| metadata.permissions().mode() & 0o777);
        fs::set_permissions(temporary.path(), fs::Permissions::from_mode(mode))
            .map_err(|error| format!("chmod: {error}"))?;
    }

    temporary
        .persist(target)
        .map_err(|error| format!("rename onto {}: {}", target.display(), error.error))?;
    Ok(())
}

#[must_use] 
pub fn modified(path: &Path) -> Option<SystemTime> {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
}

pub fn watch_file<F>(path: &Path, on_change: F) -> Result<RecommendedWatcher, String>
where
    F: Fn() + Send + 'static,
{
    let dir = path
        .parent()
        .filter(|directory| !directory.as_os_str().is_empty())
        .ok_or_else(|| format!("{} has no parent directory", path.display()))?
        .to_path_buf();
    let name: OsString = path
        .file_name()
        .ok_or_else(|| format!("{} has no file name", path.display()))?
        .to_owned();
    let (sender, receiver) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |result| {
        let _ = sender.send(result);
    })
    .map_err(|error| error.to_string())?;
    watcher
        .watch(&dir, RecursiveMode::NonRecursive)
        .map_err(|error| error.to_string())?;

    thread::spawn(move || {
        let mut pending = false;
        loop {
            let event = if pending {
                match receiver.recv_timeout(DEBOUNCE) {
                    Ok(event) => Some(event),
                    Err(RecvTimeoutError::Timeout) => {
                        pending = false;
                        on_change();
                        continue;
                    }
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            } else {
                match receiver.recv() {
                    Ok(event) => Some(event),
                    Err(_) => break,
                }
            };
            if let Some(Ok(event)) = event
                && event
                    .paths
                    .iter()
                    .any(|candidate| candidate.file_name() == Some(name.as_os_str()))
            {
                pending = true;
            }
        }
    });
    Ok(watcher)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_replaces_contents() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("note.md");
        atomic_write(&path, "one").unwrap();
        atomic_write(&path, "two").unwrap();
        assert_eq!(fs::read_to_string(path).unwrap(), "two");
    }

    #[test]
    fn recent_files_dedupe_and_cap() {
        let mut config = AppConfig::default();
        for index in 0..14 {
            push_recent(&mut config, Path::new(&format!("/{index}.md")));
        }
        push_recent(&mut config, Path::new("/5.md"));
        assert_eq!(config.recent.len(), RECENT_MAX);
        assert_eq!(config.recent[0], "/5.md");
    }

    #[test]
    fn pasted_images_are_saved_with_a_sanitized_extension() {
        let directory = tempfile::tempdir().unwrap();
        let path = save_pasted_image_in(directory.path(), b"pixels", "P.N/G").unwrap();
        assert_eq!(
            path.extension().and_then(|value| value.to_str()),
            Some("png")
        );
        assert_eq!(fs::read(path).unwrap(), b"pixels");
    }

    #[test]
    fn watcher_survives_atomic_replacement() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("note.md");
        atomic_write(&path, "one").unwrap();
        let (sender, receiver) = mpsc::channel();
        let _watcher = watch_file(&path, move || {
            let _ = sender.send(());
        })
        .unwrap();
        atomic_write(&path, "two").unwrap();
        assert!(receiver.recv_timeout(Duration::from_secs(5)).is_ok());
    }
}
