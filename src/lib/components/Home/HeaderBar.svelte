
<script lang="ts">
  import { createEventDispatcher } from "svelte";

  export let demoMode: boolean;
  export let isObsRunning: boolean;
  export let isRecording: boolean;
  export let isStreaming: boolean;
  export let busy: boolean;
  export let version: string;
  export let realtimeRefresh: boolean = false;
  export let releaseMode: boolean = false;

  const dispatch = createEventDispatcher();

  const openSettings = () => dispatch("openSettings");
  const startObs = () => dispatch("startObs");
  const stopObs = () => dispatch("stopObs");
  const startRecording = () => dispatch("startRecording");
  const stopRecording = () => dispatch("stopRecording");
  const startStreaming = () => dispatch("startStreaming");
  const stopStreaming = () => dispatch("stopStreaming");
  const forcePreviewResolution = () => dispatch("forcePreviewResolution");
  const toggleRealtimeRefresh = (event: Event) => {
    const target = event.currentTarget as HTMLInputElement | null;
    dispatch("toggleRealtimeRefresh", { enabled: Boolean(target?.checked) });
  };
</script>

<header>
  <div class="logo" title={version ? `Version ${version}` : "RevoStream"}>RevoStream</div>
  <div class="actions">
    <div class="nav">
        
      <button class="force-preview" onclick={forcePreviewResolution} title="Force 1920x1080 Preview">
        1920x1080 Preview
      </button>
      {#if !releaseMode}
        <label class="realtime-toggle" title="Realtime refresh preview">
          <input
            type="checkbox"
            checked={realtimeRefresh}
            onchange={toggleRealtimeRefresh}
            aria-label="Realtime refresh"
          />
          <span>Realtime</span>
        </label>
      {/if}
      <button class="settings" aria-label="Settings" onclick={openSettings}>
        <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
          <path
            fill="currentColor"
            d="M19.14 12.94c.04-.31.06-.63.06-.94s-.02-.63-.06-.94l2.03-1.58a.5.5 0 0 0 .12-.64l-1.92-3.32a.5.5 0 0 0-.6-.22l-2.39.96a7.28 7.28 0 0 0-1.63-.94l-.36-2.54a.5.5 0 0 0-.5-.42h-3.84a.5.5 0 0 0-.5.42l-.36 2.54c-.58.23-1.12.54-1.63.94l-2.39-.96a.5.5 0 0 0-.6.22L2.85 7.84a.5.5 0 0 0 .12.64l2.03 1.58c-.04.31-.06.63-.06.94s.02.63.06.94l-2.03 1.58a.5.5 0 0 0-.12.64l1.92 3.32a.5.5 0 0 0 .6.22l2.39-.96c.5.4 1.05.71 1.63.94l.36 2.54a.5.5 0 0 0 .5.42h3.84a.5.5 0 0 0 .5-.42l.36-2.54c.58-.23 1.12-.54 1.63-.94l2.39.96a.5.5 0 0 0 .6-.22l1.92-3.32a.5.5 0 0 0-.12-.64l-2.03-1.58zM12 15.5A3.5 3.5 0 1 1 12 8a3.5 3.5 0 0 1 0 7.5z"
          />
        </svg>
      </button>
      {#if !demoMode && !releaseMode}
        <button
          class={isObsRunning ? "ghost" : "primary"}
          onclick={isObsRunning ? stopObs : startObs}
          disabled={busy}
        >
          {isObsRunning ? "Stop OBS" : "Start OBS"}
        </button>
      {/if}
      <button class="rec" onclick={isRecording ? stopRecording : startRecording} disabled={busy}>
        {isRecording ? "Recording" : "Record"}
      </button>
      <button
        class={isStreaming ? "streaming" : "primary"}
        onclick={isStreaming ? stopStreaming : startStreaming}
        disabled={busy}
      >
        {isStreaming ? "Streaming" : "Go live"}
      </button>
    </div>
  </div>
</header>

<style>
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 1rem 1.3rem;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    z-index: 1000;
  }

  header .logo {
    font-weight: 700;
    font-size: 1.4rem;
  }

  header .nav {
    display: flex;
    justify-items: center;
    align-items: center;
    gap: 0.45rem;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.75rem;
  }

  button {
    border: none;
    border-radius: 14px;
    padding: 0.7rem 1.4rem;
    font-weight: 600;
    cursor: pointer;
    background: var(--surface-3);
    color: var(--text);
  }

  button.primary {
    background: var(--accent);
    color: #ffffff;
  }

  button.rec {
    background: var(--danger);
    color: #ffffff;
  }

  button.settings {
    background: none;
    color: var(--icon-color, var(--text));
    width: 42px;
    height: 42px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0;
  }

  button.settings svg {
    width: 20px;
    height: 20px;
    display: block;
  }

  button.streaming {
    background: var(--success);
    color: #ffffff;
  }

  button.ghost {
    background: transparent;
    border: 1px solid var(--border-strong);
  }

  button:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  button.force-preview {
    display: none;
    visibility: hidden;
  }

  .realtime-toggle {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.8rem;
    color: var(--text-dim);
    user-select: none;
  }

  .realtime-toggle input {
    width: 16px;
    height: 16px;
    accent-color: var(--accent);
    cursor: pointer;
  }

  .realtime-toggle span {
    line-height: 1;
  }
</style>
