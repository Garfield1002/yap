<script lang="ts">
  import { basename, fileState } from "../persistence/fileStore.svelte";

  interface Props {
    onKeepMine: () => void;
    onLoadTheirs: () => void;
  }

  let { onKeepMine, onLoadTheirs }: Props = $props();
  let dialog: HTMLDivElement;

  // Autosave is suspended while this is open, so neither answer can be raced by
  // a debounce timer firing underneath it.
  $effect(() => {
    dialog?.focus();
  });
</script>

<div class="backdrop">
  <div
    class="dialog"
    role="alertdialog"
    aria-modal="true"
    aria-labelledby="conflict-title"
    tabindex="-1"
    bind:this={dialog}
  >
    <h2 id="conflict-title">{basename(fileState.path)} changed on disk</h2>
    <p>You have unsaved edits, and something else wrote to this file.</p>
    <div class="actions">
      <button onclick={onLoadTheirs}>Load theirs</button>
      <button class="primary" onclick={onKeepMine}>Keep mine</button>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    background: color-mix(in srgb, var(--bg) 70%, transparent);
    backdrop-filter: blur(2px);
    z-index: 10;
  }
  .dialog {
    max-width: 24rem;
    padding: 1.25rem 1.4rem;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--bg-subtle);
    box-shadow: 0 12px 40px rgb(0 0 0 / 0.35);
    outline: none;
  }
  h2 {
    margin: 0 0 0.5rem;
    font-size: 1rem;
  }
  p {
    margin: 0 0 1.1rem;
    color: var(--fg-dim);
    font-size: 0.88rem;
    line-height: 1.5;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }
  button {
    padding: 0.4rem 0.9rem;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg);
    color: var(--fg);
    font: inherit;
    font-size: 0.85rem;
    cursor: pointer;
  }
  button:hover {
    border-color: var(--accent);
  }
  button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--bg);
    font-weight: 600;
  }
</style>
