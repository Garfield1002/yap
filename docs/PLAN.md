# yap — plan and decision record

A desktop markdown editor where the block under the cursor shows raw markdown
source and every other block renders rich (the Typora/Obsidian "live preview"
model). Built to be a daily driver.

## Decisions

| Decision | Choice |
| --- | --- |
| Purpose | Daily-driver tool for personal use |
| Platform | Linux only for now; keep portable |
| Shell | Tauri 2 (Rust) — native shell, webview editor accepted |
| Frontend | Svelte 5 + TypeScript + Vite (plain Svelte, **not** SvelteKit) |
| Editing core | CodeMirror 6 |
| Source of truth | Plain markdown text in the CM6 buffer; rendering is pure view-layer decorations |
| Raw region | Current block (not line); multi-line selection reveals all touched blocks |
| Parsing | Lezer in-process (`@codemirror/lang-markdown` + GFM); no IPC in the keystroke path |
| Flavor | GFM (tables stay raw source in v1) + LaTeX math (KaTeX) + footnotes |
| Render depth | Inline styles w/ hidden markup, headings, links, inline images, highlighted code blocks, clickable checkboxes, math widgets, styled footnotes |
| Typography | One proportional font raw and rendered; monospace only for code |
| Keys | Standard + markdown shortcuts (Ctrl+B/I/K, Tab list indent); no vim in v1 |
| Persistence | Debounced autosave (~1s) + atomic write (Rust), file watcher w/ clean auto-reload, dirty conflict prompt |
| Windows | One window = one file; `yap file.md` CLI; separate process per file; open dialog when bare |
| Theme | Light + dark, follow system, CSS variables from day one |

Explicitly **out of scope for v1**: rendered tables, wiki-links, callouts,
highlights (`==x==`), vim mode, tabs/single-instance, hover popups, user theming.

## Architecture

### Live-preview engine

One `StateField<{deco, atomic, pinned}>`, **not** a `ViewPlugin`. Decorations
that affect vertical layout (block widgets, multi-line replaces, tall inline
widgets) must be known to the state before the view measures heights; supplying
them from a `ViewPlugin` corrupts height estimates and makes scrolling jump.

Two range sets, because `EditorView.atomicRanges` must only receive
replace/widget ranges — atomic *marks* would make the cursor skip over styled
words.

A companion `ViewPlugin` dispatches `refreshDecorations` when the async Lezer
parse extends the tree (comparing `syntaxTree(state).length` against previous),
or the tail of a large file never renders rich. `buildDecorations` also calls
`ensureSyntaxTree(state, doc.length, 100)`.

### activeRegions (pure, heavily unit-tested)

Per selection range, resolve into the tree and walk up:

1. If any ancestor is in `{FencedCode, CodeBlock, Blockquote, BlockMath,
   FootnoteDefinition, Table, HTMLBlock, CommentBlock}` → the **highest** such
   node is the active block.
2. Else the **nearest** `Paragraph`/`ATXHeading*`/`SetextHeading*`/… (inside
   list items this yields the item's paragraph, not the whole list).
3. Blank-line fallback → that line.

Expand to full lines; a non-empty selection spans block-of-`from` →
block-of-`to`; merge overlaps. Multi-cursor works for free.

**Key semantic:** "raw" suppresses only replace/widget decorations. Mark and
line decorations (bold styling, heading size, code background) persist in the
active block. Combined with one-proportional-font, revealing a block only makes
markup characters *appear*; text never restyles or jumps.

### Block pinning (added after Phase 3 — not in the original plan)

A half-typed block corrupts the parse of everything after it. Delete one
backtick from a fence and the surviving fence opens a code block that swallows
the rest of the file: headings and emphasis below stop existing as syntax nodes
at all, so no care in the builders can keep them rendered. This is correct
CommonMark, not a parser bug.

So while the cursor edits inside a block, that block stays **pinned** raw and
the rest of the document keeps the decorations it already had, merely mapped
through the edit. We do not ask the contaminated tree about the parts of the
file the user is not touching. The moment the cursor leaves the block, or an
edit lands outside it, everything is rebuilt from the real tree.

Two traps, both guarded by tests:

- Pinned regions map `from` toward the text and `to` away from it, so typing at
  either end *leaves* the block rather than stretching it. Map both edges
  outward and a cursor at the end of a paragraph extends the region with every
  keystroke until the whole document is pinned raw and nothing renders.
- An emptied block must unpin, or the remainder is never rebuilt again.

### Builder invariants

- Collect ranges, then `Decoration.set(ranges, true)`. Never `RangeSetBuilder`:
  nested constructs emit out of order.
- One replace per markup token.
- Whole-node replaces (`Image`, `InlineMath`) return `false` from `enter` so
  children cannot emit inside a replaced range (partial overlap throws).
- Never a non-block replace containing `\n`.
- Inside `Table`, emit nothing (tables stay raw in v1).

### Rust side

- `main.rs`: resolve `argv[1]` against the cwd **before** Tauri starts (the
  webview process has no meaningful cwd), stash in `AppState`. `canonicalize` is
  allowed to fail so `yap new.md` opens an empty buffer that saves into place.
  No single-instance plugin — one process per file.
- `write_file_atomic`: `NamedTempFile::new_in(parent_dir)` → write → `sync_all`
  → `persist()`. Same-directory temp is non-negotiable: `rename(2)` is only
  atomic within one filesystem. Target's mode is preserved (0644 for a new file)
  so saving never tightens permissions to the temp file's 0600.
- `start_watch`: `notify` on the **parent directory**, filtered to the filename.
  A file-level watch follows the inode and goes deaf the moment another editor
  rename-replaces it. ~200ms debounce; emits `file-changed`.
- Autosave: `updateListener` → dirty + 1s debounce → `write_file_atomic`.
  In-flight guard with a drain loop so writes never overlap and the newest text
  wins. `flush()` on `onCloseRequested` is the data-loss guard.
- Watcher flow: re-read; content equals what we last wrote → self-save echo,
  ignore (**content compare, not mtime** — immune to races); buffer clean →
  reload via a changes-based dispatch (preserves undo history and cursor);
  buffer dirty → conflict dialog ("Keep mine" / "Load theirs"). Autosave is
  suspended while the dialog is open.

## Phases

- [x] **Phase 0 — Scaffold.** Tauri + plain Svelte + Vite, CSS-var theme, bare
      CM6 markdown editor with GFM + line wrapping.
- [x] **Phase 1 — Live-preview engine core.** `activeRegions` + decoration field
      + inline and heading builders + atomic ranges. *(go/no-go gate — passed)*
- [x] **Phase 2 — Real files.** CLI arg, read/write/watch commands, autosave,
      conflict dialog, status bar. Dogfooding starts here.
- [x] **Phase 3 — Full rendering.** Links (Ctrl+Click), images (+asset
      protocol), checkboxes, highlighted code blocks. Plus block pinning and the
      Ctrl-held link cursor.
- [ ] **Phase 4 — Custom Lezer extensions.** Math + footnotes + widgets + KaTeX
      cache. The parse-tree snapshot tests over adversarial fixtures are the
      most valuable tests in the repo.
- [ ] **Phase 5 — Shortcuts, polish, packaging.** Keymap commands, typography
      pass, IME smoke test (CJK into a styled paragraph), 1–2 MB profiling pass,
      `cargo tauri build` → `.rpm` + AppImage + `.desktop` with `%F`.

### Phase 4 notes

- **Math** (`lezer/math.ts`): inline parser triggered on `$`, `before:
  "Emphasis"`; reject `$$`, whitespace-adjacent, escaped; single-line in v1.
  Block parser modeled on `FencedCode` (`$$…$$`, unterminated runs to EOF).
  Render the whole node as a `MathWidget`; block math is a `block: true` replace
  (legal only because the decorations come from a `StateField`). Module-level
  KaTeX LRU cache (~300, `throwOnError: false`) + `eq(tex, displayMode)` → zero
  KaTeX work while typing, since the active block shows raw source anyway.
  Import `katex.min.css`.
- **Footnotes** (`lezer/footnotes.ts`): inline parser on `[^…]` with `before:
  "Link"` — essential, because `Link` eats `[`. Definition is a leaf block
  parser on `/^\[\^[^\]\s]+\]:/`; single-line definitions in v1. References hide
  `[^` and `]` and superscript the label via CSS. Definitions get a line class
  and a dimmed (not hidden) marker.

No maintained Lezer markdown extension exists for math or footnotes; both are
custom `MarkdownConfig` extensions (`defineNodes` + `InlineParser`/`BlockParser`).
Lezer's own `LinkReference` and `FencedCode` are the reference models.

## Pitfalls (review checklist)

1. `StateField`, not `ViewPlugin`, for layout-affecting decorations — the #1 CM6
   live-preview mistake.
2. Async parse tail: without `refreshDecorations` the bottom of large files
   never renders rich.
3. Partially-overlapping replaces throw. Stop-descend rule and one replace per
   token; test nested bold-in-link-in-list.
4. Only block math replaces across line breaks, and only as `block: true`.
5. IME: rebuild decorations only from transactions, never measure side-channels.
   The composing block is raw (decoration-free) by design.
6. A missing `eq()` on any widget means DOM teardown plus KaTeX/image re-work on
   every keystroke.
7. Lezer ordering: footnotes `before: "Link"`, math `before: "Emphasis"`.
   Adversarial fixtures guard `@lezer/markdown` bumps.
8. `posAtDOM` at click time, never stored offsets, for widget-initiated edits.
9. Watch the directory, not the file. Temp file in the same directory as the
   target.
10. Self-save echo detection by content compare, not mtime.
11. The atomic set gets replaces only, never marks.
12. No decorations inside `Table` nodes, or raw tables render half-eaten.

## Open items

- **CSP is `null`.** A real `img-src 'self' asset: http://asset.localhost data:`
  policy interacts badly with Vite's dev server (inline scripts, HMR websocket).
  Belongs in Phase 5 with the release bundle, where it can be verified.
- **The asset protocol is unverified.** `convertFileSrc` only works inside a
  real webview, so tests stub it. `assetProtocol` is enabled with an `$HOME/**`
  scope and tauri's `protocol-asset` feature is on, but no local image has been
  confirmed to render.
- **`onCloseRequested` → `flush()` → `destroy()` is unverified end to end.**
  `flush()` itself is unit-tested.

## Verifying

`just run FILE` starts Vite and the app together. `just check` runs everything
that does not need a display.

Walk the cursor through every block type; drag selections across constructs;
toggle a checkbox and Ctrl+Z; edit the file externally while clean and while
dirty; close the window mid-typing and check the disk.
