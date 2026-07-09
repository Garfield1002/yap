//! Reactive UI surfaces plugins contribute to: status-bar items, the Plugins
//! menu, and the load-error list that drives the broken-plugin indicator. These
//! are reactive (unlike the editor builders) because they render into live Svelte
//! chrome that must update the moment a plugin is enabled, disabled, or fails.

export interface StatusItem {
  id: string;
  text: string;
  title?: string;
  onClick?: () => void;
}

export interface PluginMenuItem {
  id: string;
  label: string;
  run: () => void;
}

export interface PluginError {
  /** Plugin directory name. */
  dir: string;
  name: string;
  message: string;
}

export const statusItems = $state<StatusItem[]>([]);
export const pluginMenuItems = $state<PluginMenuItem[]>([]);
export const pluginErrors = $state<PluginError[]>([]);

function remover<T>(list: T[], item: T): () => void {
  return () => {
    const i = list.indexOf(item);
    if (i >= 0) list.splice(i, 1);
  };
}

/** Add a status-bar item; returns a disposer for per-plugin teardown. */
export function addStatusItem(item: StatusItem): () => void {
  statusItems.push(item);
  return remover(statusItems, item);
}

/** Add a Plugins-menu item; returns a disposer for per-plugin teardown. */
export function addPluginMenuItem(item: PluginMenuItem): () => void {
  pluginMenuItems.push(item);
  return remover(pluginMenuItems, item);
}

/** Record a plugin that failed to load; returns a disposer that clears it. */
export function addPluginError(err: PluginError): () => void {
  pluginErrors.push(err);
  return remover(pluginErrors, err);
}
