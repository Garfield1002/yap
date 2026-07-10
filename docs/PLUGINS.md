# yap plugin system — design record

Decisions resolved 2026-07-09. This is the agreed design for the plugin
system, recorded before implementation. Format mirrors `PLAN.md`: each
section states the decision and the reasoning behind it, so future changes
argue with the reasons, not just the conclusions.

---

## Goal & posture

Plugins exist **first for yap's own author** — a way to add features
(spell check, exports, Zotero, Marp) without bloating core — while keeping
a public ecosystem *possible* later. That ordering drives every trust and
stability call below:

- **Plugins are trusted code.** No sandboxing; they run in the app's
  webview with the app's privileges. You wrote or reviewed them.
- **The API is explicitly unstable.** `manifest.json` carries
  `apiVersion: 1`; yap warns/refuses to load on a mismatch. This one field
  is the cheap seam that makes a stable v2 possible without archaeology —
  old plugins fail *cleanly* after a break instead of mysteriously.
- Per-plugin permissions, a settings UI, and API docs are all deferred
  until third-party authors are real.

Motivating plugin list (in build order): **spell check**, HTML export,
PDF export, Zotero connector (citations), Marp (slides).

---

## What a plugin is on disk

```
$YAP_HOME/plugins/<name>/
  manifest.json    # { name, version, apiVersion, entry }
  main.js          # single ESM bundle, exports activate(yap)
  data.json        # per-plugin settings (optional, hand-edited in v1)
  *.css            # optional styles, injected as <style>
```

- Authored in TypeScript, bundled with esbuild, with `@codemirror/*`
  marked **external** — plugins must never carry their own copy of the
  CodeMirror packages (see *API surface*).
- Entry point is `activate(yap)`. No `deactivate` contract in v1;
  unloading is per-registration teardown driven by the loader.
- Settings live in the plugin's own `data.json`, read/written through a
  `yap.settings` API. Core `state.json` stays core-only (except the
  enabled-plugins list). No settings UI in v1 — that's its own feature.

Rejected: single-file plugins (no home for CSS/assets/versioning) and
npm `package.json` (drags npm semantics yap won't honor).

---

## Runtime & loading

Plugins are **JS in the app's webview** — the only placement that can
return CodeMirror extensions and join the live-preview pipeline without
IPC in the keystroke path. Sandboxed iframes/workers were rejected
because message-passing can't express decorations; Rust/WASM plugins were
rejected for the same reason.

Load mechanics:

1. A Rust command reads the plugin source from `$YAP_HOME/plugins/`
   (the frontend still never touches the filesystem directly).
2. The frontend evaluates it via
   `import(URL.createObjectURL(new Blob([src], { type: "text/javascript" })))`.
   This needs one CSP change in `tauri.conf.json`: `blob:` added to
   `script-src`. Chosen over the asset protocol (MIME/scope fragility —
   see the pasted-image scope caveat in the README) and over
   `eval`/`new Function` (needs `unsafe-eval`, loses ESM semantics).
3. Plugin CSS is injected as `<style>` elements tagged with the plugin
   name, removed on disable.

**Failure isolation.** Each plugin loads in try/catch; every registration
it makes is tracked per-plugin so the loader can tear it down. A plugin
that throws is disabled and surfaces as a clickable status-bar indicator
showing the error. A broken plugin must never block opening a document.

**Enable/disable.** A Settings → Plugins submenu (webview menus, below)
lists discovered plugins with checkboxes; the enabled set persists in
`state.json`. Plain CM6 extensions toggle live via a `Compartment`;
plugins contributing live-preview builders or grammar extensions take
effect on reload (acceptable — toggling is rare, see *Editor hooks*).

---

## API surface

`activate(yap)` receives one injected object; there is no import
resolution magic. Two copies of `@codemirror/state` silently break
extension compatibility, so the app's own module instances ride on the
API object:

- `yap.cm.state`, `yap.cm.view`, `yap.cm.language`, … — the app's
  CodeMirror modules. Plugins build raw CM6 extensions against these.
- `yap.commands.register({ id, title, run, keybinding? })` — one shared
  command registry feeding palette, menus, and keybindings uniformly.
- `yap.menus.addItem(...)`, `yap.statusBar.addItem(...)` — UI surfaces.
- `yap.livePreview.registerBuilder(nodeNames, builder)` — join the
  decoration pipeline in `livePreview/decorationField.ts`, same contract
  as the built-in `builders/`.
- `yap.markdown.extendGrammar(markdownConfig)` — contribute Lezer
  `MarkdownConfig` extensions (like the existing math/footnote ones).
- `yap.settings.get()/set()` — the plugin's `data.json`.
- Escape hatches (see below): `yap.system.fetch`, `yap.system.readFile`,
  `yap.system.writeFile`, plus named task-specific commands.

Builder and grammar registrations are **collected before `createEditor`
runs** — the Lezer parser is fixed at construction and the decoration
`StateField` wants its builder set at creation. This avoids dynamic
parser reconstruction (real complexity) for the rare operation of
toggling a plugin. Curated-API-only was rejected as too much friction
while the author is the only customer; the raw-CM6 + hooks split matches
Obsidian's proven shape.

**System access** is a small set of generic *trusted* Tauri commands exposed
through `yap.system`: native `fetch(url)` (Zotero's local HTTP API), scoped
file read/write (exports), and spawning **named, allowlisted binaries only**
(e.g. `marp`) — no generic `shell(anything)`. The fetch client runs outside the
webview, so Zotero's browser CORS policy does not apply; Tauri capabilities
scope the URLs it may access. Keeping the hatches named and narrow is what
keeps a later per-plugin-permission retrofit tractable.

---

## UI surface: webview menus + command palette

The native muda menus retire. This is cheap because the architecture
already did the hard part: there is no persistent native menu bar — the
titlebar is custom Svelte, and each `menu.rs` popup is rebuilt per click
and does nothing but emit `menu-action` events the frontend already
handles. Porting means a Svelte dropdown component, fetching Open Recent
from the existing config command, keeping `new_window` as a tiny Tauri
command, and routing Cut/Copy/Paste through CodeMirror /
`navigator.clipboard` (CM already intercepts copy in the webview, so the
hidden-markup-copy behavior survives). Estimated ~a day.

On top of that, a **Ctrl+P command palette** over the shared command
registry. A plugin registers a command once and it appears in the
palette, can be placed in menus, and can bind a key — no Rust IPC
protocol for dynamic native menus.

---

## First customer: spell check

Spell check is the API's forcing function because it exercises the
deepest surface: decorations over text, interaction with live preview
(don't flag URLs, code, math), a suggestion UI, settings, and a system
dependency.

Engine: **Rust-side hunspell** (`hunspell-rs` or similar) behind a Tauri
command. Checking runs async and debounced, so decorations arrive after
the fact — this respects the no-IPC-in-the-keystroke-path rule, which
forbids *synchronous* IPC during editing, not background work. System
dictionaries come free from `/usr/share/hunspell`. Pure-JS `nspell` was
rejected (English-centric, weaker suggestions); WebKitGTK native
spellcheck was rejected (checks raw markdown, no programmatic control).

---

## Build order

Dependency-respecting sequence; each step is usable on its own:

1. **Port menus to the webview.** Retire the `menu.rs` popups; keep
   `new_window` (and anything else process-level) as commands.
2. **Command registry + palette + keybinding layer.** Core feature,
   plugin-agnostic.
3. **Plugin loader.** Discovery, manifest + apiVersion check, blob
   import, the `yap` API object, per-plugin registration tracking,
   error isolation, Settings → Plugins toggle UX.
4. **Editor hook points.** Thread collected builders and grammar
   extensions into `createEditor` / `decorationField`.
5. **Spell check plugin** — first customer, including the hunspell Rust
   command.
6. Then: HTML export, PDF export, Zotero connector, Marp.

## Zotero connector

The Zotero citation plugin lives in `plugins/zotero/`. It uses Zotero Desktop's
read-only Local API at `http://127.0.0.1:23119/api/`, so it needs Zotero's
**Settings → Advanced → Allow other applications on this computer to communicate
with Zotero** preference enabled. Its citation picker searches the local library
and inserts Pandoc-style `[@citekey]` markup. It prefers Better BibTeX's
`Citation Key:` entry from an item's Extra field and otherwise uses Zotero's
item key. See `plugins/zotero/README.md` for its optional endpoint/library
configuration.
