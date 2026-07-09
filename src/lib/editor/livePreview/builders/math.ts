import type { SyntaxNode, SyntaxNodeRef } from "@lezer/common";
import type { EditorState } from "@codemirror/state";
import type { Builder } from "../builder";
import { MathWidget } from "../widgets/MathWidget";

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
 * Block math is treated exactly like a fenced code block: when the cursor is
 * away it renders as a boxed widget; when it is being edited the block shows its
 * source in the same slab, with the `$$` delimiters dimmed rather than hidden.
 * That, together with `BlockMath` being a pinned block container (see
 * `activeRegions`), gives it the code block's steady, non-reflowing feel.
 */
export function math(node: SyntaxNodeRef, b: Builder): boolean | void {
  if (node.name === "InlineMath") {
    b.replace(node.from, node.to, new MathWidget(texOf(node.node, b.state), false));
    return false;
  }

  if (node.name === "BlockMath") {
    const block = node.node;

    if (b.isRaw(node.from, node.to)) {
      // Editing: box the source, dim the fences. Mirrors `codeblock`.
      const doc = b.state.doc;
      const firstLine = doc.lineAt(node.from).number;
      const lastLine = doc.lineAt(node.to).number;
      for (let n = firstLine; n <= lastLine; n++) {
        const line = doc.line(n);
        b.line(line.from, "cm-math-line");
        if (n === firstLine) b.line(line.from, "cm-math-first");
        if (n === lastLine) b.line(line.from, "cm-math-last");
      }
      for (const mark of block.getChildren("MathMark")) b.mark(mark.from, mark.to, "cm-md-mark");
      return false;
    }

    // `block: true` replace, legal only because these come from a StateField.
    b.replace(node.from, node.to, new MathWidget(texOf(block, b.state), true), true);
    return false;
  }
}
