//! Persistent app state: the recent-files list and the theme override.
//!
//! Stored as `state.json` under yap's config home. That home is `$YAP_HOME`
//! when set, else the XDG config dir (`$XDG_CONFIG_HOME` or `~/.config`) plus
//! `yap`. Every read and write goes straight to the file -- the state is tiny
//! and touched rarely (only when the open document changes or the theme flips),
//! so there is no in-memory cache to keep coherent.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// How many recent files to remember.
const RECENT_MAX: usize = 12;

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Most-recently-opened first. May contain paths that no longer exist;
    /// callers filter when it matters (e.g. building the Open Recent menu).
    #[serde(default)]
    pub recent: Vec<String>,
    /// `"light"` or `"dark"` to override the system, or `None` to follow it.
    #[serde(default)]
    pub theme: Option<String>,
}

/// The directory yap keeps its state in. `$YAP_HOME` wins; otherwise the
/// standard XDG config location.
pub fn config_home() -> PathBuf {
    if let Ok(home) = std::env::var("YAP_HOME") {
        if !home.is_empty() {
            return PathBuf::from(home);
        }
    }
    let base = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"));
    base.join("yap")
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
    fs::write(state_path(dir), text).map_err(|e| format!("{}: {e}", state_path(dir).display()))
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

// --- Tauri commands ---------------------------------------------------------

#[tauri::command]
pub fn get_config() -> AppConfig {
    load()
}

#[tauri::command]
pub fn set_theme(theme: Option<String>) -> Result<(), String> {
    let mut config = load();
    config.theme = theme;
    save(&config)
}

/// Remember `path` as the most recently opened file. The frontend calls this
/// and then `refresh_menu`, so the Open Recent submenu picks it up.
#[tauri::command]
pub fn record_recent(path: String) -> Result<(), String> {
    let mut config = load();
    push_recent(&mut config, &path);
    save(&config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_a_directory() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = AppConfig::default();
        config.theme = Some("dark".into());
        push_recent(&mut config, "/a.md");

        save_to(dir.path(), &config).unwrap();
        let loaded = load_from(dir.path());

        assert_eq!(loaded.theme.as_deref(), Some("dark"));
        assert_eq!(loaded.recent, vec!["/a.md"]);
    }

    #[test]
    fn missing_file_yields_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let config = load_from(dir.path());
        assert!(config.recent.is_empty());
        assert!(config.theme.is_none());
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
    fn config_home_prefers_yap_home() {
        // Env is process-global; this is the only test that touches it.
        std::env::set_var("YAP_HOME", "/tmp/yap-test-home");
        assert_eq!(config_home(), PathBuf::from("/tmp/yap-test-home"));
        std::env::remove_var("YAP_HOME");
    }
}
