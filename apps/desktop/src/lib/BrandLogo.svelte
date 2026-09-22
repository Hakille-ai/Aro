<script lang="ts">
  import { onMount } from "svelte";
  import { getBrandLogo, ensureFileLogo, fileLogoUrls } from "./brandLogos";

  export let pluginId: string = "";
  export let icon: string | undefined = undefined;
  export let logo: string | undefined = undefined;
  export let logoKind: string | undefined = undefined;
  export let brandColor: string | undefined = undefined;
  export let serverName: string | undefined = undefined;
  /** Direct data-URL (creation preview): skips the lazy fetch. */
  export let previewDataUrl: string | undefined = undefined;
  export let size: number = 36;
  export let radius: number = 10;

  $: brand = getBrandLogo(pluginId || "", { icon, logo, logoKind, brandColor, serverName });
  $: needsBorder = brand.bg === "#ffffff";
  $: cachedUrl = previewDataUrl || $fileLogoUrls[pluginId] || null;

  onMount(() => {
    if (brand.filePluginId && !previewDataUrl && $fileLogoUrls[pluginId] === undefined) {
      ensureFileLogo(pluginId);
    }
  });

  $: if (brand.filePluginId && !previewDataUrl && $fileLogoUrls[pluginId] === undefined) {
    ensureFileLogo(pluginId);
  }
</script>

<span
  class="brand-tile"
  title={brand.label}
  style="width:{size}px;height:{size}px;border-radius:{radius}px;background:{brand.bg};color:{brand.fg};font-size:{Math.round(
    size * 0.52
  )}px;{needsBorder ? 'border:1px solid rgba(0,0,0,0.1);' : 'border:1px solid rgba(0,0,0,0.06);'}"
>
  {#if cachedUrl}
    <img
      src={cachedUrl}
      alt={brand.label}
      class="brand-img"
      style="border-radius:{Math.max(0, radius - 2)}px;"
      draggable="false"
    />
  {:else if brand.svg}
    <span class="brand-svg" style="width:{Math.round(size * 0.56)}px;height:{Math.round(size * 0.56)}px;">
      {@html brand.svg}
    </span>
  {:else}
    <span class="brand-emoji">{brand.emoji}</span>
  {/if}
</span>

<style>
  .brand-tile {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    overflow: hidden;
    user-select: none;
  }
  .brand-svg {
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .brand-svg :global(svg) {
    width: 100%;
    height: 100%;
    display: block;
  }
  .brand-img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .brand-emoji {
    line-height: 1;
  }
</style>
