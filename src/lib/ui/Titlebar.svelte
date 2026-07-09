<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { basename, fileState } from "../persistence/fileStore.svelte";
  import { popupMenu } from "../persistence/api";

  const win = getCurrentWindow();

  const menus: Array<{ id: "file" | "edit" | "settings"; label: string }> = [
    { id: "file", label: "File" },
    { id: "edit", label: "Edit" },
    { id: "settings", label: "Settings" },
  ];

  function openMenu(which: "file" | "edit" | "settings") {
    void popupMenu(which, !!fileState.path);
  }
</script>

<!-- data-tauri-drag-region only on the non-interactive centre, so the menu and
     window buttons stay clickable. -->
<header class="titlebar">
  <nav class="menus">
    {#each menus as menu (menu.id)}
      <button class="menu-btn" onclick={() => openMenu(menu.id)}>{menu.label}</button>
    {/each}
  </nav>

  <div class="title" data-tauri-drag-region>
    <span class="name">{basename(fileState.path)}</span>
    {#if fileState.dirty}<span class="dot" aria-hidden="true">•</span>{/if}
  </div>

  <div class="controls">
    <button class="ctl" aria-label="Minimize" onclick={() => win.minimize()}>
      <svg viewBox="0 0 10 10" width="10" height="10"><path d="M0 5 H10" /></svg>
    </button>
    <button class="ctl" aria-label="Maximize" onclick={() => win.toggleMaximize()}>
      <svg viewBox="0 0 10 10" width="10" height="10"><rect x="0.5" y="0.5" width="9" height="9" /></svg>
    </button>
    <button class="ctl close" aria-label="Close" onclick={() => win.close()}>
      <svg viewBox="0 0 10 10" width="10" height="10"><path d="M0 0 L10 10 M10 0 L0 10" /></svg>
    </button>
  </div>
</header>

<style>
  .titlebar {
    flex: 0 0 auto;
    display: flex;
    align-items: stretch;
    height: 34px;
    background: var(--bg-subtle);
    border-bottom: 1px solid var(--border);
    user-select: none;
    font-size: 0.8rem;
  }

  .menus {
    display: flex;
    align-items: stretch;
    padding-left: 0.15rem;
  }
  .menu-btn {
    border: 0;
    background: transparent;
    color: var(--fg);
    font: inherit;
    padding: 0 0.7rem;
    cursor: pointer;
  }
  .menu-btn:hover {
    background: color-mix(in srgb, var(--fg) 10%, transparent);
  }

  .title {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.35rem;
    min-width: 0;
    color: var(--fg-dim);
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dot {
    color: var(--accent);
  }

  .controls {
    display: flex;
    align-items: stretch;
  }
  .ctl {
    border: 0;
    background: transparent;
    color: var(--fg-dim);
    width: 44px;
    display: grid;
    place-items: center;
    cursor: pointer;
  }
  .ctl svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1;
  }
  .ctl:hover {
    background: color-mix(in srgb, var(--fg) 10%, transparent);
    color: var(--fg);
  }
  .ctl.close:hover {
    background: var(--danger);
    color: #fff;
  }
</style>
