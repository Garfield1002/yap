import { EditorView } from "@codemirror/view";
import type { EditorState } from "@codemirror/state";
import { syntaxTree } from "@codemirror/language";
import { openUrl } from "@tauri-apps/plugin-opener";

/** Only schemes that are safe to hand to the desktop's URL handler. */
const OPENABLE = /^(https?|mailto):/i;

/** The URL of the Link or Autolink covering `pos`, if any. */
export function urlAt(state: EditorState, pos: number): string | null {
  for (let node = syntaxTree(state).resolveInner(pos, -1); node; node = node.parent!) {
    if (node.name === "Link" || node.name === "Autolink") {
      const url = node.getChild("URL");
      if (!url) return null;
      const text = state.doc.sliceString(url.from, url.to);
      return OPENABLE.test(text) ? text : null;
    }
    if (!node.parent) break;
  }
  return null;
}

/**
 * Ctrl+Click (Cmd+Click) opens a link in the system browser. A plain click just
 * places the cursor, which reveals the raw source -- that is how you edit a link.
 */
export const linkClickHandler = EditorView.domEventHandlers({
  mousedown(event, view) {
    if (event.button !== 0 || !(event.ctrlKey || event.metaKey)) return false;

    const pos = view.posAtCoords({ x: event.clientX, y: event.clientY });
    if (pos == null) return false;

    const url = urlAt(view.state, pos);
    if (!url) return false;

    event.preventDefault();
    void openUrl(url);
    return true;
  },
});
