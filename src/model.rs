use std::{ops::Range, time::Duration};

use pulldown_cmark::{Event, Parser, Tag, TagEnd};

pub type BlockId = u64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InlineStyle {
    pub italic: bool,
    pub strong: bool,
    pub link: bool,
    pub code: bool,
}

#[derive(Clone, Debug)]
pub struct RenderSpan {
    pub len: usize,
    pub style: InlineStyle,
}

#[derive(Clone, Debug)]
pub struct RenderLine {
    pub text: String,
    pub spans: Vec<RenderSpan>,
    /// Rendered UTF-8 byte boundary -> absolute source byte offset.
    pub source_map: Vec<usize>,
    pub level: u8,
    pub code_block: bool,
    pub image: Option<MarkdownImage>,
    /// Set when the line is a task-list item, carrying its checkbox state and
    /// the source range of the `[ ]`/`[x]` box so a click can toggle it.
    pub task: Option<TaskMark>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownImage {
    pub src: String,
    pub alt: String,
}

/// A rendered task-list checkbox: whether it is checked and the source byte
/// range of its `[ ]`/`[x]` marker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskMark {
    pub checked: bool,
    pub box_range: Range<usize>,
}

impl RenderLine {
    #[must_use] 
    pub fn source_for_rendered(&self, offset: usize) -> usize {
        self.source_map
            .get(offset.min(self.source_map.len().saturating_sub(1)))
            .copied()
            .unwrap_or(0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockKind {
    Blank,
    Paragraph,
    Heading(u8),
    List,
    Code,
}

#[derive(Clone, Debug)]
pub struct Block {
    pub id: BlockId,
    pub range: Range<usize>,
    pub raw_lines: Vec<Range<usize>>,
    pub kind: BlockKind,
    pub rendered: Vec<RenderLine>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counters {
    pub global_reparses: usize,
    pub local_reparses: usize,
    pub parsed_blocks: usize,
    pub raw_reshapes: usize,
    pub rendered_reshapes: usize,
}

#[derive(Clone, Debug)]
pub struct EditTransaction {
    pub range: Range<usize>,
    pub deleted: String,
    pub inserted: String,
    pub before_selection: Range<usize>,
    pub after_selection: Range<usize>,
    pub before_reversed: bool,
    pub after_reversed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditMode {
    Ordinary,
    Enter,
    FusePrevious,
    FuseNext,
    CrossBlock,
}

pub struct DocumentModel {
    pub content: String,
    pub blocks: Vec<Block>,
    pub counters: Counters,
    next_id: BlockId,
}

impl DocumentModel {
    #[must_use] 
    pub fn new(content: String) -> Self {
        let mut this = Self {
            content,
            blocks: Vec::new(),
            counters: Counters::default(),
            next_id: 1,
        };
        this.global_reparse();
        // Opening is the baseline, not an edit invalidation.
        this.counters = Counters::default();
        this
    }

    #[must_use] 
    pub fn block_at(&self, offset: usize) -> usize {
        if self.blocks.is_empty() {
            return 0;
        }
        self.blocks
            .iter()
            .position(|block| block.range.start <= offset && offset <= block.range.end)
            .unwrap_or_else(|| {
                self.blocks
                    .iter()
                    .rposition(|block| block.range.start <= offset)
                    .unwrap_or(0)
            })
    }

    #[must_use] 
    pub fn touched_blocks(&self, range: &Range<usize>) -> Range<usize> {
        let first = self.block_at(range.start);
        let last = self.block_at(range.end);
        first.min(last)..first.max(last) + 1
    }

    #[must_use] 
    pub fn raw(&self, block: usize) -> &str {
        let range = self.blocks[block].range.clone();
        &self.content[range]
    }

    #[must_use] 
    pub fn raw_line(&self, range: &Range<usize>) -> &str {
        &self.content[range.clone()]
    }

    /// Source lines shown while a block is being edited. Blank rows are
    /// represented by their own blocks, so a block never borrows lines from
    /// the following source block.
    #[must_use] 
    pub fn editing_lines(&self, index: usize, _caret: usize) -> Vec<Range<usize>> {
        self.blocks[index].raw_lines.clone()
    }

    pub fn commit_block(&mut self, index: usize) -> Option<BlockId> {
        let block = self.blocks.get_mut(index)?;
        block.kind = classify(&self.content[block.range.clone()]);
        block.rendered = render_block(&self.content, block);
        Some(block.id)
    }

    pub fn apply_edit(
        &mut self,
        range: Range<usize>,
        inserted: &str,
        mode: EditMode,
        before_selection: Range<usize>,
        before_reversed: bool,
    ) -> (EditTransaction, Vec<BlockId>) {
        let touched = self.touched_blocks(&range);
        let deleted = self.content[range.clone()].to_string();
        let delta = inserted.len() as isize - range.len() as isize;
        let cursor = range.start + inserted.len();

        let local_region = match mode {
            EditMode::Ordinary if touched.len() == 1 && !inserted.contains('\n') => None,
            EditMode::FusePrevious => {
                let active = touched.end - 1;
                let mut first = active.saturating_sub(1);
                while first > 0 && self.blocks[first].kind == BlockKind::Blank {
                    first -= 1;
                }
                let mut end = active + 1;
                while end < self.blocks.len() && self.blocks[end - 1].kind == BlockKind::Blank {
                    end += 1;
                }
                if self.blocks[active].kind == BlockKind::Blank && end < self.blocks.len() {
                    end += 1;
                }
                Some(first..end)
            }
            EditMode::FuseNext => {
                let active = touched.start;
                let mut start = active;
                while start > 0 && self.blocks[start].kind == BlockKind::Blank {
                    start -= 1;
                }
                let mut end = (active + 2).min(self.blocks.len());
                while end < self.blocks.len() && self.blocks[end - 1].kind == BlockKind::Blank {
                    end += 1;
                }
                Some(start..end)
            }
            _ => Some(touched.clone()),
        };

        self.content.replace_range(range.clone(), inserted);
        let mut invalidated = Vec::new();
        if let Some(region) = local_region {
            let start = self.blocks[region.start].range.start;
            let old_end = self.blocks[region.end - 1].range.end;
            let new_end = (old_end as isize + delta).max(start as isize) as usize;
            invalidated.extend(self.blocks[region.clone()].iter().map(|block| block.id));
            self.local_resegment(region, start..new_end);
        } else {
            let index = touched.start;
            let block = &mut self.blocks[index];
            invalidated.push(block.id);
            block.range.end = (block.range.end as isize + delta) as usize;
            block.raw_lines = source_lines(&self.content, block.range.clone());
            for later in &mut self.blocks[index + 1..] {
                later.range.start = (later.range.start as isize + delta) as usize;
                later.range.end = (later.range.end as isize + delta) as usize;
                for line in &mut later.raw_lines {
                    line.start = (line.start as isize + delta) as usize;
                    line.end = (line.end as isize + delta) as usize;
                }
                shift_render_maps(&mut later.rendered, delta);
            }
        }

        let transaction = EditTransaction {
            range,
            deleted,
            inserted: inserted.to_string(),
            before_selection,
            after_selection: cursor..cursor,
            before_reversed,
            after_reversed: false,
        };
        (transaction, invalidated)
    }

    fn local_resegment(&mut self, old_blocks: Range<usize>, source: Range<usize>) {
        let old_end = self.blocks[old_blocks.end - 1].range.end;
        let shift = source.end as isize - old_end as isize;
        let mut replacement = parse_region(
            &self.content,
            source.clone(),
            &mut self.next_id,
            &mut self.counters.parsed_blocks,
        );
        if replacement.is_empty() {
            replacement.push(empty_block(self.next_id, source.start));
            self.next_id += 1;
        }
        let replacement_len = replacement.len();
        self.blocks.splice(old_blocks.clone(), replacement);
        let replaced_end = old_blocks.start + replacement_len;
        for later in &mut self.blocks[replaced_end..] {
            later.range.start = (later.range.start as isize + shift) as usize;
            later.range.end = (later.range.end as isize + shift) as usize;
            for line in &mut later.raw_lines {
                line.start = (line.start as isize + shift) as usize;
                line.end = (line.end as isize + shift) as usize;
            }
            shift_render_maps(&mut later.rendered, shift);
        }
        self.counters.local_reparses += 1;
    }

    pub fn global_reparse(&mut self) {
        let mut parsed = 0;
        self.blocks = parse_region(
            &self.content,
            0..self.content.len(),
            &mut self.next_id,
            &mut parsed,
        );
        if self.blocks.is_empty() {
            self.blocks.push(empty_block(self.next_id, 0));
            self.next_id += 1;
        }
        self.counters.global_reparses += 1;
        self.counters.parsed_blocks += parsed;
    }

    pub fn apply_inverse(&mut self, tx: &EditTransaction) {
        let end = tx.range.start + tx.inserted.len();
        self.content.replace_range(tx.range.start..end, &tx.deleted);
        self.global_reparse();
    }

    pub fn apply_forward(&mut self, tx: &EditTransaction) {
        let end = tx.range.start + tx.deleted.len();
        self.content
            .replace_range(tx.range.start..end, &tx.inserted);
        self.global_reparse();
    }

    /// What pressing Enter should do when the caret sits on a list item line.
    ///
    /// Returns `None` when the caret's line is not a list item (or is inside a
    /// code block), in which case the caller inserts a plain newline.
    #[must_use]
    pub fn list_continuation(&self, cursor: usize) -> Option<ListContinuation> {
        if self.blocks[self.block_at(cursor)].kind == BlockKind::Code {
            return None;
        }
        list_continuation(&self.content, cursor)
    }
}

/// The outcome of pressing Enter on a list item line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListContinuation {
    /// Insert this text (a newline followed by the next item's marker prefix).
    Continue(String),
    /// The current item is empty: clear this byte range to exit the list.
    Clear(Range<usize>),
}

/// Computes the [`ListContinuation`] for `cursor` within `content`. Pure and
/// standalone so it can be unit-tested without a full document.
#[must_use]
fn list_continuation(content: &str, cursor: usize) -> Option<ListContinuation> {
    let line_start = content[..cursor].rfind('\n').map_or(0, |i| i + 1);
    let line_end = content[cursor..]
        .find('\n')
        .map_or(content.len(), |i| cursor + i);
    let line = &content[line_start..line_end];
    let (prefix_len, continuation) = parse_list_marker(line)?;
    if line[prefix_len..].trim().is_empty() {
        return Some(ListContinuation::Clear(line_start..line_end));
    }
    Some(ListContinuation::Continue(format!("\n{continuation}")))
}

/// Parses a leading list marker (`-`, `*`, `+`, or `<digits>.`/`<digits>)`,
/// with an optional `[ ]`/`[x]` task box) from `line`.
///
/// Returns the byte length of the whole marker prefix (indent through the space
/// before the item content) and the prefix to start the next line with: the
/// same marker for bullets, the incremented number for ordered lists, and a
/// reset `[ ]` box when the source item had one.
fn parse_list_marker(line: &str) -> Option<(usize, String)> {
    let indent_len = line.len() - line.trim_start_matches([' ', '\t']).len();
    let indent = &line[..indent_len];
    let rest = &line[indent_len..];

    let (bullet_len, next_bullet) = match rest.chars().next()? {
        c @ ('-' | '*' | '+') => (1, c.to_string()),
        '0'..='9' => {
            let digits_len = rest.find(|c: char| !c.is_ascii_digit())?;
            let delimiter = rest[digits_len..].chars().next()?;
            if delimiter != '.' && delimiter != ')' {
                return None;
            }
            let number: u64 = rest[..digits_len].parse().ok()?;
            (digits_len + 1, format!("{}{delimiter}", number.saturating_add(1)))
        }
        _ => return None,
    };

    let after_bullet = &rest[bullet_len..];
    let spaces_len = after_bullet.len() - after_bullet.trim_start_matches([' ', '\t']).len();
    if spaces_len == 0 {
        return None;
    }

    let mut prefix_len = indent_len + bullet_len + spaces_len;
    let after_marker = &line[prefix_len..];
    let task = ["[ ] ", "[x] ", "[X] "]
        .iter()
        .any(|box_| after_marker.starts_with(box_));
    if task {
        prefix_len += 4;
    }

    let continuation = format!("{indent}{next_bullet} {}", if task { "[ ] " } else { "" });
    Some((prefix_len, continuation))
}

/// The closing delimiter to auto-insert after typing `opening`.
///
/// `preceding` is the character immediately before the caret. Returns `None`
/// when `opening` is not an auto-closed delimiter, or for `_` when not preceded
/// by whitespace (so it stays usable mid-word for `snake_case`).
#[must_use]
pub fn auto_close(opening: &str, preceding: Option<char>) -> Option<char> {
    Some(match opening {
        "(" => ')',
        "[" => ']',
        "{" => '}',
        "*" => '*',
        "_" if preceding.is_none_or(char::is_whitespace) => '_',
        _ => return None,
    })
}

fn empty_block(id: BlockId, offset: usize) -> Block {
    Block {
        id,
        range: offset..offset,
        raw_lines: Vec::from([offset..offset]),
        kind: BlockKind::Paragraph,
        rendered: vec![RenderLine {
            text: String::new(),
            spans: vec![RenderSpan {
                len: 0,
                style: InlineStyle::default(),
            }],
            source_map: vec![offset],
            level: 0,
            code_block: false,
            image: None,
            task: None,
        }],
    }
}

fn parse_region(
    content: &str,
    region: Range<usize>,
    next_id: &mut BlockId,
    parsed: &mut usize,
) -> Vec<Block> {
    if region.is_empty() {
        return Vec::new();
    }
    let mut spans = Vec::new();
    let mut current_start = None;
    let mut current_end = region.start;
    let mut in_code = false;
    let mut offset = region.start;
    for segment in content[region.clone()].split_inclusive('\n') {
        let line = segment.strip_suffix('\n').unwrap_or(segment);
        let line_start = offset;
        let line_end = offset + line.len();
        offset += segment.len();
        let fence = line.trim_start().starts_with("```");
        let heading = !in_code && heading_level(line).is_some();
        if fence {
            if current_start.is_none() {
                current_start = Some(line_start);
            }
            in_code = !in_code;
            current_end = line_end;
            if !in_code {
                spans.push(current_start.take().unwrap()..current_end);
            }
        } else if heading {
            if let Some(start) = current_start.take() {
                spans.push(start..current_end);
            }
            spans.push(line_start..line_end);
            current_end = line_end;
        } else if line.trim().is_empty() && !in_code {
            if let Some(start) = current_start.take() {
                spans.push(start..current_end);
            }
            spans.push(line_start..line_end);
        } else {
            current_start.get_or_insert(line_start);
            current_end = line_end;
        }
    }
    if let Some(start) = current_start {
        spans.push(start..current_end);
    }
    if content[region.clone()].ends_with('\n') {
        // The line after a trailing newline is the new current editing line.
        // Keep it as a separate block so Enter commits the block above it.
        spans.push(region.end..region.end);
    }

    spans
        .into_iter()
        .map(|range| {
            let id = *next_id;
            *next_id += 1;
            *parsed += 1;
            let kind = classify(&content[range.clone()]);
            let mut block = Block {
                id,
                raw_lines: source_lines(content, range.clone()),
                range,
                kind,
                rendered: Vec::new(),
            };
            block.rendered = render_block(content, &block);
            block
        })
        .collect()
}

fn source_lines(content: &str, range: Range<usize>) -> Vec<Range<usize>> {
    let mut result = Vec::new();
    let mut offset = range.start;
    for segment in content[range.clone()].split_inclusive('\n') {
        let line = segment.strip_suffix('\n').unwrap_or(segment);
        result.push(offset..offset + line.len());
        offset += segment.len();
    }
    if content[range.clone()].ends_with('\n') {
        result.push(range.end..range.end);
    }
    if result.is_empty() {
        result.push(range.start..range.start);
    }
    result
}

fn heading_level(line: &str) -> Option<u8> {
    let first = line.trim_start();
    if first.starts_with("### ") {
        Some(3)
    } else if first.starts_with("## ") {
        Some(2)
    } else if first.starts_with("# ") {
        Some(1)
    } else {
        None
    }
}

fn classify(raw: &str) -> BlockKind {
    let first = raw.lines().next().unwrap_or("").trim_start();
    if raw.trim().is_empty() {
        BlockKind::Blank
    } else if first.starts_with("```") {
        BlockKind::Code
    } else if let Some(level) = heading_level(first) {
        BlockKind::Heading(level)
    } else if list_prefix(first).is_some() {
        BlockKind::List
    } else {
        BlockKind::Paragraph
    }
}

fn render_block(content: &str, block: &Block) -> Vec<RenderLine> {
    let mut result = Vec::new();
    for (line_index, range) in block.raw_lines.iter().enumerate() {
        let raw = &content[range.clone()];
        if block.kind == BlockKind::Code
            && (line_index == 0 || line_index + 1 == block.raw_lines.len())
            && raw.trim_start().starts_with("```")
        {
            // A rendered fence hides its source characters, not its row. Keeping
            // an empty mapped line here preserves the code body's vertical
            // position and makes raw/rendered slabs exactly the same height.
            result.push(RenderLine {
                text: String::new(),
                spans: vec![RenderSpan {
                    len: 0,
                    style: InlineStyle {
                        code: true,
                        ..Default::default()
                    },
                }],
                source_map: vec![range.start],
                level: 0,
                code_block: true,
                image: None,
                task: None,
            });
            continue;
        }
        let task = (block.kind == BlockKind::List)
            .then(|| task_prefix(raw, range.start))
            .flatten();
        let (prefix_len, visible_prefix, level) = match block.kind {
            BlockKind::Heading(level) => (level as usize + 1, String::new(), level),
            BlockKind::List => task.clone().map_or_else(
                || list_prefix(raw).map_or((0, String::new(), 0), |(len, prefix)| (len, prefix, 0)),
                |(len, mark)| {
                    let glyph = if mark.checked { "☑ " } else { "☐ " };
                    (len, glyph.to_string(), 0)
                },
            ),
            _ => (0, String::new(), 0),
        };
        let task = task.map(|(_, mark)| mark);
        if block.kind == BlockKind::Code {
            result.push(code_line(raw, range.start));
            continue;
        }
        if let Some(image) = standalone_image(raw) {
            result.push(RenderLine {
                text: String::new(),
                spans: vec![RenderSpan {
                    len: 0,
                    style: InlineStyle::default(),
                }],
                source_map: vec![range.start],
                level: 0,
                code_block: false,
                image: Some(image),
                task: None,
            });
            continue;
        }
        let source = &raw[prefix_len.min(raw.len())..];
        let base = range.start + prefix_len.min(raw.len());
        let (mut text, mut spans, mut map) = render_inline(source, base);
        if !visible_prefix.is_empty() {
            let mut prefix_map = if raw.get(..prefix_len) == Some(visible_prefix.as_str()) {
                (range.start..=range.start + visible_prefix.len()).collect()
            } else {
                vec![range.start; visible_prefix.len() + 1]
            };
            prefix_map.extend(map.into_iter().skip(1));
            map = prefix_map;
            text.insert_str(0, &visible_prefix);
            spans.insert(
                0,
                RenderSpan {
                    len: visible_prefix.len(),
                    style: InlineStyle::default(),
                },
            );
        }
        result.push(RenderLine {
            text,
            spans,
            source_map: map,
            level,
            code_block: false,
            image: None,
            task,
        });
    }
    if result.is_empty() {
        result.push(RenderLine {
            text: String::new(),
            spans: vec![RenderSpan {
                len: 0,
                style: InlineStyle::default(),
            }],
            source_map: vec![block.range.start],
            level: 0,
            code_block: block.kind == BlockKind::Code,
            image: None,
            task: None,
        });
    }
    result
}

fn list_prefix(raw: &str) -> Option<(usize, String)> {
    let trimmed = raw.trim_start();
    let indent = raw.len() - trimmed.len();
    if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
        return Some((indent + 2, "• ".into()));
    }
    let digits = trimmed.bytes().take_while(u8::is_ascii_digit).count();
    if digits > 0 && trimmed[digits..].starts_with(". ") {
        let len = indent + digits + 2;
        return Some((len, raw[..len].to_string()));
    }
    None
}

/// Detects a task-list item (`- [ ] `/`- [x] `, `*`/`+` bullets too) at the
/// start of `raw`, whose source begins at absolute offset `base`.
///
/// Returns the byte length of the whole `bullet + box + space` prefix to hide,
/// and the [`TaskMark`] carrying the checked state and the absolute source
/// range of the three-character `[ ]`/`[x]` box.
fn task_prefix(raw: &str, base: usize) -> Option<(usize, TaskMark)> {
    let trimmed = raw.trim_start();
    let indent = raw.len() - trimmed.len();
    let after_bullet = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("+ "))?;
    let checked = match after_bullet.get(..4) {
        Some("[ ] ") => false,
        Some("[x] " | "[X] ") => true,
        _ => return None,
    };
    let box_start = base + indent + 2;
    Some((
        indent + 2 + 4,
        TaskMark {
            checked,
            box_range: box_start..box_start + 3,
        },
    ))
}

fn code_line(raw: &str, base: usize) -> RenderLine {
    let mut map = Vec::with_capacity(raw.len() + 1);
    map.extend(base..=base + raw.len());
    RenderLine {
        text: raw.to_string(),
        spans: vec![RenderSpan {
            len: raw.len(),
            style: InlineStyle {
                code: true,
                ..Default::default()
            },
        }],
        source_map: map,
        level: 0,
        code_block: true,
        image: None,
        task: None,
    }
}

fn standalone_image(raw: &str) -> Option<MarkdownImage> {
    let trimmed = raw.trim();
    let mut depth = 0usize;
    let mut src = None;
    let mut alt = String::new();
    let mut saw_outside = false;
    for (event, _) in Parser::new(trimmed).into_offset_iter() {
        match event {
            Event::Start(Tag::Paragraph) | Event::End(TagEnd::Paragraph) if depth == 0 => {}
            Event::Start(Tag::Image { dest_url, .. }) if depth == 0 && src.is_none() => {
                depth = 1;
                src = Some(dest_url.into_string());
            }
            Event::Start(_) if depth > 0 => depth += 1,
            Event::End(TagEnd::Image) if depth == 1 => depth = 0,
            Event::End(_) if depth > 1 => depth -= 1,
            Event::Text(text) | Event::Code(text) if depth > 0 => alt.push_str(&text),
            Event::SoftBreak | Event::HardBreak if depth > 0 => alt.push(' '),
            _ if depth == 0 => saw_outside = true,
            _ => {}
        }
    }
    if saw_outside || depth != 0 {
        return None;
    }
    Some(MarkdownImage { src: src?, alt })
}

fn render_inline(source: &str, base: usize) -> (String, Vec<RenderSpan>, Vec<usize>) {
    let mut text = String::new();
    let mut spans = Vec::new();
    let mut map = vec![base];
    let mut style = InlineStyle::default();
    for (event, range) in Parser::new(source).into_offset_iter() {
        match event {
            Event::Start(Tag::Emphasis) => style.italic = true,
            Event::End(TagEnd::Emphasis) => style.italic = false,
            Event::Start(Tag::Strong) => style.strong = true,
            Event::End(TagEnd::Strong) => style.strong = false,
            Event::Start(Tag::Link { .. }) => style.link = true,
            Event::End(TagEnd::Link) => style.link = false,
            Event::Text(value) => append_mapped(
                &mut text,
                &mut spans,
                &mut map,
                &value,
                base + range.start,
                base + range.end,
                style,
            ),
            Event::Code(value) => append_mapped(
                &mut text,
                &mut spans,
                &mut map,
                &value,
                base + range.start + 1,
                base + range.end.saturating_sub(1),
                InlineStyle {
                    code: true,
                    ..style
                },
            ),
            Event::SoftBreak | Event::HardBreak => append_mapped(
                &mut text,
                &mut spans,
                &mut map,
                " ",
                base + range.start,
                base + range.end,
                style,
            ),
            _ => {}
        }
    }
    if spans.is_empty() {
        spans.push(RenderSpan { len: 0, style });
    }
    (text, spans, map)
}

fn append_mapped(
    text: &mut String,
    spans: &mut Vec<RenderSpan>,
    map: &mut Vec<usize>,
    value: &str,
    source_start: usize,
    source_end: usize,
    style: InlineStyle,
) {
    if text.is_empty() {
        map[0] = source_start;
    }
    text.push_str(value);
    spans.push(RenderSpan {
        len: value.len(),
        style,
    });
    for byte in 1..=value.len() {
        map.push((source_start + byte).min(source_end));
    }
}

fn shift_render_maps(lines: &mut [RenderLine], delta: isize) {
    for line in lines {
        for offset in &mut line.source_map {
            *offset = (*offset as isize + delta) as usize;
        }
    }
}

#[derive(Default)]
pub struct LatencySamples(pub Vec<Duration>);

impl LatencySamples {
    pub fn record(&mut self, duration: Duration) {
        self.0.push(duration);
    }
    pub fn summary_ms(&self) -> Option<(f64, f64, f64)> {
        if self.0.is_empty() {
            return None;
        }
        let mut values: Vec<f64> = self.0.iter().map(|d| d.as_secs_f64() * 1000.).collect();
        values.sort_by(f64::total_cmp);
        let median = values[values.len() / 2];
        let p95 = values[((values.len() as f64 * 0.95).ceil() as usize)
            .saturating_sub(1)
            .min(values.len() - 1)];
        Some((median, p95, *values.last().unwrap()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_typing_preserves_inactive_blocks() {
        let mut model = DocumentModel::new("one\n\ntwo\n\nthree".into());
        let ids: Vec<_> = model.blocks.iter().map(|b| b.id).collect();
        let before = model.counters;
        model.apply_edit(1..1, "x", EditMode::Ordinary, 1..1, false);
        assert_eq!(model.blocks[1].id, ids[1]);
        assert_eq!(model.blocks[2].id, ids[2]);
        assert_eq!(model.counters.local_reparses, before.local_reparses);
        assert_eq!(model.counters.parsed_blocks, before.parsed_blocks);
    }

    #[test]
    fn enter_resegments_only_active_region() {
        let mut model = DocumentModel::new("one\n\ntwo\n\nthree".into());
        let last = model.blocks.last().unwrap().id;
        model.apply_edit(1..1, "\n\n", EditMode::Enter, 1..1, false);
        assert_eq!(model.blocks.last().unwrap().id, last);
        assert_eq!(model.counters.local_reparses, 1);
    }

    #[test]
    fn boundary_fusion_cannot_absorb_third_block() {
        let mut model = DocumentModel::new("one\n\ntwo\n\n```\nthree".into());
        let third = model.blocks.last().unwrap().id;
        let active = model.blocks[2].range.start;
        model.apply_edit(
            active - 1..active,
            "",
            EditMode::FusePrevious,
            active..active,
            false,
        );
        assert_eq!(model.blocks.last().unwrap().id, third);
    }

    #[test]
    fn rendered_mapping_hides_markers_but_maps_text() {
        let model = DocumentModel::new("**bold** and [link](url)".into());
        let line = &model.blocks[0].rendered[0];
        assert_eq!(line.text, "bold and link");
        assert_eq!(line.source_for_rendered(0), 2);
        assert!(line.spans.iter().any(|span| span.style.strong));
        assert!(line.spans.iter().any(|span| span.style.link));
    }

    #[test]
    fn rendered_inline_code_keeps_its_monospace_style() {
        let model = DocumentModel::new("before `code` after".into());
        let line = &model.blocks[0].rendered[0];
        assert_eq!(line.text, "before code after");
        assert!(line.spans.iter().any(|span| span.style.code));
    }

    #[test]
    fn rendered_code_hides_fences_without_removing_their_rows() {
        let model = DocumentModel::new("```rust\nlet answer = 42;\n```".into());
        let lines = &model.blocks[0].rendered;
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].text, "");
        assert_eq!(lines[1].text, "let answer = 42;");
        assert_eq!(lines[2].text, "");
        assert!(lines.iter().all(|line| line.code_block));
    }

    #[test]
    fn standalone_image_becomes_a_rendered_image() {
        let model = DocumentModel::new("![the *alt*](images/pic.png)".into());
        let image = model.blocks[0].rendered[0].image.as_ref().unwrap();
        assert_eq!(image.src, "images/pic.png");
        assert_eq!(image.alt, "the alt");
        assert!(model.blocks[0].rendered[0].text.is_empty());
    }

    #[test]
    fn image_mixed_with_text_remains_inline_text() {
        let model = DocumentModel::new("before ![alt](pic.png) after".into());
        assert!(model.blocks[0].rendered[0].image.is_none());
        assert_eq!(model.blocks[0].rendered[0].text, "before alt after");
    }

    #[test]
    fn one_hundred_insertions_never_parse_an_inactive_block() {
        let mut model = DocumentModel::new("active\n\ninactive\n\nlast".into());
        let inactive = [model.blocks[1].id, model.blocks[2].id];
        for offset in 0..100 {
            model.apply_edit(
                offset..offset,
                "x",
                EditMode::Ordinary,
                offset..offset,
                false,
            );
        }
        assert_eq!(model.counters.local_reparses, 0);
        assert_eq!(model.counters.parsed_blocks, 0);
        assert_eq!([model.blocks[1].id, model.blocks[2].id], inactive);
    }

    #[test]
    fn deleting_one_boundary_fuses_exactly_two_blocks() {
        let mut model = DocumentModel::new("one\n\ntwo\n\nthree".into());
        let third = model.blocks.last().unwrap().id;
        let active = model.blocks[2].range.start;
        model.apply_edit(
            active - 1..active,
            "",
            EditMode::FusePrevious,
            active..active,
            false,
        );
        assert_eq!(model.blocks.len(), 3);
        assert_eq!(model.raw(0), "one\ntwo");
        assert_eq!(model.blocks[2].id, third);
    }

    #[test]
    fn local_unmatched_fence_stops_at_hard_outer_boundary() {
        let mut model = DocumentModel::new("one\n\ntwo\n\nthree".into());
        let third = model.blocks.last().unwrap().id;
        let second = model.blocks[2].range.clone();
        model.apply_edit(
            second.clone(),
            "```\nunclosed",
            EditMode::Enter,
            second,
            false,
        );
        assert_eq!(model.blocks.last().unwrap().id, third);
        assert_eq!(model.raw(model.blocks.len() - 1), "three");
    }

    #[test]
    fn ordered_list_keeps_its_number_and_source_mapping() {
        let model = DocumentModel::new("12. numbered".into());
        let line = &model.blocks[0].rendered[0];
        assert_eq!(line.text, "12. numbered");
        assert_eq!(line.source_for_rendered(0), 0);
        assert_eq!(line.source_for_rendered(4), 4);
    }

    #[test]
    fn enter_at_end_owns_the_new_empty_line() {
        let mut model = DocumentModel::new("hello".into());
        model.apply_edit(5..5, "\n", EditMode::Enter, 5..5, false);
        assert_eq!(model.blocks.len(), 2);
        assert_eq!(model.blocks[0].raw_lines, vec![0..5]);
        assert_eq!(model.blocks[1].range, 6..6);
        assert_eq!(model.block_at(6), 1);
    }

    #[test]
    fn second_enter_creates_an_owned_blank_block() {
        let mut model = DocumentModel::new("hello".into());
        model.apply_edit(5..5, "\n", EditMode::Enter, 5..5, false);
        model.apply_edit(6..6, "\n", EditMode::Enter, 6..6, false);
        assert_eq!(model.blocks.len(), 3);
        assert_eq!(model.blocks[1].range, 6..6);
        assert_eq!(model.blocks[2].range, 7..7);
        assert_eq!(model.block_at(7), 2);
    }

    #[test]
    fn deletion_followed_by_insertion_never_restores_deleted_text() {
        let mut model = DocumentModel::new("mark".into());
        model.apply_edit(0..1, "", EditMode::Ordinary, 1..1, false);
        model.apply_edit(0..0, "l", EditMode::Ordinary, 0..0, false);
        assert_eq!(model.content, "lark");
        assert_eq!(model.raw(0), "lark");
    }

    #[test]
    fn editing_lines_cover_separator_caret_positions() {
        let model = DocumentModel::new("one\n\ntwo".into());
        assert_eq!(model.editing_lines(0, 3), vec![0..3]);
        assert_eq!(model.editing_lines(1, 4), vec![4..4]);
        assert_eq!(model.editing_lines(2, 5), vec![5..8]);
    }

    #[test]
    fn editing_lines_own_the_final_eof_row() {
        let model = DocumentModel::new("one\n".into());
        assert_eq!(model.editing_lines(1, 4), vec![4..4]);
    }

    #[test]
    fn activating_a_title_does_not_absorb_its_separator_lines() {
        let model = DocumentModel::new("# title\n\n\nbody".into());
        assert_eq!(model.editing_lines(0, 3), vec![0..7]);
    }

    #[test]
    fn blank_source_rows_become_their_own_blocks() {
        let model = DocumentModel::new(include_str!("../fixtures/sample.md").into());
        assert!(
            model
                .blocks
                .iter()
                .any(|block| block.kind == BlockKind::Blank)
        );
        assert!(model.blocks.iter().all(|block| {
            block.kind != BlockKind::Blank || model.raw_line(&block.range).trim().is_empty()
        }));
    }

    #[test]
    fn typing_on_a_real_separator_resegments_both_neighbors() {
        let mut model = DocumentModel::new("one\n\ntwo".into());
        model.apply_edit(4..4, "x", EditMode::FuseNext, 4..4, false);
        assert_eq!(model.content, "one\nx\ntwo");
        assert_eq!(model.blocks.len(), 1);
        assert_eq!(model.raw(0), "one\nx\ntwo");
    }

    #[test]
    fn heading_and_following_line_are_adjacent_blocks() {
        let model = DocumentModel::new("# Hello\nworld".into());
        assert_eq!(model.blocks.len(), 2);
        assert_eq!(model.raw(0), "# Hello");
        assert_eq!(model.raw(1), "world");
        assert_eq!(model.blocks[1].range.start, 8);
    }

    #[test]
    fn one_empty_source_line_reserves_one_grid_row() {
        let model = DocumentModel::new("# Hello\n\nworld".into());
        assert_eq!(model.blocks.len(), 3);
        assert_eq!(model.blocks[1].kind, BlockKind::Blank);
        assert_eq!(model.blocks[1].range, 8..8);
        assert_eq!(model.editing_lines(1, 8), vec![8..8]);
    }

    fn continue_text(content: &str, cursor: usize) -> Option<String> {
        match list_continuation(content, cursor) {
            Some(ListContinuation::Continue(text)) => Some(text),
            _ => None,
        }
    }

    #[test]
    fn continues_unordered_bullets() {
        let content = "- one";
        assert_eq!(continue_text(content, content.len()).as_deref(), Some("\n- "));
        let content = "* one";
        assert_eq!(continue_text(content, content.len()).as_deref(), Some("\n* "));
        let content = "+ one";
        assert_eq!(continue_text(content, content.len()).as_deref(), Some("\n+ "));
    }

    #[test]
    fn increments_ordered_markers() {
        let content = "1. one";
        assert_eq!(continue_text(content, content.len()).as_deref(), Some("\n2. "));
        let content = "9) nine";
        assert_eq!(continue_text(content, content.len()).as_deref(), Some("\n10) "));
    }

    #[test]
    fn preserves_indent_and_task_box() {
        let content = "  - one";
        assert_eq!(continue_text(content, content.len()).as_deref(), Some("\n  - "));
        let content = "- [x] done";
        assert_eq!(continue_text(content, content.len()).as_deref(), Some("\n- [ ] "));
    }

    #[test]
    fn continues_from_mid_line_caret() {
        // Splitting a non-empty item still carries the marker to the new line.
        let content = "- onetwo";
        assert_eq!(continue_text(content, 5).as_deref(), Some("\n- "));
    }

    #[test]
    fn empty_item_clears_marker() {
        let content = "- ";
        assert_eq!(
            list_continuation(content, content.len()),
            Some(ListContinuation::Clear(0..2))
        );
        let content = "- [ ] ";
        assert_eq!(
            list_continuation(content, content.len()),
            Some(ListContinuation::Clear(0..6))
        );
    }

    #[test]
    fn non_list_lines_have_no_continuation() {
        assert_eq!(list_continuation("plain text", 5), None);
        assert_eq!(list_continuation("-no space", 4), None);
        assert_eq!(list_continuation("# heading", 4), None);
    }

    #[test]
    fn continuation_uses_the_caret_line() {
        let content = "- one\nplain";
        assert_eq!(continue_text(content, content.len()), None);
        assert_eq!(continue_text(content, 5).as_deref(), Some("\n- "));
    }

    #[test]
    fn auto_close_pairs() {
        assert_eq!(auto_close("(", None), Some(')'));
        assert_eq!(auto_close("[", Some('a')), Some(']'));
        assert_eq!(auto_close("{", Some('a')), Some('}'));
        assert_eq!(auto_close("*", Some('a')), Some('*'));
    }

    #[test]
    fn underscore_only_closes_after_whitespace() {
        assert_eq!(auto_close("_", None), Some('_'));
        assert_eq!(auto_close("_", Some(' ')), Some('_'));
        assert_eq!(auto_close("_", Some('a')), None);
    }

    #[test]
    fn non_delimiters_do_not_auto_close() {
        assert_eq!(auto_close("a", None), None);
        assert_eq!(auto_close(")", None), None);
    }

    #[test]
    fn task_prefix_detects_state_and_box() {
        assert_eq!(
            task_prefix("- [ ] todo", 0),
            Some((6, TaskMark { checked: false, box_range: 2..5 }))
        );
        assert_eq!(
            task_prefix("- [x] done", 0),
            Some((6, TaskMark { checked: true, box_range: 2..5 }))
        );
        assert_eq!(
            task_prefix("  * [X] indented", 10),
            Some((8, TaskMark { checked: true, box_range: 14..17 }))
        );
        assert_eq!(task_prefix("- plain", 0), None);
        assert_eq!(task_prefix("1. ordered", 0), None);
    }

    #[test]
    fn task_line_renders_a_checkbox_glyph_and_mark() {
        let model = DocumentModel::new("- [ ] todo\n- [x] done".into());
        let lines: Vec<_> = model.blocks.iter().flat_map(|b| &b.rendered).collect();
        let todo = lines.iter().find(|l| l.text.contains("todo")).unwrap();
        assert_eq!(todo.task.as_ref().map(|t| t.checked), Some(false));
        assert!(todo.text.starts_with('☐'));
        let done = lines.iter().find(|l| l.text.contains("done")).unwrap();
        assert_eq!(done.task.as_ref().map(|t| t.checked), Some(true));
        assert!(done.text.starts_with('☑'));
    }
}
