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
 * and CodeMirror throws. Block math is a `block: true` replace, which is legal
 * only because these decorations come from a StateField (see decorationField).
 */
export function math(node: SyntaxNodeRef, b: Builder): boolean | void {
  if (node.name === "InlineMath") {
    b.replace(node.from, node.to, new MathWidget(texOf(node.node, b.state), false));
    return false;
  }

  if (node.name === "BlockMath") {
    b.replace(node.from, node.to, new MathWidget(texOf(node.node, b.state), true), true);
    return false;
  }
}
