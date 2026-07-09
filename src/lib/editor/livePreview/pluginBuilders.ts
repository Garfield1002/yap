//! Live-preview builders contributed by plugins.
//!
//! The built-in builders in `buildDecorations.ts` are a fixed module-level list.
//! Plugins add to *this* list instead. The decoration `StateField` reads it when
//! it builds, so a builder registered before the editor is created is picked up;
//! toggling a plugin re-creates the editor (see the loader), which is why this
//! does not need to be reactive.

import type { SyntaxNodeRef } from "@lezer/common";
import type { Builder } from "./builder";

/** Same contract as the built-in builders: inspect a node, emit decorations,
 *  return `false` to stop descending into it. */
export type PluginBuilder = (node: SyntaxNodeRef, b: Builder) => boolean | void;

interface Entry {
  /** Node names this builder cares about; null means "every node". */
  names: Set<string> | null;
  build: PluginBuilder;
}

const entries: Entry[] = [];

/** Register a builder scoped to `nodeNames` (or all nodes when null). Returns a
 *  disposer the plugin loader calls on teardown. */
export function registerBuilder(nodeNames: string[] | null, build: PluginBuilder): () => void {
  const entry: Entry = { names: nodeNames ? new Set(nodeNames) : null, build };
  entries.push(entry);
  return () => {
    const i = entries.indexOf(entry);
    if (i >= 0) entries.splice(i, 1);
  };
}

/** The registered builders as plain node-builders, node-name filtering folded
 *  in, for `buildDecorations` to run alongside the built-ins. */
export function pluginBuilders(): PluginBuilder[] {
  return entries.map(
    (entry): PluginBuilder =>
      (node, b) => {
        if (entry.names && !entry.names.has(node.name)) return;
        return entry.build(node, b);
      },
  );
}
