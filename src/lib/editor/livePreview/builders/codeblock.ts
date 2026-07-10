import type { SyntaxNodeRef } from "@lezer/common";
import type { Builder } from "../builder";
import { slabLines, dimMarks } from "./block";

/**
 * Gives fenced and indented code blocks a monospace, tinted slab -- the same
 * `cm-block-*` slab block math uses (see `block.ts`).
 *
 * A rendered fence hides its ``` characters but keeps the fence lines as blank
 * slab rows, so the slab spans the same lines whether or not the cursor is in
 * it. When the block is pinned raw (the cursor is editing it) the ``` come
 * back, dimmed in place. Because the fence line stays a full line tall either
 * way, the toggle never shifts the document.
 *
 * Nothing about the code text is replaced, so the highlighting of the embedded
 * language -- which comes from the nested Lezer parse, not from decorations --
 * is untouched. The line-break-spanning hides are legal only because these
 * decorations come from a StateField, not a plugin.
 */
export function codeblock(node: SyntaxNodeRef, b: Builder): boolean | void {
  if (node.name !== "FencedCode" && node.name !== "CodeBlock") return;

  const doc = b.state.doc;
  const firstLine = doc.lineAt(node.from).number;
  const lastLine = doc.lineAt(node.to).number;

  // Only a fully-rendered fence with a body has ``` lines worth hiding.
  // Indented code has none; a raw block shows its source; a bodyless fence
  // (`lastLine - firstLine < 2`) has no interior to distinguish from its marks.
  const raw = b.isRaw(node.from, node.to);
  const hideFences = node.name === "FencedCode" && !raw && lastLine - firstLine >= 2;

  slabLines(b, firstLine, lastLine);

  if (node.name !== "FencedCode") return;

  if (hideFences) {
    // Hide the ``` text on each fence line but keep the line itself, so the
    // block stays the same height and the slab keeps its full extent. Hiding
    // stays *within* each line -- no line break is folded -- so CodeMirror
    // never drops a line's slab decorations.
    const open = doc.line(firstLine);
    const close = doc.line(lastLine);
    b.hide(open.from, open.to);
    b.hide(close.from, close.to);
    return;
  }

  // Raw, or nothing to collapse: keep the fences visible but dimmed.
  dimMarks(b, node.node, "CodeMark");
  const info = node.node.getChild("CodeInfo");
  if (info) b.mark(info.from, info.to, "cm-md-mark");
}
