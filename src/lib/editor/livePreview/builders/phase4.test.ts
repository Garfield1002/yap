import { describe, expect, it, vi } from "vitest";
import { EditorState, EditorSelection } from "@codemirror/state";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { ensureSyntaxTree, syntaxTree } from "@codemirror/language";

// KaTeX is lazy-loaded inside `toDOM`, which never runs in these DOM-free
// tests, so the module is never actually imported here. The stub is a belt-and-
// suspenders guard in case that ever changes; the decoration layer only ever
// constructs the widget. Mirrors phase3's tauri stub.
vi.mock("katex", () => ({ default: { renderToString: () => "" } }));

const { yapLezerExtensions } = await import("../../lezer");
const { buildDecorations } = await import("../buildDecorations");
const { MathWidget } = await import("../widgets/MathWidget");

function mkState(doc: string, cursor: number) {
  const state = EditorState.create({
    doc,
    selection: EditorSelection.cursor(Math.min(cursor, doc.length)),
    extensions: [markdown({ base: markdownLanguage, extensions: yapLezerExtensions })],
  });
  // Loop until the parse really completes: `ensureSyntaxTree` returns null when
  // it runs out of budget, which under the parallel suite's CPU load happens
  // even for a tiny doc.
  let tree = ensureSyntaxTree(state, doc.length, 5000);
  while (!tree || tree.length < doc.length) tree = ensureSyntaxTree(state, doc.length, 5000);
  return state;
}

/** A compact "Name(from,to)" dump of the parse tree, for adversarial fixtures. */
function treeShape(doc: string): string[] {
  const state = mkState(doc, 0);
  const out: string[] = [];
  syntaxTree(state).iterate({
    enter: (n) => {
      if (n.name !== "Document") out.push(`${n.name}(${n.from},${n.to})`);
    },
  });
  return out;
}

/** Node names present anywhere in the tree. */
function nodes(doc: string): Set<string> {
  return new Set(treeShape(doc).map((s) => s.replace(/\(.*/, "")));
}

interface Entry {
  kind: "hidden" | "widget" | "mark" | "line";
  text: string;
  class?: string;
  widget?: unknown;
}

function entries(doc: string, cursor: number): Entry[] {
  const state = mkState(doc, cursor);
  const { deco } = buildDecorations(state);
  const out: Entry[] = [];
  const iter = deco.iter();
  while (iter.value) {
    const spec = iter.value.spec as { class?: string; widget?: unknown };
    const kind: Entry["kind"] = spec.widget
      ? "widget"
      : iter.from === iter.to
        ? "line"
        : spec.class
          ? "mark"
          : "hidden";
    out.push({
      kind,
      text: state.doc.sliceString(iter.from, iter.to),
      ...(spec.class ? { class: spec.class } : {}),
      ...(spec.widget ? { widget: spec.widget } : {}),
    });
    iter.next();
  }
  return out;
}

const hidden = (doc: string, cursor: number) =>
  entries(doc, cursor).filter((e) => e.kind === "hidden").map((e) => e.text);

const classes = (doc: string, cursor: number) =>
  entries(doc, cursor).filter((e) => e.class).map((e) => `${e.class}:${e.text}`);

describe("inline math parsing", () => {
  it("recognises a simple `$x$`", () => {
    expect(nodes("a $x+1$ b")).toContain("InlineMath");
  });

  it("does not treat a lone `$` as math", () => {
    expect(nodes("it cost $5 yesterday")).not.toContain("InlineMath");
  });

  it("rejects whitespace hugging a delimiter", () => {
    expect(nodes("a $ x$ b")).not.toContain("InlineMath");
    expect(nodes("a $x $ b")).not.toContain("InlineMath");
  });

  it("does not open on `$$`", () => {
    expect(nodes("a $$ inline")).not.toContain("InlineMath");
  });

  it("does not cross a line break", () => {
    expect(nodes("a $x\ny$ b")).not.toContain("InlineMath");
  });

  it("honours an escaped dollar as the closer", () => {
    // The first real `$` closes at the second real `$`, past the escaped one.
    expect(nodes("a $x \\$ y$ b")).toContain("InlineMath");
  });
});

describe("block math parsing", () => {
  it("parses a single-line `$$…$$`", () => {
    expect(nodes("$$E = mc^2$$")).toContain("BlockMath");
  });

  it("parses a fenced block across lines", () => {
    expect(nodes("$$\n\\int_0^1 x\\,dx\n$$")).toContain("BlockMath");
  });

  it("runs an unterminated block to the end of the document", () => {
    const shape = treeShape("$$\nx = 1\n\nstill math");
    const block = shape.find((s) => s.startsWith("BlockMath"));
    expect(block).toBe(`BlockMath(0,20)`);
  });
});

describe("footnote parsing", () => {
  it("recognises a reference", () => {
    expect(nodes("see this[^1] here")).toContain("FootnoteReference");
  });

  it("does not fight the link parser for `[text](url)`", () => {
    const n = nodes("a [text](http://x.dev) b");
    expect(n).toContain("Link");
    expect(n).not.toContain("FootnoteReference");
  });

  it("rejects an empty or spaced label", () => {
    expect(nodes("a [^] b")).not.toContain("FootnoteReference");
    expect(nodes("a [^a b] c")).not.toContain("FootnoteReference");
  });

  it("recognises a definition and inline-parses its body", () => {
    const n = nodes("[^1]: see [x](http://y.dev)");
    expect(n).toContain("FootnoteDefinition");
    expect(n).toContain("Link");
  });
});

describe("math decorations", () => {
  it("replaces inline math with a widget", () => {
    const doc = "a $x+1$ b\n\nelsewhere";
    const w = entries(doc, doc.length).find((e) => e.kind === "widget");
    expect(w?.text).toBe("$x+1$");
    expect(w?.widget).toBeInstanceOf(MathWidget);
    expect((w?.widget as InstanceType<typeof MathWidget>).tex).toBe("x+1");
    expect((w?.widget as InstanceType<typeof MathWidget>).displayMode).toBe(false);
  });

  it("replaces block math with a display-mode widget", () => {
    const doc = "para\n\n$$\ny = x\n$$";
    const w = entries(doc, 0).find((e) => e.kind === "widget");
    expect((w?.widget as InstanceType<typeof MathWidget>).tex).toBe("y = x");
    expect((w?.widget as InstanceType<typeof MathWidget>).displayMode).toBe(true);
  });

  it("shows raw source when the cursor is inside the math block", () => {
    const doc = "$$\ny = x\n$$";
    expect(entries(doc, 3).filter((e) => e.kind === "widget")).toEqual([]);
  });

  it("boxes the raw source like a code block while editing", () => {
    const doc = "$$\ny = x\n$$";
    const found = classes(doc, 3); // cursor on the content line
    expect(found.filter((c) => c.startsWith("cm-block-line")).length).toBe(3);
    expect(found).toContain("cm-block-first:");
    expect(found).toContain("cm-block-last:");
    // The `$$` fences are dimmed, not hidden.
    expect(found.filter((c) => c === "cm-md-mark:$$").length).toBe(2);
    expect(hidden(doc, 3)).toEqual([]);
  });
});

describe("footnote decorations", () => {
  it("hides the brackets and superscripts the label", () => {
    const doc = "text[^note] more\n\nelsewhere";
    expect(hidden(doc, doc.length)).toEqual(["[^", "]"]);
    expect(classes(doc, doc.length)).toContain("cm-footnote-ref:note");
  });

  it("dims the definition marker and flags the line", () => {
    const doc = "before\n\n[^note]: the text";
    const found = classes(doc, 0);
    expect(found).toContain("cm-md-mark:[^note]:");
    expect(found).toContain("cm-footnote-def:");
  });

  it("shows raw reference source when the cursor is in its block", () => {
    const doc = "text[^note] more";
    expect(hidden(doc, 2)).toEqual([]);
  });
});

describe("adversarial fixtures build without throwing", () => {
  const gauntlet = [
    "inline $a_b$ and block:\n\n$$\n\\frac{1}{2}\n$$",
    "a footnote[^1] and math $x$ together[^2]",
    "[^1]: definition with $inline math$ inside",
    "money $5 and $10 but not math",
    "nested [**bold [^fn]**](http://x.dev) $y$",
    "- [ ] task with $math$ and [^fn]",
    "> quote with $x$ and [^fn] inside",
    "$$ unterminated block\n\nnext para $z$",
    "table | $x$ | [^fn] |\n| - | - |\n| a | b |",
  ];

  it.each(gauntlet)("%j", (doc) => {
    expect(() => entries(doc, 0)).not.toThrow();
    expect(() => entries(doc, doc.length)).not.toThrow();
  });
});
