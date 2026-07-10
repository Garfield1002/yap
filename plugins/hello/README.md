# Hello plugin

Hello is a minimal working example of the bulletmd plugin API. It is useful as a
starting point for a new plugin or as a quick check that plugin installation is
working.

## Install

Install this `hello/` directory through **Settings → Plugins → Install
Plugin…**, then enable **Hello** from the Plugins settings.

## What it adds

- A `Hello: Greet` command in the command palette, bound to `Ctrl/Cmd+Shift+H`.
- A `Say hello` item in the Plugins menu.
- A status-bar indicator.
- A subtle style for `TODO` inside inline-code spans, demonstrating a
  live-preview builder.

Read `main.js` alongside this guide: it deliberately touches the simple plugin
surfaces without importing CodeMirror packages. bulletmd supplies its own module
instances through `bulletmd.cm`, which prevents incompatible duplicate copies.
