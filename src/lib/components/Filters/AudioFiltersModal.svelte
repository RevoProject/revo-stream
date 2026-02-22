<script lang="ts">
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
  export let allowDraggablePopups = false;
  export let openAdditionalSettingsInWindows = false;
  export let dragX = 0;
  export let dragY = 0;
  export let modalEl: HTMLDivElement | null = null;

  export let audioFiltersSourceLabel = "";
  export let audioFiltersDraft: FilterItem[] = [];
  export let audioFiltersSelectedId: string | null = null;
  export let audioFiltersRenamingId: string | null = null;
  export let audioFiltersRenameValue = "";
  export let audioFilterNewKind = "noise_suppression";
  export let selectedAudioFilter: FilterItem | null = null;
  export let selectedAudioFilterPresetFields: FilterFieldDef[] = [];
  export let audioFiltersPreviewUrl = "";
  export let audioFiltersContextMenu: { open: boolean; x: number; y: number; filterId: string | null } = {
    open: false,
    x: 0,
    y: 0,
    filterId: null
  };

  export let onClose: () => void | Promise<void> = () => {};
  export let onSave: () => void | Promise<void> = () => {};
  export let onBeginDrag: (event: PointerEvent) => void = () => {};
  export let onMoveDrag: (event: PointerEvent) => void = () => {};
  export let onEndDrag: (event: PointerEvent) => void = () => {};
  export let onSelectFilter: (id: string) => void = () => {};
  export let onOpenContextMenu: (event: MouseEvent, id: string) => void = () => {};
  export let onSetRenameValue: (value: string) => void = () => {};
  export let onCommitRename: (id: string) => void = () => {};
  export let onCancelRename: () => void = () => {};
  export let onMoveFilter: (id: string, direction: "up" | "down") => void = () => {};
  export let onSetNewKind: (value: string) => void = () => {};
  export let onAddFilter: () => void = () => {};
  export let onResetSelectedToDefaults: () => void = () => {};
  export let onUpdatePresetField: (filterId: string, key: string, value: string) => void = () => {};
  export let onUpdateFilter: (filterId: string, patch: Partial<FilterItem>) => void = () => {};
  export let onRemoveFilter: (id: string) => void = () => {};
  export let onStartRename: (id: string) => void = () => {};
  export let onToggleLock: (id: string) => void = () => {};
  export let onCloseContextMenu: () => void = () => {};
  export let handleBackdropKey: (event: KeyboardEvent, close: () => void) => void = () => {};
  export let isTruthy: (value: string) => boolean = (value) => Boolean(value);
</script>

{#if open}
  <div
    class="audio-filters-modal-backdrop"
    role="button"
    tabindex="0"
    on:click={() => void onClose()}
    on:keydown={(e) => handleBackdropKey(e, () => void onClose())}
  >
    <div
      class="audio-filters-modal"
      class:draggable-popup={allowDraggablePopups && !openAdditionalSettingsInWindows}
      bind:this={modalEl}
      role="dialog"
      tabindex="-1"
      aria-modal="true"
      aria-label="Audio filters"
      style={`--audio-filters-dx:${dragX}px; --audio-filters-dy:${dragY}px;`}
      on:pointerdown={onBeginDrag}
      on:pointermove={onMoveDrag}
      on:pointerup={onEndDrag}
      on:pointercancel={onEndDrag}
      on:click|stopPropagation
      on:keydown|stopPropagation
    >
      <div class="audio-filters-header">
        <h3>Audio Filters</h3>
        <button class="modal-close-x" aria-label="Close audio filters" on:click={() => void onClose()}>✕</button>
      </div>
      <p class="muted">{audioFiltersSourceLabel}</p>

      <div class="filters-shell">
        <aside class="filters-sidebar">
          <div class="filters-sidebar-list">
            {#if !audioFiltersDraft.length}
              <p class="muted">No audio filters configured.</p>
            {:else}
              {#each audioFiltersDraft as filter, index (filter.id)}
                <div
                  class="filters-list-item"
                  class:selected={audioFiltersSelectedId === filter.id}
                  class:locked={Boolean(filter.locked)}
                  role="button"
                  tabindex="0"
                  on:click={() => onSelectFilter(filter.id)}
                  on:keydown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      onSelectFilter(filter.id);
                    }
                  }}
                  on:contextmenu={(e) => onOpenContextMenu(e, filter.id)}
                >
                  {#if audioFiltersRenamingId === filter.id}
                    <input
                      class="filter-chip-rename"
                      value={audioFiltersRenameValue}
                      on:input={(e) => onSetRenameValue((e.currentTarget as HTMLInputElement).value)}
                      on:blur={() => onCommitRename(filter.id)}
                      on:keydown={(e) => {
                        if (e.key === "Enter") onCommitRename(filter.id);
                        if (e.key === "Escape") onCancelRename();
                      }}
                    />
                  {:else}
                    <button class="filter-chip" on:click={() => onSelectFilter(filter.id)}>{filter.name.trim() || `Filter ${index + 1}`}</button>
                  {/if}
                  <div class="filter-order-actions">
                    <button class="order-btn" disabled={filter.locked || index === 0} on:click={() => onMoveFilter(filter.id, "up")}>↑</button>
                    <button class="order-btn" disabled={filter.locked || index === audioFiltersDraft.length - 1} on:click={() => onMoveFilter(filter.id, "down")}>↓</button>
                  </div>
                </div>
              {/each}
            {/if}
          </div>

          <div class="filters-sidebar-add">
            <select value={audioFilterNewKind} on:change={(e) => onSetNewKind((e.currentTarget as HTMLSelectElement).value)}>
              <option value="noise_suppression">Noise Suppression</option>
              <option value="noise_gate">Noise Gate</option>
              <option value="compressor">Compressor</option>
              <option value="limiter">Limiter</option>
              <option value="expander">Expander</option>
              <option value="vst2">VST2 Plugin</option>
              <option value="vst3">VST3 Plugin</option>
              <option value="custom">Custom</option>
            </select>
            <button class="primary" on:click={onAddFilter}>Add Filter</button>
          </div>
        </aside>

        <section class="filters-main">
          <div class="filters-main-toolbar">
            <button class="default-settings-btn" on:click={onResetSelectedToDefaults} disabled={!selectedAudioFilter || Boolean(selectedAudioFilter?.locked)}>
              Default filter settings
            </button>
          </div>
          <div class="filters-preview-panel">
            {#if audioFiltersPreviewUrl}
              <img src={audioFiltersPreviewUrl} alt="Preview source with effects" />
            {:else}
              <span>Preview source with effects</span>
            {/if}
          </div>

          {#if selectedAudioFilter}
            <div class="filters-settings-list">
              {#if selectedAudioFilterPresetFields.length}
                {#each selectedAudioFilterPresetFields as field}
                  {#if field.kind === "checkbox"}
                    <label class="enabled field-row">
                      <span>{field.label}</span>
                      <input
                        type="checkbox"
                        checked={isTruthy((selectedAudioFilter.params ?? {})[field.key] ?? field.defaultValue)}
                        disabled={Boolean(selectedAudioFilter.locked)}
                        on:change={(e) =>
                          onUpdatePresetField(
                            selectedAudioFilter.id,
                            field.key,
                            (e.currentTarget as HTMLInputElement).checked ? "true" : "false"
                          )}
                      />
                    </label>
                  {:else}
                    <div class="audio-filter-row compact two-col">
                      <label class="muted-inline" for={`audio-preset-${selectedAudioFilter.id}-${field.key}`}>{field.label}</label>
                      <input
                        id={`audio-preset-${selectedAudioFilter.id}-${field.key}`}
                        type={field.kind === "number" ? "number" : "text"}
                        min={field.min}
                        max={field.max}
                        step={field.step}
                        value={(selectedAudioFilter.params ?? {})[field.key] ?? field.defaultValue}
                        disabled={Boolean(selectedAudioFilter.locked)}
                        on:input={(e) =>
                          onUpdatePresetField(selectedAudioFilter.id, field.key, (e.currentTarget as HTMLInputElement).value)}
                      />
                    </div>
                  {/if}
                {/each}
              {:else if !Object.keys(selectedAudioFilter.params ?? {}).length}
                <div class="filter-settings-placeholder">Add filter to edit filter settings</div>
              {:else}
                {#each Object.entries(selectedAudioFilter.params ?? {}) as [paramKey, paramValue]}
                  <div class="audio-filter-row compact two-col">
                    <input
                      value={paramKey}
                      disabled={Boolean(selectedAudioFilter.locked)}
                      on:input={(e) => {
                        const nextKey = (e.currentTarget as HTMLInputElement).value.trim();
                        if (!nextKey || nextKey === paramKey) return;
                        const params = { ...(selectedAudioFilter.params ?? {}) };
                        const value = params[paramKey] ?? "";
                        delete params[paramKey];
                        params[nextKey] = value;
                        onUpdateFilter(selectedAudioFilter.id, { params });
                      }}
                    />
                    <input
                      value={paramValue}
                      disabled={Boolean(selectedAudioFilter.locked)}
                      on:input={(e) => {
                        const params = { ...(selectedAudioFilter.params ?? {}) };
                        params[paramKey] = (e.currentTarget as HTMLInputElement).value;
                        onUpdateFilter(selectedAudioFilter.id, { params });
                      }}
                    />
                  </div>
                {/each}
              {/if}
            </div>
          {:else}
            <div class="filters-settings-list">
              <div class="filter-settings-placeholder">Add filter to edit filter settings</div>
            </div>
          {/if}
        </section>
      </div>

      {#if audioFiltersContextMenu.open && audioFiltersContextMenu.filterId}
        {@const ctxFilter = audioFiltersDraft.find((f) => f.id === audioFiltersContextMenu.filterId)}
        <div class="context-menu" style={`top:${audioFiltersContextMenu.y}px; left:${audioFiltersContextMenu.x}px;`} role="menu">
          <button
            on:click={() => {
              if (audioFiltersContextMenu.filterId) onRemoveFilter(audioFiltersContextMenu.filterId);
              onCloseContextMenu();
            }}
          >
            Remove
          </button>
          <button
            on:click={() => {
              if (audioFiltersContextMenu.filterId) onStartRename(audioFiltersContextMenu.filterId);
            }}
          >
            Rename
          </button>
          <button
            on:click={() => {
              if (audioFiltersContextMenu.filterId) onToggleLock(audioFiltersContextMenu.filterId);
            }}
          >
            {ctxFilter?.locked ? "Unlock source" : "Lock source"}
          </button>
        </div>
        <div
          class="context-overlay"
          role="button"
          tabindex="0"
          on:click={onCloseContextMenu}
          on:keydown={(e) => handleBackdropKey(e, onCloseContextMenu)}
        ></div>
      {/if}

      <div class="quick-text-actions">
        <button class="ghost" on:click={() => void onClose()}>Cancel</button>
        <button class="primary" on:click={() => void onSave()}>Save</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .audio-filters-modal-backdrop {
    position: fixed;
    inset: 0;
    background: #0009;
    z-index: 2040;
  }

  .audio-filters-modal {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: min(880px, calc(100vw - 2rem));
    max-height: calc(100vh - 2rem);
    overflow: auto;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: 0 10px 36px #000a;
    z-index: 2050;
    padding: 1rem;
    display: grid;
    gap: 0.65rem;
  }

  .audio-filters-modal.draggable-popup {
    transform: translate(calc(-50% + var(--audio-filters-dx, 0px)), calc(-50% + var(--audio-filters-dy, 0px)));
  }

  .audio-filters-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 0.65rem;
  }

  .audio-filters-header h3 {
    margin: 0;
    font-size: 1rem;
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
    overflow: auto;
    max-height: 420px;
  }

  .filters-list-item {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 0.4rem;
    align-items: center;
  }

  .filters-list-item.selected .filter-chip {
    border-color: color-mix(in srgb, var(--accent) 45%, var(--border-strong));
    background: color-mix(in srgb, var(--accent) 16%, var(--surface-3));
  }

  .filters-list-item.locked .filter-chip {
    opacity: 0.7;
  }

  .filter-chip {
    width: 100%;
    text-align: left;
    background: var(--surface-3);
    border: 1px solid var(--border);
    color: var(--text);
    border-radius: 10px;
    padding: 0.44rem 0.58rem;
    cursor: pointer;
  }

  .filter-chip-rename {
    width: 100%;
    border: 1px solid var(--border-strong);
    border-radius: 9px;
    background: var(--surface);
    color: var(--text);
    padding: 0.44rem 0.58rem;
  }

  .filter-order-actions {
    display: inline-flex;
    gap: 0.25rem;
  }

  .order-btn {
    width: 24px;
    height: 24px;
    border-radius: 7px;
    border: 1px solid var(--border-strong);
    background: var(--surface-3);
    color: var(--text);
    cursor: pointer;
  }

  .order-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .filters-sidebar-add {
    display: grid;
    gap: 0.42rem;
  }

  .filters-main {
    display: grid;
    grid-template-rows: auto 210px 1fr;
    gap: 0.75rem;
  }

  .filters-shell select,
  .filters-shell input:not([type="checkbox"]):not([type="radio"]):not([type="range"]):not([type="file"]):not([type="color"]) {
    width: 100%;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    color: var(--text);
    padding: 0.52rem 0.72rem;
    outline: none;
    box-sizing: border-box;
  }

  .filters-shell select:focus,
  .filters-shell input:not([type="checkbox"]):not([type="radio"]):not([type="range"]):not([type="file"]):not([type="color"]):focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 22%, transparent);
  }

  .primary {
    border: 1px solid color-mix(in srgb, var(--accent) 55%, #000 45%);
    border-radius: 9px;
    background: var(--accent);
    color: #fff;
    padding: 0.5rem 0.85rem;
    cursor: pointer;
  }

  .primary:hover {
    filter: brightness(1.04);
  }

  .filters-main-toolbar {
    display: flex;
    justify-content: center;
  }

  .default-settings-btn {
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-3);
    color: var(--text);
    padding: 0.45rem 1rem;
  }

  .filters-preview-panel {
    border-radius: 14px;
    background: #050505;
    border: 1px solid #222;
    display: grid;
    place-items: center;
    color: #d7d7d7;
    font-size: 1.9rem;
    text-align: center;
    padding: 0.8rem;
  }

  .filters-preview-panel img {
    width: 100%;
    height: 100%;
    object-fit: contain;
    border-radius: 10px;
  }

  .filters-settings-list {
    display: grid;
    gap: 0.55rem;
    align-content: start;
  }

  .filter-settings-placeholder {
    color: color-mix(in srgb, var(--text-muted) 88%, #c8ceda 12%);
    font-size: 1rem;
    line-height: 1.35;
    opacity: 0.88;
    text-align: center;
    border: 1px dashed color-mix(in srgb, var(--border) 75%, transparent);
    border-radius: 10px;
    padding: 0.9rem 0.75rem;
    background: color-mix(in srgb, var(--surface-3) 82%, transparent);
  }

  .audio-filter-row {
    display: grid;
    grid-template-columns: auto 1.2fr 1fr auto;
    gap: 0.45rem;
    align-items: center;
  }

  .audio-filter-row.compact {
    grid-template-columns: auto minmax(0, 1fr);
    gap: 0.55rem;
  }

  .audio-filter-row.compact.two-col {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .field-row {
    justify-content: space-between;
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
    padding: 0.25rem;
    z-index: 60;
    min-width: 160px;
  }

  .context-menu button {
    width: 100%;
    text-align: left;
    background: transparent;
    color: var(--text);
    border: none;
    padding: 0.6rem 0.75rem;
    border-radius: 8px;
    cursor: pointer;
  }

  .context-menu button:hover {
    background: var(--surface-3);
  }

  .context-overlay {
    position: fixed;
    inset: 0;
    z-index: 55;
  }

  .quick-text-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }

  .quick-text-actions button {
    min-width: 92px;
    height: 34px;
    border-radius: 8px;
    border: 1px solid var(--border-strong);
    padding: 0 0.8rem;
    font-weight: 600;
    cursor: pointer;
  }

  .quick-text-actions button.ghost {
    background: var(--surface-3);
    color: var(--text);
  }

  .quick-text-actions button.primary {
    background: var(--accent);
    color: #fff;
    border-color: color-mix(in srgb, var(--accent) 55%, #000 45%);
  }

  .quick-text-actions button:hover {
    filter: brightness(1.04);
  }

  .modal-close-x {
    width: 30px;
    height: 30px;
    border-radius: 8px;
    border: 1px solid var(--border-strong);
    background: var(--surface-3);
    color: var(--text);
    font-weight: 700;
    cursor: pointer;
    line-height: 1;
  }

  @media (max-width: 980px) {
    .filters-shell {
      grid-template-columns: 1fr;
    }

    .filters-sidebar {
      border-right: 0;
      border-bottom: 1px solid var(--border);
      padding-right: 0;
      padding-bottom: 0.8rem;
      max-height: 260px;
    }

    .filters-sidebar-list {
      max-height: 170px;
    }

    .filters-main {
      grid-template-rows: auto 170px 1fr;
    }

    .filters-preview-panel {
      font-size: 1.15rem;
    }
  }
</style>
