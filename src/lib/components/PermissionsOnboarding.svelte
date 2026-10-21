<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  type PermissionsStatus = {
    macos: boolean;
    screen_capture: boolean;
    accessibility: boolean;
  };

  let status: PermissionsStatus | null = null;
  let dismissed = false;
  let requesting = false;
  let justGranted = false;

  const DISMISS_KEY = "revo_permissions_onboarding_dismissed";

  const isMac = () =>
    typeof navigator !== "undefined" && /Mac/.test(navigator.userAgent);

  async function refresh() {
    try {
      status = await invoke<PermissionsStatus>("macos_permissions_status");
    } catch {
      status = { macos: isMac(), screen_capture: true, accessibility: true };
    }
  }

  async function requestScreenCapture() {
    requesting = true;
    try {
      status = await invoke<PermissionsStatus>("macos_request_screen_capture");
      if (status?.screen_capture) justGranted = true;
    } catch {
      // system dialog may have been dismissed
    } finally {
      requesting = false;
    }
  }

  async function openPane(pane: string) {
    try {
      await invoke("macos_open_privacy_pane", { pane });
    } catch {
      // non-macOS or open failed
    }
  }

  async function recheck() {
    await refresh();
  }

  function dismiss() {
    dismissed = true;
    try {
      localStorage.setItem(DISMISS_KEY, "1");
    } catch {
      // storage unavailable
    }
  }

  onMount(async () => {
    try {
      dismissed =
        (localStorage.getItem(DISMISS_KEY) ?? "") === "1";
    } catch {
      dismissed = false;
    }
    await refresh();
  });

  $: visible =
    !!status && status.macos && !dismissed && !status.screen_capture;
</script>

{#if visible && status}
  <div
    class="perm-overlay"
    role="dialog"
    aria-modal="true"
    aria-label="macOS permissions"
  >
    <div class="perm-card">
      <h2>macOS permissions</h2>
      <p class="perm-lead">
        RevoStream needs a one-time system permission to capture your screen.
        Grant access below, then restart RevoStream if the preview stays black.
      </p>

      <div class="perm-row" data-testid="perm-screen">
        <div>
          <div class="perm-name">Screen Recording</div>
          <div class="perm-desc">
            Required for Display/Screen and Window capture sources.
          </div>
        </div>
        <div class="perm-actions">
          <span class="badge {status.screen_capture ? 'granted' : 'required'}">
            {status.screen_capture ? "Granted" : "Required"}
          </span>
          {#if !status.screen_capture}
            <button class="btn primary" disabled={requesting} on:click={requestScreenCapture}>
              Grant access
            </button>
          {/if}
        </div>
      </div>

      {#if justGranted && status.screen_capture}
        <p class="perm-note ok">
          Access granted. Restart RevoStream so the change takes effect.
        </p>
      {/if}

      <div class="perm-row">
        <div>
          <div class="perm-name">Microphone</div>
          <div class="perm-desc">
            macOS asks automatically the first time you add a microphone
            source.
          </div>
        </div>
        <div class="perm-actions">
          <button class="btn" on:click={() => openPane("Microphone")}>
            Open System Settings
          </button>
        </div>
      </div>

      <div class="perm-row">
        <div>
          <div class="perm-name">Camera</div>
          <div class="perm-desc">
            macOS asks automatically the first time you add a camera source.
          </div>
        </div>
        <div class="perm-actions">
          <button class="btn" on:click={() => openPane("Camera")}>
            Open System Settings
          </button>
        </div>
      </div>

      <div class="perm-row">
        <div>
          <div class="perm-name">Global hotkeys</div>
          <div class="perm-desc">
            Optional — Accessibility access enables system-wide shortcuts.
          </div>
        </div>
        <div class="perm-actions">
          <span class="badge {status.accessibility ? 'granted' : 'required'}">
            {status.accessibility ? "Granted" : "Required"}
          </span>
          {#if !status.accessibility}
            <button class="btn" on:click={() => openPane("Accessibility")}>
              Open System Settings
            </button>
          {/if}
        </div>
      </div>

      <p class="perm-note">
        After granting a permission, macOS may require restarting
        RevoStream before it takes effect.
      </p>

      <div class="perm-footer">
        <button class="btn" on:click={recheck}>Check again</button>
        <button class="btn ghost" on:click={dismiss}>Skip for now</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .perm-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 10000;
  }
  .perm-card {
    background: var(--bg-surface, #1c1c1e);
    color: var(--text-primary, #f2f2f7);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 12px;
    padding: 20px 22px;
    width: min(560px, calc(100vw - 48px));
    box-shadow: 0 18px 50px rgba(0, 0, 0, 0.45);
  }
  h2 {
    margin: 0 0 6px;
    font-size: 1.05rem;
  }
  .perm-lead {
    margin: 0 0 14px;
    font-size: 0.88rem;
    opacity: 0.85;
  }
  .perm-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 10px 0;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
  }
  .perm-name {
    font-weight: 600;
    font-size: 0.92rem;
  }
  .perm-desc {
    font-size: 0.8rem;
    opacity: 0.7;
  }
  .perm-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }
  .badge {
    font-size: 0.75rem;
    padding: 2px 8px;
    border-radius: 999px;
    border: 1px solid rgba(255, 255, 255, 0.18);
  }
  .badge.granted {
    color: #4cd964;
    border-color: rgba(76, 217, 100, 0.5);
  }
  .badge.required {
    color: #ff9f0a;
    border-color: rgba(255, 159, 10, 0.5);
  }
  .btn {
    font: inherit;
    font-size: 0.82rem;
    padding: 5px 10px;
    border-radius: 7px;
    border: 1px solid rgba(255, 255, 255, 0.18);
    background: rgba(255, 255, 255, 0.08);
    color: inherit;
    cursor: pointer;
  }
  .btn.primary {
    background: var(--accent, #0a84ff);
    border-color: transparent;
    color: #fff;
  }
  .btn.ghost {
    background: transparent;
  }
  .btn:disabled {
    opacity: 0.55;
    cursor: default;
  }
  .perm-note {
    font-size: 0.78rem;
    opacity: 0.7;
    margin: 10px 0 0;
  }
  .perm-note.ok {
    color: #4cd964;
  }
  .perm-footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 14px;
  }
</style>
