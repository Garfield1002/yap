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
  // Use the browser's native selection paint. CodeMirror's optional
  // `drawSelection()` layer is not composited by WebKitGTK in this app, so it
  // updates the selection state without leaving a visible highlight.
  ".cm-content ::selection, .cm-content::selection": {
    backgroundColor: "var(--selection)",
    color: "var(--fg)",
  },

  // Custom find/replace panel (see searchPanel.ts).
  ".cm-panels": {
    backgroundColor: "var(--bg)",
    color: "var(--fg)",
  },
  ".cm-panels.cm-panels-bottom": {
    borderTop: "1px solid var(--border, var(--fg-faint))",
  },
  ".yap-search": {
    fontFamily: "var(--font-prose)",
    padding: "7px 9px",
    display: "flex",
    flexDirection: "column",
    gap: "7px",
  },
  ".yap-search-row": {
    display: "flex",
    alignItems: "center",
    gap: "6px",
  },
  // `display: flex` above would otherwise beat the UA `[hidden]` rule, keeping
  // the collapsed replace row visible.
  ".yap-search-row[hidden]": {
    display: "none",
  },
  ".yap-search-field": {
    flex: "1",
    minWidth: "0",
    backgroundColor: "var(--bg)",
    color: "var(--fg)",
    border: "1px solid var(--border, var(--fg-faint))",
    borderRadius: "4px",
    padding: "4px 8px",
    fontFamily: "inherit",
    fontSize: "0.9em",
  },
  ".yap-search-field:focus": {
    outline: "none",
    borderColor: "var(--accent)",
  },
  ".yap-search-count": {
    fontSize: "0.8em",
    color: "var(--fg-dim)",
    minWidth: "3.5em",
    textAlign: "center",
    fontVariantNumeric: "tabular-nums",
  },
  // Icon buttons: up/down navigation, replace-expand caret, and close.
  ".yap-search-icon": {
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    width: "26px",
    height: "26px",
    fontSize: "0.95em",
    lineHeight: "1",
    background: "transparent",
    color: "var(--fg)",
    border: "1px solid var(--border, var(--fg-faint))",
    borderRadius: "4px",
    cursor: "pointer",
    padding: "0",
  },
  ".yap-search-icon:hover": {
    backgroundColor: "var(--selection-blur)",
  },
  ".yap-search-expand": {
    transition: "transform 120ms ease, color 120ms ease",
  },
  ".yap-search-expand.expanded": {
    transform: "rotate(90deg)",
    color: "var(--accent)",
    borderColor: "var(--accent)",
  },
  // A deliberately large, borderless close cross.
  ".yap-search-close": {
    width: "28px",
    height: "28px",
    fontSize: "1.25em",
    border: "none",
    color: "var(--fg-dim)",
  },
  ".yap-search-close:hover": {
    backgroundColor: "transparent",
    color: "var(--fg)",
  },
  ".yap-search-toggle": {
    fontFamily: "inherit",
    fontSize: "0.78em",
    whiteSpace: "nowrap",
    background: "transparent",
    color: "var(--fg-dim)",
    border: "1px solid var(--border, var(--fg-faint))",
    borderRadius: "4px",
    padding: "4px 9px",
    cursor: "pointer",
  },
  ".yap-search-toggle:hover": {
    backgroundColor: "var(--selection-blur)",
  },
  ".yap-search-toggle.active": {
    backgroundColor: "var(--accent)",
    borderColor: "var(--accent)",
    color: "var(--bg)",
  },
  ".yap-search-btn": {
    fontFamily: "inherit",
    fontSize: "0.85em",
    background: "transparent",
    color: "var(--fg)",
    border: "1px solid var(--border, var(--fg-faint))",
    borderRadius: "4px",
    padding: "4px 11px",
    cursor: "pointer",
  },
  ".yap-search-btn:hover": {
    backgroundColor: "var(--selection-blur)",
  },
  ".cm-searchMatch": {
    backgroundColor: "var(--selection-blur)",
  },
  ".cm-searchMatch.cm-searchMatch-selected": {
    backgroundColor: "var(--selection)",
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
