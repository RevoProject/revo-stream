
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
  export let canUndo: boolean = false;
  export let canRedo: boolean = false;
  export let undoTitle: string = "Nothing to undo";
  export let redoTitle: string = "Nothing to redo";

  const dispatch = createEventDispatcher();

  const openSettings = () => dispatch("openSettings");
  const undo = () => dispatch("undo");
  const redo = () => dispatch("redo");
  const openHistory = (event: MouseEvent) => {
    event.preventDefault();
    dispatch("openHistory");
  };
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
  <div class="brand" title={version ? `Version ${version}` : "RevoStream"}>
    <span class="brandmark" aria-hidden="true">
      <svg viewBox="0 0 40 40" width="40" height="40" focusable="false">
        <path fill="currentColor" d="M7 7h15.5C30.5 7 35 11.2 35 17.3c0 5-3.1 8.5-8.1 9.5L35 33H25.8l-7.3-8.1H14V33H7V7Zm7 6v5.9h7.7c2.5 0 4-1.1 4-3s-1.5-2.9-4-2.9H14Z"></path>
        <path fill="currentColor" opacity=".72" d="M4 5h5v30H4z"></path>
      </svg>
    </span>
    <div class="logo">
      RevoStream
      <span class="tagline">Stream. Create. Control</span>
    </div>
    <button
      class="history-btn"
      onclick={undo}
      oncontextmenu={openHistory}
      disabled={!canUndo}
      title={`${undoTitle} (right-click for history)`}
      aria-label="Undo"
    >

    <svg viewBox="4.7 3.7 14.6 15.7" width="34" height="34" aria-hidden="true" focusable="false">
      <path fill-rule="evenodd" clip-rule="evenodd" fill="currentColor" d="M10.7071 4.29289C11.0976 4.68342 11.0976 5.31658 10.7071 5.70711L8.41421 8H13.5C16.5376 8 19 10.4624 19 13.5C19 16.5376 16.5376 19 13.5 19H11C10.4477 19 10 18.5523 10 18C10 17.4477 10.4477 17 11 17H13.5C15.433 17 17 15.433 17 13.5C17 11.567 15.433 10 13.5 10H8.41421L10.7071 12.2929C11.0976 12.6834 11.0976 13.3166 10.7071 13.7071C10.3166 14.0976 9.68342 14.0976 9.29289 13.7071L5.29289 9.70711C4.90237 9.31658 4.90237 8.68342 5.29289 8.29289L9.29289 4.29289C9.68342 3.90237 10.3166 3.90237 10.7071 4.29289Z" />
    </svg>

    </button>
    <button
      class="history-btn"
      onclick={redo}
      oncontextmenu={openHistory}
      disabled={!canRedo}
      title={`${redoTitle} (right-click for history)`}
      aria-label="Redo"
    >
      <svg viewBox="4.7 3.7 14.6 15.7" width="34" height="34" aria-hidden="true" focusable="false">
        <path fill-rule="evenodd" clip-rule="evenodd" fill="currentColor" d="M13.2929 4.29289C13.6834 3.90237 14.3166 3.90237 14.7071 4.29289L18.7071 8.29289C19.0976 8.68342 19.0976 9.31658 18.7071 9.70711L14.7071 13.7071C14.3166 14.0976 13.6834 14.0976 13.2929 13.7071C12.9024 13.3166 12.9024 12.6834 13.2929 12.2929L15.5858 10H10.5C8.567 10 7 11.567 7 13.5C7 15.433 8.567 17 10.5 17H13C13.5523 17 14 17.4477 14 18C14 18.5523 13.5523 19 13 19H10.5C7.46243 19 5 16.5376 5 13.5C5 10.4624 7.46243 8 10.5 8H15.5858L13.2929 5.70711C12.9024 5.31658 12.9024 4.68342 13.2929 4.29289Z" />
      </svg>
    </button>
  </div>
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
          class={isObsRunning ? "ghost" : "secondary"}
          onclick={isObsRunning ? stopObs : startObs}
          disabled={busy}
        >
          {isObsRunning ? "Stop OBS" : "Start OBS"}
        </button>
      {/if}
      <button class="rec" onclick={isRecording ? stopRecording : startRecording} disabled={busy}>
        <span class="btn-dot" aria-hidden="true"></span>{isRecording ? "Recording" : "Record"}
      </button>
      <button
        class={isStreaming ? "streaming" : "primary"}
        onclick={isStreaming ? stopStreaming : startStreaming}
        disabled={busy}
      >
        {#if isStreaming}<span class="btn-dot" aria-hidden="true"></span>{/if}{isStreaming ? "Streaming" : "Go live"}
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
    background: var(--header-bg);
    border-bottom: 1px solid var(--border);
    z-index: 1000;
  }

  header .logo {
    font-weight: 700;
    font-size: 1.4rem;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 0.65rem;
    color: var(--text);
  }

  .brandmark {
    display: flex;
    flex: 0 0 auto;
    width: 2.1rem;
    height: 2.1rem;
  }

  .brandmark svg {
    display: block;
    width: 100%;
    height: 100%;
  }

  /* RevoFuture-only brand/status extras (hidden in RevoClassic) */
  header .logo .tagline,
  header .btn-dot {
    display: none;
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
    color: var(--text-on-primary);
  }

  button.secondary {
    background: var(--surface-2);
    color: var(--text);
  }

  button.rec {
    background: var(--danger);
    color: var(--text-on-danger);
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

  /* Undo / Redo: framed icon controls beside the brand. The frame is always
     visible; it highlights with the accent color only while an action is
     available. */
  button.history-btn {
    width: 44px;
    height: 44px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    background: var(--surface-3);
    color: var(--text);
    border: 1px solid var(--border-strong);
    border-radius: 12px;
    transition: border-color 0.15s ease, color 0.15s ease, opacity 0.15s ease;
  }

  button.history-btn svg {
    width: 18px;
    height: 18px;
    display: block;
  }

  button.history-btn:disabled {
    opacity: 0.45;
    cursor: default;
  }

  button.history-btn:not(:disabled) {
    border-color: var(--accent);
    color: var(--accent);
  }

  button.history-btn:not(:disabled):hover {
    background: color-mix(in srgb, var(--accent) 14%, var(--surface-3));
  }

  button.settings svg {
    width: 20px;
    height: 20px;
    display: block;
  }

  button.streaming {
    background: var(--success);
    color: var(--text-on-success);
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
