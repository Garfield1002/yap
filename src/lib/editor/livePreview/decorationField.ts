import { StateEffect, StateField, type EditorState, type Transaction } from "@codemirror/state";
import { Decoration, EditorView, ViewPlugin, type ViewUpdate } from "@codemirror/view";
import { syntaxTree } from "@codemirror/language";
import { buildDecorations } from "./buildDecorations";
import { activeRegions, type Region } from "./activeRegions";
import type { Decorations } from "./builder";
import {
  changesStayInside,
  inside,
  mapRegions,
  outside,
  selectionStaysInside,
} from "./pinning";

/** Ask the field to rebuild even though the doc and selection are unchanged. */
export const refreshDecorations = StateEffect.define<null>();

interface PreviewState extends Decorations {
  /** The blocks currently shown as raw source. */
  pinned: Region[];
}

function rebuild(state: EditorState): PreviewState {
  const pinned = activeRegions(state);
  return { ...buildDecorations(state, pinned), pinned };
}

/**
 * While the cursor edits inside a block, that block stays pinned raw and the
 * rest of the document keeps the decorations it already had, merely shifted by
 * the edit.
 *
 * This exists because a half-typed block corrupts the parse of everything after
 * it. Delete one backtick from a fence and the *other* fence opens a code block
 * that swallows the remainder of the file: the headings and emphasis below stop
 * existing as syntax nodes at all, so no amount of care in the builders can keep
 * them rendered. The only fix is not to ask the contaminated tree about them.
 *
 * The moment the cursor leaves the block -- or an edit lands outside it -- the
 * document is rebuilt from the real tree, broken fence and all.
 */
function updatePreview(value: PreviewState, tr: Transaction): PreviewState {
  const forced = tr.reconfigured || tr.effects.some((e) => e.is(refreshDecorations));
  if (forced) return rebuild(tr.state);
  if (!tr.docChanged && !tr.selection) return value;

  const mapped = mapRegions(value.pinned, tr.changes);
  const stillPinned =
    value.pinned.length > 0 &&
    // An emptied block (the user deleted it) must not stay pinned, or the
    // document outside it would never be rebuilt again.
    mapped.every((r) => r.to > r.from) &&
    changesStayInside(value.pinned, tr.changes) &&
    selectionStaysInside(mapped, tr.state.selection);

  if (!stillPinned) return rebuild(tr.state);
  if (!tr.docChanged) return { ...value, pinned: mapped };

  // Inside the pinned block, trust the new tree: the block is raw, so this only
  // yields line and mark decorations, and they must track the edit.
  const fresh = buildDecorations(tr.state, mapped);
  const previous = value.deco.map(tr.changes);
  const previousAtomic = value.atomic.map(tr.changes);

  return {
    pinned: mapped,
    deco: Decoration.set([...outside(previous, mapped), ...inside(fresh.deco, mapped)], true),
    // A pinned block emits no replaces, so nothing of the fresh atomic set is
    // inside it; only the untouched remainder survives.
    atomic: Decoration.set(outside(previousAtomic, mapped), true),
  };
}

/**
 * A StateField, deliberately not a ViewPlugin.
 *
 * Decorations that change vertical layout -- block widgets, replaces that span
 * a line break, tall inline widgets -- must be known to the state before the
 * view measures heights. Supplying them from a ViewPlugin corrupts CodeMirror's
 * height estimates and makes scrolling jump.
 */
export const livePreviewField = StateField.define<PreviewState>({
  create: rebuild,
  update: updatePreview,

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
