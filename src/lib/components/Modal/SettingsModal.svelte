

<script lang="ts">
  import { createEventDispatcher, onDestroy, onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  export let rootDir: string;
  export let demoMode: boolean;
  export let recordPath: string;
  export let streamUrl: string;
  export let streamKey: string = "";
  export let previewQuality: string;
  export let previewQualityOptions: { value: string; label: string }[];
  export let encoderPreference: string;
  export let encoderPreferenceOptions: { value: string; label: string }[];
  export let sceneResolution: string;
  export let sceneResolutionOptions: { value: string; label: string }[];
  export let whepUrl: string;
  export let whipUrl: string;
  export let autoRetryPreview: boolean;
  export let profileOptions: string[] = ["default"];
  export let selectedProfileName = "default";
  export let busy = false;
  export let autorescaleInputs: boolean = false;
  export let allowDraggablePopups = false;
  export let isObsRunning: boolean;
  export let version: string;
  export let initialUiProfile: Record<string, unknown> | null = null;
  export let initialAccessibilityHighContrast = false;
  export let initialAccessibilityReduceMotion = false;
  export let initialAccessibilityFocusIndicators = true;
  export let initialAccessibilityUiScale = "100";
  export let initialAccessibilityFontSize = "100";
  export let initialAccessibilityFontFamily = "system";
  export let initialAccessibilityColorVision = "none";

  type SelectOption = { value: string; label: string };
  type LanguageOption = { value: string; label: string };
  type AudioDeviceOption = { id: string; name: string };
  type ThemeInfo = { id: string; name: string; author: string; version: string };
  type LibObsSettingsSelectOptions = {
    streaming_audio_encoders?: SelectOption[];
    streaming_video_encoders?: SelectOption[];
    recording_audio_encoders?: SelectOption[];
    recording_video_encoders?: SelectOption[];
    recording_formats?: SelectOption[];
    audio_sample_rates?: SelectOption[];
    audio_channels?: SelectOption[];
    video_resolutions?: SelectOption[];
    video_downscale_filters?: SelectOption[];
    video_common_fps?: SelectOption[];
  };

  const defaultStreamingAudioEncoderOptions: SelectOption[] = [
    { value: "aac", label: "AAC" },
    { value: "opus", label: "Opus" }
  ];

  const defaultStreamingVideoEncoderOptions: SelectOption[] = [
    { value: "x264", label: "x264" },
    { value: "h264_nvenc", label: "NVIDIA NVENC" },
    { value: "h264_vaapi", label: "VAAPI" }
  ];

  const defaultAudioSampleRateOptions: SelectOption[] = [
    { value: "44100", label: "44.1 kHz" },
    { value: "48000", label: "48 kHz" }
  ];

  const defaultRecordingFormatOptions: SelectOption[] = [
    { value: "mkv", label: "mkv" },
    { value: "mp4", label: "mp4" },
    { value: "mov", label: "mov" },
    { value: "flv", label: "flv" },
    { value: "ts", label: "ts" },
    { value: "m3u8", label: "m3u8" },
    { value: "webm", label: "webm" },
    { value: "avi", label: "avi" }
  ];

  const defaultAudioChannelsOptions: SelectOption[] = [
    { value: "mono", label: "Mono" },
    { value: "stereo", label: "Stereo" },
    { value: "2.1", label: "2.1" },
    { value: "4.1", label: "4.1" },
    { value: "5.1", label: "5.1" },
    { value: "7.1", label: "7.1" }
  ];

  const defaultVideoResolutionOptions: SelectOption[] = [
    { value: "1280x720", label: "1280x720" },
    { value: "1600x900", label: "1600x900" },
    { value: "1920x1080", label: "1920x1080" },
    { value: "2560x1440", label: "2560x1440" },
    { value: "3840x2160", label: "3840x2160" }
  ];

  const defaultVideoDownscaleFilterOptions: SelectOption[] = [
    { value: "bilinear", label: "Bilinear (Fastest, soft)" },
    { value: "bicubic", label: "Bicubic (Sharpened scaling, 16 samples)" },
    { value: "lanczos", label: "Lanczos (Sharpened scaling, 32 samples)" }
  ];

  const defaultVideoCommonFpsOptions: SelectOption[] = [
    { value: "24", label: "24" },
    { value: "25", label: "25" },
    { value: "30", label: "30" },
    { value: "48", label: "48" },
    { value: "50", label: "50" },
    { value: "60", label: "60" }
  ];

  const defaultDesktopAudioDeviceOptions: SelectOption[] = [
    { value: "disabled", label: "Disabled" },
    { value: "default", label: "Default" }
  ];

  const defaultMonitoringDeviceOptions: SelectOption[] = [
    { value: "disabled", label: "Disabled" },
    { value: "default", label: "Default" }
  ];

  const languageOptions: LanguageOption[] = [
    { value: "en", label: "English - 100% translated" },
    { value: "pl", label: "Polish - 0% translated" }
  ];

  let streamingAudioEncoderOptions: SelectOption[] = [...defaultStreamingAudioEncoderOptions];
  let streamingVideoEncoderOptions: SelectOption[] = [...defaultStreamingVideoEncoderOptions];
  let recordingAudioEncoderOptions: SelectOption[] = [...defaultStreamingAudioEncoderOptions];
  let recordingVideoEncoderOptions: SelectOption[] = [...defaultStreamingVideoEncoderOptions];
  let recordingFormatOptions: SelectOption[] = [...defaultRecordingFormatOptions];
  let audioSampleRateOptions: SelectOption[] = [...defaultAudioSampleRateOptions];
  let audioChannelsOptions: SelectOption[] = [...defaultAudioChannelsOptions];
  let videoResolutionOptions: SelectOption[] = [...defaultVideoResolutionOptions];
  let videoDownscaleFilterOptions: SelectOption[] = [...defaultVideoDownscaleFilterOptions];
  let videoCommonFpsOptions: SelectOption[] = [...defaultVideoCommonFpsOptions];
  let audioDesktopDeviceOptions: SelectOption[] = [...defaultDesktopAudioDeviceOptions];
  let audioMicDeviceOptions: SelectOption[] = [...defaultDesktopAudioDeviceOptions];
  let audioMonitoringDeviceOptions: SelectOption[] = [...defaultMonitoringDeviceOptions];

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
    if (!allowDraggablePopups || openAdditionalSettingsInWindows) return;
    if (event.button !== 0) return;
    const target = event.target as HTMLElement | null;
    if (target?.closest("button,input,select,textarea,a,label")) return;
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

  const asRecord = (value: unknown): Record<string, unknown> =>
    value && typeof value === "object" && !Array.isArray(value) ? (value as Record<string, unknown>) : {};

  const readString = (obj: Record<string, unknown>, key: string, fallback: string) => {
    const v = obj[key];
    return typeof v === "string" ? v : fallback;
  };

  const readBool = (obj: Record<string, unknown>, key: string, fallback: boolean) => {
    const v = obj[key];
    return typeof v === "boolean" ? v : fallback;
  };

  const readInt = (obj: Record<string, unknown>, key: string, fallback: number, min: number, max: number) => {
    const v = obj[key];
    const parsed = typeof v === "number" ? v : Number(v);
    if (!Number.isFinite(parsed)) return fallback;
    return Math.max(min, Math.min(max, Math.round(parsed)));
  };

  type AccessibilitySnapshot = {
    zoom: string;
    fontScaleVar: string;
    fontFamilyVar: string;
    colorFilterVar: string;
    reduceMotionClass: boolean;
    strongFocusClass: boolean;
    highContrastClass: boolean;
    colorProtanopiaClass: boolean;
    colorDeuteranopiaClass: boolean;
    colorTritanopiaClass: boolean;
  };

  let accessibilityPreviewReady = false;
  let accessibilitySaved = false;
  let accessibilitySnapshot: AccessibilitySnapshot | null = null;

  const captureAccessibilitySnapshot = () => {
    if (typeof document === "undefined") return;
    const root = document.documentElement;
    accessibilitySnapshot = {
      zoom: root.style.zoom,
      fontScaleVar: root.style.getPropertyValue("--a11y-font-scale"),
      fontFamilyVar: root.style.getPropertyValue("--a11y-font-family"),
      colorFilterVar: root.style.getPropertyValue("--a11y-color-filter"),
      reduceMotionClass: root.classList.contains("a11y-reduce-motion"),
      strongFocusClass: root.classList.contains("a11y-strong-focus"),
      highContrastClass: root.classList.contains("a11y-high-contrast"),
      colorProtanopiaClass: root.classList.contains("a11y-color-protanopia"),
      colorDeuteranopiaClass: root.classList.contains("a11y-color-deuteranopia"),
      colorTritanopiaClass: root.classList.contains("a11y-color-tritanopia")
    };
  };

  const clearColorVisionClasses = (root: HTMLElement) => {
    root.classList.remove("a11y-color-protanopia", "a11y-color-deuteranopia", "a11y-color-tritanopia");
  };

  const resolveFontFamily = () => {
    if (accessibilityFontFamily === "sans") {
      return "Arial, Helvetica, sans-serif";
    }
    if (accessibilityFontFamily === "serif") {
      return "Georgia, Times New Roman, serif";
    }
    if (accessibilityFontFamily === "mono") {
      return "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, Liberation Mono, monospace";
    }
    return '"Space Grotesk", "Inter", system-ui, sans-serif';
  };

  const resolveColorVisionFilter = () => {
    if (accessibilityColorVision === "protanopia") {
      return "sepia(0.24) hue-rotate(-18deg) saturate(0.78)";
    }
    if (accessibilityColorVision === "deuteranopia") {
      return "sepia(0.2) hue-rotate(8deg) saturate(0.75)";
    }
    if (accessibilityColorVision === "tritanopia") {
      return "hue-rotate(28deg) saturate(0.78)";
    }
    return "none";
  };

  const applyAccessibilityPreview = () => {
    if (typeof document === "undefined" || !accessibilityPreviewReady) return;

    const root = document.documentElement;
    root.style.zoom = `${Number(accessibilityUiScale) / 100}`;

    const fontScale = Math.max(0.8, Number(accessibilityFontSize) / 100);
    root.style.setProperty("--a11y-font-scale", String(fontScale));
    root.style.setProperty("--a11y-font-family", resolveFontFamily());
    root.style.setProperty("--a11y-color-filter", resolveColorVisionFilter());

    root.classList.toggle("a11y-reduce-motion", accessibilityReduceMotion);
    root.classList.toggle("a11y-strong-focus", accessibilityFocusIndicators);
    root.classList.toggle("a11y-high-contrast", accessibilityHighContrast);

    clearColorVisionClasses(root);
    if (accessibilityColorVision === "protanopia") {
      root.classList.add("a11y-color-protanopia");
    } else if (accessibilityColorVision === "deuteranopia") {
      root.classList.add("a11y-color-deuteranopia");
    } else if (accessibilityColorVision === "tritanopia") {
      root.classList.add("a11y-color-tritanopia");
    }
  };

  const restoreAccessibilitySnapshot = () => {
    if (typeof document === "undefined" || !accessibilitySnapshot) return;

    const root = document.documentElement;
    root.style.zoom = accessibilitySnapshot.zoom;
    root.style.setProperty("--a11y-font-scale", accessibilitySnapshot.fontScaleVar);
    root.style.setProperty("--a11y-font-family", accessibilitySnapshot.fontFamilyVar);
    root.style.setProperty("--a11y-color-filter", accessibilitySnapshot.colorFilterVar);
    root.classList.toggle("a11y-reduce-motion", accessibilitySnapshot.reduceMotionClass);
    root.classList.toggle("a11y-strong-focus", accessibilitySnapshot.strongFocusClass);
    root.classList.toggle("a11y-high-contrast", accessibilitySnapshot.highContrastClass);
    root.classList.toggle("a11y-color-protanopia", accessibilitySnapshot.colorProtanopiaClass);
    root.classList.toggle("a11y-color-deuteranopia", accessibilitySnapshot.colorDeuteranopiaClass);
    root.classList.toggle("a11y-color-tritanopia", accessibilitySnapshot.colorTritanopiaClass);
  };

  onMount(() => {
    captureAccessibilitySnapshot();
    applyIncomingUiProfile(initialUiProfile);
    void loadLibobsSelectOptions();
    accessibilityPreviewReady = true;
    applyAccessibilityPreview();
  });

  const mapLibobsOptions = (options?: SelectOption[], fallback: SelectOption[] = []) =>
    Array.isArray(options) && options.length
      ? options
          .map((entry) => ({
            value: String(entry?.value ?? "").trim(),
            label: String(entry?.label ?? entry?.value ?? "").trim()
          }))
          .filter((entry) => Boolean(entry.value))
      : [...fallback];

  const mapAudioDevicesToOptions = (devices: AudioDeviceOption[], includeDisabled = true): SelectOption[] => {
    const base: SelectOption[] = includeDisabled
      ? [
          { value: "disabled", label: "Disabled" },
          { value: "default", label: "Default" }
        ]
      : [{ value: "default", label: "Default" }];

    const mapped = devices
      .map((device) => ({
        value: String(device?.id ?? "").trim(),
        label: String(device?.name ?? device?.id ?? "").trim()
      }))
      .filter((entry) => Boolean(entry.value));

    const merged = [...base, ...mapped];
    const seen = new Set<string>();
    return merged.filter((entry) => {
      if (seen.has(entry.value)) return false;
      seen.add(entry.value);
      return true;
    });
  };

  const ensureValueExists = (options: SelectOption[], value: string): SelectOption[] => {
    const normalized = value?.trim();
    if (!normalized) return options;
    if (options.some((option) => option.value === normalized)) return options;
    return [...options, { value: normalized, label: normalized }];
  };

  const loadLibobsSelectOptions = async () => {
    try {
      const payload = await invoke<LibObsSettingsSelectOptions>("obs_get_settings_select_options");
      streamingAudioEncoderOptions = mapLibobsOptions(payload?.streaming_audio_encoders, defaultStreamingAudioEncoderOptions);
      streamingVideoEncoderOptions = mapLibobsOptions(payload?.streaming_video_encoders, defaultStreamingVideoEncoderOptions);
      recordingAudioEncoderOptions = mapLibobsOptions(payload?.recording_audio_encoders, defaultStreamingAudioEncoderOptions);
      recordingVideoEncoderOptions = mapLibobsOptions(payload?.recording_video_encoders, defaultStreamingVideoEncoderOptions);
      recordingFormatOptions = mapLibobsOptions(payload?.recording_formats, defaultRecordingFormatOptions);
      audioSampleRateOptions = mapLibobsOptions(payload?.audio_sample_rates, defaultAudioSampleRateOptions);
      audioChannelsOptions = mapLibobsOptions(payload?.audio_channels, defaultAudioChannelsOptions);
      videoResolutionOptions = mapLibobsOptions(payload?.video_resolutions, defaultVideoResolutionOptions);
      videoDownscaleFilterOptions = mapLibobsOptions(payload?.video_downscale_filters, defaultVideoDownscaleFilterOptions);
      videoCommonFpsOptions = mapLibobsOptions(payload?.video_common_fps, defaultVideoCommonFpsOptions);

      const [inputDevices, outputDevices] = await Promise.all([
        invoke<AudioDeviceOption[]>("obs_list_pulse_devices", { kind: "input" }),
        invoke<AudioDeviceOption[]>("obs_list_pulse_devices", { kind: "output" })
      ]);

      audioDesktopDeviceOptions = mapAudioDevicesToOptions(outputDevices ?? [], true);
      audioMicDeviceOptions = mapAudioDevicesToOptions(inputDevices ?? [], true);
      audioMonitoringDeviceOptions = mapAudioDevicesToOptions(outputDevices ?? [], true);
    } catch {
      streamingAudioEncoderOptions = [...defaultStreamingAudioEncoderOptions];
      streamingVideoEncoderOptions = [...defaultStreamingVideoEncoderOptions];
      recordingAudioEncoderOptions = [...defaultStreamingAudioEncoderOptions];
      recordingVideoEncoderOptions = [...defaultStreamingVideoEncoderOptions];
      recordingFormatOptions = [...defaultRecordingFormatOptions];
      audioSampleRateOptions = [...defaultAudioSampleRateOptions];
      audioChannelsOptions = [...defaultAudioChannelsOptions];
      videoResolutionOptions = [...defaultVideoResolutionOptions];
      videoDownscaleFilterOptions = [...defaultVideoDownscaleFilterOptions];
      videoCommonFpsOptions = [...defaultVideoCommonFpsOptions];
      audioDesktopDeviceOptions = [...defaultDesktopAudioDeviceOptions];
      audioMicDeviceOptions = [...defaultDesktopAudioDeviceOptions];
      audioMonitoringDeviceOptions = [...defaultMonitoringDeviceOptions];
    }

    streamingAudioEncoderOptions = ensureValueExists(streamingAudioEncoderOptions, streamingAudioEncoder);
    streamingVideoEncoderOptions = ensureValueExists(streamingVideoEncoderOptions, streamingVideoEncoder);
    recordingAudioEncoderOptions = ensureValueExists(recordingAudioEncoderOptions, recordingAudioEncoder);
    recordingVideoEncoderOptions = ensureValueExists(recordingVideoEncoderOptions, recordingVideoEncoder);
    recordingFormatOptions = ensureValueExists(recordingFormatOptions, recordingFormat);
    audioSampleRateOptions = ensureValueExists(audioSampleRateOptions, audioSampleRate);
    audioChannelsOptions = ensureValueExists(audioChannelsOptions, audioChannels);
    videoResolutionOptions = ensureValueExists(videoResolutionOptions, videoCanvasResolution);
    videoResolutionOptions = ensureValueExists(videoResolutionOptions, videoOutputResolution);
    videoDownscaleFilterOptions = ensureValueExists(videoDownscaleFilterOptions, videoDownscaleFilter);
    videoCommonFpsOptions = ensureValueExists(videoCommonFpsOptions, videoFpsCommon);
    audioDesktopDeviceOptions = ensureValueExists(audioDesktopDeviceOptions, audioDesktopDevice);
    audioMicDeviceOptions = ensureValueExists(audioMicDeviceOptions, audioMicDevice);
    audioMonitoringDeviceOptions = ensureValueExists(audioMonitoringDeviceOptions, audioMonitoringDevice);
  };

  let lastIncomingProfileSignature = "";
  $: {
    const signature = JSON.stringify(initialUiProfile ?? {});
    if (signature !== lastIncomingProfileSignature) {
      lastIncomingProfileSignature = signature;
      applyIncomingUiProfile(initialUiProfile);
      markCloseGuardBaseline();
    }
  }

  onDestroy(() => {
    stopShortcutCaptureListener();
    if (!accessibilitySaved) {
      restoreAccessibilitySnapshot();
    }
  });

  $: applyAccessibilityPreview();

  let closeGuardBaseline = "";
  let showUnsavedCloseDialog = false;
  let lastReportedDirtyState: boolean | null = null;
  let unsavedNoBtnEl: HTMLButtonElement | null = null;
  let unsavedYesBtnEl: HTMLButtonElement | null = null;
  let unsavedFocusYes = false;
  let wasUnsavedDialogOpen = false;

  const buildCloseGuardPayload = () => ({
    rootDir,
    demoMode,
    recordPath,
    streamUrl,
    streamKey,
    previewQuality,
    encoderPreference,
    sceneResolution,
    whepUrl,
    whipUrl,
    autoRetryPreview,
    selectedProfileName,
    autorescaleInputs,
    profile: buildProfilePayload()
  });

  const markCloseGuardBaseline = () => {
    closeGuardBaseline = JSON.stringify(buildCloseGuardPayload());
  };

  const hasUnsavedChanges = () => JSON.stringify(buildCloseGuardPayload()) !== closeGuardBaseline;

  $: {
    const dirty = hasUnsavedChanges();
    if (dirty !== lastReportedDirtyState) {
      lastReportedDirtyState = dirty;
      dispatch("dirtyStateChange", { dirty });
    }
  }

  const finishClose = () => {
    showUnsavedCloseDialog = false;
    if (!accessibilitySaved) {
      restoreAccessibilitySnapshot();
    }
    dispatch("close");
  };

  const close = () => {
    if (hasUnsavedChanges()) {
      showUnsavedCloseDialog = true;
      return;
    }
    finishClose();
  };

  const focusUnsavedAction = async () => {
    await tick();
    const nextEl = (unsavedFocusYes ? unsavedYesBtnEl : unsavedNoBtnEl) as HTMLButtonElement | null;
    nextEl?.focus();
  };

  const moveUnsavedFocus = (toYes: boolean) => {
    unsavedFocusYes = toYes;
    void focusUnsavedAction();
  };

  const handleUnsavedDialogKeydown = (event: KeyboardEvent) => {
    if (event.key === "ArrowLeft" || event.key === "ArrowUp") {
      event.preventDefault();
      moveUnsavedFocus(false);
      return;
    }
    if (event.key === "ArrowRight" || event.key === "ArrowDown") {
      event.preventDefault();
      moveUnsavedFocus(true);
      return;
    }
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      if (unsavedFocusYes) {
        finishClose();
      } else {
        showUnsavedCloseDialog = false;
      }
    }
  };

  $: if (showUnsavedCloseDialog && !wasUnsavedDialogOpen) {
    unsavedFocusYes = false;
    void focusUnsavedAction();
  }

  $: wasUnsavedDialogOpen = showUnsavedCloseDialog;

  const buildProfilePayload = () => ({
    confirmations: {
      confirmStartStreaming,
      confirmStopStreaming,
      confirmStartRecording,
      confirmStopRecording
    },
    general: {
      autoRecordWhenStartStreaming,
      openAdditionalSettingsInWindows,
      allowDraggablePopups,
      quickColorHistoryLimit,
      plannerUndoRedoLimit,
      realtimeRefreshSources,
      uiLanguage
    },
    output: {
      outputTab,
      streaming: {
        streamingAudioEncoder,
        streamingVideoEncoder,
        streamingBitrate,
        streamingKeyframeInterval,
        streamingProfile,
        streamingAdvanced,
        streamingRateControl,
        streamingCpuUsagePreset,
        streamingTune,
        streamingExtraOptions
      },
      recording: {
        recordFilenameNoSpaces,
        recordingFormat,
        recordingAdvanced,
        recordingCustomOutput,
        recordingVideoEncoder,
        recordingAudioEncoder,
        recordingCustomMuxerSettings,
        recordingTypeOutput,
        recordingPathCustom,
        recordingContainerFormat,
        recordingMuxerSettings,
        recordingVideoBitrate,
        recordingVideoEncoderCustom,
        recordingVideoEncoderSettings,
        recordingAudioBitrate,
        recordingAudioEncoderCustom,
        recordingAudioEncoderSettings
      },
      audioTracks: {
        audioChannel1Bitrate,
        audioChannel1Name,
        audioChannel2Bitrate,
        audioChannel2Name,
        audioChannel3Bitrate,
        audioChannel3Name,
        audioChannel4Bitrate,
        audioChannel4Name,
        audioChannel5Bitrate,
        audioChannel5Name
      }
    },
    audio: {
      audioSampleRate,
      audioChannels,
      audioDesktopDevice,
      audioMicDevice,
      audioMonitoringDevice,
      audioMonitoringLowLatency
    },
    video: {
      videoCanvasResolution,
      videoOutputResolution,
      videoDownscaleFilter,
      videoFpsType,
      videoFpsCommon,
      videoFpsCommonCustom,
      videoFpsCommonCustomValue,
      videoFpsIntegerValue,
      videoFpsFractionalNumerator,
      videoFpsFractionalDenominator
    },
    shortcuts: {
      shortcutStartStreaming,
      shortcutStopStreaming,
      shortcutStartRecording,
      shortcutStopRecording,
      shortcutPauseRecording,
      shortcutResumeRecording,
      shortcutShowAllSources,
      shortcutHideAllSources,
      shortcutShowSourceTransform,
      shortcutHideSourceTransform,
      shortcutNextScene,
      shortcutPreviousScene,
      shortcutShowSceneItems,
      shortcutHideSceneItems
    },
    accessibility: {
      accessibilityHighContrast,
      accessibilityReduceMotion,
      accessibilityFocusIndicators,
      accessibilityUiScale,
      accessibilityFontSize,
      accessibilityFontFamily,
      accessibilityColorVision
    },
    look: {
      selectedThemeId
    }
  });

  const save = () => {
    accessibilitySaved = true;
    dispatch("save", { profile: buildProfilePayload() });
  };
  const exportScenes = () => dispatch("exportScenes");
  const exportObsScenes = () => dispatch("exportObsScenes");
  const exportObsProfile = () => dispatch("exportObsProfile");
  const importScenes = (content: string, format: "revo" | "obs") =>
    dispatch("importScenes", { content, format });
  const importObsProfile = (content: string) => dispatch("importObsProfile", { content });
  const switchProfile = (name: string) => dispatch("switchProfile", { name });
  const createProfile = (name: string) => dispatch("createProfile", { name });
  const toggleDemo = (checked: boolean) => {
    demoMode = checked;
    dispatch("toggleDemo", { checked });
  };
  const toggleAutorescaleInputs = (checked: boolean) => {
    autorescaleInputs = checked;
    dispatch("toggleAutorescaleInputs", { checked });
  };

  let active: string = 'General';
  let outputTab: "Streaming" | "Recording" | "Audio" = "Streaming";
  let uiLanguage = "en";
  let availableThemes: ThemeInfo[] = [];
  let selectedThemeId = "";
  let themesLoading = false;
  let themesError = "";
  let themesInfo = "";
  let importThemeInputEl: HTMLInputElement | null = null;

  const sanitizeThemeId = (raw: string) =>
    String(raw ?? "")
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9_-]+/g, "-")
      .replace(/-+/g, "-")
      .replace(/^-|-$/g, "");

  const titleCase = (raw: string) =>
    raw
      .replace(/[_-]+/g, " ")
      .replace(/\s+/g, " ")
      .trim()
      .split(" ")
      .filter(Boolean)
      .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
      .join(" ");

  const audioBitrateOptions = [32, 64, 96, 128, 160, 192, 224, 256, 288, 320];

  let streamingAudioEncoder = "aac";
  let streamingVideoEncoder = "x264";
  let streamingBitrate = "6000";
  let streamingKeyframeInterval = "2";
  let streamingProfile = "high";
  let streamingAdvanced = false;
  let streamingRateControl = "CBR";
  let streamingCpuUsagePreset = "veryfast";
  let streamingTune = "none";
  let streamingExtraOptions = "";

  let recordFilenameNoSpaces = true;
  let recordingFormat = "mkv";
  let recordingAdvanced = false;
  let recordingCustomOutput = false;
  let recordingVideoEncoder = "x264";
  let recordingAudioEncoder = "aac";
  let recordingCustomMuxerSettings = "";

  let recordingTypeOutput = "file";
  let recordingPathCustom = "";
  let recordingContainerFormat = "mkv";
  let recordingMuxerSettings = "";
  let recordingVideoBitrate = "6000";
  let recordingVideoEncoderCustom = "x264";
  let recordingVideoEncoderSettings = "";
  let recordingAudioBitrate = "128";
  let recordingAudioEncoderCustom = "aac";
  let recordingAudioEncoderSettings = "";

  let audioChannel1Bitrate = "128";
  let audioChannel1Name = "";
  let audioChannel2Bitrate = "128";
  let audioChannel2Name = "";
  let audioChannel3Bitrate = "128";
  let audioChannel3Name = "";
  let audioChannel4Bitrate = "128";
  let audioChannel4Name = "";
  let audioChannel5Bitrate = "128";
  let audioChannel5Name = "";

  let audioSampleRate = "48000";
  let audioChannels = "stereo";
  let audioDesktopDevice = "disabled";
  let audioMicDevice = "disabled";
  let audioMonitoringDevice = "default";
  let audioMonitoringLowLatency = false;

  let videoCanvasResolution = "1920x1080";
  let videoOutputResolution = "1920x1080";
  let videoDownscaleFilter = "lanczos";
  let videoFpsType = "common";
  let videoFpsCommon = "60";
  let videoFpsCommonCustom = false;
  let videoFpsCommonCustomValue = "";
  let videoFpsIntegerValue = "60";
  let videoFpsFractionalNumerator = "60000";
  let videoFpsFractionalDenominator = "1001";

  let shortcutStartStreaming = "";
  let shortcutStopStreaming = "";
  let shortcutStartRecording = "";
  let shortcutStopRecording = "";
  let shortcutPauseRecording = "";
  let shortcutResumeRecording = "";
  let shortcutShowAllSources = "";
  let shortcutHideAllSources = "";
  let shortcutShowSourceTransform = "";
  let shortcutHideSourceTransform = "";
  let shortcutNextScene = "";
  let shortcutPreviousScene = "";
  let shortcutShowSceneItems = "";
  let shortcutHideSceneItems = "";

  let showShortcutCapture = false;
  let shortcutCaptureField = "";
  let shortcutCaptureLabel = "";
  let shortcutCapturePreview = "";
  let shortcutCaptureListening = false;

  const getShortcutValue = (field: string) => {
    switch (field) {
      case "shortcutStartStreaming": return shortcutStartStreaming;
      case "shortcutStopStreaming": return shortcutStopStreaming;
      case "shortcutStartRecording": return shortcutStartRecording;
      case "shortcutStopRecording": return shortcutStopRecording;
      case "shortcutPauseRecording": return shortcutPauseRecording;
      case "shortcutResumeRecording": return shortcutResumeRecording;
      case "shortcutShowAllSources": return shortcutShowAllSources;
      case "shortcutHideAllSources": return shortcutHideAllSources;
      case "shortcutShowSourceTransform": return shortcutShowSourceTransform;
      case "shortcutHideSourceTransform": return shortcutHideSourceTransform;
      case "shortcutNextScene": return shortcutNextScene;
      case "shortcutPreviousScene": return shortcutPreviousScene;
      case "shortcutShowSceneItems": return shortcutShowSceneItems;
      case "shortcutHideSceneItems": return shortcutHideSceneItems;
      default: return "";
    }
  };

  const setShortcutValue = (field: string, value: string) => {
    switch (field) {
      case "shortcutStartStreaming": shortcutStartStreaming = value; break;
      case "shortcutStopStreaming": shortcutStopStreaming = value; break;
      case "shortcutStartRecording": shortcutStartRecording = value; break;
      case "shortcutStopRecording": shortcutStopRecording = value; break;
      case "shortcutPauseRecording": shortcutPauseRecording = value; break;
      case "shortcutResumeRecording": shortcutResumeRecording = value; break;
      case "shortcutShowAllSources": shortcutShowAllSources = value; break;
      case "shortcutHideAllSources": shortcutHideAllSources = value; break;
      case "shortcutShowSourceTransform": shortcutShowSourceTransform = value; break;
      case "shortcutHideSourceTransform": shortcutHideSourceTransform = value; break;
      case "shortcutNextScene": shortcutNextScene = value; break;
      case "shortcutPreviousScene": shortcutPreviousScene = value; break;
      case "shortcutShowSceneItems": shortcutShowSceneItems = value; break;
      case "shortcutHideSceneItems": shortcutHideSceneItems = value; break;
    }
  };

  const formatShortcutKey = (key: string) => {
    if (key === " ") return "Space";
    if (key === "ArrowUp" || key === "ArrowDown" || key === "ArrowLeft" || key === "ArrowRight") return key;
    if (key.length === 1) return key.toUpperCase();
    if (key === "Control" || key === "Shift" || key === "Alt" || key === "Meta") return "";
    return key;
  };

  const loadThemes = async () => {
    if (themesLoading) return;
    themesLoading = true;
    themesError = "";
    themesInfo = "";
    try {
      const rootArg = rootDir.trim().length ? rootDir.trim() : null;
      availableThemes = await invoke<ThemeInfo[]>("themes_list", { rootDir: rootArg });
    } catch (err) {
      themesError = String(err);
      availableThemes = [];
    } finally {
      themesLoading = false;
    }
  };

  const openThemeImportPicker = () => {
    themesError = "";
    themesInfo = "";
    importThemeInputEl?.click();
  };

  const importThemeFromPicker = async (event: Event) => {
    themesError = "";
    themesInfo = "";
    const input = event.currentTarget as HTMLInputElement;
    const selectedFile = input.files?.[0];
    if (!selectedFile) return;

    try {
      const fileName = selectedFile.name.toLowerCase();
      if (!fileName.endsWith(".revotheme")) {
        throw new Error("Only .revotheme files are supported");
      }

      const data = new Uint8Array(await selectedFile.arrayBuffer());
      if (!data.length) {
        throw new Error(".revotheme file is empty");
      }
      const rootArg = rootDir.trim().length ? rootDir.trim() : null;

      const imported = await invoke<ThemeInfo>("themes_import_archive", {
        rootDir: rootArg,
        fileName: selectedFile.name,
        data: Array.from(data)
      });

      await loadThemes();
      selectedThemeId = imported.id;
      themesInfo = `Imported theme: ${imported.name}`;
    } catch (err) {
      themesError = String(err);
    } finally {
      input.value = "";
    }
  };

  const extractDefaultTheme = async () => {
    try {
      const alreadyExists = availableThemes.some((theme) => theme.id === "default_local");
      if (alreadyExists && typeof window !== "undefined") {
        const confirmed = window.confirm("default_local exists. Overwrite it with values generated from the currently active theme?");
        if (!confirmed) {
          return;
        }
      }

      const rootStyle = typeof document !== "undefined" ? getComputedStyle(document.documentElement) : null;
      const bg = rootStyle?.getPropertyValue("--bg")?.trim() || "#0f1115";
      const surface = rootStyle?.getPropertyValue("--surface")?.trim() || "#151820";
      const surface2 = rootStyle?.getPropertyValue("--surface-2")?.trim() || "#1b1f2a";
      const surface3 = rootStyle?.getPropertyValue("--surface-3")?.trim() || "#232838";
      const border = rootStyle?.getPropertyValue("--border")?.trim() || "#2b3142";
      const borderStrong = rootStyle?.getPropertyValue("--border-strong")?.trim() || "#3a4258";
      const text = rootStyle?.getPropertyValue("--text")?.trim() || "#e5e7eb";
      const textMuted = rootStyle?.getPropertyValue("--text-muted")?.trim() || "#a1a1aa";
      const accent = rootStyle?.getPropertyValue("--accent")?.trim() || "#5b7cfa";
      const accentStrong = rootStyle?.getPropertyValue("--accent-strong")?.trim() || "#3b5bdb";
      const success = rootStyle?.getPropertyValue("--success")?.trim() || "#22c55e";
      const danger = rootStyle?.getPropertyValue("--danger")?.trim() || "#ef4444";
      const warning = rootStyle?.getPropertyValue("--warning")?.trim() || "#f59e0b";
      const rootArg = rootDir.trim().length ? rootDir.trim() : null;
      await invoke<string>("themes_extract_default", {
        rootDir: rootArg,
        bg,
        surface,
        surface2,
        surface3,
        border,
        borderStrong,
        text,
        textMuted,
        accent,
        accentStrong,
        success,
        danger,
        warning
      });
      await loadThemes();
      selectedThemeId = "default_local";
    } catch (err) {
      themesError = String(err);
    }
  };

  const stopShortcutCaptureListener = () => {
    if (typeof window === "undefined" || !shortcutCaptureListening) return;
    window.removeEventListener("keydown", handleShortcutCaptureKeydown, true);
    shortcutCaptureListening = false;
  };

  const closeShortcutCapture = () => {
    showShortcutCapture = false;
    shortcutCaptureField = "";
    shortcutCaptureLabel = "";
    shortcutCapturePreview = "";
    stopShortcutCaptureListener();
  };

  const handleShortcutCaptureKeydown = (event: KeyboardEvent) => {
    if (!showShortcutCapture || !shortcutCaptureField) return;
    event.preventDefault();
    event.stopPropagation();

    if (event.key === "Escape") {
      closeShortcutCapture();
      return;
    }

    if (event.key === "Backspace" || event.key === "Delete") {
      setShortcutValue(shortcutCaptureField, "");
      closeShortcutCapture();
      return;
    }

    const keyPart = formatShortcutKey(event.key);
    if (!keyPart) return;

    const parts: string[] = [];
    if (event.ctrlKey) parts.push("Ctrl");
    if (event.altKey) parts.push("Alt");
    if (event.shiftKey) parts.push("Shift");
    if (event.metaKey) parts.push("Meta");
    parts.push(keyPart);

    const combo = parts.join("+");
    shortcutCapturePreview = combo;
    setShortcutValue(shortcutCaptureField, combo);
    closeShortcutCapture();
  };

  const openShortcutCapture = (field: string, label: string) => {
    shortcutCaptureField = field;
    shortcutCaptureLabel = label;
    shortcutCapturePreview = getShortcutValue(field);
    showShortcutCapture = true;

    if (typeof window !== "undefined" && !shortcutCaptureListening) {
      window.addEventListener("keydown", handleShortcutCaptureKeydown, true);
      shortcutCaptureListening = true;
    }
  };

  let accessibilityHighContrast = false;
  let accessibilityReduceMotion = false;
  let accessibilityFocusIndicators = true;
  let accessibilityUiScale = "100";
  let accessibilityFontSize = "100";
  let accessibilityFontFamily = "system";
  let accessibilityColorVision = "none";

  const applyIncomingUiProfile = (profile: Record<string, unknown> | null) => {
    const root = asRecord(profile);
    const confirmations = asRecord(root.confirmations);
    const general = asRecord(root.general);
    const output = asRecord(root.output);
    const streaming = asRecord(asRecord(output.streaming));
    const recording = asRecord(asRecord(output.recording));
    const audioTracks = asRecord(asRecord(output.audioTracks));
    const audio = asRecord(root.audio);
    const video = asRecord(root.video);
    const shortcuts = asRecord(root.shortcuts);
    const accessibility = asRecord(root.accessibility);
    const look = asRecord(root.look);

    confirmStartStreaming = readBool(confirmations, "confirmStartStreaming", false);
    confirmStopStreaming = readBool(confirmations, "confirmStopStreaming", false);
    confirmStartRecording = readBool(confirmations, "confirmStartRecording", false);
    confirmStopRecording = readBool(confirmations, "confirmStopRecording", false);
    autoRecordWhenStartStreaming = readBool(general, "autoRecordWhenStartStreaming", false);
    openAdditionalSettingsInWindows = readBool(general, "openAdditionalSettingsInWindows", false);
    allowDraggablePopups = readBool(general, "allowDraggablePopups", false);
    quickColorHistoryLimit = readInt(general, "quickColorHistoryLimit", 5, 1, 15);
    plannerUndoRedoLimit = readInt(general, "plannerUndoRedoLimit", 5, 1, 50);
    realtimeRefreshSources = readBool(general, "realtimeRefreshSources", false);
    uiLanguage = readString(general, "uiLanguage", "en");

    outputTab = readString(output, "outputTab", "Streaming") as "Streaming" | "Recording" | "Audio";
    streamingAudioEncoder = readString(streaming, "streamingAudioEncoder", "aac");
    streamingVideoEncoder = readString(streaming, "streamingVideoEncoder", "x264");
    streamingBitrate = readString(streaming, "streamingBitrate", "6000");
    streamingKeyframeInterval = readString(streaming, "streamingKeyframeInterval", "2");
    streamingProfile = readString(streaming, "streamingProfile", "high");
    streamingAdvanced = readBool(streaming, "streamingAdvanced", false);
    streamingRateControl = readString(streaming, "streamingRateControl", "CBR");
    streamingCpuUsagePreset = readString(streaming, "streamingCpuUsagePreset", "veryfast");
    streamingTune = readString(streaming, "streamingTune", "none");
    streamingExtraOptions = readString(streaming, "streamingExtraOptions", "");

    recordFilenameNoSpaces = readBool(recording, "recordFilenameNoSpaces", true);
    recordingFormat = readString(recording, "recordingFormat", "mkv");
    recordingAdvanced = readBool(recording, "recordingAdvanced", false);
    recordingCustomOutput = readBool(recording, "recordingCustomOutput", false);
    recordingVideoEncoder = readString(recording, "recordingVideoEncoder", "x264");
    recordingAudioEncoder = readString(recording, "recordingAudioEncoder", "aac");
    recordingCustomMuxerSettings = readString(recording, "recordingCustomMuxerSettings", "");
    recordingTypeOutput = readString(recording, "recordingTypeOutput", "file");
    recordingPathCustom = readString(recording, "recordingPathCustom", "");
    recordingContainerFormat = readString(recording, "recordingContainerFormat", "mkv");
    recordingMuxerSettings = readString(recording, "recordingMuxerSettings", "");
    recordingVideoBitrate = readString(recording, "recordingVideoBitrate", "6000");
    recordingVideoEncoderCustom = readString(recording, "recordingVideoEncoderCustom", "x264");
    recordingVideoEncoderSettings = readString(recording, "recordingVideoEncoderSettings", "");
    recordingAudioBitrate = readString(recording, "recordingAudioBitrate", "128");
    recordingAudioEncoderCustom = readString(recording, "recordingAudioEncoderCustom", "aac");
    recordingAudioEncoderSettings = readString(recording, "recordingAudioEncoderSettings", "");

    audioChannel1Bitrate = readString(audioTracks, "audioChannel1Bitrate", "128");
    audioChannel1Name = readString(audioTracks, "audioChannel1Name", "");
    audioChannel2Bitrate = readString(audioTracks, "audioChannel2Bitrate", "128");
    audioChannel2Name = readString(audioTracks, "audioChannel2Name", "");
    audioChannel3Bitrate = readString(audioTracks, "audioChannel3Bitrate", "128");
    audioChannel3Name = readString(audioTracks, "audioChannel3Name", "");
    audioChannel4Bitrate = readString(audioTracks, "audioChannel4Bitrate", "128");
    audioChannel4Name = readString(audioTracks, "audioChannel4Name", "");
    audioChannel5Bitrate = readString(audioTracks, "audioChannel5Bitrate", "128");
    audioChannel5Name = readString(audioTracks, "audioChannel5Name", "");

    audioSampleRate = readString(audio, "audioSampleRate", "48000");
    audioChannels = readString(audio, "audioChannels", "stereo");
    audioDesktopDevice = readString(audio, "audioDesktopDevice", "disabled");
    audioMicDevice = readString(audio, "audioMicDevice", "disabled");
    audioMonitoringDevice = readString(audio, "audioMonitoringDevice", "default");
    audioMonitoringLowLatency = readBool(audio, "audioMonitoringLowLatency", false);

    videoCanvasResolution = readString(video, "videoCanvasResolution", "1920x1080");
    videoOutputResolution = readString(video, "videoOutputResolution", "1920x1080");
    videoDownscaleFilter = readString(video, "videoDownscaleFilter", "lanczos");
    videoFpsType = readString(video, "videoFpsType", "common");
    videoFpsCommon = readString(video, "videoFpsCommon", "60");
    videoFpsCommonCustom = readBool(video, "videoFpsCommonCustom", false);
    videoFpsCommonCustomValue = readString(video, "videoFpsCommonCustomValue", "");
    videoFpsIntegerValue = readString(video, "videoFpsIntegerValue", "60");
    videoFpsFractionalNumerator = readString(video, "videoFpsFractionalNumerator", "60000");
    videoFpsFractionalDenominator = readString(video, "videoFpsFractionalDenominator", "1001");

    shortcutStartStreaming = readString(shortcuts, "shortcutStartStreaming", "");
    shortcutStopStreaming = readString(shortcuts, "shortcutStopStreaming", "");
    shortcutStartRecording = readString(shortcuts, "shortcutStartRecording", "");
    shortcutStopRecording = readString(shortcuts, "shortcutStopRecording", "");
    shortcutPauseRecording = readString(shortcuts, "shortcutPauseRecording", "");
    shortcutResumeRecording = readString(shortcuts, "shortcutResumeRecording", "");
    shortcutShowAllSources = readString(shortcuts, "shortcutShowAllSources", "");
    shortcutHideAllSources = readString(shortcuts, "shortcutHideAllSources", "");
    shortcutShowSourceTransform = readString(shortcuts, "shortcutShowSourceTransform", "");
    shortcutHideSourceTransform = readString(shortcuts, "shortcutHideSourceTransform", "");
    shortcutNextScene = readString(shortcuts, "shortcutNextScene", "");
    shortcutPreviousScene = readString(shortcuts, "shortcutPreviousScene", "");
    shortcutShowSceneItems = readString(shortcuts, "shortcutShowSceneItems", "");
    shortcutHideSceneItems = readString(shortcuts, "shortcutHideSceneItems", "");

    accessibilityHighContrast = readBool(accessibility, "accessibilityHighContrast", initialAccessibilityHighContrast);
    accessibilityReduceMotion = readBool(accessibility, "accessibilityReduceMotion", initialAccessibilityReduceMotion);
    accessibilityFocusIndicators = readBool(accessibility, "accessibilityFocusIndicators", initialAccessibilityFocusIndicators);
    accessibilityUiScale = readString(accessibility, "accessibilityUiScale", initialAccessibilityUiScale);
    accessibilityFontSize = readString(accessibility, "accessibilityFontSize", initialAccessibilityFontSize);
    accessibilityFontFamily = readString(accessibility, "accessibilityFontFamily", initialAccessibilityFontFamily);
    accessibilityColorVision = readString(accessibility, "accessibilityColorVision", initialAccessibilityColorVision);
    selectedThemeId = readString(look, "selectedThemeId", "");
  };

  const toggleRecordingAdvanced = (checked: boolean) => {
    recordingAdvanced = checked;
    if (checked) {
      recordingCustomOutput = false;
    }
  };

  const toggleRecordingCustomOutput = (checked: boolean) => {
    recordingCustomOutput = checked;
    if (checked) {
      recordingAdvanced = false;
    }
  };

  type ServiceServer = { name: string; url: string };
  type LocalService = { name: string; servers: ServiceServer[] };

  const streamServices = [
    {
      id: 'youtube',
      name: 'YouTube',
      url: 'rtmps://a.rtmps.youtube.com/live2',
      svg: '<svg viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M21.6 7.2a2.8 2.8 0 0 0-2-2C17.9 4.7 12 4.7 12 4.7s-5.9 0-7.6.5a2.8 2.8 0 0 0-2 2C2 9 2 12 2 12s0 3 .4 4.8a2.8 2.8 0 0 0 2 2c1.7.5 7.6.5 7.6.5s5.9 0 7.6-.5a2.8 2.8 0 0 0 2-2c.4-1.8.4-4.8.4-4.8s0-3-.4-4.8ZM10 15.5V8.5L16 12l-6 3.5Z"/></svg>'
    },
    {
      id: 'twitch',
      name: 'Twitch',
      url: 'rtmp://live.twitch.tv/app',
      svg: '<svg viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M4 3h16v11l-4 4h-4l-2 2H7v-2H4V3Zm2 2v9h3v2l2-2h4l3-3V5H6Zm4 2h2v4h-2V7Zm4 0h2v4h-2V7Z"/></svg>'
    },
    {
      id: 'kick',
      name: 'Kick',
      url: 'rtmps://fa723fc1b171.global-contribute.live-video.net:443/app',
      svg: '<svg viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M4 4h6v6h2L16 4h4l-5 7 5 9h-4l-4-7h-2v7H4V4Z"/></svg>'
    }
  ];

  const selectStreamService = (url: string) => {
    streamUrl = url;
    const selected = streamServices.find((service) => service.url === url);
    selectedPresetServiceId = selected?.id ?? "";
    selectedLocalService = "";
    selectedLocalServerUrl = "";
    showOtherServices = false;
  };

  let showOtherServices = false;
  let loadingOtherServices = false;
  let otherServicesError = "";
  let localServices: LocalService[] = [];
  let selectedLocalService = "";
  let selectedLocalServerUrl = "";
  let selectedPresetServiceId = "";
  let confirmStartStreaming = false;
  let confirmStopStreaming = false;
  let confirmStartRecording = false;
  let confirmStopRecording = false;
  let autoRecordWhenStartStreaming = false;
  let openAdditionalSettingsInWindows = false;
  let quickColorHistoryLimit = 5;
  let plannerUndoRedoLimit = 5;
  let realtimeRefreshSources = false;

  $: if (openAdditionalSettingsInWindows) {
    allowDraggablePopups = false;
  }

  $: quickColorHistoryLimit = Math.max(1, Math.min(15, Number(quickColorHistoryLimit) || 5));
  $: plannerUndoRedoLimit = Math.max(1, Math.min(50, Number(plannerUndoRedoLimit) || 5));

  const isPresetServiceActive = (id: string, url: string) =>
    selectedPresetServiceId
      ? selectedPresetServiceId === id
      : streamUrl.trim() === url;

  $: selectedLocalServiceData =
    localServices.find((service) => service.name === selectedLocalService) ?? null;

  $: selectedLocalServers = selectedLocalServiceData?.servers ?? [];

  $: if (!selectedLocalService) {
    selectedLocalServerUrl = "";
  } else {
    if (!selectedLocalServers.length) {
      selectedLocalServerUrl = "";
    } else if (!selectedLocalServers.some((server) => server.url === selectedLocalServerUrl)) {
      selectedLocalServerUrl = selectedLocalServers[0].url;
      streamUrl = selectedLocalServerUrl;
      selectedPresetServiceId = "";
    }
  }

  $: if (active === 'Look' && !themesLoading && availableThemes.length === 0) {
    void loadThemes();
  }

  const pickLocalService = (serviceName: string) => {
    selectedLocalService = serviceName;
  };

  const pickLocalServer = (url: string) => {
    selectedLocalServerUrl = url;
    streamUrl = url;
    selectedPresetServiceId = "";
  };

  const loadLocalServices = async () => {
    if (localServices.length || loadingOtherServices) return;
    loadingOtherServices = true;
    otherServicesError = "";

    try {
      const raw = await invoke<string>("load_rtmp_services_json");
      const payload = JSON.parse(raw);

      if (!payload?.services || !Array.isArray(payload.services)) {
        throw new Error("Cannot load local services.json");
      }

      localServices = payload.services
        .map((service: any) => ({
          name: String(service?.name ?? ""),
          servers: Array.isArray(service?.servers)
            ? service.servers
                .map((server: any) => ({
                  name: String(server?.name ?? ""),
                  url: String(server?.url ?? "")
                }))
                .filter((server: ServiceServer) => Boolean(server.url))
            : []
        }))
            .filter((service: LocalService) => Boolean(service.name));

      if (!selectedLocalService && !selectedLocalServerUrl) {
        selectedLocalService = "";
      }
    } catch (err) {
      otherServicesError = String(err);
    } finally {
      loadingOtherServices = false;
    }
  };

  const toggleOtherServices = async () => {
    showOtherServices = !showOtherServices;
    if (showOtherServices) {
      await loadLocalServices();
    }
  };

  const navItems = [
    {
      label: 'General',
      svg: `
        <svg viewBox="0 0 32 32" aria-hidden="true" width="24" height="24">
          <path d="M14.4333 29.3333C13.8333 29.3333 13.3169 29.1333 12.884 28.7333C12.4511 28.3333 12.1898 27.8444 12.1 27.2667L11.8 25.0667C11.5111 24.9556 11.2391 24.8222 10.984 24.6667C10.7289 24.5111 10.4787 24.3444 10.2333 24.1667L8.16667 25.0333C7.61112 25.2778 7.05556 25.3 6.5 25.1C5.94445 24.9 5.51112 24.5444 5.2 24.0333L3.63334 21.3C3.32223 20.7889 3.23334 20.2444 3.36667 19.6667C3.5 19.0889 3.8 18.6111 4.26667 18.2333L6.03334 16.9C6.01112 16.7444 6 16.5942 6 16.4493V15.5493C6 15.4053 6.01112 15.2556 6.03334 15.1L4.26667 13.7667C3.8 13.3889 3.5 12.9111 3.36667 12.3333C3.23334 11.7556 3.32223 11.2111 3.63334 10.7L5.2 7.96667C5.51112 7.45556 5.94445 7.10001 6.5 6.90001C7.05556 6.70001 7.61112 6.72223 8.16667 6.96667L10.2333 7.83334C10.4778 7.65556 10.7333 7.48889 11 7.33334C11.2667 7.17778 11.5333 7.04445 11.8 6.93334L12.1 4.73334C12.1889 4.15556 12.4502 3.66667 12.884 3.26667C13.3178 2.86667 13.8342 2.66667 14.4333 2.66667H17.5667C18.1667 2.66667 18.6836 2.86667 19.1173 3.26667C19.5511 3.66667 19.812 4.15556 19.9 4.73334L20.2 6.93334C20.4889 7.04445 20.7613 7.17778 21.0173 7.33334C21.2733 7.48889 21.5231 7.65556 21.7667 7.83334L23.8333 6.96667C24.3889 6.72223 24.9444 6.70001 25.5 6.90001C26.0556 7.10001 26.4889 7.45556 26.8 7.96667L28.3667 10.7C28.6778 11.2111 28.7667 11.7556 28.6333 12.3333C28.5 12.9111 28.2 13.3889 27.7333 13.7667L25.9667 15.1C25.9889 15.2556 26 15.4058 26 15.5507V16.4493C26 16.5942 25.9778 16.7444 25.9333 16.9L27.7 18.2333C28.1667 18.6111 28.4667 19.0889 28.6 19.6667C28.7333 20.2444 28.6444 20.7889 28.3333 21.3L26.7333 24.0333C26.4222 24.5444 25.9889 24.9 25.4333 25.1C24.8778 25.3 24.3222 25.2778 23.7667 25.0333L21.7667 24.1667C21.5222 24.3444 21.2667 24.5111 21 24.6667C20.7333 24.8222 20.4667 24.9556 20.2 25.0667L19.9 27.2667C19.8111 27.8444 19.5502 28.3333 19.1173 28.7333C18.6844 29.1333 18.1676 29.3333 17.5667 29.3333H14.4333ZM16.0667 20.6667C17.3556 20.6667 18.4556 20.2111 19.3667 19.3C20.2778 18.3889 20.7333 17.2889 20.7333 16C20.7333 14.7111 20.2778 13.6111 19.3667 12.7C18.4556 11.7889 17.3556 11.3333 16.0667 11.3333C14.7556 11.3333 13.6498 11.7889 12.7493 12.7C11.8489 13.6111 11.3991 14.7111 11.4 16C11.4009 17.2889 11.8511 18.3889 12.7507 19.3C13.6502 20.2111 14.7556 20.6667 16.0667 20.6667Z" fill="#E6E6E6"/>
        </svg>
      `
    },
    {
      label: 'Look',
      svg: `
        <svg viewBox="0 0 32 32" aria-hidden="true" width="24" height="24">
          <g clip-path="url(#clip0_2_50)">
            <path d="M24.5685 18.2847L13.2565 6.97271L15.6565 4.57271C16.4565 3.67671 19.3365 3.82071 21.2725 5.08471C23.2085 6.36471 23.5605 7.13271 25.9285 8.44471C27.8165 9.46871 29.8485 10.4607 33.0485 9.80471L24.5685 18.2847ZM23.4325 19.4207L12.1205 8.10871L9.28849 10.9407C9.14016 11.0887 9.02248 11.2646 8.94219 11.4581C8.8619 11.6517 8.82058 11.8592 8.82058 12.0687C8.82058 12.2783 8.8619 12.4858 8.94219 12.6793C9.02248 12.8729 9.14016 13.0487 9.28849 13.1967L10.9845 14.8927C11.6085 15.5167 11.6085 16.5407 10.9845 17.1647C10.0245 18.1247 8.69649 18.9407 7.44849 19.8687C6.88849 20.2847 6.32849 20.7167 5.83249 21.2127C3.68849 23.3567 2.04049 26.3167 3.64049 27.9007C5.22449 29.5007 8.18449 27.8527 10.3285 25.7247C10.8245 25.2287 11.2565 24.6687 11.6885 24.0927C12.6005 22.8447 13.4165 21.5167 14.3925 20.5567C14.5405 20.4084 14.7163 20.2907 14.9099 20.2104C15.1034 20.1301 15.3109 20.0888 15.5205 20.0888C15.73 20.0888 15.9375 20.1301 16.1311 20.2104C16.3246 20.2907 16.5005 20.4084 16.6485 20.5567L18.3445 22.2527C18.9685 22.8767 19.9765 22.8767 20.6005 22.2527L23.4325 19.4207Z" fill="#E6E6E6"/>
          </g>
          <defs>
            <clipPath id="clip0_2_50">
              <rect width="32" height="32" fill="white"/>
            </clipPath>
          </defs>
        </svg>
      `
    },
    {
      label: 'Broadcast',
      svg: `
        <svg viewBox="0 0 32 32" aria-hidden="true" width="24" height="24">
          <g clip-path="url(#clip0_2_57)">
            <path d="M7.91333 27.2493C7.67189 27.2477 7.43945 27.1575 7.26 26.996C6.78127 26.5872 6.33545 26.1414 5.92667 25.6627C3.82717 23.2636 2.66992 20.184 2.66992 16.996C2.66992 13.808 3.82717 10.7284 5.92667 8.32933C6.33545 7.8506 6.78127 7.40478 7.26 6.99599C7.35884 6.90913 7.47385 6.84263 7.59843 6.80029C7.72302 6.75796 7.85474 6.74063 7.98604 6.7493C8.11734 6.75798 8.24563 6.79247 8.36357 6.85082C8.48151 6.90917 8.58678 6.99022 8.67333 7.08933C8.84545 7.29042 8.93205 7.55089 8.9146 7.81501C8.89716 8.07913 8.77707 8.32594 8.58 8.50266C8.1728 8.85431 7.78972 9.23294 7.43333 9.63599C5.67216 11.6733 4.70299 14.2763 4.70299 16.9693C4.70299 19.6623 5.67216 22.2654 7.43333 24.3027C7.79244 24.6991 8.17467 25.0724 8.58 25.4227C8.7365 25.5564 8.8479 25.7353 8.89898 25.9347C8.95006 26.1341 8.93833 26.3445 8.8654 26.537C8.79247 26.7296 8.66189 26.8949 8.4915 27.0104C8.3211 27.126 8.11919 27.1861 7.91333 27.1827V27.2493Z" fill="#E6E6E6"/>
            <path d="M11.2067 23.836C10.9798 23.837 10.7591 23.7619 10.58 23.6227C10.1139 23.2657 9.69692 22.8488 9.33999 22.3827C8.09074 20.8629 7.40781 18.9566 7.40781 16.9893C7.40781 15.022 8.09074 13.1157 9.33999 11.596C9.70193 11.1337 10.1234 10.7212 10.5933 10.3693C10.6961 10.2874 10.8141 10.2268 10.9406 10.1911C11.0671 10.1554 11.1994 10.1454 11.3298 10.1615C11.4602 10.1776 11.5861 10.2197 11.7 10.2851C11.814 10.3506 11.9137 10.4381 11.9933 10.5427C12.1353 10.7467 12.1962 10.9963 12.1642 11.2428C12.1321 11.4893 12.0094 11.715 11.82 11.876C11.4777 12.1398 11.1688 12.4443 10.9 12.7827C9.93873 13.9482 9.413 15.4119 9.413 16.9227C9.413 18.4334 9.93873 19.8971 10.9 21.0627C11.1747 21.4058 11.4858 21.7124 11.8333 21.9827C12.0423 22.1481 12.1776 22.3892 12.2101 22.6537C12.2425 22.9183 12.1694 23.185 12.0067 23.396C11.9163 23.5261 11.797 23.6335 11.6583 23.7098C11.5195 23.7861 11.3649 23.8293 11.2067 23.836Z" fill="#E6E6E6"/>
            <path d="M19.94 16.996C19.9266 17.7213 19.7094 18.4282 19.3133 19.036C18.9733 19.556 18.5168 19.9897 17.98 20.3027C17.3642 20.6871 16.6526 20.8904 15.9267 20.8893C15.203 20.8875 14.4933 20.6893 13.8733 20.316C13.3321 19.9802 12.8758 19.5239 12.54 18.9827C12.1614 18.3671 11.961 17.6586 11.961 16.936C11.961 16.2134 12.1614 15.5049 12.54 14.8893C13.0914 14.0064 13.9653 13.3732 14.9761 13.1242C15.9869 12.8752 17.0549 13.0301 17.9533 13.556C18.4946 13.8918 18.9509 14.3481 19.2867 14.8893C19.7087 15.5112 19.9361 16.2445 19.94 16.996Z" fill="#E6E6E6"/>
            <path d="M20.8067 23.836C20.6562 23.8352 20.5077 23.8014 20.3718 23.7369C20.2359 23.6723 20.1158 23.5787 20.02 23.4627C19.9377 23.3588 19.8769 23.2397 19.841 23.1122C19.8051 22.9846 19.795 22.8512 19.8111 22.7197C19.8272 22.5882 19.8693 22.4613 19.9349 22.3462C20.0006 22.2311 20.0884 22.1302 20.1933 22.0493C20.5309 21.7847 20.8353 21.4802 21.1 21.1427C22.0685 19.9805 22.599 18.5155 22.599 17.0027C22.599 15.4898 22.0685 14.0248 21.1 12.8627C20.8276 12.5235 20.5191 12.2151 20.18 11.9427C20.0661 11.8661 19.9691 11.7669 19.8952 11.6513C19.8212 11.5356 19.7719 11.406 19.7502 11.2704C19.7286 11.1349 19.735 10.9963 19.7693 10.8634C19.8035 10.7305 19.8647 10.606 19.9491 10.4977C20.0335 10.3895 20.1393 10.2998 20.2598 10.2342C20.3804 10.1686 20.5132 10.1285 20.6499 10.1165C20.7867 10.1044 20.9244 10.1207 21.0546 10.1642C21.1848 10.2077 21.3046 10.2775 21.4067 10.3693C21.8778 10.7284 22.2956 11.1418 22.66 11.6093C23.9141 13.1269 24.6001 15.034 24.6001 17.0027C24.6001 18.9713 23.9141 20.8784 22.66 22.396C22.3022 22.8574 21.8853 23.2698 21.42 23.6227C21.2455 23.7609 21.0293 23.8361 20.8067 23.836Z" fill="#E6E6E6"/>
            <path d="M24.0867 27.2493C23.882 27.25 23.682 27.1879 23.5137 27.0713C23.3454 26.9547 23.217 26.7893 23.1458 26.5974C23.0745 26.4054 23.0639 26.1963 23.1153 25.9981C23.1667 25.8 23.2778 25.6224 23.4333 25.4893C23.8378 25.1382 24.2156 24.7605 24.5667 24.356C26.3357 22.3225 27.31 19.718 27.31 17.0227C27.31 14.3274 26.3357 11.7229 24.5667 9.68934C24.2116 9.29375 23.8331 8.91975 23.4333 8.56934C23.2343 8.39446 23.1117 8.1486 23.0918 7.88443C23.0719 7.62025 23.1562 7.35878 23.3267 7.15601C23.4132 7.0569 23.5185 6.97585 23.6364 6.9175C23.7544 6.85915 23.8827 6.82466 24.014 6.81599C24.1453 6.80732 24.277 6.82464 24.4016 6.86697C24.5262 6.90931 24.6412 6.97581 24.74 7.06267C25.2173 7.47301 25.663 7.91871 26.0733 8.39601C28.1728 10.7951 29.3301 13.8747 29.3301 17.0627C29.3301 20.2507 28.1728 23.3303 26.0733 25.7293C25.663 26.2066 25.2173 26.6523 24.74 27.0627C24.5534 27.2048 24.3202 27.2714 24.0867 27.2493Z" fill="#E6E6E6"/>
          </g>
          <defs>
            <clipPath id="clip0_2_57">
              <rect width="32" height="32" fill="white"/>
            </clipPath>
          </defs>
        </svg>
      `
    },
    {
      label: 'Output',
      svg: `
        <svg viewBox="0 0 32 32" aria-hidden="true" width="24" height="24">
          <g clip-path="url(#clip0_2_64)">
            <path fill-rule="evenodd" clip-rule="evenodd" d="M3.66669 6.66669H23.6667V12.5092L30.3333 9.17594V22.8241L23.6667 19.4907V25.3333H3.66669V6.66669ZM17.6667 21.3333C18.7712 21.3333 19.6667 20.4379 19.6667 19.3333C19.6667 18.2288 18.7712 17.3333 17.6667 17.3333C16.5621 17.3333 15.6667 18.2288 15.6667 19.3333C15.6667 20.4379 16.5621 21.3333 17.6667 21.3333Z" fill="#E6E6E6"/>
          </g>
          <defs>
            <clipPath id="clip0_2_64">
              <rect width="32" height="32" fill="white"/>
            </clipPath>
          </defs>
        </svg>
      `
    },
    {
      label: 'Audio',
      svg: `
        <svg viewBox="0 0 32 32" aria-hidden="true" width="24" height="24">
          <g clip-path="url(#clip0_2_71)">
            <path d="M18 3.99999C18 3.80331 17.942 3.611 17.8332 3.44712C17.7244 3.28325 17.5698 3.15508 17.3885 3.07866C17.2073 3.00224 17.0076 2.98096 16.8143 3.01748C16.621 3.05401 16.4428 3.14671 16.302 3.28399L10.448 8.98399H7C6.20435 8.98399 5.44129 9.30006 4.87868 9.86267C4.31607 10.4253 4 11.1883 4 11.984V17.964C4 18.7596 4.31607 19.5227 4.87868 20.0853C5.44129 20.6479 6.20435 20.964 7 20.964H10.446L16.3 26.714C16.4406 26.8518 16.6188 26.9451 16.8122 26.9821C17.0057 27.019 17.2057 26.9981 17.3872 26.9218C17.5688 26.8456 17.7238 26.7174 17.8328 26.5534C17.9418 26.3894 18 26.1969 18 26V3.99999ZM20.222 9.37799C20.3038 9.2751 20.405 9.18934 20.5199 9.1256C20.6349 9.06187 20.7613 9.02141 20.8918 9.00655C21.0224 8.99169 21.1546 9.00272 21.281 9.039C21.4073 9.07528 21.5252 9.1361 21.628 9.21799L21.632 9.21999L21.636 9.22399L21.646 9.23199L21.676 9.25799L21.768 9.33799C21.8413 9.40466 21.936 9.49933 22.052 9.62199C22.278 9.86799 22.572 10.226 22.862 10.702C23.444 11.662 24.008 13.088 24.008 14.998C24.008 16.906 23.444 18.334 22.862 19.294C22.6286 19.6807 22.3573 20.0431 22.052 20.376C21.9235 20.5129 21.7887 20.6437 21.648 20.768L21.632 20.782H21.63C21.63 20.782 20.736 21.268 20.224 20.626C20.0594 20.42 19.9828 20.1573 20.0108 19.8951C20.0389 19.6329 20.1695 19.3924 20.374 19.226L20.378 19.222L20.414 19.19C20.4513 19.1567 20.508 19.1 20.584 19.02C20.7982 18.7848 20.9885 18.5289 21.152 18.256C21.572 17.566 22.008 16.492 22.008 14.996C22.008 13.5 21.572 12.43 21.152 11.742C20.9461 11.4028 20.6988 11.0904 20.416 10.812L20.38 10.78C20.1737 10.6147 20.0412 10.3743 20.0116 10.1116C19.982 9.84887 20.0576 9.5851 20.222 9.37799ZM23.626 5.21799C23.5236 5.13327 23.4054 5.0698 23.2782 5.03129C23.151 4.99278 23.0174 4.98001 22.8852 4.99373C22.7531 5.00744 22.6249 5.04736 22.5084 5.11115C22.3918 5.17495 22.2891 5.26134 22.2063 5.36528C22.1235 5.46922 22.0622 5.58862 22.0261 5.7165C21.99 5.84438 21.9797 5.97819 21.9959 6.11009C22.012 6.24199 22.0543 6.36934 22.1203 6.48471C22.1862 6.60008 22.2745 6.70114 22.38 6.78199L22.402 6.80199L22.506 6.89199C22.602 6.97599 22.736 7.10599 22.908 7.28199C23.248 7.63599 23.708 8.16799 24.168 8.86999C25.088 10.272 26.008 12.336 26.008 15.008C26.0158 17.1885 25.3754 19.3222 24.168 21.138C23.708 21.838 23.248 22.366 22.908 22.718C22.747 22.8852 22.5789 23.0454 22.404 23.198L22.382 23.218H22.38C22.1782 23.385 22.0499 23.6246 22.0229 23.8852C21.9959 24.1458 22.0722 24.4066 22.2354 24.6115C22.3987 24.8164 22.6358 24.9491 22.8959 24.981C23.1559 25.0128 23.4181 24.9414 23.626 24.782L23.692 24.728L23.842 24.598C23.968 24.484 24.142 24.322 24.348 24.108C24.9027 23.532 25.4022 22.9052 25.84 22.236C27.2607 20.0938 28.0145 17.5785 28.006 15.008C28.0113 12.4355 27.2579 9.91851 25.84 7.77199C25.4021 7.10213 24.9034 6.47413 24.35 5.89599C24.1408 5.67888 23.9219 5.47133 23.694 5.27399L23.648 5.23599L23.634 5.22399L23.63 5.21999L23.626 5.21799Z" fill="#E6E6E6"/>
          </g>
          <defs>
            <clipPath id="clip0_2_71">
              <rect width="32" height="32" fill="white"/>
            </clipPath>
          </defs>
        </svg>
      `
    },
    {
      label: 'Video',
      svg: `
        <svg viewBox="0 0 32 32" aria-hidden="true" width="24" height="24">
          <g clip-path="url(#clip0_2_78)">
            <path d="M7.66666 6.66667C7.31303 6.66667 6.9739 6.80714 6.72385 7.05719C6.4738 7.30724 6.33332 7.64638 6.33332 8V20C6.33332 20.3536 6.4738 20.6928 6.72385 20.9428C6.9739 21.1929 7.31303 21.3333 7.66666 21.3333H26.3333C26.6869 21.3333 27.0261 21.1929 27.2761 20.9428C27.5262 20.6928 27.6667 20.3536 27.6667 20V8C27.6667 7.64638 27.5262 7.30724 27.2761 7.05719C27.0261 6.80714 26.6869 6.66667 26.3333 6.66667H7.66666ZM7.66666 4H26.3333C27.3942 4 28.4116 4.42143 29.1618 5.17157C29.9119 5.92172 30.3333 6.93913 30.3333 8V20C30.3333 21.0609 29.9119 22.0783 29.1618 22.8284C28.4116 23.5786 27.3942 24 26.3333 24H7.66666C6.60579 24 5.58837 23.5786 4.83823 22.8284C4.08808 22.0783 3.66666 21.0609 3.66666 20L3.66666 8C3.66666 6.93913 4.08808 5.92172 4.83823 5.17157C5.58837 4.42143 6.60579 4 7.66666 4ZM13 25.3333H21C21.3536 25.3333 21.6928 25.4738 21.9428 25.7239C22.1928 25.9739 22.3333 26.313 22.3333 26.6667C22.3333 27.0203 22.1928 27.3594 21.9428 27.6095C21.6928 27.8595 21.3536 28 21 28H13C12.6464 28 12.3072 27.8595 12.0572 27.6095C11.8071 27.3594 11.6667 27.0203 11.6667 26.6667C11.6667 26.313 11.8071 25.9739 12.0572 25.7239C12.3072 25.4738 12.6464 25.3333 13 25.3333Z" fill="#E6E6E6"/>
          </g>
          <defs>
            <clipPath id="clip0_2_78">
              <rect width="32" height="32" fill="white"/>
            </clipPath>
          </defs>
        </svg>
      `
    },
    {
      label: 'Shortcuts',
      svg: `
        <svg viewBox="0 0 32 32" aria-hidden="true" width="24" height="24">
          <g clip-path="url(#clip0_2_85)">
            <path d="M26.6667 5.66669H5.33332C3.86666 5.66669 2.67999 6.86669 2.67999 8.33335L2.66666 21.6667C2.66666 23.1334 3.86666 24.3334 5.33332 24.3334H26.6667C28.1333 24.3334 29.3333 23.1334 29.3333 21.6667V8.33335C29.3333 6.86669 28.1333 5.66669 26.6667 5.66669ZM14.6667 9.66669H17.3333V12.3334H14.6667V9.66669ZM14.6667 13.6667H17.3333V16.3334H14.6667V13.6667ZM10.6667 9.66669H13.3333V12.3334H10.6667V9.66669ZM10.6667 13.6667H13.3333V16.3334H10.6667V13.6667ZM9.33332 16.3334H6.66666V13.6667H9.33332V16.3334ZM9.33332 12.3334H6.66666V9.66669H9.33332V12.3334ZM20 21.6667H12C11.2667 21.6667 10.6667 21.0667 10.6667 20.3334C10.6667 19.6 11.2667 19 12 19H20C20.7333 19 21.3333 19.6 21.3333 20.3334C21.3333 21.0667 20.7333 21.6667 20 21.6667ZM21.3333 16.3334H18.6667V13.6667H21.3333V16.3334ZM21.3333 12.3334H18.6667V9.66669H21.3333V12.3334ZM25.3333 16.3334H22.6667V13.6667H25.3333V16.3334ZM25.3333 12.3334H22.6667V9.66669H25.3333V12.3334Z" fill="#E6E6E6"/>
          </g>
          <defs>
            <clipPath id="clip0_2_85">
              <rect width="32" height="32" fill="white"/>
            </clipPath>
          </defs>
        </svg>
      `
    },
    {
      label: 'Accessibility',
      svg: `
        <svg viewBox="0 0 32 32" aria-hidden="true" width="24" height="24">
          <g clip-path="url(#clip0_2_92)">
            <path d="M3.20001 16C3.20001 8.93001 8.93001 3.20001 16 3.20001C23.07 3.20001 28.8 8.93001 28.8 16C28.8 23.07 23.07 28.8 16 28.8C8.93001 28.8 3.20001 23.07 3.20001 16ZM11.275 11.695C10.665 11.435 9.96001 11.715 9.70001 12.325C9.44001 12.935 9.72001 13.64 10.33 13.9L10.925 14.155C11.79 14.525 12.685 14.8 13.605 14.97V17.475C13.605 17.69 13.57 17.905 13.5 18.105L12.065 22.41C11.855 23.04 12.195 23.72 12.825 23.93C13.455 24.14 14.135 23.8 14.345 23.17L15.565 19.51C15.63 19.32 15.805 19.19 16.005 19.19C16.205 19.19 16.385 19.32 16.445 19.51L17.665 23.17C17.875 23.8 18.555 24.14 19.185 23.93C19.815 23.72 20.15 23.05 19.94 22.42L18.505 18.115C18.435 17.91 18.4 17.7 18.4 17.485V14.98C19.32 14.805 20.215 14.535 21.08 14.165L21.675 13.91C22.285 13.65 22.565 12.945 22.305 12.335C22.045 11.725 21.34 11.445 20.73 11.705L20.135 11.95C18.83 12.51 17.425 12.8 16 12.8C14.575 12.8 13.175 12.51 11.865 11.95L11.27 11.695H11.275ZM16 11.2C17.105 11.2 18 10.305 18 9.20001C18 8.09501 17.105 7.20001 16 7.20001C14.895 7.20001 14 8.09501 14 9.20001C14 10.305 14.895 11.2 16 11.2Z" fill="#E6E6E6"/>
          </g>
          <defs>
            <clipPath id="clip0_2_92">
              <rect width="32" height="32" fill="white"/>
            </clipPath>
          </defs>
        </svg>
      `
    },
    {
      label: 'Other',
      svg: `
        <svg fill="white" viewBox="0 0 24 24" aria-hidden="true" width="24" height="24">
          <circle cx="6" cy="12" r="1.5"></circle>
          <circle cx="12" cy="12" r="1.5"></circle>
          <circle cx="18" cy="12" r="1.5"></circle>
        </svg>
      `
    }
  ];

  function handleImport(event: Event, format: "revo" | "obs") {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      importScenes(String(reader.result ?? ""), format);
      input.value = "";
    };
    reader.readAsText(file);
  }

  function handleImportObsProfile(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      importObsProfile(String(reader.result ?? ""));
      input.value = "";
    };
    reader.readAsText(file);
  }

  let newProfileName = "";

  onMount(() => {
    markCloseGuardBaseline();
  });
</script>
<div
  class="modal-backdrop"
  role="button"
  tabindex="0"
  onclick={close}
  onkeydown={(e) => (e.key === "Escape" ? close() : null)}
>
  <div
    class="modal"
    class:draggable-popup={allowDraggablePopups && !openAdditionalSettingsInWindows}
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
    <div class="settings-grid">
      <aside class="sidebar" role="navigation" aria-label="Settings sections">
        <div class="sidebar-top">
          <button class="icon gear">
            Settings
          </button>
        </div>
        <nav class="nav-list">
          {#each navItems as item}
            <button
              type="button"
              class="nav-item {item.label === active ? 'selected' : ''}"
              onclick={() => (active = item.label)}
              aria-pressed={item.label === active}
            >
              <span class="nav-icon">{@html item.svg}</span>
              <span class="nav-label">{item.label}</span>
            </button>
          {/each}
        </nav>
      </aside>

      <section class="content" aria-label="Settings content">
        <header class="content-header">
          <div class="title-row">
            <h2>{active}</h2>
          </div>
          <button class="icon" aria-label="Close settings" onclick={close}>✕</button>
        </header>

        <div class="content-body">
          {#if active === 'General'}
            <div class="field">
              <div class="field-label">Import / Export</div>
              <div class="import-export-grid">
                <div class="io-group">
                  <div class="io-title">RevoStream</div>
                  <div class="io-actions">
                    <button type="button" class="ghost" onclick={exportScenes}>Export scenes</button>
                    <label class="ghost file-label">
                      Import scenes
                      <input
                        type="file"
                        accept="application/json"
                        onchange={(e) => handleImport(e, "revo")}
                      />
                    </label>
                  </div>
                </div>

                <div class="io-group">
                  <div class="io-title">OBS Studio</div>
                  <div class="io-actions">
                    <button type="button" class="ghost" onclick={exportObsScenes}>Export as OBS scenes</button>
                    <label class="ghost file-label">
                      Import scenes
                      <input
                        type="file"
                        accept="application/json"
                        onchange={(e) => handleImport(e, "obs")}
                      />
                    </label>
                  </div>
                </div>
              </div>
            </div>
            <div class="field">
              <div class="field-label">Confirmation alerts</div>
              <div class="alert-settings">
                <label class="toggle-row" for="confirmStartStreaming">
                  <span>Show dialog when starting stream</span>
                  <span class="toggle">
                    <input id="confirmStartStreaming" type="checkbox" bind:checked={confirmStartStreaming} />
                    <span class="slider"></span>
                  </span>
                </label>

                <label class="toggle-row" for="confirmStopStreaming">
                  <span>Show dialog when stopping stream</span>
                  <span class="toggle">
                    <input id="confirmStopStreaming" type="checkbox" bind:checked={confirmStopStreaming} />
                    <span class="slider"></span>
                  </span>
                </label>

                <label class="toggle-row" for="confirmStartRecording">
                  <span>Show dialog when starting recording</span>
                  <span class="toggle">
                    <input id="confirmStartRecording" type="checkbox" bind:checked={confirmStartRecording} />
                    <span class="slider"></span>
                  </span>
                </label>

                <label class="toggle-row" for="confirmStopRecording">
                  <span>Show dialog when stopping recording</span>
                  <span class="toggle">
                    <input id="confirmStopRecording" type="checkbox" bind:checked={confirmStopRecording} />
                    <span class="slider"></span>
                  </span>
                </label>
              </div>
            </div>

            <div class="field">
              <div class="field-label">Other functions</div>
              <div class="alert-settings">
                <label class="toggle-row" for="autoRecordWhenStartStreaming">
                  <span>Automatically record when start streaming</span>
                  <span class="toggle">
                    <input
                      id="autoRecordWhenStartStreaming"
                      type="checkbox"
                      bind:checked={autoRecordWhenStartStreaming}
                    />
                    <span class="slider"></span>
                  </span>
                </label>

                <div class="experimental-row">
                  <label for="plannerUndoRedoLimit">Graphic planner undo/redo actions</label>
                  <input
                    id="plannerUndoRedoLimit"
                    type="number"
                    min="1"
                    max="50"
                    step="1"
                    bind:value={plannerUndoRedoLimit}
                  />
                </div>
              </div>
            </div>

            <div class="field">
              <div class="obs-status">
                {demoMode
                  ? "Waiting for Connect"
                  : isObsRunning
                    ? `Running OBS ${version}`
                    : "OBS stopped"}
              </div>
            </div>
          {:else if active === 'Output'}
            <div class="output-tabs-layout">
              <div class="output-tabs-nav" role="tablist" aria-label="Output tabs">
                <button type="button" class="output-tab" class:active={outputTab === "Streaming"} onclick={() => (outputTab = "Streaming")}>Streaming</button>
                <button type="button" class="output-tab" class:active={outputTab === "Recording"} onclick={() => (outputTab = "Recording")}>Recording</button>
                <button type="button" class="output-tab" class:active={outputTab === "Audio"} onclick={() => (outputTab = "Audio")}>Audio</button>
              </div>

              <div class="output-tab-card">
                {#if outputTab === "Streaming"}
                  <div class="section-block">
                    <div class="section-title">Basic settings</div>
                    <div class="field">
                      <label for="streamingAudioEncoder">Audio encoder</label>
                      <select id="streamingAudioEncoder" bind:value={streamingAudioEncoder}>
                        {#each streamingAudioEncoderOptions as option}
                          <option value={option.value}>{option.label}</option>
                        {/each}
                      </select>
                    </div>
                    <div class="field">
                      <label for="streamingVideoEncoder">Video encoder</label>
                      <select id="streamingVideoEncoder" bind:value={streamingVideoEncoder}>
                        {#each streamingVideoEncoderOptions as option}
                          <option value={option.value}>{option.label}</option>
                        {/each}
                      </select>
                    </div>
                  </div>

                  <div class="section-block">
                    <div class="section-title">Encoder Settings</div>
                    <div class="field">
                      <label for="streamingBitrate">Bitrate</label>
                      <input id="streamingBitrate" bind:value={streamingBitrate} placeholder="6000" />
                    </div>
                    <div class="field">
                      <label for="streamingKeyframeInterval">Keyframe Interval</label>
                      <input id="streamingKeyframeInterval" bind:value={streamingKeyframeInterval} placeholder="2" />
                    </div>
                    <div class="field">
                      <label for="streamingProfile">Profile</label>
                      <select id="streamingProfile" bind:value={streamingProfile}>
                        <option value="baseline">Baseline</option>
                        <option value="main">Main</option>
                        <option value="high">High</option>
                      </select>
                    </div>
                    <label class="toggle-row" for="streamingAdvanced">
                      <span>Advanced options</span>
                      <span class="toggle">
                        <input id="streamingAdvanced" type="checkbox" bind:checked={streamingAdvanced} />
                        <span class="slider"></span>
                      </span>
                    </label>

                    {#if streamingAdvanced}
                      <div class="field">
                        <label for="streamingRateControl">Rate Control</label>
                        <select id="streamingRateControl" bind:value={streamingRateControl}>
                          <option value="CBR">CBR</option>
                          <option value="VBR">VBR</option>
                          <option value="CQP">CQP</option>
                        </select>
                      </div>
                      <div class="field">
                        <label for="streamingCpuUsagePreset">CPU Usage Preset (high = low CPU)</label>
                        <select id="streamingCpuUsagePreset" bind:value={streamingCpuUsagePreset}>
                          <option value="ultrafast">ultrafast</option>
                          <option value="superfast">superfast</option>
                          <option value="veryfast">veryfast</option>
                          <option value="faster">faster</option>
                          <option value="fast">fast</option>
                          <option value="medium">medium</option>
                        </select>
                      </div>
                      <div class="field">
                        <label for="streamingTune">Tune</label>
                        <select id="streamingTune" bind:value={streamingTune}>
                          <option value="none">None</option>
                          <option value="zerolatency">zerolatency</option>
                          <option value="film">film</option>
                        </select>
                      </div>
                      <div class="field">
                        <label for="streamingExtraOptions">Extra Options</label>
                        <input id="streamingExtraOptions" bind:value={streamingExtraOptions} placeholder="scenecut=0" />
                      </div>
                    {/if}
                  </div>
                {:else if outputTab === "Recording"}
                  <div class="section-block">
                    <label class="toggle-row" for="recordFilenameNoSpaces">
                      <span>Record to filename without spaces</span>
                      <span class="toggle">
                        <input id="recordFilenameNoSpaces" type="checkbox" bind:checked={recordFilenameNoSpaces} />
                        <span class="slider"></span>
                      </span>
                    </label>
                    <div class="field">
                      <label for="record">Recording path</label>
                      <input id="record" placeholder="/home/user/Videos/revo.mp4" bind:value={recordPath} />
                    </div>
                    <div class="field">
                      <label for="recordingFormat">Recording format</label>
                      <select id="recordingFormat" bind:value={recordingFormat}>
                        {#each recordingFormatOptions as option}
                          <option value={option.value}>{option.label}</option>
                        {/each}
                      </select>
                    </div>
                    <label class="toggle-row" for="recordingAdvanced">
                      <span>Advanced</span>
                      <span class="toggle">
                        <input
                          id="recordingAdvanced"
                          type="checkbox"
                          checked={recordingAdvanced}
                          onchange={(e) => toggleRecordingAdvanced((e.currentTarget as HTMLInputElement).checked)}
                        />
                        <span class="slider"></span>
                      </span>
                    </label>
                    <label class="toggle-row" for="recordingCustomOutput">
                      <span>Custom Output</span>
                      <span class="toggle">
                        <input
                          id="recordingCustomOutput"
                          type="checkbox"
                          checked={recordingCustomOutput}
                          onchange={(e) => toggleRecordingCustomOutput((e.currentTarget as HTMLInputElement).checked)}
                        />
                        <span class="slider"></span>
                      </span>
                    </label>
                  </div>

                  {#if recordingAdvanced}
                    <div class="section-block">
                      <div class="section-title">Advanced</div>
                      <div class="field">
                        <label for="recordingVideoEncoder">Video Encoder</label>
                        <select id="recordingVideoEncoder" bind:value={recordingVideoEncoder}>
                          {#each recordingVideoEncoderOptions as option}
                            <option value={option.value}>{option.label}</option>
                          {/each}
                        </select>
                      </div>
                      <div class="field">
                        <label for="recordingAudioEncoder">Audio Encoder</label>
                        <select id="recordingAudioEncoder" bind:value={recordingAudioEncoder}>
                          {#each recordingAudioEncoderOptions as option}
                            <option value={option.value}>{option.label}</option>
                          {/each}
                        </select>
                      </div>
                      <div class="field">
                        <label for="recordingCustomMuxerSettings">Custom muxer Settings</label>
                        <input id="recordingCustomMuxerSettings" bind:value={recordingCustomMuxerSettings} />
                      </div>
                    </div>
                  {/if}

                  {#if recordingCustomOutput}
                    <div class="section-block">
                      <div class="section-title">Custom Output</div>
                      <div class="field">
                        <label for="recordingTypeOutput">Type Output</label>
                        <select id="recordingTypeOutput" bind:value={recordingTypeOutput}>
                          <option value="file">Output to file</option>
                          <option value="url">Output to URL</option>
                        </select>
                      </div>
                      <div class="field">
                        <label for="recordingPathCustom">Path</label>
                        <input id="recordingPathCustom" bind:value={recordingPathCustom} />
                      </div>
                      <div class="field">
                        <label for="recordingContainerFormat">Container format</label>
                        <input id="recordingContainerFormat" bind:value={recordingContainerFormat} />
                      </div>
                      <div class="field">
                        <label for="recordingMuxerSettings">Muxer settings</label>
                        <input id="recordingMuxerSettings" bind:value={recordingMuxerSettings} />
                      </div>
                      <div class="field">
                        <label for="recordingVideoBitrate">Video bitrate</label>
                        <input id="recordingVideoBitrate" bind:value={recordingVideoBitrate} />
                      </div>
                      <div class="field">
                        <label for="recordingVideoEncoderCustom">Video encoder</label>
                        <input id="recordingVideoEncoderCustom" bind:value={recordingVideoEncoderCustom} />
                      </div>
                      <div class="field">
                        <label for="recordingVideoEncoderSettings">Video encoder settings</label>
                        <input id="recordingVideoEncoderSettings" bind:value={recordingVideoEncoderSettings} />
                      </div>
                      <div class="field">
                        <label for="recordingAudioBitrate">Audio bitrate</label>
                        <input id="recordingAudioBitrate" bind:value={recordingAudioBitrate} />
                      </div>
                      <div class="field">
                        <label for="recordingAudioEncoderCustom">Audio encoder</label>
                        <input id="recordingAudioEncoderCustom" bind:value={recordingAudioEncoderCustom} />
                      </div>
                      <div class="field">
                        <label for="recordingAudioEncoderSettings">Audio encoder settings</label>
                        <input id="recordingAudioEncoderSettings" bind:value={recordingAudioEncoderSettings} />
                      </div>
                    </div>
                  {/if}
                {:else}
                  <div class="section-block">
                    <div class="section-title">Channel 1</div>
                    <div class="field">
                      <label for="audioChannel1Bitrate">Audio bitrate</label>
                      <select id="audioChannel1Bitrate" bind:value={audioChannel1Bitrate}>
                        {#each audioBitrateOptions as bitrate}
                          <option value={String(bitrate)}>{bitrate}</option>
                        {/each}
                      </select>
                    </div>
                    <div class="field">
                      <label for="audioChannel1Name">Name</label>
                      <input id="audioChannel1Name" bind:value={audioChannel1Name} />
                    </div>
                  </div>

                  <div class="section-block">
                    <div class="section-title">Channel 2</div>
                    <div class="field">
                      <label for="audioChannel2Bitrate">Audio bitrate</label>
                      <select id="audioChannel2Bitrate" bind:value={audioChannel2Bitrate}>
                        {#each audioBitrateOptions as bitrate}
                          <option value={String(bitrate)}>{bitrate}</option>
                        {/each}
                      </select>
                    </div>
                    <div class="field">
                      <label for="audioChannel2Name">Name</label>
                      <input id="audioChannel2Name" bind:value={audioChannel2Name} />
                    </div>
                  </div>

                  <div class="section-block">
                    <div class="section-title">Channel 3</div>
                    <div class="field">
                      <label for="audioChannel3Bitrate">Audio bitrate</label>
                      <select id="audioChannel3Bitrate" bind:value={audioChannel3Bitrate}>
                        {#each audioBitrateOptions as bitrate}
                          <option value={String(bitrate)}>{bitrate}</option>
                        {/each}
                      </select>
                    </div>
                    <div class="field">
                      <label for="audioChannel3Name">Name</label>
                      <input id="audioChannel3Name" bind:value={audioChannel3Name} />
                    </div>
                  </div>

                  <div class="section-block">
                    <div class="section-title">Channel 4</div>
                    <div class="field">
                      <label for="audioChannel4Bitrate">Audio bitrate</label>
                      <select id="audioChannel4Bitrate" bind:value={audioChannel4Bitrate}>
                        {#each audioBitrateOptions as bitrate}
                          <option value={String(bitrate)}>{bitrate}</option>
                        {/each}
                      </select>
                    </div>
                    <div class="field">
                      <label for="audioChannel4Name">Name</label>
                      <input id="audioChannel4Name" bind:value={audioChannel4Name} />
                    </div>
                  </div>

                  <div class="section-block">
                    <div class="section-title">Channel 5</div>
                    <div class="field">
                      <label for="audioChannel5Bitrate">Audio bitrate</label>
                      <select id="audioChannel5Bitrate" bind:value={audioChannel5Bitrate}>
                        {#each audioBitrateOptions as bitrate}
                          <option value={String(bitrate)}>{bitrate}</option>
                        {/each}
                      </select>
                    </div>
                    <div class="field">
                      <label for="audioChannel5Name">Name</label>
                      <input id="audioChannel5Name" bind:value={audioChannel5Name} />
                    </div>
                  </div>
                {/if}
              </div>
            </div>
          {:else if active === 'Broadcast'}
            <div class="field">
              <h4>Autocomplete services</h4>
              <div class="service-list">
                {#each streamServices as service}
                  <button
                    type="button"
                    class="service-btn"
                    class:active={isPresetServiceActive(service.id, service.url)}
                    onclick={() => selectStreamService(service.url)}
                  >
                    <span class="service-logo">{@html service.svg}</span>
                    <span>{service.name}</span>
                  </button>
                {/each}
                <button
                  type="button"
                  class="service-btn other-btn"
                  class:active={showOtherServices || (selectedPresetServiceId === '' && selectedLocalServerUrl !== '' && streamUrl.trim() === selectedLocalServerUrl)}
                  onclick={toggleOtherServices}
                >
                  <span class="service-logo">
                    <svg viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M12 5a1.75 1.75 0 1 0 0 3.5A1.75 1.75 0 0 0 12 5Zm0 5.25A1.75 1.75 0 1 0 12 13.75a1.75 1.75 0 0 0 0-3.5ZM10.25 17a1.75 1.75 0 1 1 3.5 0 1.75 1.75 0 0 1-3.5 0Z"/></svg>
                  </span>
                  <span>Other</span>
                </button>
              </div>

              {#if showOtherServices}
                <div class="other-services-panel">
                  {#if loadingOtherServices}
                    <div class="services-meta">Loading local services...</div>
                  {:else if otherServicesError}
                    <div class="services-meta error">{otherServicesError}</div>
                  {:else if !localServices.length}
                    <div class="services-meta">No services found from services.json</div>
                  {:else}
                    <div class="other-services-grid">
                      <label class="service-select-group">
                        <span>Service name</span>
                        <select
                          bind:value={selectedLocalService}
                          onchange={(e) => pickLocalService((e.currentTarget as HTMLSelectElement).value)}
                        >
                          <option value="" disabled selected={selectedLocalService === ""}>Choose service</option>
                          {#each localServices as service}
                            <option value={service.name}>{service.name}</option>
                          {/each}
                        </select>
                      </label>

                      {#if selectedLocalService && selectedLocalServers.length > 0}
                        <label class="service-select-group">
                          <span>Service address</span>
                          <select
                            value={selectedLocalServerUrl}
                            onchange={(e) => pickLocalServer((e.currentTarget as HTMLSelectElement).value)}
                          >
                            {#each selectedLocalServers as server}
                              <option value={server.url}>{server.name} — {server.url}</option>
                            {/each}
                          </select>
                        </label>
                      {/if}
                    </div>
                  {/if}
                </div>
              {/if}
            </div>
            <div class="field">
              <label for="stream">URL</label>
              <input id="stream" placeholder="rtmp://server/live" bind:value={streamUrl} />
              <label for="streamKey">Key</label>
              <input id="streamKey" placeholder="key" bind:value={streamKey} />
              <span>Supports protocol rtmp, rtmps, srt, rist, whip, webrtc</span>
            </div>
          {:else if active === 'Audio'}
            <div class="section-block">
              <div class="section-title">Basic</div>
              <div class="field">
                <label for="audioSampleRate">Sample Rate</label>
                <select id="audioSampleRate" bind:value={audioSampleRate}>
                  {#each audioSampleRateOptions as option}
                    <option value={option.value}>{option.label}</option>
                  {/each}
                </select>
              </div>
              <div class="field">
                <label for="audioChannels">Channels</label>
                <select id="audioChannels" bind:value={audioChannels}>
                  {#each audioChannelsOptions as option}
                    <option value={option.value}>{option.label}</option>
                  {/each}
                </select>
              </div>
            </div>

            <div class="section-block">
              <div class="section-title">Global Audio Devices</div>
              <div class="field two-col">
                <div>
                  <label for="audioDesktopDevice">Desktop Audio</label>
                  <select id="audioDesktopDevice" bind:value={audioDesktopDevice}>
                    {#each audioDesktopDeviceOptions as option}
                      <option value={option.value}>{option.label}</option>
                    {/each}
                  </select>
                </div>
                <div>
                  <label for="audioMicDevice">Mic Audio</label>
                  <select id="audioMicDevice" bind:value={audioMicDevice}>
                    {#each audioMicDeviceOptions as option}
                      <option value={option.value}>{option.label}</option>
                    {/each}
                  </select>
                </div>
              </div>
            </div>

            <div class="section-block">
              <div class="section-title">Monitoring device</div>
              <div class="field">
                <label for="audioMonitoringDevice">Monitor device</label>
                <select id="audioMonitoringDevice" bind:value={audioMonitoringDevice}>
                  {#each audioMonitoringDeviceOptions as option}
                    <option value={option.value}>{option.label}</option>
                  {/each}
                </select>
              </div>
              <label class="toggle-row" for="audioMonitoringLowLatency">
                <span>Low latency mode (for NDI outputs)</span>
                <span class="toggle">
                  <input id="audioMonitoringLowLatency" type="checkbox" bind:checked={audioMonitoringLowLatency} />
                  <span class="slider"></span>
                </span>
              </label>
            </div>
          {:else if active === 'Video'}
            <div class="section-block">
              <div class="section-title">Basic</div>
              <div class="field">
                <label for="videoCanvasResolution">Base (Canvas) Resolution</label>
                <select id="videoCanvasResolution" bind:value={videoCanvasResolution}>
                  {#each videoResolutionOptions as option}
                    <option value={option.value}>{option.label}</option>
                  {/each}
                </select>
              </div>
              <div class="field">
                <label for="videoOutputResolution">Output (Scaled) Resolution</label>
                <select id="videoOutputResolution" bind:value={videoOutputResolution}>
                  {#each videoResolutionOptions as option}
                    <option value={option.value}>{option.label}</option>
                  {/each}
                </select>
              </div>
              <div class="field">
                <label for="videoDownscaleFilter">Downscale Filter</label>
                <select id="videoDownscaleFilter" bind:value={videoDownscaleFilter}>
                  {#each videoDownscaleFilterOptions as option}
                    <option value={option.value}>{option.label}</option>
                  {/each}
                </select>
              </div>
            </div>

            <div class="section-block">
              <div class="section-title">FPS</div>
              <div class="field">
                <label for="videoFpsType">FPS Type</label>
                <select id="videoFpsType" bind:value={videoFpsType}>
                  <option value="common">Common FPS Values</option>
                  <option value="integer">Integer FPS Value</option>
                  <option value="fractional">Fractional FPS Value</option>
                </select>
              </div>

              {#if videoFpsType === "common"}
                <div class="field">
                  <label for="videoFpsCommon">Common FPS Values</label>
                  <select id="videoFpsCommon" bind:value={videoFpsCommon} disabled={videoFpsCommonCustom}>
                    {#each videoCommonFpsOptions as option}
                      <option value={option.value}>{option.label}</option>
                    {/each}
                  </select>
                </div>
                <label class="checkbox" for="videoFpsCommonCustom">
                  <input id="videoFpsCommonCustom" type="checkbox" bind:checked={videoFpsCommonCustom} />
                  Use custom common FPS
                </label>
                {#if videoFpsCommonCustom}
                  <div class="field">
                    <label for="videoFpsCommonCustomValue">Custom FPS value</label>
                    <input id="videoFpsCommonCustomValue" type="number" min="1" step="1" bind:value={videoFpsCommonCustomValue} />
                  </div>
                {/if}
              {:else if videoFpsType === "integer"}
                <div class="field">
                  <label for="videoFpsIntegerValue">Integer FPS value</label>
                  <input id="videoFpsIntegerValue" type="number" min="1" step="1" bind:value={videoFpsIntegerValue} />
                </div>
              {:else if videoFpsType === "fractional"}
                <div class="field two-col">
                  <div>
                    <label for="videoFpsFractionalNumerator">Numerator</label>
                    <input id="videoFpsFractionalNumerator" type="number" min="1" step="1" bind:value={videoFpsFractionalNumerator} />
                  </div>
                  <div>
                    <label for="videoFpsFractionalDenominator">Denominator</label>
                    <input id="videoFpsFractionalDenominator" type="number" min="1" step="1" bind:value={videoFpsFractionalDenominator} />
                  </div>
                </div>
              {/if}
            </div>
          {:else if active === 'Look'}
            <div class="section-block">
              <div class="section-title">Theme selection</div>
              <div class="field">
                <input
                  bind:this={importThemeInputEl}
                  class="theme-import-input"
                  type="file"
                  accept=".revotheme"
                  onchange={importThemeFromPicker}
                />

                <div class="theme-gallery">
                  <button
                    type="button"
                    class="theme-card"
                    class:selected={selectedThemeId === ""}
                    onclick={() => (selectedThemeId = "")}
                    aria-label="Select default built-in theme"
                  >
                    <div class="theme-card-preview">DEFAULT</div>
                    <div class="theme-card-body">
                      <div class="theme-card-name">Default (built-in)</div>
                      <div class="theme-card-meta">RevoStream default UI theme</div>
                    </div>
                  </button>

                  {#each availableThemes as theme}
                    <button
                      type="button"
                      class="theme-card"
                      class:selected={selectedThemeId === theme.id}
                      onclick={() => (selectedThemeId = theme.id)}
                      aria-label={`Select theme ${theme.name}`}
                    >
                      <div class="theme-card-preview">{theme.id} - {theme.version}</div>
                      <div class="theme-card-body">
                        <div class="theme-card-name">{theme.name}</div>
                        <div class="theme-card-meta">👤 {theme.author}</div>
                      </div>
                    </button>
                  {/each}

                  <button
                    type="button"
                    class="theme-card theme-card-import"
                    onclick={openThemeImportPicker}
                    aria-label="Import theme"
                  >
                    <div class="theme-card-preview import">+</div>
                    <div class="theme-card-body">
                      <div class="theme-card-name">Import theme</div>
                      <div class="theme-card-meta">Select a .revotheme file</div>
                    </div>
                  </button>
                </div>

                {#if themesLoading}
                  <div class="services-meta">Loading themes from data/themes…</div>
                {:else if themesError}
                  <div class="services-meta error">{themesError}</div>
                {:else if themesInfo}
                  <div class="services-meta">{themesInfo}</div>
                {:else}
                  {@const selectedTheme = availableThemes.find((theme) => theme.id === selectedThemeId)}
                  {#if selectedTheme}
                    <div class="services-meta">Theme: <strong>{selectedTheme.name}</strong> · Author: {selectedTheme.author} · Version: {selectedTheme.version}</div>
                  {:else}
                    <div class="services-meta">Using built-in default theme.</div>
                  {/if}
                {/if}
              </div>
            </div>

            <div class="section-block">
              <div class="section-title">Additional tools</div>
              <div class="field">
                <button type="button" class="ghost" onclick={extractDefaultTheme}>Extract default theme</button>
                <div class="services-meta">Creates data/themes/default_local from the currently active theme (current CSS variables), with theme.css and config.revo.</div>
              </div>
            </div>
          {:else if active === 'Shortcuts'}
            <div class="section-block">
              <div class="section-title">Streaming and recording</div>
              <div class="field two-col">
                <div>
                  <label for="shortcutStartStreaming">Start streaming</label>
                  <div class="shortcut-input-wrap">
                    <input id="shortcutStartStreaming" placeholder="e.g. Ctrl+Shift+S" bind:value={shortcutStartStreaming} />
                    <button type="button" class="shortcut-capture-btn" aria-label="Capture start streaming shortcut" onclick={() => openShortcutCapture("shortcutStartStreaming", "Start streaming")}>⌨</button>
                  </div>
                </div>
                <div>
                  <label for="shortcutStopStreaming">Stop streaming</label>
                  <div class="shortcut-input-wrap">
                    <input id="shortcutStopStreaming" placeholder="e.g. Ctrl+Shift+X" bind:value={shortcutStopStreaming} />
                    <button type="button" class="shortcut-capture-btn" aria-label="Capture stop streaming shortcut" onclick={() => openShortcutCapture("shortcutStopStreaming", "Stop streaming")}>⌨</button>
                  </div>
                </div>
              </div>
              <div class="field two-col">
                <div>
                  <label for="shortcutStartRecording">Start recording</label>
                  <div class="shortcut-input-wrap">
                    <input id="shortcutStartRecording" placeholder="e.g. Ctrl+Shift+R" bind:value={shortcutStartRecording} />
                    <button type="button" class="shortcut-capture-btn" aria-label="Capture start recording shortcut" onclick={() => openShortcutCapture("shortcutStartRecording", "Start recording")}>⌨</button>
                  </div>
                </div>
                <div>
                  <label for="shortcutStopRecording">Stop recording</label>
                  <div class="shortcut-input-wrap">
                    <input id="shortcutStopRecording" placeholder="e.g. Ctrl+Shift+T" bind:value={shortcutStopRecording} />
                    <button type="button" class="shortcut-capture-btn" aria-label="Capture stop recording shortcut" onclick={() => openShortcutCapture("shortcutStopRecording", "Stop recording")}>⌨</button>
                  </div>
                </div>
              </div>
              <div class="field two-col">
                <div>
                  <label for="shortcutPauseRecording">Pause recording</label>
                  <div class="shortcut-input-wrap">
                    <input id="shortcutPauseRecording" placeholder="Optional" bind:value={shortcutPauseRecording} />
                    <button type="button" class="shortcut-capture-btn" aria-label="Capture pause recording shortcut" onclick={() => openShortcutCapture("shortcutPauseRecording", "Pause recording")}>⌨</button>
                  </div>
                </div>
                <div>
                  <label for="shortcutResumeRecording">Resume recording</label>
                  <div class="shortcut-input-wrap">
                    <input id="shortcutResumeRecording" placeholder="Optional" bind:value={shortcutResumeRecording} />
                    <button type="button" class="shortcut-capture-btn" aria-label="Capture resume recording shortcut" onclick={() => openShortcutCapture("shortcutResumeRecording", "Resume recording")}>⌨</button>
                  </div>
                </div>
              </div>
            </div>

            <div class="section-block">
              <div class="section-title">Sources</div>
              <div class="field two-col">
                <div>
                  <label for="shortcutShowAllSources">Show all sources</label>
                  <div class="shortcut-input-wrap">
                    <input id="shortcutShowAllSources" placeholder="Optional" bind:value={shortcutShowAllSources} />
                    <button type="button" class="shortcut-capture-btn" aria-label="Capture show all sources shortcut" onclick={() => openShortcutCapture("shortcutShowAllSources", "Show all sources")}>⌨</button>
                  </div>
                </div>
                <div>
                  <label for="shortcutHideAllSources">Hide all sources</label>
                  <div class="shortcut-input-wrap">
                    <input id="shortcutHideAllSources" placeholder="Optional" bind:value={shortcutHideAllSources} />
                    <button type="button" class="shortcut-capture-btn" aria-label="Capture hide all sources shortcut" onclick={() => openShortcutCapture("shortcutHideAllSources", "Hide all sources")}>⌨</button>
                  </div>
                </div>
              </div>
              <div class="field two-col">
                <div>
                  <label for="shortcutShowSourceTransform">Show source transform window</label>
                  <div class="shortcut-input-wrap">
                    <input id="shortcutShowSourceTransform" placeholder="Optional" bind:value={shortcutShowSourceTransform} />
                    <button type="button" class="shortcut-capture-btn" aria-label="Capture show source transform shortcut" onclick={() => openShortcutCapture("shortcutShowSourceTransform", "Show source transform")}>⌨</button>
                  </div>
                </div>
                <div>
                  <label for="shortcutHideSourceTransform">Hide source transform window</label>
                  <div class="shortcut-input-wrap">
                    <input id="shortcutHideSourceTransform" placeholder="Optional" bind:value={shortcutHideSourceTransform} />
                    <button type="button" class="shortcut-capture-btn" aria-label="Capture hide source transform shortcut" onclick={() => openShortcutCapture("shortcutHideSourceTransform", "Hide source transform")}>⌨</button>
                  </div>
                </div>
              </div>
            </div>

            <div class="section-block">
              <div class="section-title">Scenes</div>
              <div class="field two-col">
                <div>
                  <label for="shortcutNextScene">Next scene</label>
                  <div class="shortcut-input-wrap">
                    <input id="shortcutNextScene" placeholder="Optional" bind:value={shortcutNextScene} />
                    <button type="button" class="shortcut-capture-btn" aria-label="Capture next scene shortcut" onclick={() => openShortcutCapture("shortcutNextScene", "Next scene")}>⌨</button>
                  </div>
                </div>
                <div>
                  <label for="shortcutPreviousScene">Previous scene</label>
                  <div class="shortcut-input-wrap">
                    <input id="shortcutPreviousScene" placeholder="Optional" bind:value={shortcutPreviousScene} />
                    <button type="button" class="shortcut-capture-btn" aria-label="Capture previous scene shortcut" onclick={() => openShortcutCapture("shortcutPreviousScene", "Previous scene")}>⌨</button>
                  </div>
                </div>
              </div>
              <div class="field two-col">
                <div>
                  <label for="shortcutShowSceneItems">Show all scene items</label>
                  <div class="shortcut-input-wrap">
                    <input id="shortcutShowSceneItems" placeholder="Optional" bind:value={shortcutShowSceneItems} />
                    <button type="button" class="shortcut-capture-btn" aria-label="Capture show scene items shortcut" onclick={() => openShortcutCapture("shortcutShowSceneItems", "Show scene items")}>⌨</button>
                  </div>
                </div>
                <div>
                  <label for="shortcutHideSceneItems">Hide all scene items</label>
                  <div class="shortcut-input-wrap">
                    <input id="shortcutHideSceneItems" placeholder="Optional" bind:value={shortcutHideSceneItems} />
                    <button type="button" class="shortcut-capture-btn" aria-label="Capture hide scene items shortcut" onclick={() => openShortcutCapture("shortcutHideSceneItems", "Hide scene items")}>⌨</button>
                  </div>
                </div>
              </div>
            </div>
          {:else if active === 'Accessibility'}
            <div class="section-block">
              <div class="section-title">Visual accessibility</div>
              <div class="alert-settings">
                <label class="toggle-row" for="accessibilityHighContrast">
                  <span>High contrast mode (WCAG support)</span>
                  <span class="toggle">
                    <input id="accessibilityHighContrast" type="checkbox" bind:checked={accessibilityHighContrast} />
                    <span class="slider"></span>
                  </span>
                </label>
                <label class="toggle-row" for="accessibilityReduceMotion">
                  <span>Reduce motion and transitions</span>
                  <span class="toggle">
                    <input id="accessibilityReduceMotion" type="checkbox" bind:checked={accessibilityReduceMotion} />
                    <span class="slider"></span>
                  </span>
                </label>
                <label class="toggle-row" for="accessibilityFocusIndicators">
                  <span>Stronger keyboard focus indicators</span>
                  <span class="toggle">
                    <input id="accessibilityFocusIndicators" type="checkbox" bind:checked={accessibilityFocusIndicators} />
                    <span class="slider"></span>
                  </span>
                </label>
              </div>
            </div>

            <div class="section-block">
              <div class="section-title">Text and interface size</div>
              <div class="field two-col">
                <div>
                  <label for="accessibilityUiScale">UI scale</label>
                  <select id="accessibilityUiScale" bind:value={accessibilityUiScale}>
                    <option value="90">90%</option>
                    <option value="100">100%</option>
                    <option value="110">110%</option>
                    <option value="125">125%</option>
                    <option value="150">150%</option>
                  </select>
                </div>
                <div>
                  <label for="accessibilityFontSize">Font size</label>
                  <select id="accessibilityFontSize" bind:value={accessibilityFontSize}>
                    <option value="90">Small (90%)</option>
                    <option value="100">Default (100%)</option>
                    <option value="110">Large (110%)</option>
                    <option value="125">Extra large (125%)</option>
                  </select>
                </div>
              </div>
              <div class="field">
                <label for="accessibilityFontFamily">Font family</label>
                <select id="accessibilityFontFamily" bind:value={accessibilityFontFamily}>
                  <option value="system">Default</option>
                  <option value="sans">Sans-serif</option>
                  <option value="serif">Serif</option>
                  <option value="mono">Monospace</option>
                </select>
              </div>
              <div class="field">
                <label for="accessibilityColorVision">Color vision mode</label>
                <select id="accessibilityColorVision" bind:value={accessibilityColorVision}>
                  <option value="none">None</option>
                  <option value="protanopia">Protanopia support</option>
                  <option value="deuteranopia">Deuteranopia support</option>
                  <option value="tritanopia">Tritanopia support</option>
                </select>
              </div>
            </div>
          {:else if active === 'Other'}
          <div class="field">
              <div class="field-label">Language</div>
              <div class="import-export-grid">
                <div class="io-group">
                  <div class="io-title">Select language</div>
                  <select id="uiLanguage" bind:value={uiLanguage}>
                    {#each languageOptions as languageOption}
                      <option value={languageOption.value}>{languageOption.label}</option>
                    {/each}
                  </select>
                </div>
              </div>
            </div>

            <div class="field">
              <div class="field-label">Profile Import / Export</div>
              <div class="import-export-grid">
                <div class="io-group">
                  <div class="io-title">OBS Studio profile</div>
                  <div class="io-actions">
                    <button type="button" class="ghost" onclick={exportObsProfile}>Export profile</button>
                    <label class="ghost file-label">
                      Import profile
                      <input
                        type="file"
                        accept="application/json"
                        onchange={handleImportObsProfile}
                      />
                    </label>
                  </div>
                </div>
              </div>
            </div>

            <div class="field">
              <div class="field-label">Profiles</div>
              <div class="io-group">
                <div class="field two-col">
                  <div>
                    <label for="selectedProfileName">Active profile</label>
                    <select
                      id="selectedProfileName"
                      bind:value={selectedProfileName}
                      onchange={(e) => switchProfile((e.currentTarget as HTMLSelectElement).value)}
                    >
                      {#each profileOptions as profileName}
                        <option value={profileName}>{profileName}</option>
                      {/each}
                    </select>
                  </div>
                  <div>
                    <label for="newProfileName">Create new profile</label>
                    <div class="shortcut-input-wrap">
                      <input
                        id="newProfileName"
                        placeholder="profile name"
                        bind:value={newProfileName}
                      />
                      <button
                        type="button"
                        class="ghost"
                        onclick={() => {
                          const name = newProfileName.trim();
                          if (!name) return;
                          createProfile(name);
                          newProfileName = "";
                        }}
                      >
                        Create
                      </button>
                    </div>
                  </div>
                </div>
                <div class="services-meta">Profiles are stored in data/profiles and linked to settings.</div>
              </div>
            </div>

            <div class="field">
              <label for="root">Project root (optional)</label>
              <input id="root" placeholder="/home/user/Projects/revo-rust" bind:value={rootDir} />
            </div>
            <div class="field">
              <label for="previewQuality">Preview quality</label>
              <select id="previewQuality" bind:value={previewQuality}>
                {#each previewQualityOptions as option}
                  <option value={option.value}>{option.label}</option>
                {/each}
              </select>
            </div>
            <div class="field">
              <label for="encoderPreference">Video encoder</label>
              <select id="encoderPreference" bind:value={encoderPreference}>
                {#each encoderPreferenceOptions as option}
                  <option value={option.value}>{option.label}</option>
                {/each}
              </select>
            </div>
            <div class="field">
              <label for="sceneResolution">Scene resolution</label>
              <select id="sceneResolution" bind:value={sceneResolution}>
                {#each sceneResolutionOptions as option}
                  <option value={option.value}>{option.label}</option>
                {/each}
              </select>
            </div>
            <div class="field">
              <label class="toggle-row">
                <span>DEMO mode</span>
                <span class="toggle">
                  <input
                    type="checkbox"
                    checked={demoMode}
                    onchange={(e) => toggleDemo((e.currentTarget as HTMLInputElement).checked)}
                  />
                  <span class="slider"></span>
                </span>
              </label>
            </div>

            <div class="field">
              <label class="toggle-row">
                <span>Autorescale inputs on change scene resolution</span>
                <span class="toggle">
                  <input
                    type="checkbox"
                    checked={autorescaleInputs}
                    onchange={(e) => toggleAutorescaleInputs((e.currentTarget as HTMLInputElement).checked)}
                  />
                  <span class="slider"></span>
                </span>
              </label>
            </div>
            <div class="field">
              <label for="whepUrl">WHEP URL (preview pull)</label>
              <input id="whepUrl" placeholder="http://127.0.0.1:8080/whep" bind:value={whepUrl} />
            </div>
            <div class="field">
              <label for="whipUrl">WHIP URL (preview push)</label>
              <input id="whipUrl" placeholder="http://127.0.0.1:8080/whip" bind:value={whipUrl} />
            </div>
            <div class="field">
              <label class="checkbox">
                <input type="checkbox" bind:checked={autoRetryPreview} />
                Auto-retry preview
              </label>
            </div>
            <div class="field">
              <div class="field-label">Experimental features</div>
              <div class="alert-settings">
                <label class="toggle-row" for="openAdditionalSettingsInWindows">
                  <span>Open additional settings on windows instead popup</span>
                  <span class="toggle">
                    <input
                      id="openAdditionalSettingsInWindows"
                      type="checkbox"
                      bind:checked={openAdditionalSettingsInWindows}
                    />
                    <span class="slider"></span>
                  </span>
                </label>

                <label class="toggle-row" for="allowDraggablePopups">
                  <span class:toggle-label-disabled={openAdditionalSettingsInWindows}>Allow dragable popups</span>
                  <span class="toggle">
                    <input
                      id="allowDraggablePopups"
                      type="checkbox"
                      bind:checked={allowDraggablePopups}
                      disabled={openAdditionalSettingsInWindows}
                    />
                    <span class="slider"></span>
                  </span>
                </label>

                <div class="experimental-row">
                  <label for="quickColorHistoryLimit">Quick color history limit</label>
                  <input
                    id="quickColorHistoryLimit"
                    type="number"
                    min="1"
                    max="15"
                    step="1"
                    bind:value={quickColorHistoryLimit}
                  />
                </div>

                <label class="toggle-row" for="realtimeRefreshSources">
                  <span>Realtime refresh sources while editing</span>
                  <span class="toggle">
                    <input
                      id="realtimeRefreshSources"
                      type="checkbox"
                      bind:checked={realtimeRefreshSources}
                    />
                    <span class="slider"></span>
                  </span>
                </label>
              </div>
            </div>
          {:else}
            <div class="placeholder">
              <p style="color:var(--text-muted); margin-top: 8px;">(Not implemented yet)</p>
            </div>
          {/if}
        </div>

        {#if showShortcutCapture}
          <div
            class="shortcut-capture-backdrop"
            role="button"
            tabindex="0"
            onclick={closeShortcutCapture}
            onkeydown={(e) => (e.key === "Escape" ? closeShortcutCapture() : null)}
          >
            <div
              class="shortcut-capture-dialog"
              role="dialog"
              aria-modal="true"
              tabindex="0"
              onclick={(e) => e.stopPropagation()}
              onkeydown={(e) => e.stopPropagation()}
            >
              <h4>Capture shortcut</h4>
              <p>{shortcutCaptureLabel}</p>
              <div class="shortcut-capture-preview">{shortcutCapturePreview || "Press key combination now..."}</div>
              <div class="shortcut-capture-help">Press Backspace/Delete to clear. Press Esc to cancel.</div>
              <div class="shortcut-capture-actions">
                <button type="button" class="ghost" onclick={closeShortcutCapture}>Cancel</button>
              </div>
            </div>
          </div>
        {/if}

        <footer class="modal-actions content-actions">
          <button class="ghost" onclick={close}>Close</button>
          <button class="primary" onclick={save} disabled={busy}>Save</button>
        </footer>
      </section>
    </div>
  </div>
</div>

{#if showUnsavedCloseDialog}
  <div
    class="unsaved-backdrop"
    role="button"
    tabindex="0"
    onclick={() => (showUnsavedCloseDialog = false)}
    onkeydown={(e) => e.stopPropagation()}
  >
    <div
      class="unsaved-dialog"
      role="dialog"
      aria-modal="true"
      aria-label="Unsaved settings"
      tabindex="0"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => {
        e.stopPropagation();
        handleUnsavedDialogKeydown(e);
      }}
    >
      <h4>Unsaved changes</h4>
      <p>Do you want to close settings without saving?</p>
      <div class="unsaved-actions">
        <button bind:this={unsavedNoBtnEl} class="ghost" type="button" onclick={() => (showUnsavedCloseDialog = false)}>No</button>
        <button bind:this={unsavedYesBtnEl} class="primary" type="button" onclick={finishClose}>Yes</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    display: grid;
    place-items: center;
    z-index: 9999;
    padding: 1.25rem;
  }

  .modal {
    width: min(920px, 96vw);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: 0 20px 40px rgba(0, 0, 0, 0.45);
    display: block;
    padding: 0;
    overflow: hidden;
  }

  .modal.draggable-popup {
    transform: translate(calc(var(--popup-dx, 0px)), calc(var(--popup-dy, 0px)));
  }

  .settings-grid {
    display: grid;
    grid-template-columns: 200px 1fr;
    min-height: 480px;
  }

  .sidebar {
    background: var(--surface-3);
    border-right: 1px solid var(--border);
    padding: 16px;
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 12px;
  }

  .sidebar-top {
    display: flex;
    justify-content: center;
    padding-bottom: 4px;
  }

  .icon.gear {
    border-radius: 10px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    display: inline-grid;
    place-items: center;
    color: var(--text);
  }

  .nav-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: 4px;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px;
    background: transparent;
    border: none;
    color: var(--text);
    border-radius: 10px;
    text-align: left;
    cursor: pointer;
    width: 100%;
  }

  .nav-item:hover {
    background: var(--surface-2);
  }

  .nav-item.selected {
    background: var(--surface);
    box-shadow: inset 0 0 0 1px var(--border);
    font-weight: 700;
  }

  .nav-icon {
    width: 36px;
    height: 36px;
    display: inline-grid;
    place-items: center;
    background: var(--surface-2);
    border-radius: 10px;
    color: var(--text);
  }

  :global(.nav-icon svg) {
    width: 14px;
    height: 14px;
    fill: var(--icon-color, var(--text));
    stroke: none;
  }

  :global(.nav-icon svg *) {
    fill: var(--icon-color, var(--text));
    stroke: none;
  }

  .nav-label {
    flex: 1;
  }

  .content {
    padding: 18px 20px;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto;
    row-gap: 12px;
    min-height: 0;
    position: relative;
  }

  .content-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-bottom: 8px;
    border-bottom: 1px solid var(--border);
  }

  .content-header h2 {
    margin: 0;
    font-size: 1.15rem;
  }

  .title-row {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    min-width: 0;
  }

  .toggle-label-disabled {
    color: var(--text-muted);
    opacity: 0.7;
  }

  .content-body {
    padding-top: 12px;
    padding-bottom: 20px;
    overflow: auto;
    height: 530px;
    min-height: 0;
    max-height: 530px;
    display: grid;
    align-content: start;
    grid-auto-rows: max-content;
    gap: 12px;
  }

  .modal .field {
    display: grid;
    gap: 0.5rem;
  }

  .modal-actions.content-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.75rem;
    padding-top: 10px;
    margin-top: auto;
    border-top: 1px solid var(--border);
  }

  .checkbox {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    color: var(--text);
  }

  .toggle-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    font-weight: 600;
  }

  .toggle {
    position: relative;
    display: inline-flex;
    align-items: center;
  }

  .toggle input {
    position: absolute;
    opacity: 0;
    width: 0;
    height: 0;
  }

  .toggle .slider {
    width: 42px;
    height: 24px;
    background: var(--surface-3);
    border-radius: 999px;
    position: relative;
    transition: background 0.2s ease;
  }

  .toggle .slider::after {
    content: "";
    position: absolute;
    top: 3px;
    left: 3px;
    width: 18px;
    height: 18px;
    background: #ffffff;
    border-radius: 50%;
    transition: transform 0.2s ease;
  }

  .toggle input:checked + .slider {
    background: var(--accent);
  }

  .toggle input:checked + .slider::after {
    transform: translateX(18px);
  }

  input,
  select {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.65rem 0.85rem;
    color: var(--text);
  }

  input::placeholder {
    color: var(--text-muted);
  }

  .obs-status {
    color: var(--text-muted);
    font-size: 0.95rem;
  }

  button {
    background-color: var(--border);
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

  .import-export-grid {
    display: grid;
    gap: 0.75rem;
  }

  .output-tabs-layout {
    display: grid;
    grid-template-columns: 1fr;
    gap: 0.75rem;
    align-items: start;
  }

  .output-tabs-nav {
    display: flex;
    flex-wrap: wrap;
    gap: 0.45rem;
  }

  .output-tab {
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface);
    color: var(--text);
    text-align: center;
    padding: 0.6rem 0.75rem;
    min-width: 120px;
  }

  .output-tab.active {
    border-color: var(--accent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 45%, transparent);
  }

  .output-tab-card {
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface);
    padding: 0.75rem;
    display: grid;
    gap: 0.75rem;
  }

  .section-block {
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
    padding: 0.7rem;
    display: grid;
    gap: 0.6rem;
  }

  .section-title {
    font-weight: 700;
    color: var(--text);
    font-size: 0.95rem;
  }

  .io-group {
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.75rem;
    background: var(--surface);
    display: grid;
    gap: 0.6rem;
  }

  .io-title {
    font-weight: 700;
    font-size: 0.95rem;
    color: var(--text);
  }

  .io-actions {
    display: flex;
    gap: 0.6rem;
    flex-wrap: wrap;
    align-items: center;
  }

  .alert-settings {
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface);
    padding: 0.25rem 0.75rem;
  }

  .alert-settings .toggle-row {
    padding: 0.55rem 0;
    font-weight: 500;
  }

  .alert-settings .toggle-row + .toggle-row {
    border-top: 1px solid var(--border);
  }

  .experimental-row {
    display: grid;
    grid-template-columns: 1fr auto;
    align-items: center;
    gap: 0.75rem;
    padding: 0.55rem 0;
    border-top: 1px solid var(--border);
  }

  .experimental-row input {
    width: 110px;
  }

  .file-label {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    cursor: pointer;
  }

  .file-label input {
    display: none;
  }

  .service-list {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 0.6rem;
  }

  .service-btn {
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.55rem 0.7rem;
    background: var(--surface-2);
    color: var(--text);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    font-weight: 600;
  }

  .service-btn:hover {
    background: var(--surface-3);
  }

  .service-btn.active {
    border-color: var(--accent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 45%, transparent);
    outline: 1px solid color-mix(in srgb, var(--accent) 60%, transparent);
  }

  .service-logo {
    width: 18px;
    height: 18px;
    display: inline-grid;
    place-items: center;
    color: #ffffff;
  }

  .service-logo :global(svg) {
    width: 18px;
    height: 18px;
    display: block;
  }

  .other-btn {
    grid-column: 1 / -1;
  }

  .other-services-panel {
    margin-top: 0.5rem;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
    padding: 0.65rem;
  }

  .other-services-grid {
    display: grid;
    grid-template-columns: 1fr;
    gap: 0.65rem;
  }

  .service-select-group {
    display: grid;
    gap: 0.35rem;
    font-size: 0.9rem;
    color: var(--text-muted);
  }

  .services-meta {
    color: var(--text-muted);
    font-size: 0.9rem;
  }

  .services-meta.error {
    color: var(--danger);
  }

  .theme-import-input {
    display: none;
  }

  .theme-gallery {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: 0.7rem;
  }

  .theme-card {
    border: 1px solid var(--border);
    border-radius: 16px;
    padding: 0;
    background: var(--surface);
    color: var(--text);
    overflow: hidden;
    text-align: left;
    display: grid;
    grid-template-rows: 94px auto;
  }

  .theme-card:hover {
    border-color: var(--accent);
  }

  .theme-card.selected {
    border-color: var(--accent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 48%, transparent);
  }

  .theme-card-preview {
    background: linear-gradient(180deg, color-mix(in srgb, var(--surface-3) 78%, #000 22%), color-mix(in srgb, var(--surface-2) 72%, #000 28%));
    color: var(--text);
    display: grid;
    place-items: center;
    text-align: center;
    font-size: 0.9rem;
    letter-spacing: 0.02em;
    font-weight: 700;
    text-transform: uppercase;
    padding: 0.5rem;
    word-break: break-word;
  }

  .theme-card-preview.import {
    font-size: 2rem;
    line-height: 1;
    font-weight: 500;
  }

  .theme-card-body {
    border-top: 1px solid var(--border);
    padding: 0.72rem 0.78rem;
    display: grid;
    gap: 0.25rem;
    background: color-mix(in srgb, var(--surface) 88%, #000 12%);
  }

  .theme-card-name {
    font-size: 1rem;
    font-weight: 700;
    color: var(--text);
  }

  .theme-card-meta {
    font-size: 0.88rem;
    color: var(--text-muted);
  }

  .shortcut-input-wrap {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 0.45rem;
    align-items: center;
  }

  .shortcut-capture-btn {
    min-width: 38px;
    height: 38px;
    padding: 0;
    border-radius: 10px;
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text);
    font-size: 1rem;
    display: inline-grid;
    place-items: center;
  }

  .shortcut-capture-btn:hover {
    border-color: var(--accent);
    color: var(--accent);
  }

  .shortcut-capture-backdrop {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    z-index: 25;
    display: grid;
    place-items: center;
    padding: 1rem;
  }

  .shortcut-capture-dialog {
    width: min(430px, 100%);
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--surface-2);
    padding: 0.95rem;
    display: grid;
    gap: 0.65rem;
  }

  .shortcut-capture-dialog h4 {
    margin: 0;
  }

  .shortcut-capture-dialog p {
    margin: 0;
    color: var(--text-muted);
    font-size: 0.92rem;
  }

  .shortcut-capture-preview {
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface);
    padding: 0.65rem 0.75rem;
    font-weight: 700;
    color: var(--accent);
  }

  .shortcut-capture-help {
    color: var(--text-muted);
    font-size: 0.85rem;
  }

  .shortcut-capture-actions {
    display: flex;
    justify-content: flex-end;
  }

  .unsaved-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.62);
    z-index: 10010;
    display: grid;
    place-items: center;
    padding: 1rem;
  }

  .unsaved-dialog {
    width: min(420px, 100%);
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--surface-2);
    padding: 0.9rem;
    display: grid;
    gap: 0.65rem;
  }

  .unsaved-dialog h4 {
    margin: 0;
  }

  .unsaved-dialog p {
    margin: 0;
    color: var(--text-muted);
  }

  .unsaved-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.6rem;
  }

  .two-col {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 0.75rem;
  }

  .two-col > div {
    display: grid;
    gap: 0.4rem;
  }

  @media (max-width: 820px) {
    .two-col {
      grid-template-columns: 1fr;
    }
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
</style>

