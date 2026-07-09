<script lang="ts">
  import { onMount } from "svelte";
  import { EditorView } from "@codemirror/view";
  import { undo, redo } from "@codemirror/commands";
  import { openSearchPanel } from "@codemirror/search";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { open, save as saveDialog, ask } from "@tauri-apps/plugin-dialog";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";

  import { createEditor, documentDirCompartment } from "./lib/editor/createEditor";
  import { documentDirectory } from "./lib/editor/livePreview";
  import {
    getInitialFile,
    readFile,
    writeFileAtomic,
    deleteFile,
    renameFile,
    recordRecent,
  } from "./lib/persistence/api";
  import { Autosave } from "./lib/persistence/autosave";
  import { watchFile } from "./lib/persistence/watcher";
  import { basename, dirname, fileState } from "./lib/persistence/fileStore.svelte";
  import { initTheme, setTheme, type Theme } from "./lib/ui/theme";
  import { markdownToHtml } from "./lib/export/markdownToHtml";
  import Titlebar from "./lib/ui/Titlebar.svelte";
  import StatusBar from "./lib/ui/StatusBar.svelte";
  import ConflictDialog from "./lib/ui/ConflictDialog.svelte";
  import UnsavedDialog from "./lib/ui/UnsavedDialog.svelte";

  const MD_FILTERS = [{ name: "Markdown", extensions: ["md", "markdown", "mdx", "txt"] }];
  const AUTOSAVE_MS = 5000;

  let host: HTMLElement;
  let view: EditorView | undefined;
  let autosave: Autosave | undefined;
  let unlistenWatch: UnlistenFn | undefined;
  let unlistenClose: UnlistenFn | undefined;
  let unlistenMenu: UnlistenFn | undefined;

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
    if (applyingExternalChange) return;
    fileState.error = null;
    if (autosave) {
      fileState.dirty = text !== autosave.lastSavedText;
      autosave.schedule(text);
    } else {
      // Untitled: any content is unsaved until the buffer is given a path.
      fileState.dirty = text.length > 0;
    }
    refreshTitle();
  }

  function makeAutosave(path: string, initialText: string): Autosave {
    return new Autosave(
      {
        path,
        delayMs: AUTOSAVE_MS,
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
      initialText,
    );
  }

  async function setupWatcher(path: string) {
    unlistenWatch?.();
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
  }

  /** Attach persistence (autosave + watcher) for `path` to the current view.
   *  `diskText` is what is already on disk, so autosave treats it as saved. */
  async function attachPersistence(path: string, diskText: string) {
    autosave = makeAutosave(path, diskText);
    fileState.path = path;
    view?.dispatch({
      effects: documentDirCompartment.reconfigure(documentDirectory.of(dirname(path))),
    });
    await setupWatcher(path);
    await recordRecent(path);
    refreshTitle();
  }

  /** Drop persistence and become an untitled buffer, keeping the current text. */
  async function detachPersistence() {
    autosave?.suspend();
    autosave = undefined;
    unlistenWatch?.();
    unlistenWatch = undefined;
    fileState.path = null;
    view?.dispatch({
      effects: documentDirCompartment.reconfigure(documentDirectory.of("")),
    });
    refreshTitle();
  }

  async function teardown() {
    try {
      await autosave?.flush();
    } catch {
      // A failed final save must not block switching documents.
    }
    autosave?.suspend();
    autosave = undefined;
    unlistenWatch?.();
    unlistenWatch = undefined;
    view?.destroy();
    view = undefined;
  }

  /** Load a file (or a blank untitled buffer when `path` is null) into a fresh
   *  editor, replacing whatever this window held. */
  async function openDocument(path: string | null) {
    await teardown();
    const text = path ? (await readFile(path)).text : "";
    view = createEditor({
      parent: host,
      doc: text,
      documentDir: path ? dirname(path) : "",
      onDocChange,
    });

    fileState.error = null;
    fileState.conflict = false;
    fileState.conflictText = null;
    if (path) {
      await attachPersistence(path, text);
      fileState.dirty = false;
    } else {
      fileState.path = null;
      fileState.dirty = false;
      refreshTitle();
    }
    view.focus();
  }

  async function save() {
    if (!view) return;
    const text = view.state.doc.toString();
    if (fileState.path && autosave) {
      autosave.schedule(text);
      await autosave.flush();
      return;
    }
    // Untitled: choose a path, write it, then bind persistence to it.
    const path = await saveDialog({ defaultPath: "untitled.md", filters: MD_FILTERS });
    if (!path) return;
    await writeFileAtomic(path, text);
    await attachPersistence(path, text);
    fileState.dirty = false;
  }

  async function rename() {
    if (!view || !fileState.path) return;
    const target = await saveDialog({ defaultPath: fileState.path, filters: MD_FILTERS });
    if (!target || target === fileState.path) return;
    try {
      await autosave?.flush();
    } catch {
      // fall through: the rename below still moves the last-saved content
    }
    autosave?.suspend();
    unlistenWatch?.();
    unlistenWatch = undefined;
    await renameFile(fileState.path, target);
    await attachPersistence(target, view.state.doc.toString());
    fileState.dirty = false;
  }

  async function del() {
    if (!fileState.path) return;
    const confirmed = await ask(`Delete ${basename(fileState.path)}? This cannot be undone.`, {
      title: "Delete file",
      kind: "warning",
    });
    if (!confirmed) return;
    const path = fileState.path;
    await detachPersistence();
    try {
      await deleteFile(path);
      // The text lives on as an untitled buffer the user can re-save.
      fileState.dirty = (view?.state.doc.toString().length ?? 0) > 0;
      refreshTitle();
    } catch (error) {
      fileState.error = `delete failed: ${error}`;
    }
  }

  async function copyPath() {
    if (fileState.path) await navigator.clipboard.writeText(fileState.path);
  }

  async function copyHtml() {
    if (!view) return;
    const md = view.state.doc.toString();
    const html = markdownToHtml(md);
    try {
      await navigator.clipboard.write([
        new ClipboardItem({
          "text/html": new Blob([html], { type: "text/html" }),
          "text/plain": new Blob([md], { type: "text/plain" }),
        }),
      ]);
    } catch {
      await navigator.clipboard.writeText(html);
    }
  }

  async function openViaDialog() {
    const picked = await open({ multiple: false, directory: false, filters: MD_FILTERS });
    if (typeof picked === "string") await openDocument(picked);
  }

  async function chooseTheme(theme: Theme) {
    await setTheme(theme);
  }

  // Route through the close handler so an unsaved untitled buffer is caught.
  async function quit() {
    await win.close();
  }

  let showUnsaved = $state(false);
  let closeResolver: ((choice: "save" | "discard" | "cancel") => void) | null = null;

  function promptUnsaved(): Promise<"save" | "discard" | "cancel"> {
    showUnsaved = true;
    return new Promise((resolve) => (closeResolver = resolve));
  }

  function resolveUnsaved(choice: "save" | "discard" | "cancel") {
    showUnsaved = false;
    closeResolver?.(choice);
    closeResolver = null;
  }

  async function handleMenuAction(name: string, path?: string | null) {
    switch (name) {
      case "new":
        return openDocument(null);
      case "open":
        return openViaDialog();
      case "open-path":
        return path ? openDocument(path) : undefined;
      case "save":
        return save();
      case "rename":
        return rename();
      case "delete":
        return del();
      case "copy_path":
        return copyPath();
      case "open_location":
        return fileState.path ? revealItemInDir(fileState.path) : undefined;
      case "find":
      case "replace":
        if (view) {
          openSearchPanel(view);
          view.focus();
        }
        return;
      case "undo":
        if (view) undo(view);
        return;
      case "redo":
        if (view) redo(view);
        return;
      case "copy_html":
        return copyHtml();
      case "theme_light":
        return chooseTheme("light");
      case "theme_dark":
        return chooseTheme("dark");
      case "quit":
        return quit();
    }
  }

  /** Decide what to open on launch: the CLI file if one was passed, otherwise
   *  an untitled buffer. */
  async function boot() {
    await initTheme();

    const fromCli = await getInitialFile();
    if (fromCli) {
      await openDocument(fromCli);
    } else {
      // Launched bare (or with --new): start with an untitled buffer rather
      // than forcing the open dialog on the user.
      await openDocument(null);
    }

    // The window must not go away before the debounce timer has fired, and an
    // untitled buffer with content -- which autosave cannot persist -- must ask
    // before it is thrown away.
    unlistenClose = await win.onCloseRequested(async (event) => {
      event.preventDefault();

      if (fileState.path) {
        // Titled: flushing pending edits is enough, nothing is lost.
        try {
          await autosave?.flush();
        } finally {
          await win.destroy();
        }
        return;
      }

      if ((view?.state.doc.length ?? 0) === 0) {
        await win.destroy();
        return;
      }

      const choice = await promptUnsaved();
      if (choice === "cancel") return;
      if (choice === "discard") {
        await win.destroy();
        return;
      }
      // "save": succeeds only if the user picks a path in the dialog.
      await save();
      if (fileState.path) await win.destroy();
    });

    unlistenMenu = await listen<{ name: string; path: string | null }>("menu-action", (event) =>
      handleMenuAction(event.payload.name, event.payload.path),
    );
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
      unlistenMenu?.();
      view?.destroy();
    };
  });
</script>

<Titlebar />
<main bind:this={host}></main>
<StatusBar />

{#if fileState.conflict}
  <ConflictDialog onKeepMine={keepMine} onLoadTheirs={loadTheirs} />
{/if}

{#if showUnsaved}
  <UnsavedDialog
    onSave={() => resolveUnsaved("save")}
    onDiscard={() => resolveUnsaved("discard")}
    onCancel={() => resolveUnsaved("cancel")}
  />
{/if}

<style>
  main {
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }
</style>
