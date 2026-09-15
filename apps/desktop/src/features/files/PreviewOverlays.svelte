<script lang="ts">
  import FileText from "@lucide/svelte/icons/file-text";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import X from "@lucide/svelte/icons/x";

  type PreviewFile = {
    name: string;
    mimeType: string;
    url: string;
    content?: string;
  };

  export let file: PreviewFile | null;
  export let codeOpen: boolean;
  export let codeHtml: string;
  export let loading: boolean;
  export let language: "fr" | "en";
  export let theme: "light" | "dark";
  export let onCloseFile: () => void;
  export let onCloseCode: () => void;

  $: codeDocument = `<!DOCTYPE html><html><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; img-src data:"><meta name="viewport" content="width=device-width, initial-scale=1.0"><style>body { margin: 0; padding: 0; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; }</style></head><body>${codeHtml}</body></html>`;
</script>

{#if file}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="preview-modal-overlay" on:click={onCloseFile}>
    <div class="preview-modal" on:click|stopPropagation>
      <header class="preview-modal-header">
        <span class="preview-modal-title" title={file.name}>{file.name}</span>
        <button class="preview-modal-close-btn" type="button" on:click={onCloseFile}>
          <X size={16} />
        </button>
      </header>
      <div class="preview-modal-body">
        {#if file.mimeType.startsWith("image/")}
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
