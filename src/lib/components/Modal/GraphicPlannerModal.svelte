<script lang="ts">
  import { createEventDispatcher, tick } from "svelte";
  import SourceTransform from "../../../routes/source-transform/+page.svelte";

  export let open: boolean;
  export let onClose: () => void;

  const dispatch = createEventDispatcher();

  let dialogEl: HTMLDivElement | null = null;
  let previouslyFocused: HTMLElement | null = null;

  $: if (open && typeof document !== "undefined") {
    previouslyFocused = (document.activeElement as HTMLElement | null) ?? null;
    void tick().then(() => dialogEl?.focus());
  }

  const close = () => {
    if (!open) return;
    dispatch("close");
    onClose();
    previouslyFocused?.focus?.();
    previouslyFocused = null;
  };

  function handleWindowKeydown(event: KeyboardEvent) {
    if (!open || event.key !== "Escape") return;
    const target = event.target as HTMLElement | null;
    if (target?.isContentEditable) return;
    const tag = target?.tagName?.toLowerCase();
    if (tag === "input" || tag === "textarea" || tag === "select") return;
    event.preventDefault();
    event.stopPropagation();
    close();
  }
</script>

<svelte:window on:keydown={handleWindowKeydown} />

{#if open}
  <div
    class="modal-backdrop"
    role="button"
    tabindex="-1"
    onclick={close}
    onkeydown={(e) => (e.key === "Escape" ? close() : null)}
  >
    <div
      class="modal"
      role="dialog"
      aria-modal="true"
      aria-label="Graphic Planner"
      tabindex="-1"
      bind:this={dialogEl}
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}
    >
      <header class="modal-header">
        <h2>Graphic Planner</h2>
        <button type="button" class="modal-close" aria-label="Close graphic planner" onclick={close}>
          <svg viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M6.4 5 12 10.6 17.6 5 19 6.4 13.4 12l5.6 5.6-1.4 1.4L12 13.4 6.4 19 5 17.6 10.6 12 5 6.4 6.4 5Z" /></svg>
        </button>
      </header>
      <div class="modal-body">
        <SourceTransform embedded requestClose={close} />
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
    width: min(920px, 94vw);
    height: min(84vh, 820px);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 16px;
    box-shadow: 0 20px 40px rgba(0, 0, 0, 0.45);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .modal:focus {
    outline: none;
  }

  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    padding: 0.9rem 1.25rem;
    border-bottom: 1px solid var(--border);
    flex: 0 0 auto;
  }

  .modal-header h2 {
    margin: 0;
    font-size: 1.2rem;
  }

  .modal-close {
    width: 34px;
    height: 34px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    border: 1px solid var(--border-strong);
    border-radius: 8px;
    background: var(--surface-3);
    color: var(--text);
    cursor: pointer;
  }

  .modal-close:hover {
    background: var(--surface-2);
  }

  .modal-close svg {
    width: 16px;
    height: 16px;
    display: block;
  }

  .modal-body {
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
    padding: 1rem 1.25rem 1.25rem;
  }
</style>
