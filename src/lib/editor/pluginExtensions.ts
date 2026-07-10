//! Raw CodeMirror extensions contributed by plugins.
//!
//! The builder and grammar hooks cover the common cases, but a plugin sometimes
//! needs a plain CM6 extension (a keymap, a view plugin, a facet). It builds one
//! against the app's own module instances (`bulletmd.cm.*`) and registers it here;
//! createEditor layers the collected set on. Collected before construction, so a
//! toggled plugin takes effect on the editor reload the loader triggers.

import type { Extension } from "@codemirror/state";

const extensions: Extension[] = [];

/** Register a raw editor extension. Returns a disposer for the loader. */
export function registerExtension(ext: Extension): () => void {
  extensions.push(ext);
  return () => {
    const i = extensions.indexOf(ext);
    if (i >= 0) extensions.splice(i, 1);
  };
}

export function pluginExtensions(): Extension[] {
  return extensions;
}
