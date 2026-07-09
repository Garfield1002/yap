//! The one command registry. Core actions and plugins register here; the
//! command palette, the title-bar menus, and the keybinding layer all read from
//! it, so a command declared once is reachable from every surface uniformly.
//!
//! Reactivity: registrations bump a `$state` version counter that the reactive
//! readers (`listCommands`) touch, so a palette or menu built with `$derived`
//! re-runs when the set of commands changes. (A plain reactive Map would do too,
//! but the counter keeps the store a plain Map that non-Svelte code can read.)

import { eventChord, normalizeChord } from "./keys";

export interface Command {
  /** Stable, unique id, e.g. "file.save" or "spellcheck.toggle". */
  id: string;
  /** Shown in the palette and (as the label) in menus. */
  title: string;
  run: () => void | Promise<void>;
  /** Functional chord handled by the global keybinding layer, e.g. "Mod+P".
   *  Leave unset for commands whose key is owned by CodeMirror. */
  keybinding?: string;
  /** Display-only accelerator hint when the key is handled elsewhere (e.g.
   *  CodeMirror's own keymap). Defaults to `keybinding` when that is set. */
  accelerator?: string;
  /** Gate; a disabled command is dimmed in menus and inert to its key. */
  enabled?: () => boolean;
  /** Hide from the command palette (still runnable and bindable). */
  hidden?: boolean;
}

let version = $state(0);
const commands = new Map<string, Command>();

/** Register (or replace) a command. Returns a disposer that unregisters it,
 *  which the plugin loader uses for per-plugin teardown. */
export function registerCommand(cmd: Command): () => void {
  commands.set(cmd.id, cmd);
  version++;
  return () => unregisterCommand(cmd.id);
}

export function unregisterCommand(id: string): void {
  if (commands.delete(id)) version++;
}

export function getCommand(id: string): Command | undefined {
  return commands.get(id);
}

/** All commands, newest registration last. Reactive: reads the version counter
 *  so `$derived`/`$effect` consumers re-run when the set changes. */
export function listCommands(): Command[] {
  version;
  return [...commands.values()];
}

export function isEnabled(cmd: Command): boolean {
  return cmd.enabled?.() !== false;
}

/** Run a command by id, unless it is disabled. */
export async function runCommand(id: string): Promise<void> {
  const cmd = commands.get(id);
  if (cmd && isEnabled(cmd)) await cmd.run();
}

/** Try to dispatch a keyboard event to a bound command. Returns true (and
 *  runs the command, after preventDefault) when one matches. Commands without
 *  a `keybinding` are ignored here, so keys owned by CodeMirror fall through. */
export function dispatchKey(e: KeyboardEvent): boolean {
  const chord = eventChord(e);
  for (const cmd of commands.values()) {
    if (cmd.keybinding && normalizeChord(cmd.keybinding) === chord && isEnabled(cmd)) {
      e.preventDefault();
      void cmd.run();
      return true;
    }
  }
  return false;
}
