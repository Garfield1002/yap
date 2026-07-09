import { describe, it, expect } from "vitest";
import { eventChord, normalizeChord, formatChord } from "./keys";

describe("normalizeChord", () => {
  it("collapses synonyms of the command key to mod", () => {
    expect(normalizeChord("Ctrl+P")).toBe("mod+p");
    expect(normalizeChord("Cmd+P")).toBe("mod+p");
    expect(normalizeChord("CmdOrCtrl+P")).toBe("mod+p");
  });

  it("orders modifiers canonically regardless of how they were written", () => {
    expect(normalizeChord("Shift+Alt+Mod+N")).toBe("mod+alt+shift+n");
  });
});

describe("eventChord", () => {
  it("matches a normalized binding", () => {
    const e = { ctrlKey: true, metaKey: false, altKey: false, shiftKey: true, key: "N" } as KeyboardEvent;
    expect(eventChord(e)).toBe("mod+shift+n");
    expect(eventChord(e)).toBe(normalizeChord("Mod+Shift+N"));
  });

  it("treats meta like ctrl, so a mac Cmd binding still fires", () => {
    const e = { ctrlKey: false, metaKey: true, altKey: false, shiftKey: false, key: "p" } as KeyboardEvent;
    expect(eventChord(e)).toBe(normalizeChord("Mod+P"));
  });
});

describe("formatChord", () => {
  it("renders a human-readable label", () => {
    expect(formatChord("Mod+Alt+F")).toBe("Ctrl+Alt+F");
    expect(formatChord("mod+shift+n")).toBe("Ctrl+Shift+N");
  });
});
