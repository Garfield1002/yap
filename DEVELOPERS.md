# Developing bulletmd

This guide covers building bulletmd from source, its architecture, and the
day-to-day development workflow. For a user-facing overview see the
[README](README.md); for the full design decision record see
[`docs/PLAN.md`](docs/PLAN.md).

## Building from source

### Prerequisites

- [Rust](https://rustup.rs/) toolchain (for the Tauri shell)
- [Node.js](https://nodejs.org/) + npm (for the frontend)
- Tauri's platform prerequisites — WebKitGTK and related packages on Linux, or
  Xcode command-line tools on macOS — see the
  [Tauri prerequisites](https://tauri.app/start/prerequisites/)
- [`just`](https://github.com/casey/just) for the task recipes (optional but
  assumed below)

### Run from source

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

After this, `bulletmd file.md` works from anywhere. Note the source-oriented
`just install` recipe only places the CLI binary in `~/.cargo/bin`; use a
release bundle when desktop application registration (icon, file association) is
required.

### Release bundles

```bash
just bundle          # bundles for the current platform
just bundle-linux    # .deb, .rpm, and AppImage; run on Linux
just bundle-macos    # bulletmd.app and .dmg; run on macOS
```

Bundles are written under `src-tauri/target/release/bundle/`.

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

## Architecture

bulletmd is a [Tauri 2](https://tauri.app/) app: a Rust shell around a WebKit
webview running a [Svelte 5](https://svelte.dev/) + TypeScript frontend built
with Vite. The editing core is [CodeMirror 6](https://codemirror.dev/); markdown
is parsed in-process by [Lezer](https://lezer.codemirror.net/)
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

## Development workflow

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

## Plugins

Plugins are optional feature packages: a folder containing a `manifest.json`, a
JavaScript entry point, and any CSS or data the feature needs. They run as
trusted code inside bulletmd, so install only plugins you wrote or reviewed.

To install one, open **Settings → Plugins → Install Plugin…** and choose the
plugin folder — the folder that directly contains `manifest.json`. bulletmd
copies it into `$BULLETMD_HOME/plugins/` (or `~/.config/bulletmd/plugins/` by
default); enable or disable installed plugins from the same menu. To update a
plugin, replace its installed folder and re-enable it.

The bundled plugin folders and their individual setup guides live in
[`plugins/`](plugins/README.md). The full plugin API is documented in
[`docs/PLUGINS.md`](docs/PLUGINS.md).
