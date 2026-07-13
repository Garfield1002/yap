use super::*;

// Extracted from editor/mod.rs. See mod.rs for the Editor struct.

impl Focusable for Editor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

// Spell-check overlay wiring (feature: `spellcheck`). These decorate the render
// tree with the right-click handler and the floating suggestion menu. Kept as
// element-wrapping helpers with a no-op stub below so the render chain reads the
// same whether or not the feature is built.
#[cfg(feature = "spellcheck")]
impl Editor {
    /// Adds the right-click handler that opens the suggestion menu over a
    /// misspelled word to the document surface element.
    fn with_spell_right_click<E: InteractiveElement + 'static>(
        &self,
        el: E,
        cx: &mut Context<Self>,
    ) -> E {
        el.on_mouse_down(MouseButton::Right, cx.listener(Self::spell_context_menu))
    }

    fn spell_menu_item(
        id: usize,
        label: String,
        colors: Palette,
        on_click: impl Fn(&mut Self, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(("spell-item", id))
            .h(px(26.))
            .px(px(10.))
            .flex()
            .items_center()
            .text_size(px(12.8))
            .text_color(colors.fg)
            .cursor_pointer()
            .hover(move |style| style.bg(alpha(colors.fg, 0.1)))
            .on_click(cx.listener(move |this, _, _, cx| on_click(this, cx)))
            .child(label)
    }

    /// Appends the clickable `spell:en_US` / `spell:off` indicator to the status
    /// bar, when the dictionary loaded. Clicking it toggles spell checking.
    fn with_spell_status<E: ParentElement + 'static>(
        &self,
        el: E,
        colors: Palette,
        cx: &mut Context<Self>,
    ) -> E {
        if self.spell.is_none() {
            return el;
        }
        let label = if self.spell_enabled {
            "spell:en_US"
        } else {
            "spell:off"
        };
        el.child(
            div()
                .id("spell-toggle")
                .ml(px(14.))
                .cursor_pointer()
                .hover(move |style| style.text_color(colors.fg))
                .on_click(cx.listener(|this, _, _, cx| this.toggle_spell(cx)))
                .child(label),
        )
    }

    /// Appends the floating suggestion menu, if one is open, as a deferred
    /// overlay anchored at the click position.
    fn with_spell_menu<E: ParentElement + 'static>(
        &self,
        el: E,
        colors: Palette,
        cx: &mut Context<Self>,
    ) -> E {
        let Some(menu) = self.spell_menu.as_ref() else {
            return el;
        };
        let word = menu.word.clone();
        let mut items: Vec<gpui::AnyElement> = Vec::new();
        if menu.suggestions.is_empty() {
            items.push(
                div()
                    .h(px(26.))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .text_size(px(12.8))
                    .text_color(colors.fg_dim)
                    .child("No suggestions")
                    .into_any_element(),
            );
        } else {
            for (i, suggestion) in menu.suggestions.iter().enumerate() {
                let target = word.clone();
                let replacement = suggestion.clone();
                items.push(
                    Self::spell_menu_item(
                        i,
                        suggestion.clone(),
                        colors,
                        move |this, cx| this.apply_suggestion(target.clone(), &replacement, cx),
                        cx,
                    )
                    .into_any_element(),
                );
            }
        }
        items.push(
            div()
                .h(px(9.))
                .mx(px(7.))
                .border_b_1()
                .border_color(colors.border)
                .into_any_element(),
        );
        let target = word;
        items.push(
            Self::spell_menu_item(
                usize::MAX,
                "Ignore".into(),
                colors,
                move |this, cx| this.ignore_spelling(target.clone(), cx),
                cx,
            )
            .into_any_element(),
        );
        el.child(
            deferred(
                div()
                    .absolute()
                    .left(menu.position.x)
                    .top(menu.position.y)
                    .w(px(200.))
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
            .with_priority(30),
        )
    }
}

#[cfg(not(feature = "spellcheck"))]
impl Editor {
    #[allow(clippy::unused_self)]
    const fn with_spell_right_click<E>(&self, el: E, _cx: &mut Context<Self>) -> E {
        el
    }

    #[allow(clippy::unused_self)]
    const fn with_spell_menu<E>(&self, el: E, _colors: Palette, _cx: &mut Context<Self>) -> E {
        el
    }

    #[allow(clippy::unused_self)]
    const fn with_spell_status<E>(&self, el: E, _colors: Palette, _cx: &mut Context<Self>) -> E {
        el
    }
}

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dark = self.theming.theme.dark(window.appearance());
        if self.theming.dark != dark {
            self.theming.dark = dark;
            self.layout.shapes.clear();
        }
        let colors = palette(dark);
        let document_margin =
            ((window.viewport_size().width - px(DOCUMENT_WIDTH)) / 2.).max(px(0.));
        let filename = self
            .path
            .as_deref()
            .and_then(Path::file_name).map_or_else(|| "untitled.md".into(), |name| name.to_string_lossy().into_owned());
        self.with_spell_menu(
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
                        this.recent_submenu_open = false;
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
                this.new_document_with_prompt(window, cx);
            }))
            .on_action(cx.listener(|this: &mut Self, _: &NewWindow, _, _| this.new_window()))
            .on_action(
                cx.listener(|this: &mut Self, _: &OpenDocument, window, cx| {
                    this.open_with_prompt(window, cx);
                }),
            )
            .on_action(cx.listener(|this: &mut Self, _: &SaveAs, window, cx| {
                this.save_as_dialog(window, cx);
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
                                    .child(img(logo_image(dark)).w(px(18.)).h(px(18.))),
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
                                                this.recent_submenu_open = false;
                                                cx.notify();
                                            }))
                                            .child("File"),
                                    )
                                    .when(self.open_menu == Some(OpenMenu::File), |slot| {
                                        slot.child(
                                            deferred(Self::menu_dropdown(
                                                OpenMenu::File,
                                                self.theming.theme,
                                                self.theming.dot_opacity,
                                                self.config.recent.clone(),
                                                self.recent_submenu_open,
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
                                                this.recent_submenu_open = false;
                                                cx.notify();
                                            }))
                                            .child("Edit"),
                                    )
                                    .when(self.open_menu == Some(OpenMenu::Edit), |slot| {
                                        slot.child(
                                            deferred(Self::menu_dropdown(
                                                OpenMenu::Edit,
                                                self.theming.theme,
                                                self.theming.dot_opacity,
                                                self.config.recent.clone(),
                                                self.recent_submenu_open,
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
                                                this.recent_submenu_open = false;
                                                cx.notify();
                                            }))
                                            .child("Settings"),
                                    )
                                    .when(self.open_menu == Some(OpenMenu::Settings), |slot| {
                                        slot.child(
                                            deferred(Self::menu_dropdown(
                                                OpenMenu::Settings,
                                                self.theming.theme,
                                                self.theming.dot_opacity,
                                                self.config.recent.clone(),
                                                self.recent_submenu_open,
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
                            .items_center()
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
                                        this.request_close(window, cx);
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
                            .child(self.with_spell_right_click(
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
                                cx,
                            )),
                    ),
            )
            .child(self.with_spell_status(
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
                    .child(self.status.clone()),
                colors,
                cx,
            ))
            .when(self.save.conflict_text.is_some(), |root| {
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
                                                this.load_conflict_disk(cx);
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
                                                this.keep_conflict_mine(cx);
                                            }))
                                            .child("Keep Mine"),
                                    ),
                            ),
                    )
                    .with_priority(20),
                )
            }),
            colors,
            cx,
        )
    }
}
