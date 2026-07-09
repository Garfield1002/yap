import { EditorView } from "@codemirror/view";
import { savePastedImage } from "../persistence/api";

/** Clipboard MIME type -> file extension for the common image formats. */
const EXTENSIONS: Record<string, string> = {
  "image/png": "png",
  "image/jpeg": "jpg",
  "image/gif": "gif",
  "image/webp": "webp",
  "image/bmp": "bmp",
  "image/svg+xml": "svg",
  "image/tiff": "tiff",
};

function extensionFor(mime: string): string {
  return EXTENSIONS[mime] ?? mime.split("/")[1]?.replace(/\+.*/, "") ?? "png";
}

/** Persist a clipboard image blob and drop an `![](path)` reference at the cursor. */
async function insertImage(blob: Blob, view: EditorView): Promise<void> {
  const bytes = [...new Uint8Array(await blob.arrayBuffer())];
  const path = await savePastedImage(bytes, extensionFor(blob.type));
  view.dispatch(view.state.replaceSelection(`![](${path})`));
}

/** The first image file exposed on a paste event's DataTransfer, if any. */
function imageFromEvent(event: ClipboardEvent): File | null {
  const data = event.clipboardData;
  if (!data) return null;
  for (const item of data.items) {
    if (item.kind === "file" && item.type.startsWith("image/")) {
      const file = item.getAsFile();
      if (file) return file;
    }
  }
  // Some webviews populate `files` but leave `items` empty.
  for (const file of data.files) {
    if (file.type.startsWith("image/")) return file;
  }
  return null;
}

/**
 * Read an image off the async clipboard. WebKitGTK (the Linux webview) usually
 * does *not* put image data on the paste event's DataTransfer -- only text --
 * so this is the path that actually fires on Linux.
 */
async function imageFromClipboard(): Promise<Blob | null> {
  if (!navigator.clipboard?.read) return null;
  const contents = await navigator.clipboard.read();
  for (const item of contents) {
    const type = item.types.find((t) => t.startsWith("image/"));
    if (type) return item.getType(type);
  }
  return null;
}

/**
 * Paste an image from the clipboard: write the bytes into `YAP_HOME/assets/` and
 * drop an `![](absolute/path)` reference at the cursor. Non-image pastes fall
 * through to CodeMirror's default text handling untouched.
 *
 * The absolute path is used so the reference resolves regardless of where the
 * document lives -- the asset dir is shared, not co-located with the note.
 */
export const imagePaste = EditorView.domEventHandlers({
  paste(event, view) {
    // Fast path: the image is already on the event (Chromium-family webviews).
    const file = imageFromEvent(event);
    if (file) {
      event.preventDefault();
      insertImage(file, view).catch((err) => console.error("paste image failed:", err));
      return true;
    }

    // The event carried real text: let CodeMirror paste it normally.
    if (event.clipboardData?.getData("text/plain")) return false;

    // WebKitGTK fallback: nothing usable on the event, so ask the async
    // clipboard. This resolves after the default paste, but an image-only
    // clipboard pastes no text, so there is nothing to double up.
    imageFromClipboard()
      .then((blob) => {
        if (blob) return insertImage(blob, view);
      })
      .catch((err) => console.error("paste image failed:", err));
    return false;
  },
});
