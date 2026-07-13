use std::fs;
use std::path::PathBuf;

use bulletmd_native_poc::{
    editor::{
        Backspace, Bold, Copy, Cut, Delete, Down, End, Enter, Escape, Home, InlineCode, Italic,
        Left, Link, NewDocument, NewWindow, OpenDocument, Paste, Quit, Redo, Right, Save, SaveAs,
        SelectAll, SelectDown, SelectLeft, SelectRight, SelectUp, SelectWordLeft, SelectWordRight,
        ThemePreference, Undo, Up, WordLeft, WordRight, DeleteWordBackward, Editor,
    },
    persistence,
};
use gpui::{
    App, Application, AppContext, Bounds, KeyBinding, WindowBounds, WindowDecorations,
    WindowOptions, px, size,
};

fn main() {
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
                    app_id: Some("bulletmd".into()),
                    ..Default::default()
                },
                |_, cx| {
                    cx.new(|cx| {
                        Editor::new(path.clone(), content.clone(), theme, config.clone(), cx)
                    })
                },
            )
            .unwrap();
        window
            .update(cx, |e, w, cx| {
                w.focus(e.focus());
                e.start_watch(cx);
                let editor = cx.entity().downgrade();
                w.on_window_should_close(cx, move |window, cx| {
                    let dirty = editor
                        .upgrade()
                        .is_some_and(|editor| editor.read(cx).is_dirty());
                    if dirty {
                        let _ = editor.update(cx, |editor, cx| editor.request_close(window, cx));
                        false
                    } else {
                        true
                    }
                });
                cx.activate(true);
            })
            .unwrap();
    });
}

#[cfg(test)]
mod ui_tests {
    use bulletmd_native_poc::shaping::{source_line_is_code, previous_word_boundary, next_word_boundary};
    use bulletmd_native_poc::model::BlockKind;

    #[test]
    fn code_block_does_not_style_following_rows_as_code() {
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
