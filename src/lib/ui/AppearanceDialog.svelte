<script lang="ts">
  import type { Theme } from "./theme";

  interface Props {
    theme: Theme;
    dotOpacity: number;
    onTheme: (theme: Theme) => void;
    onDotOpacity: (opacity: number) => void;
    onClose: () => void;
  }

  let { theme, dotOpacity, onTheme, onDotOpacity, onClose }: Props = $props();
  let dialog: HTMLDivElement;

  $effect(() => dialog?.focus());

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") onClose();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="backdrop" role="presentation" onclick={(event) => event.target === event.currentTarget && onClose()}>
  <div
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-labelledby="appearance-title"
    tabindex="-1"
    bind:this={dialog}
  >
    <h2 id="appearance-title">Appearance</h2>

    <fieldset>
      <legend>Theme</legend>
      <div class="theme-options">
        <label class:active={theme === "light"}>
          <input type="radio" name="theme" value="light" checked={theme === "light"} onchange={() => onTheme("light")} />
          Light
        </label>
        <label class:active={theme === "dark"}>
          <input type="radio" name="theme" value="dark" checked={theme === "dark"} onchange={() => onTheme("dark")} />
          Dark
        </label>
      </div>
    </fieldset>

    <div class="dot-setting">
      <div class="setting-heading">
        <label for="dot-opacity">Dot opacity</label>
        <output for="dot-opacity">{Math.round(dotOpacity * 100)}%</output>
      </div>
      <input
        id="dot-opacity"
        type="range"
        min="0"
        max="0.3"
        step="0.01"
        value={dotOpacity}
        oninput={(event) => onDotOpacity(Number(event.currentTarget.value))}
      />
      <div class="preview" aria-hidden="true"></div>
    </div>

    <div class="actions">
      <button onclick={() => onDotOpacity(0.12)}>Reset</button>
      <button class="primary" onclick={onClose}>Close</button>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 40;
    display: grid;
    place-items: center;
    background: color-mix(in srgb, var(--bg) 70%, transparent);
    backdrop-filter: blur(2px);
  }
  .dialog {
    width: min(24rem, calc(100vw - 3rem));
    padding: 1.25rem 1.4rem;
    box-sizing: border-box;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--bg-subtle);
    box-shadow: 0 12px 40px rgb(0 0 0 / 0.35);
    outline: none;
  }
  h2 {
    margin: 0 0 1rem;
    font-size: 1rem;
  }
  fieldset {
    margin: 0 0 1.25rem;
    padding: 0;
    border: 0;
  }
  legend,
  .setting-heading {
    margin-bottom: 0.55rem;
    color: var(--fg-dim);
    font-size: 0.8rem;
    font-weight: 600;
  }
  .theme-options {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.5rem;
  }
  .theme-options label {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    padding: 0.55rem 0.7rem;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg);
    font-size: 0.88rem;
    cursor: pointer;
  }
  .theme-options label.active {
    border-color: var(--accent);
  }
  .setting-heading {
    display: flex;
    justify-content: space-between;
  }
  output {
    color: var(--fg);
    font-variant-numeric: tabular-nums;
  }
  input[type="range"] {
    width: 100%;
    accent-color: var(--accent);
  }
  .preview {
    height: 72px;
    margin-top: 0.65rem;
    border: 1px solid var(--border);
    border-radius: 6px;
    background-color: var(--bg);
    background-image: radial-gradient(
      circle at 0 0,
      color-mix(in srgb, var(--fg) var(--dot-opacity), transparent) 1px,
      transparent 1.15px
    );
    background-size: 24px 24px;
  }
  .actions {
    display: flex;
    justify-content: space-between;
    margin-top: 1.25rem;
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
    border-color: var(--accent);
    background: var(--accent);
    color: var(--bg);
    font-weight: 600;
  }
</style>
