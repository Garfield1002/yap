use super::*;

// Extracted from editor/mod.rs. Additional inherent methods on Editor;
// child module reaches Editor's private fields via the parent module.

impl Editor {
    fn replace_document(&mut self, path: Option<PathBuf>, content: String, cx: &mut Context<Self>) {
        self.path = path;
        self.document = DocumentModel::new(content.clone());
        self.sel.range = 0..0;
        self.sel.reversed = false;
        self.sel.marked = None;
        self.layout.revealed = HashSet::from([self.document.blocks[0].id]);
        self.layout.shapes.clear();
        self.layout.hit_lines.clear();
        self.history.undo.clear();
        self.history.redo.clear();
        self.save.saved_text = content;
        self.save.dirty = false;
        self.save.autosave_generation += 1;
        self.save.watch_generation += 1;
        self.save.watcher = None;
        self.save.conflict_text = None;
        self.status = "saved".into();
        cx.notify();
    }

    fn new_document(&mut self, cx: &mut Context<Self>) {
        self.replace_document(None, String::new(), cx);
        self.status = "untitled".into();
    }

    pub(crate) fn new_document_with_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.save.dirty {
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
                let _ = this.update(cx, Self::new_document);
            }
        })
        .detach();
    }

    pub(crate) fn open_with_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.save.dirty {
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
                let _ = this.update_in(cx, Self::open_dialog);
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

    pub(crate) fn open_path_with_prompt(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.save.dirty {
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

    pub(crate) fn save_as_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
                    editor.layout.shapes.clear();
                    persistence::push_recent(&mut editor.config, &path);
                    let _ = persistence::save_config(&editor.config);
                    if let Err(error) = editor.save_now() {
                        editor.status = format!("save failed: {error}");
                    } else {
                        editor.start_watch(cx);
                        if editor.save.close_after_save {
                            editor.save.close_after_save = false;
                            window.remove_window();
                        }
                    }
                    cx.notify();
                });
            } else {
                let _ = this.update_in(cx, |editor, _, _| {
                    editor.save.close_after_save = false;
                });
            }
        })
        .detach();
    }

    pub(crate) fn rename_dialog(&mut self, _: &mut Window, cx: &mut Context<Self>) {
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
                            editor.layout.shapes.clear();
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

    pub(crate) fn delete_with_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

    pub(crate) fn new_window(&self) {
        if let Ok(executable) = std::env::current_exe() {
            let _ = std::process::Command::new(executable).arg("--new").spawn();
        }
    }

    pub fn request_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.save.dirty {
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
                        editor.save.close_after_save = true;
                        editor.save_as_dialog(window, cx);
                    } else if editor.save_now().is_ok() {
                        window.remove_window();
                    }
                });
            }
            Ok(1) => {
                let _ = this.update_in(cx, |editor, window, _| {
                    editor.save.dirty = false;
                    window.remove_window();
                });
            }
            _ => {}
        })
        .detach();
    }

    pub(crate) fn quit(&mut self, _: &Quit, window: &mut Window, cx: &mut Context<Self>) {
        self.request_close(window, cx);
    }

    pub fn start_watch(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.path.clone() else {
            return;
        };
        self.save.watch_generation += 1;
        let generation = self.save.watch_generation;
        let (sender, receiver) = async_channel::bounded(1);
        match persistence::watch_file(&path, move || {
            let _ = sender.try_send(());
        }) {
            Ok(watcher) => self.save.watcher = Some(watcher),
            Err(error) => {
                self.status = format!("watch failed: {error}");
                return;
            }
        }
        cx.spawn(async move |this, cx| {
            while receiver.recv().await.is_ok() {
                let keep_watching = this
                    .update(cx, |editor, cx| {
                        if editor.save.watch_generation != generation {
                            return false;
                        }
                        let Ok(disk) = fs::read_to_string(&path) else {
                            return true;
                        };
                        if disk == editor.save.saved_text {
                            return true;
                        }
                        if editor.save.dirty {
                            editor.save.conflict_text = Some(disk);
                            editor.status = "file changed on disk — conflict".into();
                        } else {
                            editor.document = DocumentModel::new(disk.clone());
                            editor.save.saved_text = disk;
                            editor.sel.range = 0..0;
                            editor.layout.revealed = HashSet::from([editor.document.blocks[0].id]);
                            editor.layout.shapes.clear();
                            editor.history.undo.clear();
                            editor.history.redo.clear();
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

    pub(crate) fn keep_conflict_mine(&mut self, cx: &mut Context<Self>) {
        self.save.conflict_text = None;
        if let Err(error) = self.save_now() {
            self.status = format!("save failed: {error}");
        }
        cx.notify();
    }

    pub(crate) fn load_conflict_disk(&mut self, cx: &mut Context<Self>) {
        let Some(disk) = self.save.conflict_text.take() else {
            return;
        };
        let path = self.path.clone();
        self.replace_document(path, disk, cx);
        self.start_watch(cx);
        self.status = "loaded disk version".into();
    }

    pub(crate) const fn cursor(&self) -> usize {
        if self.sel.reversed {
            self.sel.range.start
        } else {
            self.sel.range.end
        }
    }
}
