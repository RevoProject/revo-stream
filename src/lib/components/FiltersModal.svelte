<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { createEventDispatcher } from "svelte";

  type FilterItem = {
    id: string;
    name: string;
    kind: string;
    enabled: boolean;
    locked?: boolean;
    params: Record<string, string>;
  };

  type FilterFieldDef = {
    key: string;
    label: string;
    kind: "number" | "text" | "checkbox";
    min?: number;
    max?: number;
    step?: number;
    defaultValue: string;
  };

  export let open = false;
  export let targetType: "source" | "scene" = "source";
  export let targetId = "";
  export let targetLabel = "";
  export let filters: FilterItem[] = [];

  const dispatch = createEventDispatcher();

  const filterPresetFields: Record<string, FilterFieldDef[]> = {
    crop_pad: [
      { key: "left", label: "Left", kind: "number", min: -8192, max: 8192, step: 1, defaultValue: "0" },
      { key: "right", label: "Right", kind: "number", min: -8192, max: 8192, step: 1, defaultValue: "0" },
      { key: "top", label: "Top", kind: "number", min: -8192, max: 8192, step: 1, defaultValue: "0" },
      { key: "bottom", label: "Bottom", kind: "number", min: -8192, max: 8192, step: 1, defaultValue: "0" },
      { key: "relative", label: "Relative", kind: "checkbox", defaultValue: "false" }
    ],
    color_correction: [
      { key: "brightness", label: "Brightness", kind: "number", min: -1, max: 1, step: 0.01, defaultValue: "0" },
      { key: "contrast", label: "Contrast", kind: "number", min: -2, max: 2, step: 0.01, defaultValue: "0" },
      { key: "saturation", label: "Saturation", kind: "number", min: -1, max: 5, step: 0.01, defaultValue: "1" },
      { key: "hue_shift", label: "Hue Shift", kind: "number", min: -180, max: 180, step: 1, defaultValue: "0" },
      { key: "opacity", label: "Opacity", kind: "number", min: 0, max: 1, step: 0.01, defaultValue: "1" }
    ]
  };

  let draft: FilterItem[] = [];
  let selectedFilterId: string | null = null;
  let selectedFilter: FilterItem | null = null;
  let selectedFilterPresetFields: FilterFieldDef[] = [];
  let newKind = "color_correction";
  let renamingFilterId: string | null = null;
  let renameValue = "";
  let contextMenu = { open: false, x: 0, y: 0, filterId: null as string | null };
  let previewImageUrl = "";
  let previewRefreshTimer: ReturnType<typeof setTimeout> | null = null;
  let wasOpen = false;

  const isTruthy = (value: string) => ["1", "true", "yes", "on"].includes(String(value ?? "").trim().toLowerCase());

  const close = () => dispatch("close");
  const save = () => dispatch("save", { filters: draft });

  const refreshPreviewScreenshot = async () => {
    try {
      const image = await invoke<string>("obs_take_screenshot", {
        width: 640,
        height: 360,
        sourceId: targetType === "source" && targetId.trim().length ? targetId : undefined
      });
      if (typeof image === "string" && image.startsWith("data:image")) {
        previewImageUrl = image;
      }
    } catch {
      // ignore when backend is unavailable
    }
  };

  const schedulePreviewRefresh = () => {
    if (!open) return;
    if (previewRefreshTimer) clearTimeout(previewRefreshTimer);
    previewRefreshTimer = setTimeout(() => {
      previewRefreshTimer = null;
      void refreshPreviewScreenshot();
    }, 140);
  };

  const updateFilter = (id: string, patch: Partial<FilterItem>) => {
    draft = draft.map((f) => (f.id === id && !f.locked ? { ...f, ...patch } : f));
  };

  const updateFilterPresetField = (filterId: string, key: string, value: string) => {
    const filter = draft.find((f) => f.id === filterId);
    if (!filter || filter.locked) return;
    const params = { ...(filter.params ?? {}) };
    params[key] = value;
    updateFilter(filterId, { params });
  };

  const addFilter = () => {
    const id = crypto.randomUUID ? crypto.randomUUID() : `${Date.now()}-${Math.random()}`;
    const defaultName = `Filter ${draft.length + 1}`;
    draft = [
      ...draft,
      {
        id,
        name: defaultName,
        kind: newKind,
        enabled: true,
        locked: false,
        params: {}
      }
    ];
    selectedFilterId = id;
    schedulePreviewRefresh();
  };

  const resetSelectedFilterToDefaults = () => {
    if (!selectedFilter || selectedFilter.locked) return;
    const activeFilter = selectedFilter;
    const index = draft.findIndex((f) => f.id === activeFilter.id);
    if (index < 0) return;
    const fields = filterPresetFields[activeFilter.kind] ?? [];
    const defaultParams: Record<string, string> = {};
    for (const field of fields) {
      defaultParams[field.key] = field.defaultValue;
    }
    updateFilter(activeFilter.id, {
      enabled: true,
      name: `Filter ${index + 1}`,
      params: defaultParams
    });
    schedulePreviewRefresh();
  };

  const removeFilter = (id: string) => {
    const next = draft.filter((f) => f.id !== id);
    draft = next;
    if (selectedFilterId === id) {
      selectedFilterId = next[0]?.id ?? null;
    }
    contextMenu = { open: false, x: 0, y: 0, filterId: null };
    schedulePreviewRefresh();
  };

  const moveFilter = (id: string, direction: "up" | "down") => {
    const index = draft.findIndex((f) => f.id === id);
    if (index < 0) return;
    const current = draft[index];
    if (current?.locked) return;
    const target = direction === "up" ? index - 1 : index + 1;
    if (target < 0 || target >= draft.length) return;
    const next = [...draft];
    const [moved] = next.splice(index, 1);
    next.splice(target, 0, moved);
    draft = next;
    schedulePreviewRefresh();
  };

  const renameParam = (id: string, oldKey: string, newKeyRaw: string) => {
    const nextKey = newKeyRaw.trim();
    if (!nextKey || nextKey === oldKey) return;
    draft = draft.map((f) => {
      if (f.id !== id || f.locked) return f;
      const params = { ...(f.params ?? {}) };
      const value = params[oldKey] ?? "";
      delete params[oldKey];
      params[nextKey] = value;
      return { ...f, params };
    });
    schedulePreviewRefresh();
  };

  const setParam = (id: string, key: string, value: string) => {
    draft = draft.map((f) => {
      if (f.id !== id || f.locked) return f;
      return {
        ...f,
        params: {
          ...(f.params ?? {}),
          [key]: value
        }
      };
    });
    schedulePreviewRefresh();
  };

  const openContextMenu = (event: MouseEvent, filterId: string) => {
    event.preventDefault();
    event.stopPropagation();
    selectedFilterId = filterId;
    contextMenu = { open: true, x: event.clientX, y: event.clientY, filterId };
  };

  const closeContextMenu = () => {
    contextMenu = { open: false, x: 0, y: 0, filterId: null };
  };

  const startRenameFilter = (id: string) => {
    const filter = draft.find((f) => f.id === id);
    if (!filter || filter.locked) return;
    renamingFilterId = id;
    renameValue = filter.name ?? "";
    closeContextMenu();
  };

  const commitRenameFilter = (id: string) => {
    updateFilter(id, { name: renameValue });
    renamingFilterId = null;
  };

  const toggleFilterLock = (id: string) => {
    draft = draft.map((f) => (f.id === id ? { ...f, locked: !f.locked } : f));
    schedulePreviewRefresh();
    closeContextMenu();
  };

  $: if (open && !wasOpen) {
    draft = filters.map((f) => ({ ...f, locked: Boolean(f.locked), params: { ...(f.params ?? {}) } }));
    selectedFilterId = draft[0]?.id ?? null;
    renamingFilterId = null;
    renameValue = "";
    contextMenu = { open: false, x: 0, y: 0, filterId: null };
    schedulePreviewRefresh();
  }

  $: if (!open && wasOpen) {
    previewImageUrl = "";
  }

  $: wasOpen = open;

  $: selectedFilter = selectedFilterId ? draft.find((f) => f.id === selectedFilterId) ?? null : null;
  $: selectedFilterPresetFields = selectedFilter ? (filterPresetFields[selectedFilter.kind] ?? []) : [];

  $: if (open && selectedFilter && selectedFilterPresetFields.length) {
    const nextParams = { ...(selectedFilter.params ?? {}) };
    let changed = false;
    for (const field of selectedFilterPresetFields) {
      if (!(field.key in nextParams)) {
        nextParams[field.key] = field.defaultValue;
        changed = true;
      }
    }
    if (changed) {
      updateFilter(selectedFilter.id, { params: nextParams });
    }
  }

  $: if (open) {
    draft;
    selectedFilterId;
    schedulePreviewRefresh();
  }
</script>

{#if open}
  <div class="modal-backdrop" role="button" tabindex="0" on:click={close} on:keydown={(e) => e.key === "Escape" && close()}>
    <div class="modal" role="dialog" aria-modal="true" tabindex="0" on:click={(e) => e.stopPropagation()} on:keydown={(e) => e.stopPropagation()}>
      <header class="filters-header">
        <h2>{targetType === "source" ? "Graphic Filters" : "Scene Filters"}</h2>
        <button class="close-x" on:click={close} aria-label="Close filters">✕</button>
      </header>
      <p class="muted">{targetLabel}</p>

      <div class="filters-shell">
        <aside class="filters-sidebar">
          <div class="filters-sidebar-list">
            {#if !draft.length}
              <p class="muted">No filters.</p>
            {:else}
              {#each draft as filter, index (filter.id)}
                <div
                  class="filters-list-item"
                  class:selected={selectedFilterId === filter.id}
                  class:locked={Boolean(filter.locked)}
                  role="button"
                  tabindex="0"
                  on:click={() => (selectedFilterId = filter.id)}
                  on:keydown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      selectedFilterId = filter.id;
                    }
                  }}
                  on:contextmenu={(e) => openContextMenu(e, filter.id)}
                >
                  {#if renamingFilterId === filter.id}
                    <input
                      class="filter-chip-rename"
                      value={renameValue}
                      on:input={(e) => (renameValue = (e.currentTarget as HTMLInputElement).value)}
                      on:blur={() => commitRenameFilter(filter.id)}
                      on:keydown={(e) => {
                        if (e.key === "Enter") commitRenameFilter(filter.id);
                        if (e.key === "Escape") renamingFilterId = null;
                      }}
                    />
                  {:else}
                    <button class="filter-chip" on:click={() => (selectedFilterId = filter.id)}>{filter.name.trim() || `Filter ${index + 1}`}</button>
                  {/if}
                  <div class="filter-order-actions">
                    <button class="order-btn" aria-label="Move filter up" disabled={filter.locked || index === 0} on:click={() => moveFilter(filter.id, "up")}> 
                      <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true" focusable="false">
                        <path d="M6 2 L2.2 6.3 H4.7 V10 H7.3 V6.3 H9.8 Z" fill="currentColor" />
                      </svg>
                    </button>
                    <button class="order-btn" aria-label="Move filter down" disabled={filter.locked || index === draft.length - 1} on:click={() => moveFilter(filter.id, "down")}> 
                      <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true" focusable="false">
                        <path d="M6 10 L9.8 5.7 H7.3 V2 H4.7 V5.7 H2.2 Z" fill="currentColor" />
                      </svg>
                    </button>
                  </div>
                </div>
              {/each}
            {/if}
          </div>

          <div class="filters-sidebar-add">
            <select bind:value={newKind}>
              <option value="color_correction">Color correction</option>
              <option value="chroma_key">Chroma key</option>
              <option value="crop_pad">Crop/Pad</option>
              <option value="gain">Gain</option>
              <option value="sharpness">Sharpness</option>
              <option value="scroll">Scroll</option>
              <option value="custom">Custom</option>
            </select>
            <button class="primary" on:click={addFilter}>Add Filter</button>
          </div>
        </aside>

        <section class="filters-main">
          <div class="filters-main-toolbar">
            <button class="default-settings-btn" on:click={resetSelectedFilterToDefaults} disabled={!selectedFilter || Boolean(selectedFilter?.locked)}>Default filter settings</button>
          </div>
          <div class="filters-preview-panel">
            {#if previewImageUrl}
              <img src={previewImageUrl} alt="Preview source with effects" />
            {:else}
              <span>Preview source with effects</span>
            {/if}
          </div>

          {#if selectedFilter}
            <div class="filters-settings-list">
              {#if selectedFilterPresetFields.length}
                {#each selectedFilterPresetFields as field}
                  {#if field.kind === "checkbox"}
                    <label class="enabled field-row">
                      <span>{field.label}</span>
                      <input
                        type="checkbox"
                        checked={isTruthy((selectedFilter.params ?? {})[field.key] ?? field.defaultValue)}
                        disabled={Boolean(selectedFilter.locked)}
                        on:change={(e) => updateFilterPresetField(selectedFilter.id, field.key, (e.currentTarget as HTMLInputElement).checked ? "true" : "false")}
                      />
                    </label>
                  {:else}
                    <div class="row two-col">
                      <label class="muted-inline" for={`preset-${selectedFilter.id}-${field.key}`}>{field.label}</label>
                      <input
                        id={`preset-${selectedFilter.id}-${field.key}`}
                        type={field.kind === "number" ? "number" : "text"}
                        min={field.min}
                        max={field.max}
                        step={field.step}
                        value={(selectedFilter.params ?? {})[field.key] ?? field.defaultValue}
                        disabled={Boolean(selectedFilter.locked)}
                        on:input={(e) => updateFilterPresetField(selectedFilter.id, field.key, (e.currentTarget as HTMLInputElement).value)}
                      />
                    </div>
                  {/if}
                {/each}
              {:else if Object.keys(selectedFilter.params ?? {}).length === 0}
                <div class="settings-placeholder">Add filter to edit filter settings</div>
              {:else}
                {#each Object.entries(selectedFilter.params ?? {}) as [key, value]}
                  <div class="row two-col">
                    <input value={key} disabled={Boolean(selectedFilter.locked)} on:input={(e) => renameParam(selectedFilter.id, key, (e.currentTarget as HTMLInputElement).value)} />
                    <input value={value} disabled={Boolean(selectedFilter.locked)} on:input={(e) => setParam(selectedFilter.id, key, (e.currentTarget as HTMLInputElement).value)} />
                  </div>
                {/each}
              {/if}
            </div>
          {:else}
            <div class="filters-settings-list">
              <div class="settings-placeholder">Add filter to edit filter settings</div>
            </div>
          {/if}
        </section>
      </div>

      {#if contextMenu.open && contextMenu.filterId}
        {@const ctxFilter = draft.find((f) => f.id === contextMenu.filterId)}
        <div class="context-menu" style={`top:${contextMenu.y}px; left:${contextMenu.x}px;`} role="menu">
          <button on:click={() => {
            if (contextMenu.filterId) removeFilter(contextMenu.filterId);
            closeContextMenu();
          }}>Remove</button>
          <button on:click={() => {
            if (contextMenu.filterId) startRenameFilter(contextMenu.filterId);
          }}>Rename</button>
          <button on:click={() => {
            if (contextMenu.filterId) toggleFilterLock(contextMenu.filterId);
          }}>{ctxFilter?.locked ? "Unlock source" : "Lock source"}</button>
        </div>
        <div class="context-overlay" role="button" tabindex="0" on:click={closeContextMenu} on:keydown={(e) => e.key === "Escape" && closeContextMenu()}></div>
      {/if}

      <footer>
        <button class="ghost" on:click={close}>Cancel</button>
        <button class="primary" on:click={save}>Save</button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop { position: fixed; inset: 0; background: rgba(0,0,0,.55); display:grid; place-items:center; z-index:80; }
  .modal { width: min(1080px, 95vw); max-height: 90vh; overflow: auto; background: var(--surface-2); border: 1px solid var(--border); border-radius: 14px; padding: 1rem; position: relative; }

  .filters-header { display: flex; align-items: center; justify-content: space-between; gap: .7rem; }
  .filters-header h2 { margin: 0; }

  .close-x {
    width: 30px;
    height: 30px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 8px;
    border: 1px solid var(--border-strong);
    background: var(--surface-3);
    color: var(--text);
    font-weight: 700;
    cursor: pointer;
    line-height: 1;
  }

  .filters-shell {
    display: grid;
    grid-template-columns: 260px minmax(0, 1fr);
    gap: .9rem;
    min-height: 500px;
    margin-top: .4rem;
  }

  .filters-sidebar {
    border-right: 1px solid var(--border);
    padding-right: .8rem;
    display: grid;
    grid-template-rows: 1fr auto;
    gap: .7rem;
  }

  .filters-sidebar-list {
    display: grid;
    align-content: start;
    gap: .4rem;
    overflow: auto;
    max-height: 460px;
  }

  .filters-list-item {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: .4rem;
    align-items: center;
  }

  .filters-list-item.selected .filter-chip {
    border-color: color-mix(in srgb, var(--accent) 45%, var(--border-strong));
    background: color-mix(in srgb, var(--accent) 16%, var(--surface-3));
  }

  .filters-list-item.locked .filter-chip {
    opacity: .7;
  }

  .filter-chip {
    width: 100%;
    text-align: left;
    background: var(--surface-3);
    border: 1px solid var(--border);
    color: var(--text);
    border-radius: 10px;
    padding: .44rem .58rem;
    cursor: pointer;
  }

  .filter-chip-rename {
    width: 100%;
    border: 1px solid var(--border-strong);
    border-radius: 9px;
    background: var(--surface);
    color: var(--text);
    padding: .44rem .58rem;
  }

  .filter-order-actions { display: inline-flex; gap: .25rem; }

  .order-btn {
    width: 24px;
    height: 24px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    line-height: 0;
    border-radius: 7px;
    border: 1px solid var(--border-strong);
    background: var(--surface-3);
    color: var(--text);
    cursor: pointer;
  }

  .order-btn:disabled { opacity: .45; cursor: not-allowed; }

  .filters-sidebar-add { display: grid; gap: .42rem; }

  .filters-main {
    display: grid;
    grid-template-rows: auto 220px 1fr;
    gap: .75rem;
  }

  .filters-main-toolbar { display: flex; justify-content: center; }

  .default-settings-btn {
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-3);
    color: var(--text);
    padding: .45rem 1rem;
  }

  .filters-preview-panel {
    border-radius: 14px;
    background: color-mix(in srgb, var(--surface) 88%, #000 12%);
    border: 1px solid var(--border);
    display: grid;
    place-items: center;
    color: var(--text-muted);
    font-size: 1.8rem;
    text-align: center;
    padding: .8rem;
  }

  .filters-preview-panel img {
    width: 100%;
    height: 100%;
    object-fit: contain;
    border-radius: 10px;
  }

  .filters-settings-list {
    display: grid;
    gap: .55rem;
    align-content: start;
  }

  .settings-placeholder {
    color: color-mix(in srgb, var(--text-muted) 88%, #c8ceda 12%);
    font-size: 1rem;
    line-height: 1.35;
    opacity: .88;
    text-align: center;
    border: 1px dashed color-mix(in srgb, var(--border) 75%, transparent);
    border-radius: 10px;
    padding: .9rem .75rem;
    background: color-mix(in srgb, var(--surface-3) 82%, transparent);
  }

  .row {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: .5rem;
    align-items: center;
  }

  .row.two-col { grid-template-columns: repeat(2, minmax(0, 1fr)); }

  .enabled { display:inline-flex; align-items:center; gap:.35rem; font-size:.85rem; color:var(--text-muted); }
  .field-row { justify-content: space-between; }
  .muted-inline { color: var(--text-muted); font-size: .85rem; align-self: center; }

  input, select {
    background: var(--surface);
    border: 1px solid var(--border);
    color: var(--text);
    border-radius: 9px;
    padding: .5rem .65rem;
  }

  input:focus, select:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 22%, transparent);
  }

  footer { display:flex; justify-content:flex-end; gap:.6rem; margin-top: .9rem; }
  button { border:none; border-radius: 10px; padding: .55rem .8rem; cursor:pointer; }
  .ghost { background: transparent; border: 1px solid var(--border); color: var(--text); }
  .ghost:hover { background: var(--surface-3); }
  .primary {
    background: var(--accent);
    color: #fff;
    border: 1px solid color-mix(in srgb, var(--accent) 55%, #000 45%);
  }
  .primary:hover { filter: brightness(1.04); }

  .context-menu {
    position: fixed;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: .25rem;
    z-index: 110;
    min-width: 160px;
  }

  .context-menu button {
    width: 100%;
    text-align: left;
    background: transparent;
    color: var(--text);
    border: none;
    padding: .6rem .75rem;
    border-radius: 8px;
    cursor: pointer;
  }

  .context-menu button:hover { background: var(--surface-3); }

  .context-overlay { position: fixed; inset: 0; z-index: 105; }
  .muted { color: var(--text-muted); margin: .2rem 0; }
</style>
