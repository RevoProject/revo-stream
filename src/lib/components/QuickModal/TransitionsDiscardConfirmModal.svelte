<script lang="ts">
  import { tick } from "svelte";

  export let open = false;
  export let onCancel: () => void;
  export let onConfirm: () => void;

  let cancelBtnEl: HTMLButtonElement | null = null;
  let confirmBtnEl: HTMLButtonElement | null = null;
  let focusConfirm = false;
  let wasOpen = false;

  const focusActiveButton = async () => {
    await tick();
    if (focusConfirm) {
      confirmBtnEl?.focus();
    } else {
      cancelBtnEl?.focus();
    }
  };

  const moveFocus = (toConfirm: boolean) => {
    focusConfirm = toConfirm;
    void focusActiveButton();
  };

  const handleModalKeydown = (event: KeyboardEvent) => {
    if (event.key === "ArrowLeft" || event.key === "ArrowUp") {
      event.preventDefault();
      moveFocus(false);
      return;
    }
    if (event.key === "ArrowRight" || event.key === "ArrowDown") {
      event.preventDefault();
      moveFocus(true);
      return;
    }
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      if (focusConfirm) {
        onConfirm();
      } else {
        onCancel();
      }
    }
  };

  $: if (open && !wasOpen) {
    focusConfirm = false;
    void focusActiveButton();
  }

  $: wasOpen = open;
</script>

{#if open}
  <div class="modal-backdrop" role="button" tabindex="0" on:click={onCancel} on:keydown={(e) => e.stopPropagation()}>
    <div
      class="quick-text-modal stream-confirm-modal"
      role="dialog"
      tabindex="0"
      aria-modal="true"
      aria-label="Discard transition changes"
      on:click|stopPropagation
      on:keydown={(e) => {
        e.stopPropagation();
        handleModalKeydown(e);
      }}
    >
      <div class="audio-filters-header transitions-header">
        <h3>Unsaved changes</h3>
      </div>
      <p class="stream-confirm-copy">Discard unsaved Transition changes?</p>
      <div class="stream-confirm-actions">
        <button bind:this={cancelBtnEl} type="button" class="stream-confirm-btn ghost" on:click={onCancel}>Back to Transitions</button>
        <button bind:this={confirmBtnEl} type="button" class="stream-confirm-btn primary" on:click={onConfirm}>Discard</button>
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

  .stream-confirm-modal {
    width: min(460px, calc(100vw - 2rem));
    min-width: 320px;
    gap: 1rem;
  }

  .audio-filters-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }

  .audio-filters-header h3 {
    margin: 0;
  }

  .stream-confirm-copy {
    margin: 0;
    color: var(--text);
    font-size: 0.98rem;
  }

  .stream-confirm-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.65rem;
  }

  .stream-confirm-btn {
    border: 1px solid var(--border-strong);
    border-radius: 10px;
    padding: 0.5rem 1rem;
    background: var(--surface-3);
    color: var(--text);
    cursor: pointer;
    font-weight: 600;
  }

  .stream-confirm-btn.primary {
    background: var(--accent);
    color: #fff;
    border-color: color-mix(in srgb, var(--accent) 70%, #000 30%);
  }

  .stream-confirm-btn.ghost {
    background: transparent;
  }
</style>
