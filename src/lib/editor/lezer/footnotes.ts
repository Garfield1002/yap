import type {
  BlockContext,
  BlockParser,
  InlineContext,
  InlineParser,
  LeafBlock,
  LeafBlockParser,
  MarkdownConfig,
} from "@lezer/markdown";

/**
 * GFM/pandoc footnotes, as a custom `MarkdownConfig`.
 *
 *   FootnoteReference   `[^label]`      inline
 *   FootnoteDefinition  `[^label]: …`   block, single line in v1
 *
 * Both carry `FootnoteMark` children so the decoration layer can hide the
 * reference's brackets (superscripting the label) while merely dimming the
 * definition's marker.
 */

const OPEN_BRACKET = 91; // [
const CARET = 94; // ^
const CLOSE_BRACKET = 93; // ]

const DEFINITION = /^\[\^[^\]\s]+\]:/;

function isSpaceCode(code: number): boolean {
  return code === 32 || code === 9 || code === 10 || code === 13;
}

/**
 * `[^label]`. Installed `before: "Link"` because the standard link parser also
 * opens on `[` and would otherwise swallow the reference. Labels hold no
 * whitespace or nested brackets, and must be non-empty.
 */
const footnoteReference: InlineParser = {
  name: "FootnoteReference",
  before: "Link",
  parse(cx: InlineContext, next: number, pos: number): number {
    if (next !== OPEN_BRACKET || cx.char(pos + 1) !== CARET) return -1;

    let i = pos + 2;
    for (; i < cx.end; i++) {
      const c = cx.char(i);
      if (c === CLOSE_BRACKET) break;
      if (c === OPEN_BRACKET || isSpaceCode(c)) return -1;
    }
    if (i >= cx.end || cx.char(i) !== CLOSE_BRACKET || i === pos + 2) return -1;

    return cx.addElement(
      cx.elt("FootnoteReference", pos, i + 1, [
        cx.elt("FootnoteMark", pos, pos + 2),
        cx.elt("FootnoteMark", i, i + 1),
      ]),
    );
  },
};

/**
 * `[^label]: text`. A leaf-block parser modelled on `LinkReference`: the body
 * is inline-parsed, so links and emphasis inside a footnote render. The `[^…]:`
 * marker becomes a `FootnoteMark` the decoration layer dims rather than hides,
 * keeping the definition legible as a footnote.
 */
class FootnoteDefinitionParser implements LeafBlockParser {
  /** Absorb continuation lines like a paragraph; finish on blank line. */
  nextLine(): boolean {
    return false;
  }

  finish(cx: BlockContext, leaf: LeafBlock): boolean {
    const match = DEFINITION.exec(leaf.content);
    if (!match) return false;

    const from = leaf.start;
    const markerEnd = from + match[0].length;
    cx.addLeafElement(
      leaf,
      cx.elt("FootnoteDefinition", from, from + leaf.content.length, [
        cx.elt("FootnoteMark", from, markerEnd),
        ...cx.parser.parseInline(leaf.content.slice(match[0].length), markerEnd),
      ]),
    );
    return true;
  }
}

const footnoteDefinition: BlockParser = {
  name: "FootnoteDefinition",
  before: "LinkReference",
  leaf(_cx: BlockContext, leaf: LeafBlock): LeafBlockParser | null {
    return DEFINITION.test(leaf.content) ? new FootnoteDefinitionParser() : null;
  },
};

export const footnoteExtension: MarkdownConfig = {
  defineNodes: [
    "FootnoteReference",
    { name: "FootnoteDefinition", block: true },
    "FootnoteMark",
  ],
  parseInline: [footnoteReference],
  parseBlock: [footnoteDefinition],
};
