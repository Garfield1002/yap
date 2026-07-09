//! The spell-check engine: a pure-Rust, Hunspell-compatible checker (spellbook)
//! reading the system's own Hunspell dictionaries from `/usr/share/hunspell`.
//!
//! The design named `hunspell-rs`; this uses spellbook instead ("or similar")
//! because it needs no libhunspell dev package to link, while reading the exact
//! same `.aff`/`.dic` files and keeping checking and suggestions under our own
//! programmatic control.
//!
//! Tokenizing is deliberately *not* done here: the plugin walks the CodeMirror
//! syntax tree to pull word candidates while skipping code, math, and URLs, then
//! sends the words to `spell_check`. Checking runs async and debounced from the
//! frontend, so nothing here sits in the keystroke path.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use spellbook::Dictionary;

/// Where system Hunspell/MySpell dictionaries live.
const DICT_DIRS: &[&str] = &["/usr/share/hunspell", "/usr/share/myspell"];

/// Loaded dictionaries, keyed by language, built once on first use.
fn cache() -> &'static Mutex<HashMap<String, Dictionary>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Dictionary>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Sanitize a language id to a dictionary basename: only `[A-Za-z0-9_]`, so it
/// can never escape the dictionary directories.
fn sanitize(lang: &str) -> String {
    lang.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_').collect()
}

fn load_dictionary(lang: &str) -> Result<Dictionary, String> {
    for dir in DICT_DIRS {
        let aff = Path::new(dir).join(format!("{lang}.aff"));
        let dic = Path::new(dir).join(format!("{lang}.dic"));
        if aff.exists() && dic.exists() {
            let aff_src = fs::read_to_string(&aff).map_err(|e| format!("{}: {e}", aff.display()))?;
            let dic_src = fs::read_to_string(&dic).map_err(|e| format!("{}: {e}", dic.display()))?;
            return Dictionary::new(&aff_src, &dic_src)
                .map_err(|e| format!("parsing {lang}: {e}"));
        }
    }
    Err(format!("no dictionary '{lang}' under {DICT_DIRS:?}"))
}

/// Run `f` against the (cached) dictionary for `lang`, loading it if needed.
fn with_dict<T>(lang: &str, f: impl FnOnce(&Dictionary) -> T) -> Result<T, String> {
    let lang = sanitize(lang);
    if lang.is_empty() {
        return Err("invalid language".into());
    }
    let mut map = cache().lock().map_err(|e| e.to_string())?;
    if !map.contains_key(&lang) {
        let dict = load_dictionary(&lang)?;
        map.insert(lang.clone(), dict);
    }
    Ok(f(map.get(&lang).expect("just inserted")))
}

/// Which languages have a dictionary installed, so the frontend can fall back
/// from a missing default.
#[tauri::command]
pub fn spell_languages() -> Vec<String> {
    let mut langs: Vec<String> = DICT_DIRS
        .iter()
        .flat_map(|dir| fs::read_dir(dir).into_iter().flatten().flatten())
        .filter_map(|e| {
            let path = e.path();
            if path.extension().is_some_and(|x| x == "dic") {
                path.file_stem().map(|s| s.to_string_lossy().into_owned())
            } else {
                None
            }
        })
        .collect();
    langs.sort();
    langs.dedup();
    langs
}

/// Return the subset of `words` that are misspelled in `lang`.
#[tauri::command]
pub fn spell_check(words: Vec<String>, lang: String) -> Result<Vec<String>, String> {
    with_dict(&lang, |dict| words.into_iter().filter(|w| !dict.check(w)).collect())
}

/// Suggest corrections for a single misspelled `word`.
#[tauri::command]
pub fn spell_suggest(word: String, lang: String) -> Result<Vec<String>, String> {
    with_dict(&lang, |dict| {
        let mut out = Vec::new();
        dict.suggest(&word, &mut out);
        out
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // These need the system en_US dictionary; skip cleanly when it is absent so
    // the suite still passes on a machine without hunspell dictionaries.
    fn en_us_available() -> bool {
        Path::new("/usr/share/hunspell/en_US.dic").exists()
    }

    #[test]
    fn flags_only_the_misspelled_words() {
        if !en_us_available() {
            return;
        }
        let bad = spell_check(
            vec!["hello".into(), "wrold".into(), "world".into(), "teh".into()],
            "en_US".into(),
        )
        .unwrap();
        assert!(bad.contains(&"wrold".to_string()));
        assert!(bad.contains(&"teh".to_string()));
        assert!(!bad.contains(&"hello".to_string()));
        assert!(!bad.contains(&"world".to_string()));
    }

    #[test]
    fn suggests_the_obvious_correction() {
        if !en_us_available() {
            return;
        }
        let suggestions = spell_suggest("wrold".into(), "en_US".into()).unwrap();
        assert!(suggestions.iter().any(|s| s == "world"));
    }

    #[test]
    fn sanitize_blocks_path_tricks() {
        assert_eq!(sanitize("../etc/passwd"), "etcpasswd");
        assert_eq!(sanitize("en_US"), "en_US");
    }

    #[test]
    fn missing_language_is_an_error() {
        assert!(with_dict("zz_NOPE", |_| ()).is_err());
    }
}
