<script lang="ts">
  import { createEventDispatcher } from "svelte";

  export let newSourceType: string;
  export let sourceTypes: { id: string; label: string }[];
  export let externalSourceTypes: { id: string; label: string }[] = [];

  const dispatch = createEventDispatcher();
  const close = () => dispatch("close");
  const add = () => dispatch("add");

  const iconMap: Record<string, string> = {
    browser_source: `
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20Zm7.7 9h-3.2a15.7 15.7 0 0 0-1.2-5.1 8 8 0 0 1 4.4 5.1Zm-5.3 0H9.6A13.7 13.7 0 0 1 12 4.6 13.7 13.7 0 0 1 14.4 11Zm-6.8 2h6.8A13.7 13.7 0 0 1 12 19.4 13.7 13.7 0 0 1 9.6 13Zm6.8 0h4.8a13.7 13.7 0 0 1-2.4 6.4 13.7 13.7 0 0 1-2.4-6.4Zm-6.9-2H4.3a8 8 0 0 1 4.4-5.1A15.7 15.7 0 0 0 7.5 11Zm0 2a15.7 15.7 0 0 0 1.2 5.1 8 8 0 0 1-4.4-5.1h3.2Z"/></svg>
    `,
    image_source: `
      <svg viewBox="0 0 24 24" fill="none" aria-hidden="true"><path fill-rule="evenodd" clip-rule="evenodd" d="M23 4C23 2.34315 21.6569 1 20 1H4C2.34315 1 1 2.34315 1 4V20C1 21.6569 2.34315 23 4 23H20C21.6569 23 23 21.6569 23 20V4ZM21 4C21 3.44772 20.5523 3 20 3H4C3.44772 3 3 3.44772 3 4V20C3 20.5523 3.44772 21 4 21H20C20.5523 21 21 20.5523 21 20V4Z" fill="currentColor"></path><path d="M4.80665 17.5211L9.1221 9.60947C9.50112 8.91461 10.4989 8.91461 10.8779 9.60947L14.0465 15.4186L15.1318 13.5194C15.5157 12.8476 16.4843 12.8476 16.8682 13.5194L19.1451 17.5039C19.526 18.1705 19.0446 19 18.2768 19H5.68454C4.92548 19 4.44317 18.1875 4.80665 17.5211Z" fill="currentColor"></path><path d="M18 8C18 9.10457 17.1046 10 16 10C14.8954 10 14 9.10457 14 8C14 6.89543 14.8954 6 16 6C17.1046 6 18 6.89543 18 8Z" fill="currentColor"></path></svg>
    `,
    ffmpeg_source: `
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 6a2 2 0 0 1 2-2h4l2 2h6a2 2 0 0 1 2 2v7a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3V6Zm4 6 3-2v4l-3-2Zm7-2 3 2-3 2v-4Z"/></svg>
    `,
    text_ft2_source_v2: `
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 5h16v3h-2V7H6v1H4V5Zm6 5h4v9h-4v-9Z"/></svg>
    `,
    color_source_v2: `
      <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M8 .5C3.58.5 0 3.86 0 8s3.58 7.5 8 7.5c4.69 0 1.04-2.83 2.79-4.55.76-.75 1.63-.87 2.44-.87.37 0 .73.03 1.06.03.99 0 1.72-.23 1.72-2.1C16 3.86 12.42.5 8 .5zm6.65 8.32c-.05.01-.16.02-.37.02-.14 0-.29 0-.45-.01-.19 0-.39-.01-.61-.01-.89 0-2.19.13-3.32 1.23-1.17 1.16-.9 2.6-.74 3.47.03.18.08.44.09.6-.16.05-.52.13-1.26.13-3.72 0-6.75-2.8-6.75-6.25S4.28 1.75 8 1.75s6.75 2.8 6.75 6.25c0 .5-.06.74-.1.82z" fill="currentColor"></path><path d="M5.9 9.47c-1.03 0-1.86.8-1.86 1.79s.84 1.79 1.86 1.79 1.86-.8 1.86-1.79-.84-1.79-1.86-1.79zm0 2.35c-.35 0-.64-.25-.64-.56s.29-.56.64-.56.64.25.64.56-.29.56-.64.56zm-.2-4.59c0-.99-.84-1.79-1.86-1.79s-1.86.8-1.86 1.79.84 1.79 1.86 1.79 1.86-.8 1.86-1.79zm-1.86.56c-.35 0-.64-.25-.64-.56s.29-.56.64-.56.64.25.64.56-.29.56-.64.56zM7.37 2.5c-1.03 0-1.86.8-1.86 1.79s.84 1.79 1.86 1.79 1.86-.8 1.86-1.79S8.39 2.5 7.37 2.5zm0 2.35c-.35 0-.64-.25-.64-.56s.29-.56.64-.56.64.25.64.56-.29.56-.64.56zm2.47 1.31c0 .99.84 1.79 1.86 1.79s1.86-.8 1.86-1.79-.84-1.79-1.86-1.79-1.86.8-1.86 1.79zm2.5 0c0 .31-.29.56-.64.56s-.64-.25-.64-.56.29-.56.64-.56.64.25.64.56z" fill="currentColor"></path></svg>
    `,
    window_capture: `
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 5a3 3 0 0 1 3-3h10a3 3 0 0 1 3 3v10a3 3 0 0 1-3 3h-4l2 2v1H9v-1l2-2H7a3 3 0 0 1-3-3V5Zm3-1a1 1 0 0 0-1 1v10a1 1 0 0 0 1 1h10a1 1 0 0 0 1-1V5a1 1 0 0 0-1-1H7Z"/></svg>
    `,
    xcomposite_input: `
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 6a2 2 0 0 1 2-2h8v4H5v10h10v-2h4v2a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V6Zm12-2h4v4h-4V4Zm0 6h4v4h-4v-4Z"/></svg>
    `,
    pulse_input_capture: `
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 3a4 4 0 0 1 4 4v4a4 4 0 1 1-8 0V7a4 4 0 0 1 4-4Zm-6 8h2a4 4 0 0 0 8 0h2a6 6 0 0 1-5 5.9V20h-2v-3.1A6 6 0 0 1 6 11Z"/></svg>
    `,
    pulse_output_capture: `
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 9v6h4l5 4V5L8 9H4Zm12 2h2v2h-2v-2Zm3-3h2v8h-2V8Z"/></svg>
    `
  };

  const handlePick = (id: string) => {
    newSourceType = id;
    add();
  };
</script>

<div
  class="modal-backdrop"
  role="button"
  tabindex="0"
  onclick={close}
  onkeydown={(e) => (e.key === "Escape" ? close() : null)}
>
  <div
    class="modal"
    role="dialog"
    aria-modal="true"
    tabindex="0"
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.stopPropagation()}
  >
    <header class="modal-header">
      <h2>Add Source</h2>
    </header>
    <div class="modal-body">
      <div class="source-grid builtin-grid" role="list">
        {#each sourceTypes as type}
          <button
            type="button"
            class="source-card"
            onclick={() => handlePick(type.id)}
            aria-label={`Add ${type.label}`}
          >
            <div class="icon" aria-hidden="true">
              {@html iconMap[type.id] ?? ''}
            </div>
            <div class="label">{type.label}</div>
          </button>
        {/each}
      </div>

      {#if externalSourceTypes.length}
        <section class="external-section">
          <h3>External</h3>
          <div class="source-grid external-grid" role="list">
            {#each externalSourceTypes as type}
              <button
                type="button"
                class="source-card external-card"
                onclick={() => handlePick(type.id)}
                aria-label={`Add ${type.label}`}
              >
                <div class="icon" aria-hidden="true">
                  {@html iconMap[type.id] ?? '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 2a4 4 0 0 1 4 4v1h2a4 4 0 0 1 4 4v2a4 4 0 0 1-4 4h-2v1a4 4 0 0 1-8 0v-1H6a4 4 0 0 1-4-4v-2a4 4 0 0 1 4-4h2V6a4 4 0 0 1 4-4Zm0 2a2 2 0 0 0-2 2v3h4V6a2 2 0 0 0-2-2Zm6 5H6a2 2 0 0 0-2 2v2a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-2a2 2 0 0 0-2-2Zm-8 8v1a2 2 0 1 0 4 0v-1h-4Z"/></svg>'}
                </div>
                <div class="label">{type.label}</div>
              </button>
            {/each}
          </div>
        </section>
      {/if}
    </div>
    <footer class="modal-actions">
      <button class="ghost" onclick={close}>Cancel</button>
    </footer>
  </div>
</div>

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
    width: min(760px, 94vw);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 16px;
    box-shadow: 0 20px 40px rgba(0, 0, 0, 0.45);
    display: grid;
    gap: 1rem;
    padding: 1.25rem;
  }

  h2 {
    margin: 0;
    font-size: 1.2rem;
  }

  .source-grid {
    display: grid;
    gap: 0.9rem;
  }

  .builtin-grid {
    grid-template-columns: repeat(4, minmax(150px, 1fr));
    max-height: 460px;
    overflow-y: auto;
    padding-right: 0.35rem;
  }

  .external-section {
    margin-top: 1rem;
    display: grid;
    gap: 0.65rem;
  }

  .external-section h3 {
    margin: 0;
    font-size: 0.9rem;
    font-weight: 700;
    color: var(--text-muted);
  }

  .source-card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 1.05rem 0.85rem;
    display: grid;
    gap: 0.5rem;
    place-items: center;
    color: var(--text);
    cursor: pointer;
    transition: transform 0.15s ease, border-color 0.15s ease, background 0.15s ease;
  }

  .external-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 0.55rem;
    max-height: 250px;
    overflow-y: auto;
    padding-right: 0.35rem;
  }

  .external-card {
    padding: 0.5rem 0.65rem;
    border-radius: 10px;
    display: grid;
    grid-template-columns: 20px 1fr;
    align-items: center;
    justify-items: start;
    gap: 0.5rem;
  }

  .external-card .icon {
    width: 20px;
    height: 20px;
  }

  .external-card .icon :global(svg) {
    width: 20px;
    height: 20px;
  }

  .external-card .label {
    font-size: 0.86rem;
    text-align: left;
    line-height: 1.2;
  }

  @media (max-width: 760px) {
    .builtin-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
      max-height: 360px;
    }
  }

  .source-card:hover {
    transform: translateY(-2px);
    border-color: var(--accent);
    background: var(--surface-3);
  }

  .source-card:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .icon {
    width: 32px;
    height: 32px;
  }

  .icon :global(svg) {
    width: 32px;
    height: 32px;
    display: block;
    fill: var(--icon-color, var(--text));
  }

  .label {
    font-size: 0.95rem;
    text-align: center;
    white-space: nowrap;
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

  button.ghost {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--text);
  }

  button:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
</style>
