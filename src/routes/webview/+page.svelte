<script lang="ts">
  import { onMount } from "svelte";

  let targetUrl = "https://google.com";
  let inputUrl = targetUrl;
  let frameKey = 0;
  let error = "";

  const applyUrl = (value: string) => {
    try {
      const url = new URL(value.trim());
      if (url.protocol !== "http:" && url.protocol !== "https:") {
        throw new Error("Only http/https URLs are supported.");
      }
      targetUrl = url.href;
      inputUrl = targetUrl;
      error = "";
      frameKey += 1;
    } catch {
      error = "Enter a valid http/https URL.";
    }
  };

  onMount(() => {
    const target = new URLSearchParams(window.location.search).get("target");
    if (target) applyUrl(target);
  });
</script>

<!-- Browser-only fallback. Tauri docks use an unprivileged native child instead. -->
<div class="root">
  <form class="bar" onsubmit={(event) => { event.preventDefault(); applyUrl(inputUrl); }}>
    <input bind:value={inputUrl} aria-label="Webview URL" />
    <button type="submit">Go</button>
    <button type="button" onclick={() => frameKey += 1}>Refresh</button>
    <a href={targetUrl} target="_blank" rel="noopener noreferrer">Open</a>
  </form>
  <div class="notice">
    {error || "Browser preview: sites that prohibit framing must be opened separately. Native webviews are available in the desktop app."}
  </div>
  <div class="body">
    {#key frameKey}
      <iframe
        src={targetUrl}
        title="Browser dock preview"
        sandbox="allow-scripts allow-forms"
        referrerpolicy="no-referrer"
      ></iframe>
    {/key}
  </div>
</div>

<style>
  .root {
    height: 100vh;
    display: grid;
    grid-template-rows: auto auto 1fr;
    background: #11131a;
    color: #e5e7eb;
    font-family: Inter, system-ui, sans-serif;
  }

  .bar {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto auto auto;
    gap: 0.4rem;
    padding: 0.35rem;
    border-bottom: 1px solid #2a2f3a;
    background: #171b24;
  }

  .bar input, .bar button, .bar a {
    min-width: 0;
    border: 1px solid #323a49;
    background: #1f2735;
    color: #e5e7eb;
    border-radius: 8px;
    padding: 0.35rem 0.55rem;
    font: inherit;
    text-decoration: none;
  }

  .bar input { background: #0f131b; }
  .notice { padding: 0.4rem 0.6rem; font-size: 0.75rem; color: #aeb8c8; }
  .body { min-height: 0; }
  iframe { width: 100%; height: 100%; border: 0; display: block; background: #111; }
</style>
