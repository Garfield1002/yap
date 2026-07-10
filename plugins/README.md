# yap plugins

Each directory here is an optional, installable feature for yap. A plugin is a
trusted JavaScript package that runs inside the application and can add editor
behavior, commands, menu entries, status-bar items, and styles.

## Install a bundled plugin

1. In yap, choose **Settings → Plugins → Install Plugin…**.
2. Choose one plugin directory from this folder, such as `spellcheck/`. Select
   the directory itself, which must contain `manifest.json`.
3. Enable the plugin from **Settings → Plugins**.

yap copies the plugin to `$YAP_HOME/plugins/`, or `~/.config/yap/plugins/` by
default. To update it, reinstall the directory and re-enable the plugin.

Install only plugins you trust: plugins execute with yap's webview privileges.

## Bundled plugins

- [Hello](hello/README.md) — a small example plugin and API reference by use.
- [Spell Check](spellcheck/README.md) — background Hunspell spell checking and
  correction suggestions.
- [PDF Export](pdf-export/README.md) — A4 page mode and printing to PDF.
- [Page Break](page-break/README.md) — deterministic A4 page-break spacing for
  use with PDF Export.
- [Zotero Citations](zotero/README.md) — search Zotero Desktop and insert
  Pandoc-style citations.
