<script lang="ts">
  interface Props {
    onSave: () => void;
    onDiscard: () => void;
    onCancel: () => void;
  }

  let { onSave, onDiscard, onCancel }: Props = $props();
  let dialog: HTMLDivElement;

  $effect(() => {
    dialog?.focus();
  });

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") onCancel();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="backdrop">
  <div
    class="dialog"
    role="alertdialog"
    aria-modal="true"
    aria-labelledby="unsaved-title"
    tabindex="-1"
    bind:this={dialog}
  >
    <h2 id="unsaved-title">Unsaved changes</h2>
    <p>This buffer has never been saved. Save it before closing?</p>
    <div class="actions">
      <button onclick={onCancel}>Cancel</button>
      <button class="danger" onclick={onDiscard}>Discard</button>
      <button class="primary" onclick={onSave}>Save…</button>
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
  button.danger:hover {
    border-color: var(--danger);
    color: var(--danger);
  }
</style>
