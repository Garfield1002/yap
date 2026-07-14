use std::ops::Range;
use std::path::Path;
use std::sync::Arc;

use gpui::{
    FontStyle, FontWeight, Hsla, ImageFormat, Pixels, RenderImage, Resource, SharedString, TextRun,
    UnderlineStyle, Window, WrappedLine, font, hsla, px,
};
use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use unicode_segmentation::UnicodeSegmentation;

use crate::layout::{DEFAULT_IMAGE_ROWS, GRID, MONO_FONT, PROSE_FONT, WRAP_WIDTH};
use crate::model::{BlockKind, InlineStyle, RenderLine, RenderSpan, TaskMark};
use crate::theme::{alpha, Palette};

#[derive(Clone)]
pub struct ShapedLine {
    pub source: Range<usize>,
    pub layout: WrappedLine,
    pub map: Option<Vec<usize>>,
    pub code: bool,
    pub inline_code: Vec<Range<usize>>,
    pub image: Option<ShapedImage>,
    pub task: Option<TaskMark>,
    /// Ascent used for vertical placement. Falls back to the font's metrics
    /// when the line has no glyphs, since an empty shaped line reports zero.
    pub ascent: Pixels,
    /// Descent used for vertical placement. See [`ShapedLine::ascent`].
    pub descent: Pixels,
}

/// Ascent and descent of a generic prose line, measured once when the editor
/// first shapes so empty lines can reuse it.
///
/// An empty line has no glyphs, so its `WrappedLine` reports zero ascent and
/// descent, which would drop the caret below where a line of text sits. The
/// font's own metrics don't match either — they're the typographic maxima, not
/// the tighter box a real shaped line reports — so we measure an actual line of
/// prose and reuse those values.
#[must_use]
pub fn default_text_metrics(w: &mut Window) -> (Pixels, Pixels) {
    let text = SharedString::from("Ag");
    let layout = w
        .text_system()
        .shape_text(
            text.clone(),
            px(16.),
            &[TextRun {
                len: text.len(),
                font: font(PROSE_FONT),
                color: hsla(0., 0., 0., 1.),
                background_color: None,
                underline: None,
                strikethrough: None,
            }],
            None,
            None,
        )
        .unwrap()
        .remove(0);
    (layout.ascent(), layout.descent())
}

/// Ascent and descent for vertical placement of a shaped line.
///
/// A non-empty line reports the metrics of its own glyphs; an empty line has
/// none, so it reuses the generic prose metrics measured at open time.
fn line_metrics(text: &str, layout: &WrappedLine, default: (Pixels, Pixels)) -> (Pixels, Pixels) {
    if text.is_empty() {
        default
    } else {
        (layout.ascent(), layout.descent())
    }
}

#[derive(Clone)]
pub struct ShapedImage {
    pub alt: String,
    pub resource: Resource,
    pub data: Option<Arc<RenderImage>>,
    pub failed: bool,
    pub rows: usize,
}

#[derive(Clone, Default)]
pub struct ShapeCache {
    pub raw: Option<Vec<ShapedLine>>,
    pub rendered: Option<Vec<ShapedLine>>,
    pub raw_rows: usize,
    pub rendered_rows: usize,
}

pub fn image_resource(src: &str, document_path: Option<&Path>) -> Resource {
    if src.starts_with("http://") || src.starts_with("https://") || src.starts_with("data:") {
        return Resource::Uri(src.to_string().into());
    }
    let src = src.strip_prefix("file://").unwrap_or(src);
    let path = std::path::PathBuf::from(src);
    let path = if path.is_absolute() {
        path
    } else {
        document_path
            .and_then(Path::parent)
            .unwrap_or_else(|| Path::new("."))
            .join(path)
    };
    Resource::from(path)
}

#[must_use] 
pub fn image_allocation_rows(data: &RenderImage) -> usize {
    let dimensions = data.size(0);
    let width = dimensions.width.0.max(1) as f32;
    let height = dimensions.height.0.max(1) as f32;
    let ratio = width / height;
    let intrinsic_height = width.min(WRAP_WIDTH) / ratio;
    let mut rows = (intrinsic_height / GRID).ceil().max(1.) as usize;
    if rows as f32 * GRID * ratio > WRAP_WIDTH + 0.5 {
        rows = ((WRAP_WIDTH / ratio) / GRID).floor().max(1.) as usize;
    }
    rows
}

#[must_use] 
pub const fn image_extension(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpg",
        ImageFormat::Webp => "webp",
        ImageFormat::Gif => "gif",
        ImageFormat::Svg => "svg",
        ImageFormat::Bmp => "bmp",
        ImageFormat::Tiff => "tiff",
    }
}

#[must_use] 
pub fn source_line_is_code(kind: BlockKind, block_end: usize, line_start: usize) -> bool {
    kind == BlockKind::Code && line_start <= block_end
}

pub fn shape_raw(
    range: Range<usize>,
    text: &str,
    code_block: bool,
    palette: Palette,
    default_metrics: (Pixels, Pixels),
    w: &mut Window,
) -> ShapedLine {
    let runs = if code_block {
        code_text_runs(text, text.trim_start().starts_with("```"), palette)
    } else {
        raw_text_runs(text, palette)
    };
    let layout = w
        .text_system()
        .shape_text(
            SharedString::from(text.to_string()),
            px(if code_block { 14.08 } else { 16. }),
            &runs,
            Some(px(if code_block {
                WRAP_WIDTH - 24.
            } else {
                WRAP_WIDTH
            })),
            None,
        )
        .unwrap()
        .remove(0);
    let (ascent, descent) = line_metrics(text, &layout, default_metrics);
    ShapedLine {
        source: range,
        layout,
        map: None,
        code: code_block,
        inline_code: if code_block {
            Vec::new()
        } else {
            inline_code_ranges(text)
        },
        image: None,
        task: None,
        ascent,
        descent,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RawRole {
    Text,
    Mark,
    InlineCode,
}

#[must_use] 
pub fn inline_code_background(is_code: bool, palette: Palette) -> Option<Hsla> {
    is_code.then_some(palette.code_bg)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RawStyle {
    pub inline: InlineStyle,
    pub role: RawRole,
}

#[must_use] 
pub fn raw_text_runs(text: &str, palette: Palette) -> Vec<TextRun> {
    let mut byte_styles = vec![
        RawStyle {
            inline: InlineStyle::default(),
            role: RawRole::Mark,
        };
        text.len()
    ];
    let mut current = InlineStyle::default();
    for (event, range) in Parser::new(text).into_offset_iter() {
        match event {
            Event::Start(Tag::Emphasis) => current.italic = true,
            Event::End(TagEnd::Emphasis) => current.italic = false,
            Event::Start(Tag::Strong) => current.strong = true,
            Event::End(TagEnd::Strong) => current.strong = false,
            Event::Start(Tag::Link { .. }) => current.link = true,
            Event::End(TagEnd::Link) => current.link = false,
            Event::Text(_) => {
                for style in &mut byte_styles[range] {
                    *style = RawStyle {
                        inline: current,
                        role: RawRole::Text,
                    };
                }
            }
            Event::Code(_) => {
                let mut code = current;
                code.code = true;
                let inner = range.start.saturating_add(1)..range.end.saturating_sub(1);
                if inner.start <= inner.end && inner.end <= byte_styles.len() {
                    for style in &mut byte_styles[inner] {
                        *style = RawStyle {
                            inline: code,
                            role: RawRole::InlineCode,
                        };
                    }
                }
            }
            _ => {}
        }
    }

    // The task-list extension is off, so `[ ]`/`[x]` parses as plain text. Grey
    // it like the leading list marker so the whole checkbox syntax reads dim.
    if let Some((_, mark)) = crate::model::task_prefix(text, 0) {
        for style in &mut byte_styles[mark.box_range] {
            style.role = RawRole::Mark;
        }
    }

    let mut spans = Vec::<(usize, RawStyle)>::new();
    for style in byte_styles {
        if let Some((len, previous)) = spans.last_mut()
            && *previous == style
        {
            *len += 1;
        } else {
            spans.push((1, style));
        }
    }
    if spans.is_empty() {
        spans.push((
            0,
            RawStyle {
                inline: InlineStyle::default(),
                role: RawRole::Text,
            },
        ));
    }
    spans
        .into_iter()
        .map(|(len, style)| {
            let mut face = font(PROSE_FONT);
            if style.inline.strong {
                face.weight = FontWeight::BOLD;
            }
            if style.inline.italic {
                face.style = FontStyle::Italic;
            }
            if style.inline.code {
                face = font(MONO_FONT);
            }
            let color = if style.role == RawRole::Mark {
                palette.fg_faint
            } else if style.inline.link {
                palette.accent
            } else {
                palette.fg
            };
            TextRun {
                len,
                font: face,
                color,
                background_color: inline_code_background(
                    style.role == RawRole::InlineCode,
                    palette,
                ),
                underline: style.inline.link.then_some(UnderlineStyle {
                    thickness: px(1.),
                    color: Some(alpha(color, 0.4)),
                    wavy: false,
                }),
                strikethrough: None,
            }
        })
        .collect()
}

#[must_use] 
pub fn inline_code_ranges(text: &str) -> Vec<Range<usize>> {
    Parser::new(text)
        .into_offset_iter()
        .filter_map(|(event, range)| match event {
            Event::Code(_) => Some(
                range.start.saturating_add(1)..range.end.saturating_sub(1),
            ),
            _ => None,
        })
        .filter(|range| range.start <= range.end)
        .collect()
}

#[must_use] 
pub fn inline_code_ranges_from_spans(spans: &[RenderSpan]) -> Vec<Range<usize>> {
    let mut ranges: Vec<Range<usize>> = Vec::new();
    let mut offset = 0;
    for span in spans {
        let end = offset + span.len;
        if span.style.code {
            if let Some(previous) = ranges.last_mut()
                && previous.end == offset
            {
                previous.end = end;
            } else {
                ranges.push(offset..end);
            }
        }
        offset = end;
    }
    ranges
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CodeRole {
    Text,
    Keyword,
    String,
    Comment,
    Number,
    Name,
    Type,
    Operator,
    Fence,
}

pub fn code_text_runs(text: &str, fence: bool, palette: Palette) -> Vec<TextRun> {
    let mut roles = vec![
        if fence {
            CodeRole::Fence
        } else {
            CodeRole::Text
        };
        text.len()
    ];
    if !fence {
        let bytes = text.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
                roles[i..].fill(CodeRole::Comment);
                break;
            }
            if bytes[i] == b'\'' || bytes[i] == b'"' {
                let quote = bytes[i];
                let start = i;
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == b'\\' {
                        i = (i + 2).min(bytes.len());
                        continue;
                    }
                    i += 1;
                    if bytes[i - 1] == quote {
                        break;
                    }
                }
                roles[start..i].fill(CodeRole::String);
                continue;
            }
            if bytes[i].is_ascii_digit() {
                let start = i;
                i += 1;
                while i < bytes.len()
                    && (bytes[i].is_ascii_hexdigit() || matches!(bytes[i], b'.' | b'_' | b'x'))
                {
                    i += 1;
                }
                roles[start..i].fill(CodeRole::Number);
                continue;
            }
            if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
                let start = i;
                i += 1;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                let word = &text[start..i];
                let role = if matches!(
                    word,
                    "as" | "async"
                        | "await"
                        | "break"
                        | "class"
                        | "const"
                        | "continue"
                        | "def"
                        | "else"
                        | "enum"
                        | "false"
                        | "fn"
                        | "for"
                        | "from"
                        | "if"
                        | "impl"
                        | "import"
                        | "in"
                        | "let"
                        | "loop"
                        | "match"
                        | "mod"
                        | "move"
                        | "mut"
                        | "new"
                        | "none"
                        | "null"
                        | "pub"
                        | "return"
                        | "self"
                        | "static"
                        | "struct"
                        | "super"
                        | "this"
                        | "trait"
                        | "true"
                        | "type"
                        | "use"
                        | "var"
                        | "where"
                        | "while"
                        | "yield"
                ) {
                    CodeRole::Keyword
                } else if word.as_bytes().first().is_some_and(u8::is_ascii_uppercase) {
                    CodeRole::Type
                } else if text[i..].trim_start().starts_with('(') {
                    CodeRole::Name
                } else {
                    CodeRole::Text
                };
                roles[start..i].fill(role);
                continue;
            }
            if b"+-*/%=!<>|&^~?:".contains(&bytes[i]) {
                roles[i] = CodeRole::Operator;
            }
            i += 1;
        }
        if text.trim_start().starts_with("# ") || text.trim_start().starts_with("#!") {
            let start = text.len() - text.trim_start().len();
            roles[start..].fill(CodeRole::Comment);
        }
    }

    let mut spans = Vec::<(usize, CodeRole)>::new();
    for role in roles {
        if let Some((len, previous)) = spans.last_mut()
            && *previous == role
        {
            *len += 1;
        } else {
            spans.push((1, role));
        }
    }
    if spans.is_empty() {
        spans.push((0, CodeRole::Text));
    }
    spans
        .into_iter()
        .map(|(len, role)| TextRun {
            len,
            font: font(MONO_FONT),
            color: match role {
                CodeRole::Text => palette.fg,
                CodeRole::Keyword => palette.syntax.keyword,
                CodeRole::String => palette.syntax.string,
                CodeRole::Comment => palette.syntax.comment,
                CodeRole::Number => palette.syntax.number,
                CodeRole::Name => palette.syntax.name,
                CodeRole::Type => palette.syntax.type_,
                CodeRole::Operator => palette.syntax.operator,
                CodeRole::Fence => palette.fg_faint,
            },
            background_color: None,
            underline: None,
            strikethrough: None,
        })
        .collect()
}

pub fn shape_render(
    line: &RenderLine,
    document_path: Option<&Path>,
    palette: Palette,
    default_metrics: (Pixels, Pixels),
    w: &mut Window,
) -> ShapedLine {
    let size = match line.level {
        1 => 30.4,
        2 => 24.,
        3 => 20.,
        _ => 16.,
    };
    let runs = if line.code_block {
        code_text_runs(&line.text, false, palette)
    } else {
        line.spans
            .iter()
            .map(|s| {
                let mut f = font(PROSE_FONT);
                if line.level > 0 || s.style.strong {
                    f.weight = FontWeight::BOLD;
                }
                if s.style.italic {
                    f.style = FontStyle::Italic;
                }
                if s.style.code {
                    f = font(MONO_FONT);
                }
                let color = if s.style.link {
                    palette.accent
                } else {
                    palette.fg
                };
                TextRun {
                    len: s.len,
                    font: f,
                    color,
                    background_color: inline_code_background(s.style.code, palette),
                    underline: s.style.link.then_some(UnderlineStyle {
                        thickness: px(1.),
                        color: Some(alpha(color, 0.4)),
                        wavy: false,
                    }),
                    strikethrough: None,
                }
            })
            .collect::<Vec<_>>()
    };
    let layout = w
        .text_system()
        .shape_text(
            SharedString::from(line.text.clone()),
            px(if line.code_block { 14.08 } else { size }),
            &runs,
            Some(px(if line.code_block {
                WRAP_WIDTH - 24.
            } else {
                WRAP_WIDTH
            })),
            None,
        )
        .unwrap()
        .remove(0);
    let (ascent, descent) = line_metrics(&line.text, &layout, default_metrics);
    ShapedLine {
        source: 0..0,
        layout,
        map: Some(line.source_map.clone()),
        code: line.code_block,
        inline_code: if line.code_block {
            Vec::new()
        } else {
            inline_code_ranges_from_spans(&line.spans)
        },
        image: line.image.as_ref().map(|image| ShapedImage {
            alt: image.alt.clone(),
            resource: image_resource(&image.src, document_path),
            data: None,
            failed: false,
            rows: DEFAULT_IMAGE_ROWS,
        }),
        task: line.task.clone(),
        ascent,
        descent,
    }
}

pub fn rows(lines: &[ShapedLine]) -> usize {
    lines.iter().map(line_rows).sum()
}

#[must_use]
pub fn line_rows(line: &ShapedLine) -> usize {
    if let Some(image) = &line.image {
        return image.rows;
    }
    // One grid row per wrapped visual line. Headings are shaped larger than the
    // grid pitch, but they are not rounded up to extra rows here: their glyphs
    // are centred on the row and overflow into the explicit margin rows that
    // `block_gap` reserves around every heading (see `src/model.rs`).
    line.layout.wrap_boundaries().len() + 1
}

#[must_use] 
pub fn grapheme_offset(s: &str, n: usize) -> usize {
    s.grapheme_indices(true).nth(n).map_or(s.len(), |(i, _)| i)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum WordClass {
    Whitespace,
    Word,
    Punctuation,
}

pub fn word_class(grapheme: &str) -> WordClass {
    if grapheme.chars().all(char::is_whitespace) {
        WordClass::Whitespace
    } else if grapheme
        .chars()
        .any(|character| character.is_alphanumeric() || character == '_')
    {
        WordClass::Word
    } else {
        WordClass::Punctuation
    }
}

#[must_use] 
pub fn previous_word_boundary(text: &str, at: usize) -> usize {
    let at = at.min(text.len());
    let graphemes = text[..at].grapheme_indices(true).collect::<Vec<_>>();
    let Some((mut index, grapheme)) = graphemes.last().copied() else {
        return 0;
    };
    let mut position = graphemes.len() - 1;
    if word_class(grapheme) == WordClass::Whitespace {
        while position > 0 && word_class(graphemes[position].1) == WordClass::Whitespace {
            position -= 1;
        }
        if word_class(graphemes[position].1) == WordClass::Whitespace {
            return 0;
        }
        index = graphemes[position].0;
    }
    let class = word_class(graphemes[position].1);
    while position > 0 && word_class(graphemes[position - 1].1) == class {
        position -= 1;
        index = graphemes[position].0;
    }
    index
}

#[must_use] 
pub fn next_word_boundary(text: &str, at: usize) -> usize {
    let at = at.min(text.len());
    let graphemes = text[at..]
        .grapheme_indices(true)
        .map(|(index, grapheme)| (at + index, grapheme))
        .collect::<Vec<_>>();
    let Some((_, first)) = graphemes.first().copied() else {
        return text.len();
    };
    let mut position = 0;
    let class = word_class(first);
    while position < graphemes.len() && word_class(graphemes[position].1) == class {
        position += 1;
    }
    while position < graphemes.len() && word_class(graphemes[position].1) == WordClass::Whitespace {
        position += 1;
    }
    graphemes
        .get(position)
        .map_or(text.len(), |(index, _)| *index)
}

#[must_use] 
pub fn utf16_to_utf8(s: &str, n: usize) -> usize {
    let (mut u, mut b) = (0, 0);
    for c in s.chars() {
        if u >= n {
            break;
        }
        u += c.len_utf16();
        b += c.len_utf8();
    }
    b
}

#[must_use] 
pub fn utf8_to_utf16(s: &str, n: usize) -> usize {
    s[..n.min(s.len())].encode_utf16().count()
}
