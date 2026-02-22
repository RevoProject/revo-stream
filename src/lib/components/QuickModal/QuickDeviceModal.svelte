<script lang="ts">
  type DeviceOption = { value: string; label: string };

  export let open = false;
  export let value = "";
  export let options: DeviceOption[] = [];
  export let showMonitoring = false;
  export let monitoring = "off";
  export let monitoringOptions: DeviceOption[] = [];

  export let onClose: () => void;
  export let onSave: () => void;
  export let onValueChange: (value: string) => void;
  export let onMonitoringChange: (value: string) => void;
  export let handleBackdropKey: (event: KeyboardEvent, closeFn: () => void | Promise<void>) => void;
</script>

{#if open}
  <div class="modal-backdrop" role="button" tabindex="0" on:click={onClose} on:keydown={(e) => handleBackdropKey(e, onClose)}>
    <div class="quick-text-modal" role="dialog" tabindex="-1" aria-modal="true" aria-label="Quick device" on:click|stopPropagation on:keydown|stopPropagation>
      <h3>Select device</h3>
      <select value={value} on:change={(e) => onValueChange((e.currentTarget as HTMLSelectElement).value)}>
        {#if !options.length}
          <option value="">Default</option>
        {/if}
        {#each options as option}
          <option value={option.value}>{option.label}</option>
        {/each}
      </select>
      {#if showMonitoring}
        <div class="field">
          <label for="quick-monitoring">Audio monitoring</label>
          <select id="quick-monitoring" value={monitoring} on:change={(e) => onMonitoringChange((e.currentTarget as HTMLSelectElement).value)}>
            {#each monitoringOptions as option}
              <option value={option.value}>{option.label}</option>
            {/each}
          </select>
        </div>
      {/if}
      <div class="quick-text-actions">
        <button class="ghost" on:click={onClose}>Cancel</button>
        <button class="primary" on:click={onSave}>Save</button>
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

  .quick-text-modal select {
    width: 100%;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    color: var(--text);
    padding: 0.55rem 0.7rem;
  }

  .field {
    display: grid;
    gap: 0.45rem;
  }

  .field label {
    color: var(--text-muted);
    font-size: 0.86rem;
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

  .quick-text-actions button.primary {
    background: var(--accent);
    color: #fff;
    border-color: color-mix(in srgb, var(--accent) 55%, #000 45%);
  }
</style>
