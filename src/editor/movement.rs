use super::*;

// Extracted from editor/mod.rs. Additional inherent methods on Editor;
// child module reaches Editor's private fields via the parent module.

impl Editor {
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
        let touched = self.document.touched_blocks(&self.sel.range);
        self.document.blocks[touched].iter().map(|b| b.id).collect()
    }

    pub(crate) fn sync_revealed(&mut self) {
        let next = self.desired_revealed();
        let leaving: Vec<_> = self.layout.revealed.difference(&next).copied().collect();
        for id in leaving {
            if let Some(index) = self.document.blocks.iter().position(|b| b.id == id) {
                self.document.commit_block(index);
                if let Some(cache) = self.layout.shapes.get_mut(&id) {
                    cache.rendered = None;
                    cache.rendered_rows = 0;
                }
            }
        }
        self.layout.revealed = next;
    }

    pub(crate) fn move_to(&mut self, at: usize, cx: &mut Context<Self>) {
        let at = at.min(self.document.content.len());
        self.sel.range = at..at;
        self.sel.reversed = false;
        self.sel.marked = None;
        self.sel.preferred_column = None;
        self.sel.ensure_caret_visible = true;
        self.sync_revealed();
        cx.notify();
    }
    pub(crate) fn select_to(&mut self, at: usize, cx: &mut Context<Self>) {
        let at = at.min(self.document.content.len());
        if self.sel.reversed {
            self.sel.range.start = at;
        } else {
            self.sel.range.end = at;
        }
        if self.sel.range.end < self.sel.range.start {
            self.sel.reversed = !self.sel.reversed;
            self.sel.range = self.sel.range.end..self.sel.range.start;
        }
        self.sync_revealed();
        self.sel.preferred_column = None;
        self.sel.ensure_caret_visible = true;
        cx.notify();
    }

    pub(crate) fn edit(
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
                .apply_edit(range, text, mode, self.sel.range.clone(), self.sel.reversed);
        self.sel.range = tx.after_selection.clone();
        self.sel.reversed = false;
        self.sel.marked = None;
        self.sel.preferred_column = None;
        self.sel.ensure_caret_visible = true;
        if record {
            self.history.undo.push(tx);
            if self.history.undo.len() > 500 {
                self.history.undo.remove(0);
            }
            self.history.redo.clear();
        }
        if ordinary {
            for id in invalidated {
                if let Some(cache) = self.layout.shapes.get_mut(&id) {
                    cache.raw = None;
                    cache.raw_rows = 0;
                }
            }
        } else {
            for id in old_ids {
                if !self.document.blocks.iter().any(|b| b.id == id) {
                    self.layout.shapes.remove(&id);
                }
            }
        }
        self.sync_revealed();
        self.status = "unsaved".into();
        self.save.dirty = true;
        self.schedule_autosave(cx);
        cx.notify();
    }

    /// Inserts an auto-closed delimiter pair. A bare caret lands between the
    /// delimiters; a non-empty range is wrapped and stays selected.
    pub(crate) fn close_delimiter(
        &mut self,
        range: Range<usize>,
        opening: &str,
        close: char,
        cx: &mut Context<Self>,
    ) {
        let selected = self.document.content[range.clone()].to_string();
        let text = format!("{opening}{selected}{close}");
        let mode = if selected.contains('\n') || self.document.touched_blocks(&range).len() > 1 {
            EditMode::CrossBlock
        } else {
            EditMode::Ordinary
        };
        let inner = range.start + opening.len();
        self.edit(range, &text, mode, true, cx);
        self.sel.range = inner..inner + selected.len();
        cx.notify();
    }

    pub(crate) fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        let p = if self.sel.range.is_empty() {
            self.previous(self.cursor())
        } else {
            self.sel.range.start
        };
        self.move_to(p, cx);
    }
    pub(crate) fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        let p = if self.sel.range.is_empty() {
            self.next(self.cursor())
        } else {
            self.sel.range.end
        };
        self.move_to(p, cx);
    }
    pub(crate) fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous(self.cursor()), cx);
    }
    pub(crate) fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next(self.cursor()), cx);
    }
    fn vertical(&mut self, dir: isize) -> usize {
        let c = self.cursor();
        let current_block = self.document.block_at(c);
        let start = self.document.content[..c].rfind('\n').map_or(0, |i| i + 1);
        let col = self
            .sel
            .preferred_column
            .unwrap_or_else(|| self.document.content[start..c].graphemes(true).count());
        self.sel.preferred_column = Some(col);
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
        if target_block == current_block {
            target
        } else {
            self.sel.preferred_column = Some(0);
            self.document.blocks[target_block].range.start
        }
    }
    pub(crate) fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        let column = self.sel.preferred_column;
        let target = self.vertical(-1);
        let desired = self.sel.preferred_column.or(column);
        self.move_to(target, cx);
        self.sel.preferred_column = desired;
    }
    pub(crate) fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        let column = self.sel.preferred_column;
        let target = self.vertical(1);
        let desired = self.sel.preferred_column.or(column);
        self.move_to(target, cx);
        self.sel.preferred_column = desired;
    }
    pub(crate) fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        let target = self.vertical(-1);
        let desired = self.sel.preferred_column;
        self.select_to(target, cx);
        self.sel.preferred_column = desired;
    }
    pub(crate) fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        let target = self.vertical(1);
        let desired = self.sel.preferred_column;
        self.select_to(target, cx);
        self.sel.preferred_column = desired;
    }
    pub(crate) fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        let target = if self.sel.range.is_empty() {
            previous_word_boundary(&self.document.content, self.cursor())
        } else {
            self.sel.range.start
        };
        self.move_to(target, cx);
    }
    pub(crate) fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        let target = if self.sel.range.is_empty() {
            next_word_boundary(&self.document.content, self.cursor())
        } else {
            self.sel.range.end
        };
        self.move_to(target, cx);
    }
    pub(crate) fn select_word_left(&mut self, _: &SelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(
            previous_word_boundary(&self.document.content, self.cursor()),
            cx,
        );
    }
    pub(crate) fn select_word_right(&mut self, _: &SelectWordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(
            next_word_boundary(&self.document.content, self.cursor()),
            cx,
        );
    }
    pub(crate) fn delete_word_backward(
        &mut self,
        _: &DeleteWordBackward,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = if self.sel.range.is_empty() {
            previous_word_boundary(&self.document.content, self.cursor())..self.cursor()
        } else {
            self.sel.range.clone()
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
        self.edit(range, "", mode, true, cx);
    }
    pub(crate) fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        let p = self.document.content[..self.cursor()]
            .rfind('\n')
            .map_or(0, |i| i + 1);
        self.move_to(p, cx);
    }
    pub(crate) fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        let c = self.cursor();
        let p = self.document.content[c..]
            .find('\n')
            .map_or(self.document.content.len(), |i| c + i);
        self.move_to(p, cx);
    }
    pub(crate) fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.sel.range = 0..self.document.content.len();
        self.sel.reversed = false;
        self.sync_revealed();
        cx.notify();
    }
    pub(crate) fn enter(&mut self, _: &Enter, _: &mut Window, cx: &mut Context<Self>) {
        if self.sel.range.is_empty() {
            match self.document.list_continuation(self.cursor()) {
                Some(ListContinuation::Clear(range)) => {
                    self.edit(range, "", EditMode::CrossBlock, true, cx);
                    return;
                }
                Some(ListContinuation::Continue(text)) => {
                    self.edit(self.sel.range.clone(), &text, EditMode::CrossBlock, true, cx);
                    return;
                }
                None => {}
            }
        }
        self.edit(self.sel.range.clone(), "\n", EditMode::Enter, true, cx);
    }
    pub(crate) fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        let c = self.cursor();
        let block = self.document.block_at(c);
        let mode = if self.sel.range.is_empty()
            && block > 0
            && c == self.document.blocks[block].range.start
        {
            EditMode::FusePrevious
        } else {
            EditMode::Ordinary
        };
        let r = if self.sel.range.is_empty() {
            self.previous(c)..c
        } else {
            self.sel.range.clone()
        };
        if !r.is_empty() {
            self.edit(r, "", mode, true, cx);
        }
    }
    pub(crate) fn delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        let c = self.cursor();
        let block = self.document.block_at(c);
        let mode = if self.sel.range.is_empty()
            && block + 1 < self.document.blocks.len()
            && c == self.document.blocks[block].range.end
        {
            EditMode::FuseNext
        } else {
            EditMode::Ordinary
        };
        let r = if self.sel.range.is_empty() {
            c..self.next(c)
        } else {
            self.sel.range.clone()
        };
        if !r.is_empty() {
            self.edit(r, "", mode, true, cx);
        }
    }
    pub(crate) fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.sel.range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.document.content[self.sel.range.clone()].to_string(),
            ));
        }
    }
    pub(crate) fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
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
                        self.sel.range.clone(),
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
                if t.contains('\n') || self.document.touched_blocks(&self.sel.range).len() > 1 {
                    EditMode::CrossBlock
                } else {
                    EditMode::Ordinary
                };
            self.edit(self.sel.range.clone(), &t, mode, true, cx);
        }
    }
    pub(crate) fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(tx) = self.history.undo.pop() {
            self.document.apply_inverse(&tx);
            self.sel.range = tx.before_selection.clone();
            self.sel.reversed = tx.before_reversed;
            self.history.redo.push(tx);
            self.layout.shapes.clear();
            self.sync_revealed();
            self.sel.ensure_caret_visible = true;
            self.sel.preferred_column = None;
            cx.notify();
        }
    }
    pub(crate) fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(tx) = self.history.redo.pop() {
            self.document.apply_forward(&tx);
            self.sel.range = tx.after_selection.clone();
            self.sel.reversed = tx.after_reversed;
            self.history.undo.push(tx);
            self.layout.shapes.clear();
            self.sync_revealed();
            self.sel.ensure_caret_visible = true;
            self.sel.preferred_column = None;
            cx.notify();
        }
    }
    pub(crate) fn save_now(&mut self) -> Result<(), String> {
        let path = self.path.as_ref().ok_or_else(|| "untitled".to_string())?;
        persistence::atomic_write(path, &self.document.content)?;
        self.save.saved_text = self.document.content.clone();
        self.save.dirty = false;
        self.status = "saved".into();
        Ok(())
    }
    fn schedule_autosave(&mut self, cx: &mut Context<Self>) {
        self.save.autosave_generation += 1;
        if self.path.is_none() {
            return;
        }
        let generation = self.save.autosave_generation;
        let timer = cx.background_executor().timer(Duration::from_secs(5));
        cx.spawn(async move |this, cx| {
            timer.await;
            let _ = this.update(cx, |editor, cx| {
                if editor.save.autosave_generation == generation && editor.save.dirty {
                    if let Err(error) = editor.save_now() {
                        editor.status = format!("autosave failed: {error}");
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }
    pub(crate) fn save(&mut self, _: &Save, window: &mut Window, cx: &mut Context<Self>) {
        if self.path.is_none() {
            self.save_as_dialog(window, cx);
            return;
        }
        if let Err(error) = self.save_now() {
            self.status = format!("save failed: {error}");
        }
        cx.notify();
    }
    pub(crate) fn escape(&mut self, _: &Escape, window: &mut Window, cx: &mut Context<Self>) {
        let anchor = self.cursor();
        let old_block = self.document.block_at(anchor);
        if let Some(line) = self.layout.hit_lines.iter().find(|line| line.block == old_block) {
            self.sel.pending_anchor = Some((anchor, line.bounds.top()));
        }
        self.document.global_reparse();
        self.layout.shapes.clear();
        self.layout.revealed.clear();
        self.sel.range =
            anchor.min(self.document.content.len())..anchor.min(self.document.content.len());
        window.blur();
        cx.notify();
    }
}
