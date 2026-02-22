<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import type { DemoSource } from "../../types";

  export let source: DemoSource | null;
  const dispatch = createEventDispatcher();
  const close = () => dispatch("close");
</script>

{#if source}
  <div
    class="modal-backdrop"
    role="button"
    tabindex="0"
    onclick={close}
    onkeydown={(e) => (e.key === "Escape" ? close() : null)}
  >
    <div
      class="messagebox"
      role="dialog"
      aria-modal="true"
      tabindex="0"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}
    >
      <h3>Source details</h3>
      <p>
        Name: {source.name}
        <br />
        Type: {source.source_type}
      </p>
      <div class="modal-actions">
        <button class="primary" onclick={close}>OK</button>
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

  .messagebox {
    width: min(520px, 92vw);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 16px;
    box-shadow: 0 20px 40px rgba(0, 0, 0, 0.45);
    display: grid;
    gap: 0.75rem;
    padding: 1.25rem;
  }

  h3 {
    margin: 0;
    font-size: 1.1rem;
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.75rem;
  }

  button {
    border: none;
    border-radius: 14px;
    padding: 0.7rem 1.2rem;
    font-weight: 600;
    cursor: pointer;
  }

  button.primary {
    background: var(--accent);
    color: #ffffff;
  }

  button:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
</style>
