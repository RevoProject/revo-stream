<script lang="ts">
  import { createEventDispatcher } from "svelte";

  export let sourceTypeLabel = "Source";
  export let items: Array<{ name: string; source_type: string; thumbnail?: string }> = [];
  export let loading = false;
  export let creating = false;
  export let error = "";

  const dispatch = createEventDispatcher<{
    close: void;
    back: void;
    createNew: void;
    select: { name: string; source_type: string };
  }>();
</script>

<div class="modal-backdrop" role="button" tabindex="0" onclick={() => dispatch("close")} onkeydown={(e) => e.key === "Escape" && dispatch("close")}>
  <div class="modal" role="dialog" aria-modal="true" aria-label={`Add Source — ${sourceTypeLabel}`} tabindex="0" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()}>
    <header>
      <div>
        <h2>Add Source — {sourceTypeLabel}</h2>
        <p>Reuse an existing source (shared settings) or create a new one.</p>
      </div>
    </header>

    <main>
      {#if error}<p class="status error" role="alert">{error}</p>{/if}
      {#if loading}<p class="status">Loading existing sources and previews...</p>{/if}
      <div class="grid">
        <button class="card create" disabled={creating} onclick={() => dispatch("createNew")}>
          <div class="preview plus" aria-hidden="true">+</div>
          <div class="name">Create new</div>
        </button>
        {#each items as item (item.name)}
          <button class="card" disabled={creating} onclick={() => dispatch("select", item)} title={item.name}>
            <div class="preview">
              {#if item.thumbnail}
                <img src={item.thumbnail} alt={`Preview of ${item.name}`} />
              {:else}
                <span aria-hidden="true">{loading ? "…" : "▧"}</span>
              {/if}
            </div>
            <div class="name">{item.name}</div>
          </button>
        {/each}
      </div>
      {#if !loading && !items.length && !error}
        <p class="empty">No existing {sourceTypeLabel} sources. Create a new one.</p>
      {/if}
    </main>

    <footer>
      <button class="ghost" disabled={creating} onclick={() => dispatch("back")}>Back</button>
      <button class="ghost" disabled={creating} onclick={() => dispatch("close")}>Cancel</button>
    </footer>
  </div>
</div>

<style>
  .modal-backdrop { position: fixed; inset: 0; z-index: 55; display: grid; place-items: center; padding: 1.25rem; background: rgba(0,0,0,.55); }
  .modal { width: min(820px, 95vw); max-height: min(820px, 92vh); display: grid; grid-template-rows: auto minmax(0,1fr) auto; overflow: hidden; background: var(--surface-2); border: 1px solid var(--border); border-radius: 16px; box-shadow: 0 20px 40px rgba(0,0,0,.45); }
  header { padding: 1.05rem 1.25rem; border-bottom: 1px solid var(--border); }
  h2, p { margin: 0; }
  header h2 { margin-top: 15px; }
  header p { margin-top: .2rem; color: var(--text-muted); font-size: .84rem; }
  main { min-height: 0; overflow: auto; padding: 1rem 1.25rem; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(170px, 1fr)); gap: .8rem; }
  .card { min-width: 0; padding: .65rem; display: grid; gap: .5rem; color: var(--text); background: var(--surface-3); border: 1px solid var(--border); border-radius: 12px; cursor: pointer; text-align: center; }
  .card:hover:not(:disabled) { border-color: var(--accent); transform: translateY(-1px); }
  .card:disabled { opacity: .55; cursor: wait; }
  .preview { aspect-ratio: 16 / 9; min-height: 90px; display: grid; place-items: center; overflow: hidden; background: #050505; border: 1px solid #222; border-radius: 9px; color: var(--text-muted); font-size: 2rem; }
  .preview img { width: 100%; height: 100%; object-fit: contain; display: block; }
  .preview.plus { color: var(--accent); font-size: 3rem; font-weight: 300; }
  .name { min-width: 0; overflow-wrap: anywhere; font-size: .9rem; font-weight: 650; line-height: 1.25; }
  .status, .empty { margin: 0 0 .75rem; color: var(--text-muted); }
  .status.error { color: var(--danger, #ef4444); }
  footer { display: flex; justify-content: flex-end; gap: .6rem; padding: .85rem 1.25rem; border-top: 1px solid var(--border); }
  .ghost { padding: .5rem .9rem; color: var(--text); background: var(--surface-3); border: 1px solid var(--border-strong); border-radius: 9px; cursor: pointer; }
  @media (max-width: 520px) { .grid { grid-template-columns: 1fr 1fr; } }
</style>
