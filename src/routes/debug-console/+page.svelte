<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  type LibObsActionEvent = {
    timestamp_ms: number;
    action: string;
    detail?: unknown;
  };

  type ParsedLog = {
    level: "debug" | "info" | "warning" | "error" | "action";
    message: string;
    detail?: string;
  };

  let logs: LibObsActionEvent[] = [];
  let listEl: HTMLDivElement | null = null;
  let autoScroll = true;
  let activeFilter: "all" | ParsedLog["level"] = "all";
  let searchQuery = "";

  const fmtTime = (ms: number) => new Date(ms).toLocaleTimeString();

  const parseLog = (log: LibObsActionEvent): ParsedLog => {
    if (log.action.startsWith("libobs:")) {
      const levelPart = log.action.split(":")[1] ?? "debug";
      const level = (["debug", "info", "warning", "error"].includes(levelPart)
        ? levelPart
        : "debug") as ParsedLog["level"];

      if (log.detail && typeof log.detail === "object") {
        const detailObj = log.detail as Record<string, unknown>;
        const message = typeof detailObj.message === "string" ? detailObj.message : "";
        return {
          level,
          message: message || "(empty)",
          detail: undefined
        };
      }

      return {
        level,
        message: typeof log.detail === "string" ? log.detail : log.action
      };
    }

    return {
      level: "action",
      message: log.action,
      detail: log.detail == null ? undefined : JSON.stringify(log.detail)
    };
  };

  const appendLog = (entry: LibObsActionEvent) => {
    logs = [...logs, entry].slice(-1000);
    if (autoScroll) {
      requestAnimationFrame(() => {
        if (listEl) listEl.scrollTop = listEl.scrollHeight;
      });
    }
  };

  const levelTabs: { id: "all" | ParsedLog["level"]; label: string }[] = [
    { id: "all", label: "All" },
    { id: "action", label: "Action" },
    { id: "debug", label: "Debug" },
    { id: "info", label: "Info" },
    { id: "warning", label: "Warning" },
    { id: "error", label: "Error" }
  ];

  const normalizeSearch = (value: string) =>
    (value ?? "")
      .toLowerCase()
      .normalize("NFKD")
      .replace(/[^\p{L}\p{N}\s]+/gu, " ")
      .replace(/\s+/g, " ")
      .trim();

  const hasFuzzySubsequence = (query: string, text: string) => {
    let i = 0;
    for (const ch of text) {
      if (ch === query[i]) i += 1;
      if (i >= query.length) return true;
    }
    return query.length === 0;
  };

  const fuzzyMatches = (query: string, text: string) => {
    if (!query) return true;
    if (!text) return false;
    if (text.includes(query)) return true;

    const qTokens = query.split(" ").filter(Boolean);
    if (qTokens.length > 1) {
      const tTokens = text.split(" ").filter(Boolean);
      if (qTokens.every((q) => tTokens.some((t) => t.includes(q) || hasFuzzySubsequence(q, t)))) return true;
    }

    return hasFuzzySubsequence(query.replace(/\s+/g, ""), text.replace(/\s+/g, ""));
  };

  $: parsedLogs = logs.map((log) => ({ log, parsed: parseLog(log) }));
  $: filteredLogs = parsedLogs.filter(({ parsed }) => {
    if (activeFilter !== "all" && parsed.level !== activeFilter) return false;

    const query = normalizeSearch(searchQuery);
    if (!query) return true;

    const haystack = normalizeSearch(`${parsed.message} ${parsed.detail ?? ""} ${parsed.level}`);
    return fuzzyMatches(query, haystack);
  });

  const clearLogs = async () => {
    logs = [];
    try {
      await invoke("debug_console_clear");
    } catch {
      // ignore
    }
  };

  const refreshHistory = async () => {
    try {
      const history = await invoke<LibObsActionEvent[]>("debug_console_history");
      logs = history.slice(-1000);
      if (autoScroll) {
        requestAnimationFrame(() => {
          if (listEl) listEl.scrollTop = listEl.scrollHeight;
        });
      }
    } catch {
      // ignore
    }
  };

  onMount(() => {
    void refreshHistory();

    const pollId = setInterval(() => {
      void refreshHistory();
    }, 1200);

    return () => {
      clearInterval(pollId);
    };
  });
</script>

<main class="console-root">
  <header class="toolbar">
    <div class="toolbar-top">
      <h1>Debug Console</h1>
      <div class="actions">
        <label>
          <input type="checkbox" bind:checked={autoScroll} />
          Auto-scroll
        </label>
        <button onclick={() => void clearLogs()}>Clear</button>
      </div>
    </div>
    <div class="toolbar-filters">
      <div class="tabs" role="tablist" aria-label="Log type filter">
        {#each levelTabs as tab}
          <button
            type="button"
            class="tab"
            class:active={activeFilter === tab.id}
            aria-pressed={activeFilter === tab.id}
            onclick={() => (activeFilter = tab.id)}
          >
            {tab.label}
          </button>
        {/each}
      </div>
      <input
        class="search"
        type="search"
        placeholder="Search logs (similar phrases)"
        bind:value={searchQuery}
      />
    </div>
  </header>

  <div class="log-list" bind:this={listEl}>
    {#if !filteredLogs.length}
      <div class="empty">No actions yet. Press F10 in main window and use app actions.</div>
    {:else}
      {#each filteredLogs as item, i (i)}
        {@const log = item.log}
        {@const parsed = item.parsed}
        <div class="row">
          <span class="time">{fmtTime(log.timestamp_ms)}</span>
          <span class="level level-{parsed.level}">{parsed.level}</span>
          <span class="action">{parsed.message}</span>
          {#if parsed.detail}
            <pre class="detail">{parsed.detail}</pre>
          {/if}
        </div>
      {/each}
    {/if}
  </div>
</main>

<style>
  .console-root {
    height: 100vh;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    background: #0e1117;
    color: #d6deeb;
    font-family: Inter, Segoe UI, Roboto, Arial, sans-serif;
  }

  .toolbar {
    display: grid;
    gap: 0.6rem;
    padding: 0.8rem 1rem;
    border-bottom: 1px solid #2a3140;
    position: sticky;
    top: 0;
    z-index: 10;
    background: #0e1117;
  }

  .toolbar-top {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1rem;
  }

  .toolbar-filters {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 320px;
    gap: 0.55rem;
    align-items: center;
  }

  .tabs {
    display: flex;
    gap: 0.35rem;
    flex-wrap: wrap;
  }

  .tab {
    border: 1px solid #34405a;
    background: #131a27;
    color: #aebddd;
    border-radius: 999px;
    padding: 0.25rem 0.65rem;
    font-size: 0.76rem;
  }

  .tab.active {
    color: #ffffff;
    border-color: #5a8dff;
    background: #1b2a47;
  }

  .search {
    border: 1px solid #34405a;
    border-radius: 9px;
    background: #121a27;
    color: #d6deeb;
    padding: 0.4rem 0.55rem;
    font-size: 0.8rem;
  }

  .toolbar h1 {
    margin: 0;
    font-size: 0.95rem;
    font-weight: 700;
    letter-spacing: 0.02em;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 0.8rem;
    font-size: 0.85rem;
  }

  .actions button {
    border: 1px solid #3a4357;
    background: #161b26;
    color: #d6deeb;
    border-radius: 8px;
    padding: 0.35rem 0.7rem;
    cursor: pointer;
  }

  @media (max-width: 860px) {
    .toolbar-filters {
      grid-template-columns: 1fr;
    }
  }

  .log-list {
    overflow: auto;
    padding: 0.45rem 0.55rem 0.7rem;
    display: grid;
    align-content: start;
    gap: 0.25rem;
  }

  .empty {
    color: #8b95aa;
    font-size: 0.88rem;
  }

  .row {
    border: 1px solid #1a2130;
    border-radius: 6px;
    background: #121826;
    padding: 0.3rem 0.45rem;
    display: grid;
    grid-template-columns: 72px 70px minmax(0, 1fr);
    gap: 0.12rem 0.45rem;
    align-items: center;
    min-height: 54px;
  }

  .time {
    color: #7ea7ff;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    font-size: 0.74rem;
  }

  .level {
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    border: 1px solid #2b3447;
    border-radius: 999px;
    padding: 0.1rem 0.35rem;
    width: fit-content;
  }

  .level-debug {
    color: #8aa0c8;
  }

  .level-info {
    color: #8fc7ff;
  }

  .level-warning {
    color: #ffd27d;
  }

  .level-error {
    color: #ff9ea0;
  }

  .level-action {
    color: #b3e5bf;
  }

  .action {
    color: #d7dfef;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    font-size: 0.77rem;
    align-self: center;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .detail {
    grid-column: 2 / -1;
    margin: 0.05rem 0 0;
    padding: 0.28rem 0.35rem;
    border-radius: 6px;
    background: #0d1320;
    border: 1px solid #1e2635;
    color: #aab8d3;
    font-size: 0.7rem;
    white-space: pre-wrap;
    word-break: break-word;
    max-height: 10rem;
    overflow: auto;
  }
</style>
