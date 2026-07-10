import { Compartment, EditorState, type Extension } from "@codemirror/state";
import {
  EditorView,
  keymap,
  dropCursor,
  rectangularSelection,
  crosshairCursor,
  highlightSpecialChars,
} from "@codemirror/view";
import { history, historyKeymap, defaultKeymap } from "@codemirror/commands";
import { search, searchKeymap } from "@codemirror/search";
import { searchPanel } from "./searchPanel";
import { indentOnInput, bracketMatching } from "@codemirror/language";
import { yapMarkdown } from "./markdownLang";
import { yapLezerExtensions } from "./lezer";
import { pluginGrammar } from "./lezer/pluginGrammar";
import { pluginExtensions } from "./pluginExtensions";
import { yapTheme, yapHighlighting } from "./theme";
import { livePreview, documentDirectory } from "./livePreview";
import { linkHandling } from "./linkHandler";
import { markdownShortcuts } from "./markdownShortcuts";
import { imagePaste } from "./imagePaste";
import { blockNavigation } from "./blockNavigation";

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
      dropCursor(),
      rectangularSelection(),
      crosshairCursor(),
      highlightSpecialChars(),
      indentOnInput(),
      bracketMatching(),
      // Custom bottom find/replace panel (Mod-f / Mod-Alt-f): worded toggles,
      // up/down navigation, a live match count, and a collapsible replace row.
      search({ createPanel: searchPanel }),
      EditorState.allowMultipleSelections.of(true),
      EditorView.lineWrapping,
      // Built-in grammar (math, footnotes) plus any plugin-contributed Lezer
      // extensions collected before this editor was created.
      yapMarkdown([...yapLezerExtensions, ...pluginGrammar()]),
      yapHighlighting,
      documentDirCompartment.of(documentDirectory.of(opts.documentDir ?? "")),
      livePreview(),
      linkHandling,
      imagePaste,
      // Ahead of the default keymap so Mod-b/i/k and Tab win the precedence tie.
      markdownShortcuts,
      // Arrow keys step *into* a rendered block-math widget rather than past it.
      blockNavigation,
      // Escape blurs the editor; a blurred editor renders every block as a clean
      // preview (see the focus watcher in livePreview).
      keymap.of([{ key: "Escape", run: (view) => (view.contentDOM.blur(), true) }]),
      yapTheme,
      // Raw CM6 extensions contributed by enabled plugins, collected before the
      // editor was built.
      ...pluginExtensions(),
      ...(opts.extensions ?? []),
      // Defaults last: earlier extensions win precedence ties, and the markdown
      // keymap (Enter/Backspace) is installed by `yapMarkdown` above.
      keymap.of([...searchKeymap, ...historyKeymap, ...defaultKeymap]),
      listener,
    ],
  });

  return new EditorView({ state, parent: opts.parent });
}
