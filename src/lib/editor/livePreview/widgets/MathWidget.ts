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

  toDOM(view: EditorView): HTMLElement {
    const wrap = document.createElement(this.displayMode ? "div" : "span");
    wrap.className = this.displayMode ? "cm-math cm-math-block" : "cm-math cm-math-inline";

    if (katex) {
      wrap.innerHTML = render(this.tex, this.displayMode);
    } else {
      // Show the source until KaTeX arrives; then swap it in and, for block
      // math, ask the view to re-measure the height that just changed.
      wrap.textContent = this.tex;
      void ensureKatex().then(() => {
        wrap.innerHTML = render(this.tex, this.displayMode);
        if (this.displayMode) view.requestMeasure();
      });
    }

    return wrap;
  }

  ignoreEvent(): boolean {
    return false;
  }
}
