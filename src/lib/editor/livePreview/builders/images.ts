import type { SyntaxNodeRef } from "@lezer/common";
import type { Builder } from "../builder";
import { documentDirectory } from "../context";
import { ImageWidget, renderedImageHeight } from "../widgets/ImageWidget";

/**
 * `![alt](src)` becomes the picture.
 *
 * The whole node is replaced, so nothing may descend into it: a decoration
 * emitted inside a replaced range partially overlaps it, and CodeMirror throws.
 */
export function images(node: SyntaxNodeRef, b: Builder): boolean | void {
  if (node.name !== "Image") return;

  const image = node.node;
  const urlNode = image.getChild("URL");
  if (!urlNode) return false;

  const src = b.state.doc.sliceString(urlNode.from, urlNode.to);

  // Children run `![`, `]`, `(`, URL, `)`. The alt text sits between the first
  // two, not between the first and the last.
  const open = image.firstChild;
  const altEnd = image
    .getChildren("LinkMark")
    .find((m) => m !== open && b.state.doc.sliceString(m.from, m.to) === "]");
  const alt = open && altEnd ? b.state.doc.sliceString(open.to, altEnd.from) : "";

  const directory = b.state.facet(documentDirectory);
  if (b.isRaw(image.from, image.to)) {
    const reserved = renderedImageHeight(src, directory);
    if (reserved) {
      b.lineAttributes(image.from, {
        class: "cm-image-source-active",
        style: `min-height: ${reserved}px; box-sizing: border-box;`,
      });
    }
    return false;
  }

  b.replace(image.from, image.to, new ImageWidget(src, alt, directory));
  return false;
}
