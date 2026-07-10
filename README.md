# bulletmd

A native-feeling digital bullet journal built on Markdown. A subtle dot-grid
background gives notes the rhythm of paper and its opacity can be adjusted under
Settings → Appearance.
The block under your cursor shows raw markdown source; every other block renders rich — the
Typora / Obsidian "live preview" model — with no mode switch and no separate
preview pane.

Built as a daily driver: plain markdown files on disk, atomic saves, a file
watcher that reloads clean edits and prompts on conflicts, and a snappy launch.

> **Status:** Linux packages are available as `.deb`, `.rpm`, and AppImage.
> macOS application and DMG bundles are configured but still need testing on
> supported Apple hardware. Windows remains untested.

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
- **Optional plugins** for features that do not belong in the core editor,
  including spell check, PDF export, Zotero citations, and live Marp slides.
- **One window, one file.** `bulletmd file.md` opens an editor; each file is its own
  process, matching a window manager and a `%f` desktop entry.

---

## Installing / running

### Prerequisites

- [Rust](https://rustup.rs/) toolchain (for the Tauri shell)
- [Node.js](https://nodejs.org/) + npm (for the frontend)
- Tauri's platform prerequisites — WebKitGTK and related packages on Linux, or
  Xcode command-line tools on macOS — see the
  [Tauri prerequisites](https://tauri.app/start/prerequisites/)
- [`just`](https://github.com/casey/just) for the task recipes (optional but
  assumed below)

### From source

```bash
npm install                 # install frontend deps
just run path/to/file.md    # build the binary, start Vite, and launch bulletmd
just run                    # launch with a fresh untitled buffer
```

`just run` compiles the debug binary, starts the Vite dev server, waits for it
to come up, then launches the app against it.

### Install the binary

```bash
just install    # builds the frontend, then `cargo install` into ~/.cargo/bin
```

After this, `bulletmd file.md` works from anywhere.

### Release bundles

```bash
just bundle          # bundles for the current platform
just bundle-linux    # .deb, .rpm, and AppImage; run on Linux
just bundle-macos    # bulletmd.app and .dmg; run on macOS
```

Bundles are written under `src-tauri/target/release/bundle/`. Install the
`.deb` or `.rpm` to register bulletmd's application entry, icon, and `text/markdown`
file association with Linux desktops such as KDE Plasma. The AppImage is
portable but does not install a permanent application-menu entry by itself.

On macOS, move `bulletmd.app` into Applications or install it from the DMG. Finder
opens associated `.md` / `.markdown` documents through the native application
event; bulletmd preserves its one-window, one-file behavior by launching a separate
process for each additional document.

The source-oriented `just install` recipe only places the CLI binary in
`~/.cargo/bin`; use a release bundle when desktop application registration is
required.

### Release automation and macOS signing

Pushing a `v*` tag or manually starting the **Release desktop bundles** GitHub
Actions workflow builds Linux x86_64 packages plus Apple Silicon and Intel
macOS bundles. It collects them in a draft GitHub release so the artifacts can
be tested before publishing.

macOS CI builds use an ad-hoc signature by default. For distributable Developer
ID signing and notarization, configure these repository secrets:

- `APPLE_CERTIFICATE`: base64-encoded Developer ID Application `.p12`;
- `APPLE_CERTIFICATE_PASSWORD`: password used when exporting that certificate;
- `KEYCHAIN_PASSWORD`: throwaway password for the CI keychain;
- `APPLE_ID`: Apple account email;
- `APPLE_PASSWORD`: app-specific password for that account;
- `APPLE_TEAM_ID`: Apple Developer team identifier.

When the certificate is present, the workflow imports it and Tauri signs the
bundles. When all three Apple account values are also present, Tauri submits the
signed application for notarization and staples the result.

---

## Usage

| Action | How |
| --- | --- |
| Open a file | `bulletmd file.md`, or File → Open |
| New untitled buffer | `bulletmd` with no argument, or File → New |
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

Pasting an image from the clipboard writes it to `assets/` under `$BULLETMD_HOME`
(see below) and inserts an `![](/absolute/path)` reference at the cursor. The
image renders immediately in live preview.

The assets directory is shared, not co-located with the note, and the reference
uses an absolute path — so a document that lives anywhere can reference it, but
the markdown is not self-contained if you move it to another machine.

---

## Configuration

bulletmd keeps a small `state.json` (recent files and appearance preferences) and the pasted-
image `assets/` directory under its **config home**:

1. `$BULLETMD_HOME`, if set and non-empty;
2. otherwise `$XDG_CONFIG_HOME/bulletmd` (or `~/.config/bulletmd`).

Set `BULLETMD_HOME` to relocate everything bulletmd persists:

```bash
export BULLETMD_HOME="$HOME/notes/.bulletmd"
```

> **Note:** pasted images render through Tauri's asset protocol, whose scope is
> `$HOME/**` (`src-tauri/tauri.conf.json`). If you point `BULLETMD_HOME` outside your
> home directory, pasted images will save but won't display until that scope is
> widened.

## Plugins

Plugins are optional feature packages: a folder containing a `manifest.json`, a
JavaScript entry point, and any CSS or data the feature needs. They run as
trusted code inside bulletmd, so install only plugins you wrote or reviewed.

To install one, open **Settings → Plugins → Install Plugin…** and choose the
plugin folder — the folder that directly contains `manifest.json`. bulletmd copies
it into `$BULLETMD_HOME/plugins/` (or `~/.config/bulletmd/plugins/` by default); enable or
disable installed plugins from the same menu. To update a plugin, replace its
installed folder and re-enable it.

The bundled plugin folders and their individual setup guides live in
[`plugins/`](plugins/README.md). They can also be installed manually by copying
their folder into the plugin directory above.

### Zotero citations

The `plugins/zotero/` plugin searches Zotero Desktop and inserts a
Pandoc-style citation such as `[@doe2024]`. Open **Zotero: Insert Citation…**
from the command palette, Plugins menu, or Zotero status-bar item, then search
by title or creator.

Inserted citations render as compact numbered references (`[1]`) outside the
cursor range. Add `<!--bibliography-->` on its own line to render the cited
items as `[number] title, year, authors` in first-citation order.

Before using it, enable Zotero's local HTTP API in **Zotero → Settings →
Advanced → Allow other applications on this computer to communicate with
Zotero**. Zotero can be open while you change the setting, but fully restart it
if the picker still cannot connect. The plugin reads only from
`http://127.0.0.1:23119/api/`; your library stays on the local machine. You can
verify the endpoint after enabling it:

```bash
curl -i http://127.0.0.1:23119/api/
```

The plugin uses bulletmd's native HTTP client because Zotero's Local API does not
grant browser CORS access. It uses a Better BibTeX
`Citation Key:` value from an item's **Extra** field when present; otherwise it
inserts Zotero's eight-character item key. See
[Zotero's Local API documentation](https://www.zotero.org/support/dev/web_api/v3/local_api)
for the preference and endpoint details.

---

## Architecture

bulletmd is a [Tauri 2](https://tauri.app/) app: a Rust shell around a WebKit webview
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
