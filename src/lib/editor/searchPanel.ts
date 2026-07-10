import type { EditorView, Panel } from "@codemirror/view";
import type { ViewUpdate } from "@codemirror/view";
import {
  SearchQuery,
  getSearchQuery,
  setSearchQuery,
  findNext,
  findPrevious,
  replaceNext,
  replaceAll,
  closeSearchPanel,
} from "@codemirror/search";

/**
 * A custom find/replace panel, replacing CodeMirror's stock one.
 *
 * It sits at the bottom, labels its toggles in words ("Match Case", "Regex",
 * "Whole Words"), navigates with up/down arrows, shows a live match count, and
 * keeps the replace row hidden until the caret-right arrow (next to close)
 * expands it. The search behaviour underneath is CodeMirror's -- we only own
 * the DOM and forward to `setSearchQuery`, `findNext`, `replaceNext`, etc.
 */
export function searchPanel(view: EditorView): Panel {
  const existing = getSearchQuery(view.state);
  const flags = {
    caseSensitive: existing.caseSensitive,
    regexp: existing.regexp,
    wholeWord: existing.wholeWord,
  };

  const dom = document.createElement("div");
  dom.className = "cm-panel bulletmd-search";
  dom.addEventListener("keydown", onKeydown);

  const searchField = input("Find", existing.search);
  const replaceField = input("Replace", existing.replace);

  const count = el("span", "bulletmd-search-count");

  const prev = iconButton("↑", "Previous match", () => run(findPrevious));
  const next = iconButton("↓", "Next match", () => run(findNext));

  const caseToggle = toggle("Match Case", flags.caseSensitive, (on) => {
    flags.caseSensitive = on;
    commit();
  });
  const regexpToggle = toggle("Regex", flags.regexp, (on) => {
    flags.regexp = on;
    commit();
  });
  const wordToggle = toggle("Whole Words", flags.wholeWord, (on) => {
    flags.wholeWord = on;
    commit();
  });

  const replaceRow = el("div", "bulletmd-search-row bulletmd-search-replace-row");
  const expandReplace = iconButton("»", "Toggle replace", toggleReplace);
  expandReplace.classList.add("bulletmd-search-expand");
  const close = iconButton("✕", "Close", () => closeSearchPanel(view));
  close.classList.add("bulletmd-search-close");

  const replaceBtn = textButton("Replace", () => run(replaceNext));
  const replaceAllBtn = textButton("All", () => run(replaceAll));

  const searchRow = el("div", "bulletmd-search-row");
  searchRow.append(
    searchField,
    count,
    prev,
    next,
    caseToggle,
    regexpToggle,
    wordToggle,
    expandReplace,
    close,
  );

  replaceRow.append(replaceField, replaceBtn, replaceAllBtn);
  replaceRow.hidden = true;

  dom.append(searchRow, replaceRow);

  searchField.addEventListener("input", commit);
  replaceField.addEventListener("input", commit);

  function query(): SearchQuery {
    return new SearchQuery({
      search: searchField.value,
      replace: replaceField.value,
      caseSensitive: flags.caseSensitive,
      regexp: flags.regexp,
      wholeWord: flags.wholeWord,
    });
  }

  /** Push the current form state into the editor and refresh the count. */
  function commit(): void {
    view.dispatch({ effects: setSearchQuery.of(query()) });
    refreshCount();
  }

  function run(command: (v: EditorView) => boolean): void {
    command(view);
    view.focus();
  }

  function toggleReplace(): void {
    replaceRow.hidden = !replaceRow.hidden;
    expandReplace.classList.toggle("expanded", !replaceRow.hidden);
    if (!replaceRow.hidden) replaceField.focus();
    else searchField.focus();
  }

  function onKeydown(event: KeyboardEvent): void {
    if (event.key === "Escape") {
      event.preventDefault();
      closeSearchPanel(view);
    } else if (event.key === "Enter" && event.target === searchField) {
      event.preventDefault();
      run(event.shiftKey ? findPrevious : findNext);
    } else if (event.key === "Enter" && event.target === replaceField) {
      event.preventDefault();
      run(replaceNext);
    }
  }

  /** Recompute "n/total", where n is the match holding the main selection. */
  function refreshCount(): void {
    const q = getSearchQuery(view.state);
    if (!q.search) {
      count.textContent = "";
      return;
    }
    const main = view.state.selection.main;
    let total = 0;
    let current = 0;
    try {
      const cursor = q.getCursor(view.state);
      for (let r = cursor.next(); !r.done; r = cursor.next()) {
        total++;
        if (r.value.from === main.from && r.value.to === main.to) current = total;
      }
    } catch {
      count.textContent = "—"; // invalid regexp
      return;
    }
    count.textContent = total ? `${current || "–"}/${total}` : "0/0";
  }

  return {
    dom,
    top: false,
    mount() {
      refreshCount();
      searchField.focus();
      searchField.select();
    },
    update(update: ViewUpdate) {
      const queryChanged = update.transactions.some((tr) =>
        tr.effects.some((e) => e.is(setSearchQuery)),
      );
      if (update.docChanged || update.selectionSet || queryChanged) refreshCount();
    },
  };
}

// --- tiny DOM helpers -------------------------------------------------------

function el(tag: string, className: string): HTMLElement {
  const node = document.createElement(tag);
  node.className = className;
  return node;
}

function input(placeholder: string, value: string): HTMLInputElement {
  const node = document.createElement("input");
  node.type = "text";
  node.placeholder = placeholder;
  node.value = value;
  node.className = "bulletmd-search-field";
  return node;
}

function iconButton(glyph: string, title: string, onClick: () => void): HTMLButtonElement {
  const node = document.createElement("button");
  node.type = "button";
  node.textContent = glyph;
  node.title = title;
  node.className = "bulletmd-search-icon";
  node.addEventListener("click", onClick);
  return node;
}

function textButton(label: string, onClick: () => void): HTMLButtonElement {
  const node = document.createElement("button");
  node.type = "button";
  node.textContent = label;
  node.className = "bulletmd-search-btn";
  node.addEventListener("click", onClick);
  return node;
}

function toggle(label: string, on: boolean, onChange: (on: boolean) => void): HTMLButtonElement {
  const node = document.createElement("button");
  node.type = "button";
  node.textContent = label;
  node.className = "bulletmd-search-toggle";
  node.setAttribute("aria-pressed", String(on));
  node.classList.toggle("active", on);
  node.addEventListener("click", () => {
    const next = node.getAttribute("aria-pressed") !== "true";
    node.setAttribute("aria-pressed", String(next));
    node.classList.toggle("active", next);
    onChange(next);
  });
  return node;
}
