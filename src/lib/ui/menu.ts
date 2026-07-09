/** The menu model the title bar renders. Built in App.svelte, where the actions
 *  and the current state (open path, recent files, theme) all live. When the
 *  command registry lands (build step 2), `run` callbacks come from it; the
 *  shape here does not need to change. */

export interface MenuAction {
  type: "action";
  id: string;
  label: string;
  /** Display-only accelerator, e.g. "Ctrl+S". Actual key handling is global
   *  (see App.svelte); this string is just the hint shown on the right. */
  accelerator?: string;
  enabled?: boolean;
  checked?: boolean;
  run: () => void;
}

export interface MenuSeparator {
  type: "separator";
}

export interface MenuSubmenu {
  type: "submenu";
  label: string;
  enabled?: boolean;
  items: MenuItem[];
}

export type MenuItem = MenuAction | MenuSeparator | MenuSubmenu;

/** One top-level menu in the title bar. */
export interface Menu {
  id: string;
  label: string;
  items: MenuItem[];
}
