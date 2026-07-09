# yap

A native-feeling, live-preview markdown editor for the desktop. The block under
your cursor shows raw markdown source; every other block renders rich — the
Typora / Obsidian "live preview" model — with no mode switch and no separate
preview pane.

Built as a daily driver: plain markdown files on disk, atomic saves, a file
watcher that reloads clean edits and prompts on conflicts, and a snappy launch.

> **Status:** Linux is the supported platform today (packaged as `.rpm` and
> AppImage). The stack is kept portable, but macOS and Windows are untested.

---

## Features

- **Live preview, block-scoped.** Put the cursor in a block and it reveals its
  markdown source; move away and it renders. Revealing a block only makes the
  markup characters *appear* — the text itself never restyles or reflows,
  because one proportional font is used for both raw and rendered text.
- **GitHub-Flavored Markdown** — headings, emphasis, links, lists, task
  checkboxes (clickable), fenced code with syntax highlighting, blockquotes,
  images.
- **LaTeX math** via KaTeX — inline `$x$` and block `$$…$$`.
- **Footnotes** — `[^ref]` references and definitions.
- **Rendered code blocks** hide their ` ``` ` fences (revealed while editing),
  keeping embedded-language syntax highlighting.
- **Inline images**, including ones **pasted from the clipboard** — the bytes
  are saved into an assets directory and an `![](…)` reference is inserted.
- **Real files, safely** — debounced autosave (~1s), atomic writes that preserve
  file permissions, a directory-level watcher, clean auto-reload that keeps your
  undo history and cursor, and a conflict prompt when the file changed on disk
  and your buffer is dirty.
- **Markdown shortcuts** — `Ctrl/Cmd+B` / `I` / `E` toggle bold / italic /
  inline code, `Ctrl/Cmd+K` wraps a link, `Tab` / `Shift+Tab` nest and un-nest
  list items.
- **Search & replace** with regex, match-case, and whole-word toggles
  (`Ctrl/Cmd+F`).
- **Custom themed title bar** with native File / Edit / Settings menus, light and
  dark themes that follow the system (with a manual override), and Open Recent.
- **One window, one file.** `yap file.md` opens an editor; each file is its own
  process, matching a window manager and a `%F` desktop entry.

---

## Installing / running

### Prerequisites

- [Rust](https://rustup.rs/) toolchain (for the Tauri shell)
- [Node.js](https://nodejs.org/) + npm (for the frontend)
- Tauri's Linux system dependencies (WebKitGTK etc.) — see the
  [Tauri prerequisites](https://tauri.app/start/prerequisites/)
- [`just`](https://github.com/casey/just) for the task recipes (optional but
  assumed below)

### From source

```bash
npm install                 # install frontend deps
just run path/to/file.md    # build the binary, start Vite, and launch yap
just run                    # launch with a fresh untitled buffer
```

`just run` compiles the debug binary, starts the Vite dev server, waits for it
to come up, then launches the app against it.

### Install the binary

```bash
just install    # builds the frontend, then `cargo install` into ~/.cargo/bin
```

After this, `yap file.md` works from anywhere.

### Release bundle

```bash
just bundle     # produces .rpm and AppImage under src-tauri/target/release/bundle
```

Bundling registers `.md` / `.markdown` with the `text/markdown` MIME type and
writes an `Exec=… %F` desktop entry, so files open one-process-per-file from the
file manager.

---

## Usage

| Action | How |
| --- | --- |
| Open a file | `yap file.md`, or File → Open |
| New untitled buffer | `yap` with no argument, or File → New |
| New window | File → New Window (spawns a separate process) |
| Save | `Ctrl/Cmd+S` (untitled buffers prompt for a location) |
| Bold / italic / inline code | `Ctrl/Cmd+B` / `I` / `E` |
| Insert link | `Ctrl/Cmd+K` |
| Follow a link | `Ctrl/Cmd+Click` |
| Nest / un-nest a list item | `Tab` / `Shift+Tab` |
| Find & replace | `Ctrl/Cmd+F` |
| Undo / redo | `Ctrl/Cmd+Z` / `Ctrl/Cmd+Shift+Z` (or `Ctrl+Y`) |
| Paste an image | `Ctrl/Cmd+V` with an image on the clipboard |
| Toggle a checkbox | Click it |

### Pasting images

Pasting an image from the clipboard writes it to `assets/` under `$YAP_HOME`
(see below) and inserts an `![](/absolute/path)` reference at the cursor. The
image renders immediately in live preview.

The assets directory is shared, not co-located with the note, and the reference
uses an absolute path — so a document that lives anywhere can reference it, but
the markdown is not self-contained if you move it to another machine.

---

## Configuration

yap keeps a small `state.json` (recent files, theme override) and the pasted-
image `assets/` directory under its **config home**:

1. `$YAP_HOME`, if set and non-empty;
2. otherwise `$XDG_CONFIG_HOME/yap` (or `~/.config/yap`).

Set `YAP_HOME` to relocate everything yap persists:

```bash
export YAP_HOME="$HOME/notes/.yap"
```

> **Note:** pasted images render through Tauri's asset protocol, whose scope is
> `$HOME/**` (`src-tauri/tauri.conf.json`). If you point `YAP_HOME` outside your
> home directory, pasted images will save but won't display until that scope is
> widened.

---

## Architecture

yap is a [Tauri 2](https://tauri.app/) app: a Rust shell around a WebKit webview
running a [Svelte 5](https://svelte.dev/) + TypeScript frontend built with Vite.
The editing core is [CodeMirror 6](https://codemirror.dev/); markdown is parsed
in-process by [Lezer](https://lezer.codemirror.net/)
(`@codemirror/lang-markdown` + GFM, plus custom math and footnote extensions).
There is **no IPC in the keystroke path** — rendering is pure CodeMirror
view-layer decoration.

```
src/
  App.svelte                     document lifecycle: open / save / rename / watch
  lib/
    editor/
      createEditor.ts            assembles the CodeMirror instance
      markdownShortcuts.ts       Ctrl+B/I/E/K, Tab list indent
      imagePaste.ts              clipboard-image paste handler
      linkHandler.ts             Ctrl+Click to follow links
      lezer/                     custom math + footnote grammar extensions
      livePreview/               the live-preview engine (see below)
    persistence/                 autosave, file watcher, atomic write, Tauri API
    export/                      markdown → HTML for Copy as HTML
    ui/                          title bar, status bar, dialogs
  styles/                        global + markdown CSS, CSS-variable themes

src-tauri/src/
  main.rs                        CLI arg resolution, one process per file
  lib.rs                         Tauri builder, command registration
  commands.rs                    read/write/watch/delete/rename, save_pasted_image
  config.rs                      state.json (recent files, theme), config home
  watcher.rs                     directory-level file watching
  menu.rs                        native popup menus
```

### The live-preview engine

The core is a single **`StateField`** (`livePreview/decorationField.ts`), not a
`ViewPlugin` — decorations that affect vertical layout (block widgets, multi-line
replaces, tall inline widgets) must be known to the state before the view
measures heights, or scrolling jumps.

- **`activeRegions.ts`** (pure, heavily unit-tested) decides which blocks show
  raw source for the current selection. Multi-line constructs (fences,
  blockquotes, block math, footnote definitions) reveal as a whole; leaf blocks
  reveal individually; multi-cursor works for free. "Raw" suppresses only
  replace/widget decorations — mark and line styling persist, which is why
  revealing a block doesn't restyle its text.
- **`builders/`** turn syntax-tree nodes into decorations, one builder per
  disjoint set of node names (headings, code blocks, images, links, tasks, math,
  footnotes, inline emphasis).
- **Block pinning** (`pinning.ts`): while the cursor edits inside a block, that
  block stays pinned raw and the rest of the document keeps its existing
  decorations, mapped through the edit. A half-typed fence otherwise corrupts the
  parse of everything after it, so the whole tail of the file would stop
  rendering.

For the full decision record — every non-obvious choice and the pitfalls behind
it — see [`docs/PLAN.md`](docs/PLAN.md).

### Rust side

- Reads and writes are Tauri commands; the frontend never touches the filesystem
  directly.
- `write_file_atomic` writes to a temp file in the target's own directory,
  `fsync`s, then `rename`s over the target (atomic only within one filesystem),
  preserving the target's permission bits.
- `start_watch` watches the **parent directory** filtered to the filename — a
  file-level watch goes deaf when another editor rename-replaces the file.
- Self-save echoes are detected by **content comparison**, not mtime, so the
  watcher is immune to timing races.

---

## Development

```bash
just check       # everything that doesn't need a display: tests + typecheck
just test        # vitest (frontend) + cargo test (Rust)
just typecheck   # svelte-check
just run FILE    # build + launch against the Vite dev server
```

The most valuable tests live in `src/lib/editor/livePreview/` — the
`activeRegions` policy and the builder snapshot tests over adversarial markdown
fixtures. Anything touching rendering behavior should keep those green.

To sanity-check a change by hand: walk the cursor through every block type; drag
selections across constructs; toggle a checkbox and undo; edit the file
externally while the buffer is clean and while it's dirty; close the window
mid-typing and check the disk.

---

## License

MIT
</content>
</invoke>
