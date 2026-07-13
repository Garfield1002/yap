//! Spell-check "plugin" (feature: `spellcheck`).
//!
//! This is the first feature-gated plugin. It is deliberately concrete — there
//! is no `Plugin` trait to conform to — so the integration points stay honest
//! and no speculative abstraction gets locked in before a second plugin exists.
//!
//! Like `model.rs`, this module carries no GPUI dependency: it works purely in
//! UTF-8 byte offsets into `DocumentModel::content` and hands back misspelled
//! byte ranges. The editor paints wavy underlines under them (`element.rs`) and
//! offers corrections in a context menu (`editor/interaction.rs`).
//!
//! The bundled `en_US` Hunspell dictionary (`dictionaries/`) is embedded at
//! compile time, so the feature has no runtime file dependency.

use std::collections::HashSet;
use std::ops::Range;

use spellbook::Dictionary;

use crate::model::{Block, BlockKind, DocumentModel};

const AFF: &str = include_str!("../dictionaries/en_US.aff");
const DIC: &str = include_str!("../dictionaries/en_US.dic");

/// The spell checker's state.
///
/// Owns the dictionary, the user's session ignore-list, and the most recent set
/// of misspelled ranges. `dirty` is set whenever the document changes and the
/// cached ranges are refreshed lazily in `Editor::ensure_shapes`.
pub struct SpellChecker {
    dict: Dictionary,
    ignored: HashSet<String>,
    /// Misspelled words as absolute source byte ranges, in document order.
    misspellings: Vec<Range<usize>>,
    /// Whether `misspellings` is stale relative to the document.
    pub dirty: bool,
}

impl SpellChecker {
    /// Builds a checker from the embedded dictionary. Returns `None` if the
    /// dictionary fails to parse, so a bad build degrades to "no spell check"
    /// rather than crashing the editor.
    #[must_use]
    pub fn new() -> Option<Self> {
        let dict = Dictionary::new(AFF, DIC).ok()?;
        Some(Self {
            dict,
            ignored: HashSet::new(),
            misspellings: Vec::new(),
            dirty: true,
        })
    }

    /// The cached misspelled ranges (absolute source byte offsets).
    #[must_use]
    pub fn misspellings(&self) -> &[Range<usize>] {
        &self.misspellings
    }

    /// The misspelled word range covering `offset`, if any — used to resolve a
    /// right-click into the word whose suggestions to show.
    #[must_use]
    pub fn misspelling_at(&self, offset: usize) -> Option<Range<usize>> {
        self.misspellings
            .iter()
            .find(|r| r.start <= offset && offset <= r.end)
            .cloned()
    }

    /// Up to `MAX_SUGGESTIONS` corrections for `word`, best first.
    #[must_use]
    pub fn suggest(&self, word: &str) -> Vec<String> {
        const MAX_SUGGESTIONS: usize = 7;
        let mut out = Vec::new();
        self.dict.suggest(word, &mut out);
        out.truncate(MAX_SUGGESTIONS);
        out
    }

    /// Accepts `word` for the rest of the session so it stops being flagged.
    pub fn ignore(&mut self, word: &str) {
        self.ignored.insert(word.to_string());
        self.dirty = true;
    }

    /// Recomputes `misspellings` over the whole document. Code and blank blocks
    /// are skipped; every other block is tokenized into words and each unknown
    /// word is recorded. Cheap enough to run on the full document for this POC.
    pub fn rescan(&mut self, doc: &DocumentModel) {
        self.misspellings.clear();
        for block in &doc.blocks {
            if matches!(block.kind, BlockKind::Code | BlockKind::Blank) {
                continue;
            }
            let base = block.range.start;
            let text = &doc.content[block.range.clone()];
            // Inline code (`` `like this` ``) is exempt: it is identifiers and
            // code, not prose. Fenced code blocks are already skipped above.
            let code = code_source_ranges(block);
            for word in word_ranges(text) {
                let token = &text[word.clone()];
                if is_skippable(token) || self.ignored.contains(token) {
                    continue;
                }
                let span = base + word.start..base + word.end;
                if code
                    .iter()
                    .any(|c| span.start < c.end && c.start < span.end)
                {
                    continue;
                }
                if self.dict.check(token) {
                    continue;
                }
                self.misspellings.push(span);
            }
        }
        self.dirty = false;
    }
}

/// Absolute source byte ranges of inline-code spans within `block`, derived
/// from the rendered spans' `code` style and their source map.
fn code_source_ranges(block: &Block) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    for line in &block.rendered {
        let mut offset = 0;
        for span in &line.spans {
            if span.style.code
                && let (Some(&start), Some(&end)) = (
                    line.source_map.get(offset),
                    line.source_map.get(offset + span.len),
                )
            {
                ranges.push(start..end);
            }
            offset += span.len;
        }
    }
    ranges
}

/// Words that are checkable-in-principle but which we deliberately never flag:
/// single characters and all-caps tokens (acronyms like `TODO`, `HTTP`).
fn is_skippable(token: &str) -> bool {
    token.chars().count() < 2
        || token
            .chars()
            .all(|c| c.is_uppercase() || !c.is_alphabetic())
}

/// Byte ranges of word-like tokens in `text`: maximal runs of alphabetic
/// characters, allowing a single apostrophe between letters (so "doesn't" is one
/// word). Punctuation, digits and markup split words apart.
fn word_ranges(text: &str) -> Vec<Range<usize>> {
    let mut words = Vec::new();
    let mut start: Option<usize> = None;
    let mut end = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        let is_apostrophe = c == '\'' || c == '\u{2019}';
        let keeps_word = c.is_alphabetic()
            || (is_apostrophe
                && start.is_some()
                && chars.peek().is_some_and(|&(_, n)| n.is_alphabetic()));
        if keeps_word {
            start.get_or_insert(i);
            end = i + c.len_utf8();
        } else if let Some(s) = start.take() {
            words.push(s..end);
        }
    }
    if let Some(s) = start {
        words.push(s..end);
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_words_with_internal_apostrophes() {
        let text = "It doesn't work, really—2nd time.";
        let words: Vec<&str> = word_ranges(text).iter().map(|r| &text[r.clone()]).collect();
        assert_eq!(words, ["It", "doesn't", "work", "really", "nd", "time"]);
    }

    #[test]
    fn skips_acronyms_and_single_letters() {
        assert!(is_skippable("HTTP"));
        assert!(is_skippable("a"));
        assert!(!is_skippable("hello"));
    }

    #[test]
    fn flags_only_unknown_words() {
        let Some(mut checker) = SpellChecker::new() else {
            return;
        };
        let doc = DocumentModel::new("the quikc brown fox".into());
        checker.rescan(&doc);
        let flagged: Vec<&str> = checker
            .misspellings()
            .iter()
            .map(|r| &doc.content[r.clone()])
            .collect();
        assert_eq!(flagged, ["quikc"]);
    }

    #[test]
    fn inline_code_is_exempt() {
        let Some(mut checker) = SpellChecker::new() else {
            return;
        };
        // `xyzzy` is inline code and must not be flagged; the bare word is.
        let doc = DocumentModel::new("call `xyzzy` then xyzzy".into());
        checker.rescan(&doc);
        let flagged: Vec<&str> = checker
            .misspellings()
            .iter()
            .map(|r| &doc.content[r.clone()])
            .collect();
        assert_eq!(flagged, ["xyzzy"]);
        assert_eq!(checker.misspellings()[0].start, "call `xyzzy` then ".len());
    }

    #[test]
    fn ignore_clears_a_flag() {
        let Some(mut checker) = SpellChecker::new() else {
            return;
        };
        let doc = DocumentModel::new("frobnicate".into());
        checker.rescan(&doc);
        assert_eq!(checker.misspellings().len(), 1);
        checker.ignore("frobnicate");
        checker.rescan(&doc);
        assert!(checker.misspellings().is_empty());
    }
}
