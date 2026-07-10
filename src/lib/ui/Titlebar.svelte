<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { basename, fileState } from "../persistence/fileStore.svelte";
  import type { Menu as MenuModel } from "./menu";
  import Menu from "./Menu.svelte";
  import lightLogo from "../../../assets/logo_white.svg?url";
  import darkLogo from "../../../assets/logo_black.svg?url";

  let {
    menus,
    onmenuopen,
  }: { menus: MenuModel[]; onmenuopen?: (id: string) => void } = $props();

  const win = getCurrentWindow();

  // Which top-level menu is dropped open, by id, or null when none is.
  let open = $state<string | null>(null);

  function toggle(id: string) {
    if (open === id) {
      open = null;
      return;
    }
    onmenuopen?.(id);
    open = id;
  }

  // Once a menu is open, hovering onto a sibling button switches to it, like a
  // native menu bar.
  function hover(id: string) {
    if (open !== null && open !== id) {
      onmenuopen?.(id);
      open = id;
    }
  }

  function close() {
    open = null;
  }
</script>

<svelte:window
  onkeydown={(e) => {
    if (e.key === "Escape") close();
  }}
/>

<!-- data-tauri-drag-region only on the non-interactive centre, so the menu and
     window buttons stay clickable. -->
<header class="titlebar">
  <div class="brand" aria-hidden="true">
    <img class="logo logo-light" src={lightLogo} alt="" />
    <img class="logo logo-dark" src={darkLogo} alt="" />
  </div>
  <nav class="menus">
    {#each menus as menu (menu.id)}
      <div class="menu-slot">
        <button
          class="menu-btn"
          class:active={open === menu.id}
          onclick={() => toggle(menu.id)}
          onmouseenter={() => hover(menu.id)}>{menu.label}</button
        >
        {#if open === menu.id}
          <div class="dropdown">
            <Menu items={menu.items} onclose={close} />
          </div>
        {/if}
      </div>
    {/each}
  </nav>

  <!-- Flow spacer carries the drag region and pushes the controls right; the
       title itself is centred on the window, not on this leftover space. -->
  <div class="spacer" data-tauri-drag-region></div>

  <div class="title">
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

{#if open !== null}
  <!-- Click-away backdrop: any click outside the dropdown closes the menu. -->
  <button class="backdrop" aria-label="Close menu" onclick={close}></button>
{/if}

<style>
  .titlebar {
    position: relative;
    z-index: 20;
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
  .brand {
    flex: 0 0 34px;
    display: grid;
    place-items: center;
  }
  .logo {
    grid-area: 1 / 1;
    width: 20px;
    height: 20px;
    display: block;
  }
  .logo-dark {
    display: none;
  }
  @media (prefers-color-scheme: dark) {
    :global(:root:not([data-theme="light"])) .logo-light {
      display: none;
    }
    :global(:root:not([data-theme="light"])) .logo-dark {
      display: block;
    }
  }
  :global(:root[data-theme="dark"]) .logo-light {
    display: none;
  }
  :global(:root[data-theme="dark"]) .logo-dark {
    display: block;
  }
  .menu-slot {
    position: relative;
    display: flex;
  }
  .menu-btn {
    border: 0;
    background: transparent;
    color: var(--fg);
    font: inherit;
    padding: 0 0.7rem;
    cursor: pointer;
  }
  .menu-btn:hover,
  .menu-btn.active {
    background: color-mix(in srgb, var(--fg) 10%, transparent);
  }
  .dropdown {
    position: absolute;
    top: 100%;
    left: 0;
    z-index: 30;
  }

  .spacer {
    flex: 1;
    min-width: 0;
  }
  /* Centred on the whole titlebar (the window), independent of the unequal
     left/right regions. `pointer-events: none` lets a drag pass through to the
     spacer beneath; `max-width` keeps a long name from colliding with them. */
  .title {
    position: absolute;
    left: 50%;
    top: 0;
    bottom: 0;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.35rem;
    max-width: 50%;
    pointer-events: none;
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

  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 10;
    border: 0;
    padding: 0;
    background: transparent;
    cursor: default;
  }
</style>
