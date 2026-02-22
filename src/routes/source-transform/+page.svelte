<svelte:head>
  <title>{plannerWindowTitle}</title>
  <meta name="viewport" content="width=900, initial-scale=1.0" />
  <style>
    @import url("https://fonts.googleapis.com/css2?family=Space+Grotesk:wght@400;600;700&display=swap");

    :root {
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

    html.revo-no-rounded *,
    html.revo-no-rounded *::before,
    html.revo-no-rounded *::after {
      border-radius: 0 !important;
    }

    html {
      filter: var(--a11y-color-filter);
      font-size: calc(16px * var(--a11y-font-scale));
    }

    html, body {
      background: #181c22;
      color: #e5e7eb;
      font-family: var(--a11y-font-family);
      font-size: 1rem;
      margin: 0;
    }

    button,
    input,
    select,
    textarea,
    option,
    optgroup {
      font-family: var(--a11y-font-family);
      font-size: 1em;
    }
  </style>
</svelte:head>

<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import GraphicPlanner from "$lib/components/Modules/GraphicPlanner.svelte";
  import type { DemoSource } from "$lib/types";

  let sources: DemoSource[] = [];
  let sceneResolution = "1920x1080";
  let sceneName = "";
  $: plannerWindowTitle = sceneName.trim()
    ? `RevoStream - Graphic planner (${sceneName.trim()})`
    : "RevoStream - Graphic planner";

  $: {
    if (typeof window !== "undefined") {
      void (async () => {
        try {
          const { getCurrentWindow } = await import("@tauri-apps/api/window");
          await getCurrentWindow().setTitle(plannerWindowTitle);
        } catch {
          // browser popup or unsupported environment
        }
        document.title = plannerWindowTitle;
      })();
    }
  }
  let plannerUndoRedoLimit = 5;
  let ready = false;
  let initTimedOut = false;
  let injectedThemeStyleEl: HTMLStyleElement | null = null;
  let unlistenInitEvent: (() => void) | null = null;
  let unlistenThemeRefreshEvent: (() => void) | null = null;

  const THEME_REFRESH_EVENT = "revo:theme-refresh";
  const THEME_REFRESH_STORAGE_KEY = "revo_theme_refresh_ts";
  const MIN_WINDOW_WIDTH = 680;
  const MIN_WINDOW_HEIGHT = 620;

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

  const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

  async function onClose() {
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().close();
      return;
    } catch {
      // browser popup fallback
    }
    window.close();
  }

  async function onSave(transforms: Record<string, PlannerTransformPayload>) {
    // Browser popup path
    window.opener?.postMessage({ type: "saveTransforms", transforms }, "*");

    // Tauri window path (no opener)
    try {
      const { emit } = await import("@tauri-apps/api/event");
      await emit("saveTransforms", transforms);
    } catch {
      // no-op: browser path above already handled when available
    }
  }

  const asRecord = (value: unknown): Record<string, unknown> =>
    value && typeof value === "object" && !Array.isArray(value) ? (value as Record<string, unknown>) : {};

  const resolveA11yFontFamily = (value: string) => {
    if (value === "sans") return "Arial, Helvetica, sans-serif";
    if (value === "serif") return "Georgia, Times New Roman, serif";
    if (value === "mono") return "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, Liberation Mono, monospace";
    return '"Space Grotesk", "Inter", system-ui, sans-serif';
  };

  const resolveA11yColorFilter = (value: string) => {
    if (value === "protanopia") return "sepia(0.24) hue-rotate(-18deg) saturate(0.78)";
    if (value === "deuteranopia") return "sepia(0.2) hue-rotate(8deg) saturate(0.75)";
    if (value === "tritanopia") return "hue-rotate(28deg) saturate(0.78)";
    return "none";
  };

  const applyAccessibilityFromSettings = (uiProfile: Record<string, unknown>) => {
    const accessibility = asRecord(uiProfile.accessibility);
    const root = document.documentElement;

    const uiScale = Math.max(75, Math.min(200, Number(accessibility.accessibilityUiScale ?? "100")));
    root.style.zoom = `${uiScale / 100}`;

    const fontScale = Math.max(0.8, Math.min(2, Number(accessibility.accessibilityFontSize ?? "100") / 100));
    root.style.setProperty("--a11y-font-scale", String(fontScale));
    root.style.setProperty("--a11y-font-family", resolveA11yFontFamily(String(accessibility.accessibilityFontFamily ?? "system")));
    root.style.setProperty("--a11y-color-filter", resolveA11yColorFilter(String(accessibility.accessibilityColorVision ?? "none")));
  };

  const clearAppliedThemeVars = () => {
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
    const root = document.documentElement;
    const roundedFlag = root.style.getPropertyValue("--revo-disable-rounded").trim().toLowerCase();
    const disableRounded = roundedFlag === "1" || roundedFlag === "true" || roundedFlag === "yes" || roundedFlag === "on";
    root.classList.toggle("revo-no-rounded", disableRounded);
  };

  const applyThemeVarsFromCss = (css: string) => {
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

  const applyLookThemeFromSettings = async () => {
    try {
      const settings = await invoke<Record<string, unknown>>("settings_get");
      const uiProfile = asRecord(settings?.ui_profile);
      const general = asRecord(uiProfile.general);
      const look = asRecord(uiProfile.look);
      applyAccessibilityFromSettings(uiProfile);
      plannerUndoRedoLimit = Math.max(1, Math.min(50, Number(general.plannerUndoRedoLimit) || 5));
      const selectedThemeId = String(look.selectedThemeId ?? "").trim();
      if (!selectedThemeId) {
        clearAppliedThemeVars();
        if (injectedThemeStyleEl) {
          injectedThemeStyleEl.remove();
          injectedThemeStyleEl = null;
        }
        return;
      }

      const css = await invoke<string>("themes_get_css", { themeId: selectedThemeId, rootDir: null });
      if (!injectedThemeStyleEl) {
        injectedThemeStyleEl = document.createElement("style");
        injectedThemeStyleEl.setAttribute("data-revo-theme", "source-transform");
        document.head.appendChild(injectedThemeStyleEl);
      }
      injectedThemeStyleEl.textContent = css;
      applyThemeVarsFromCss(css);
    } catch {
      clearAppliedThemeVars();
      // ignore theme loading errors in planner window
    }
  };

  onMount(() => {
    const fallbackTimer = setTimeout(() => {
      if (!ready) {
        console.warn("graphic-planner: init timed out, showing fallback UI");
        initTimedOut = true;
        ready = true;
      }
    }, 4500);

    const setupInitEventListener = async () => {
      try {
        const { listen } = await import("@tauri-apps/api/event");
        unlistenInitEvent = await listen("init", (event) => {
          const payload = (event.payload ?? {}) as {
            sources?: DemoSource[];
            sceneResolution?: string;
            scene_resolution?: string;
            sceneName?: string;
            scene_name?: string;
          };
          sources = Array.isArray(payload.sources) ? payload.sources : [];
          const incomingResolution = String(payload.sceneResolution ?? payload.scene_resolution ?? "").trim();
          if (incomingResolution) {
            sceneResolution = incomingResolution;
          }
          const incomingSceneName = String(payload.sceneName ?? payload.scene_name ?? "").trim();
          if (incomingSceneName) {
            sceneName = incomingSceneName;
          }
          initTimedOut = false;
          ready = true;
        });
      } catch {
        // event bridge unavailable, invoke/postMessage paths still handle init
      }
    };

    const setup = async () => {
      await setupInitEventListener();

      try {
        await applyLookThemeFromSettings();

        const stored = localStorage.getItem("sceneResolution");
        if (stored) sceneResolution = stored;

        try {
          const liveResolution = await invoke<string>("obs_get_current_scene_resolution");
          if (typeof liveResolution === "string" && /^\d+x\d+$/.test(liveResolution.trim())) {
            sceneResolution = liveResolution.trim();
          }
        } catch {
          // fallback paths below
        }

        let loaded = false;
        let sawSuccessfulInit = false;
        for (let i = 0; i < 10; i += 1) {
          try {
            const init = await invoke<{
              sources: DemoSource[];
              scene_resolution: string;
              scene_name?: string | null;
            }>("obs_get_graphic_planner_init");
            sawSuccessfulInit = true;
            if (init?.scene_resolution) {
              sceneResolution = init.scene_resolution;
            }
            sceneName = String(init.scene_name ?? "").trim();
            sources = init?.sources ?? [];

            if (sources.length === 0) {
              try {
                const directSources = await invoke<DemoSource[]>("obs_list_sources");
                if (Array.isArray(directSources) && directSources.length > 0) {
                  sources = directSources;
                  await invoke("obs_set_graphic_planner_init", {
                    init: {
                      sources: directSources,
                      scene_resolution: sceneResolution,
                      selected_source_id: null
                    }
                  });
                }
              } catch {
                // ignore fallback fetch errors
              }
            }

            if (sources.length > 0) {
              loaded = true;
              break;
            }
          } catch {
            // keep retrying; backend/window may still be initializing
          }
          await sleep(180);
        }

        if (!loaded && sawSuccessfulInit) {
          loaded = true;
        }

        if (!loaded) {
          window.opener?.postMessage({ type: "requestInit" }, "*");
          await sleep(300);
        }

        if (!loaded && sources.length === 0) {
          const cached = localStorage.getItem("sourcesList");
          if (cached) {
            try {
              const parsed = JSON.parse(cached) as DemoSource[];
              if (Array.isArray(parsed) && parsed.length > 0) {
                sources = parsed;
                loaded = true;
              }
            } catch {
              // ignore cache parse errors
            }
          }
        }

        initTimedOut = false;
        ready = true;
      } catch (err) {
        console.warn("graphic-planner: failed to initialize", err);
        if (!ready) {
          initTimedOut = true;
          ready = true;
        }
      }
    };

    const enforceMinWindowSize = () => {
      if (typeof window === "undefined") return;
      const nextWidth = Math.max(window.innerWidth, MIN_WINDOW_WIDTH);
      const nextHeight = Math.max(window.innerHeight, MIN_WINDOW_HEIGHT);
      if (nextWidth === window.innerWidth && nextHeight === window.innerHeight) return;
      window.resizeTo(nextWidth, nextHeight);
    };

    const handleThemeRefreshMessage = (event: MessageEvent) => {
      const data = event.data as { type?: string } | null;
      if (!data || data.type !== THEME_REFRESH_EVENT) return;
      void applyLookThemeFromSettings();
    };

    const handleThemeRefreshStorage = (event: StorageEvent) => {
      if (event.key !== THEME_REFRESH_STORAGE_KEY) return;
      void applyLookThemeFromSettings();
    };

    window.addEventListener("message", handleThemeRefreshMessage);
    window.addEventListener("storage", handleThemeRefreshStorage);
    window.addEventListener("resize", enforceMinWindowSize, { passive: true });
    requestAnimationFrame(enforceMinWindowSize);

    void (async () => {
      try {
        const { listen } = await import("@tauri-apps/api/event");
        unlistenThemeRefreshEvent = await listen(THEME_REFRESH_EVENT, () => {
          void applyLookThemeFromSettings();
        });
      } catch {
        unlistenThemeRefreshEvent = null;
      }
    })();

    setup();

    return () => {
      clearTimeout(fallbackTimer);
      window.removeEventListener("message", handleThemeRefreshMessage);
      window.removeEventListener("storage", handleThemeRefreshStorage);
      window.removeEventListener("resize", enforceMinWindowSize);
      if (injectedThemeStyleEl) {
        injectedThemeStyleEl.remove();
        injectedThemeStyleEl = null;
      }
      if (unlistenInitEvent) {
        unlistenInitEvent();
        unlistenInitEvent = null;
      }
      if (unlistenThemeRefreshEvent) {
        unlistenThemeRefreshEvent();
        unlistenThemeRefreshEvent = null;
      }
      clearAppliedThemeVars();
    };
  });
</script>

{#if ready}
  {#if initTimedOut}
    <div style="position: fixed; top: 12px; right: 12px; background: #f59e0b; color: #111; padding: 6px 10px; border-radius: 8px; font-size: 0.85rem; z-index: 9999;">
      Init data missing – showing empty planner.
    </div>
  {/if}
  <GraphicPlanner {sources} {sceneResolution} {onClose} {onSave} undoRedoLimit={plannerUndoRedoLimit} />
{:else}
  <div id="app-loader" style="min-height: 100vh; display: flex; align-items: center; justify-content: center;">
    Loading editor…
  </div>
{/if}
