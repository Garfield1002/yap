import type { SyntaxNode, SyntaxNodeRef } from "@lezer/common";
import type { EditorState } from "@codemirror/state";
import type { Builder } from "../builder";
import { MathWidget, renderedMathHeight } from "../widgets/MathWidget";
import { slabLines, dimMarks } from "./block";

/** The tex between the delimiters. Handles an unterminated block (one mark). */
function texOf(node: SyntaxNode, state: EditorState): string {
  const marks = node.getChildren("MathMark");
  const from = marks.length > 0 ? marks[0].to : node.from;
  const to = marks.length > 1 ? marks[marks.length - 1].from : node.to;
  return state.doc.sliceString(from, to).trim();
}

/**
 * `$x$` and `$$x$$` become KaTeX. The whole node is replaced, so nothing may
 * descend into it -- a decoration inside a replaced range partially overlaps it
 * and CodeMirror throws.
 *
 * Block math is treated exactly like a fenced code block, sharing its slab
 * rendering (`block.ts`): when the cursor is away it renders as a boxed widget;
 * when it is being edited the block shows its source in the same `cm-block-*`
 * slab, with the `$$` delimiters dimmed rather than hidden. `BlockMath` is a
 * pinned block container (see `activeRegions`), and the rendered widget is
 * *non-atomic*, so an arrow key lands on the block and reveals it -- just like
 * moving onto a code line -- instead of skipping over it.
 */
export function math(node: SyntaxNodeRef, b: Builder): boolean | void {
  if (node.name === "InlineMath") {
    b.replace(node.from, node.to, new MathWidget(texOf(node.node, b.state), false));
    return false;
  }

  if (node.name === "BlockMath") {
    const block = node.node;

    if (b.isRaw(node.from, node.to)) {
      // Editing: box the source, dim the fences. Same slab as `codeblock`.
      const doc = b.state.doc;
      slabLines(b, doc.lineAt(node.from).number, doc.lineAt(node.to).number);
      dimMarks(b, block, "MathMark");
      const previous = renderedMathHeight(texOf(block, b.state));
      if (previous) {
        const first = doc.lineAt(node.from).number;
        const last = doc.lineAt(node.to).number;
        const sourceRows = last - first + 1;
        const lastLine = doc.line(last);
        b.lineAttributes(lastLine.from, {
          class: "cm-math-source-active",
          style: `min-height: ${24 + Math.max(0, previous - sourceRows * 24)}px; box-sizing: border-box;`,
        });
      }
      return false;
    }

    // `block: true` replace, legal only because these come from a StateField;
    // `atomic: false` so the cursor can enter the block instead of hopping past.
    b.replace(node.from, node.to, new MathWidget(texOf(block, b.state), true), true, false);
    return false;
  }
}
