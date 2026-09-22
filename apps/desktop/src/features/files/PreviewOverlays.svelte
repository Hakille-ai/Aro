<script lang="ts">
  import FileText from "@lucide/svelte/icons/file-text";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import X from "@lucide/svelte/icons/x";
  import AlertCircle from "@lucide/svelte/icons/alert-circle";
  import Download from "@lucide/svelte/icons/download";
  import SpreadsheetViewer from "./SpreadsheetViewer.svelte";
  import WordDocumentViewer from "./WordDocumentViewer.svelte";
  import MediaViewer from "./MediaViewer.svelte";
  import CodeMarkdownViewer from "./CodeMarkdownViewer.svelte";
  import { parseExcelArrayBuffer, parseCsvText, type SpreadsheetData } from "../../lib/documents/excel-parser";
  import { parseWordDocxArrayBuffer, type WordDocumentData } from "../../lib/documents/word-parser";
  import type { PreviewFile } from "../../lib/types";

  export let file: PreviewFile | null;
  export let codeOpen: boolean;
  export let codeHtml: string;
  export let loading: boolean;
  export let language: "fr" | "en";
  export let theme: "light" | "dark";
  export let onCloseFile: () => void;
  export let onCloseCode: () => void;

  let spreadsheetData: SpreadsheetData | null = null;
  let wordData: WordDocumentData | null = null;
  let parseError: string | null = null;

  async function resolveBuffer(f: PreviewFile): Promise<ArrayBuffer | Uint8Array | null> {
    if (f.data) return f.data;
    if (f.content && (f.content.startsWith("data:") || f.content.startsWith("UEsD") || f.content.startsWith("0M8R"))) {
      try {
        const b64 = f.content.includes(",") ? f.content.split(",")[1] : f.content;
        const bin = atob(b64);
        const bytes = new Uint8Array(bin.length);
        for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
        return bytes;
      } catch {
        // ignore base64 decode failure
      }
    }
    if (f.url && (f.url.startsWith("http") || f.url.startsWith("blob:") || f.url.startsWith("asset:"))) {
      try {
        const resp = await fetch(f.url);
        return await resp.arrayBuffer();
      } catch {
        // ignore fetch failure
      }
    }
    return null;
  }

  $: {
    spreadsheetData = null;
    wordData = null;
    parseError = null;

    if (file) {
      const isXlsx = /\.(xlsx|xls)$/i.test(file.name) || file.mimeType.includes("spreadsheet");
      const isCsv = /\.(csv|tsv)$/i.test(file.name) || file.mimeType === "text/csv";
      const isDocx = /\.(docx|doc)$/i.test(file.name) || file.mimeType.includes("wordprocessingml");

      if (isXlsx) {
        resolveBuffer(file)
          .then((buf) => {
            if (buf) {
              parseExcelArrayBuffer(buf)
                .then((res) => (spreadsheetData = res))
                .catch((err) => (parseError = String(err?.message || err)));
            } else if (file?.content) {
              spreadsheetData = parseCsvText(file.content, file.name);
            }
          })
          .catch((err) => (parseError = String(err?.message || err)));
      } else if (isCsv) {
        if (file.content) {
          spreadsheetData = parseCsvText(file.content, file.name);
        } else {
          resolveBuffer(file).then((buf) => {
            if (buf) {
              const text = new TextDecoder("utf-8").decode(buf);
              spreadsheetData = parseCsvText(text, file?.name || "Données");
            }
          });
        }
      } else if (isDocx) {
        resolveBuffer(file)
          .then((buf) => {
            if (buf) {
              parseWordDocxArrayBuffer(buf)
                .then((res) => (wordData = res))
                .catch((err) => (parseError = String(err?.message || err)));
            }
          })
          .catch((err) => (parseError = String(err?.message || err)));
      }
    }
  }

  $: isAudioOrVideo = Boolean(
    file &&
      (file.mimeType.startsWith("audio/") ||
        file.mimeType.startsWith("video/") ||
        /\.(mp3|wav|ogg|m4a|aac|flac|mp4|webm|mov)$/i.test(file.name))
  );

  $: isCodeOrMarkdown = Boolean(
    file &&
      file.content !== undefined &&
      !spreadsheetData &&
      !wordData &&
      !isAudioOrVideo &&
      (file.mimeType.startsWith("text/") ||
        file.mimeType === "application/json" ||
        /\.(md|json|txt|py|js|ts|rs|html|css|sql|sh|yaml|yml|xml|toml)$/i.test(file.name))
  );

  $: codeDocument = `<!DOCTYPE html><html><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; img-src data:"><meta name="viewport" content="width=device-width, initial-scale=1.0"><style>body { margin: 0; padding: 0; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; }</style></head><body>${codeHtml}</body></html>`;
</script>

{#if file}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="preview-modal-overlay" on:click={onCloseFile}>
    <div class="preview-modal" class:rich-preview={Boolean(spreadsheetData || wordData || isCodeOrMarkdown || isAudioOrVideo)} on:click|stopPropagation>
      <header class="preview-modal-header">
        <span class="preview-modal-title" title={file.name}>{file.name}</span>
        <button class="preview-modal-close-btn" type="button" on:click={onCloseFile}>
          <X size={16} />
        </button>
      </header>
      <div class="preview-modal-body">
        {#if parseError}
          <div class="preview-error-box">
            <AlertCircle size={44} class="parse-error-icon" />
            <h3 class="parse-error-title">{language === "fr" ? "Impossible d'afficher l'aperçu" : "Unable to display preview"}</h3>
            <p class="parse-error-desc">{parseError}</p>
            {#if file.url}
              <a href={file.url} download={file.name} class="preview-download-btn">
                <Download size={14} />
                <span>{language === "fr" ? "Télécharger le fichier" : "Download file"}</span>
              </a>
            {/if}
          </div>
        {:else if spreadsheetData}
          <SpreadsheetViewer data={spreadsheetData} fileName={file.name} {language} {theme} />
        {:else if wordData}
          <WordDocumentViewer data={wordData} fileName={file.name} {language} {theme} />
        {:else if isAudioOrVideo}
          <MediaViewer url={file.url} fileName={file.name} mimeType={file.mimeType} {theme} />
        {:else if isCodeOrMarkdown && file.content !== undefined}
          <CodeMarkdownViewer content={file.content} fileName={file.name} mimeType={file.mimeType} {language} {theme} />
        {:else if file.mimeType.startsWith("image/")}
          <div class="preview-image-container">
            <img src={file.url} alt={file.name} />
          </div>
        {:else if file.mimeType === "application/pdf"}
          <iframe src={file.url} title={file.name} class="preview-pdf-iframe"></iframe>
        {:else if file.content !== undefined}
          <pre class="preview-text-container"><code>{file.content}</code></pre>
        {:else}
          <div class="preview-unsupported">
            <FileText size={48} />
            <p>{language === "fr" ? "Aperçu non disponible pour ce type de fichier." : "Preview not available for this file type."}</p>
            <a href={file.url} download={file.name} class="preview-download-btn">
              {language === "fr" ? "Télécharger" : "Download"}
            </a>
          </div>
        {/if}
      </div>
    </div>
  </div>
{/if}

{#if codeOpen}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="preview-modal-overlay code-preview-modal-overlay" on:click={onCloseCode}>
    <div class="preview-modal code-preview-modal" on:click|stopPropagation style="display: flex; flex-direction: column;">
      <header class="preview-modal-header" style="display: flex; justify-content: space-between; align-items: center; padding: 14px 20px; border-bottom: 1px solid {theme === 'dark' ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.06)'};">
        <div style="display: flex; align-items: center; gap: 8px;">
          <div style="width: 8px; height: 8px; border-radius: 50%; background: #2997ff;"></div>
          <span class="preview-modal-title" style="font-weight: 600; font-size: 15px;">
            {language === "fr" ? "Aperçu Interactif (HTML/SVG)" : "Interactive Preview (HTML/SVG)"}
          </span>
        </div>
        <button class="preview-modal-close-btn" type="button" on:click={onCloseCode} style="background: transparent; border: none; cursor: pointer; color: inherit; display: flex; align-items: center; justify-content: center; width: 28px; height: 28px; border-radius: 50%;">
          <X size={18} />
        </button>
      </header>
      <div class="preview-modal-body" style="flex-grow: 1; padding: 16px; box-sizing: border-box; background: {theme === 'dark' ? '#1c1c1e' : '#f5f5f7'}; height: calc(100% - 58px); display: flex;">
        <iframe
          srcdoc={codeDocument}
          title="Interactive Render"
          style="width: 100%; height: 100%; border: none; border-radius: 12px; background: white; box-shadow: 0 4px 20px rgba(0,0,0,0.06);"
          sandbox="allow-scripts"
        ></iframe>
      </div>
    </div>
  </div>
{/if}

{#if loading}
  <div class="preview-loading-overlay">
    <div class="preview-loading-spinner">
      <RefreshCw size={24} class="spin-icon" />
      <span>{language === "fr" ? "Chargement de l'aperçu..." : "Loading preview..."}</span>
    </div>
  </div>
{/if}

<style>
  .preview-error-box {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    min-height: 280px;
    padding: 32px;
    text-align: center;
    gap: 12px;
  }

  :global(.parse-error-icon) {
    color: #ff9f0a;
    margin-bottom: 4px;
  }

  .parse-error-title {
    font-size: 16px;
    font-weight: 600;
    margin: 0;
  }

  .parse-error-desc {
    font-size: 13px;
    color: #86868b;
    max-width: 520px;
    line-height: 1.5;
    margin: 0 0 12px 0;
  }
</style>
