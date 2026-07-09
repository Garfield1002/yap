import { EditorSelection } from "@codemirror/state";
import { EditorView, keymap } from "@codemirror/view";
import { syntaxTree } from "@codemirror/language";

/**
 * A rendered block-math node is a single block widget with no interior cursor
 * position, so a plain arrow key moves the caret *past* it -- you can never
 * land inside to edit. These commands catch that: when a vertical move would
 * step over a rendered `BlockMath`, the caret is placed on the block's near
 * edge instead, which reveals its source (see `activeRegions`). From there the
 * revealed block is ordinary text and arrows move through it line by line, so
 * this only ever fires on the way *in*.
 */

/** A `BlockMath` whose whole range lies between `a` and `b`, if any. */
function blockMathBetween(
  state: EditorView["state"],
  a: number,
  b: number,
): { from: number; to: number } | null {
  const lo = Math.min(a, b);
  const hi = Math.max(a, b);
  let found: { from: number; to: number } | null = null;
  syntaxTree(state).iterate({
    from: lo,
    to: hi,
    enter: (node) => {
      if (node.name === "BlockMath" && node.from >= lo && node.to <= hi) {
        found = { from: node.from, to: node.to };
        return false;
      }
    },
  });
  return found;
}

function enterBlockMath(forward: boolean) {
  return (view: EditorView): boolean => {
    const range = view.state.selection.main;
    if (!range.empty) return false;

    const target = view.moveVertically(range, forward).head;
    if (target === range.head) return false; // already at the document edge

    const block = blockMathBetween(view.state, range.head, target);
    // Nothing skipped, or the caret is already inside (revealed): let the
    // default command move normally.
    if (!block || (range.head > block.from && range.head < block.to)) return false;

    const pos = forward ? block.from : block.to;
    view.dispatch({ selection: EditorSelection.cursor(pos), scrollIntoView: true });
    return true;
  };
}

/** Installed ahead of the default keymap so it wins the Arrow-key precedence tie. */
export const blockNavigation = keymap.of([
  { key: "ArrowDown", run: enterBlockMath(true) },
  { key: "ArrowUp", run: enterBlockMath(false) },
]);
