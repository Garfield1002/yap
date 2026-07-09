import type { SyntaxNodeRef } from "@lezer/common";
import type { Builder } from "../builder";

/**
 * `[^label]` renders as a superscript label with the brackets hidden;
 * `[^label]: text` keeps its marker (merely dimmed) and inline-renders its body.
 */
export function footnotes(node: SyntaxNodeRef, b: Builder): boolean | void {
  if (node.name === "FootnoteReference") {
    const ref = node.node;
    const open = ref.firstChild; // `[^`
    const close = ref.lastChild; // `]`
    if (open?.name === "FootnoteMark" && close?.name === "FootnoteMark" && open !== close) {
      b.hide(open.from, open.to);
      b.hide(close.from, close.to);
      b.mark(open.to, close.from, "cm-footnote-ref");
    }
    return false;
  }

  if (node.name === "FootnoteDefinition") {
    b.line(node.from, "cm-footnote-def");
    const marker = node.node.firstChild;
    if (marker?.name === "FootnoteMark") b.mark(marker.from, marker.to, "cm-md-mark");
    // Descend: the definition body may hold links and emphasis.
    return;
  }
}
