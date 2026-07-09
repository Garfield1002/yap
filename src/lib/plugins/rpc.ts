import { invoke } from "@tauri-apps/api/core";

/** A plugin as discovered on disk. `error` is set (and `source` empty) when the
 *  manifest or entry file could not be read. */
export interface PluginInfo {
  dir: string;
  name: string;
  version: string;
  api_version: number;
  source: string;
  css: string;
  error: string | null;
}

/** Discover every plugin under `$YAP_HOME/plugins/`. */
export const listPlugins = () => invoke<PluginInfo[]>("list_plugins");

/** Read a plugin's `data.json` as a raw JSON string (`{}` when absent). */
export const readPluginData = (dir: string) => invoke<string>("read_plugin_data", { dir });

export const writePluginData = (dir: string, contents: string) =>
  invoke<void>("write_plugin_data", { dir, contents });

export const setPluginsEnabled = (enabled: string[]) =>
  invoke<void>("set_plugins_enabled", { enabled });
