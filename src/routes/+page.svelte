<script lang="ts">
  // The RevoStream UI has moved to the native GPUI application
  // (crates/revo-gpui). This legacy Tauri adapter window is kept only as the
  // ObsRuntime parity reference until the native backend cutover completes.
</script>

<div class="root">
  <div class="card">
    <h1>RevoStream</h1>
    <p>
      The application UI has moved to the native GPUI application
      (<code>crates/revo-gpui</code>). Run it with
      <code>cargo run --manifest-path crates/revo-gpui/Cargo.toml</code>.
    </p>
    <p class="muted">
      This legacy window remains as a backend parity reference and has no
      interactive UI.
    </p>
  </div>
</div>

<style>
  :global(html, body) {
    margin: 0;
    background: #0f1115;
    color: #e5e7eb;
    font-family: Inter, system-ui, sans-serif;
  }
  .root {
    min-height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #0f1115;
    color: #e5e7eb;
  }
  .card {
    max-width: 480px;
    padding: 2rem;
    border: 1px solid #2b3142;
    border-radius: 12px;
    background: #151820;
  }
  h1 {
    margin: 0 0 0.75rem;
    font-size: 1.25rem;
  }
  p {
    margin: 0 0 0.75rem;
    line-height: 1.5;
  }
  code {
    background: #1b1f2a;
    border: 1px solid #2b3142;
    border-radius: 6px;
    padding: 0.1rem 0.35rem;
    font-size: 0.85em;
  }
  .muted {
    color: #a1a1aa;
    font-size: 0.85rem;
  }
</style>
