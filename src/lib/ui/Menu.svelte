<script lang="ts">
  import type { MenuItem } from "./menu";
  import Self from "./Menu.svelte";

  let { items, onclose }: { items: MenuItem[]; onclose: () => void } = $props();

  // Index of the submenu currently expanded (hovered open), if any.
  let openSub = $state<number | null>(null);

  function activate(item: MenuItem) {
    if (item.type !== "action" || item.enabled === false) return;
    onclose();
    item.run();
  }
</script>

<ul class="menu" role="menu">
  {#each items as item, i (i)}
    {#if item.type === "separator"}
      <li class="sep" role="separator"></li>
    {:else if item.type === "submenu"}
      <li role="none" class="slot">
        <button
          type="button"
          class="item sub"
          class:disabled={item.enabled === false}
          disabled={item.enabled === false}
          role="menuitem"
          aria-haspopup="menu"
          aria-expanded={openSub === i}
          onmouseenter={() => (openSub = item.enabled === false ? null : i)}
          onclick={() => (openSub = openSub === i ? null : i)}
        >
          <span class="check" aria-hidden="true"></span>
          <span class="label">{item.label}</span>
          <span class="chevron" aria-hidden="true">›</span>
        </button>
        {#if openSub === i}
          <div class="submenu">
            <Self items={item.items} {onclose} />
          </div>
        {/if}
      </li>
    {:else}
      <li role="none" class="slot">
        <button
          type="button"
          class="item"
          class:disabled={item.enabled === false}
          disabled={item.enabled === false}
          role="menuitem"
          onmouseenter={() => (openSub = null)}
          onclick={() => activate(item)}
        >
          <span class="check" aria-hidden="true">{item.checked ? "✓" : ""}</span>
          <span class="label">{item.label}</span>
          {#if item.accelerator}<span class="accel">{item.accelerator}</span>{/if}
        </button>
      </li>
    {/if}
  {/each}
</ul>

<style>
  .menu {
    list-style: none;
    margin: 0;
    padding: 0.25rem 0;
    min-width: 200px;
    background: var(--bg-subtle);
    border: 1px solid var(--border);
    border-radius: 6px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.28);
    color: var(--fg);
    font-size: 0.8rem;
  }
  .slot {
    position: relative;
  }
  .item {
    width: 100%;
    border: 0;
    background: transparent;
    font: inherit;
    color: inherit;
    text-align: left;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.3rem 0.8rem 0.3rem 0.5rem;
    cursor: default;
    white-space: nowrap;
  }
  .item:hover:not(:disabled) {
    background: var(--accent);
    color: #fff;
  }
  .item:disabled {
    color: var(--fg-dim);
    opacity: 0.55;
  }
  .check {
    width: 1em;
    text-align: center;
    flex: 0 0 auto;
  }
  .label {
    flex: 1;
  }
  .accel,
  .chevron {
    color: var(--fg-dim);
    font-variant-numeric: tabular-nums;
  }
  .item:hover:not(:disabled) .accel,
  .item:hover:not(:disabled) .chevron {
    color: inherit;
  }
  .sep {
    height: 1px;
    margin: 0.25rem 0;
    background: var(--border);
  }
  .submenu {
    position: absolute;
    top: -0.25rem;
    left: 100%;
    z-index: 1;
  }
</style>
