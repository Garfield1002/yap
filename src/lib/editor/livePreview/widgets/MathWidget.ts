import { EditorView, WidgetType } from "@codemirror/view";

/**
 * KaTeX is ~270 KB of JS plus a stylesheet and fonts. Loading it at startup
 * would tax the very thing that matters most here -- a snappy launch -- for a
 * document that may contain no math at all. So it is imported lazily on the
 * first formula, and until it lands the widget shows its raw source. A math-free
 * file never touches KaTeX.
 */
type Katex = typeof import("katex")["default"];

let katex: Katex | null = null;
let loading: Promise<void> | null = null;
const GRID = 24;
const renderedHeights = new Map<string, number>();
const observers = new WeakMap<HTMLElement, ResizeObserver>();

function widgetKey(tex: string): string {
  return `D ${tex}`;
}

/** Last stable display allocation, used while block-math source is active. */
export function renderedMathHeight(tex: string): number | undefined {
  return renderedHeights.get(widgetKey(tex));
}

function ensureKatex(): Promise<void> {
  if (katex) return Promise.resolve();
  if (!loading) {
    loading = Promise.all([import("katex"), import("katex/dist/katex.min.css")]).then(
      ([mod]) => {
        katex = mod.default;
      },
    );
  }
  return loading;
}

/**
 * Rendered KaTeX HTML, keyed by `displayMode` + source.
 *
 * Rebuilding decorations reconstructs widgets on every keystroke, and KaTeX
 * layout is not cheap. A module-level LRU means the active block (which shows
 * raw source anyway) costs no KaTeX work, and scrolling past a formula reuses
 * the last render. `throwOnError: false` keeps a half-typed formula from
 * throwing; KaTeX renders the error inline in red.
 */
const CACHE_MAX = 300;
const cache = new Map<string, string>();

function render(tex: string, displayMode: boolean): string {
  const key = `${displayMode ? "D" : "I"} ${tex}`;

  const cached = cache.get(key);
  if (cached !== undefined) {
    cache.delete(key); // touch: move to the most-recently-used end
    cache.set(key, cached);
    return cached;
  }

  const html = katex!.renderToString(tex, { displayMode, throwOnError: false, output: "html" });

  cache.set(key, html);
  if (cache.size > CACHE_MAX) cache.delete(cache.keys().next().value as string);
  return html;
}

export class MathWidget extends WidgetType {
  constructor(
    readonly tex: string,
    readonly displayMode: boolean,
  ) {
    super();
  }

  /** Same tex, same mode, same DOM: no KaTeX re-layout while typing nearby. */
  eq(other: MathWidget): boolean {
    return other.tex === this.tex && other.displayMode === this.displayMode;
  }

  get estimatedHeight(): number {
    return this.displayMode ? renderedHeights.get(widgetKey(this.tex)) ?? GRID * 3 : -1;
  }

  toDOM(view: EditorView): HTMLElement {
    const wrap = document.createElement(this.displayMode ? "div" : "span");
    wrap.className = this.displayMode ? "cm-math cm-math-block-grid" : "cm-math cm-math-inline";
    const surface = this.displayMode ? document.createElement("div") : wrap;
    const content = this.displayMode ? document.createElement("div") : wrap;
    if (this.displayMode) {
      wrap.style.position = "relative";
      wrap.style.boxSizing = "border-box";
      surface.className = "cm-math-block";
      surface.style.position = "absolute";
      surface.style.top = "var(--baseline-block-inset, 18px)";
      surface.style.right = "0";
      surface.style.left = "0";
      content.className = "cm-math-block-content";
      surface.style.boxSizing = "border-box";
      surface.style.overflowX = "auto";
      surface.style.overflowY = "hidden";
      surface.style.background = "var(--bg)";
      const knownHeight = renderedHeights.get(widgetKey(this.tex));
      if (knownHeight) {
        wrap.style.height = `${knownHeight}px`;
        surface.style.height = `${Math.max(GRID, knownHeight - GRID)}px`;
      }
      surface.append(content);
      wrap.append(surface);
    }

    if (katex) {
      content.innerHTML = render(this.tex, this.displayMode);
    } else {
      // Show the source until KaTeX arrives; then swap it in and, for block
      // math, ask the view to re-measure the height that just changed.
      content.textContent = this.tex;
      void ensureKatex().then(() => {
        content.innerHTML = render(this.tex, this.displayMode);
        if (this.displayMode) view.requestMeasure();
      });
    }

    if (this.displayMode) {
      let applied = 0;
      const observer = new ResizeObserver(() => {
        const style = getComputedStyle(surface);
        const chrome =
          Number.parseFloat(style.paddingTop) +
          Number.parseFloat(style.paddingBottom) +
          Number.parseFloat(style.borderTopWidth) +
          Number.parseFloat(style.borderBottomWidth);
        const visibleHeight = Math.max(
          GRID,
          Math.ceil((content.scrollHeight + chrome) / GRID) * GRID,
        );
        if (visibleHeight === applied) return;
        applied = visibleHeight;
        const allocationHeight = visibleHeight + GRID;
        wrap.style.height = `${allocationHeight}px`;
        surface.style.height = `${visibleHeight}px`;
        renderedHeights.set(widgetKey(this.tex), allocationHeight);
        view.requestMeasure();
      });
      observer.observe(content);
      observers.set(wrap, observer);
    }

    return wrap;
  }

  destroy(dom: HTMLElement): void {
    observers.get(dom)?.disconnect();
    observers.delete(dom);
  }

  ignoreEvent(): boolean {
    return false;
  }
}
