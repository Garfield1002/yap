import type { SyntaxNodeRef } from "@lezer/common";
import type { Builder } from "../builder";

const ATX: Record<string, string> = {
  ATXHeading1: "cm-h1",
  ATXHeading2: "cm-h2",
  ATXHeading3: "cm-h3",
  ATXHeading4: "cm-h4",
  ATXHeading5: "cm-h5",
  ATXHeading6: "cm-h6",
};

const SETEXT: Record<string, string> = {
  SetextHeading1: "cm-h1",
  SetextHeading2: "cm-h2",
};

/**
 * Font size rides on a line decoration, not on the text, so hiding or showing
 * the `#` markers never changes the line box.
 */
export function headings(node: SyntaxNodeRef, b: Builder): boolean | void {
  const atx = ATX[node.name];
  if (atx) {
    b.line(node.from, atx);

    const doc = b.state.doc;
    const first = node.node.firstChild;
    if (first?.name === "HeaderMark" && first.from === node.from) {
      let to = first.to;
      if (doc.sliceString(to, to + 1) === " ") to += 1;
      b.hide(node.from, to);
    }

    // Optional closing run: `## Title ##`
    const last = node.node.lastChild;
    if (last && last !== first && last.name === "HeaderMark" && last.to === node.to) {
      let from = last.from;
      while (from > node.from && doc.sliceString(from - 1, from) === " ") from -= 1;
      b.hide(from, node.to);
    }
    return; // descend: the heading text may contain inline markup
  }

  const setext = SETEXT[node.name];
  if (setext) {
    b.line(node.from, setext);
    // The `===` underline is dimmed rather than hidden: collapsing it would
    // delete a whole line from the layout every time the cursor leaves.
    const last = node.node.lastChild;
    if (last?.name === "HeaderMark") b.mark(last.from, last.to, "cm-md-mark");
    return;
  }
}
