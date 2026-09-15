<script lang="ts">
  import Terminal from "@lucide/svelte/icons/terminal";
  import Copy from "@lucide/svelte/icons/copy";
  import Check from "@lucide/svelte/icons/check";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import Search from "@lucide/svelte/icons/search";
  import Filter from "@lucide/svelte/icons/filter";
  import ArrowDown from "@lucide/svelte/icons/arrow-down";
  import { onMount, tick } from "svelte";

  type LogLevel = "info" | "tool" | "success" | "warning" | "error";

  interface LogEntry {
    id: string;
    timestamp: string;
    level: "info" | "tool" | "success" | "warning" | "error";
    category: string;
    message: string;
    details?: string;
  }


  export let logs: LogEntry[] = [];
  export let language: "fr" | "en" = "fr";
  export let onClear: () => void = () => {};

  let searchQuery = "";
  let selectedLevel: "all" | "tool" | "error" | "info" = "all";
  let autoScroll = true;
  let copied = false;
  let terminalElement: HTMLElement;

  $: filteredLogs = logs.filter((log) => {
    const matchesLevel = selectedLevel === "all" || log.level === selectedLevel || (selectedLevel === "tool" && log.level === "success");
    const matchesSearch = !searchQuery.trim() || log.message.toLowerCase().includes(searchQuery.toLowerCase()) || log.category.toLowerCase().includes(searchQuery.toLowerCase());
    return matchesLevel && matchesSearch;
  });

  $: if (filteredLogs.length && autoScroll) {
    scrollToBottom();
  }

  async function scrollToBottom() {
    await tick();
    if (terminalElement) {
      terminalElement.scrollTop = terminalElement.scrollHeight;
    }
  }

  async function copyLogs() {
    const text = filteredLogs
      .map((l) => `[${l.timestamp}] [${l.level.toUpperCase()}] [${l.category}] ${l.message}${l.details ? `\n  ${l.details}` : ""}`)
      .join("\n");
    await navigator.clipboard.writeText(text);
    copied = true;
    setTimeout(() => (copied = false), 2000);
  }
</script>

<div class="live-terminal-panel glassmorphic-panel">
  <!-- Header Bar -->
  <div class="terminal-header">
    <div class="terminal-title">
      <Terminal size={16} class="terminal-icon" />
      <span>{language === "fr" ? "Journal Live Agent" : "Live Agent Log"}</span>
      <span class="live-badge">LIVE</span>
    </div>

    <div class="terminal-actions">
      <button class="icon-btn" title={language === "fr" ? "Copier les logs" : "Copy logs"} on:click={copyLogs}>
        {#if copied}<Check size={14} class="success-icon" />{:else}<Copy size={14} />{/if}
      </button>
      <button class="icon-btn" title={language === "fr" ? "Effacer les logs" : "Clear logs"} on:click={onClear}>
        <Trash2 size={14} />
      </button>
    </div>
  </div>

  <!-- Filter & Search Controls -->
  <div class="terminal-toolbar">
    <div class="search-wrap">
      <Search size={13} class="search-icon" />
      <input
        type="text"
        bind:value={searchQuery}
        placeholder={language === "fr" ? "Filtrer les logs..." : "Filter logs..."}
      />
    </div>

    <div class="filter-pills">
      <button class="pill-btn" class:active={selectedLevel === "all"} on:click={() => (selectedLevel = "all")}>Tous</button>
      <button class="pill-btn tool" class:active={selectedLevel === "tool"} on:click={() => (selectedLevel = "tool")}>Outils</button>
      <button class="pill-btn error" class:active={selectedLevel === "error"} on:click={() => (selectedLevel = "error")}>Erreurs</button>
    </div>
  </div>

  <!-- Logs Output Window -->
  <div class="terminal-body" bind:this={terminalElement}>
    {#if filteredLogs.length === 0}
      <div class="empty-logs">
        <span>{language === "fr" ? "Aucun événement enregistré." : "No events logged."}</span>
      </div>
    {:else}
      {#each filteredLogs as log (log.id)}
        <div class="log-line {log.level}">
          <span class="log-time">{log.timestamp}</span>
          <span class="log-tag {log.level}">[{log.category}]</span>
          <span class="log-msg">{log.message}</span>
          {#if log.details}
            <pre class="log-details">{log.details}</pre>
          {/if}
        </div>
      {/each}
    {/if}
  </div>

  <!-- Footer Info -->
  <div class="terminal-footer">
    <span class="count-label">{filteredLogs.length} {language === "fr" ? "entrées" : "entries"}</span>
    <button class="autoscroll-toggle" class:active={autoScroll} on:click={() => (autoScroll = !autoScroll)}>
      <ArrowDown size={12} />
      <span>Auto-scroll {autoScroll ? "ON" : "OFF"}</span>
    </button>
  </div>
</div>

<style>
  .live-terminal-panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    border-radius: 12px;
    background: rgba(10, 15, 30, 0.92);
    border: 1px solid rgba(255, 255, 255, 0.1);
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
    overflow: hidden;
    font-family: 'JetBrains Mono', 'Fira Code', 'Consolas', monospace;
  }

  .terminal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 14px;
    background: rgba(255, 255, 255, 0.03);
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  }

  .terminal-title {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.82rem;
    font-weight: 600;
    color: #e2e8f0;
  }

  .terminal-icon {
    color: #3b82f6;
  }

  .live-badge {
    font-size: 0.62rem;
    font-weight: 700;
    padding: 2px 6px;
    border-radius: 4px;
    background: rgba(16, 185, 129, 0.2);
    color: #10b981;
    border: 1px solid rgba(16, 185, 129, 0.4);
    letter-spacing: 0.5px;
  }

  .terminal-actions {
    display: flex;
    gap: 6px;
  }

  .icon-btn {
    background: transparent;
    border: none;
    color: #94a3b8;
    padding: 4px;
    border-radius: 4px;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .icon-btn:hover {
    color: #ffffff;
    background: rgba(255, 255, 255, 0.1);
  }

  .success-icon {
    color: #10b981;
  }

  .terminal-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 12px;
    gap: 10px;
    background: rgba(0, 0, 0, 0.2);
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  }

  .search-wrap {
    position: relative;
    flex: 1;
  }

  .search-wrap :global(.search-icon) {
    position: absolute;
    left: 8px;
    top: 50%;
    transform: translateY(-50%);
    color: #64748b;
  }

  .search-wrap input {
    width: 100%;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 6px;
    padding: 4px 8px 4px 26px;
    color: #e2e8f0;
    font-size: 0.74rem;
    outline: none;
  }

  .search-wrap input:focus {
    border-color: #3b82f6;
  }

  .filter-pills {
    display: flex;
    gap: 4px;
  }

  .pill-btn {
    font-size: 0.68rem;
    padding: 3px 8px;
    border-radius: 4px;
    background: transparent;
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #94a3b8;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .pill-btn.active {
    background: rgba(59, 130, 246, 0.2);
    border-color: #3b82f6;
    color: #60a5fa;
  }

  .pill-btn.tool.active {
    background: rgba(16, 185, 129, 0.2);
    border-color: #10b981;
    color: #34d399;
  }

  .pill-btn.error.active {
    background: rgba(239, 68, 68, 0.2);
    border-color: #ef4444;
    color: #f87171;
  }

  .terminal-body {
    flex: 1;
    padding: 10px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 0.76rem;
    line-height: 1.45;
  }

  .empty-logs {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: #64748b;
    font-style: italic;
  }

  .log-line {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: 6px;
    word-break: break-all;
  }

  .log-time {
    color: #475569;
    font-size: 0.7rem;
  }

  .log-tag {
    font-weight: 600;
    padding: 0 4px;
    border-radius: 3px;
  }

  .log-tag.info { color: #3b82f6; background: rgba(59, 130, 246, 0.1); }
  .log-tag.tool { color: #10b981; background: rgba(16, 185, 129, 0.1); }
  .log-tag.success { color: #34d399; background: rgba(52, 211, 153, 0.1); }
  .log-tag.warning { color: #f59e0b; background: rgba(245, 158, 11, 0.1); }
  .log-tag.error { color: #ef4444; background: rgba(239, 68, 68, 0.1); }

  .log-msg {
    color: #cbd5e1;
  }

  .log-details {
    width: 100%;
    margin-top: 4px;
    padding: 6px 10px;
    background: rgba(0, 0, 0, 0.4);
    border-radius: 6px;
    border-left: 2px solid #3b82f6;
    color: #94a3b8;
    font-size: 0.72rem;
    white-space: pre-wrap;
  }

  .terminal-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 12px;
    background: rgba(0, 0, 0, 0.3);
    border-top: 1px solid rgba(255, 255, 255, 0.05);
    font-size: 0.68rem;
    color: #64748b;
  }

  .autoscroll-toggle {
    display: flex;
    align-items: center;
    gap: 4px;
    background: transparent;
    border: none;
    color: #64748b;
    cursor: pointer;
    font-size: 0.68rem;
  }

  .autoscroll-toggle.active {
    color: #3b82f6;
  }
</style>
