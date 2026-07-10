// Zotero Citations — search the local Zotero library and insert a Pandoc-style
// `[@citekey]` citation at the cursor. It talks only to Zotero's Local API, so
// the library never leaves this machine.
//
// Optional data.json settings (hand-edit in v1):
// {
//   "library": "users/0"
// }
// `library` can also be a group path such as "groups/123456". Citation keys
// come from Better BibTeX's `Citation Key:` line in an item's Extra field when
// present; otherwise the Zotero item key is inserted.

const DEFAULT_LIBRARY = "users/0";
const GRID = 24;
const EXCLUDED_ITEM_TYPES = new Set(["annotation", "attachment", "note"]);

function text(value) {
  return typeof value === "string" ? value.trim() : "";
}

function citationKey(item) {
  const extra = text(item.data?.extra);
  const fromExtra = extra.match(/^\s*Citation Key\s*:\s*(\S.*?)\s*$/im)?.[1];
  return text(fromExtra) || text(item.citationKey) || text(item.data?.citationKey) || text(item.key) || text(item.data?.key);
}

function creatorLabel(creators) {
  const first = Array.isArray(creators) ? creators[0] : undefined;
  if (!first) return "Unknown author";
  const name = text(first.name) || [text(first.firstName), text(first.lastName)].filter(Boolean).join(" ");
  if (!name) return "Unknown author";
  return creators.length > 1 ? `${name} et al.` : name;
}

function itemLabel(item) {
  const data = item.data ?? {};
  const year = text(data.date).match(/^\d{4}/)?.[0];
  const head = year ? `${creatorLabel(data.creators)} (${year})` : creatorLabel(data.creators);
  return `${head} — ${text(data.title) || "Untitled"}`;
}

function searchError(error) {
  return error instanceof Error ? error.message : "Could not search Zotero.";
}

export async function activate(bulletmd) {
  const { StateEffect, StateField } = bulletmd.cm.state;
  const { Decoration, EditorView, ViewPlugin, WidgetType, keymap } = bulletmd.cm.view;
  const settings = await bulletmd.settings.get();
  const library = text(settings.library).replace(/^\/+|\/+$/g, "") || DEFAULT_LIBRARY;
  let activeView = null;
  let openPicker = null;
  const views = new Set();
  const cachedCitations =
    settings.citations && typeof settings.citations === "object" && !Array.isArray(settings.citations)
      ? settings.citations
      : {};
  const resolving = new Set();
  const bibliographyHeights = new Map();
  const bibliographyObservers = new WeakMap();

  // Citations retain their Markdown source on disk. These decorations merely
  // render it as a numbered reference while the cursor is elsewhere.
  const refreshReferences = StateEffect.define();

  class CitationWidget extends WidgetType {
    constructor(number, followBibliography) {
      super();
      this.number = number;
      this.followBibliography = followBibliography;
    }
    eq(other) {
      return other.number === this.number && !!other.followBibliography === !!this.followBibliography;
    }
    toDOM() {
      const ref = document.createElement(this.followBibliography ? "button" : "span");
      ref.className = "bulletmd-zotero-citation";
      ref.textContent = `[${this.number}]`;
      if (this.followBibliography) {
        ref.type = "button";
        ref.title = `Go to bibliography entry ${this.number}`;
        ref.addEventListener("click", (event) => {
          event.preventDefault();
          event.stopPropagation();
          this.followBibliography();
        });
      } else {
        ref.title = `Citation ${this.number}`;
      }
      return ref;
    }
    ignoreEvent() {
      // Keep CodeMirror from moving the selection and rebuilding this widget
      // before the button's click handler can follow the bibliography link.
      return true;
    }
  }

  class BibliographyWidget extends WidgetType {
    constructor(entries, sourcePos, revealSource) {
      super();
      this.entries = entries;
      this.sourcePos = sourcePos;
      this.revealSource = revealSource;
    }
    eq(other) {
      return (
        other.sourcePos === this.sourcePos &&
        !!other.revealSource === !!this.revealSource &&
        JSON.stringify(other.entries) === JSON.stringify(this.entries)
      );
    }
    get estimatedHeight() {
      return (
        bibliographyHeights.get(this.sourcePos) ??
        Math.max(GRID, this.entries.length * GRID)
      );
    }
    toDOM(view) {
      const bibliography = document.createElement("section");
      bibliography.className = "bulletmd-zotero-bibliography";
      bibliography.dataset.sourcePos = String(this.sourcePos);
      bibliography.style.boxSizing = "border-box";
      bibliography.style.position = "relative";
      const surface = document.createElement("div");
      surface.className = "bulletmd-zotero-bibliography-surface";
      surface.style.position = "absolute";
      // Fill the block, top-aligned: no baseline inset and no extra row, so the
      // first entry lands where the comment source sits and the block doesn't
      // shift when toggling source/rendered (mirrors the image widget).
      surface.style.top = "0";
      surface.style.right = "0";
      surface.style.left = "0";
      surface.style.overflow = "hidden";
      surface.style.background = "var(--bg)";
      const knownHeight = bibliographyHeights.get(this.sourcePos);
      if (knownHeight) {
        bibliography.style.height = `${knownHeight}px`;
        surface.style.height = `${knownHeight}px`;
      }
      const content = document.createElement("div");
      content.className = "bulletmd-zotero-bibliography-content";
      bibliography.addEventListener("mousedown", (event) => {
        event.preventDefault();
        event.stopPropagation();
        this.revealSource();
      });
      if (this.entries.length === 0) {
        content.textContent = "No citations in this document.";
      } else {
        for (const entry of this.entries) {
          const line = document.createElement("p");
          line.textContent = `[${entry.number}] ${entry.title}, ${entry.year}, ${entry.authors}`;
          content.append(line);
        }
      }
      surface.append(content);
      bibliography.append(surface);

      let applied = 0;
      const resize = new ResizeObserver(() => {
        const visibleHeight = Math.max(
          GRID,
          Math.ceil(content.scrollHeight / GRID) * GRID,
        );
        if (visibleHeight === applied) return;
        applied = visibleHeight;
        bibliography.style.height = `${visibleHeight}px`;
        surface.style.height = `${visibleHeight}px`;
        bibliographyHeights.set(this.sourcePos, visibleHeight);
        view.requestMeasure();
      });
      resize.observe(content);
      bibliographyObservers.set(bibliography, resize);
      return bibliography;
    }
    destroy(dom) {
      bibliographyObservers.get(dom)?.disconnect();
      bibliographyObservers.delete(dom);
    }
    ignoreEvent() {
      return true;
    }
  }

  function isActive(state, from, to) {
    // Match the editor's live-preview posture: once a cursor enters a source
    // line, its markup is editable. A range selection likewise reveals every
    // line it touches.
    const line = state.doc.lineAt(from);
    return state.selection.ranges.some((range) => range.from <= line.to && range.to >= line.from);
  }

  function referenceData(key) {
    const value = cachedCitations[key];
    if (!value || typeof value !== "object") {
      return { title: key, year: "n.d.", authors: "metadata unavailable" };
    }
    return {
      title: text(value.title) || key,
      year: text(value.year) || "n.d.",
      authors: text(value.authors) || "Unknown author",
    };
  }

  function scanCitations(doc) {
    const occurrences = [];
    const numbers = new Map();
    const re = /\[@([^\]\s;]+)\]/g;
    let match;
    while ((match = re.exec(doc))) {
      const key = match[1];
      if (!numbers.has(key)) numbers.set(key, numbers.size + 1);
      occurrences.push({ from: match.index, to: match.index + match[0].length, key, number: numbers.get(key) });
    }
    return { occurrences, bibliography: [...numbers].map(([key, number]) => ({ number, ...referenceData(key) })) };
  }

  function bibliographyDirectives(doc) {
    const directives = [];
    const directive = /^[ \t]*<!--[ \t]*bibliography[ \t]*-->[ \t]*$/gm;
    let match;
    while ((match = directive.exec(doc))) {
      directives.push({ from: match.index, to: match.index + match[0].length });
    }
    return directives;
  }

  function revealBibliography(view, from, selectSource = false) {
    if (!view) return;
    try {
      if (selectSource) {
        view.dispatch({ selection: { anchor: from }, scrollIntoView: true });
        view.focus();
      } else {
        // Citation navigation must leave the directive unselected, or its
        // live-preview rule would replace the bibliography with raw source.
        view.dispatch({ effects: EditorView.scrollIntoView(from, { y: "center" }) });
      }
    } catch {
      /* view was destroyed between a widget click and its dispatch */
    }
  }

  function decorations(state) {
    const doc = state.doc.toString();
    const { occurrences, bibliography } = scanCitations(doc);
    const directives = bibliographyDirectives(doc);
    const ranges = [];
    for (const citation of occurrences) {
      if (!isActive(state, citation.from, citation.to)) {
        const follow = directives.length ? () => revealBibliography(activeView, directives[0].from) : null;
        ranges.push(Decoration.replace({ widget: new CitationWidget(citation.number, follow) }).range(citation.from, citation.to));
      }
    }
    for (const directive of directives) {
      if (isActive(state, directive.from, directive.to)) {
        const height = bibliographyHeights.get(directive.from);
        if (height) {
          ranges.push(
            Decoration.line({
              attributes: {
                class: "bulletmd-zotero-bibliography-source-active",
                style: `min-height: ${height}px; box-sizing: border-box;`,
              },
            }).range(state.doc.lineAt(directive.from).from),
          );
        }
      } else {
        ranges.push(
          Decoration.replace({
            widget: new BibliographyWidget(bibliography, directive.from, () => revealBibliography(activeView, directive.from, true)),
            block: true,
          }).range(
            directive.from,
            directive.to,
          ),
        );
      }
    }
    ranges.sort((a, b) => a.from - b.from);
    return Decoration.set(ranges, true);
  }

  const referenceField = StateField.define({
    create: decorations,
    update(value, transaction) {
      if (
        transaction.docChanged ||
        transaction.selection ||
        transaction.effects.some((effect) => effect.is(refreshReferences))
      ) {
        return decorations(transaction.state);
      }
      return value;
    },
    provide: (field) => EditorView.decorations.from(field),
  });

  // A rendered bibliography is a single block widget with no interior cursor
  // stop, so a plain arrow steps past it and its source can never be reached.
  // Mirror the core's block-math navigation (`blockNavigation.ts`): when a
  // vertical move would skip a rendered directive, land the caret on its near
  // edge, which reveals the source. It only ever fires on the way *in* --
  // once revealed, the directive is ordinary text the arrows move through.
  function directiveBetween(state, a, b) {
    const lo = Math.min(a, b);
    const hi = Math.max(a, b);
    return bibliographyDirectives(state.doc.toString()).find((d) => d.from >= lo && d.to <= hi) ?? null;
  }

  function enterBibliography(forward) {
    return (view) => {
      const range = view.state.selection.main;
      if (!range.empty) return false;
      const target = view.moveVertically(range, forward).head;
      if (target === range.head) return false; // already at the document edge
      const directive = directiveBetween(view.state, range.head, target);
      // Nothing skipped, or the caret is already on the directive line (it's
      // revealed, so a normal move must carry the caret off it -- inclusive
      // bounds because a one-line directive has no strict interior): move on.
      if (!directive || (range.head >= directive.from && range.head <= directive.to)) return false;
      view.dispatch({ selection: { anchor: forward ? directive.from : directive.to }, scrollIntoView: true });
      return true;
    };
  }

  const bibliographyNavigation = keymap.of([
    { key: "ArrowDown", run: enterBibliography(true) },
    { key: "ArrowUp", run: enterBibliography(false) },
  ]);

  function refreshViews() {
    for (const view of views) {
      try {
        view.dispatch({ effects: refreshReferences.of(null) });
      } catch {
        /* a view may have been destroyed while metadata was saved */
      }
    }
  }

  function rememberCitation(item) {
    const key = citationKey(item);
    if (!key) return;
    const data = item.data ?? {};
    const authors = Array.isArray(data.creators)
      ? data.creators
          .map((creator) => text(creator.name) || [text(creator.firstName), text(creator.lastName)].filter(Boolean).join(" "))
          .filter(Boolean)
          .join(", ")
      : "";
    cachedCitations[key] = {
      title: text(data.title),
      year: text(data.date).match(/^\d{4}/)?.[0] || "",
      authors,
    };
    void bulletmd.settings.set({ ...settings, citations: cachedCitations }).catch(() => {});
    refreshViews();
  }

  function resolveUnknownReferences(state) {
    for (const { key } of scanCitations(state.doc.toString()).occurrences) {
      if (cachedCitations[key] || resolving.has(key)) continue;
      resolving.add(key);
      void lookupCitation(key)
        .then((item) => {
          if (item) rememberCitation(item);
        })
        .catch(() => {
          /* keep the useful placeholder if Zotero is unavailable */
        })
        .finally(() => resolving.delete(key));
    }
  }

  const tracker = ViewPlugin.fromClass(
    class {
      constructor(view) {
        this.view = view;
        views.add(view);
        activeView = view;
        resolveUnknownReferences(view.state);
      }
      update(update) {
        if (update.focusChanged && update.view.hasFocus) activeView = update.view;
        if (update.docChanged) resolveUnknownReferences(update.state);
      }
      destroy() {
        views.delete(this.view);
        if (activeView === this.view) activeView = null;
      }
    },
  );

  async function search(query, signal) {
    const url = new URL(`http://127.0.0.1:23119/api/${library}/items`);
    url.searchParams.set("format", "json");
    url.searchParams.set("q", query);
    url.searchParams.set("qmode", "titleCreatorYear");
    url.searchParams.set("sort", "creator");
    url.searchParams.set("limit", "12");
    url.searchParams.set("v", "3");

    const response = await bulletmd.system.fetch(url, {
      signal,
      headers: { Accept: "application/json", "Zotero-API-Version": "3" },
    });
    if (!response.ok) throw new Error(`Zotero returned ${response.status} ${response.statusText}`);
    const payload = await response.json();
    if (!Array.isArray(payload)) throw new Error("Zotero returned an unexpected response.");
    return payload.filter((item) => item?.data && !EXCLUDED_ITEM_TYPES.has(item.data.itemType) && citationKey(item));
  }

  async function lookupCitation(key) {
    const url = new URL(`http://127.0.0.1:23119/api/${library}/items`);
    url.searchParams.set("format", "json");
    url.searchParams.set("limit", "12");
    url.searchParams.set("v", "3");
    if (/^[A-Za-z0-9]{8}$/.test(key)) {
      url.searchParams.set("itemKey", key);
    } else {
      url.searchParams.set("q", key);
      url.searchParams.set("qmode", "everything");
    }
    const response = await bulletmd.system.fetch(url, {
      headers: { Accept: "application/json", "Zotero-API-Version": "3" },
    });
    if (!response.ok) throw new Error(`Zotero returned ${response.status} ${response.statusText}`);
    const items = await response.json();
    if (!Array.isArray(items)) return null;
    return items.find((item) => citationKey(item) === key) ?? null;
  }

  function insertCitation(view, item) {
    const key = citationKey(item);
    if (!key) return;
    rememberCitation(item);
    const range = view.state.selection.main;
    view.dispatch({
      changes: { from: range.from, to: range.to, insert: `[@${key}]` },
      selection: { anchor: range.from + key.length + 3 },
    });
    view.focus();
  }

  function showPicker() {
    const view = activeView;
    if (!view || openPicker) return;

    const overlay = document.createElement("div");
    overlay.className = "bulletmd-zotero-overlay";
    overlay.setAttribute("role", "presentation");
    const dialog = document.createElement("section");
    dialog.className = "bulletmd-zotero-dialog";
    dialog.setAttribute("role", "dialog");
    dialog.setAttribute("aria-label", "Insert Zotero citation");
    const input = document.createElement("input");
    input.className = "bulletmd-zotero-query";
    input.type = "search";
    input.placeholder = "Search your Zotero library…";
    input.autocomplete = "off";
    const results = document.createElement("div");
    results.className = "bulletmd-zotero-results";
    const hint = document.createElement("p");
    hint.className = "bulletmd-zotero-hint";
    hint.textContent = "Type a title or author to search Zotero.";
    dialog.append(input, results, hint);
    overlay.append(dialog);
    document.body.append(overlay);

    let abort = null;
    let timer = 0;
    let sequence = 0;
    let items = [];
    let selected = 0;

    function close() {
      clearTimeout(timer);
      abort?.abort();
      overlay.remove();
      if (openPicker === close) openPicker = null;
      view.focus();
    }
    openPicker = close;

    function render() {
      results.replaceChildren();
      items.forEach((item, index) => {
        const button = document.createElement("button");
        button.type = "button";
        button.className = "bulletmd-zotero-result";
        if (index === selected) button.dataset.selected = "true";
        const label = document.createElement("span");
        label.className = "bulletmd-zotero-result-label";
        label.textContent = itemLabel(item);
        const key = document.createElement("code");
        key.textContent = `@${citationKey(item)}`;
        button.append(label, key);
        button.addEventListener("click", () => {
          insertCitation(view, item);
          close();
        });
        results.append(button);
      });
    }

    function setHint(message, kind = "") {
      hint.textContent = message;
      hint.dataset.kind = kind;
    }

    function scheduleSearch() {
      clearTimeout(timer);
      abort?.abort();
      const current = ++sequence;
      const query = input.value.trim();
      if (!query) {
        items = [];
        render();
        setHint("Type a title or author to search Zotero.");
        return;
      }
      setHint("Searching Zotero…");
      timer = window.setTimeout(async () => {
        abort = new AbortController();
        try {
          const found = await search(query, abort.signal);
          if (current !== sequence) return;
          items = found;
          selected = 0;
          render();
          setHint(found.length ? `${found.length} result${found.length === 1 ? "" : "s"}` : "No matching library items.");
        } catch (error) {
          if (error instanceof DOMException && error.name === "AbortError") return;
          if (current !== sequence) return;
          items = [];
          render();
          setHint(searchError(error), "error");
        }
      }, 180);
    }

    input.addEventListener("input", scheduleSearch);
    input.addEventListener("keydown", (event) => {
      if (event.key === "Escape") {
        event.preventDefault();
        close();
      } else if (event.key === "ArrowDown" && items.length) {
        event.preventDefault();
        selected = (selected + 1) % items.length;
        render();
      } else if (event.key === "ArrowUp" && items.length) {
        event.preventDefault();
        selected = (selected - 1 + items.length) % items.length;
        render();
      } else if (event.key === "Enter" && items[selected]) {
        event.preventDefault();
        insertCitation(view, items[selected]);
        close();
      }
    });
    overlay.addEventListener("mousedown", (event) => {
      if (event.target === overlay) close();
    });
    input.focus();
  }

  bulletmd.editor.registerExtension([referenceField, tracker, bibliographyNavigation]);
  bulletmd.commands.register({
    id: "zotero.insert-citation",
    title: "Zotero: Insert Citation…",
    run: showPicker,
  });
  bulletmd.menus.addItem({ label: "Insert Zotero Citation…", run: showPicker });
  bulletmd.statusBar.addItem({ id: "zotero", text: "Zotero", title: "Insert a Zotero citation", onClick: showPicker });
}
