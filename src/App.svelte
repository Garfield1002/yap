<script lang="ts">
  import { onMount } from "svelte";
  import { EditorView } from "@codemirror/view";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { open } from "@tauri-apps/plugin-dialog";

  import { createEditor } from "./lib/editor/createEditor";
  import { getInitialFile, readFile } from "./lib/persistence/api";
  import { Autosave } from "./lib/persistence/autosave";
  import { watchFile } from "./lib/persistence/watcher";
  import { basename, dirname, fileState } from "./lib/persistence/fileStore.svelte";
  import StatusBar from "./lib/ui/StatusBar.svelte";
  import ConflictDialog from "./lib/ui/ConflictDialog.svelte";

  let host: HTMLElement;
  let view: EditorView | undefined;
  let autosave: Autosave | undefined;
  let unlistenWatch: UnlistenFn | undefined;
  let unlistenClose: UnlistenFn | undefined;

  /** Set while we rewrite the buffer from disk, so it is not mistaken for typing. */
  let applyingExternalChange = false;

  const win = getCurrentWindow();

  function refreshTitle() {
    void win.setTitle(`${basename(fileState.path)}${fileState.dirty ? " •" : ""}`);
  }

  /** Replace the buffer with `text` via a change, not a new state: undo history
   *  and the cursor both survive. */
  function replaceBuffer(text: string) {
    if (!view) return;
    applyingExternalChange = true;
    try {
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text } });
    } finally {
      applyingExternalChange = false;
    }
  }

  function onDocChange(text: string) {
    if (applyingExternalChange || !autosave) return;
    fileState.dirty = text !== autosave.lastSavedText;
    fileState.error = null;
    refreshTitle();
    autosave.schedule(text);
  }

  async function resolvePath(): Promise<string | null> {
    const fromCli = await getInitialFile();
    if (fromCli) return fromCli;
    const picked = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "Markdown", extensions: ["md", "markdown", "mdx", "txt"] }],
    });
    return typeof picked === "string" ? picked : null;
  }

  async function boot() {
    const path = await resolvePath();
    if (!path) {
      // Launched bare and the user cancelled the picker: there is nothing to edit.
      await win.destroy();
      return;
    }

    fileState.path = path;
    const { text } = await readFile(path);

    autosave = new Autosave(
      {
        path,
        delayMs: 5000,
        onSaveStart: () => (fileState.saving = true),
        onSaved: (saved) => {
          fileState.dirty = (view?.state.doc.toString() ?? saved) !== saved;
          refreshTitle();
        },
        onIdle: () => {
          fileState.saving = false;
          refreshTitle();
        },
        onError: (error) => (fileState.error = `save failed: ${error}`),
      },
      text,
    );

    view = createEditor({ parent: host, doc: text, documentDir: dirname(path), onDocChange });
    view.focus();
    refreshTitle();

    unlistenWatch = await watchFile(path, {
      currentText: () => view?.state.doc.toString() ?? "",
      lastSavedText: () => autosave?.lastSavedText ?? "",
      isDirty: () => fileState.dirty,
      onReload: (diskText) => {
        replaceBuffer(diskText);
        autosave?.reset(diskText);
        fileState.dirty = false;
        refreshTitle();
      },
      onConflict: (diskText) => {
        autosave?.suspend();
        fileState.conflictText = diskText;
        fileState.conflict = true;
      },
      onError: (error) => (fileState.error = `reload failed: ${error}`),
    });

    // The window must not go away before the debounce timer has fired.
    unlistenClose = await win.onCloseRequested(async (event) => {
      event.preventDefault();
      try {
        await autosave?.flush();
      } finally {
        await win.destroy();
      }
    });
  }

  function keepMine() {
    const text = view?.state.doc.toString() ?? "";
    fileState.conflict = false;
    fileState.conflictText = null;
    autosave?.resume();
    autosave?.schedule(text);
    void autosave?.flush();
    view?.focus();
  }

  function loadTheirs() {
    const text = fileState.conflictText ?? "";
    replaceBuffer(text);
    autosave?.reset(text);
    autosave?.resume();
    fileState.conflict = false;
    fileState.conflictText = null;
    fileState.dirty = false;
    refreshTitle();
    view?.focus();
  }

  onMount(() => {
    void boot().catch((error) => (fileState.error = String(error)));
    return () => {
      unlistenWatch?.();
      unlistenClose?.();
      view?.destroy();
    };
  });
</script>

<main bind:this={host}></main>
<StatusBar />

{#if fileState.conflict}
  <ConflictDialog onKeepMine={keepMine} onLoadTheirs={loadTheirs} />
{/if}

<style>
  main {
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }
</style>
