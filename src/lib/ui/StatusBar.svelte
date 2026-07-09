<script lang="ts">
  import { fileState, basename } from "../persistence/fileStore.svelte";

  let status = $derived(
    fileState.error
      ? fileState.error
      : fileState.saving
        ? "saving…"
        : fileState.dirty
          ? "unsaved"
          : "saved",
  );
</script>

<footer class:error={!!fileState.error}>
  <span class="name">{basename(fileState.path)}</span>
  {#if fileState.dirty && !fileState.error}<span class="dot" aria-hidden="true">•</span>{/if}
  <span class="spacer"></span>
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
</style>
