<script lang="ts">
  export let open = false;
  export let value = "#ffffff";
  export let recent: string[] = [];

  export let onClose: () => void;
  export let onSave: () => void;
  export let onValueChange: (value: string) => void;
  export let normalizeColor: (value: string) => string;
  export let handleBackdropKey: (event: KeyboardEvent, closeFn: () => void | Promise<void>) => void;
</script>

{#if open}
  <div class="modal-backdrop" role="button" tabindex="0" on:click={onClose} on:keydown={(e) => handleBackdropKey(e, onClose)}>
    <div class="quick-text-modal" role="dialog" tabindex="-1" aria-modal="true" aria-label="Quick color" on:click|stopPropagation on:keydown|stopPropagation>
      <h3>Change color</h3>
      <div class="quick-color-row">
        <input type="text" value={value} placeholder="#ffffff" on:input={(e) => onValueChange(normalizeColor((e.currentTarget as HTMLInputElement).value))} />
        <input type="color" value={normalizeColor(value)} on:input={(e) => onValueChange(normalizeColor((e.currentTarget as HTMLInputElement).value))} />
      </div>
      {#if recent.length}
        <div class="quick-color-history">
          {#each recent as color}
            <button type="button" class="quick-color-chip" title={color} style={`background:${color};`} on:click={() => onValueChange(color)}></button>
          {/each}
        </div>
      {/if}
      <div class="quick-text-actions">
        <button class="ghost" on:click={onClose}>Cancel</button>
        <button class="primary" on:click={onSave}>Save</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: #000a;
    z-index: 2000;
  }

  .quick-text-modal {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    background: var(--surface-2);
    color: var(--text);
    border-radius: 12px;
    box-shadow: 0 4px 32px #000a;
    z-index: 2010;
    border: 1px solid var(--border);
    padding: 1rem;
    width: min(640px, calc(100vw - 2rem));
    min-width: 460px;
    display: grid;
    gap: 0.75rem;
    box-sizing: border-box;
    overflow: hidden;
  }

  .quick-color-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 0.6rem;
    align-items: center;
  }

  .quick-color-row input[type="text"] {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    color: var(--text);
    padding: 0.55rem 0.7rem;
  }

  .quick-color-row input[type="color"] {
    width: 42px;
    height: 36px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: transparent;
    padding: 0.2rem;
  }

  .quick-color-history {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
  }

  .quick-color-chip {
    width: 22px;
    height: 22px;
    border-radius: 999px;
    border: 1px solid color-mix(in srgb, var(--border) 70%, #fff 30%);
    cursor: pointer;
    box-shadow: inset 0 0 0 1px #0004;
  }

  .quick-text-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }

  .quick-text-actions button {
    min-width: 92px;
    height: 34px;
    border-radius: 8px;
    border: 1px solid var(--border-strong);
    padding: 0 0.8rem;
    font-weight: 600;
    cursor: pointer;
  }

  .quick-text-actions button.ghost {
    background: var(--surface-3);
    color: var(--text);
  }

  .quick-text-actions button.primary {
    background: var(--accent);
    color: #fff;
    border-color: color-mix(in srgb, var(--accent) 55%, #000 45%);
  }
</style>
