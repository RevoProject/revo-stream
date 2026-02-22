<script lang="ts">
  import { onMount, tick } from "svelte";
  import type { DemoSource } from "../../types";

  export let sources: DemoSource[] = [];
  export let onClose: () => void;
  export let onSave: (transforms: Record<string, {
    x: number;
    y: number;
    w: number;
    h: number;
    rot?: number;
    cropL?: number;
    cropR?: number;
    cropT?: number;
    cropB?: number;
  }>) => void | Promise<void>;
  export let sceneResolution: string;
  export let undoRedoLimit: number = 5;

  type ResizeHandle = "n" | "s" | "e" | "w" | "ne" | "nw" | "se" | "sw";
  type InteractionMode = "idle" | "move" | "resize" | "crop" | "rotate";
  type Transform = {
    x: number;
    y: number;
    w: number;
    h: number;
    rot: number;
    cropL: number;
    cropR: number;
    cropT: number;
    cropB: number;
  };

  const MIN_SIZE = 16;
  const SNAP_THRESHOLD = 10;
  const GRID_SIZE = 20;

  const sanitizeUndoLimit = (value: number) => Math.max(1, Math.min(50, Math.round(Number(value) || 5)));

  let transforms: Record<string, Transform> = {};
  let activeId: string | null = null;
  let sceneCanvasEl: HTMLDivElement | null = null;
  let sceneViewportEl: HTMLDivElement | null = null;
  let plannerLayoutEl: HTMLDivElement | null = null;
  let propsPopupEl: HTMLDivElement | null = null;

  let interactionMode: InteractionMode = "idle";
  let interactionId: string | null = null;
  let interactionHandle: ResizeHandle | null = null;
  let interactionStartMouse = { x: 0, y: 0 };
  let interactionStartTransform: Transform | null = null;
  let interactionStartAngle = 0;

  let guideV: number | null = null;
  let guideH: number | null = null;
  let sceneScale = 1;
  let showPropertiesPopup = false;
  let showGrid = true;
  let snapToGrid = true;
  let cropMode = false;
  let activeSource: DemoSource | null = null;
  let activeTransform: Transform | null = null;
  let propsPopupPos = { left: 18, top: 18 };
  let propsPopupDragActive = false;
  let propsPopupDragStartMouse = { x: 0, y: 0 };
  let propsPopupDragStartPos = { left: 18, top: 18 };
  let propsPopupAnchorId: string | null = null;
  let undoStack: Array<Record<string, Transform>> = [];
  let redoStack: Array<Record<string, Transform>> = [];
  let saveInFlight = false;

  const parseNumber = (raw: unknown, fallback: number) => {
    const parsed = Number(String(raw ?? "").trim());
    return Number.isFinite(parsed) ? parsed : fallback;
  };

  const cloneTransforms = (state: Record<string, Transform>) => {
    const next: Record<string, Transform> = {};
    for (const [id, t] of Object.entries(state)) {
      next[id] = { ...t };
    }
    return next;
  };

  const transformsEqual = (a: Record<string, Transform>, b: Record<string, Transform>) => {
    const keysA = Object.keys(a);
    const keysB = Object.keys(b);
    if (keysA.length !== keysB.length) return false;
    for (const key of keysA) {
      const va = a[key];
      const vb = b[key];
      if (!vb) return false;
      if (
        va.x !== vb.x ||
        va.y !== vb.y ||
        va.w !== vb.w ||
        va.h !== vb.h ||
        va.rot !== vb.rot ||
        va.cropL !== vb.cropL ||
        va.cropR !== vb.cropR ||
        va.cropT !== vb.cropT ||
        va.cropB !== vb.cropB
      ) return false;
    }
    return true;
  };

  const trimUndoStack = () => {
    const limit = sanitizeUndoLimit(undoRedoLimit);
    if (undoStack.length > limit) {
      undoStack = undoStack.slice(undoStack.length - limit);
    }
  };

  const pushHistoryCheckpoint = () => {
    const snapshot = cloneTransforms(transforms);
    const last = undoStack[undoStack.length - 1] ?? null;
    if (last && transformsEqual(last, snapshot)) return;
    undoStack = [...undoStack, snapshot];
    trimUndoStack();
    redoStack = [];
  };

  const undoTransform = () => {
    const prev = undoStack[undoStack.length - 1];
    if (!prev) return;
    const current = cloneTransforms(transforms);
    undoStack = undoStack.slice(0, -1);
    redoStack = [...redoStack, current];
    transforms = cloneTransforms(prev);
    if (activeId && !transforms[activeId]) activeId = null;
  };

  const redoTransform = () => {
    const next = redoStack[redoStack.length - 1];
    if (!next) return;
    const current = cloneTransforms(transforms);
    redoStack = redoStack.slice(0, -1);
    undoStack = [...undoStack, current];
    trimUndoStack();
    transforms = cloneTransforms(next);
    if (activeId && !transforms[activeId]) activeId = null;
  };

  const normalizeAngle = (angle: number) => {
    let next = angle % 360;
    if (next < 0) next += 360;
    return next;
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
      ...transform,
      x,
      y,
      w,
      h,
      rot: normalizeAngle(transform.rot),
      cropL: Math.max(0, Math.min(w - 1, transform.cropL)),
      cropR: Math.max(0, Math.min(w - 1, transform.cropR)),
      cropT: Math.max(0, Math.min(h - 1, transform.cropT)),
      cropB: Math.max(0, Math.min(h - 1, transform.cropB))
    };
  };

  const buildInitialTransform = (source: DemoSource): Transform => {
    const params = source.params ?? {};
    return clampTransform({
      x: parseNumber(params.pos_x, 100),
      y: parseNumber(params.pos_y, 100),
      w: parseNumber(params.item_width, parseNumber(params.width, 320)),
      h: parseNumber(params.item_height, parseNumber(params.height, 180)),
      rot: parseNumber(params.rot, parseNumber(params.rotation, 0)),
      cropL: parseNumber(params.crop_left, 0),
      cropR: parseNumber(params.crop_right, 0),
      cropT: parseNumber(params.crop_top, 0),
      cropB: parseNumber(params.crop_bottom, 0)
    });
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
    for (const id of Object.keys(next)) {
      if (!sources.some((s) => s.id === id)) {
        delete next[id];
        changed = true;
      }
    }
    if (changed) {
      transforms = next;
      undoStack = [];
      redoStack = [];
    }
    if (activeId && !next[activeId]) activeId = null;
  };

  const updateSceneScale = () => {
    const { width, height } = getSceneSize();
    const viewportRect = sceneViewportEl?.getBoundingClientRect();
    const maxW = Math.max(260, (viewportRect?.width ?? window.innerWidth) - 24);
    const maxH = Math.max(220, (viewportRect?.height ?? window.innerHeight) - 24);
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

  const snapValue = (value: number, targets: number[]) => {
    for (const t of targets) {
      if (Math.abs(value - t) <= SNAP_THRESHOLD) return t;
    }
    return value;
  };

  const getAlignmentTargets = (excludeId: string) => {
    const { width, height } = getSceneSize();
    const xTargets = [0, width / 2, width];
    const yTargets = [0, height / 2, height];
    for (const [id, t] of Object.entries(transforms)) {
      if (id === excludeId) continue;
      xTargets.push(t.x, t.x + t.w / 2, t.x + t.w);
      yTargets.push(t.y, t.y + t.h / 2, t.y + t.h);
    }
    return { xTargets, yTargets };
  };

  const findClosestTarget = (value: number, targets: number[]) => {
    let best = value;
    let bestDist = Number.POSITIVE_INFINITY;
    for (const t of targets) {
      const d = Math.abs(value - t);
      if (d <= SNAP_THRESHOLD && d < bestDist) {
        best = t;
        bestDist = d;
      }
    }
    return best;
  };

  const applyMove = (id: string, point: { x: number; y: number }) => {
    const start = interactionStartTransform;
    if (!start) return;
    const { width, height } = getSceneSize();
    const dx = point.x - interactionStartMouse.x;
    const dy = point.y - interactionStartMouse.y;

    let x = Math.max(0, Math.min(width - start.w, start.x + dx));
    let y = Math.max(0, Math.min(height - start.h, start.y + dy));

    const { xTargets, yTargets } = getAlignmentTargets(id);
    const leftSnap = findClosestTarget(x, xTargets);
    const centerSnap = findClosestTarget(x + start.w / 2, xTargets) - start.w / 2;
    const rightSnap = findClosestTarget(x + start.w, xTargets) - start.w;

    const topSnap = findClosestTarget(y, yTargets);
    const middleSnap = findClosestTarget(y + start.h / 2, yTargets) - start.h / 2;
    const bottomSnap = findClosestTarget(y + start.h, yTargets) - start.h;

    const xCandidates = [leftSnap, centerSnap, rightSnap];
    const yCandidates = [topSnap, middleSnap, bottomSnap];
    const snappedX = xCandidates.reduce((best, next) =>
      Math.abs(next - x) < Math.abs(best - x) ? next : best
    , x);
    const snappedY = yCandidates.reduce((best, next) =>
      Math.abs(next - y) < Math.abs(best - y) ? next : best
    , y);

    const clampedX = Math.max(0, Math.min(width - start.w, snappedX));
    const clampedY = Math.max(0, Math.min(height - start.h, snappedY));

    const xGuideCandidate = [clampedX, clampedX + start.w / 2, clampedX + start.w]
      .find((v) => xTargets.some((t) => Math.abs(t - v) <= 0.01)) ?? null;
    const yGuideCandidate = [clampedY, clampedY + start.h / 2, clampedY + start.h]
      .find((v) => yTargets.some((t) => Math.abs(t - v) <= 0.01)) ?? null;

    let finalX = clampedX;
    let finalY = clampedY;
    if (snapToGrid) {
      finalX = snapValue(finalX, [Math.round(finalX / GRID_SIZE) * GRID_SIZE]);
      finalY = snapValue(finalY, [Math.round(finalY / GRID_SIZE) * GRID_SIZE]);
    }

    guideV = xGuideCandidate;
    guideH = yGuideCandidate;

    transforms[id] = { ...start, x: finalX, y: finalY };
  };

  const applyResizeOrCrop = (id: string, point: { x: number; y: number }) => {
    const start = interactionStartTransform;
    const handle = interactionHandle;
    if (!start || !handle) return;

    const dx = point.x - interactionStartMouse.x;
    const dy = point.y - interactionStartMouse.y;

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

    const { width: sceneW, height: sceneH } = getSceneSize();
    let x = start.x;
    let y = start.y;
    let w = start.w;
    let h = start.h;

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

    const { xTargets, yTargets } = getAlignmentTargets(id);
    const right = x + w;
    const bottom = y + h;

    if (handle.includes("e")) {
      const snappedRight = findClosestTarget(right, xTargets);
      w += snappedRight - right;
      guideV = Math.abs(snappedRight - right) <= SNAP_THRESHOLD ? snappedRight : null;
    } else if (handle.includes("w")) {
      const snappedLeft = findClosestTarget(x, xTargets);
      const delta = snappedLeft - x;
      x += delta;
      w -= delta;
      guideV = Math.abs(snappedLeft - x) <= SNAP_THRESHOLD ? snappedLeft : null;
    } else {
      guideV = null;
    }

    if (handle.includes("s")) {
      const snappedBottom = findClosestTarget(bottom, yTargets);
      h += snappedBottom - bottom;
      guideH = Math.abs(snappedBottom - bottom) <= SNAP_THRESHOLD ? snappedBottom : null;
    } else if (handle.includes("n")) {
      const snappedTop = findClosestTarget(y, yTargets);
      const delta = snappedTop - y;
      y += delta;
      h -= delta;
      guideH = Math.abs(snappedTop - y) <= SNAP_THRESHOLD ? snappedTop : null;
    } else {
      guideH = null;
    }

    if (snapToGrid) {
      if (handle.includes("e")) w = Math.max(MIN_SIZE, Math.round((x + w) / GRID_SIZE) * GRID_SIZE - x);
      if (handle.includes("s")) h = Math.max(MIN_SIZE, Math.round((y + h) / GRID_SIZE) * GRID_SIZE - y);
      if (handle.includes("w")) {
        const snappedLeft = Math.round(x / GRID_SIZE) * GRID_SIZE;
        const delta = snappedLeft - x;
        x += delta;
        w -= delta;
      }
      if (handle.includes("n")) {
        const snappedTop = Math.round(y / GRID_SIZE) * GRID_SIZE;
        const delta = snappedTop - y;
        y += delta;
        h -= delta;
      }
    }

    transforms[id] = clampTransform({ ...start, x, y, w, h });
  };

  const applyRotate = (id: string, point: { x: number; y: number }) => {
    const start = interactionStartTransform;
    if (!start) return;
    const center = { x: start.x + start.w / 2, y: start.y + start.h / 2 };
    const angle = Math.atan2(point.y - center.y, point.x - center.x) * (180 / Math.PI);
    const snapped = Math.round((angle - interactionStartAngle + start.rot) / 5) * 5;
    transforms[id] = { ...start, rot: normalizeAngle(snapped) };
  };

  const endInteraction = () => {
    interactionMode = "idle";
    interactionId = null;
    interactionHandle = null;
    interactionStartTransform = null;
    guideV = null;
    guideH = null;
    window.removeEventListener("mousemove", onGlobalPointerMove);
    window.removeEventListener("mouseup", onGlobalPointerUp);
  };

  const onGlobalPointerMove = (e: MouseEvent) => {
    if (!interactionId) return;
    const point = toScenePoint(e.clientX, e.clientY);
    if (interactionMode === "move") return applyMove(interactionId, point);
    if (interactionMode === "resize" || interactionMode === "crop") return applyResizeOrCrop(interactionId, point);
    if (interactionMode === "rotate") return applyRotate(interactionId, point);
  };

  const onGlobalPointerUp = () => endInteraction();

  const beginMove = (e: MouseEvent, id: string) => {
    e.stopPropagation();
    activeId = id;
    interactionMode = "move";
    interactionId = id;
    interactionStartMouse = toScenePoint(e.clientX, e.clientY);
    interactionStartTransform = transforms[id] ? { ...transforms[id] } : null;
    if (interactionStartTransform) pushHistoryCheckpoint();
    window.addEventListener("mousemove", onGlobalPointerMove);
    window.addEventListener("mouseup", onGlobalPointerUp);
  };

  const beginResize = (e: MouseEvent, id: string, handle: ResizeHandle) => {
    e.stopPropagation();
    activeId = id;
    interactionMode = (cropMode || e.altKey) ? "crop" : "resize";
    interactionId = id;
    interactionHandle = handle;
    interactionStartMouse = toScenePoint(e.clientX, e.clientY);
    interactionStartTransform = transforms[id] ? { ...transforms[id] } : null;
    if (interactionStartTransform) pushHistoryCheckpoint();
    window.addEventListener("mousemove", onGlobalPointerMove);
    window.addEventListener("mouseup", onGlobalPointerUp);
  };

  const beginRotate = (e: MouseEvent, id: string) => {
    e.stopPropagation();
    activeId = id;
    interactionMode = "rotate";
    interactionId = id;
    interactionStartTransform = transforms[id] ? { ...transforms[id] } : null;
    const start = interactionStartTransform;
    if (!start) return;
    pushHistoryCheckpoint();
    const p = toScenePoint(e.clientX, e.clientY);
    const center = { x: start.x + start.w / 2, y: start.y + start.h / 2 };
    interactionStartAngle = Math.atan2(p.y - center.y, p.x - center.x) * (180 / Math.PI);
    window.addEventListener("mousemove", onGlobalPointerMove);
    window.addEventListener("mouseup", onGlobalPointerUp);
  };

  const patchActive = (patch: Partial<Transform>) => {
    if (!activeId || !transforms[activeId]) return;
    const current = transforms[activeId];
    const next = clampTransform({ ...current, ...patch });
    if (transformsEqual({ [activeId]: current }, { [activeId]: next })) return;
    pushHistoryCheckpoint();
    transforms[activeId] = next;
  };

  const handleGlobalKey = (e: KeyboardEvent) => {
    const target = e.target as HTMLElement | null;
    const tag = target?.tagName?.toLowerCase() ?? "";
    const typingTarget = Boolean(target?.isContentEditable) || tag === "input" || tag === "textarea" || tag === "select";

    if (e.key === "Escape") {
      if (typingTarget) return;
      if (showPropertiesPopup) {
        showPropertiesPopup = false;
        return;
      }
      onClose();
      return;
    }
    if (typingTarget) return;
    if (!activeId || !transforms[activeId]) return;
    if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(e.key)) return;
    const { width, height } = getSceneSize();
    const step = e.ctrlKey || e.metaKey ? 1 : e.shiftKey ? 20 : 5;
    const t = transforms[activeId];
    let x = t.x;
    let y = t.y;
    if (e.key === "ArrowLeft") x = Math.max(0, t.x - step);
    if (e.key === "ArrowRight") x = Math.min(width - t.w, t.x + step);
    if (e.key === "ArrowUp") y = Math.max(0, t.y - step);
    if (e.key === "ArrowDown") y = Math.min(height - t.h, t.y + step);
    transforms[activeId] = { ...t, x, y };
    e.preventDefault();
  };

  const saveChanges = async () => {
    if (saveInFlight) return;
    saveInFlight = true;
    const payload: Record<string, {
      x: number;
      y: number;
      w: number;
      h: number;
      rot: number;
      cropL: number;
      cropR: number;
      cropT: number;
      cropB: number;
    }> = {};
    for (const [id, t] of Object.entries(transforms)) {
      payload[id] = {
        x: Math.round(t.x),
        y: Math.round(t.y),
        w: Math.round(t.w),
        h: Math.round(t.h),
        rot: Math.round(t.rot),
        cropL: Math.round(t.cropL),
        cropR: Math.round(t.cropR),
        cropT: Math.round(t.cropT),
        cropB: Math.round(t.cropB)
      };
    }
    try {
      await onSave(payload);
    } finally {
      saveInFlight = false;
    }
  };

  const fitToScreen = () => {
    updateSceneScale();
  };

  const centerActive = () => {
    if (!activeId || !transforms[activeId]) return;
    const { width, height } = getSceneSize();
    const t = transforms[activeId];
    patchActive({
      x: Math.max(0, (width - t.w) / 2),
      y: Math.max(0, (height - t.h) / 2)
    });
  };

  const resetActiveTransform = () => {
    if (!activeId) return;
    const source = sources.find((s) => s.id === activeId);
    if (!source) return;
    pushHistoryCheckpoint();
    transforms[activeId] = buildInitialTransform(source);
  };

  const clampPropsPopupPos = (left: number, top: number) => {
    const host = plannerLayoutEl;
    if (!host) return { left, top };
    const hostRect = host.getBoundingClientRect();
    const popupRect = propsPopupEl?.getBoundingClientRect();
    const popupW = Math.min(popupRect?.width ?? 560, Math.max(320, hostRect.width - 16));
    const popupH = Math.min(popupRect?.height ?? 260, Math.max(160, hostRect.height - 16));

    const minLeft = 8;
    const maxLeft = Math.max(minLeft, hostRect.width - popupW - 8);
    const minTop = Math.max(8, hostRect.height * 0.15);
    const maxTop = Math.max(minTop, hostRect.height - popupH - 8);

    return {
      left: Math.max(minLeft, Math.min(maxLeft, left)),
      top: Math.max(minTop, Math.min(maxTop, top))
    };
  };

  const placePropsPopupNearActive = () => {
    if (!showPropertiesPopup || !activeId || !activeTransform || !plannerLayoutEl || !sceneCanvasEl) return;
    const hostRect = plannerLayoutEl.getBoundingClientRect();
    const canvasRect = sceneCanvasEl.getBoundingClientRect();
    const popupRect = propsPopupEl?.getBoundingClientRect();
    const popupW = popupRect?.width ?? 560;
    const popupH = popupRect?.height ?? 260;

    const sourceLeft = canvasRect.left - hostRect.left + activeTransform.x * sceneScale;
    const sourceTop = canvasRect.top - hostRect.top + activeTransform.y * sceneScale;
    const sourceW = activeTransform.w * sceneScale;
    const sourceH = activeTransform.h * sceneScale;

    let left = sourceLeft + sourceW / 2 - popupW / 2;
    let top = sourceTop + sourceH + 12;

    if (top + popupH > hostRect.height - 8) {
      top = sourceTop - popupH - 12;
    }

    propsPopupPos = clampPropsPopupPos(left, top);
  };

  const onPropsPopupDragMove = (e: MouseEvent) => {
    if (!propsPopupDragActive) return;
    const dx = e.clientX - propsPopupDragStartMouse.x;
    const dy = e.clientY - propsPopupDragStartMouse.y;
    propsPopupPos = clampPropsPopupPos(
      propsPopupDragStartPos.left + dx,
      propsPopupDragStartPos.top + dy
    );
  };

  const endPropsPopupDrag = () => {
    propsPopupDragActive = false;
    window.removeEventListener("mousemove", onPropsPopupDragMove);
    window.removeEventListener("mouseup", endPropsPopupDrag);
  };

  const beginPropsPopupDrag = (e: MouseEvent) => {
    if (e.button !== 0) return;
    const target = e.target as HTMLElement | null;
    if (target?.closest("button, input, select, textarea")) return;
    e.stopPropagation();
    propsPopupDragActive = true;
    propsPopupDragStartMouse = { x: e.clientX, y: e.clientY };
    propsPopupDragStartPos = { ...propsPopupPos };
    window.addEventListener("mousemove", onPropsPopupDragMove);
    window.addEventListener("mouseup", endPropsPopupDrag);
  };

  onMount(() => {
    syncTransformsFromSources();
    requestAnimationFrame(updateSceneScale);
    window.addEventListener("resize", updateSceneScale);
    window.addEventListener("keydown", handleGlobalKey);
    return () => {
      window.removeEventListener("resize", updateSceneScale);
      window.removeEventListener("keydown", handleGlobalKey);
      endPropsPopupDrag();
      endInteraction();
    };
  });

  $: syncTransformsFromSources();
  $: sceneResolution, requestAnimationFrame(updateSceneScale);
  $: showPropertiesPopup, tick().then(() => updateSceneScale());
  $: if (showPropertiesPopup && activeId && activeTransform) {
    tick().then(() => {
      if (!showPropertiesPopup || !activeId) return;
      if (propsPopupAnchorId !== activeId) {
        placePropsPopupNearActive();
        propsPopupAnchorId = activeId;
      } else {
        propsPopupPos = clampPropsPopupPos(propsPopupPos.left, propsPopupPos.top);
      }
    });
  }
  $: undoRedoLimit, trimUndoStack();
  $: activeSource = activeId ? sources.find((s) => s.id === activeId) ?? null : null;
  $: activeTransform = activeId ? transforms[activeId] ?? null : null;
  $: if (!activeId) {
    showPropertiesPopup = false;
    propsPopupAnchorId = null;
  }
</script>

<div class="transform-modal">
  <div class="quick-toolbar">
    <button class="tool-btn" on:click={fitToScreen}>Fit to screen</button>
    <button class="tool-btn" on:click={centerActive} disabled={!activeId}>Center</button>
    <button class="tool-btn" on:click={resetActiveTransform} disabled={!activeId}>Reset transform</button>
    <button class="tool-btn" on:click={() => (showGrid = !showGrid)} class:active={showGrid}>Grid</button>
    <button class="tool-btn" on:click={() => (snapToGrid = !snapToGrid)} class:active={snapToGrid}>Snap grid</button>
    <button class="tool-btn" on:click={() => (cropMode = !cropMode)} class:active={cropMode}>Crop mode</button>
    <button class="tool-btn" on:click={() => (showPropertiesPopup = !showPropertiesPopup)} disabled={!activeId} class:active={showPropertiesPopup}>Properties</button>
    <button class="tool-btn" on:click={undoTransform} disabled={undoStack.length === 0}>Undo</button>
    <button class="tool-btn" on:click={redoTransform} disabled={redoStack.length === 0}>Redo</button>
    <div class="toolbar-spacer"></div>
    <button class="tool-btn primary" on:click={saveChanges} disabled={saveInFlight}>{saveInFlight ? "Saving..." : "Save"}</button>
  </div>
  <div class="planner-layout" bind:this={plannerLayoutEl}>
    <div class="scene-viewport" bind:this={sceneViewportEl} role="presentation" on:mousedown={() => (activeId = null)}>
      <div class="scene-canvas-shell" style={`width:${getSceneSize().width * sceneScale}px;height:${getSceneSize().height * sceneScale}px;`}>
        <div class="scene-canvas" class:show-grid={showGrid} bind:this={sceneCanvasEl} style={`width:${getSceneSize().width}px;height:${getSceneSize().height}px;transform:scale(${sceneScale});`} on:mousedown|stopPropagation role="presentation">
          {#if guideV !== null}
            <div class="snap-guide-v" style={`left:${guideV}px;`}></div>
          {/if}
          {#if guideH !== null}
            <div class="snap-guide-h" style={`top:${guideH}px;`}></div>
          {/if}

          {#each sources as s (s.id)}
            {#if transforms[s.id]}
              {@const t = transforms[s.id]}
              <div class="source-box" class:active={activeId === s.id} style={`left:${t.x}px;top:${t.y}px;width:${t.w}px;height:${t.h}px;transform:rotate(${t.rot}deg);`} on:mousedown={(e) => beginMove(e, s.id)} role="button" tabindex="0" aria-label={`Select ${s.name}`}>
                <div class="source-label">{s.name}</div>
                <div class="crop-overlay" style={`inset:${t.cropT}px ${t.cropR}px ${t.cropB}px ${t.cropL}px;`}></div>
                <div class="rotate-link"></div>
                <div class="rotate-handle" on:mousedown={(e) => beginRotate(e, s.id)} role="presentation" aria-hidden="true"></div>
                <div class="resize-handle n" on:mousedown={(e) => beginResize(e, s.id, "n")} role="presentation" aria-hidden="true"></div>
                <div class="resize-handle s" on:mousedown={(e) => beginResize(e, s.id, "s")} role="presentation" aria-hidden="true"></div>
                <div class="resize-handle e" on:mousedown={(e) => beginResize(e, s.id, "e")} role="presentation" aria-hidden="true"></div>
                <div class="resize-handle w" on:mousedown={(e) => beginResize(e, s.id, "w")} role="presentation" aria-hidden="true"></div>
                <div class="resize-handle ne" on:mousedown={(e) => beginResize(e, s.id, "ne")} role="presentation" aria-hidden="true"></div>
                <div class="resize-handle nw" on:mousedown={(e) => beginResize(e, s.id, "nw")} role="presentation" aria-hidden="true"></div>
                <div class="resize-handle se" on:mousedown={(e) => beginResize(e, s.id, "se")} role="presentation" aria-hidden="true"></div>
                <div class="resize-handle sw" on:mousedown={(e) => beginResize(e, s.id, "sw")} role="presentation" aria-hidden="true"></div>
              </div>
            {/if}
          {/each}
        </div>
      </div>
    </div>
  </div>

  {#if showPropertiesPopup && activeSource && activeTransform}
    <div class="props-popup" bind:this={propsPopupEl} style={`left:${propsPopupPos.left}px;top:${propsPopupPos.top}px;`} role="dialog" tabindex="0" aria-label="Source properties" on:mousedown|stopPropagation>
      <div class="props-head" on:mousedown={beginPropsPopupDrag} role="presentation">
        <h3>Properties</h3>
        <button class="x" on:click={() => (showPropertiesPopup = false)} aria-label="Close properties">×</button>
      </div>
      <p class="props-name">{activeSource.name}</p>
      <div class="props-grid five">
        <label>X<input type="number" value={Math.round(activeTransform.x)} on:input={(e) => patchActive({ x: Number((e.currentTarget as HTMLInputElement).value) })} /></label>
        <label>Y<input type="number" value={Math.round(activeTransform.y)} on:input={(e) => patchActive({ y: Number((e.currentTarget as HTMLInputElement).value) })} /></label>
        <label>W<input type="number" min={MIN_SIZE} value={Math.round(activeTransform.w)} on:input={(e) => patchActive({ w: Number((e.currentTarget as HTMLInputElement).value) })} /></label>
        <label>H<input type="number" min={MIN_SIZE} value={Math.round(activeTransform.h)} on:input={(e) => patchActive({ h: Number((e.currentTarget as HTMLInputElement).value) })} /></label>
        <label>Rot<input type="number" value={Math.round(activeTransform.rot)} on:input={(e) => patchActive({ rot: Number((e.currentTarget as HTMLInputElement).value) })} /></label>
      </div>
      <div class="props-grid four">
        <label>Crop L<input type="number" min="0" value={Math.round(activeTransform.cropL)} on:input={(e) => patchActive({ cropL: Number((e.currentTarget as HTMLInputElement).value) })} /></label>
        <label>Crop R<input type="number" min="0" value={Math.round(activeTransform.cropR)} on:input={(e) => patchActive({ cropR: Number((e.currentTarget as HTMLInputElement).value) })} /></label>
        <label>Crop T<input type="number" min="0" value={Math.round(activeTransform.cropT)} on:input={(e) => patchActive({ cropT: Number((e.currentTarget as HTMLInputElement).value) })} /></label>
        <label>Crop B<input type="number" min="0" value={Math.round(activeTransform.cropB)} on:input={(e) => patchActive({ cropB: Number((e.currentTarget as HTMLInputElement).value) })} /></label>
      </div>
    </div>
  {/if}
  <div class="hint">Drag to move, handles to resize, <b>Crop mode</b> or <b>Alt+handle</b> for crop, rotation handle above box, arrow keys nudge selected source, snap to scene/sources/grid.</div>
</div>

<style>
  .transform-modal {
    position: fixed;
    inset: 0;
    transform: none;
    background: var(--surface-2);
    color: var(--text);
    border-radius: 0;
    box-shadow: none;
    z-index: 3010;
    padding: 0.9rem;
    width: auto;
    height: auto;
    min-width: 0;
    min-height: 0;
    max-height: 92vh;
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 1.2rem;
    overflow: hidden;
  }
  .planner-layout {
    position: relative;
    flex: 1;
    min-height: 0;
  }
  .quick-toolbar {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    align-items: center;
    padding: 0.2rem;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--surface);
  }
  .toolbar-spacer {
    flex: 1;
  }
  .tool-btn {
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.5rem 0.9rem;
    background: var(--surface-2);
    color: var(--text);
    font-size: 0.9rem;
    font-weight: 600;
    cursor: pointer;
    transition: background 120ms ease, border-color 120ms ease, transform 120ms ease;
  }
  .tool-btn:hover:not(:disabled) {
    background: color-mix(in srgb, var(--surface-2) 70%, var(--surface));
    border-color: color-mix(in srgb, var(--border) 60%, var(--text-muted));
  }
  .tool-btn:active:not(:disabled) {
    transform: translateY(1px);
  }
  .tool-btn:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }
  .tool-btn.active {
    background: color-mix(in srgb, var(--accent) 30%, var(--surface-2));
    border-color: color-mix(in srgb, var(--accent) 45%, var(--border));
  }
  .tool-btn.primary {
    background: var(--accent);
    color: #fff;
    border-color: color-mix(in srgb, var(--accent) 60%, #000 40%);
  }
  .tool-btn.primary:hover:not(:disabled) {
    background: color-mix(in srgb, var(--accent) 86%, #fff 14%);
  }
  .scene-viewport {
    width: 100%;
    display: grid;
    place-items: center;
    height: 100%;
    min-height: 0;
  }
  .scene-canvas-shell {
    position: relative;
    border: 2px solid var(--border);
    border-radius: 10px;
    overflow: hidden;
    background: #222c;
    max-width: 100%;
    max-height: 100%;
  }
  .scene-canvas {
    position: relative;
    transform-origin: top left;
    margin-bottom: 0;
  }
  .scene-canvas.show-grid {
    background-image:
      linear-gradient(to right, color-mix(in srgb, var(--border) 55%, transparent) 1px, transparent 1px),
      linear-gradient(to bottom, color-mix(in srgb, var(--border) 55%, transparent) 1px, transparent 1px);
    background-size: 20px 20px;
    background-position: 0 0;
  }
  .snap-guide-v,
  .snap-guide-h {
    position: absolute;
    pointer-events: none;
    z-index: 1;
    opacity: 0.85;
    background: color-mix(in srgb, var(--accent) 85%, #fff 15%);
  }
  .snap-guide-v { top: 0; bottom: 0; width: 2px; }
  .snap-guide-h { left: 0; right: 0; height: 2px; }
  .source-box {
    position: absolute;
    border: 2px solid var(--accent);
    background: #5b7cfa22;
    border-radius: 8px;
    cursor: move;
    box-sizing: border-box;
    user-select: none;
    transform-origin: center center;
  }
  .source-box.active {
    border-color: var(--warning);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--warning) 45%, transparent);
    background: color-mix(in srgb, var(--warning) 15%, #5b7cfa22);
  }
  .source-label {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    background: color-mix(in srgb, #000 35%, var(--surface));
    color: var(--text);
    font-size: 28px;
    font-weight: 700;
    line-height: 1;
    padding: 8px 14px;
    border-radius: 10px;
    pointer-events: none;
    z-index: 4;
    text-shadow: 0 1px 2px #0008;
    max-width: 92%;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .crop-overlay {
    position: absolute;
    border: 1px dashed color-mix(in srgb, var(--danger) 75%, transparent);
    background: color-mix(in srgb, var(--danger) 16%, transparent);
    pointer-events: none;
    z-index: 2;
  }
  .rotate-link {
    position: absolute;
    top: -24px;
    left: 50%;
    width: 2px;
    height: 16px;
    transform: translateX(-50%);
    background: color-mix(in srgb, var(--accent) 70%, #fff 30%);
  }
  .rotate-handle {
    position: absolute;
    top: -34px;
    left: 50%;
    width: 14px;
    height: 14px;
    transform: translateX(-50%);
    border-radius: 999px;
    background: var(--warning);
    border: 1px solid color-mix(in srgb, var(--warning) 80%, #000 20%);
    cursor: grab;
    z-index: 3;
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
  .props-popup {
    position: absolute;
    width: min(880px, calc(100% - 1rem));
    border: 1px solid var(--border);
    background: var(--surface);
    border-radius: 12px;
    padding: 0.9rem;
    display: grid;
    gap: 0.6rem;
    z-index: 20;
    box-shadow: 0 8px 24px #0008;
  }
  .props-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    cursor: grab;
    user-select: none;
  }
  .props-head:active { cursor: grabbing; }
  .props-popup h3 {
    margin: 0;
  }
  .props-head .x {
    width: 34px;
    min-width: 34px;
    height: 34px;
    padding: 0;
    border-radius: 8px;
    font-size: 1rem;
    line-height: 1;
    display: grid;
    place-items: center;
    font-weight: 700;
  }
  .props-name {
    margin: 0;
    color: var(--text-muted);
  }
  .props-grid {
    display: grid;
    gap: 0.45rem;
  }
  .props-grid.five { grid-template-columns: repeat(5, minmax(0, 1fr)); }
  .props-grid.four { grid-template-columns: repeat(4, minmax(0, 1fr)); }
  .props-grid label {
    display: grid;
    gap: 0.25rem;
    font-size: 0.82rem;
    color: var(--text-muted);
  }
  .props-grid input {
    width: 100%;
    box-sizing: border-box;
    background: var(--surface-2);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.35rem 0.45rem;
  }
  @media (max-width: 1100px) {
    .transform-modal {
      inset: 0;
      padding: 0.85rem;
      gap: 0.8rem;
    }
    .toolbar-spacer {
      display: none;
    }
    .props-popup {
      position: static;
      width: 100%;
    }
    .props-grid.five { grid-template-columns: repeat(3, minmax(0, 1fr)); }
    .props-grid.four { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  }
</style>
