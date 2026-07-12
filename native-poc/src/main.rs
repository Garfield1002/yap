use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs,
    hash::{DefaultHasher, Hash, Hasher},
    ops::Range,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};

use bulletmd_native_poc::{
    model::{
        BlockId, BlockKind, DocumentModel, EditMode, EditTransaction, InlineStyle, LatencySamples,
        RenderLine,
    },
    persistence::{self, AppConfig},
};
use gpui::{
    App, Application, Bounds, BoxShadow, ClipboardEntry, ClipboardItem, Context, CursorStyle,
    Element, ElementId, ElementInputHandler, Entity, EntityInputHandler, FocusHandle, Focusable,
    FontStyle, FontWeight, GlobalElementId, Hsla, ImageFormat, ImgResourceLoader, KeyBinding,
    LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad,
    PathPromptOptions, Pixels, Point, PromptLevel, RenderImage, Resource, ScrollHandle,
    SharedString, Style, TextAlign, TextRun, UTF16Selection, UnderlineStyle, Window,
    WindowAppearance, WindowBounds, WindowDecorations, WindowOptions, WrappedLine, actions,
    deferred, div, fill, font, point, prelude::*, px, rgb, size,
};
use notify::RecommendedWatcher;
use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use unicode_segmentation::UnicodeSegmentation;

const GRID: f32 = 24.;
const FIRST_BASELINE: f32 = 48.;
const DOCUMENT_WIDTH: f32 = 768.;
const INSET: f32 = 24.;
const WRAP_WIDTH: f32 = DOCUMENT_WIDTH - INSET * 2.;
// The Tauri stack starts with Inter but resolves to Noto Sans on this Linux
// machine. Naming the installed face directly matters in GPUI: unlike CSS,
// its missing-family fallback does not reliably retain bold and italic faces.
const PROSE_FONT: &str = "Noto Sans";
const MONO_FONT: &str = "Noto Sans Mono";
const DEFAULT_IMAGE_ROWS: usize = 8;

fn image_resource(src: &str, document_path: Option<&Path>) -> Resource {
    if src.starts_with("http://") || src.starts_with("https://") || src.starts_with("data:") {
        return Resource::Uri(src.to_string().into());
    }
    let src = src.strip_prefix("file://").unwrap_or(src);
    let path = PathBuf::from(src);
    let path = if path.is_absolute() {
        path
    } else {
        document_path
            .and_then(Path::parent)
            .unwrap_or_else(|| Path::new("."))
            .join(path)
    };
    Resource::from(path)
}

fn image_allocation_rows(data: &RenderImage) -> usize {
    let dimensions = data.size(0);
    let width = dimensions.width.0.max(1) as f32;
    let height = dimensions.height.0.max(1) as f32;
    let ratio = width / height;
    let intrinsic_height = width.min(WRAP_WIDTH) / ratio;
    let mut rows = (intrinsic_height / GRID).ceil().max(1.) as usize;
    if rows as f32 * GRID * ratio > WRAP_WIDTH + 0.5 {
        rows = ((WRAP_WIDTH / ratio) / GRID).floor().max(1.) as usize;
    }
    rows
}

fn image_extension(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpg",
        ImageFormat::Webp => "webp",
        ImageFormat::Gif => "gif",
        ImageFormat::Svg => "svg",
        ImageFormat::Bmp => "bmp",
        ImageFormat::Tiff => "tiff",
    }
}

#[derive(Clone, Copy)]
struct Palette {
    bg: Hsla,
    bg_subtle: Hsla,
    fg: Hsla,
    fg_dim: Hsla,
    fg_faint: Hsla,
    accent: Hsla,
    border: Hsla,
    code_bg: Hsla,
    code_border: Hsla,
    selection: Hsla,
    dot: Hsla,
    syn_keyword: Hsla,
    syn_string: Hsla,
    syn_comment: Hsla,
    syn_number: Hsla,
    syn_name: Hsla,
    syn_type: Hsla,
    syn_operator: Hsla,
}

fn color(value: u32) -> Hsla {
    rgb(value).into()
}

fn alpha(mut color: Hsla, alpha: f32) -> Hsla {
    color.a = alpha;
    color
}

fn palette(dark: bool, dot_opacity: f32) -> Palette {
    if dark {
        Palette {
            bg: color(0x171717),
            bg_subtle: color(0x202020),
            fg: color(0xf7f7f5),
            fg_dim: color(0x8b949e),
            fg_faint: color(0x5a626c),
            accent: color(0x6cb6ff),
            border: color(0x303030),
            code_bg: color(0x202020),
            code_border: color(0x303030),
            selection: alpha(color(0x6cb6ff), 0.36),
            dot: alpha(color(0xf7f7f5), dot_opacity),
            syn_keyword: color(0xff7b72),
            syn_string: color(0x7ee787),
            syn_comment: color(0x8b949e),
            syn_number: color(0x79c0ff),
            syn_name: color(0xffa657),
            syn_type: color(0xd2a8ff),
            syn_operator: color(0x79c0ff),
        }
    } else {
        Palette {
            bg: color(0xf7f7f5),
            bg_subtle: color(0xefefeb),
            fg: color(0x171717),
            fg_dim: color(0x6e7781),
            fg_faint: color(0xadb3b9),
            accent: color(0x0969da),
            border: color(0xdcdcd7),
            code_bg: color(0xefefec),
            code_border: color(0xdfdfda),
            selection: alpha(color(0x0969da), 0.32),
            dot: alpha(color(0x171717), dot_opacity),
            syn_keyword: color(0xcf222e),
            syn_string: color(0x0a6847),
            syn_comment: color(0x6e7781),
            syn_number: color(0x0550ae),
            syn_name: color(0x953800),
            syn_type: color(0x6639ba),
            syn_operator: color(0x0550ae),
        }
    }
}

fn is_dark(appearance: WindowAppearance) -> bool {
    matches!(
        appearance,
        WindowAppearance::Dark | WindowAppearance::VibrantDark
    )
}

fn source_line_is_code(kind: BlockKind, block_end: usize, line_start: usize) -> bool {
    kind == BlockKind::Code && line_start <= block_end
}

static STARTED: OnceLock<Instant> = OnceLock::new();
static FIRST_FRAME: OnceLock<()> = OnceLock::new();

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

#[derive(Clone)]
struct ShapedLine {
    source: Range<usize>,
    layout: WrappedLine,
    map: Option<Vec<usize>>,
    code: bool,
    image: Option<ShapedImage>,
}

#[derive(Clone)]
struct ShapedImage {
    alt: String,
    resource: Resource,
    data: Option<Arc<RenderImage>>,
    failed: bool,
    rows: usize,
}

#[derive(Clone, Default)]
struct ShapeCache {
    raw: Option<Vec<ShapedLine>>,
    rendered: Option<Vec<ShapedLine>>,
    raw_rows: usize,
    raw_separator_rows: usize,
    rendered_rows: usize,
}

#[derive(Clone)]
struct HitLine {
    block: usize,
    row: usize,
    source: Range<usize>,
    bounds: Bounds<Pixels>,
    paint_origin: Point<Pixels>,
    layout: WrappedLine,
    map: Option<Vec<usize>>,
    separator: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OpenMenu {
    File,
    Edit,
    Settings,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ThemePreference {
    System,
    Light,
    Dark,
}

impl ThemePreference {
    fn name(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    fn dark(self, appearance: WindowAppearance) -> bool {
        match self {
            Self::System => is_dark(appearance),
            Self::Light => false,
            Self::Dark => true,
        }
    }
}

#[derive(Clone, Copy)]
enum MenuCommand {
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
    ToggleDebugBoxes,
}

struct Editor {
    path: Option<PathBuf>,
    focus: FocusHandle,
    document: DocumentModel,
    selection: Range<usize>,
    reversed: bool,
    marked: Option<Range<usize>>,
    selecting: bool,
    revealed: HashSet<BlockId>,
    shapes: HashMap<BlockId, ShapeCache>,
    hit_lines: Vec<HitLine>,
    doc_bounds: Option<Bounds<Pixels>>,
    vertical_scroll: ScrollHandle,
    horizontal_scroll: ScrollHandle,
    undo: Vec<EditTransaction>,
    redo: Vec<EditTransaction>,
    pending_edits: VecDeque<Instant>,
    synthetic_remaining: usize,
    pending_anchor: Option<(usize, Pixels)>,
    ensure_caret_visible: bool,
    preferred_column: Option<usize>,
    latencies: LatencySamples,
    status: String,
    saved_text: String,
    dirty: bool,
    autosave_generation: u64,
    dark: bool,
    open_menu: Option<OpenMenu>,
    theme: ThemePreference,
    debug_boxes: bool,
    last_debug_geometry: Option<u64>,
    config: AppConfig,
    dot_opacity: f32,
    watcher: Option<RecommendedWatcher>,
    watch_generation: u64,
    conflict_text: Option<String>,
    close_after_save: bool,
}

impl Editor {
    fn run_menu_command(
        &mut self,
        command: MenuCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_menu = None;
        window.focus(&self.focus);
        match command {
            MenuCommand::New => self.new_document_with_prompt(window, cx),
            MenuCommand::NewWindow => self.new_window(),
            MenuCommand::Open => self.open_with_prompt(window, cx),
            MenuCommand::OpenRecent(index) => {
                if let Some(path) = self.config.recent.get(index).cloned() {
                    self.open_path_with_prompt(PathBuf::from(path), window, cx);
                }
            }
            MenuCommand::Save => self.save(&Save, window, cx),
            MenuCommand::SaveAs => self.save_as_dialog(window, cx),
            MenuCommand::Rename => self.rename_dialog(window, cx),
            MenuCommand::Delete => self.delete_with_prompt(window, cx),
            MenuCommand::CopyPath => {
                if let Some(path) = &self.path {
                    cx.write_to_clipboard(ClipboardItem::new_string(
                        path.to_string_lossy().into_owned(),
                    ));
                }
            }
            MenuCommand::OpenLocation => {
                if let Some(path) = &self.path {
                    cx.reveal_path(path);
                }
            }
            MenuCommand::Quit => self.request_close(window, cx),
            MenuCommand::Undo => self.undo(&Undo, window, cx),
            MenuCommand::Redo => self.redo(&Redo, window, cx),
            MenuCommand::Cut => self.cut_impl(cx),
            MenuCommand::Copy => self.copy(&Copy, window, cx),
            MenuCommand::Paste => self.paste(&Paste, window, cx),
            MenuCommand::SelectAll => self.select_all(&SelectAll, window, cx),
            MenuCommand::SetTheme(theme) => {
                self.theme = theme;
                self.dark = theme.dark(window.appearance());
                self.shapes.clear();
                self.config.theme = match theme {
                    ThemePreference::System => None,
                    _ => Some(theme.name().into()),
                };
                if let Err(error) = persistence::save_config(&self.config) {
                    self.status = format!("theme save failed: {error}");
                }
                cx.notify();
            }
            MenuCommand::SetDotOpacity(percent) => {
                self.dot_opacity = (percent as f32 / 100.).clamp(0.0, 0.30);
                self.config.dot_opacity = self.dot_opacity;
                if let Err(error) = persistence::save_config(&self.config) {
                    self.status = format!("appearance save failed: {error}");
                }
                cx.notify();
            }
            MenuCommand::ToggleDebugBoxes => {
                self.debug_boxes = !self.debug_boxes;
                self.last_debug_geometry = None;
                cx.notify();
            }
        }
    }

    fn menu_item(
        id: &'static str,
        label: &'static str,
        shortcut: &'static str,
        command: MenuCommand,
        colors: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(id)
            .h(px(28.))
            .px(px(10.))
            .flex()
            .items_center()
            .justify_between()
            .text_size(px(12.8))
            .text_color(colors.fg)
            .cursor_pointer()
            .hover(move |style| style.bg(alpha(colors.fg, 0.1)))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.run_menu_command(command, window, cx);
            }))
            .child(label)
            .child(div().ml(px(36.)).text_color(colors.fg_dim).child(shortcut))
    }

    fn menu_separator(colors: Palette) -> impl IntoElement {
        div()
            .h(px(9.))
            .mx(px(7.))
            .border_b_1()
            .border_color(colors.border)
    }

    fn recent_menu_item(
        index: usize,
        path: String,
        colors: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let label = Path::new(&path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        div()
            .id(("recent", index))
            .h(px(28.))
            .px(px(10.))
            .flex()
            .items_center()
            .text_size(px(12.8))
            .text_color(colors.fg)
            .cursor_pointer()
            .hover(move |style| style.bg(alpha(colors.fg, 0.1)))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.run_menu_command(MenuCommand::OpenRecent(index), window, cx);
            }))
            .child(label)
    }

    fn theme_menu_item(
        id: &'static str,
        label: &'static str,
        theme: ThemePreference,
        selected: bool,
        colors: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(id)
            .h(px(28.))
            .px(px(10.))
            .flex()
            .items_center()
            .text_size(px(12.8))
            .text_color(colors.fg)
            .cursor_pointer()
            .hover(move |style| style.bg(alpha(colors.fg, 0.1)))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.run_menu_command(MenuCommand::SetTheme(theme), window, cx);
            }))
            .child(
                div()
                    .w(px(22.))
                    .flex_none()
                    .text_color(colors.accent)
                    .child(if selected { "✓" } else { "" }),
            )
            .child(label)
    }

    fn debug_menu_item(
        colors: Palette,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("debug-boxes")
            .h(px(28.))
            .px(px(10.))
            .flex()
            .items_center()
            .text_size(px(12.8))
            .text_color(colors.fg)
            .cursor_pointer()
            .hover(move |style| style.bg(alpha(colors.fg, 0.1)))
            .on_click(cx.listener(|this, _, window, cx| {
                this.run_menu_command(MenuCommand::ToggleDebugBoxes, window, cx);
            }))
            .child(
                div()
                    .w(px(22.))
                    .flex_none()
                    .text_color(colors.accent)
                    .child(if selected { "✓" } else { "" }),
            )
            .child("Debug boxes")
    }

    fn dot_menu_item(
        percent: u8,
        selected: bool,
        colors: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(("dot-opacity", percent as usize))
            .h(px(28.))
            .px(px(10.))
            .flex()
            .items_center()
            .text_size(px(12.8))
            .text_color(colors.fg)
            .cursor_pointer()
            .hover(move |style| style.bg(alpha(colors.fg, 0.1)))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.run_menu_command(MenuCommand::SetDotOpacity(percent), window, cx);
            }))
            .child(
                div()
                    .w(px(22.))
                    .flex_none()
                    .text_color(colors.accent)
                    .child(if selected { "✓" } else { "" }),
            )
            .child(format!("Dots {percent}%"))
    }

    fn menu_dropdown(
        menu: OpenMenu,
        selected_theme: ThemePreference,
        dot_opacity: f32,
        debug_boxes: bool,
        recent: Vec<String>,
        colors: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let recent_items = recent
            .into_iter()
            .take(12)
            .enumerate()
            .map(|(index, path)| Self::recent_menu_item(index, path, colors, cx).into_any_element())
            .collect::<Vec<_>>();
        let dot_items = [0, 6, 12, 18, 24, 30]
            .into_iter()
            .map(|percent| {
                Self::dot_menu_item(
                    percent,
                    (dot_opacity * 100. - percent as f32).abs() < 0.5,
                    colors,
                    cx,
                )
                .into_any_element()
            })
            .collect::<Vec<_>>();
        div()
            .absolute()
            .top(px(33.))
            .left(px(0.))
            .w(px(214.))
            .py(px(4.))
            .bg(colors.bg)
            .border_1()
            .border_color(colors.border)
            .rounded(px(4.))
            .shadow_lg()
            .occlude()
            .flex()
            .flex_col()
            .when(menu == OpenMenu::File, |menu| {
                menu.child(Self::menu_item(
                    "menu-new",
                    "New",
                    "Ctrl+N",
                    MenuCommand::New,
                    colors,
                    cx,
                ))
                .child(Self::menu_item(
                    "menu-new-window",
                    "New Window",
                    "Ctrl+Shift+N",
                    MenuCommand::NewWindow,
                    colors,
                    cx,
                ))
                .child(Self::menu_item(
                    "menu-open",
                    "Open…",
                    "Ctrl+O",
                    MenuCommand::Open,
                    colors,
                    cx,
                ))
                .children(recent_items)
                .child(Self::menu_separator(colors))
                .child(Self::menu_item(
                    "menu-save",
                    "Save",
                    "Ctrl+S",
                    MenuCommand::Save,
                    colors,
                    cx,
                ))
                .child(Self::menu_item(
                    "menu-save-as",
                    "Save As…",
                    "Ctrl+Shift+S",
                    MenuCommand::SaveAs,
                    colors,
                    cx,
                ))
                .child(Self::menu_item(
                    "menu-rename",
                    "Rename…",
                    "",
                    MenuCommand::Rename,
                    colors,
                    cx,
                ))
                .child(Self::menu_item(
                    "menu-delete",
                    "Delete",
                    "",
                    MenuCommand::Delete,
                    colors,
                    cx,
                ))
                .child(Self::menu_separator(colors))
                .child(Self::menu_item(
                    "menu-copy-path",
                    "Copy Path",
                    "",
                    MenuCommand::CopyPath,
                    colors,
                    cx,
                ))
                .child(Self::menu_item(
                    "menu-open-location",
                    "Open File Location",
                    "",
                    MenuCommand::OpenLocation,
                    colors,
                    cx,
                ))
                .child(Self::menu_separator(colors))
                .child(Self::menu_item(
                    "menu-quit",
                    "Quit",
                    "Ctrl+Q",
                    MenuCommand::Quit,
                    colors,
                    cx,
                ))
            })
            .when(menu == OpenMenu::Edit, |menu| {
                menu.child(Self::menu_item(
                    "menu-undo",
                    "Undo",
                    "Ctrl+Z",
                    MenuCommand::Undo,
                    colors,
                    cx,
                ))
                .child(Self::menu_item(
                    "menu-redo",
                    "Redo",
                    "Ctrl+Y",
                    MenuCommand::Redo,
                    colors,
                    cx,
                ))
                .child(Self::menu_separator(colors))
                .child(Self::menu_item(
                    "menu-cut",
                    "Cut",
                    "Ctrl+X",
                    MenuCommand::Cut,
                    colors,
                    cx,
                ))
                .child(Self::menu_item(
                    "menu-copy",
                    "Copy",
                    "Ctrl+C",
                    MenuCommand::Copy,
                    colors,
                    cx,
                ))
                .child(Self::menu_item(
                    "menu-paste",
                    "Paste",
                    "Ctrl+V",
                    MenuCommand::Paste,
                    colors,
                    cx,
                ))
                .child(Self::menu_separator(colors))
                .child(Self::menu_item(
                    "menu-select-all",
                    "Select All",
                    "Ctrl+A",
                    MenuCommand::SelectAll,
                    colors,
                    cx,
                ))
            })
            .when(menu == OpenMenu::Settings, |menu| {
                menu.child(
                    div()
                        .h(px(25.))
                        .px(px(10.))
                        .flex()
                        .items_center()
                        .text_size(px(11.5))
                        .text_color(colors.fg_dim)
                        .child("Appearance"),
                )
                .child(Self::theme_menu_item(
                    "theme-system",
                    "System",
                    ThemePreference::System,
                    selected_theme == ThemePreference::System,
                    colors,
                    cx,
                ))
                .child(Self::theme_menu_item(
                    "theme-light",
                    "Light",
                    ThemePreference::Light,
                    selected_theme == ThemePreference::Light,
                    colors,
                    cx,
                ))
                .child(Self::theme_menu_item(
                    "theme-dark",
                    "Dark",
                    ThemePreference::Dark,
                    selected_theme == ThemePreference::Dark,
                    colors,
                    cx,
                ))
                .child(Self::menu_separator(colors))
                .children(dot_items)
                .child(Self::menu_separator(colors))
                .child(Self::debug_menu_item(colors, debug_boxes, cx))
            })
    }

    fn replace_document(&mut self, path: Option<PathBuf>, content: String, cx: &mut Context<Self>) {
        self.path = path;
        self.document = DocumentModel::new(content.clone());
        self.selection = 0..0;
        self.reversed = false;
        self.marked = None;
        self.revealed = HashSet::from([self.document.blocks[0].id]);
        self.shapes.clear();
        self.hit_lines.clear();
        self.undo.clear();
        self.redo.clear();
        self.saved_text = content;
        self.dirty = false;
        self.autosave_generation += 1;
        self.watch_generation += 1;
        self.watcher = None;
        self.conflict_text = None;
        self.status = "saved".into();
        cx.notify();
    }

    fn new_document(&mut self, cx: &mut Context<Self>) {
        self.replace_document(None, String::new(), cx);
        self.status = "untitled".into();
    }

    fn new_document_with_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.dirty {
            self.new_document(cx);
            return;
        }
        let answer = window.prompt(
            PromptLevel::Warning,
            "Discard unsaved changes and create a new document?",
            None,
            &["Discard", "Cancel"],
            cx,
        );
        cx.spawn(async move |this, cx| {
            if answer.await == Ok(0) {
                let _ = this.update(cx, |editor, cx| editor.new_document(cx));
            }
        })
        .detach();
    }

    fn open_with_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.dirty {
            self.open_dialog(window, cx);
            return;
        }
        let answer = window.prompt(
            PromptLevel::Warning,
            "Discard unsaved changes and open another file?",
            None,
            &["Discard", "Cancel"],
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            if answer.await == Ok(0) {
                let _ = this.update_in(cx, |editor, window, cx| editor.open_dialog(window, cx));
            }
        })
        .detach();
    }

    fn open_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        match fs::read_to_string(&path) {
            Ok(content) => {
                persistence::push_recent(&mut self.config, &path);
                let _ = persistence::save_config(&self.config);
                self.replace_document(Some(path), content, cx);
                self.start_watch(cx);
            }
            Err(error) => self.status = format!("open failed: {error}"),
        }
    }

    fn open_path_with_prompt(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.dirty {
            self.open_path(path, cx);
            return;
        }
        let answer = window.prompt(
            PromptLevel::Warning,
            "Discard unsaved changes and open the recent file?",
            None,
            &["Discard", "Cancel"],
            cx,
        );
        cx.spawn(async move |this, cx| {
            if answer.await == Ok(0) {
                let _ = this.update(cx, |editor, cx| editor.open_path(path, cx));
            }
        })
        .detach();
    }

    fn open_dialog(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let selected = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Open Markdown".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = selected.await
                && let Some(path) = paths.into_iter().next()
            {
                let _ = this.update(cx, |editor, cx| editor.open_path(path, cx));
            }
        })
        .detach();
    }

    fn save_as_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let directory = self
            .path
            .as_deref()
            .and_then(Path::parent)
            .unwrap_or_else(|| Path::new("."));
        let selected = cx.prompt_for_new_path(directory, Some("untitled.md"));
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(path))) = selected.await {
                let _ = this.update_in(cx, |editor, window, cx| {
                    editor.path = Some(path.clone());
                    editor.shapes.clear();
                    persistence::push_recent(&mut editor.config, &path);
                    let _ = persistence::save_config(&editor.config);
                    if let Err(error) = editor.save_now() {
                        editor.status = format!("save failed: {error}");
                    } else {
                        editor.start_watch(cx);
                        if editor.close_after_save {
                            editor.close_after_save = false;
                            window.remove_window();
                        }
                    }
                    cx.notify();
                });
            } else {
                let _ = this.update_in(cx, |editor, _, _| {
                    editor.close_after_save = false;
                });
            }
        })
        .detach();
    }

    fn rename_dialog(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let Some(current) = self.path.clone() else {
            return;
        };
        let directory = current.parent().unwrap_or_else(|| Path::new("."));
        let name = current.file_name().and_then(|name| name.to_str());
        let selected = cx.prompt_for_new_path(directory, name);
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(target))) = selected.await
                && target != current
            {
                let result = if target.exists() {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::AlreadyExists,
                        "target already exists",
                    ))
                } else {
                    fs::rename(&current, &target)
                };
                let _ = this.update(cx, |editor, cx| {
                    match result {
                        Ok(()) => {
                            editor.path = Some(target.clone());
                            editor.shapes.clear();
                            persistence::push_recent(&mut editor.config, &target);
                            let _ = persistence::save_config(&editor.config);
                            editor.status = "renamed".into();
                            editor.start_watch(cx);
                        }
                        Err(error) => editor.status = format!("rename failed: {error}"),
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn delete_with_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.path.clone() else {
            return;
        };
        let answer = window.prompt(
            PromptLevel::Warning,
            &format!("Delete {}? This cannot be undone.", path.display()),
            None,
            &["Delete", "Cancel"],
            cx,
        );
        cx.spawn(async move |this, cx| {
            if answer.await == Ok(0) {
                let result = fs::remove_file(&path);
                let _ = this.update(cx, |editor, cx| match result {
                    Ok(()) => editor.new_document(cx),
                    Err(error) => editor.status = format!("delete failed: {error}"),
                });
            }
        })
        .detach();
    }

    fn new_window(&self) {
        if let Ok(executable) = std::env::current_exe() {
            let _ = std::process::Command::new(executable).arg("--new").spawn();
        }
    }

    fn request_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.dirty {
            window.remove_window();
            return;
        }
        let answer = window.prompt(
            PromptLevel::Warning,
            "Save changes before closing?",
            None,
            &["Save", "Discard", "Cancel"],
            cx,
        );
        cx.spawn_in(window, async move |this, cx| match answer.await {
            Ok(0) => {
                let _ = this.update_in(cx, |editor, window, cx| {
                    if editor.path.is_none() {
                        editor.close_after_save = true;
                        editor.save_as_dialog(window, cx);
                    } else if editor.save_now().is_ok() {
                        window.remove_window();
                    }
                });
            }
            Ok(1) => {
                let _ = this.update_in(cx, |editor, window, _| {
                    editor.dirty = false;
                    window.remove_window();
                });
            }
            _ => {}
        })
        .detach();
    }

    fn quit(&mut self, _: &Quit, window: &mut Window, cx: &mut Context<Self>) {
        self.request_close(window, cx)
    }

    fn start_watch(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.path.clone() else {
            return;
        };
        self.watch_generation += 1;
        let generation = self.watch_generation;
        let (sender, receiver) = async_channel::bounded(1);
        match persistence::watch_file(&path, move || {
            let _ = sender.try_send(());
        }) {
            Ok(watcher) => self.watcher = Some(watcher),
            Err(error) => {
                self.status = format!("watch failed: {error}");
                return;
            }
        }
        cx.spawn(async move |this, cx| {
            while receiver.recv().await.is_ok() {
                let keep_watching = this
                    .update(cx, |editor, cx| {
                        if editor.watch_generation != generation {
                            return false;
                        }
                        let Ok(disk) = fs::read_to_string(&path) else {
                            return true;
                        };
                        if disk == editor.saved_text {
                            return true;
                        }
                        if editor.dirty {
                            editor.conflict_text = Some(disk);
                            editor.status = "file changed on disk — conflict".into();
                        } else {
                            editor.document = DocumentModel::new(disk.clone());
                            editor.saved_text = disk;
                            editor.selection = 0..0;
                            editor.revealed = HashSet::from([editor.document.blocks[0].id]);
                            editor.shapes.clear();
                            editor.undo.clear();
                            editor.redo.clear();
                            editor.status = "reloaded from disk".into();
                        }
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !keep_watching {
                    break;
                }
            }
        })
        .detach();
    }

    fn keep_conflict_mine(&mut self, cx: &mut Context<Self>) {
        self.conflict_text = None;
        if let Err(error) = self.save_now() {
            self.status = format!("save failed: {error}");
        }
        cx.notify();
    }

    fn load_conflict_disk(&mut self, cx: &mut Context<Self>) {
        let Some(disk) = self.conflict_text.take() else {
            return;
        };
        let path = self.path.clone();
        self.replace_document(path, disk, cx);
        self.start_watch(cx);
        self.status = "loaded disk version".into();
    }

    fn cursor(&self) -> usize {
        if self.reversed {
            self.selection.start
        } else {
            self.selection.end
        }
    }
    fn previous(&self, at: usize) -> usize {
        self.document
            .content
            .grapheme_indices(true)
            .rev()
            .find_map(|(i, _)| (i < at).then_some(i))
            .unwrap_or(0)
    }
    fn next(&self, at: usize) -> usize {
        self.document
            .content
            .grapheme_indices(true)
            .find_map(|(i, _)| (i > at).then_some(i))
            .unwrap_or(self.document.content.len())
    }

    fn desired_revealed(&self) -> HashSet<BlockId> {
        let touched = self.document.touched_blocks(&self.selection);
        self.document.blocks[touched].iter().map(|b| b.id).collect()
    }

    fn sync_revealed(&mut self) {
        let next = self.desired_revealed();
        let leaving: Vec<_> = self.revealed.difference(&next).copied().collect();
        for id in leaving {
            if let Some(index) = self.document.blocks.iter().position(|b| b.id == id) {
                self.document.commit_block(index);
                if let Some(cache) = self.shapes.get_mut(&id) {
                    cache.rendered = None;
                    cache.rendered_rows = 0;
                }
            }
        }
        self.revealed = next;
    }

    fn move_to(&mut self, at: usize, cx: &mut Context<Self>) {
        let at = at.min(self.document.content.len());
        self.selection = at..at;
        self.reversed = false;
        self.marked = None;
        self.preferred_column = None;
        self.ensure_caret_visible = true;
        self.sync_revealed();
        cx.notify();
    }
    fn select_to(&mut self, at: usize, cx: &mut Context<Self>) {
        let at = at.min(self.document.content.len());
        if self.reversed {
            self.selection.start = at
        } else {
            self.selection.end = at
        }
        if self.selection.end < self.selection.start {
            self.reversed = !self.reversed;
            self.selection = self.selection.end..self.selection.start;
        }
        self.sync_revealed();
        self.preferred_column = None;
        self.ensure_caret_visible = true;
        cx.notify();
    }

    fn edit(
        &mut self,
        range: Range<usize>,
        text: &str,
        mut mode: EditMode,
        record: bool,
        cx: &mut Context<Self>,
    ) {
        // Source navigation can place the caret in the separator between two
        // semantic blocks. Editing there changes the boundary, so both
        // neighbors must be resegmented. Updating only the preceding range
        // leaves the inserted bytes unowned and causes stale/duplicate glyphs.
        if range.is_empty() {
            let block = self.document.block_at(range.start);
            if range.start > self.document.blocks[block].range.end
                && block + 1 < self.document.blocks.len()
            {
                mode = EditMode::FuseNext;
            }
        }
        let ordinary = mode == EditMode::Ordinary;
        let old_ids: HashSet<_> = self.document.blocks.iter().map(|b| b.id).collect();
        let (tx, invalidated) =
            self.document
                .apply_edit(range, text, mode, self.selection.clone(), self.reversed);
        self.selection = tx.after_selection.clone();
        self.reversed = false;
        self.marked = None;
        self.preferred_column = None;
        self.ensure_caret_visible = true;
        if record {
            self.undo.push(tx);
            if self.undo.len() > 500 {
                self.undo.remove(0);
            }
            self.redo.clear();
        }
        if ordinary {
            for id in invalidated {
                if let Some(cache) = self.shapes.get_mut(&id) {
                    cache.raw = None;
                    cache.raw_rows = 0;
                    cache.raw_separator_rows = 0;
                }
            }
        } else {
            for id in old_ids {
                if !self.document.blocks.iter().any(|b| b.id == id) {
                    self.shapes.remove(&id);
                }
            }
        }
        self.sync_revealed();
        self.pending_edits.push_back(Instant::now());
        self.status = "unsaved".into();
        self.dirty = true;
        self.schedule_autosave(cx);
        cx.notify();
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        let p = if self.selection.is_empty() {
            self.previous(self.cursor())
        } else {
            self.selection.start
        };
        self.move_to(p, cx)
    }
    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        let p = if self.selection.is_empty() {
            self.next(self.cursor())
        } else {
            self.selection.end
        };
        self.move_to(p, cx)
    }
    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous(self.cursor()), cx)
    }
    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next(self.cursor()), cx)
    }
    fn vertical(&mut self, dir: isize) -> usize {
        let c = self.cursor();
        let current_block = self.document.block_at(c);
        let start = self.document.content[..c].rfind('\n').map_or(0, |i| i + 1);
        let col = self
            .preferred_column
            .unwrap_or_else(|| self.document.content[start..c].graphemes(true).count());
        self.preferred_column = Some(col);
        let target = if dir < 0 {
            if start == 0 {
                return 0;
            }
            let end = start - 1;
            let s = self.document.content[..end]
                .rfind('\n')
                .map_or(0, |i| i + 1);
            s + grapheme_offset(&self.document.content[s..end], col)
        } else {
            let Some(s) = self.document.content[c..].find('\n').map(|i| c + i + 1) else {
                return self.document.content.len();
            };
            let e = self.document.content[s..]
                .find('\n')
                .map_or(self.document.content.len(), |i| s + i);
            s + grapheme_offset(&self.document.content[s..e], col)
        };
        let target_block = self.document.block_at(target);
        let target_is_separator = target > self.document.blocks[target_block].range.end;
        if target_is_separator {
            // Blank source lines are structural whitespace rather than a
            // semantic block crossing. Both Up and Down must land on the same
            // zero-column caret stop before entering either neighbour.
            self.preferred_column = Some(0);
            target
        } else if target_block != current_block {
            self.preferred_column = Some(0);
            self.document.blocks[target_block].range.start
        } else {
            target
        }
    }
    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        let column = self.preferred_column;
        let target = self.vertical(-1);
        let desired = self.preferred_column.or(column);
        self.move_to(target, cx);
        self.preferred_column = desired;
    }
    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        let column = self.preferred_column;
        let target = self.vertical(1);
        let desired = self.preferred_column.or(column);
        self.move_to(target, cx);
        self.preferred_column = desired;
    }
    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        let target = self.vertical(-1);
        let desired = self.preferred_column;
        self.select_to(target, cx);
        self.preferred_column = desired;
    }
    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        let target = self.vertical(1);
        let desired = self.preferred_column;
        self.select_to(target, cx);
        self.preferred_column = desired;
    }
    fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        let target = if self.selection.is_empty() {
            previous_word_boundary(&self.document.content, self.cursor())
        } else {
            self.selection.start
        };
        self.move_to(target, cx)
    }
    fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        let target = if self.selection.is_empty() {
            next_word_boundary(&self.document.content, self.cursor())
        } else {
            self.selection.end
        };
        self.move_to(target, cx)
    }
    fn select_word_left(&mut self, _: &SelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(
            previous_word_boundary(&self.document.content, self.cursor()),
            cx,
        )
    }
    fn select_word_right(&mut self, _: &SelectWordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(
            next_word_boundary(&self.document.content, self.cursor()),
            cx,
        )
    }
    fn delete_word_backward(
        &mut self,
        _: &DeleteWordBackward,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = if self.selection.is_empty() {
            previous_word_boundary(&self.document.content, self.cursor())..self.cursor()
        } else {
            self.selection.clone()
        };
        if range.is_empty() {
            return;
        }
        let mode = if self.document.content[range.clone()].contains('\n')
            || self.document.touched_blocks(&range).len() > 1
        {
            EditMode::CrossBlock
        } else {
            EditMode::Ordinary
        };
        self.edit(range, "", mode, true, cx)
    }
    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        let p = self.document.content[..self.cursor()]
            .rfind('\n')
            .map_or(0, |i| i + 1);
        self.move_to(p, cx)
    }
    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        let c = self.cursor();
        let p = self.document.content[c..]
            .find('\n')
            .map_or(self.document.content.len(), |i| c + i);
        self.move_to(p, cx)
    }
    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.selection = 0..self.document.content.len();
        self.reversed = false;
        self.sync_revealed();
        cx.notify()
    }
    fn enter(&mut self, _: &Enter, _: &mut Window, cx: &mut Context<Self>) {
        self.edit(self.selection.clone(), "\n", EditMode::Enter, true, cx)
    }
    fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        let c = self.cursor();
        let block = self.document.block_at(c);
        let mode = if self.selection.is_empty()
            && block > 0
            && c == self.document.blocks[block].range.start
        {
            EditMode::FusePrevious
        } else {
            EditMode::Ordinary
        };
        let r = if self.selection.is_empty() {
            self.previous(c)..c
        } else {
            self.selection.clone()
        };
        if !r.is_empty() {
            self.edit(r, "", mode, true, cx)
        }
    }
    fn delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        let c = self.cursor();
        let block = self.document.block_at(c);
        let mode = if self.selection.is_empty()
            && block + 1 < self.document.blocks.len()
            && c == self.document.blocks[block].range.end
        {
            EditMode::FuseNext
        } else {
            EditMode::Ordinary
        };
        let r = if self.selection.is_empty() {
            c..self.next(c)
        } else {
            self.selection.clone()
        };
        if !r.is_empty() {
            self.edit(r, "", mode, true, cx)
        }
    }
    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selection.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.document.content[self.selection.clone()].to_string(),
            ))
        }
    }
    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = cx.read_from_clipboard() else {
            return;
        };
        if let Some(image) = item.entries().iter().find_map(|entry| match entry {
            ClipboardEntry::Image(image) => Some(image),
            ClipboardEntry::String(_) => None,
        }) {
            match persistence::save_pasted_image(image.bytes(), image_extension(image.format())) {
                Ok(path) => {
                    let path = path.to_string_lossy();
                    let markdown = if path.chars().any(char::is_whitespace) {
                        format!("![](<{path}>)")
                    } else {
                        format!("![]({path})")
                    };
                    self.edit(
                        self.selection.clone(),
                        &markdown,
                        EditMode::CrossBlock,
                        true,
                        cx,
                    );
                }
                Err(error) => {
                    self.status = format!("image paste failed: {error}");
                    cx.notify();
                }
            }
        } else if let Some(t) = item.text() {
            let mode =
                if t.contains('\n') || self.document.touched_blocks(&self.selection).len() > 1 {
                    EditMode::CrossBlock
                } else {
                    EditMode::Ordinary
                };
            self.edit(self.selection.clone(), &t, mode, true, cx)
        }
    }
    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(tx) = self.undo.pop() {
            self.document.apply_inverse(&tx);
            self.selection = tx.before_selection.clone();
            self.reversed = tx.before_reversed;
            self.redo.push(tx);
            self.shapes.clear();
            self.sync_revealed();
            self.ensure_caret_visible = true;
            self.preferred_column = None;
            cx.notify()
        }
    }
    fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(tx) = self.redo.pop() {
            self.document.apply_forward(&tx);
            self.selection = tx.after_selection.clone();
            self.reversed = tx.after_reversed;
            self.undo.push(tx);
            self.shapes.clear();
            self.sync_revealed();
            self.ensure_caret_visible = true;
            self.preferred_column = None;
            cx.notify()
        }
    }
    fn save_now(&mut self) -> Result<(), String> {
        let path = self.path.as_ref().ok_or_else(|| "untitled".to_string())?;
        persistence::atomic_write(path, &self.document.content)?;
        self.saved_text = self.document.content.clone();
        self.dirty = false;
        self.status = "saved".into();
        Ok(())
    }
    fn schedule_autosave(&mut self, cx: &mut Context<Self>) {
        self.autosave_generation += 1;
        if self.path.is_none() {
            return;
        }
        let generation = self.autosave_generation;
        let timer = cx.background_executor().timer(Duration::from_secs(5));
        cx.spawn(async move |this, cx| {
            timer.await;
            let _ = this.update(cx, |editor, cx| {
                if editor.autosave_generation == generation && editor.dirty {
                    if let Err(error) = editor.save_now() {
                        editor.status = format!("autosave failed: {error}");
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }
    fn save(&mut self, _: &Save, window: &mut Window, cx: &mut Context<Self>) {
        if self.path.is_none() {
            self.save_as_dialog(window, cx);
            return;
        }
        if let Err(error) = self.save_now() {
            self.status = format!("save failed: {error}");
        }
        cx.notify()
    }
    fn escape(&mut self, _: &Escape, window: &mut Window, cx: &mut Context<Self>) {
        let anchor = self.cursor();
        let old_block = self.document.block_at(anchor);
        if let Some(line) = self.hit_lines.iter().find(|line| line.block == old_block) {
            self.pending_anchor = Some((anchor, line.bounds.top()));
        }
        self.document.global_reparse();
        self.shapes.clear();
        self.revealed.clear();
        self.selection =
            anchor.min(self.document.content.len())..anchor.min(self.document.content.len());
        window.blur();
        cx.notify()
    }

    fn toggle(&mut self, mark: &str, cx: &mut Context<Self>) {
        let len = mark.len();
        let r = self.selection.clone();
        if r.start >= len
            && self.document.content.get(r.start - len..r.start) == Some(mark)
            && self.document.content.get(r.end..r.end + len) == Some(mark)
        {
            let from = r.start - len;
            let to = r.end + len;
            let text = self.document.content[r.clone()].to_string();
            self.edit(from..to, &text, EditMode::Ordinary, true, cx);
            self.selection = from..from + text.len()
        } else {
            let text = format!("{mark}{}{mark}", &self.document.content[r.clone()]);
            let from = r.start;
            self.edit(r, &text, EditMode::Ordinary, true, cx);
            self.selection = from + len..from + text.len() - len
        }
        self.sync_revealed();
        cx.notify()
    }
    fn bold(&mut self, _: &Bold, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle("**", cx)
    }
    fn italic(&mut self, _: &Italic, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle("*", cx)
    }
    fn inline_code(&mut self, _: &InlineCode, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle("`", cx)
    }
    fn link(&mut self, _: &Link, _: &mut Window, cx: &mut Context<Self>) {
        let start = self.selection.start;
        let text = format!("[{}]()", &self.document.content[self.selection.clone()]);
        self.edit(self.selection.clone(), &text, EditMode::Ordinary, true, cx);
        let caret = start + text.len() - 1;
        self.selection = caret..caret;
        self.sync_revealed();
        cx.notify()
    }

    fn index_at(&self, p: Point<Pixels>) -> usize {
        if let Some(line) = self
            .hit_lines
            .iter()
            .find(|line| p.y >= line.bounds.top() && p.y <= line.bounds.bottom())
        {
            return source_offset_for_hit(line, p);
        }

        for pair in self.hit_lines.windows(2) {
            let before = &pair[0];
            let after = &pair[1];
            if p.y > before.bounds.bottom() && p.y < after.bounds.top() {
                if before.block != after.block {
                    return self.document.blocks[before.block].range.end;
                }
                return if p.y - before.bounds.bottom() < after.bounds.top() - p.y {
                    source_offset_for_hit(before, p)
                } else {
                    source_offset_for_hit(after, p)
                };
            }
        }

        self.hit_lines
            .iter()
            .min_by(|a, b| {
                vertical_distance(a.bounds, p.y).total_cmp(&vertical_distance(b.bounds, p.y))
            })
            .map_or(self.cursor(), |line| source_offset_for_hit(line, p))
    }
    fn mouse_down(&mut self, e: &MouseDownEvent, w: &mut Window, cx: &mut Context<Self>) {
        w.focus(&self.focus);
        self.selecting = true;
        let source = self.index_at(e.position);
        if self.debug_boxes {
            let hits = self
                .hit_lines
                .iter()
                .filter(|line| {
                    e.position.y >= line.bounds.top() && e.position.y <= line.bounds.bottom()
                })
                .collect::<Vec<_>>();
            if let Some(winner) = hits.first() {
                eprintln!(
                    "DEBUG_CLICK x={:.2} y={:.2} hits={} winner={} block={} row={} source={}..{} offset={} top={:.2} bottom={:.2}",
                    f32::from(e.position.x),
                    f32::from(e.position.y),
                    hits.len(),
                    if winner.separator { "blank" } else { "line" },
                    winner.block,
                    winner.row,
                    winner.source.start,
                    winner.source.end,
                    source,
                    f32::from(winner.bounds.top()),
                    f32::from(winner.bounds.bottom()),
                );
            } else {
                eprintln!(
                    "DEBUG_CLICK x={:.2} y={:.2} hits=0 winner=fallback offset={}",
                    f32::from(e.position.x),
                    f32::from(e.position.y),
                    source,
                );
            }
        }
        if e.modifiers.shift {
            self.select_to(source, cx)
        } else {
            self.move_to(source, cx)
        }
    }
    fn mouse_move(&mut self, e: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.selecting {
            self.select_to(self.index_at(e.position), cx)
        }
    }
    fn mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.selecting = false
    }

    fn ensure_shapes(&mut self, window: &mut Window, cx: &mut App) {
        let revealed = self.revealed.clone();
        let caret = self.cursor();
        let palette = palette(self.dark, self.dot_opacity);
        let document_path = self.path.clone();
        for (i, block) in self.document.blocks.iter().enumerate() {
            let editing_lines = self.document.editing_lines(i, caret);
            let cache = self.shapes.entry(block.id).or_default();
            if cache.rendered.is_none() {
                let lines = block
                    .rendered
                    .iter()
                    .map(|l| shape_render(l, document_path.as_deref(), palette, window))
                    .collect::<Vec<_>>();
                cache.rendered_rows = rows(&lines);
                cache.rendered = Some(lines);
                self.document.counters.rendered_reshapes += 1
            }
            if let Some(lines) = cache.rendered.as_mut() {
                for line in lines.iter_mut() {
                    let Some(image) = line.image.as_mut() else {
                        continue;
                    };
                    match window.use_asset::<ImgResourceLoader>(&image.resource, cx) {
                        Some(Ok(data)) => {
                            image.rows = image_allocation_rows(&data);
                            image.data = Some(data);
                            image.failed = false;
                        }
                        Some(Err(_)) => {
                            image.rows = 2;
                            image.data = None;
                            image.failed = true;
                        }
                        None => {}
                    }
                }
                cache.rendered_rows = rows(lines);
            }
            let raw_is_stale = cache.raw.as_ref().is_some_and(|lines| {
                lines.len() != editing_lines.len()
                    || lines.iter().zip(&editing_lines).any(|(line, range)| {
                        line.source != *range
                            || line.layout.text.as_ref() != self.document.raw_line(range)
                    })
            });
            if raw_is_stale {
                cache.raw = None;
                cache.raw_rows = 0;
                cache.raw_separator_rows = 0;
            }
            if revealed.contains(&block.id) && cache.raw.is_none() {
                let lines = editing_lines
                    .iter()
                    .map(|r| {
                        let code_line = source_line_is_code(block.kind, block.range.end, r.start);
                        shape_raw(
                            r.clone(),
                            self.document.raw_line(r),
                            code_line,
                            palette,
                            window,
                        )
                    })
                    .collect::<Vec<_>>();
                cache.raw_rows = rows(&lines);
                cache.raw_separator_rows =
                    editing_lines.len().saturating_sub(block.raw_lines.len());
                cache.raw = Some(lines);
                self.document.counters.raw_reshapes += 1
            }
            let _ = i;
        }
    }

    fn synthetic_edit(&mut self) {
        let cursor = self.cursor();
        let (_, invalidated) = self.document.apply_edit(
            cursor..cursor,
            "x",
            EditMode::Ordinary,
            self.selection.clone(),
            self.reversed,
        );
        self.selection = cursor + 1..cursor + 1;
        for id in invalidated {
            if let Some(cache) = self.shapes.get_mut(&id) {
                cache.raw = None;
                cache.raw_rows = 0;
                cache.raw_separator_rows = 0;
            }
        }
        self.pending_edits.push_back(Instant::now());
    }
    fn total_rows(&self) -> usize {
        self.document
            .blocks
            .iter()
            .enumerate()
            .map(|(index, b)| {
                let c = &self.shapes[&b.id];
                let separator_rows = self.document.separator_rows_after(index);
                let semantic_raw_rows = c.raw_rows.saturating_sub(c.raw_separator_rows);
                semantic_raw_rows.max(c.rendered_rows).max(1) + separator_rows
            })
            .sum()
    }
}

fn source_offset_for_hit(line: &HitLine, position: Point<Pixels>) -> usize {
    if line.separator {
        return line.source.start;
    }
    let local = point(
        position.x - line.paint_origin.x,
        position.y - line.paint_origin.y,
    );
    let display = line
        .layout
        .closest_index_for_position(local, px(GRID))
        .unwrap_or_else(|index| index);
    if let Some(map) = &line.map {
        map.get(display.min(map.len().saturating_sub(1)))
            .copied()
            .unwrap_or(line.source.start)
    } else {
        line.source.start + display.min(line.source.len())
    }
}

fn vertical_distance(bounds: Bounds<Pixels>, y: Pixels) -> f32 {
    if y < bounds.top() {
        (bounds.top() - y).into()
    } else if y > bounds.bottom() {
        (y - bounds.bottom()).into()
    } else {
        0.
    }
}

fn paint_outline(window: &mut Window, bounds: Bounds<Pixels>, color: Hsla) {
    let stroke = px(1.);
    window.paint_quad(fill(
        Bounds::new(bounds.origin, size(bounds.size.width, stroke)),
        color,
    ));
    window.paint_quad(fill(
        Bounds::new(
            point(bounds.left(), bounds.bottom() - stroke),
            size(bounds.size.width, stroke),
        ),
        color,
    ));
    window.paint_quad(fill(
        Bounds::new(bounds.origin, size(stroke, bounds.size.height)),
        color,
    ));
    window.paint_quad(fill(
        Bounds::new(
            point(bounds.right() - stroke, bounds.top()),
            size(stroke, bounds.size.height),
        ),
        color,
    ));
}

fn hash_bounds(hasher: &mut DefaultHasher, bounds: Bounds<Pixels>) {
    f32::from(bounds.left()).to_bits().hash(hasher);
    f32::from(bounds.top()).to_bits().hash(hasher);
    f32::from(bounds.right()).to_bits().hash(hasher);
    f32::from(bounds.bottom()).to_bits().hash(hasher);
}

fn debug_geometry_signature(bounds: Bounds<Pixels>, prepared: &Prepared) -> u64 {
    let mut hasher = DefaultHasher::new();
    hash_bounds(&mut hasher, bounds);
    for (block, bounds) in &prepared.block_boxes {
        block.hash(&mut hasher);
        hash_bounds(&mut hasher, *bounds);
    }
    for (block, bounds) in &prepared.code_slabs {
        block.hash(&mut hasher);
        hash_bounds(&mut hasher, *bounds);
    }
    for line in &prepared.lines {
        line.block.hash(&mut hasher);
        line.row.hash(&mut hasher);
        line.source.start.hash(&mut hasher);
        line.source.end.hash(&mut hasher);
        line.separator.hash(&mut hasher);
        hash_bounds(&mut hasher, line.bounds);
        f32::from(line.paint_origin.x).to_bits().hash(&mut hasher);
        f32::from(line.paint_origin.y).to_bits().hash(&mut hasher);
    }
    hasher.finish()
}

fn overlap_size(a: Bounds<Pixels>, b: Bounds<Pixels>) -> Option<(f32, f32)> {
    let width = f32::from(a.right().min(b.right()) - a.left().max(b.left()));
    let height = f32::from(a.bottom().min(b.bottom()) - a.top().max(b.top()));
    (width > 0.01 && height > 0.01).then_some((width, height))
}

fn log_debug_geometry(signature: u64, prepared: &Prepared) {
    let mut overlaps = 0usize;
    for (index, a) in prepared.lines.iter().enumerate() {
        for b in &prepared.lines[index + 1..] {
            let Some((width, height)) = overlap_size(a.bounds, b.bounds) else {
                continue;
            };
            overlaps += 1;
            eprintln!(
                "DEBUG_OVERLAP layer=line width={width:.2} height={height:.2} a_kind={} a_block={} a_row={} a_source={}..{} a_top={:.2} a_bottom={:.2} b_kind={} b_block={} b_row={} b_source={}..{} b_top={:.2} b_bottom={:.2}",
                if a.separator { "blank" } else { "line" },
                a.block,
                a.row,
                a.source.start,
                a.source.end,
                f32::from(a.bounds.top()),
                f32::from(a.bounds.bottom()),
                if b.separator { "blank" } else { "line" },
                b.block,
                b.row,
                b.source.start,
                b.source.end,
                f32::from(b.bounds.top()),
                f32::from(b.bounds.bottom()),
            );
        }
    }
    for (index, (a_block, a)) in prepared.block_boxes.iter().enumerate() {
        for (b_block, b) in &prepared.block_boxes[index + 1..] {
            let Some((width, height)) = overlap_size(*a, *b) else {
                continue;
            };
            overlaps += 1;
            eprintln!(
                "DEBUG_OVERLAP layer=block width={width:.2} height={height:.2} a_block={a_block} a_top={:.2} a_bottom={:.2} b_block={b_block} b_top={:.2} b_bottom={:.2}",
                f32::from(a.top()),
                f32::from(a.bottom()),
                f32::from(b.top()),
                f32::from(b.bottom()),
            );
        }
    }
    for (index, (a_block, a)) in prepared.code_slabs.iter().enumerate() {
        for (b_block, b) in &prepared.code_slabs[index + 1..] {
            let Some((width, height)) = overlap_size(*a, *b) else {
                continue;
            };
            overlaps += 1;
            eprintln!(
                "DEBUG_OVERLAP layer=code width={width:.2} height={height:.2} a_block={a_block} a_top={:.2} a_bottom={:.2} b_block={b_block} b_top={:.2} b_bottom={:.2}",
                f32::from(a.top()),
                f32::from(a.bottom()),
                f32::from(b.top()),
                f32::from(b.bottom()),
            );
        }
    }
    eprintln!(
        "DEBUG_GEOMETRY signature={signature:016x} blocks={} lines={} blanks={} code_slabs={} overlaps={overlaps}",
        prepared.block_boxes.len(),
        prepared.lines.len(),
        prepared.lines.iter().filter(|line| line.separator).count(),
        prepared.code_slabs.len(),
    );
}

// Window::dummy does not exist; keep cut explicit instead of sharing the action handler.
impl Editor {
    fn cut_impl(&mut self, cx: &mut Context<Self>) {
        if !self.selection.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.document.content[self.selection.clone()].to_string(),
            ));
            let mode = if self.document.touched_blocks(&self.selection).len() > 1 {
                EditMode::CrossBlock
            } else {
                EditMode::Ordinary
            };
            self.edit(self.selection.clone(), "", mode, true, cx)
        }
    }
}

impl EntityInputHandler for Editor {
    fn text_for_range(
        &mut self,
        r: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let n = utf16_to_utf8(&self.document.content, r.start)
            ..utf16_to_utf8(&self.document.content, r.end);
        *actual = Some(
            utf8_to_utf16(&self.document.content, n.start)
                ..utf8_to_utf16(&self.document.content, n.end),
        );
        Some(self.document.content[n].into())
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: utf8_to_utf16(&self.document.content, self.selection.start)
                ..utf8_to_utf16(&self.document.content, self.selection.end),
            reversed: self.reversed,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.as_ref().map(|r| {
            utf8_to_utf16(&self.document.content, r.start)
                ..utf8_to_utf16(&self.document.content, r.end)
        })
    }
    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked = None
    }
    fn replace_text_in_range(
        &mut self,
        r: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let n = r
            .map(|r| {
                utf16_to_utf8(&self.document.content, r.start)
                    ..utf16_to_utf8(&self.document.content, r.end)
            })
            .or(self.marked.clone())
            .unwrap_or(self.selection.clone());
        let mode = if text.contains('\n') || self.document.touched_blocks(&n).len() > 1 {
            EditMode::CrossBlock
        } else {
            EditMode::Ordinary
        };
        self.edit(n, text, mode, true, cx)
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        r: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let n = r
            .map(|r| {
                utf16_to_utf8(&self.document.content, r.start)
                    ..utf16_to_utf8(&self.document.content, r.end)
            })
            .or(self.marked.clone())
            .unwrap_or(self.selection.clone());
        let start = n.start;
        self.edit(n, text, EditMode::Ordinary, true, cx);
        self.marked = (!text.is_empty()).then_some(start..start + text.len());
        let relative = selected
            .map(|r| utf16_to_utf8(text, r.start)..utf16_to_utf8(text, r.end))
            .unwrap_or(text.len()..text.len());
        self.selection = start + relative.start..start + relative.end
    }
    fn bounds_for_range(
        &mut self,
        r: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let n = utf16_to_utf8(&self.document.content, r.start);
        let line = self
            .hit_lines
            .iter()
            .find(|l| l.map.is_none() && l.source.start <= n && l.source.end >= n)?;
        let p = line
            .layout
            .position_for_index(n - line.source.start, px(GRID))?;
        let caret_top = if line.layout.text.is_empty() {
            line.bounds.top()
        } else {
            line.paint_origin.y + p.y
        };
        Some(Bounds::new(
            point(line.paint_origin.x + p.x, caret_top),
            size(px(1.), px(GRID)),
        ))
    }
    fn character_index_for_point(
        &mut self,
        p: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(utf8_to_utf16(&self.document.content, self.index_at(p)))
    }
}

impl Focusable for Editor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dark = self.theme.dark(window.appearance());
        if self.dark != dark {
            self.dark = dark;
            self.shapes.clear();
        }
        let colors = palette(dark, self.dot_opacity);
        let document_margin =
            ((window.viewport_size().width - px(DOCUMENT_WIDTH)) / 2.).max(px(0.));
        let filename = self
            .path
            .as_deref()
            .and_then(Path::file_name)
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "untitled.md".into());
        div()
            .size_full()
            .font(font(PROSE_FONT))
            .text_color(colors.fg)
            .bg(colors.bg)
            .border_2()
            .border_color(colors.border)
            .flex()
            .flex_col()
            .key_context("NativeMarkdownEditor")
            .track_focus(&self.focus)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.open_menu.take().is_some() {
                        cx.notify();
                    }
                }),
            )
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::word_left))
            .on_action(cx.listener(Self::word_right))
            .on_action(cx.listener(Self::select_word_left))
            .on_action(cx.listener(Self::select_word_right))
            .on_action(cx.listener(Self::delete_word_backward))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::enter))
            .on_action(cx.listener(Self::escape))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(|this: &mut Self, _: &Cut, _w, cx| this.cut_impl(cx)))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_action(cx.listener(Self::save))
            .on_action(cx.listener(|this: &mut Self, _: &NewDocument, window, cx| {
                this.new_document_with_prompt(window, cx)
            }))
            .on_action(cx.listener(|this: &mut Self, _: &NewWindow, _, _| this.new_window()))
            .on_action(
                cx.listener(|this: &mut Self, _: &OpenDocument, window, cx| {
                    this.open_with_prompt(window, cx)
                }),
            )
            .on_action(cx.listener(|this: &mut Self, _: &SaveAs, window, cx| {
                this.save_as_dialog(window, cx)
            }))
            .on_action(cx.listener(Self::bold))
            .on_action(cx.listener(Self::italic))
            .on_action(cx.listener(Self::inline_code))
            .on_action(cx.listener(Self::link))
            .on_action(cx.listener(Self::quit))
            .child(
                div()
                    .h(px(34.))
                    .flex_none()
                    .border_b_1()
                    .border_color(colors.border)
                    .flex()
                    .items_center()
                    .bg(colors.bg_subtle)
                    .text_size(px(12.8))
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .flex()
                            .child(
                                div()
                                    .w(px(34.))
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(colors.accent)
                                    .child("●"),
                            )
                            .child(
                                div()
                                    .relative()
                                    .h_full()
                                    .child(
                                        div()
                                            .id("file-menu-button")
                                            .h_full()
                                            .px(px(11.))
                                            .flex()
                                            .items_center()
                                            .cursor_pointer()
                                            .when(
                                                self.open_menu == Some(OpenMenu::File),
                                                |button| button.bg(alpha(colors.fg, 0.1)),
                                            )
                                            .hover(move |button| button.bg(alpha(colors.fg, 0.1)))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.open_menu =
                                                    if this.open_menu == Some(OpenMenu::File) {
                                                        None
                                                    } else {
                                                        Some(OpenMenu::File)
                                                    };
                                                cx.notify();
                                            }))
                                            .child("File"),
                                    )
                                    .when(self.open_menu == Some(OpenMenu::File), |slot| {
                                        slot.child(
                                            deferred(Self::menu_dropdown(
                                                OpenMenu::File,
                                                self.theme,
                                                self.dot_opacity,
                                                self.debug_boxes,
                                                self.config.recent.clone(),
                                                colors,
                                                cx,
                                            ))
                                            .with_priority(10),
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .relative()
                                    .h_full()
                                    .child(
                                        div()
                                            .id("edit-menu-button")
                                            .h_full()
                                            .px(px(11.))
                                            .flex()
                                            .items_center()
                                            .cursor_pointer()
                                            .when(
                                                self.open_menu == Some(OpenMenu::Edit),
                                                |button| button.bg(alpha(colors.fg, 0.1)),
                                            )
                                            .hover(move |button| button.bg(alpha(colors.fg, 0.1)))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.open_menu =
                                                    if this.open_menu == Some(OpenMenu::Edit) {
                                                        None
                                                    } else {
                                                        Some(OpenMenu::Edit)
                                                    };
                                                cx.notify();
                                            }))
                                            .child("Edit"),
                                    )
                                    .when(self.open_menu == Some(OpenMenu::Edit), |slot| {
                                        slot.child(
                                            deferred(Self::menu_dropdown(
                                                OpenMenu::Edit,
                                                self.theme,
                                                self.dot_opacity,
                                                self.debug_boxes,
                                                self.config.recent.clone(),
                                                colors,
                                                cx,
                                            ))
                                            .with_priority(10),
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .relative()
                                    .h_full()
                                    .child(
                                        div()
                                            .id("settings-menu-button")
                                            .h_full()
                                            .px(px(11.))
                                            .flex()
                                            .items_center()
                                            .cursor_pointer()
                                            .when(
                                                self.open_menu == Some(OpenMenu::Settings),
                                                |button| button.bg(alpha(colors.fg, 0.1)),
                                            )
                                            .hover(move |button| button.bg(alpha(colors.fg, 0.1)))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.open_menu =
                                                    if this.open_menu == Some(OpenMenu::Settings) {
                                                        None
                                                    } else {
                                                        Some(OpenMenu::Settings)
                                                    };
                                                cx.notify();
                                            }))
                                            .child("Settings"),
                                    )
                                    .when(self.open_menu == Some(OpenMenu::Settings), |slot| {
                                        slot.child(
                                            deferred(Self::menu_dropdown(
                                                OpenMenu::Settings,
                                                self.theme,
                                                self.dot_opacity,
                                                self.debug_boxes,
                                                self.config.recent.clone(),
                                                colors,
                                                cx,
                                            ))
                                            .with_priority(10),
                                        )
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(colors.fg_dim)
                            .on_mouse_down(MouseButton::Left, |_, window, _| {
                                window.start_window_move();
                            })
                            .child(filename.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .flex()
                            .justify_end()
                            .child(
                                div()
                                    .id("window-minimize")
                                    .w(px(44.))
                                    .h_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .text_color(colors.fg_dim)
                                    .hover(move |button| button.bg(alpha(colors.fg, 0.1)))
                                    .on_click(|_, window, _| window.minimize_window())
                                    .child("−"),
                            )
                            .child(
                                div()
                                    .id("window-maximize")
                                    .w(px(44.))
                                    .h_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .text_color(colors.fg_dim)
                                    .hover(move |button| button.bg(alpha(colors.fg, 0.1)))
                                    .on_click(|_, window, _| window.zoom_window())
                                    .child("□"),
                            )
                            .child(
                                div()
                                    .id("window-close")
                                    .w(px(44.))
                                    .h_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .text_color(colors.fg_dim)
                                    .hover(|button| {
                                        button.bg(color(0xcf222e)).text_color(color(0xffffff))
                                    })
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.request_close(window, cx)
                                    }))
                                    .child("×"),
                            ),
                    ),
            )
            .child(
                div()
                    .id("document-scroll")
                    .flex_1()
                    .min_h(px(0.))
                    .min_w(px(0.))
                    .overflow_y_scroll()
                    .track_scroll(&self.vertical_scroll)
                    .bg(colors.bg_subtle)
                    .child(
                        div()
                            .id("document-horizontal-scroll")
                            .w_full()
                            .overflow_x_scroll()
                            .track_scroll(&self.horizontal_scroll)
                            .child(
                                div()
                                    .flex_none()
                                    .w(px(DOCUMENT_WIDTH))
                                    .min_w(px(DOCUMENT_WIDTH))
                                    .ml(document_margin)
                                    .bg(colors.bg)
                                    .border_l_1()
                                    .border_r_1()
                                    .border_color(colors.border)
                                    .shadow(vec![BoxShadow {
                                        color: alpha(colors.fg, 0.12),
                                        offset: point(px(0.), px(2.)),
                                        blur_radius: px(12.),
                                        spread_radius: px(0.),
                                    }])
                                    .cursor(CursorStyle::IBeam)
                                    .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
                                    .on_mouse_move(cx.listener(Self::mouse_move))
                                    .on_mouse_up(MouseButton::Left, cx.listener(Self::mouse_up))
                                    .on_mouse_up_out(MouseButton::Left, cx.listener(Self::mouse_up))
                                    .child(DocumentElement {
                                        editor: cx.entity(),
                                    }),
                            ),
                    ),
            )
            .child(
                div()
                    .h(px(25.))
                    .flex_none()
                    .px(px(14.))
                    .border_t_1()
                    .border_color(colors.border)
                    .bg(colors.bg_subtle)
                    .flex()
                    .items_center()
                    .text_size(px(12.5))
                    .text_color(colors.fg_dim)
                    .child(div().flex_1().text_color(colors.fg).child(filename))
                    .when(self.debug_boxes, |bar| {
                        bar.child(div().mr(px(18.)).child(
                            "debug: page red · block blue · line pink · blank orange · code green",
                        ))
                    })
                    .child(self.status.clone()),
            )
            .when(self.conflict_text.is_some(), |root| {
                root.child(
                    deferred(
                        div()
                            .absolute()
                            .top(px(110.))
                            .left(px(300.))
                            .w(px(400.))
                            .p(px(18.))
                            .rounded(px(6.))
                            .border_1()
                            .border_color(colors.border)
                            .bg(colors.bg)
                            .shadow_lg()
                            .flex()
                            .flex_col()
                            .gap(px(14.))
                            .child("This file changed on disk while you had unsaved edits.")
                            .child(
                                div()
                                    .flex()
                                    .justify_end()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .id("conflict-load")
                                            .px(px(12.))
                                            .py(px(6.))
                                            .border_1()
                                            .border_color(colors.border)
                                            .rounded(px(4.))
                                            .cursor_pointer()
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.load_conflict_disk(cx)
                                            }))
                                            .child("Load Disk"),
                                    )
                                    .child(
                                        div()
                                            .id("conflict-keep")
                                            .px(px(12.))
                                            .py(px(6.))
                                            .bg(colors.accent)
                                            .text_color(colors.bg)
                                            .rounded(px(4.))
                                            .cursor_pointer()
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.keep_conflict_mine(cx)
                                            }))
                                            .child("Keep Mine"),
                                    ),
                            ),
                    )
                    .with_priority(20),
                )
            })
    }
}

struct DocumentElement {
    editor: Entity<Editor>,
}
struct Prepared {
    lines: Vec<HitLine>,
    block_boxes: Vec<(usize, Bounds<Pixels>)>,
    code_slabs: Vec<(usize, Bounds<Pixels>)>,
    images: Vec<PreparedImage>,
    cursor: Option<PaintQuad>,
    selection: Vec<PaintQuad>,
    visible: Bounds<Pixels>,
    anchor_delta: Option<Pixels>,
}

struct PreparedImage {
    surface: Bounds<Pixels>,
    bounds: Option<Bounds<Pixels>>,
    data: Option<Arc<RenderImage>>,
    fallback: Option<(WrappedLine, Point<Pixels>)>,
}
impl IntoElement for DocumentElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for DocumentElement {
    type RequestLayoutState = ();
    type PrepaintState = Prepared;
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        self.editor.update(cx, |e, cx| e.ensure_shapes(window, cx));
        let rows = self.editor.read(cx).total_rows();
        let mut s = Style::default();
        s.size.width = px(DOCUMENT_WIDTH).into();
        s.size.height = px(FIRST_BASELINE + rows as f32 * GRID + GRID).into();
        (window.request_layout(s, [], cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Prepared {
        let e = self.editor.read(cx);
        let colors = palette(e.dark, e.dot_opacity);
        let visible = window.content_mask().bounds;
        let overscan = Bounds::from_corners(
            point(visible.left(), visible.top() - visible.size.height),
            point(visible.right(), visible.bottom() + visible.size.height),
        );
        let mut y = 0usize;
        let mut lines = Vec::new();
        let mut block_boxes = Vec::new();
        let mut code_slabs = Vec::new();
        let mut images = Vec::new();
        let mut cursor = None;
        let mut selections = Vec::new();
        let mut anchor_delta = None;
        for (bi, b) in e.document.blocks.iter().enumerate() {
            let c = &e.shapes[&b.id];
            let shown = if e.revealed.contains(&b.id) {
                c.raw.as_ref().unwrap()
            } else {
                c.rendered.as_ref().unwrap()
            };
            let semantic_raw_rows = c.raw_rows.saturating_sub(c.raw_separator_rows);
            let block_rows = semantic_raw_rows.max(c.rendered_rows).max(1);
            let separator_rows = e.document.separator_rows_after(bi);
            let block_top = bounds.top() + px(FIRST_BASELINE + y as f32 * GRID - GRID);
            let block_bottom = block_top + px(block_rows as f32 * GRID);
            let active = e.revealed.contains(&b.id);
            if let Some((source, screen_y)) = e.pending_anchor
                && bi == e.document.block_at(source)
            {
                anchor_delta = Some(screen_y - block_top);
            }
            if active || block_bottom >= overscan.top() && block_top <= overscan.bottom() {
                block_boxes.push((
                    bi,
                    Bounds::from_corners(
                        point(bounds.left() + px(INSET), block_top),
                        point(bounds.left() + px(INSET + WRAP_WIDTH), block_bottom),
                    ),
                ));
                if b.kind == BlockKind::Code {
                    code_slabs.push((
                        bi,
                        Bounds::from_corners(
                            point(bounds.left() + px(INSET), block_top),
                            point(bounds.left() + px(INSET + WRAP_WIDTH), block_bottom),
                        ),
                    ));
                }
                let mut row = 0;
                for line in shown {
                    if line.source.start > b.range.end {
                        row = row.max(block_rows);
                    }
                    let n = line_rows(line);
                    let baseline = bounds.top() + px(FIRST_BASELINE + (y + row) as f32 * GRID);
                    let pad = (px(GRID) - line.layout.ascent() - line.layout.descent()) / 2.;
                    let paint_top = baseline - pad - line.layout.ascent();
                    let cell_top = block_top + px(row as f32 * GRID);
                    let code_inset = if line.code { 12. } else { 0. };
                    let lb = Bounds::new(
                        point(bounds.left() + px(INSET + code_inset), cell_top),
                        size(px(WRAP_WIDTH - code_inset * 2.), px(n as f32 * GRID)),
                    );
                    if let Some(image) = &line.image {
                        let image_bounds = image.data.as_ref().map(|data| {
                            let dimensions = data.size(0);
                            let ratio = dimensions.width.0.max(1) as f32
                                / dimensions.height.0.max(1) as f32;
                            let height = n as f32 * GRID;
                            let width = (height * ratio).min(WRAP_WIDTH);
                            Bounds::new(
                                point(lb.left() + (px(WRAP_WIDTH) - px(width)) / 2., lb.top()),
                                size(px(width), px(height)),
                            )
                        });
                        let fallback = image.failed.then(|| {
                            let text = if image.alt.is_empty() {
                                "Image failed to load".to_string()
                            } else {
                                image.alt.clone()
                            };
                            let layout = window
                                .text_system()
                                .shape_text(
                                    SharedString::from(text.clone()),
                                    px(13.5),
                                    &[TextRun {
                                        len: text.len(),
                                        font: font(PROSE_FONT),
                                        color: color(0xcf222e),
                                        background_color: None,
                                        underline: None,
                                        strikethrough: None,
                                    }],
                                    Some(px(WRAP_WIDTH - 16.)),
                                    None,
                                )
                                .unwrap()
                                .remove(0);
                            let origin = point(lb.left() + px(8.), lb.top() + px(GRID / 2.));
                            (layout, origin)
                        });
                        images.push(PreparedImage {
                            surface: lb,
                            bounds: image_bounds,
                            data: image.data.clone(),
                            fallback,
                        });
                    }
                    let paint_origin = point(lb.left(), paint_top);
                    let hit = HitLine {
                        block: bi,
                        row,
                        source: line.source.clone(),
                        bounds: lb,
                        paint_origin,
                        layout: line.layout.clone(),
                        map: line.map.clone(),
                        separator: false,
                    };
                    if active && line.map.is_none() {
                        let pos = e
                            .cursor()
                            .saturating_sub(line.source.start)
                            .min(line.source.len());
                        if line.source.start <= e.cursor()
                            && e.cursor() <= line.source.end
                            && e.selection.is_empty()
                        {
                            if let Some(p) = line.layout.position_for_index(pos, px(GRID)) {
                                let caret_top = if line.layout.text.is_empty() {
                                    lb.top()
                                } else {
                                    paint_origin.y + p.y
                                };
                                cursor = Some(fill(
                                    Bounds::new(
                                        point(paint_origin.x + p.x, caret_top),
                                        size(px(1.5), px(GRID)),
                                    ),
                                    colors.accent,
                                ))
                            }
                        }
                        let overlap = e.selection.start.max(line.source.start)
                            ..e.selection.end.min(line.source.end);
                        if overlap.start < overlap.end {
                            if let (Some(a), Some(z)) = (
                                line.layout.position_for_index(
                                    overlap.start - line.source.start,
                                    px(GRID),
                                ),
                                line.layout
                                    .position_for_index(overlap.end - line.source.start, px(GRID)),
                            ) {
                                selections.push(fill(
                                    Bounds::from_corners(
                                        point(paint_origin.x + a.x, paint_origin.y + a.y),
                                        point(
                                            paint_origin.x + z.x,
                                            paint_origin.y + z.y + px(GRID),
                                        ),
                                    ),
                                    colors.selection,
                                ))
                            }
                        }
                    }
                    lines.push(hit);
                    row += n;
                }

                // Blank rows between semantic blocks are source positions too.
                // They used to exist only in the vertical row count, leaving no
                // hit target for the mouse and making clicks fall back to the
                // nearest non-empty line.
                let included_separator_rows = if active { c.raw_separator_rows } else { 0 };
                if separator_rows > included_separator_rows {
                    let metric_line = shown.last().expect("every block has a shaped line");
                    let pad =
                        (px(GRID) - metric_line.layout.ascent() - metric_line.layout.descent())
                            / 2.;
                    for separator in included_separator_rows..separator_rows {
                        let baseline = bounds.top()
                            + px(FIRST_BASELINE + (y + block_rows + separator) as f32 * GRID);
                        let top = baseline - pad - metric_line.layout.ascent();
                        let cell_top = block_top + px((block_rows + separator) as f32 * GRID);
                        let source = (b.range.end + 1 + separator).min(e.document.content.len());
                        lines.push(HitLine {
                            block: bi,
                            row: block_rows + separator,
                            source: source..source,
                            bounds: Bounds::new(
                                point(bounds.left() + px(INSET), cell_top),
                                size(px(WRAP_WIDTH), px(GRID)),
                            ),
                            paint_origin: point(bounds.left() + px(INSET), top),
                            layout: metric_line.layout.clone(),
                            map: None,
                            separator: true,
                        });
                    }
                }
            }
            y += block_rows + separator_rows;
        }
        Prepared {
            lines,
            block_boxes,
            code_slabs,
            images,
            cursor,
            selection: selections,
            visible,
            anchor_delta,
        }
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        p: &mut Prepared,
        window: &mut Window,
        cx: &mut App,
    ) {
        let editor = self.editor.read(cx);
        let colors = palette(editor.dark, editor.dot_opacity);
        let left = p.visible.left().max(bounds.left());
        let right = p.visible.right().min(bounds.right());
        let top = p.visible.top().max(bounds.top());
        let bottom = p.visible.bottom().min(bounds.bottom());
        let grid_left = bounds.left() + px(INSET);
        let grid_right = bounds.right() - px(INSET);
        let grid_top = bounds.top() + px(GRID);
        let c0 = ((left.max(grid_left) - grid_left) / px(GRID)).floor() as i32;
        // Include the closing grid post at the inner edge of the 24 px page
        // margin. The quad itself remains inside the paper bounds.
        let last_inner_column = (((grid_right - grid_left) / px(GRID)).floor() as i32).max(0);
        let c1 =
            (((right.min(grid_right) - grid_left) / px(GRID)).ceil() as i32).min(last_inner_column);
        let r0 = ((top.max(grid_top) - grid_top) / px(GRID)).floor() as i32;
        let last_row = (((bounds.bottom() - grid_top) / px(GRID)).floor() as i32 - 1).max(0);
        let r1 = (((bottom - grid_top) / px(GRID)).ceil() as i32).min(last_row);
        for r in r0..=r1 {
            for c in c0..=c1 {
                window.paint_quad(
                    fill(
                        Bounds::new(
                            point(
                                grid_left + px(c as f32 * GRID),
                                grid_top + px(r as f32 * GRID),
                            ),
                            size(px(2.), px(2.)),
                        ),
                        colors.dot,
                    )
                    .corner_radii(px(1.)),
                )
            }
        }

        for (_, outer) in p.code_slabs.iter().copied() {
            window.paint_quad(fill(outer, colors.code_border).corner_radii(px(6.)));
            let inner = Bounds::from_corners(
                point(outer.left() + px(1.), outer.top() + px(1.)),
                point(outer.right() - px(1.), outer.bottom() - px(1.)),
            );
            window.paint_quad(fill(inner, colors.code_bg).corner_radii(px(5.)));
        }
        for image in &p.images {
            window.paint_quad(fill(image.surface, colors.bg));
            if let (Some(bounds), Some(data)) = (image.bounds, image.data.clone()) {
                let _ = window.paint_image(bounds, px(6.).into(), data, 0, false);
            } else if let Some((layout, origin)) = &image.fallback {
                paint_outline(window, image.surface, color(0xcf222e));
                let _ = layout.paint(
                    *origin,
                    px(GRID),
                    TextAlign::Left,
                    Some(image.surface),
                    window,
                    cx,
                );
            }
        }
        for q in p.selection.drain(..) {
            window.paint_quad(q)
        }
        for l in &p.lines {
            if l.separator {
                continue;
            }
            let _ = l.layout.paint(
                l.paint_origin,
                px(GRID),
                TextAlign::Left,
                Some(l.bounds),
                window,
                cx,
            );
        }
        let had_cursor = p.cursor.is_some();
        if let Some(q) = p.cursor.take() {
            if self.editor.read(cx).ensure_caret_visible {
                let vertical = &self.editor.read(cx).vertical_scroll;
                let vertical_view = vertical.bounds();
                let mut vertical_offset = vertical.offset();
                let old_vertical = vertical_offset;
                if q.bounds.bottom() > vertical_view.bottom() {
                    vertical_offset.y -= q.bounds.bottom() - vertical_view.bottom()
                } else if q.bounds.top() < vertical_view.top() {
                    vertical_offset.y += vertical_view.top() - q.bounds.top()
                }
                vertical_offset.y = vertical_offset
                    .y
                    .clamp(-vertical.max_offset().height, px(0.));
                if vertical_offset != old_vertical {
                    vertical.set_offset(vertical_offset);
                    window.request_animation_frame();
                }

                let horizontal = &self.editor.read(cx).horizontal_scroll;
                let horizontal_view = horizontal.bounds();
                let mut horizontal_offset = horizontal.offset();
                let old_horizontal = horizontal_offset;
                if q.bounds.right() > horizontal_view.right() {
                    horizontal_offset.x -= q.bounds.right() - horizontal_view.right()
                } else if q.bounds.left() < horizontal_view.left() {
                    horizontal_offset.x += horizontal_view.left() - q.bounds.left()
                }
                horizontal_offset.x = horizontal_offset
                    .x
                    .clamp(-horizontal.max_offset().width, px(0.));
                if horizontal_offset != old_horizontal {
                    horizontal.set_offset(horizontal_offset);
                    window.request_animation_frame();
                }
            }
            window.paint_quad(q)
        }
        if self.editor.read(cx).debug_boxes {
            let signature = debug_geometry_signature(bounds, p);
            let should_log = self.editor.update(cx, |editor, _| {
                if editor.last_debug_geometry == Some(signature) {
                    false
                } else {
                    editor.last_debug_geometry = Some(signature);
                    true
                }
            });
            if should_log {
                log_debug_geometry(signature, p);
            }
            paint_outline(window, bounds, alpha(color(0xff3b30), 0.9));
            for (_, block) in &p.block_boxes {
                paint_outline(window, *block, alpha(color(0x0969da), 0.85));
            }
            for (_, slab) in &p.code_slabs {
                paint_outline(window, *slab, alpha(color(0x2da44e), 0.9));
            }
            for image in &p.images {
                paint_outline(window, image.surface, alpha(color(0x8250df), 0.9));
                if let Some(bounds) = image.bounds {
                    paint_outline(window, bounds, alpha(color(0x1f883d), 0.9));
                }
            }
            for line in &p.lines {
                let debug_color = if line.separator {
                    color(0xfb8500)
                } else {
                    color(0xbf3989)
                };
                paint_outline(window, line.bounds, alpha(debug_color, 0.8));
            }
        }
        let applied_anchor = p.anchor_delta.is_some();
        if let Some(delta) = p.anchor_delta.take() {
            let scroll = &self.editor.read(cx).vertical_scroll;
            let mut offset = scroll.offset();
            let old_offset = offset;
            offset.y += delta;
            let max = scroll.max_offset();
            offset.y = offset.y.clamp(-max.height, px(0.));
            if offset != old_offset {
                scroll.set_offset(offset);
                window.request_animation_frame();
            }
        }
        window.handle_input(
            &self.editor.read(cx).focus,
            ElementInputHandler::new(bounds, self.editor.clone()),
            cx,
        );
        let needs_refresh = self.editor.update(cx, |e, app| {
            e.hit_lines = p.lines.clone();
            e.doc_bounds = Some(bounds);
            if had_cursor {
                e.ensure_caret_visible = false;
            }
            if applied_anchor {
                e.pending_anchor = None;
            }
            while let Some(start) = e.pending_edits.pop_front() {
                e.latencies.record(start.elapsed());
                if e.latencies.0.len() == 100 {
                    if let Some((m, p95, max)) = e.latencies.summary_ms() {
                        eprintln!(
                            "EDIT_LATENCY_100 median_ms={m:.3} p95_ms={p95:.3} max_ms={max:.3}"
                        );
                        eprintln!(
                            "CACHE_COUNTERS raw_reshapes={} rendered_reshapes={} local_reparses={} parsed_blocks={}",
                            e.document.counters.raw_reshapes,
                            e.document.counters.rendered_reshapes,
                            e.document.counters.local_reparses,
                            e.document.counters.parsed_blocks,
                        );
                    }
                }
            }
            if e.synthetic_remaining > 0 {
                e.synthetic_remaining -= 1;
                e.synthetic_edit();
                app.notify();
            }
            e.synthetic_remaining > 0 || !e.pending_edits.is_empty()
        });
        if needs_refresh {
            window.request_animation_frame();
        }
        if FIRST_FRAME.set(()).is_ok() {
            eprintln!(
                "FIRST_INTERACTIVE_FRAME_MS={:.3}",
                STARTED.get().unwrap().elapsed().as_secs_f64() * 1000.
            )
        }
    }
}

fn shape_raw(
    range: Range<usize>,
    text: &str,
    code_block: bool,
    palette: Palette,
    w: &mut Window,
) -> ShapedLine {
    let runs = if code_block {
        code_text_runs(text, text.trim_start().starts_with("```"), palette)
    } else {
        raw_text_runs(text, palette)
    };
    let layout = w
        .text_system()
        .shape_text(
            SharedString::from(text.to_string()),
            px(if code_block { 14.08 } else { 16. }),
            &runs,
            Some(px(if code_block {
                WRAP_WIDTH - 24.
            } else {
                WRAP_WIDTH
            })),
            None,
        )
        .unwrap()
        .remove(0);
    ShapedLine {
        source: range,
        layout,
        map: None,
        code: code_block,
        image: None,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RawRole {
    Text,
    Mark,
    InlineCode,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct RawStyle {
    inline: InlineStyle,
    role: RawRole,
}

fn raw_text_runs(text: &str, palette: Palette) -> Vec<TextRun> {
    let mut byte_styles = vec![
        RawStyle {
            inline: InlineStyle::default(),
            role: RawRole::Mark,
        };
        text.len()
    ];
    let mut current = InlineStyle::default();
    for (event, range) in Parser::new(text).into_offset_iter() {
        match event {
            Event::Start(Tag::Emphasis) => current.italic = true,
            Event::End(TagEnd::Emphasis) => current.italic = false,
            Event::Start(Tag::Strong) => current.strong = true,
            Event::End(TagEnd::Strong) => current.strong = false,
            Event::Start(Tag::Link { .. }) => current.link = true,
            Event::End(TagEnd::Link) => current.link = false,
            Event::Text(_) => {
                for style in &mut byte_styles[range] {
                    *style = RawStyle {
                        inline: current,
                        role: RawRole::Text,
                    };
                }
            }
            Event::Code(_) => {
                let mut code = current;
                code.code = true;
                let inner = range.start.saturating_add(1)..range.end.saturating_sub(1);
                if inner.start <= inner.end && inner.end <= byte_styles.len() {
                    for style in &mut byte_styles[inner] {
                        *style = RawStyle {
                            inline: code,
                            role: RawRole::InlineCode,
                        };
                    }
                }
            }
            _ => {}
        }
    }

    let mut spans = Vec::<(usize, RawStyle)>::new();
    for style in byte_styles {
        if let Some((len, previous)) = spans.last_mut()
            && *previous == style
        {
            *len += 1;
        } else {
            spans.push((1, style));
        }
    }
    if spans.is_empty() {
        spans.push((
            0,
            RawStyle {
                inline: InlineStyle::default(),
                role: RawRole::Text,
            },
        ));
    }
    spans
        .into_iter()
        .map(|(len, style)| {
            let mut face = font(PROSE_FONT);
            if style.inline.strong {
                face.weight = FontWeight::BOLD;
            }
            if style.inline.italic {
                face.style = FontStyle::Italic;
            }
            if style.inline.code {
                face = font(MONO_FONT);
            }
            let color = if style.role == RawRole::Mark {
                palette.fg_faint
            } else if style.inline.link {
                palette.accent
            } else {
                palette.fg
            };
            TextRun {
                len,
                font: face,
                color,
                background_color: (style.role == RawRole::InlineCode).then_some(palette.code_bg),
                underline: style.inline.link.then_some(UnderlineStyle {
                    thickness: px(1.),
                    color: Some(alpha(color, 0.4)),
                    wavy: false,
                }),
                strikethrough: None,
            }
        })
        .collect()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CodeRole {
    Text,
    Keyword,
    String,
    Comment,
    Number,
    Name,
    Type,
    Operator,
    Fence,
}

fn code_text_runs(text: &str, fence: bool, palette: Palette) -> Vec<TextRun> {
    let mut roles = vec![
        if fence {
            CodeRole::Fence
        } else {
            CodeRole::Text
        };
        text.len()
    ];
    if !fence {
        let bytes = text.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
                roles[i..].fill(CodeRole::Comment);
                break;
            }
            if bytes[i] == b'\'' || bytes[i] == b'"' {
                let quote = bytes[i];
                let start = i;
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == b'\\' {
                        i = (i + 2).min(bytes.len());
                        continue;
                    }
                    i += 1;
                    if bytes[i - 1] == quote {
                        break;
                    }
                }
                roles[start..i].fill(CodeRole::String);
                continue;
            }
            if bytes[i].is_ascii_digit() {
                let start = i;
                i += 1;
                while i < bytes.len()
                    && (bytes[i].is_ascii_hexdigit() || matches!(bytes[i], b'.' | b'_' | b'x'))
                {
                    i += 1;
                }
                roles[start..i].fill(CodeRole::Number);
                continue;
            }
            if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
                let start = i;
                i += 1;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                let word = &text[start..i];
                let role = if matches!(
                    word,
                    "as" | "async"
                        | "await"
                        | "break"
                        | "class"
                        | "const"
                        | "continue"
                        | "def"
                        | "else"
                        | "enum"
                        | "false"
                        | "fn"
                        | "for"
                        | "from"
                        | "if"
                        | "impl"
                        | "import"
                        | "in"
                        | "let"
                        | "loop"
                        | "match"
                        | "mod"
                        | "move"
                        | "mut"
                        | "new"
                        | "none"
                        | "null"
                        | "pub"
                        | "return"
                        | "self"
                        | "static"
                        | "struct"
                        | "super"
                        | "this"
                        | "trait"
                        | "true"
                        | "type"
                        | "use"
                        | "var"
                        | "where"
                        | "while"
                        | "yield"
                ) {
                    CodeRole::Keyword
                } else if word.as_bytes().first().is_some_and(u8::is_ascii_uppercase) {
                    CodeRole::Type
                } else if text[i..].trim_start().starts_with('(') {
                    CodeRole::Name
                } else {
                    CodeRole::Text
                };
                roles[start..i].fill(role);
                continue;
            }
            if b"+-*/%=!<>|&^~?:".contains(&bytes[i]) {
                roles[i] = CodeRole::Operator;
            }
            i += 1;
        }
        if text.trim_start().starts_with("# ") || text.trim_start().starts_with("#!") {
            let start = text.len() - text.trim_start().len();
            roles[start..].fill(CodeRole::Comment);
        }
    }

    let mut spans = Vec::<(usize, CodeRole)>::new();
    for role in roles {
        if let Some((len, previous)) = spans.last_mut()
            && *previous == role
        {
            *len += 1;
        } else {
            spans.push((1, role));
        }
    }
    if spans.is_empty() {
        spans.push((0, CodeRole::Text));
    }
    spans
        .into_iter()
        .map(|(len, role)| TextRun {
            len,
            font: font(MONO_FONT),
            color: match role {
                CodeRole::Text => palette.fg,
                CodeRole::Keyword => palette.syn_keyword,
                CodeRole::String => palette.syn_string,
                CodeRole::Comment => palette.syn_comment,
                CodeRole::Number => palette.syn_number,
                CodeRole::Name => palette.syn_name,
                CodeRole::Type => palette.syn_type,
                CodeRole::Operator => palette.syn_operator,
                CodeRole::Fence => palette.fg_faint,
            },
            background_color: None,
            underline: None,
            strikethrough: None,
        })
        .collect()
}

fn shape_render(
    line: &RenderLine,
    document_path: Option<&Path>,
    palette: Palette,
    w: &mut Window,
) -> ShapedLine {
    let size = match line.level {
        1 => 30.4,
        2 => 24.,
        3 => 20.,
        _ => 16.,
    };
    let runs = if line.code_block {
        code_text_runs(&line.text, false, palette)
    } else {
        line.spans
            .iter()
            .map(|s| {
                let mut f = font(PROSE_FONT);
                if line.level > 0 || s.style.strong {
                    f.weight = FontWeight::BOLD
                }
                if s.style.italic {
                    f.style = FontStyle::Italic
                }
                if s.style.code {
                    f = font(MONO_FONT)
                }
                let color = if s.style.link {
                    palette.accent
                } else if s.style.code {
                    palette.fg
                } else {
                    palette.fg
                };
                TextRun {
                    len: s.len,
                    font: f,
                    color,
                    background_color: s.style.code.then_some(palette.code_bg),
                    underline: s.style.link.then_some(UnderlineStyle {
                        thickness: px(1.),
                        color: Some(alpha(color, 0.4)),
                        wavy: false,
                    }),
                    strikethrough: None,
                }
            })
            .collect::<Vec<_>>()
    };
    let layout = w
        .text_system()
        .shape_text(
            SharedString::from(line.text.clone()),
            px(if line.code_block { 14.08 } else { size }),
            &runs,
            Some(px(if line.code_block {
                WRAP_WIDTH - 24.
            } else {
                WRAP_WIDTH
            })),
            None,
        )
        .unwrap()
        .remove(0);
    ShapedLine {
        source: 0..0,
        layout,
        map: Some(line.source_map.clone()),
        code: line.code_block,
        image: line.image.as_ref().map(|image| ShapedImage {
            alt: image.alt.clone(),
            resource: image_resource(&image.src, document_path),
            data: None,
            failed: false,
            rows: DEFAULT_IMAGE_ROWS,
        }),
    }
}
fn rows(lines: &[ShapedLine]) -> usize {
    lines.iter().map(line_rows).sum()
}
fn line_rows(line: &ShapedLine) -> usize {
    if let Some(image) = &line.image {
        return image.rows;
    }
    let wrapped = line.layout.wrap_boundaries().len() + 1;
    let glyph_height = line.layout.ascent() + line.layout.descent();
    let height_rows = (glyph_height / px(GRID)).ceil() as usize;
    wrapped.max(height_rows).max(1)
}
fn grapheme_offset(s: &str, n: usize) -> usize {
    s.grapheme_indices(true).nth(n).map_or(s.len(), |(i, _)| i)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WordClass {
    Whitespace,
    Word,
    Punctuation,
}

fn word_class(grapheme: &str) -> WordClass {
    if grapheme.chars().all(char::is_whitespace) {
        WordClass::Whitespace
    } else if grapheme
        .chars()
        .any(|character| character.is_alphanumeric() || character == '_')
    {
        WordClass::Word
    } else {
        WordClass::Punctuation
    }
}

fn previous_word_boundary(text: &str, at: usize) -> usize {
    let at = at.min(text.len());
    let graphemes = text[..at].grapheme_indices(true).collect::<Vec<_>>();
    let Some((mut index, grapheme)) = graphemes.last().copied() else {
        return 0;
    };
    let mut position = graphemes.len() - 1;
    if word_class(grapheme) == WordClass::Whitespace {
        while position > 0 && word_class(graphemes[position].1) == WordClass::Whitespace {
            position -= 1;
        }
        if word_class(graphemes[position].1) == WordClass::Whitespace {
            return 0;
        }
        index = graphemes[position].0;
    }
    let class = word_class(graphemes[position].1);
    while position > 0 && word_class(graphemes[position - 1].1) == class {
        position -= 1;
        index = graphemes[position].0;
    }
    index
}

fn next_word_boundary(text: &str, at: usize) -> usize {
    let at = at.min(text.len());
    let graphemes = text[at..]
        .grapheme_indices(true)
        .map(|(index, grapheme)| (at + index, grapheme))
        .collect::<Vec<_>>();
    let Some((_, first)) = graphemes.first().copied() else {
        return text.len();
    };
    let mut position = 0;
    let class = word_class(first);
    while position < graphemes.len() && word_class(graphemes[position].1) == class {
        position += 1;
    }
    while position < graphemes.len() && word_class(graphemes[position].1) == WordClass::Whitespace {
        position += 1;
    }
    graphemes
        .get(position)
        .map_or(text.len(), |(index, _)| *index)
}

fn utf16_to_utf8(s: &str, n: usize) -> usize {
    let (mut u, mut b) = (0, 0);
    for c in s.chars() {
        if u >= n {
            break;
        }
        u += c.len_utf16();
        b += c.len_utf8()
    }
    b
}
fn utf8_to_utf16(s: &str, n: usize) -> usize {
    s[..n.min(s.len())].encode_utf16().count()
}

#[cfg(test)]
mod ui_tests {
    use super::*;

    #[test]
    fn separator_borrowed_by_code_block_keeps_prose_styling() {
        assert!(source_line_is_code(BlockKind::Code, 20, 20));
        assert!(!source_line_is_code(BlockKind::Code, 20, 21));
        assert!(!source_line_is_code(BlockKind::Paragraph, 20, 10));
    }

    #[test]
    fn word_navigation_uses_unicode_grapheme_boundaries() {
        let text = "héllo  brave_world";
        assert_eq!(next_word_boundary(text, 0), 8);
        assert_eq!(previous_word_boundary(text, text.len()), 8);
        assert_eq!(previous_word_boundary(text, 8), 0);
        assert_eq!(next_word_boundary(text, 8), text.len());
    }
}

fn main() {
    STARTED.set(Instant::now()).ok();
    let argument = std::env::args_os().nth(1);
    let path = argument
        .filter(|argument| argument != "--new")
        .map(PathBuf::from)
        .or_else(|| {
            (std::env::args_os().len() == 1)
                .then(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/sample.md"))
        });
    let path = path.map(|path| path.canonicalize().unwrap_or(path));
    let content = path
        .as_ref()
        .and_then(|path| fs::read_to_string(path).ok())
        .unwrap_or_default();
    let mut config = persistence::load_config();
    if let Some(path) = &path {
        persistence::push_recent(&mut config, path);
        let _ = persistence::save_config(&config);
    }
    let theme = match config.theme.as_deref() {
        Some("light") => ThemePreference::Light,
        Some("dark") => ThemePreference::Dark,
        _ => ThemePreference::System,
    };
    let debug_boxes = std::env::var_os("BULLETMD_DEBUG_BOXES").is_some_and(|value| value != "0");
    Application::new().run(move |cx: &mut App| {
        cx.bind_keys([
            KeyBinding::new("backspace", Backspace, None),
            KeyBinding::new("delete", Delete, None),
            KeyBinding::new("left", Left, None),
            KeyBinding::new("right", Right, None),
            KeyBinding::new("up", Up, None),
            KeyBinding::new("down", Down, None),
            KeyBinding::new("shift-left", SelectLeft, None),
            KeyBinding::new("shift-right", SelectRight, None),
            KeyBinding::new("shift-up", SelectUp, None),
            KeyBinding::new("shift-down", SelectDown, None),
            KeyBinding::new("ctrl-left", WordLeft, None),
            KeyBinding::new("ctrl-right", WordRight, None),
            KeyBinding::new("ctrl-shift-left", SelectWordLeft, None),
            KeyBinding::new("ctrl-shift-right", SelectWordRight, None),
            KeyBinding::new("ctrl-backspace", DeleteWordBackward, None),
            KeyBinding::new("home", Home, None),
            KeyBinding::new("end", End, None),
            KeyBinding::new("ctrl-a", SelectAll, None),
            KeyBinding::new("enter", Enter, None),
            KeyBinding::new("escape", Escape, None),
            KeyBinding::new("ctrl-c", Copy, None),
            KeyBinding::new("ctrl-x", Cut, None),
            KeyBinding::new("ctrl-v", Paste, None),
            KeyBinding::new("ctrl-z", Undo, None),
            KeyBinding::new("ctrl-shift-z", Redo, None),
            KeyBinding::new("ctrl-y", Redo, None),
            KeyBinding::new("ctrl-s", Save, None),
            KeyBinding::new("ctrl-shift-s", SaveAs, None),
            KeyBinding::new("ctrl-n", NewDocument, None),
            KeyBinding::new("ctrl-shift-n", NewWindow, None),
            KeyBinding::new("ctrl-o", OpenDocument, None),
            KeyBinding::new("ctrl-b", Bold, None),
            KeyBinding::new("ctrl-i", Italic, None),
            KeyBinding::new("ctrl-e", InlineCode, None),
            KeyBinding::new("ctrl-k", Link, None),
            KeyBinding::new("ctrl-q", Quit, None),
        ]);
        let bounds = Bounds::centered(None, size(px(1000.), px(760.)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: None,
                    window_decorations: Some(WindowDecorations::Client),
                    ..Default::default()
                },
                |_, cx| {
                    cx.new(|cx| {
                        let document = DocumentModel::new(content.clone());
                        let first = document.blocks[0].id;
                        let synthetic_remaining = std::env::var("BULLETMD_BENCH_EDIT")
                            .ok()
                            .and_then(|value| value.parse().ok())
                            .unwrap_or(0);
                        if synthetic_remaining > 0 {
                            eprintln!("EDIT_BENCH_SCHEDULED count={synthetic_remaining}");
                        }
                        Editor {
                            path: path.clone(),
                            focus: cx.focus_handle(),
                            document,
                            selection: 0..0,
                            reversed: false,
                            marked: None,
                            selecting: false,
                            revealed: HashSet::from([first]),
                            shapes: HashMap::new(),
                            hit_lines: Vec::new(),
                            doc_bounds: None,
                            vertical_scroll: ScrollHandle::new(),
                            horizontal_scroll: ScrollHandle::new(),
                            undo: Vec::new(),
                            redo: Vec::new(),
                            pending_edits: VecDeque::new(),
                            synthetic_remaining,
                            latencies: LatencySamples::default(),
                            pending_anchor: None,
                            ensure_caret_visible: true,
                            preferred_column: None,
                            status: "saved".into(),
                            saved_text: content.clone(),
                            dirty: false,
                            autosave_generation: 0,
                            dark: false,
                            open_menu: None,
                            theme,
                            debug_boxes,
                            last_debug_geometry: None,
                            dot_opacity: config.dot_opacity.clamp(0.0, 0.30),
                            config: config.clone(),
                            watcher: None,
                            watch_generation: 0,
                            conflict_text: None,
                            close_after_save: false,
                        }
                    })
                },
            )
            .unwrap();
        window
            .update(cx, |e, w, cx| {
                w.focus(&e.focus);
                e.start_watch(cx);
                let editor = cx.entity().downgrade();
                w.on_window_should_close(cx, move |window, cx| {
                    let dirty = editor.upgrade().is_some_and(|editor| editor.read(cx).dirty);
                    if dirty {
                        let _ = editor.update(cx, |editor, cx| editor.request_close(window, cx));
                        false
                    } else {
                        true
                    }
                });
                cx.activate(true)
            })
            .unwrap();
    })
}
