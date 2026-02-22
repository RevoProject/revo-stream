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
  export let source: DemoSource | null = null;
  export let state: AudioMixerItemState | null = null;
  export let allowDraggablePopups = false;
  export let openAdditionalSettingsInWindows = false;
  export let modalEl: HTMLDivElement | null = null;
  export let dragX = 0;
  export let dragY = 0;
  export let monitoringOptions: { value: string; label: string }[] = [];
  export let trackIds: string[] = ["1", "2", "3", "4", "5", "6"];

  export let beginDrag: (event: PointerEvent) => void;
  export let moveDrag: (event: PointerEvent) => void;
  export let endDrag: (event: PointerEvent) => void;
  export let onClose: () => void;
  export let getBalancePan: (state: AudioMixerItemState) => number;
  export let onMonitoringChange: (sourceId: string, monitoring: string) => void;
  export let onBalancePanInput: (sourceId: string, pan: number) => void;
  export let onBalancePanCommit: (sourceId: string) => void;
  export let onToggleTrack: (sourceId: string, track: string) => void;
</script>

{#if open && source && state}
  <div
    class="audio-advanced-modal"
    class:draggable-popup={allowDraggablePopups && !openAdditionalSettingsInWindows}
    bind:this={modalEl}
    role="dialog"
    tabindex="-1"
    aria-modal="false"
    aria-label="Advanced audio properties"
    style={`--audio-advanced-dx:${dragX}px; --audio-advanced-dy:${dragY}px;`}
    on:pointerdown={beginDrag}
    on:pointermove={moveDrag}
    on:pointerup={endDrag}
    on:pointercancel={endDrag}
  >
    <div class="audio-advanced-header">
      <h3>Advanced Audio Properties</h3>
      <button class="modal-close-x" aria-label="Close advanced audio properties" on:click={onClose}>✕</button>
    </div>
    <p class="muted">{source.name}</p>

    <div class="audio-mixer-advanced">
      <div class="audio-mixer-advanced-row">
        <span class="audio-mixer-row-label">Monitoring</span>
        <select value={state.monitoring} on:change={(e) => onMonitoringChange(source.id, (e.currentTarget as HTMLSelectElement).value)}>
          {#each monitoringOptions as option}
            <option value={option.value}>{option.label}</option>
          {/each}
        </select>
      </div>

      <div class="audio-mixer-advanced-row audio-mixer-balance-row">
        <span class="audio-mixer-row-label">L/R Balance</span>
        <div class="audio-mixer-balance-single">
          <div class="audio-mixer-balance-labels"><span>Left</span><span>Right</span></div>
          <input
            type="range"
            min="-100"
            max="100"
            step="1"
            value={getBalancePan(state)}
            on:input={(e) => onBalancePanInput(source.id, Number((e.currentTarget as HTMLInputElement).value))}
            on:change={() => onBalancePanCommit(source.id)}
          />
        </div>
      </div>

      <div class="audio-mixer-advanced-row">
        <span class="audio-mixer-row-label">Tracks</span>
        <div class="audio-mixer-track-list improved">
          {#each trackIds as track}
            <button
              type="button"
              class:active={state.tracks.includes(track)}
              on:click={() => onToggleTrack(source.id, track)}
            >
              Track {track}
            </button>
          {/each}
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .audio-advanced-modal {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: min(560px, calc(100vw - 2rem));
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: 0 10px 36px #000a;
    padding: 1rem;
    z-index: 2025;
    display: grid;
    gap: 0.7rem;
  }

  .audio-advanced-modal.draggable-popup {
    transform: translate(calc(-50% + var(--audio-advanced-dx, 0px)), calc(-50% + var(--audio-advanced-dy, 0px)));
  }

  .audio-advanced-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }

  .audio-advanced-header h3 {
    margin: 0;
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

  .audio-mixer-advanced {
    display: grid;
    gap: 0.45rem;
  }

  .audio-mixer-advanced-row {
    display: grid;
    gap: 0.35rem;
  }

  .audio-mixer-row-label {
    font-size: 0.78rem;
    color: var(--text-muted);
  }

  .audio-mixer-advanced-row select,
  .audio-mixer-advanced-row input[type="range"] {
    width: 100%;
  }

  .audio-mixer-advanced-row select {
    background: var(--surface);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.52rem 0.72rem;
    outline: none;
  }

  .audio-mixer-advanced-row select:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 22%, transparent);
  }

  .audio-mixer-balance-single {
    display: grid;
    gap: 0.35rem;
  }

  .audio-mixer-balance-labels {
    display: flex;
    justify-content: space-between;
    font-size: 0.72rem;
    color: var(--text-muted);
  }

  .audio-mixer-track-list {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
  }

  .audio-mixer-track-list.improved button {
    border: 1px solid var(--border-strong);
    border-radius: 8px;
    background: var(--surface-3);
    color: var(--text);
    padding: 0.35rem 0.55rem;
    cursor: pointer;
  }

  .audio-mixer-track-list.improved button.active {
    background: color-mix(in srgb, var(--accent) 35%, var(--surface-3));
    border-color: color-mix(in srgb, var(--accent) 45%, var(--border-strong));
    color: #fff;
  }

  .muted {
    color: var(--text-muted);
    margin: 0;
  }
</style>
