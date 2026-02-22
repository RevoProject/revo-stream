<script lang="ts">
  type TransitionItem = {
    id: string;
    name: string;
    kind: string;
    params: Record<string, string>;
  };

  export let open = false;
  export let transitionsDraft: TransitionItem[] = [];
  export let transitionsSelectedId: string | null = null;
  export let activeTransitionId: string | null = null;
  export let selectedTransitionItem: TransitionItem | null = null;
  export let transitionNewKind = "fade";
  export let transitionNewName = "";
  export let transitionKinds: { value: string; label: string }[] = [];

  export let closeTransitions: () => void;
  export let saveTransitions: () => void;
  export let addTransition: () => void;
  export let removeTransition: (id: string) => void;
  export let setTransitionKind: (id: string, kind: string) => void;
  export let updateTransitionParam: (id: string, key: string, value: string) => void;
  export let updateTransitionName: (id: string, value: string) => void;
  export let pickStingerMediaFile: () => Promise<void>;
  export let pickStingerSequenceFolder: () => Promise<void>;
  export let setTransitionsSelectedId: (id: string) => void;
  export let setActiveTransitionId: (id: string) => void;
  export let setTransitionNewKind: (kind: string) => void;
  export let setTransitionNewName: (name: string) => void;
  export let handleBackdropKey: (event: KeyboardEvent, closeFn: () => void | Promise<void>) => void;
</script>

{#if open}
  <div class="modal-backdrop" role="button" tabindex="0" on:click={closeTransitions} on:keydown={(e) => handleBackdropKey(e, closeTransitions)}>
    <div class="quick-text-modal transitions-modal" role="dialog" tabindex="-1" aria-modal="true" aria-label="Transitions" on:click|stopPropagation on:keydown|stopPropagation>
      <div class="audio-filters-header transitions-header">
        <h3>Transitions</h3>
        <button class="modal-close-x" aria-label="Close transitions" on:click={closeTransitions}>✕</button>
      </div>

      <div class="transitions-shell">
        <aside class="transitions-sidebar">
          <div class="transitions-list">
            {#if !transitionsDraft.length}
              <p class="muted">No transitions configured.</p>
            {:else}
              {#each transitionsDraft as transition, index (transition.id)}
                <div
                  class="transition-card"
                  class:selected={transitionsSelectedId === transition.id}
                  role="button"
                  tabindex="0"
                  on:click={() => setTransitionsSelectedId(transition.id)}
                  on:keydown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      setTransitionsSelectedId(transition.id);
                    }
                  }}
                >
                  <div class="transition-card-head">
                    <strong>{transition.name || `Transition ${index + 1}`}</strong>
                    <div class="transition-kind-wrap">
                      <span class="transition-kind">{transitionKinds.find((k) => k.value === transition.kind)?.label ?? transition.kind}</span>
                      {#if transition.kind === "stinger"}
                        <span class="transition-exp-badge">Experimental</span>
                      {/if}
                    </div>
                  </div>
                  <div class="transition-card-meta">
                    <span class="transition-badge" class:active={activeTransitionId === transition.id}>{activeTransitionId === transition.id ? "Active" : "Inactive"}</span>
                    <span>#{index + 1}</span>
                  </div>
                  <button type="button" class="transition-card-remove" aria-label="Remove transition" on:click|stopPropagation={() => removeTransition(transition.id)}>✕</button>
                </div>
              {/each}
            {/if}
          </div>

          <div class="transitions-add">
            <div class="field">
              <label for="transitionAddName">Transition name</label>
              <input id="transitionAddName" value={transitionNewName} placeholder="Transition name" on:input={(e) => setTransitionNewName((e.currentTarget as HTMLInputElement).value)} />
            </div>
            <div class="field">
              <label for="transitionAddKind">Type</label>
              <select id="transitionAddKind" value={transitionNewKind} on:change={(e) => setTransitionNewKind((e.currentTarget as HTMLSelectElement).value)}>
                {#each transitionKinds as kind}
                  <option value={kind.value}>{kind.label}</option>
                {/each}
              </select>
            </div>
            <button class="transition-add-btn" on:click={addTransition}>Add transition</button>
          </div>
        </aside>

        <section class="transitions-main">
          {#if selectedTransitionItem}
            <div class="section-block transitions-active-block">
              <div>
                <div class="section-title">Current transition</div>
                <div class="services-meta">Set selected transition as active for scene changes.</div>
              </div>
              <button
                class="primary"
                on:click={() => setActiveTransitionId(selectedTransitionItem.id)}
                disabled={activeTransitionId === selectedTransitionItem.id}
              >
                {activeTransitionId === selectedTransitionItem.id ? "Active" : "Set active"}
              </button>
            </div>

            <div class="section-block">
              <div class="field">
                <label for="transitionName">Name</label>
                <input
                  id="transitionName"
                  value={selectedTransitionItem.name}
                  on:input={(e) => updateTransitionName(selectedTransitionItem.id, (e.currentTarget as HTMLInputElement).value)}
                />
              </div>

              <div class="field">
                <label for="transitionKind">Transition type</label>
                <select id="transitionKind" value={selectedTransitionItem.kind} on:change={(e) => setTransitionKind(selectedTransitionItem.id, (e.currentTarget as HTMLSelectElement).value)}>
                  {#each transitionKinds as kind}
                    <option value={kind.value}>{kind.label}</option>
                  {/each}
                </select>
              </div>

              {#if selectedTransitionItem.kind === "fade" || selectedTransitionItem.kind === "swipe" || selectedTransitionItem.kind === "slide" || selectedTransitionItem.kind === "fade_to_color" || selectedTransitionItem.kind === "luma_wipe"}
                <div class="field">
                  <label for="transitionDuration">Duration (ms)</label>
                  <input
                    id="transitionDuration"
                    type="number"
                    min="0"
                    step="10"
                    value={selectedTransitionItem.params.duration_ms ?? "300"}
                    on:input={(e) => updateTransitionParam(selectedTransitionItem.id, "duration_ms", (e.currentTarget as HTMLInputElement).value)}
                  />
                </div>
              {/if}

              {#if selectedTransitionItem.kind === "swipe" || selectedTransitionItem.kind === "slide" || selectedTransitionItem.kind === "luma_wipe"}
                <div class="field">
                  <label for="transitionDirection">Direction</label>
                  <select
                    id="transitionDirection"
                    value={selectedTransitionItem.params.direction ?? "left"}
                    on:change={(e) => updateTransitionParam(selectedTransitionItem.id, "direction", (e.currentTarget as HTMLSelectElement).value)}
                  >
                    <option value="left">Left</option>
                    <option value="right">Right</option>
                    <option value="up">Up</option>
                    <option value="down">Down</option>
                  </select>
                </div>
              {/if}

              {#if selectedTransitionItem.kind === "fade_to_color"}
                <div class="field">
                  <label for="transitionFadeToColor">Color</label>
                  <input
                    id="transitionFadeToColor"
                    type="color"
                    value={selectedTransitionItem.params.color ?? "#000000"}
                    on:input={(e) => updateTransitionParam(selectedTransitionItem.id, "color", (e.currentTarget as HTMLInputElement).value)}
                  />
                </div>
              {/if}

              {#if selectedTransitionItem.kind === "luma_wipe"}
                <div class="field">
                  <label for="transitionLumaSoftness">Softness (%)</label>
                  <input
                    id="transitionLumaSoftness"
                    type="number"
                    min="0"
                    max="100"
                    step="1"
                    value={selectedTransitionItem.params.softness_pct ?? "35"}
                    on:input={(e) => updateTransitionParam(selectedTransitionItem.id, "softness_pct", (e.currentTarget as HTMLInputElement).value)}
                  />
                </div>
              {/if}

              {#if selectedTransitionItem.kind === "stinger"}
                <div class="field">
                  <label for="transitionStingerMode">Source</label>
                  <select
                    id="transitionStingerMode"
                    value={selectedTransitionItem.params.source_mode ?? "media"}
                    on:change={(e) => updateTransitionParam(selectedTransitionItem.id, "source_mode", (e.currentTarget as HTMLSelectElement).value)}
                  >
                    <option value="media">Media file (video)</option>
                    <option value="sequence">Sequence (images)</option>
                  </select>
                </div>

                {#if (selectedTransitionItem.params.source_mode ?? "media") === "sequence"}
                  <div class="field">
                    <label for="transitionStingerSequenceDir">Sequence folder</label>
                    <div class="transition-file-row">
                      <input
                        id="transitionStingerSequenceDir"
                        value={selectedTransitionItem.params.sequence_dir ?? ""}
                        placeholder="/path/to/sequence-folder"
                        on:input={(e) => updateTransitionParam(selectedTransitionItem.id, "sequence_dir", (e.currentTarget as HTMLInputElement).value)}
                      />
                      <button class="ghost" type="button" on:click={() => void pickStingerSequenceFolder()}>Import folder</button>
                    </div>
                  </div>

                  <div class="field">
                    <label for="transitionStingerSequenceFps">Sequence FPS</label>
                    <input
                      id="transitionStingerSequenceFps"
                      type="number"
                      min="1"
                      max="240"
                      step="1"
                      value={selectedTransitionItem.params.sequence_fps ?? "30"}
                      on:input={(e) => updateTransitionParam(selectedTransitionItem.id, "sequence_fps", (e.currentTarget as HTMLInputElement).value)}
                    />
                  </div>
                {:else}
                  <div class="field">
                    <label for="transitionStingerFile">Stinger media file</label>
                    <div class="transition-file-row">
                      <input
                        id="transitionStingerFile"
                        value={selectedTransitionItem.params.media_file ?? ""}
                        placeholder="/path/to/stinger.mov"
                        on:input={(e) => updateTransitionParam(selectedTransitionItem.id, "media_file", (e.currentTarget as HTMLInputElement).value)}
                      />
                      <button class="ghost" type="button" on:click={() => void pickStingerMediaFile()}>Import file</button>
                    </div>
                  </div>
                {/if}

                <div class="field">
                  <label for="transitionStingerPoint">Transition point (ms)</label>
                  <input
                    id="transitionStingerPoint"
                    type="number"
                    min="0"
                    step="10"
                    value={selectedTransitionItem.params.transition_point_ms ?? "1000"}
                    on:input={(e) => updateTransitionParam(selectedTransitionItem.id, "transition_point_ms", (e.currentTarget as HTMLInputElement).value)}
                  />
                </div>
              {/if}
            </div>
          {:else}
            <div class="section-block">
              <div class="muted">Add transition to edit settings.</div>
            </div>
          {/if}
        </section>
      </div>

      <div class="quick-text-actions">
        <button class="ghost" on:click={closeTransitions}>Cancel</button>
        <button class="primary" on:click={saveTransitions}>Save</button>
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

  .transitions-modal {
    width: min(980px, calc(100vw - 2rem));
    min-width: min(720px, calc(100vw - 2rem));
  }

  .audio-filters-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }

  .audio-filters-header h3 {
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

  .transitions-shell {
    display: grid;
    grid-template-columns: 320px minmax(0, 1fr);
    gap: 0.9rem;
    min-height: 440px;
  }

  .transitions-sidebar {
    border-right: 1px solid var(--border);
    padding-right: 0.8rem;
    display: grid;
    grid-template-rows: 1fr auto;
    gap: 0.7rem;
    min-height: 0;
  }

  .transitions-list {
    display: grid;
    align-content: start;
    gap: 0.45rem;
    overflow: auto;
    max-height: 420px;
    min-height: 0;
  }

  .transition-card {
    position: relative;
    text-align: left;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: color-mix(in srgb, var(--surface-3) 88%, #000 12%);
    color: var(--text);
    padding: 0.58rem 2rem 0.58rem 0.62rem;
    display: grid;
    gap: 0.3rem;
    cursor: pointer;
  }

  .transition-card.selected {
    border-color: color-mix(in srgb, var(--accent) 55%, var(--border-strong));
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 34%, transparent);
  }

  .transition-card-head {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 0.5rem;
    min-width: 0;
  }

  .transition-kind-wrap {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
  }

  .transition-card-head strong {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .transition-kind {
    font-size: 0.72rem;
    border: 1px solid var(--border-strong);
    border-radius: 10px;
    padding: 0.05rem 0.42rem;
    color: var(--text-muted);
    background: var(--surface-2);
  }

  .transition-exp-badge {
    font-size: 0.64rem;
    font-weight: 700;
    letter-spacing: 0.02em;
    color: #9ec5ff;
    border: 1px solid color-mix(in srgb, #3b82f6 55%, var(--border-strong));
    background: color-mix(in srgb, #3b82f6 20%, var(--surface-2));
    border-radius: 999px;
    padding: 0.04rem 0.38rem;
    white-space: nowrap;
  }

  .transition-card-meta {
    display: flex;
    justify-content: space-between;
    gap: 0.5rem;
    color: var(--text-muted);
    font-size: 0.78rem;
  }

  .transition-badge {
    border: 1px solid var(--border-strong);
    border-radius: 999px;
    padding: 0.03rem 0.4rem;
    background: var(--surface-2);
  }

  .transition-badge.active {
    color: #fff;
    border-color: color-mix(in srgb, var(--accent) 55%, var(--border-strong));
    background: color-mix(in srgb, var(--accent) 68%, var(--surface-2));
  }

  .transition-card-remove {
    position: absolute;
    top: 7px;
    right: 7px;
    width: 25px;
    height: 25px;
    border: 1px solid var(--border-strong);
    border-radius: 8px;
    background: var(--surface-2);
    color: var(--text-muted);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    line-height: 1;
    cursor: pointer;
  }

  .transition-card-remove:hover {
    background: var(--surface-3);
    color: var(--text);
  }

  .transitions-add {
    display: grid;
    gap: 0.42rem;
  }

  .transition-add-btn {
    height: 36px;
    border: none;
    border-radius: 999px;
    padding: 0 0.95rem;
    font-weight: 600;
    cursor: pointer;
    background: var(--accent);
    color: #fff;
  }

  .transition-add-btn:hover {
    background: color-mix(in srgb, var(--accent) 86%, #000 14%);
  }

  .transition-add-btn:focus-visible {
    outline: 2px solid color-mix(in srgb, var(--accent) 62%, #fff 38%);
    outline-offset: 2px;
  }

  .transitions-main {
    display: grid;
    grid-template-rows: auto 1fr;
    gap: 0.75rem;
    align-content: start;
    min-height: 0;
    overflow: auto;
  }

  .transitions-active-block {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
  }

  .section-block {
    border: 1px solid var(--border);
    border-radius: 12px;
    background: color-mix(in srgb, var(--surface-3) 86%, transparent);
    padding: 0.75rem;
    gap: 0.65rem;
  }

  .field {
    display: grid;
    gap: 0.35rem;
  }

  .field label {
    color: var(--text-muted);
    font-size: 0.84rem;
  }

  .services-meta {
    color: var(--text-muted);
    font-size: 0.85rem;
  }

  .transitions-active-block .primary {
    min-width: 108px;
  }

  .transition-file-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 0.5rem;
    align-items: center;
  }

  .transitions-modal input:not([type="checkbox"]):not([type="radio"]):not([type="range"]):not([type="color"]),
  .transitions-modal select {
    width: 100%;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.65rem 0.85rem;
    color: var(--text);
    box-sizing: border-box;
    outline: none;
  }

  .transitions-modal input:not([type="checkbox"]):not([type="radio"]):not([type="range"]):not([type="color"]):focus,
  .transitions-modal select:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 24%, transparent);
  }

  .transitions-modal button {
    border-radius: 9px;
    border: 1px solid var(--border-strong);
    transition: background-color 0.15s ease, border-color 0.15s ease, filter 0.15s ease;
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

  .quick-text-actions button.ghost {
    background: var(--surface-3);
    color: var(--text);
  }

  .quick-text-actions button.ghost:hover,
  .ghost:hover {
    background: var(--surface-2);
  }

  .quick-text-actions button.primary,
  .primary {
    background: var(--accent);
    color: #fff;
    border-color: color-mix(in srgb, var(--accent) 55%, #000 45%);
  }

  .quick-text-actions button.primary:hover,
  .primary:hover {
    filter: brightness(1.04);
  }

  .ghost {
    background: var(--surface-3);
    color: var(--text);
    border-color: var(--border-strong);
  }

  @media (max-width: 920px) {
    .transitions-shell {
      grid-template-columns: 1fr;
    }

    .transitions-sidebar {
      border-right: 0;
      border-bottom: 1px solid var(--border);
      padding-right: 0;
      padding-bottom: 0.75rem;
      grid-template-rows: auto auto;
    }

    .transitions-list {
      max-height: 260px;
    }
  }
</style>
