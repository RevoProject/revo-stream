<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import type { DemoSource } from "../../types";

  export let sources: DemoSource[] = [];
  // Most important layer at top, least at bottom
  $: sortedSources = [...sources].reverse();
  export let emptyMessage = "No sources available";

  const dispatch = createEventDispatcher();
  let dragSourceId: string | null = null;
  let dragOverId: string | null = null;
  let dragInsertPosition: "before" | "after" | null = null;
  let selectedSourceId: string | null = null;

  $: if (selectedSourceId && !sources.some((s) => s.id === selectedSourceId)) {
    selectedSourceId = null;
  }
  $: selectedSource = selectedSourceId ? sources.find((s) => s.id === selectedSourceId) ?? null : null;
  $: selectedBrowserSource = selectedSource && selectedSource.source_type?.toLowerCase() === "browser_source" ? selectedSource : null;
  $: selectedTextSource = selectedSource && selectedSource.source_type?.toLowerCase().includes("text") ? selectedSource : null;
  $: selectedColorSource = selectedSource && selectedSource.source_type?.toLowerCase().includes("color_source") ? selectedSource : null;
  $: selectedImageSource = selectedSource && ((selectedSource.source_type?.toLowerCase().includes("image") ?? false) || (selectedSource.source_type?.toLowerCase().includes("slideshow") ?? false)) ? selectedSource : null;
  const isDeviceSelectableSourceType = (type: string | undefined) => {
    const t = (type ?? "").toLowerCase();
    return (
      t === "pulse_input_capture" ||
      t === "pulse_output_capture" ||
      t.includes("audio_input") ||
      t.includes("audio_output") ||
      t.includes("audio_capture") ||
      t.includes("audio_line") ||
      t.includes("video_capture") ||
      t.includes("v4l2_input")
    );
  };
  $: selectedDeviceSource = selectedSource && isDeviceSelectableSourceType(selectedSource.source_type) ? selectedSource : null;

  const openAddSource = () => dispatch("openAddSource");
  const openInteract = () => {
    if (!selectedBrowserSource) return;
    dispatch("interact", { source: selectedBrowserSource });
  };
  const openTextEdit = () => {
    if (!selectedTextSource) return;
    dispatch("textEdit", { source: selectedTextSource });
  };
  const openChangeColor = () => {
    if (!selectedColorSource) return;
    dispatch("quickChangeColor", { source: selectedColorSource });
  };
  const openSelectFile = () => {
    if (!selectedImageSource) return;
    dispatch("quickSelectFile", { source: selectedImageSource });
  };
  const openSelectDevice = () => {
    if (!selectedDeviceSource) return;
    dispatch("quickSelectDevice", { source: selectedDeviceSource });
  };
  const toggleVisibility = (source: DemoSource) => dispatch("toggleVisibility", { source });
  // Swap arrow logic: up moves up, down moves down
  const move = (source: DemoSource, direction: "up" | "down" | "top" | "bottom") => {
    dispatch("move", { source, direction });
  };
    // Lock logic
    const toggleLock = (source: DemoSource) => dispatch("toggleLock", { source });
  const openEdit = (source: DemoSource) => dispatch("openEdit", { source });
  const openMenu = (event: MouseEvent, source: DemoSource) => dispatch("openMenu", { event, source });
  const reorder = (sourceId: string, toIndex: number) => dispatch("reorder", { sourceId, toIndex });
  const selectSource = (source: DemoSource) => {
    selectedSourceId = source.id;
  };

  function handleDragStart(event: DragEvent, source: DemoSource) {
    if (source.locked) {
      event.preventDefault();
      return;
    }
    dragSourceId = source.id;
    event.dataTransfer?.setData("text/plain", source.id);
    if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
  }

  function handleDragOver(event: DragEvent, target: DemoSource) {
    if (target.locked) return;
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
    if (dragSourceId === target.id) {
      dragOverId = null;
      dragInsertPosition = null;
      return;
    }
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    const offset = event.clientY - rect.top;
    dragOverId = target.id;
    dragInsertPosition = offset < rect.height / 2 ? "before" : "after";
  }

  function handleDragLeave(event: DragEvent) {
    const related = event.relatedTarget as Node | null;
    if (related && (event.currentTarget as HTMLElement).contains(related)) return;
    dragOverId = null;
    dragInsertPosition = null;
  }

  function handleDragEnd() {
    dragSourceId = null;
    dragOverId = null;
    dragInsertPosition = null;
  }

  function handleDrop(event: DragEvent, target: DemoSource) {
    event.preventDefault();
    if (target.locked) {
      dragSourceId = null;
      dragOverId = null;
      dragInsertPosition = null;
      return;
    }
    const id = dragSourceId ?? event.dataTransfer?.getData("text/plain");
    dragSourceId = null;
    dragOverId = null;
    const insert = dragInsertPosition;
    dragInsertPosition = null;
    if (!id || id === target.id) return;
    const source = sources.find((s) => s.id === id);
    if (!source || source.locked) return;
    const targetDisplayIndex = sortedSources.findIndex((s) => s.id === target.id);
    if (targetDisplayIndex === -1) return;
    const displayIndex = insert === "after" ? targetDisplayIndex + 1 : targetDisplayIndex;
    const toIndex = Math.max(0, Math.min(sources.length, sources.length - displayIndex));
    reorder(id, toIndex);
  }
</script>

<div class="sources">
  <div class="sources-header">
    <div class="sources-header-left">
      <h2>Sources</h2>
      {#if selectedBrowserSource}
        <button class="interact" aria-label="Interact with browser source" onclick={openInteract}>Interact</button>
      {:else if selectedTextSource}
        <button class="interact" aria-label="Edit text source" onclick={openTextEdit}>Text Edit</button>
      {:else if selectedColorSource}
        <button class="interact" aria-label="Change source color" onclick={openChangeColor}>Change color</button>
      {:else if selectedImageSource}
        <button class="interact" aria-label="Select image file" onclick={openSelectFile}>Select file</button>
      {:else if selectedDeviceSource}
        <button class="interact" aria-label="Select source device" onclick={openSelectDevice}>Select device</button>
      {/if}
    </div>
    <button class="icon" aria-label="Add source" onclick={openAddSource}>+</button>
  </div>
  <div class="source-list">
    {#if sources.length === 0}
      <span class="muted">{emptyMessage}</span>
    {:else}
      {#each sortedSources as source (source.id)}
        <div
          class="source-item"
          class:locked={source.locked}
          class:selected={selectedSourceId === source.id}
          class:drop-before={dragOverId === source.id && dragInsertPosition === "before"}
          class:drop-after={dragOverId === source.id && dragInsertPosition === "after"}
          tabindex="0"
          role="button"
          oncontextmenu={(e) => {
            selectSource(source);
            openMenu(e, source);
          }}
          draggable={!source.locked}
          ondragstart={(e) => handleDragStart(e, source)}
          ondragover={(e) => handleDragOver(e, source)}
          ondragleave={handleDragLeave}
          ondragend={handleDragEnd}
          ondrop={(e) => handleDrop(e, source)}
        >
            <button class="source-name" type="button" title={source.name} onclick={() => selectSource(source)} ondblclick={() => openEdit(source)}>
              {source.name}
            </button>
            <div class="order-controls">
              <button class="order-btn" aria-label="Move up" disabled={source.locked} onclick={() => move(source, "up")}> 
                <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true" focusable="false">
                  <path fill="currentColor" d="M12 4l-8 8h6v8h4v-8h6z"/>
                </svg>
              </button>
              <button class="order-btn" aria-label="Move down" disabled={source.locked} onclick={() => move(source, "down")}> 
                <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true" focusable="false">
                  <path fill="currentColor" d="M12 20l8-8h-6V4h-4v8H4z"/>
                </svg>
              </button>
            </div>
            <button class="eye" aria-label="Toggle visibility" onclick={() => toggleVisibility(source)}>
              {#if source.visible}
                <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
                  <path
                    fill="currentColor"
                    d="M12 5c4.5 0 8.3 2.9 9.8 7-1.5 4.1-5.3 7-9.8 7S3.7 16.1 2.2 12C3.7 7.9 7.5 5 12 5zm0 2c-3.2 0-5.9 2-7.2 5 1.3 3 4 5 7.2 5s5.9-2 7.2-5c-1.3-3-4-5-7.2-5zm0 2.5a2.5 2.5 0 1 1 0 5 2.5 2.5 0 0 1 0-5z"
                  />
                </svg>
              {:else}
                <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
                  <path
                    fill="currentColor"
                    d="M2.8 4.2 4.2 2.8l16.9 16.9-1.4 1.4-3.1-3.1c-1.3.6-2.8 1-4.6 1-4.5 0-8.3-2.9-9.8-7 1-2.7 3-4.9 5.5-6.1L2.8 4.2zm7.2 7.2a2.5 2.5 0 0 0 3.4 3.4l-3.4-3.4zM12 5c4.5 0 8.3 2.9 9.8 7-.7 1.9-1.9 3.6-3.5 4.8l-1.5-1.5c1.3-.9 2.3-2.1 2.9-3.3-1.3-3-4-5-7.2-5-1 0-1.9.2-2.8.5L7.7 5.7C9 5.2 10.5 5 12 5z"
                  />
                </svg>
              {/if}
            </button>
            <button
              class="lock"
              aria-label={source.locked ? "Unlock source" : "Lock source"}
              title={source.locked ? "Locked" : "Unlocked"}
              onclick={() => toggleLock(source)}
            >
              {#if source.locked}
                <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
                  <path fill="currentColor" d="M7 10V8a5 5 0 1 1 10 0v2h1a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2v-8a2 2 0 0 1 2-2h1Zm2 0h6V8a3 3 0 1 0-6 0v2Zm3 4a2 2 0 0 1 1 3.732V19h-2v-1.268A2 2 0 0 1 12 14Z"/>
                </svg>
              {:else}
                <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
                  <path fill="currentColor" d="M7 10V8a5 5 0 0 1 8.662-3.396l-1.414 1.414A3 3 0 0 0 9 8v2h9a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2v-8a2 2 0 0 1 2-2h1Zm5 4a2 2 0 0 1 1 3.732V19h-2v-1.268A2 2 0 0 1 12 14Z"/>
                </svg>
              {/if}
            </button>
        </div>
      {/each}
    {/if}
  </div>
</div>

  <style>
    .sources {
      display: flex;
      flex-direction: column;
      min-height: 0;
      height: 100%;
    }
    .sources-header {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 0.75rem;
    }

    .sources-header-left {
      display: inline-flex;
      align-items: center;
      gap: 0.5rem;
      min-width: 0;
    }

    .interact {
      background: var(--surface-3);
      color: var(--text);
      border: 1px solid var(--border-strong);
      border-radius: 8px;
      padding: 0.28rem 0.55rem;
      font-size: 0.82rem;
      line-height: 1.1;
      white-space: nowrap;
    }

    .interact:hover {
      background: var(--surface-2);
    }

    .sources-header .icon {
      background: var(--surface-3);
      color: var(--text);
      border: 1px solid var(--border-strong);
      width: 30px;
      height: 30px;
      border-radius: 8px;
      padding: 0;
    }

    .source-list {
      display: flex;
      flex-direction: column;
      gap: 0.4rem;
      background: var(--surface-2);
      border: 1px solid var(--border);
      border-radius: 8px;
      padding: 0.35rem;
      min-height: 160px;
      overflow: auto;
      flex: 1 1 auto;
    }

    .source-list > * {
      display: flex;
      align-items: stretch;
      gap: 0.6rem;
      padding: 0.5rem 0.65rem;
      border-radius: 6px;
      background: var(--surface-3);
      color: var(--text);
      font-size: 0.95rem;
    }

    .source-list > * + * {
      border-top: 1px solid var(--border);
    }

    .source-list > *:hover {
      background: var(--surface-2);
    }

    .source-item {
      position: relative;
      display: grid;
      grid-template-columns: minmax(0, 1fr) auto auto auto;
      align-items: center;
      gap: 0.5rem;
    }

    .source-item.selected {
      outline: 1px solid var(--accent);
      outline-offset: -1px;
    }

    .source-item.locked {
      box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 45%, transparent);
    }

    .source-item.drop-before::before,
    .source-item.drop-after::after {
      content: "";
      position: absolute;
      left: 0.5rem;
      right: 0.5rem;
      height: 2px;
      background: var(--accent);
      border-radius: 999px;
    }

    .source-item.drop-before::before {
      top: -2px;
    }

    .source-item.drop-after::after {
      bottom: -2px;
    }

    .source-name {
      min-width: 0;
      display: block;
      background: transparent;
      border: none;
      color: var(--text);
      text-align: left;
      padding: 0;
      cursor: text;
      white-space: normal;
      word-break: break-word;
      line-height: 1.25;
    }

    .eye {
      background: transparent;
      border: 1px solid var(--border-strong);
      color: var(--text);
      width: 32px;
      height: 32px;
      border-radius: 8px;
      padding: 0;
      display: inline-flex;
      align-items: center;
      justify-content: center;
    }

    .lock {
      background: transparent;
      border: 1px solid var(--border-strong);
      color: var(--text);
      width: 32px;
      height: 32px;
      border-radius: 8px;
      padding: 0;
      display: inline-flex;
      align-items: center;
      justify-content: center;
    }

    .lock:hover {
      background: var(--surface-2);
    }

    .order-controls {
      display: flex;
      flex-direction: row;
      gap: 0.25rem;
      flex-shrink: 0;
    }

    .order-btn {
      background: var(--surface-2);
      border: 1px solid var(--border-strong);
      color: var(--text);
      width: 32px;
      height: 32px;
      border-radius: 6px;
      padding: 0;
      font-size: 0.9rem;
      line-height: 1;
    }

    .order-btn:hover {
      background: var(--surface-3);
    }

    .order-btn:disabled {
      opacity: 0.45;
      cursor: not-allowed;
    }

    .eye svg {
      width: 18px;
      height: 18px;
    }

    .lock svg {
      width: 18px;
      height: 18px;
    }

    button {
      border: none;
      background: transparent;
      color: inherit;
      cursor: pointer;
    }
  </style>
