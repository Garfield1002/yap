import { Compartment, EditorState, type Extension } from "@codemirror/state";
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
import { search, searchKeymap } from "@codemirror/search";
import { indentOnInput, bracketMatching } from "@codemirror/language";
import { yapMarkdown } from "./markdownLang";
import { yapLezerExtensions } from "./lezer";
import { yapTheme, yapHighlighting } from "./theme";
import { livePreview, documentDirectory } from "./livePreview";
import { linkHandling } from "./linkHandler";
import { markdownShortcuts } from "./markdownShortcuts";
import { imagePaste } from "./imagePaste";

/**
 * Holds the `documentDirectory` facet so it can be reconfigured in place when an
 * untitled buffer is first saved (or a file is renamed), without tearing down
 * the editor and losing the undo history and cursor.
 */
export const documentDirCompartment = new Compartment();

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
      // Search/replace panel (Mod-f / Mod-Alt-f) with regex, match-case and
      // whole-word toggles built in.
      search({ top: true }),
      EditorState.allowMultipleSelections.of(true),
      EditorView.lineWrapping,
      yapMarkdown(yapLezerExtensions),
      yapHighlighting,
      documentDirCompartment.of(documentDirectory.of(opts.documentDir ?? "")),
      livePreview(),
      linkHandling,
      imagePaste,
      // Ahead of the default keymap so Mod-b/i/k and Tab win the precedence tie.
      markdownShortcuts,
      yapTheme,
      ...(opts.extensions ?? []),
      // Defaults last: earlier extensions win precedence ties, and the markdown
      // keymap (Enter/Backspace) is installed by `yapMarkdown` above.
      keymap.of([...searchKeymap, ...historyKeymap, ...defaultKeymap]),
      listener,
    ],
  });

  return new EditorView({ state, parent: opts.parent });
}
