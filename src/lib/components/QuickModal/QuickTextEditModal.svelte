<script lang="ts">
  export let open = false;
  export let value = "";
  export let modalEl: HTMLDivElement | null = null;
  export let allowDraggablePopups = false;
  export let openAdditionalSettingsInWindows = false;
  export let dragX = 0;
  export let dragY = 0;

  export let onClose: () => void;
  export let onSave: () => void;
  export let onValueChange: (value: string) => void;
  export let handleBackdropKey: (event: KeyboardEvent, closeFn: () => void | Promise<void>) => void;
  export let beginDrag: (event: PointerEvent) => void;
  export let moveDrag: (event: PointerEvent) => void;
  export let endDrag: (event: PointerEvent) => void;
</script>

{#if open}
  <div class="modal-backdrop" role="button" tabindex="0" on:click={onClose} on:keydown={(e) => handleBackdropKey(e, onClose)}>
    <div
      class="quick-text-modal"
      class:draggable-popup={allowDraggablePopups && !openAdditionalSettingsInWindows}
      bind:this={modalEl}
      role="dialog"
      tabindex="-1"
      aria-modal="true"
      aria-label="Text edit"
      style={`--quick-text-dx:${dragX}px; --quick-text-dy:${dragY}px;`}
      on:pointerdown={beginDrag}
      on:pointermove={moveDrag}
      on:pointerup={endDrag}
      on:pointercancel={endDrag}
      on:click|stopPropagation
      on:keydown|stopPropagation
    >
      <h3>Text Edit</h3>
      <textarea rows="2" value={value} placeholder="Enter text..." on:input={(e) => onValueChange((e.currentTarget as HTMLTextAreaElement).value)}></textarea>
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

  .quick-text-modal.draggable-popup {
    transform: translate(calc(-50% + var(--quick-text-dx, 0px)), calc(-50% + var(--quick-text-dy, 0px)));
  }

  .quick-text-modal h3 {
    margin: 0;
  }

  .quick-text-modal textarea {
    width: 100%;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    color: var(--text);
    padding: 0.6rem 0.75rem;
    resize: vertical;
    box-sizing: border-box;
    min-height: 90px;
  }

  .quick-text-modal textarea::placeholder {
    color: var(--text-muted);
  }

  .quick-text-modal textarea:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 30%, transparent);
    outline: none;
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
