use std::{
    ops::Range,
    sync::Arc,
};

use crate::{
    editor::Editor,
    layout::{DOCUMENT_WIDTH, FIRST_BASELINE, GRID, INSET, PROSE_FONT, WRAP_WIDTH},
    model::TaskMark,
    shaping::line_rows,
    theme::{alpha, color, palette},
};
use gpui::{
    App, Bounds, CursorStyle, Element, ElementId, ElementInputHandler, Entity, GlobalElementId,
    Hitbox, HitboxBehavior, Hsla, IntoElement, LayoutId, PaintQuad,
    Pixels, Point, RenderImage, SharedString, Style, TextAlign, TextRun, Window, WrappedLine,
    fill, font, point, px, size,
};
#[cfg(feature = "spellcheck")]
use gpui::UnderlineStyle;

/// Width of the selection stub drawn for a selected trailing newline, so an
/// empty selected line (or a line break) shows something instead of collapsing
/// to zero width.
const NEWLINE_STUB: f32 = 8.;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum RenderClass {
    Blank,
    Prose,
    Code,
}

pub(crate) trait GridWidget {
    fn block_index(&self) -> usize;
    fn top_row(&self) -> usize;
    fn rows(&self) -> usize;
    fn render_class(&self) -> RenderClass;
}

#[derive(Clone, Copy)]
pub(crate) struct BlockWidget {
    pub(crate) index: usize,
    pub(crate) top_row: usize,
    pub(crate) rows: usize,
    pub(crate) render_class: RenderClass,
}

impl GridWidget for BlockWidget {
    fn block_index(&self) -> usize {
        self.index
    }

    fn top_row(&self) -> usize {
        self.top_row
    }

    fn rows(&self) -> usize {
        self.rows
    }

    fn render_class(&self) -> RenderClass {
        self.render_class
    }
}

#[derive(Clone)]
pub struct HitLine {
    pub block: usize,
    pub source: Range<usize>,
    pub bounds: Bounds<Pixels>,
    pub paint_origin: Point<Pixels>,
    pub layout: WrappedLine,
    pub map: Option<Vec<usize>>,
    pub task: Option<TaskMark>,
}

pub(crate) fn source_offset_for_hit(line: &HitLine, position: Point<Pixels>) -> usize {
    let local = point(
        position.x - line.paint_origin.x,
        position.y - line.paint_origin.y,
    );
    let display = line
        .layout
        .closest_index_for_position(local, px(GRID))
        .unwrap_or_else(|index| index);
    line.map.as_ref().map_or_else(
        || line.source.start + display.min(line.source.len()),
        |map| {
            map.get(display.min(map.len().saturating_sub(1)))
                .copied()
                .unwrap_or(line.source.start)
        },
    )
}

/// The pixel span and vertical offset of the portion of source range `word`
/// that falls on `line`, for painting a spell-check underline. Returns the left
/// and right x (already in absolute coordinates) plus the wrapped-row y offset
/// within the line. `None` if the word does not resolve to positions on a single
/// visual row of this line.
#[cfg(feature = "spellcheck")]
fn spell_underline_span(line: &HitLine, word: &Range<usize>) -> Option<(Pixels, Pixels, Pixels)> {
    // Raw (revealed) lines carry a real `source` range and no map; rendered
    // lines carry `source = 0..0` and map every rendered byte to its absolute
    // source offset, so their covered source range comes from the map itself.
    let (display_start, display_end) = match &line.map {
        None => {
            let start = word.start.max(line.source.start);
            let end = word.end.min(line.source.end);
            if end <= start {
                return None;
            }
            (start - line.source.start, end - line.source.start)
        }
        Some(map) => {
            let src_start = *map.first()?;
            let src_end = *map.last()?;
            if word.end <= src_start || word.start >= src_end {
                return None;
            }
            // The map is sticky at span boundaries: the entry for the first
            // rendered byte of a styled run keeps the *previous* run's source
            // offset (see `append_mapped` in model.rs). So the start uses a floor
            // lookup — the last rendered byte whose source offset is still at or
            // before the word — while the end uses the first byte at or past it.
            (
                map.iter().rposition(|&o| o <= word.start).unwrap_or(0),
                map.iter().position(|&o| o >= word.end).unwrap_or(map.len()),
            )
        }
    };
    let a = line.layout.position_for_index(display_start, px(GRID))?;
    let z = line.layout.position_for_index(display_end, px(GRID))?;
    // Skip words that wrap across visual rows — a rare edge in this POC.
    if a.y != z.y || z.x <= a.x {
        return None;
    }
    Some((line.paint_origin.x + a.x, line.paint_origin.x + z.x, a.y))
}

/// Byte offsets of the blank region reserved for the checkbox within a task
/// line's rendered text, i.e. just past the `" - "` bullet up to the trailing
/// space (see `render` in `model.rs`).
const TASK_BOX_START: usize = 3;
const TASK_BOX_END: usize = 7;

/// The square drawn for a task item's checkbox on line `line`, or `None` when
/// the line is not a task item. The square is centred — both horizontally
/// within the blank region reserved after the `" - "` bullet, and vertically on
/// the text glyphs so it aligns with the text rather than the grid row.
pub(crate) fn checkbox_bounds(line: &HitLine) -> Option<Bounds<Pixels>> {
    line.task.as_ref()?;
    let ascent = f32::from(line.layout.ascent());
    let descent = f32::from(line.layout.descent());
    // Size the box to roughly the font's x-height and rest it just below the
    // baseline (dipping slightly into the descent) so it lines up with the
    // lowercase letters beside it. `paint_origin.y` is the top of the glyph
    // body; the baseline sits an `ascent` below it.
    let side = ascent * 0.72;
    let baseline = f32::from(line.paint_origin.y) + ascent;
    let box_top = descent.mul_add(0.5, baseline) - side;
    let index_x = |i| {
        line.layout
            .position_for_index(i, px(GRID))
            .map_or(0., |p| f32::from(p.x))
    };
    // Centre the square horizontally within the reserved blank region.
    let region_mid = f32::midpoint(index_x(TASK_BOX_START), index_x(TASK_BOX_END));
    let left = f32::from(line.paint_origin.x) + region_mid - side / 2.;
    Some(Bounds::new(point(px(left), px(box_top)), size(px(side), px(side))))
}

pub(crate) fn inline_code_decorations(
    layout: &WrappedLine,
    origin: Point<Pixels>,
    bounds: Bounds<Pixels>,
    ranges: &[Range<usize>],
) -> Vec<Bounds<Pixels>> {
    const HORIZONTAL_PADDING: f32 = 3.;
    const VERTICAL_PADDING: f32 = 2.;
    let mut result = Vec::new();
    for range in ranges {
        let Some(start) = layout.position_for_index(range.start, px(GRID)) else {
            continue;
        };
        let Some(end) = layout.position_for_index(range.end, px(GRID)) else {
            continue;
        };
        let push = |result: &mut Vec<Bounds<Pixels>>,
                    y: Pixels,
                    left: Pixels,
                    right: Pixels| {
            let left = (origin.x + left - px(HORIZONTAL_PADDING)).max(bounds.left());
            let right = (origin.x + right + px(HORIZONTAL_PADDING)).min(bounds.right());
            if right > left {
                result.push(Bounds::new(
                    point(left, origin.y + y - px(VERTICAL_PADDING)),
                    size(
                        right - left,
                        px(VERTICAL_PADDING.mul_add(2., GRID) + 2.),
                    ),
                ));
            }
        };
        if start.y == end.y {
            push(&mut result, start.y, start.x, end.x);
        } else {
            push(&mut result, start.y, start.x, bounds.size.width);
            push(&mut result, end.y, px(0.), end.x);
        }
    }
    result
}

pub(crate) fn vertical_distance(bounds: Bounds<Pixels>, y: Pixels) -> f32 {
    if y < bounds.top() {
        (bounds.top() - y).into()
    } else if y > bounds.bottom() {
        (y - bounds.bottom()).into()
    } else {
        0.
    }
}

pub(crate) fn paint_outline(window: &mut Window, bounds: Bounds<Pixels>, color: Hsla) {
    let stroke = px(1.);
    window.paint_quad(fill(
        Bounds::new(bounds.origin, size(bounds.size.width, stroke)),
        color,
    ));
    window.paint_quad(fill(
        Bounds::new(
            point(bounds.left(), bounds.bottom() - stroke),
            size(bounds.size.width, stroke),
        ),
        color,
    ));
    window.paint_quad(fill(
        Bounds::new(bounds.origin, size(stroke, bounds.size.height)),
        color,
    ));
    window.paint_quad(fill(
        Bounds::new(
            point(bounds.right() - stroke, bounds.top()),
            size(stroke, bounds.size.height),
        ),
        color,
    ));
}

pub struct DocumentElement {
    pub editor: Entity<Editor>,
}

pub struct Prepared {
    pub lines: Vec<HitLine>,
    pub code_slabs: Vec<(usize, Bounds<Pixels>)>,
    pub inline_code_boxes: Vec<Bounds<Pixels>>,
    pub images: Vec<PreparedImage>,
    pub cursor: Option<PaintQuad>,
    pub selection: Vec<PaintQuad>,
    pub visible: Bounds<Pixels>,
    pub anchor_delta: Option<Pixels>,
    /// Task checkboxes to paint, each with a hitbox (for the pointer cursor) and
    /// its checked state.
    pub checkboxes: Vec<(Hitbox, bool)>,
}

pub struct PreparedImage {
    pub surface: Bounds<Pixels>,
    pub bounds: Option<Bounds<Pixels>>,
    pub data: Option<Arc<RenderImage>>,
    pub fallback: Option<(WrappedLine, Point<Pixels>)>,
}

impl IntoElement for DocumentElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for DocumentElement {
    type RequestLayoutState = ();
    type PrepaintState = Prepared;
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        self.editor.update(cx, |e, cx| e.ensure_shapes(window, cx));
        let rows = self.editor.read(cx).total_rows();
        let mut s = Style::default();
        s.size.width = px(DOCUMENT_WIDTH).into();
        s.size.height = px((rows as f32).mul_add(GRID, FIRST_BASELINE) + GRID).into();
        (window.request_layout(s, [], cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        (): &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Prepared {
        let e = self.editor.read(cx);
        let colors = palette(e.theming.dark);
        let visible = window.content_mask().bounds;
        let overscan = Bounds::from_corners(
            point(visible.left(), visible.top() - visible.size.height),
            point(visible.right(), visible.bottom() + visible.size.height),
        );
        let mut lines = Vec::new();
        let mut code_slabs = Vec::new();
        let mut inline_code_boxes = Vec::new();
        let mut images = Vec::new();
        let mut cursor = None;
        let mut selections = Vec::new();
        let mut anchor_delta = None;
        for widget in e.grid_widgets() {
            let bi = widget.block_index();
            let b = &e.document.blocks[bi];
            let c = &e.layout.shapes[&b.id];
            let shown = if e.layout.revealed.contains(&b.id) {
                c.raw.as_ref().unwrap()
            } else {
                c.rendered.as_ref().unwrap()
            };
            let block_rows = widget.rows();
            let block_top =
                bounds.top() + px((widget.top_row() as f32).mul_add(GRID, FIRST_BASELINE) - GRID);
            let block_bottom = block_top + px(block_rows as f32 * GRID);
            let active = e.layout.revealed.contains(&b.id);
            if let Some((source, screen_y)) = e.sel.pending_anchor
                && bi == e.document.block_at(source)
            {
                anchor_delta = Some(screen_y - block_top);
            }
            if active || block_bottom >= overscan.top() && block_top <= overscan.bottom() {
                if widget.render_class() == RenderClass::Code {
                    code_slabs.push((
                        bi,
                        Bounds::from_corners(
                            point(bounds.left() + px(INSET), block_top),
                            point(bounds.left() + px(INSET + WRAP_WIDTH), block_bottom),
                        ),
                    ));
                }
                let mut row = 0;
                for line in shown {
                    if line.source.start > b.range.end {
                        row = row.max(block_rows);
                    }
                    let n = line_rows(line);
                    let baseline =
                        bounds.top() + px(((widget.top_row() + row) as f32).mul_add(GRID, FIRST_BASELINE));
                    let pad = (px(GRID) - line.ascent - line.descent) / 2.;
                    let paint_top = baseline - pad - line.ascent;
                    let cell_top = block_top + px(row as f32 * GRID);
                    let code_inset = if line.code { 12. } else { 0. };
                    let lb = Bounds::new(
                        point(bounds.left() + px(INSET + code_inset), cell_top),
                        size(px(WRAP_WIDTH - code_inset * 2.), px(n as f32 * GRID)),
                    );
                    if let Some(image) = &line.image {
                        let image_bounds = image.data.as_ref().map(|data| {
                            let dimensions = data.size(0);
                            let ratio = dimensions.width.0.max(1) as f32
                                / dimensions.height.0.max(1) as f32;
                            let height = n as f32 * GRID;
                            let width = (height * ratio).min(WRAP_WIDTH);
                            Bounds::new(
                                point(lb.left() + (px(WRAP_WIDTH) - px(width)) / 2., lb.top()),
                                size(px(width), px(height)),
                            )
                        });
                        let fallback = image.failed.then(|| {
                            let text = if image.alt.is_empty() {
                                "Image failed to load".to_string()
                            } else {
                                image.alt.clone()
                            };
                            let layout = window
                                .text_system()
                                .shape_text(
                                    SharedString::from(text.clone()),
                                    px(13.5),
                                    &[TextRun {
                                        len: text.len(),
                                        font: font(PROSE_FONT),
                                        color: color(0xcf222e),
                                        background_color: None,
                                        underline: None,
                                        strikethrough: None,
                                    }],
                                    Some(px(WRAP_WIDTH - 16.)),
                                    None,
                                )
                                .unwrap()
                                .remove(0);
                            let origin = point(lb.left() + px(8.), lb.top() + px(GRID / 2.));
                            (layout, origin)
                        });
                        images.push(PreparedImage {
                            surface: lb,
                            bounds: image_bounds,
                            data: image.data.clone(),
                            fallback,
                        });
                    }
                    let paint_origin = point(lb.left(), paint_top);
                    inline_code_boxes.extend(inline_code_decorations(
                        &line.layout,
                        paint_origin,
                        lb,
                        &line.inline_code,
                    ));
                    let hit = HitLine {
                        block: bi,
                        source: line.source.clone(),
                        bounds: lb,
                        paint_origin,
                        layout: line.layout.clone(),
                        map: line.map.clone(),
                        task: line.task.clone(),
                    };
                    if active && line.map.is_none() {
                        let pos = e
                            .cursor()
                            .saturating_sub(line.source.start)
                            .min(line.source.len());
                        if line.source.start <= e.cursor()
                            && e.cursor() <= line.source.end
                            && e.sel.range.is_empty()
                            && let Some(p) = line.layout.position_for_index(pos, px(GRID)) {
                                cursor = Some(fill(
                                    Bounds::new(
                                        point(paint_origin.x + p.x, paint_origin.y + p.y),
                                        size(px(1.5), px(GRID)),
                                    ),
                                    colors.accent,
                                ));
                            }
                        let sel = &e.sel.range;
                        if sel.start <= line.source.end
                            && sel.end >= line.source.start
                            && let (Some(a), Some(z)) = (
                                line.layout.position_for_index(
                                    sel.start.max(line.source.start) - line.source.start,
                                    px(GRID),
                                ),
                                line.layout.position_for_index(
                                    sel.end.min(line.source.end) - line.source.start,
                                    px(GRID),
                                ),
                            ) {
                                // The newline ending this line is selected when the
                                // selection continues past its last character. Draw a
                                // small stub for it so selecting an empty line (or the
                                // line break at the end of any line) stays visible.
                                let newline_selected = sel.end > line.source.end;
                                let right = paint_origin.x
                                    + z.x
                                    + if newline_selected { px(NEWLINE_STUB) } else { px(0.) };
                                if right > paint_origin.x + a.x {
                                    selections.push(fill(
                                        Bounds::from_corners(
                                            point(paint_origin.x + a.x, paint_origin.y + a.y),
                                            point(right, paint_origin.y + z.y + px(GRID)),
                                        ),
                                        colors.selection,
                                    ));
                                }
                            }
                    }
                    lines.push(hit);
                    row += n;
                }
            }
        }
        let checkboxes = lines
            .iter()
            .filter_map(|line| {
                let checked = line.task.as_ref()?.checked;
                let bounds = checkbox_bounds(line)?;
                Some((window.insert_hitbox(bounds, HitboxBehavior::Normal), checked))
            })
            .collect();
        Prepared {
            lines,
            code_slabs,
            inline_code_boxes,
            images,
            cursor,
            selection: selections,
            visible,
            anchor_delta,
            checkboxes,
        }
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        (): &mut (),
        p: &mut Prepared,
        window: &mut Window,
        cx: &mut App,
    ) {
        let editor = self.editor.read(cx);
        let colors = palette(editor.theming.dark);
        let left = p.visible.left().max(bounds.left());
        let right = p.visible.right().min(bounds.right());
        let top = p.visible.top().max(bounds.top());
        let bottom = p.visible.bottom().min(bounds.bottom());
        let grid_left = bounds.left() + px(INSET);
        let grid_right = bounds.right() - px(INSET);
        let grid_top = bounds.top() + px(GRID);
        let c0 = ((left.max(grid_left) - grid_left) / px(GRID)).floor() as i32;
        // Include the closing grid post at the inner edge of the 24 px page
        // margin. The quad itself remains inside the paper bounds.
        let last_inner_column = (((grid_right - grid_left) / px(GRID)).floor() as i32).max(0);
        let c1 =
            (((right.min(grid_right) - grid_left) / px(GRID)).ceil() as i32).min(last_inner_column);
        let r0 = ((top.max(grid_top) - grid_top) / px(GRID)).floor() as i32;
        let last_row = (((bounds.bottom() - grid_top) / px(GRID)).floor() as i32 - 1).max(0);
        let r1 = (((bottom - grid_top) / px(GRID)).ceil() as i32).min(last_row);
        for r in r0..=r1 {
            for c in c0..=c1 {
                window.paint_quad(
                    fill(
                        Bounds::new(
                            point(
                                grid_left + px(c as f32 * GRID),
                                grid_top + px(r as f32 * GRID),
                            ),
                            size(px(2.), px(2.)),
                        ),
                        alpha(colors.fg, editor.theming.dot_opacity),
                    )
                    .corner_radii(px(1.)),
                );
            }
        }

        for (_, outer) in p.code_slabs.iter().copied() {
            window.paint_quad(fill(outer, colors.code_border).corner_radii(px(6.)));
            let inner = Bounds::from_corners(
                point(outer.left() + px(1.), outer.top() + px(1.)),
                point(outer.right() - px(1.), outer.bottom() - px(1.)),
            );
            window.paint_quad(fill(inner, colors.code_bg).corner_radii(px(5.)));
        }
        for image in &p.images {
            window.paint_quad(fill(image.surface, colors.bg));
            if let (Some(bounds), Some(data)) = (image.bounds, image.data.clone()) {
                let _ = window.paint_image(bounds, px(6.).into(), data, 0, false);
            } else if let Some((layout, origin)) = &image.fallback {
                paint_outline(window, image.surface, color(0xcf222e));
                let _ = layout.paint(
                    *origin,
                    px(GRID),
                    TextAlign::Left,
                    Some(image.surface),
                    window,
                    cx,
                );
            }
        }
        for q in p.selection.drain(..) {
            window.paint_quad(q);
        }
        for slab in &p.inline_code_boxes {
            window.paint_quad(fill(*slab, colors.code_border).corner_radii(px(6.)));
            let inner = Bounds::from_corners(
                point(slab.left() + px(1.), slab.top() + px(1.)),
                point(slab.right() - px(1.), slab.bottom() - px(1.)),
            );
            window.paint_quad(fill(inner, colors.code_bg).corner_radii(px(5.)));
        }
        for l in &p.lines {
            let _ = l.layout.paint(
                l.paint_origin,
                px(GRID),
                TextAlign::Left,
                Some(l.bounds),
                window,
                cx,
            );
        }
        #[cfg(feature = "spellcheck")]
        {
            let misspellings = self.editor.read(cx).misspellings().to_vec();
            if !misspellings.is_empty() {
                let style = UnderlineStyle {
                    thickness: px(1.),
                    color: Some(color(0xcf222e)),
                    wavy: true,
                };
                for line in &p.lines {
                    let ascent = line.layout.ascent();
                    for word in &misspellings {
                        if let Some((left, right, row_y)) = spell_underline_span(line, word) {
                            let origin = point(left, line.paint_origin.y + row_y + ascent + px(1.));
                            window.paint_underline(origin, right - left, &style);
                        }
                    }
                }
            }
        }
        for (hitbox, checked) in &p.checkboxes {
            let outer = hitbox.bounds;
            window.paint_quad(fill(outer, colors.code_border).corner_radii(px(4.)));
            let inner = Bounds::from_corners(
                point(outer.left() + px(1.5), outer.top() + px(1.5)),
                point(outer.right() - px(1.5), outer.bottom() - px(1.5)),
            );
            let inner_color = if *checked { colors.accent } else { colors.code_bg };
            window.paint_quad(fill(inner, inner_color).corner_radii(px(3.)));
            window.set_cursor_style(CursorStyle::PointingHand, hitbox);
        }
        let had_cursor = p.cursor.is_some();
        if let Some(q) = p.cursor.take() {
            if self.editor.read(cx).sel.ensure_caret_visible {
                let vertical = &self.editor.read(cx).vertical_scroll;
                let vertical_view = vertical.bounds();
                let mut vertical_offset = vertical.offset();
                let old_vertical = vertical_offset;
                if q.bounds.bottom() > vertical_view.bottom() {
                    vertical_offset.y -= q.bounds.bottom() - vertical_view.bottom();
                } else if q.bounds.top() < vertical_view.top() {
                    vertical_offset.y += vertical_view.top() - q.bounds.top();
                }
                vertical_offset.y = vertical_offset
                    .y
                    .clamp(-vertical.max_offset().height, px(0.));
                if vertical_offset != old_vertical {
                    vertical.set_offset(vertical_offset);
                    window.request_animation_frame();
                }

                let horizontal = &self.editor.read(cx).horizontal_scroll;
                let horizontal_view = horizontal.bounds();
                let mut horizontal_offset = horizontal.offset();
                let old_horizontal = horizontal_offset;
                if q.bounds.right() > horizontal_view.right() {
                    horizontal_offset.x -= q.bounds.right() - horizontal_view.right();
                } else if q.bounds.left() < horizontal_view.left() {
                    horizontal_offset.x += horizontal_view.left() - q.bounds.left();
                }
                horizontal_offset.x = horizontal_offset
                    .x
                    .clamp(-horizontal.max_offset().width, px(0.));
                if horizontal_offset != old_horizontal {
                    horizontal.set_offset(horizontal_offset);
                    window.request_animation_frame();
                }
            }
            window.paint_quad(q);
        }
        let applied_anchor = p.anchor_delta.is_some();
        if let Some(delta) = p.anchor_delta.take() {
            let scroll = &self.editor.read(cx).vertical_scroll;
            let mut offset = scroll.offset();
            let old_offset = offset;
            offset.y += delta;
            let max = scroll.max_offset();
            offset.y = offset.y.clamp(-max.height, px(0.));
            if offset != old_offset {
                scroll.set_offset(offset);
                window.request_animation_frame();
            }
        }
        window.handle_input(
            &self.editor.read(cx).focus,
            ElementInputHandler::new(bounds, self.editor.clone()),
            cx,
        );
        self.editor.update(cx, |e, _| {
            e.layout.hit_lines.clone_from(&p.lines);
            e.layout.doc_bounds = Some(bounds);
            if had_cursor {
                e.sel.ensure_caret_visible = false;
            }
            if applied_anchor {
                e.sel.pending_anchor = None;
            }
        });
    }
}
