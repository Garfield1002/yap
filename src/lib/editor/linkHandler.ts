import { EditorView, ViewPlugin } from "@codemirror/view";
import type { EditorState, Extension } from "@codemirror/state";
import { syntaxTree } from "@codemirror/language";
import { openUrl } from "@tauri-apps/plugin-opener";

/** Only schemes that are safe to hand to the desktop's URL handler. */
const OPENABLE = /^(https?|mailto):/i;

/** Set on the editor while Ctrl/Cmd is down, so links can show a pointer. */
const MODIFIER_CLASS = "cm-mod-held";

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

const isModifier = (key: string) => key === "Control" || key === "Meta";

/**
 * Reflects the Ctrl/Cmd key into a class on the editor. Without it the link
 * cursor could only be styled unconditionally, which would promise a click
 * target that a plain click does not honour.
 */
class ModifierTracker {
  private held = false;

  constructor(readonly view: EditorView) {
    window.addEventListener("blur", this.release);
  }

  set(held: boolean) {
    if (held === this.held) return;
    this.held = held;
    this.view.dom.classList.toggle(MODIFIER_CLASS, held);
  }

  // The keyup never arrives if focus moves while the key is down.
  release = () => this.set(false);

  destroy() {
    window.removeEventListener("blur", this.release);
    this.view.dom.classList.remove(MODIFIER_CLASS);
  }
}

const modifierTracker = ViewPlugin.fromClass(ModifierTracker, {
  eventHandlers: {
    keydown(event: KeyboardEvent) {
      if (isModifier(event.key)) this.set(true);
    },
    keyup(event: KeyboardEvent) {
      if (isModifier(event.key)) this.set(false);
    },
    blur() {
      this.set(false);
    },
  },
});

/**
 * Ctrl+Click (Cmd+Click) opens a link in the system browser. A plain click just
 * places the cursor, which reveals the raw source -- that is how you edit a link.
 */
const clickHandler = EditorView.domEventHandlers({
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

export const linkHandling: Extension = [clickHandler, modifierTracker];
