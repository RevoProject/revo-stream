<script lang="ts">
  import { convertFileSrc, invoke } from "@tauri-apps/api/core";
  import { createEventDispatcher } from "svelte";
  import { isAudioCaptureSourceType } from "../../sourceClassification";
  import Icon from "../UI/Icon.svelte";

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

  export let active = false;
  export let targetType: "source" | "scene" = "source";
  export let sourceCategory: "graphic" | "image" | "video" | "audio" | undefined = undefined;
  export let sourceType = "";
  export let targetId = "";
  export let title = "";
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
  let liveSyncTimer: ReturnType<typeof setTimeout> | null = null;
  let lastLiveFingerprint = "";
  let wasActive = false;

  const isTruthy = (value: string) => ["1", "true", "yes", "on"].includes(String(value ?? "").trim().toLowerCase());

  const scheduleLiveChange = () => {
    if (!active) return;
    if (liveSyncTimer) clearTimeout(liveSyncTimer);
    liveSyncTimer = setTimeout(() => {
      liveSyncTimer = null;
      const payload = draft.map((f) => ({ ...f, params: { ...(f.params ?? {}) } }));
      const fingerprint = JSON.stringify(payload);
      if (fingerprint === lastLiveFingerprint) return;
      lastLiveFingerprint = fingerprint;
      dispatch("change", { filters: payload });
    }, 90);
  };

  const refreshPreviewScreenshot = async () => {
    try {
      const image = await invoke<string>("obs_take_screenshot", {
        width: 640,
        height: 360,
        sourceId: targetType === "source" && targetId.trim().length ? targetId : undefined
      });
      if (typeof image === "string" && image.trim()) {
        if (image.startsWith("data:image")) {
          previewImageUrl = image;
        } else {
          previewImageUrl = `${convertFileSrc(image)}?t=${Date.now()}`;
        }
      }
    } catch {
      // ignore when backend is unavailable
    }
  };

  const schedulePreviewRefresh = () => {
    if (!active) return;
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

  const filterKindLabels: Record<string, string> = {
    color_correction: "Color correction",
    chroma_key: "Chroma key",
    crop_pad: "Crop/Pad",
    gain: "Gain",
    sharpness: "Sharpness",
    scroll: "Scroll",
    custom: "Custom"
  };

  const audioKindOptions = [
    { value: "gain", label: "Gain" },
    { value: "custom", label: "Custom" }
  ];
  const graphicKindOptions = [
    { value: "color_correction", label: "Color correction" },
    { value: "chroma_key", label: "Chroma key" },
    { value: "crop_pad", label: "Crop/Pad" },
    { value: "sharpness", label: "Sharpness" },
    { value: "scroll", label: "Scroll" },
    { value: "custom", label: "Custom" }
  ];

  $: audioTarget =
    sourceCategory === "audio" || isAudioCaptureSourceType(sourceType);
  $: kindOptions = audioTarget ? audioKindOptions : graphicKindOptions;
  $: if (audioTarget && !audioKindOptions.some((k) => k.value === newKind)) {
    newKind = "gain";
  }
  $: if (!audioTarget && !graphicKindOptions.some((k) => k.value === newKind)) {
    newKind = "color_correction";
  }

  const addFilter = () => {
    const id = crypto.randomUUID ? crypto.randomUUID() : `${Date.now()}-${Math.random()}`;
    const baseName = filterKindLabels[newKind] ?? `Filter ${draft.length + 1}`;
    const existing = new Set(draft.map((f) => f.name.trim().toLowerCase()));
    let name = baseName;
    let suffix = 2;
    while (existing.has(name.trim().toLowerCase())) {
      name = `${baseName} ${suffix}`;
      suffix += 1;
    }
    draft = [
      ...draft,
      {
        id,
        name,
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

  export function commitSave() {
    dispatch("save", {
      filters: draft.map((f) => ({ ...f, params: { ...(f.params ?? {}) } }))
    });
  }

  $: if (active && !wasActive) {
    draft = filters.map((f) => ({ ...f, locked: Boolean(f.locked), params: { ...(f.params ?? {}) } }));
    selectedFilterId = draft[0]?.id ?? null;
    renamingFilterId = null;
    renameValue = "";
    contextMenu = { open: false, x: 0, y: 0, filterId: null };
    schedulePreviewRefresh();
  }

  $: if (!active && wasActive) {
    previewImageUrl = "";
    if (liveSyncTimer) {
      clearTimeout(liveSyncTimer);
      liveSyncTimer = null;
    }
    if (previewRefreshTimer) {
      clearTimeout(previewRefreshTimer);
      previewRefreshTimer = null;
    }
    lastLiveFingerprint = "";
  }

  $: wasActive = active;

  $: selectedFilter = selectedFilterId ? draft.find((f) => f.id === selectedFilterId) ?? null : null;
  $: selectedFilterPresetFields = selectedFilter ? (filterPresetFields[selectedFilter.kind] ?? []) : [];

  $: if (active && selectedFilter && selectedFilterPresetFields.length) {
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

  $: if (active) {
    draft;
    selectedFilterId;
    schedulePreviewRefresh();
    scheduleLiveChange();
  }
</script>

{#if title}
  <h3 class="filters-panel-title">{title}</h3>
{/if}

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
            onclick={() => (selectedFilterId = filter.id)}
            onkeydown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                selectedFilterId = filter.id;
              }
            }}
            oncontextmenu={(e) => openContextMenu(e, filter.id)}
          >
            {#if renamingFilterId === filter.id}
              <input
                class="filter-chip-rename"
                value={renameValue}
                oninput={(e) => (renameValue = (e.currentTarget as HTMLInputElement).value)}
                onblur={() => commitRenameFilter(filter.id)}
                onkeydown={(e) => {
                  if (e.key === "Enter") commitRenameFilter(filter.id);
                  if (e.key === "Escape") renamingFilterId = null;
                }}
              />
            {:else}
              <button class="filter-chip" onclick={() => (selectedFilterId = filter.id)}>{filter.name.trim() || `Filter ${index + 1}`}</button>
            {/if}
            <div class="filter-order-actions">
              <button class="order-btn" aria-label="Move filter up" disabled={filter.locked || index === 0} onclick={() => moveFilter(filter.id, "up")}>
                <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true" focusable="false">
                  <path d="M6 2 L2.2 6.3 H4.7 V10 H7.3 V6.3 H9.8 Z" fill="currentColor" />
                </svg>
              </button>
              <button class="order-btn" aria-label="Move filter down" disabled={filter.locked || index === draft.length - 1} onclick={() => moveFilter(filter.id, "down")}>
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
        {#each kindOptions as kind}
          <option value={kind.value}>{kind.label}</option>
        {/each}
      </select>
      <button class="primary" onclick={addFilter}>Add Filter</button>
    </div>
  </aside>

  <section class="filters-main">
    <div class="filters-main-toolbar">
      <button class="default-settings-btn" onclick={resetSelectedFilterToDefaults} disabled={!selectedFilter || Boolean(selectedFilter?.locked)}>Default filter settings</button>
    </div>
    <div class="filters-preview-panel">
      <div class="filters-preview-viewport">
        {#if previewImageUrl}
          <img src={previewImageUrl} alt="Preview source with effects" />
        {:else}
          <span>Preview source with effects</span>
        {/if}
      </div>
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
                  onchange={(e) => updateFilterPresetField(selectedFilter.id, field.key, (e.currentTarget as HTMLInputElement).checked ? "true" : "false")}
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
                  oninput={(e) => updateFilterPresetField(selectedFilter.id, field.key, (e.currentTarget as HTMLInputElement).value)}
                />
              </div>
            {/if}
          {/each}
        {:else if Object.keys(selectedFilter.params ?? {}).length === 0}
          <div class="settings-placeholder">Add filter to edit filter settings</div>
        {:else}
          {#each Object.entries(selectedFilter.params ?? {}) as [key, value]}
            <div class="row two-col">
              <input value={key} disabled={Boolean(selectedFilter.locked)} oninput={(e) => renameParam(selectedFilter.id, key, (e.currentTarget as HTMLInputElement).value)} />
              <input value={value} disabled={Boolean(selectedFilter.locked)} oninput={(e) => setParam(selectedFilter.id, key, (e.currentTarget as HTMLInputElement).value)} />
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
    <button onclick={() => {
      if (contextMenu.filterId) removeFilter(contextMenu.filterId);
      closeContextMenu();
    }}><span class="menu-icon danger" aria-hidden="true">✕</span>Remove</button>
    <button onclick={() => {
      if (contextMenu.filterId) startRenameFilter(contextMenu.filterId);
    }}><span class="menu-icon" aria-hidden="true">✎</span>Rename</button>
    <button onclick={() => {
      if (contextMenu.filterId) toggleFilterLock(contextMenu.filterId);
    }}><span class="menu-icon" aria-hidden="true"><Icon name={ctxFilter?.locked ? "unlock" : "lock"} size={13} /></span>{ctxFilter?.locked ? "Unlock source" : "Lock source"}</button>
  </div>
  <div class="context-overlay" role="button" tabindex="0" onclick={closeContextMenu} onkeydown={(e) => e.key === "Escape" && closeContextMenu()}></div>
{/if}

<style>
  .filters-panel-title {
    margin: 0 0 0.5rem;
    font-size: 0.95rem;
    font-weight: 700;
    color: var(--text);
  }

  .filters-shell {
    display: grid;
    grid-template-columns: 260px minmax(0, 1fr);
    gap: 0.9rem;
    min-height: 440px;
  }

  .filters-sidebar {
    border-right: 1px solid var(--border);
    padding-right: 0.8rem;
    display: grid;
    grid-template-rows: 1fr auto;
    gap: 0.7rem;
  }

  .filters-sidebar-list {
    display: grid;
    align-content: start;
    gap: 0.4rem;
    overflow-y: auto;
    min-height: 0;
  }

  .filters-list-item {
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }

  .filters-list-item .filter-chip {
    flex: 1 1 auto;
    min-width: 0;
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .filters-list-item.selected .filter-chip {
    border-color: var(--accent, #4c8dff);
    color: var(--accent, #4c8dff);
  }

  .filters-list-item.locked {
    opacity: 0.6;
  }

  .filter-chip {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 0.6rem;
    color: var(--text);
    padding: 0.45rem 0.7rem;
    cursor: pointer;
  }

  .filter-chip-rename {
    flex: 1 1 auto;
    min-width: 0;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 0.6rem;
    color: var(--text);
    padding: 0.4rem 0.6rem;
  }

  .filter-order-actions {
    display: inline-flex;
    gap: 0.2rem;
    flex: 0 0 auto;
  }

  .order-btn {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    background: var(--surface);
    color: var(--text-muted);
    cursor: pointer;
  }

  .order-btn:hover:not(:disabled) {
    color: var(--text);
    border-color: var(--border-strong, var(--border));
  }

  .order-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .filters-sidebar-add {
    display: grid;
    gap: 0.5rem;
  }

  .filters-sidebar-add select {
    width: 100%;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 0.6rem;
    color: var(--text);
    padding: 0.5rem 0.6rem;
  }

  .filters-sidebar-add .primary {
    background: var(--accent, #4c8dff);
    color: #fff;
    border: 0;
    border-radius: 0.6rem;
    padding: 0.55rem 0.8rem;
    font-weight: 650;
    cursor: pointer;
  }

  .filters-main {
    display: grid;
    grid-template-rows: auto minmax(200px, 1fr) auto;
    gap: 0.7rem;
    min-width: 0;
  }

  .filters-main-toolbar {
    display: flex;
    justify-content: flex-end;
  }

  .default-settings-btn {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 0.6rem;
    color: var(--text);
    padding: 0.45rem 0.8rem;
    cursor: pointer;
  }

  .default-settings-btn:hover:not(:disabled) {
    border-color: var(--border-strong, var(--border));
  }

  .default-settings-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .filters-preview-panel {
    border: 1px solid var(--border);
    border-radius: 0.7rem;
    background: #0b0e14;
    display: grid;
    place-items: center;
    overflow: hidden;
    min-height: 200px;
  }

  .filters-preview-viewport img {
    width: 100%;
    height: auto;
    display: block;
  }

  .filters-preview-viewport span {
    padding: 1rem;
    color: var(--text-muted);
    font-size: 0.85rem;
  }

  .filters-settings-list {
    display: grid;
    gap: 0.55rem;
    align-content: start;
    max-height: 220px;
    overflow-y: auto;
  }

  .row.two-col {
    display: grid;
    grid-template-columns: 120px minmax(0, 1fr);
    gap: 0.6rem;
    align-items: center;
  }

  .row input {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 0.6rem;
    color: var(--text);
    padding: 0.45rem 0.6rem;
    min-width: 0;
    width: 100%;
    box-sizing: border-box;
  }

  label.enabled.field-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.6rem;
  }

  .settings-placeholder {
    border: 1px dashed var(--border);
    border-radius: 0.7rem;
    padding: 1rem;
    color: var(--text-muted);
    text-align: center;
    font-size: 0.85rem;
  }

  .muted {
    color: var(--text-muted);
    margin: 0.2rem 0;
  }

  .muted-inline {
    color: var(--text-muted);
    font-size: 0.85rem;
    align-self: center;
  }

  .context-menu {
    position: fixed;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.3rem;
    box-shadow: 0 8px 28px #0009;
    z-index: 120;
    min-width: 160px;
  }

  .context-menu button {
    width: 100%;
    text-align: left;
    background: transparent;
    border: 0;
    color: var(--text);
    padding: 0.45rem 0.6rem;
    border-radius: 0.4rem;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
  }

  .context-menu button:hover {
    background: var(--surface-3);
  }

  .menu-icon {
    display: inline-grid;
    place-items: center;
    width: 1.1rem;
    flex: 0 0 auto;
  }

  .menu-icon.danger {
    color: var(--danger, #ef4444);
  }



  .context-overlay {
    position: fixed;
    inset: 0;
    z-index: 119;
  }
</style>
