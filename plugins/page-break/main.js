// Deterministic A4 pager. It measures document lines after subtracting its
// previous spacers, then emits the next complete set in one pass. That keeps
// page gaps in text flow without feeding their own height back into the result.

const PX_PER_MM = 96 / 25.4;
const PAGE_HEIGHT = 297 * PX_PER_MM;
const PAGE_PADDING = 20 * PX_PER_MM;

export function activate(yap) {
  const { StateEffect, StateField } = yap.cm.state;
  const { BlockType, Decoration, EditorView, ViewPlugin, WidgetType } = yap.cm.view;
  const setGaps = StateEffect.define();

  class PageGapWidget extends WidgetType {
    constructor(height, lineOffset, label) {
      super();
      this.height = height;
      this.lineOffset = lineOffset;
      this.label = label;
    }
    eq(other) {
      return other.height === this.height && other.lineOffset === this.lineOffset && other.label === this.label;
    }
    get estimatedHeight() {
      return this.height;
    }
    toDOM() {
      const gap = document.createElement("div");
      gap.className = "yap-page-break-gap";
      gap.style.height = `${this.height}px`;
      gap.style.setProperty("--yap-page-end", `${this.lineOffset}px`);
      if (this.label) {
        const title = document.createElement("span");
        title.className = "yap-page-break-title";
        title.textContent = "Page break";
        gap.appendChild(title);
      }
      return gap;
    }
  }

  const gaps = StateField.define({
    create: () => Decoration.none,
    update(value, transaction) {
      for (const effect of transaction.effects) {
        if (effect.is(setGaps)) {
          const ranges = [];
          for (const entry of effect.value) {
            const widget = new PageGapWidget(entry.height, entry.lineOffset, entry.directive);
            ranges.push(Decoration.widget({ widget, block: true, side: entry.before ? -1 : 1 }).range(entry.at));
            if (entry.directive) {
              ranges.push(Decoration.mark({ class: "yap-page-break-directive" }).range(entry.from, entry.to));
            }
          }
          return Decoration.set(ranges, true);
        }
      }
      return value.map(transaction.changes);
    },
    provide: (field) => EditorView.decorations.from(field),
  });

  // A line that carries block widgets comes back from lineBlockAt as one
  // composite block whose top/bottom span the widgets too. Measure only its
  // text part, so our own spacers never feed back into the arithmetic.
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
        this.resizeObserver = new ResizeObserver(() => this.schedule());
        this.resizeObserver.observe(view.scrollDOM);
        this.schedule();
      }

      update(update) {
        // Geometry changes matter too: lines outside the viewport only have
        // estimated heights, refined as they scroll in. Re-measuring cannot
        // loop, because measurement subtracts our own spacers back out and so
        // always reaches the same fixed point.
        if (update.docChanged || update.geometryChanged || update.transactions.some((tr) => tr.effects.some((e) => !e.is(setGaps)))) {
          this.schedule();
        }
      }

      /** The spacers actually in the document right now, positions kept
       *  current by the state field's own mapping through edits. */
      currentGaps() {
        const entries = [];
        this.view.state.field(gaps).between(0, this.view.state.doc.length, (from, to, deco) => {
          const widget = deco.spec?.widget;
          if (widget instanceof PageGapWidget) {
            entries.push({
              at: from,
              before: deco.spec.side < 0,
              height: widget.height,
              lineOffset: widget.lineOffset,
              directive: !!widget.label,
            });
          }
        });
        return entries;
      }

      schedule() {
        if (this.scheduled) return;
        this.scheduled = true;
        this.view.requestMeasure({
          read: () => this.measurePages(),
          write: (entries) => {
            this.scheduled = false;
            if (sameEntries(entries, this.currentGaps())) return;
            const doc = this.view.state.doc;
            window.setTimeout(() => {
              // An edit in between invalidates the measured offsets; the edit
              // itself already scheduled a fresh measurement.
              if (this.destroyed || this.view.state.doc !== doc) return;
              this.view.dispatch({ effects: setGaps.of(entries) });
            }, 0);
          },
        });
      }

      measurePages() {
        if (!this.view.dom.classList.contains("yap-pdf-page-mode")) return [];

        const old = this.currentGaps();
        let oldIndex = 0;
        let oldHeight = 0;
        const next = [];
        const firstTop = textExtent(this.view.lineBlockAt(0)).top;
        const paperTop = firstTop - PAGE_PADDING;
        let pageEnd = paperTop + PAGE_HEIGHT;
        let inserted = 0;

        for (let number = 1; number <= this.view.state.doc.lines; number++) {
          const line = this.view.state.doc.line(number);
          while (oldIndex < old.length && (old[oldIndex].at < line.from || (old[oldIndex].before && old[oldIndex].at === line.from))) {
            oldHeight += old[oldIndex++].height;
          }
          const block = textExtent(this.view.lineBlockAt(line.from));
          let top = block.top - oldHeight + inserted;
          let bottom = block.bottom - oldHeight + inserted;

          // Keep a whole line out of the bottom margin: break when the line
          // would cross the writable bottom edge, one padding above the
          // physical paper edge at pageEnd. The spacer begins before that
          // line and ends at the next page's writable top edge.
          if (line.from > 0 && bottom > pageEnd - PAGE_PADDING) {
            const height = pageEnd + PAGE_PADDING - top;
            next.push({ at: line.from, from: line.from, to: line.from, before: true, height, lineOffset: pageEnd - top, directive: false });
            inserted += height;
            pageEnd += PAGE_HEIGHT;
            top += height;
            bottom += height;
          }

          if (/^[ \t]*<!--[ \t]*page-break[ \t]*-->[ \t]*$/.test(line.text)) {
            // The source stays visible on the preceding page. Its spacer begins
            // after the line and finishes at the next page's writable top edge.
            const height = pageEnd + PAGE_PADDING - bottom;
            next.push({ at: line.to, from: line.from, to: line.to, before: false, height, lineOffset: pageEnd - bottom, directive: true });
            inserted += height;
            pageEnd += PAGE_HEIGHT;
          }
        }
        return next;
      }

      destroy() {
        this.destroyed = true;
        this.resizeObserver.disconnect();
      }
    },
  );

  // Subpixel measurement noise (device-pixel rounding of rendered spacers)
  // must not count as a change, or every dispatch would schedule the next.
  const EPSILON = 1;
  function sameEntries(a, b) {
    return a.length === b.length && a.every((entry, i) =>
      entry.at === b[i].at && entry.before === b[i].before && entry.directive === b[i].directive &&
      Math.abs(entry.height - b[i].height) < EPSILON &&
      Math.abs(entry.lineOffset - b[i].lineOffset) < EPSILON,
    );
  }

  yap.editor.registerExtension([gaps, pager]);
  yap.statusBar.addItem({ id: "page-break", text: "Page breaks", title: "<!--page-break--> markers are active" });
}
