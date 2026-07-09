import type {
  BlockContext,
  BlockParser,
  InlineContext,
  InlineParser,
  Line,
  MarkdownConfig,
} from "@lezer/markdown";

/**
 * LaTeX math, in two flavours, as a custom `MarkdownConfig`. No maintained
 * Lezer markdown math extension exists, so both parsers are hand-written and
 * modelled on `@lezer/markdown`'s own `InlineCode` and `FencedCode`.
 *
 *   InlineMath  `$x$`      -- single line, whitespace- and `$$`-averse
 *   BlockMath   `$$…$$`    -- one line or fenced; unterminated runs to EOF
 *
 * Both wrap the whole span in one node with `MathMark` children for the `$`
 * delimiters. The decoration layer replaces the entire node with a KaTeX
 * widget, so the tree only needs to locate the tex, not tokenize inside it.
 */

const DOLLAR = 36; // $
const BACKSLASH = 92; // \

function isSpaceCode(code: number): boolean {
  return code === 32 || code === 9 || code === 10 || code === 13;
}

/**
 * `$x$`. Rejected when it opens `$$` (that is block math), when a delimiter
 * touches whitespace (so a lone `$` in prose stays literal), or when the run
 * crosses a line break. A `\$` is skipped: the default Escape parser runs first
 * and never offers us an escaped opener, and this loop honours the same rule
 * for the closer.
 */
const mathInline: InlineParser = {
  name: "InlineMath",
  before: "Emphasis",
  parse(cx: InlineContext, next: number, pos: number): number {
    if (next !== DOLLAR) return -1;
    // `$$` is a block delimiter, and an empty `$$` has nothing to render.
    if (cx.char(pos + 1) === DOLLAR) return -1;
    // Opener must hug its content: `$ x$` is not math.
    if (pos + 1 >= cx.end || isSpaceCode(cx.char(pos + 1))) return -1;

    for (let i = pos + 1; i < cx.end; i++) {
      const c = cx.char(i);
      if (c === BACKSLASH) {
        i++; // the escaped character cannot be a delimiter
        continue;
      }
      if (c === 10) break; // newline: inline math is single-line
      if (c === DOLLAR) {
        // Closer must hug its content too, and not itself be a `$$`.
        if (isSpaceCode(cx.char(i - 1))) continue;
        if (cx.char(i + 1) === DOLLAR) continue;
        return cx.addElement(
          cx.elt("InlineMath", pos, i + 1, [
            cx.elt("MathMark", pos, pos + 1),
            cx.elt("MathMark", i, i + 1),
          ]),
        );
      }
    }
    return -1;
  },
};

/** Count the leading `$` run at `from`, capped at what the line holds. */
function dollarRun(text: string, from: number): number {
  let i = from;
  while (i < text.length && text.charCodeAt(i) === DOLLAR) i++;
  return i - from;
}

/**
 * `$$…$$` as a block. Two shapes:
 *
 *   $$ x $$              on one line
 *   $$\n x \n$$          fenced across lines
 *
 * An opener with no closing fence runs to the end of the document, exactly like
 * an unterminated ``` ``` `` block. Only fires at the start of a block, so a
 * `$$` mid-paragraph is left to inline handling.
 */
const mathBlock: BlockParser = {
  name: "BlockMath",
  before: "FencedCode",
  parse(cx: BlockContext, line: Line): boolean {
    if (line.next !== DOLLAR || dollarRun(line.text, line.pos) < 2) return false;

    const from = cx.lineStart + line.pos;
    const rest = line.text.slice(line.pos + 2);
    const trimmed = rest.replace(/\s+$/, "");

    // Single line: `$$ … $$` with a closing pair of its own.
    if (trimmed.length >= 2 && trimmed.endsWith("$$")) {
      const closeFrom = cx.lineStart + line.pos + 2 + trimmed.length - 2;
      cx.addElement(
        cx.elt("BlockMath", from, closeFrom + 2, [
          cx.elt("MathMark", from, from + 2),
          cx.elt("MathMark", closeFrom, closeFrom + 2),
        ]),
      );
      cx.nextLine();
      return true;
    }

    // Otherwise the opener must stand alone on its line.
    if (trimmed.length !== 0) return false;

    const marks = [cx.elt("MathMark", from, from + 2)];
    while (cx.nextLine()) {
      const closeLen = dollarRun(line.text, line.pos);
      if (closeLen >= 2 && line.skipSpace(line.pos + closeLen) === line.text.length) {
        const closeFrom = cx.lineStart + line.pos;
        marks.push(cx.elt("MathMark", closeFrom, closeFrom + closeLen));
        cx.nextLine();
        break;
      }
    }
    cx.addElement(cx.elt("BlockMath", from, cx.prevLineEnd(), marks));
    return true;
  },
};

export const mathExtension: MarkdownConfig = {
  defineNodes: [
    { name: "InlineMath" },
    { name: "BlockMath", block: true },
    "MathMark",
  ],
  parseInline: [mathInline],
  parseBlock: [mathBlock],
};
