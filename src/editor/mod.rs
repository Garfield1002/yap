use std::{
    collections::{HashMap, HashSet},
    fs,
    ops::Range,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

// Submodules pull these in via `use super::*`; some are unused by mod.rs itself.
#[allow(unused_imports)]
use crate::{
    model::{
        BlockId, BlockKind, DocumentModel, EditMode, EditTransaction, ListContinuation, TaskMark,
        auto_close, block_gap,
    },
    layout::{DOCUMENT_WIDTH, GRID, PROSE_FONT},
    persistence::{self, AppConfig},
    shaping::{
        ShapeCache, ShapedLine, default_text_metrics, grapheme_offset, image_allocation_rows,
        image_extension, next_word_boundary, previous_word_boundary, rows, shape_raw, shape_render,
        source_line_is_code, utf16_to_utf8, utf8_to_utf16,
    },
    theme::{Palette, alpha, color, is_dark, palette},
};
use gpui::{
    App, Bounds, BoxShadow, ClipboardEntry, ClipboardItem, Context, CursorStyle,
    EntityInputHandler, FocusHandle, Focusable, Image, ImageFormat, ImgResourceLoader,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    PathPromptOptions, Pixels, Point, PromptLevel, ScrollHandle,
    UTF16Selection, Window,
    WindowAppearance, actions,
    deferred, div, font, img, point, prelude::*, px, size,
};
use notify::RecommendedWatcher;
use unicode_segmentation::UnicodeSegmentation;

mod commands;
mod input;
mod interaction;
mod menu;
mod movement;
mod render;
mod view;


/// The `BulletMD` logo as a color SVG image.
///
/// Selects the light-on-dark or dark-on-light variant for the current theme.
/// The asset cache dedupes on the byte hash, so building this per render only
/// rasterizes once.
#[must_use] 
pub fn logo_image(dark: bool) -> Arc<Image> {
    let bytes: &[u8] = if dark {
        include_bytes!("../../assets/logo_white.svg")
    } else {
        include_bytes!("../../assets/logo_black.svg")
    };
    Arc::new(Image::from_bytes(ImageFormat::Svg, bytes.to_vec()))
}


actions!(
    editor,
    [
        Backspace,
        Delete,
        Left,
        Right,
        Up,
        Down,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        WordLeft,
        WordRight,
        SelectWordLeft,
        SelectWordRight,
        DeleteWordBackward,
        Home,
        End,
        SelectAll,
        Enter,
        Escape,
        Paste,
        Cut,
        Copy,
        Undo,
        Redo,
        Save,
        NewDocument,
        OpenDocument,
        SaveAs,
        RenameDocument,
        DeleteDocument,
        CopyPath,
        OpenLocation,
        NewWindow,
        Bold,
        Italic,
        InlineCode,
        Link,
        Quit
    ]
);

use crate::element::{
    BlockWidget, DocumentElement, HitLine, RenderClass, checkbox_bounds, source_offset_for_hit,
    vertical_distance,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OpenMenu {
    File,
    Edit,
    Settings,
}

/// The spell-check suggestion context menu: where it was opened, which source
/// word range it targets, and the offered corrections. Set on right-click over
/// a misspelled word (see `interaction.rs`), rendered as a floating overlay.
#[cfg(feature = "spellcheck")]
pub(crate) struct SpellMenu {
    pub(crate) position: Point<Pixels>,
    pub(crate) word: Range<usize>,
    pub(crate) suggestions: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ThemePreference {
    System,
    Light,
    Dark,
}

impl ThemePreference {
    #[must_use] 
    pub const fn name(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    #[must_use] 
    pub const fn dark(self, appearance: WindowAppearance) -> bool {
        match self {
            Self::System => is_dark(appearance),
            Self::Light => false,
            Self::Dark => true,
        }
    }
}

#[derive(Clone, Copy)]
pub enum MenuCommand {
    New,
    NewWindow,
    Open,
    OpenRecent(usize),
    Save,
    SaveAs,
    Rename,
    Delete,
    CopyPath,
    OpenLocation,
    Quit,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    SetTheme(ThemePreference),
    SetDotOpacity(u8),
}

/// Undo/redo stacks of committed edit transactions.
#[derive(Default)]
struct History {
    undo: Vec<EditTransaction>,
    redo: Vec<EditTransaction>,
}

/// The caret/selection state: the selected byte range and the ancillary flags
/// that shape how it moves and renders.
pub(crate) struct Selection {
    /// Selected byte range; empty (`start == end`) means a bare caret.
    pub(crate) range: Range<usize>,
    /// Whether the caret is the range's start (selecting leftward).
    reversed: bool,
    /// IME pre-edit range, if a composition is in progress.
    marked: Option<Range<usize>>,
    /// Whether a drag-select is currently in progress.
    selecting: bool,
    /// Sticky column for vertical caret motion across short lines. Used by the
    /// logical fallback path when no shaped layout is available.
    preferred_column: Option<usize>,
    /// Sticky horizontal position (pixels from the line's left edge) for
    /// wrap-aware vertical caret motion. Preferred over `preferred_column` when
    /// the caret's block has a cached shaped layout.
    preferred_x: Option<Pixels>,
    /// Pending scroll anchor: (source offset, screen y) to hold steady.
    pub(crate) pending_anchor: Option<(usize, Pixels)>,
    /// Request to scroll the caret into view on the next paint.
    pub(crate) ensure_caret_visible: bool,
}

impl Default for Selection {
    fn default() -> Self {
        Self {
            range: 0..0,
            reversed: false,
            marked: None,
            selecting: false,
            preferred_column: None,
            preferred_x: None,
            pending_anchor: None,
            ensure_caret_visible: false,
        }
    }
}

/// Per-frame render cache: shaped lines keyed by block, the flat hit-test
/// lines from the last paint, the document bounds, and which blocks are
/// revealed as editable source.
#[derive(Default)]
pub(crate) struct LayoutState {
    pub(crate) shapes: HashMap<BlockId, ShapeCache>,
    pub(crate) hit_lines: Vec<HitLine>,
    pub(crate) doc_bounds: Option<Bounds<Pixels>>,
    pub(crate) revealed: HashSet<BlockId>,
    /// Ascent/descent of a generic prose line, measured once on first shape and
    /// reused for empty lines (which carry no glyphs to measure themselves).
    pub(crate) default_text_metrics: Option<(Pixels, Pixels)>,
}

/// Resolved appearance: whether we're painting dark, the user's preference it
/// derives from, and the background dot opacity.
pub(crate) struct ThemeState {
    pub(crate) dark: bool,
    theme: ThemePreference,
    pub(crate) dot_opacity: f32,
}

/// The save/persistence lifecycle: the last-saved text (for dirty detection),
/// the dirty flag, autosave/file-watch generations, an external-edit conflict,
/// and whether a close is pending on the next save.
#[derive(Default)]
struct SaveState {
    saved_text: String,
    dirty: bool,
    autosave_generation: u64,
    watcher: Option<RecommendedWatcher>,
    watch_generation: u64,
    conflict_text: Option<String>,
    close_after_save: bool,
}

pub struct Editor {
    path: Option<PathBuf>,
    pub(crate) focus: FocusHandle,
    pub(crate) document: DocumentModel,
    pub(crate) sel: Selection,
    pub(crate) layout: LayoutState,
    pub(crate) vertical_scroll: ScrollHandle,
    pub(crate) horizontal_scroll: ScrollHandle,
    history: History,
    status: String,
    pub(crate) theming: ThemeState,
    save: SaveState,
    config: AppConfig,
    open_menu: Option<OpenMenu>,
    /// Whether the File menu's "Open Recent" submenu is expanded.
    recent_submenu_open: bool,
    /// Acceleration state for held undo/redo: the time of the last history step,
    /// its direction, and how long the current run is. Consecutive presses in
    /// quick succession apply progressively more steps, so holding Ctrl+Z races
    /// through history instead of crawling one step per keypress.
    pub(crate) history_repeat: Option<(std::time::Instant, isize, u32)>,
    /// The spell checker, when the `spellcheck` feature is built and the bundled
    /// dictionary loaded. `None` disables the plugin without any other branch
    /// needing to care.
    #[cfg(feature = "spellcheck")]
    pub(crate) spell: Option<crate::spellcheck::SpellChecker>,
    /// The open spell-suggestion context menu, if any.
    #[cfg(feature = "spellcheck")]
    pub(crate) spell_menu: Option<SpellMenu>,
    /// Whether spell checking is currently on. Toggled from the status bar; when
    /// off, no words are underlined and the context menu is suppressed.
    #[cfg(feature = "spellcheck")]
    pub(crate) spell_enabled: bool,
}

impl Editor {
    /// Builds an editor for `content` loaded from `path` (if any), applying the
    /// persisted `config` and resolved `theme`. Nothing starts revealed, so the
    /// document opens fully rendered with no caret until the reader interacts.
    pub fn new(
        path: Option<PathBuf>,
        content: String,
        theme: ThemePreference,
        config: AppConfig,
        cx: &mut Context<Self>,
    ) -> Self {
        let document = DocumentModel::new(content.clone());
        Self {
            path,
            focus: cx.focus_handle(),
            document,
            sel: Selection {
                ensure_caret_visible: true,
                ..Selection::default()
            },
            layout: LayoutState::default(),
            vertical_scroll: ScrollHandle::new(),
            horizontal_scroll: ScrollHandle::new(),
            history: History::default(),
            status: "saved".into(),
            theming: ThemeState {
                dark: false,
                theme,
                dot_opacity: config.dot_opacity.clamp(0.0, 0.30),
            },
            save: SaveState {
                saved_text: content,
                ..SaveState::default()
            },
            config,
            open_menu: None,
            recent_submenu_open: false,
            history_repeat: None,
            #[cfg(feature = "spellcheck")]
            spell: crate::spellcheck::SpellChecker::new(),
            #[cfg(feature = "spellcheck")]
            spell_menu: None,
            #[cfg(feature = "spellcheck")]
            spell_enabled: true,
        }
    }

    /// Toggles spell checking on or off (from the status-bar indicator).
    #[cfg(feature = "spellcheck")]
    pub(crate) fn toggle_spell(&mut self, cx: &mut Context<Self>) {
        self.spell_enabled = !self.spell_enabled;
        self.spell_menu = None;
        cx.notify();
    }

    /// Marks the spell checker's cached results stale after a document change,
    /// so `ensure_shapes` rescans before the next paint. No-op without the
    /// `spellcheck` feature.
    #[cfg(feature = "spellcheck")]
    pub(crate) fn mark_spell_dirty(&mut self) {
        if let Some(spell) = self.spell.as_mut() {
            spell.dirty = true;
        }
        self.spell_menu = None;
    }

    /// The misspelled word ranges to underline, or empty when the plugin is
    /// disabled. Read by `element.rs` during paint.
    #[cfg(feature = "spellcheck")]
    pub(crate) fn misspellings(&self) -> &[Range<usize>] {
        if !self.spell_enabled {
            return &[];
        }
        self.spell.as_ref().map_or(&[], |spell| spell.misspellings())
    }

    /// The focus handle, for the window to focus the editor on open.
    #[must_use] 
    pub const fn focus(&self) -> &FocusHandle {
        &self.focus
    }

    /// Whether the document has unsaved changes.
    #[must_use] 
    pub const fn is_dirty(&self) -> bool {
        self.save.dirty
    }
}


