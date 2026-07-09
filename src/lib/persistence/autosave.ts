import { writeFileAtomic } from "./api";

export interface AutosaveOptions {
  path: string;
  /** Quiet period after the last keystroke before writing. */
  delayMs?: number;
  onSaveStart?: () => void;
  onSaved?: (text: string) => void;
  /** No write is in flight any more, successfully or not. */
  onIdle?: () => void;
  onError?: (error: unknown) => void;
}

/**
 * Debounced, atomic autosave with an in-flight guard.
 *
 * Two invariants matter here. Writes never overlap: a save requested while one
 * is in flight sets `pending` instead, and the running save loops to pick it up,
 * so the last text always wins and the file is never written out of order.
 * And `flush()` exists so the close handler can get the last keystrokes onto
 * disk before the window goes away -- that is the whole data-loss guard.
 */
export class Autosave {
  private timer: ReturnType<typeof setTimeout> | null = null;
  private inFlight: Promise<void> | null = null;
  private pending: string | null = null;
  private suspended = false;

  /**
   * The text of our most recent successful write. The file watcher compares
   * against this to recognise the echo of our own save.
   */
  lastSavedText: string;

  constructor(
    private readonly options: AutosaveOptions,
    initialText: string,
  ) {
    this.lastSavedText = initialText;
  }

  /** Note new buffer text and (re)start the debounce timer. */
  schedule(text: string): void {
    this.pending = text;
    if (this.timer) clearTimeout(this.timer);
    this.timer = setTimeout(() => void this.save(), this.options.delayMs ?? 1000);
  }

  /** Stop writing. Used while a conflict dialog owns the file's fate. */
  suspend(): void {
    this.suspended = true;
    this.cancelTimer();
  }

  resume(): void {
    this.suspended = false;
  }

  /** Forget unsaved text and treat `text` as what is on disk. */
  reset(text: string): void {
    this.cancelTimer();
    this.pending = null;
    this.lastSavedText = text;
  }

  /** Write now and wait for the disk. Safe to call when nothing is pending. */
  async flush(): Promise<void> {
    this.cancelTimer();
    await this.save();
    while (this.inFlight) await this.inFlight;
  }

  private cancelTimer(): void {
    if (this.timer) {
      clearTimeout(this.timer);
      this.timer = null;
    }
  }

  private save(): Promise<void> {
    this.cancelTimer();
    if (this.suspended || this.pending === null) return Promise.resolve();
    // A save is already running; it will pick up `pending` before it returns.
    if (this.inFlight) return this.inFlight;

    this.options.onSaveStart?.();

    const run = async () => {
      // Drain: text that arrives mid-write is written by the next iteration
      // rather than racing this one.
      while (this.pending !== null && !this.suspended) {
        const text = this.pending;
        this.pending = null;
        await writeFileAtomic(this.options.path, text);
        this.lastSavedText = text;
        this.options.onSaved?.(text);
      }
    };

    this.inFlight = run()
      .catch((error) => this.options.onError?.(error))
      .finally(() => {
        this.inFlight = null;
        this.options.onIdle?.();
      });

    return this.inFlight;
  }
}
