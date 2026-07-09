import { describe, expect, it } from "vitest";
import { EditorState, EditorSelection, ChangeSet } from "@codemirror/state";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { ensureSyntaxTree } from "@codemirror/language";
import { livePreviewField, refreshDecorations } from "./decorationField";
import { contains, mapRegions } from "./pinning";

const DOC = ["para **bold**", "", "```", "Code", "```", "", "# Heading after", "", "tail *em*"].join(
  "\n",
);

const at = (needle: string, doc = DOC) => doc.indexOf(needle);

function mkState(doc: string, cursor: number) {
  const state = EditorState.create({
    doc,
    selection: EditorSelection.cursor(cursor),
    extensions: [markdown({ base: markdownLanguage }), livePreviewField],
  });
  // `ensureSyntaxTree` returns null when it cannot finish inside its budget,
  // which under the parallel suite's CPU contention happens even for a tiny
  // doc. Loop until the tree really covers the doc.
  let tree = ensureSyntaxTree(state, doc.length, 5000);
  while (!tree || tree.length < doc.length) tree = ensureSyntaxTree(state, doc.length, 5000);
  // `livePreviewField.create` ran inside `EditorState.create` above, off
  // whatever the incremental 100ms parse in `buildDecorations` had reached --
  // which under load can stop short of the tail. Now that the tree is complete,
  // rebuild the field from it. In the app the `parseTailWatcher` does this; the
  // test has no view, so it does it here.
  return state.update({ effects: refreshDecorations.of(null) }).state;
}

/** All decoration classes present, as `class` or `class:text`. */
function classesOf(state: EditorState): string[] {
  const { deco } = state.field(livePreviewField);
  const out: string[] = [];
  for (const iter = deco.iter(); iter.value; iter.next()) {
    const cls = (iter.value.spec as { class?: string }).class;
    if (cls) out.push(cls);
  }
  return out;
}

function hiddenOf(state: EditorState): string[] {
  const { deco } = state.field(livePreviewField);
  const out: string[] = [];
  for (const iter = deco.iter(); iter.value; iter.next()) {
    const spec = iter.value.spec as { class?: string; widget?: unknown };
    if (!spec.class && !spec.widget && iter.from !== iter.to) {
      out.push(state.doc.sliceString(iter.from, iter.to));
    }
  }
  return out;
}

/** Apply a change, keeping the cursor where the caller says. */
function edit(state: EditorState, changes: { from: number; to?: number; insert?: string }, cursor: number) {
  return state.update({ changes, selection: EditorSelection.cursor(cursor) }).state;
}

describe("mapRegions", () => {
  it("does not grow a region when text is inserted at its edges", () => {
    const changes = ChangeSet.of([{ from: 10, insert: "xx" }], 20);
    // Region [10, 15): insertion sits exactly at `from`.
    expect(mapRegions([{ from: 10, to: 15 }], changes)).toEqual([{ from: 12, to: 17 }]);

    const atEnd = ChangeSet.of([{ from: 15, insert: "yy" }], 20);
    expect(mapRegions([{ from: 10, to: 15 }], atEnd)).toEqual([{ from: 10, to: 15 }]);
  });

  it("grows a region when text is inserted inside it", () => {
    const changes = ChangeSet.of([{ from: 12, insert: "zzz" }], 20);
    expect(mapRegions([{ from: 10, to: 15 }], changes)).toEqual([{ from: 10, to: 18 }]);
  });
});

describe("contains", () => {
  const regions = [{ from: 5, to: 10 }];
  it("includes the endpoints", () => {
    expect(contains(regions, 5, 5)).toBe(true);
    expect(contains(regions, 10, 10)).toBe(true);
  });
  it("excludes anything outside", () => {
    expect(contains(regions, 4, 6)).toBe(false);
    expect(contains(regions, 11, 11)).toBe(false);
  });
});

describe("editing inside a block does not repaint the rest of the document", () => {
  it("keeps the heading below rendered when a fence is broken mid-edit", () => {
    const cursor = at("Code");
    const before = mkState(DOC, cursor);
    expect(classesOf(before)).toContain("cm-h1");

    // Delete one backtick from the *opening* fence. In CommonMark the surviving
    // fence now opens a code block that swallows the rest of the file.
    const openingFence = at("```");
    const after = edit(before, { from: openingFence, to: openingFence + 1 }, cursor - 1);

    const tree = ensureSyntaxTree(after, after.doc.length, 5000)!;
    const swallowed = tree.resolveInner(at("# Heading after") - 1, 1);
    expect(swallowed.name, "the parse really is contaminated").toBe("CodeText");

    // ...but the decorations for the untouched remainder are the old ones.
    expect(classesOf(after), "heading below must stay rendered").toContain("cm-h1");
    expect(hiddenOf(after), "emphasis below must stay collapsed").toContain("*");
  });

  it("still hides markup in the paragraph above the edited block", () => {
    const cursor = at("Code");
    const before = mkState(DOC, cursor);
    const openingFence = at("```");
    const after = edit(before, { from: openingFence, to: openingFence + 1 }, cursor - 1);
    expect(hiddenOf(after)).toContain("**");
  });

  it("rebuilds from the real tree once the cursor leaves the block", () => {
    const cursor = at("Code");
    const before = mkState(DOC, cursor);
    const openingFence = at("```");
    const broken = edit(before, { from: openingFence, to: openingFence + 1 }, cursor - 1);

    // Move the cursor to the top paragraph: the block is no longer pinned.
    const moved = broken.update({ selection: EditorSelection.cursor(1) }).state;

    expect(classesOf(moved), "the broken fence is now honestly reflected").not.toContain("cm-h1");
  });

  it("unpins once the block is deleted entirely", () => {
    const cursor = at("Code");
    const before = mkState(DOC, cursor);
    const blockFrom = at("```");
    const blockTo = blockFrom + "```\nCode\n```".length;

    const after = edit(before, { from: blockFrom, to: blockTo }, blockFrom);

    expect(classesOf(after)).toContain("cm-h1");
    expect(classesOf(after), "no code slab should survive").not.toContain("cm-code-line");
  });

  it("rebuilds when an edit lands outside the pinned block", () => {
    const before = mkState(DOC, at("Code"));
    // An edit in the first paragraph while the cursor claims to be in the fence.
    const after = before.update({ changes: { from: 0, insert: "X" } }).state;
    expect(classesOf(after)).toContain("cm-h1");
  });

  it("does not pin the whole document as the user types at a block's end", () => {
    // Typing at the end of a paragraph must leave it, not stretch it.
    const doc = "alpha\n\n# Heading\n";
    let state = mkState(doc, 5);
    for (const ch of "bcd") {
      const pos = state.selection.main.head;
      state = edit(state, { from: pos, insert: ch }, pos + 1);
    }
    expect(state.doc.toString().startsWith("alphabcd")).toBe(true);
    expect(classesOf(state), "the heading must still render").toContain("cm-h1");
  });
});
