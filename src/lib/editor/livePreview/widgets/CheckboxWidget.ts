import { EditorView, WidgetType } from "@codemirror/view";

/**
 * A `[ ]` / `[x]` task marker rendered as a real checkbox.
 *
 * Toggling dispatches an ordinary one-character change, so it lands in the undo
 * history and the autosave listener sees it like any other edit.
 */
export class CheckboxWidget extends WidgetType {
  constructor(readonly checked: boolean) {
    super();
  }

  eq(other: CheckboxWidget): boolean {
    return other.checked === this.checked;
  }

  toDOM(view: EditorView): HTMLElement {
    const box = document.createElement("input");
    box.type = "checkbox";
    box.className = "cm-task-checkbox";
    box.checked = this.checked;

    box.addEventListener("mousedown", (event) => {
      event.preventDefault();
      // Ask the view where this DOM node currently is. A position captured when
      // the widget was built would be stale after any edit above it.
      const pos = view.posAtDOM(box);
      const marker = view.state.doc.sliceString(pos, pos + 3);
      if (!/^\[[ xX]\]$/.test(marker)) return;

      view.dispatch({
        changes: { from: pos + 1, to: pos + 2, insert: this.checked ? " " : "x" },
      });
    });

    return box;
  }

  /** Let the checkbox handle its own clicks instead of moving the cursor. */
  ignoreEvent(): boolean {
    return true;
  }
}
