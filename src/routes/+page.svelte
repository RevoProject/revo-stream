<script lang="ts">
  import { convertFileSrc, invoke as tauriInvoke } from "@tauri-apps/api/core";
  import { LogicalPosition, LogicalSize } from "@tauri-apps/api/dpi";
  import { Webview } from "@tauri-apps/api/webview";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { onMount, tick } from "svelte";
  import HeaderBar from "../lib/components/Home/HeaderBar.svelte";
  import ScenesPanel from "../lib/components/Home/ScenesPanel.svelte";
  import SourcesPanel from "../lib/components/Home/SourcesPanel.svelte";
  import DemoMode from "../lib/components/Home/DemoMode.svelte";
  import DockPanel from "../lib/components/Home/Experimental/DockPanel.svelte";
  import SettingsModal from "../lib/components/Modal/SettingsModal.svelte";
  import PluginsModal from "../lib/components/Modal/PluginsModal.svelte";
  import AddSourceModal from "../lib/components/Modal/AddSourceModal.svelte";
  import AddSceneModal from "../lib/components/Modal/AddSceneModal.svelte";
  import EditSourceModal from "../lib/components/Modal/EditSourceModal.svelte";
  import SourceInfoModal from "../lib/components/Modal/SourceInfoModal.svelte";
  import FiltersModal from "../lib/components/Filters/FiltersModal.svelte";
  import AudioFiltersModal from "../lib/components/Filters/AudioFiltersModal.svelte";
  import Transitions from "../lib/components/Modules/Transitions.svelte";
  import AudioMixer from "../lib/components/Modules/AudioMixer.svelte";
  import AudioMixerContextMenu from "../lib/components/Modules/AudioMixerContextMenu.svelte";
  import AudioMixerAdvancedModal from "../lib/components/Modules/AudioMixerAdvancedModal.svelte";
  import StreamConfirmModal from "../lib/components/QuickModal/StreamConfirmModal.svelte";
  import TransitionsDiscardConfirmModal from "../lib/components/QuickModal/TransitionsDiscardConfirmModal.svelte";
  import QuickTextEditModal from "../lib/components/QuickModal/QuickTextEditModal.svelte";
  import QuickColorModal from "../lib/components/QuickModal/QuickColorModal.svelte";
  import QuickDeviceModal from "../lib/components/QuickModal/QuickDeviceModal.svelte";
  import type { DemoSource, SceneInfo } from "../lib/types";

  type PersistedSettings = {
    root_dir?: string | null;
    record_path?: string;
    stream_url?: string;
    stream_key?: string;
    preview_quality?: string | null;
    encoder_preference?: string | null;
    scene_resolution?: string | null;
    whep_url?: string | null;
    whip_url?: string | null;
    auto_retry_preview?: boolean | null;
    autorescale_inputs?: boolean | null;
    ui_profile?: Record<string, unknown> | null;
    active_profile?: string | null;
  };

  type AccessibilityUiProfile = {
    accessibilityHighContrast?: boolean;
    accessibilityReduceMotion?: boolean;
    accessibilityFocusIndicators?: boolean;
    accessibilityUiScale?: string;
    accessibilityFontSize?: string;
    accessibilityFontFamily?: string;
    accessibilityColorVision?: string;
  };

  type PluginInfo = {
    name: string;
    file_name: string;
    module_name: string;
  };

  type CefBridgeInfo = {
    compiled: boolean;
    major: number;
    minor: number;
    patch: number;
    commit: number;
  };

  type PluginProfileState = {
    profile_name: string;
    enabled_modules: string[];
  };

  type SourceTypeItem = {
    id: string;
    label: string;
  };

  type SourcePropertyOption = {
    value: string;
    label: string;
  };

  type SourcePropertySpec = {
    key: string;
    label: string;
    kind: string;
    hint?: string;
    options?: SourcePropertyOption[];
  };

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

  type TransitionItem = {
    id: string;
    name: string;
    kind: string;
    params: Record<string, string>;
  };

  const normalizePropertyKey = (value: string) => value.toLowerCase().replace(/[^a-z0-9]+/g, "").trim();

  const sourcePropertyHasOptions = (specs: SourcePropertySpec[], keys: string[]) => {
    const wanted = keys.map((k) => normalizePropertyKey(k));
    return specs.some((spec) => {
      const key = normalizePropertyKey(spec.key);
      const label = normalizePropertyKey(spec.label ?? "");
      const matches = wanted.some((w) => w === key || (w && key.includes(w)) || (key && w.includes(key)) || w === label || (w && label.includes(w)) || (label && w.includes(label)));
      return matches && (spec.options?.length ?? 0) > 0;
    });
  };

  const isReleaseBuild = import.meta.env.MODE === "production";

  async function invoke<T>(cmd: string, args?: Record<string, unknown>) {
    if (cmd.startsWith("obs_")) {
      try {
        await tauriInvoke("debug_console_log", {
          action: cmd,
          detail: args ?? null
        });
      } catch {
        // ignore debug log errors
      }
    }
    return tauriInvoke<T>(cmd, args);
  }

  async function logThemeDebug(action: string, detail?: Record<string, unknown>) {
    try {
      await tauriInvoke("debug_console_log", {
        action: `[theme] ${action}`,
        detail: detail ?? null
      });
    } catch {
      // ignore
    }
  }

  const previewQualityOptions = [
    { value: "very_low", label: "Very Low" },
    { value: "low", label: "Low" },
    { value: "medium", label: "Medium" },
    { value: "high", label: "High" }
  ];

  const encoderPreferenceOptions = [
    { value: "hardware", label: "Hardware (if supported)" },
    { value: "software", label: "Software (x264)" }
  ];

  const previewRenderOptions = [
    { value: "720x480", label: "SD" },
    { value: "1280x720", label: "HD" },
    { value: "1920x1080", label: "FULL HD" },
    { value: "2560x1440", label: "2K" }
  ];

  const sceneResolutionOptions = [
    { value: "720x480", label: "720x480 SD" },
    { value: "1280x720", label: "1280x720 HD" },
    { value: "1920x1080", label: "1920x1080 FULL HD" },
    { value: "2560x1440", label: "2560x1440 2K" }
  ];

  const sourceTypes = [
    { id: "browser_source", label: "Browser" },
    { id: "image_source", label: "Image" },
    { id: "ffmpeg_source", label: "Media (FFmpeg)" },
    { id: "text_ft2_source_v2", label: "Text (FT2)" },
    { id: "color_source_v2", label: "Color" },
    { id: "window_capture", label: "Window Capture" },
    { id: "xcomposite_input", label: "XComposite Capture" },
    { id: "pulse_input_capture", label: "Audio Input (Pulse)" },
    { id: "pulse_output_capture", label: "Audio Output (Pulse)" }
  ];

  const builtInSourceTypeIds = new Set(sourceTypes.map((t) => t.id));
  const normalizeSourceLabel = (label: string) =>
    label
      .toLowerCase()
      .replace(/\(.*?\)/g, "")
      .replace(/\bsource\b/g, "")
      .replace(/[^a-z0-9]+/g, " ")
      .trim();
  const builtInSourceTypeLabels = new Set(sourceTypes.map((t) => normalizeSourceLabel(t.label)));
  const externalSourceTypeBlacklist = new Set([
    "color_source",
    "color_source_v2",
    "text_ft2_source",
    "text_ft2_source_v2"
  ]);

  const sourceParamSchemas: Record<string, { key: string; label: string; placeholder: string }[]> = {
    browser_source: [
      { key: "url", label: "URL", placeholder: "https://example.com" },
      { key: "pos_x", label: "Position X", placeholder: "0" },
      { key: "pos_y", label: "Position Y", placeholder: "0" },
      { key: "width", label: "Width", placeholder: "640" },
      { key: "height", label: "Height", placeholder: "360" }
    ],
    image_source: [
      { key: "file", label: "Image path", placeholder: "/path/to/image.png" },
      { key: "pos_x", label: "Position X", placeholder: "0" },
      { key: "pos_y", label: "Position Y", placeholder: "0" },
      { key: "width", label: "Width", placeholder: "640" },
      { key: "height", label: "Height", placeholder: "360" }
    ],
    ffmpeg_source: [
      { key: "local_file", label: "Media path", placeholder: "/path/to/video.mp4" },
      { key: "pos_x", label: "Position X", placeholder: "0" },
      { key: "pos_y", label: "Position Y", placeholder: "0" },
      { key: "width", label: "Width", placeholder: "640" },
      { key: "height", label: "Height", placeholder: "360" }
    ],
    text_ft2_source_v2: [
      { key: "text", label: "Text", placeholder: "Hello world" },
      { key: "color1", label: "Color 1", placeholder: "#ffffff" },
      { key: "font_face", label: "Font", placeholder: "DejaVu Sans" },
      { key: "font_size", label: "Font size", placeholder: "24" },
      { key: "pos_x", label: "Position X", placeholder: "0" },
      { key: "pos_y", label: "Position Y", placeholder: "0" },
      { key: "width", label: "Width", placeholder: "640" },
      { key: "height", label: "Height", placeholder: "360" }
    ],
    text_ft2_source: [
      { key: "text", label: "Text", placeholder: "Hello world" },
      { key: "color1", label: "Color 1", placeholder: "#ffffff" },
      { key: "font_face", label: "Font", placeholder: "DejaVu Sans" },
      { key: "font_size", label: "Font size", placeholder: "24" },
      { key: "pos_x", label: "Position X", placeholder: "0" },
      { key: "pos_y", label: "Position Y", placeholder: "0" },
      { key: "width", label: "Width", placeholder: "640" },
      { key: "height", label: "Height", placeholder: "360" }
    ],
    color_source_v2: [
      { key: "color", label: "Hex color", placeholder: "#ff0000" },
      { key: "pos_x", label: "Position X", placeholder: "0" },
      { key: "pos_y", label: "Position Y", placeholder: "0" },
      { key: "width", label: "Width", placeholder: "640" },
      { key: "height", label: "Height", placeholder: "360" }
    ],
    window_capture: [
      { key: "window", label: "Window", placeholder: "Title:Class" },
      { key: "pos_x", label: "Position X", placeholder: "0" },
      { key: "pos_y", label: "Position Y", placeholder: "0" },
      { key: "width", label: "Width", placeholder: "640" },
      { key: "height", label: "Height", placeholder: "360" }
    ],
    xcomposite_input: [
      { key: "window", label: "Window", placeholder: "Title:Class" },
      { key: "pos_x", label: "Position X", placeholder: "0" },
      { key: "pos_y", label: "Position Y", placeholder: "0" },
      { key: "width", label: "Width", placeholder: "640" },
      { key: "height", label: "Height", placeholder: "360" }
    ],
    pulse_input_capture: [{ key: "device", label: "Device", placeholder: "default" }],
    pulse_output_capture: [{ key: "device", label: "Device", placeholder: "default" }]
  };

    const fontOptions = ["Inter", "Space Grotesk", "Roboto", "Open Sans"];

    let rootDir = "";
    let version = "0.1.0";
    // let message = ""; // removed, now using globalDialogs
    // Global dialog queue system
    type DialogType = "info" | "warning" | "error";
    interface GlobalDialog {
      id: number;
      type: DialogType;
      text: string;
      icon?: string;
      timeout?: number;
    }
    let globalDialogs: GlobalDialog[] = [];
    let dialogIdCounter = 1;
    const DIALOG_DEDUP_WINDOW_MS = 1800;
    let dialogLastShownAt: Map<string, number> = new Map();

    function showGlobalDialog(text: string, type: DialogType = "info", timeout = 3200) {
      const normalizedText = String(text ?? "").trim();
      if (!normalizedText) return;

      const dedupKey = `${type}:${normalizedText}`;
      const now = Date.now();

      if (globalDialogs.some((d) => d.type === type && d.text === normalizedText)) {
        return;
      }

      const lastShown = dialogLastShownAt.get(dedupKey) ?? 0;
      if (now - lastShown < DIALOG_DEDUP_WINDOW_MS) {
        return;
      }
      dialogLastShownAt.set(dedupKey, now);

      if (dialogLastShownAt.size > 200) {
        const cutoff = now - Math.max(timeout, DIALOG_DEDUP_WINDOW_MS) * 4;
        dialogLastShownAt = new Map(
          [...dialogLastShownAt.entries()].filter(([, ts]) => ts >= cutoff)
        );
      }

      const id = dialogIdCounter++;
      let icon = "";
      if (type === "warning") icon = "⚠️";
      else if (type === "error") icon = "❌";
      else icon = "ℹ️";
      globalDialogs = [...globalDialogs, { id, type, text: normalizedText, icon, timeout }];
      setTimeout(() => {
        globalDialogs = globalDialogs.filter((d) => d.id !== id);
      }, timeout);
    }
    let busy = false;
    let demoMode = false;
    let tauriAvailable = false;
    let isObsRunning = false;
    let isRecording = false;
    let isStreaming = false;
    type MediaConfirmAction = "start-streaming" | "stop-streaming" | "start-recording" | "stop-recording";
    let showMediaConfirm = false;
    let showCloseRiskConfirm = false;
    let showUnsavedSettingsCloseConfirm = false;
    let showKeyboardShortcutsHelp = false;
    let pendingMediaConfirmAction: MediaConfirmAction | null = null;
    let mediaActionBusy = false;
    let closeRiskBypassOnce = false;
    let allowExplicitAppClose = false;
    let settingsHasUnsavedChanges = false;

    let recordPath = "";
    let streamUrl = "";
    let streamKey = "";
    let previewQuality = "high";
    let previewRenderIndex = 2;
    let previewRenderTimer: ReturnType<typeof setTimeout> | null = null;
    let encoderPreference = "hardware";
    let sceneResolution = "1920x1080";
    let realtimeRefresh = isReleaseBuild;
    let previewUrl = "";
    let previewInterval: ReturnType<typeof setTimeout> | null = null;
    let previewAnimationFrame: number | null = null;
    let previewInFlight = false;
    let previewPendingRequest = false;
    let previewLastFrameAt = 0;
    let previewResolutionSyncAt = 0;
    let previewVideo: HTMLVideoElement | null = null;
    let stingerPreviewVideo: HTMLVideoElement | null = null;
    let previewDirty = true;
    let previewTransitionVisible = false;
    let previewTransitionKind: "cut" | "fade" | "swipe" | "slide" | "fade_to_color" | "luma_wipe" | "stinger" | null = null;
    let previewTransitionDirection: "left" | "right" | "up" | "down" = "left";
    let previewTransitionDurationMs = 320;
    let previewTransitionColor = "#000000";
    let previewTransitionSoftness = 35;
    let previewTransitionToken = 0;
    let previewTransitionStingerSrc = "";
    let previewTransitionTimer: ReturnType<typeof setTimeout> | null = null;
    let webrtcPc: RTCPeerConnection | null = null;
    let webrtcActive = false;
    let webrtcError = "";
    let whepUrl = "http://127.0.0.1:8080/whep";
    let whipUrl = "http://127.0.0.1:8080/whip";
    let autoRetryPreview = true;
    let webrtcRetryTimer: ReturnType<typeof setTimeout> | null = null;
    let webrtcRetryAttempt = 0;
    let webrtcWhipStarted = false;
    let isResizing = false;
    let resizeTimer: ReturnType<typeof setTimeout> | null = null;
    let showCannotRemoveLastScene = false;
    let settingsLoaded = false;
    let currentUiProfile: Record<string, unknown> | null = null;
    let profileOptions: string[] = ["default"];
    let selectedProfileName = "default";
    let settingsModalSaved = false;
    let settingsSnapshotBeforeOpen: PersistedSettings | null = null;
    let accessibilityHighContrast = false;
    let accessibilityReduceMotion = false;
    let accessibilityFocusIndicators = true;
    let accessibilityUiScale = "100";
    let accessibilityFontSize = "100";
    let accessibilityFontFamily = "system";
    let accessibilityColorVision = "none";
    let audioInputDevices: { id: string; name: string }[] = [];
    let audioOutputDevices: { id: string; name: string }[] = [];
    let showPlugins = false;
    let pluginProfiles: string[] = ["default"];
    let activePluginProfile = "default";
    let pluginsList: PluginInfo[] = [];
    let enabledPluginModules: string[] = [];
    let pluginBaselineModulesByProfile: Record<string, string[]> = {};
    let pluginsLoadSeq = 0;
    let showTransitionsModal = false;
    let transitionsDraft: TransitionItem[] = [];
    let transitionsSelectedId: string | null = null;
    let activeTransitionId: string | null = null;
    let selectedTransitionItem: TransitionItem | null = null;
    let transitionNewKind = "fade";
    let transitionNewName = "";
    type TransitionModalSnapshot = {
      transitionsDraft: TransitionItem[];
      transitionsSelectedId: string | null;
      activeTransitionId: string | null;
      transitionNewKind: string;
      transitionNewName: string;
    };
    let transitionsSnapshotBeforeOpen: TransitionModalSnapshot | null = null;
    let showTransitionsDiscardConfirm = false;
    let injectedThemeId = "";
    let injectedThemeStyleEl: HTMLStyleElement | null = null;
    // Store manual transforms for sources (by id)
    type PlannerTransformPayload = {
      x: number;
      y: number;
      w: number;
      h: number;
      rot?: number;
      cropL?: number;
      cropR?: number;
      cropT?: number;
      cropB?: number;
    };
    let sourceTransforms: Record<string, PlannerTransformPayload> = {};
    let plannerTransformsByScene: Record<string, Record<string, PlannerTransformPayload>> = {};
    let transformWindow: Window | null = null;
    let previewFrameEl: HTMLDivElement | null = null;
    let panelScenesEl: HTMLDivElement | null = null;
    let panelSourcesEl: HTMLDivElement | null = null;
    let panelToolsEl: HTMLDivElement | null = null;
    let saveTransformsUnlisten: (() => void) | null = null;
    let themeRefreshUnlisten: (() => void) | null = null;
    let closeRequestUnlisten: (() => void) | null = null;

    const THEME_REFRESH_EVENT = "revo:theme-refresh";
    const THEME_REFRESH_STORAGE_KEY = "revo_theme_refresh_ts";

    const clearA11yColorVisionClasses = (root: HTMLElement) => {
      root.classList.remove("a11y-color-protanopia", "a11y-color-deuteranopia", "a11y-color-tritanopia");
    };

    function applyAccessibilityProfile(profile?: Record<string, unknown> | null) {
      if (typeof document === "undefined") return;

      const root = document.documentElement;
      const a11y = (profile?.accessibility as AccessibilityUiProfile | undefined) ?? {};

      const uiScale = Math.max(75, Math.min(200, Number(a11y.accessibilityUiScale ?? "100")));
      root.style.zoom = `${uiScale / 100}`;

      const fontScale = Math.max(0.8, Math.min(2, Number(a11y.accessibilityFontSize ?? "100") / 100));
      root.style.setProperty("--a11y-font-scale", String(fontScale));

      const fontFamilyValue = a11y.accessibilityFontFamily;
      if (fontFamilyValue === "sans") {
        root.style.setProperty("--a11y-font-family", "Arial, Helvetica, sans-serif");
      } else if (fontFamilyValue === "serif") {
        root.style.setProperty("--a11y-font-family", "Georgia, Times New Roman, serif");
      } else if (fontFamilyValue === "mono") {
        root.style.setProperty("--a11y-font-family", "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, Liberation Mono, monospace");
      } else {
        root.style.setProperty("--a11y-font-family", '"Space Grotesk", "Inter", system-ui, sans-serif');
      }

      if (a11y.accessibilityColorVision === "protanopia") {
        root.style.setProperty("--a11y-color-filter", "sepia(0.24) hue-rotate(-18deg) saturate(0.78)");
      } else if (a11y.accessibilityColorVision === "deuteranopia") {
        root.style.setProperty("--a11y-color-filter", "sepia(0.2) hue-rotate(8deg) saturate(0.75)");
      } else if (a11y.accessibilityColorVision === "tritanopia") {
        root.style.setProperty("--a11y-color-filter", "hue-rotate(28deg) saturate(0.78)");
      } else {
        root.style.setProperty("--a11y-color-filter", "none");
      }

      root.classList.toggle("a11y-high-contrast", Boolean(a11y.accessibilityHighContrast));
      root.classList.toggle("a11y-reduce-motion", Boolean(a11y.accessibilityReduceMotion));
      root.classList.toggle("a11y-strong-focus", a11y.accessibilityFocusIndicators !== false);

      clearA11yColorVisionClasses(root);
      if (a11y.accessibilityColorVision === "protanopia") {
        root.classList.add("a11y-color-protanopia");
      } else if (a11y.accessibilityColorVision === "deuteranopia") {
        root.classList.add("a11y-color-deuteranopia");
      } else if (a11y.accessibilityColorVision === "tritanopia") {
        root.classList.add("a11y-color-tritanopia");
      }
    }

    const asRecord = (value: unknown): Record<string, unknown> =>
      value && typeof value === "object" && !Array.isArray(value) ? (value as Record<string, unknown>) : {};

    const transitionPresetDefaults: Record<string, Record<string, string>> = {
      cut: {},
      fade: { duration_ms: "300" },
      swipe: { duration_ms: "350", direction: "left" },
      slide: { duration_ms: "350", direction: "left" },
      fade_to_color: { duration_ms: "400", color: "#000000" },
      luma_wipe: { duration_ms: "450", direction: "left", softness_pct: "35" },
      stinger: { source_mode: "media", media_file: "", sequence_dir: "", sequence_fps: "30", transition_point_ms: "1000" }
    };

    const getTransitionDefaults = (kind: string) => ({ ...(transitionPresetDefaults[kind] ?? {}) });

    const toPositiveMs = (raw: unknown, fallback: number) => {
      const parsed = Number(String(raw ?? "").trim());
      if (!Number.isFinite(parsed) || parsed <= 0) return fallback;
      return Math.max(50, Math.min(15000, Math.round(parsed)));
    };

    const normalizeTransitionColor = (raw: unknown, fallback = "#000000") => {
      const value = String(raw ?? "").trim();
      if (!value) return fallback;
      return value;
    };

    const toSoftnessPercent = (raw: unknown, fallback = 35) => {
      const parsed = Number(String(raw ?? "").trim());
      if (!Number.isFinite(parsed)) return fallback;
      return Math.max(0, Math.min(100, Math.round(parsed)));
    };

    const resolveLocalMediaSrc = (path: string) => {
      const trimmed = String(path ?? "").trim();
      if (!trimmed) return "";
      if (trimmed.startsWith("http://") || trimmed.startsWith("https://") || trimmed.startsWith("asset:")) {
        return trimmed;
      }
      if (trimmed.startsWith("file://")) {
        try {
          return convertFileSrc(decodeURIComponent(trimmed.replace(/^file:\/\//i, "")));
        } catch {
          return convertFileSrc(trimmed.replace(/^file:\/\//i, ""));
        }
      }
      if (trimmed.startsWith("/")) {
        return convertFileSrc(trimmed);
      }
      return trimmed;
    };

    const clearPreviewTransition = () => {
      if (previewTransitionTimer) {
        clearTimeout(previewTransitionTimer);
        previewTransitionTimer = null;
      }
      previewTransitionVisible = false;
      previewTransitionKind = null;
      previewTransitionColor = "#000000";
      previewTransitionSoftness = 35;
      previewTransitionStingerSrc = "";
      if (stingerPreviewVideo) {
        try {
          stingerPreviewVideo.pause();
          stingerPreviewVideo.currentTime = 0;
        } catch {
          // ignore
        }
      }
    };

    const getActiveTransition = () => transitionsDraft.find((t) => t.id === activeTransitionId) ?? null;

    const getTransitionSceneSwitchDelayMs = () => {
      const active = getActiveTransition();
      if (!active || active.kind !== "stinger") return 0;
      return toPositiveMs((active.params ?? {}).transition_point_ms, 1000);
    };

    const playStingerPreviewFromStart = () => {
      void tick().then(async () => {
        const video = stingerPreviewVideo;
        if (!video || previewTransitionKind !== "stinger") return;
        try {
          video.pause();
          video.currentTime = 0;
          await video.play();
        } catch {
          // ignore autoplay/codec issues in preview
        }
      });
    };

    const triggerRenderTransition = () => {
      const active = getActiveTransition();
      if (!active) return;

      previewTransitionToken += 1;
      const token = previewTransitionToken;

      let kind = (active.kind || "fade") as "cut" | "fade" | "swipe" | "slide" | "fade_to_color" | "luma_wipe" | "stinger";
      let duration = toPositiveMs((active.params ?? {}).duration_ms, kind === "cut" ? 120 : 320);
      previewTransitionDirection = ((active.params ?? {}).direction as "left" | "right" | "up" | "down") || "left";
      previewTransitionColor = normalizeTransitionColor((active.params ?? {}).color, "#000000");
      previewTransitionSoftness = toSoftnessPercent((active.params ?? {}).softness_pct, 35);

      if (kind === "stinger") {
        const params = active.params ?? {};
        const sourceMode = String(params.source_mode ?? "media").trim().toLowerCase();
        if (sourceMode === "media") {
          const stingerSrc = resolveLocalMediaSrc(String(params.media_file ?? ""));
          if (stingerSrc) {
            previewTransitionStingerSrc = stingerSrc;
            const transitionPoint = toPositiveMs(params.transition_point_ms, 1000);
            duration = Math.min(15000, transitionPoint + 450);
          } else {
            kind = "fade";
            duration = 320;
          }
        } else {
          // Sequence transition visualization fallback for preview render.
          kind = "fade";
          duration = toPositiveMs(params.transition_point_ms, 700);
        }
      }

      previewTransitionKind = kind;
      previewTransitionDurationMs = duration;
      previewTransitionVisible = true;

      if (kind === "stinger") {
        playStingerPreviewFromStart();
      }

      previewTransitionTimer = setTimeout(() => {
        if (token !== previewTransitionToken) return;
        clearPreviewTransition();
      }, duration + 80);
    };

    const normalizeTransitionsFromProfile = (profile?: Record<string, unknown> | null) => {
      const tools = asRecord(asRecord(profile).tools);
      const transitions = asRecord(tools.transitions);
      const list = Array.isArray(transitions.items) ? transitions.items : [];
      const normalized: TransitionItem[] = list
        .map((entry, index) => {
          const row = asRecord(entry);
          const kind = String(row.kind ?? "fade").trim() || "fade";
          const id = String(row.id ?? "").trim() || (crypto.randomUUID ? crypto.randomUUID() : `${Date.now()}-${Math.random()}`);
          const name = String(row.name ?? "").trim() || `Transition ${index + 1}`;
          const paramsRaw = asRecord(row.params);
          const params = { ...getTransitionDefaults(kind) };
          for (const [k, v] of Object.entries(paramsRaw)) {
            params[String(k)] = String(v ?? "");
          }
          return { id, name, kind, params };
        })
        .filter((item) => item.id.trim().length > 0);

      transitionsDraft = normalized.length
        ? normalized
        : [{ id: crypto.randomUUID ? crypto.randomUUID() : `${Date.now()}-${Math.random()}`, name: "Transition 1", kind: "fade", params: getTransitionDefaults("fade") }];

      const incomingActive = String(transitions.activeTransitionId ?? "").trim();
      activeTransitionId = transitionsDraft.some((t) => t.id === incomingActive)
        ? incomingActive
        : transitionsDraft[0]?.id ?? null;
      transitionsSelectedId = activeTransitionId;
    };

    const mergeTransitionsIntoProfile = (profile: Record<string, unknown> | null | undefined): Record<string, unknown> => {
      const root = asRecord(profile);
      const tools = asRecord(root.tools);
      return {
        ...root,
        tools: {
          ...tools,
          transitions: {
            items: transitionsDraft.map((t) => ({ ...t, params: { ...(t.params ?? {}) } })),
            activeTransitionId
          }
        }
      };
    };

    const clearAppliedThemeVars = () => {
      if (typeof document === "undefined") return;
      const root = document.documentElement;
      const keys = [
        "--bg",
        "--surface",
        "--surface-2",
        "--surface-3",
        "--border",
        "--border-strong",
        "--text",
        "--text-muted",
        "--accent",
        "--accent-strong",
        "--success",
        "--danger",
        "--warning",
        "--icon-color",
        "--revo-disable-rounded",
        "--preview-bg",
        "--scene-active-text"
      ];
      for (const key of keys) {
        root.style.removeProperty(key);
      }
      root.style.removeProperty("color-scheme");
      root.classList.remove("revo-no-rounded");
    };

    const syncThemeCssFlags = () => {
      if (typeof document === "undefined") return;
      const root = document.documentElement;
      const roundedFlag = root.style.getPropertyValue("--revo-disable-rounded").trim().toLowerCase();
      const disableRounded = roundedFlag === "1" || roundedFlag === "true" || roundedFlag === "yes" || roundedFlag === "on";
      root.classList.toggle("revo-no-rounded", disableRounded);
    };

    const applyThemeVarsFromCss = (css: string) => {
      if (typeof document === "undefined") return;
      clearAppliedThemeVars();
      const root = document.documentElement;
      const colorSchemeMatch = css.match(/color-scheme\s*:\s*([^;]+);/i);
      if (colorSchemeMatch?.[1]) {
        root.style.setProperty("color-scheme", colorSchemeMatch[1].trim());
      }
      const varRegex = /(--[a-z0-9-]+)\s*:\s*([^;]+);/gi;
      let match: RegExpExecArray | null = null;
      while ((match = varRegex.exec(css)) !== null) {
        const key = (match[1] ?? "").trim();
        const value = (match[2] ?? "").trim();
        if (!key || !value) continue;
        root.style.setProperty(key, value);
      }
      syncThemeCssFlags();
    };

    const applyLookProfile = async (profile?: Record<string, unknown> | null) => {
      if (typeof document === "undefined") return;
      const look = asRecord(asRecord(profile).look);
      const selectedThemeId = String(look.selectedThemeId ?? "").trim();
      const rootArg = rootDir.trim().length ? rootDir.trim() : null;
      void logThemeDebug("apply:start", { selectedThemeId, rootArg });

      if (!selectedThemeId) {
        injectedThemeId = "";
        clearAppliedThemeVars();
        if (injectedThemeStyleEl) {
          injectedThemeStyleEl.remove();
          injectedThemeStyleEl = null;
        }
        void logThemeDebug("apply:cleared-default-theme", {});
        return;
      }

      if (!tauriAvailable) {
        void logThemeDebug("apply:skip-no-tauri", { selectedThemeId });
        return;
      }
      try {
        const css = await invoke<string>("themes_get_css", { themeId: selectedThemeId, rootDir: rootArg });
        const varCount = (css.match(/--[a-z0-9-]+\s*:/gi) ?? []).length;
        if (!injectedThemeStyleEl) {
          injectedThemeStyleEl = document.createElement("style");
          injectedThemeStyleEl.setAttribute("data-revo-theme", "custom");
          document.head.appendChild(injectedThemeStyleEl);
        }
        injectedThemeStyleEl.textContent = css;
        applyThemeVarsFromCss(css);
        injectedThemeId = selectedThemeId;
        const root = document.documentElement;
        void logThemeDebug("apply:ok", {
          selectedThemeId,
          cssLength: css.length,
          varCount,
          accent: root.style.getPropertyValue("--accent").trim(),
          bg: root.style.getPropertyValue("--bg").trim()
        });
      } catch (err) {
        clearAppliedThemeVars();
        if (injectedThemeStyleEl) {
          injectedThemeStyleEl.remove();
          injectedThemeStyleEl = null;
        }
        injectedThemeId = "";
        void logThemeDebug("apply:error", { selectedThemeId, error: String(err) });
      }
    };

    $: selectedTransitionItem = transitionsDraft.find((t) => t.id === transitionsSelectedId) ?? null;

    const getActiveSceneName = () =>
      scenes.find((s) => s.active)?.name?.trim() || "default";

    let plannerSaveApplyInFlight = false;
    let plannerQueuedTransforms: Record<string, PlannerTransformPayload> | null = null;

    const applySourceTransforms = async (newTransforms: Record<string, PlannerTransformPayload>) => {
      const activeSceneName = getActiveSceneName();
      sourceTransforms = { ...newTransforms };
      plannerTransformsByScene = {
        ...plannerTransformsByScene,
        [activeSceneName]: { ...newTransforms }
      };
      sourcesList = sourcesList.map(s => {
        const t = sourceTransforms[s.id];
        if (!t) return s;
        const nextParams: Record<string, string> = {
          ...(s.params || {}),
          pos_x: t.x.toString(),
          pos_y: t.y.toString(),
          item_width: t.w.toString(),
          item_height: t.h.toString()
        };
        if (typeof t.rot === "number") nextParams.rot = t.rot.toString();
        if (typeof t.cropL === "number") nextParams.crop_left = t.cropL.toString();
        if (typeof t.cropR === "number") nextParams.crop_right = t.cropR.toString();
        if (typeof t.cropT === "number") nextParams.crop_top = t.cropT.toString();
        if (typeof t.cropB === "number") nextParams.crop_bottom = t.cropB.toString();
        return {
          ...s,
          params: nextParams
        };
      });

      if (backendEnabled) {
        try {
          for (const s of sourcesList) {
            const t = sourceTransforms[s.id];
            if (!t) continue;
            const params: Record<string, string> = {
              ...(s.params || {}),
              pos_x: t.x.toString(),
              pos_y: t.y.toString(),
              item_width: t.w.toString(),
              item_height: t.h.toString()
            };
            if (typeof t.rot === "number") params.rot = t.rot.toString();
            if (typeof t.cropL === "number") params.crop_left = t.cropL.toString();
            if (typeof t.cropR === "number") params.crop_right = t.cropR.toString();
            if (typeof t.cropT === "number") params.crop_top = t.cropT.toString();
            if (typeof t.cropB === "number") params.crop_bottom = t.cropB.toString();
            await invoke<string>("obs_update_source", {
              update: {
                id: s.id,
                name: s.name,
                source_type: s.source_type,
                params
              }
            });
          }
          await loadSources();
          previewDirty = true;
          await refreshPreview(true);
          requestPreviewUpdate();
          showGlobalDialog("Source transforms saved", "info");
        } catch (err) {
          showGlobalDialog(String(err), "error");
        }
      } else {
        previewDirty = true;
        await refreshPreview(true);
        showGlobalDialog("Source transforms saved", "info");
      }
    };

    const queueApplySourceTransforms = async (newTransforms: Record<string, PlannerTransformPayload>) => {
      plannerQueuedTransforms = { ...newTransforms };
      if (plannerSaveApplyInFlight) return;
      plannerSaveApplyInFlight = true;
      try {
        while (plannerQueuedTransforms) {
          const next = plannerQueuedTransforms;
          plannerQueuedTransforms = null;
          await applySourceTransforms(next);
        }
      } finally {
        plannerSaveApplyInFlight = false;
      }
    };

    if (typeof window !== 'undefined') {
      (window as any).openGraphicPlanner = async () => {
        console.info("openGraphicPlanner: starting");
        try {
          await openSourceTransformModal();
          console.info("openGraphicPlanner: done");
        } catch (err) {
          console.error("openGraphicPlanner: failed", err);
          throw err;
        }
      };
    }

    async function openSourceTransformModal(sourceOrEvent?: DemoSource | MouseEvent) {
      if (typeof window === 'undefined') return;
      const source =
        sourceOrEvent && typeof sourceOrEvent === "object" && "id" in sourceOrEvent
          ? (sourceOrEvent as DemoSource)
          : undefined;

      if (!backendEnabled || !isObsRunning) {
        showGlobalDialog("Graphic planner required running obs", "warning");
        return;
      }

      try {
        const activeSceneName = getActiveSceneName();
        sourceTransforms = { ...(plannerTransformsByScene[activeSceneName] ?? {}) };
        const { emitTo, listen } = await import('@tauri-apps/api/event');
        let plannerSceneResolution = sceneResolution;
        try {
          const liveResolution = await invoke<string>('obs_get_current_scene_resolution');
          if (typeof liveResolution === 'string' && /^\d+x\d+$/.test(liveResolution.trim())) {
            plannerSceneResolution = liveResolution.trim();
            sceneResolution = plannerSceneResolution;
          }
        } catch {
          // use current UI resolution fallback
        }
        await invoke('obs_set_graphic_planner_init', {
          init: {
            sources: sourcesList,
            scene_resolution: plannerSceneResolution,
            selected_source_id: source?.id ?? null,
            scene_name: activeSceneName
          }
        });
        await invoke('open_graphic_planner');
        let attempts = 0;
        const sendInit = () => {
          emitTo('source-transform', 'init', {
            sources: sourcesList,
            sceneResolution: plannerSceneResolution,
            sceneName: activeSceneName
          });
          attempts += 1;
          if (attempts < 5) {
            setTimeout(sendInit, 200);
          }
        };
        setTimeout(sendInit, 50);

        if (!saveTransformsUnlisten) {
          saveTransformsUnlisten = await listen('saveTransforms', async (event) => {
            await queueApplySourceTransforms(event.payload as Record<string, PlannerTransformPayload>);
          });
        }
        return;
      } catch (err) {
        console.warn('openGraphicPlanner: tauri window failed, falling back', err);
        // fall back to browser window
      }

      // Browser fallback
      const win = window.open(
        '/source-transform',
        'RevoStream - Graphic planner',
        'width=900,height=700,resizable,scrollbars=yes,status=0,menubar=0,toolbar=0,location=0'
      );
      if (!win) {
        showGlobalDialog('Could not open transform window (popup blocked?)', 'error');
        return;
      }
      transformWindow = win;
      const sendInit = () => {
        const activeSceneName = getActiveSceneName();
        win.postMessage({
          type: 'init',
          sources: sourcesList,
          sceneResolution,
          sceneName: activeSceneName
        }, '*');
      };
      win.addEventListener('load', sendInit);
      setTimeout(sendInit, 500);
    }

    onMount(() => {
      if (typeof window === 'undefined') return;

      window.addEventListener('message', (event) => {
        if (!event.data || typeof event.data !== 'object') return;
        if (event.data.type === 'requestInit') {
          if (event.source && typeof (event.source as Window).postMessage === 'function') {
            const activeSceneName = getActiveSceneName();
            (event.source as Window).postMessage({
              type: 'init',
              sources: sourcesList,
              sceneResolution,
              sceneName: activeSceneName
            }, '*');
          }
          return;
        }
        if (event.data.type === 'saveTransforms' && event.data.transforms) {
          void queueApplySourceTransforms(event.data.transforms as Record<string, PlannerTransformPayload>);
        }
      });

      (async () => {
        try {
          const { listen, emitTo } = await import('@tauri-apps/api/event');
          await listen('requestInit', () => {
            console.info('main: received requestInit');
            emitTo('source-transform', 'init', {
              sources: sourcesList,
              sceneResolution,
              sceneName: getActiveSceneName()
            });
          });
        } catch {
          // no-op
        }
      })();
    });

    $: activePreviewQuality = previewQuality;
    $: if (typeof window !== 'undefined') {
      localStorage.setItem('sceneResolution', sceneResolution);
    }
    $: if (typeof window !== 'undefined') {
      try {
        localStorage.setItem('sourcesList', JSON.stringify(sourcesList));
      } catch {
        // ignore storage errors
      }
    }

    const demoScenes: SceneInfo[] = [
      { name: "Scene 1", active: true, locked: false },
      { name: "Scene 2", active: false, locked: false }
    ];
    const demoSources: DemoSource[] = [
      { id: "cam", name: "Camera", visible: true, source_type: "Video", locked: false },
      { id: "screen", name: "Screen Capture", visible: true, source_type: "Capture", locked: false }
    ];

    let scenes: SceneInfo[] = [];
    let renamingScene: string | null = null;
    let renameSceneValue = "";
    let showAddScene = false;
    let newSceneName = "";
    let scenePlaceholder = "Scene 1";

    let sourcesList: DemoSource[] = [];
    const isMediaSourceType = (sourceType: string) =>
      (sourceType ?? "").trim().toLowerCase() === "ffmpeg_source";
    $: hasVisibleMediaSource = sourcesList.some(
      (source) => Boolean(source.visible) && isMediaSourceType(source.source_type ?? "")
    );
    let showAddSource = false;
    let newSourceType = sourceTypes[0].id;
    let externalSourceTypes: SourceTypeItem[] = [];

    let showSettings = false;
    let autorescaleInputs = false;
    let showEditSource = false;
    let editSource: DemoSource | null = null;
    let editName = "";
    let editType = "";
    let editParams: Record<string, string> = {};
    let sourcePropertySpecs: SourcePropertySpec[] = [];
    $: sourcePropertyKeys = new Set(sourcePropertySpecs.map((p) => p.key));
    $: sourcePropertyEntries = sourcePropertySpecs.map((spec) => [spec.key, editParams[spec.key] ?? ""] as [string, string]);
    $: dynamicParamEntries = Object.entries(editParams).filter(
      ([key]) => !(sourceParamSchemas[editType] ?? []).some((field) => field.key === key)
    );
    const isCropParamKey = (key: string) => {
      const normalized = key.toLowerCase().replace(/[^a-z0-9]+/g, "").trim();
      return normalized === "cropleft" || normalized === "cropright" || normalized === "croptop" || normalized === "cropbottom";
    };
    $: extraParamEntries = Object.entries(editParams).filter(([key]) =>
      isCropParamKey(key) ||
      (
        !(sourceParamSchemas[editType] ?? []).some((field) => field.key === key) &&
        !sourcePropertyKeys.has(key)
      )
    );

    let showSourceInfo = false;
    let selectedSource: DemoSource | null = null;
    let browserDockUrl = "https://google.com";
    let dockHostWebviewUrl = "";
    let dockPinnedSide: "left" | "right" | "bottom" | null = null;
    let dockPaneWidth = 420;
    let showDockHandle = false;
    let dockDragActive = false;
    let dockDropTarget: "left" | "right" | null = null;
    let dockDropCommitted = false;
    let dockFrameKey = 0;
    let dockFrameLoaded = false;
    let dockFrameBlocked = false;
    let dockFrameErrorMessage = "";
    let dockLoadTimer: ReturnType<typeof setTimeout> | null = null;
    let dockLibcefAvailable = false;
    let dockCefBridgeCompiled = false;
    let dockBodyEl: HTMLDivElement | null = null;
    let dockPaneEl: HTMLElement | null = null;
    let dockInlineWebview: Webview | null = null;
    let dockInlineWebviewUrl = "";
    let dockInlineWebviewActive = false;
    let dockEnsureInFlight = false;
    const dockUseSubWebview = false;
    let dockBoundsRaf: number | null = null;
    let dockCanvasEl: HTMLCanvasElement | null = null;
    let dockCanvasPollTimer: ReturnType<typeof setTimeout> | null = null;
    let dockCanvasInFlight = false;
    let dockResizing = false;
    let dockResizeSide: "left" | "right" | null = null;
    let dockResizeStartX = 0;
    let dockResizeStartWidth = 420;
    let dockHeaderMoveActive = false;
    let dockHeaderStartX = 0;
    let releaseObsAutoStarted = false;
    let showFiltersModal = false;
    let filtersTargetType: "source" | "scene" = "source";
    let filtersTargetId = "";
    let filtersTargetLabel = "";
    let filtersDraft: FilterItem[] = [];
    let filtersSnapshotBeforeOpen: FilterItem[] | null = null;
    let sourceFiltersLiveTimer: ReturnType<typeof setTimeout> | null = null;
    let sourceFiltersLiveInFlight = false;
    let sourceFiltersLiveQueued: { sourceId: string; filters: FilterItem[] } | null = null;
    let sourceFiltersMap: Record<string, FilterItem[]> = {};
    let audioSourceFiltersMap: Record<string, FilterItem[]> = {};
    let sceneFiltersMap: Record<string, FilterItem[]> = {};
    let showTextEditModal = false;
    let textEditSource: DemoSource | null = null;
    let textEditValue = "";
    let showQuickColorModal = false;
    let quickColorSource: DemoSource | null = null;
    let quickColorValue = "#ffffff";
    let quickColorParamKey = "color";
    let quickColorRecent: string[] = [];
    let quickColorHistoryLimit = 5;
    let showQuickDeviceModal = false;
    let quickDeviceSource: DemoSource | null = null;
    let quickDeviceOptions: { value: string; label: string }[] = [];
    let quickDeviceValue = "";
    let quickDeviceParamKey = "device";
    let quickDeviceMonitoring = "off";
    const quickMonitoringOptions = [
      { value: "off", label: "Off" },
      { value: "monitor_only", label: "Monitor only (silent output)" },
      { value: "on", label: "On" }
    ];
    const audioFilterPresetFields: Record<string, FilterFieldDef[]> = {
      noise_suppression: [
        { key: "suppression_level", label: "Suppression level (dB)", kind: "number", min: -60, max: 0, step: 1, defaultValue: "-30" }
      ],
      noise_gate: [
        { key: "open_threshold", label: "Open threshold (dB)", kind: "number", min: -96, max: 0, step: 1, defaultValue: "-26" },
        { key: "close_threshold", label: "Close threshold (dB)", kind: "number", min: -96, max: 0, step: 1, defaultValue: "-32" },
        { key: "attack_time", label: "Attack (ms)", kind: "number", min: 1, max: 500, step: 1, defaultValue: "25" },
        { key: "hold_time", label: "Hold (ms)", kind: "number", min: 1, max: 2000, step: 1, defaultValue: "200" },
        { key: "release_time", label: "Release (ms)", kind: "number", min: 1, max: 2000, step: 1, defaultValue: "150" }
      ],
      compressor: [
        { key: "ratio", label: "Ratio", kind: "number", min: 1, max: 32, step: 0.1, defaultValue: "10" },
        { key: "threshold", label: "Threshold (dB)", kind: "number", min: -60, max: 0, step: 1, defaultValue: "-18" },
        { key: "attack_time", label: "Attack (ms)", kind: "number", min: 1, max: 500, step: 1, defaultValue: "6" },
        { key: "release_time", label: "Release (ms)", kind: "number", min: 1, max: 2000, step: 1, defaultValue: "60" },
        { key: "output_gain", label: "Output gain (dB)", kind: "number", min: -30, max: 30, step: 0.1, defaultValue: "0" }
      ],
      limiter: [
        { key: "threshold", label: "Threshold (dB)", kind: "number", min: -60, max: 0, step: 1, defaultValue: "-6" },
        { key: "release_time", label: "Release (ms)", kind: "number", min: 1, max: 2000, step: 1, defaultValue: "60" }
      ],
      expander: [
        { key: "ratio", label: "Ratio", kind: "number", min: 1, max: 32, step: 0.1, defaultValue: "2" },
        { key: "threshold", label: "Threshold (dB)", kind: "number", min: -60, max: 0, step: 1, defaultValue: "-40" },
        { key: "attack_time", label: "Attack (ms)", kind: "number", min: 1, max: 500, step: 1, defaultValue: "10" },
        { key: "release_time", label: "Release (ms)", kind: "number", min: 1, max: 2000, step: 1, defaultValue: "80" },
        { key: "output_gain", label: "Output gain (dB)", kind: "number", min: -30, max: 30, step: 0.1, defaultValue: "0" }
      ],
      vst2: [
        { key: "plugin_path", label: "Plugin path", kind: "text", defaultValue: "" },
        { key: "bypass", label: "Bypass", kind: "checkbox", defaultValue: "false" }
      ],
      vst3: [
        { key: "plugin_path", label: "Plugin path", kind: "text", defaultValue: "" },
        { key: "bypass", label: "Bypass", kind: "checkbox", defaultValue: "false" }
      ]
    };
    const isTruthy = (value: string) => ["1", "true", "yes", "on"].includes(String(value ?? "").trim().toLowerCase());
    type AudioMixerItemState = {
      volumePercent: number;
      monitoring: string;
      balanceLeft: number;
      balanceRight: number;
      tracks: string[];
      locked: boolean;
      volumeMode: "percent" | "db";
      volumeDb: number;
    };
    let showAudioMixerModal = false;
    let audioMixerOrientation: "vertical" | "horizontal" = "horizontal";
    let showTemplatesFutureDialog = false;
    let audioMixerState: Record<string, AudioMixerItemState> = {};
    let audioMixerVolumeRealtimeTimers: Record<string, ReturnType<typeof setTimeout>> = {};
    let audioMixerMenu = { open: false, x: 0, y: 0, sourceId: null as string | null };
    let audioMixerAdvancedSourceId: string | null = null;
    let showAudioMixerAdvancedModal = false;
    let showAudioFiltersModal = false;
    let audioFiltersSourceId: string | null = null;
    let audioFiltersSourceLabel = "";
    let audioFiltersDraft: FilterItem[] = [];
    let audioFiltersSelectedId: string | null = null;
    let selectedAudioFilter: FilterItem | null = null;
    let selectedAudioFilterPresetFields: FilterFieldDef[] = [];
    let audioFiltersContextMenu = { open: false, x: 0, y: 0, filterId: null as string | null };
    let audioFiltersRenamingId: string | null = null;
    let audioFiltersRenameValue = "";
    let audioFiltersPreviewUrl = "";
    let audioFiltersPreviewRefreshTimer: ReturnType<typeof setTimeout> | null = null;
    let audioFilterNewKind = "noise_suppression";
    let audioMixerRenameSourceId: string | null = null;
    let audioMixerRenameValue = "";
    let audioMixerModalEl: HTMLDivElement | null = null;
    let audioMixerDragActive = false;
    let audioMixerDragStartX = 0;
    let audioMixerDragStartY = 0;
    let audioMixerDragOriginX = 0;
    let audioMixerDragOriginY = 0;
    let audioMixerDragX = 0;
    let audioMixerDragY = 0;
    let audioAdvancedModalEl: HTMLDivElement | null = null;
    let audioAdvancedDragActive = false;
    let audioAdvancedDragStartX = 0;
    let audioAdvancedDragStartY = 0;
    let audioAdvancedDragOriginX = 0;
    let audioAdvancedDragOriginY = 0;
    let audioAdvancedDragX = 0;
    let audioAdvancedDragY = 0;
    let audioFiltersModalEl: HTMLDivElement | null = null;
    let audioFiltersDragActive = false;
    let audioFiltersDragStartX = 0;
    let audioFiltersDragStartY = 0;
    let audioFiltersDragOriginX = 0;
    let audioFiltersDragOriginY = 0;
    let audioFiltersDragX = 0;
    let audioFiltersDragY = 0;
    let quickTextModalEl: HTMLDivElement | null = null;
    let quickTextDragActive = false;
    let quickTextDragStartX = 0;
    let quickTextDragStartY = 0;
    let quickTextDragOriginX = 0;
    let quickTextDragOriginY = 0;
    let quickTextDragX = 0;
    let quickTextDragY = 0;
    let openAdditionalSettingsInWindows = false;
    let allowDraggablePopups = false;
    let realtimeRefreshSources = false;
    let auxWindowMode: "plugins" | "edit-source" | "audio-mixer" | "audio-advanced" | "audio-filters" | null = null;
    let auxWindowSourceId = "";
    const auxWindowPendingLabels = new Set<string>();
    let editOriginalSnapshot: { id: string; name: string; source_type: string; params: Record<string, string> } | null = null;
    let editRealtimeTimer: ReturnType<typeof setTimeout> | null = null;
    let editRealtimeInFlight = false;
    let editRealtimeQueued = false;

    $: selectedAudioFilter = audioFiltersSelectedId
      ? audioFiltersDraft.find((f) => f.id === audioFiltersSelectedId) ?? null
      : null;
    $: selectedAudioFilterPresetFields = selectedAudioFilter
      ? (audioFilterPresetFields[selectedAudioFilter.kind] ?? [])
      : [];
    $: if (showAudioFiltersModal) {
      audioFiltersDraft;
      audioFiltersSelectedId;
      scheduleAudioFiltersPreviewRefresh();
    }

    let sourceMenu = { open: false, x: 0, y: 0, source: null as DemoSource | null };
    let sceneMenu = { open: false, x: 0, y: 0, scene: null as SceneInfo | null };
    let dockMenu = { open: false, x: 0, y: 0 };
    let appContextMenu = { open: false, x: 0, y: 0 };
    let previewMenu = { open: false, x: 0, y: 0 };
    const useCustomContextMenu = import.meta.env.MODE === "production";
    const canInspect = true;

    $: backendEnabled = !demoMode && tauriAvailable;
    $: if (openAdditionalSettingsInWindows) allowDraggablePopups = false;
    $: browserDockTitle = "Dock 1";
    $: dockCanvasRuntimeReady = tauriAvailable;
    $: dockCanvasUnavailableReason = "Renderer backend unavailable in this runtime.";
    $: dockEngineLabel = dockInlineWebviewActive
      ? "tauri-webview"
      : (dockCefBridgeCompiled ? "libcef" : "chromium");
    $: dockEngineActive = dockInlineWebviewActive || dockCefBridgeCompiled;

    const normalizeHttpUrl = (value: string | null | undefined): string | null => {
      const trimmed = (value ?? "").trim();
      if (!trimmed || !/^https?:\/\//i.test(trimmed)) return null;
      return trimmed;
    };

    const browserSourceUrl = (source: DemoSource | null | undefined): string | null => {
      if (!source || !isBrowserSource(source)) return null;
      return normalizeHttpUrl(source.params?.url);
    };

    const resolveDockUrl = (): string | null => {
      const current = normalizeHttpUrl(browserDockUrl);
      if (current) return current;
      for (const source of sourcesList) {
        const candidate = browserSourceUrl(source);
        if (candidate) return candidate;
      }
      return null;
    };

    const syncDockUrl = () => {
      const resolved = resolveDockUrl();
      if (!resolved) return browserDockUrl;
      if (resolved !== browserDockUrl) {
        browserDockUrl = resolved;
        if (typeof window !== "undefined") {
          localStorage.setItem("browserDockUrl", browserDockUrl);
        }
      }
      return resolved;
    };

    const buildDockHostWebviewUrl = (target: string) => {
      if (typeof window === "undefined") return "/webview";
      const origin = window.location.origin;
      const normalizedTarget = normalizeHttpUrl(target) ?? "https://google.com";
      return `${origin}/webview?target=${encodeURIComponent(normalizedTarget)}`;
    };

    const clampDockWidth = (value: number) => {
      const max = typeof window !== "undefined"
        ? Math.max(300, Math.floor(window.innerWidth * 0.62))
        : 760;
      return Math.max(280, Math.min(max, Math.floor(value)));
    };

    const saveDockWidth = () => {
      if (typeof window === "undefined") return;
      localStorage.setItem("dockPaneWidth", String(dockPaneWidth));
    };

    const startDockResize = (event: MouseEvent, side: "left" | "right") => {
      event.preventDefault();
      event.stopPropagation();
      dockResizing = true;
      dockResizeSide = side;
      dockResizeStartX = event.clientX;
      dockResizeStartWidth = dockPaneWidth;
    };

    const handleDockResizeMove = (event: MouseEvent) => {
      if (!dockResizing || !dockResizeSide) return;
      const delta = event.clientX - dockResizeStartX;
      const next = dockResizeSide === "left"
        ? dockResizeStartWidth + delta
        : dockResizeStartWidth - delta;
      dockPaneWidth = clampDockWidth(next);
    };

    const stopDockResize = () => {
      if (!dockResizing) return;
      dockResizing = false;
      dockResizeSide = null;
      saveDockWidth();
    };

    const startDockHeaderMove = (event: MouseEvent) => {
      const target = event.target as HTMLElement | null;
      if (target?.closest("button")) return;
      dockHeaderMoveActive = true;
      dockHeaderStartX = event.clientX;
    };

    const handleDockHeaderMove = (event: MouseEvent) => {
      if (!dockHeaderMoveActive || !dockPinnedSide) return;
      const delta = event.clientX - dockHeaderStartX;
      if (dockPinnedSide === "left" && delta > 90) {
        pinDockPane("right");
        dockHeaderMoveActive = false;
      } else if (dockPinnedSide === "right" && delta < -90) {
        pinDockPane("left");
        dockHeaderMoveActive = false;
      }
    };

    const stopDockHeaderMove = () => {
      dockHeaderMoveActive = false;
    };

    $: dockHostWebviewUrl = buildDockHostWebviewUrl(syncDockUrl());

    const paintDockCanvasPlaceholder = (message: string) => {
      if (!dockCanvasEl) return;
      const canvas = dockCanvasEl;
      const width = Math.max(320, Math.floor(canvas.clientWidth || 640));
      const height = Math.max(180, Math.floor(canvas.clientHeight || 360));
      canvas.width = width;
      canvas.height = height;
      const ctx = canvas.getContext("2d");
      if (!ctx) return;
      ctx.fillStyle = "#0f1218";
      ctx.fillRect(0, 0, width, height);
      ctx.fillStyle = "#1f2937";
      for (let y = 0; y < height; y += 24) {
        for (let x = 0; x < width; x += 24) {
          if ((((x + y) / 24) & 1) === 0) {
            ctx.fillRect(x, y, 24, 24);
          }
        }
      }
      ctx.fillStyle = "#d1d5db";
      ctx.font = "600 13px system-ui";
      ctx.fillText(message, 14, Math.max(24, height - 16));
    };

    const paintDockCanvasIdle = () => {
      if (!dockCanvasEl) return;
      const canvas = dockCanvasEl;
      const width = Math.max(320, Math.floor(canvas.clientWidth || 640));
      const height = Math.max(180, Math.floor(canvas.clientHeight || 360));
      canvas.width = width;
      canvas.height = height;
      const ctx = canvas.getContext("2d");
      if (!ctx) return;
      ctx.fillStyle = "#0f1218";
      ctx.fillRect(0, 0, width, height);
      ctx.fillStyle = "#171b23";
      for (let y = 0; y < height; y += 24) {
        for (let x = 0; x < width; x += 24) {
          if ((((x + y) / 24) & 1) === 0) {
            ctx.fillRect(x, y, 24, 24);
          }
        }
      }
    };

    const stopDockCanvasLoop = () => {
      if (dockCanvasPollTimer) {
        clearTimeout(dockCanvasPollTimer);
        dockCanvasPollTimer = null;
      }
    };

    const disposeDockInlineWebview = async () => {
      if (dockBoundsRaf !== null) {
        cancelAnimationFrame(dockBoundsRaf);
        dockBoundsRaf = null;
      }
      if (dockInlineWebview) {
        try {
          await dockInlineWebview.close();
        } catch {
          // ignore close errors
        }
      }
      dockInlineWebview = null;
      dockInlineWebviewUrl = "";
      dockInlineWebviewActive = false;
      if (!dockPinnedSide) {
        dockPaneEl = null;
      }
    };

    const syncDockInlineWebviewBounds = async () => {
      if (!dockInlineWebview || !dockPaneEl) return;
      const paneRect = dockPaneEl.getBoundingClientRect();
      const headerEl = dockPaneEl.querySelector(".dock-compact-header") as HTMLElement | null;
      const headerHeight = Math.max(0, Math.floor(headerEl?.getBoundingClientRect().height ?? 34));
      const x = Math.floor(paneRect.left);
      const y = Math.floor(paneRect.top + headerHeight);
      const width = Math.max(1, Math.floor(paneRect.width));
      const height = Math.max(1, Math.floor(paneRect.height - headerHeight));
      if (width < 2 || height < 2) return;

      for (let attempt = 0; attempt < 2; attempt += 1) {
        try {
          await dockInlineWebview.setPosition(new LogicalPosition(x, y));
          await dockInlineWebview.setSize(new LogicalSize(width, height));
          return;
        } catch (err) {
          const message = String(err);
          if (/webview not found/i.test(message) && attempt === 0) {
            await new Promise<void>((resolve) => setTimeout(resolve, 120));
            continue;
          }
          throw err;
        }
      }
    };

    const requestDockInlineBoundsSync = () => {
      if (typeof window === "undefined") return;
      if (dockBoundsRaf !== null) {
        cancelAnimationFrame(dockBoundsRaf);
      }
      dockBoundsRaf = requestAnimationFrame(() => {
        dockBoundsRaf = null;
        void syncDockInlineWebviewBounds();
      });
    };

    const waitForDockInlineWebviewReady = async (webview: Webview, timeoutMs = 2000) => {
      await Promise.race([
        new Promise<void>((resolve, reject) => {
          let done = false;
          void webview.once("tauri://created", () => {
            if (done) return;
            done = true;
            resolve();
          });
          void webview.once("tauri://error", (event) => {
            if (done) return;
            done = true;
            reject(new Error(String(event?.payload ?? "tauri://error")));
          });
        }),
        new Promise<void>((_, reject) => {
          setTimeout(() => reject(new Error("dock inline webview create timeout")), timeoutMs);
        })
      ]);
    };

    const ensureDockInlineWebview = async (forceRecreate = false) => {
      if (!dockUseSubWebview) {
        await disposeDockInlineWebview();
        return;
      }
      if (dockEnsureInFlight) return;
      dockEnsureInFlight = true;
      if (!tauriAvailable || !dockPinnedSide || !dockPaneEl) {
        await disposeDockInlineWebview();
        dockEnsureInFlight = false;
        return;
      }

      const targetUrl = syncDockUrl();
      if (!/^https?:\/\//i.test(targetUrl ?? "")) {
        dockInlineWebviewActive = false;
        dockFrameLoaded = false;
        dockFrameBlocked = true;
        dockFrameErrorMessage = "Dock URL must start with http:// or https://";
        dockEnsureInFlight = false;
        return;
      }

      const loadUrl = targetUrl;

      try {
        let recovered = false;
        const urlChanged = dockInlineWebviewUrl !== loadUrl;
        if (forceRecreate || !dockInlineWebview || urlChanged) {
          await disposeDockInlineWebview();
          const currentWindow = getCurrentWindow();
          const label = `dock-inline-${Date.now().toString(36)}`;
          dockInlineWebview = new Webview(currentWindow, label, {
            url: loadUrl,
            x: 0,
            y: 0,
            width: 16,
            height: 16,
            focus: false
          });
          await dockInlineWebview.setAutoResize(false);
          dockInlineWebviewUrl = loadUrl;
          await waitForDockInlineWebviewReady(dockInlineWebview);
        }

        await syncDockInlineWebviewBounds();
        dockInlineWebviewActive = true;
        dockFrameLoaded = true;
        dockFrameBlocked = false;
        dockFrameErrorMessage = "";
      } catch (err) {
        let message = String(err);
        if (/webview not found|not supported|failed to create webview|set_webview_position/i.test(message)) {
          try {
            await disposeDockInlineWebview();
            await new Promise<void>((resolve) => setTimeout(resolve, 140));
            const currentWindow = getCurrentWindow();
            const label = `dock-inline-recover-${Date.now().toString(36)}`;
            dockInlineWebview = new Webview(currentWindow, label, {
              url: loadUrl,
              x: 0,
              y: 0,
              width: 16,
              height: 16,
              focus: false
            });
            await dockInlineWebview.setAutoResize(false);
            dockInlineWebviewUrl = loadUrl;
            await waitForDockInlineWebviewReady(dockInlineWebview);
            await syncDockInlineWebviewBounds();
            dockInlineWebviewActive = true;
            dockFrameLoaded = true;
            dockFrameBlocked = false;
            dockFrameErrorMessage = "";
            dockEnsureInFlight = false;
            return;
          } catch (recoverErr) {
            message = String(recoverErr);
          }
        }
        dockInlineWebviewActive = false;
        dockFrameLoaded = false;
        dockFrameBlocked = true;
        dockFrameErrorMessage = message;
      } finally {
        dockEnsureInFlight = false;
      }
    };

    $: if (tauriAvailable && dockPinnedSide && dockPaneEl) {
      if (!dockInlineWebviewActive) {
        void ensureDockInlineWebview(false);
      } else {
        requestDockInlineBoundsSync();
      }
    }

    const scheduleDockCanvasFrame = (delay = 0) => {
      stopDockCanvasLoop();
      dockCanvasPollTimer = setTimeout(() => {
        void renderDockCanvasFrame();
      }, delay);
    };

    const renderDockCanvasFrame = async () => {
      if (!dockUseSubWebview) return;
      if (tauriAvailable) {
        await ensureDockInlineWebview(false);
        return;
      }
      if (!dockPinnedSide || !dockCanvasEl) return;
      if (dockCanvasInFlight) return;

      if (!tauriAvailable) {
        paintDockCanvasIdle();
        scheduleDockCanvasFrame(1200);
        return;
      }

      const canvas = dockCanvasEl;
      const width = Math.max(320, Math.floor(canvas.clientWidth || 640));
      const height = Math.max(180, Math.floor(canvas.clientHeight || 360));
      const targetUrl = syncDockUrl();
      dockCanvasInFlight = true;
      try {
        const b64 = await invoke<string>("cef_dock_render_frame", {
          url: targetUrl,
          width,
          height
        });
        const img = new Image();
        img.src = `data:image/png;base64,${b64}`;
        await img.decode();
        canvas.width = width;
        canvas.height = height;
        const ctx = canvas.getContext("2d");
        if (ctx) {
          ctx.clearRect(0, 0, width, height);
          ctx.drawImage(img, 0, 0, width, height);
          dockFrameLoaded = true;
          dockFrameBlocked = false;
          dockFrameErrorMessage = "";
        }
      } catch (err) {
        const message = String(err);
        const bridgeUnavailable = /cef canvas renderer unavailable|feature 'cef'|bridge unavailable/i.test(message);
        if (bridgeUnavailable) {
          dockCefBridgeCompiled = false;
          dockFrameLoaded = false;
          dockFrameBlocked = false;
          dockFrameErrorMessage = "";
          paintDockCanvasIdle();
        } else {
          dockFrameLoaded = false;
          dockFrameBlocked = true;
          dockFrameErrorMessage = /no chromium binary found/i.test(message)
            ? "Chromium binary not found. Install chromium, brave-browser, or google-chrome."
            : message;
          paintDockCanvasPlaceholder(`CEF frame error: ${message}`);
        }
      } finally {
        dockCanvasInFlight = false;
      }
      scheduleDockCanvasFrame(dockCefBridgeCompiled ? 120 : 1800);
    };

    const refreshDockPane = () => {
      dockFrameLoaded = false;
      dockFrameBlocked = false;
      dockFrameErrorMessage = "";
      if (!dockUseSubWebview) {
        dockFrameKey += 1;
        return;
      }
      if (tauriAvailable) {
        stopDockCanvasLoop();
        void ensureDockInlineWebview(true);
        return;
      }
      if (dockLoadTimer) {
        clearTimeout(dockLoadTimer);
      }
      dockLoadTimer = setTimeout(() => {
        if (!dockFrameLoaded && dockPinnedSide) {
          dockFrameBlocked = true;
          if (!dockFrameErrorMessage) {
            dockFrameErrorMessage = "Renderer timeout. Try refresh or open source URL in separate window.";
          }
        }
      }, 2800);
      dockFrameKey += 1;
      scheduleDockCanvasFrame(20);
    };

    const handleDockRefreshClick = async () => {
      syncDockUrl();
      if (dockPinnedSide) {
        refreshDockPane();
        return;
      }
      showDockHandle = true;
      pinDockPane("right");
    };

    const refreshDockCapability = async () => {
      if (!tauriAvailable) {
        dockLibcefAvailable = false;
        dockCefBridgeCompiled = false;
        return;
      }
      try {
        const [plugins, bridge] = await Promise.all([
          invoke<PluginInfo[]>("plugins_list"),
          invoke<CefBridgeInfo>("cef_bridge_info")
        ]);
        const hasObsBrowser = plugins.some((p) => {
          const m = (p.module_name ?? "").toLowerCase();
          return m === "obs-browser" || m === "libobs-browser";
        });
        dockLibcefAvailable = hasObsBrowser;
        dockCefBridgeCompiled = Boolean(bridge?.compiled);
      } catch {
        dockLibcefAvailable = false;
        dockCefBridgeCompiled = false;
      }
    };

    const pinDockPane = (side: "left" | "right" | "bottom") => {
      dockPinnedSide = side;
      refreshDockPane();
    };

    const undockDockPane = async () => {
      if (!isReleaseBuild) return;
      await disposeDockInlineWebview();
      if (tauriAvailable) {
        try {
          await invoke<string>("open_browser_dock", { url: dockHostWebviewUrl });
        } catch (err) {
          showGlobalDialog(String(err), "error");
        }
      }
      dockPinnedSide = null;
      dockDragActive = false;
      dockDropTarget = null;
      dockDropCommitted = false;
      stopDockCanvasLoop();
    };

    const startDockHandleDrag = (event: DragEvent) => {
      dockDragActive = true;
      dockDropTarget = null;
      dockDropCommitted = false;
      if (event.dataTransfer) {
        event.dataTransfer.effectAllowed = "move";
        event.dataTransfer.setData("text/plain", "browser-dock");
      }
    };

    const handleDockZoneDragOver = (event: DragEvent, side: "left" | "right") => {
      event.preventDefault();
      dockDropTarget = side;
      if (event.dataTransfer) {
        event.dataTransfer.dropEffect = "move";
      }
    };

    const handleDockZoneDrop = (event: DragEvent, side: "left" | "right") => {
      event.preventDefault();
      dockDropCommitted = true;
      void attachDockToSide(side);
      dockDropTarget = null;
      dockDragActive = false;
    };

    const attachDockToSide = async (side: "left" | "right" | "bottom") => {
      if (tauriAvailable) {
        try {
          const state = await invoke<{ is_open: boolean; url: string | null }>("browser_dock_state");
          const dockUrl = (state?.url ?? "").trim();
          if (dockUrl && /^https?:\/\//i.test(dockUrl)) {
            browserDockUrl = dockUrl;
            localStorage.setItem("browserDockUrl", browserDockUrl);
          }
          if (state?.is_open) {
            await invoke<void>("close_browser_dock");
          }
        } catch (err) {
          showGlobalDialog(String(err), "error");
        }
      }
      pinDockPane(side);
    };

    const handleDockHandleDragEnd = () => {
      dockDropTarget = null;
      dockDragActive = false;
      dockDropCommitted = false;
    };

    const openAuxSettingsWindow = async (
      mode: "plugins" | "edit-source" | "audio-mixer" | "audio-advanced" | "audio-filters",
      sourceId?: string
    ) => {
      if (typeof window === "undefined") return;
      const params = new URLSearchParams(window.location.search);
      params.set("auxWindow", mode);
      if (sourceId) {
        params.set("sourceId", sourceId);
      } else {
        params.delete("sourceId");
      }
      const url = `${window.location.pathname}?${params.toString()}`;
      const windowLabel = mode === "plugins"
        ? "revo-plugins-window"
        : mode === "edit-source"
          ? "revo-edit-source-window"
          : mode === "audio-mixer"
            ? "revo-audio-mixer-window"
            : mode === "audio-advanced"
              ? "revo-audio-advanced-window"
              : "revo-audio-filters-window";
      const windowTitle = mode === "plugins"
        ? "RevoStream - Plugins"
        : mode === "edit-source"
          ? "RevoStream - Edit source"
          : mode === "audio-mixer"
            ? "RevoStream - Audio mixer"
            : mode === "audio-advanced"
              ? "RevoStream - Advanced audio properties"
              : "RevoStream - Audio filters";
      if (auxWindowPendingLabels.has(windowLabel)) return;

      if (tauriAvailable) {
        try {
          auxWindowPendingLabels.add(windowLabel);
          const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
          const existing = await WebviewWindow.getByLabel(windowLabel);
          if (existing) {
            if (mode === "edit-source" || mode === "audio-advanced" || mode === "audio-filters") {
              try {
                await existing.close();
              } catch {
                try {
                  await existing.setFocus();
                } catch {
                  // no-op
                }
                auxWindowPendingLabels.delete(windowLabel);
                return;
              }
            } else {
              try {
                await existing.setFocus();
              } catch {
                // no-op
              }
              auxWindowPendingLabels.delete(windowLabel);
              return;
            }
          }
          const child = new WebviewWindow(windowLabel, {
            url,
            title: windowTitle,
            width: 1100,
            height: 820,
            resizable: true,
            center: true
          });
          child.once("tauri://created", () => {
            auxWindowPendingLabels.delete(windowLabel);
          });
          child.once("tauri://error", (e) => {
            auxWindowPendingLabels.delete(windowLabel);
            showGlobalDialog(`Window open failed: ${String((e as { payload?: unknown })?.payload ?? "unknown error")}`, "error", 2600);
          });
          return;
        } catch (err) {
          auxWindowPendingLabels.delete(windowLabel);
          console.warn("openAuxSettingsWindow: tauri window failed, falling back", err);
        }
      }

      const popup = window.open(
        url,
        windowLabel,
        "width=1100,height=820,resizable,scrollbars=yes,status=0,menubar=0,toolbar=0,location=0"
      );
      if (!popup) {
        showGlobalDialog("Could not open window (popup blocked?)", "error", 2400);
      }
    };

    const handleDockFrameLoad = () => {
      // no-op in canvas path (kept for compatibility)
    };

    function toggleDemo(event: CustomEvent<{ checked: boolean }>) {
      demoMode = event.detail.checked;
      showGlobalDialog(demoMode ? "Demo mode enabled" : "Demo mode disabled", "info");
      if (demoMode) {
        isRecording = false;
        isStreaming = false;
        isObsRunning = false;
        scenes = [...demoScenes];
        sourcesList = [...demoSources];
        stopPreviewLoop();
        stopWhepPreview();
        if (webrtcWhipStarted && tauriAvailable) {
          void invoke<string>("obs_stop_streaming").finally(() => {
            webrtcWhipStarted = false;
          });
        }
      } else {
        scenes = [];
        sourcesList = [];
        if (tauriAvailable) {
          void loadScenes();
          void loadSources();
        }
      }
    }

    async function loadSources() {
      if (!backendEnabled) return;
      try {
        const lockedById = new Map(sourcesList.map((s) => [s.id, Boolean(s.locked)]));
        const list = await invoke<DemoSource[]>("obs_list_sources");
        sourcesList = list.map((source) => ({
          ...source,
          locked: lockedById.get(source.id) ?? Boolean(source.locked)
        }));
      } catch (err) {
        showGlobalDialog(String(err), "error");
      }
    }

    async function loadScenes() {
      if (!backendEnabled) return;
      try {
        const list = await invoke<SceneInfo[]>("obs_list_scenes");
        scenes = list;
      } catch (err) {
        showGlobalDialog(String(err), "error");
      }
    }

    async function loadVersion() {
      try {
        version = await invoke<string>("obs_version");
        tauriAvailable = true;
      } catch {
        tauriAvailable = false;
      }
    }

    async function checkInitialized() {
      if (!tauriAvailable) return;
      try {
        isObsRunning = await invoke<boolean>("obs_is_initialized");
      } catch {
        isObsRunning = false;
      }
    }

    async function forcePreviewResolution() {
      sceneResolution = "1920x1080";
      previewRenderIndex = previewRenderOptions.findIndex(opt => opt.value === "1920x1080");
      if (backendEnabled) {
        try {
          await invoke<string>("obs_set_scene_resolution", { resolution: "1920x1080" });
          await refreshPreviewAfterResolutionChange();
        } catch (err) {
          showGlobalDialog(String(err), "error");
        }
      }
    }

    function parseSceneResolution(value: string) {
      const match = value.trim().match(/^(\d+)x(\d+)$/i);
      if (!match) return null;
      const width = Number(match[1]);
      const height = Number(match[2]);
      if (!Number.isFinite(width) || !Number.isFinite(height)) return null;
      if (width <= 0 || height <= 0) return null;
      return { width, height };
    }

    $: previewRenderResolution = previewRenderOptions[previewRenderIndex]?.value ?? "1920x1080";

    $: {
      const idx = previewRenderOptions.findIndex((option) => option.value === sceneResolution);
      if (idx !== -1 && idx !== previewRenderIndex) {
        previewRenderIndex = idx;
      }
    }

    function applyPreviewRenderResolution(index: number) {
      const option = previewRenderOptions[index];
      if (!option) return;
      sceneResolution = option.value;
      if (!backendEnabled) return;
      if (previewRenderTimer) {
        clearTimeout(previewRenderTimer);
      }
      previewRenderTimer = setTimeout(async () => {
        previewRenderTimer = null;
        try {
          await invoke<string>("obs_set_scene_resolution", { resolution: option.value });
          if (autorescaleInputs) {
            await refreshPreviewAfterResolutionChange();
          } else {
            await refreshPreview();
          }
        } catch (err) {
          showGlobalDialog(String(err), "error");
        }
      }, 150);
    }

    function getBasePreviewSize() {
      const renderSize = parseSceneResolution(previewRenderResolution);
      if (renderSize) return renderSize;
      const sceneSize = parseSceneResolution(sceneResolution);
      if (sceneSize) return sceneSize;
      return { width: 1280, height: 720 };
    }

    function getRenderScale() {
      const sceneSize = parseSceneResolution(sceneResolution);
      const renderSize = parseSceneResolution(previewRenderResolution);
      if (!sceneSize || !renderSize) return 1;
      const scaleX = renderSize.width / sceneSize.width;
      const scaleY = renderSize.height / sceneSize.height;
      const scale = Math.min(scaleX, scaleY);
      if (!Number.isFinite(scale) || scale <= 0) return 1;
      return Math.min(1, scale);
    }

    function getPreviewScale() {
      if (previewQuality === "very_low") return 0.3;
      if (previewQuality === "low") return 0.5;
      if (previewQuality === "medium") return 0.8;
      return 1;
    }

    function getPreviewScaleFinal() {
      return getPreviewScale() * getRenderScale();
    }

    function getPreviewAspect() {
      const sceneSize = parseSceneResolution(sceneResolution);
      if (!sceneSize) return "16 / 9";
      return `${sceneSize.width} / ${sceneSize.height}`;
    }

    function getPreviewSize() {
      const base = getBasePreviewSize();
      if (!isResizing) return base;
      const scale = 0.3;
      return {
        width: Math.max(160, Math.round(base.width * scale)),
        height: Math.max(90, Math.round(base.height * scale))
      };
    }

    function getPreviewIntervalMs() {
      if (realtimeRefresh) {
        if (hasVisibleMediaSource) return 150;
        return 0;
      }
      if (isResizing) return 400;
      return 700;
    }

    function useRafPreviewLoop() {
      return realtimeRefresh && !hasVisibleMediaSource;
    }

    const setRealtimeRefresh = (enabled: boolean) => {
      realtimeRefresh = enabled;
      if (!backendEnabled || !isObsRunning || webrtcActive || demoMode || document.hidden) return;
      stopPreviewLoop();
      startPreviewLoop();
    };

    function requestPreviewUpdate() {
      if (!backendEnabled || !isObsRunning || webrtcActive || demoMode) return;
      previewDirty = true;
      if (hasVisibleMediaSource) {
        if (!previewInterval && previewAnimationFrame === null) {
          startPreviewLoop();
        }
        return;
      }
      void refreshPreview(true);
    }

    async function refreshPreviewAfterResolutionChange() {
      if (!backendEnabled || !isObsRunning || demoMode) return;
      if (webrtcActive || webrtcPc) {
        stopWhepPreview();
        await startWhepPreview();
        return;
      }
      previewDirty = true;
      await refreshPreview(true);
      startPreviewLoop();
    }

    async function refreshPreview(force = false) {
      if (!backendEnabled || !isObsRunning || document.hidden) return;
      if (!force && !previewDirty) return;

      if (previewInFlight) {
        previewPendingRequest = true;
        return;
      }

      if (!force && hasVisibleMediaSource) {
        const minFrameGapMs = 120;
        const elapsed = Date.now() - previewLastFrameAt;
        if (elapsed > 0 && elapsed < minFrameGapMs) {
          previewPendingRequest = true;
          return;
        }
      }

      previewInFlight = true;
      try {
        const now = Date.now();
        if (now - previewResolutionSyncAt >= 2500) {
          previewResolutionSyncAt = now;
          try {
            const liveResolution = await invoke<string>("obs_get_current_scene_resolution");
            const trimmed = typeof liveResolution === "string" ? liveResolution.trim() : "";
            if (/^\d+x\d+$/.test(trimmed) && trimmed !== sceneResolution) {
              sceneResolution = trimmed;
            }
          } catch {
            // ignore sync errors
          }
        }
        const { width, height } = getPreviewSize();
        const screenshot = await invoke<string>("obs_take_screenshot", { width, height });
        if (screenshot.startsWith("data:")) {
          previewUrl = screenshot;
        } else {
          previewUrl = `${convertFileSrc(screenshot)}?t=${Date.now()}`;
        }
        previewLastFrameAt = Date.now();
        previewDirty = false;
      } catch (err) {
        showGlobalDialog(String(err), "error");
      } finally {
        previewInFlight = false;
        if (previewPendingRequest) {
          previewPendingRequest = false;
          previewDirty = true;
          const delay = hasVisibleMediaSource ? 80 : 0;
          setTimeout(() => {
            void refreshPreview(false);
          }, delay);
        }
      }
    }

    async function ensureWhipStreaming() {
      if (!backendEnabled || !whipUrl.trim() || webrtcWhipStarted) return;
      try {
        await invoke<string>("obs_start_streaming", { streamUrl: whipUrl.trim() });
        isStreaming = true;
        webrtcWhipStarted = true;
      } catch (err) {
        webrtcError = String(err);
      }
    }

    async function startWhepPreview() {
      if (!backendEnabled || !isObsRunning || webrtcPc || !whepUrl.trim()) return false;
      webrtcError = "";
      try {
        if (typeof RTCPeerConnection === "undefined") {
          webrtcError = "WebRTC not available in this runtime";
          console.warn(webrtcError);
          return false;
        }
        await ensureWhipStreaming();
        const pc = new RTCPeerConnection();
        webrtcPc = pc;
        pc.addTransceiver("video", { direction: "recvonly" });
        pc.ontrack = (event) => {
          if (previewVideo) {
            previewVideo.srcObject = event.streams[0];
            webrtcActive = true;
          }
        };
        const offer = await pc.createOffer({ offerToReceiveVideo: true, offerToReceiveAudio: false });
        await pc.setLocalDescription(offer);
        const response = await fetch(whepUrl, {
          method: "POST",
          headers: { "Content-Type": "application/sdp" },
          body: offer.sdp ?? ""
        });
        if (!response.ok) {
          throw new Error(`WHEP ${response.status}`);
        }
        const answerSdp = await response.text();
        await pc.setRemoteDescription({ type: "answer", sdp: answerSdp });
        webrtcRetryAttempt = 0;
        return true;
      } catch (err) {
        webrtcError = String(err);
        console.warn("WHEP preview error:", err);
        stopWhepPreview();
        scheduleWhepRetry();
        return false;
      }
    }

    function stopWhepPreview() {
      webrtcActive = false;
      if (previewVideo) {
        previewVideo.srcObject = null;
      }
      if (webrtcPc) {
        webrtcPc.getSenders().forEach((s) => s.track?.stop());
        webrtcPc.close();
        webrtcPc = null;
      }
      if (webrtcRetryTimer) {
        clearTimeout(webrtcRetryTimer);
        webrtcRetryTimer = null;
      }
      webrtcRetryAttempt = 0;
    }

    function scheduleWhepRetry() {
      if (!autoRetryPreview || !backendEnabled || webrtcRetryTimer) return;
      const delay = Math.min(10000, 500 * Math.pow(2, webrtcRetryAttempt));
      webrtcRetryAttempt += 1;
      webrtcRetryTimer = setTimeout(async () => {
        webrtcRetryTimer = null;
        if (!backendEnabled || demoMode || !isObsRunning) return;
        const ok = await startWhepPreview();
        if (!ok) {
          scheduleWhepRetry();
        }
      }, delay);
    }

    function startPreviewLoop() {
      if (previewInterval || previewAnimationFrame !== null) return;
      const tick = async () => {
        previewInterval = null;
        await refreshPreview();
        const canContinue =
          backendEnabled &&
          isObsRunning &&
          !webrtcActive &&
          !demoMode &&
          !document.hidden;
        if (!canContinue) return;

        if (useRafPreviewLoop()) {
          previewDirty = true;
          previewAnimationFrame = requestAnimationFrame(() => {
            previewAnimationFrame = null;
            void tick();
          });
          return;
        }

        previewInterval = setTimeout(tick, getPreviewIntervalMs());
      };
      previewDirty = true;
      if (useRafPreviewLoop()) {
        previewAnimationFrame = requestAnimationFrame(() => {
          previewAnimationFrame = null;
          void tick();
        });
      } else {
        previewInterval = setTimeout(tick, 0);
      }
    }

    function stopPreviewLoop() {
      if (previewInterval) {
        clearTimeout(previewInterval);
        previewInterval = null;
      }
      if (previewAnimationFrame !== null) {
        cancelAnimationFrame(previewAnimationFrame);
        previewAnimationFrame = null;
      }
      previewUrl = "";
      previewDirty = true;
    }

    function showCannotRemoveLastSceneDialog() {
      showCannotRemoveLastScene = true;
      setTimeout(() => { showCannotRemoveLastScene = false; }, 3200);
    }

    const applyLoadedSettings = (settings: PersistedSettings) => {
      rootDir = settings.root_dir ?? rootDir;
      if (typeof settings.record_path === "string") recordPath = settings.record_path;
      if (typeof settings.stream_url === "string") streamUrl = settings.stream_url;
      if (typeof settings.stream_key === "string") streamKey = settings.stream_key;
      if (typeof settings.preview_quality === "string") previewQuality = settings.preview_quality;
      if (typeof settings.encoder_preference === "string") encoderPreference = settings.encoder_preference;
      if (typeof settings.scene_resolution === "string") sceneResolution = settings.scene_resolution;
      if (typeof settings.whep_url === "string") whepUrl = settings.whep_url;
      if (typeof settings.whip_url === "string") whipUrl = settings.whip_url;
      if (typeof settings.auto_retry_preview === "boolean") autoRetryPreview = settings.auto_retry_preview;
      if (typeof settings.autorescale_inputs === "boolean") autorescaleInputs = settings.autorescale_inputs;

      if (typeof settings.active_profile === "string" && settings.active_profile.trim()) {
        selectedProfileName = settings.active_profile.trim();
      }

      currentUiProfile = settings.ui_profile ?? null;
      const general = (settings.ui_profile?.general as Record<string, unknown> | undefined) ?? {};
      openAdditionalSettingsInWindows = Boolean(general.openAdditionalSettingsInWindows);
      allowDraggablePopups = Boolean(general.allowDraggablePopups);
      quickColorHistoryLimit = Math.max(1, Math.min(15, Number(general.quickColorHistoryLimit) || 5));
      realtimeRefreshSources = Boolean(general.realtimeRefreshSources);

      const a11y = (settings.ui_profile?.accessibility as AccessibilityUiProfile | undefined) ?? {};
      accessibilityHighContrast = Boolean(a11y.accessibilityHighContrast);
      accessibilityReduceMotion = Boolean(a11y.accessibilityReduceMotion);
      accessibilityFocusIndicators = a11y.accessibilityFocusIndicators !== false;
      accessibilityUiScale = String(a11y.accessibilityUiScale ?? "100");
      accessibilityFontSize = String(a11y.accessibilityFontSize ?? "100");
      accessibilityFontFamily = String(a11y.accessibilityFontFamily ?? "system");
      accessibilityColorVision = String(a11y.accessibilityColorVision ?? "none");

      applyAccessibilityProfile(settings.ui_profile ?? null);
      normalizeTransitionsFromProfile(settings.ui_profile ?? null);
      void applyLookProfile(settings.ui_profile ?? null);
    };

    const captureCurrentSettingsSnapshot = (): PersistedSettings => ({
      root_dir: rootDir,
      record_path: recordPath,
      stream_url: streamUrl,
      stream_key: streamKey,
      preview_quality: previewQuality,
      encoder_preference: encoderPreference,
      scene_resolution: sceneResolution,
      whep_url: whepUrl,
      whip_url: whipUrl,
      auto_retry_preview: autoRetryPreview,
      autorescale_inputs: autorescaleInputs,
      ui_profile: currentUiProfile,
      active_profile: selectedProfileName
    });

    const refreshLookThemeFromPersistedSettings = async () => {
      if (!tauriAvailable) return;
      try {
        const settings = await invoke<PersistedSettings>("settings_get");
        currentUiProfile = settings.ui_profile ?? null;
        await applyLookProfile(settings.ui_profile ?? null);
      } catch {
        // ignore cross-window refresh errors
      }
    };

    const broadcastThemeRefresh = async () => {
      if (typeof window !== "undefined") {
        const stamp = String(Date.now());
        localStorage.setItem(THEME_REFRESH_STORAGE_KEY, stamp);
        window.postMessage({ type: THEME_REFRESH_EVENT, ts: stamp }, "*");
        if (transformWindow && !transformWindow.closed) {
          transformWindow.postMessage({ type: THEME_REFRESH_EVENT, ts: stamp }, "*");
        }
      }

      if (!tauriAvailable) return;
      try {
        const { emit } = await import("@tauri-apps/api/event");
        await emit(THEME_REFRESH_EVENT, { ts: Date.now() });
      } catch {
        // ignore event bridge errors
      }
    };

    const loadProfileOptions = async () => {
      if (!tauriAvailable) return;
      try {
        const list = await invoke<string[]>("profiles_list");
        profileOptions = list.length ? list : ["default"];
        if (!profileOptions.includes(selectedProfileName)) {
          selectedProfileName = profileOptions[0] ?? "default";
        }
      } catch (err) {
        console.warn("Failed to load profiles list", err);
      }
    };




    async function refreshBackend() {
      await loadVersion();
      if (!tauriAvailable) return;
      await loadPersistedSettings();
      await refreshDockCapability();
      await checkInitialized();
      if (!demoMode && isObsRunning) {
        await loadScenes();
        await loadSources();
        const webrtcOk = await startWhepPreview();
        if (webrtcOk) {
          stopPreviewLoop();
        } else {
          startPreviewLoop();
        }
      } else {
        stopPreviewLoop();
        stopWhepPreview();
      }
    }

    async function loadPersistedSettings() {
      if (!tauriAvailable || settingsLoaded) return;
      try {
        const settings = await invoke<PersistedSettings>("settings_get");
        applyLoadedSettings(settings);
        await loadProfileOptions();
      } catch (err) {
        console.warn("Failed to load persisted settings", err);
      } finally {
        settingsLoaded = true;
      }
    }

    onMount(() => {
      const refreshPromise = refreshBackend();
      if (typeof window !== "undefined") {
        const params = new URLSearchParams(window.location.search);
        const requestedMode = params.get("auxWindow");
        if (
          requestedMode === "plugins" ||
          requestedMode === "edit-source" ||
          requestedMode === "audio-mixer" ||
          requestedMode === "audio-advanced" ||
          requestedMode === "audio-filters"
        ) {
          auxWindowMode = requestedMode;
          auxWindowSourceId = params.get("sourceId") ?? "";
        }

        const savedDockUrl = localStorage.getItem("browserDockUrl");
        if (savedDockUrl && /^https?:\/\//i.test(savedDockUrl)) {
          browserDockUrl = savedDockUrl;
        }
        const savedDockWidth = Number(localStorage.getItem("dockPaneWidth") ?? "");
        if (Number.isFinite(savedDockWidth) && savedDockWidth > 0) {
          dockPaneWidth = clampDockWidth(savedDockWidth);
        }

        try {
          const rawColors = localStorage.getItem("quickColorRecent");
          if (rawColors) {
            const parsed = JSON.parse(rawColors);
            if (Array.isArray(parsed)) {
              quickColorRecent = parsed
                .filter((c) => typeof c === "string")
                .map((c) => normalizeQuickHexColor(c))
                .slice(0, quickColorHistoryLimit);
            }
          }
        } catch {
          // ignore
        }
      }
      const handleDockShortcut = (event: KeyboardEvent) => {
        if (event.key !== "F2") return;
        if (getActiveModalDialog()) return;
        const target = event.target as HTMLElement | null;
        const tag = target?.tagName?.toLowerCase() ?? "";
        if (tag === "input" || tag === "textarea" || target?.isContentEditable) return;
        event.preventDefault();

        if (event.shiftKey) {
          const entered = window.prompt("Browser dock URL (http/https)", browserDockUrl) ?? "";
          const trimmed = entered.trim();
          if (trimmed) {
            if (!/^https?:\/\//i.test(trimmed)) {
              showGlobalDialog("URL must start with http:// or https://", "warning", 2000);
              return;
            }
            browserDockUrl = trimmed;
            localStorage.setItem("browserDockUrl", browserDockUrl);
            if (dockPinnedSide) {
              refreshDockPane();
            }
          }
        } else {
          syncDockUrl();
          showDockHandle = true;
          if (!dockPinnedSide) {
            pinDockPane("right");
          } else {
            refreshDockPane();
          }
        }
      };
      const maybeAutoStartObs = () => {
        if (!isReleaseBuild || releaseObsAutoStarted) return;
        if (demoMode || !tauriAvailable || isObsRunning || busy) return;
        releaseObsAutoStarted = true;
        void startObsInternal(true);
      };
      const maybeOpenAuxWindowModal = async () => {
        if (auxWindowMode === "plugins") {
          await loadPluginsModalData();
          showPlugins = true;
          return;
        }
        if (auxWindowMode === "edit-source" && auxWindowSourceId) {
          let source = sourcesList.find((s) => s.id === auxWindowSourceId);
          if (!source && backendEnabled) {
            await loadSources();
            source = sourcesList.find((s) => s.id === auxWindowSourceId);
          }
          if (source) {
            await openEditSource(source);
            return;
          }
          showGlobalDialog("Requested source not found for edit window", "warning", 1800);
          return;
        }
        if (auxWindowMode === "audio-mixer") {
          openAudioMixer();
          return;
        }
        if ((auxWindowMode === "audio-advanced" || auxWindowMode === "audio-filters") && auxWindowSourceId) {
          let source = sourcesList.find((s) => s.id === auxWindowSourceId);
          if (!source && backendEnabled) {
            await loadSources();
            source = sourcesList.find((s) => s.id === auxWindowSourceId);
          }
          if (!source) {
            showGlobalDialog("Requested audio source not found", "warning", 1800);
            return;
          }
          if (auxWindowMode === "audio-advanced") {
            await openAudioMixerAdvanced(source.id);
          } else {
            await openAudioMixerFilters(source.id);
          }
        }
      };
      void refreshPromise.finally(maybeAutoStartObs);
      void refreshPromise.finally(() => {
        void maybeOpenAuxWindowModal();
      });
      const handleVisibility = () => {
        if (document.hidden) {
          stopPreviewLoop();
        } else if (!webrtcActive && backendEnabled && isObsRunning && !demoMode) {
          startPreviewLoop();
        }
      };
      const handleResize = () => {
        dockPaneWidth = clampDockWidth(dockPaneWidth);
        isResizing = true;
        requestDockInlineBoundsSync();
        requestPreviewUpdate();
        if (resizeTimer) {
          clearTimeout(resizeTimer);
        }
        resizeTimer = setTimeout(() => {
          isResizing = false;
          resizeTimer = null;
          previewDirty = true;
          void refreshPreview(true);
          setTimeout(() => {
            previewDirty = true;
            void refreshPreview(true);
          }, 150);
        }, 120);
      };
      document.addEventListener("visibilitychange", handleVisibility);
      window.addEventListener("resize", handleResize, { passive: true });
      window.addEventListener("mousemove", handleDockResizeMove);
      window.addEventListener("mouseup", stopDockResize);
      window.addEventListener("mouseleave", stopDockResize);
      window.addEventListener("mousemove", handleDockHeaderMove);
      window.addEventListener("mouseup", stopDockHeaderMove);
      window.addEventListener("mouseleave", stopDockHeaderMove);
      window.addEventListener("scroll", requestDockInlineBoundsSync, true);
      window.addEventListener("keydown", handleDockShortcut);
      const autoStartTimer = setTimeout(maybeAutoStartObs, 120);
      const handleBeforeUnload = () => {
        if (isReleaseBuild && tauriAvailable && isObsRunning) {
          void invoke<string>("obs_shutdown").catch(() => undefined);
        }
      };
      const handleThemeRefreshMessage = (event: MessageEvent) => {
        const data = event.data as { type?: string } | null;
        if (!data || data.type !== THEME_REFRESH_EVENT) return;
        void refreshLookThemeFromPersistedSettings();
      };
      const handleThemeRefreshStorage = (event: StorageEvent) => {
        if (event.key !== THEME_REFRESH_STORAGE_KEY) return;
        void refreshLookThemeFromPersistedSettings();
      };

      const handleModalFocusIn = (event: FocusEvent) => {
        const activeModal = getActiveModalDialog();
        if (!activeModal) return;
        const target = event.target;
        if (target instanceof Node && activeModal.contains(target)) return;
        focusDialogEntry(activeModal);
      };

      const handleModalTabTrap = (event: KeyboardEvent) => {
        if (event.key !== "Tab") return;
        const activeModal = getActiveModalDialog();
        if (!activeModal) return;
        const focusables = getFocusableInDialog(activeModal);
        if (!focusables.length) {
          event.preventDefault();
          activeModal.focus();
          return;
        }
        const first = focusables[0];
        const last = focusables[focusables.length - 1];
        const activeEl = document.activeElement as HTMLElement | null;
        const insideModal = activeEl ? activeModal.contains(activeEl) : false;

        if (!insideModal) {
          event.preventDefault();
          (event.shiftKey ? last : first).focus();
          return;
        }

        if (!event.shiftKey && activeEl === last) {
          event.preventDefault();
          first.focus();
          return;
        }
        if (event.shiftKey && activeEl === first) {
          event.preventDefault();
          last.focus();
        }
      };

      if (tauriAvailable) {
        void (async () => {
          try {
            const { listen } = await import("@tauri-apps/api/event");
            themeRefreshUnlisten = await listen(THEME_REFRESH_EVENT, () => {
              void refreshLookThemeFromPersistedSettings();
            });
          } catch {
            themeRefreshUnlisten = null;
          }
        })();
      }

      void (async () => {
        try {
          const current = getCurrentWindow() as unknown as { label?: string };
          if (current.label && current.label !== "main") {
            closeRequestUnlisten = null;
            return;
          }
          closeRequestUnlisten = await getCurrentWindow().onCloseRequested(async (event) => {
            if (allowExplicitAppClose) {
              return;
            }
            if (closeRiskBypassOnce) {
              closeRiskBypassOnce = false;
              return;
            }
            if (showSettings && settingsHasUnsavedChanges) {
              (event as { preventDefault?: () => void })?.preventDefault?.();
              showUnsavedSettingsCloseConfirm = true;
              return;
            }
            if (!isStreaming && !isRecording) return;
            (event as { preventDefault?: () => void })?.preventDefault?.();
            showCloseRiskConfirm = true;
          });
        } catch {
          closeRequestUnlisten = null;
        }
      })();

      window.addEventListener("message", handleThemeRefreshMessage);
      window.addEventListener("storage", handleThemeRefreshStorage);
      window.addEventListener("beforeunload", handleBeforeUnload);
      document.addEventListener("focusin", handleModalFocusIn, true);
      document.addEventListener("keydown", handleModalTabTrap, true);
      return () => {
        clearTimeout(autoStartTimer);
        document.removeEventListener("visibilitychange", handleVisibility);
        window.removeEventListener("resize", handleResize);
        window.removeEventListener("mousemove", handleDockResizeMove);
        window.removeEventListener("mouseup", stopDockResize);
        window.removeEventListener("mouseleave", stopDockResize);
        window.removeEventListener("mousemove", handleDockHeaderMove);
        window.removeEventListener("mouseup", stopDockHeaderMove);
        window.removeEventListener("mouseleave", stopDockHeaderMove);
        window.removeEventListener("scroll", requestDockInlineBoundsSync, true);
        window.removeEventListener("keydown", handleDockShortcut);
        window.removeEventListener("message", handleThemeRefreshMessage);
        window.removeEventListener("storage", handleThemeRefreshStorage);
        window.removeEventListener("beforeunload", handleBeforeUnload);
        document.removeEventListener("focusin", handleModalFocusIn, true);
        document.removeEventListener("keydown", handleModalTabTrap, true);
        if (themeRefreshUnlisten) {
          themeRefreshUnlisten();
          themeRefreshUnlisten = null;
        }
        if (closeRequestUnlisten) {
          closeRequestUnlisten();
          closeRequestUnlisten = null;
        }
        void disposeDockInlineWebview();
        if (isReleaseBuild && tauriAvailable && isObsRunning) {
          void stopObsInternal(true);
        }
      };
    });

    $: {
      const trimmed = quickColorRecent.slice(0, quickColorHistoryLimit);
      if (trimmed.length !== quickColorRecent.length) {
        quickColorRecent = trimmed;
        persistQuickColorHistory();
      }
    }
  const openSettings = () => {
    settingsModalSaved = false;
    settingsSnapshotBeforeOpen = captureCurrentSettingsSnapshot();
    settingsHasUnsavedChanges = false;
    showSettings = true;
  };

  const closeSettings = async () => {
    if (!settingsModalSaved && settingsSnapshotBeforeOpen) {
      const previous = settingsSnapshotBeforeOpen;
      const previousProfile = (previous.active_profile ?? "default").trim() || "default";
      const currentProfile = selectedProfileName.trim() || "default";

      if (tauriAvailable && previousProfile !== currentProfile) {
        try {
          const restored = await invoke<PersistedSettings>("profiles_activate", { name: previousProfile });
          applyLoadedSettings(restored);
          await loadProfileOptions();
        } catch (err) {
          console.warn("Failed to restore previous profile on cancel", err);
          applyLoadedSettings(previous);
        }
      } else {
        applyLoadedSettings(previous);
      }
    }

    showSettings = false;
    settingsSnapshotBeforeOpen = null;
    settingsModalSaved = false;
    settingsHasUnsavedChanges = false;
  };

  const saveSettings = async (event?: CustomEvent<{ profile?: Record<string, unknown> }>) => {
    settingsModalSaved = true;
    showSettings = false;

    const profileFromEvent = event?.detail?.profile ?? null;
    const mergedProfile = mergeTransitionsIntoProfile(profileFromEvent);
    const mergedLook = asRecord(mergedProfile.look);
    const mergedThemeId = String(mergedLook.selectedThemeId ?? "").trim();
    void logThemeDebug("save:incoming-profile", { selectedThemeId: mergedThemeId });
    currentUiProfile = mergedProfile;
    const general = (mergedProfile?.general as Record<string, unknown> | undefined) ?? {};
    openAdditionalSettingsInWindows = Boolean(general.openAdditionalSettingsInWindows);
    allowDraggablePopups = Boolean(general.allowDraggablePopups);
    quickColorHistoryLimit = Math.max(1, Math.min(15, Number(general.quickColorHistoryLimit) || 5));
    realtimeRefreshSources = Boolean(general.realtimeRefreshSources);
    const a11y = (mergedProfile?.accessibility as AccessibilityUiProfile | undefined) ?? {};
    accessibilityHighContrast = Boolean(a11y.accessibilityHighContrast);
    accessibilityReduceMotion = Boolean(a11y.accessibilityReduceMotion);
    accessibilityFocusIndicators = a11y.accessibilityFocusIndicators !== false;
    accessibilityUiScale = String(a11y.accessibilityUiScale ?? "100");
    accessibilityFontSize = String(a11y.accessibilityFontSize ?? "100");
    accessibilityFontFamily = String(a11y.accessibilityFontFamily ?? "system");
    accessibilityColorVision = String(a11y.accessibilityColorVision ?? "none");

    try {
      if (tauriAvailable) {
        const rootArg = rootDir.trim().length ? rootDir.trim() : null;
        await invoke<string>("settings_save", {
          settings: {
            root_dir: rootArg,
            record_path: recordPath,
            stream_url: streamUrl,
            stream_key: streamKey,
            preview_quality: previewQuality,
            encoder_preference: encoderPreference,
            scene_resolution: sceneResolution,
            whep_url: whepUrl,
            whip_url: whipUrl,
            auto_retry_preview: autoRetryPreview,
            autorescale_inputs: autorescaleInputs,
            ui_profile: mergedProfile,
            active_profile: selectedProfileName
          }
        });
      }
    } catch (err) {
      showGlobalDialog(`Settings profile save failed: ${String(err)}`, "error");
      return;
    }

    applyAccessibilityProfile(mergedProfile);
    void applyLookProfile(mergedProfile);
    void broadcastThemeRefresh();

    if (backendEnabled && isObsRunning) {
      try {
        await invoke<string>("obs_set_scene_resolution", { resolution: sceneResolution });
        await invoke<string>("obs_set_encoder_preference", { preference: encoderPreference });
        await refreshPreviewAfterResolutionChange();
      } catch (err) {
        showGlobalDialog(`Settings saved, but OBS apply failed: ${String(err)}`, "warning");
        return;
      }
    }
    showGlobalDialog("Settings saved", "info");
    settingsSnapshotBeforeOpen = null;
    settingsHasUnsavedChanges = false;
  };

  const loadPluginsModalData = async (preferredProfile?: string) => {
    const loadSeq = ++pluginsLoadSeq;

    if (!tauriAvailable) {
      pluginsList = [];
      pluginProfiles = ["default"];
      activePluginProfile = "default";
      enabledPluginModules = [];
      pluginBaselineModulesByProfile = {};
      return;
    }

    try {
      console.log("[plugins-ui] load:start", { preferredProfile });
      const loadedPlugins = await invoke<PluginInfo[]>("plugins_list");
      if (loadSeq !== pluginsLoadSeq) return;
      pluginsList = loadedPlugins;
      console.log("[plugins-ui] step:plugins_list", { count: pluginsList.length });
      const loadedProfiles = await invoke<string[]>("plugin_profiles_list");
      if (loadSeq !== pluginsLoadSeq) return;
      pluginProfiles = loadedProfiles;
      console.log("[plugins-ui] step:plugin_profiles_list", { count: pluginProfiles.length });
      if (!pluginProfiles.length) {
        pluginProfiles = ["default"];
      }

      const resolvedProfile = preferredProfile
        ? preferredProfile
        : await invoke<string>("plugin_profile_get_active");
      if (loadSeq !== pluginsLoadSeq) return;
      console.log("[plugins-ui] step:plugin_profile_get_active", { resolvedProfile });
      activePluginProfile = pluginProfiles.includes(resolvedProfile)
        ? resolvedProfile
        : pluginProfiles[0];

      const state = await invoke<PluginProfileState>("plugin_profile_get", { name: activePluginProfile });
      if (loadSeq !== pluginsLoadSeq) return;
      enabledPluginModules = state.enabled_modules;
      if (!pluginBaselineModulesByProfile[activePluginProfile]) {
        pluginBaselineModulesByProfile = {
          ...pluginBaselineModulesByProfile,
          [activePluginProfile]: [...state.enabled_modules]
        };
      }
      console.log("[plugins-ui] step:plugin_profile_get", {
        activePluginProfile,
        enabledCount: enabledPluginModules.length
      });
    } catch (err) {
      console.error("[plugins-ui] load:error", err);
      showGlobalDialog(String(err), "error");
    }
  };

  const openPlugins = async () => {
    if (openAdditionalSettingsInWindows && auxWindowMode !== "plugins") {
      await openAuxSettingsWindow("plugins");
      return;
    }
    await loadPluginsModalData();
    showPlugins = true;
  };

  const closeCurrentAuxWindow = async () => {
    let closed = false;
    try {
      await getCurrentWindow().close();
      closed = true;
    } catch {
      // fall through
    }
    if (!closed && tauriAvailable) {
      try {
        const { getCurrentWebviewWindow } = await import("@tauri-apps/api/webviewWindow");
        await getCurrentWebviewWindow().close();
        closed = true;
      } catch {
        // fall through
      }
    }
    if (!closed && typeof window !== "undefined") {
      window.close();
      setTimeout(() => {
        if (!window.closed) {
          window.close();
        }
      }, 30);
    }
  };

  const closePlugins = async () => {
    if (auxWindowMode === "plugins") {
      await closeCurrentAuxWindow();
      return;
    }
    showPlugins = false;
  };

  const selectPluginProfile = async (event: CustomEvent<{ name: string }>) => {
    const name = event.detail?.name?.trim();
    if (!name || !tauriAvailable) return;
    activePluginProfile = name;
    await loadPluginsModalData(name);
  };

  const createPluginProfile = async (event: CustomEvent<{ name: string }>) => {
    const name = event.detail?.name?.trim();
    if (!name || !tauriAvailable) return;
    try {
      const safe = await invoke<string>("plugin_profile_save", { name, enabledModules: [] });
      await loadPluginsModalData(safe);
      showGlobalDialog(`Plugin profile '${safe}' created`, "info");
    } catch (err) {
      showGlobalDialog(String(err), "error");
    }
  };

  const savePlugins = async (event: CustomEvent<{ activeProfile: string; enabledModules: string[] }>) => {
    if (!tauriAvailable) {
      showPlugins = false;
      return;
    }

    const profile = event.detail?.activeProfile?.trim() || "default";
    const enabled = event.detail?.enabledModules ?? [];

    try {
      await invoke<string>("plugin_profile_save", { name: profile, enabledModules: enabled });
      await invoke<string>("plugin_profile_set_active", { name: profile });
      activePluginProfile = profile;
      enabledPluginModules = enabled;
      showGlobalDialog("Plugin configuration saved", "info");
      if (auxWindowMode === "plugins") {
        await closeCurrentAuxWindow();
        return;
      }
      showPlugins = false;
    } catch (err) {
      showGlobalDialog(String(err), "error");
    }
  };

  const transitionKinds = [
    { value: "cut", label: "Cut" },
    { value: "fade", label: "Fade" },
    { value: "swipe", label: "Swipe" },
    { value: "slide", label: "Slide" },
    { value: "fade_to_color", label: "Fade to Color" },
    { value: "luma_wipe", label: "Luma Wipe" },
    { value: "stinger", label: "Stinger" }
  ];

  const updateTransitionParam = (id: string, key: string, value: string) => {
    transitionsDraft = transitionsDraft.map((t) =>
      t.id === id
        ? { ...t, params: { ...(t.params ?? {}), [key]: value } }
        : t
    );
  };

  const updateTransitionName = (id: string, value: string) => {
    transitionsDraft = transitionsDraft.map((t) => (t.id === id ? { ...t, name: value } : t));
  };

  const pickStingerMediaFile = async () => {
    const id = selectedTransitionItem?.id;
    if (!id) return;
    try {
      const selected = await open({
        title: "Select stinger video",
        multiple: false,
        directory: false,
        filters: [
          { name: "Stinger Video", extensions: ["mov", "mp4", "mkv", "webm", "avi", "m4v"] }
        ]
      });
      const selectedPath = resolveDialogSelection(selected);
      if (!selectedPath) return;
      updateTransitionParam(id, "media_file", selectedPath);
    } catch (err) {
      showGlobalDialog(`Could not open file picker: ${String(err)}`, "error", 1800);
    }
  };

  const pickStingerSequenceFolder = async () => {
    const id = selectedTransitionItem?.id;
    if (!id) return;
    try {
      const selected = await open({
        title: "Select image sequence folder",
        multiple: false,
        directory: true
      });
      const selectedPath = resolveDialogSelection(selected);
      if (!selectedPath) return;
      await invoke<string>("transitions_validate_sequence_dir", { path: selectedPath });
      updateTransitionParam(id, "sequence_dir", selectedPath);
      updateTransitionParam(id, "source_mode", "sequence");
    } catch (err) {
      showGlobalDialog(`Invalid sequence folder: ${String(err)}`, "error", 2200);
    }
  };

  const openTransitions = () => {
    transitionsSnapshotBeforeOpen = {
      transitionsDraft: transitionsDraft.map((t) => ({ ...t, params: { ...(t.params ?? {}) } })),
      transitionsSelectedId,
      activeTransitionId,
      transitionNewKind,
      transitionNewName
    };

    if (!transitionsDraft.length) {
      transitionsDraft = [{
        id: crypto.randomUUID ? crypto.randomUUID() : `${Date.now()}-${Math.random()}`,
        name: "Transition 1",
        kind: "fade",
        params: getTransitionDefaults("fade")
      }];
      activeTransitionId = transitionsDraft[0].id;
      transitionsSelectedId = transitionsDraft[0].id;
    } else if (!transitionsSelectedId) {
      transitionsSelectedId = activeTransitionId ?? transitionsDraft[0]?.id ?? null;
    }
    showTransitionsModal = true;
  };

  const getTransitionsStateFingerprint = (draft: TransitionItem[], activeId: string | null) =>
    JSON.stringify({
      activeTransitionId: activeId,
      transitions: draft.map((t) => ({
        id: t.id,
        name: t.name,
        kind: t.kind,
        params: { ...(t.params ?? {}) }
      }))
    });

  const transitionsChangedSinceOpen = () => {
    if (!transitionsSnapshotBeforeOpen) return false;
    return (
      getTransitionsStateFingerprint(transitionsDraft, activeTransitionId) !==
      getTransitionsStateFingerprint(
        transitionsSnapshotBeforeOpen.transitionsDraft,
        transitionsSnapshotBeforeOpen.activeTransitionId
      )
    );
  };

  const restoreTransitionsSnapshot = () => {
    if (!transitionsSnapshotBeforeOpen) return;
    transitionsDraft = transitionsSnapshotBeforeOpen.transitionsDraft.map((t) => ({
      ...t,
      params: { ...(t.params ?? {}) }
    }));
    transitionsSelectedId = transitionsSnapshotBeforeOpen.transitionsSelectedId;
    activeTransitionId = transitionsSnapshotBeforeOpen.activeTransitionId;
    transitionNewKind = transitionsSnapshotBeforeOpen.transitionNewKind;
    transitionNewName = transitionsSnapshotBeforeOpen.transitionNewName;
  };

  const closeTransitions = () => {
    if (transitionsChangedSinceOpen()) {
      showTransitionsDiscardConfirm = true;
      return;
    }

    showTransitionsModal = false;
    transitionsSnapshotBeforeOpen = null;
    showTransitionsDiscardConfirm = false;
  };

  const cancelDiscardTransitions = () => {
    showTransitionsDiscardConfirm = false;
  };

  const confirmDiscardTransitions = () => {
    restoreTransitionsSnapshot();
    showTransitionsDiscardConfirm = false;
    showTransitionsModal = false;
    transitionsSnapshotBeforeOpen = null;
  };

  const setTransitionKind = (id: string, kind: string) => {
    transitionsDraft = transitionsDraft.map((t) => {
      if (t.id !== id) return t;
      const nextParams = { ...getTransitionDefaults(kind), ...(t.params ?? {}) };
      return { ...t, kind, params: nextParams };
    });
  };

  const addTransition = () => {
    const kind = transitionNewKind;
    const nextId = crypto.randomUUID ? crypto.randomUUID() : `${Date.now()}-${Math.random()}`;
    const nextName = transitionNewName.trim() || `Transition ${transitionsDraft.length + 1}`;
    transitionsDraft = [
      ...transitionsDraft,
      {
        id: nextId,
        name: nextName,
        kind,
        params: getTransitionDefaults(kind)
      }
    ];
    transitionsSelectedId = nextId;
    transitionNewName = "";
  };

  const removeTransition = (id: string) => {
    const next = transitionsDraft.filter((t) => t.id !== id);
    transitionsDraft = next;
    if (activeTransitionId === id) {
      activeTransitionId = next[0]?.id ?? null;
    }
    if (transitionsSelectedId === id) {
      transitionsSelectedId = next[0]?.id ?? null;
    }
  };

  const saveTransitions = async () => {
    if (!activeTransitionId && transitionsDraft.length) {
      activeTransitionId = transitionsDraft[0].id;
    }
    const mergedProfile = mergeTransitionsIntoProfile(currentUiProfile);
    currentUiProfile = mergedProfile;
    if (tauriAvailable) {
      try {
        const rootArg = rootDir.trim().length ? rootDir.trim() : null;
        await invoke<string>("settings_save", {
          settings: {
            root_dir: rootArg,
            record_path: recordPath,
            stream_url: streamUrl,
            stream_key: streamKey,
            preview_quality: previewQuality,
            encoder_preference: encoderPreference,
            scene_resolution: sceneResolution,
            whep_url: whepUrl,
            whip_url: whipUrl,
            auto_retry_preview: autoRetryPreview,
            autorescale_inputs: autorescaleInputs,
            ui_profile: mergedProfile,
            active_profile: selectedProfileName
          }
        });
      } catch {
        // keep in-memory transitions even when persistence fails
      }
    }
    showTransitionsModal = false;
    transitionsSnapshotBeforeOpen = null;
    showTransitionsDiscardConfirm = false;
    showGlobalDialog("Transitions saved", "info", 1200);
  };

  const asPlainRecord = (value: unknown): Record<string, unknown> =>
    value && typeof value === "object" && !Array.isArray(value) ? (value as Record<string, unknown>) : {};

  const sanitizeFilterMap = (raw: unknown, allowedKeys?: Set<string>) => {
    const root = asPlainRecord(raw);
    const next: Record<string, FilterItem[]> = {};
    for (const [ownerKey, rawList] of Object.entries(root)) {
      if (allowedKeys && !allowedKeys.has(ownerKey)) continue;
      if (!Array.isArray(rawList)) continue;
      const mapped: FilterItem[] = rawList
        .map((entry, index) => {
          const row = asPlainRecord(entry);
          const id = String(row.id ?? "").trim() || (crypto.randomUUID ? crypto.randomUUID() : `${Date.now()}-${Math.random()}`);
          const kind = String(row.kind ?? "custom").trim() || "custom";
          const name = String(row.name ?? "").trim() || `Filter ${index + 1}`;
          const paramsRaw = asPlainRecord(row.params);
          const params: Record<string, string> = {};
          for (const [k, v] of Object.entries(paramsRaw)) {
            params[String(k)] = String(v ?? "");
          }
          return {
            id,
            name,
            kind,
            enabled: Boolean(row.enabled ?? true),
            locked: Boolean(row.locked ?? false),
            params
          };
        })
        .filter((item) => item.id.trim().length > 0);
      if (mapped.length) {
        next[ownerKey] = mapped;
      }
    }
    return next;
  };

  const sanitizeAudioMixerStateMap = (raw: unknown, allowedSourceIds: Set<string>) => {
    const root = asPlainRecord(raw);
    const next: Record<string, AudioMixerItemState> = {};
    for (const [sourceId, rawState] of Object.entries(root)) {
      if (!allowedSourceIds.has(sourceId)) continue;
      const state = asPlainRecord(rawState);
      const tracksRaw = Array.isArray(state.tracks)
        ? state.tracks.map((t) => String(t ?? "").trim()).filter(Boolean)
        : String(state.tracks ?? "")
            .split(",")
            .map((t) => t.trim())
            .filter(Boolean);
      const volumePercent = clampAudioMixerPercent(Number(state.volumePercent ?? state.volume_percent ?? "100") || 100);
      const volumeDb = clampAudioMixerDb(Number(state.volumeDb ?? state.volume_db ?? percentToDb(volumePercent)) || percentToDb(volumePercent));
      next[sourceId] = {
        volumePercent,
        monitoring: String(state.monitoring ?? "off"),
        balanceLeft: Math.max(0, Math.min(100, Number(state.balanceLeft ?? state.balance_left ?? "100") || 100)),
        balanceRight: Math.max(0, Math.min(100, Number(state.balanceRight ?? state.balance_right ?? "100") || 100)),
        tracks: tracksRaw.length ? [...new Set(tracksRaw)] : ["1"],
        locked: Boolean(state.locked ?? state.volume_locked ?? false),
        volumeMode: String(state.volumeMode ?? state.volume_mode ?? "percent") === "db" ? "db" : "percent",
        volumeDb
      };
    }
    return next;
  };

  const sanitizePlannerTransformsByScene = (raw: unknown, allowedSceneNames: Set<string>, allowedSourceIds: Set<string>) => {
    const root = asPlainRecord(raw);
    const next: Record<string, Record<string, PlannerTransformPayload>> = {};
    const toNumber = (value: unknown, fallback: number) => {
      const parsed = Number(value ?? "");
      return Number.isFinite(parsed) ? parsed : fallback;
    };

    for (const [sceneName, rawBySource] of Object.entries(root)) {
      if (!allowedSceneNames.has(sceneName)) continue;
      const bySource = asPlainRecord(rawBySource);
      const sceneTransforms: Record<string, PlannerTransformPayload> = {};

      for (const [sourceId, rawTransform] of Object.entries(bySource)) {
        if (!allowedSourceIds.has(sourceId)) continue;
        const transform = asPlainRecord(rawTransform);
        const payload: PlannerTransformPayload = {
          x: toNumber(transform.x, 0),
          y: toNumber(transform.y, 0),
          w: Math.max(1, toNumber(transform.w, 1)),
          h: Math.max(1, toNumber(transform.h, 1))
        };
        if (Number.isFinite(Number(transform.rot))) payload.rot = Number(transform.rot);
        if (Number.isFinite(Number(transform.cropL))) payload.cropL = Number(transform.cropL);
        if (Number.isFinite(Number(transform.cropR))) payload.cropR = Number(transform.cropR);
        if (Number.isFinite(Number(transform.cropT))) payload.cropT = Number(transform.cropT);
        if (Number.isFinite(Number(transform.cropB))) payload.cropB = Number(transform.cropB);
        sceneTransforms[sourceId] = payload;
      }

      if (Object.keys(sceneTransforms).length) {
        next[sceneName] = sceneTransforms;
      }
    }

    return next;
  };

  const extractSourceFiltersFromRevoScenes = (
    rawScenes: unknown,
    sources: DemoSource[]
  ): Record<string, FilterItem[]> => {
    if (!Array.isArray(rawScenes) || !sources.length) return {};
    const bySourceName = new Map(sources.map((s) => [s.name, s.id]));
    const result: Record<string, FilterItem[]> = {};

    for (const scene of rawScenes) {
      const sceneObj = asPlainRecord(scene);
      const sceneSources = Array.isArray(sceneObj.sources) ? sceneObj.sources : [];
      for (const rawSource of sceneSources) {
        const sourceObj = asPlainRecord(rawSource);
        const sourceName = String(sourceObj.name ?? sourceObj.source_name ?? "").trim();
        if (!sourceName) continue;
        const sourceId = bySourceName.get(sourceName);
        if (!sourceId) continue;

        const sourceFilters = sanitizeFilterMap({ [sourceId]: sourceObj.filters }, new Set([sourceId]));
        if (sourceFilters[sourceId]?.length) {
          result[sourceId] = sourceFilters[sourceId];
        }
      }
    }

    return result;
  };

  const buildSceneUiExportPayload = () =>
    JSON.stringify({
      source_filters: sourceFiltersMap,
      scene_filters: sceneFiltersMap,
      audio_source_filters: audioSourceFiltersMap,
      audio_mixer_state: audioMixerState,
      graphic_planner_transforms_by_scene: plannerTransformsByScene
    });

  const exportScenes = async () => {
    if (!backendEnabled) {
      showGlobalDialog("Backend unavailable", "error");
      return;
    }
    try {
      const filePath = await save({
        defaultPath: "revo-scenes.json",
        filters: [{ name: "JSON", extensions: ["json"] }]
      });
      if (!filePath) {
        return;
      }
      showGlobalDialog(
        await invoke<string>("obs_export_scene_collection_to_file", {
          path: filePath,
          ui_json: buildSceneUiExportPayload()
        }),
        "info"
      );
    } catch (err) {
      showGlobalDialog(String(err), "error");
    }
  };

  const exportObsScenes = async () => {
    if (!backendEnabled) {
      showGlobalDialog("Backend unavailable", "error");
      return;
    }
    try {
      const filePath = await save({
        defaultPath: "obs-scenes.json",
        filters: [{ name: "JSON", extensions: ["json"] }]
      });
      if (!filePath) {
        return;
      }
      showGlobalDialog(
        await invoke<string>("obs_export_scene_collection_obs_to_file", {
          path: filePath,
          ui_json: buildSceneUiExportPayload()
        }),
        "info"
      );
    } catch (err) {
      showGlobalDialog(String(err), "error");
    }
  };

  const exportObsProfile = async () => {
    if (!tauriAvailable) {
      showGlobalDialog("Backend unavailable", "error");
      return;
    }
    try {
      const filePath = await save({
        defaultPath: "obs-profile.json",
        filters: [{ name: "JSON", extensions: ["json"] }]
      });
      if (!filePath) return;
      showGlobalDialog(await invoke<string>("obs_export_profile_obs_to_file", { path: filePath }), "info");
    } catch (err) {
      showGlobalDialog(String(err), "error");
    }
  };

  const importObsProfile = async (content: string) => {
    if (!tauriAvailable) {
      showGlobalDialog("Backend unavailable", "error");
      return;
    }
    try {
      showGlobalDialog(await invoke<string>("obs_import_profile_obs", { json: content }), "info");
      settingsLoaded = false;
      await loadPersistedSettings();
    } catch (err) {
      showGlobalDialog(String(err), "error");
    }
  };

  const createProfile = async (event: CustomEvent<{ name: string }>) => {
    if (!tauriAvailable) {
      showGlobalDialog("Backend unavailable", "error");
      return;
    }
    const name = (event.detail?.name ?? "").trim();
    if (!name) {
      showGlobalDialog("Profile name required", "warning");
      return;
    }
    try {
      const created = await invoke<string>("profiles_create", { name });
      selectedProfileName = created;
      await loadProfileOptions();
      showGlobalDialog(`Profile '${created}' created`, "info");
    } catch (err) {
      showGlobalDialog(String(err), "error");
    }
  };

  const switchProfile = async (event: CustomEvent<{ name: string }>) => {
    if (!tauriAvailable) {
      showGlobalDialog("Backend unavailable", "error");
      return;
    }
    const name = (event.detail?.name ?? "").trim();
    if (!name) return;
    try {
      const settings = await invoke<PersistedSettings>("profiles_activate", { name });
      applyLoadedSettings(settings);
      await loadProfileOptions();
      requestPreviewUpdate();
      showGlobalDialog(`Switched to profile '${name}'`, "info");
    } catch (err) {
      showGlobalDialog(String(err), "error");
    }
  };

  const importScenes = async (content: string, format: "revo" | "obs" = "revo") => {
    if (!backendEnabled) {
      showGlobalDialog("Backend unavailable", "error");
      return;
    }
    try {
      let importedUiPayload: Record<string, unknown> | null = null;
      try {
        const parsed = JSON.parse(content) as Record<string, unknown>;
        importedUiPayload = asPlainRecord(parsed?.revo_ui ?? parsed?.revoUi ?? parsed?.revo_stream_ui ?? null);
      } catch {
        importedUiPayload = null;
      }

      if (format === "revo") {
        let parsed: any = null;
        try {
          parsed = JSON.parse(content);
        } catch {
          showGlobalDialog("Invalid JSON (RevoStream format)", "error");
          return;
        }
        const hasRevoScenes = Array.isArray(parsed?.scenes);
        if (!hasRevoScenes) {
          showGlobalDialog("Invalid RevoStream scene format (missing scenes array)", "error");
          return;
        }
      }

      await invoke<string>("obs_import_scene_collection", { json: content });
      await loadScenes();
      await loadSources();

      if (format === "revo") {
        try {
          const parsed = JSON.parse(content) as Record<string, unknown>;
          const fromScenes = extractSourceFiltersFromRevoScenes(parsed?.scenes, sourcesList);
          if (Object.keys(fromScenes).length) {
            sourceFiltersMap = {
              ...sourceFiltersMap,
              ...fromScenes
            };
          }
        } catch {
          // ignore additional filter extraction errors
        }
      }

      if (importedUiPayload) {
        const sourceIds = new Set(sourcesList.map((s) => s.id));
        const sceneNames = new Set(scenes.map((s) => s.name));
        sourceFiltersMap = sanitizeFilterMap(importedUiPayload.source_filters, sourceIds);
        audioSourceFiltersMap = sanitizeFilterMap(importedUiPayload.audio_source_filters, sourceIds);
        sceneFiltersMap = sanitizeFilterMap(importedUiPayload.scene_filters, sceneNames);
        plannerTransformsByScene = sanitizePlannerTransformsByScene(
          importedUiPayload.graphic_planner_transforms_by_scene,
          sceneNames,
          sourceIds
        );
        audioMixerState = {
          ...audioMixerState,
          ...sanitizeAudioMixerStateMap(importedUiPayload.audio_mixer_state, sourceIds)
        };
      }

      requestPreviewUpdate();
      showGlobalDialog("Scenes imported", "info");
    } catch (err) {
      showGlobalDialog(String(err), "error");
    }
  };

  const startObsInternal = async (silent = false) => {
    if (busy) return;
    if (demoMode) {
      showGlobalDialog("Disable demo mode to start OBS", "warning");
      return;
    }
    if (!tauriAvailable) {
      showGlobalDialog("Backend unavailable (not running in Tauri)", "error");
      return;
    }
    busy = true;
    // message = "";
    try {
      stopPreviewLoop();
      stopWhepPreview();
      const rootArg = rootDir.trim().length ? rootDir.trim() : null;
      const startMsg = await invoke<string>("obs_start", { root_dir: rootArg });
      if (!silent) {
        showGlobalDialog(startMsg, "info");
      }
      isObsRunning = true;
      await loadVersion();
      await loadScenes();
      await loadSources();
      try {
        const runtimeActiveProfile = await invoke<string>("plugin_profile_get_active");
        const runtimePluginState = await invoke<PluginProfileState>("plugin_profile_get", { name: runtimeActiveProfile });
        activePluginProfile = runtimeActiveProfile;
        enabledPluginModules = [...runtimePluginState.enabled_modules];
        pluginBaselineModulesByProfile = {
          ...pluginBaselineModulesByProfile,
          [runtimeActiveProfile]: [...runtimePluginState.enabled_modules]
        };
      } catch {
        pluginBaselineModulesByProfile = {
          ...pluginBaselineModulesByProfile,
          [activePluginProfile]: [...enabledPluginModules]
        };
      }
      const webrtcOk = await startWhepPreview();
      if (webrtcOk) {
        stopPreviewLoop();
      } else {
        startPreviewLoop();
      }
    } catch (err) {
      showGlobalDialog(String(err), "error");
    } finally {
      busy = false;
    }
  };
  const stopObsInternal = async (silent = false) => {
    if (busy) return;
    if (!tauriAvailable) {
      isObsRunning = false;
      showGlobalDialog("OBS stopped", "info");
      stopPreviewLoop();
      stopWhepPreview();
      return;
    }
    busy = true;
    // message = "";
    try {
      if (webrtcWhipStarted) {
        await invoke<string>("obs_stop_streaming");
        webrtcWhipStarted = false;
        isStreaming = false;
      }
      const stopMsg = await invoke<string>("obs_shutdown");
      if (!silent) {
        showGlobalDialog(stopMsg, "info");
      }
      isObsRunning = false;
      scenes = [];
      sourcesList = [];
      stopPreviewLoop();
      stopWhepPreview();
    } catch (err) {
      showGlobalDialog(String(err), "error");
    } finally {
      busy = false;
    }
  };

  const startObs = () => {
    void startObsInternal(false);
  };

  const stopObs = () => {
    void stopObsInternal(false);
  };

  const startRecording = async () => {
    const confirmations = asRecord(asRecord(currentUiProfile).confirmations);
    if (Boolean(confirmations.confirmStartRecording)) {
      pendingMediaConfirmAction = "start-recording";
      showMediaConfirm = true;
      return;
    }
    await startRecordingConfirmed();
  };

  const startRecordingConfirmed = async () => {
    if (mediaActionBusy || isRecording) return;
    if (!backendEnabled) {
      showGlobalDialog("Backend unavailable", "error");
      return;
    }
    if (!isObsRunning) {
      showGlobalDialog("Start OBS before recording.", "warning");
      return;
    }
    if (!recordPath.trim()) {
      showGlobalDialog("Recording path required", "warning");
      return;
    }

    mediaActionBusy = true;
    try {
      const startMsg = await invoke<string>("obs_start_recording", { outputPath: recordPath.trim() });
      isRecording = true;
      showGlobalDialog(startMsg || "Recording started", "info");
    } catch (err) {
      isRecording = false;
      showGlobalDialog(`Recording failed: ${String(err)}`, "error");
    } finally {
      mediaActionBusy = false;
    }
  };

  const stopRecording = async () => {
    const confirmations = asRecord(asRecord(currentUiProfile).confirmations);
    if (Boolean(confirmations.confirmStopRecording)) {
      pendingMediaConfirmAction = "stop-recording";
      showMediaConfirm = true;
      return;
    }
    await stopRecordingConfirmed();
  };

  const stopRecordingConfirmed = async () => {
    if (mediaActionBusy || !isRecording) return;
    if (!backendEnabled) {
      showGlobalDialog("Backend unavailable", "error");
      return;
    }

    mediaActionBusy = true;
    try {
      const stopMsg = await invoke<string>("obs_stop_recording");
      isRecording = false;
      showGlobalDialog(stopMsg || "Recording stopped", "info");
    } catch (err) {
      showGlobalDialog(`Failed to stop recording: ${String(err)}`, "error");
    } finally {
      mediaActionBusy = false;
    }
  };

  const buildStreamTarget = () => {
    const base = streamUrl.trim();
    const key = streamKey.trim();
    if (!key) return base;
    const baseNormalized = base.replace(/\/+$/, "");
    const keyNormalized = key.replace(/^\/+/, "");
    return `${baseNormalized}/${keyNormalized}`;
  };

  const startStreamingConfirmed = async () => {
    if (mediaActionBusy || isStreaming) return;
    if (!backendEnabled) {
      showGlobalDialog("Backend unavailable", "error");
      return;
    }
    if (!isObsRunning) {
      showGlobalDialog("Start OBS before streaming.", "warning");
      return;
    }

    const target = buildStreamTarget();
    if (!target) {
      showGlobalDialog("Stream URL required", "warning");
      return;
    }

    mediaActionBusy = true;
    try {
      const startMsg = await invoke<string>("obs_start_streaming", { streamUrl: target });
      isStreaming = true;
      showGlobalDialog(startMsg || "Streaming started", "info");

      const general = asRecord(asRecord(currentUiProfile).general);
      const autoRecordEnabled = Boolean(general.autoRecordWhenStartStreaming);
      if (autoRecordEnabled && !isRecording) {
        await startRecordingConfirmed();
      }
    } catch (err) {
      isStreaming = false;
      console.error("stream start error", err);
      showGlobalDialog("Connection to stream source failed. Check your streaming settings.", "error");
    } finally {
      mediaActionBusy = false;
    }
  };

  const startStreaming = () => {
    if (mediaActionBusy || isStreaming) return;
    if (!streamUrl.trim()) {
      showGlobalDialog("Stream URL required", "warning");
      return;
    }
    const confirmations = asRecord(asRecord(currentUiProfile).confirmations);
    if (Boolean(confirmations.confirmStartStreaming)) {
      pendingMediaConfirmAction = "start-streaming";
      showMediaConfirm = true;
      return;
    }
    void startStreamingConfirmed();
  };

  const stopStreaming = async () => {
    const confirmations = asRecord(asRecord(currentUiProfile).confirmations);
    if (Boolean(confirmations.confirmStopStreaming)) {
      pendingMediaConfirmAction = "stop-streaming";
      showMediaConfirm = true;
      return;
    }
    await stopStreamingConfirmed();
  };

  const stopStreamingConfirmed = async () => {
    if (mediaActionBusy || !isStreaming) return;
    if (!backendEnabled) {
      showGlobalDialog("Backend unavailable", "error");
      return;
    }

    mediaActionBusy = true;
    try {
      const stopMsg = await invoke<string>("obs_stop_streaming");
      isStreaming = false;
      showGlobalDialog(stopMsg || "Streaming stopped", "info");
    } catch (err) {
      showGlobalDialog(`Failed to stop streaming: ${String(err)}`, "error");
    } finally {
      mediaActionBusy = false;
    }
  };

  const mediaConfirmTitle = (action: MediaConfirmAction | null) => {
    if (action === "start-streaming") return "Start Streaming";
    if (action === "stop-streaming") return "Stop Streaming";
    if (action === "start-recording") return "Start Recording";
    if (action === "stop-recording") return "Stop Recording";
    return "Confirm Action";
  };

  const mediaConfirmMessage = (action: MediaConfirmAction | null) => {
    if (action === "start-streaming") return "Do you want to start streaming now?";
    if (action === "stop-streaming") return "Do you want to stop streaming now?";
    if (action === "start-recording") return "Do you want to start recording now?";
    if (action === "stop-recording") return "Do you want to stop recording now?";
    return "Do you want to continue?";
  };

  const closeRiskConfirmTitle = () => "Close app while live/recording";

  const closeRiskConfirmMessage = () => {
    if (isStreaming && isRecording) {
      return "Closing now may stop both streaming and recording. If the window cannot be closed now, stop streaming/recording and use Force close from the app context menu.";
    }
    if (isStreaming) {
      return "Closing now may stop streaming. If the window cannot be closed now, stop streaming and use Force close from the app context menu.";
    }
    return "Closing now may stop recording. If the window cannot be closed now, stop recording and use Force close from the app context menu.";
  };

  const executeMediaConfirmAction = async (action: MediaConfirmAction) => {
    if (action === "start-streaming") {
      await startStreamingConfirmed();
      return;
    }
    if (action === "stop-streaming") {
      await stopStreamingConfirmed();
      return;
    }
    if (action === "start-recording") {
      await startRecordingConfirmed();
      return;
    }
    await stopRecordingConfirmed();
  };

  const answerMediaConfirm = (accepted: boolean) => {
    const action = pendingMediaConfirmAction;
    showMediaConfirm = false;
    pendingMediaConfirmAction = null;
    if (!accepted || !action) return;
    void executeMediaConfirmAction(action);
  };

  const answerCloseRiskConfirm = async (accepted: boolean) => {
    showCloseRiskConfirm = false;
    if (!accepted) return;
    closeRiskBypassOnce = true;
    await closeEntireApp();
  };

  const unsavedSettingsCloseTitle = () => "Unsaved settings";

  const unsavedSettingsCloseMessage = () =>
    "You have unsaved settings changes. Do you want to close the app and lose these changes?";

  const answerUnsavedSettingsCloseConfirm = async (accepted: boolean) => {
    showUnsavedSettingsCloseConfirm = false;
    if (!accepted) return;
    if (isStreaming || isRecording) {
      showCloseRiskConfirm = true;
      return;
    }
    closeRiskBypassOnce = true;
    await closeEntireApp();
  };

  const closeEntireApp = async () => {
    allowExplicitAppClose = true;
    closeRiskBypassOnce = true;
    const rollbackTimer = setTimeout(() => {
      allowExplicitAppClose = false;
    }, 2500);

    try {
      await invoke<void>("app_close_all_windows");
      clearTimeout(rollbackTimer);
      return;
    } catch {
      // fallback below
    }

    try {
      await getCurrentWindow().close();
      clearTimeout(rollbackTimer);
      return;
    } catch {
      // fallback below
    }

    try {
      await closeCurrentAuxWindow();
    } catch {
      showGlobalDialog("Close request failed. Use Force close from the context menu.", "warning", 2200);
    } finally {
      clearTimeout(rollbackTimer);
    }
  };

  const addScene = async () => {
    const name = newSceneName.trim() || scenePlaceholder;
    if (backendEnabled) {
      try {
        await invoke<string>("obs_create_scene", { name });
        await loadScenes();
        requestPreviewUpdate();
      } catch (err) {
        showGlobalDialog(String(err), "error");
      }
    } else {
      scenes = [...scenes, { name, active: false, locked: false }];
      requestPreviewUpdate();
    }
    newSceneName = "";
    showAddScene = false;
  };

  const getDefaultSceneName = () => {
    const existing = new Set(scenes.map((s) => s.name));
    let idx = 1;
    let name = `Scene ${idx}`;
    while (existing.has(name)) {
      idx += 1;
      name = `Scene ${idx}`;
    }
    return name;
  };

  const openAddSceneModal = () => {
    scenePlaceholder = getDefaultSceneName();
    showAddScene = true;
  };

  const setCurrentScene = async (name: string) => {
    const sceneSwitchDelayMs = getTransitionSceneSwitchDelayMs();
    triggerRenderTransition();
    if (sceneSwitchDelayMs > 0) {
      await new Promise((resolve) => setTimeout(resolve, sceneSwitchDelayMs));
    }
    if (backendEnabled) {
      try {
        await invoke<string>("obs_set_current_scene", { name });
        await loadScenes();
        await loadSources();
        previewDirty = true;
        await refreshPreview(true);
      } catch (err) {
        showGlobalDialog(String(err), "error");
      }
    } else {
      scenes = scenes.map((scene) => ({ ...scene, active: scene.name === name }));
      requestPreviewUpdate();
    }
    closeSceneMenu();
  };

  const startRenameScene = (scene: SceneInfo) => {
    renamingScene = scene.name;
    renameSceneValue = scene.name;
  };

  const commitRenameScene = async () => {
    if (!renamingScene) return;
    const trimmed = renameSceneValue.trim();
    if (!trimmed) {
      cancelRenameScene();
      return;
    }
    if (backendEnabled) {
      try {
        await invoke<string>("obs_rename_scene", { oldName: renamingScene, newName: trimmed });
        await loadScenes();
      } catch (err) {
        showGlobalDialog(String(err), "error");
      }
    } else {
      scenes = scenes.map((scene) => (scene.name === renamingScene ? { ...scene, name: trimmed } : scene));
    }
    renamingScene = null;
    renameSceneValue = "";
  };

  const cancelRenameScene = () => {
    renamingScene = null;
    renameSceneValue = "";
  };

  const toggleSceneLock = async (scene: SceneInfo) => {
    if (backendEnabled) {
      try {
        await invoke<string>("obs_set_scene_lock", { name: scene.name, locked: !scene.locked });
        await loadScenes();
        requestPreviewUpdate();
      } catch (err) {
        showGlobalDialog(String(err), "error");
      }
    } else {
      scenes = scenes.map((s) => (s.name === scene.name ? { ...s, locked: !s.locked } : s));
      requestPreviewUpdate();
    }
    closeSceneMenu();
  };

  const toggleSourceLock = (source: DemoSource) => {
    sourcesList = sourcesList.map((s) =>
      s.id === source.id ? { ...s, locked: !Boolean(s.locked) } : s
    );
    requestPreviewUpdate();
    if (sourceMenu.open && sourceMenu.source?.id === source.id) {
      sourceMenu = {
        ...sourceMenu,
        source: sourcesList.find((s) => s.id === source.id) ?? null
      };
    }
  };

  const removeScene = async (scene: SceneInfo) => {
    if (scenes.length <= 1) {
      showCannotRemoveLastSceneDialog();
      closeSceneMenu();
      return;
    }
    if (backendEnabled) {
      try {
        await invoke<string>("obs_remove_scene", { name: scene.name });
        await loadScenes();
        requestPreviewUpdate();
      } catch (err) {
        showGlobalDialog(String(err), "error");
      }
    } else {
      scenes = scenes.filter((s) => s.name !== scene.name);
      requestPreviewUpdate();
    }
    if (sceneFiltersMap[scene.name]) {
      const { [scene.name]: _removed, ...rest } = sceneFiltersMap;
      sceneFiltersMap = rest;
    }
    closeSceneMenu();
  };

  const addSource = async () => {
    const id = crypto.randomUUID ? crypto.randomUUID() : `${Date.now()}`;
    const escapedType = newSourceType.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const suffixPattern = new RegExp(`^${escapedType}_(\\d+)$`);
    let maxSuffix = 0;
    for (const src of sourcesList) {
      const match = src.name.match(suffixPattern);
      if (!match) continue;
      const value = Number(match[1]);
      if (Number.isFinite(value)) {
        maxSuffix = Math.max(maxSuffix, value);
      }
    }
    const name = `${newSourceType}_${maxSuffix + 1}`;
    if (backendEnabled) {
      try {
        await invoke<string>("obs_create_source", {
          create: {
            id,
            name,
            source_type: newSourceType,
            params: {}
          }
        });
        await loadSources();
        requestPreviewUpdate();
      } catch (err) {
        showGlobalDialog(String(err), "error");
      }
    } else {
      const source: DemoSource = { id, name, visible: true, source_type: newSourceType, locked: false };
      sourcesList = [...sourcesList, source];
      requestPreviewUpdate();
    }
    showAddSource = false;
  };

  const openAddSourceModal = async () => {
    externalSourceTypes = [];

    if (backendEnabled && isObsRunning) {
      try {
        const listed = await invoke<SourceTypeItem[]>("obs_list_external_source_types");
        const seen = new Set<string>();
        externalSourceTypes = listed.filter((t) => {
          if (externalSourceTypeBlacklist.has(t.id)) return false;
          if (builtInSourceTypeIds.has(t.id)) return false;

          const normalized = normalizeSourceLabel(t.label || t.id);
          if (builtInSourceTypeLabels.has(normalized)) return false;

          if (seen.has(t.id)) return false;
          seen.add(t.id);
          return true;
        });
      } catch (err) {
        console.error("[sources] external source types load error", err);
      }
    }

    if (![...sourceTypes, ...externalSourceTypes].some((t) => t.id === newSourceType)) {
      newSourceType = sourceTypes[0].id;
    }

    showAddSource = true;
  };

  const toggleSourceVisibility = async (source: DemoSource) => {
    if (backendEnabled) {
      try {
        await invoke("obs_set_source_visible", { id: source.id, visible: !source.visible });
        await loadSources();
        requestPreviewUpdate();
      } catch (err) {
        showGlobalDialog(String(err), "error");
      }
      return;
    }
    sourcesList = sourcesList.map((s) => (s.id === source.id ? { ...s, visible: !s.visible } : s));
    requestPreviewUpdate();
  };

  const moveSource = async (source: DemoSource, direction: "up" | "down" | "top" | "bottom") => {
    if (source.locked) {
      showGlobalDialog("Source is locked", "warning", 1200);
      return;
    }
    if (backendEnabled) {
      try {
        await invoke("obs_move_source", { id: source.id, direction });
        await loadSources();
        requestPreviewUpdate();
      } catch (err) {
        showGlobalDialog(String(err), "error");
      }
      return;
    }
    const idx = sourcesList.findIndex((s) => s.id === source.id);
    if (idx === -1) return;
    const list = [...sourcesList];
    list.splice(idx, 1);
    let target = idx;
    if (direction === "up") target = Math.max(0, idx - 1);
    if (direction === "down") target = Math.min(list.length, idx + 1);
    if (direction === "top") target = 0;
    if (direction === "bottom") target = list.length;
    list.splice(target, 0, source);
    sourcesList = list;
    requestPreviewUpdate();
  };

  const moveSourceToIndex = (id: string, toIndex: number) => {
    const source = sourcesList.find((s) => s.id === id);
    if (!source) return;
    if (source.locked) {
      showGlobalDialog("Source is locked", "warning", 1200);
      return;
    }
    if (backendEnabled) {
      invoke("obs_reorder_source", { id, toIndex })
        .then(() => loadSources())
        .then(() => requestPreviewUpdate())
        .catch((err) => showGlobalDialog(String(err), "error"));
      return;
    }
    const currentIndex = sourcesList.findIndex((s) => s.id === id);
    if (currentIndex === -1) return;
    const current = sourcesList[currentIndex];
    const list = sourcesList.filter((s) => s.id !== id);
    let index = toIndex > currentIndex ? toIndex - 1 : toIndex;
    index = Math.max(0, Math.min(list.length, index));
    list.splice(index, 0, current);
    sourcesList = list;
    requestPreviewUpdate();
  };

  const moveSceneToIndex = (name: string, toIndex: number) => {
    if (backendEnabled) {
      invoke("obs_reorder_scene", { name, toIndex })
        .then(() => loadScenes())
        .then(() => requestPreviewUpdate())
        .catch((err) => showGlobalDialog(String(err), "error"));
      return;
    }
    const currentIndex = scenes.findIndex((s) => s.name === name);
    if (currentIndex === -1) return;
    const current = scenes[currentIndex];
    const list = scenes.filter((s) => s.name !== name);
    let index = toIndex > currentIndex ? toIndex - 1 : toIndex;
    index = Math.max(0, Math.min(list.length, index));
    list.splice(index, 0, current);
    scenes = list;
    requestPreviewUpdate();
  };

  const clearEditRealtimeTimer = () => {
    if (!editRealtimeTimer) return;
    clearTimeout(editRealtimeTimer);
    editRealtimeTimer = null;
  };

  const waitForEditRealtimeIdle = async () => {
    if (!editRealtimeInFlight) return;
    const startedAt = Date.now();
    while (editRealtimeInFlight && Date.now() - startedAt < 2000) {
      await new Promise((resolve) => setTimeout(resolve, 20));
    }
  };

  const normalizeSourceParamsForSave = (sourceType: string, params: Record<string, string>) => {
    const normalized: Record<string, string> = { ...(params ?? {}) };
    if (sourceType !== "ffmpeg_source") return normalized;

    const localFile = String(normalized.local_file ?? normalized.file ?? "").trim();
    const networkInput = String(normalized.input ?? normalized.url ?? "").trim();

    if (localFile && !normalized.local_file) normalized.local_file = localFile;
    if (networkInput && !normalized.input) normalized.input = networkInput;

    if (typeof normalized.is_local_file !== "string" || !normalized.is_local_file.trim()) {
      normalized.is_local_file = networkInput && !localFile ? "false" : "true";
    }

    return normalized;
  };

  const isRealtimeSourceRefreshEnabled = () => {
    const general = asRecord(asRecord(currentUiProfile).general);
    if (typeof general.realtimeRefreshSources === "boolean") {
      return general.realtimeRefreshSources;
    }
    return realtimeRefreshSources;
  };

  const shouldApplyRealtimeImmediately = (sourceType: string, key: string) => {
    const normalizedType = (sourceType ?? "").trim().toLowerCase();
    const normalizedKey = key.toLowerCase().replace(/[^a-z0-9]+/g, "").trim();
    if (!normalizedKey) return false;

    if (normalizedType === "ffmpeg_source") {
      return (
        normalizedKey === "localfile" ||
        normalizedKey === "file" ||
        normalizedKey === "input" ||
        normalizedKey === "url" ||
        normalizedKey === "islocalfile"
      );
    }

    if (normalizedType === "image_source" || normalizedType.includes("image")) {
      return normalizedKey === "file";
    }

    if (normalizedType === "pulse_input_capture" || normalizedType === "pulse_output_capture") {
      return normalizedKey === "device" || normalizedKey === "deviceid";
    }

    return false;
  };

  const shouldDeferRealtimeUntilSave = (sourceType: string) => {
    const normalizedType = (sourceType ?? "").trim().toLowerCase();
    if (!normalizedType) return false;

    return (
      normalizedType === "ffmpeg_source" ||
      normalizedType === "pulse_input_capture" ||
      normalizedType === "pulse_output_capture" ||
      normalizedType === "image_source" ||
      normalizedType.includes("image")
    );
  };

  const applyEditSourceRealtimeUpdate = async (force = false) => {
    if (!editSource) return;
    if (!force && !isRealtimeSourceRefreshEnabled()) return;

    if (editRealtimeInFlight) {
      editRealtimeQueued = true;
      return;
    }

    editRealtimeInFlight = true;
    try {
      const normalizedParams = normalizeSourceParamsForSave(editType, editParams);
      if (backendEnabled) {
        await invoke<string>("obs_update_source", {
          update: {
            id: editSource.id,
            name: editName.trim() || editSource.name,
            source_type: editType,
            params: normalizedParams
          }
        });
      }

      sourcesList = sourcesList.map((s) =>
        s.id === editSource?.id
          ? { ...s, name: editName.trim() || s.name, source_type: editType, params: { ...normalizedParams } }
          : s
      );

      if (backendEnabled) {
        requestPreviewUpdate();
      } else {
        previewDirty = true;
        await refreshPreview(true);
      }
    } catch (err) {
      showGlobalDialog(`Realtime source refresh failed: ${String(err)}`, "warning", 1700);
    } finally {
      editRealtimeInFlight = false;
      if (editRealtimeQueued) {
        editRealtimeQueued = false;
        void applyEditSourceRealtimeUpdate(force);
      }
    }
  };

  const scheduleEditSourceRealtimeUpdate = () => {
    if (!isRealtimeSourceRefreshEnabled() || !showEditSource || !editSource) return;
    if (shouldDeferRealtimeUntilSave(editType)) return;
    clearEditRealtimeTimer();
    editRealtimeTimer = setTimeout(() => {
      void applyEditSourceRealtimeUpdate();
    }, 120);
  };

  const rollbackEditSourceRealtimeChanges = async () => {
    if (!editOriginalSnapshot) return;
    const snapshot = editOriginalSnapshot;
    clearEditRealtimeTimer();
    editRealtimeQueued = false;
    await waitForEditRealtimeIdle();

    try {
      if (backendEnabled) {
        await invoke<string>("obs_update_source", {
          update: {
            id: snapshot.id,
            name: snapshot.name,
            source_type: snapshot.source_type,
            params: snapshot.params
          }
        });
      }

      sourcesList = sourcesList.map((s) =>
        s.id === snapshot.id
          ? { ...s, name: snapshot.name, source_type: snapshot.source_type, params: { ...snapshot.params } }
          : s
      );

      if (backendEnabled) {
        requestPreviewUpdate();
      } else {
        previewDirty = true;
        await refreshPreview(true);
      }
    } catch (err) {
      showGlobalDialog(`Rollback source changes failed: ${String(err)}`, "error");
    }
  };

  const openEditSource = async (source: DemoSource) => {
    if (openAdditionalSettingsInWindows && auxWindowMode !== "edit-source") {
      await openAuxSettingsWindow("edit-source", source.id);
      closeSourceMenu();
      return;
    }
    editSource = source;
    editName = source.name;
    editType = source.source_type;
    editParams = {};
    sourcePropertySpecs = [];
    editOriginalSnapshot = {
      id: source.id,
      name: source.name,
      source_type: source.source_type,
      params: { ...(source.params ?? {}) }
    };
    clearEditRealtimeTimer();
    editRealtimeQueued = false;
    if (backendEnabled) {
      try {
        const result = await invoke<{
          name: string;
          source_type: string;
          params: Record<string, string>;
          source_properties?: SourcePropertySpec[];
        }>(
          "obs_get_source_settings",
          { id: source.id }
        );
        editName = result.name || source.name;
        editType = result.source_type || source.source_type;
        editParams = result.params ?? {};
        editOriginalSnapshot = {
          id: source.id,
          name: result.name || source.name,
          source_type: result.source_type || source.source_type,
          params: { ...(result.params ?? {}) }
        };
        sourcePropertySpecs = (result.source_properties ?? []).filter((p) => Boolean(p?.key));

        if (
          (editType === "xcomposite_input" || editType === "window_capture") &&
          !sourcePropertyHasOptions(sourcePropertySpecs, ["window", "capture_window"])
        ) {
          try {
            const windows = await invoke<SourceTypeItem[]>("obs_list_window_picker_items");
            if (windows.length) {
              sourcePropertySpecs = [
                ...sourcePropertySpecs,
                {
                  key: "window",
                  label: "Window",
                  kind: "list",
                  options: windows.map((w) => ({ value: w.id, label: w.label }))
                }
              ];
            }
          } catch (err) {
            console.warn("Window picker fallback failed", err);
          }
        }

        if (
          editType === "v4l2_input" &&
          !sourcePropertyHasOptions(sourcePropertySpecs, ["device", "device_id"])
        ) {
          try {
            const devices = await invoke<SourceTypeItem[]>("obs_list_video_device_picker_items");
            if (devices.length) {
              sourcePropertySpecs = [
                ...sourcePropertySpecs,
                {
                  key: "device_id",
                  label: "Device",
                  kind: "list",
                  options: devices.map((d) => ({ value: d.id, label: d.label }))
                },
                {
                  key: "device",
                  label: "Device",
                  kind: "list",
                  options: devices.map((d) => ({ value: d.id, label: d.label }))
                }
              ];
            }
          } catch (err) {
            console.warn("Video device picker fallback failed", err);
          }
        }

        if (editType === "pulse_input_capture") {
          audioInputDevices = await invoke("obs_list_pulse_devices", { kind: "input" });
          if ((!editParams.device || !String(editParams.device).trim()) && audioInputDevices.length) {
            editParams = { ...editParams, device: String(audioInputDevices[0].id ?? "") };
          }
        } else if (editType === "pulse_output_capture") {
          audioOutputDevices = await invoke("obs_list_pulse_devices", { kind: "output" });
          if ((!editParams.device || !String(editParams.device).trim()) && audioOutputDevices.length) {
            editParams = { ...editParams, device: String(audioOutputDevices[0].id ?? "") };
          }
        }
      } catch (err) {
        showGlobalDialog(String(err), "error");
      }
    }
    showEditSource = true;
    closeSourceMenu();
  };

  const saveEditSource = async () => {
    if (!editSource) return;

    if (isRealtimeSourceRefreshEnabled()) {
      clearEditRealtimeTimer();
      editRealtimeQueued = false;
      await waitForEditRealtimeIdle();
      await applyEditSourceRealtimeUpdate(true);
      await waitForEditRealtimeIdle();
      if (backendEnabled) {
        await loadSources();
      }
      requestPreviewUpdate();
      editOriginalSnapshot = null;
      if (auxWindowMode === "edit-source") {
        await closeCurrentAuxWindow();
        return;
      }
      showEditSource = false;
      editSource = null;
      return;
    }

    const current = editSource;
    const name = editName.trim() || current.name;
    const normalizedParams = normalizeSourceParamsForSave(editType, editParams);
    if (backendEnabled) {
      try {
        await invoke<string>("obs_update_source", {
          update: {
            id: current.id,
            name,
            source_type: editType,
            params: normalizedParams
          }
        });
        await loadSources();
        requestPreviewUpdate();
      } catch (err) {
        showGlobalDialog(String(err), "error");
      }
    } else {
      sourcesList = sourcesList.map((s) =>
        s.id === current.id ? { ...s, name, source_type: editType, params: { ...normalizedParams } } : s
      );
      previewDirty = true;
      await refreshPreview(true);
    }
    editOriginalSnapshot = null;
    if (auxWindowMode === "edit-source") {
      await closeCurrentAuxWindow();
      return;
    }
    showEditSource = false;
    editSource = null;
  };

  const cancelEditSource = async () => {
    if (isRealtimeSourceRefreshEnabled()) {
      clearEditRealtimeTimer();
      editRealtimeQueued = false;
      await waitForEditRealtimeIdle();
      await rollbackEditSourceRealtimeChanges();
    }
    editOriginalSnapshot = null;
    if (auxWindowMode === "edit-source") {
      showEditSource = false;
      editSource = null;
      void closeCurrentAuxWindow();
      return;
    }
    showEditSource = false;
    editSource = null;
  };

  const updateParamValue = (key: string, value: string) => {
    editParams = { ...editParams, [key]: value };
    if (shouldDeferRealtimeUntilSave(editType)) {
      clearEditRealtimeTimer();
      editRealtimeQueued = false;
      return;
    }
    if (isRealtimeSourceRefreshEnabled() && shouldApplyRealtimeImmediately(editType, key)) {
      clearEditRealtimeTimer();
      void applyEditSourceRealtimeUpdate(true);
      return;
    }
    scheduleEditSourceRealtimeUpdate();
  };

  const updateEditNameValue = (value: string) => {
    editName = value;
    scheduleEditSourceRealtimeUpdate();
  };

  const renameParamKey = (oldKey: string, newKey: string) => {
    const trimmed = newKey.trim();
    if (!trimmed || trimmed === oldKey) return;
    const { [oldKey]: oldVal, ...rest } = editParams;
    editParams = { ...rest, [trimmed]: oldVal ?? "" };
    scheduleEditSourceRealtimeUpdate();
  };

  const removeParam = (key: string) => {
    const normalized = key.toLowerCase().replace(/[^a-z0-9]+/g, "").trim();
    if (normalized === "posx" || normalized === "posy" || normalized === "itemwidth" || normalized === "itemheight") {
      showGlobalDialog("This transform parameter cannot be removed. Use Reset.", "warning", 1600);
      return;
    }
    const { [key]: _removed, ...rest } = editParams;
    editParams = rest;
    scheduleEditSourceRealtimeUpdate();
  };

  const resetProtectedEditParam = (key: string) => {
    const normalized = key.toLowerCase().replace(/[^a-z0-9]+/g, "").trim();
    const snapshotParams = editOriginalSnapshot?.params ?? {};
    const findByNormalized = (wanted: string) => {
      const current = Object.keys(editParams).find((k) => k.toLowerCase().replace(/[^a-z0-9]+/g, "").trim() === wanted);
      if (current) return current;
      const fromSnapshot = Object.keys(snapshotParams).find((k) => k.toLowerCase().replace(/[^a-z0-9]+/g, "").trim() === wanted);
      if (fromSnapshot) return fromSnapshot;
      if (wanted === "itemwidth") return "item_width";
      if (wanted === "itemheight") return "item_height";
      if (wanted === "posx") return "pos_x";
      if (wanted === "posy") return "pos_y";
      return wanted;
    };

    const widthKey = findByNormalized("itemwidth");
    const heightKey = findByNormalized("itemheight");
    const posXKey = findByNormalized("posx");
    const posYKey = findByNormalized("posy");

    const parseSceneResolution = () => {
      const match = /^\s*(\d+)\s*x\s*(\d+)\s*$/i.exec(sceneResolution ?? "");
      if (!match) return { w: 1920, h: 1080 };
      return {
        w: Math.max(1, Number(match[1]) || 1920),
        h: Math.max(1, Number(match[2]) || 1080)
      };
    };

    const { w: sceneW, h: sceneH } = parseSceneResolution();

    const pickNumber = (first: string | undefined, second: string | undefined, fallback: number) => {
      const one = Number(first ?? "");
      if (Number.isFinite(one) && one > 0) return one;
      const two = Number(second ?? "");
      if (Number.isFinite(two) && two > 0) return two;
      return fallback;
    };

    const next = { ...editParams };

    if (normalized === "itemwidth" || normalized === "itemheight") {
      const defaultWidth = pickNumber(snapshotParams[widthKey], editParams[widthKey], sceneW);
      const defaultHeight = pickNumber(snapshotParams[heightKey], editParams[heightKey], sceneH);
      next[widthKey] = String(Math.round(defaultWidth));
      next[heightKey] = String(Math.round(defaultHeight));
    }

    if (normalized === "posx" || normalized === "posy" || normalized === "itemwidth" || normalized === "itemheight") {
      const itemW = pickNumber(next[widthKey], snapshotParams[widthKey], sceneW);
      const itemH = pickNumber(next[heightKey], snapshotParams[heightKey], sceneH);
      const centerX = Math.round((sceneW - itemW) / 2);
      const centerY = Math.round((sceneH - itemH) / 2);
      next[posXKey] = String(centerX);
      next[posYKey] = String(centerY);
    }

    editParams = next;
    scheduleEditSourceRealtimeUpdate();
  };

  const addParamEntry = (key: string, value: string) => {
    const trimmed = key.trim();
    if (!trimmed) return;
    editParams = { ...editParams, [trimmed]: value };
    scheduleEditSourceRealtimeUpdate();
  };

  const showInfo = (source: DemoSource) => {
    selectedSource = source;
    showSourceInfo = true;
    closeSourceMenu();
  };

  const cloneFilterList = (filters: FilterItem[]) =>
    (filters ?? []).map((f) => ({ ...f, params: { ...(f.params ?? {}) } }));

  const applySourceFiltersToBackend = async (sourceId: string, filters: FilterItem[]) => {
    if (!backendEnabled || !sourceId.trim()) return;
    await invoke<string>("obs_set_source_filters", {
      sourceId,
      filters: cloneFilterList(filters)
    });
    requestPreviewUpdate();
  };

  const flushSourceFiltersLive = async () => {
    if (sourceFiltersLiveInFlight) return;
    const queued = sourceFiltersLiveQueued;
    if (!queued) return;

    sourceFiltersLiveQueued = null;
    sourceFiltersLiveInFlight = true;
    try {
      await applySourceFiltersToBackend(queued.sourceId, queued.filters);
    } catch (err) {
      showGlobalDialog(`Realtime filters refresh failed: ${String(err)}`, "warning", 1700);
    } finally {
      sourceFiltersLiveInFlight = false;
      if (sourceFiltersLiveQueued) {
        void flushSourceFiltersLive();
      }
    }
  };

  const scheduleSourceFiltersLiveApply = (sourceId: string, filters: FilterItem[]) => {
    if (!backendEnabled || !sourceId.trim()) return;
    sourceFiltersLiveQueued = { sourceId, filters: cloneFilterList(filters) };
    if (sourceFiltersLiveTimer) {
      clearTimeout(sourceFiltersLiveTimer);
    }
    sourceFiltersLiveTimer = setTimeout(() => {
      sourceFiltersLiveTimer = null;
      void flushSourceFiltersLive();
    }, 110);
  };

  const handleFiltersLiveChange = (event: CustomEvent<{ filters: FilterItem[] }>) => {
    const liveFilters = cloneFilterList(event.detail?.filters ?? []);
    filtersDraft = liveFilters;

    if (filtersTargetType === "source") {
      sourceFiltersMap = { ...sourceFiltersMap, [filtersTargetId]: liveFilters };
      scheduleSourceFiltersLiveApply(filtersTargetId, liveFilters);
    } else {
      sceneFiltersMap = { ...sceneFiltersMap, [filtersTargetId]: liveFilters };
    }
  };

  const openSourceFilters = (source: DemoSource) => {
    filtersTargetType = "source";
    filtersTargetId = source.id;
    filtersTargetLabel = source.name;
    filtersDraft = cloneFilterList(sourceFiltersMap[source.id] ?? []);
    filtersSnapshotBeforeOpen = cloneFilterList(sourceFiltersMap[source.id] ?? []);
    showFiltersModal = true;
    closeSourceMenu();
  };

  const isBrowserSource = (source: DemoSource | null | undefined) => {
    return (source?.source_type ?? "").toLowerCase() === "browser_source";
  };

  const isTextSource = (source: DemoSource | null | undefined) => {
    return (source?.source_type ?? "").toLowerCase().includes("text");
  };

  const openSourceInteraction = async (source: DemoSource) => {
    if (!backendEnabled) {
      showGlobalDialog("Interaction is available only with OBS backend enabled", "warning", 1800);
      closeSourceMenu();
      return;
    }

    try {
      await invoke<string>("obs_open_source_interaction", { id: source.id });
      closeSourceMenu();
    } catch (err) {
      showGlobalDialog(String(err), "error");
    }
  };

  const openTextEdit = async (source: DemoSource) => {
    if (!isTextSource(source)) return;

    if (!backendEnabled) {
      textEditSource = source;
      textEditValue = source.params?.text ?? "";
      showTextEditModal = true;
      closeSourceMenu();
      return;
    }

    try {
      const result = await invoke<{ params: Record<string, string> }>("obs_get_source_settings", { id: source.id });
      textEditSource = source;
      textEditValue = result?.params?.text ?? source.params?.text ?? "";
      showTextEditModal = true;
      closeSourceMenu();
    } catch (err) {
      showGlobalDialog(String(err), "error");
    }
  };

  const closeTextEditModal = () => {
    showTextEditModal = false;
    textEditSource = null;
    quickTextDragX = 0;
    quickTextDragY = 0;
    quickTextDragActive = false;
  };

  const clampValue = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

  const beginQuickTextDrag = (event: PointerEvent) => {
    if (!allowDraggablePopups || openAdditionalSettingsInWindows) return;
    if (event.button !== 0) return;
    const target = event.target as HTMLElement | null;
    if (target?.closest("button,input,select,textarea,a,label")) return;
    if (!quickTextModalEl) return;
    quickTextDragActive = true;
    quickTextDragStartX = event.clientX;
    quickTextDragStartY = event.clientY;
    quickTextDragOriginX = quickTextDragX;
    quickTextDragOriginY = quickTextDragY;
    const currentTarget = event.currentTarget as HTMLElement | null;
    currentTarget?.setPointerCapture?.(event.pointerId);
  };

  const moveQuickTextDrag = (event: PointerEvent) => {
    if (!quickTextDragActive || !quickTextModalEl) return;
    const rect = quickTextModalEl.getBoundingClientRect();
    const viewportW = window.innerWidth;
    const viewportH = window.innerHeight;
    const minVisibleRatio = 0.25;
    const centerLeft = (viewportW - rect.width) / 2;
    const centerTop = (viewportH - rect.height) / 2;
    const nextDx = quickTextDragOriginX + (event.clientX - quickTextDragStartX);
    const nextDy = quickTextDragOriginY + (event.clientY - quickTextDragStartY);
    const minLeft = -rect.width * (1 - minVisibleRatio);
    const maxLeft = viewportW - rect.width * minVisibleRatio;
    const minTop = -rect.height * (1 - minVisibleRatio);
    const maxTop = viewportH - rect.height * minVisibleRatio;
    const clampedLeft = clampValue(centerLeft + nextDx, minLeft, maxLeft);
    const clampedTop = clampValue(centerTop + nextDy, minTop, maxTop);
    quickTextDragX = clampedLeft - centerLeft;
    quickTextDragY = clampedTop - centerTop;
  };

  const endQuickTextDrag = (event: PointerEvent) => {
    if (!quickTextDragActive) return;
    quickTextDragActive = false;
    const target = event.currentTarget as HTMLElement | null;
    target?.releasePointerCapture?.(event.pointerId);
  };

  const beginPopupDrag = (
    event: PointerEvent,
    element: HTMLDivElement | null,
    originX: number,
    originY: number,
    setActive: (value: boolean) => void,
    setStart: (x: number, y: number) => void,
    setOrigin: (x: number, y: number) => void
  ) => {
    if (!allowDraggablePopups || openAdditionalSettingsInWindows) return;
    if (event.button !== 0) return;
    const target = event.target as HTMLElement | null;
    if (target?.closest("button,input,select,textarea,a,label")) return;
    if (!element) return;
    setActive(true);
    setStart(event.clientX, event.clientY);
    setOrigin(originX, originY);
    const currentTarget = event.currentTarget as HTMLElement | null;
    currentTarget?.setPointerCapture?.(event.pointerId);
  };

  const movePopupDrag = (
    event: PointerEvent,
    active: boolean,
    element: HTMLDivElement | null,
    dragStartX: number,
    dragStartY: number,
    dragOriginX: number,
    dragOriginY: number,
    setOffset: (x: number, y: number) => void
  ) => {
    if (!active || !element) return;
    const rect = element.getBoundingClientRect();
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
    setOffset(clampedLeft - centerLeft, clampedTop - centerTop);
  };

  const endPopupDrag = (event: PointerEvent, active: boolean, setActive: (value: boolean) => void) => {
    if (!active) return;
    setActive(false);
    const target = event.currentTarget as HTMLElement | null;
    target?.releasePointerCapture?.(event.pointerId);
  };

  const beginAudioMixerDrag = (event: PointerEvent) => {
    beginPopupDrag(
      event,
      audioMixerModalEl,
      audioMixerDragX,
      audioMixerDragY,
      (value) => (audioMixerDragActive = value),
      (x, y) => {
        audioMixerDragStartX = x;
        audioMixerDragStartY = y;
      },
      (x, y) => {
        audioMixerDragOriginX = x;
        audioMixerDragOriginY = y;
      }
    );
  };

  const moveAudioMixerDrag = (event: PointerEvent) => {
    movePopupDrag(
      event,
      audioMixerDragActive,
      audioMixerModalEl,
      audioMixerDragStartX,
      audioMixerDragStartY,
      audioMixerDragOriginX,
      audioMixerDragOriginY,
      (x, y) => {
        audioMixerDragX = x;
        audioMixerDragY = y;
      }
    );
  };

  const endAudioMixerDrag = (event: PointerEvent) => {
    endPopupDrag(event, audioMixerDragActive, (value) => (audioMixerDragActive = value));
  };

  const beginAudioAdvancedDrag = (event: PointerEvent) => {
    beginPopupDrag(
      event,
      audioAdvancedModalEl,
      audioAdvancedDragX,
      audioAdvancedDragY,
      (value) => (audioAdvancedDragActive = value),
      (x, y) => {
        audioAdvancedDragStartX = x;
        audioAdvancedDragStartY = y;
      },
      (x, y) => {
        audioAdvancedDragOriginX = x;
        audioAdvancedDragOriginY = y;
      }
    );
  };

  const moveAudioAdvancedDrag = (event: PointerEvent) => {
    movePopupDrag(
      event,
      audioAdvancedDragActive,
      audioAdvancedModalEl,
      audioAdvancedDragStartX,
      audioAdvancedDragStartY,
      audioAdvancedDragOriginX,
      audioAdvancedDragOriginY,
      (x, y) => {
        audioAdvancedDragX = x;
        audioAdvancedDragY = y;
      }
    );
  };

  const endAudioAdvancedDrag = (event: PointerEvent) => {
    endPopupDrag(event, audioAdvancedDragActive, (value) => (audioAdvancedDragActive = value));
  };

  const beginAudioFiltersDrag = (event: PointerEvent) => {
    beginPopupDrag(
      event,
      audioFiltersModalEl,
      audioFiltersDragX,
      audioFiltersDragY,
      (value) => (audioFiltersDragActive = value),
      (x, y) => {
        audioFiltersDragStartX = x;
        audioFiltersDragStartY = y;
      },
      (x, y) => {
        audioFiltersDragOriginX = x;
        audioFiltersDragOriginY = y;
      }
    );
  };

  const moveAudioFiltersDrag = (event: PointerEvent) => {
    movePopupDrag(
      event,
      audioFiltersDragActive,
      audioFiltersModalEl,
      audioFiltersDragStartX,
      audioFiltersDragStartY,
      audioFiltersDragOriginX,
      audioFiltersDragOriginY,
      (x, y) => {
        audioFiltersDragX = x;
        audioFiltersDragY = y;
      }
    );
  };

  const endAudioFiltersDrag = (event: PointerEvent) => {
    endPopupDrag(event, audioFiltersDragActive, (value) => (audioFiltersDragActive = value));
  };

  const saveTextEditModal = async () => {
    if (!textEditSource) return;

    if (!backendEnabled) {
      sourcesList = sourcesList.map((s) =>
        s.id === textEditSource?.id
          ? { ...s, params: { ...(s.params ?? {}), text: textEditValue } }
          : s
      );
      closeTextEditModal();
      return;
    }

    try {
      await invoke<string>("obs_update_source", {
        update: {
          id: textEditSource.id,
          name: textEditSource.name,
          source_type: textEditSource.source_type,
          params: {
            ...(textEditSource.params ?? {}),
            text: textEditValue
          }
        }
      });
      await loadSources();
      requestPreviewUpdate();
      closeTextEditModal();
      showGlobalDialog("Text updated", "info", 1300);
    } catch (err) {
      showGlobalDialog(String(err), "error");
    }
  };

  const normalizeQuickHexColor = (value: string) => {
    let cleaned = (value ?? "").trim().toLowerCase();
    if (cleaned.startsWith("0x")) cleaned = cleaned.slice(2);
    if (cleaned.startsWith("#")) cleaned = cleaned.slice(1);
    cleaned = cleaned.replace(/[^0-9a-f]/g, "");
    if (cleaned.length > 6) cleaned = cleaned.slice(cleaned.length - 6);
    return `#${cleaned.padStart(6, "0")}`;
  };

  const resolveDialogSelection = (value: string | string[] | null): string | null => {
    if (!value) return null;
    const raw = Array.isArray(value) ? value[0] : value;
    if (!raw || !raw.trim()) return null;
    const trimmed = raw.trim();
    if (trimmed.startsWith("file://")) {
      try {
        return decodeURIComponent(trimmed.replace(/^file:\/\//i, ""));
      } catch {
        return trimmed.replace(/^file:\/\//i, "");
      }
    }
    return trimmed;
  };

  const persistQuickColorHistory = () => {
    if (typeof window === "undefined") return;
    try {
      localStorage.setItem("quickColorRecent", JSON.stringify(quickColorRecent));
    } catch {
      // ignore
    }
  };

  const pushQuickColorRecent = (color: string) => {
    const normalized = normalizeQuickHexColor(color);
    quickColorRecent = [normalized, ...quickColorRecent.filter((c) => c !== normalized)].slice(
      0,
      quickColorHistoryLimit
    );
    persistQuickColorHistory();
  };

  const closeQuickColorModal = () => {
    showQuickColorModal = false;
    quickColorSource = null;
  };

  const openQuickColorModal = (source: DemoSource) => {
    quickColorSource = source;
    const hasColor1 = Object.prototype.hasOwnProperty.call(source.params ?? {}, "color1");
    quickColorParamKey = hasColor1 ? "color1" : "color";
    quickColorValue = normalizeQuickHexColor((source.params ?? {})[quickColorParamKey] ?? "#ffffff");
    showQuickColorModal = true;
    closeSourceMenu();
  };

  const saveQuickColorModal = async () => {
    if (!quickColorSource) return;
    const color = normalizeQuickHexColor(quickColorValue);
    const source = quickColorSource;
    const nextParams = { ...(source.params ?? {}), [quickColorParamKey]: color };

    if (!backendEnabled) {
      sourcesList = sourcesList.map((s) => (s.id === source.id ? { ...s, params: nextParams } : s));
      previewDirty = true;
      await refreshPreview(true);
      pushQuickColorRecent(color);
      closeQuickColorModal();
      showGlobalDialog("Color updated", "info", 1200);
      return;
    }

    try {
      await invoke<string>("obs_update_source", {
        update: {
          id: source.id,
          name: source.name,
          source_type: source.source_type,
          params: nextParams
        }
      });
      await loadSources();
      requestPreviewUpdate();
      pushQuickColorRecent(color);
      closeQuickColorModal();
      showGlobalDialog("Color updated", "info", 1200);
    } catch (err) {
      showGlobalDialog(String(err), "error");
    }
  };

  const closeQuickDeviceModal = () => {
    showQuickDeviceModal = false;
    quickDeviceSource = null;
    quickDeviceOptions = [];
  };

  const isAudioDeviceSourceType = (type: string) => {
    const t = (type ?? "").toLowerCase();
    return (
      t === "pulse_input_capture" ||
      t === "pulse_output_capture" ||
      t.includes("audio_input") ||
      t.includes("audio_output") ||
      t.includes("audio_capture") ||
      t.includes("audio_line")
    );
  };

  const openQuickDeviceModal = async (source: DemoSource) => {
    quickDeviceSource = source;
    const type = (source.source_type ?? "").toLowerCase();
    const prefersDeviceId = type.includes("video") || type.includes("v4l2");
    const audioLike = isAudioDeviceSourceType(type);
    quickDeviceParamKey = audioLike ? "device" : prefersDeviceId ? "device_id" : "device";

    let options: { value: string; label: string }[] = [];
    if (backendEnabled) {
      try {
        if (type === "pulse_input_capture" || type.includes("audio_input") || type.includes("audio_capture") || type.includes("audio_line")) {
          const items = await invoke<{ id: string; name: string }[]>("obs_list_pulse_devices", { kind: "input" });
          options = items.map((d) => ({ value: d.id, label: d.name }));
        } else if (type === "pulse_output_capture" || type.includes("audio_output")) {
          const items = await invoke<{ id: string; name: string }[]>("obs_list_pulse_devices", { kind: "output" });
          options = items.map((d) => ({ value: d.id, label: d.name }));
        } else if (type.includes("video") || type.includes("v4l2")) {
          const items = await invoke<{ id: string; label: string }[]>("obs_list_video_device_picker_items");
          options = items.map((d) => ({ value: d.id, label: d.label }));
        }
      } catch {
        // fallback below
      }
    }

    if (!options.length && backendEnabled) {
      try {
        const settings = await invoke<{ source_properties?: Array<{ key: string; label: string; kind: string; options?: Array<{ value: string; label: string }> }> }>("obs_get_source_settings", { id: source.id });
        const prop = (settings.source_properties ?? []).find((p) => {
          const key = (p.key ?? "").toLowerCase();
          return (key.includes("device") || key.includes("input") || key.includes("output")) && (p.options?.length ?? 0) > 0;
        });
        options = (prop?.options ?? []).map((o) => ({ value: o.value, label: o.label }));
      } catch {
        // no-op
      }
    }

    quickDeviceOptions = options.map((o) => ({ value: String(o.value), label: String(o.label) }));
    quickDeviceValue =
      String(
        (source.params ?? {})[quickDeviceParamKey] ??
        (audioLike ? (source.params ?? {}).device_id : (source.params ?? {}).device) ??
        quickDeviceOptions[0]?.value ??
        ""
      );
    quickDeviceMonitoring = String(audioMixerState[source.id]?.monitoring ?? (source.params ?? {}).monitoring ?? "off");
    showQuickDeviceModal = true;
    closeSourceMenu();
  };

  const saveQuickDeviceModal = async () => {
    if (!quickDeviceSource) return;
    const source = quickDeviceSource;
    const nextParams = { ...(source.params ?? {}), [quickDeviceParamKey]: quickDeviceValue };
    if (isAudioDeviceSourceType(source.source_type ?? "")) {
      nextParams.device = quickDeviceValue;
      nextParams.device_id = quickDeviceValue;
      nextParams.monitoring = quickDeviceMonitoring;
    } else if ((source.source_type ?? "").toLowerCase().includes("video") || (source.source_type ?? "").toLowerCase().includes("v4l2")) {
      nextParams.device_id = quickDeviceValue;
    }

    if (!backendEnabled) {
      sourcesList = sourcesList.map((s) => (s.id === source.id ? { ...s, params: nextParams } : s));
      if (isAudioDeviceSourceType(source.source_type ?? "")) {
        updateAudioMixerState(source.id, { monitoring: quickDeviceMonitoring });
      }
      previewDirty = true;
      await refreshPreview(true);
      closeQuickDeviceModal();
      showGlobalDialog("Device updated", "info", 1200);
      return;
    }

    try {
      await invoke<string>("obs_update_source", {
        update: {
          id: source.id,
          name: source.name,
          source_type: source.source_type,
          params: nextParams
        }
      });
      await loadSources();
      if (isAudioDeviceSourceType(source.source_type ?? "")) {
        updateAudioMixerState(source.id, { monitoring: quickDeviceMonitoring });
      }
      requestPreviewUpdate();
      closeQuickDeviceModal();
      showGlobalDialog("Device updated", "info", 1200);
    } catch (err) {
      showGlobalDialog(String(err), "error");
    }
  };

  const allAudioTrackIds = ["1", "2", "3", "4", "5", "6"];

  const getActiveAudioTrackIds = () => {
    const output = asRecord(asRecord(currentUiProfile).output);
    const audioTracks = asRecord(output.audioTracks);
    const tracks: string[] = [];
    for (let i = 1; i <= 5; i += 1) {
      const key = `audioChannel${i}Bitrate`;
      const bitrate = String(audioTracks[key] ?? "").trim();
      if (!bitrate || bitrate === "0") continue;
      tracks.push(String(i));
    }
    return tracks.length ? tracks : ["1"];
  };

  const isAudioMixerSource = (source: DemoSource) => {
    const type = (source.source_type ?? "").toLowerCase();
    return (
      type.includes("audio") ||
      type === "pulse_input_capture" ||
      type === "pulse_output_capture" ||
      type === "ffmpeg_source"
    );
  };

  $: audioMixerSources = sourcesList.filter((s) => isAudioMixerSource(s));

  const AUDIO_MIXER_DB_MIN = -60;
  const AUDIO_MIXER_DB_MAX = 12;
  const clampAudioMixerPercent = (value: number) => Math.max(0, Math.min(200, Number.isFinite(value) ? value : 100));
  const clampAudioMixerDb = (value: number) => Math.max(AUDIO_MIXER_DB_MIN, Math.min(AUDIO_MIXER_DB_MAX, Number.isFinite(value) ? value : 0));
  const percentToDb = (percent: number) => {
    const normalized = clampAudioMixerPercent(percent) / 100;
    if (normalized <= 0.0001) return AUDIO_MIXER_DB_MIN;
    return clampAudioMixerDb(20 * Math.log10(normalized));
  };
  const dbToPercent = (db: number) => clampAudioMixerPercent(Math.pow(10, clampAudioMixerDb(db) / 20) * 100);
  const getAudioMixerVisualLevel = (source: DemoSource, state: AudioMixerItemState) => {
    const params = source.params ?? {};
    const maybeLevel = Number(
      params.input_level_percent ??
      params.output_level_percent ??
      params.audio_level_percent ??
      params.level_percent ??
      params.level
    );
    if (Number.isFinite(maybeLevel) && maybeLevel >= 0) {
      return Math.max(0, Math.min(100, Math.round(maybeLevel)));
    }
    const sourcePercent = state.volumeMode === "db" ? dbToPercent(state.volumeDb) : state.volumePercent;
    const normalized = Math.max(0, Math.min(1, sourcePercent / 100));
    return Math.round(normalized * 100);
  };
  const formatAudioMixerDb = (value: number) => `${value >= 0 ? "+" : ""}${value.toFixed(1)} dB`;

  const ensureAudioMixerState = (source: DemoSource): AudioMixerItemState => {
    const existing = audioMixerState[source.id];
    if (existing) return existing;
    const params = source.params ?? {};
    const tracks = String(params.audio_tracks ?? "")
      .split(",")
      .map((x) => x.trim())
      .filter(Boolean);
    const volumePercent = clampAudioMixerPercent(Number(params.volume_percent ?? "100") || 100);
    const volumeDb = clampAudioMixerDb(Number(params.volume_db ?? percentToDb(volumePercent)) || percentToDb(volumePercent));
    const created: AudioMixerItemState = {
      volumePercent,
      monitoring: String(params.monitoring ?? "off"),
      balanceLeft: Math.max(0, Math.min(100, Number(params.balance_left ?? "100") || 100)),
      balanceRight: Math.max(0, Math.min(100, Number(params.balance_right ?? "100") || 100)),
      tracks: tracks.length ? tracks : getActiveAudioTrackIds(),
      locked: String(params.volume_locked ?? "false") === "true",
      volumeMode: String(params.volume_mode ?? "percent") === "db" ? "db" : "percent",
      volumeDb
    };
    audioMixerState = { ...audioMixerState, [source.id]: created };
    return created;
  };

  const openAudioMixer = async () => {
    if (openAdditionalSettingsInWindows && auxWindowMode !== "audio-mixer") {
      await openAuxSettingsWindow("audio-mixer");
      return;
    }
    for (const source of audioMixerSources) {
      ensureAudioMixerState(source);
    }
    showAudioMixerModal = true;
  };

  const closeAudioMixer = async () => {
    for (const timer of Object.values(audioMixerVolumeRealtimeTimers)) {
      clearTimeout(timer);
    }
    audioMixerVolumeRealtimeTimers = {};
    showAudioMixerModal = false;
    audioMixerMenu = { open: false, x: 0, y: 0, sourceId: null };
    showAudioMixerAdvancedModal = false;
    showAudioFiltersModal = false;
    audioMixerAdvancedSourceId = null;
    audioMixerRenameSourceId = null;
    audioMixerDragX = 0;
    audioMixerDragY = 0;
    audioMixerDragActive = false;
    if (auxWindowMode === "audio-mixer") {
      await closeCurrentAuxWindow();
    }
  };

  const updateAudioMixerState = (sourceId: string, patch: Partial<AudioMixerItemState>) => {
    const current = audioMixerState[sourceId];
    if (!current) return;
    audioMixerState = {
      ...audioMixerState,
      [sourceId]: { ...current, ...patch }
    };
  };

  const setAudioMixerOrientation = (orientation: "vertical" | "horizontal") => {
    audioMixerOrientation = orientation;
  };

  const openAudioMixerContextMenu = (event: MouseEvent, sourceId: string) => {
    event.preventDefault();
    event.stopPropagation();
    const menuWidth = 210;
    const menuHeight = 92;
    const margin = 10;
    const preferredLeft = event.clientX - menuWidth + 24;
    const preferredTop = event.clientY;
    const maxLeft = typeof window !== "undefined" ? window.innerWidth - menuWidth - margin : preferredLeft;
    const maxTop = typeof window !== "undefined" ? window.innerHeight - menuHeight - margin : preferredTop;
    audioMixerMenu = {
      open: true,
      x: Math.max(margin, Math.min(preferredLeft, maxLeft)),
      y: Math.max(margin, Math.min(preferredTop, maxTop)),
      sourceId
    };
  };

  const closeAudioMixerContextMenu = () => {
    audioMixerMenu = { open: false, x: 0, y: 0, sourceId: null };
  };

  const toggleAudioMixerLock = async (sourceId: string) => {
    const current = audioMixerState[sourceId];
    if (!current) return;
    const next = !current.locked;
    updateAudioMixerState(sourceId, { locked: next });
    await persistAudioMixerSource(sourceId, { locked: next });
  };

  const setAudioMixerVolumeMode = async (sourceId: string, mode: "percent" | "db") => {
    const current = audioMixerState[sourceId];
    if (!current) return;
    if (mode === current.volumeMode) return;
    const nextPatch = mode === "db"
      ? { volumeMode: mode, volumeDb: percentToDb(current.volumePercent) }
      : { volumeMode: mode, volumePercent: dbToPercent(current.volumeDb) };
    updateAudioMixerState(sourceId, nextPatch);
    await persistAudioMixerSource(sourceId, nextPatch);
  };

  const scheduleAudioMixerRealtimeVolumePersist = (sourceId: string) => {
    if (!sourceId) return;
    const existing = audioMixerVolumeRealtimeTimers[sourceId];
    if (existing) {
      clearTimeout(existing);
    }
    audioMixerVolumeRealtimeTimers = {
      ...audioMixerVolumeRealtimeTimers,
      [sourceId]: setTimeout(() => {
        const state = audioMixerState[sourceId];
        if (!state || state.locked) return;
        const patch = state.volumeMode === "db"
          ? { volumeDb: clampAudioMixerDb(state.volumeDb), volumePercent: dbToPercent(state.volumeDb) }
          : { volumePercent: clampAudioMixerPercent(state.volumePercent), volumeDb: percentToDb(state.volumePercent) };
        void persistAudioMixerSource(sourceId, patch);

        const { [sourceId]: _removed, ...rest } = audioMixerVolumeRealtimeTimers;
        audioMixerVolumeRealtimeTimers = rest;
      }, 90)
    };
  };

  const setAudioMixerVolumePercentLocal = (sourceId: string, value: number) => {
    const current = audioMixerState[sourceId];
    if (!current || current.locked) return;
    const volumePercent = clampAudioMixerPercent(value);
    updateAudioMixerState(sourceId, { volumePercent, volumeDb: percentToDb(volumePercent) });
    scheduleAudioMixerRealtimeVolumePersist(sourceId);
  };

  const commitAudioMixerVolumePercent = async (sourceId: string) => {
    const state = audioMixerState[sourceId];
    if (!state || state.locked) return;
    await persistAudioMixerSource(sourceId, {
      volumePercent: clampAudioMixerPercent(state.volumePercent),
      volumeDb: percentToDb(state.volumePercent)
    });
  };

  const setAudioMixerVolumeDbLocal = (sourceId: string, value: number) => {
    const current = audioMixerState[sourceId];
    if (!current || current.locked) return;
    const volumeDb = clampAudioMixerDb(value);
    updateAudioMixerState(sourceId, { volumeDb, volumePercent: dbToPercent(volumeDb) });
    scheduleAudioMixerRealtimeVolumePersist(sourceId);
  };

  const commitAudioMixerVolumeDb = async (sourceId: string) => {
    const state = audioMixerState[sourceId];
    if (!state || state.locked) return;
    await persistAudioMixerSource(sourceId, {
      volumeDb: clampAudioMixerDb(state.volumeDb),
      volumePercent: dbToPercent(state.volumeDb)
    });
  };

  const setAudioMixerMonitoring = async (sourceId: string, monitoring: string) => {
    updateAudioMixerState(sourceId, { monitoring });
    if (quickDeviceSource?.id === sourceId) {
      quickDeviceMonitoring = monitoring;
    }
    await persistAudioMixerSource(sourceId, { monitoring });
  };

  const getAudioMixerBalancePan = (state: AudioMixerItemState) => {
    const left = Math.max(0, Math.min(100, state.balanceLeft));
    const right = Math.max(0, Math.min(100, state.balanceRight));
    if (right >= left) {
      return Math.max(0, Math.min(100, Math.round(100 - left)));
    }
    return -Math.max(0, Math.min(100, Math.round(100 - right)));
  };

  const setAudioMixerBalancePanLocal = (sourceId: string, pan: number) => {
    const current = audioMixerState[sourceId];
    if (!current) return;
    const clampedPan = Math.max(-100, Math.min(100, pan));
    const patch = clampedPan >= 0
      ? { balanceLeft: 100 - clampedPan, balanceRight: 100 }
      : { balanceLeft: 100, balanceRight: 100 + clampedPan };
    updateAudioMixerState(sourceId, patch);
  };

  const commitAudioMixerBalancePan = async (sourceId: string) => {
    const state = audioMixerState[sourceId];
    if (!state) return;
    const patch = {
      balanceLeft: Math.max(0, Math.min(100, state.balanceLeft)),
      balanceRight: Math.max(0, Math.min(100, state.balanceRight))
    };
    updateAudioMixerState(sourceId, patch);
    await persistAudioMixerSource(sourceId, patch);
  };

  const toggleAudioMixerTrack = async (sourceId: string, track: string) => {
    const current = audioMixerState[sourceId];
    if (!current) return;
    const hasTrack = current.tracks.includes(track);
    let tracks = hasTrack ? current.tracks.filter((t) => t !== track) : [...current.tracks, track];
    tracks = [...new Set(tracks)].sort((a, b) => Number(a) - Number(b));
    if (!tracks.length) tracks = [track];
    updateAudioMixerState(sourceId, { tracks });
    await persistAudioMixerSource(sourceId, { tracks });
  };

  const persistAudioMixerSource = async (sourceId: string, statePatch?: Partial<AudioMixerItemState>, renameTo?: string) => {
    const source = sourcesList.find((s) => s.id === sourceId);
    if (!source) return;
    const current = ensureAudioMixerState(source);
    const merged = { ...current, ...(statePatch ?? {}) };
    updateAudioMixerState(sourceId, merged);

    const params = {
      ...(source.params ?? {}),
      volume_percent: String(Math.round(merged.volumePercent)),
      volume_db: String(merged.volumeDb),
      monitoring: merged.monitoring,
      balance_left: String(Math.round(merged.balanceLeft)),
      balance_right: String(Math.round(merged.balanceRight)),
      audio_tracks: merged.tracks.join(","),
      volume_locked: merged.locked ? "true" : "false",
      volume_mode: merged.volumeMode
    };

    const nextName = (renameTo ?? source.name).trim() || source.name;

    if (!backendEnabled) {
      sourcesList = sourcesList.map((s) =>
        s.id === sourceId
          ? { ...s, name: nextName, params }
          : s
      );
      previewDirty = true;
      await refreshPreview(true);
      return;
    }

    try {
      await invoke<string>("obs_update_source", {
        update: {
          id: source.id,
          name: nextName,
          source_type: source.source_type,
          params
        }
      });
      await loadSources();
      requestPreviewUpdate();
    } catch (err) {
      showGlobalDialog(String(err), "error");
    }
  };

  const openAudioMixerAdvanced = async (sourceId: string) => {
    if (openAdditionalSettingsInWindows && auxWindowMode !== "audio-advanced") {
      await openAuxSettingsWindow("audio-advanced", sourceId);
      closeAudioMixerContextMenu();
      return;
    }
    audioMixerAdvancedSourceId = sourceId;
    showAudioMixerAdvancedModal = true;
    closeAudioMixerContextMenu();
  };

  const closeAudioMixerAdvanced = async () => {
    showAudioMixerAdvancedModal = false;
    audioMixerAdvancedSourceId = null;
    audioAdvancedDragActive = false;
    audioAdvancedDragX = 0;
    audioAdvancedDragY = 0;
    if (auxWindowMode === "audio-advanced") {
      await closeCurrentAuxWindow();
    }
  };

  const openAudioMixerFilters = async (sourceId: string) => {
    if (openAdditionalSettingsInWindows && auxWindowMode !== "audio-filters") {
      await openAuxSettingsWindow("audio-filters", sourceId);
      closeAudioMixerContextMenu();
      return;
    }
    const source = sourcesList.find((s) => s.id === sourceId);
    if (!source) return;
    audioFiltersSourceId = sourceId;
    audioFiltersSourceLabel = source.name;
    audioFiltersDraft = (audioSourceFiltersMap[sourceId] ?? []).map((f) => ({ ...f, params: { ...(f.params ?? {}) } }));
    audioFiltersSelectedId = audioFiltersDraft[0]?.id ?? null;
    if (audioFiltersSelectedId) ensureAudioFilterPresetDefaults(audioFiltersSelectedId);
    audioFiltersContextMenu = { open: false, x: 0, y: 0, filterId: null };
    audioFiltersRenamingId = null;
    audioFiltersRenameValue = "";
    audioFiltersPreviewUrl = "";
    audioFilterNewKind = "noise_suppression";
    showAudioFiltersModal = true;
    scheduleAudioFiltersPreviewRefresh();
    closeAudioMixerContextMenu();
  };

  const closeAudioFiltersModal = async () => {
    showAudioFiltersModal = false;
    audioFiltersSourceId = null;
    audioFiltersSourceLabel = "";
    audioFiltersDraft = [];
    audioFiltersSelectedId = null;
    audioFiltersContextMenu = { open: false, x: 0, y: 0, filterId: null };
    audioFiltersRenamingId = null;
    audioFiltersRenameValue = "";
    audioFiltersPreviewUrl = "";
    if (audioFiltersPreviewRefreshTimer) {
      clearTimeout(audioFiltersPreviewRefreshTimer);
      audioFiltersPreviewRefreshTimer = null;
    }
    audioFiltersDragActive = false;
    audioFiltersDragX = 0;
    audioFiltersDragY = 0;
    if (auxWindowMode === "audio-filters") {
      await closeCurrentAuxWindow();
    }
  };

  const openTemplatesDialog = () => {
    showTemplatesFutureDialog = true;
  };

  const closeTemplatesDialog = () => {
    showTemplatesFutureDialog = false;
  };

  const saveAudioFiltersModal = async () => {
    if (!audioFiltersSourceId) return;
    const sourceId = audioFiltersSourceId;
    const nextFilters = audioFiltersDraft.map((f) => ({ ...f, params: { ...(f.params ?? {}) } }));
    audioSourceFiltersMap = {
      ...audioSourceFiltersMap,
      [sourceId]: nextFilters
    };

    if (backendEnabled && sourceId.trim()) {
      try {
        await invoke<string>("obs_set_source_filters", {
          sourceId,
          filters: nextFilters
        });
      } catch (err) {
        showGlobalDialog(`Audio filters apply failed: ${String(err)}`, "error");
        return;
      }
    }

    requestPreviewUpdate();
    showGlobalDialog("Audio filters saved", "info", 1300);
    if (auxWindowMode === "audio-filters") {
      await closeCurrentAuxWindow();
      return;
    }
    showAudioFiltersModal = false;
  };

  const ensureAudioFilterPresetDefaults = (filterId: string) => {
    const target = audioFiltersDraft.find((f) => f.id === filterId);
    if (!target) return;
    const fields = audioFilterPresetFields[target.kind] ?? [];
    if (!fields.length) return;
    const params = { ...(target.params ?? {}) };
    let changed = false;
    for (const field of fields) {
      if (!(field.key in params)) {
        params[field.key] = field.defaultValue;
        changed = true;
      }
    }
    if (!changed) return;
    audioFiltersDraft = audioFiltersDraft.map((f) => (f.id === filterId ? { ...f, params } : f));
  };

  const addAudioFilter = () => {
    const nextId = crypto.randomUUID ? crypto.randomUUID() : `${Date.now()}-${Math.random()}`;
    const defaultName = `Filter ${audioFiltersDraft.length + 1}`;
    audioFiltersDraft = [
      ...audioFiltersDraft,
      {
        id: nextId,
        name: defaultName,
        kind: audioFilterNewKind,
        enabled: true,
        locked: false,
        params: {}
      }
    ];
    audioFiltersSelectedId = nextId;
    ensureAudioFilterPresetDefaults(nextId);
    scheduleAudioFiltersPreviewRefresh();
  };

  const removeAudioFilter = (id: string) => {
    const next = audioFiltersDraft.filter((f) => f.id !== id);
    audioFiltersDraft = next;
    if (audioFiltersSelectedId === id) {
      audioFiltersSelectedId = next[0]?.id ?? null;
    }
    scheduleAudioFiltersPreviewRefresh();
    audioFiltersContextMenu = { open: false, x: 0, y: 0, filterId: null };
  };

  const updateAudioFilter = (id: string, patch: Partial<FilterItem>) => {
    audioFiltersDraft = audioFiltersDraft.map((f) => (f.id === id && !f.locked ? { ...f, ...patch } : f));
    ensureAudioFilterPresetDefaults(id);
    scheduleAudioFiltersPreviewRefresh();
  };

  const moveAudioFilter = (id: string, direction: "up" | "down") => {
    const index = audioFiltersDraft.findIndex((f) => f.id === id);
    if (index < 0) return;
    const filter = audioFiltersDraft[index];
    if (filter?.locked) return;
    const target = direction === "up" ? index - 1 : index + 1;
    if (target < 0 || target >= audioFiltersDraft.length) return;
    const next = [...audioFiltersDraft];
    const [moved] = next.splice(index, 1);
    next.splice(target, 0, moved);
    audioFiltersDraft = next;
    scheduleAudioFiltersPreviewRefresh();
  };

  const openAudioFiltersContextMenu = (event: MouseEvent, filterId: string) => {
    event.preventDefault();
    event.stopPropagation();
    audioFiltersSelectedId = filterId;
    audioFiltersContextMenu = { open: true, x: event.clientX, y: event.clientY, filterId };
  };

  const closeAudioFiltersContextMenu = () => {
    audioFiltersContextMenu = { open: false, x: 0, y: 0, filterId: null };
  };

  const selectAudioFilter = (id: string) => {
    audioFiltersSelectedId = id;
    ensureAudioFilterPresetDefaults(id);
  };

  const startAudioFilterRename = (id: string) => {
    const filter = audioFiltersDraft.find((f) => f.id === id);
    if (!filter || filter.locked) return;
    audioFiltersRenamingId = id;
    audioFiltersRenameValue = filter.name ?? "";
    closeAudioFiltersContextMenu();
  };

  const commitAudioFilterRename = (id: string) => {
    updateAudioFilter(id, { name: audioFiltersRenameValue });
    audioFiltersRenamingId = null;
  };

  const toggleAudioFilterLock = (id: string) => {
    audioFiltersDraft = audioFiltersDraft.map((f) => (f.id === id ? { ...f, locked: !f.locked } : f));
    scheduleAudioFiltersPreviewRefresh();
    closeAudioFiltersContextMenu();
  };

  const updateAudioFilterPresetField = (filterId: string, key: string, value: string) => {
    const filter = audioFiltersDraft.find((f) => f.id === filterId);
    if (!filter || filter.locked) return;
    const params = { ...(filter.params ?? {}) };
    params[key] = value;
    updateAudioFilter(filterId, { params });
  };

  const resetSelectedAudioFilterToDefaults = () => {
    if (!selectedAudioFilter || selectedAudioFilter.locked) return;
    const index = audioFiltersDraft.findIndex((f) => f.id === selectedAudioFilter.id);
    if (index < 0) return;
    const fields = audioFilterPresetFields[selectedAudioFilter.kind] ?? [];
    const defaultParams: Record<string, string> = {};
    for (const field of fields) {
      defaultParams[field.key] = field.defaultValue;
    }
    updateAudioFilter(selectedAudioFilter.id, {
      enabled: true,
      name: `Filter ${index + 1}`,
      params: defaultParams
    });
    scheduleAudioFiltersPreviewRefresh();
  };

  const refreshAudioFiltersPreview = async () => {
    try {
      const image = await invoke<string>("obs_take_screenshot", {
        width: 640,
        height: 360,
        sourceId: audioFiltersSourceId && audioFiltersSourceId.trim().length ? audioFiltersSourceId : undefined
      });
      if (typeof image === "string" && image.trim()) {
        if (image.startsWith("data:image")) {
          audioFiltersPreviewUrl = image;
        } else {
          audioFiltersPreviewUrl = `${convertFileSrc(image)}?t=${Date.now()}`;
        }
      }
    } catch {
      // ignore when backend is unavailable
    }
  };

  const scheduleAudioFiltersPreviewRefresh = () => {
    if (!showAudioFiltersModal) return;
    if (audioFiltersPreviewRefreshTimer) clearTimeout(audioFiltersPreviewRefreshTimer);
    audioFiltersPreviewRefreshTimer = setTimeout(() => {
      audioFiltersPreviewRefreshTimer = null;
      void refreshAudioFiltersPreview();
    }, 140);
  };

  const startAudioMixerRename = (sourceId: string, currentName: string) => {
    audioMixerRenameSourceId = sourceId;
    audioMixerRenameValue = currentName;
  };

  const commitAudioMixerRename = async (sourceId: string) => {
    const source = sourcesList.find((s) => s.id === sourceId);
    if (!source) return;
    await persistAudioMixerSource(sourceId, undefined, audioMixerRenameValue);
    audioMixerRenameSourceId = null;
  };

  const quickSelectImageFile = async (source: DemoSource) => {
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [
        { name: "Images", extensions: ["png", "jpg", "jpeg", "webp", "bmp", "gif", "svg"] }
      ]
    });
    const selectedPath = resolveDialogSelection(selected);
    if (!selectedPath) return;

    const nextParams = { ...(source.params ?? {}), file: selectedPath };

    if (!backendEnabled) {
      sourcesList = sourcesList.map((s) =>
        s.id === source.id ? { ...s, params: nextParams } : s
      );
      previewDirty = true;
      await refreshPreview(true);
      showGlobalDialog("Image file updated", "info", 1200);
      return;
    }

    try {
      await invoke<string>("obs_update_source", {
        update: {
          id: source.id,
          name: source.name,
          source_type: source.source_type,
          params: nextParams
        }
      });
      await loadSources();
      requestPreviewUpdate();
      showGlobalDialog("Image file updated", "info", 1200);
    } catch (err) {
      showGlobalDialog(String(err), "error");
    }
  };

  const openSceneFilters = (scene: SceneInfo) => {
    filtersTargetType = "scene";
    filtersTargetId = scene.name;
    filtersTargetLabel = scene.name;
    filtersDraft = cloneFilterList(sceneFiltersMap[scene.name] ?? []);
    filtersSnapshotBeforeOpen = cloneFilterList(sceneFiltersMap[scene.name] ?? []);
    showFiltersModal = true;
    closeSceneMenu();
  };

  const closeFiltersModal = async () => {
    if (sourceFiltersLiveTimer) {
      clearTimeout(sourceFiltersLiveTimer);
      sourceFiltersLiveTimer = null;
    }
    sourceFiltersLiveQueued = null;

    if (filtersTargetType === "source" && backendEnabled && filtersSnapshotBeforeOpen) {
      try {
        await applySourceFiltersToBackend(filtersTargetId, filtersSnapshotBeforeOpen);
      } catch (err) {
        showGlobalDialog(String(err), "error");
      }
      sourceFiltersMap = { ...sourceFiltersMap, [filtersTargetId]: cloneFilterList(filtersSnapshotBeforeOpen) };
    }

    filtersSnapshotBeforeOpen = null;
    showFiltersModal = false;
  };

  const saveFiltersModal = async (event: CustomEvent<{ filters: FilterItem[] }>) => {
    const filters = cloneFilterList(event.detail?.filters ?? []);
    if (filtersTargetType === "source") {
      sourceFiltersMap = { ...sourceFiltersMap, [filtersTargetId]: filters };
      if (backendEnabled) {
        try {
          await applySourceFiltersToBackend(filtersTargetId, filters);
        } catch (err) {
          showGlobalDialog(String(err), "error");
          return;
        }
      }
    } else {
      sceneFiltersMap = { ...sceneFiltersMap, [filtersTargetId]: filters };
    }

    if (sourceFiltersLiveTimer) {
      clearTimeout(sourceFiltersLiveTimer);
      sourceFiltersLiveTimer = null;
    }
    sourceFiltersLiveQueued = null;
    filtersSnapshotBeforeOpen = null;
    showFiltersModal = false;
    showGlobalDialog("Filters saved", "info", 1500);
  };

  const getUniqueName = (base: string, existing: Set<string>) => {
    if (!existing.has(base)) return base;
    let idx = 2;
    while (existing.has(`${base} ${idx}`)) idx += 1;
    return `${base} ${idx}`;
  };

  const cloneSource = async (source: DemoSource) => {
    const nameSet = new Set(sourcesList.map((s) => s.name));
    const idSet = new Set(sourcesList.map((s) => s.id));
    const newName = getUniqueName(source.name, nameSet);
    const newId = getUniqueName(source.id, idSet);
    if (backendEnabled) {
      try {
        await invoke<string>("obs_create_source", {
          create: {
            id: newId,
            name: newName,
            source_type: source.source_type,
            params: source.params ?? {}
          }
        });
        await loadSources();
        requestPreviewUpdate();
      } catch (err) {
        showGlobalDialog(String(err), "error");
      }
    } else {
      sourcesList = [
        ...sourcesList,
        { ...source, id: newId, name: newName }
      ];
      requestPreviewUpdate();
    }
    closeSourceMenu();
  };

  const removeSource = async (source: DemoSource) => {
    if (backendEnabled) {
      try {
        await invoke<string>("obs_remove_source", { id: source.id });
        await loadSources();
        requestPreviewUpdate();
      } catch (err) {
        showGlobalDialog(String(err), "error");
      }
    } else {
      sourcesList = sourcesList.filter((s) => s.id !== source.id);
      requestPreviewUpdate();
    }
    if (sourceFiltersMap[source.id]) {
      const { [source.id]: _removed, ...rest } = sourceFiltersMap;
      sourceFiltersMap = rest;
    }
    if (audioSourceFiltersMap[source.id]) {
      const { [source.id]: _removedAudio, ...restAudio } = audioSourceFiltersMap;
      audioSourceFiltersMap = restAudio;
    }
    closeSourceMenu();
  };

  const openSourceMenu = (event: MouseEvent, source: DemoSource) => {
    event.preventDefault();
    closePreviewMenu();
    closeAppContextMenu();
    closeAudioMixerContextMenu();
    const margin = 10;
    const estimatedWidth = 220;
    const estimatedHeight = 340;
    const maxX = Math.max(margin, window.innerWidth - estimatedWidth - margin);
    const maxY = Math.max(margin, window.innerHeight - estimatedHeight - margin);
    const x = Math.min(Math.max(event.clientX, margin), maxX);
    const y = Math.min(Math.max(event.clientY, margin), maxY);
    sourceMenu = { open: true, x, y, source };
  };

  const closeSourceMenu = () => {
    sourceMenu = { open: false, x: 0, y: 0, source: null };
  };

  const openSceneMenu = (event: MouseEvent, scene: SceneInfo) => {
    event.preventDefault();
    closePreviewMenu();
    closeAppContextMenu();
    closeAudioMixerContextMenu();
    const estimatedWidth = 190;
    const estimatedHeight = 180;
    const margin = 8;
    const maxX = Math.max(margin, window.innerWidth - estimatedWidth - margin);
    const maxY = Math.max(margin, window.innerHeight - estimatedHeight - margin);
    const x = Math.min(Math.max(event.clientX, margin), maxX);
    const y = Math.min(Math.max(event.clientY, margin), maxY);
    sceneMenu = { open: true, x, y, scene };
  };

  const closeSceneMenu = () => {
    sceneMenu = { open: false, x: 0, y: 0, scene: null };
  };

  const openPreviewMenu = (event: MouseEvent) => {
    event.preventDefault();
    closeAppContextMenu();
    closeAudioMixerContextMenu();
    closeSourceMenu();
    closeSceneMenu();
    closeDockMenu();
    const margin = 8;
    const estimatedWidth = 220;
    const estimatedHeight = 96;
    const maxX = Math.max(margin, window.innerWidth - estimatedWidth - margin);
    const maxY = Math.max(margin, window.innerHeight - estimatedHeight - margin);
    const x = Math.min(Math.max(event.clientX, margin), maxX);
    const y = Math.min(Math.max(event.clientY, margin), maxY);
    previewMenu = { open: true, x, y };
  };

  const closePreviewMenu = () => {
    previewMenu = { open: false, x: 0, y: 0 };
  };

  const refreshRenderFrame = async () => {
    closePreviewMenu();
    if (!backendEnabled || !isObsRunning || demoMode) return;
    try {
      const liveResolution = await invoke<string>("obs_get_current_scene_resolution");
      const trimmed = typeof liveResolution === "string" ? liveResolution.trim() : "";
      if (/^\d+x\d+$/.test(trimmed) && trimmed !== sceneResolution) {
        sceneResolution = trimmed;
        await refreshPreviewAfterResolutionChange();
        showGlobalDialog(`Render refreshed (${trimmed})`, "info", 1200);
        return;
      }
      previewDirty = true;
      await refreshPreview(true);
      requestPreviewUpdate();
      showGlobalDialog("Render refreshed", "info", 1000);
    } catch (err) {
      showGlobalDialog(String(err), "error");
    }
  };

  const openGraphicPlannerFromPreviewMenu = async () => {
    closePreviewMenu();
    await openSourceTransformModal();
  };

  const openAppContextMenu = (event: MouseEvent) => {
    if (!useCustomContextMenu) return;
    if (event.defaultPrevented) return;
    event.preventDefault();
    closePreviewMenu();
    closeSourceMenu();
    closeSceneMenu();
    closeAudioMixerContextMenu();
    appContextMenu = { open: true, x: event.clientX, y: event.clientY };
  };

  const closeAppContextMenu = () => {
    if (!useCustomContextMenu) return;
    appContextMenu = { open: false, x: 0, y: 0 };
  };

    const openDockMenu = (event: MouseEvent) => {
      event.preventDefault();
      closePreviewMenu();
      closeSourceMenu();
      closeSceneMenu();
      closeAppContextMenu();
      closeAudioMixerContextMenu();
      dockMenu = { open: true, x: event.clientX, y: event.clientY };
    };

    const closeDockMenu = () => {
      dockMenu = { open: false, x: 0, y: 0 };
    };

    const moveDockToSide = async (side: "left" | "right" | "bottom") => {
      closeDockMenu();
      showDockHandle = true;
      if (dockPinnedSide === side) {
        refreshDockPane();
        return;
      }
      await attachDockToSide(side);
    };

    const removeDockFromWorkspace = async () => {
      closeDockMenu();
      await disposeDockInlineWebview();
      dockPinnedSide = null;
      showDockHandle = false;
      dockDragActive = false;
      dockDropTarget = null;
      dockDropCommitted = false;
      stopDockCanvasLoop();
      if (tauriAvailable) {
        try {
          await invoke<void>("close_browser_dock");
        } catch {
          // ignore close errors
        }
      }
    };

  const reloadApp = () => {
    closeAppContextMenu();
    if (typeof window !== "undefined") {
      window.location.reload();
    }
  };

  const openHomepage = () => {
    closeAppContextMenu();
    if (typeof window !== "undefined") {
      window.open("https://revostream.in", "_blank", "noopener,noreferrer");
    }
  };

  const openInspectMode = async () => {
    closeAppContextMenu();
    if (!canInspect || !tauriAvailable) return;
    try {
      const { getCurrentWebviewWindow } = await import("@tauri-apps/api/webviewWindow");
      const currentWindow = getCurrentWebviewWindow() as unknown as {
        openDevtools?: () => void;
      };
      currentWindow.openDevtools?.();
    } catch {
      // no-op
    }
  };

  const canUseForceClose = () => !isStreaming && !isRecording;

  const forceCloseFromContextMenu = async () => {
    closeAppContextMenu();
    if (!canUseForceClose()) {
      showGlobalDialog("Force close is available only when streaming/recording is stopped.", "warning", 2200);
      return;
    }
    await closeEntireApp();
  };

  const cloneScene = async (scene: SceneInfo) => {
    const nameSet = new Set(scenes.map((s) => s.name));
    const newName = getUniqueName(scene.name, nameSet);
    if (backendEnabled) {
      try {
        await invoke<string>("obs_set_current_scene", { name: scene.name });
        const list = await invoke<DemoSource[]>("obs_list_sources");
        await invoke<string>("obs_create_scene", { name: newName });
        await invoke<string>("obs_set_current_scene", { name: newName });
        for (const s of list) {
          const idSet = new Set(list.map((it) => it.id));
          const newId = s.id === "accent" || s.id === "title"
            ? getUniqueName(`${s.id}-copy`, idSet)
            : s.id;
          await invoke<string>("obs_create_source", {
            create: {
              id: newId,
              name: s.name,
              source_type: s.source_type,
              params: s.params ?? {}
            }
          });
        }
        await loadScenes();
        await loadSources();
        requestPreviewUpdate();
      } catch (err) {
        showGlobalDialog(String(err), "error");
      }
    } else {
      scenes = [...scenes, { name: newName, active: false, locked: false }];
    }
    closeSceneMenu();
  };

  const handleBackdropKey = (event: KeyboardEvent, closeFn: () => void | Promise<void>) => {
    if (event.key === "Escape") {
      void closeFn();
    }
  };

  const MODAL_DIALOG_SELECTOR = '[role="dialog"][aria-modal="true"]';
  const FOCUSABLE_SELECTOR = [
    'a[href]',
    'button:not([disabled])',
    'input:not([disabled])',
    'select:not([disabled])',
    'textarea:not([disabled])',
    '[tabindex]:not([tabindex="-1"])'
  ].join(",");

  const getActiveModalDialog = (): HTMLElement | null => {
    if (typeof document === "undefined") return null;
    const dialogs = Array.from(document.querySelectorAll<HTMLElement>(MODAL_DIALOG_SELECTOR));
    for (let i = dialogs.length - 1; i >= 0; i -= 1) {
      const dialog = dialogs[i];
      if (dialog && dialog.getClientRects().length > 0) {
        return dialog;
      }
    }
    return null;
  };

  const getFocusableInDialog = (dialog: HTMLElement): HTMLElement[] =>
    Array.from(dialog.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(
      (el) => el.getClientRects().length > 0 && !el.hasAttribute("disabled")
    );

  const focusDialogEntry = (dialog: HTMLElement) => {
    const focusables = getFocusableInDialog(dialog);
    const nextTarget = focusables[0] ?? dialog;
    nextTarget.focus();
  };

  const isModalKeyboardScopeActive = (target: EventTarget | null) => {
    const dialog = getActiveModalDialog();
    if (!dialog) return false;
    if (!(target instanceof Node)) return true;
    return !dialog.contains(target);
  };

  const isTypingTarget = (event: KeyboardEvent) => {
    const target = event.target as HTMLElement | null;
    if (!target) return false;
    if (target.isContentEditable) return true;
    const tag = target.tagName.toLowerCase();
    return tag === "input" || tag === "textarea" || tag === "select";
  };

  const toggleKeyboardShortcutsHelp = () => {
    showKeyboardShortcutsHelp = !showKeyboardShortcutsHelp;
  };

  let lastFocusedModalDialog: HTMLElement | null = null;

  $: {
    if (typeof document === "undefined") {
      lastFocusedModalDialog = null;
    } else {
      const activeModal = getActiveModalDialog();
      if (!activeModal) {
        lastFocusedModalDialog = null;
      } else if (activeModal !== lastFocusedModalDialog) {
        lastFocusedModalDialog = activeModal;
        void tick().then(() => focusDialogEntry(activeModal));
      }
    }
  }

  const focusRegion = (el: HTMLElement | null, label: string) => {
    if (!el) return;
    el.focus();
    showGlobalDialog(`${label} focused`, "info", 900);
  };

  const openDebugConsole = async () => {
    try {
      await invoke<void>("open_debug_console");
    } catch (err) {
      showGlobalDialog(`Debug console open failed: ${String(err)}`, "error");
    }
  };
</script>

<svelte:window
  on:contextmenu={openAppContextMenu}
  on:click={() => {
    closeAppContextMenu();
    closePreviewMenu();
  }}
  on:keydown={(e) => {
    const ctrlQuestion = e.ctrlKey && (e.key === "?" || e.key === "/" || (e.key === "/" && e.shiftKey));

    if (isModalKeyboardScopeActive(e.target)) {
      const activeModal = getActiveModalDialog();
      if (activeModal) {
        e.preventDefault();
        e.stopPropagation();
        focusDialogEntry(activeModal);
      }
      return;
    }

    if (ctrlQuestion) {
      e.preventDefault();
      toggleKeyboardShortcutsHelp();
      return;
    }

    if (showKeyboardShortcutsHelp && e.key === "Escape") {
      e.preventDefault();
      showKeyboardShortcutsHelp = false;
      return;
    }

    if (isTypingTarget(e)) {
      return;
    }

    if (e.key === "F10") {
      e.preventDefault();
      void openDebugConsole();
      return;
    }

    if (e.altKey && e.key === "1") {
      e.preventDefault();
      focusRegion(panelScenesEl, "Scenes");
      return;
    }

    if (e.altKey && e.key === "2") {
      e.preventDefault();
      focusRegion(panelSourcesEl, "Sources");
      return;
    }

    if (e.altKey && e.key === "3") {
      e.preventDefault();
      focusRegion(panelToolsEl, "Tools");
      return;
    }

    if (e.altKey && e.key === "4") {
      e.preventDefault();
      focusRegion(previewFrameEl, "Preview");
      return;
    }

    if (e.key === "Escape") {
      closeAppContextMenu();
      closePreviewMenu();
    }
  }}
/>

{#if globalDialogs.length}
  <div class="global-dialog-stack">
    {#each globalDialogs as dialog (dialog.id)}
      <div class="global-dialog global-dialog-{dialog.type}">
        <div class="global-dialog-inner">
          <span class="icon">{dialog.icon}</span>
          <span>{dialog.text}</span>
        </div>
      </div>
    {/each}
  </div>
{/if}

<StreamConfirmModal
  open={showMediaConfirm}
  onAnswer={answerMediaConfirm}
  title={mediaConfirmTitle(pendingMediaConfirmAction)}
  message={mediaConfirmMessage(pendingMediaConfirmAction)}
  ariaLabel={mediaConfirmTitle(pendingMediaConfirmAction)}
/>

<StreamConfirmModal
  open={showCloseRiskConfirm}
  onAnswer={(accepted) => void answerCloseRiskConfirm(accepted)}
  title={closeRiskConfirmTitle()}
  message={closeRiskConfirmMessage()}
  ariaLabel={closeRiskConfirmTitle()}
  confirmLabel="Yes, close app"
  cancelLabel="No, I will stay"
/>

<StreamConfirmModal
  open={showUnsavedSettingsCloseConfirm}
  onAnswer={(accepted) => void answerUnsavedSettingsCloseConfirm(accepted)}
  title={unsavedSettingsCloseTitle()}
  message={unsavedSettingsCloseMessage()}
  ariaLabel={unsavedSettingsCloseTitle()}
  confirmLabel="Yes, close app"
  cancelLabel="No, I will stay"
/>

{#if showKeyboardShortcutsHelp}
  <div
    class="modal-backdrop"
    role="button"
    tabindex="0"
    on:click={() => (showKeyboardShortcutsHelp = false)}
    on:keydown={(e) => handleBackdropKey(e, () => {
      showKeyboardShortcutsHelp = false;
    })}
  >
    <div
      class="quick-text-modal shortcuts-help-modal"
      role="dialog"
      tabindex="-1"
      aria-modal="true"
      aria-label="Keyboard shortcuts"
      on:click|stopPropagation
      on:keydown|stopPropagation
    >
      <h3>Keyboard shortcuts</h3>
      <div class="shortcuts-help-list">
        <div><span>Ctrl + ? / Ctrl + /</span><span>Show/hide this dialog</span></div>
        <div><span>Esc</span><span>Close dialog / menu</span></div>
        <div><span>Alt + 1</span><span>Focus Scenes panel</span></div>
        <div><span>Alt + 2</span><span>Focus Sources panel</span></div>
        <div><span>Alt + 3</span><span>Focus Tools panel</span></div>
        <div><span>Alt + 4</span><span>Focus Preview area</span></div>
        <div><span>Enter / Space</span><span>Activate focused button/item</span></div>
        <div><span>Tab / Shift + Tab</span><span>Move focus forward/backward</span></div>
      </div>
      <div class="quick-text-actions">
        <button class="primary" on:click={() => (showKeyboardShortcutsHelp = false)}>Close</button>
      </div>
    </div>
  </div>
{/if}

<canvas class="dock-canvas-offscreen" bind:this={dockCanvasEl} aria-hidden="true"></canvas>

<main class:aux-window={Boolean(auxWindowMode)}>
  <div class="aux-only-host">
    {#if auxWindowMode === "plugins" && showPlugins}
      <PluginsModal
        windowMode={true}
        {pluginProfiles}
        {activePluginProfile}
        plugins={pluginsList}
        enabledModules={enabledPluginModules}
        baselineEnabledModules={pluginBaselineModulesByProfile[activePluginProfile] ?? enabledPluginModules}
        {busy}
        on:close={closePlugins}
        on:selectProfile={selectPluginProfile}
        on:createProfile={createPluginProfile}
        on:save={savePlugins}
      />
    {/if}

    {#if auxWindowMode === "edit-source" && showEditSource}
      <EditSourceModal
        windowMode={true}
        {editSource}
        {editName}
        {editType}
        {editParams}
        {audioInputDevices}
        {audioOutputDevices}
        {sourceParamSchemas}
        sourcePropertySpecs={sourcePropertySpecs}
        sourcePropertyEntries={sourcePropertyEntries}
        {extraParamEntries}
        {fontOptions}
        on:close={cancelEditSource}
        on:save={saveEditSource}
        on:updateName={(e) => updateEditNameValue(e.detail.value)}
        on:updateParam={(e) => updateParamValue(e.detail.key, e.detail.value)}
        on:renameParam={(e) => renameParamKey(e.detail.oldKey, e.detail.newKey)}
        on:removeParam={(e) => removeParam(e.detail.key)}
        on:resetProtectedParam={(e) => resetProtectedEditParam(e.detail.key)}
        on:addParam={(e) => addParamEntry(e.detail.key, e.detail.value)}
        on:requestLiveUpdate={scheduleEditSourceRealtimeUpdate}
      />
    {/if}
  </div>

  <style>
    .global-dialog-stack {
      position: fixed;
      top: 0;
      left: 0;
      width: 100vw;
      z-index: 30000;
      display: flex;
      flex-direction: column;
      align-items: center;
      pointer-events: none;
      gap: 0.5rem;
    }
    .global-dialog {
      display: flex;
      justify-content: center;
      width: auto;
      pointer-events: none;
      margin-top: 65px;
    }
    .global-dialog-inner {
      background: linear-gradient(90deg, #232838 60%, #151820 100%);
      color: #fff;
      border: 2px solid var(--border-strong);
      border-radius: 16px;
      box-shadow: 0 4px 24px #0008;
      padding: 0.85rem 2.2rem;
      font-size: 1.15rem;
      font-weight: 600;
      display: flex;
      align-items: center;
      gap: 1.1rem;
      pointer-events: all;
      letter-spacing: 0.02em;
      animation: global-dialog-fadein 0.3s;
    }
    .global-dialog-info .global-dialog-inner {
      border-color: var(--accent);
    }
    .global-dialog-warning .global-dialog-inner {
      border-color: var(--warning);
    }
    .global-dialog-error .global-dialog-inner {
      border-color: var(--danger);
    }
    .global-dialog-inner .icon {
      font-size: 1.45rem;
      filter: drop-shadow(0 0 2px #f59e0b);
    }

    .stream-confirm-modal {
      width: min(460px, calc(100vw - 2rem));
      min-width: 320px;
      gap: 1rem;
    }

    .stream-confirm-copy {
      margin: 0;
      color: var(--text);
      font-size: 0.98rem;
    }

    .stream-confirm-actions {
      display: flex;
      justify-content: flex-end;
      gap: 0.65rem;
    }

    .stream-confirm-btn {
      border: 1px solid var(--border-strong);
      border-radius: 10px;
      padding: 0.5rem 1rem;
      background: var(--surface-3);
      color: var(--text);
      cursor: pointer;
      font-weight: 600;
    }

    .stream-confirm-btn.primary {
      background: var(--accent);
      color: #fff;
      border-color: color-mix(in srgb, var(--accent) 70%, #000 30%);
    }

    .stream-confirm-btn.ghost {
      background: transparent;
    }

    .shortcuts-help-modal {
      width: min(620px, calc(100vw - 2rem));
      min-width: 340px;
      display: grid;
      gap: 0.75rem;
    }

    .shortcuts-help-list {
      display: grid;
      gap: 0.35rem;
    }

    .shortcuts-help-list > div {
      display: grid;
      grid-template-columns: 180px 1fr;
      gap: 0.8rem;
      align-items: center;
      padding: 0.35rem 0.45rem;
      border-radius: 8px;
      background: color-mix(in srgb, var(--surface-3) 68%, transparent);
      border: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
      font-size: 0.93rem;
    }

    .shortcuts-help-list > div > span:first-child {
      font-weight: 700;
      color: var(--text);
    }

    .shortcuts-help-list > div > span:last-child {
      color: var(--text-muted);
    }
    @keyframes global-dialog-fadein {
      from { opacity: 0; transform: translateY(-24px); }
      to { opacity: 1; transform: translateY(0); }
    }
  </style>

  <div class="header-shell" class:settings-open={showSettings}>
    <HeaderBar
      {demoMode}
      releaseMode={isReleaseBuild}
      {busy}
      {version}
      {isObsRunning}
      {isRecording}
      {isStreaming}
      on:openSettings={openSettings}
      on:startObs={startObs}
      on:stopObs={stopObs}
      on:startRecording={startRecording}
      on:stopRecording={stopRecording}
      on:startStreaming={startStreaming}
      on:stopStreaming={stopStreaming}
      on:forcePreviewResolution={forcePreviewResolution}
      {realtimeRefresh}
      on:toggleRealtimeRefresh={(e) => setRealtimeRefresh(Boolean(e.detail?.enabled))}
    />
  </div>

  {#if showSettings}
    <SettingsModal
      {allowDraggablePopups}
      bind:rootDir
      {demoMode}
      {isObsRunning}
      {version}
      bind:recordPath
      bind:streamUrl
      bind:streamKey
      bind:previewQuality
      {previewQualityOptions}
      bind:encoderPreference
      {encoderPreferenceOptions}
      bind:sceneResolution
      {sceneResolutionOptions}
      bind:whepUrl
      bind:whipUrl
      bind:autoRetryPreview
      {profileOptions}
      bind:selectedProfileName
      {busy}
      bind:autorescaleInputs
      initialAccessibilityHighContrast={accessibilityHighContrast}
      initialAccessibilityReduceMotion={accessibilityReduceMotion}
      initialAccessibilityFocusIndicators={accessibilityFocusIndicators}
      initialAccessibilityUiScale={accessibilityUiScale}
      initialAccessibilityFontSize={accessibilityFontSize}
      initialAccessibilityFontFamily={accessibilityFontFamily}
      initialAccessibilityColorVision={accessibilityColorVision}
      initialUiProfile={currentUiProfile}
      on:close={closeSettings}
      on:save={saveSettings}
      on:exportScenes={exportScenes}
      on:exportObsScenes={exportObsScenes}
      on:exportObsProfile={exportObsProfile}
      on:importScenes={(e) => importScenes(e.detail.content, e.detail.format)}
      on:importObsProfile={(e) => importObsProfile(e.detail.content)}
      on:createProfile={createProfile}
      on:switchProfile={switchProfile}
      on:toggleDemo={(e) => toggleDemo(e)}
      on:toggleAutorescaleInputs={(e) => (autorescaleInputs = e.detail.checked)}
      on:dirtyStateChange={(e) => (settingsHasUnsavedChanges = Boolean(e.detail?.dirty))}
    />
  {/if}

  {#if showPlugins && !auxWindowMode}
    <PluginsModal
      {allowDraggablePopups}
      {pluginProfiles}
      {activePluginProfile}
      plugins={pluginsList}
      enabledModules={enabledPluginModules}
      baselineEnabledModules={pluginBaselineModulesByProfile[activePluginProfile] ?? enabledPluginModules}
      {busy}
      on:close={closePlugins}
      on:selectProfile={selectPluginProfile}
      on:createProfile={createPluginProfile}
      on:save={savePlugins}
    />
  {/if}

  <Transitions
    open={showTransitionsModal}
    {transitionsDraft}
    {transitionsSelectedId}
    {activeTransitionId}
    {selectedTransitionItem}
    {transitionNewKind}
    {transitionNewName}
    {transitionKinds}
    {closeTransitions}
    {saveTransitions}
    {addTransition}
    {removeTransition}
    {setTransitionKind}
    {updateTransitionParam}
    {updateTransitionName}
    {pickStingerMediaFile}
    {pickStingerSequenceFolder}
    setTransitionsSelectedId={(id) => (transitionsSelectedId = id)}
    setActiveTransitionId={(id) => (activeTransitionId = id)}
    setTransitionNewKind={(kind) => (transitionNewKind = kind)}
    setTransitionNewName={(name) => (transitionNewName = name)}
    {handleBackdropKey}
  />

  <TransitionsDiscardConfirmModal
    open={showTransitionsDiscardConfirm}
    onCancel={cancelDiscardTransitions}
    onConfirm={confirmDiscardTransitions}
  />

  {#if showAddSource}
    <AddSourceModal
      bind:newSourceType
      {sourceTypes}
      {externalSourceTypes}
      on:close={() => (showAddSource = false)}
      on:add={addSource}
    />
  {/if}

  {#if showAddScene}
    <AddSceneModal
      bind:newSceneName
      scenePlaceholder={scenePlaceholder}
      on:close={() => (showAddScene = false)}
      on:add={addScene}
    />
  {/if}

  {#if showEditSource && !auxWindowMode}
    <EditSourceModal
      {allowDraggablePopups}
      {editSource}
      {editName}
      {editType}
      {editParams}
      {audioInputDevices}
      {audioOutputDevices}
      {sourceParamSchemas}
      sourcePropertySpecs={sourcePropertySpecs}
      sourcePropertyEntries={sourcePropertyEntries}
      {extraParamEntries}
      {fontOptions}
      on:close={cancelEditSource}
      on:save={saveEditSource}
      on:updateName={(e) => updateEditNameValue(e.detail.value)}
      on:updateParam={(e) => updateParamValue(e.detail.key, e.detail.value)}
      on:renameParam={(e) => renameParamKey(e.detail.oldKey, e.detail.newKey)}
      on:removeParam={(e) => removeParam(e.detail.key)}
      on:resetProtectedParam={(e) => resetProtectedEditParam(e.detail.key)}
      on:addParam={(e) => addParamEntry(e.detail.key, e.detail.value)}
      on:requestLiveUpdate={scheduleEditSourceRealtimeUpdate}
    />
  {/if}

  <FiltersModal
    open={showFiltersModal}
    targetType={filtersTargetType}
    targetId={filtersTargetId}
    targetLabel={filtersTargetLabel}
    filters={filtersDraft}
    on:close={closeFiltersModal}
    on:save={saveFiltersModal}
    on:liveChange={handleFiltersLiveChange}
  />

  <SourceInfoModal
    source={showSourceInfo ? selectedSource : null}
    on:close={() => (showSourceInfo = false)}
  />

  <QuickTextEditModal
    open={showTextEditModal && Boolean(textEditSource)}
    value={textEditValue}
    bind:modalEl={quickTextModalEl}
    {allowDraggablePopups}
    {openAdditionalSettingsInWindows}
    dragX={quickTextDragX}
    dragY={quickTextDragY}
    onClose={closeTextEditModal}
    onSave={saveTextEditModal}
    onValueChange={(value) => (textEditValue = value)}
    {handleBackdropKey}
    beginDrag={beginQuickTextDrag}
    moveDrag={moveQuickTextDrag}
    endDrag={endQuickTextDrag}
  />

  <QuickColorModal
    open={showQuickColorModal && Boolean(quickColorSource)}
    value={quickColorValue}
    recent={quickColorRecent}
    onClose={closeQuickColorModal}
    onSave={saveQuickColorModal}
    onValueChange={(value) => (quickColorValue = value)}
    normalizeColor={normalizeQuickHexColor}
    {handleBackdropKey}
  />

  <QuickDeviceModal
    open={showQuickDeviceModal && Boolean(quickDeviceSource)}
    value={quickDeviceValue}
    options={quickDeviceOptions}
    showMonitoring={Boolean(quickDeviceSource && isAudioDeviceSourceType(quickDeviceSource.source_type))}
    monitoring={quickDeviceMonitoring}
    monitoringOptions={quickMonitoringOptions}
    onClose={closeQuickDeviceModal}
    onSave={saveQuickDeviceModal}
    onValueChange={(value) => (quickDeviceValue = value)}
    onMonitoringChange={(value) => (quickDeviceMonitoring = value)}
    {handleBackdropKey}
  />

  <AudioMixer
    open={showAudioMixerModal}
    {closeAudioMixer}
    {handleBackdropKey}
    {allowDraggablePopups}
    {openAdditionalSettingsInWindows}
    {audioMixerDragX}
    {audioMixerDragY}
    {beginAudioMixerDrag}
    {moveAudioMixerDrag}
    {endAudioMixerDrag}
    bind:modalEl={audioMixerModalEl}
    {audioMixerOrientation}
    {setAudioMixerOrientation}
    {audioMixerSources}
    {ensureAudioMixerState}
    {getAudioMixerVisualLevel}
    {formatAudioMixerDb}
    {audioMixerRenameSourceId}
    {audioMixerRenameValue}
    setAudioMixerRenameValue={(value) => (audioMixerRenameValue = value)}
    setAudioMixerRenameSourceId={(id) => (audioMixerRenameSourceId = id)}
    {startAudioMixerRename}
    {commitAudioMixerRename}
    {openAudioMixerContextMenu}
    {setAudioMixerVolumeMode}
    {setAudioMixerVolumePercentLocal}
    {commitAudioMixerVolumePercent}
    {setAudioMixerVolumeDbLocal}
    {commitAudioMixerVolumeDb}
    {toggleAudioMixerLock}
    {AUDIO_MIXER_DB_MIN}
    {AUDIO_MIXER_DB_MAX}
  />

  <AudioMixerContextMenu
    open={audioMixerMenu.open}
    x={audioMixerMenu.x}
    y={audioMixerMenu.y}
    sourceId={audioMixerMenu.sourceId}
    onOpenFilters={openAudioMixerFilters}
    onOpenAdvanced={openAudioMixerAdvanced}
    onClose={closeAudioMixerContextMenu}
    {handleBackdropKey}
  />

  <AudioMixerAdvancedModal
    open={showAudioMixerAdvancedModal}
    source={audioMixerAdvancedSourceId ? (sourcesList.find((s) => s.id === audioMixerAdvancedSourceId) ?? null) : null}
    state={audioMixerAdvancedSourceId ? (() => {
      const src = sourcesList.find((s) => s.id === audioMixerAdvancedSourceId);
      return src ? ensureAudioMixerState(src) : null;
    })() : null}
    {allowDraggablePopups}
    {openAdditionalSettingsInWindows}
    bind:modalEl={audioAdvancedModalEl}
    dragX={audioAdvancedDragX}
    dragY={audioAdvancedDragY}
    monitoringOptions={quickMonitoringOptions}
    trackIds={allAudioTrackIds}
    beginDrag={beginAudioAdvancedDrag}
    moveDrag={moveAudioAdvancedDrag}
    endDrag={endAudioAdvancedDrag}
    onClose={() => void closeAudioMixerAdvanced()}
    getBalancePan={getAudioMixerBalancePan}
    onMonitoringChange={(sourceId, monitoring) => void setAudioMixerMonitoring(sourceId, monitoring)}
    onBalancePanInput={setAudioMixerBalancePanLocal}
    onBalancePanCommit={(sourceId) => void commitAudioMixerBalancePan(sourceId)}
    onToggleTrack={(sourceId, track) => void toggleAudioMixerTrack(sourceId, track)}
  />

  <AudioFiltersModal
    open={showAudioFiltersModal}
    {allowDraggablePopups}
    {openAdditionalSettingsInWindows}
    bind:modalEl={audioFiltersModalEl}
    dragX={audioFiltersDragX}
    dragY={audioFiltersDragY}
    {audioFiltersSourceLabel}
    {audioFiltersDraft}
    {audioFiltersSelectedId}
    {audioFiltersRenamingId}
    {audioFiltersRenameValue}
    {audioFilterNewKind}
    {selectedAudioFilter}
    {selectedAudioFilterPresetFields}
    {audioFiltersPreviewUrl}
    {audioFiltersContextMenu}
    onClose={() => void closeAudioFiltersModal()}
    onSave={() => void saveAudioFiltersModal()}
    onBeginDrag={beginAudioFiltersDrag}
    onMoveDrag={moveAudioFiltersDrag}
    onEndDrag={endAudioFiltersDrag}
    onSelectFilter={selectAudioFilter}
    onOpenContextMenu={openAudioFiltersContextMenu}
    onSetRenameValue={(value) => (audioFiltersRenameValue = value)}
    onCommitRename={commitAudioFilterRename}
    onCancelRename={() => (audioFiltersRenamingId = null)}
    onMoveFilter={moveAudioFilter}
    onSetNewKind={(value) => (audioFilterNewKind = value)}
    onAddFilter={addAudioFilter}
    onResetSelectedToDefaults={resetSelectedAudioFilterToDefaults}
    onUpdatePresetField={updateAudioFilterPresetField}
    onUpdateFilter={updateAudioFilter}
    onRemoveFilter={removeAudioFilter}
    onStartRename={startAudioFilterRename}
    onToggleLock={toggleAudioFilterLock}
    onCloseContextMenu={closeAudioFiltersContextMenu}
    {handleBackdropKey}
    {isTruthy}
  />

  {#if showTemplatesFutureDialog}
    <div class="modal-backdrop" role="button" tabindex="0" on:click={closeTemplatesDialog} on:keydown={(e) => handleBackdropKey(e, closeTemplatesDialog)}>
      <div class="quick-text-modal" role="dialog" tabindex="-1" aria-modal="true" aria-label="Templates availability" on:click|stopPropagation on:keydown|stopPropagation>
        <h3>Templates</h3>
        <p class="muted">Future will be available on newer versions.</p>
        <div class="quick-text-actions">
          <button class="primary" on:click={closeTemplatesDialog}>OK</button>
        </div>
      </div>
    </div>
  {/if}

  {#if useCustomContextMenu && appContextMenu.open}
    <div class="context-menu app-context-menu" style={`top:${appContextMenu.y}px; left:${appContextMenu.x}px;`} role="menu">
      {#if isReleaseBuild}
        <button on:click={openHomepage}>Homepage</button>
      {:else}
        <button on:click={reloadApp}>Reload</button>
      {/if}
      {#if canInspect}
        <button class="disabled">Inspect</button>
      {/if}
      <button
        class:disabled={!canUseForceClose()}
        title={canUseForceClose() ? "Force close all app windows" : "Stop streaming/recording first"}
        on:click={() => void forceCloseFromContextMenu()}
      >
        Force close
      </button>
    </div>
    <div
      class="context-overlay"
      role="button"
      tabindex="0"
      on:click={closeAppContextMenu}
      on:keydown={(e) => handleBackdropKey(e, closeAppContextMenu)}
    ></div>
  {/if}

  {#if previewMenu.open}
    <div class="context-menu" style={`top:${previewMenu.y}px; left:${previewMenu.x}px;`} role="menu">
      <button on:click={() => void refreshRenderFrame()}>Refresh render</button>
      <button on:click={() => void openGraphicPlannerFromPreviewMenu()}>Graphic planner</button>
    </div>
    <div
      class="context-overlay"
      role="button"
      tabindex="0"
      on:click={closePreviewMenu}
      on:keydown={(e) => handleBackdropKey(e, closePreviewMenu)}
    ></div>
  {/if}

  {#if sourceMenu.open && sourceMenu.source}
    <div class="context-menu" style={`top:${sourceMenu.y}px; left:${sourceMenu.x}px;`} role="menu">
      <button on:click={() => sourceMenu.source && cloneSource(sourceMenu.source)}>Clone</button>
      <button on:click={() => sourceMenu.source && removeSource(sourceMenu.source)}>Remove</button>
      <button on:click={() => sourceMenu.source && openSourceFilters(sourceMenu.source)}>Filters</button>
      {#if isBrowserSource(sourceMenu.source)}
        <button on:click={() => sourceMenu.source && openSourceInteraction(sourceMenu.source)}>Interact</button>
      {:else if isTextSource(sourceMenu.source)}
        <button on:click={() => sourceMenu.source && openTextEdit(sourceMenu.source)}>Text Edit</button>
      {/if}
      <button on:click={() => sourceMenu.source && showInfo(sourceMenu.source)}>Information</button>
      <button on:click={() => sourceMenu.source && openEditSource(sourceMenu.source)}>Edit</button>
      <button on:click={() => sourceMenu.source && toggleSourceLock(sourceMenu.source)}>
        {sourceMenu.source?.locked ? "Unlock" : "Lock"}
      </button>
    </div>
    <div
      class="context-overlay"
      role="button"
      tabindex="0"
      on:click={closeSourceMenu}
      on:keydown={(e) => handleBackdropKey(e, closeSourceMenu)}
    ></div>
  {/if}

  {#if sceneMenu.open && sceneMenu.scene}
    <div class="context-menu" style={`top:${sceneMenu.y}px; left:${sceneMenu.x}px;`} role="menu">
      <button on:click={() => sceneMenu.scene && cloneScene(sceneMenu.scene)}>Clone</button>
      <button on:click={() => sceneMenu.scene && removeScene(sceneMenu.scene)}>Remove</button>
      <button on:click={() => sceneMenu.scene && openSceneFilters(sceneMenu.scene)}>Filters</button>
      <button on:click={() => sceneMenu.scene && toggleSceneLock(sceneMenu.scene)}>
        {sceneMenu.scene?.locked ? "Unlock" : "Lock"}
      </button>
    </div>
    <div
      class="context-overlay"
      role="button"
      tabindex="0"
      on:click={closeSceneMenu}
      on:keydown={(e) => handleBackdropKey(e, closeSceneMenu)}
    ></div>
  {/if}

  <DockPanel
    {dockMenu}
    {dockPinnedSide}
    {dockPaneWidth}
    bind:dockPaneEl
    bind:dockBodyEl
    {browserDockTitle}
    {dockEngineActive}
    {dockEngineLabel}
    {isReleaseBuild}
    {dockFrameKey}
    {dockHostWebviewUrl}
    {dockCanvasRuntimeReady}
    {dockCanvasUnavailableReason}
    {dockFrameBlocked}
    {dockFrameErrorMessage}
    {showDockHandle}
    {dockDragActive}
    {dockDropTarget}
    {moveDockToSide}
    {removeDockFromWorkspace}
    {closeDockMenu}
    {handleBackdropKey}
    {startDockResize}
    {openDockMenu}
    {startDockHandleDrag}
    {handleDockHandleDragEnd}
    {startDockHeaderMove}
    {undockDockPane}
    {handleDockRefreshClick}
    {handleDockZoneDragOver}
    {handleDockZoneDrop}
  >

      <section class="render">
        {#if demoMode}
          <DemoMode />
        {:else}
          <div
            class="live-preview"
            style={`--preview-scale:${getPreviewScaleFinal()};`}
            on:dblclick={openSourceTransformModal}
            role="button"
            tabindex="0"
            aria-label="Open source transform modal"
          >
                <!-- SourceTransformModal now opens in a new window -->
            <div
              class="preview-frame"
              bind:this={previewFrameEl}
              style={`--preview-aspect:${getPreviewAspect()};`}
              on:dblclick={openSourceTransformModal}
              on:contextmenu={openPreviewMenu}
              role="button"
              tabindex="0"
              on:keydown={(e) => {
                if (e.key === 'Enter' || e.key === ' ') {
                  e.preventDefault();
                  openSourceTransformModal();
                }
              }}
              aria-label="Open graphic planner"
            >
              {#if previewTransitionVisible && previewTransitionKind}
                <div
                  class="preview-transition-overlay"
                  class:cut={previewTransitionKind === "cut"}
                  class:fade={previewTransitionKind === "fade"}
                  class:swipe={previewTransitionKind === "swipe"}
                  class:slide={previewTransitionKind === "slide"}
                  class:fade_to_color={previewTransitionKind === "fade_to_color"}
                  class:luma_wipe={previewTransitionKind === "luma_wipe"}
                  class:stinger={previewTransitionKind === "stinger"}
                  class:swipe-left={previewTransitionDirection === "left"}
                  class:swipe-right={previewTransitionDirection === "right"}
                  class:swipe-up={previewTransitionDirection === "up"}
                  class:swipe-down={previewTransitionDirection === "down"}
                  style={`--transition-ms:${previewTransitionDurationMs}ms;--transition-color:${previewTransitionColor};--transition-softness:${previewTransitionSoftness}%;`}
                >
                  {#if previewTransitionKind === "stinger" && previewTransitionStingerSrc}
                    <video
                      bind:this={stingerPreviewVideo}
                      src={previewTransitionStingerSrc}
                      autoplay
                      muted
                      playsinline
                    ></video>
                  {/if}
                </div>
              {/if}

              {#if webrtcActive}
                <video bind:this={previewVideo} autoplay playsinline muted></video>
              {:else if previewUrl}
                <img src={previewUrl} alt="Scene preview" decoding="async" />
              {:else}
                <div class="preview-placeholder">No preview captured</div>
              {/if}
            </div>
          </div>
        {/if}
      </section>

      <section class="panel">
        <div class="panel-cell panel-scenes" bind:this={panelScenesEl} tabindex="-1" aria-label="Scenes panel">
          <ScenesPanel
            {scenes}
            {backendEnabled}
            {renamingScene}
            {renameSceneValue}
            on:openAddScene={openAddSceneModal}
            on:setScene={(e) => setCurrentScene(e.detail.name)}
            on:startRename={(e) => startRenameScene(e.detail.scene)}
            on:commitRename={commitRenameScene}
            on:cancelRename={cancelRenameScene}
            on:openMenu={(e) => openSceneMenu(e.detail.event, e.detail.scene)}
            on:updateRenameValue={(e) => (renameSceneValue = e.detail.value)}
            on:reorder={(e) => moveSceneToIndex(e.detail.sceneName, e.detail.toIndex)}
          />
        </div>

        <div class="panel-cell panel-sources" bind:this={panelSourcesEl} tabindex="-1" aria-label="Sources panel">
          <SourcesPanel
            sources={sourcesList}
            emptyMessage={demoMode ? "Source list will appear here." : "No sources available"}
            on:openAddSource={openAddSourceModal}
            on:interact={(e) => openSourceInteraction(e.detail.source)}
            on:textEdit={(e) => openTextEdit(e.detail.source)}
            on:quickChangeColor={(e) => openQuickColorModal(e.detail.source)}
            on:quickSelectFile={(e) => quickSelectImageFile(e.detail.source)}
            on:quickSelectDevice={(e) => openQuickDeviceModal(e.detail.source)}
            on:toggleVisibility={(e) => toggleSourceVisibility(e.detail.source)}
            on:toggleLock={(e) => toggleSourceLock(e.detail.source)}
            on:move={(e) => moveSource(e.detail.source, e.detail.direction)}
            on:openEdit={(e) => openEditSource(e.detail.source)}
            on:openMenu={(e) => openSourceMenu(e.detail.event, e.detail.source)}
            on:reorder={(e) => moveSourceToIndex(e.detail.sourceId, e.detail.toIndex)}
          />
        </div>

        <div class="tools panel-cell panel-tools" bind:this={panelToolsEl} tabindex="-1" aria-label="Tools panel">
          <h2>Tools</h2>
          <div class="tool-list">
            <button on:click={openAudioMixer}>
              <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 7h3v10H4V7Zm6-3h3v16h-3V4Zm6 6h3v7h-3v-7Z"/></svg>
              Audio Mixer
            </button>
            <button on:click={openTransitions}>
              <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 6h7v7H5V6Zm7 7h7v7h-7v-7Zm0-7 7 7-7 7V6Z"/></svg>
              Transitions
            </button>
            <button on:click={openPlugins}>
              <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M14 4a2 2 0 0 1 2 2v2h2a2 2 0 1 1 0 4h-2v2a2 2 0 0 1-2 2h-2v-2a2 2 0 1 0-4 0v2H6a2 2 0 0 1-2-2v-2H2a2 2 0 1 1 0-4h2V6a2 2 0 0 1 2-2h2v2a2 2 0 1 0 4 0V4h2Z"/></svg>
              Plugins
            </button>
            <button on:click={openTemplatesDialog}>
              <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 5h7v7H4V5Zm9 0h7v7h-7V5ZM4 14h7v5H4v-5Zm9 0h7v5h-7v-5Z"/></svg>
              Templates
            </button>
          </div>
        </div>
      </section>
  </DockPanel>
</main>

<style>
  @import url("https://fonts.googleapis.com/css2?family=Space+Grotesk:wght@400;600;700&display=swap");

  :global(:root) {
    color-scheme: dark;
    --bg: #0f1115;
    --surface: #151820;
    --surface-2: #1b1f2a;
    --surface-3: #232838;
    --border: #2b3142;
    --border-strong: #3a4258;
    --text: #e5e7eb;
    --text-muted: #a1a1aa;
    --accent: #5b7cfa;
    --accent-strong: #3b5bdb;
    --success: #22c55e;
    --danger: #ef4444;
    --warning: #f59e0b;
    --icon-color: var(--text);
    --preview-bg: var(--surface);
    --scene-active-text: var(--text);
    --a11y-font-scale: 1;
    --a11y-font-family: "Space Grotesk", "Inter", system-ui, sans-serif;
    --a11y-color-filter: none;
  }

  :global(html.revo-no-rounded *),
  :global(html.revo-no-rounded *::before),
  :global(html.revo-no-rounded *::after) {
    border-radius: 0 !important;
  }

  :global(html) {
    filter: var(--a11y-color-filter);
    font-size: calc(16px * var(--a11y-font-scale));
  }

  :global(body) {
    margin: 0;
    font-family: var(--a11y-font-family);
    font-size: 1rem;
    background: var(--bg);
    color: var(--text);
  }

  :global(body button),
  :global(body input),
  :global(body select),
  :global(body textarea),
  :global(body option),
  :global(body optgroup) {
    font-family: var(--a11y-font-family);
    font-size: 1em;
  }

  :global(html.a11y-high-contrast) {
    --bg: #000000;
    --surface: #000000;
    --surface-2: #050505;
    --surface-3: #0d0d0d;
    --border: #6cc7ff;
    --border-strong: #a7ddff;
    --text: #ffffff;
    --text-muted: #c6e6ff;
    --accent: #72d2ff;
    --accent-strong: #2aa8ff;
    --warning: #ffde3b;
    --success: #4cff8f;
    --danger: #ff7070;
  }

  :global(html.a11y-color-protanopia) {
    --accent: #6ec5ff;
    --accent-strong: #3b8dd0;
    --warning: #ffd166;
    --success: #5fd3bc;
    --danger: #ff8a80;
  }

  :global(html.a11y-color-deuteranopia) {
    --accent: #7cb6ff;
    --accent-strong: #4b7fd1;
    --warning: #ffd166;
    --success: #4fc3f7;
    --danger: #ff8a65;
  }

  :global(html.a11y-color-tritanopia) {
    --accent: #9c7dff;
    --accent-strong: #7857df;
    --warning: #ffe082;
    --success: #7ad67a;
    --danger: #ff8ca8;
  }

  :global(html.a11y-reduce-motion *),
  :global(html.a11y-reduce-motion *::before),
  :global(html.a11y-reduce-motion *::after) {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
    scroll-behavior: auto !important;
  }

  :global(html.a11y-strong-focus *:focus-visible) {
    outline: 3px solid var(--accent) !important;
    outline-offset: 2px !important;
  }

  main {
    height: 100vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .dock-canvas-offscreen {
    position: fixed;
    width: 1px;
    height: 1px;
    opacity: 0;
    pointer-events: none;
    left: -9999px;
    top: -9999px;
  }

  .header-shell {
    transition: opacity 0.2s ease, filter 0.2s ease;
  }

  .header-shell.settings-open {
    opacity: 0.45;
    filter: saturate(0.7);
  }

  :global(select) {
    -webkit-appearance: none;
    appearance: none;
    color-scheme: normal;
    background-color: var(--surface) !important;
    color: var(--text);
  }

  :global(input:not([type="checkbox"]):not([type="radio"]):not([type="range"]):not([type="file"]):not([type="color"])) {
    background-color: var(--surface) !important;
    color: var(--text);
    outline: none;
  }

  :global(option) {
    background-color: var(--surface) !important;
    color: var(--text);
  }

  .render {
    flex: 1 1 auto;
    min-height: 0;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 1rem;
    padding: 0 1.5rem 1rem;
    overflow: hidden;
  }

  .title {
    text-align: center;
    margin-top: 2rem;
    letter-spacing: 0.02em;
  }

  .subtitle {
    text-align: center;
    color: var(--text-muted);
    margin: 0;
  }

  .message-bar {
    margin: 0 auto;
    padding: 0.65rem 1rem;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 10px;
    color: var(--text);
    max-width: 720px;
  }

  .live-preview {
    width: min(960px, 100%);
    max-width: 100%;
    max-height: 100%;
    margin: 0 auto;
    flex: 1 1 auto;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    align-items: center;
    justify-content: center;
  }

  .preview-frame {
    width: auto;
    height: 100%;
    max-width: 100%;
    max-height: 100%;
    aspect-ratio: var(--preview-aspect, 16 / 9);
    background: var(--preview-bg, var(--surface));
    border: 1px solid var(--border);
    border-radius: 18px;
    display: grid;
    place-items: center;
    overflow: hidden;
    position: relative;
  }

  .preview-frame img,
  .preview-frame video {
    position: relative;
    z-index: 1;
    width: 100%;
    height: 100%;
    object-fit: contain;
    transform: scale(var(--preview-scale, 1));
    transform-origin: center;
  }

  .preview-transition-overlay {
    position: absolute;
    inset: 0;
    z-index: 6;
    pointer-events: none;
    background: #000;
    animation-duration: var(--transition-ms, 320ms);
    animation-fill-mode: both;
  }

  .preview-transition-overlay.cut {
    animation-name: preview-cut;
  }

  .preview-transition-overlay.fade {
    animation-name: preview-fade;
  }

  .preview-transition-overlay.fade_to_color {
    background: var(--transition-color, #000);
    animation-name: preview-fade;
  }

  .preview-transition-overlay.swipe {
    animation-name: preview-swipe-left;
  }

  .preview-transition-overlay.swipe.swipe-right {
    animation-name: preview-swipe-right;
  }

  .preview-transition-overlay.swipe.swipe-up {
    animation-name: preview-swipe-up;
  }

  .preview-transition-overlay.swipe.swipe-down {
    animation-name: preview-swipe-down;
  }

  .preview-transition-overlay.slide {
    animation-name: preview-slide-left;
  }

  .preview-transition-overlay.slide.swipe-right {
    animation-name: preview-slide-right;
  }

  .preview-transition-overlay.slide.swipe-up {
    animation-name: preview-slide-up;
  }

  .preview-transition-overlay.slide.swipe-down {
    animation-name: preview-slide-down;
  }

  .preview-transition-overlay.luma_wipe {
    animation-name: preview-luma-wipe-left;
  }

  .preview-transition-overlay.luma_wipe.swipe-right {
    animation-name: preview-luma-wipe-right;
  }

  .preview-transition-overlay.luma_wipe.swipe-up {
    animation-name: preview-luma-wipe-up;
  }

  .preview-transition-overlay.luma_wipe.swipe-down {
    animation-name: preview-luma-wipe-down;
  }

  .preview-transition-overlay.stinger {
    background: transparent;
    animation: none;
    opacity: 1;
  }

  .preview-transition-overlay.stinger video {
    width: 100%;
    height: 100%;
    object-fit: cover;
    transform: none;
    z-index: 7;
  }

  @keyframes preview-cut {
    0% {
      opacity: 0;
    }
    40% {
      opacity: 0.95;
    }
    100% {
      opacity: 0;
    }
  }

  @keyframes preview-fade {
    0% {
      opacity: 0;
    }
    45% {
      opacity: 1;
    }
    100% {
      opacity: 0;
    }
  }

  @keyframes preview-swipe-left {
    0% {
      transform: translateX(0%);
      opacity: 1;
    }
    100% {
      transform: translateX(-100%);
      opacity: 1;
    }
  }

  @keyframes preview-swipe-right {
    0% {
      transform: translateX(0%);
      opacity: 1;
    }
    100% {
      transform: translateX(100%);
      opacity: 1;
    }
  }

  @keyframes preview-swipe-up {
    0% {
      transform: translateY(0%);
      opacity: 1;
    }
    100% {
      transform: translateY(-100%);
      opacity: 1;
    }
  }

  @keyframes preview-swipe-down {
    0% {
      transform: translateY(0%);
      opacity: 1;
    }
    100% {
      transform: translateY(100%);
      opacity: 1;
    }
  }

  @keyframes preview-slide-left {
    0% {
      transform: translateX(100%);
      opacity: 1;
    }
    100% {
      transform: translateX(-100%);
      opacity: 1;
    }
  }

  @keyframes preview-slide-right {
    0% {
      transform: translateX(-100%);
      opacity: 1;
    }
    100% {
      transform: translateX(100%);
      opacity: 1;
    }
  }

  @keyframes preview-slide-up {
    0% {
      transform: translateY(100%);
      opacity: 1;
    }
    100% {
      transform: translateY(-100%);
      opacity: 1;
    }
  }

  @keyframes preview-slide-down {
    0% {
      transform: translateY(-100%);
      opacity: 1;
    }
    100% {
      transform: translateY(100%);
      opacity: 1;
    }
  }

  @keyframes preview-luma-wipe-left {
    0% {
      clip-path: inset(0 100% 0 0);
    }
    100% {
      clip-path: inset(0 0 0 100%);
    }
  }

  @keyframes preview-luma-wipe-right {
    0% {
      clip-path: inset(0 0 0 100%);
    }
    100% {
      clip-path: inset(0 100% 0 0);
    }
  }

  @keyframes preview-luma-wipe-up {
    0% {
      clip-path: inset(100% 0 0 0);
    }
    100% {
      clip-path: inset(0 0 100% 0);
    }
  }

  @keyframes preview-luma-wipe-down {
    0% {
      clip-path: inset(0 0 100% 0);
    }
    100% {
      clip-path: inset(100% 0 0 0);
    }
  }

  .preview-note {
    margin-top: 0.5rem;
    color: var(--text-muted);
    font-size: 0.9rem;
    text-align: center;
  }

  .preview-placeholder {
    color: var(--text-muted);
  }

  .panel {
    flex: 0 0 240px;
    overflow: hidden;
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 1rem;
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 1.25rem;
    margin: 1.5rem;
    container-type: inline-size;
    container-name: panel;
    align-items: stretch;
  }

  .panel-cell {
    min-width: 0;
    min-height: 0;
  }

  .tools {
    background: transparent;
    border: none;
    padding: 0;
  }

  .tools .tool-list {
    display: grid;
    gap: 0.55rem;
    margin-top: 0.75rem;
    padding-bottom: 0.35rem;
  }

  .tools .tool-list button {
    text-align: left;
    background: var(--surface);
    border: 1px solid var(--border);
    color: var(--text);
    padding: 0.58rem 0.72rem;
    border-radius: 10px;
    font-size: 0.92rem;
    line-height: 1.25;
    font-weight: 500;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }

  .tools .tool-list button svg {
    width: 18px;
    height: 18px;
    fill: var(--icon-color, currentColor);
    flex-shrink: 0;
  }

  :global(.context-menu) {
    position: fixed;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.25rem;
    z-index: 60;
    min-width: 160px;
    box-shadow: 0 8px 28px #0009;
  }

  :global(.context-menu button) {
    width: 100%;
    text-align: left;
    background: transparent;
    color: var(--text);
    border: none;
    padding: 0.6rem 0.75rem;
    border-radius: 8px;
    cursor: pointer;
  }

  :global(.context-menu button:hover) {
    background: var(--surface-3);
  }

  :global(.context-menu button.disabled) {
    color: var(--text-muted);
    cursor: not-allowed;
  }

  :global(.context-menu button.disabled:hover) {
    background: transparent;
  }

  :global(.context-overlay) {
    position: fixed;
    inset: 0;
    z-index: 55;
  }

  .aux-only-host {
    display: none;
  }

  main.aux-window {
    min-height: 100vh;
    display: block;
  }

  main.aux-window > :not(.aux-only-host) {
    display: none !important;
  }

  main.aux-window .aux-only-host {
    display: block;
  }

  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: #000a;
    z-index: 2000;
  }
  .modal-transform {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    background: var(--surface-2);
    color: var(--text);
    border-radius: 16px;
    box-shadow: 0 4px 32px #000a;
    z-index: 2010;
    padding: 2.5rem 2.5rem 2rem;
    min-width: 420px;
    min-height: 220px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1.2rem;
  }

  .quick-text-modal {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    background: var(--surface-2);
    color: var(--text);
    border-radius: 12px;
    box-shadow: 0 4px 32px #000a;
    z-index: 2010;
    border: 1px solid var(--border);
    padding: 1rem;
    width: min(640px, calc(100vw - 2rem));
    min-width: 460px;
    display: grid;
    gap: 0.75rem;
    box-sizing: border-box;
    overflow: hidden;
  }

  .quick-text-modal.draggable-popup {
    transform: translate(calc(-50% + var(--quick-text-dx, 0px)), calc(-50% + var(--quick-text-dy, 0px)));
  }

  .quick-text-modal h3 {
    margin: 0;
    font-size: 1rem;
    font-weight: 700;
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

  @media (max-width: 640px) {
    .quick-text-modal {
      min-width: unset;
    }
  }


  @media (max-width: 1200px) {
    .panel {
      max-height: 250px;
      flex: 0 0 420px;
      grid-template-columns: repeat(2, minmax(0, 1fr));
      grid-template-areas:
        "scenes tools"
        "sources sources";
      grid-template-rows: auto minmax(0, 1fr);
      overflow: auto;
    }

    .panel-scenes {
      grid-area: scenes;
    }

    .panel-tools {
      grid-area: tools;
    }

    .panel-sources {
      grid-area: sources;
      min-height: 180px;
    }
  }

  @media (max-width: 800px) {
    .panel {
      flex: 0 0 min(70vh, 560px);
      grid-template-columns: 1fr;
      grid-template-areas:
        "scenes"
        "tools"
        "sources";
      overflow: auto;
    }

    .panel-scenes {
      grid-area: scenes;
    }

    .panel-tools {
      grid-area: tools;
    }

    .panel-sources {
      grid-area: sources;
      min-height: 180px;
    }
  }
</style>
