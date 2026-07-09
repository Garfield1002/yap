import { describe, expect, it, vi } from "vitest";
import { EditorState, EditorSelection } from "@codemirror/state";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { ensureSyntaxTree } from "@codemirror/language";

// The image widget resolves paths through Tauri at render time; the decoration
// layer only ever constructs it, so a stub keeps these tests in plain node.
vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (path: string) => `asset://localhost/${path}`,
}));

const { buildDecorations } = await import("../buildDecorations");
const { documentDirectory } = await import("../context");
const { ImageWidget } = await import("../widgets/ImageWidget");
const { CheckboxWidget } = await import("../widgets/CheckboxWidget");

function mkState(doc: string, cursor: number, documentDir = "/home/j/notes") {
  const state = EditorState.create({
    doc,
    selection: EditorSelection.cursor(Math.min(cursor, doc.length)),
    extensions: [markdown({ base: markdownLanguage }), documentDirectory.of(documentDir)],
  });
  // Loop until the parse really completes: `ensureSyntaxTree` returns null when
  // it runs out of budget, which under the parallel suite's CPU load happens
  // even for a tiny doc.
  let tree = ensureSyntaxTree(state, doc.length, 5000);
  while (!tree || tree.length < doc.length) tree = ensureSyntaxTree(state, doc.length, 5000);
  return state;
}

interface Entry {
  kind: "hidden" | "widget" | "mark" | "line";
  text: string;
  class?: string;
  widget?: unknown;
}

function entries(doc: string, cursor: number, documentDir?: string): Entry[] {
  const state = mkState(doc, cursor, documentDir);
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

describe("links", () => {
  it("hides the brackets and the whole url tail", () => {
    const doc = "see [text](http://x.dev) end\n\nelsewhere";
    expect(hidden(doc, doc.length)).toEqual(["[", "](http://x.dev)"]);
  });

  it("hides a link title along with the url", () => {
    const doc = 'see [text](http://x.dev "why") end\n\nelsewhere';
    expect(hidden(doc, doc.length)).toEqual(["[", '](http://x.dev "why")']);
  });

  it("marks the link text", () => {
    const doc = "see [text](http://x.dev)\n\nelsewhere";
    expect(classes(doc, doc.length)).toContain("cm-link:text");
  });

  it("still renders emphasis inside link text", () => {
    const doc = "see [**bold** it](http://x.dev)\n\nelsewhere";
    expect(hidden(doc, doc.length)).toEqual(["[", "**", "**", "](http://x.dev)"]);
  });

  it("collapses the angle brackets of an autolink", () => {
    const doc = "see <http://auto.dev> end\n\nelsewhere";
    expect(hidden(doc, doc.length)).toEqual(["<", ">"]);
    expect(classes(doc, doc.length)).toContain("cm-link:http://auto.dev");
  });

  it("shows the raw source when the cursor is in the link's block", () => {
    const doc = "see [text](http://x.dev)";
    expect(hidden(doc, 2)).toEqual([]);
    expect(classes(doc, 2)).toContain("cm-link:text");
  });
});

describe("images", () => {
  it("replaces the whole node with a widget", () => {
    const doc = "before\n\n![alt](pic.png)";
    const image = entries(doc, 0).find((e) => e.kind === "widget");
    expect(image?.text).toBe("![alt](pic.png)");
    expect(image?.widget).toBeInstanceOf(ImageWidget);
  });

  it("extracts alt text from between the brackets, not the parens", () => {
    const doc = "before\n\n![the alt](pic.png)";
    const widget = entries(doc, 0).find((e) => e.kind === "widget")?.widget as InstanceType<
      typeof ImageWidget
    >;
    expect(widget.alt).toBe("the alt");
    expect(widget.src).toBe("pic.png");
  });

  it("emits nothing inside the replaced range", () => {
    // Emphasis markers in the alt text would partially overlap the replace.
    const doc = "before\n\n![*em* alt](pic.png)";
    expect(entries(doc, 0).filter((e) => e.kind === "hidden")).toEqual([]);
  });

  it("shows raw source when the cursor is inside the image", () => {
    const doc = "![alt](pic.png)";
    expect(entries(doc, 3).filter((e) => e.kind === "widget")).toEqual([]);
  });

  it("carries the document directory so relative paths can resolve", () => {
    const doc = "before\n\n![a](sub/pic.png)";
    const widget = entries(doc, 0, "/tmp/notes").find((e) => e.kind === "widget")
      ?.widget as InstanceType<typeof ImageWidget>;
    expect(widget.documentDir).toBe("/tmp/notes");
  });
});

describe("task list", () => {
  it("replaces the marker with an unchecked box", () => {
    const doc = "- [ ] todo\n\nelsewhere";
    const widget = entries(doc, doc.length).find((e) => e.kind === "widget");
    expect(widget?.text).toBe("[ ]");
    expect(widget?.widget).toBeInstanceOf(CheckboxWidget);
    expect((widget?.widget as InstanceType<typeof CheckboxWidget>).checked).toBe(false);
  });

  it("recognises a checked marker and strikes the line", () => {
    const doc = "- [x] done\n\nelsewhere";
    const list = entries(doc, doc.length);
    expect((list.find((e) => e.kind === "widget")?.widget as InstanceType<typeof CheckboxWidget>).checked).toBe(true);
    expect(classes(doc, doc.length)).toContain("cm-task-done:");
  });

  it("accepts an uppercase X", () => {
    const doc = "- [X] done\n\nelsewhere";
    const widget = entries(doc, doc.length).find((e) => e.kind === "widget");
    expect((widget?.widget as InstanceType<typeof CheckboxWidget>).checked).toBe(true);
  });

  it("still renders emphasis in the task text", () => {
    const doc = "- [ ] a **b**\n\nelsewhere";
    expect(hidden(doc, doc.length)).toEqual(["**", "**"]);
  });
});

describe("code blocks", () => {
  it("collapses the fences and slabs only the body when rendered", () => {
    const doc = "para\n\n```rust\nlet x = 1;\n```";
    const found = classes(doc, 0); // cursor outside the block
    // Only the single body line carries the slab, with both rounded corners.
    expect(found.filter((c) => c.startsWith("cm-code-line")).length).toBe(1);
    expect(found).toContain("cm-code-first:");
    expect(found).toContain("cm-code-last:");
    // The ``` and info string are gone, not merely dimmed.
    expect(found.some((c) => c.startsWith("cm-md-mark"))).toBe(false);
    // Each fence is hidden together with the newline that precedes it, so the
    // empty line folds up into the line above rather than into the body.
    expect(hidden(doc, 0)).toEqual(["\n```rust", "\n```"]);
  });

  it("reveals and dims the fences while the block is edited", () => {
    const doc = "para\n\n```rust\nlet x = 1;\n```";
    const found = classes(doc, 16); // cursor on the body line
    expect(found.filter((c) => c.startsWith("cm-code-line")).length).toBe(3);
    expect(found).toContain("cm-md-mark:```");
    expect(found).toContain("cm-md-mark:rust");
    expect(hidden(doc, 16)).toEqual([]);
  });

  it("keeps a bodyless fence visible rather than collapsing to nothing", () => {
    const doc = "para\n\n```\n```";
    expect(hidden(doc, 0)).toEqual([]);
  });

  it("styles an indented code block too", () => {
    const doc = "para\n\n    indented\n";
    expect(classes(doc, 0)).toContain("cm-code-line:");
  });
});

describe("no overlapping replaces", () => {
  const gauntlet = [
    "- [ ] see [**bold** link](http://x.dev) and ![a](p.png)",
    "> quote with [link](http://x.dev)",
    "# heading with [link](http://x.dev) and `code`",
    "- item\n  - [x] ![img](p.png) nested",
    "[![image link](p.png)](http://x.dev)",
    "| a | [b](http://x.dev) |\n| - | - |\n| 1 | 2 |",
  ];

  it.each(gauntlet)("builds without throwing: %j", (doc) => {
    // A replace that partially overlaps another makes Decoration.set throw.
    expect(() => entries(doc, 0)).not.toThrow();
    expect(() => entries(doc, doc.length)).not.toThrow();
  });
});
