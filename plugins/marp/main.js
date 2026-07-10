// Marp WYSIWYG mode keeps editing inside CodeMirror. It lays the document out
// as 16:9 slide canvases, treats thematic `---` lines as slide boundaries, and
// collapses Marp front matter unless the cursor is editing it.

const ASPECT_HEIGHT = 9 / 16;
const SLIDE_GAP = 28;
const SLIDE_BASELINE = 48;
const SLIDE_BOTTOM_PADDING = 48;
const GRID = 24;

function slideDotPattern(slideHeight) {
  const dots = [];
  for (let y = SLIDE_BASELINE; y <= slideHeight - SLIDE_BOTTOM_PADDING; y += GRID) {
    dots.push(`radial-gradient(circle at 1px ${y}px, currentColor 1px, transparent 1.15px)`);
  }
  return dots.join(",");
}

export async function activate(bulletmd) {
  const { StateEffect, StateField } = bulletmd.cm.state;
  const { BlockType, Decoration, EditorView, ViewPlugin, WidgetType } = bulletmd.cm.view;
  const settings = await bulletmd.settings.get();
  let slideMode = settings.slideMode !== false;
  let exporting = false;
  const views = new Set();

  const setSlideMode = StateEffect.define();
  const setGaps = StateEffect.define();

  const slideModeField = StateField.define({
    create: () => slideMode,
    update(value, transaction) {
      for (const effect of transaction.effects) {
        if (effect.is(setSlideMode)) return effect.value;
      }
      return value;
    },
    provide: (field) =>
      EditorView.editorAttributes.from(field, (enabled) =>
        enabled ? { class: "bulletmd-marp-slide-mode" } : {},
      ),
  });

  class SlideGapWidget extends WidgetType {
    constructor(height) {
      super();
      this.height = height;
    }
    eq(other) {
      return Math.abs(other.height - this.height) < 1;
    }
    get estimatedHeight() {
      return this.height;
    }
    toDOM() {
      const gap = document.createElement("div");
      gap.className = "bulletmd-marp-slide-gap";
      gap.style.height = `${this.height}px`;
      return gap;
    }
  }

  function scanDeck(doc) {
    const separators = [];
    const frontmatterLines = [];
    let frontmatter = null;
    let firstContentLine = 1;

    if (doc.lines > 1 && /^\s*---\s*$/.test(doc.line(1).text)) {
      for (let number = 2; number <= doc.lines; number++) {
        const line = doc.line(number);
        if (/^\s*---\s*$/.test(line.text)) {
          frontmatter = { from: doc.line(1).from, to: line.to };
          for (let n = 1; n <= number; n++) frontmatterLines.push(doc.line(n));
          firstContentLine = number + 1;
          break;
        }
      }
    }

    for (let number = firstContentLine; number <= doc.lines; number++) {
      const line = doc.line(number);
      if (/^\s*---\s*$/.test(line.text)) separators.push(line);
    }
    return { separators, frontmatter, frontmatterLines };
  }

  function selectionTouches(state, from, to) {
    return state.selection.ranges.some((range) => range.from <= to && range.to >= from);
  }

  function currentEntries(decorations) {
    const entries = [];
    const iterator = decorations.iter();
    while (iterator.value) {
      if (iterator.value.spec?.widget instanceof SlideGapWidget) {
        entries.push({ at: iterator.from, height: iterator.value.spec.widget.height });
      }
      iterator.next();
    }
    return entries;
  }

  function decorate(state, entries) {
    const ranges = [];
    const deck = scanDeck(state.doc);
    const editingFrontmatter =
      deck.frontmatter && selectionTouches(state, deck.frontmatter.from, deck.frontmatter.to);
    for (const line of deck.frontmatterLines) {
      ranges.push(
        Decoration.line({
          attributes: {
            class: editingFrontmatter
              ? "bulletmd-marp-frontmatter bulletmd-marp-source-active"
              : "bulletmd-marp-frontmatter",
          },
        }).range(line.from),
      );
    }

    const byPosition = new Map(entries.map((entry) => [entry.at, entry]));
    for (const line of deck.separators) {
      const active = selectionTouches(state, line.from, line.to);
      ranges.push(
        Decoration.line({
          attributes: {
            class: active
              ? "bulletmd-marp-separator bulletmd-marp-source-active"
              : "bulletmd-marp-separator",
          },
        }).range(line.from),
      );
      const entry = byPosition.get(line.to);
      if (entry) {
        ranges.push(
          Decoration.widget({ widget: new SlideGapWidget(entry.height), block: true, side: 1 }).range(
            line.to,
          ),
        );
      }
    }
    return Decoration.set(ranges, true);
  }

  const gaps = StateField.define({
    create: (state) => decorate(state, []),
    update(value, transaction) {
      let mapped = value.map(transaction.changes);
      let entries = currentEntries(mapped);
      for (const effect of transaction.effects) {
        if (effect.is(setGaps)) entries = effect.value;
      }
      return decorate(transaction.state, entries);
    },
    provide: (field) => EditorView.decorations.from(field),
  });

  function textExtent(block) {
    if (!Array.isArray(block.type)) return block;
    const parts = block.type.filter((part) => part.type === BlockType.Text);
    return parts.length ? { top: parts[0].top, bottom: parts[parts.length - 1].bottom } : block;
  }

  const pager = ViewPlugin.fromClass(
    class {
      constructor(view) {
        this.view = view;
        this.scheduled = false;
        this.destroyed = false;
        views.add(view);
        this.resizeObserver = new ResizeObserver(() => this.schedule());
        this.resizeObserver.observe(view.contentDOM);
        this.schedule();
      }
      update(update) {
        if (update.docChanged || update.geometryChanged || update.transactions.some((tr) => tr.effects.some((effect) => !effect.is(setGaps)))) {
          this.schedule();
        }
      }
      schedule() {
        if (this.scheduled) return;
        this.scheduled = true;
        this.view.requestMeasure({
          read: () => this.measure(),
          write: (entries) => {
            this.scheduled = false;
            const current = currentEntries(this.view.state.field(gaps));
            const same = entries.length === current.length && entries.every((entry, i) =>
              entry.at === current[i].at && Math.abs(entry.height - current[i].height) < 1,
            );
            if (!same && !this.destroyed) this.view.dispatch({ effects: setGaps.of(entries) });
          },
        });
      }
      measure() {
        if (!this.view.dom.classList.contains("bulletmd-marp-slide-mode")) return [];
        const width = this.view.contentDOM.getBoundingClientRect().width;
        const slideHeight = width * ASPECT_HEIGHT;
        this.view.dom.style.setProperty("--bulletmd-marp-slide-height", `${slideHeight}px`);
        this.view.contentDOM.style.setProperty("--bulletmd-marp-dot-pattern", slideDotPattern(slideHeight));
        this.view.contentDOM.style.setProperty(
          "--bulletmd-marp-page-stride",
          `${slideHeight + SLIDE_GAP}px`,
        );
        const old = currentEntries(this.view.state.field(gaps));
        let removed = 0;
        let oldIndex = 0;
        let inserted = 0;
        const firstTop = textExtent(this.view.lineBlockAt(0)).top;
        const contentTopPadding =
          Number.parseFloat(getComputedStyle(this.view.contentDOM).paddingTop) || GRID;
        let slideTop = firstTop - contentTopPadding;
        const entries = [];

        for (const line of scanDeck(this.view.state.doc).separators) {
          while (oldIndex < old.length && old[oldIndex].at < line.from) {
            removed += old[oldIndex++].height;
          }
          const bottom = textExtent(this.view.lineBlockAt(line.from)).bottom - removed + inserted;
          const target = slideTop + slideHeight + SLIDE_GAP + contentTopPadding;
          const height = Math.max(SLIDE_GAP + contentTopPadding, target - bottom);
          entries.push({ at: line.to, height });
          inserted += height;
          slideTop += slideHeight + SLIDE_GAP;
        }
        return entries;
      }
      destroy() {
        this.destroyed = true;
        this.resizeObserver.disconnect();
        this.view.contentDOM.style.removeProperty("--bulletmd-marp-dot-pattern");
        this.view.contentDOM.style.removeProperty("--bulletmd-marp-page-stride");
        this.view.dom.style.removeProperty("--bulletmd-marp-slide-height");
        views.delete(this.view);
      }
    },
  );

  function toggleSlideMode() {
    slideMode = !slideMode;
    for (const view of views) {
      view.dispatch({
        effects: [setSlideMode.of(slideMode), ...(slideMode ? [] : [setGaps.of([])])],
      });
    }
    void bulletmd.settings.set({ ...settings, slideMode });
  }

  function deckName(markdown) {
    const title = markdown.match(/^#\s+(.+)$/m)?.[1] ?? "slides";
    return title.toLowerCase().replace(/[^\p{L}\p{N}]+/gu, "-").replace(/^-+|-+$/g, "") || "slides";
  }

  async function exportDeck(format) {
    if (exporting) return;
    const view = views.values().next().value;
    if (!view) return;
    const markdown = view.state.doc.toString();
    const dir = view.state.facet(bulletmd.editor.documentDirectory);
    const filename = `${deckName(markdown)}.${format}`;
    const output = await bulletmd.dialogs.save({
      title: `Export Marp ${format.toUpperCase()}`,
      defaultPath: dir ? `${dir}/${filename}` : filename,
      filters: [{ name: `Marp ${format.toUpperCase()}`, extensions: [format] }],
    });
    if (!output) return;
    exporting = true;
    try {
      await bulletmd.system.marpExport(markdown, output, dir, settings.allowLocalFiles === true);
      alert(`Marp export saved to ${output}`);
    } catch (error) {
      alert(`Marp export failed: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      exporting = false;
    }
  }

  bulletmd.editor.registerExtension([slideModeField, gaps, pager]);
  bulletmd.commands.register({
    id: "marp.toggle-slide-mode",
    title: "Marp: Toggle Slide Mode",
    keybinding: "Mod+Shift+M",
    run: toggleSlideMode,
  });
  bulletmd.menus.addItem({ label: "Toggle Marp Slide Mode", run: toggleSlideMode });
  for (const [format, label] of [["html", "HTML"], ["pdf", "PDF"], ["pptx", "PowerPoint"]]) {
    const run = () => void exportDeck(format);
    bulletmd.commands.register({ id: `marp.export-${format}`, title: `Marp: Export ${label}…`, run });
    bulletmd.menus.addItem({ label: `Export Marp ${label}…`, run });
  }
  bulletmd.statusBar.addItem({
    id: "marp",
    text: "Marp: slides",
    title: "Toggle inline 16:9 slide layout",
    onClick: toggleSlideMode,
  });
}
