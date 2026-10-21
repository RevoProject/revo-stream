<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import FiltersPanel from "./FiltersPanel.svelte";

  type FilterItem = {
    id: string;
    name: string;
    kind: string;
    enabled: boolean;
    locked?: boolean;
    params: Record<string, string>;
  };

  export let open = false;
  export let targetType: "source" | "scene" = "source";
  export let sourceCategory: "graphic" | "image" | "video" | "audio" = "graphic";
  export let sourceType = "";
  export let targetId = "";
  export let targetLabel = "";
  export let filters: FilterItem[] = [];

  const sourceFiltersTitle = () => {
    if (targetType !== "source") return "Scene Filters";
    if (sourceCategory === "audio") return "Audio Filters";
    if (sourceCategory === "image") return "Image Filters";
    if (sourceCategory === "video") return "Video Filters";
    return "Graphic Filters";
  };

  const dispatch = createEventDispatcher<{
    close: void;
    save: { filters: FilterItem[] };
    liveChange: { filters: FilterItem[] };
  }>();

  let panel: FiltersPanel;

  const close = () => dispatch("close");
  const save = () => panel.commitSave();
</script>

{#if open}
  <div class="modal-backdrop" role="button" tabindex="0" onclick={close} onkeydown={(e) => e.key === "Escape" && close()}>
    <div class="modal" role="dialog" aria-modal="true" tabindex="0" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()}>
      <header class="filters-header">
        <h2>{sourceFiltersTitle()}</h2>
        <button class="close-x" onclick={close} aria-label="Close filters">✕</button>
      </header>
      <p class="muted">{targetLabel}</p>

      <FiltersPanel
        bind:this={panel}
        active={open}
        {targetType}
        {targetId}
        {sourceType}
        {filters}
        on:change={(e) => dispatch("liveChange", e.detail)}
        on:save={(e) => dispatch("save", e.detail)}
      />

      <footer>
        <button class="ghost" onclick={close}>Cancel</button>
        <button class="primary" onclick={save}>Save</button>
      </footer>
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
    z-index: 80;
  }

  .modal {
    width: min(1080px, 95vw);
    max-height: 90vh;
    overflow: auto;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 14px;
    padding: 1rem;
    position: relative;
  }

  .filters-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.7rem;
  }

  .filters-header h2 {
    margin: 0;
  }

  .close-x {
    width: 40px;
    height: 40px;
    display: grid;
    place-items: center;
    border-radius: 10px;
    border: 1px solid var(--border);
    background: var(--surface-3, rgba(128, 128, 128, 0.14));
    color: var(--text);
    cursor: pointer;
    line-height: 1;
    transition:
      background-color 0.15s ease,
      color 0.15s ease,
      border-color 0.15s ease;
  }

  .close-x:hover {
    background: var(--surface-4, rgba(128, 128, 128, 0.24));
    border-color: var(--text-muted);
  }

  .muted {
    color: var(--text-muted);
    margin: 0.2rem 0;
  }

  footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.75rem;
    margin-top: 0.8rem;
  }

  footer .ghost {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--text);
    border-radius: 10px;
    padding: 0.6rem 1.1rem;
    cursor: pointer;
  }

  footer .primary {
    background: var(--accent, #4c8dff);
    color: #fff;
    border: 0;
    border-radius: 10px;
    padding: 0.6rem 1.2rem;
    font-weight: 650;
    cursor: pointer;
  }
</style>
