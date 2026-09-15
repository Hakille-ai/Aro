<script lang="ts">
  import Cpu from "@lucide/svelte/icons/cpu";
  import AlertTriangle from "@lucide/svelte/icons/alert-triangle";
  import Zap from "@lucide/svelte/icons/zap";

  export let usedTokens: number = 0;
  export let maxTokens: number = 128000;
  export let language: "fr" | "en" = "fr";

  $: percentage = Math.min(100, Math.round((usedTokens / maxTokens) * 100));
  $: isWarning = percentage >= 80;
  $: isCritical = percentage >= 92;

  function formatNumber(num: number): string {
    if (num >= 1000) return `${(num / 1000).toFixed(1)}k`;
    return num.toString();
  }
</script>

<div class="token-gauge-container glassmorphic-panel" class:warning={isWarning} class:critical={isCritical}>
  <div class="gauge-header">
    <div class="gauge-title">
      {#if isCritical}
        <AlertTriangle size={14} class="gauge-icon critical" />
      {:else}
        <Cpu size={14} class="gauge-icon" />
      {/if}
      <span>{language === "fr" ? "Fenêtre de Contexte" : "Context Window"}</span>
    </div>

    <div class="gauge-val">
      <span class="tokens-num">{formatNumber(usedTokens)}</span>
      <span class="tokens-max">/ {formatNumber(maxTokens)} tokens ({percentage}%)</span>
    </div>
  </div>

  <div class="gauge-track">
    <div
      class="gauge-fill"
      style="width: {percentage}%;"
      class:fill-warning={isWarning}
      class:fill-critical={isCritical}
    ></div>
  </div>

  {#if isWarning}
    <div class="gauge-hint">
      <Zap size={12} />
      <span>{language === "fr" ? "Contexte élevé : résumé automatique recommandé" : "High context: auto-summarize recommended"}</span>
    </div>
  {/if}
</div>

<style>
  .token-gauge-container {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px 12px;
    border-radius: 10px;
    background: rgba(15, 23, 42, 0.85);
    border: 1px solid rgba(255, 255, 255, 0.08);
    backdrop-filter: blur(8px);
    -webkit-backdrop-filter: blur(8px);
    font-size: 0.76rem;
  }

  .token-gauge-container.warning {
    border-color: rgba(245, 158, 11, 0.4);
  }

  .token-gauge-container.critical {
    border-color: rgba(239, 68, 68, 0.5);
    background: rgba(239, 68, 68, 0.08);
  }

  .gauge-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .gauge-title {
    display: flex;
    align-items: center;
    gap: 6px;
    font-weight: 600;
    color: #cbd5e1;
  }

  :global(.gauge-icon) {
    color: #3b82f6;
  }

  :global(.gauge-icon.critical) {
    color: #ef4444;
  }

  .gauge-val {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .tokens-num {
    font-weight: 700;
    color: #f8fafc;
  }

  .tokens-max {
    color: #64748b;
    font-size: 0.7rem;
  }

  .gauge-track {
    height: 5px;
    border-radius: 3px;
    background: rgba(255, 255, 255, 0.08);
    overflow: hidden;
  }

  .gauge-fill {
    height: 100%;
    border-radius: 3px;
    background: linear-gradient(90deg, #3b82f6, #60a5fa);
    transition: width 0.3s ease;
  }

  .fill-warning {
    background: linear-gradient(90deg, #f59e0b, #fbbf24);
  }

  .fill-critical {
    background: linear-gradient(90deg, #ef4444, #f87171);
  }

  .gauge-hint {
    display: flex;
    align-items: center;
    gap: 4px;
    color: #f59e0b;
    font-size: 0.68rem;
    font-weight: 500;
  }
</style>
