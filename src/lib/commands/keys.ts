//! Keychord parsing and matching, shared by the keybinding layer and the menu /
//! palette accelerator hints.
//!
//! A chord is written "Mod+Shift+P": `Mod` is the platform command key (Ctrl
//! everywhere yap runs today; Cmd if it is ever ported to macOS). Everything is
//! normalized to a canonical `mod+alt+shift+key` string so a written binding and
//! a live event compare as plain strings.

/** Canonical chord for a keyboard event, e.g. "mod+shift+p". */
export function eventChord(e: KeyboardEvent): string {
  const parts: string[] = [];
  if (e.ctrlKey || e.metaKey) parts.push("mod");
  if (e.altKey) parts.push("alt");
  if (e.shiftKey) parts.push("shift");
  parts.push(e.key.toLowerCase());
  return parts.join("+");
}

/** Canonical form of a written binding, so "Ctrl+P" and "mod+p" both match. */
export function normalizeChord(chord: string): string {
  const mods = new Set<string>();
  let key = "";
  for (const part of chord.toLowerCase().split("+")) {
    const p = part.trim();
    if (["mod", "ctrl", "control", "cmd", "meta", "cmdorctrl", "super"].includes(p)) mods.add("mod");
    else if (p === "alt" || p === "option") mods.add("alt");
    else if (p === "shift") mods.add("shift");
    else if (p) key = p;
  }
  const out: string[] = [];
  if (mods.has("mod")) out.push("mod");
  if (mods.has("alt")) out.push("alt");
  if (mods.has("shift")) out.push("shift");
  out.push(key);
  return out.join("+");
}

/** Human-readable label for a chord, e.g. "Ctrl+Shift+P". */
export function formatChord(chord: string): string {
  return normalizeChord(chord)
    .split("+")
    .map((p) => {
      if (p === "mod") return "Ctrl";
      if (p === "alt") return "Alt";
      if (p === "shift") return "Shift";
      return p.length === 1 ? p.toUpperCase() : p.charAt(0).toUpperCase() + p.slice(1);
    })
    .join("+");
}
