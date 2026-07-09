import { describe, expect, it } from "vitest";
import { EditorState, EditorSelection } from "@codemirror/state";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { ensureSyntaxTree } from "@codemirror/language";
import { buildDecorations } from "./buildDecorations";

function mkState(doc: string, cursor = 0) {
  const state = EditorState.create({
    doc,
    selection: EditorSelection.cursor(Math.min(cursor, doc.length)),
    extensions: [markdown({ base: markdownLanguage })],
  });
  ensureSyntaxTree(state, doc.length, 5000);
  return state;
}

interface Snapshot {
  kind: "hidden" | "mark" | "line";
  text: string;
  class?: string;
}

/** Flatten a decoration set into something readable and order-stable. */
function snapshot(doc: string, cursor = 0): Snapshot[] {
  const state = mkState(doc, cursor);
  const { deco } = buildDecorations(state);
  const out: Snapshot[] = [];
  const iter = deco.iter();
  while (iter.value) {
    const spec = iter.value.spec as { class?: string; widget?: unknown };
    const kind: Snapshot["kind"] =
      iter.value.spec.widget || (iter.from !== iter.to && !spec.class)
        ? "hidden"
        : iter.from === iter.to
          ? "line"
          : "mark";
    out.push({
      kind,
      text: state.doc.sliceString(iter.from, iter.to),
      ...(spec.class ? { class: spec.class } : {}),
    });
    iter.next();
  }
  return out;
}

/** The atomic set must never contain a mark decoration. */
function atomicTexts(doc: string, cursor = 0): string[] {
  const state = mkState(doc, cursor);
  const { atomic } = buildDecorations(state);
  const out: string[] = [];
  const iter = atomic.iter();
  while (iter.value) {
    expect(iter.value.spec.class, "atomic set must hold only replaces").toBeUndefined();
    out.push(state.doc.sliceString(iter.from, iter.to));
    iter.next();
  }
  return out;
}

// Cursor parked far from the interesting block so nothing is raw.
const AWAY = 0;

describe("headings", () => {
  it("hides the ATX marker and sizes the line", () => {
    const doc = "para\n\n## Title";
    expect(snapshot(doc, AWAY)).toEqual([
      { kind: "line", text: "", class: "cm-h2" },
      { kind: "hidden", text: "## " },
    ]);
  });

  it("hides a closing ATX run", () => {
    const doc = "para\n\n# Title #";
    expect(snapshot(doc, AWAY)).toEqual([
      { kind: "line", text: "", class: "cm-h1" },
      { kind: "hidden", text: "# " },
      { kind: "hidden", text: " #" },
    ]);
  });

  it("keeps the line class but shows the marker when the cursor is inside", () => {
    const doc = "## Title";
    expect(snapshot(doc, 4)).toEqual([{ kind: "line", text: "", class: "cm-h2" }]);
  });

  it("dims rather than hides a setext underline", () => {
    const doc = "para\n\nTitle\n=====";
    expect(snapshot(doc, AWAY)).toEqual([
      { kind: "line", text: "", class: "cm-h1" },
      { kind: "mark", text: "=====", class: "cm-md-mark" },
    ]);
  });

  it("styles heading text inside a heading", () => {
    const doc = "para\n\n# a **b** c";
    expect(snapshot(doc, AWAY)).toEqual([
      { kind: "line", text: "", class: "cm-h1" },
      { kind: "hidden", text: "# " },
      { kind: "hidden", text: "**" },
      { kind: "hidden", text: "**" },
    ]);
  });
});

describe("inline markup", () => {
  it("hides emphasis, strong and strikethrough markers", () => {
    const doc = "*a* **b** ~~c~~\n\nelsewhere";
    expect(snapshot(doc, doc.length).filter((s) => s.kind === "hidden")).toEqual([
      { kind: "hidden", text: "*" },
      { kind: "hidden", text: "*" },
      { kind: "hidden", text: "**" },
      { kind: "hidden", text: "**" },
      { kind: "hidden", text: "~~" },
      { kind: "hidden", text: "~~" },
    ]);
  });

  it("handles bold nested inside a link inside a list without overlapping replaces", () => {
    const doc = "- see [**bold** link](http://x.dev)\n- other";
    expect(() => snapshot(doc, doc.length)).not.toThrow();
    expect(snapshot(doc, doc.length).filter((s) => s.kind === "hidden")).toEqual([
      { kind: "hidden", text: "[" },
      { kind: "hidden", text: "**" },
      { kind: "hidden", text: "**" },
      { kind: "hidden", text: "](http://x.dev)" },
    ]);
  });

  it("hides inline-code backticks but not fenced-code fences", () => {
    const doc = "use `let x` here\n\n```js\nlet y\n```";
    const hidden = snapshot(doc, doc.length).filter((s) => s.kind === "hidden");
    expect(hidden).toEqual([
      { kind: "hidden", text: "`" },
      { kind: "hidden", text: "`" },
    ]);
  });

  it("marks the inline-code content between the backticks", () => {
    const doc = "use `let x` here";
    expect(snapshot(doc, 100)).toContainEqual({
      kind: "mark",
      text: "let x",
      class: "cm-inline-code",
    });
  });
});

describe("raw regions", () => {
  it("emits no replaces inside the active block", () => {
    const doc = "*a*\n\n*b*";
    expect(snapshot(doc, 1).filter((s) => s.kind === "hidden")).toEqual([
      { kind: "hidden", text: "*" },
      { kind: "hidden", text: "*" },
    ]);
  });

  it("emits nothing at all inside a table", () => {
    const doc = "| **a** | b |\n| ----- | - |\n| 1     | 2 |";
    expect(snapshot(doc, 100)).toEqual([]);
  });
});

describe("atomic set", () => {
  it("contains exactly the hidden markup", () => {
    const doc = "# Title\n\n*em*\n\nelsewhere";
    expect(atomicTexts(doc, doc.length)).toEqual(["# ", "*", "*"]);
  });
});
