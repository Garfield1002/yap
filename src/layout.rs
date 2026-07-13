//! Shared layout metrics for the document view.

pub const GRID: f32 = 24.;
pub const FIRST_BASELINE: f32 = 48.;
pub const DOCUMENT_WIDTH: f32 = 768.;
pub const INSET: f32 = 24.;
pub const WRAP_WIDTH: f32 = DOCUMENT_WIDTH - INSET * 2.;
// The Tauri stack starts with Inter but resolves to Noto Sans on this Linux
// machine. Naming the installed face directly matters in GPUI: unlike CSS,
// its missing-family fallback does not reliably retain bold and italic faces.
pub const PROSE_FONT: &str = "Noto Sans";
pub const MONO_FONT: &str = "Noto Sans Mono";
pub const DEFAULT_IMAGE_ROWS: usize = 8;
