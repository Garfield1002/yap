use super::*;

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
            range: utf8_to_utf16(&self.document.content, self.sel.range.start)
                ..utf8_to_utf16(&self.document.content, self.sel.range.end),
            reversed: self.sel.reversed,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.sel.marked.as_ref().map(|r| {
            utf8_to_utf16(&self.document.content, r.start)
                ..utf8_to_utf16(&self.document.content, r.end)
        })
    }
    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.sel.marked = None;
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
            .or_else(|| self.sel.marked.clone())
            .unwrap_or_else(|| self.sel.range.clone());
        let mode = if text.contains('\n') || self.document.touched_blocks(&n).len() > 1 {
            EditMode::CrossBlock
        } else {
            EditMode::Ordinary
        };
        self.edit(n, text, mode, true, cx);
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
            .or_else(|| self.sel.marked.clone())
            .unwrap_or_else(|| self.sel.range.clone());
        let start = n.start;
        self.edit(n, text, EditMode::Ordinary, true, cx);
        self.sel.marked = (!text.is_empty()).then_some(start..start + text.len());
        let relative = selected
            .map(|r| utf16_to_utf8(text, r.start)..utf16_to_utf8(text, r.end))
            .unwrap_or(text.len()..text.len());
        self.sel.range = start + relative.start..start + relative.end;
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
            .layout.hit_lines
            .iter()
            .find(|l| l.map.is_none() && l.source.start <= n && l.source.end >= n)?;
        let p = line
            .layout
            .position_for_index(n - line.source.start, px(GRID))?;
        Some(Bounds::new(
            point(line.paint_origin.x + p.x, line.paint_origin.y + p.y),
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
