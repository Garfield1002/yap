import type { SyntaxNodeRef } from "@lezer/common";
import type { Builder } from "../builder";

/**
 * Emphasis, strong, strikethrough and inline code. The visual styling is
 * already applied by the highlight style via Lezer tags; all this does is
 * collapse the markup characters.
 */
export function inline(node: SyntaxNodeRef, b: Builder): boolean | void {
  switch (node.name) {
    case "EmphasisMark":
    case "StrikethroughMark":
      // Only ever children of Emphasis/StrongEmphasis/Strikethrough.
      b.hide(node.from, node.to);
      return;

    case "InlineCode": {
      const open = node.node.firstChild;
      const close = node.node.lastChild;
      if (open?.name === "CodeMark") {
        b.hide(open.from, open.to);
        const end = close && close !== open && close.name === "CodeMark" ? close.from : node.to;
        if (close && close !== open && close.name === "CodeMark") b.hide(close.from, close.to);
        b.mark(open.to, end, "cm-inline-code");
      } else {
        b.mark(node.from, node.to, "cm-inline-code");
      }
      // Stop: `CodeMark` also names the fences of a FencedCode block, which
      // must stay visible. Handling it here keeps the two cases apart.
      return false;
    }
  }
}
