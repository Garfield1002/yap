import { EditorView, WidgetType } from "@codemirror/view";
import { convertFileSrc } from "@tauri-apps/api/core";

/**
 * Remembered pixel size per resolved src.
 *
 * Rebuilding decorations reconstructs widgets constantly. Without a size to
 * report up front, every rebuild would hand CodeMirror a zero-height image that
 * grows when the file loads, and the document would jump under the cursor.
 */
const dimensions = new Map<string, { width: number; height: number }>();

const REMOTE = /^(https?:|data:|asset:|blob:)/i;

/** Resolve a markdown image path to something the webview can actually load. */
export function resolveImageSrc(src: string, documentDir: string): string {
  if (REMOTE.test(src)) return src;
  const absolute = src.startsWith("/") ? src : `${documentDir}/${src}`;
  return convertFileSrc(absolute);
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
    return dimensions.get(resolved)?.height ?? 180;
  }

  private resolved(): string {
    try {
      return resolveImageSrc(this.src, this.documentDir);
    } catch {
      return this.src;
    }
  }

  toDOM(view: EditorView): HTMLElement {
    const wrap = document.createElement("span");
    wrap.className = "cm-image";

    const img = document.createElement("img");
    img.alt = this.alt;
    img.src = this.resolved();

    // Reserve space via aspect-ratio, not an explicit pixel width. A width
    // attribute wider than the text column would defeat `max-width: 100%` and
    // push the image past the document width; aspect-ratio lets the CSS cap the
    // rendered width while the height still follows without a layout jump.
    const known = dimensions.get(img.src);
    if (known) {
      img.style.aspectRatio = `${known.width} / ${known.height}`;
    }

    img.onload = () => {
      const size = { width: img.naturalWidth, height: img.naturalHeight };
      const first = !dimensions.has(img.src);
      dimensions.set(img.src, size);
      // Only the first load changes the layout; tell CodeMirror to re-measure
      // rather than letting it discover the new height mid-scroll.
      if (first) view.requestMeasure();
    };
    img.onerror = () => {
      wrap.classList.add("cm-image-broken");
      wrap.textContent = this.alt || this.src;
    };

    wrap.appendChild(img);
    return wrap;
  }

  ignoreEvent(): boolean {
    return false;
  }
}
