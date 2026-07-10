import { invoke } from "@tauri-apps/api/core";

export interface FileContents {
  text: string;
  mtime_ms: number;
}

export interface AppConfig {
  recent: string[];
  theme: string | null;
  dot_opacity: number;
  plugins_enabled: string[];
}

/** The path named on the command line, or null when bulletmd was launched bare. */
export const getInitialFile = () => invoke<string | null>("get_initial_file");

/** True when launched with `--new` (File > New Window): open an untitled buffer. */
export const getStartUntitled = () => invoke<boolean>("get_start_untitled");

export const readFile = (path: string) => invoke<FileContents>("read_file", { path });

/** Returns the new mtime in epoch milliseconds. */
export const writeFileAtomic = (path: string, contents: string) =>
  invoke<number>("write_file_atomic", { path, contents });

export const deleteFile = (path: string) => invoke<void>("delete_file", { path });

export const renameFile = (from: string, to: string) => invoke<void>("rename_file", { from, to });

export const startWatch = (path: string) => invoke<void>("start_watch", { path });

/** Save clipboard image bytes under BULLETMD_HOME/assets; returns the absolute path. */
export const savePastedImage = (bytes: number[], ext: string) =>
  invoke<string>("save_pasted_image", { bytes, ext });

export const getConfig = () => invoke<AppConfig>("get_config");

/** Persist the theme override; `null` follows the system. */
export const setThemeSetting = (theme: string | null) => invoke<void>("set_theme", { theme });

/** Persist the dot grid opacity, expressed as a fraction from 0 to 0.30. */
export const setDotOpacitySetting = (opacity: number) =>
  invoke<void>("set_dot_opacity", { opacity });

export const recordRecent = (path: string) => invoke<void>("record_recent", { path });

/** Spawn a second editor process showing an untitled buffer (File > New Window). */
export const newWindow = () => invoke<void>("new_window");
