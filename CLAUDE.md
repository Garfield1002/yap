# bulletmd-native-poc

A native Markdown editor built on [GPUI](https://www.gpui.rs/) (Zed's UI
framework). It renders Markdown live: the block under the caret shows editable
raw source, every other block shows rendered output, and edits reparse
incrementally. Crate name: `bulletmd_native_poc`.

## Build / run

```bash
cargo build            # or: cargo run [path/to/file.md]
cargo clippy           # aggressive lint policy is enforced (see commit history)
cargo test             # model-level unit tests live in src/model.rs
```

Do **not** verify changes by driving the GUI — use build, clippy, and tests.
`cargo run` with no args opens `fixtures/sample.md`. `--new` opens blank.

## Module map (`src/`)

| File | Responsibility |
|------|----------------|
| `main.rs` | Binary entry: arg parsing, config load, key bindings, window setup. |
| `lib.rs` | Re-exports the public modules. |
| `model.rs` | **Document model** — the source of truth. No GPUI deps. |
| `element.rs` | Custom GPUI `Element` (`DocumentElement`): layout, paint, hit-testing. |
| `shaping.rs` | Text shaping + caches, utf8⇄utf16, word/grapheme boundaries. |
| `layout.rs` | Layout constants (`GRID`, `DOCUMENT_WIDTH`, fonts, insets). |
| `theme.rs` | Palette, dark/light resolution, color helpers. |
| `persistence.rs` | Config, recent files, save, file-watch. |
| `editor/` | The `Editor` GPUI entity (the view/controller). |

### `editor/` submodules

All share `use super::*`; the `Editor` struct + shared types live in `mod.rs`.

- `mod.rs` — `Editor` struct, `Selection`, `LayoutState`, `actions!` list, enums.
- `input.rs` — `EntityInputHandler` impl (IME + typed-text entry point:
  `replace_text_in_range`). Typed characters and IME flow through here.
- `movement.rs` — caret motion, `edit()` (the central mutation), `enter()`,
  `backspace()`, vertical movement with sticky `preferred_column`.
- `commands.rs` — file ops (new/open/save/rename/delete), watch, conflict.
- `interaction.rs` — mouse handling.
- `menu.rs` — menu bar / `MenuCommand`.
- `render.rs` — building the element tree each frame.
- `view.rs` — `ensure_shapes()`: shape/cache raw + rendered lines per block.

## Core concepts

**Offsets.** Everything internal uses **UTF-8 byte offsets** into
`document.content`. Convert to/from UTF-16 only at the GPUI input boundary
(`input.rs`) via `utf8_to_utf16` / `utf16_to_utf8`.

**Blocks.** `DocumentModel` splits `content` into semantic `Block`s (paragraph,
heading, list, code, blank), each with a byte `range`, `raw_lines`, a `kind`,
and precomputed `rendered: Vec<RenderLine>`. Block 0 always exists.

**Incremental reparse.** `DocumentModel::apply_edit(range, text, mode, …)` is
the only mutation path. `EditMode` picks how much to resegment:
- `Ordinary` — single block, no newline: cheap in-place range shift, no reparse.
- `Enter` / `CrossBlock` — reparse the touched block region.
- `FusePrevious` / `FuseNext` — merge with a neighbor (backspace/delete at a
  block boundary), reparsing the merged region.
It returns an `EditTransaction` (for undo/redo) + invalidated block ids.
`Editor::edit()` in `movement.rs` wraps this: it updates selection, invalidates
shape caches, records history, marks dirty, schedules autosave.

**Rendered ⇄ source mapping.** Each `RenderLine` carries a `source_map`
(rendered byte offset → absolute source byte offset) so clicks and the caret
translate between rendered view and raw source.

**Revealed blocks.** `LayoutState.revealed` is the set of block ids currently
shown as editable raw source (the block(s) the selection touches). Others render
Markdown. `sync_revealed()` keeps this in step with the selection.

**Shape caches.** `LayoutState.shapes: HashMap<BlockId, ShapeCache>` holds
shaped `raw` and `rendered` lines. `view.rs::ensure_shapes` refills stale
entries; edits clear the relevant caches in `edit()`.

**Painting.** `element.rs` is a hand-rolled GPUI `Element`. It lays blocks out on
a vertical `GRID`, paints rendered/raw lines, selection, caret, and produces
`HitLine`s used for hit-testing (`index_at`, `bounds_for_range`). Empty/blank
lines have no glyphs, so ascent/descent come from `default_text_metrics`.

## Conventions

- Byte offsets internally, UTF-16 only at the GPUI edge.
- Selection is `sel.range` (byte range) + `reversed`; empty range = bare caret.
  `cursor()` returns the active end.
- Keep `model.rs` free of GPUI dependencies (it is unit-tested standalone).
- Match surrounding style; the repo runs an aggressive clippy policy.
