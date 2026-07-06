<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  let sourceId = "";
  let imgEl: HTMLImageElement | null = null;
  let pollTimer: ReturnType<typeof setTimeout> | null = null;
  let inFlight = false;
  let error = "";

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
    if (inFlight) {
      scheduleFrame(16);
      return;
    }
    inFlight = true;
    try {
      const width = Math.max(320, Math.floor(window.innerWidth));
      const height = Math.max(180, Math.floor(window.innerHeight));
      const screenshot = await invoke<string>("obs_take_screenshot", {
        width,
        height,
        sourceId: sourceId || null,
      });
      if (imgEl) {
        if (screenshot.startsWith("data:")) {
          imgEl.src = screenshot;
        } else {
          imgEl.src = `${screenshot}?t=${Date.now()}`;
        }
      }
      error = "";
    } catch (err) {
      error = String(err);
    } finally {
      inFlight = false;
    }
    scheduleFrame(66);
  };

  onMount(() => {
    if (typeof window === "undefined") return;
    const params = new URLSearchParams(window.location.search);
    sourceId = params.get("source") ?? "";

    scheduleFrame(100);

    const resizeHandler = () => scheduleFrame(30);
    window.addEventListener("resize", resizeHandler, { passive: true });

    return () => {
      window.removeEventListener("resize", resizeHandler);
      stopLoop();
    };
  });
</script>

<svelte:head>
  <title>Preview - RevoStream</title>
  <style>
    :root { color-scheme: dark; }
    html, body { margin: 0; background: #000; overflow: hidden; }
  </style>
</svelte:head>

<div class="root">
  <img bind:this={imgEl} alt="Preview" class="preview-img" />
  {#if error}
    <div class="error">{error}</div>
  {/if}
</div>

<style>
  .root {
    width: 100vw;
    height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #000;
  }
  .preview-img {
    max-width: 100vw;
    max-height: 100vh;
    object-fit: contain;
    display: block;
  }
  .error {
    position: absolute;
    bottom: 1rem;
    left: 50%;
    transform: translateX(-50%);
    color: #ef4444;
    background: rgba(0,0,0,0.7);
    padding: 0.5rem 1rem;
    border-radius: 8px;
    font: 13px system-ui;
  }
</style>
