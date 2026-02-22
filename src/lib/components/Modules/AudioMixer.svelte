<script lang="ts">
  import type { DemoSource } from "../../types";

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

  export let open = false;
  export let closeAudioMixer: () => void;
  export let handleBackdropKey: (event: KeyboardEvent, closeFn: () => void | Promise<void>) => void;

  export let allowDraggablePopups = false;
  export let openAdditionalSettingsInWindows = false;
  export let audioMixerDragX = 0;
  export let audioMixerDragY = 0;
  export let beginAudioMixerDrag: (event: PointerEvent) => void;
  export let moveAudioMixerDrag: (event: PointerEvent) => void;
  export let endAudioMixerDrag: (event: PointerEvent) => void;
  export let modalEl: HTMLDivElement | null = null;

  export let audioMixerOrientation: "vertical" | "horizontal" = "horizontal";
  export let setAudioMixerOrientation: (orientation: "vertical" | "horizontal") => void;
  export let audioMixerSources: DemoSource[] = [];
  export let ensureAudioMixerState: (source: DemoSource) => AudioMixerItemState;
  export let getAudioMixerVisualLevel: (source: DemoSource, state: AudioMixerItemState) => number;
  export let formatAudioMixerDb: (value: number) => string;

  export let audioMixerRenameSourceId: string | null = null;
  export let audioMixerRenameValue = "";
  export let setAudioMixerRenameValue: (value: string) => void;
  export let setAudioMixerRenameSourceId: (id: string | null) => void;
  export let startAudioMixerRename: (id: string, name: string) => void;
  export let commitAudioMixerRename: (id: string) => Promise<void>;

  export let openAudioMixerContextMenu: (event: MouseEvent, sourceId: string) => void;
  export let setAudioMixerVolumeMode: (sourceId: string, mode: "percent" | "db") => Promise<void>;
  export let setAudioMixerVolumePercentLocal: (sourceId: string, value: number) => void;
  export let commitAudioMixerVolumePercent: (sourceId: string) => Promise<void>;
  export let setAudioMixerVolumeDbLocal: (sourceId: string, value: number) => void;
  export let commitAudioMixerVolumeDb: (sourceId: string) => Promise<void>;
  export let toggleAudioMixerLock: (sourceId: string) => Promise<void>;

  export let AUDIO_MIXER_DB_MIN = -60;
  export let AUDIO_MIXER_DB_MAX = 12;
</script>

{#if open}
  <div class="modal-backdrop" role="button" tabindex="0" on:click={closeAudioMixer} on:keydown={(e) => handleBackdropKey(e, closeAudioMixer)}>
    <div
      class="audio-mixer-modal"
      class:draggable-popup={allowDraggablePopups && !openAdditionalSettingsInWindows}
      bind:this={modalEl}
      role="dialog"
      tabindex="-1"
      aria-modal="true"
      aria-label="Audio mixer"
      style={`--audio-mixer-dx:${audioMixerDragX}px; --audio-mixer-dy:${audioMixerDragY}px;`}
      on:pointerdown={beginAudioMixerDrag}
      on:pointermove={moveAudioMixerDrag}
      on:pointerup={endAudioMixerDrag}
      on:pointercancel={endAudioMixerDrag}
      on:click|stopPropagation
      on:keydown|stopPropagation
    >
      <div class="audio-mixer-header">
        <h3>Audio Mixer</h3>
        <div class="audio-mixer-header-actions">
          <div class="audio-mixer-orientation">
            <button class:active={audioMixerOrientation === "vertical"} on:click={() => setAudioMixerOrientation("vertical")}>Vertical</button>
            <button class:active={audioMixerOrientation === "horizontal"} on:click={() => setAudioMixerOrientation("horizontal")}>Horizontal</button>
          </div>
          <button class="modal-close-x" aria-label="Close audio mixer" on:click={closeAudioMixer}>✕</button>
        </div>
      </div>

      <div class="audio-mixer-list" class:vertical={audioMixerOrientation === "vertical"} class:horizontal={audioMixerOrientation === "horizontal"}>
        {#if !audioMixerSources.length}
          <p class="muted">No audio-capable sources found.</p>
        {:else}
          {#each audioMixerSources as source (source.id)}
            {@const state = ensureAudioMixerState(source)}
            {@const level = getAudioMixerVisualLevel(source, state)}
            <article class="audio-mixer-strip" class:vertical={audioMixerOrientation === "vertical"} class:horizontal={audioMixerOrientation === "horizontal"}>
              <div class="audio-mixer-strip-top">
                {#if audioMixerRenameSourceId === source.id}
                  <input
                    class="audio-mixer-rename-input"
                    value={audioMixerRenameValue}
                    on:input={(e) => setAudioMixerRenameValue((e.currentTarget as HTMLInputElement).value)}
                    on:blur={() => void commitAudioMixerRename(source.id)}
                    on:keydown={(e) => {
                      if (e.key === "Enter") void commitAudioMixerRename(source.id);
                      if (e.key === "Escape") setAudioMixerRenameSourceId(null);
                    }}
                  />
                {:else}
                  <button class="audio-mixer-source-name" on:dblclick={() => startAudioMixerRename(source.id, source.name)}>{source.name}</button>
                {/if}

                <div class="audio-mixer-strip-actions">
                  <button class="mini" on:click={(e) => openAudioMixerContextMenu(e, source.id)} on:contextmenu={(e) => openAudioMixerContextMenu(e, source.id)}>⋯</button>
                </div>
              </div>

              <div class="audio-mixer-main-controls" class:vertical={audioMixerOrientation === "vertical"}>
                <div class="audio-mixer-meter" class:vertical={audioMixerOrientation === "vertical"} title={`Current level ${level}%`}>
                  <div class="audio-mixer-meter-fill" style={audioMixerOrientation === "vertical" ? `height:${level}%;` : `width:${level}%;`}></div>
                </div>

                <div class="audio-mixer-volume-controls" class:vertical={audioMixerOrientation === "vertical"}>
                  <div class="audio-mixer-volume-mode">
                    <button class:active={state.volumeMode === "percent"} on:click={() => void setAudioMixerVolumeMode(source.id, "percent")}>%</button>
                    <button class:active={state.volumeMode === "db"} on:click={() => void setAudioMixerVolumeMode(source.id, "db")}>dB</button>
                  </div>
                  {#if state.volumeMode === "percent"}
                    <div class="audio-mixer-slider-row" class:vertical={audioMixerOrientation === "vertical"}>
                      <input
                        type="range"
                        min="0"
                        max="200"
                        step="1"
                        value={state.volumePercent}
                        disabled={state.locked}
                        on:input={(e) => setAudioMixerVolumePercentLocal(source.id, Number((e.currentTarget as HTMLInputElement).value))}
                        on:change={() => void commitAudioMixerVolumePercent(source.id)}
                      />
                      <span>{Math.round(state.volumePercent)}%</span>
                    </div>
                  {:else}
                    <div class="audio-mixer-slider-row" class:vertical={audioMixerOrientation === "vertical"}>
                      <input
                        type="range"
                        min={AUDIO_MIXER_DB_MIN}
                        max={AUDIO_MIXER_DB_MAX}
                        step="0.1"
                        value={state.volumeDb}
                        disabled={state.locked}
                        on:input={(e) => setAudioMixerVolumeDbLocal(source.id, Number((e.currentTarget as HTMLInputElement).value))}
                        on:change={() => void commitAudioMixerVolumeDb(source.id)}
                      />
                      <span>{formatAudioMixerDb(state.volumeDb)}</span>
                    </div>
                  {/if}
                </div>

                <button class="audio-mixer-lock-btn" aria-label={state.locked ? "Unlock volume" : "Lock volume"} title={state.locked ? "Unlock volume" : "Lock volume"} on:click={() => void toggleAudioMixerLock(source.id)}>{state.locked ? "🔒" : "🔓"}</button>
              </div>
            </article>
          {/each}
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: #000a;
    z-index: 2000;
  }

  .audio-mixer-modal {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: min(1100px, calc(100vw - 2rem));
    max-height: calc(100vh - 2rem);
    overflow: auto;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: 0 10px 36px #000a;
    padding: 1rem;
    z-index: 2010;
    display: grid;
    gap: 0.9rem;
  }

  .audio-mixer-modal.draggable-popup {
    transform: translate(calc(-50% + var(--audio-mixer-dx, 0px)), calc(-50% + var(--audio-mixer-dy, 0px)));
  }

  .audio-mixer-header {
    display: flex;
    justify-content: space-between;
    gap: 0.75rem;
    align-items: center;
  }

  .audio-mixer-header h3 {
    margin: 0;
  }

  .audio-mixer-header-actions {
    display: flex;
    align-items: center;
    gap: 0.55rem;
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

  .audio-mixer-orientation {
    display: inline-flex;
    border: 1px solid var(--border-strong);
    border-radius: 8px;
    overflow: hidden;
  }

  .audio-mixer-orientation button,
  .audio-mixer-volume-mode button,
  .audio-mixer-strip-actions button {
    background: var(--surface-3);
    color: var(--text);
    border: 1px solid var(--border-strong);
    cursor: pointer;
  }

  .audio-mixer-orientation button {
    border: none;
    padding: 0.34rem 0.65rem;
  }

  .audio-mixer-orientation button.active,
  .audio-mixer-volume-mode button.active {
    background: color-mix(in srgb, var(--accent) 35%, var(--surface-3));
    color: #fff;
  }

  .audio-mixer-list {
    display: grid;
    gap: 0.7rem;
  }

  .audio-mixer-list.vertical {
    grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
  }

  .audio-mixer-list.horizontal {
    grid-template-columns: 1fr;
  }

  .audio-mixer-strip {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.7rem;
    display: grid;
    gap: 0.55rem;
  }

  .audio-mixer-strip.vertical {
    grid-template-rows: auto 1fr;
    min-height: 310px;
  }

  .audio-mixer-strip.horizontal {
    grid-template-rows: auto auto;
  }

  .audio-mixer-strip-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }

  .audio-mixer-source-name {
    border: none;
    background: transparent;
    color: var(--text);
    font-weight: 700;
    text-align: left;
    cursor: text;
    padding: 0;
  }

  .audio-mixer-rename-input {
    width: 100%;
    min-width: 0;
    background: var(--surface-3);
    border: 1px solid var(--border-strong);
    color: var(--text);
    border-radius: 8px;
    padding: 0.35rem 0.5rem;
  }

  .audio-mixer-rename-input:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 24%, transparent);
  }

  .audio-mixer-strip-actions {
    display: inline-flex;
    gap: 0.4rem;
  }

  .audio-mixer-strip-actions .mini {
    border-radius: 8px;
    padding: 0.32rem 0.5rem;
  }

  .audio-mixer-main-controls {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto auto;
    gap: 0.55rem;
    align-items: center;
  }

  .audio-mixer-main-controls.vertical {
    grid-template-columns: minmax(0, 1fr);
    align-content: start;
    justify-items: center;
    gap: 0.7rem;
  }

  .audio-mixer-volume-controls {
    display: grid;
    gap: 0.45rem;
    min-width: 160px;
  }

  .audio-mixer-volume-controls.vertical {
    justify-items: center;
    min-width: 0;
  }

  .audio-mixer-meter {
    width: 100%;
    height: 12px;
    border-radius: 999px;
    background: linear-gradient(90deg, #223 0%, #253 60%, #432 100%);
    border: 1px solid var(--border);
    overflow: hidden;
  }

  .audio-mixer-meter.vertical {
    width: 12px;
    height: 170px;
    background: linear-gradient(0deg, #223 0%, #253 60%, #432 100%);
    display: flex;
    align-items: flex-end;
  }

  .audio-mixer-meter-fill {
    height: 100%;
    background: linear-gradient(90deg, #22c55e 0%, #eab308 65%, #ef4444 100%);
    transition: width 120ms ease;
  }

  .audio-mixer-meter.vertical .audio-mixer-meter-fill {
    width: 100%;
    height: 0;
    transition: height 120ms ease;
    background: linear-gradient(0deg, #22c55e 0%, #eab308 65%, #ef4444 100%);
  }

  .audio-mixer-volume-mode {
    display: inline-flex;
    border: 1px solid var(--border-strong);
    border-radius: 8px;
    overflow: hidden;
    width: fit-content;
  }

  .audio-mixer-volume-mode button {
    border: none;
    padding: 0.2rem 0.45rem;
    min-width: 40px;
  }

  .audio-mixer-modal button:hover {
    filter: brightness(1.04);
  }

  .audio-mixer-modal button:disabled {
    opacity: 0.55;
    cursor: not-allowed;
    filter: none;
  }

  .audio-mixer-slider-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 0.5rem;
    align-items: center;
  }

  .audio-mixer-slider-row input[type="range"] {
    width: 100%;
  }

  .audio-mixer-slider-row.vertical {
    grid-template-columns: 1fr;
    justify-items: center;
  }

  .audio-mixer-slider-row.vertical input[type="range"] {
    width: 150px;
    writing-mode: vertical-lr;
    direction: rtl;
  }

  .audio-mixer-slider-row span {
    font-size: 0.84rem;
    color: var(--text-muted);
    min-width: 68px;
    text-align: right;
  }

  .audio-mixer-slider-row.vertical span {
    text-align: center;
    min-width: 0;
  }

  .audio-mixer-lock-btn {
    width: 36px;
    height: 36px;
    border-radius: 999px;
    border: 1px solid var(--border-strong);
    background: var(--surface-3);
    cursor: pointer;
    font-size: 1rem;
  }
</style>
