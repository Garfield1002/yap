import type { EditorState } from "@codemirror/state";
import { ensureSyntaxTree, syntaxTree } from "@codemirror/language";
import { activeRegions } from "./activeRegions";
import { Builder, type Decorations } from "./builder";
import { headings } from "./builders/headings";
import { inline } from "./builders/inline";

type NodeBuilder = (node: Parameters<typeof headings>[0], b: Builder) => boolean | void;

const BUILDERS: NodeBuilder[] = [headings, inline];

/** v1 renders no table markup; a half-decorated table is worse than a raw one. */
const OPAQUE = new Set(["Table"]);

export function buildDecorations(state: EditorState): Decorations {
  // Without this the parse only covers the viewport-ish prefix on a large file
  // and the tail would render as plain text until the user scrolled into it.
  ensureSyntaxTree(state, state.doc.length, 100);

  const regions = activeRegions(state);
  const b = new Builder(state, regions);

  syntaxTree(state).iterate({
    enter: (node) => {
      if (OPAQUE.has(node.name)) return false;
      for (const build of BUILDERS) {
        const result = build(node, b);
        if (result === false) return false;
      }
      return undefined;
    },
  });

  return b.finish();
}
