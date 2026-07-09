<script lang="ts">
  import { fileState, basename } from "../persistence/fileStore.svelte";
  import { statusItems, pluginErrors } from "../plugins/surfaces.svelte";

  let status = $derived(
    fileState.error
      ? fileState.error
      : fileState.saving
        ? "saving…"
        : fileState.dirty
          ? "unsaved"
          : "saved",
  );

  function showError() {
    const lines = pluginErrors.map((e) => `${e.name}: ${e.message}`).join("\n");
    // eslint-disable-next-line no-alert
    alert(`Plugin errors\n\n${lines}`);
  }
</script>

<footer class:error={!!fileState.error}>
  <span class="name">{basename(fileState.path)}</span>
  {#if fileState.dirty && !fileState.error}<span class="dot" aria-hidden="true">•</span>{/if}
  <span class="spacer"></span>

  {#each statusItems as item (item.id)}
    {#if item.onClick}
      <button type="button" class="plugin-item" title={item.title} onclick={item.onClick}
        >{item.text}</button
      >
    {:else}
      <span class="plugin-item" title={item.title}>{item.text}</span>
    {/if}
  {/each}

  {#if pluginErrors.length > 0}
    <button
      type="button"
      class="plugin-error"
      title="A plugin failed to load — click for details"
      onclick={showError}>⚠ {pluginErrors.length} plugin error{pluginErrors.length > 1 ? "s" : ""}</button
    >
  {/if}

  <span class="status">{status}</span>
</footer>

<style>
  footer {
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.25rem 0.9rem;
    border-top: 1px solid var(--border);
    background: var(--bg-subtle);
    color: var(--fg-dim);
    font-size: 0.78rem;
    user-select: none;
  }
  footer.error {
    color: var(--danger);
  }
  .name {
    color: var(--fg);
  }
  footer.error .name {
    color: inherit;
  }
  .dot {
    color: var(--accent);
  }
  .spacer {
    flex: 1;
  }
  .status {
    font-variant-numeric: tabular-nums;
  }
  .plugin-item,
  .plugin-error {
    border: 0;
    background: transparent;
    font: inherit;
    color: var(--fg-dim);
    padding: 0;
    cursor: default;
  }
  button.plugin-item {
    cursor: pointer;
  }
  button.plugin-item:hover {
    color: var(--fg);
  }
  .plugin-error {
    color: var(--danger);
    cursor: pointer;
  }
</style>
