//! Plugin discovery and per-plugin storage. The frontend never touches the
//! filesystem: it asks here for the source it then evaluates in the webview.
//!
//! A plugin is a directory under `$YAP_HOME/plugins/<dir>/` holding:
//!   manifest.json  { name, version, apiVersion, entry }
//!   main.js        the ESM bundle (or whatever `entry` names)
//!   *.css          optional styles, concatenated and returned
//!   data.json      optional per-plugin settings, read/written on demand
//!
//! A directory whose manifest is missing or unparseable is still reported, with
//! `error` set, so the loader can surface it rather than silently skipping it.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::config_home;

fn plugins_dir() -> PathBuf {
    config_home().join("plugins")
}

/// The `manifest.json` shape. `entry` defaults to `main.js`.
#[derive(Deserialize)]
struct Manifest {
    name: String,
    #[serde(default)]
    version: String,
    #[serde(rename = "apiVersion")]
    api_version: u32,
    #[serde(default)]
    entry: Option<String>,
}

/// One discovered plugin, ready for the frontend to evaluate (or to show as
/// broken when `error` is set).
#[derive(Serialize)]
pub struct PluginInfo {
    /// Directory name; the stable id the enabled-set and data.json key on.
    dir: String,
    name: String,
    version: String,
    api_version: u32,
    /// The entry module's source; empty when `error` is set.
    source: String,
    /// All `*.css` in the directory, concatenated.
    css: String,
    /// Set when the plugin could not be read; `source` is then empty.
    error: Option<String>,
}

fn broken(dir: &str, error: String) -> PluginInfo {
    PluginInfo {
        dir: dir.to_owned(),
        name: dir.to_owned(),
        version: String::new(),
        api_version: 0,
        source: String::new(),
        css: String::new(),
        error: Some(error),
    }
}

fn read_css(dir: &Path) -> String {
    let mut names: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "css"))
        .collect();
    names.sort();
    names
        .iter()
        .filter_map(|p| fs::read_to_string(p).ok())
        .collect::<Vec<_>>()
        .join("\n")
}

fn load_one(dir: &Path) -> PluginInfo {
    let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();

    let manifest_path = dir.join("manifest.json");
    let manifest: Manifest = match fs::read_to_string(&manifest_path) {
        Ok(text) => match serde_json::from_str(&text) {
            Ok(m) => m,
            Err(e) => return broken(&name, format!("manifest.json: {e}")),
        },
        Err(e) => return broken(&name, format!("manifest.json: {e}")),
    };

    let entry = manifest.entry.as_deref().unwrap_or("main.js");
    let source = match fs::read_to_string(dir.join(entry)) {
        Ok(s) => s,
        Err(e) => return broken(&name, format!("{entry}: {e}")),
    };

    PluginInfo {
        dir: name,
        name: manifest.name,
        version: manifest.version,
        api_version: manifest.api_version,
        source,
        css: read_css(dir),
        error: None,
    }
}

/// Discover every plugin under `$YAP_HOME/plugins/`. A missing directory is not
/// an error -- it just means no plugins are installed.
#[tauri::command]
pub fn list_plugins() -> Vec<PluginInfo> {
    let dir = plugins_dir();
    let mut out: Vec<PluginInfo> = fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .map(|p| load_one(&p))
        .collect();
    // Stable order so menus and the enabled list read consistently.
    out.sort_by(|a, b| a.dir.cmp(&b.dir));
    out
}

/// Guard against `..`/absolute paths in a plugin id before it touches the FS.
fn plugin_dir(dir: &str) -> Result<PathBuf, String> {
    if dir.is_empty() || dir.contains('/') || dir.contains('\\') || dir.contains("..") {
        return Err(format!("invalid plugin id: {dir}"));
    }
    Ok(plugins_dir().join(dir))
}

/// Read a plugin's `data.json`. A missing file yields `{}` so the plugin's
/// settings code always gets valid JSON.
#[tauri::command]
pub fn read_plugin_data(dir: String) -> Result<String, String> {
    let path = plugin_dir(&dir)?.join("data.json");
    match fs::read_to_string(&path) {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok("{}".to_owned()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// Install a plugin by copying a folder (the one the user picked) into
/// `$YAP_HOME/plugins/`. The source must contain a valid `manifest.json`; the
/// folder's own name becomes the plugin id. Returns that id so the frontend can
/// enable the freshly installed plugin. Refuses to clobber an existing install.
#[tauri::command]
pub fn install_plugin(source: String) -> Result<String, String> {
    let src = PathBuf::from(&source);

    let manifest = src.join("manifest.json");
    let text = fs::read_to_string(&manifest)
        .map_err(|e| format!("no manifest.json in {source}: {e}"))?;
    serde_json::from_str::<Manifest>(&text).map_err(|e| format!("invalid manifest.json: {e}"))?;

    let name = src
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "cannot determine a folder name to install as".to_owned())?;
    let dest = plugin_dir(name)?;
    if dest.exists() {
        return Err(format!("'{name}' is already installed; disable and remove it first"));
    }

    copy_dir(&src, &dest).map_err(|e| format!("copying into {}: {e}", dest.display()))?;
    Ok(name.to_owned())
}

/// Write a plugin's `data.json`, creating the plugin directory if needed.
#[tauri::command]
pub fn write_plugin_data(dir: String, contents: String) -> Result<(), String> {
    let dir = plugin_dir(&dir)?;
    fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let path = dir.join("data.json");
    fs::write(&path, contents).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make(dir: &Path, name: &str) -> PathBuf {
        let p = dir.join(name);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn loads_a_well_formed_plugin_with_its_css() {
        let root = tempfile::tempdir().unwrap();
        let p = make(root.path(), "spell");
        fs::write(
            p.join("manifest.json"),
            r#"{ "name": "Spell Check", "version": "0.1.0", "apiVersion": 1 }"#,
        )
        .unwrap();
        fs::write(p.join("main.js"), "export function activate() {}").unwrap();
        fs::write(p.join("a.css"), ".x{}").unwrap();

        let info = load_one(&p);
        assert!(info.error.is_none());
        assert_eq!(info.dir, "spell");
        assert_eq!(info.name, "Spell Check");
        assert_eq!(info.api_version, 1);
        assert!(info.source.contains("activate"));
        assert!(info.css.contains(".x{}"));
    }

    #[test]
    fn honors_a_custom_entry() {
        let root = tempfile::tempdir().unwrap();
        let p = make(root.path(), "custom");
        fs::write(
            p.join("manifest.json"),
            r#"{ "name": "C", "apiVersion": 1, "entry": "index.js" }"#,
        )
        .unwrap();
        fs::write(p.join("index.js"), "export function activate() {}").unwrap();

        assert!(load_one(&p).error.is_none());
    }

    #[test]
    fn a_bad_manifest_becomes_an_error_entry_not_a_skip() {
        let root = tempfile::tempdir().unwrap();
        let p = make(root.path(), "broken");
        fs::write(p.join("manifest.json"), "{ not json").unwrap();

        let info = load_one(&p);
        assert!(info.error.is_some());
        assert_eq!(info.dir, "broken");
        assert!(info.source.is_empty());
    }

    #[test]
    fn a_missing_entry_file_is_an_error() {
        let root = tempfile::tempdir().unwrap();
        let p = make(root.path(), "noentry");
        fs::write(p.join("manifest.json"), r#"{ "name": "N", "apiVersion": 1 }"#).unwrap();

        assert!(load_one(&p).error.is_some());
    }

    #[test]
    fn copy_dir_copies_nested_contents() {
        let root = tempfile::tempdir().unwrap();
        let src = make(root.path(), "src");
        fs::write(src.join("manifest.json"), "{}").unwrap();
        fs::create_dir_all(src.join("sub")).unwrap();
        fs::write(src.join("sub/a.js"), "x").unwrap();

        let dst = root.path().join("dst");
        copy_dir(&src, &dst).unwrap();

        assert_eq!(fs::read_to_string(dst.join("manifest.json")).unwrap(), "{}");
        assert_eq!(fs::read_to_string(dst.join("sub/a.js")).unwrap(), "x");
    }

    #[test]
    fn plugin_dir_rejects_traversal() {
        assert!(plugin_dir("..").is_err());
        assert!(plugin_dir("a/b").is_err());
        assert!(plugin_dir("").is_err());
        assert!(plugin_dir("ok-name").is_ok());
    }
}
