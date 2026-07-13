use super::*;

// Extracted from editor/mod.rs. See mod.rs for the Editor struct.

impl Focusable for Editor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
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
                                                self.theming.theme,
                                                self.theming.dot_opacity,
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
                                                self.theming.theme,
                                                self.theming.dot_opacity,
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
                            .items_center()
                            .justify_end()
                            .child(
                                div()
                                    .flex_none()
                                    .px(px(10.))
                                    .flex()
                                    .items_center()
                                    .child(img(logo_image(dark)).w(px(18.)).h(px(18.))),
                            )
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
                    .child(self.status.clone()),
            )
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
