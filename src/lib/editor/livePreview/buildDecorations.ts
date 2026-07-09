import type { EditorState } from "@codemirror/state";
import { ensureSyntaxTree, syntaxTree } from "@codemirror/language";
import { activeRegions, type Region } from "./activeRegions";
import { Builder, type Decorations } from "./builder";
import { headings } from "./builders/headings";
import { inline } from "./builders/inline";
import { links } from "./builders/links";
import { images } from "./builders/images";
import { tasklist } from "./builders/tasklist";
import { codeblock } from "./builders/codeblock";
import { math } from "./builders/math";
import { footnotes } from "./builders/footnotes";

type NodeBuilder = (node: Parameters<typeof headings>[0], b: Builder) => boolean | void;

/** Order is irrelevant: each builder keys off a disjoint set of node names. */
const BUILDERS: NodeBuilder[] = [
  headings,
  codeblock,
  images,
  links,
  tasklist,
  math,
  footnotes,
  inline,
];

/** v1 renders no table markup; a half-decorated table is worse than a raw one. */
const OPAQUE = new Set(["Table"]);

/**
 * `regions` names the blocks to show as raw source. It defaults to whatever the
 * selection implies, but the decoration field pins it while a block is being
 * edited -- see `decorationField.ts`.
 */
export function buildDecorations(state: EditorState, regions?: Region[]): Decorations {
  // Without this the parse only covers the viewport-ish prefix on a large file
  // and the tail would render as plain text until the user scrolled into it.
  ensureSyntaxTree(state, state.doc.length, 100);

  const b = new Builder(state, regions ?? activeRegions(state));

  syntaxTree(state).iterate({
    enter: (node) => {
      if (OPAQUE.has(node.name)) return false;
      for (const build of BUILDERS) {
        if (build(node, b) === false) return false;
      }
      return undefined;
    },
  });

  return b.finish();
}
