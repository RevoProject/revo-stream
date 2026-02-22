<script lang="ts">
  export let open = false;
  export let x = 0;
  export let y = 0;
  export let sourceId: string | null = null;
  export let onOpenFilters: (sourceId: string) => void;
  export let onOpenAdvanced: (sourceId: string) => void;
  export let onClose: () => void;
  export let handleBackdropKey: (event: KeyboardEvent, closeFn: () => void | Promise<void>) => void;
</script>

{#if open && sourceId}
  <div class="context-menu audio-mixer-context-menu" style={`top:${y}px; left:${x}px;`} role="menu">
    <button
      on:click={() => {
        onOpenFilters(sourceId);
        onClose();
      }}
    >
      Audio Filters
    </button>
    <button
      on:click={() => {
        onOpenAdvanced(sourceId);
        onClose();
      }}
    >
      Advanced Audio Properties
    </button>
  </div>
  <div class="context-overlay audio-mixer-menu-overlay" role="button" tabindex="0" on:click={onClose} on:keydown={(e) => handleBackdropKey(e, onClose)}></div>
{/if}

<style>
  .audio-mixer-context-menu {
    z-index: 2030;
  }

  .audio-mixer-menu-overlay {
    z-index: 2020;
  }
</style>
