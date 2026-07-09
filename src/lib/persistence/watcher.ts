import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { readFile, startWatch } from "./api";

export interface WatchHandlers {
  /** The buffer's current text, used to recognise a no-op change. */
  currentText: () => string;
  /** The text of our own most recent save, used to recognise our echo. */
  lastSavedText: () => string;
  /** True when the buffer holds edits that are not on disk. */
  isDirty: () => boolean;
  /** Disk changed under a clean buffer: adopt it silently. */
  onReload: (text: string) => void;
  /** Disk changed under a dirty buffer: the user has to choose. */
  onConflict: (diskText: string) => void;
  onError?: (error: unknown) => void;
}

/**
 * Reload-on-external-change.
 *
 * Echo detection compares *content*, not mtime. Every save we make fires the
 * watcher, and an mtime check would race against the write we just did; the
 * text either matches what we last wrote or it does not.
 */
export async function watchFile(path: string, handlers: WatchHandlers): Promise<UnlistenFn> {
  const unlisten = await listen("file-changed", async () => {
    try {
      const { text } = await readFile(path);

      // Our own save landing, or a write that changed nothing.
      if (text === handlers.lastSavedText() || text === handlers.currentText()) return;

      if (handlers.isDirty()) handlers.onConflict(text);
      else handlers.onReload(text);
    } catch (error) {
      handlers.onError?.(error);
    }
  });

  await startWatch(path);
  return unlisten;
}
