import { invoke } from "@tauri-apps/api/core";

export interface FileContents {
  text: string;
  mtime_ms: number;
}

/** The path named on the command line, or null when yap was launched bare. */
export const getInitialFile = () => invoke<string | null>("get_initial_file");

export const readFile = (path: string) => invoke<FileContents>("read_file", { path });

/** Returns the new mtime in epoch milliseconds. */
export const writeFileAtomic = (path: string, contents: string) =>
  invoke<number>("write_file_atomic", { path, contents });

export const startWatch = (path: string) => invoke<void>("start_watch", { path });
