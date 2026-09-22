<script lang="ts">
  import Download from "@lucide/svelte/icons/download";
  import Copy from "@lucide/svelte/icons/copy";
  import Check from "@lucide/svelte/icons/check";
  import Search from "@lucide/svelte/icons/search";
  import ArrowUpDown from "@lucide/svelte/icons/arrow-up-down";
  import ArrowUp from "@lucide/svelte/icons/arrow-up";
  import ArrowDown from "@lucide/svelte/icons/arrow-down";
  import FileSpreadsheet from "@lucide/svelte/icons/file-spreadsheet";
  import type { SpreadsheetData } from "../../lib/documents/excel-parser";

  export let data: SpreadsheetData;
  export let fileName: string;
  export let language: "fr" | "en" = "fr";
  export let theme: "light" | "dark" = "dark";

  let activeSheetIdx = 0;
  let searchQuery = "";
  let sortColIndex: number | null = null;
  let sortDirection: "asc" | "desc" = "asc";
  let copied = false;

  $: currentSheet = data.sheets[activeSheetIdx] || data.sheets[0] || { name: "Sheet", headers: [], rows: [] };

  $: filteredRows = (() => {
    let rows = currentSheet.rows;
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase().trim();
      rows = rows.filter((row) =>
        row.some((cell) => cell !== null && cell !== undefined && String(cell).toLowerCase().includes(q))
      );
    }
    if (sortColIndex !== null && sortColIndex >= 0) {
      const col = sortColIndex;
      const dir = sortDirection === "asc" ? 1 : -1;
      rows = [...rows].sort((a, b) => {
        const valA = a[col];
        const valB = b[col];
        if (valA === valB) return 0;
        if (valA === null || valA === undefined) return 1;
        if (valB === null || valB === undefined) return -1;
        if (typeof valA === "number" && typeof valB === "number") {
          return (valA - valB) * dir;
        }
        return String(valA).localeCompare(String(valB), undefined, { numeric: true }) * dir;
      });
    }
    return rows;
  })();

  function toggleSort(colIdx: number) {
    if (sortColIndex === colIdx) {
      if (sortDirection === "asc") {
        sortDirection = "desc";
      } else {
        sortColIndex = null;
        sortDirection = "asc";
      }
    } else {
      sortColIndex = colIdx;
      sortDirection = "asc";
    }
  }

  function handleCopyTable() {
    const lines = [
      currentSheet.headers.join("\t"),
      ...filteredRows.map((row) => row.map((c) => (c === null || c === undefined ? "" : String(c))).join("\t")),
    ];
    navigator.clipboard.writeText(lines.join("\n")).then(() => {
      copied = true;
      setTimeout(() => (copied = false), 2000);
    });
  }

  function handleDownloadCsv() {
    const lines = [
      currentSheet.headers.map((h) => `"${h.replace(/"/g, '""')}"`).join(","),
      ...currentSheet.rows.map((row) =>
        row.map((c) => (c === null || c === undefined ? "" : `"${String(c).replace(/"/g, '""')}"`)).join(",")
      ),
    ];
    const blob = new Blob([lines.join("\n")], { type: "text/csv;charset=utf-8;" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `${currentSheet.name || fileName.replace(/\.[^/.]+$/, "")}.csv`;
    a.click();
    URL.revokeObjectURL(url);
  }

  function getColLetter(idx: number): string {
    let name = "";
    let i = idx;
    while (i >= 0) {
      name = String.fromCharCode(65 + (i % 26)) + name;
      i = Math.floor(i / 26) - 1;
    }
    return name;
  }
</script>

<div class="spreadsheet-container" class:dark={theme === "dark"}>
  <!-- Top Toolbar -->
  <div class="spreadsheet-toolbar">
    <div class="toolbar-left">
      <div class="file-badge">
        <FileSpreadsheet size={16} class="excel-icon" />
        <span class="file-name" title={fileName}>{fileName}</span>
      </div>
      <div class="search-box">
        <Search size={14} class="search-icon" />
        <input
          type="text"
          bind:value={searchQuery}
          placeholder={language === "fr" ? "Rechercher dans les cellules..." : "Search cells..."}
          class="search-input"
        />
        {#if searchQuery}
          <button class="clear-search-btn" type="button" on:click={() => (searchQuery = "")}>×</button>
        {/if}
      </div>
    </div>

    <div class="toolbar-right">
      <span class="stats-badge">
        {filteredRows.length} {language === "fr" ? "lignes" : "rows"}
        {#if searchQuery}
          <span class="match-info">({language === "fr" ? "filtrées" : "filtered"})</span>
        {/if}
        • {currentSheet.headers.length} {language === "fr" ? "colonnes" : "cols"}
      </span>

      <button class="action-btn" type="button" on:click={handleCopyTable} title={language === "fr" ? "Copier le tableau" : "Copy table"}>
        {#if copied}
          <Check size={14} style="color: #30d158;" />
          <span style="color: #30d158;">{language === "fr" ? "Copié !" : "Copied!"}</span>
        {:else}
          <Copy size={14} />
          <span>{language === "fr" ? "Copier" : "Copy"}</span>
        {/if}
      </button>

      <button class="action-btn" type="button" on:click={handleDownloadCsv} title={language === "fr" ? "Exporter en CSV" : "Export to CSV"}>
        <Download size={14} />
        <span>CSV</span>
      </button>
    </div>
  </div>

  <!-- Table Body -->
  <div class="spreadsheet-table-wrapper">
    <table class="spreadsheet-table">
      <thead>
        <tr>
          <th class="corner-header">#</th>
          {#each currentSheet.headers as header, cIdx}
            <th class="col-header" class:sorted={sortColIndex === cIdx} on:click={() => toggleSort(cIdx)}>
              <div class="header-content">
                <span class="col-letter">{getColLetter(cIdx)}</span>
                <span class="col-title" title={header}>{header}</span>
                <span class="sort-icon">
                  {#if sortColIndex === cIdx}
                    {#if sortDirection === "asc"}
                      <ArrowUp size={12} />
                    {:else}
                      <ArrowDown size={12} />
                    {/if}
                  {:else}
                    <ArrowUpDown size={11} class="sort-neutral" />
                  {/if}
                </span>
              </div>
            </th>
          {/each}
        </tr>
      </thead>
      <tbody>
        {#if filteredRows.length === 0}
          <tr>
            <td colspan={Math.max(currentSheet.headers.length + 1, 1)} class="empty-cell">
              {searchQuery
                ? (language === "fr" ? "Aucune ligne ne correspond à la recherche." : "No rows match the search.")
                : (language === "fr" ? "Feuille vide." : "Empty sheet.")}
            </td>
          </tr>
        {:else}
          {#each filteredRows as row, rIdx}
            <tr class:even-row={rIdx % 2 === 1}>
              <td class="row-index">{rIdx + 1}</td>
              {#each currentSheet.headers as _, cIdx}
                {@const cellVal = row[cIdx]}
                <td
                  class="data-cell"
                  class:num-cell={typeof cellVal === "number"}
                  class:bool-cell={typeof cellVal === "boolean"}
                  class:null-cell={cellVal === null || cellVal === undefined}
                >
                  {#if typeof cellVal === "boolean"}
                    <span class="bool-pill" class:true-val={cellVal}>{cellVal ? "TRUE" : "FALSE"}</span>
                  {:else if typeof cellVal === "number"}
                    {cellVal.toLocaleString(undefined, { maximumFractionDigits: 4 })}
                  {:else if cellVal !== null && cellVal !== undefined}
                    {cellVal}
                  {:else}
                    <span class="null-val">—</span>
                  {/if}
                </td>
              {/each}
            </tr>
          {/each}
        {/if}
      </tbody>
    </table>
  </div>

  <!-- Bottom Sheets Bar (if multiple sheets) -->
  {#if data.sheets.length > 1}
    <div class="sheets-bar">
      {#each data.sheets as sheet, idx}
        <button
          class="sheet-tab-btn"
          class:active={activeSheetIdx === idx}
          type="button"
          on:click={() => {
            activeSheetIdx = idx;
            sortColIndex = null;
          }}
        >
          <span class="sheet-indicator"></span>
          <span>{sheet.name}</span>
          <span class="sheet-count">({sheet.rows.length})</span>
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .spreadsheet-container {
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

  .spreadsheet-container.dark {
    background: #1c1c1e;
    color: #f5f5f7;
  }

  .spreadsheet-toolbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px 16px;
    border-bottom: 1px solid rgba(0, 0, 0, 0.08);
    background: rgba(245, 245, 247, 0.85);
    backdrop-filter: blur(12px);
    gap: 12px;
  }

  .dark .spreadsheet-toolbar {
    background: rgba(30, 30, 35, 0.85);
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  }

  .toolbar-left {
    display: flex;
    align-items: center;
    gap: 12px;
    flex: 1;
  }

  .file-badge {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    font-weight: 600;
    max-width: 220px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  :global(.excel-icon) {
    color: #107c41;
    flex-shrink: 0;
  }

  .search-box {
    position: relative;
    display: flex;
    align-items: center;
    max-width: 240px;
    width: 100%;
  }

  .search-box :global(.search-icon) {
    position: absolute;
    left: 8px;
    color: #86868b;
    pointer-events: none;
  }

  .search-input {
    width: 100%;
    padding: 5px 24px 5px 28px;
    border-radius: 8px;
    border: 1px solid rgba(0, 0, 0, 0.12);
    background: #ffffff;
    font-size: 12px;
    outline: none;
    color: inherit;
    transition: all 0.15s;
  }

  .dark .search-input {
    background: rgba(255, 255, 255, 0.06);
    border-color: rgba(255, 255, 255, 0.12);
  }

  .search-input:focus {
    border-color: #0071e3;
    box-shadow: 0 0 0 2px rgba(0, 113, 227, 0.2);
  }

  .clear-search-btn {
    position: absolute;
    right: 6px;
    background: transparent;
    border: none;
    cursor: pointer;
    font-size: 14px;
    color: #86868b;
  }

  .toolbar-right {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .stats-badge {
    font-size: 11px;
    color: #86868b;
    font-weight: 500;
    white-space: nowrap;
  }

  .match-info {
    color: #0071e3;
    font-weight: 600;
  }

  .action-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 5px 10px;
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

  .spreadsheet-table-wrapper {
    flex: 1;
    overflow: auto;
    position: relative;
    max-height: calc(100vh - 220px);
  }

  .spreadsheet-table {
    border-collapse: collapse;
    width: 100%;
    font-size: 12px;
  }

  .corner-header {
    position: sticky;
    top: 0;
    left: 0;
    z-index: 3;
    background: #e8e8ed;
    color: #86868b;
    padding: 6px 10px;
    width: 44px;
    font-weight: 600;
    font-size: 11px;
    border-right: 1px solid rgba(0, 0, 0, 0.1);
    border-bottom: 1px solid rgba(0, 0, 0, 0.1);
  }

  .dark .corner-header {
    background: #2c2c2e;
    color: #98989d;
    border-color: rgba(255, 255, 255, 0.08);
  }

  .col-header {
    position: sticky;
    top: 0;
    z-index: 2;
    background: #f2f2f7;
    padding: 6px 12px;
    text-align: left;
    font-weight: 600;
    border-right: 1px solid rgba(0, 0, 0, 0.08);
    border-bottom: 1px solid rgba(0, 0, 0, 0.12);
    cursor: pointer;
    user-select: none;
    transition: background 0.15s;
  }

  .dark .col-header {
    background: #252528;
    border-color: rgba(255, 255, 255, 0.08);
  }

  .col-header:hover {
    background: #e5e5ea;
  }

  .dark .col-header:hover {
    background: #323236;
  }

  .col-header.sorted {
    background: rgba(0, 113, 227, 0.1);
    color: #0071e3;
  }

  .header-content {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .col-letter {
    font-size: 10px;
    color: #86868b;
    font-family: monospace;
  }

  .col-title {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 200px;
  }

  .sort-icon {
    display: flex;
    align-items: center;
  }

  :global(.sort-neutral) {
    color: #aeaeb2;
    opacity: 0.5;
  }

  .row-index {
    position: sticky;
    left: 0;
    z-index: 1;
    background: #f2f2f7;
    color: #86868b;
    text-align: center;
    font-size: 11px;
    font-family: monospace;
    padding: 6px 10px;
    border-right: 1px solid rgba(0, 0, 0, 0.1);
    border-bottom: 1px solid rgba(0, 0, 0, 0.04);
  }

  .dark .row-index {
    background: #252528;
    color: #98989d;
    border-color: rgba(255, 255, 255, 0.08);
  }

  .data-cell {
    padding: 6px 12px;
    border-right: 1px solid rgba(0, 0, 0, 0.06);
    border-bottom: 1px solid rgba(0, 0, 0, 0.05);
    white-space: nowrap;
    max-width: 320px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .dark .data-cell {
    border-color: rgba(255, 255, 255, 0.05);
  }

  .even-row td {
    background: rgba(0, 0, 0, 0.015);
  }

  .dark .even-row td {
    background: rgba(255, 255, 255, 0.015);
  }

  .num-cell {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  .bool-pill {
    display: inline-block;
    padding: 2px 6px;
    border-radius: 4px;
    font-size: 10px;
    font-weight: 700;
    background: rgba(255, 69, 58, 0.15);
    color: #ff453a;
  }

  .bool-pill.true-val {
    background: rgba(48, 209, 88, 0.15);
    color: #30d158;
  }

  .null-val {
    color: #8e8e93;
    opacity: 0.5;
  }

  .empty-cell {
    text-align: center;
    padding: 40px;
    color: #86868b;
    font-style: italic;
  }

  .sheets-bar {
    display: flex;
    gap: 4px;
    padding: 6px 12px;
    background: #f2f2f7;
    border-top: 1px solid rgba(0, 0, 0, 0.08);
    overflow-x: auto;
  }

  .dark .sheets-bar {
    background: #252528;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
  }

  .sheet-tab-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 5px 12px;
    border-radius: 6px;
    border: none;
    background: transparent;
    font-size: 12px;
    font-weight: 500;
    color: #86868b;
    cursor: pointer;
    transition: all 0.15s;
  }

  .sheet-tab-btn.active {
    background: #ffffff;
    color: #0071e3;
    font-weight: 600;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.08);
  }

  .dark .sheet-tab-btn.active {
    background: #1c1c1e;
    color: #2997ff;
  }

  .sheet-indicator {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #107c41;
  }

  .sheet-count {
    font-size: 10px;
    opacity: 0.7;
  }
</style>
