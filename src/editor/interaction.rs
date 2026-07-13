use super::*;

// Extracted from editor/mod.rs. See mod.rs for the Editor struct.

impl Editor {
    fn toggle(&mut self, mark: &str, cx: &mut Context<Self>) {
        let len = mark.len();
        let r = self.sel.range.clone();
        if r.start >= len
            && self.document.content.get(r.start - len..r.start) == Some(mark)
            && self.document.content.get(r.end..r.end + len) == Some(mark)
        {
            let from = r.start - len;
            let to = r.end + len;
            let text = self.document.content[r].to_string();
            self.edit(from..to, &text, EditMode::Ordinary, true, cx);
            self.sel.range = from..from + text.len();
        } else {
            let text = format!("{mark}{}{mark}", &self.document.content[r.clone()]);
            let from = r.start;
            self.edit(r, &text, EditMode::Ordinary, true, cx);
            self.sel.range = from + len..from + text.len() - len;
        }
        self.sync_revealed();
        cx.notify();
    }
    pub(crate) fn bold(&mut self, _: &Bold, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle("**", cx);
    }
    pub(crate) fn italic(&mut self, _: &Italic, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle("*", cx);
    }
    pub(crate) fn inline_code(&mut self, _: &InlineCode, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle("`", cx);
    }
    pub(crate) fn link(&mut self, _: &Link, _: &mut Window, cx: &mut Context<Self>) {
        let start = self.sel.range.start;
        let text = format!("[{}]()", &self.document.content[self.sel.range.clone()]);
        self.edit(self.sel.range.clone(), &text, EditMode::Ordinary, true, cx);
        let caret = start + text.len() - 1;
        self.sel.range = caret..caret;
        self.sync_revealed();
        cx.notify();
    }

    pub(crate) fn index_at(&self, p: Point<Pixels>) -> usize {
        if let Some(line) = self
            .layout.hit_lines
            .iter()
            .find(|line| p.y >= line.bounds.top() && p.y <= line.bounds.bottom())
        {
            return source_offset_for_hit(line, p);
        }

        for pair in self.layout.hit_lines.windows(2) {
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

        self.layout.hit_lines
            .iter()
            .min_by(|a, b| {
                vertical_distance(a.bounds, p.y).total_cmp(&vertical_distance(b.bounds, p.y))
            })
            .map_or_else(|| self.cursor(), |line| source_offset_for_hit(line, p))
    }
    /// The task checkbox under `p`, if the point falls on a rendered task
    /// item's painted checkbox square.
    fn task_box_at(&self, p: Point<Pixels>) -> Option<TaskMark> {
        self.layout.hit_lines.iter().find_map(|line| {
            let b = checkbox_bounds(line)?;
            // Pad the target a little so the box is comfortable to click.
            let pad = px(3.);
            (p.x >= b.left() - pad
                && p.x <= b.right() + pad
                && p.y >= b.top() - pad
                && p.y <= b.bottom() + pad)
                .then(|| line.task.clone())
                .flatten()
        })
    }

    fn toggle_task(&mut self, mark: &TaskMark, cx: &mut Context<Self>) {
        let prev_range = self.sel.range.clone();
        let prev_reversed = self.sel.reversed;
        let prev_revealed = self.layout.revealed.clone();
        let replacement = if mark.checked { "[ ]" } else { "[x]" };
        self.edit(mark.box_range.clone(), replacement, EditMode::Ordinary, true, cx);
        // The box keeps its length, so the prior offsets stay valid: restore the
        // selection and revealed set so toggling never moves the caret or pops
        // the item open into source view.
        self.sel.range = prev_range;
        self.sel.reversed = prev_reversed;
        self.sel.ensure_caret_visible = false;
        self.layout.revealed = prev_revealed;
        // Re-render the affected block so its checkbox glyph reflects the toggle.
        let index = self.document.block_at(mark.box_range.start);
        if let Some(id) = self.document.commit_block(index)
            && let Some(cache) = self.layout.shapes.get_mut(&id)
        {
            cache.rendered = None;
            cache.rendered_rows = 0;
        }
        cx.notify();
    }

    pub(crate) fn mouse_down(&mut self, e: &MouseDownEvent, w: &mut Window, cx: &mut Context<Self>) {
        w.focus(&self.focus);
        #[cfg(feature = "spellcheck")]
        if self.spell_menu.take().is_some() {
            cx.notify();
        }
        if !e.modifiers.shift
            && let Some(mark) = self.task_box_at(e.position)
        {
            self.toggle_task(&mark, cx);
            return;
        }
        self.sel.selecting = true;
        let source = self.index_at(e.position);
        if e.modifiers.shift {
            self.select_to(source, cx);
        } else {
            self.move_to(source, cx);
        }
    }
    pub(crate) fn mouse_move(&mut self, e: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.sel.selecting {
            self.select_to(self.index_at(e.position), cx);
        }
    }
    pub(crate) const fn mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.sel.selecting = false;
    }
}

// Spell-check context menu (feature: `spellcheck`). Right-clicking a misspelled
// word opens a floating menu of corrections; picking one replaces the word.
#[cfg(feature = "spellcheck")]
impl Editor {
    pub(crate) fn spell_context_menu(
        &mut self,
        e: &MouseDownEvent,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        w.focus(&self.focus);
        if !self.spell_enabled {
            return;
        }
        let offset = self.index_at(e.position);
        self.spell_menu = self.spell.as_ref().and_then(|spell| {
            let word = spell.misspelling_at(offset)?;
            let suggestions = spell.suggest(&self.document.content[word.clone()]);
            Some(SpellMenu {
                position: e.position,
                word,
                suggestions,
            })
        });
        cx.notify();
    }

    pub(crate) fn apply_suggestion(
        &mut self,
        word: Range<usize>,
        replacement: &str,
        cx: &mut Context<Self>,
    ) {
        self.spell_menu = None;
        self.edit(word.clone(), replacement, EditMode::Ordinary, true, cx);
        let caret = word.start + replacement.len();
        self.sel.range = caret..caret;
        self.sync_revealed();
        cx.notify();
    }

    pub(crate) fn ignore_spelling(&mut self, word: Range<usize>, cx: &mut Context<Self>) {
        let text = self.document.content[word].to_string();
        if let Some(spell) = self.spell.as_mut() {
            spell.ignore(&text);
        }
        self.spell_menu = None;
        cx.notify();
    }
}

// Window::dummy does not exist; keep cut explicit instead of sharing the action handler.
impl Editor {
    pub(crate) fn cut_impl(&mut self, cx: &mut Context<Self>) {
        if !self.sel.range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.document.content[self.sel.range.clone()].to_string(),
            ));
            let mode = if self.document.touched_blocks(&self.sel.range).len() > 1 {
                EditMode::CrossBlock
            } else {
                EditMode::Ordinary
            };
            self.edit(self.sel.range.clone(), "", mode, true, cx);
        }
    }
}

