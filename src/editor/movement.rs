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

    /// Drops the shape caches for blocks that were replaced by a reparse. The
    /// fresh blocks that took their place carry new ids and so have no cache
    /// entry, reshaping on the next frame; unaffected blocks keep theirs.
    pub(crate) fn invalidate_shapes(&mut self, replaced: &[BlockId]) {
        for id in replaced {
            self.layout.shapes.remove(id);
        }
    }

    pub(crate) fn move_to(&mut self, at: usize, cx: &mut Context<Self>) {
        let at = at.min(self.document.content.len());
        self.sel.range = at..at;
        self.sel.reversed = false;
        self.sel.marked = None;
        self.sel.preferred_column = None;
        self.sel.preferred_x = None;
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
        self.sel.preferred_x = None;
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
        self.sel.preferred_x = None;
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
    /// Shapes a single source `range` belonging to `block` on demand, so
    /// wrap-aware vertical motion can resolve a sticky-x position inside a line
    /// that isn't currently revealed (e.g. the edge line of an adjacent block).
    fn shape_line(&self, range: &Range<usize>, block: usize, window: &mut Window) -> ShapedLine {
        let blk = &self.document.blocks[block];
        let code = source_line_is_code(blk.kind, blk.range.end, range.start);
        let default = self
            .layout
            .default_text_metrics
            .unwrap_or_else(|| default_text_metrics(window));
        shape_raw(
            range.clone(),
            self.document.raw_line(range),
            code,
            palette(self.theming.dark),
            default,
            window,
        )
    }

    /// Wrap-aware vertical motion, using shaped raw lines so a visually wrapped
    /// source line moves row by row like a normal editor, preserving the
    /// sticky-x column across wrap boundaries and block boundaries alike.
    /// Returns `None` (to defer to the logical `\n` fallback) only when the
    /// caret's block has no cached layout, or when moving off the top/bottom of
    /// the whole document.
    fn wrapped_vertical(&mut self, dir: isize, window: &mut Window) -> Option<usize> {
        let c = self.cursor();
        let block = self.document.block_at(c);
        let id = self.document.blocks[block].id;
        let lines = self.layout.shapes.get(&id)?.raw.as_ref()?;
        let li = lines
            .iter()
            .position(|l| l.source.start <= c && c <= l.source.end)?;
        let line = &lines[li];
        let pos = line.layout.position_for_index(c - line.source.start, px(GRID))?;
        let sticky_x = self.sel.preferred_x.unwrap_or(pos.x);
        self.sel.preferred_x = Some(sticky_x);

        // Resolve a `(shaped line, row-relative y)` to an absolute source offset.
        let resolve = |line: &ShapedLine, y: Pixels| {
            let index = line
                .layout
                .closest_index_for_position(point(sticky_x, y), px(GRID))
                .unwrap_or_else(|i| i);
            line.source.start + index
        };
        let last_row_of = |line: &ShapedLine| px(line.layout.wrap_boundaries().len() as f32 * GRID);
        if dir < 0 {
            if pos.y > px(0.) {
                // A wrapped row above, still inside this source line.
                Some(resolve(line, pos.y - px(GRID)))
            } else if let Some(prev) = lines.get(li.wrapping_sub(1)).filter(|_| li > 0) {
                // Last row of the previous source line in the same block.
                Some(resolve(prev, last_row_of(prev)))
            } else {
                // Last line of the previous block, shaped on demand.
                let pb = block.checked_sub(1)?;
                let range = self.document.blocks[pb].raw_lines.last()?.clone();
                let shaped = self.shape_line(&range, pb, window);
                Some(resolve(&shaped, last_row_of(&shaped)))
            }
        } else if pos.y < last_row_of(line) {
            // A wrapped row below, still inside this source line.
            Some(resolve(line, pos.y + px(GRID)))
        } else if let Some(next) = lines.get(li + 1) {
            // First row of the next source line in the same block.
            Some(resolve(next, px(0.)))
        } else {
            // First line of the next block, shaped on demand.
            let nb = block + 1;
            let range = self.document.blocks.get(nb)?.raw_lines.first()?.clone();
            let shaped = self.shape_line(&range, nb, window);
            Some(resolve(&shaped, px(0.)))
        }
    }
    fn vertical(&mut self, dir: isize, window: &mut Window) -> usize {
        if let Some(target) = self.wrapped_vertical(dir, window) {
            return target;
        }
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
    pub(crate) fn up(&mut self, _: &Up, window: &mut Window, cx: &mut Context<Self>) {
        let sticky = (self.sel.preferred_column, self.sel.preferred_x);
        let target = self.vertical(-1, window);
        let desired = (
            self.sel.preferred_column.or(sticky.0),
            self.sel.preferred_x.or(sticky.1),
        );
        self.move_to(target, cx);
        (self.sel.preferred_column, self.sel.preferred_x) = desired;
    }
    pub(crate) fn down(&mut self, _: &Down, window: &mut Window, cx: &mut Context<Self>) {
        let sticky = (self.sel.preferred_column, self.sel.preferred_x);
        let target = self.vertical(1, window);
        let desired = (
            self.sel.preferred_column.or(sticky.0),
            self.sel.preferred_x.or(sticky.1),
        );
        self.move_to(target, cx);
        (self.sel.preferred_column, self.sel.preferred_x) = desired;
    }
    pub(crate) fn select_up(&mut self, _: &SelectUp, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.vertical(-1, window);
        let desired = (self.sel.preferred_column, self.sel.preferred_x);
        self.select_to(target, cx);
        (self.sel.preferred_column, self.sel.preferred_x) = desired;
    }
    pub(crate) fn select_down(&mut self, _: &SelectDown, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.vertical(1, window);
        let desired = (self.sel.preferred_column, self.sel.preferred_x);
        self.select_to(target, cx);
        (self.sel.preferred_column, self.sel.preferred_x) = desired;
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
        // Coalesce a burst of presses the user hasn't seen the result of yet.
        if self.awaiting_repaint {
            return;
        }
        if let Some(tx) = self.history.undo.pop() {
            let replaced = self.document.apply_inverse(&tx);
            self.invalidate_shapes(&replaced);
            self.sel.range = tx.before_selection.clone();
            self.sel.reversed = tx.before_reversed;
            self.history.redo.push(tx);
            self.sync_revealed();
            self.sel.ensure_caret_visible = true;
            self.sel.preferred_column = None;
            self.sel.preferred_x = None;
            self.awaiting_repaint = true;
            cx.notify();
        }
    }
    pub(crate) fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        // Coalesce a burst of presses the user hasn't seen the result of yet.
        if self.awaiting_repaint {
            return;
        }
        if let Some(tx) = self.history.redo.pop() {
            let replaced = self.document.apply_forward(&tx);
            self.invalidate_shapes(&replaced);
            self.sel.range = tx.after_selection.clone();
            self.sel.reversed = tx.after_reversed;
            self.history.undo.push(tx);
            self.sync_revealed();
            self.sel.ensure_caret_visible = true;
            self.sel.preferred_column = None;
            self.sel.preferred_x = None;
            self.awaiting_repaint = true;
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
