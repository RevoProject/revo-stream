<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import type { SceneInfo } from "../../types";

  export let scenes: SceneInfo[] = [];
  export let backendEnabled = false;
  export let renamingScene: string | null = null;
  export let renameSceneValue = "";

  const dispatch = createEventDispatcher();
  let dragSceneName: string | null = null;

  const openAddScene = () => dispatch("openAddScene");
  const setScene = (name: string) => dispatch("setScene", { name });
  const startRename = (scene: SceneInfo) => dispatch("startRename", { scene });
  const commitRename = () => dispatch("commitRename");
  const cancelRename = () => dispatch("cancelRename");
  const openMenu = (event: MouseEvent, scene: SceneInfo) => dispatch("openMenu", { event, scene });
  const updateRenameValue = (value: string) => dispatch("updateRenameValue", { value });
  const reorder = (sceneName: string, toIndex: number) => dispatch("reorder", { sceneName, toIndex });

  const formatSceneName = (name: string) => (name.length > 12 ? `${name.slice(0, 11)}...` : name);

  function handleDragStart(event: DragEvent, scene: SceneInfo) {
    dragSceneName = scene.name;
    event.dataTransfer?.setData("text/plain", scene.name);
    if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
  }

  function handleDragOver(event: DragEvent) {
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
  }

  function handleDrop(event: DragEvent, target: SceneInfo) {
    event.preventDefault();
    const name = dragSceneName ?? event.dataTransfer?.getData("text/plain");
    dragSceneName = null;
    if (!name || name === target.name) return;
    const targetIndex = scenes.findIndex((s) => s.name === target.name);
    reorder(name, targetIndex);
  }
</script>

<div class="scenes">
  <div class="section-title">
    <h2>Scenes</h2>
    <button class="icon" aria-label="Add scene" onclick={openAddScene}>+</button>
  </div>
  <div class="scene-list">
    {#if !backendEnabled}
      <button>Scene 1</button>
      <button>Scene 2</button>
      <button>Scene 3</button>
      <button>Scene 4</button>
      <button>Scene 5</button>
      <button>Scene 6</button>
    {:else if scenes.length === 0}
      <span class="muted" style="width: 200px;">No scenes available</span>
    {:else}
      {#each scenes as scene}
        <div
          class="scene-item"
          oncontextmenu={(e) => openMenu(e, scene)}
          role="button"
          tabindex="0"
          draggable={renamingScene !== scene.name}
          ondragstart={(e) => handleDragStart(e, scene)}
          ondragover={handleDragOver}
          ondrop={(e) => handleDrop(e, scene)}
        >
          {#if renamingScene === scene.name}
            <input
              class="scene-rename"
              value={renameSceneValue}
              oninput={(e) => updateRenameValue((e.currentTarget as HTMLInputElement).value)}
              onblur={commitRename}
              onkeydown={(e) => {
                if (e.key === "Enter") commitRename();
                if (e.key === "Escape") cancelRename();
              }}
            />
          {:else}
            <button
              class={`scene-name ${scene.active ? "active" : ""}`}
              title={scene.name}
              onclick={() => setScene(scene.name)}
              ondblclick={() => startRename(scene)}
            >
              {formatSceneName(scene.name)}
              {#if scene.locked}
                <span class="lock">🔒</span>
              {/if}
            </button>
          {/if}
        </div>
      {/each}
    {/if}
  </div>
</div>

  <style>
    .scenes {
      justify-self: start;
      display: flex;
      flex-direction: column;
      min-height: 0;
      height: 100%;
    }

    .section-title {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 0.5rem;
    }

    .section-title .icon {
      background: var(--surface-3);
      color: var(--text);
      border: 1px solid var(--border-strong);
      width: 30px;
      height: 30px;
      border-radius: 8px;
      padding: 0;
      text-align: center;
    }

    .scene-list {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(105px, 1fr));
      gap: 0.6rem;
      align-content: start;
      align-items: start;
      overflow: auto;
      min-height: 0;
      flex: 1 1 auto;
    }

    .scene-item {
      width: 100%;
      display: flex;
      align-items: center;
    }

    button {
      border: 1px solid var(--border);
      border-radius: 10px;
      padding: 0.55rem 0.75rem;
      background: var(--surface-2);
      color: var(--text);
      width: 100%;
      text-align: left;
      cursor: pointer;
    }

    .scene-name {
      white-space: normal;
      word-break: break-word;
      line-height: 1.25;
      min-height: 42px;
    }

    button.active {
      background: var(--surface-3);
      color: var(--scene-active-text, var(--text));
    }

    .scene-rename {
      width: 100%;
      padding: 0.4rem 0.6rem;
      border-radius: 0.4rem;
      border: 1px solid var(--border-strong);
      background: var(--surface-3);
      color: var(--scene-active-text, var(--text));
    }

    .lock {
      margin-left: 0.4rem;
      font-size: 0.85rem;
      opacity: 0.8;
    }

    @container panel (max-width: 900px) {
      .scene-list {
        grid-template-columns: repeat(auto-fill, minmax(105px, 1fr));
      }
    }
  </style>
