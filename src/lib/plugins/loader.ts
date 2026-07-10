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
import { convertFileSrc } from "@tauri-apps/api/core";
import { fetch as nativeFetch } from "@tauri-apps/plugin-http";

import { registerCommand } from "../commands/registry.svelte";
import { registerBuilder } from "../editor/livePreview/pluginBuilders";
import { registerGrammar } from "../editor/lezer/pluginGrammar";
import { registerExtension } from "../editor/pluginExtensions";
import { markdownToHtml } from "../export/markdownToHtml";
import { documentDirectory } from "../editor/livePreview";
import {
  addStatusItem,
  addPluginMenuItem,
  addPluginError,
  pluginErrors,
} from "./surfaces.svelte";
import { readFile, writeFileAtomic } from "../persistence/api";
import {
  listPlugins,
  readPluginData,
  writePluginData,
  spellLanguages,
  spellCheck,
  spellSuggest,
  type PluginInfo,
} from "./rpc";
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
/** Loading is async (blob import + plugin activation), so `loaded` alone is not
 * enough to protect UI registrations from overlapping enable requests. */
const activating = new Set<string>();

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

/** Print a complete static document in a same-origin frame. CodeMirror only
 *  mounts the viewport, so printing the live editor would silently omit pages. */
function printHtml(html: string, options: { title?: string; css?: string } = {}): Promise<void> {
  return new Promise((resolve, reject) => {
    const frame = document.createElement("iframe");
    frame.title = options.title ?? "Print document";
    frame.style.cssText = "position:fixed;width:1px;height:1px;opacity:0;pointer-events:none;";
    document.body.appendChild(frame);

    const cleanup = () => frame.remove();
    const printWindow = frame.contentWindow;
    const printDocument = frame.contentDocument;
    if (!printWindow || !printDocument) {
      cleanup();
      reject(new Error("could not create print document"));
      return;
    }

    frame.addEventListener(
      "load",
      () => {
        const images = Array.from(printDocument.images).filter((image) => !image.complete);
        Promise.all(
          images.map(
            (image) =>
              new Promise<void>((done) => {
                image.addEventListener("load", () => done(), { once: true });
                image.addEventListener("error", () => done(), { once: true });
              }),
          ),
        ).then(() => {
          printWindow.addEventListener("afterprint", cleanup, { once: true });
          printWindow.print();
          // Some WebKit print paths do not issue afterprint for a cancelled dialog.
          window.setTimeout(cleanup, 60_000);
          resolve();
        });
      },
      { once: true },
    );
    printDocument.open();
    printDocument.write(`<!doctype html><html><head><meta charset="utf-8"><title>${options.title ?? "yap document"}</title><style>${options.css ?? ""}</style></head><body>${html}</body></html>`);
    printDocument.close();
  });
}

/** The PDF path needs the editor's actual math and image behavior, not the
 * intentionally lightweight clipboard converter alone. */
async function printMarkdown(
  markdown: string,
  dir: string,
  options: { title?: string; css?: string; renderLine?: (line: string) => string | undefined } = {},
): Promise<void> {
  const [{ default: katex }, { default: katexCss }] = await Promise.all([
    import("katex"),
    import("katex/dist/katex.min.css?inline"),
  ]);
  const remote = /^(https?:|data:|asset:|blob:)/i;
  const resolveImage = (src: string) => {
    if (remote.test(src)) return src;
    return convertFileSrc(src.startsWith("/") ? src : `${dir}/${src}`);
  };
  const html = markdownToHtml(markdown, {
    resolveImageSrc: resolveImage,
    renderMath: (tex, displayMode) => katex.renderToString(tex, { displayMode, throwOnError: false }),
    renderLine: options.renderLine,
  });
  return printHtml(html, { ...options, css: `${katexCss}\n${options.css ?? ""}` });
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
    editor: { registerExtension: (ext) => track(registerExtension(ext)), documentDirectory },
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
    export: { markdownToHtml, printHtml, printMarkdown },
    system: {
      // Zotero rejects browser origins. Tauri's native HTTP plugin performs the
      // request outside the webview; an empty Origin asks its unsafe-header
      // support to omit the header altogether. Its capability scope allows
      // only Zotero's loopback Local API.
      fetch: (input, init) => {
        const headers = new Headers(init?.headers);
        headers.set("Origin", "");
        return nativeFetch(input, { ...init, headers });
      },
      readFile: async (path) => (await readFile(path)).text,
      writeFile: async (path, contents) => {
        await writeFileAtomic(path, contents);
      },
      spellLanguages,
      spellCheck,
      spellSuggest,
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
  if (loaded.has(info.dir) || activating.has(info.dir)) return;
  activating.add(info.dir);
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
  } finally {
    activating.delete(info.dir);
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
