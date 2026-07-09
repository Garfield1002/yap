<script lang="ts">
  import { onMount } from "svelte";
  import { EditorView } from "@codemirror/view";
  import { undo, redo } from "@codemirror/commands";
  import { openSearchPanel } from "@codemirror/search";
  import { type UnlistenFn } from "@tauri-apps/api/event";
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
    getConfig,
    newWindow,
  } from "./lib/persistence/api";
  import { Autosave } from "./lib/persistence/autosave";
  import { watchFile } from "./lib/persistence/watcher";
  import { basename, dirname, fileState } from "./lib/persistence/fileStore.svelte";
  import { initTheme, setTheme, type Theme } from "./lib/ui/theme";
  import { markdownToHtml } from "./lib/export/markdownToHtml";
  import type { Menu, MenuAction } from "./lib/ui/menu";
  import {
    registerCommand,
    getCommand,
    runCommand,
    dispatchKey,
  } from "./lib/commands/registry.svelte";
  import { formatChord } from "./lib/commands/keys";
  import Titlebar from "./lib/ui/Titlebar.svelte";
  import CommandPalette from "./lib/ui/CommandPalette.svelte";
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

  /** Recent files for the File menu, refreshed whenever it is opened. */
  let recent = $state<string[]>([]);
  /** Persisted theme override, so the Settings menu can tick the active one. */
  let themePref = $state<Theme | null>(null);
  /** Whether the Ctrl+P command palette is showing. */
  let paletteOpen = $state(false);

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
    themePref = theme;
    await setTheme(theme);
  }

  function openFind() {
    if (!view) return;
    openSearchPanel(view);
    view.focus();
  }

  /** Copy/Cut run through the DOM so CodeMirror's own copy handler fires and
   *  fills the clipboard with document source (hidden markup and all). Paste
   *  reads text and inserts it via a transaction. */
  function clipboard(kind: "copy" | "cut") {
    view?.focus();
    document.execCommand(kind);
  }

  async function pasteText() {
    if (!view) return;
    view.focus();
    try {
      const text = await navigator.clipboard.readText();
      view.dispatch(view.state.replaceSelection(text));
    } catch (error) {
      fileState.error = `paste failed: ${error}`;
    }
  }

  async function refreshRecent() {
    try {
      recent = (await getConfig()).recent;
    } catch {
      recent = [];
    }
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

  const hasPath = () => !!fileState.path;

  // The core commands. Registered once, up front, so the menus (built below) and
  // the palette both resolve them, and the keybinding layer can dispatch their
  // chords. Plugins will add to this same registry. Editor commands carry a
  // display-only `accelerator` because CodeMirror's own keymap owns the key.
  registerCommand({ id: "file.new", title: "New", keybinding: "Mod+N", run: () => openDocument(null) });
  registerCommand({ id: "file.newWindow", title: "New Window", keybinding: "Mod+Shift+N", run: () => void newWindow() });
  registerCommand({ id: "file.open", title: "Open…", keybinding: "Mod+O", run: () => void openViaDialog() });
  registerCommand({ id: "file.save", title: "Save", keybinding: "Mod+S", run: () => void save() });
  registerCommand({ id: "file.rename", title: "Rename…", enabled: hasPath, run: () => void rename() });
  registerCommand({ id: "file.delete", title: "Delete", enabled: hasPath, run: () => void del() });
  registerCommand({ id: "file.copyPath", title: "Copy Path", enabled: hasPath, run: () => void copyPath() });
  registerCommand({
    id: "file.openLocation",
    title: "Open File Location",
    enabled: hasPath,
    run: () => (fileState.path ? void revealItemInDir(fileState.path) : undefined),
  });
  registerCommand({ id: "file.quit", title: "Quit", keybinding: "Mod+Q", run: () => void quit() });

  registerCommand({ id: "edit.undo", title: "Undo", accelerator: "Mod+Z", run: () => { if (view) undo(view); } });
  registerCommand({ id: "edit.redo", title: "Redo", accelerator: "Mod+Y", run: () => { if (view) redo(view); } });
  registerCommand({ id: "edit.cut", title: "Cut", accelerator: "Mod+X", run: () => clipboard("cut") });
  registerCommand({ id: "edit.copy", title: "Copy", accelerator: "Mod+C", run: () => clipboard("copy") });
  registerCommand({ id: "edit.copyHtml", title: "Copy HTML", run: () => void copyHtml() });
  registerCommand({ id: "edit.paste", title: "Paste", accelerator: "Mod+V", run: () => void pasteText() });
  registerCommand({ id: "edit.find", title: "Find…", accelerator: "Mod+F", run: openFind });
  registerCommand({ id: "edit.replace", title: "Find and Replace…", accelerator: "Mod+Alt+F", run: openFind });

  registerCommand({ id: "settings.themeLight", title: "Light Theme", run: () => void chooseTheme("light") });
  registerCommand({ id: "settings.themeDark", title: "Dark Theme", run: () => void chooseTheme("dark") });

  registerCommand({
    id: "view.commandPalette",
    title: "Command Palette",
    keybinding: "Mod+P",
    hidden: true,
    run: () => { paletteOpen = true; },
  });

  /** Build a menu item from a registered command, resolving its label,
   *  accelerator hint, and enabled state; `extra` layers on menu-only bits
   *  (e.g. a checkmark). */
  function cmd(id: string, extra: Partial<MenuAction> = {}): MenuAction {
    const c = getCommand(id);
    const chord = c?.accelerator ?? c?.keybinding;
    return {
      type: "action",
      id,
      label: c?.title ?? id,
      accelerator: chord ? formatChord(chord) : undefined,
      enabled: c?.enabled ? c.enabled() : true,
      run: () => void runCommand(id),
      ...extra,
    };
  }

  /** The title-bar menus, sourced from the shared command registry so a command
   *  is declared once and appears in menu, palette, and keybindings alike. */
  const menus = $derived<Menu[]>([
    {
      id: "file",
      label: "File",
      items: [
        cmd("file.new"),
        cmd("file.newWindow"),
        cmd("file.open"),
        {
          type: "submenu",
          label: "Open Recent",
          enabled: recent.length > 0,
          items:
            recent.length > 0
              ? recent.map((p, i) => ({
                  type: "action" as const,
                  id: `recent:${i}`,
                  label: basename(p),
                  run: () => void openDocument(p),
                }))
              : [{ type: "action" as const, id: "recent_none", label: "No recent files", enabled: false, run: () => {} }],
        },
        { type: "separator" },
        cmd("file.save"),
        cmd("file.rename"),
        cmd("file.delete"),
        { type: "separator" },
        cmd("file.copyPath"),
        cmd("file.openLocation"),
        { type: "separator" },
        cmd("file.quit"),
      ],
    },
    {
      id: "edit",
      label: "Edit",
      items: [
        cmd("edit.undo"),
        cmd("edit.redo"),
        { type: "separator" },
        cmd("edit.cut"),
        cmd("edit.copy"),
        cmd("edit.copyHtml"),
        cmd("edit.paste"),
        { type: "separator" },
        cmd("edit.find"),
        cmd("edit.replace"),
      ],
    },
    {
      id: "settings",
      label: "Settings",
      items: [
        cmd("settings.themeLight", { checked: themePref === "light" }),
        cmd("settings.themeDark", { checked: themePref === "dark" }),
      ],
    },
  ]);

  /** Decide what to open on launch: the CLI file if one was passed, otherwise
   *  an untitled buffer. */
  async function boot() {
    await initTheme();
    try {
      const cfg = await getConfig();
      themePref = cfg.theme === "light" || cfg.theme === "dark" ? cfg.theme : null;
      recent = cfg.recent;
    } catch {
      // No config yet: menus fall back to their empty states.
    }

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

<svelte:window onkeydown={(e) => dispatchKey(e)} />

<Titlebar {menus} onmenuopen={(id) => id === "file" && refreshRecent()} />
<main bind:this={host}></main>
<StatusBar />

{#if paletteOpen}
  <CommandPalette onclose={() => paletteOpen = false} />
{/if}

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
