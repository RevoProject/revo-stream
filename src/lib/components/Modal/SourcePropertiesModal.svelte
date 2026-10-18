<script lang="ts">
  import { createEventDispatcher, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { DemoSource } from "../../types";

  export let open = false;
  export let source: DemoSource | null = null;
  export let sources: DemoSource[] = [];

  const dispatch = createEventDispatcher<{ close: void; changed: void; openFullEditor: void }>();
  let dialogEl: HTMLDivElement | null = null;
  let boundId: string | null = null;
  let name = "";
  let sourceType = "";
  let itemWidth = "";
  let itemHeight = "";
  let scaleX = "";
  let scaleY = "";
  let posX = "";
  let posY = "";
  let dimsReady = false;
  let loading = false;
  let saving = false;
  let error = "";
  let notice = "";

  $: if (!open) boundId = source?.id ?? null;
  $: if (open && !boundId && source) boundId = source.id;
  $: stillPresent = Boolean(boundId && sources.some((item) => item.id === boundId));
  const number = (value: string) => Number.isFinite(Number.parseFloat(value)) ? Number.parseFloat(value) : 0;
  let nativeWidth = 0;
  let nativeHeight = 0;
  let canvasSize = "";

  const load = async (id: string) => {
    loading = true;
    error = "";
    try {
      const result = await invoke<{ name: string; source_type: string; params: Record<string, string> }>(
        "obs_get_source_settings",
        { id }
      );
      if (boundId !== id) return;
      name = result.name || id;
      sourceType = result.source_type || "unknown type";
      itemWidth = result.params.item_width ?? "";
      itemHeight = result.params.item_height ?? "";
      scaleX = result.params.scale_x ?? "";
      scaleY = result.params.scale_y ?? "";
      posX = result.params.pos_x ?? "";
      posY = result.params.pos_y ?? "";
      dimsReady = result.params.dims_ready !== "0";
      try {
        const [size, resolution] = await Promise.all([
          invoke<[number, number]>("obs_get_source_size", { sourceId: id }),
          invoke<string>("obs_get_current_scene_resolution")
        ]);
        if (boundId !== id) return;
        nativeWidth = Number(size?.[0]) || 0;
        nativeHeight = Number(size?.[1]) || 0;
        canvasSize = typeof resolution === "string" && /^\d+x\d+$/.test(resolution.trim())
          ? resolution.trim()
          : "";
      } catch {
        // Informational only; keep transform editors available.
      }
    } catch (err) {
      if (boundId === id) error = String(err);
    } finally {
      loading = false;
    }
  };

  $: if (open && boundId) void load(boundId);
  $: if (open) void tick().then(() => dialogEl?.focus());

  const close = () => {
    error = "";
    notice = "";
    dispatch("close");
  };

  const applyParams = async (params: Record<string, string>, label: string) => {
    if (!boundId || !stillPresent || saving) return;
    saving = true;
    error = "";
    notice = "";
    try {
      await invoke<string>("obs_update_source", {
        update: { id: boundId, name: "", source_type: "", params }
      });
      notice = `${label} applied`;
      dispatch("changed");
      await load(boundId);
    } catch (err) {
      error = String(err);
    } finally {
      saving = false;
    }
  };

  const applySize = () => {
    if (number(itemWidth) <= 0 || number(itemHeight) <= 0) {
      error = "Width and height must be greater than zero.";
      return;
    }
    void applyParams({ item_width: String(Math.round(number(itemWidth))), item_height: String(Math.round(number(itemHeight))) }, "Size");
  };
  const applyScale = () => {
    if (number(scaleX) <= 0 || number(scaleY) <= 0) {
      error = "Scale X and Y must be greater than zero.";
      return;
    }
    void applyParams({ scale_x: String(number(scaleX)), scale_y: String(number(scaleY)) }, "Scale");
  };
  const applyPosition = () => void applyParams({ pos_x: String(number(posX)), pos_y: String(number(posY)) }, "Position");

  const fit = async (command: "obs_fit_source_to_canvas" | "obs_stretch_source_to_canvas", label: string) => {
    if (!boundId || !stillPresent || saving) return;
    saving = true;
    error = "";
    try {
      await invoke<string>(command, { sourceId: boundId });
      notice = `${label} applied`;
      dispatch("changed");
      await load(boundId);
    } catch (err) {
      error = String(err);
    } finally {
      saving = false;
    }
  };

  const handleDialogKeydown = (event: KeyboardEvent) => {
    if (event.key === "Escape") close();
    else event.stopPropagation();
  };
</script>

<svelte:window on:keydown={(event) => open && event.key === "Escape" && close()} />

{#if open}
  <div class="backdrop" role="button" tabindex="-1" onclick={close} onkeydown={(event) => event.key === "Escape" && close()}>
    <div class="modal" role="dialog" aria-modal="true" aria-label="Source properties" tabindex="-1" bind:this={dialogEl} onclick={(event) => event.stopPropagation()} onkeydown={handleDialogKeydown}>
      <header>
        <div><h2>Properties</h2><span>{name || boundId} · {sourceType}</span></div>
        <button class="icon" aria-label="Close properties" onclick={close}>×</button>
      </header>
      <main>
        {#if !stillPresent}
          <p class="status error">This source is no longer in the active scene.</p>
        {:else}
          {#if loading}<p class="status">Loading...</p>{/if}
          <section>
            <h3>Size</h3>
            <p class="dimensions">Source: {nativeWidth} × {nativeHeight} · On canvas: {Math.round(number(itemWidth))} × {Math.round(number(itemHeight))}{#if canvasSize} · Output canvas: {canvasSize}{/if}</p>
            <div class="row">
              <label>Width<input type="number" min="1" bind:value={itemWidth} disabled={saving || !dimsReady} /></label>
              <label>Height<input type="number" min="1" bind:value={itemHeight} disabled={saving || !dimsReady} /></label>
              <button onclick={applySize} disabled={saving || !dimsReady}>Apply size</button>
            </div>
            <div class="row actions">
              <button onclick={() => void fit("obs_fit_source_to_canvas", "Fit to Canvas")} disabled={saving}>Fit to Canvas</button>
              <button onclick={() => void fit("obs_stretch_source_to_canvas", "Stretch to Canvas")} disabled={saving}>Stretch to Canvas</button>
            </div>
          </section>
          <section>
            <h3>Scale</h3>
            <div class="row">
              <label>Scale X<input type="number" min="0.01" step="0.01" bind:value={scaleX} disabled={saving} /></label>
              <label>Scale Y<input type="number" min="0.01" step="0.01" bind:value={scaleY} disabled={saving} /></label>
              <button onclick={applyScale} disabled={saving}>Apply scale</button>
            </div>
          </section>
          <section>
            <h3>Position</h3>
            <div class="row">
              <label>X<input type="number" step="1" bind:value={posX} disabled={saving} /></label>
              <label>Y<input type="number" step="1" bind:value={posY} disabled={saving} /></label>
              <button onclick={applyPosition} disabled={saving}>Apply position</button>
            </div>
          </section>
          <button class="link" onclick={() => dispatch("openFullEditor")}>Open the full source editor</button>
        {/if}
        {#if error}<p class="status error">{error}</p>{/if}
        {#if notice}<p class="status success">{notice}</p>{/if}
      </main>
      <footer><button class="primary" onclick={close}>Close</button></footer>
    </div>
  </div>
{/if}

<style>
  .backdrop { position: fixed; inset: 0; z-index: 50; display: grid; place-items: center; padding: 1rem; background: rgba(0,0,0,.55); }
  .modal { width: min(640px, 94vw); max-height: 86vh; display: grid; grid-template-rows: auto minmax(0,1fr) auto; overflow: hidden; background: var(--surface-2); border: 1px solid var(--border); border-radius: 16px; box-shadow: 0 20px 40px rgba(0,0,0,.45); }
  .modal:focus { outline: none; }
  header, footer { display: flex; align-items: center; justify-content: space-between; gap: .75rem; padding: .85rem 1.2rem; border-bottom: 1px solid var(--border); }
  footer { justify-content: flex-end; border-top: 1px solid var(--border); border-bottom: 0; }
  h2, h3, p { margin: 0; }
  header span { color: var(--text-muted); font-size: .8rem; }
  .icon { width: 34px; height: 34px; color: var(--text); background: var(--surface-3); border: 1px solid var(--border); border-radius: 8px; }
  main { min-height: 0; overflow: auto; display: grid; gap: .85rem; padding: 1rem 1.2rem; }
  section { display: grid; gap: .6rem; padding: .8rem; background: var(--surface-3); border: 1px solid var(--border); border-radius: 10px; }
  .row { display: grid; grid-template-columns: 1fr 1fr auto; align-items: end; gap: .6rem; }
  .row.actions { grid-template-columns: auto auto 1fr; align-items: center; }
  label { display: grid; gap: .3rem; color: var(--text-muted); font-size: .78rem; }
  input { width: 100%; box-sizing: border-box; padding: .45rem .55rem; color: var(--text); background: var(--surface); border: 1px solid var(--border); border-radius: 7px; }
  button { padding: .48rem .75rem; color: var(--text); background: var(--surface); border: 1px solid var(--border); border-radius: 8px; cursor: pointer; }
  button:disabled { opacity: .5; cursor: not-allowed; }
  .primary { color: var(--text-on-primary, #fff); background: var(--accent); }
  .link { justify-self: start; color: var(--accent); background: transparent; border: 0; }
  .dimensions, .status { color: var(--text-muted); font-size: .82rem; }
  .error { color: var(--danger, #ef4444); }
  .success { color: var(--success, #22c55e); }
  @media (max-width: 620px) { .row, .row.actions { grid-template-columns: 1fr; } }
</style>
