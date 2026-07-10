<script lang="ts">
  import lightLogo from "../../../assets/logo_white.svg?url";
  import darkLogo from "../../../assets/logo_black.svg?url";
  import { fade } from "svelte/transition";
</script>

<div class="splash" role="status" aria-label="Starting bulletmd" transition:fade={{ duration: 150 }}>
  <div class="mark">
    <img class="logo logo-light" src={lightLogo} alt="" />
    <img class="logo logo-dark" src={darkLogo} alt="" />
    <div class="spinner" aria-hidden="true"></div>
  </div>
  <span>bulletmd</span>
</div>

<style>
  .splash {
    position: fixed;
    inset: 0;
    z-index: 100;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 0.75rem;
    background: var(--bg);
    color: var(--fg);
    font-size: 0.9rem;
    font-weight: 600;
    letter-spacing: 0.04em;
  }
  .mark {
    position: relative;
    display: grid;
    place-items: center;
    width: 64px;
    height: 64px;
  }
  .logo {
    grid-area: 1 / 1;
    width: 48px;
    height: 48px;
  }
  .logo-dark {
    display: none;
  }
  @media (prefers-color-scheme: dark) {
    :global(:root:not([data-theme="light"])) .logo-light { display: none; }
    :global(:root:not([data-theme="light"])) .logo-dark { display: block; }
  }
  :global(:root[data-theme="dark"]) .logo-light { display: none; }
  :global(:root[data-theme="dark"]) .logo-dark { display: block; }
  .spinner {
    position: absolute;
    inset: 0;
    border: 1px solid transparent;
    border-top-color: var(--fg-dim);
    border-radius: 50%;
    opacity: 0;
    animation: reveal 0s 750ms forwards, spin 0.9s 750ms linear infinite;
  }
  @keyframes reveal { to { opacity: 1; } }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
