import { EditorState, type Extension } from "@codemirror/state";
import {
  EditorView,
  keymap,
  drawSelection,
  dropCursor,
  rectangularSelection,
  crosshairCursor,
  highlightSpecialChars,
} from "@codemirror/view";
import { history, historyKeymap, defaultKeymap } from "@codemirror/commands";
import { indentOnInput, bracketMatching } from "@codemirror/language";
import { yapMarkdown } from "./markdownLang";
import { yapLezerExtensions } from "./lezer";
import { yapTheme, yapHighlighting } from "./theme";
import { livePreview, documentDirectory } from "./livePreview";
import { linkHandling } from "./linkHandler";
import { markdownShortcuts } from "./markdownShortcuts";

export interface EditorOptions {
  parent: HTMLElement;
  doc: string;
  /** Directory of the open file; relative image paths resolve against it. */
  documentDir?: string;
  /** Called for every transaction that changed the document. */
  onDocChange?: (doc: string) => void;
  /** Extra extensions (live preview, custom keymaps) layered on top. */
  extensions?: Extension[];
}

export function createEditor(opts: EditorOptions): EditorView {
  const listener = EditorView.updateListener.of((update) => {
    if (update.docChanged) opts.onDocChange?.(update.state.doc.toString());
  });

  const state = EditorState.create({
    doc: opts.doc,
    extensions: [
      history(),
      drawSelection(),
      dropCursor(),
      rectangularSelection(),
      crosshairCursor(),
      highlightSpecialChars(),
      indentOnInput(),
      bracketMatching(),
      EditorState.allowMultipleSelections.of(true),
      EditorView.lineWrapping,
      yapMarkdown(yapLezerExtensions),
      yapHighlighting,
      documentDirectory.of(opts.documentDir ?? ""),
      livePreview(),
      linkHandling,
      // Ahead of the default keymap so Mod-b/i/k and Tab win the precedence tie.
      markdownShortcuts,
      yapTheme,
      ...(opts.extensions ?? []),
      // Defaults last: earlier extensions win precedence ties, and the markdown
      // keymap (Enter/Backspace) is installed by `yapMarkdown` above.
      keymap.of([...historyKeymap, ...defaultKeymap]),
      listener,
    ],
  });

  return new EditorView({ state, parent: opts.parent });
}
