<script lang="ts">
  type DockSide = "left" | "right" | "bottom" | null;
  type DockMenuState = { open: boolean; x: number; y: number };

  export let dockMenu: DockMenuState = { open: false, x: 0, y: 0 };
  export let dockPinnedSide: DockSide = null;
  export let dockPaneWidth = 420;
  export let dockPaneEl: HTMLElement | null = null;
  export let dockBodyEl: HTMLDivElement | null = null;
  export let browserDockTitle = "Dock 1";
  export let dockEngineActive = false;
  export let dockEngineLabel = "chromium";
  export let isReleaseBuild = false;
  export let dockFrameKey = 0;
  export let dockHostWebviewUrl = "";
  export let dockCanvasRuntimeReady = false;
  export let dockCanvasUnavailableReason = "";
  export let dockFrameBlocked = false;
  export let dockFrameErrorMessage = "";
  export let showDockHandle = false;
  export let dockDragActive = false;
  export let dockDropTarget: "left" | "right" | null = null;

  export let moveDockToSide: (side: "left" | "right" | "bottom") => void | Promise<void> = () => {};
  export let removeDockFromWorkspace: () => void | Promise<void> = () => {};
  export let closeDockMenu: () => void = () => {};
  export let handleBackdropKey: (event: KeyboardEvent, closeFn: () => void | Promise<void>) => void = () => {};
  export let startDockResize: (event: MouseEvent, side: "left" | "right") => void = () => {};
  export let openDockMenu: (event: MouseEvent) => void = () => {};
  export let startDockHandleDrag: (event: DragEvent) => void = () => {};
  export let handleDockHandleDragEnd: () => void = () => {};
  export let startDockHeaderMove: (event: MouseEvent) => void = () => {};
  export let undockDockPane: () => void | Promise<void> = () => {};
  export let handleDockRefreshClick: () => void | Promise<void> = () => {};
  export let handleDockZoneDragOver: (event: DragEvent, side: "left" | "right") => void = () => {};
  export let handleDockZoneDrop: (event: DragEvent, side: "left" | "right") => void = () => {};
</script>

{#if dockMenu.open}
  <div class="context-menu" style={`top:${dockMenu.y}px; left:${dockMenu.x}px;`} role="menu">
    {#if dockPinnedSide === "left"}
      <button on:click={() => void moveDockToSide("right")}>Move to right</button>
      <button on:click={() => void moveDockToSide("bottom")}>Move to bottom</button>
    {:else if dockPinnedSide === "right"}
      <button on:click={() => void moveDockToSide("left")}>Move to left</button>
      <button on:click={() => void moveDockToSide("bottom")}>Move to bottom</button>
    {:else if dockPinnedSide === "bottom"}
      <button on:click={() => void moveDockToSide("left")}>Move to left</button>
      <button on:click={() => void moveDockToSide("right")}>Move to right</button>
    {:else}
      <button on:click={() => void moveDockToSide("left")}>Move to left</button>
      <button on:click={() => void moveDockToSide("right")}>Move to right</button>
      <button on:click={() => void moveDockToSide("bottom")}>Move to bottom</button>
    {/if}
    <button on:click={() => void removeDockFromWorkspace()}>Remove</button>
  </div>
  <div
    class="context-overlay"
    role="button"
    tabindex="0"
    on:click={closeDockMenu}
    on:keydown={(e) => handleBackdropKey(e, closeDockMenu)}
  ></div>
{/if}

<div class="workspace-shell" class:dock-left={dockPinnedSide === "left"} class:dock-right={dockPinnedSide === "right"} class:dock-bottom={dockPinnedSide === "bottom"}>
  {#if dockPinnedSide === "left"}
    <aside class="browser-dock-pane" bind:this={dockPaneEl} style={`--dock-pane-width:${dockPaneWidth}px;`}>
      <button
        type="button"
        class="dock-resize-handle right"
        aria-label="Resize dock"
        on:mousedown={(e) => startDockResize(e, "left")}
      ></button>
      <div
        class="dock-compact-header"
        role="button"
        tabindex="0"
        draggable="true"
        on:contextmenu={openDockMenu}
        on:dragstart={startDockHandleDrag}
        on:dragend={handleDockHandleDragEnd}
        on:mousedown={startDockHeaderMove}
        on:dblclick={() => {
          if (isReleaseBuild) void undockDockPane();
        }}
        title="Drag to move"
      >
        <span>{browserDockTitle}</span>
        <span class="dock-engine" class:active={dockEngineActive}>{dockEngineLabel}</span>
        <div class="dock-header-actions">
          {#if isReleaseBuild}
            <button class="dock-popout" on:click={() => void undockDockPane()} aria-label="Undock to separate window">↗</button>
          {/if}
          <button
            class="dock-refresh"
            draggable="true"
            on:dragstart={startDockHandleDrag}
            on:dragend={handleDockHandleDragEnd}
            on:click={handleDockRefreshClick}
            aria-label="Refresh dock content"
          >↻</button>
        </div>
      </div>
      <div class="dock-body" bind:this={dockBodyEl}>
        {#if isReleaseBuild}
          {#key dockFrameKey}
            <iframe class="dock-iframe" src={dockHostWebviewUrl} title="Dock webview host"></iframe>
          {/key}
        {:else}
          <div class="dock-frame-warning dock-frame-warning-top">
            <p>Webview frame and dock webview rendering is available only in release mode</p>
          </div>
        {/if}
        {#if !dockCanvasRuntimeReady}
          <div class="dock-frame-warning dock-frame-warning-top">
            <p>{dockCanvasUnavailableReason}</p>
          </div>
        {/if}
        {#if dockFrameBlocked}
          <div class="dock-frame-warning">
            <p>Canvas frame pipeline failed for this dock source.</p>
            {#if dockFrameErrorMessage}
              <p>{dockFrameErrorMessage}</p>
            {/if}
            <button on:click={() => void undockDockPane()}>Open source URL in separate window</button>
          </div>
        {/if}
      </div>
    </aside>
  {/if}

  <div class="workspace-main">
    {#if !dockPinnedSide && showDockHandle}
      <div class="dock-floating-row">
        <div
          class="dock-compact-header dock-compact-floating"
          role="button"
          tabindex="0"
          draggable="true"
          on:contextmenu={openDockMenu}
          on:dragstart={startDockHandleDrag}
          on:dragend={handleDockHandleDragEnd}
          title="Drag to pin dock"
        >
          <span>{browserDockTitle}</span>
          <span class="dock-engine" class:active={dockEngineActive}>{dockEngineLabel}</span>
          <button
            class="dock-refresh"
            draggable="true"
            on:dragstart={startDockHandleDrag}
            on:dragend={handleDockHandleDragEnd}
            on:click={handleDockRefreshClick}
            aria-label="Open/refresh dock"
          >↻</button>
        </div>
      </div>
    {/if}

    <slot />
  </div>

  {#if dockPinnedSide === "bottom"}
    <aside class="browser-dock-pane dock-pane-bottom" bind:this={dockPaneEl}>
      <div
        class="dock-compact-header"
        role="button"
        tabindex="0"
        draggable="true"
        on:contextmenu={openDockMenu}
        on:dragstart={startDockHandleDrag}
        on:dragend={handleDockHandleDragEnd}
        on:mousedown={startDockHeaderMove}
        on:dblclick={() => {
          if (isReleaseBuild) void undockDockPane();
        }}
        title="Drag to move"
      >
        <span>{browserDockTitle}</span>
        <span class="dock-engine" class:active={dockEngineActive}>{dockEngineLabel}</span>
        <div class="dock-header-actions">
          {#if isReleaseBuild}
            <button class="dock-popout" on:click={() => void undockDockPane()} aria-label="Undock to separate window">↗</button>
          {/if}
          <button
            class="dock-refresh"
            draggable="true"
            on:dragstart={startDockHandleDrag}
            on:dragend={handleDockHandleDragEnd}
            on:click={handleDockRefreshClick}
            aria-label="Refresh dock content"
          >↻</button>
        </div>
      </div>
      <div class="dock-body" bind:this={dockBodyEl}>
        {#if isReleaseBuild}
          {#key dockFrameKey}
            <iframe class="dock-iframe" src={dockHostWebviewUrl} title="Dock webview host"></iframe>
          {/key}
        {:else}
          <div class="dock-frame-warning dock-frame-warning-top">
            <p>Webview frame and dock webview rendering is available only in release mode.</p>
          </div>
        {/if}
        {#if !dockCanvasRuntimeReady}
          <div class="dock-frame-warning dock-frame-warning-top">
            <p>{dockCanvasUnavailableReason}</p>
          </div>
        {/if}
        {#if dockFrameBlocked}
          <div class="dock-frame-warning">
            <p>Canvas frame pipeline failed for this dock source.</p>
            {#if dockFrameErrorMessage}
              <p>{dockFrameErrorMessage}</p>
            {/if}
            <button on:click={() => void undockDockPane()}>Open source URL in separate window</button>
          </div>
        {/if}
      </div>
    </aside>
  {/if}

  {#if dockPinnedSide === "right"}
    <aside class="browser-dock-pane" bind:this={dockPaneEl} style={`--dock-pane-width:${dockPaneWidth}px;`}>
      <button
        type="button"
        class="dock-resize-handle left"
        aria-label="Resize dock"
        on:mousedown={(e) => startDockResize(e, "right")}
      ></button>
      <div
        class="dock-compact-header"
        role="button"
        tabindex="0"
        draggable="true"
        on:contextmenu={openDockMenu}
        on:dragstart={startDockHandleDrag}
        on:dragend={handleDockHandleDragEnd}
        on:mousedown={startDockHeaderMove}
        on:dblclick={() => {
          if (isReleaseBuild) void undockDockPane();
        }}
        title="Drag to move"
      >
        <span>{browserDockTitle}</span>
        <span class="dock-engine" class:active={dockEngineActive}>{dockEngineLabel}</span>
        <div class="dock-header-actions">
          {#if isReleaseBuild}
            <button class="dock-popout" on:click={() => void undockDockPane()} aria-label="Undock to separate window">↗</button>
          {/if}
          <button
            class="dock-refresh"
            draggable="true"
            on:dragstart={startDockHandleDrag}
            on:dragend={handleDockHandleDragEnd}
            on:click={handleDockRefreshClick}
            aria-label="Refresh dock content"
          >↻</button>
        </div>
      </div>
      <div class="dock-body" bind:this={dockBodyEl}>
        {#if isReleaseBuild}
          {#key dockFrameKey}
            <iframe class="dock-iframe" src={dockHostWebviewUrl} title="Dock webview host"></iframe>
          {/key}
        {:else}
          <div class="dock-frame-warning dock-frame-warning-top">
            <p>Webview frame and dock webview rendering is available only in release mode.</p>
          </div>
        {/if}
        {#if !dockCanvasRuntimeReady}
          <div class="dock-frame-warning dock-frame-warning-top">
            <p>{dockCanvasUnavailableReason}</p>
          </div>
        {/if}
        {#if dockFrameBlocked}
          <div class="dock-frame-warning">
            <p>Canvas frame pipeline failed for this dock source.</p>
            {#if dockFrameErrorMessage}
              <p>{dockFrameErrorMessage}</p>
            {/if}
            <button on:click={() => void undockDockPane()}>Open source URL in separate window</button>
          </div>
        {/if}
      </div>
    </aside>
  {/if}
</div>

{#if dockDragActive}
  <div class="dock-drop-overlay" role="presentation">
    <div
      class="dock-drop-zone"
      role="button"
      tabindex="-1"
      class:active={dockDropTarget === "left"}
      on:dragover={(e) => handleDockZoneDragOver(e, "left")}
      on:drop={(e) => handleDockZoneDrop(e, "left")}
    >
      Pin left
    </div>
    <div
      class="dock-drop-zone"
      role="button"
      tabindex="-1"
      class:active={dockDropTarget === "right"}
      on:dragover={(e) => handleDockZoneDragOver(e, "right")}
      on:drop={(e) => handleDockZoneDrop(e, "right")}
    >
      Pin right
    </div>
  </div>
{/if}

<style>
  .workspace-shell {
    flex: 1 1 auto;
    min-height: 0;
    display: flex;
    gap: 0.75rem;
    padding: 0.75rem 0.75rem 0;
  }

  .workspace-shell.dock-bottom {
    flex-direction: column;
  }

  .workspace-main {
    flex: 1 1 auto;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .dock-floating-row {
    display: flex;
    justify-content: flex-end;
    padding: 0 1.5rem 0.35rem;
    flex: 0 0 auto;
  }

  .dock-compact-floating {
    width: min(360px, 42vw);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow: 0 4px 18px #0006;
  }

  .browser-dock-pane {
    width: clamp(280px, var(--dock-pane-width, 420px), 62vw);
    min-width: 280px;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--surface);
    overflow: hidden;
    display: flex;
    flex-direction: column;
    min-height: 0;
    position: relative;
  }

  .dock-pane-bottom {
    width: 100%;
    min-width: 0;
    height: clamp(220px, 33vh, 460px);
  }

  .dock-resize-handle {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 8px;
    z-index: 8;
    border: 0;
    background: transparent;
    padding: 0;
  }

  .dock-resize-handle.left {
    left: -4px;
    cursor: ew-resize;
  }

  .dock-resize-handle.right {
    right: -4px;
    cursor: ew-resize;
  }

  .dock-compact-header {
    height: 34px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    padding: 0 0.5rem 0 0.65rem;
    border-bottom: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text);
    font-size: 0.83rem;
    font-weight: 600;
    user-select: none;
    cursor: grab;
  }

  .dock-compact-header:active {
    cursor: grabbing;
  }

  .dock-compact-header span {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .dock-engine {
    flex: 0 0 auto;
    font-size: 0.68rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    border: 1px solid var(--border-strong);
    border-radius: 999px;
    padding: 0.08rem 0.4rem;
    color: var(--text-muted);
    background: var(--surface-3);
  }

  .dock-engine.active {
    color: #c7ffd8;
    border-color: #2f8f58;
    background: #19412d;
  }

  .dock-header-actions {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    flex: 0 0 auto;
  }

  .dock-popout {
    width: 26px;
    height: 26px;
    border-radius: 6px;
    border: 1px solid var(--border-strong);
    background: var(--surface-3);
    color: var(--text);
    font-weight: 700;
    line-height: 1;
    cursor: pointer;
  }

  .dock-popout:hover {
    background: var(--surface-2);
  }

  .dock-refresh {
    width: 26px;
    height: 26px;
    border-radius: 6px;
    border: 1px solid var(--border-strong);
    background: var(--surface-3);
    color: var(--text);
    font-weight: 700;
    line-height: 1;
    cursor: pointer;
    flex: 0 0 auto;
  }

  .dock-refresh:hover {
    background: var(--surface-2);
  }

  .dock-body {
    flex: 1 1 auto;
    min-height: 0;
    background: #10131a;
    position: relative;
  }

  .dock-body .dock-iframe {
    border: 0;
    width: 100%;
    height: 100%;
    display: block;
    background: #111;
  }

  .dock-frame-warning {
    position: absolute;
    inset: 0;
    display: grid;
    place-content: center;
    gap: 0.65rem;
    text-align: center;
    padding: 1rem;
    background: color-mix(in srgb, var(--surface) 86%, #000 14%);
  }

  .dock-frame-warning-top {
    place-content: start center;
    inset: 0 0 auto 0;
    min-height: 0;
    padding: 0.35rem 0.5rem;
    background: color-mix(in srgb, var(--surface-2) 90%, #000 10%);
    border-bottom: 1px solid var(--border);
    pointer-events: none;
  }

  .dock-frame-warning-top p {
    font-size: 0.72rem;
  }

  .dock-frame-warning p {
    margin: 0;
    color: var(--text-muted);
  }

  .dock-frame-warning button {
    justify-self: center;
    border: 1px solid var(--border-strong);
    background: var(--surface-3);
    color: var(--text);
    border-radius: 8px;
    padding: 0.4rem 0.75rem;
    cursor: pointer;
  }

  .dock-drop-overlay {
    position: fixed;
    inset: 58px 8px 8px;
    z-index: 120;
    pointer-events: none;
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.75rem;
  }

  .dock-drop-zone {
    pointer-events: auto;
    border: 2px dashed color-mix(in srgb, var(--accent) 55%, transparent);
    border-radius: 14px;
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    color: var(--text);
    font-weight: 700;
    display: grid;
    place-items: center;
    letter-spacing: 0.02em;
  }

  .dock-drop-zone.active {
    background: color-mix(in srgb, var(--accent) 28%, transparent);
    border-color: var(--accent);
  }

  @media (max-width: 1200px) {
    .browser-dock-pane {
      width: min(360px, 40vw);
    }
  }

  @media (max-width: 800px) {
    .dock-floating-row {
      padding: 0 0.5rem 0.35rem;
      justify-content: stretch;
    }

    .dock-compact-floating {
      width: 100%;
    }

    .workspace-shell {
      flex-direction: column;
      gap: 0.5rem;
      padding: 0.5rem 0.5rem 0;
    }

    .browser-dock-pane {
      width: 100%;
      min-width: 0;
      height: 36vh;
    }

    .dock-drop-overlay {
      inset: 56px 6px 6px;
    }
  }
</style>
