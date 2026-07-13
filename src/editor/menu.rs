use super::*;

// Extracted from editor/mod.rs. Additional inherent methods on Editor;
// child module reaches Editor's private fields via the parent module.

impl Editor {
    fn run_menu_command(
        &mut self,
        command: MenuCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_menu = None;
        self.recent_submenu_open = false;
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
                self.theming.theme = theme;
                self.theming.dark = theme.dark(window.appearance());
                self.layout.shapes.clear();
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
                self.theming.dot_opacity = (f32::from(percent) / 100.).clamp(0.0, 0.30);
                self.config.dot_opacity = self.theming.dot_opacity;
                if let Err(error) = persistence::save_config(&self.config) {
                    self.status = format!("appearance save failed: {error}");
                }
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
        path: &str,
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

    /// The "Open Recent ›" row and, when expanded, the side submenu listing the
    /// ten most recent files.
    fn open_recent_item(
        recent: Vec<String>,
        expanded: bool,
        colors: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let items = recent
            .into_iter()
            .take(10)
            .enumerate()
            .map(|(index, path)| Self::recent_menu_item(index, &path, colors, cx).into_any_element())
            .collect::<Vec<_>>();
        let has_recent = !items.is_empty();
        let expanded = expanded && has_recent;
        div()
            .relative()
            .child(
                div()
                    .id("menu-open-recent")
                    .h(px(28.))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_size(px(12.8))
                    .text_color(if has_recent { colors.fg } else { colors.fg_dim })
                    .when(has_recent, |row| {
                        row.cursor_pointer()
                            .hover(move |style| style.bg(alpha(colors.fg, 0.1)))
                    })
                    .when(expanded, |row| row.bg(alpha(colors.fg, 0.1)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.recent_submenu_open = !this.recent_submenu_open;
                        cx.notify();
                    }))
                    .child("Open Recent")
                    .child(div().text_color(colors.fg_dim).child("›")),
            )
            .when(expanded, |slot| {
                slot.child(
                    deferred(
                        div()
                            .absolute()
                            .top(px(-4.))
                            .left(px(214.))
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
                            .children(items),
                    )
                    .with_priority(11),
                )
            })
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

    pub(crate) fn menu_dropdown(
        menu: OpenMenu,
        selected_theme: ThemePreference,
        dot_opacity: f32,
        recent: Vec<String>,
        recent_expanded: bool,
        colors: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let dot_items = [0, 6, 12, 18, 24, 30]
            .into_iter()
            .map(|percent| {
                Self::dot_menu_item(
                    percent,
                    dot_opacity.mul_add(100., -f32::from(percent)).abs() < 0.5,
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
                .child(Self::open_recent_item(recent, recent_expanded, colors, cx))
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
            })
    }
}
