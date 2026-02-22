<script lang="ts">
  import { invoke as tauriInvoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  let targetUrl = "https://google.com";
  let inputUrl = targetUrl;
  let renderMode: "canvas" | "iframe" = "canvas";
  let canvasEl: HTMLCanvasElement | null = null;
  let pollTimer: ReturnType<typeof setTimeout> | null = null;
  let inFlight = false;
  let frameLoaded = false;
  let frameBlocked = false;
  let frameError = "";

  const normalizeHttpUrl = (value: string) => {
    const trimmed = value.trim();
    if (!/^https?:\/\//i.test(trimmed)) return null;
    return trimmed;
  };

  const applyUrl = (value: string) => {
    const normalized = normalizeHttpUrl(value);
    if (!normalized) return;
    targetUrl = normalized;
    inputUrl = normalized;
    frameLoaded = false;
    frameBlocked = false;
    frameError = "";
    renderMode = "canvas";
    scheduleFrame(20);
  };

  const refresh = () => {
    frameLoaded = false;
    frameBlocked = false;
    frameError = "";
    renderMode = "canvas";
    scheduleFrame(20);
  };

  const openExternal = () => {
    if (typeof window !== "undefined") {
      window.location.href = targetUrl;
    }
  };

  const paintPlaceholder = (message: string) => {
    if (!canvasEl) return;
    const canvas = canvasEl;
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

  const stopLoop = () => {
    if (pollTimer) {
      clearTimeout(pollTimer);
      pollTimer = null;
    }
  };

  const scheduleFrame = (delay = 0) => {
    stopLoop();
    pollTimer = setTimeout(() => {
      void renderFrame();
    }, delay);
  };

  const renderFrame = async () => {
    if (renderMode !== "canvas") return;
    if (!canvasEl || inFlight) return;

    const canvas = canvasEl;
    const width = Math.max(320, Math.floor(canvas.clientWidth || 640));
    const height = Math.max(180, Math.floor(canvas.clientHeight || 360));

    inFlight = true;
    try {
      const b64 = await tauriInvoke<string>("cef_dock_render_frame", {
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
      }
      frameLoaded = true;
      frameBlocked = false;
      frameError = "";
    } catch (err) {
      const message = String(err);
      if (/tauri|invoke|backend unavailable|command .* not found|not allowed/i.test(message)) {
        // Release/runtime can disable IPC in nested contexts; gracefully fallback.
        renderMode = "iframe";
        frameLoaded = false;
        frameBlocked = false;
        frameError = "";
        stopLoop();
        return;
      }
      frameLoaded = false;
      frameBlocked = true;
      frameError = message;
      paintPlaceholder(`Frame error: ${frameError}`);
    } finally {
      inFlight = false;
    }
    scheduleFrame(220);
  };

  onMount(() => {
    if (typeof window === "undefined") return;
    const params = new URLSearchParams(window.location.search);
    const target = params.get("target");
    if (target) {
      const normalized = normalizeHttpUrl(target);
      if (normalized) {
        targetUrl = normalized;
        inputUrl = normalized;
      }
    }

    scheduleFrame(20);

    const resizeHandler = () => scheduleFrame(30);
    window.addEventListener("resize", resizeHandler, { passive: true });

    return () => {
      window.removeEventListener("resize", resizeHandler);
      stopLoop();
    };
  });
</script>

<div class="root">
  <div class="bar">
    <input bind:value={inputUrl} aria-label="Webview URL" />
    <button on:click={() => applyUrl(inputUrl)}>Go</button>
    <button on:click={refresh}>Refresh</button>
    <button on:click={openExternal}>Open</button>
  </div>

  <div class="body">
    {#if renderMode === "canvas"}
      <canvas bind:this={canvasEl} aria-label="Webview canvas"></canvas>
    {:else}
      <iframe
        src={targetUrl}
        title="Webview iframe fallback"
        on:load={() => {
          frameLoaded = true;
          frameBlocked = false;
        }}
      ></iframe>
    {/if}

    {#if frameBlocked}
      <div class="warn">
        <p>Canvas frame pipeline failed for this URL.</p>
        {#if frameError}
          <p>{frameError}</p>
        {/if}
        <button on:click={openExternal}>Open page directly</button>
      </div>
    {/if}
  </div>
</div>

<style>
  .root {
    height: 100vh;
    display: grid;
    grid-template-rows: 42px 1fr;
    background: #11131a;
    color: #e5e7eb;
    font-family: Inter, system-ui, sans-serif;
  }

  .bar {
    display: grid;
    grid-template-columns: 1fr auto auto auto;
    gap: 0.4rem;
    padding: 0.35rem;
    border-bottom: 1px solid #2a2f3a;
    background: #171b24;
  }

  .bar input {
    min-width: 0;
    border: 1px solid #323a49;
    background: #0f131b;
    color: #e5e7eb;
    border-radius: 8px;
    padding: 0.35rem 0.55rem;
  }

  .bar button {
    border: 1px solid #323a49;
    background: #1f2735;
    color: #e5e7eb;
    border-radius: 8px;
    padding: 0.35rem 0.6rem;
  }

  .body {
    position: relative;
    min-height: 0;
  }

  .body canvas,
  .body iframe {
    width: 100%;
    height: 100%;
    border: 0;
    background: #111;
    display: block;
  }

  .warn {
    position: absolute;
    inset: 0;
    display: grid;
    place-content: center;
    gap: 0.55rem;
    text-align: center;
    background: color-mix(in srgb, #11131a 86%, #000 14%);
    padding: 1rem;
  }

  .warn button {
    justify-self: center;
    border: 1px solid #323a49;
    background: #1f2735;
    color: #e5e7eb;
    border-radius: 8px;
    padding: 0.35rem 0.75rem;
  }
</style>
