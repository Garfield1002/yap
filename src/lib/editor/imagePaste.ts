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

/**
 * Paste an image from the clipboard: write the bytes into `YAP_HOME/assets/`
 * and drop an `![](absolute/path)` reference at the cursor. Non-image pastes
 * fall through to CodeMirror's default text handling untouched.
 *
 * The write is async, but the paste event is not: we grab the file, preventing
 * the default, then insert the markdown once the save resolves. The absolute
 * path is used so the reference resolves regardless of where the document
 * lives -- the asset dir is shared, not co-located with the note.
 */
export const imagePaste = EditorView.domEventHandlers({
  paste(event, view) {
    const items = event.clipboardData?.items;
    if (!items) return false;

    const item = [...items].find(
      (i) => i.kind === "file" && i.type.startsWith("image/"),
    );
    const file = item?.getAsFile();
    if (!file) return false;

    event.preventDefault();
    const ext = extensionFor(file.type);
    file
      .arrayBuffer()
      .then((buf) => savePastedImage([...new Uint8Array(buf)], ext))
      .then((path) => view.dispatch(view.state.replaceSelection(`![](${path})`)))
      .catch((err) => console.error("paste image failed:", err));
    return true;
  },
});
