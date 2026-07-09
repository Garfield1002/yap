//! Lezer `MarkdownConfig` extensions contributed by plugins, mirroring the
//! built-in math/footnote ones. Collected before the editor is created (the
//! parser is fixed at construction), so a toggled plugin takes effect on the
//! editor reload the loader triggers.

import type { MarkdownExtension } from "@lezer/markdown";

const extensions: MarkdownExtension[] = [];

/** Register a grammar extension. Returns a disposer for the loader's teardown. */
export function registerGrammar(ext: MarkdownExtension): () => void {
  extensions.push(ext);
  return () => {
    const i = extensions.indexOf(ext);
    if (i >= 0) extensions.splice(i, 1);
  };
}

export function pluginGrammar(): MarkdownExtension[] {
  return extensions;
}
