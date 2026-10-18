<script lang="ts">
  import { createEventDispatcher, tick } from "svelte";

  export let open = false;
  export let title = "Please confirm";
  export let message = "";
  export let confirmLabel = "Delete";
  export let cancelLabel = "Cancel";

  const dispatch = createEventDispatcher<{ confirm: void; cancel: void }>();
  let confirmEl: HTMLButtonElement | null = null;
  let previouslyFocused: HTMLElement | null = null;

  $: if (open && typeof document !== "undefined") {
    previouslyFocused = (document.activeElement as HTMLElement | null) ?? null;
    void tick().then(() => confirmEl?.focus());
  }

  const close = (outcome: "confirm" | "cancel") => {
    if (!open) return;
    dispatch(outcome);
    if (outcome === "cancel") {
      previouslyFocused?.focus?.();
    }
    previouslyFocused = null;
  };

  const handleKeydown = (event: KeyboardEvent) => {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      close("cancel");
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      close("confirm");
      return;
    }
    event.stopPropagation();
  };
</script>

<svelte:window on:keydown={open ? handleKeydown : undefined} />

{#if open}
  <div
    class="backdrop"
    role="button"
    tabindex="-1"
    onclick={() => close("cancel")}
    onkeydown={(e) => e.key === "Escape" && close("cancel")}
  >
    <div
      class="modal"
      role="alertdialog"
      aria-modal="true"
      aria-label={title}
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={handleKeydown}
    >
      <h2>{title}</h2>
      <p class="message">{message}</p>
      <div class="actions">
        <button class="cancel" type="button" onclick={() => close("cancel")}>{cancelLabel}</button>
        <button class="confirm" type="button" bind:this={confirmEl} onclick={() => close("confirm")}>
          {confirmLabel}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 2400;
    display: grid;
    place-items: center;
    padding: 1rem;
    background: rgba(0, 0, 0, 0.55);
  }

  .modal {
    width: min(440px, 94vw);
    display: grid;
    gap: 0.8rem;
    padding: 1.2rem 1.3rem;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 14px;
    box-shadow: 0 20px 40px rgba(0, 0, 0, 0.45);
  }

  h2,
  p {
    margin: 0;
  }

  h2 {
    font-size: 1.05rem;
  }

  .message {
    color: var(--text-muted);
    font-size: 0.9rem;
    line-height: 1.45;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.6rem;
    margin-top: 0.3rem;
  }

  button {
    padding: 0.5rem 1rem;
    border-radius: 9px;
    cursor: pointer;
    font-weight: 600;
  }

  .cancel {
    color: var(--text);
    background: var(--surface-3);
    border: 1px solid var(--border-strong);
  }

  .confirm {
    color: var(--text-on-danger, #fff);
    background: var(--danger, #dc2626);
    border: 1px solid transparent;
  }
</style>
