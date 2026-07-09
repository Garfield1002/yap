import { StateEffect, StateField } from "@codemirror/state";
import { EditorView, ViewPlugin, type ViewUpdate } from "@codemirror/view";
import { syntaxTree } from "@codemirror/language";
import { buildDecorations } from "./buildDecorations";
import type { Decorations } from "./builder";

/** Ask the field to rebuild even though the doc and selection are unchanged. */
export const refreshDecorations = StateEffect.define<null>();

/**
 * A StateField, deliberately not a ViewPlugin.
 *
 * Decorations that change vertical layout — block widgets, replaces that span
 * a line break, tall inline widgets — must be known to the state before the
 * view measures heights. Supplying them from a ViewPlugin corrupts CodeMirror's
 * height estimates and makes scrolling jump.
 */
export const livePreviewField = StateField.define<Decorations>({
  create: buildDecorations,

  update(value, tr) {
    const rebuild =
      tr.docChanged ||
      tr.selection != null ||
      tr.reconfigured ||
      tr.effects.some((e) => e.is(refreshDecorations));
    return rebuild ? buildDecorations(tr.state) : value;
  },

  provide: (field) => [
    EditorView.decorations.from(field, (v) => v.deco),
    EditorView.atomicRanges.of((view) => view.state.field(field).atomic),
  ],
});

/**
 * Lezer parses large documents incrementally in the background, extending the
 * tree over several idle callbacks. Those extensions arrive as transactions
 * with no doc or selection change, so the field above would ignore them and the
 * tail of the file would never render rich. This nudges it.
 */
const parseTailWatcher = ViewPlugin.fromClass(
  class {
    private lastTreeLength = -1;
    private frame = 0;

    constructor(private readonly view: EditorView) {
      this.lastTreeLength = syntaxTree(view.state).length;
    }

    update(update: ViewUpdate) {
      const length = syntaxTree(update.state).length;
      const grew = length !== this.lastTreeLength;
      this.lastTreeLength = length;
      // docChanged/selectionSet transactions already rebuilt the field.
      if (grew && !update.docChanged && !update.selectionSet) this.schedule();
    }

    private schedule() {
      if (this.frame) return;
      this.frame = requestAnimationFrame(() => {
        this.frame = 0;
        this.view.dispatch({ effects: refreshDecorations.of(null) });
      });
    }

    destroy() {
      if (this.frame) cancelAnimationFrame(this.frame);
    }
  },
);

export const livePreview = () => [livePreviewField, parseTailWatcher];
