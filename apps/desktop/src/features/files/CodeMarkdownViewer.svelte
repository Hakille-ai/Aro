<script lang="ts">
  import Copy from "@lucide/svelte/icons/copy";
  import Check from "@lucide/svelte/icons/check";
  import FileCode from "@lucide/svelte/icons/file-code";
  import Eye from "@lucide/svelte/icons/eye";
  import Code from "@lucide/svelte/icons/code";
  import { renderMarkdown } from "../../lib/markdown";

  export let content: string;
  export let fileName: string;
  export let mimeType: string;
  export let language: "fr" | "en" = "fr";
  export let theme: "light" | "dark" = "dark";

  $: isMarkdown = /\.md$/i.test(fileName) || mimeType === "text/markdown";
  $: isJson = /\.json$/i.test(fileName) || mimeType === "application/json";

  let viewMode: "preview" | "code" = isMarkdown ? "preview" : "code";
  let copied = false;

  $: formattedContent = (() => {
    if (isJson) {
      try {
        const parsed = JSON.parse(content);
        return JSON.stringify(parsed, null, 2);
      } catch {
        return content;
      }
    }
    return content;
  })();

  $: lines = formattedContent.split("\n");

  function copyContent() {
    navigator.clipboard.writeText(formattedContent).then(() => {
      copied = true;
      setTimeout(() => (copied = false), 2000);
    });
  }
</script>

<div class="code-viewer-container" class:dark={theme === "dark"}>
  <!-- Toolbar -->
  <div class="code-toolbar">
    <div class="toolbar-left">
      <FileCode size={16} class="code-icon" />
      <span class="file-name" title={fileName}>{fileName}</span>
      <span class="lines-count">{lines.length} {language === "fr" ? "lignes" : "lines"}</span>
    </div>

    <div class="toolbar-right">
      {#if isMarkdown}
        <div class="mode-toggle">
          <button
            class="mode-btn"
            class:active={viewMode === "preview"}
            type="button"
            on:click={() => (viewMode = "preview")}
          >
            <Eye size={13} />
            <span>{language === "fr" ? "Aperçu" : "Preview"}</span>
          </button>
          <button
            class="mode-btn"
            class:active={viewMode === "code"}
            type="button"
            on:click={() => (viewMode = "code")}
          >
            <Code size={13} />
            <span>{language === "fr" ? "Source" : "Source"}</span>
          </button>
        </div>
      {/if}

      <button class="action-btn" type="button" on:click={copyContent} title={language === "fr" ? "Copier le contenu" : "Copy content"}>
        {#if copied}
          <Check size={14} style="color: #30d158;" />
          <span style="color: #30d158;">{language === "fr" ? "Copié !" : "Copied!"}</span>
        {:else}
          <Copy size={14} />
          <span>{language === "fr" ? "Copier" : "Copy"}</span>
        {/if}
      </button>
    </div>
  </div>

  <!-- Content Area -->
  <div class="code-content-area">
    {#if isMarkdown && viewMode === "preview"}
      <div class="markdown-preview-pane">
        <div class="markdown-body">
          {@html renderMarkdown(content, language)}
        </div>
      </div>
    {:else}
      <div class="code-pane">
        <div class="line-numbers">
          {#each lines as _, i}
            <span>{i + 1}</span>
          {/each}
        </div>
        <pre class="code-pre"><code>{formattedContent}</code></pre>
      </div>
    {/if}
  </div>
</div>

<style>
  .code-viewer-container {
    display: flex;
    flex-direction: column;
    height: 100%;
    width: 100%;
    background: #ffffff;
    color: #1d1d1f;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
    border-radius: 12px;
    overflow: hidden;
  }

  .code-viewer-container.dark {
    background: #1c1c1e;
    color: #f5f5f7;
  }

  .code-toolbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px 16px;
    border-bottom: 1px solid rgba(0, 0, 0, 0.08);
    background: rgba(245, 245, 247, 0.85);
    backdrop-filter: blur(12px);
    gap: 12px;
  }

  .dark .code-toolbar {
    background: rgba(30, 30, 35, 0.85);
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  }

  .toolbar-left {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  :global(.code-icon) {
    color: #ff9500;
  }

  .file-name {
    font-size: 13px;
    font-weight: 600;
    max-width: 260px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .lines-count {
    font-size: 11px;
    color: #86868b;
  }

  .toolbar-right {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .mode-toggle {
    display: flex;
    background: rgba(0, 0, 0, 0.06);
    border-radius: 7px;
    padding: 2px;
    gap: 2px;
  }

  .dark .mode-toggle {
    background: rgba(255, 255, 255, 0.08);
  }

  .mode-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 4px 10px;
    border-radius: 5px;
    border: none;
    background: transparent;
    font-size: 11px;
    font-weight: 500;
    cursor: pointer;
    color: #86868b;
    transition: all 0.15s;
  }

  .mode-btn.active {
    background: #ffffff;
    color: #1d1d1f;
    font-weight: 600;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
  }

  .dark .mode-btn.active {
    background: #2c2c2e;
    color: #f5f5f7;
  }

  .action-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 5px 12px;
    border-radius: 7px;
    border: 1px solid rgba(0, 0, 0, 0.1);
    background: #ffffff;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    color: inherit;
    transition: all 0.15s;
  }

  .dark .action-btn {
    background: rgba(255, 255, 255, 0.08);
    border-color: rgba(255, 255, 255, 0.12);
  }

  .action-btn:hover {
    background: rgba(0, 113, 227, 0.08);
    border-color: #0071e3;
    color: #0071e3;
  }

  .code-content-area {
    flex: 1;
    overflow: auto;
    display: flex;
  }

  .markdown-preview-pane {
    flex: 1;
    padding: 32px 48px;
    max-width: 860px;
    margin: 0 auto;
  }

  .code-pane {
    display: flex;
    width: 100%;
    min-height: 100%;
    font-family: ui-monospace, "SF Mono", "Menlo", monospace;
    font-size: 12.5px;
    line-height: 1.6;
  }

  .line-numbers {
    display: flex;
    flex-direction: column;
    padding: 16px 12px;
    background: rgba(0, 0, 0, 0.02);
    color: #8e8e93;
    user-select: none;
    text-align: right;
    border-right: 1px solid rgba(0, 0, 0, 0.06);
    font-size: 11px;
    min-width: 32px;
  }

  .dark .line-numbers {
    background: rgba(255, 255, 255, 0.02);
    border-color: rgba(255, 255, 255, 0.06);
  }

  .code-pre {
    flex: 1;
    margin: 0;
    padding: 16px 20px;
    overflow: visible;
    white-space: pre-wrap;
    word-break: break-word;
  }
</style>
