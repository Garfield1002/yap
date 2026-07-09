<script lang="ts">
  import { listCommands, isEnabled, runCommand, type Command } from "../commands/registry.svelte";
  import { formatChord } from "../commands/keys";

  let { onclose }: { onclose: () => void } = $props();

  let query = $state("");
  let active = $state(0);
  let input: HTMLInputElement | undefined;

  /** Case-insensitive subsequence match: "opn" matches "Open…". Returns a rank
   *  (lower is tighter) or null when it does not match. */
  function score(title: string, q: string): number | null {
    if (!q) return 0;
    const t = title.toLowerCase();
    let i = 0;
    let last = -1;
    let gaps = 0;
    for (const ch of q.toLowerCase()) {
      const at = t.indexOf(ch, i);
      if (at === -1) return null;
      if (last >= 0) gaps += at - last - 1;
      last = at;
      i = at + 1;
    }
    return gaps;
  }

  const matches = $derived.by(() => {
    const q = query.trim();
    return listCommands()
      .filter((c) => !c.hidden && isEnabled(c))
      .map((c) => ({ cmd: c, rank: score(c.title, q) }))
      .filter((m): m is { cmd: Command; rank: number } => m.rank !== null)
      .sort((a, b) => a.rank - b.rank || a.cmd.title.localeCompare(b.cmd.title))
      .map((m) => m.cmd);
  });

  // Keep the highlighted row in range as the list narrows.
  $effect(() => {
    if (active >= matches.length) active = Math.max(0, matches.length - 1);
  });

  $effect(() => {
    input?.focus();
  });

  function accel(cmd: Command): string {
    const chord = cmd.accelerator ?? cmd.keybinding;
    return chord ? formatChord(chord) : "";
  }

  function choose(cmd: Command | undefined) {
    if (!cmd) return;
    onclose();
    void runCommand(cmd.id);
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      onclose();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      active = matches.length ? (active + 1) % matches.length : 0;
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      active = matches.length ? (active - 1 + matches.length) % matches.length : 0;
    } else if (e.key === "Enter") {
      e.preventDefault();
      choose(matches[active]);
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="overlay" onclick={onclose}>
  <div class="palette" role="dialog" tabindex="-1" aria-label="Command palette" onclick={(e) => e.stopPropagation()}>
    <input
      bind:this={input}
      bind:value={query}
      {onkeydown}
      class="search"
      type="text"
      placeholder="Type a command…"
      spellcheck="false"
      autocomplete="off"
    />
    <ul class="list" role="listbox">
      {#each matches as cmd, i (cmd.id)}
        <li>
          <button
            type="button"
            class="row"
            class:active={i === active}
            role="option"
            aria-selected={i === active}
            onmousemove={() => (active = i)}
            onclick={() => choose(cmd)}
          >
            <span class="title">{cmd.title}</span>
            {#if accel(cmd)}<span class="accel">{accel(cmd)}</span>{/if}
          </button>
        </li>
      {:else}
        <li class="empty">No matching commands</li>
      {/each}
    </ul>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 100;
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 12vh;
    background: rgba(0, 0, 0, 0.28);
  }
  .palette {
    width: min(560px, 90vw);
    max-height: 60vh;
    display: flex;
    flex-direction: column;
    background: var(--bg-subtle);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.4);
    overflow: hidden;
  }
  .search {
    border: 0;
    border-bottom: 1px solid var(--border);
    background: transparent;
    color: var(--fg);
    font: inherit;
    font-size: 0.95rem;
    padding: 0.7rem 0.9rem;
    outline: none;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0.25rem 0;
    overflow-y: auto;
  }
  .row {
    width: 100%;
    border: 0;
    background: transparent;
    font: inherit;
    color: var(--fg);
    text-align: left;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.4rem 0.9rem;
    cursor: default;
  }
  .row.active {
    background: var(--accent);
    color: #fff;
  }
  .title {
    flex: 1;
  }
  .accel {
    color: var(--fg-dim);
    font-size: 0.78rem;
    font-variant-numeric: tabular-nums;
  }
  .row.active .accel {
    color: inherit;
  }
  .empty {
    padding: 0.6rem 0.9rem;
    color: var(--fg-dim);
    font-size: 0.85rem;
  }
</style>
