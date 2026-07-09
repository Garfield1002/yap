import { EditorView } from "@codemirror/view";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { tags as t } from "@lezer/highlight";

/**
 * Chrome-level theme. Colors are always `var(--x)` so a system light/dark flip
 * repaints without reconfiguring the editor.
 */
export const yapTheme = EditorView.theme({
  "&": {
    height: "100%",
    color: "var(--fg)",
    backgroundColor: "var(--bg)",
    fontFamily: "var(--font-prose)",
    fontSize: "var(--font-size)",
  },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": {
    fontFamily: "inherit",
    lineHeight: "var(--line-height)",
    overflowY: "auto",
    overflowX: "hidden",
  },
  ".cm-content": {
    caretColor: "var(--cursor)",
    maxWidth: "var(--measure)",
    width: "100%",
    boxSizing: "border-box",
    margin: "0 auto",
    padding: "var(--editor-pad-y) var(--editor-pad-x)",
  },
  ".cm-line": { padding: "0 2px" },
  ".cm-cursor, .cm-dropCursor": {
    borderLeftColor: "var(--cursor)",
    borderLeftWidth: "2px",
  },
  "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection": {
    backgroundColor: "var(--selection)",
  },
  "&:not(.cm-focused) .cm-selectionBackground": {
    backgroundColor: "var(--selection-blur)",
  },
});

/**
 * Token styling. Applies in raw *and* rendered regions on purpose: revealing a
 * block should only make markup characters appear, never restyle its text.
 */
export const yapHighlightStyle = HighlightStyle.define([
  { tag: t.heading1, fontWeight: "700" },
  { tag: t.heading2, fontWeight: "700" },
  { tag: t.heading3, fontWeight: "600" },
  { tag: [t.heading4, t.heading5, t.heading6], fontWeight: "600" },
  { tag: t.strong, fontWeight: "700" },
  { tag: t.emphasis, fontStyle: "italic" },
  { tag: t.strikethrough, textDecoration: "line-through" },
  { tag: t.link, color: "var(--accent)" },
  { tag: t.url, color: "var(--fg-dim)" },
  { tag: t.quote, color: "var(--fg-dim)" },
  { tag: t.monospace, fontFamily: "var(--font-mono)", fontSize: "0.9em" },
  { tag: t.processingInstruction, color: "var(--fg-faint)" },
  { tag: t.contentSeparator, color: "var(--fg-faint)" },

  // fenced-code interiors, via @codemirror/language-data grammars
  { tag: t.keyword, color: "var(--syn-keyword)" },
  { tag: [t.string, t.special(t.string)], color: "var(--syn-string)" },
  { tag: [t.comment, t.lineComment, t.blockComment], color: "var(--syn-comment)", fontStyle: "italic" },
  { tag: [t.number, t.bool, t.null], color: "var(--syn-number)" },
  { tag: [t.function(t.variableName), t.function(t.propertyName)], color: "var(--syn-name)" },
  { tag: [t.typeName, t.className, t.namespace], color: "var(--syn-type)" },
  { tag: [t.operator, t.operatorKeyword], color: "var(--syn-operator)" },
  { tag: t.definition(t.variableName), color: "var(--fg)" },
]);

export const yapHighlighting = syntaxHighlighting(yapHighlightStyle);
