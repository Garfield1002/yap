use super::*;

// Extracted from editor/mod.rs. See mod.rs for the Editor struct.

impl Editor {
    pub(crate) fn ensure_shapes(&mut self, window: &mut Window, cx: &mut App) {
        let revealed = self.layout.revealed.clone();
        let caret = self.cursor();
        let palette = palette(self.theming.dark);
        let document_path = self.path.clone();
        let default_metrics = *self
            .layout
            .default_text_metrics
            .get_or_insert_with(|| default_text_metrics(window));
        for (i, block) in self.document.blocks.iter().enumerate() {
            let editing_lines = self.document.editing_lines(i, caret);
            let cache = self.layout.shapes.entry(block.id).or_default();
            if cache.rendered.is_none() {
                let lines = block
                    .rendered
                    .iter()
                    .map(|l| shape_render(l, document_path.as_deref(), palette, default_metrics, window))
                    .collect::<Vec<_>>();
                cache.rendered_rows = rows(&lines);
                cache.rendered = Some(lines);
                self.document.counters.rendered_reshapes += 1;
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
                            default_metrics,
                            window,
                        )
                    })
                    .collect::<Vec<_>>();
                cache.raw_rows = rows(&lines);
                cache.raw = Some(lines);
                self.document.counters.raw_reshapes += 1;
            }
        }
    }

    pub(crate) fn grid_widgets(&self) -> Vec<BlockWidget> {
        let mut top_row = 0;
        self.document
            .blocks
            .iter()
            .enumerate()
            .map(|(index, block)| {
                let prev = index.checked_sub(1).map(|j| self.document.blocks[j].kind);
                top_row += block_gap(prev, block.kind);
                let cache = &self.layout.shapes[&block.id];
                let widget = BlockWidget {
                    index,
                    top_row,
                    rows: cache.raw_rows.max(cache.rendered_rows).max(1),
                    render_class: match block.kind {
                        BlockKind::Blank => RenderClass::Blank,
                        BlockKind::Code => RenderClass::Code,
                        _ => RenderClass::Prose,
                    },
                };
                top_row += widget.rows;
                widget
            })
            .collect()
    }

    pub(crate) fn total_rows(&self) -> usize {
        self.grid_widgets()
            .last()
            .map_or(0, |widget| widget.top_row + widget.rows)
    }
}
