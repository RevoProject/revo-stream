<script lang="ts">
  import { onMount } from "svelte";
  import type { DemoSource } from "../../types";

  export let sources: DemoSource[] = [];
  export let onClose: () => void;
  export let onSave: (transforms: Record<string, { x: number; y: number; w: number; h: number }>) => void;
  export let sceneResolution: string;

  type Transform = {
    x: number;
    y: number;
    w: number;
    h: number;
    cropL: number;
    cropR: number;
    cropT: number;
    cropB: number;
  };
  type ResizeHandle = "n" | "s" | "e" | "w" | "ne" | "nw" | "se" | "sw";
  type InteractionMode = "idle" | "move" | "resize" | "crop";

  const MIN_SIZE = 16;

  let transforms: Record<string, Transform> = {};
  let activeId: string | null = null;
  let sceneCanvasEl: HTMLDivElement | null = null;

  let interactionMode: InteractionMode = "idle";
  let interactionId: string | null = null;
  let interactionHandle: ResizeHandle | null = null;
  let interactionStartMouse = { x: 0, y: 0 };
  let interactionStartTransform: Transform | null = null;

  let sceneScale = 1;

  const parseNumber = (raw: unknown, fallback: number) => {
    const parsed = Number(String(raw ?? "").trim());
    return Number.isFinite(parsed) ? parsed : fallback;
  };

  const getSceneSize = () => {
    const m = sceneResolution.match(/^(\d+)x(\d+)$/);
    if (!m) return { width: 1280, height: 720 };
    return { width: +m[1], height: +m[2] };
  };

  const clampTransform = (transform: Transform) => {
    const { width, height } = getSceneSize();
    let w = Math.max(MIN_SIZE, Math.min(width, transform.w));
    let h = Math.max(MIN_SIZE, Math.min(height, transform.h));
    let x = Math.max(0, Math.min(width - w, transform.x));
    let y = Math.max(0, Math.min(height - h, transform.y));
    return {
      x,
      y,
      w,
      h,
      cropL: Math.max(0, Math.min(w - 1, transform.cropL)),
      cropR: Math.max(0, Math.min(w - 1, transform.cropR)),
      cropT: Math.max(0, Math.min(h - 1, transform.cropT)),
      cropB: Math.max(0, Math.min(h - 1, transform.cropB))
    };
  };

  const buildInitialTransform = (source: DemoSource): Transform => {
    const params = source.params ?? {};
    const x = parseNumber(params.pos_x, 100);
    const y = parseNumber(params.pos_y, 100);
    const w = parseNumber(params.item_width, parseNumber(params.width, 320));
    const h = parseNumber(params.item_height, parseNumber(params.height, 180));
    const cropL = parseNumber(params.crop_left, 0);
    const cropR = parseNumber(params.crop_right, 0);
    const cropT = parseNumber(params.crop_top, 0);
    const cropB = parseNumber(params.crop_bottom, 0);
    return clampTransform({ x, y, w, h, cropL, cropR, cropT, cropB });
  };

  const syncTransformsFromSources = () => {
    const next = { ...transforms };
    let changed = false;
    for (const s of sources) {
      if (!next[s.id]) {
        next[s.id] = buildInitialTransform(s);
        changed = true;
      }
    }
    for (const existingId of Object.keys(next)) {
      if (!sources.some((s) => s.id === existingId)) {
        delete next[existingId];
        changed = true;
      }
    }
    if (changed) {
      transforms = next;
      if (activeId && !next[activeId]) {
        activeId = null;
      }
    }
  };

  const updateSceneScale = () => {
    const { width, height } = getSceneSize();
    const maxW = Math.max(320, window.innerWidth - 180);
    const maxH = Math.max(220, window.innerHeight - 300);
    sceneScale = Math.max(0.2, Math.min(1, maxW / width, maxH / height));
  };

  const toScenePoint = (clientX: number, clientY: number) => {
    if (!sceneCanvasEl) return { x: 0, y: 0 };
    const rect = sceneCanvasEl.getBoundingClientRect();
    return {
      x: (clientX - rect.left) / sceneScale,
      y: (clientY - rect.top) / sceneScale
    };
  };

  const applyMove = (id: string, point: { x: number; y: number }) => {
    const start = interactionStartTransform;
    if (!start) return;
    const dx = point.x - interactionStartMouse.x;
    const dy = point.y - interactionStartMouse.y;
    transforms[id] = clampTransform({
      x: start.x + dx,
      y: start.y + dy,
      w: start.w,
      h: start.h,
      cropL: start.cropL,
      cropR: start.cropR,
      cropT: start.cropT,
      cropB: start.cropB
    });
  };

  const applyResizeOrCrop = (id: string, point: { x: number; y: number }) => {
    const start = interactionStartTransform;
    const handle = interactionHandle;
    if (!start || !handle) return;

    const { width: sceneW, height: sceneH } = getSceneSize();
    const dx = point.x - interactionStartMouse.x;
    const dy = point.y - interactionStartMouse.y;

    let x = start.x;
    let y = start.y;
    let w = start.w;
    let h = start.h;

    if (interactionMode === "crop") {
      let cropL = start.cropL;
      let cropR = start.cropR;
      let cropT = start.cropT;
      let cropB = start.cropB;
      if (handle.includes("w")) cropL = Math.max(0, start.cropL + dx);
      if (handle.includes("e")) cropR = Math.max(0, start.cropR - dx);
      if (handle.includes("n")) cropT = Math.max(0, start.cropT + dy);
      if (handle.includes("s")) cropB = Math.max(0, start.cropB - dy);
      transforms[id] = clampTransform({ ...start, cropL, cropR, cropT, cropB });
      return;
    }

    if (handle.includes("e")) w = start.w + dx;
    if (handle.includes("s")) h = start.h + dy;
    if (handle.includes("w")) {
      x = start.x + dx;
      w = start.w - dx;
    }
    if (handle.includes("n")) {
      y = start.y + dy;
      h = start.h - dy;
    }

    if (w < MIN_SIZE) {
      if (handle.includes("w")) x -= MIN_SIZE - w;
      w = MIN_SIZE;
    }
    if (h < MIN_SIZE) {
      if (handle.includes("n")) y -= MIN_SIZE - h;
      h = MIN_SIZE;
    }

    if (x < 0) {
      if (handle.includes("w")) w += x;
      x = 0;
    }
    if (y < 0) {
      if (handle.includes("n")) h += y;
      y = 0;
    }
    if (x + w > sceneW) {
      if (handle.includes("e")) w = sceneW - x;
      else x = sceneW - w;
    }
    if (y + h > sceneH) {
      if (handle.includes("s")) h = sceneH - y;
      else y = sceneH - h;
    }

    transforms[id] = clampTransform({ x, y, w, h, cropL: start.cropL, cropR: start.cropR, cropT: start.cropT, cropB: start.cropB });
  };

  const endInteraction = () => {
    interactionMode = "idle";
    interactionId = null;
    interactionHandle = null;
    interactionStartTransform = null;
    window.removeEventListener("mousemove", onGlobalPointerMove);
    window.removeEventListener("mouseup", onGlobalPointerUp);
  };

  const onGlobalPointerMove = (e: MouseEvent) => {
    if (!interactionId) return;
    const point = toScenePoint(e.clientX, e.clientY);
    if (interactionMode === "move") {
      applyMove(interactionId, point);
      return;
    }
    if (interactionMode === "resize" || interactionMode === "crop") {
      applyResizeOrCrop(interactionId, point);
    }
  };

  const onGlobalPointerUp = () => {
    endInteraction();
  };

  const beginMove = (e: MouseEvent, id: string) => {
    e.stopPropagation();
    activeId = id;
    interactionMode = "move";
    interactionId = id;
    interactionHandle = null;
    interactionStartMouse = toScenePoint(e.clientX, e.clientY);
    interactionStartTransform = transforms[id] ? { ...transforms[id] } : null;
    window.addEventListener("mousemove", onGlobalPointerMove);
    window.addEventListener("mouseup", onGlobalPointerUp);
  };

  const beginResize = (e: MouseEvent, id: string, handle: ResizeHandle) => {
    e.stopPropagation();
    activeId = id;
    interactionMode = e.altKey ? "crop" : "resize";
    interactionId = id;
    interactionHandle = handle;
    interactionStartMouse = toScenePoint(e.clientX, e.clientY);
    interactionStartTransform = transforms[id] ? { ...transforms[id] } : null;
    window.addEventListener("mousemove", onGlobalPointerMove);
    window.addEventListener("mouseup", onGlobalPointerUp);
  };

  const handleGlobalKey = (e: KeyboardEvent) => {
    if (!activeId || !transforms[activeId]) return;
    if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(e.key)) return;

    const { width, height } = getSceneSize();
    const step = e.ctrlKey || e.metaKey ? 1 : e.shiftKey ? 20 : 5;
    const current = transforms[activeId];
    let x = current.x;
    let y = current.y;

    if (e.key === "ArrowLeft") x = Math.max(0, current.x - step);
    if (e.key === "ArrowRight") x = Math.min(width - current.w, current.x + step);
    if (e.key === "ArrowUp") y = Math.max(0, current.y - step);
    if (e.key === "ArrowDown") y = Math.min(height - current.h, current.y + step);

    transforms[activeId] = { ...current, x, y };
    e.preventDefault();
  };

  onMount(() => {
    syncTransformsFromSources();
    updateSceneScale();

    window.addEventListener("resize", updateSceneScale);
    window.addEventListener("keydown", handleGlobalKey);

    return () => {
      window.removeEventListener("resize", updateSceneScale);
      window.removeEventListener("keydown", handleGlobalKey);
      endInteraction();
    };
  });

  $: syncTransformsFromSources();
  $: sceneResolution, updateSceneScale();
</script>

<div
  class="transform-modal-bg"
  on:click={() => onClose()}
  role="button"
  tabindex="0"
  on:keydown={(e) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      onClose();
    }
  }}
  aria-label="Close graphic planner"
></div>
<div class="transform-modal">
  <h2>Manual Scene Edit</h2>
  <div class="scene-viewport" role="presentation" on:mousedown={() => (activeId = null)}>
    <div class="scene-canvas-shell" style={`width:${getSceneSize().width * sceneScale}px;height:${getSceneSize().height * sceneScale}px;`}>
      <div
        class="scene-canvas"
        bind:this={sceneCanvasEl}
        style={`width:${getSceneSize().width}px;height:${getSceneSize().height}px;transform:scale(${sceneScale});`}
        on:mousedown|stopPropagation
        role="presentation"
      >
        {#each sources as s (s.id)}
          {#if transforms[s.id]}
            <div
              class="source-box"
              class:active={activeId === s.id}
              style={`left:${transforms[s.id].x}px;top:${transforms[s.id].y}px;width:${transforms[s.id].w}px;height:${transforms[s.id].h}px;`}
              on:mousedown={(e) => beginMove(e, s.id)}
              role="button"
              tabindex="0"
              on:keydown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  activeId = s.id;
                }
              }}
              aria-label={`Select ${s.name}`}
            >
              <div class="source-label">{s.name}</div>
              <div class="crop-overlay" style={`inset:${transforms[s.id].cropT}px ${transforms[s.id].cropR}px ${transforms[s.id].cropB}px ${transforms[s.id].cropL}px;`}></div>
              <div class="resize-handle n" on:mousedown={(e) => beginResize(e, s.id, "n")} role="presentation" tabindex="-1" aria-hidden="true"></div>
              <div class="resize-handle s" on:mousedown={(e) => beginResize(e, s.id, "s")} role="presentation" tabindex="-1" aria-hidden="true"></div>
              <div class="resize-handle e" on:mousedown={(e) => beginResize(e, s.id, "e")} role="presentation" tabindex="-1" aria-hidden="true"></div>
              <div class="resize-handle w" on:mousedown={(e) => beginResize(e, s.id, "w")} role="presentation" tabindex="-1" aria-hidden="true"></div>
              <div class="resize-handle ne" on:mousedown={(e) => beginResize(e, s.id, "ne")} role="presentation" tabindex="-1" aria-hidden="true"></div>
              <div class="resize-handle nw" on:mousedown={(e) => beginResize(e, s.id, "nw")} role="presentation" tabindex="-1" aria-hidden="true"></div>
              <div class="resize-handle se" on:mousedown={(e) => beginResize(e, s.id, "se")} role="presentation" tabindex="-1" aria-hidden="true"></div>
              <div class="resize-handle sw" on:mousedown={(e) => beginResize(e, s.id, "sw")} role="presentation" tabindex="-1" aria-hidden="true"></div>
            </div>
          {/if}
        {/each}
      </div>
    </div>
  </div>
  <div style="display: flex; gap: 1.2rem; margin-top: 1.2rem;">
    <button on:click={() => onClose()}>Close</button>
    <button on:click={() => onSave(transforms)}>Save</button>
  </div>
  <div class="hint">Przeciągaj źródła myszką. Uchwyty skalują źródło, a <b>Alt + uchwyt</b> działa jak crop. Strzałki przesuwają zaznaczony element (Ctrl/Cmd = 1px, Shift = 20px).</div>
</div>

<style>
  .transform-modal-bg {
    position: fixed;
    inset: 0;
    background: #000a;
    z-index: 3000;
  }
  .transform-modal {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    background: var(--surface-2);
    color: var(--text);
    border-radius: 16px;
    box-shadow: 0 4px 32px #000a;
    z-index: 3010;
    padding: 2.5rem 2.5rem 2rem;
    min-width: 520px;
    min-height: 320px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1.2rem;
  }
  .scene-viewport {
    width: 100%;
    display: grid;
    place-items: center;
  }
  .scene-canvas-shell {
    position: relative;
    border: 2px solid var(--border);
    border-radius: 10px;
    overflow: hidden;
    background: #000c;
  }
  .scene-canvas {
    position: relative;
    transform-origin: top left;
    margin-bottom: 0;
  }
  .source-box {
    position: absolute;
    border: 2px solid var(--accent);
    background: #5b7cfa22;
    border-radius: 8px;
    cursor: move;
    box-sizing: border-box;
    user-select: none;
  }
  .source-box.active {
    border-color: var(--warning);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--warning) 45%, transparent);
    background: color-mix(in srgb, var(--warning) 15%, #5b7cfa22);
  }
  .source-label {
    position: absolute;
    left: 0; top: 0;
    background: var(--surface);
    color: var(--text);
    font-size: 0.9rem;
    padding: 2px 8px;
    border-radius: 0 0 8px 0;
    pointer-events: none;
  }
  .crop-overlay {
    position: absolute;
    border: 1px dashed color-mix(in srgb, var(--warning) 70%, transparent);
    background: color-mix(in srgb, var(--warning) 12%, transparent);
    pointer-events: none;
    z-index: 1;
  }
  .resize-handle {
    position: absolute;
    width: 14px; height: 14px;
    background: var(--accent);
    opacity: 0.7;
    z-index: 2;
    border-radius: 50%;
    cursor: pointer;
  }
  .resize-handle.n { top: -7px; left: 50%; transform: translateX(-50%); cursor: ns-resize; }
  .resize-handle.s { bottom: -7px; left: 50%; transform: translateX(-50%); cursor: ns-resize; }
  .resize-handle.e { right: -7px; top: 50%; transform: translateY(-50%); cursor: ew-resize; }
  .resize-handle.w { left: -7px; top: 50%; transform: translateY(-50%); cursor: ew-resize; }
  .resize-handle.ne { right: -7px; top: -7px; cursor: nesw-resize; }
  .resize-handle.nw { left: -7px; top: -7px; cursor: nwse-resize; }
  .resize-handle.se { right: -7px; bottom: -7px; cursor: nwse-resize; }
  .resize-handle.sw { left: -7px; bottom: -7px; cursor: nesw-resize; }
  .hint {
    color: var(--text-muted);
    font-size: 0.95rem;
    margin-top: 0.5rem;
    text-align: center;
  }
</style>
