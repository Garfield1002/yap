# bulletmd — plan and decision record

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
| Windows | One window = one file; `bulletmd file.md` CLI; separate process per file; open dialog when bare |
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
  allowed to fail so `bulletmd new.md` opens an empty buffer that saves into place.
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
- [x] **Phase 4 — Custom Lezer extensions.** Math + footnotes + widgets + KaTeX
      cache. The parse-tree snapshot tests over adversarial fixtures are the
      most valuable tests in the repo.
- [x] **Phase 5 — Shortcuts, polish, packaging.** Keymap commands, typography
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

### Phase 5 notes

- **Shortcuts** (`markdownShortcuts.ts`): `Mod-b`/`Mod-i`/`Mod-e` toggle bold /
  italic / inline code (wrap, or unwrap when the selection is already wrapped);
  `Mod-k` wraps as `[text]()` with the cursor in the parens; `Tab` nests a list
  item (`indentMore`) and inserts indentation elsewhere, `Shift-Tab` dedents.
  Installed ahead of the default keymap so it wins the precedence tie. All
  commands are ordinary buffer edits, so undo and autosave see them normally,
  and all are unit-tested as pure state transforms.
- **Startup / profiling.** Snappy launch is the priority. KaTeX (~280 KB of JS +
  CSS + fonts) is the dominant frontend weight, so `MathWidget` imports it
  lazily on the first formula; a math-free document never loads it. This moved
  KaTeX out of the eager bundle into its own async chunk and roughly halved the
  startup JS (index ~560 KB → ~297 KB). Fenced-code grammars were already lazy
  via `codeLanguages`. Until KaTeX lands, a formula shows its raw source, then
  swaps in and (for block math) requests a re-measure.
- **Packaging.** `bundle.fileAssociations` registers `.md`/`.markdown` with
  `text/markdown`; Tauri writes the `MimeType=` and the `Exec=… %F` into the
  generated `.desktop`, matching the one-process-per-file model. Linux targets
  pinned to `rpm` + `appimage`.

**Not verified in this session** (need a display or a full toolchain, deferred
to a real build/QA pass): `cargo tauri build` producing the `.rpm`/AppImage and
the resulting `.desktop`; the CJK IME smoke test (the design guarantee -- the
composing block is raw and decorations rebuild only from transactions -- is in
place, but was not exercised live); and a runtime 1–2 MB scroll/typing profile
in the running webview. KaTeX painting in a live webview also remains unverified
(see Open items).

### Custom title bar, menus, untitled buffers, recent files, theme (post-v1)

The OS window decorations are off (`decorations: false`); the frontend draws a
themed, compact title bar (`Titlebar.svelte`): File / Edit / Settings buttons on
the left, the filename (with a dirty dot) centred over a drag region, and
minimize / maximize / close controls on the right. This is the only way on Linux
to theme the header, control its height, and put the filename in it.

The menus themselves are still native (Tauri `menu.rs`): clicking a button calls
`popup_menu`, which rebuilds the requested submenu from the current config and
pops it up. Building on demand means Open Recent, the enabled state of the
path-only items, and the theme tick are always current -- no persistent menu to
refresh. Most items forward their id to the frontend as a `menu-action` event
(the frontend owns the editor, the path, and the dialogs); `new_window` spawns a
second process (`--new`); Cut/Copy/Paste are Tauri predefined items (CodeMirror
fills the clipboard with document *source* on the native copy event, so a
hidden-markup selection still copies correct markdown); Undo/Redo route to
CodeMirror's own history (a webview's native undo does not track it) and are also
bound to Ctrl+Z / Ctrl+Shift+Z / Ctrl+Y in the editor keymap.

Closing an **untitled buffer with content** (which autosave cannot persist)
raises a Save / Discard / Cancel prompt (`UnsavedDialog.svelte`); a titled buffer
just flushes and closes.

This relaxes the original "always has a path" decision: **New opens an untitled
buffer** with no path and no autosave until the first Save picks a location.
`App.svelte` gained a document lifecycle -- `openDocument(path|null)` recreates
the editor; `attachPersistence` / `detachPersistence` bind or drop autosave +
watcher in place (via a `documentDirectory` compartment) so Save-as, Rename, and
Delete keep the undo history and cursor. Open replaces the current window.

Recent files and appearance preferences persist as `state.json` under
`$BULLETMD_HOME`, else the XDG config dir + `bulletmd` (`config.rs`). The theme
override is a `data-theme` attribute on the root that beats the system `prefers-color-scheme`
(the dark palette is written twice in `global.css`, once per selector); the text
selection colour is derived from `--accent` with `color-mix`, so it tracks the
theme automatically. Copy HTML uses a small dependency-free markdown converter
(`markdownToHtml.ts`); math and footnotes fall through as source. Task checkboxes
are custom-drawn (`appearance: none` + a CSS check) so they stay visible in light
mode, where a native checkbox all but disappears.

**Not verified live** (no display in this session): the title bar renders,
drags, and its window controls work; the popup menus and their actions;
Rename/Delete/New Window/Open Recent; theme flip; clipboard HTML; the unsaved
close prompt. All of it compiles (`cargo build`) and the pure logic (config
store, markdown→HTML) is unit-tested. One known caveat of `decorations: false`
on Linux: without WM decorations the window may lack resize borders on some
compositors -- a CSS/JS resize affordance would be a follow-up.

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
- **KaTeX rendering is unverified in a live webview.** Parsing, decoration
  building and the widget/cache logic are unit-tested (KaTeX is stubbed there,
  which also keeps its heavy module out of the parallel test workers), and the
  frontend bundles the CSS and fonts, but no formula has been confirmed to paint
  in the running app. Belongs in the Phase 5 verification pass.

## Verifying

`just run FILE` starts Vite and the app together. `just check` runs everything
that does not need a display.

Walk the cursor through every block type; drag selections across constructs;
toggle a checkbox and Ctrl+Z; edit the file externally while clean and while
dirty; close the window mid-typing and check the disk.
