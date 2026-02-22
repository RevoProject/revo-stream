<script lang="ts">
  import { createEventDispatcher } from "svelte";

  export let pluginProfiles: string[] = ["default"];
  export let activePluginProfile = "default";
  export let plugins: { name: string; file_name: string; module_name: string }[] = [];
  export let enabledModules: string[] = [];
  export let baselineEnabledModules: string[] = [];
  export let busy = false;
  export let windowMode = false;
  export let allowDraggablePopups = false;

  const dispatch = createEventDispatcher();

  let modalEl: HTMLDivElement | null = null;
  let dragActive = false;
  let dragStartX = 0;
  let dragStartY = 0;
  let dragOriginX = 0;
  let dragOriginY = 0;
  let dragX = 0;
  let dragY = 0;

  const clampValue = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

  const beginDrag = (event: PointerEvent) => {
    if (windowMode || !allowDraggablePopups) return;
    if (event.button !== 0) return;
    const target = event.target as HTMLElement | null;
    if (target?.closest("button,input,select,textarea,a")) return;
    if (!modalEl) return;
    dragActive = true;
    dragStartX = event.clientX;
    dragStartY = event.clientY;
    dragOriginX = dragX;
    dragOriginY = dragY;
    (event.currentTarget as HTMLElement | null)?.setPointerCapture?.(event.pointerId);
  };

  const moveDrag = (event: PointerEvent) => {
    if (!dragActive || !modalEl) return;
    const rect = modalEl.getBoundingClientRect();
    const viewportW = window.innerWidth;
    const viewportH = window.innerHeight;
    const minVisibleRatio = 0.25;
    const centerLeft = (viewportW - rect.width) / 2;
    const centerTop = (viewportH - rect.height) / 2;
    const nextDx = dragOriginX + (event.clientX - dragStartX);
    const nextDy = dragOriginY + (event.clientY - dragStartY);
    const minLeft = -rect.width * (1 - minVisibleRatio);
    const maxLeft = viewportW - rect.width * minVisibleRatio;
    const minTop = -rect.height * (1 - minVisibleRatio);
    const maxTop = viewportH - rect.height * minVisibleRatio;
    const clampedLeft = clampValue(centerLeft + nextDx, minLeft, maxLeft);
    const clampedTop = clampValue(centerTop + nextDy, minTop, maxTop);
    dragX = clampedLeft - centerLeft;
    dragY = clampedTop - centerTop;
  };

  const endDrag = (event: PointerEvent) => {
    if (!dragActive) return;
    dragActive = false;
    (event.currentTarget as HTMLElement | null)?.releasePointerCapture?.(event.pointerId);
  };

  let localActiveProfile = activePluginProfile;
  let localEnabledModules = new Set<string>(enabledModules);
  let newPluginProfileName = "";
  let lastIncomingActiveProfile = activePluginProfile;
  let lastIncomingEnabledSignature = [...enabledModules].sort().join("|");

  $: if (activePluginProfile !== lastIncomingActiveProfile) {
    localActiveProfile = activePluginProfile;
    localEnabledModules = new Set(enabledModules);
    lastIncomingEnabledSignature = [...enabledModules].sort().join("|");
    lastIncomingActiveProfile = activePluginProfile;
  }

  const close = () => dispatch("close");

  const selectProfile = (name: string) => {
    localActiveProfile = name;
    dispatch("selectProfile", { name });
  };

  const toggleModule = (moduleName: string, checked: boolean) => {
    if (checked) localEnabledModules.add(moduleName);
    else localEnabledModules.delete(moduleName);
    localEnabledModules = new Set(localEnabledModules);
  };

  const createProfile = () => {
    const name = newPluginProfileName.trim();
    if (!name) return;
    dispatch("createProfile", { name });
    newPluginProfileName = "";
  };

  const save = () => {
    dispatch("save", {
      activeProfile: localActiveProfile,
      enabledModules: [...localEnabledModules]
    });
  };

  const isEnabled = (moduleName: string) => localEnabledModules.has(moduleName);

  type PluginViewRow = {
    plugin: { name: string; file_name: string; module_name: string };
    enabled: boolean;
    pendingEnable: boolean;
    pendingDisable: boolean;
    badgeText: string;
    statusText: string;
  };

  let pluginViewRows: PluginViewRow[] = [];

  $: {
    const baselineEnabledSet = new Set(
      baselineEnabledModules.length ? baselineEnabledModules : enabledModules
    );
    pluginViewRows = plugins.map((plugin) => {
      const enabled = localEnabledModules.has(plugin.module_name);
      const pendingEnable = !baselineEnabledSet.has(plugin.module_name) && enabled;
      const pendingDisable = baselineEnabledSet.has(plugin.module_name) && !enabled;
      const badgeText = pendingEnable
        ? "Future Enable"
        : pendingDisable
          ? "Future Disable"
          : enabled
            ? "Enabled"
            : "Disabled";
      const statusText = pendingEnable
        ? "Enable on restart"
        : pendingDisable
          ? "Disable on restart"
          : enabled
            ? "Enabled in profile"
            : "Disabled in profile";

      return {
        plugin,
        enabled,
        pendingEnable,
        pendingDisable,
        badgeText,
        statusText
      };
    });
  }
</script>

<div class="modal-backdrop" class:window-mode={windowMode} role="button" tabindex="0" onclick={close} onkeydown={(e) => (e.key === "Escape" ? close() : null)}>
  <div
    class="modal"
    class:window-mode={windowMode}
    class:draggable-popup={allowDraggablePopups && !windowMode}
    bind:this={modalEl}
    role="dialog"
    aria-modal="true"
    tabindex="0"
    style={`--popup-dx:${dragX}px; --popup-dy:${dragY}px;`}
    onpointerdown={beginDrag}
    onpointermove={moveDrag}
    onpointerup={endDrag}
    onpointercancel={endDrag}
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.stopPropagation()}
  >
    <header class="content-header">
      <div class="title-row">
        <h2>Plugins</h2>
      </div>
      <button class="icon" aria-label="Close plugins" onclick={close}>✕</button>
    </header>

    <div class="content-body">
      <div class="section-block">
        <div class="section-title">Plugin profiles</div>
        <div class="field">
          <label for="pluginProfileSelect">Active profile</label>
          <select id="pluginProfileSelect" bind:value={localActiveProfile} onchange={(e) => selectProfile((e.currentTarget as HTMLSelectElement).value)}>
            {#each pluginProfiles as profileName}
              <option value={profileName}>{profileName}</option>
            {/each}
          </select>
        </div>
        <div class="field">
          <label for="newPluginProfile">Create profile</label>
          <div class="row">
            <input id="newPluginProfile" placeholder="profile name" bind:value={newPluginProfileName} />
            <button type="button" class="ghost" onclick={createProfile}>Create</button>
          </div>
        </div>
      </div>

      <div class="section-block">
        <div class="section-title">Available plugins</div>
        {#if !plugins.length}
          <div class="meta">No plugin files found in data/plugins.</div>
        {:else}
          <div class="plugins-grid">
            {#each pluginViewRows as row (row.plugin.module_name)}
              <label class="plugin-card">
                <div class="plugin-card-head">
                  <div class="plugin-name">{row.plugin.name}</div>
                  <span
                    class="plugin-badge"
                    class:active={row.enabled}
                    class:pending-enable={row.pendingEnable}
                    class:pending-disable={row.pendingDisable}
                  >
                    {row.badgeText}
                  </span>
                </div>
                <div class="plugin-meta">{row.plugin.module_name}</div>
                <div class="plugin-file">{row.plugin.file_name}</div>
                <div class="plugin-toggle-row">
                  <span
                    class="plugin-state-text"
                    class:pending-enable={row.pendingEnable}
                    class:pending-disable={row.pendingDisable}
                  >
                    {row.statusText}
                  </span>
                  <span class="plugin-toggle">
                    <input
                      type="checkbox"
                      checked={row.enabled}
                      onchange={(e) => toggleModule(row.plugin.module_name, (e.currentTarget as HTMLInputElement).checked)}
                    />
                    <span class="plugin-toggle-slider"></span>
                  </span>
                </div>
              </label>
            {/each}
          </div>
        {/if}
      </div>
    </div>

    <footer class="modal-actions">
      <button class="ghost" onclick={close}>Cancel</button>
      <button class="primary" onclick={save} disabled={busy}>Save</button>
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
    z-index: 10000;
    padding: 1.25rem;
  }

  .modal-backdrop.window-mode {
    padding: 0;
    background: var(--surface-2);
  }

  .modal {
    width: min(920px, 96vw);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: 0 20px 40px rgba(0, 0, 0, 0.45);
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto;
    max-height: min(760px, 92vh);
  }

  .modal.window-mode {
    width: 100vw;
    max-width: 100vw;
    height: 100dvh;
    max-height: 100dvh;
    border-radius: 0;
    border: 0;
    box-shadow: none;
    box-sizing: border-box;
  }

  .modal.draggable-popup {
    transform: translate(calc(var(--popup-dx, 0px)), calc(var(--popup-dy, 0px)));
  }

  .content-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 18px;
    border-bottom: 1px solid var(--border);
  }

  .title-row {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
  }

  .content-header h2 {
    margin: 0;
    font-size: 1.1rem;
  }

  .content-body {
    padding: 12px 16px;
    overflow: auto;
    display: grid;
    gap: 12px;
    align-content: start;
    min-height: 0;
  }

  .section-block {
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface);
    padding: 0.75rem;
    display: grid;
    gap: 0.7rem;
  }

  .section-title {
    font-weight: 700;
    font-size: 0.95rem;
  }

  .field {
    display: grid;
    gap: 0.4rem;
  }

  .row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 0.5rem;
  }

  .plugins-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: 0.7rem;
  }

  .plugin-card {
    border: 1px solid var(--border);
    border-radius: 12px;
    background: color-mix(in srgb, var(--surface) 88%, #000 12%);
    padding: 0.75rem;
    display: grid;
    gap: 0.5rem;
    cursor: pointer;
  }

  .plugin-card:hover {
    border-color: var(--accent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 28%, transparent);
  }

  .plugin-card-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.6rem;
  }

  .plugin-name {
    font-weight: 700;
    color: var(--text);
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .plugin-badge {
    border: 1px solid var(--border-strong);
    border-radius: 999px;
    padding: 0.12rem 0.5rem;
    font-size: 0.72rem;
    color: var(--text-muted);
    background: var(--surface-2);
    flex: 0 0 auto;
  }

  .plugin-badge.active {
    color: #fff;
    background: color-mix(in srgb, var(--success) 65%, var(--surface-2));
    border-color: color-mix(in srgb, var(--success) 68%, var(--border-strong));
  }

  .plugin-badge.pending-enable {
    color: #f3e8ff;
    background: color-mix(in srgb, #7c3aed 75%, var(--surface-2));
    border-color: color-mix(in srgb, #8b5cf6 78%, var(--border-strong));
  }

  .plugin-badge.pending-disable {
    color: #ffe4e6;
    background: color-mix(in srgb, #dc2626 72%, var(--surface-2));
    border-color: color-mix(in srgb, #ef4444 75%, var(--border-strong));
  }

  .plugin-meta {
    color: var(--text-muted);
    font-size: 0.82rem;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, Liberation Mono, monospace;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .plugin-file {
    color: var(--text);
    font-size: 0.86rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .plugin-toggle-row {
    margin-top: 0.2rem;
    display: flex;
    align-items: center;
    justify-content: space-between;
    color: var(--text-muted);
    font-size: 0.86rem;
  }

  .plugin-state-text.pending-enable {
    color: #c4b5fd;
    font-weight: 700;
  }

  .plugin-state-text.pending-disable {
    color: #fca5a5;
    font-weight: 700;
  }

  .plugin-toggle {
    position: relative;
    display: inline-flex;
    align-items: center;
  }

  .plugin-toggle input {
    position: absolute;
    opacity: 0;
    width: 0;
    height: 0;
  }

  .plugin-toggle-slider {
    width: 38px;
    height: 22px;
    border-radius: 999px;
    background: var(--surface-3);
    border: 1px solid var(--border-strong);
    position: relative;
    transition: background 0.15s ease;
  }

  .plugin-toggle-slider::after {
    content: "";
    position: absolute;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    left: 2px;
    top: 2px;
    background: var(--text);
    transition: transform 0.15s ease;
  }

  .plugin-toggle input:checked + .plugin-toggle-slider {
    background: color-mix(in srgb, var(--accent) 70%, var(--surface-3));
    border-color: color-mix(in srgb, var(--accent) 60%, var(--border-strong));
  }

  .plugin-toggle input:checked + .plugin-toggle-slider::after {
    transform: translateX(16px);
    background: #fff;
  }

  .meta {
    color: var(--text-muted);
    font-size: 0.92rem;
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.75rem;
    padding: 12px 16px;
    border-top: 1px solid var(--border);
    flex-wrap: wrap;
  }

  input,
  select {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.65rem 0.85rem;
    color: var(--text);
    outline: none;
  }

  input:focus,
  select:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 24%, transparent);
  }

  button {
    background: var(--surface-3);
    border: 1px solid var(--border-strong);
    border-radius: 12px;
    padding: 0.62rem 1rem;
    font-weight: 600;
    cursor: pointer;
    color: var(--text);
  }

  button:hover {
    background: var(--surface-2);
  }

  button.primary {
    background: var(--accent);
    color: #fff;
  }

  button.ghost {
    background: transparent;
    border: 1px solid var(--border);
  }

  button.icon {
    width: 34px;
    height: 34px;
    padding: 0;
    background: transparent;
    border: 1px solid var(--border);
    border-radius: 10px;
  }

  @media (max-width: 820px) {
    .plugins-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
