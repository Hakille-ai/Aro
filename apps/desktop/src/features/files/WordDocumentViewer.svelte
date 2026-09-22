<script lang="ts">
  import FileText from "@lucide/svelte/icons/file-text";
  import ZoomIn from "@lucide/svelte/icons/zoom-in";
  import ZoomOut from "@lucide/svelte/icons/zoom-out";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import Copy from "@lucide/svelte/icons/copy";
  import Check from "@lucide/svelte/icons/check";
  import Printer from "@lucide/svelte/icons/printer";
  import type { WordDocumentData } from "../../lib/documents/word-parser";

  export let data: WordDocumentData;
  export let fileName: string;
  export let language: "fr" | "en" = "fr";
  export let theme: "light" | "dark" = "dark";

  let zoom = 1.0;
  let copied = false;

  function zoomIn() {
    zoom = Math.min(zoom + 0.1, 2.0);
  }

  function zoomOut() {
    zoom = Math.max(zoom - 0.1, 0.5);
  }

  function resetZoom() {
    zoom = 1.0;
  }

  function copyText() {
    const textLines: string[] = [];
    if (data.title) textLines.push(data.title);
    if (data.subtitle) textLines.push(data.subtitle);
    textLines.push("");

    for (const sec of data.sections) {
      if (sec.text) {
        textLines.push(sec.text);
      } else if (sec.tableData) {
        textLines.push(sec.tableData.headers.join("\t"));
        for (const row of sec.tableData.rows) {
          textLines.push(row.join("\t"));
        }
      }
    }

    navigator.clipboard.writeText(textLines.join("\n")).then(() => {
      copied = true;
      setTimeout(() => (copied = false), 2000);
    });
  }

  function printDocument() {
    window.print();
  }
</script>

<div class="word-viewer-container" class:dark={theme === "dark"}>
  <!-- Top Toolbar -->
  <div class="word-toolbar">
    <div class="toolbar-left">
      <div class="file-badge">
        <FileText size={16} class="word-icon" />
        <span class="file-name" title={fileName}>{fileName}</span>
      </div>
      <span class="doc-badge">DOCX</span>
    </div>

    <div class="toolbar-right">
      <div class="zoom-controls">
        <button class="icon-btn" type="button" on:click={zoomOut} title={language === "fr" ? "Zoom arrière" : "Zoom out"} disabled={zoom <= 0.5}>
          <ZoomOut size={14} />
        </button>
        <span class="zoom-label">{Math.round(zoom * 100)}%</span>
        <button class="icon-btn" type="button" on:click={zoomIn} title={language === "fr" ? "Zoom avant" : "Zoom in"} disabled={zoom >= 2.0}>
          <ZoomIn size={14} />
        </button>
        <button class="icon-btn" type="button" on:click={resetZoom} title={language === "fr" ? "Réinitialiser" : "Reset"}>
          <RotateCcw size={12} />
        </button>
      </div>

      <div class="toolbar-divider"></div>

      <button class="action-btn" type="button" on:click={copyText} title={language === "fr" ? "Copier le texte" : "Copy text"}>
        {#if copied}
          <Check size={14} style="color: #30d158;" />
          <span style="color: #30d158;">{language === "fr" ? "Copié !" : "Copied!"}</span>
        {:else}
          <Copy size={14} />
          <span>{language === "fr" ? "Copier" : "Copy"}</span>
        {/if}
      </button>

      <button class="action-btn" type="button" on:click={printDocument} title={language === "fr" ? "Imprimer" : "Print"}>
        <Printer size={14} />
        <span>{language === "fr" ? "Imprimer" : "Print"}</span>
      </button>
    </div>
  </div>

  <!-- Document Canvas / Paper Sheet -->
  <div class="word-canvas">
    <div class="paper-sheet" style="zoom: {zoom}; transform-origin: top center;">
      <!-- Title Block -->
      {#if data.title}
        <h1 class="doc-title">{data.title}</h1>
      {/if}

      {#if data.subtitle}
        <div class="doc-subtitle">{data.subtitle}</div>
      {/if}

      {#if data.title || data.subtitle}
        <div class="doc-divider"></div>
      {/if}

      <!-- Document Sections -->
      <div class="doc-body">
        {#each data.sections as section}
          {#if section.type === "title" && section.text !== data.title}
            <h1 class="doc-heading level-1">{section.text}</h1>
          {:else if section.type === "subtitle" && section.text !== data.subtitle}
            <p class="doc-subtitle">{section.text}</p>
          {:else if section.type === "heading"}
            {#if section.level === 1}
              <h2 class="doc-heading level-1">{section.text}</h2>
            {:else if section.level === 2}
              <h3 class="doc-heading level-2">{section.text}</h3>
            {:else}
              <h4 class="doc-heading level-3">{section.text}</h4>
            {/if}
          {:else if section.type === "list-item"}
            <div class="doc-list-item">
              <span class="bullet">•</span>
              <span class="list-text">
                {#if section.runs && section.runs.length > 0}
                  {#each section.runs as run}
                    <span class:bold={run.bold} class:italic={run.italic} class:underline={run.underline}>{run.text}</span>
                  {/each}
                {:else}
                  {section.text}
                {/if}
              </span>
            </div>
          {:else if section.type === "paragraph"}
            <p class="doc-paragraph">
              {#if section.runs && section.runs.length > 0}
                {#each section.runs as run}
                  <span class:bold={run.bold} class:italic={run.italic} class:underline={run.underline}>{run.text}</span>
                {/each}
              {:else}
                {section.text}
              {/if}
            </p>
          {:else if section.type === "table" && section.tableData}
            <div class="doc-table-wrapper">
              <table class="doc-table">
                {#if section.tableData.headers.length > 0}
                  <thead>
                    <tr>
                      {#each section.tableData.headers as h}
                        <th>{h}</th>
                      {/each}
                    </tr>
                  </thead>
                {/if}
                <tbody>
                  {#each section.tableData.rows as row}
                    <tr>
                      {#each row as cell}
                        <td>{cell}</td>
                      {/each}
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
          {/if}
        {/each}
      </div>
    </div>
  </div>
</div>

<style>
  .word-viewer-container {
    display: flex;
    flex-direction: column;
    height: 100%;
    width: 100%;
    background: #f5f5f7;
    color: #1d1d1f;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
    border-radius: 12px;
    overflow: hidden;
  }

  .word-viewer-container.dark {
    background: #151516;
    color: #f5f5f7;
  }

  .word-toolbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px 16px;
    border-bottom: 1px solid rgba(0, 0, 0, 0.08);
    background: rgba(255, 255, 255, 0.85);
    backdrop-filter: blur(12px);
    gap: 12px;
  }

  .dark .word-toolbar {
    background: rgba(30, 30, 35, 0.85);
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  }

  .toolbar-left {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .file-badge {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    font-weight: 600;
    max-width: 260px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  :global(.word-icon) {
    color: #2b579a;
    flex-shrink: 0;
  }

  .doc-badge {
    padding: 2px 6px;
    border-radius: 4px;
    background: rgba(43, 87, 154, 0.12);
    color: #2b579a;
    font-size: 10px;
    font-weight: 700;
  }

  .dark .doc-badge {
    background: rgba(43, 87, 154, 0.25);
    color: #5c9aff;
  }

  .toolbar-right {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .zoom-controls {
    display: flex;
    align-items: center;
    background: rgba(0, 0, 0, 0.05);
    border-radius: 8px;
    padding: 2px 4px;
    gap: 2px;
  }

  .dark .zoom-controls {
    background: rgba(255, 255, 255, 0.08);
  }

  .icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: 5px;
    border: none;
    background: transparent;
    cursor: pointer;
    color: inherit;
    transition: background 0.15s;
  }

  .icon-btn:hover:not(:disabled) {
    background: rgba(0, 0, 0, 0.08);
  }

  .dark .icon-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.12);
  }

  .icon-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .zoom-label {
    font-size: 11px;
    font-weight: 600;
    min-width: 36px;
    text-align: center;
    font-variant-numeric: tabular-nums;
  }

  .toolbar-divider {
    width: 1px;
    height: 18px;
    background: rgba(0, 0, 0, 0.1);
    margin: 0 4px;
  }

  .dark .toolbar-divider {
    background: rgba(255, 255, 255, 0.1);
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

  .word-canvas {
    flex: 1;
    overflow: auto;
    padding: 32px 16px;
    display: flex;
    justify-content: center;
    align-items: flex-start;
  }

  .paper-sheet {
    width: 100%;
    max-width: 800px;
    min-height: 1050px;
    background: #ffffff;
    color: #262626;
    padding: 56px 64px;
    box-sizing: border-box;
    box-shadow: 0 8px 30px rgba(0, 0, 0, 0.12);
    border-radius: 4px;
    transition: transform 0.2s ease-out;
  }

  .doc-title {
    font-size: 26px;
    font-weight: 700;
    margin: 0 0 10px 0;
    color: #111111;
    line-height: 1.25;
  }

  .doc-subtitle {
    font-size: 15px;
    color: #666666;
    margin: 0 0 16px 0;
    font-style: italic;
  }

  .doc-divider {
    height: 1px;
    background: #e0e0e0;
    margin: 20px 0 28px 0;
  }

  .doc-body {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .doc-heading {
    margin: 18px 0 6px 0;
    color: #111111;
    font-weight: 600;
  }

  .doc-heading.level-1 {
    font-size: 20px;
    border-bottom: 1px solid #eeeeee;
    padding-bottom: 6px;
  }

  .doc-heading.level-2 {
    font-size: 16px;
  }

  .doc-heading.level-3 {
    font-size: 14px;
  }

  .doc-paragraph {
    font-size: 14px;
    line-height: 1.65;
    margin: 0;
    color: #333333;
  }

  .doc-list-item {
    display: flex;
    gap: 8px;
    font-size: 14px;
    line-height: 1.6;
    color: #333333;
    padding-left: 8px;
  }

  .bullet {
    color: #0071e3;
    font-weight: bold;
  }

  .list-text {
    flex: 1;
  }

  .bold {
    font-weight: 700;
  }

  .italic {
    font-style: italic;
  }

  .underline {
    text-decoration: underline;
  }

  .doc-table-wrapper {
    overflow-x: auto;
    margin: 14px 0;
  }

  .doc-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
  }

  .doc-table th {
    background: #f4f4f6;
    color: #222222;
    font-weight: 600;
    text-align: left;
    padding: 8px 12px;
    border: 1px solid #dcdce0;
  }

  .doc-table td {
    padding: 8px 12px;
    border: 1px solid #e2e2e6;
    color: #333333;
  }

  .doc-table tr:nth-child(even) td {
    background: #fafafc;
  }
</style>
