<script lang="ts">
  export let open = false;
  // Newest-last order straight from the live stacks.
  export let undoEntries: { label: string }[] = [];
  export let redoEntries: { label: string }[] = [];

  export let onClose: () => void;
  // detail: { direction: "undo" | "redo", steps: number }
  export let onRestore: (detail: { direction: "undo" | "redo"; steps: number }) => void;

  let dialogEl: HTMLDivElement | null = null;
  let previouslyFocused: HTMLElement | null = null;

  $: if (open && typeof document !== "undefined") {
    previouslyFocused = (document.activeElement as HTMLElement | null) ?? null;
    setTimeout(() => dialogEl?.focus(), 0);
  }

  const close = () => {
    if (!open) return;
    onClose();
    previouslyFocused?.focus?.();
    previouslyFocused = null;
  };

  const handleKeydown = (event: KeyboardEvent) => {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      close();
      return;
    }
    event.stopPropagation();
  };

  // Clicking an undo entry restores the state right after that action: the
  // newest entry (top of the list) is one step, the next one down is two.
  const restoreUndo = (displayIndex: number) => {
    onRestore({ direction: "undo", steps: displayIndex + 1 });
    close();
  };

  // Clicking a redo entry replays forward up to and including it: the entry
  // closest to "now" (bottom of the displayed list) is a single step.
  const restoreRedo = (displayIndex: number) => {
    onRestore({ direction: "redo", steps: redoEntries.length - displayIndex });
    close();
  };
</script>

<svelte:window on:keydown={open ? handleKeydown : undefined} />

{#if open}
  <div class="backdrop" role="button" tabindex="-1" onclick={close} onkeydown={(e) => e.key === "Escape" && close()}>
    <div
      class="modal"
      role="dialog"
      aria-modal="true"
      aria-label="Action history"
      tabindex="-1"
      bind:this={dialogEl}
      onclick={(e) => e.stopPropagation()}
      onkeydown={handleKeydown}
    >
      <header>
        <h2>Action history</h2>
        <button class="icon" aria-label="Close history" onclick={close}>×</button>
      </header>
      <main>
        {#if !undoEntries.length && !redoEntries.length}
          <p class="status">No actions recorded yet.</p>
        {:else}
          {#each redoEntries as entry, index (redoEntries.length - index)}
            <button
              class="entry redo"
              title={`Redo ${redoEntries.length - index} step(s)`}
              onclick={() => restoreRedo(index)}
            >
              <span class="glyph">↷</span>
              <span class="label">{entry.label}</span>
            </button>
          {/each}
          {#if redoEntries.length}
            <div class="marker">Current state</div>
          {/if}
          {#each undoEntries as entry, index (undoEntries.length - index)}
            <button
              class="entry"
              title={`Undo ${index + 1} step(s)`}
              onclick={() => restoreUndo(index)}
            >
              <span class="glyph">↶</span>
              <span class="label">{entry.label}</span>
            </button>
          {/each}
          {#if !redoEntries.length}
            <div class="marker">Current state</div>
          {/if}
        {/if}
      </main>
      <footer>
        <p>Click an entry to restore the state right after that action.</p>
        <button class="primary" onclick={close}>Close</button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 2200;
    display: grid;
    place-items: center;
    padding: 1rem;
    background: rgba(0, 0, 0, 0.55);
  }

  .modal {
    width: min(480px, 94vw);
    max-height: 78vh;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto;
    overflow: hidden;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 14px;
    box-shadow: 0 20px 40px rgba(0, 0, 0, 0.45);
  }

  .modal:focus {
    outline: none;
  }

  header,
  footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    padding: 0.85rem 1.1rem;
    border-bottom: 1px solid var(--border);
  }

  footer {
    border-bottom: 0;
    border-top: 1px solid var(--border);
  }

  h2,
  p {
    margin: 0;
  }

  footer p {
    color: var(--text-muted);
    font-size: 0.78rem;
  }

  .icon {
    width: 34px;
    height: 34px;
    color: var(--text);
    background: var(--surface-3);
    border: 1px solid var(--border);
    border-radius: 8px;
    cursor: pointer;
  }

  main {
    min-height: 0;
    overflow: auto;
    display: grid;
    gap: 0.4rem;
    align-content: start;
    padding: 0.9rem 1.1rem;
  }

  .entry {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.55rem 0.7rem;
    color: var(--text);
    background: var(--surface-3);
    border: 1px solid var(--border);
    border-radius: 9px;
    text-align: left;
    cursor: pointer;
  }

  .entry:hover {
    border-color: var(--accent);
  }

  .entry.redo {
    opacity: 0.75;
  }

  .glyph {
    flex: 0 0 auto;
    color: var(--text-muted);
  }

  .label {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .marker {
    padding: 0.15rem 0.2rem;
    color: var(--text-muted);
    font-size: 0.74rem;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .status {
    color: var(--text-muted);
  }

  footer button.primary {
    padding: 0.45rem 0.9rem;
    color: var(--text-on-primary, #fff);
    background: var(--accent);
    border: 1px solid var(--border-strong);
    border-radius: 8px;
    cursor: pointer;
  }
</style>
