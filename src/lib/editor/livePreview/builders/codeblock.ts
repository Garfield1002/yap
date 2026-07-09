import type { SyntaxNodeRef } from "@lezer/common";
import type { Builder } from "../builder";
import { slabLines, dimMarks } from "./block";

/**
 * Gives fenced and indented code blocks a monospace, tinted slab -- the same
 * `cm-block-*` slab block math uses (see `block.ts`).
 *
 * A rendered fence collapses its ``` lines entirely -- newline included -- so
 * the slab shows only the code, the way block math drops its `$$`. The
 * slab's rounded top/bottom then land on the first and last *content* lines.
 * When the block is pinned raw (the cursor is editing it) the fences come back,
 * dimmed in place. Because a revealed fence line is the same height as the gap
 * it leaves when hidden, the toggle doesn't shift the document.
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

  // Only a fully-rendered fence with a body has fences worth collapsing.
  // Indented code has none; a raw block shows its source; a bodyless fence
  // (`lastLine - firstLine < 2`) would collapse to nothing.
  const raw = b.isRaw(node.from, node.to);
  const hideFences = node.name === "FencedCode" && !raw && lastLine - firstLine >= 2;

  slabLines(b, hideFences ? firstLine + 1 : firstLine, hideFences ? lastLine - 1 : lastLine);

  if (node.name !== "FencedCode") return;

  if (hideFences) {
    // Drop each fence line by hiding it together with the line break that
    // *precedes* it, folding the empty line up into the line above. Hiding the
    // *following* break instead would merge the fence into the first body line
    // and CodeMirror would drop that line's slab decorations with it. The
    // opening fence has no preceding line to fold into only when it is line 1.
    if (firstLine > 1) {
      b.hide(doc.line(firstLine - 1).to, doc.line(firstLine).to);
    } else {
      b.hide(doc.line(firstLine).from, doc.line(firstLine + 1).from);
    }
    b.hide(doc.line(lastLine - 1).to, doc.line(lastLine).to);
    return;
  }

  // Raw, or nothing to collapse: keep the fences visible but dimmed.
  dimMarks(b, node.node, "CodeMark");
  const info = node.node.getChild("CodeInfo");
  if (info) b.mark(info.from, info.to, "cm-md-mark");
}
