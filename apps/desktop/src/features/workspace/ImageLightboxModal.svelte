<script lang="ts">
  import X from "@lucide/svelte/icons/x";
  import Download from "@lucide/svelte/icons/download";
  import ZoomIn from "@lucide/svelte/icons/zoom-in";
  import ZoomOut from "@lucide/svelte/icons/zoom-out";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";

  export let imageUrl: string = "";
  export let title: string = "";
  export let language: "fr" | "en" = "fr";
  export let onClose: () => void = () => {};

  let zoomLevel = 1;

  function handleZoomIn() {
    zoomLevel = Math.min(3, zoomLevel + 0.25);
  }

  function handleZoomOut() {
    zoomLevel = Math.max(0.5, zoomLevel - 0.25);
  }

  function handleResetZoom() {
    zoomLevel = 1;
  }

  async function handleDownload() {
    try {
      const response = await fetch(imageUrl);
      const blob = await response.blob();
      const blobUrl = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = blobUrl;
      a.download = imageUrl.split("/").pop() || "image-aro.png";
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(blobUrl);
    } catch {
      window.open(imageUrl, "_blank");
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") onClose();
  }
</script>

<svelte:window on:keydown={handleKeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="lightbox-overlay" on:click={onClose}>
  <div class="lightbox-toolbar" on:click|stopPropagation>
    {#if title}
      <span class="lightbox-title">{title}</span>
    {/if}

    <div class="lightbox-actions">
      <button class="tool-btn" type="button" title="Zoom In" on:click={handleZoomIn}><ZoomIn size={16} /></button>
      <button class="tool-btn" type="button" title="Zoom Out" on:click={handleZoomOut}><ZoomOut size={16} /></button>
      <button class="tool-btn" type="button" title="Reset Zoom" on:click={handleResetZoom}><RotateCcw size={16} /></button>
      <button class="download-btn" type="button" on:click={handleDownload}>
        <Download size={14} />
        <span>{language === "fr" ? "Télécharger" : "Download"}</span>
      </button>
      <button class="close-btn" type="button" on:click={onClose}><X size={18} /></button>
    </div>
  </div>

  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="lightbox-stage" on:click|stopPropagation>
    <img
      src={imageUrl}
      alt={title}
      style="transform: scale({zoomLevel});"
      class="lightbox-img"
    />
  </div>
</div>

<style>
  .lightbox-overlay {
    position: fixed;
    top: 0; left: 0; right: 0; bottom: 0;
    z-index: 99999;
    background: rgba(0, 0, 0, 0.88);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    display: flex;
    flex-direction: column;
  }

  .lightbox-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 24px;
    background: rgba(15, 23, 42, 0.8);
    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
  }

  .lightbox-title {
    font-size: 0.9rem;
    font-weight: 600;
    color: #f8fafc;
    max-width: 400px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .lightbox-actions {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .tool-btn, .close-btn {
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 8px;
    color: #e2e8f0;
    padding: 6px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.15s ease;
  }

  .tool-btn:hover, .close-btn:hover {
    background: rgba(255, 255, 255, 0.15);
    color: #ffffff;
  }

  .download-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 14px;
    border-radius: 8px;
    background: #3b82f6;
    color: #ffffff;
    border: none;
    font-size: 0.8rem;
    font-weight: 600;
    cursor: pointer;
    transition: transform 0.15s ease;
  }

  .download-btn:hover {
    transform: scale(1.03);
    box-shadow: 0 0 12px rgba(59, 130, 246, 0.5);
  }

  .lightbox-stage {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    padding: 24px;
  }

  .lightbox-img {
    max-width: 90vw;
    max-height: 82vh;
    object-fit: contain;
    border-radius: 12px;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.7);
    transition: transform 0.2s ease;
  }
</style>
