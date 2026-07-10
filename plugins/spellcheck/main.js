// Spell check — the plugin API's forcing function. It exercises the deepest
// surface: async decorations over text, cooperation with live preview (skip
// code, math, and URLs), a suggestion UI, settings, and a system dependency.
//
// The engine is a Rust command (spellbook, reading the system Hunspell dicts),
// reached through bulletmd.system.spellCheck / spellSuggest. Checking is debounced
// and runs off the keystroke path: misspelling underlines arrive after the fact.
//
// Settings live in this plugin's data.json. The language is hand-edited in v1;
// words added from the suggestion popup are persisted automatically:
//   { "lang": "en_US", "words": ["bulletmd"] }

export async function activate(bulletmd) {
  const { StateField, StateEffect, RangeSetBuilder } = bulletmd.cm.state;
  const { Decoration, EditorView, ViewPlugin } = bulletmd.cm.view;
  const { syntaxTree } = bulletmd.cm.language;

  // --- settings -------------------------------------------------------------
  const settings = await bulletmd.settings.get();
  const languages = await bulletmd.system.spellLanguages();
  const lang =
    (typeof settings.lang === "string" && settings.lang) ||
    (languages.includes("en_US") ? "en_US" : languages[0]);
  const customWords = new Set(
    Array.isArray(settings.words)
      ? settings.words.filter((word) => typeof word === "string" && word.length > 0)
      : [],
  );

  if (!lang) {
    // No dictionaries installed; nothing to do but say so.
    bulletmd.statusBar.addItem({ id: "spell", text: "spell: no dictionary" });
    return;
  }

  // --- decoration state -----------------------------------------------------
  // The misspelling set is replaced wholesale by an effect the async checker
  // dispatches, and mapped through edits in between so underlines track text.
  const setMisspellings = StateEffect.define();
  const underline = Decoration.mark({ class: "bulletmd-misspelled" });

  const field = StateField.define({
    create: () => Decoration.none,
    update(deco, tr) {
      deco = deco.map(tr.changes);
      for (const e of tr.effects) {
        if (e.is(setMisspellings)) {
          const b = new RangeSetBuilder();
          for (const r of e.value) b.add(r.from, r.to, underline);
          deco = b.finish();
        }
      }
      return deco;
    },
    provide: (f) => EditorView.decorations.from(f),
  });

  // --- word extraction ------------------------------------------------------
  // Skip anything that is not prose: code (any *Code* node), math (*Math*), and
  // URL targets. Link *text* is left in, so typos in it are still caught.
  function isSkipped(name) {
    return name.includes("Code") || name.includes("Math") || name === "URL" || name === "Autolink";
  }

  function candidateWords(state) {
    const skip = [];
    syntaxTree(state).iterate({
      enter: (n) => {
        if (isSkipped(n.name)) {
          skip.push([n.from, n.to]);
          return false; // don't descend into skipped regions
        }
      },
    });
    const text = state.doc.toString();
    const words = [];
    const re = /[A-Za-z][A-Za-z']*/g;
    let m;
    while ((m = re.exec(text))) {
      const from = m.index;
      const to = from + m[0].length;
      // Drop a trailing apostrophe so "dog's" checks as "dog's" but "foo'" -> "foo".
      const word = m[0].replace(/'+$/, "");
      if (word.length < 2) continue;
      if (skip.some(([a, b]) => from < b && to > a)) continue;
      words.push({ word, from, to: from + word.length });
    }
    return words;
  }

  async function recheck(view) {
    // Effects carry absolute document offsets. If an edit lands while the IPC
    // request is in flight, those offsets no longer describe this view's
    // document, so discard the response and let the edit's debounced check win.
    const checkedDoc = view.state.doc;
    const words = candidateWords(view.state);
    if (words.length === 0) {
      view.dispatch({ effects: setMisspellings.of([]) });
      return;
    }
    const unique = [...new Set(words.map((w) => w.word))].filter(
      (word) => !customWords.has(word),
    );
    if (unique.length === 0) {
      view.dispatch({ effects: setMisspellings.of([]) });
      return;
    }
    let bad;
    try {
      bad = new Set(await bulletmd.system.spellCheck(unique, lang));
    } catch {
      return; // engine unavailable this round; leave the last result in place
    }
    if (view.state.doc !== checkedDoc) return;
    const ranges = words
      .filter((w) => bad.has(w.word))
      .map((w) => ({ from: w.from, to: w.to }));
    try {
      view.dispatch({ effects: setMisspellings.of(ranges) });
    } catch {
      /* view torn down mid-check */
    }
  }

  // The live editor, captured so the palette command can reach it (commands run
  // without a view handle in v1).
  let activeView = null;

  // Debounced: re-check shortly after typing stops, and once on load.
  const checker = ViewPlugin.fromClass(
    class {
      constructor(view) {
        this.view = view;
        activeView = view;
        this.timer = 0;
        this.schedule();
      }
      update(u) {
        if (u.docChanged) this.schedule();
      }
      schedule() {
        clearTimeout(this.timer);
        this.timer = setTimeout(() => recheck(this.view), 400);
      }
      destroy() {
        clearTimeout(this.timer);
        if (activeView === this.view) activeView = null;
      }
    },
  );

  // --- suggestion popup -----------------------------------------------------
  let popup = null;

  function closePopup() {
    if (popup) {
      popup.remove();
      popup = null;
      document.removeEventListener("mousedown", onOutside, true);
    }
  }
  function onOutside(e) {
    if (popup && !popup.contains(e.target)) closePopup();
  }

  async function addToDictionary(word, view) {
    const words = [...customWords, word].sort((a, b) => a.localeCompare(b));
    await bulletmd.settings.set({ ...settings, words });
    customWords.add(word);
    await recheck(view);
  }

  async function openSuggestions(view, range) {
    const word = view.state.doc.sliceString(range.from, range.to);
    let suggestions = [];
    try {
      suggestions = await bulletmd.system.spellSuggest(word, lang);
    } catch {
      /* leave empty */
    }
    closePopup();

    const coords = view.coordsAtPos(range.from);
    if (!coords) return;
    popup = document.createElement("div");
    popup.className = "bulletmd-spell-popup";
    popup.style.left = `${coords.left}px`;
    popup.style.top = `${coords.bottom + 2}px`;

    if (suggestions.length === 0) {
      const none = document.createElement("div");
      none.className = "bulletmd-spell-none";
      none.textContent = "No suggestions";
      popup.appendChild(none);
    } else {
      for (const s of suggestions.slice(0, 8)) {
        const btn = document.createElement("button");
        btn.type = "button";
        btn.className = "bulletmd-spell-item";
        btn.textContent = s;
        btn.addEventListener("click", () => {
          view.dispatch({ changes: { from: range.from, to: range.to, insert: s } });
          closePopup();
          view.focus();
        });
        popup.appendChild(btn);
      }
    }

    const add = document.createElement("button");
    add.type = "button";
    add.className = "bulletmd-spell-item bulletmd-spell-add";
    add.textContent = `Add “${word}” to dictionary`;
    add.addEventListener("click", async () => {
      add.disabled = true;
      try {
        await addToDictionary(word, view);
        closePopup();
        view.focus();
      } catch {
        add.disabled = false;
        add.textContent = "Could not add word";
      }
    });
    popup.appendChild(add);

    document.body.appendChild(popup);
    // Defer so this same click doesn't immediately dismiss it.
    setTimeout(() => document.addEventListener("mousedown", onOutside, true), 0);
  }

  // Right-click a misspelling to see corrections.
  const suggestHandler = EditorView.domEventHandlers({
    contextmenu(event, view) {
      const pos = view.posAtCoords({ x: event.clientX, y: event.clientY });
      if (pos == null) return false;
      const deco = view.state.field(field, false);
      if (!deco) return false;
      let hit = null;
      deco.between(pos, pos, (from, to) => {
        hit = { from, to };
        return false;
      });
      if (!hit) return false;
      event.preventDefault();
      void openSuggestions(view, hit);
      return true;
    },
  });

  // --- wire in --------------------------------------------------------------
  bulletmd.editor.registerExtension([field, checker, suggestHandler]);

  bulletmd.statusBar.addItem({ id: "spell", text: `spell: ${lang}`, title: "Spell check active" });

  bulletmd.commands.register({
    id: "spellcheck.suggest",
    title: "Spell Check: Suggest at Cursor",
    keybinding: "Mod+.",
    run: () => {
      const view = activeView;
      if (!view) return;
      const pos = view.state.selection.main.head;
      const deco = view.state.field(field, false);
      let hit = null;
      deco?.between(pos, pos, (from, to) => {
        hit = { from, to };
        return false;
      });
      if (hit) void openSuggestions(view, hit);
    },
  });
}
