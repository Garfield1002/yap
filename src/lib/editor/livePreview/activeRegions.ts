import type { EditorState } from "@codemirror/state";
import { syntaxTree } from "@codemirror/language";
import type { SyntaxNode, Tree } from "@lezer/common";

/** A half-open document range whose markdown source should be shown raw. */
export interface Region {
  from: number;
  to: number;
}

/**
 * Multi-line constructs that reveal as a whole. Putting the cursor anywhere
 * inside a fence, a quote, or a footnote definition shows the entire thing,
 * because editing one line of them without seeing the others is miserable.
 *
 * The *highest* matching ancestor wins, so a fence nested in a quote reveals
 * the quote.
 */
export const BLOCK_CONTAINERS = new Set([
  "FencedCode",
  "CodeBlock",
  "Blockquote",
  "BlockMath",
  "FootnoteDefinition",
  "Table",
  "HTMLBlock",
  "CommentBlock",
]);

/**
 * Leaf blocks. The *nearest* matching ancestor wins, so inside a list the
 * active region is the item's paragraph rather than the whole list.
 */
export const LEAF_BLOCKS = new Set([
  "Paragraph",
  "ATXHeading1",
  "ATXHeading2",
  "ATXHeading3",
  "ATXHeading4",
  "ATXHeading5",
  "ATXHeading6",
  "SetextHeading1",
  "SetextHeading2",
  "HorizontalRule",
  "LinkReference",
  "Comment",
]);

interface BlockHit {
  region: Region;
  /** A multi-line container (code/math/quote/…) as opposed to a leaf block. */
  container: boolean;
}

function blockFromNode(node: SyntaxNode | null): BlockHit | null {
  let highestContainer: SyntaxNode | null = null;
  let nearestLeaf: SyntaxNode | null = null;

  for (let n: SyntaxNode | null = node; n; n = n.parent) {
    if (BLOCK_CONTAINERS.has(n.name)) highestContainer = n;
    else if (!nearestLeaf && LEAF_BLOCKS.has(n.name)) nearestLeaf = n;
  }

  const hit = highestContainer ?? nearestLeaf;
  if (!hit) return null;
  return { region: { from: hit.from, to: hit.to }, container: hit === highestContainer };
}

/**
 * The block containing `pos`, or null when `pos` sits in structural whitespace
 * (a blank line between blocks).
 *
 * Resolution is tried from both sides: at a block's last position only the
 * `-1` side lands inside it, and at its first position only `+1` does. When
 * `pos` is strictly interior both agree.
 *
 * A block *container* on either side wins over a leaf on the other. A caret at
 * the top edge of a block-math (or code) block otherwise resolves to the
 * paragraph above -- and since block math renders as a widget with no interior
 * cursor stop, an arrow key can only ever land the caret on that edge. Without
 * this, moving onto the block would reveal the neighbour instead of the block.
 */
export function blockAt(tree: Tree, pos: number): Region | null {
  const minus = blockFromNode(tree.resolveInner(pos, -1));
  const plus = blockFromNode(tree.resolveInner(pos, 1));

  if (minus?.container) return minus.region;
  if (plus?.container) return plus.region;
  return (minus ?? plus)?.region ?? null;
}

/**
 * Every range of source that must render as raw markdown for the current
 * selection. Pure: same state in, same regions out. This is the whole
 * cursor-reveal policy, and it is where the unit tests point.
 */
export function activeRegions(state: EditorState): Region[] {
  const tree = syntaxTree(state);
  const doc = state.doc;
  const regions: Region[] = [];

  for (const range of state.selection.ranges) {
    let from = range.from;
    let to = range.to;

    // A selection ending exactly at a line start hasn't really touched that
    // line's block; don't reveal it.
    if (to > from && doc.lineAt(to).from === to) to -= 1;

    const a = blockAt(tree, from) ?? lineRegion(doc, from);
    const b = to === from ? a : (blockAt(tree, to) ?? lineRegion(doc, to));

    regions.push({
      from: doc.lineAt(Math.min(a.from, b.from)).from,
      to: doc.lineAt(Math.max(a.to, b.to)).to,
    });
  }

  return mergeRegions(regions);
}

function lineRegion(doc: EditorState["doc"], pos: number): Region {
  const line = doc.lineAt(pos);
  return { from: line.from, to: line.to };
}

/** Sort by start and coalesce overlapping or touching regions. */
export function mergeRegions(regions: Region[]): Region[] {
  if (regions.length < 2) return regions;
  const sorted = [...regions].sort((x, y) => x.from - y.from || x.to - y.to);
  const out: Region[] = [sorted[0]];
  for (const r of sorted.slice(1)) {
    const last = out[out.length - 1];
    if (r.from <= last.to) last.to = Math.max(last.to, r.to);
    else out.push(r);
  }
  return out;
}

/** True when [from, to) intersects any raw region. Touching endpoints count. */
export function overlapsRegion(regions: Region[], from: number, to: number): boolean {
  for (const r of regions) {
    if (from <= r.to && to >= r.from) return true;
  }
  return false;
}
