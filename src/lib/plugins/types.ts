//! The `yap` object handed to a plugin's `activate(yap)`. This is the whole
//! public API surface (v1). It is deliberately a single injected object -- there
//! is no import-resolution magic -- so the app's own CodeMirror module instances
//! ride along on `yap.cm` and plugins build extensions against those rather than
//! importing (and duplicating) `@codemirror/*` themselves.

import type { Command } from "../commands/registry.svelte";
import type { PluginBuilder } from "../editor/livePreview/pluginBuilders";
import type { MarkdownExtension } from "@lezer/markdown";
import type { Extension } from "@codemirror/state";
import type { Facet } from "@codemirror/state";
import type { StatusItem } from "./surfaces.svelte";

export interface YapApi {
  /** The app's own CodeMirror / Lezer module instances. */
  cm: {
    state: typeof import("@codemirror/state");
    view: typeof import("@codemirror/view");
    language: typeof import("@codemirror/language");
    commands: typeof import("@codemirror/commands");
    search: typeof import("@codemirror/search");
    markdown: typeof import("@codemirror/lang-markdown");
    lezerMarkdown: typeof import("@lezer/markdown");
  };
  /** Register a command; it appears in the palette, is placeable in menus, and
   *  can bind a key -- all through the one shared registry. */
  commands: { register(cmd: Command): void };
  /** Join the live-preview decoration pipeline, same contract as the built-in
   *  builders. Takes effect on the next editor reload. */
  livePreview: { registerBuilder(nodeNames: string[] | null, builder: PluginBuilder): void };
  /** Contribute a Lezer `MarkdownConfig` extension. Takes effect on reload. */
  markdown: { extendGrammar(ext: MarkdownExtension): void };
  /** Contribute a raw CM6 extension (keymap, view plugin, facet), built against
   *  `yap.cm.*`. Takes effect on reload. */
  editor: {
    registerExtension(ext: Extension): void;
    /** The open document's directory, for resolving document-relative assets. */
    documentDirectory: Facet<string, string>;
  };
  /** Add an item to the Plugins menu. */
  menus: { addItem(item: { label: string; run: () => void }): void };
  /** Add an item to the status bar. */
  statusBar: { addItem(item: StatusItem): void };
  /** This plugin's own `data.json`. */
  settings: {
    get(): Promise<Record<string, unknown>>;
    set(data: Record<string, unknown>): Promise<void>;
  };
  /** Render markdown using yap's own clipboard-export renderer, then open the
   *  native print dialog for the complete rendered document. */
  export: {
    markdownToHtml(markdown: string): string;
    printHtml(html: string, options?: { title?: string; css?: string }): Promise<void>;
    printMarkdown(
      markdown: string,
      documentDir: string,
      options?: { title?: string; css?: string; renderLine?: (line: string) => string | undefined },
    ): Promise<void>;
  };
  /** Narrow, named escape hatches for system access. Generic file IO plus named,
   *  task-specific commands are exposed instead of a blanket shell capability. */
  system: {
    /** Native HTTP fetch, scoped by Tauri capabilities rather than webview CORS. */
    fetch: typeof fetch;
    readFile(path: string): Promise<string>;
    writeFile(path: string, contents: string): Promise<void>;
    /** Installed spell-check dictionary languages. */
    spellLanguages(): Promise<string[]>;
    /** The subset of `words` misspelled in `lang`. */
    spellCheck(words: string[], lang: string): Promise<string[]>;
    /** Suggested corrections for a single `word`. */
    spellSuggest(word: string, lang: string): Promise<string[]>;
  };
}

/** What a plugin's entry module must export. */
export interface PluginModule {
  activate(yap: YapApi): void | Promise<void>;
}
