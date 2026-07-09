import { EditorSelection, type StateCommand } from "@codemirror/state";
import { keymap } from "@codemirror/view";
import { syntaxTree } from "@codemirror/language";
import { indentLess, indentMore, insertTab, redo, undo } from "@codemirror/commands";

/**
 * Standard editing keys plus the small set of markdown shortcuts from the plan:
 * Ctrl/Cmd+B/I for emphasis, Ctrl/Cmd+K for a link, Tab to nest a list item.
 * No vim in v1.
 *
 * The commands are plain buffer edits, so they land in the undo history and the
 * autosave listener sees them like any other typing.
 */

/**
 * Wrap the selection in `mark`, or peel it off when it is already wrapped.
 *
 * Toggling looks at the characters just outside each range: `**word**` with
 * `word` selected unwraps back to `word`. An empty selection inserts the pair
 * and parks the cursor between the halves, so Ctrl+B then typing produces bold.
 */
function toggleWrap(mark: string): StateCommand {
  return ({ state, dispatch }) => {
    const len = mark.length;
    const tr = state.changeByRange((range) => {
      const before = state.sliceDoc(range.from - len, range.from);
      const after = state.sliceDoc(range.to, range.to + len);

      if (before === mark && after === mark) {
        return {
          changes: [
            { from: range.from - len, to: range.from },
            { from: range.to, to: range.to + len },
          ],
          range: EditorSelection.range(range.from - len, range.to - len),
        };
      }

      return {
        changes: [
          { from: range.from, insert: mark },
          { from: range.to, insert: mark },
        ],
        range: EditorSelection.range(range.from + len, range.to + len),
      };
    });

    dispatch(state.update(tr, { scrollIntoView: true, userEvent: "input.wrap" }));
    return true;
  };
}

export const toggleBold = toggleWrap("**");
export const toggleItalic = toggleWrap("*");
export const toggleInlineCode = toggleWrap("`");

/**
 * Turn the selection into `[selection]()` and drop the cursor between the
 * parentheses, ready for the URL. With no selection this inserts an empty
 * `[]()` -- still cursor-in-parens, since that is where you usually paste.
 */
export const insertLink: StateCommand = ({ state, dispatch }) => {
  const tr = state.changeByRange((range) => {
    const text = state.sliceDoc(range.from, range.to);
    const insert = `[${text}]()`;
    // One character back from the end: inside the closing paren.
    const caret = range.from + insert.length - 1;
    return {
      changes: { from: range.from, to: range.to, insert },
      range: EditorSelection.cursor(caret),
    };
  });

  dispatch(state.update(tr, { scrollIntoView: true, userEvent: "input.link" }));
  return true;
};

/** True when the primary cursor sits inside a bullet or ordered list item. */
function inList(state: Parameters<StateCommand>[0]["state"]): boolean {
  for (
    let node = syntaxTree(state).resolveInner(state.selection.main.head, -1);
    node;
    node = node.parent!
  ) {
    if (node.name === "ListItem" || node.name === "BulletList" || node.name === "OrderedList") {
      return true;
    }
    if (!node.parent) break;
  }
  return false;
}

/**
 * Tab nests the current list item; outside a list it inserts indentation like
 * any editor, so Tab never becomes dead. Shift+Tab always dedents, which is
 * harmless off a list.
 */
export const indentListOrTab: StateCommand = (target) =>
  inList(target.state) ? indentMore(target) : insertTab(target);

export const markdownShortcuts = keymap.of([
  { key: "Mod-b", run: toggleBold, preventDefault: true },
  { key: "Mod-i", run: toggleItalic, preventDefault: true },
  { key: "Mod-e", run: toggleInlineCode, preventDefault: true },
  { key: "Mod-k", run: insertLink, preventDefault: true },
  { key: "Tab", run: indentListOrTab, shift: indentLess, preventDefault: true },
  // Undo/redo bound here too (not only via the default keymap) so they are
  // guaranteed even though the menu items carry no accelerator.
  { key: "Mod-z", run: undo, preventDefault: true },
  { key: "Mod-Shift-z", run: redo, preventDefault: true },
  { key: "Mod-y", run: redo, preventDefault: true },
]);
