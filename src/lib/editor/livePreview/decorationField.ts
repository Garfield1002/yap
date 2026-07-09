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

/** Set editor focus. Blurred, every block renders -- nothing shows raw source,
 *  so pressing Escape (which blurs) turns the document into a clean preview. */
export const setFocused = StateEffect.define<boolean>();

interface PreviewState extends Decorations {
  /** The blocks currently shown as raw source. */
  pinned: Region[];
  /** Whether the editor is focused; blurred pins nothing. */
  focused: boolean;
}

function rebuild(state: EditorState, focused: boolean): PreviewState {
  const pinned = focused ? activeRegions(state) : [];
  return { ...buildDecorations(state, pinned), pinned, focused };
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
  let focused = value.focused;
  let focusChanged = false;
  for (const e of tr.effects) {
    if (e.is(setFocused)) {
      focused = e.value;
      focusChanged = true;
    }
  }

  const forced =
    tr.reconfigured || focusChanged || tr.effects.some((e) => e.is(refreshDecorations));
  if (forced) return rebuild(tr.state, focused);
  if (!tr.docChanged && !tr.selection) return value;

  const mapped = mapRegions(value.pinned, tr.changes);
  const stillPinned =
    value.pinned.length > 0 &&
    // An emptied block (the user deleted it) must not stay pinned, or the
    // document outside it would never be rebuilt again.
    mapped.every((r) => r.to > r.from) &&
    changesStayInside(value.pinned, tr.changes) &&
    selectionStaysInside(mapped, tr.state.selection);

  if (!stillPinned) return rebuild(tr.state, focused);
  if (!tr.docChanged) return { ...value, pinned: mapped };

  // Inside the pinned block, trust the new tree: the block is raw, so this only
  // yields line and mark decorations, and they must track the edit.
  const fresh = buildDecorations(tr.state, mapped);
  const previous = value.deco.map(tr.changes);
  const previousAtomic = value.atomic.map(tr.changes);

  return {
    focused,
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
  // Assume focused: the editor is focused right after it mounts, and the first
  // focus event would only re-confirm it.
  create: (state) => rebuild(state, true),
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

/** Mirror DOM focus into the field so a blurred editor renders every block. */
const focusWatcher = EditorView.domEventHandlers({
  focus: (_event, view) => {
    setFocus(view, true);
    return false;
  },
  blur: (_event, view) => {
    setFocus(view, false);
    return false;
  },
});

/** A blur can fire while the view is being torn down (document switch); a
 *  dispatch then throws, so swallow it -- the field is about to vanish anyway. */
function setFocus(view: EditorView, focused: boolean): void {
  try {
    view.dispatch({ effects: setFocused.of(focused) });
  } catch {
    /* view already destroyed */
  }
}

export const livePreview = () => [livePreviewField, parseTailWatcher, focusWatcher];
