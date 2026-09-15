<script lang="ts">
  import Check from "@lucide/svelte/icons/check";
  import X from "@lucide/svelte/icons/x";
  import FileCode from "@lucide/svelte/icons/file-code";
  import Copy from "@lucide/svelte/icons/copy";
  import { highlightCode } from "../../lib/markdown";
  import { generateSplitDiffRows, type DiffLine, type SplitDiffRow } from "../../lib/diff";
  import { writeWorkspaceFile, applyWorkspaceDiff } from "../../lib/api/transport";

  // Core props
  export let filePath: string = "";
  export let diffLines: DiffLine[] = [];
  export let language: "fr" | "en" = "fr";
  export let onAccept: (() => void | Promise<void>) = () => {};
  export let onReject: (() => void | Promise<void>) = () => {};

  // Interactive extensions (Milestone 2)
  export let viewMode: "unified" | "split" = "unified";
  export let fileLanguage: string | undefined = undefined;
  export let modifiedContent: string | undefined = undefined;
  export let diffPatch: string | undefined = undefined;
  export let conversationId: string | undefined = undefined;
  export let applyLabel: string | undefined = undefined;
  export let rejectLabel: string | undefined = undefined;
  export let copyLabel: string | undefined = undefined;
  export let actionVariant: "accept" | "apply" = "accept";
  export let status: "pending" | "applied" | "rejected" = "pending";

  // Local state
  let copied = false;
  let copyTimeout: ReturnType<typeof setTimeout> | null = null;
  let isApplying = false;
  let isApplied = false;
  let isRejected = false;
  let actionError: string | null = null;
  let mountedUnified = viewMode === "unified";
  let mountedSplit = viewMode === "split";
  $: if (viewMode === "unified") mountedUnified = true;
  $: if (viewMode === "split") mountedSplit = true;

  // Detect code language for syntax highlighting
  function detectLanguage(path: string, explicit: string | undefined = undefined): string {
    if (explicit) return explicit.toLowerCase().trim();
    if (!path) return "text";
    const ext = path.split(".").pop()?.toLowerCase() || "";
    switch (ext) {
      case "ts":
      case "tsx":
        return "ts";
      case "js":
      case "jsx":
        return "js";
      case "svelte":
        return "svelte";
      case "rs":
        return "rust";
      case "py":
        return "python";
      case "json":
        return "json";
      case "html":
        return "html";
      case "css":
        return "css";
      case "sh":
      case "bash":
      case "zsh":
        return "bash";
      case "go":
        return "go";
      case "c":
      case "cpp":
      case "h":
      case "hpp":
        return "cpp";
      case "md":
      case "markdown":
        return "markdown";
      default:
        return ext || "text";
    }
  }

  $: detectedLang = detectLanguage(filePath, fileLanguage);
  $: additionsCount = diffLines ? diffLines.filter((l) => l && l.type === "added").length : 0;
  $: deletionsCount = diffLines ? diffLines.filter((l) => l && l.type === "removed").length : 0;
  $: splitRows = diffLines ? generateSplitDiffRows(diffLines) : [];
  $: currentStatus = isApplied ? "applied" : isRejected ? "rejected" : (status || "pending");

  $: applyButtonLabel = (isApplied || currentStatus === "applied")
    ? (language === "fr" ? "Appliqué" : "Applied")
    : isApplying
    ? (language === "fr" ? "Application..." : "Applying...")
    : applyLabel
    ? applyLabel
    : actionVariant === "apply"
    ? (language === "fr" ? "Appliquer au projet" : "Apply to project")
    : (language === "fr" ? "Accepter" : "Accept");

  $: rejectButtonLabel = (isRejected || currentStatus === "rejected")
    ? (language === "fr" ? "Refusé" : "Rejected")
    : rejectLabel
    ? rejectLabel
    : actionVariant === "apply"
    ? (language === "fr" ? "Rejeter" : "Reject")
    : (language === "fr" ? "Refuser" : "Reject");

  const highlightCache = new Map<string, string>();
  function getCachedHighlight(code: string, lang: string): string {
    if (!code || !lang || lang === "text") return "";
    const key = `${lang}:${code}`;
    let res = highlightCache.get(key);
    if (res === undefined) {
      res = highlightCode(code, lang);
      if (highlightCache.size > 2000) {
        highlightCache.clear();
      }
      highlightCache.set(key, res);
    }
    return res;
  }

  function formatCodeLine(code: string, lang: string): string {
    if (code === undefined || code === null || code === "") return "";
    return getCachedHighlight(code, lang);
  }

  function hasSyntaxTokens(code: string, lang: string): boolean {
    if (!code || !lang || lang === "text") return false;
    const highlighted = getCachedHighlight(code, lang);
    return highlighted.includes('class="hl-');
  }

  function getModifiedCode(): string {
    if (modifiedContent !== undefined && modifiedContent !== null) {
      return modifiedContent;
    }
    if (!diffLines || diffLines.length === 0) return "";
    return diffLines
      .filter((l) => l && l.type !== "removed")
      .map((l) => l.content)
      .join("\n");
  }

  async function handleCopy() {
    try {
      const code = getModifiedCode();
      if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(code);
        copied = true;
        if (copyTimeout) clearTimeout(copyTimeout);
        copyTimeout = setTimeout(() => {
          copied = false;
        }, 2000);
      }
    } catch (err) {
      console.warn("CodeDiffViewer: clipboard copy failed", err);
    }
  }

  async function handleApply() {
    if (isApplying || isApplied || currentStatus === "applied") return;
    isApplying = true;
    actionError = null;

    try {
      if (onAccept) {
        await onAccept();
      }

      if (filePath && filePath.trim() !== "") {
        if (modifiedContent !== undefined) {
          await writeWorkspaceFile(filePath, modifiedContent, conversationId);
        } else if (diffPatch !== undefined) {
          await applyWorkspaceDiff(filePath, diffPatch, conversationId);
        } else if (diffLines && diffLines.length > 0) {
          const content = getModifiedCode();
          await writeWorkspaceFile(filePath, content, conversationId);
        }

        if (typeof window !== "undefined") {
          window.dispatchEvent(
            new CustomEvent("aro:workspace-tree-refresh", {
              detail: { path: filePath, source: "CodeDiffViewer" },
            })
          );
        }
      }

      isApplied = true;
      status = "applied";
    } catch (err: any) {
      console.error("CodeDiffViewer: failed to apply diff", err);
      actionError = err?.message || String(err);
    } finally {
      isApplying = false;
    }
  }

  async function handleReject() {
    if (isApplying) return;
    isRejected = true;
    status = "rejected";
    try {
      if (onReject) {
        await onReject();
      }
    } catch (err) {
      console.error("CodeDiffViewer: failed to reject diff", err);
    }
  }

  function getApplyButtonLabel(): string {
    return applyButtonLabel;
  }

  function getRejectButtonLabel(): string {
    return rejectButtonLabel;
  }
</script>

<div class="code-diff-card glassmorphic-panel" class:applied={currentStatus === "applied"} class:rejected={currentStatus === "rejected"}>
  <!-- HEADER -->
  <div class="diff-header">
    <div class="diff-title-area">
      <FileCode size={16} class="file-icon" />
      <span class="file-path" title={filePath}>{filePath || (language === "fr" ? "Fichier sans titre" : "Untitled file")}</span>
      {#if additionsCount > 0 || deletionsCount > 0}
        <div class="diff-delta-pills">
          {#if additionsCount > 0}
            <span class="delta-pill added">+{additionsCount}</span>
          {/if}
          {#if deletionsCount > 0}
            <span class="delta-pill removed">-{deletionsCount}</span>
          {/if}
        </div>
      {/if}
    </div>

    <!-- VIEW MODE PILL (Segmented control) -->
    <div class="view-mode-pill" role="group" aria-label={language === "fr" ? "Mode d'affichage" : "View mode"}>
      <button
        type="button"
        class="pill-btn"
        class:active={viewMode === "unified"}
        aria-pressed={viewMode === "unified"}
        on:click={() => (viewMode = "unified")}
      >
        <span>{language === "fr" ? "Unifié" : "Unified"}</span>
      </button>
      <button
        type="button"
        class="pill-btn"
        class:active={viewMode === "split"}
        aria-pressed={viewMode === "split"}
        on:click={() => (viewMode = "split")}
      >
        <span>{language === "fr" ? "Scindé" : "Split"}</span>
      </button>
    </div>

    <!-- ACTIONS -->
    <div class="diff-actions">
      <!-- COPY BUTTON -->
      <button
        type="button"
        class="diff-btn copy"
        class:copied
        title={language === "fr" ? "Copier le code modifié dans le presse-papier" : "Copy modified code to clipboard"}
        on:click={handleCopy}
      >
        {#if copied}
          <Check size={13} />
          <span>{language === "fr" ? "Copié !" : "Copied!"}</span>
        {:else}
          <Copy size={13} />
          <span>{copyLabel ?? (language === "fr" ? "Copier" : "Copy")}</span>
        {/if}
      </button>

      <!-- ACCEPT / APPLY BUTTON -->
      <button
        type="button"
        class="diff-btn accept"
        class:applied={isApplied || currentStatus === "applied"}
        disabled={isApplying || isApplied || currentStatus === "applied"}
        on:click={handleApply}
      >
        <Check size={13} />
        <span>{applyButtonLabel}</span>
      </button>

      <!-- REJECT BUTTON -->
      <button
        type="button"
        class="diff-btn reject"
        class:rejected={isRejected || currentStatus === "rejected"}
        disabled={isApplying || isRejected || currentStatus === "rejected"}
        on:click={handleReject}
      >
        <X size={13} />
        <span>{rejectButtonLabel}</span>
      </button>
    </div>
  </div>

  {#if actionError}
    <div class="diff-error-banner">
      <span>{actionError}</span>
    </div>
  {/if}

  <!-- BODY: UNIFIED VIEW -->
  {#if mountedUnified}
    <div class="diff-body unified-view" style:display={viewMode === "unified" ? "block" : "none"}>
      {#each (diffLines || []).filter(Boolean) as line, idx (idx)}
        <div class="diff-row {line.type}">
          <span class="line-num old">{line.oldLineNo ?? ""}</span>
          <span class="line-num new">{line.newLineNo ?? ""}</span>
          <span class="line-sign">{line.type === "added" ? "+" : line.type === "removed" ? "-" : " "}</span>
          <span class="line-code">
            {#if hasSyntaxTokens(line.content, detectedLang)}
              <span class="line-code-raw">{line.content}</span>
              <span class="line-code-formatted" aria-hidden="true">{@html formatCodeLine(line.content, detectedLang)}</span>
            {:else}
              {line.content}
            {/if}
          </span>
        </div>
      {/each}
    </div>
  {/if}

  <!-- BODY: SPLIT VIEW (SIDE-BY-SIDE) -->
  {#if mountedSplit}
    <div class="diff-body split-view" style:display={viewMode === "split" ? "block" : "none"}>
      <!-- Column headers for split view -->
      <div class="split-header-row">
        <div class="split-col-header old-header">
          <span>{language === "fr" ? "Original" : "Original"}</span>
        </div>
        <div class="split-col-header new-header">
          <span>{language === "fr" ? "Modifié" : "Modified"}</span>
        </div>
      </div>

      <!-- Split rows -->
      {#each splitRows as row, idx (idx)}
        <div class="split-row">
          <!-- Left / Old pane -->
          <div class="split-cell old-cell {row.oldLine ? row.oldLine.type : 'empty'}">
            {#if row.oldLine}
              <span class="line-num old">{row.oldLine.lineNo || ""}</span>
              <span class="line-sign">{row.oldLine.type === "removed" ? "-" : " "}</span>
              <span class="line-code">
                {#if hasSyntaxTokens(row.oldLine.content, detectedLang)}
                  <span class="line-code-raw">{row.oldLine.content}</span>
                  <span class="line-code-formatted" aria-hidden="true">{@html formatCodeLine(row.oldLine.content, detectedLang)}</span>
                {:else}
                  {row.oldLine.content}
                {/if}
              </span>
            {:else}
              <span class="line-num empty"></span>
              <span class="line-sign"></span>
              <span class="line-code empty"></span>
            {/if}
          </div>

          <!-- Right / New pane -->
          <div class="split-cell new-cell {row.newLine ? row.newLine.type : 'empty'}">
            {#if row.newLine}
              <span class="line-num new">{row.newLine.lineNo || ""}</span>
              <span class="line-sign">{row.newLine.type === "added" ? "+" : " "}</span>
              <span class="line-code">
                {#if hasSyntaxTokens(row.newLine.content, detectedLang)}
                  <span class="line-code-raw">{row.newLine.content}</span>
                  <span class="line-code-formatted" aria-hidden="true">{@html formatCodeLine(row.newLine.content, detectedLang)}</span>
                {:else}
                  {row.newLine.content}
                {/if}
              </span>
            {:else}
              <span class="line-num empty"></span>
              <span class="line-sign"></span>
              <span class="line-code empty"></span>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .code-diff-card {
    --diff-bg: rgba(255, 255, 255, 0.88);
    --diff-border: rgba(0, 0, 0, 0.08);
    --diff-shadow: 0 8px 32px rgba(0, 0, 0, 0.08);
    --diff-header-bg: rgba(0, 0, 0, 0.03);
    --diff-header-border: rgba(0, 0, 0, 0.06);
    --diff-title-color: #1d1d1f;
    --diff-meta-color: #86868b;
    --diff-code-color: #24292f;
    --diff-gutter-num: #8c959f;
    
    --diff-added-bg: rgba(46, 160, 67, 0.12);
    --diff-added-text: #1a7f37;
    --diff-removed-bg: rgba(248, 81, 73, 0.12);
    --diff-removed-text: #cf222e;
    
    --diff-pill-bg: rgba(0, 0, 0, 0.06);
    --diff-pill-text: #6e7781;
    --diff-pill-active-bg: #ffffff;
    --diff-pill-active-text: #1d1d1f;
    --diff-pill-shadow: 0 1px 3px rgba(0, 0, 0, 0.12);
    
    --diff-empty-cell-bg: rgba(0, 0, 0, 0.02);
    --diff-cell-border: rgba(0, 0, 0, 0.04);

    border-radius: 12px;
    background: var(--diff-bg);
    border: 1px solid var(--diff-border);
    box-shadow: var(--diff-shadow);
    backdrop-filter: blur(20px) saturate(180%);
    -webkit-backdrop-filter: blur(20px) saturate(180%);
    overflow: hidden;
    font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Segoe UI', sans-serif;
    margin: 10px 0;
    transition: all 0.2s ease;
  }

  :global(body.dark-theme) .code-diff-card {
    --diff-bg: rgba(15, 23, 42, 0.92);
    --diff-border: rgba(255, 255, 255, 0.1);
    --diff-shadow: 0 8px 32px rgba(0, 0, 0, 0.35);
    --diff-header-bg: rgba(255, 255, 255, 0.04);
    --diff-header-border: rgba(255, 255, 255, 0.08);
    --diff-title-color: #f1f5f9;
    --diff-meta-color: #94a3b8;
    --diff-code-color: #e2e8f0;
    --diff-gutter-num: #64748b;
    
    --diff-added-bg: rgba(16, 185, 129, 0.14);
    --diff-added-text: #34d399;
    --diff-removed-bg: rgba(239, 68, 68, 0.14);
    --diff-removed-text: #f87171;
    
    --diff-pill-bg: rgba(255, 255, 255, 0.08);
    --diff-pill-text: #94a3b8;
    --diff-pill-active-bg: rgba(255, 255, 255, 0.2);
    --diff-pill-active-text: #ffffff;
    --diff-pill-active-shadow: 0 1px 4px rgba(0, 0, 0, 0.4);
    
    --diff-empty-cell-bg: rgba(255, 255, 255, 0.015);
    --diff-cell-border: rgba(255, 255, 255, 0.06);
  }

  .code-diff-card.applied {
    border-color: rgba(16, 185, 129, 0.3);
  }

  .code-diff-card.rejected {
    opacity: 0.75;
    border-color: rgba(239, 68, 68, 0.3);
  }

  /* HEADER */
  .diff-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 8px 14px;
    background: var(--diff-header-bg);
    border-bottom: 1px solid var(--diff-header-border);
    flex-wrap: wrap;
  }

  .diff-title-area {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    flex: 1 1 auto;
  }

  .file-icon {
    color: #007aff;
    flex-shrink: 0;
  }

  .file-path {
    font-size: 0.84rem;
    font-weight: 600;
    color: var(--diff-title-color);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 320px;
    font-family: 'JetBrains Mono', 'Fira Code', monospace;
  }

  .diff-delta-pills {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    margin-left: 4px;
  }

  .delta-pill {
    font-size: 0.7rem;
    font-weight: 700;
    padding: 1px 6px;
    border-radius: 9999px;
    font-family: 'JetBrains Mono', monospace;
  }

  .delta-pill.added {
    background: rgba(16, 185, 129, 0.15);
    color: #10b981;
  }

  .delta-pill.removed {
    background: rgba(239, 68, 68, 0.15);
    color: #ef4444;
  }

  /* VIEW MODE SEGMENTED PILL */
  .view-mode-pill {
    display: inline-flex;
    align-items: center;
    padding: 2px;
    background: var(--diff-pill-bg);
    border-radius: 8px;
    user-select: none;
  }

  .pill-btn {
    border: none;
    background: transparent;
    color: var(--diff-pill-text);
    font-size: 0.72rem;
    font-weight: 500;
    padding: 3px 10px;
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .pill-btn.active {
    background: var(--diff-pill-active-bg);
    color: var(--diff-pill-active-text);
    box-shadow: var(--diff-pill-active-shadow);
    font-weight: 600;
  }

  /* ACTIONS */
  .diff-actions {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .diff-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 4px 10px;
    border-radius: 6px;
    font-size: 0.75rem;
    font-weight: 600;
    cursor: pointer;
    border: 1px solid transparent;
    transition: all 0.15s ease;
    white-space: nowrap;
  }

  .diff-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .diff-btn.copy {
    background: rgba(0, 122, 255, 0.08);
    color: #007aff;
    border-color: rgba(0, 122, 255, 0.2);
  }

  .diff-btn.copy:hover:not(:disabled) {
    background: #007aff;
    color: #ffffff;
  }

  .diff-btn.copy.copied {
    background: rgba(16, 185, 129, 0.2);
    color: #10b981;
    border-color: rgba(16, 185, 129, 0.4);
  }

  .diff-btn.accept {
    background: rgba(16, 185, 129, 0.2);
    color: #10b981;
    border-color: rgba(16, 185, 129, 0.4);
  }

  .diff-btn.accept:hover:not(:disabled) {
    background: #10b981;
    color: #ffffff;
  }

  .diff-btn.accept.applied {
    background: rgba(16, 185, 129, 0.15);
    color: #10b981;
  }

  .diff-btn.reject {
    background: rgba(239, 68, 68, 0.15);
    color: #ef4444;
    border-color: rgba(239, 68, 68, 0.3);
  }

  .diff-btn.reject:hover:not(:disabled) {
    background: #ef4444;
    color: #ffffff;
  }

  .diff-btn.reject.rejected {
    background: rgba(239, 68, 68, 0.1);
    color: #ef4444;
  }

  .diff-error-banner {
    padding: 6px 12px;
    background: rgba(239, 68, 68, 0.15);
    color: #ef4444;
    font-size: 0.75rem;
    border-bottom: 1px solid rgba(239, 68, 68, 0.3);
  }

  /* BODY */
  .diff-body {
    padding: 6px 0;
    overflow-x: auto;
    font-size: 0.76rem;
    line-height: 1.5;
    font-family: 'JetBrains Mono', 'Fira Code', 'Consolas', monospace;
  }

  /* UNIFIED VIEW */
  .diff-row {
    display: flex;
    align-items: flex-start;
    padding: 1px 8px;
    min-height: 1.35rem;
  }

  .diff-row.added {
    background: var(--diff-added-bg);
    color: var(--diff-added-text);
  }

  .diff-row.removed {
    background: var(--diff-removed-bg);
    color: var(--diff-removed-text);
  }

  .diff-row.context {
    color: var(--diff-code-color);
  }

  .line-num {
    width: 34px;
    text-align: right;
    color: var(--diff-gutter-num);
    user-select: none;
    font-size: 0.7rem;
    padding-right: 6px;
    flex-shrink: 0;
  }

  .line-sign {
    width: 14px;
    font-weight: 700;
    user-select: none;
    flex-shrink: 0;
    text-align: center;
  }

  .line-code {
    flex: 1;
    white-space: pre-wrap;
    word-break: break-all;
    min-height: 1.25rem;
    position: relative;
  }

  .line-code-raw {
    position: absolute;
    opacity: 0;
    pointer-events: none;
    width: 0;
    height: 0;
    overflow: hidden;
  }

  .line-code-formatted {
    display: inline;
  }

  /* SPLIT VIEW */
  .split-header-row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    border-bottom: 1px solid var(--diff-header-border);
    background: var(--diff-header-bg);
    font-size: 0.72rem;
    font-weight: 600;
    color: var(--diff-meta-color);
    user-select: none;
  }

  .split-col-header {
    padding: 4px 12px;
  }

  .split-col-header.old-header {
    border-right: 1px solid var(--diff-header-border);
  }

  .split-row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    border-bottom: 1px solid var(--diff-cell-border);
  }

  .split-cell {
    display: flex;
    align-items: flex-start;
    padding: 1px 8px;
    min-height: 1.35rem;
  }

  .split-cell.old-cell {
    border-right: 1px solid var(--diff-header-border);
  }

  .split-cell.removed {
    background: var(--diff-removed-bg);
    color: var(--diff-removed-text);
  }

  .split-cell.added {
    background: var(--diff-added-bg);
    color: var(--diff-added-text);
  }

  .split-cell.context {
    color: var(--diff-code-color);
  }

  .split-cell.empty {
    background: var(--diff-empty-cell-bg);
  }

  /* SYNTAX HIGHLIGHTING CLASSES */
  :global(.hl-keyword) {
    color: #c678dd;
    font-weight: 600;
  }
  :global(.hl-string) {
    color: #98c379;
  }
  :global(.hl-number) {
    color: #d19a66;
  }
  :global(.hl-function) {
    color: #61afef;
  }
  :global(.hl-comment) {
    color: #5c6370;
    font-style: italic;
  }
  :global(.hl-attr) {
    color: #e5c07b;
  }
  :global(.hl-key) {
    color: #e06c75;
  }

  :global(body:not(.dark-theme)) :global(.hl-keyword) {
    color: #cf222e;
  }
  :global(body:not(.dark-theme)) :global(.hl-string) {
    color: #0a3069;
  }
  :global(body:not(.dark-theme)) :global(.hl-number) {
    color: #0550ae;
  }
  :global(body:not(.dark-theme)) :global(.hl-function) {
    color: #8250df;
  }
  :global(body:not(.dark-theme)) :global(.hl-comment) {
    color: #6e7781;
  }
  :global(body:not(.dark-theme)) :global(.hl-attr) {
    color: #953800;
  }
  :global(body:not(.dark-theme)) :global(.hl-key) {
    color: #116329;
  }
</style>
