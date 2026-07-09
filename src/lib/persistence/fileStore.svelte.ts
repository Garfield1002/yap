/** Everything the chrome (title bar, status bar, dialog) needs to know. */
export const fileState = $state({
  path: null as string | null,
  dirty: false,
  saving: false,
  /** Set when the file changed on disk while the buffer had unsaved edits. */
  conflict: false,
  /** Disk text at the moment the conflict was detected. */
  conflictText: null as string | null,
  error: null as string | null,
});

export function basename(path: string | null): string {
  if (!path) return "untitled";
  return path.slice(path.lastIndexOf("/") + 1);
}
