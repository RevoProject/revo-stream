<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import DebugConsole from "../../../routes/debug-console/+page.svelte";

  export let open: boolean;
  export let onClose: () => void;

  const dispatch = createEventDispatcher();

  function handleBackdropClick() {
    onClose();
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      onClose();
    }
  }
</script>

{#if open}
  <div class="modal-backdrop"
       role="button"
       tabindex="0"
       on:click={handleBackdropClick}
       on:keydown={handleKeydown}
  >
    <div class="modal"
         role="dialog"
         aria-modal="true"
         tabindex="0"
         on:click|stopPropagation
         on:keydown|stopPropagation>
      <div class="modal-header">
        <h2>Debug Console</h2>
      </div>
      <div class="modal-body">
        <DebugConsole />
      </div>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    display: grid;
    place-items: center;
    z-index: 50;
    padding: 1.25rem;
  }

  .modal {
    width: min(980px, 94vw);
    max-height: 90vh;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 16px;
    box-shadow: 0 20px 40px rgba(0, 0, 0, 0.45);
    display: flex;
    flex-direction: column;
  }

  .modal-header {
    padding: 1.25rem;
    border-bottom: 1px solid var(--border);
  }

  .modal-header h2 {
    margin: 0;
    font-size: 1.2rem;
  }

  .modal-body {
    flex: 1;
    overflow: auto;
    padding: 1.25rem;
  }
</style>
