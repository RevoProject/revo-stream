<script lang="ts">
  import { createEventDispatcher } from "svelte";

  export let newSceneName: string;
  export let scenePlaceholder = "Scene 1";

  const dispatch = createEventDispatcher();
  const close = () => dispatch("close");
  const add = () => dispatch("add");
</script>

<div
  class="modal-backdrop"
  role="button"
  tabindex="0"
  onclick={close}
  onkeydown={(e) => (e.key === "Escape" ? close() : null)}
>
  <div
    class="modal"
    role="dialog"
    aria-modal="true"
    tabindex="0"
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.stopPropagation()}
  >
    <header class="modal-header">
      <h2>Add Scene</h2>
    </header>
    <div class="modal-body">
      <div class="field">
        <label for="sceneName">Scene name</label>
        <input id="sceneName" placeholder={scenePlaceholder} bind:value={newSceneName} />
      </div>
      <div class="templates">
        <div class="templates-divider" aria-hidden="true"></div>
        <div class="templates-title">Templates</div>
        <div class="templates-actions">
          <button class="template-btn" type="button" disabled>Soon</button>
          <button class="template-btn" type="button" disabled>Soon</button>
          <button class="template-btn" type="button" disabled>Soon</button>
        </div>
      </div>
    </div>
    <footer class="modal-actions">
      <button class="ghost" onclick={close}>Cancel</button>
      <button class="primary" onclick={add}>Add</button>
    </footer>
  </div>
</div>

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    display: grid;
    place-items: center;
    z-index: 50;
    padding: 1.25rem;
  }

  .modal {
    width: min(460px, 92vw);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 16px;
    box-shadow: 0 20px 40px rgba(0, 0, 0, 0.45);
    display: grid;
    gap: 1rem;
    padding: 1.25rem;
  }

  h2 {
    margin: 0;
    font-size: 1.2rem;
  }

  .field {
    display: grid;
    gap: 0.5rem;
  }

  input {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.65rem 0.85rem;
    color: var(--text);
  }

  input::placeholder {
    color: var(--text-muted);
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.75rem;
  }

  .templates {
    display: grid;
    gap: 0.75rem;
    padding-top: calc(0.25rem + 15px);
  }

  .templates-divider {
    height: 1px;
    background: var(--border);
  }

  .templates-title {
    text-align: center;
    font-size: 1.1rem;
    font-weight: 600;
  }

  .templates-actions {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 0.6rem;
  }

  .template-btn {
    border: 1px dashed var(--border-strong, var(--border));
    border-radius: 12px;
    padding: 0.6rem 0.8rem;
    background: var(--surface);
    color: var(--text-muted);
    font-weight: 600;
  }

  button {
    border: none;
    border-radius: 14px;
    padding: 0.7rem 1.2rem;
    font-weight: 600;
    cursor: pointer;
  }

  button.primary {
    background: var(--accent);
    color: #ffffff;
  }

  button.ghost {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--text);
  }

  button:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
</style>
