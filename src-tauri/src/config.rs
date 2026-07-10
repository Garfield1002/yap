//! Persistent app state: recent files and appearance preferences.
//!
//! Stored as `state.json` under bulletmd's config home. That home is
//! `$BULLETMD_HOME` when set, else the XDG config dir (`$XDG_CONFIG_HOME` or
//! `~/.config`) plus `bulletmd`.
//! Every read and write goes straight to the file -- the state is tiny
//! and touched rarely (only when the open document or a preference changes),
//! so there is no in-memory cache to keep coherent.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use serde::{Deserialize, Serialize};

/// How many recent files to remember.
const RECENT_MAX: usize = 12;
static CONFIG_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Most-recently-opened first. May contain paths that no longer exist;
    /// callers filter when it matters (e.g. building the Open Recent menu).
    #[serde(default)]
    pub recent: Vec<String>,
    /// `"light"` or `"dark"` to override the system, or `None` to follow it.
    #[serde(default)]
    pub theme: Option<String>,
    /// Opacity of the bullet-journal dot grid, from fully hidden to 30%.
    #[serde(
        default = "default_dot_opacity",
        deserialize_with = "deserialize_dot_opacity"
    )]
    pub dot_opacity: f32,
    /// Directory names of the plugins the user has enabled. This is the only
    /// plugin state core owns; everything else lives in each plugin's data.json.
    #[serde(default)]
    pub plugins_enabled: Vec<String>,
}

const fn default_dot_opacity() -> f32 {
    0.12
}

fn clamp_dot_opacity(value: f32) -> f32 {
    value.clamp(0.0, 0.30)
}

fn deserialize_dot_opacity<'de, D>(deserializer: D) -> Result<f32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    f32::deserialize(deserializer).map(clamp_dot_opacity)
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

/// The directory bulletmd keeps its state in.
pub fn config_home() -> PathBuf {
    if let Ok(home) = std::env::var("BULLETMD_HOME") {
        if !home.is_empty() {
            return PathBuf::from(home);
        }
    }
    let base = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|h| PathBuf::from(h).join(".config"))
        })
        .unwrap_or_else(|| PathBuf::from(".config"));
    base.join("bulletmd")
}

fn state_path(dir: &Path) -> PathBuf {
    dir.join("state.json")
}

/// Read the config from `dir`. A missing or unparseable file is not an error --
/// a first run, or a file someone hand-edited into nonsense, just yields the
/// defaults rather than blocking the app from starting.
pub fn load_from(dir: &Path) -> AppConfig {
    fs::read_to_string(state_path(dir))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Write the config to `dir`, creating the directory if needed.
pub fn save_to(dir: &Path, config: &AppConfig) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let text = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    let path = state_path(dir);
    let mut temporary =
        tempfile::NamedTempFile::new_in(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    temporary
        .write_all(text.as_bytes())
        .map_err(|e| format!("{}: {e}", temporary.path().display()))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|e| format!("{}: {e}", temporary.path().display()))?;
    temporary
        .persist(&path)
        .map_err(|e| format!("{}: {}", path.display(), e.error))?;
    Ok(())
}

/// Move `path` to the front of the recent list, de-duplicating and capping.
pub fn push_recent(config: &mut AppConfig, path: &str) {
    config.recent.retain(|p| p != path);
    config.recent.insert(0, path.to_owned());
    config.recent.truncate(RECENT_MAX);
}

pub fn load() -> AppConfig {
    load_from(&config_home())
}

pub fn save(config: &AppConfig) -> Result<(), String> {
    save_to(&config_home(), config)
}

fn lock_config() -> MutexGuard<'static, ()> {
    CONFIG_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn update_config(change: impl FnOnce(&mut AppConfig)) -> Result<(), String> {
    let _guard = lock_config();
    let mut config = load();
    change(&mut config);
    save(&config)
}

// --- Tauri commands ---------------------------------------------------------

#[tauri::command]
pub fn get_config() -> AppConfig {
    let _guard = lock_config();
    load()
}

#[tauri::command]
pub fn set_theme(theme: Option<String>) -> Result<(), String> {
    update_config(|config| config.theme = theme)
}

#[tauri::command]
pub fn set_dot_opacity(opacity: f32) -> Result<(), String> {
    update_config(|config| config.dot_opacity = clamp_dot_opacity(opacity))
}

/// Remember `path` as the most recently opened file. The next File-menu popup
/// rebuilds Open Recent from the config, so it picks this up automatically.
#[tauri::command]
pub fn record_recent(path: String) -> Result<(), String> {
    update_config(|config| push_recent(config, &path))
}

/// Persist the set of enabled plugins (directory names).
#[tauri::command]
pub fn set_plugins_enabled(enabled: Vec<String>) -> Result<(), String> {
    update_config(|config| config.plugins_enabled = enabled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_a_directory() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = AppConfig::default();
        config.theme = Some("dark".into());
        config.dot_opacity = 0.2;
        push_recent(&mut config, "/a.md");

        save_to(dir.path(), &config).unwrap();
        let loaded = load_from(dir.path());

        assert_eq!(loaded.theme.as_deref(), Some("dark"));
        assert_eq!(loaded.dot_opacity, 0.2);
        assert_eq!(loaded.recent, vec!["/a.md"]);
    }

    #[test]
    fn missing_file_yields_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let config = load_from(dir.path());
        assert!(config.recent.is_empty());
        assert!(config.theme.is_none());
        assert_eq!(config.dot_opacity, 0.12);
    }

    #[test]
    fn push_recent_dedupes_and_moves_to_front() {
        let mut config = AppConfig::default();
        push_recent(&mut config, "/a.md");
        push_recent(&mut config, "/b.md");
        push_recent(&mut config, "/a.md"); // seen again

        assert_eq!(config.recent, vec!["/a.md", "/b.md"]);
    }

    #[test]
    fn push_recent_caps_the_list() {
        let mut config = AppConfig::default();
        for i in 0..(RECENT_MAX + 5) {
            push_recent(&mut config, &format!("/f{i}.md"));
        }
        assert_eq!(config.recent.len(), RECENT_MAX);
        // The most recent push is at the front.
        assert_eq!(config.recent[0], format!("/f{}.md", RECENT_MAX + 4));
    }

    #[test]
    fn config_home_prefers_bulletmd_home() {
        // Env is process-global; this is the only test that touches it.
        std::env::set_var("BULLETMD_HOME", "/tmp/bulletmd-test-home");
        assert_eq!(config_home(), PathBuf::from("/tmp/bulletmd-test-home"));
        std::env::remove_var("BULLETMD_HOME");
    }

    #[test]
    fn state_without_dot_opacity_uses_the_default() {
        let config: AppConfig = serde_json::from_str(r#"{"recent":[],"theme":null}"#).unwrap();
        assert_eq!(config.dot_opacity, 0.12);
    }

    #[test]
    fn dot_opacity_is_clamped_when_loading() {
        let high: AppConfig = serde_json::from_str(r#"{"dot_opacity":0.8}"#).unwrap();
        let low: AppConfig = serde_json::from_str(r#"{"dot_opacity":-0.2}"#).unwrap();
        assert_eq!(high.dot_opacity, 0.30);
        assert_eq!(low.dot_opacity, 0.0);
    }
}
