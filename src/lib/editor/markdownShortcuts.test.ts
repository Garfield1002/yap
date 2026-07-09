import { describe, expect, it } from "vitest";
import { EditorState, EditorSelection, type StateCommand } from "@codemirror/state";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { ensureSyntaxTree } from "@codemirror/language";
import {
  toggleBold,
  toggleItalic,
  insertLink,
  indentListOrTab,
} from "./markdownShortcuts";

/** Run a command against a doc + selection, returning the doc and cursor after. */
function run(
  command: StateCommand,
  doc: string,
  anchor: number,
  head = anchor,
): { doc: string; from: number; to: number; handled: boolean } {
  const state = EditorState.create({
    doc,
    selection: EditorSelection.range(anchor, head),
    extensions: [markdown({ base: markdownLanguage })],
  });
  // Some commands consult the syntax tree; make sure it is fully built.
  let tree = ensureSyntaxTree(state, doc.length, 5000);
  while (!tree || tree.length < doc.length) tree = ensureSyntaxTree(state, doc.length, 5000);

  let next = state;
  const handled = command({ state, dispatch: (tr) => (next = tr.state) });
  const sel = next.selection.main;
  return { doc: next.doc.toString(), from: sel.from, to: sel.to, handled };
}

describe("toggleBold", () => {
  it("wraps a selection in **", () => {
    const r = run(toggleBold, "make me bold", 8, 12); // "bold"
    expect(r.doc).toBe("make me **bold**");
    expect([r.from, r.to]).toEqual([10, 14]); // still selecting "bold"
  });

  it("unwraps when already bold", () => {
    const r = run(toggleBold, "a **bold** word", 4, 8); // inner "bold"
    expect(r.doc).toBe("a bold word");
    expect([r.from, r.to]).toEqual([2, 6]);
  });

  it("inserts an empty pair and parks the cursor between the halves", () => {
    const r = run(toggleBold, "x ", 2);
    expect(r.doc).toBe("x ****");
    expect([r.from, r.to]).toEqual([4, 4]);
  });
});

describe("toggleItalic", () => {
  it("wraps in a single *", () => {
    const r = run(toggleItalic, "make me em", 8, 10);
    expect(r.doc).toBe("make me *em*");
  });

  it("unwraps an italic selection", () => {
    const r = run(toggleItalic, "a *em* b", 3, 5);
    expect(r.doc).toBe("a em b");
  });
});

describe("insertLink", () => {
  it("wraps the selection and lands the cursor in the parens", () => {
    const r = run(insertLink, "see docs here", 4, 8); // "docs"
    expect(r.doc).toBe("see [docs]() here");
    expect([r.from, r.to]).toEqual([11, 11]); // between ( and )
  });

  it("inserts an empty link at the cursor", () => {
    const r = run(insertLink, "x ", 2);
    expect(r.doc).toBe("x []()");
    expect(r.from).toBe(5);
  });
});

describe("indentListOrTab", () => {
  it("nests a list item", () => {
    const r = run(indentListOrTab, "- item", 6);
    expect(r.doc).toBe("  - item");
  });

  it("inserts indentation outside a list", () => {
    const r = run(indentListOrTab, "plain", 5);
    expect(r.doc.startsWith("plain")).toBe(true);
    expect(r.doc.length).toBeGreaterThan(5); // some whitespace was inserted
  });
});
