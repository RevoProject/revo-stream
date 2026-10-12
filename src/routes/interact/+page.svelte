<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { goto } from "$app/navigation";
  import Icon from "$lib/components/UI/Icon.svelte";

  let sourceId = "";
  let frame: string | null = null;
  let viewport = { width: 1280, height: 720 };
  let status = "Starting interaction session...";
  let error: string | null = null;
  let running = false;
  let stageEl: HTMLDivElement | null = null;
  let streamEl: HTMLDivElement | null = null;
  let movePending = false;
  let lastPointer = { x: 0, y: 0, buttons: 0 };
  let keysHeld = new Set<string>();

  const startSession = async () => {
    try {
      await invoke("obs_open_source_interaction", { id: sourceId });
      error = null;
      status = "Interaction session ready";
      void loop();
    } catch (err) {
      error = String(err);
    }
  };

  const loop = async () => {
    if (running) return;
    running = true;
    try {
      while (!error) {
        const frameData = await invoke<string>("obs_interact_frame", { sourceId });
        frame = frameData;
        await new Promise((resolve) => setTimeout(resolve, 120));
      }
    } catch (err) {
      error = String(err);
    } finally {
      running = false;
    }
  };

  const pagePoint = (event: PointerEvent | WheelEvent) => {
    const img = streamEl?.querySelector("img");
    if (!img || !stageEl) return null;
    const bounds = img.getBoundingClientRect();
    const scaleX = viewport.width / bounds.width;
    const scaleY = viewport.height / bounds.height;
    return {
      x: Math.round((event.clientX - bounds.left) * scaleX),
      y: Math.round((event.clientY - bounds.top) * scaleY)
    };
  };

  const sendMouse = (kind: string, event: PointerEvent | WheelEvent, extra: Record<string, unknown> = {}) => {
    const point = pagePoint(event);
    if (!point) return;
    const button = "button" in event
      ? (event.button === 1 ? "middle" : event.button === 2 ? "right" : "left")
      : "none";
    const buttons = "buttons" in event ? event.buttons : 0;
    void invoke("obs_interact_mouse", {
      sourceId,
      kind,
      x: point.x,
      y: point.y,
      button,
      buttons,
      clickCount: extra.clickCount ?? 0,
      deltaX: extra.deltaX ?? 0,
      deltaY: extra.deltaY ?? 0
    }).catch(() => undefined);
  };

  const onPointerMove = (event: PointerEvent) => {
    if (movePending) return;
    const point = pagePoint(event);
    if (!point) return;
    movePending = true;
    window.setTimeout(() => (movePending = false), 30);
    lastPointer = { x: point.x, y: point.y, buttons: event.buttons };
    sendMouse("mouseMoved", event);
  };

  const onPointerDown = (event: PointerEvent) => {
    event.preventDefault();
    sendMouse("mousePressed", event, { clickCount: event.detail || 1 });
  };

  const onPointerUp = (event: PointerEvent) => {
    sendMouse("mouseReleased", event, { clickCount: event.detail || 1 });
  };

  const onWheel = (event: WheelEvent) => {
    event.preventDefault();
    const point = pagePoint(event);
    if (!point) return;
    void invoke("obs_interact_mouse", {
      sourceId,
      kind: "mouseWheel",
      x: point.x,
      y: point.y,
      button: "none",
      buttons: 0,
      clickCount: 0,
      deltaX: event.deltaX,
      deltaY: event.deltaY
    }).catch(() => undefined);
  };

  const mapKey = (event: KeyboardEvent) => {
    const params: { key: string; code: string; keyCode: number; text: string } = {
      key: event.key === " " ? " " : event.key,
      code: event.code || "",
      keyCode: event.keyCode,
      text: ""
    };
    if (event.key.length === 1 && !event.ctrlKey && !event.metaKey && !event.altKey) {
      params.text = event.key;
    }
    return params;
  };

  const onKeyDown = (event: KeyboardEvent) => {
    event.preventDefault();
    const params = mapKey(event);
    const heldKey = `${event.code}:${event.location}`;
    if (keysHeld.has(heldKey)) return;
    keysHeld.add(heldKey);
    void invoke("obs_interact_key", {
      sourceId,
      kind: params.text ? "char" : "keyDown",
      key: params.key,
      code: params.code,
      keyCode: params.keyCode,
      text: params.text
    }).catch(() => undefined);
  };

  const onKeyUp = (event: KeyboardEvent) => {
    event.preventDefault();
    const params = mapKey(event);
    const heldKey = `${event.code}:${event.location}`;
    keysHeld.delete(heldKey);
    void invoke("obs_interact_key", {
      sourceId,
      kind: "keyUp",
      key: params.key,
      code: params.code,
      keyCode: params.keyCode,
      text: ""
    }).catch(() => undefined);
  };

  const onContextMenu = (event: MouseEvent) => event.preventDefault();

  onMount(() => {
    const query = new URLSearchParams(window.location.search);
    const id = query.get("id") ?? "";
    if (!id) {
      goto("/");
      return;
    }
    sourceId = id;
    void startSession();
    return () => {
      error = "closed";
      void invoke("obs_interact_close", { sourceId }).catch(() => undefined);
    };
  });
</script>

<div class="interact-stage" bind:this={stageEl}>
  <header class="interact-header">
    <span class="interact-status" role="status">{error ?? status}</span>
    <button
      type="button"
      class="ghost interact-reapply"
      title="Re-run custom CSS/JS in the page"
      onclick={() => void invoke("obs_interact_reapply", { sourceId }).catch(() => undefined)}
    ><Icon name="refresh" size={14} /> Reapply code</button>
  </header>
  {#if error && !frame}
    <div class="interact-empty" role="alert">{error}</div>
  {:else}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
    <div
      class="interact-stream"
      bind:this={streamEl}
      tabindex="0"
      role="application"
      aria-label="Browser source interaction surface"
      onpointermove={onPointerMove}
      onpointerdown={onPointerDown}
      onpointerup={onPointerUp}
      onwheel={onWheel}
      onkeydown={onKeyDown}
      onkeyup={onKeyUp}
      oncontextmenu={onContextMenu}
    >
      {#if frame}
        <img src={frame} alt="Live browser source" draggable="false" />
      {:else}
        <span class="interact-empty">Waiting for first frame...</span>
      {/if}
    </div>
  {/if}
</div>

<style>
  .interact-stage {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    height: 100vh;
    background: #0b0e14;
    color: var(--text, #e5e9f0);
  }

  .interact-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    padding: 8px 12px;
    border-bottom: 1px solid var(--border, #2a2f3a);
  }

  .interact-status {
    font-size: 12px;
    color: var(--text-muted, #9aa3b2);
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .interact-reapply {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border: 1px solid var(--border, #2a2f3a);
    border-radius: 8px;
    background: transparent;
    color: var(--text, #e5e9f0);
    padding: 4px 10px;
    cursor: pointer;
    font-size: 12px;
  }

  .interact-stream {
    display: grid;
    place-items: center;
    overflow: hidden;
    outline: none;
    cursor: default;
    user-select: none;
  }

  .interact-stream img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    pointer-events: none;
  }

  .interact-empty {
    padding: 1.5rem;
    color: var(--text-muted, #9aa3b2);
    font-size: 14px;
    text-align: center;
  }
</style>
