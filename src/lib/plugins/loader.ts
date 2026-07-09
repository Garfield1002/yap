//! The plugin loader: discover, gate on apiVersion, evaluate in the webview,
//! and track every registration a plugin makes so it can be torn down cleanly.
//!
//! Plugins are trusted code -- no sandbox -- but a *broken* one must never take
//! the app down with it. Each loads inside try/catch; a throw disables just that
//! plugin and surfaces as a status-bar error (see surfaces.svelte). Because the
//! parser and the decoration field are fixed when the editor is created, builder
//! and grammar contributions only take effect on the next editor reload, which
//! the caller triggers after enabling/disabling (see App.svelte).

import * as cmState from "@codemirror/state";
import * as cmView from "@codemirror/view";
import * as cmLanguage from "@codemirror/language";
import * as cmCommands from "@codemirror/commands";
import * as cmSearch from "@codemirror/search";
import * as langMarkdown from "@codemirror/lang-markdown";
import * as lezerMarkdown from "@lezer/markdown";

import { registerCommand } from "../commands/registry.svelte";
import { registerBuilder } from "../editor/livePreview/pluginBuilders";
import { registerGrammar } from "../editor/lezer/pluginGrammar";
import { registerExtension } from "../editor/pluginExtensions";
import {
  addStatusItem,
  addPluginMenuItem,
  addPluginError,
  pluginErrors,
} from "./surfaces.svelte";
import { readFile, writeFileAtomic } from "../persistence/api";
import { listPlugins, readPluginData, writePluginData, type PluginInfo } from "./rpc";
import type { YapApi, PluginModule } from "./types";

/** The API contract version. A plugin whose manifest names a different one is
 *  refused with a clear error rather than loaded into a break. */
export const API_VERSION = 1;

interface Loaded {
  info: PluginInfo;
  /** Tears down every registration this plugin made. */
  dispose: () => void;
}

const loaded = new Map<string, Loaded>();

function runDisposers(disposers: (() => void)[]): void {
  // Reverse order, and never let one failing teardown block the rest.
  for (const d of disposers.reverse()) {
    try {
      d();
    } catch {
      /* a plugin's own teardown threw; nothing more we can do */
    }
  }
}

/** Build the `yap` object for one plugin. Every registration is pushed onto
 *  `disposers` so disabling the plugin fully reverses it. */
function makeApi(info: PluginInfo, disposers: (() => void)[]): YapApi {
  const track = (dispose: () => void) => disposers.push(dispose);
  return {
    cm: {
      state: cmState,
      view: cmView,
      language: cmLanguage,
      commands: cmCommands,
      search: cmSearch,
      markdown: langMarkdown,
      lezerMarkdown,
    },
    commands: { register: (cmd) => track(registerCommand(cmd)) },
    livePreview: { registerBuilder: (names, builder) => track(registerBuilder(names, builder)) },
    markdown: { extendGrammar: (ext) => track(registerGrammar(ext)) },
    editor: { registerExtension: (ext) => track(registerExtension(ext)) },
    menus: {
      addItem: (item) =>
        track(addPluginMenuItem({ id: `${info.dir}:${item.label}`, ...item })),
    },
    statusBar: {
      addItem: (item) => track(addStatusItem({ ...item, id: `${info.dir}:${item.id}` })),
    },
    settings: {
      get: async () => JSON.parse(await readPluginData(info.dir)) as Record<string, unknown>,
      set: (data) => writePluginData(info.dir, JSON.stringify(data, null, 2)),
    },
    system: {
      fetch: (input, init) => fetch(input, init),
      readFile: async (path) => (await readFile(path)).text,
      writeFile: async (path, contents) => {
        await writeFileAtomic(path, contents);
      },
    },
  };
}

/** Evaluate a plugin's ESM source from a blob URL and call its `activate`. */
async function evaluate(info: PluginInfo, yap: YapApi): Promise<void> {
  const url = URL.createObjectURL(new Blob([info.source], { type: "text/javascript" }));
  try {
    const mod = (await import(/* @vite-ignore */ url)) as Partial<PluginModule>;
    if (typeof mod.activate !== "function") {
      throw new Error("entry module does not export activate(yap)");
    }
    await mod.activate(yap);
  } finally {
    URL.revokeObjectURL(url);
  }
}

/** Load a single plugin. Any failure disables just this plugin and records the
 *  error; every registration made before the failure is rolled back. */
async function activate(info: PluginInfo): Promise<void> {
  if (loaded.has(info.dir)) return;
  const disposers: (() => void)[] = [];
  try {
    if (info.error) throw new Error(info.error);
    if (info.api_version !== API_VERSION) {
      throw new Error(`plugin targets API v${info.api_version}; yap provides v${API_VERSION}`);
    }

    if (info.css.trim()) {
      const style = document.createElement("style");
      style.dataset.plugin = info.dir;
      style.textContent = info.css;
      document.head.appendChild(style);
      disposers.push(() => style.remove());
    }

    await evaluate(info, makeApi(info, disposers));
    loaded.set(info.dir, { info, dispose: () => runDisposers(disposers) });
  } catch (e) {
    runDisposers(disposers);
    addPluginError({
      dir: info.dir,
      name: info.name || info.dir,
      message: e instanceof Error ? e.message : String(e),
    });
  }
}

/** Discover all plugins and activate the ones in `enabled`. */
export async function loadEnabledPlugins(enabled: string[]): Promise<void> {
  const set = new Set(enabled);
  for (const info of await listPlugins()) {
    if (set.has(info.dir)) await activate(info);
  }
}

/** Tear a single plugin down (used when the user disables it). */
export function unloadPlugin(dir: string): void {
  const l = loaded.get(dir);
  if (l) {
    l.dispose();
    loaded.delete(dir);
  }
  // Clear any error indicator for this plugin too.
  const i = pluginErrors.findIndex((e) => e.dir === dir);
  if (i >= 0) pluginErrors.splice(i, 1);
}

/** Enable a plugin that is currently off: discover it fresh and activate it. */
export async function loadPlugin(dir: string): Promise<void> {
  if (loaded.has(dir)) return;
  const info = (await listPlugins()).find((p) => p.dir === dir);
  if (info) await activate(info);
}

export function isLoaded(dir: string): boolean {
  return loaded.has(dir);
}
