import type { SyntaxNode, SyntaxNodeRef } from "@lezer/common";
import type { EditorState } from "@codemirror/state";
import type { Builder } from "../builder";

/** The `]` that closes the link text, i.e. the first `]` among the LinkMarks. */
function closingBracket(link: SyntaxNode, state: EditorState): SyntaxNode | null {
  for (let child = link.firstChild; child; child = child.nextSibling) {
    if (child.name === "LinkMark" && state.doc.sliceString(child.from, child.to) === "]") {
      return child;
    }
  }
  return null;
}

/**
 * `[text](url)` renders as just `text`.
 *
 * The leading `[` collapses as one replace and the whole tail -- `](url "title")`
 * -- as another. Link text keeps its own markup, so bold inside a link renders.
 */
export function links(node: SyntaxNodeRef, b: Builder): boolean | void {
  if (node.name === "Link") {
    const link = node.node;
    const open = link.firstChild;
    const close = closingBracket(link, b.state);
    if (!open || open.name !== "LinkMark" || !close) return;

    b.hide(open.from, open.to);
    b.hide(close.from, link.to);
    b.mark(open.to, close.from, "cm-link");
    return; // descend: link text may hold emphasis
  }

  if (node.name === "Autolink") {
    const auto = node.node;
    const open = auto.firstChild;
    const close = auto.lastChild;
    if (open?.name !== "LinkMark" || close?.name !== "LinkMark" || open === close) return false;

    b.hide(open.from, open.to);
    b.hide(close.from, close.to);
    b.mark(open.to, close.from, "cm-link");
    return false;
  }
}
