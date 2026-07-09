import { describe, expect, it } from "vitest";
import { EditorState, EditorSelection } from "@codemirror/state";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { ensureSyntaxTree } from "@codemirror/language";
import { activeRegions, mergeRegions } from "./activeRegions";

/** Build a parsed state. `ranges` are [anchor, head] pairs or bare positions. */
function mkState(doc: string, ...ranges: (number | [number, number])[]) {
  const selection = ranges.length
    ? EditorSelection.create(
        ranges.map((r) =>
          typeof r === "number"
            ? EditorSelection.cursor(r)
            : EditorSelection.range(r[0], r[1]),
        ),
      )
    : undefined;
  const state = EditorState.create({
    doc,
    selection,
    extensions: [
      markdown({ base: markdownLanguage }),
      EditorState.allowMultipleSelections.of(true),
    ],
  });
  // activeRegions reads the cached tree; force a complete parse first.
  ensureSyntaxTree(state, doc.length, 5000);
  return state;
}

/** The raw text each region covers, for legible assertions. */
function revealed(doc: string, ...ranges: (number | [number, number])[]) {
  const state = mkState(doc, ...ranges);
  return activeRegions(state).map((r) => state.doc.sliceString(r.from, r.to));
}

const at = (doc: string, needle: string) => doc.indexOf(needle);

describe("activeRegions", () => {
  it("reveals only the paragraph under the cursor", () => {
    const doc = "first para\n\nsecond para\n\nthird para";
    expect(revealed(doc, at(doc, "second"))).toEqual(["second para"]);
  });

  it("reveals a heading line", () => {
    const doc = "# Title\n\nbody text";
    expect(revealed(doc, at(doc, "Title"))).toEqual(["# Title"]);
  });

  it("reveals a whole fenced code block from any line inside it", () => {
    const doc = "para\n\n```rust\nfn main() {}\n```\n\nafter";
    expect(revealed(doc, at(doc, "fn main"))).toEqual(["```rust\nfn main() {}\n```"]);
  });

  it("reveals the whole blockquote, not one of its lines", () => {
    const doc = "before\n\n> quoted one\n> quoted two\n\nafter";
    expect(revealed(doc, at(doc, "quoted two"))).toEqual(["> quoted one\n> quoted two"]);
  });

  it("reveals the outermost container: a fence nested in a quote", () => {
    const doc = "> intro\n>\n> ```js\n> let x = 1\n> ```\n\nafter";
    expect(revealed(doc, at(doc, "let x"))).toEqual([
      "> intro\n>\n> ```js\n> let x = 1\n> ```",
    ]);
  });

  it("reveals a list item's own line, not the whole list", () => {
    const doc = "- alpha\n- beta\n- gamma";
    expect(revealed(doc, at(doc, "beta"))).toEqual(["- beta"]);
  });

  it("reveals a nested list item's own line", () => {
    const doc = "- alpha\n  - nested one\n  - nested two\n- omega";
    expect(revealed(doc, at(doc, "nested two"))).toEqual(["  - nested two"]);
  });

  it("falls back to the line when the cursor is on a blank line", () => {
    const doc = "para one\n\npara two";
    expect(revealed(doc, at(doc, "\n\n") + 1)).toEqual([""]);
  });

  it("reveals the block at each end of the document", () => {
    const doc = "# Top\n\nmiddle\n\nlast one";
    expect(revealed(doc, 0)).toEqual(["# Top"]);
    expect(revealed(doc, doc.length)).toEqual(["last one"]);
  });

  it("reveals every block a selection touches", () => {
    const doc = "# Head\n\nmiddle para\n\ntail para";
    const from = at(doc, "Head");
    const to = at(doc, "tail") + 2;
    expect(revealed(doc, [from, to])).toEqual([doc]);
  });

  it("does not reveal a block the selection merely ends at the start of", () => {
    const doc = "first\n\nsecond";
    const to = at(doc, "second");
    expect(revealed(doc, [0, to])).toEqual(["first\n"]);
  });

  it("handles multiple cursors in separate blocks", () => {
    const doc = "alpha\n\nbeta\n\ngamma";
    expect(revealed(doc, at(doc, "alpha"), at(doc, "gamma"))).toEqual(["alpha", "gamma"]);
  });

  it("merges two cursors that land in the same block", () => {
    const doc = "alpha beta\n\ngamma";
    expect(revealed(doc, at(doc, "alpha"), at(doc, "beta"))).toEqual(["alpha beta"]);
  });

  it("reveals a whole table as one opaque block", () => {
    const doc = "text\n\n| a | b |\n| - | - |\n| 1 | 2 |\n\nafter";
    expect(revealed(doc, at(doc, "| 1"))).toEqual(["| a | b |\n| - | - |\n| 1 | 2 |"]);
  });
});

describe("mergeRegions", () => {
  it("coalesces overlapping and touching regions", () => {
    expect(mergeRegions([{ from: 5, to: 9 }, { from: 0, to: 5 }, { from: 20, to: 25 }])).toEqual([
      { from: 0, to: 9 },
      { from: 20, to: 25 },
    ]);
  });

  it("leaves disjoint regions alone", () => {
    const input = [{ from: 0, to: 3 }, { from: 10, to: 12 }];
    expect(mergeRegions(input)).toEqual(input);
  });
});
