# Marp Slides plugin

Marp Slides lays the document out as editable 16:9 slide canvases directly in
bulletmd's live-preview editor. It also offers HTML, PDF, and PowerPoint (`.pptx`)
exports through the command palette and the Plugins menu.

## Export prerequisites

Inline slide mode uses bulletmd's live Markdown renderer and needs no external
program. For standalone exports, install
[Marp CLI](https://github.com/marp-team/marp-cli) so the `marp` command is
available on `PATH`:

```bash
npm install --global @marp-team/marp-cli
```

PDF and PowerPoint export also require a browser supported by Marp CLI (Chrome,
Edge, or Firefox). HTML export does not.

## Install and use

1. Install this `marp/` directory through **Settings → Plugins → Install
   Plugin…**, then enable **Marp Slides**.
2. Write a deck using Marp Markdown. Separate slides with `---`; front matter
   such as `marp: true`, `theme`, and `paginate` is passed through unchanged.
3. Slide mode is enabled by default. Click **Marp: slides** in the status bar,
   press `Ctrl/Cmd+Shift+M`, or run **Marp: Toggle Slide Mode** to switch between
   slide canvases and the normal document layout. `---` starts a new slide;
   front matter is hidden until its source is selected.
4. Run **Marp: Export HTML…**, **Marp: Export PDF…**, or **Marp: Export
   PowerPoint…** from the command palette when you want a standalone file.

The export is generated from the current in-memory text, including edits that
autosave has not written yet.

## Local images and themes

For safety, Marp CLI blocks local files by default. To allow document-relative
images and local themes, add this plugin setting to its installed `data.json`:

```json
{
  "allowLocalFiles": true
}
```

Only enable this for Markdown you trust. Marp warns that local-file access can
expose files readable by your account to the generated presentation.
