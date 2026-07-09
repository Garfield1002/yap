import type { SyntaxNodeRef } from "@lezer/common";
import type { Builder } from "../builder";

/**
 * Gives fenced and indented code blocks a monospace, tinted slab.
 *
 * The fence lines are dimmed rather than hidden. Collapsing them would delete
 * two lines from the layout every time the cursor leaves the block, and the
 * document would jump.
 *
 * Nothing is replaced here, so the highlighting of the embedded language --
 * which comes from the nested Lezer parse, not from decorations -- is untouched.
 */
export function codeblock(node: SyntaxNodeRef, b: Builder): boolean | void {
  if (node.name !== "FencedCode" && node.name !== "CodeBlock") return;

  const doc = b.state.doc;
  const firstLine = doc.lineAt(node.from).number;
  const lastLine = doc.lineAt(node.to).number;

  for (let n = firstLine; n <= lastLine; n++) {
    const line = doc.line(n);
    b.line(line.from, "cm-code-line");
    if (n === firstLine) b.line(line.from, "cm-code-first");
    if (n === lastLine) b.line(line.from, "cm-code-last");
  }

  if (node.name === "FencedCode") {
    for (const mark of node.node.getChildren("CodeMark")) {
      b.mark(mark.from, mark.to, "cm-md-mark");
    }
    const info = node.node.getChild("CodeInfo");
    if (info) b.mark(info.from, info.to, "cm-md-mark");
  }
}
