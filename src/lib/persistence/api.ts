import { invoke } from "@tauri-apps/api/core";

export interface FileContents {
  text: string;
  mtime_ms: number;
}

export interface AppConfig {
  recent: string[];
  theme: string | null;
}

/** The path named on the command line, or null when yap was launched bare. */
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

export const getConfig = () => invoke<AppConfig>("get_config");

/** Persist the theme override; `null` follows the system. */
export const setThemeSetting = (theme: string | null) => invoke<void>("set_theme", { theme });

export const recordRecent = (path: string) => invoke<void>("record_recent", { path });

/** Pop up a native submenu ("file" | "edit" | "settings") under its title-bar
 *  button. `hasPath` gates the File items that only apply to a saved file. */
export const popupMenu = (
  which: "file" | "edit" | "settings",
  hasPath: boolean,
  x: number,
  y: number,
) => invoke<void>("popup_menu", { which, hasPath, x, y });
