import { beforeEach, describe, expect, it, vi } from "vitest";

const writes: string[] = [];
let resolveWrite: (() => void) | null = null;
let blockWrites = false;

vi.mock("./api", () => ({
  writeFileAtomic: vi.fn(async (_path: string, contents: string) => {
    writes.push(contents);
    if (blockWrites) await new Promise<void>((r) => (resolveWrite = r));
    return 1;
  }),
}));

const { Autosave } = await import("./autosave");

const tick = () => new Promise((r) => setTimeout(r, 0));

beforeEach(() => {
  writes.length = 0;
  resolveWrite = null;
  blockWrites = false;
});

describe("Autosave", () => {
  it("debounces a burst of keystrokes into a single write", async () => {
    vi.useFakeTimers();
    const save = new Autosave({ path: "/n.md", delayMs: 1000 }, "");
    save.schedule("a");
    save.schedule("ab");
    save.schedule("abc");

    expect(writes).toEqual([]);
    await vi.advanceTimersByTimeAsync(1000);
    vi.useRealTimers();
    await tick();

    expect(writes).toEqual(["abc"]);
  });

  it("does not write until the debounce elapses", async () => {
    vi.useFakeTimers();
    const save = new Autosave({ path: "/n.md", delayMs: 1000 }, "");
    save.schedule("a");
    await vi.advanceTimersByTimeAsync(999);
    expect(writes).toEqual([]);
    vi.useRealTimers();
  });

  it("flush() writes immediately without waiting for the timer", async () => {
    const save = new Autosave({ path: "/n.md", delayMs: 10_000 }, "");
    save.schedule("typed");
    await save.flush();
    expect(writes).toEqual(["typed"]);
  });

  it("flush() is a no-op when nothing is pending", async () => {
    const save = new Autosave({ path: "/n.md" }, "same");
    await save.flush();
    expect(writes).toEqual([]);
  });

  it("never overlaps writes, and the last text wins", async () => {
    blockWrites = true;
    const save = new Autosave({ path: "/n.md", delayMs: 0 }, "");

    save.schedule("first");
    await tick();
    expect(writes).toEqual(["first"]);

    // Two more edits arrive while the first write is still in flight.
    save.schedule("second");
    save.schedule("third");
    await tick();
    expect(writes, "no second write may start mid-flight").toEqual(["first"]);

    blockWrites = false;
    resolveWrite?.();
    await tick();
    await tick();

    expect(writes, "the drain writes only the newest text").toEqual(["first", "third"]);
  });

  it("tracks the last saved text so the watcher can spot its own echo", async () => {
    const save = new Autosave({ path: "/n.md", delayMs: 0 }, "initial");
    expect(save.lastSavedText).toBe("initial");
    save.schedule("edited");
    await save.flush();
    expect(save.lastSavedText).toBe("edited");
  });

  it("writes nothing while suspended, and resumes afterwards", async () => {
    const save = new Autosave({ path: "/n.md", delayMs: 0 }, "");
    save.suspend();
    save.schedule("blocked");
    await save.flush();
    expect(writes).toEqual([]);

    save.resume();
    save.schedule("allowed");
    await save.flush();
    expect(writes).toEqual(["allowed"]);
  });

  it("reset() drops pending text and adopts the given text as on-disk", async () => {
    const save = new Autosave({ path: "/n.md", delayMs: 10_000 }, "old");
    save.schedule("mine");
    save.reset("theirs");
    await save.flush();

    expect(writes, "reset must cancel the pending write").toEqual([]);
    expect(save.lastSavedText).toBe("theirs");
  });

  it("reports errors and stays usable", async () => {
    const api = await import("./api");
    const onError = vi.fn();
    vi.mocked(api.writeFileAtomic).mockRejectedValueOnce(new Error("disk full"));

    const save = new Autosave({ path: "/n.md", delayMs: 0, onError }, "");
    save.schedule("boom");
    await save.flush();
    expect(onError).toHaveBeenCalledOnce();

    save.schedule("ok");
    await save.flush();
    expect(writes).toEqual(["ok"]);
  });

  it("signals idle only once the drain is finished", async () => {
    const onIdle = vi.fn();
    blockWrites = true;
    const save = new Autosave({ path: "/n.md", delayMs: 0, onIdle }, "");

    save.schedule("one");
    await tick();
    save.schedule("two");
    await tick();
    expect(onIdle).not.toHaveBeenCalled();

    blockWrites = false;
    resolveWrite?.();
    await tick();
    await tick();
    expect(onIdle).toHaveBeenCalledOnce();
  });
});
