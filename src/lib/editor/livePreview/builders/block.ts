import type { SyntaxNode } from "@lezer/common";
import type { Builder } from "../builder";

/**
 * Shared rendering for the "fenced" block constructs -- fenced code and block
 * math. They look identical while being edited: the source lines sit in one
 * monospace slab (`cm-block-*`) with the delimiter lines dimmed but visible.
 * They diverge only in how they render once the cursor leaves, which each
 * builder supplies itself (code hides its fences and keeps the highlighted
 * source; math swaps in a KaTeX widget).
 *
 * Keeping the revealed/raw state in one place is what makes the two blocks feel
 * like the same thing: the same slab, the same dimmed fences, and -- because
 * neither replaces its source atomically while raw -- the same line-by-line
 * cursor movement through them.
 */

/** Slab the document lines `[firstLine, lastLine]`, rounding the outer corners. */
export function slabLines(b: Builder, firstLine: number, lastLine: number): void {
  const doc = b.state.doc;
  for (let n = firstLine; n <= lastLine; n++) {
    const line = doc.line(n);
    b.line(line.from, "cm-block-line");
    if (n === firstLine) b.line(line.from, "cm-block-first");
    if (n === lastLine) b.line(line.from, "cm-block-last");
  }
}

/** Dim the delimiter marks (``` or `$$`) in place rather than hiding them. */
export function dimMarks(b: Builder, node: SyntaxNode, markName: string): void {
  for (const mark of node.getChildren(markName)) {
    b.mark(mark.from, mark.to, "cm-md-mark");
  }
}
