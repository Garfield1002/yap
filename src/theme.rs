use gpui::{Hsla, WindowAppearance, rgb};

#[derive(Clone, Copy)]
pub struct SyntaxPalette {
    pub keyword: Hsla,
    pub string: Hsla,
    pub comment: Hsla,
    pub number: Hsla,
    pub name: Hsla,
    pub type_: Hsla,
    pub operator: Hsla,
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Hsla,
    pub bg_subtle: Hsla,
    pub fg: Hsla,
    pub fg_dim: Hsla,
    pub fg_faint: Hsla,
    pub accent: Hsla,
    pub border: Hsla,
    pub code_bg: Hsla,
    pub code_border: Hsla,
    pub selection: Hsla,
    pub syntax: SyntaxPalette,
}

#[must_use] 
pub fn color(value: u32) -> Hsla {
    rgb(value).into()
}

#[must_use] 
pub const fn alpha(mut color: Hsla, alpha: f32) -> Hsla {
    color.a = alpha;
    color
}

#[must_use] 
pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            bg: color(0x171717),
            bg_subtle: color(0x202020),
            fg: color(0xf7f7f5),
            fg_dim: color(0x8b949e),
            fg_faint: color(0x5a626c),
            accent: color(0x6cb6ff),
            border: color(0x303030),
            code_bg: color(0x202020),
            code_border: color(0x303030),
            selection: alpha(color(0x6cb6ff), 0.36),
            syntax: SyntaxPalette {
                keyword: color(0xff7b72),
                string: color(0x7ee787),
                comment: color(0x8b949e),
                number: color(0x79c0ff),
                name: color(0xffa657),
                type_: color(0xd2a8ff),
                operator: color(0x79c0ff),
            },
        }
    } else {
        Palette {
            bg: color(0xf7f7f5),
            bg_subtle: color(0xefefeb),
            fg: color(0x171717),
            fg_dim: color(0x6e7781),
            fg_faint: color(0xadb3b9),
            accent: color(0x0969da),
            border: color(0xdcdcd7),
            code_bg: color(0xefefec),
            code_border: color(0xdfdfda),
            selection: alpha(color(0x0969da), 0.32),
            syntax: SyntaxPalette {
                keyword: color(0xcf222e),
                string: color(0x0a6847),
                comment: color(0x6e7781),
                number: color(0x0550ae),
                name: color(0x953800),
                type_: color(0x6639ba),
                operator: color(0x0550ae),
            },
        }
    }
}

#[must_use] 
pub const fn is_dark(appearance: WindowAppearance) -> bool {
    matches!(
        appearance,
        WindowAppearance::Dark | WindowAppearance::VibrantDark
    )
}
