<script lang="ts">
  import { fade } from "svelte/transition";

  export let dark = false;
  export let progress = 0;
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="splash-screen"
  class:dark-theme={dark}
  transition:fade={{ duration: 500 }}
  on:click|stopPropagation
>
  <div class="splash-aurora aurora-1"></div>
  <div class="splash-aurora aurora-2"></div>
  <div class="splash-aurora aurora-3"></div>

  <div class="splash-content">
    <div class="splash-logo-container">
      <div class="splash-icon-frame">
        <img src="/logo.png" alt="ARO Logo" class="splash-logo" />
        <div class="splash-icon-glass"></div>
      </div>
    </div>
    <h1 class="splash-title">ARO</h1>
    <p class="splash-subtitle">L'assistant IA local & privé</p>

    <div class="splash-progress-track">
      <div class="splash-progress-bar" style="width: {progress}%;"></div>
    </div>
  </div>

  <div class="splash-footer">
    <span>Designed by ARO • Local Runtime</span>
  </div>
</div>
