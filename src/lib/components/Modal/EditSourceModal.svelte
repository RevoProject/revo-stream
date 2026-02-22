<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import type { DemoSource } from "../../types";

  export let editSource: DemoSource | null;
  export let editName: string;
  export let editType: string;
  export let editParams: Record<string, string>;
  export let audioInputDevices: { id: string; name: string }[] = [];
  export let audioOutputDevices: { id: string; name: string }[] = [];
  export let sourceParamSchemas: Record<string, { key: string; label: string; placeholder: string }[]>;
  export let sourcePropertySpecs: { key: string; label: string; kind: string; hint?: string; options?: { value: string; label: string }[] }[] = [];
  export let sourcePropertyEntries: [string, string][] = [];
  export let extraParamEntries: [string, string][];
  export let fontOptions: string[];
  export let windowMode = false;
  export let allowDraggablePopups = false;

  const dispatch = createEventDispatcher();
  let newParamKey = "";
  let newParamValue = "";
  let modalEl: HTMLDivElement | null = null;
  let dragActive = false;
  let dragStartX = 0;
  let dragStartY = 0;
  let dragOriginX = 0;
  let dragOriginY = 0;
  let dragX = 0;
  let dragY = 0;

  const normalizeParamKey = (key: string) => key.toLowerCase().replace(/[^a-z0-9]+/g, "").trim();
  const managedTextKeys = new Set(
    [
      "text",
      "color",
      "color1",
      "from_file",
      "read_from_file",
      "is_from_file",
      "text_file",
      "file",
      "font",
      "face",
      "size",
      "font_face",
      "font_size",
      "autoresize",
      "autoresize_font_width",
      "auto_resize",
      "auto_resize_font_width",
      "extents"
    ].map((k) => normalizeParamKey(k))
  );
  const isManagedTextKey = (key: string) => managedTextKeys.has(normalizeParamKey(key));
  const managedMediaKeys = new Set(
    ["local_file", "file", "input", "url", "is_local_file"].map((k) => normalizeParamKey(k))
  );
  const isManagedMediaKey = (key: string) => managedMediaKeys.has(normalizeParamKey(key));
  const managedAudioDeviceKeys = new Set(["device", "device_id"].map((k) => normalizeParamKey(k)));
  const isManagedAudioDeviceKey = (key: string) => managedAudioDeviceKeys.has(normalizeParamKey(key));
  const protectedTransformKeys = new Set(["posx", "posy", "itemwidth", "itemheight"]);
  const isProtectedTransformKey = (key: string) => protectedTransformKeys.has(normalizeParamKey(key));

  const close = () => dispatch("close");
  const save = () => dispatch("save");

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

  function normalizeHexColor(value: string) {
    let cleaned = value.trim().toLowerCase();
    if (cleaned.startsWith("0x")) {
      cleaned = cleaned.slice(2);
    }
    if (cleaned.startsWith("#")) cleaned = cleaned.slice(1);
    cleaned = cleaned.replace(/[^0-9a-f]/g, "");
    if (cleaned.length === 8) {
      const b = cleaned.slice(2, 4);
      const g = cleaned.slice(4, 6);
      const r = cleaned.slice(6, 8);
      return `#${r}${g}${b}`;
    }
    if (cleaned.length > 6) cleaned = cleaned.slice(cleaned.length - 6);
    return `#${cleaned.padStart(6, "0")}`;
  }

  const normalizeKey = (key: string) => key.toLowerCase().replace(/[^a-z0-9]+/g, "").trim();

  const getExistingParamKey = (candidates: string[]) => {
    const wanted = new Set(candidates.map((k) => normalizeParamKey(k)));

    const existing = Object.keys(editParams).find((key) => wanted.has(normalizeParamKey(key)));
    if (existing) return existing;

    const fromSpec = sourcePropertySpecs.find((spec) => wanted.has(normalizeParamKey(spec.key)))?.key;
    if (fromSpec) return fromSpec;

    return candidates[0] ?? "";
  };

  const sliderSpecs: Record<string, { min: number; max: number; step: number; fallback: number }> = {
    brightness: { min: -1, max: 1, step: 0.01, fallback: 0 },
    contrast: { min: 0, max: 4, step: 0.01, fallback: 1 },
    constrast: { min: 0, max: 4, step: 0.01, fallback: 1 },
    gamma: { min: 0, max: 3, step: 0.01, fallback: 1 },
    hueshift: { min: -180, max: 180, step: 1, fallback: 0 },
    saturation: { min: 0, max: 3, step: 0.01, fallback: 1 }
  };

  const getSliderSpec = (key: string) => sliderSpecs[normalizeKey(key)];
  const isSliderKey = (key: string) => Boolean(getSliderSpec(key));

  const isColorKey = (key: string, kind?: string) => {
    if (kind === "color") return true;
    return normalizeKey(key).includes("color");
  };

  const isListLikeKey = (key: string, kind?: string) => {
    if (kind === "editable_list") return true;
    const normalized = normalizeKey(key);
    return (
      normalized.includes("imagefiles") ||
      normalized.includes("files") ||
      normalized.includes("list") ||
      normalized.includes("paths") ||
      normalized.includes("urls")
    );
  };

  const normalizeListValue = (value: string) =>
    (value ?? "")
      .split("\n")
      .map((line) => line.trim())
      .filter((line) => line.length > 0)
      .join("\n");

  const isListValue = (key: string, value: string, kind?: string) =>
    isListLikeKey(key, kind) || (value ?? "").includes("\n");

  const abgrIntToHex = (value: number) => {
    const int = value >>> 0;
    const r = int & 0xff;
    const g = (int >>> 8) & 0xff;
    const b = (int >>> 16) & 0xff;
    return `#${r.toString(16).padStart(2, "0")}${g.toString(16).padStart(2, "0")}${b
      .toString(16)
      .padStart(2, "0")}`;
  };

  function formatColorValue(key: string, value: string, kind?: string) {
    if (!isColorKey(key, kind)) return value;
    const trimmed = (value ?? "").trim();
    if (!trimmed) return "#000000";
    if (/^#?[0-9a-fA-F]{6}$/.test(trimmed) || /^0x[0-9a-fA-F]{8}$/.test(trimmed)) {
      return normalizeHexColor(trimmed);
    }
    const numeric = Number(trimmed);
    if (Number.isFinite(numeric)) {
      return normalizeHexColor(abgrIntToHex(Math.trunc(numeric)));
    }
    return normalizeHexColor(trimmed);
  }

  const sliderValue = (key: string, value: string) => {
    const spec = getSliderSpec(key);
    if (!spec) return 0;
    const parsed = Number(value);
    const current = Number.isFinite(parsed) ? parsed : spec.fallback;
    return Math.min(spec.max, Math.max(spec.min, current));
  };

  function updateName(value: string) {
    dispatch("updateName", { value });
    dispatch("requestLiveUpdate");
  }

  function updateParam(key: string, value: string) {
    dispatch("updateParam", { key, value });
    dispatch("requestLiveUpdate");
  }

  function renameParam(oldKey: string, newKey: string) {
    dispatch("renameParam", { oldKey, newKey });
    dispatch("requestLiveUpdate");
  }

  function removeParam(key: string) {
    dispatch("removeParam", { key });
    dispatch("requestLiveUpdate");
  }

  function resetProtectedParam(key: string) {
    dispatch("resetProtectedParam", { key });
    dispatch("requestLiveUpdate");
  }

  function addParam() {
    const key = newParamKey.trim();
    if (!key) return;
    dispatch("addParam", { key, value: newParamValue });
    dispatch("requestLiveUpdate");
    newParamKey = "";
    newParamValue = "";
  }

  function setTextInputMode(mode: "manual" | "from-file") {
    updateParam(fromFileKey, mode === "from-file" ? "true" : "false");
  }

  async function chooseTextFile() {
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: [
          { name: "Text files", extensions: ["txt"] },
          { name: "All files", extensions: ["*"] }
        ]
      });
      if (typeof selected === "string" && selected.trim()) {
        updateParam(textFileKey, selected);
      }
    } catch {
      // no-op
    }
  }

  function setMediaInputMode(mode: "local" | "network") {
    updateParam(mediaIsLocalFileKey, mode === "local" ? "true" : "false");
  }

  async function chooseMediaFile() {
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: [
          { name: "Media files", extensions: ["mp4", "mkv", "mov", "webm", "avi", "m4v", "mp3", "wav", "flac", "ogg", "m4a"] },
          { name: "All files", extensions: ["*"] }
        ]
      });
      if (typeof selected === "string" && selected.trim()) {
        updateParam(mediaLocalFileKey, selected);
      }
    } catch {
      // no-op
    }
  }

  function setAutoresizeFontWidth(enabled: boolean) {
    updateParam(autoresizeKey, enabled ? "true" : "false");
    if (extentsKey && extentsKey in editParams) {
      updateParam(extentsKey, enabled ? "false" : "true");
    }
  }

  const monitoringOptions = [
    { value: "off", label: "Off" },
    { value: "monitor_only", label: "Monitor only (silent output)" },
    { value: "on", label: "On" }
  ];

  $: isAudioSource = editType === "pulse_input_capture" || editType === "pulse_output_capture";
  $: isTextSource = editType === "text_ft2_source" || editType === "text_ft2_source_v2";
  $: isMediaSource = editType === "ffmpeg_source";
  $: audioDevices = editType === "pulse_input_capture" ? audioInputDevices : audioOutputDevices;
  $: audioLevel = Math.max(0, Math.min(100, Number(editParams.audio_level ?? 0)));
  $: textKey = getExistingParamKey(["text"]);
  $: colorKey = getExistingParamKey(["color1", "color"]);
  $: fromFileKey = getExistingParamKey(["from_file", "read_from_file", "is_from_file"]);
  $: textFileKey = getExistingParamKey(["text_file", "file"]);
  $: fontFaceKey = "font_face";
  $: fontSizeKey = "font_size";
  $: autoresizeKey = getExistingParamKey([
    "autoresize_font_width",
    "autoresize",
    "auto_resize_font_width",
    "auto_resize"
  ]);
  $: extentsKey = getExistingParamKey(["extents"]);
  $: mediaLocalFileKey = getExistingParamKey(["local_file", "file"]);
  $: mediaNetworkInputKey = getExistingParamKey(["input", "url"]);
  $: mediaIsLocalFileKey = getExistingParamKey(["is_local_file"]);
  $: isFromFileEnabled = isChecked(editParams[fromFileKey] ?? "false");
  $: textInputMode = isFromFileEnabled ? "from-file" : "manual";
  $: mediaInputMode = (() => {
    const explicit = (editParams[mediaIsLocalFileKey] ?? "").trim();
    if (explicit) return isChecked(explicit) ? "local" : "network";
    const hasNetwork = (editParams[mediaNetworkInputKey] ?? "").trim().length > 0;
    const hasLocal = (editParams[mediaLocalFileKey] ?? "").trim().length > 0;
    if (hasNetwork && !hasLocal) return "network";
    return "local";
  })();
  $: fontSizeValue = Math.max(6, Math.min(300, Number(editParams[fontSizeKey] ?? 24) || 24));
  $: autoresizeFontWidth = (() => {
    if (autoresizeKey && autoresizeKey in editParams) {
      return isChecked(editParams[autoresizeKey] ?? "true");
    }
    if (extentsKey && extentsKey in editParams) {
      return !isChecked(editParams[extentsKey] ?? "false");
    }
    return true;
  })();
  $: sourcePropertyByKey = new Map(sourcePropertySpecs.map((spec) => [spec.key, spec]));
  $: visibleSourcePropertyEntries = sourcePropertyEntries.filter(
    ([key]) =>
      !(isTextSource && isManagedTextKey(key)) &&
      !(isMediaSource && isManagedMediaKey(key)) &&
      !(isAudioSource && isManagedAudioDeviceKey(key))
  );
  $: visibleExtraParamEntries = extraParamEntries.filter(
    ([key]) =>
      !(isTextSource && isManagedTextKey(key)) &&
      !(isMediaSource && isManagedMediaKey(key)) &&
      !(isAudioSource && isManagedAudioDeviceKey(key))
  );
  const resolvePropertySpec = (key: string) => {
    const direct = sourcePropertyByKey.get(key);
    if (direct) return direct;

    const nk = normalizeKey(key);
    if (!nk) return undefined;

    return sourcePropertySpecs.find((spec) => {
      const sk = normalizeKey(spec.key);
      const sl = normalizeKey(spec.label ?? "");
      if (!sk) return false;
      return (
        sk === nk ||
        sk.includes(nk) ||
        nk.includes(sk) ||
        (!!sl && (sl === nk || sl.includes(nk) || nk.includes(sl)))
      );
    });
  };

  const getPropertyOptions = (key: string) => resolvePropertySpec(key)?.options ?? [];
  const hasPropertyOptions = (key: string) => getPropertyOptions(key).length > 0;
  const isChecked = (value: string) => {
    const normalized = (value ?? "").trim().toLowerCase();
    return normalized === "true" || normalized === "1" || normalized === "yes" || normalized === "on";
  };

  type InfoSegment = { type: "text"; text: string } | { type: "link"; text: string; href: string };
  const stripTags = (value: string) => value.replace(/<[^>]+>/g, "");
  function parseInfoSegments(raw: string): InfoSegment[] {
    const text = raw ?? "";
    if (!text.includes("<a")) {
      const plain = stripTags(text);
      return plain ? [{ type: "text", text: plain }] : [];
    }

    const segments: InfoSegment[] = [];
    const anchorRegex = /<a\s+[^>]*href=["']([^"']+)["'][^>]*>(.*?)<\/a>/gi;
    let last = 0;
    let match: RegExpExecArray | null;

    while ((match = anchorRegex.exec(text)) !== null) {
      if (match.index > last) {
        const before = stripTags(text.slice(last, match.index));
        if (before) segments.push({ type: "text", text: before });
      }

      const href = (match[1] ?? "").trim();
      const label = stripTags(match[2] ?? "").trim();
      if (href) {
        segments.push({ type: "link", href, text: label || href });
      }
      last = anchorRegex.lastIndex;
    }

    if (last < text.length) {
      const after = stripTags(text.slice(last));
      if (after) segments.push({ type: "text", text: after });
    }

    return segments;
  }
</script>

{#if editSource}
  <div
    class="modal-backdrop"
    class:window-mode={windowMode}
    role="button"
    tabindex="0"
    onclick={close}
    onkeydown={(e) => (e.key === "Escape" ? close() : null)}
  >
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
      <header class="modal-header">
        <div class="modal-title-row">
          <h2>Editing Source: {editName || editSource.name}</h2>
        </div>
        <button class="icon" aria-label="Close edit source" onclick={close}>
          ✕
        </button>
      </header>
      <div class="modal-body">
        <div class="field">
          <label for="editName">Name</label>
          <input
            id="editName"
            placeholder="Source name"
            value={editName}
            oninput={(e) => updateName((e.currentTarget as HTMLInputElement).value)}
          />
        </div>

        {#if isTextSource}
          <div class="field-group">
            <h3>Text</h3>
            <div class="field">
              <label for="text-input-mode">Text input mode</label>
              <select
                id="text-input-mode"
                value={textInputMode}
                onchange={(e) => setTextInputMode((e.currentTarget as HTMLSelectElement).value as "manual" | "from-file")}
              >
                <option value="manual">Manual</option>
                <option value="from-file">From file</option>
              </select>
            </div>

            {#if textInputMode === "manual"}
              <div class="field">
                <label for="text-content">Text</label>
                <textarea
                  id="text-content"
                  rows="4"
                  placeholder="Hello world"
                  value={editParams[textKey] ?? ""}
                  oninput={(e) => updateParam(textKey, (e.currentTarget as HTMLTextAreaElement).value)}
                ></textarea>
              </div>
            {:else}
              <div class="field">
                <label for="text-file">Text file</label>
                <div class="file-input-row">
                  <input
                    id="text-file"
                    type="text"
                    value={editParams[textFileKey] ?? ""}
                    placeholder="/path/to/file.txt"
                    oninput={(e) => updateParam(textFileKey, (e.currentTarget as HTMLInputElement).value)}
                  />
                  <button type="button" class="ghost picker-btn" onclick={chooseTextFile} aria-label="Select text file">📄</button>
                </div>
              </div>
            {/if}

            <div class="field">
              <label for="text-color-1">Color 1</label>
              <div class="color-input">
                <input
                  id="text-color-1"
                  type="text"
                  value={formatColorValue(colorKey, editParams[colorKey] ?? "")}
                  placeholder="#ffffff"
                  oninput={(e) =>
                    updateParam(
                      colorKey,
                      normalizeHexColor((e.currentTarget as HTMLInputElement).value)
                    )}
                />
                <input
                  class="picker"
                  type="color"
                  value={formatColorValue(colorKey, editParams[colorKey] ?? "#ffffff")}
                  oninput={(e) =>
                    updateParam(
                      colorKey,
                      normalizeHexColor((e.currentTarget as HTMLInputElement).value)
                    )}
                />
              </div>
            </div>

            <div class="field">
              <label for="text-font-face">Font</label>
              <input
                id="text-font-face"
                type="text"
                list="font-list"
                placeholder="DejaVu Sans"
                value={editParams[fontFaceKey] ?? ""}
                oninput={(e) => updateParam(fontFaceKey, (e.currentTarget as HTMLInputElement).value)}
              />
            </div>

            <div class="field">
              <label for="text-font-size">Font size</label>
              <div class="slider-input">
                <input
                  id="text-font-size"
                  type="range"
                  min="6"
                  max="300"
                  step="1"
                  value={fontSizeValue}
                  oninput={(e) => updateParam(fontSizeKey, (e.currentTarget as HTMLInputElement).value)}
                />
                <input
                  class="slider-value"
                  type="number"
                  min="6"
                  max="300"
                  step="1"
                  value={fontSizeValue}
                  oninput={(e) => updateParam(fontSizeKey, (e.currentTarget as HTMLInputElement).value)}
                />
              </div>
            </div>
          </div>
        {/if}

        {#if isMediaSource}
          <div class="field-group">
            <h3>Media source</h3>
            <div class="field">
              <label for="media-input-mode">Input type</label>
              <select
                id="media-input-mode"
                value={mediaInputMode}
                onchange={(e) => setMediaInputMode((e.currentTarget as HTMLSelectElement).value as "local" | "network")}
              >
                <option value="local">Local file</option>
                <option value="network">Network</option>
              </select>
            </div>

            {#if mediaInputMode === "local"}
              <div class="field">
                <label for="media-local-file">Media path</label>
                <div class="file-input-row">
                  <input
                    id="media-local-file"
                    type="text"
                    value={editParams[mediaLocalFileKey] ?? ""}
                    placeholder="/path/to/video.mp4"
                    oninput={(e) => updateParam(mediaLocalFileKey, (e.currentTarget as HTMLInputElement).value)}
                  />
                  <button type="button" class="ghost picker-btn" onclick={chooseMediaFile} aria-label="Select media file">📂</button>
                </div>
              </div>
            {:else}
              <div class="field">
                <label for="media-network-input">Network URL</label>
                <input
                  id="media-network-input"
                  type="text"
                  value={editParams[mediaNetworkInputKey] ?? ""}
                  placeholder="https://example.com/stream.m3u8"
                  oninput={(e) => updateParam(mediaNetworkInputKey, (e.currentTarget as HTMLInputElement).value)}
                />
              </div>
            {/if}
          </div>
        {/if}

        {#each (sourceParamSchemas[editType] ?? []) as field}
          {#if !(isTextSource && isManagedTextKey(field.key)) && !(isMediaSource && isManagedMediaKey(field.key))}
          <div class="field">
            <label for={`param-${field.key}`}>{field.label}</label>
            {#if field.key === "color"}
              <div class="color-input">
                <input
                  id={`param-${field.key}`}
                  type="text"
                  value={editParams[field.key] ?? ""}
                  placeholder={field.placeholder}
                  oninput={(e) =>
                    updateParam(
                      field.key,
                      normalizeHexColor((e.currentTarget as HTMLInputElement).value)
                    )}
                />
                <input
                  class="picker"
                  type="color"
                  value={normalizeHexColor(editParams[field.key] ?? "#ffffff")}
                  oninput={(e) =>
                    updateParam(
                      field.key,
                      normalizeHexColor((e.currentTarget as HTMLInputElement).value)
                    )}
                />
              </div>
            {:else if field.key === "font_face"}
              <input
                id={`param-${field.key}`}
                type="text"
                list="font-list"
                value={editParams[field.key] ?? ""}
                placeholder={field.placeholder}
                oninput={(e) => updateParam(field.key, (e.currentTarget as HTMLInputElement).value)}
              />
            {:else if field.key === "device" && isAudioSource}
              <select
                id={`param-${field.key}`}
                value={editParams.device && String(editParams.device).trim().length ? editParams.device : (audioDevices[0]?.id ?? "")}
                onchange={(e) => updateParam("device", (e.currentTarget as HTMLSelectElement).value)}
              >
                <option value={audioDevices[0]?.id ?? ""}>Default</option>
                {#each audioDevices as dev}
                  <option value={dev.id}>{dev.name}</option>
                {/each}
              </select>
            {:else if hasPropertyOptions(field.key)}
              <select
                id={`param-${field.key}`}
                value={editParams[field.key] ?? ""}
                onchange={(e) => updateParam(field.key, (e.currentTarget as HTMLSelectElement).value)}
              >
                {#if (editParams[field.key] ?? "") && !getPropertyOptions(field.key).some((o) => o.value === (editParams[field.key] ?? ""))}
                  <option value={editParams[field.key] ?? ""}>{editParams[field.key] ?? ""}</option>
                {/if}
                {#each getPropertyOptions(field.key) as option}
                  <option value={option.value}>{option.label}</option>
                {/each}
              </select>
            {:else}
              <input
                id={`param-${field.key}`}
                type="text"
                value={editParams[field.key] ?? ""}
                placeholder={field.placeholder}
                oninput={(e) => updateParam(field.key, (e.currentTarget as HTMLInputElement).value)}
              />
            {/if}
          </div>
          {/if}
        {/each}

        {#if isAudioSource}
          <div class="field">
            <div class="field-label">Audio level</div>
            <div class="meter">
              <div class="meter-fill" style={`width:${audioLevel}%`}></div>
            </div>
          </div>
          <div class="field">
            <label for="monitoring">Audio monitoring</label>
            <select
              id="monitoring"
              value={editParams.monitoring ?? "off"}
              onchange={(e) => updateParam("monitoring", (e.currentTarget as HTMLSelectElement).value)}
            >
              {#each monitoringOptions as option}
                <option value={option.value}>{option.label}</option>
              {/each}
            </select>
          </div>
        {/if}

        {#if visibleSourcePropertyEntries.length > 0}
          <div class="field-group">
            <h3>Source settings</h3>
            {#each visibleSourcePropertyEntries as [key, value]}
              {@const spec = sourcePropertyByKey.get(key)}
              <div class="field" class:field-inline={spec?.kind === "bool"}>
                {#if spec?.kind === "bool"}
                  <label class="checkbox-inline" for={`dynamic-${key}`}>
                    <input
                      id={`dynamic-${key}`}
                      type="checkbox"
                      checked={isChecked(value)}
                      onchange={(e) => updateParam(key, (e.currentTarget as HTMLInputElement).checked ? "true" : "false")}
                    />
                    <span>{spec?.label ?? key}</span>
                  </label>
                {:else if spec?.kind === "info"}
                  <div class="field-label">{spec?.label ?? key}</div>
                  <p class="info-text">
                    {#each parseInfoSegments((spec?.hint?.trim() || value || spec?.label || key)) as segment}
                      {#if segment.type === "link"}
                        <a href={segment.href} target="_blank" rel="noreferrer noopener">{segment.text}</a>
                      {:else}
                        <span>{segment.text}</span>
                      {/if}
                    {/each}
                  </p>
                {:else if spec?.kind === "list" && (spec.options?.length ?? 0) > 0}
                  <label for={`dynamic-${key}`}>{spec?.label ?? key}</label>
                  <select
                    id={`dynamic-${key}`}
                    value={value}
                    onchange={(e) => updateParam(key, (e.currentTarget as HTMLSelectElement).value)}
                  >
                    {#each spec.options ?? [] as option}
                      <option value={option.value}>{option.label}</option>
                    {/each}
                  </select>
                {:else if isListValue(key, value, spec?.kind)}
                  <label for={`dynamic-${key}`}>{spec?.label ?? key}</label>
                  <textarea
                    id={`dynamic-${key}`}
                    class="list-editor"
                    rows="5"
                    value={value}
                    placeholder="one item per line"
                    oninput={(e) => updateParam(key, normalizeListValue((e.currentTarget as HTMLTextAreaElement).value))}
                  ></textarea>
                {:else if spec?.kind === "int" || spec?.kind === "float"}
                  <label for={`dynamic-${key}`}>{spec?.label ?? key}</label>
                  {#if isSliderKey(key)}
                    {@const slider = getSliderSpec(key)!}
                    <div class="slider-input">
                      <input
                        id={`dynamic-${key}`}
                        type="range"
                        min={slider.min}
                        max={slider.max}
                        step={slider.step}
                        value={sliderValue(key, value)}
                        oninput={(e) => updateParam(key, (e.currentTarget as HTMLInputElement).value)}
                      />
                      <input
                        class="slider-value"
                        type="number"
                        min={slider.min}
                        max={slider.max}
                        step={slider.step}
                        value={sliderValue(key, value)}
                        oninput={(e) => updateParam(key, (e.currentTarget as HTMLInputElement).value)}
                      />
                    </div>
                  {:else}
                    <input
                      id={`dynamic-${key}`}
                      type="number"
                      value={value}
                      placeholder={spec?.label ?? key}
                      oninput={(e) => updateParam(key, (e.currentTarget as HTMLInputElement).value)}
                    />
                  {/if}
                {:else if isColorKey(key, spec?.kind)}
                  <label for={`dynamic-${key}`}>{spec?.label ?? key}</label>
                  <div class="color-input">
                    <input
                      id={`dynamic-${key}`}
                      type="text"
                      value={formatColorValue(key, value, spec?.kind)}
                      placeholder={spec?.label ?? key}
                      oninput={(e) => updateParam(key, normalizeHexColor((e.currentTarget as HTMLInputElement).value))}
                    />
                    <input
                      class="picker"
                      type="color"
                      value={formatColorValue(key, value, spec?.kind)}
                      oninput={(e) => updateParam(key, normalizeHexColor((e.currentTarget as HTMLInputElement).value))}
                    />
                  </div>
                {:else}
                  <label for={`dynamic-${key}`}>{spec?.label ?? key}</label>
                  <input
                    id={`dynamic-${key}`}
                    type="text"
                    value={value}
                    placeholder={spec?.label ?? key}
                    oninput={(e) => updateParam(key, (e.currentTarget as HTMLInputElement).value)}
                  />
                {/if}
              </div>
            {/each}
          </div>
        {/if}

        {#if isTextSource}
          <div class="field field-inline">
            <label class="checkbox-inline" for="autoresize-font-width">
              <input
                id="autoresize-font-width"
                type="checkbox"
                checked={autoresizeFontWidth}
                onchange={(e) => setAutoresizeFontWidth((e.currentTarget as HTMLInputElement).checked)}
              />
              <span>Autoresize font width to text length</span>
            </label>
          </div>
        {/if}

        <div class="field-group">
          <h3>Additional settings</h3>
          {#if visibleExtraParamEntries.length === 0}
            <p class="muted">No extra parameters.</p>
          {:else}
            {#each visibleExtraParamEntries as [key, value]}
              <div class="param-row">
                <input
                  class="param-key"
                  type="text"
                  value={key}
                  placeholder="key"
                  readonly={isProtectedTransformKey(key)}
                  oninput={(e) => renameParam(key, (e.currentTarget as HTMLInputElement).value)}
                />
                {#if isColorKey(key)}
                  <div class="color-input param-value">
                    <input
                      class="param-value"
                      type="text"
                      value={formatColorValue(key, value)}
                      placeholder="value"
                      oninput={(e) => updateParam(key, normalizeHexColor((e.currentTarget as HTMLInputElement).value))}
                    />
                    <input
                      class="picker"
                      type="color"
                      value={formatColorValue(key, value)}
                      oninput={(e) => updateParam(key, normalizeHexColor((e.currentTarget as HTMLInputElement).value))}
                    />
                  </div>
                {:else if isSliderKey(key)}
                  {@const slider = getSliderSpec(key)!}
                  <div class="slider-input param-value">
                    <input
                      type="range"
                      min={slider.min}
                      max={slider.max}
                      step={slider.step}
                      value={sliderValue(key, value)}
                      oninput={(e) => updateParam(key, (e.currentTarget as HTMLInputElement).value)}
                    />
                    <input
                      class="slider-value"
                      type="number"
                      min={slider.min}
                      max={slider.max}
                      step={slider.step}
                      value={sliderValue(key, value)}
                      oninput={(e) => updateParam(key, (e.currentTarget as HTMLInputElement).value)}
                    />
                  </div>
                {:else if hasPropertyOptions(key)}
                  <select
                    class="param-value"
                    value={value}
                    onchange={(e) => updateParam(key, (e.currentTarget as HTMLSelectElement).value)}
                  >
                    {#if value && !getPropertyOptions(key).some((o) => o.value === value)}
                      <option value={value}>{value}</option>
                    {/if}
                    {#each getPropertyOptions(key) as option}
                      <option value={option.value}>{option.label}</option>
                    {/each}
                  </select>
                {:else if isListValue(key, value)}
                  <textarea
                    class="param-value list-editor"
                    rows="4"
                    value={value}
                    placeholder="one item per line"
                    oninput={(e) => updateParam(key, normalizeListValue((e.currentTarget as HTMLTextAreaElement).value))}
                  ></textarea>
                {:else}
                  <input
                    class="param-value"
                    type="text"
                    value={value}
                    placeholder="value"
                    oninput={(e) => updateParam(key, (e.currentTarget as HTMLInputElement).value)}
                  />
                {/if}
                {#if isProtectedTransformKey(key)}
                  <button class="ghost" type="button" onclick={() => resetProtectedParam(key)}>Reset</button>
                {:else}
                  <button class="ghost" type="button" onclick={() => removeParam(key)}>Remove</button>
                {/if}
              </div>
            {/each}
          {/if}
          <div class="param-row">
            <input
              class="param-key"
              type="text"
              placeholder="new key"
              bind:value={newParamKey}
            />
            <input
              class="param-value"
              type="text"
              placeholder="new value"
              bind:value={newParamValue}
            />
            <button class="primary" type="button" onclick={addParam}>Add</button>
          </div>
        </div>

      </div>
      <footer class="modal-actions">
        <button class="ghost" onclick={close}>Cancel</button>
        <button class="primary" onclick={save}>Save</button>
      </footer>
    </div>
  </div>
{/if}

<datalist id="font-list">
  {#each fontOptions as font}
    <option value={font}></option>
  {/each}
</datalist>

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    display: grid;
    place-items: center;
    z-index: 50;
    padding: 1.25rem;
  }

  .modal-backdrop.window-mode {
    padding: 0;
    background: var(--surface-2);
  }

    .meter {
      height: 12px;
      border-radius: 999px;
      background: #1b1f2a;
      border: 1px solid var(--border);
      overflow: hidden;
    }

    .meter-fill {
      height: 100%;
      background: linear-gradient(90deg, #22c55e, #f59e0b, #ef4444);
      transition: width 0.2s ease;
    }
  .modal {
    width: min(620px, 95vw);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 16px;
    box-shadow: 0 20px 40px rgba(0, 0, 0, 0.45);
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto;
    gap: 1rem;
    padding: 1.25rem;
    max-height: min(820px, 94vh);
  }

  .modal.window-mode {
    width: 100vw;
    max-width: 100vw;
    height: 100dvh;
    max-height: 100dvh;
    border-radius: 0;
    border: 0;
    box-shadow: none;
    grid-template-rows: auto minmax(0, 1fr) auto;
    gap: 0.75rem;
    box-sizing: border-box;
  }

  .modal.draggable-popup {
    transform: translate(calc(var(--popup-dx, 0px)), calc(var(--popup-dy, 0px)));
  }

  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .modal-title-row {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    min-width: 0;
  }

  .modal-body {
    overflow-y: auto;
    min-height: 0;
    padding-right: 0.25rem;
  }

  .modal.window-mode .modal-body {
    overflow-y: auto;
  }

  h2 {
    margin: 0;
    font-size: 1.2rem;
  }

  .field-group {
    display: grid;
    gap: 0.75rem;
  }

  .field {
    display: grid;
    gap: 0.5rem;
  }

  .field-inline {
    display: block;
  }

  .checkbox-inline {
    display: inline-flex;
    align-items: center;
    gap: 0.55rem;
    font-weight: 500;
  }

  .checkbox-inline input[type="checkbox"] {
    width: 16px;
    height: 16px;
    margin: 0;
  }

  .info-text {
    margin: 0;
    color: var(--text-dim);
    line-height: 1.5;
    white-space: normal;
  }

  .info-text a {
    color: var(--accent);
    text-decoration: underline;
    text-underline-offset: 2px;
  }

  .param-row {
    display: grid;
    grid-template-columns: 1fr 1fr auto;
    gap: 0.5rem;
    align-items: center;
  }

  .slider-input {
    display: grid;
    grid-template-columns: 1fr 94px;
    gap: 0.5rem;
    align-items: center;
  }

  .file-input-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 0.5rem;
    align-items: center;
  }

  .picker-btn {
    min-width: 42px;
    padding-inline: 0.8rem;
  }

  .slider-input .slider-value {
    text-align: right;
  }

  .param-value {
    min-width: 0;
  }

  input,
  select,
  textarea {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.65rem 0.85rem;
    color: var(--text);
  }

  textarea {
    resize: vertical;
    min-height: 90px;
    font-family: inherit;
    line-height: 1.35;
  }

  input::placeholder {
    color: var(--text-muted);
  }

  input[type="color"] {
    padding: 0.35rem;
    height: 42px;
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.75rem;
    flex-wrap: wrap;
  }

  button {
    border: none;
    border-radius: 14px;
    padding: 0.7rem 1.2rem;
    font-weight: 600;
    cursor: pointer;
  }

  button.primary {
    background: var(--accent);
    color: #ffffff;
  }

  button.ghost {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--text);
  }

  button:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
</style>
