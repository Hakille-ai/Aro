<script lang="ts">
  import Code from "@lucide/svelte/icons/code";
  import Copy from "@lucide/svelte/icons/copy";
  import Check from "@lucide/svelte/icons/check";
  import FileText from "@lucide/svelte/icons/file-text";
  import Loader2 from "@lucide/svelte/icons/loader-2";
  import AlertCircle from "@lucide/svelte/icons/alert-circle";
  import X from "@lucide/svelte/icons/x";
  import { highlightCode, renderMarkdown } from "../../lib/markdown";

  interface ActiveWorkspaceFile {
    relativePath: string;
    rootPath: string;
  }

  export let file: ActiveWorkspaceFile | null = null;
  export let content: string = "";
  export let loading: boolean = false;
  export let error: string | null = null;
  export let truncated: boolean = false;
  export let language: "fr" | "en" = "fr";
  export let onClose: () => void = () => {};
  export let onExamine: (relativePath: string) => void = () => {};

  let copied = false;

  $: relativePath = file?.relativePath ?? "";
  $: rootPath = file?.rootPath ?? "";
  $: crumbs = relativePath ? relativePath.split("/") : [];
  $: fileName = crumbs.length > 0 ? crumbs[crumbs.length - 1] : "";
  $: ext = (() => {
    const parts = fileName.split(".");
    return parts.length > 1 ? parts.pop()!.toLowerCase() : "";
  })();
  $: rootShort = (() => {
    if (!rootPath || rootPath === ".") return language === "fr" ? "racine" : "root";
    const clean = rootPath.replace(/\\/g, "/").replace(/\/$/, "");
    const segs = clean.split("/");
    return segs[segs.length - 1] || rootPath;
  })();
  $: isMarkdown = ext === "md" || ext === "markdown";
  $: isCode = ["ts", "js", "tsx", "jsx", "svelte", "rs", "py", "json", "toml", "yaml", "yml", "css", "scss", "html", "sql", "sh", "bash", "xml", "c", "cpp", "h", "java", "go"].includes(ext);
  $: renderedHtml = (() => {
    if (loading || error || !file) return "";
    try {
      if (isMarkdown) return renderMarkdown(content, language);
      if (isCode) return highlightCode(content, ext);
    } catch {
      return "";
    }
    return "";
  })();

  async function copyPath() {
    if (!relativePath) return;
    try {
      await navigator.clipboard.writeText(relativePath);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch {
      copied = false;
    }
  }
</script>

<div class="file-viewer">
  {#if !file && !loading}
    <div class="viewer-empty">
      <FileText size={22} />
      <span>{language === "fr" ? "Cliquez sur un fichier pour le visualiser ici" : "Click a file to preview it here"}</span>
    </div>
  {:else}
    <div class="viewer-header">
      <div class="crumbs" title={relativePath}>
        <span class="root-badge" title={rootPath}>
          <span class="root-dot"></span>
          {rootShort}
        </span>
        {#each crumbs as seg, i}
          <span class="crumb-sep">/</span>
          <span class="crumb" class:last={i === crumbs.length - 1}>{seg}</span>
        {/each}
      </div>
      <div class="viewer-actions">
        <button type="button" class="icon-btn" title={language === "fr" ? "Copier le chemin" : "Copy path"} on:click={copyPath}>
          {#if copied}<Check size={13} />{:else}<Copy size={13} />{/if}
        </button>
        <button
          type="button"
          class="icon-btn"
          title={language === "fr" ? "Examiner avec l'Agent" : "Examine with Agent"}
          on:click={() => relativePath && onExamine(relativePath)}
        >
          <Code size={13} />
        </button>
        <button type="button" class="icon-btn" title={language === "fr" ? "Fermer" : "Close"} on:click={onClose}>
          <X size={13} />
        </button>
      </div>
    </div>

    <div class="viewer-body">
      {#if loading}
        <div class="viewer-loading">
          <Loader2 size={18} class="spin" />
          <span>{language === "fr" ? "Chargement du fichier…" : "Loading file…"}</span>
        </div>
      {:else if error}
        <div class="viewer-error">
          <AlertCircle size={18} />
          <span>{error}</span>
        </div>
      {:else if isMarkdown}
        <div class="md-content">{@html renderedHtml}</div>
      {:else if isCode}
        <pre class="code-content"><code>{@html renderedHtml}</code></pre>
      {:else}
        <pre class="text-content">{content}</pre>
      {/if}
      {#if truncated && !loading && !error}
        <div class="truncated-note">
          {language === "fr"
            ? "Aperçu tronqué (fichier volumineux)."
            : "Truncated preview (large file)."}
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .file-viewer {
    display: flex;
    flex-direction: column;
    min-height: 0;
    flex: 1;
    background: #ffffff;
    border-radius: 12px;
    border: 1px solid #cbd5e1;
    overflow: hidden;
  }
  :global(body.dark-theme) .file-viewer {
    background: rgba(255, 255, 255, 0.02);
    border-color: rgba(255, 255, 255, 0.06);
  }
  .viewer-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 28px 16px;
    color: #64748b;
    font-size: 0.78rem;
    text-align: center;
  }
  :global(body.dark-theme) .viewer-empty { color: rgba(255, 255, 255, 0.4); }
  .viewer-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border-bottom: 1px solid #e2e8f0;
    background: #f8fafc;
  }
  :global(body.dark-theme) .viewer-header {
    background: rgba(0, 0, 0, 0.2);
    border-bottom-color: rgba(255, 255, 255, 0.06);
  }
  .crumbs {
    flex: 1;
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 2px;
    font-size: 0.74rem;
    min-width: 0;
  }
  .root-badge {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 2px 8px;
    border-radius: 999px;
    background: #eff6ff;
    border: 1px solid #bfdbfe;
    color: #1d4ed8;
    font-weight: 600;
    font-size: 0.7rem;
    max-width: 140px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  :global(body.dark-theme) .root-badge {
    background: rgba(59, 130, 246, 0.12);
    border-color: rgba(59, 130, 246, 0.25);
    color: #60a5fa;
  }
  .root-dot { width: 6px; height: 6px; border-radius: 50%; background: #3b82f6; flex-shrink: 0; }
  .crumb-sep { color: #94a3b8; }
  .crumb { color: #475569; overflow: hidden; text-overflow: ellipsis; max-width: 160px; white-space: nowrap; }
  .crumb.last { color: #0f172a; font-weight: 600; }
  :global(body.dark-theme) .crumb { color: rgba(255, 255, 255, 0.55); }
  :global(body.dark-theme) .crumb.last { color: #f8fafc; }
  .viewer-actions { display: flex; align-items: center; gap: 4px; }
  .icon-btn {
    background: transparent;
    border: none;
    color: #64748b;
    cursor: pointer;
    padding: 5px;
    border-radius: 6px;
    display: flex;
    align-items: center;
  }
  .icon-btn:hover { background: #e2e8f0; color: #0f172a; }
  :global(body.dark-theme) .icon-btn { color: rgba(255, 255, 255, 0.5); }
  :global(body.dark-theme) .icon-btn:hover { background: rgba(255, 255, 255, 0.1); color: #fff; }
  .viewer-body {
    flex: 1;
    overflow-y: auto;
    padding: 10px 12px;
    min-height: 120px;
    max-height: 420px;
  }
  .viewer-loading, .viewer-error {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.78rem;
    color: #64748b;
    padding: 16px 4px;
  }
  .viewer-error { color: #dc2626; }
  :global(body.dark-theme) .viewer-loading { color: rgba(255, 255, 255, 0.5); }
  :global(.spin) { animation: spin 1s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
  .md-content { font-size: 0.8rem; line-height: 1.55; color: #1e293b; word-wrap: break-word; }
  :global(body.dark-theme) .md-content { color: #e2e8f0; }
  .md-content :global(pre) {
    background: #0f172a; color: #e2e8f0; padding: 10px; border-radius: 8px;
    overflow-x: auto; font-size: 0.72rem;
  }
  .md-content :global(code) { font-family: ui-monospace, monospace; }
  .code-content, .text-content {
    margin: 0; font-size: 0.72rem; line-height: 1.5;
    font-family: ui-monospace, SFMono-Regular, monospace;
    white-space: pre-wrap; word-break: break-word;
    color: #1e293b; background: #f8fafc;
    border: 1px solid #e2e8f0; border-radius: 8px; padding: 10px;
  }
  :global(body.dark-theme) .code-content,
  :global(body.dark-theme) .text-content {
    color: #e2e8f0; background: rgba(0, 0, 0, 0.3); border-color: transparent;
  }
  .truncated-note { margin-top: 8px; font-size: 0.7rem; color: #d97706; }
  .code-content :global(.hl-string) { color: #a5d6a7; }
  .code-content :global(.hl-keyword) { color: #c792ea; }
  .code-content :global(.hl-number) { color: #f78c6c; }
  .code-content :global(.hl-key) { color: #82aaff; }
  .code-content :global(.hl-comment) { color: #64748b; font-style: italic; }
  .code-content :global(.hl-function) { color: #ffcb6b; }
</style>
