import { EditorView, WidgetType } from "@codemirror/view";
import { convertFileSrc } from "@tauri-apps/api/core";

/**
 * Remembered pixel size per resolved src.
 *
 * Rebuilding decorations reconstructs widgets constantly. Without a size to
 * report up front, every rebuild would hand CodeMirror a zero-height image that
 * grows when the file loads, and the document would jump under the cursor.
 */
const GRID = 24;
const DEFAULT_VISIBLE_HEIGHT = GRID * 8;
const DEFAULT_ALLOCATION_HEIGHT = DEFAULT_VISIBLE_HEIGHT + GRID;

interface ImageDimensions {
  width: number;
  height: number;
  visibleHeight?: number;
  allocationHeight?: number;
}

const dimensions = new Map<string, ImageDimensions>();
const observers = new WeakMap<HTMLElement, ResizeObserver>();

const REMOTE = /^(https?:|data:|asset:|blob:)/i;

/** Resolve a markdown image path to something the webview can actually load. */
export function resolveImageSrc(src: string, documentDir: string): string {
  if (REMOTE.test(src)) return src;
  const absolute = src.startsWith("/") ? src : `${documentDir}/${src}`;
  return convertFileSrc(absolute);
}

function resolvedImageSrc(src: string, documentDir: string): string {
  try {
    return resolveImageSrc(src, documentDir);
  } catch {
    return src;
  }
}

/** Last stable grid allocation, used while the source line is being edited. */
export function renderedImageHeight(src: string, documentDir: string): number | undefined {
  return dimensions.get(resolvedImageSrc(src, documentDir))?.allocationHeight;
}

function floorToGrid(value: number): number {
  return Math.max(GRID, Math.floor(value / GRID) * GRID);
}

function ceilToGrid(value: number): number {
  return Math.max(GRID, Math.ceil(value / GRID) * GRID);
}

export class ImageWidget extends WidgetType {
  constructor(
    readonly src: string,
    readonly alt: string,
    readonly documentDir: string,
  ) {
    super();
  }

  /** Same picture, same DOM: never reload the image while the user types. */
  eq(other: ImageWidget): boolean {
    return other.src === this.src && other.alt === this.alt && other.documentDir === this.documentDir;
  }

  get estimatedHeight(): number {
    const resolved = this.resolved();
    return dimensions.get(resolved)?.allocationHeight ?? DEFAULT_ALLOCATION_HEIGHT;
  }

  private resolved(): string {
    try {
      return resolvedImageSrc(this.src, this.documentDir);
    } catch {
      return this.src;
    }
  }

  toDOM(view: EditorView): HTMLElement {
    const wrap = document.createElement("span");
    wrap.className = "cm-image";
    wrap.style.boxSizing = "border-box";
    wrap.style.position = "relative";

    const surface = document.createElement("span");
    surface.className = "cm-image-surface";
    surface.style.position = "absolute";
    surface.style.top = "var(--baseline-block-inset, 18px)";
    surface.style.right = "0";
    surface.style.left = "0";
    surface.style.display = "flex";
    surface.style.alignItems = "center";
    surface.style.justifyContent = "center";
    surface.style.background = "var(--bg)";
    surface.style.overflow = "hidden";

    const img = document.createElement("img");
    img.alt = this.alt;
    img.src = this.resolved();

    const known = dimensions.get(img.src);
    if (known) {
      img.style.aspectRatio = `${known.width} / ${known.height}`;
      if (known.visibleHeight && known.allocationHeight) {
        wrap.style.height = `${known.allocationHeight}px`;
        surface.style.height = `${known.visibleHeight}px`;
        img.style.width = `${known.visibleHeight * (known.width / known.height)}px`;
        img.style.height = `${known.visibleHeight}px`;
      }
    }

    let appliedHeight = 0;
    let scheduled = false;

    const scheduleLayout = () => {
      if (scheduled || !img.naturalWidth || !img.naturalHeight) return;
      scheduled = true;
      view.requestMeasure({
        read: () => {
          const availableWidth = wrap.clientWidth || view.contentDOM.clientWidth;
          const ratio = img.naturalWidth / img.naturalHeight;
          const intrinsicHeight = Math.min(img.naturalWidth, availableWidth) / ratio;
          const pageLimit = view.dom.classList.contains("bulletmd-pdf-page-mode")
            ? GRID * 42
            : view.dom.classList.contains("bulletmd-marp-slide-mode")
              ? floorToGrid(
                  (Number.parseFloat(
                    getComputedStyle(view.dom).getPropertyValue("--bulletmd-marp-slide-height"),
                  ) || 540) - GRID * 4,
                )
              : Number.POSITIVE_INFINITY;
          let height = ceilToGrid(intrinsicHeight);
          // Rounding upward must never push the aspect-ratio-derived width out
          // of the text column. In that case, take the nearest lower row.
          if (height * ratio > availableWidth + 0.5) {
            height = floorToGrid(availableWidth / ratio);
          }
          return Math.min(height, pageLimit);
        },
        write: (height) => {
          scheduled = false;
          if (!height || Math.abs(height - appliedHeight) < 0.5) return;
          appliedHeight = height;
          const ratio = img.naturalWidth / img.naturalHeight;
          const allocation = height + GRID;
          wrap.style.height = `${allocation}px`;
          surface.style.height = `${height}px`;
          img.style.width = `${height * ratio}px`;
          img.style.height = `${height}px`;
          dimensions.set(img.src, {
            width: img.naturalWidth,
            height: img.naturalHeight,
            visibleHeight: height,
            allocationHeight: allocation,
          });
          // The write changed a block widget's allocation. One fresh CM pass
          // records it; observing width (not height) prevents feedback loops.
          view.requestMeasure();
        },
      });
    };

    img.onload = () => {
      const previous = dimensions.get(img.src);
      dimensions.set(img.src, {
        width: img.naturalWidth,
        height: img.naturalHeight,
        visibleHeight: previous?.visibleHeight,
        allocationHeight: previous?.allocationHeight,
      });
      scheduleLayout();
    };
    img.onerror = () => {
      wrap.classList.add("cm-image-broken");
      wrap.style.height = `${GRID * 2}px`;
      surface.style.height = `${GRID}px`;
      surface.textContent = this.alt || this.src;
    };

    surface.appendChild(img);
    wrap.appendChild(surface);
    const observer = new ResizeObserver(scheduleLayout);
    observer.observe(wrap);
    observers.set(wrap, observer);
    if (known) queueMicrotask(scheduleLayout);
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
