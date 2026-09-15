<script lang="ts">
  import TriangleAlert from "@lucide/svelte/icons/triangle-alert";
  import X from "@lucide/svelte/icons/x";
  import { confirmRequest, resolveConfirm } from "../../lib/confirm";

  export let language: "fr" | "en" = "fr";

  function onBackdropKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") resolveConfirm(false);
  }
</script>

{#if $confirmRequest}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="confirm-backdrop"
    role="presentation"
    on:click={() => resolveConfirm(false)}
    on:keydown={onBackdropKeydown}
  >
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div
      class="confirm-card"
      role="alertdialog"
      aria-modal="true"
      aria-label={$confirmRequest.title}
      tabindex="-1"
      on:click|stopPropagation
    >
      <div class="confirm-head">
        <span class="confirm-icon" class:danger={$confirmRequest.danger}>
          <TriangleAlert size={18} />
        </span>
        <h2>{$confirmRequest.title}</h2>
        <button class="confirm-x" type="button" on:click={() => resolveConfirm(false)} aria-label={language === "fr" ? "Fermer" : "Close"}>
          <X size={15} />
        </button>
      </div>
      {#if $confirmRequest.body}
        <p class="confirm-body">{$confirmRequest.body}</p>
      {/if}
      <div class="confirm-actions">
        <button class="confirm-btn secondary" type="button" on:click={() => resolveConfirm(false)}>
          {$confirmRequest.cancelLabel}
        </button>
        <button
          class="confirm-btn primary"
          class:danger={$confirmRequest.danger}
          type="button"
          on:click={() => resolveConfirm(true)}
        >
          {$confirmRequest.confirmLabel}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .confirm-backdrop {
    position: fixed;
    inset: 0;
    z-index: 99996;
    background: rgba(0, 0, 0, 0.55);
    backdrop-filter: blur(6px);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 20px;
  }

  .confirm-card {
    width: 100%;
    max-width: 400px;
    background: #ffffff;
    border-radius: 14px;
    padding: 18px;
    box-shadow: 0 20px 50px rgba(0, 0, 0, 0.3);
  }

  :global(body.dark-theme) .confirm-card {
    background: #1c1c1e;
    border: 1px solid rgba(255, 255, 255, 0.1);
  }

  .confirm-head {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .confirm-head h2 {
    flex: 1;
    margin: 0;
    font-size: 0.95rem;
    font-weight: 700;
    color: #0f172a;
  }

  :global(body.dark-theme) .confirm-head h2 {
    color: #f8fafc;
  }

  .confirm-icon {
    display: flex;
    color: #3b82f6;
  }

  .confirm-icon.danger {
    color: #dc2626;
  }

  .confirm-x {
    background: transparent;
    border: none;
    color: #94a3b8;
    cursor: pointer;
    padding: 4px;
    border-radius: 6px;
  }

  .confirm-body {
    margin: 10px 0 0;
    font-size: 0.82rem;
    color: #475569;
    line-height: 1.5;
  }

  :global(body.dark-theme) .confirm-body {
    color: #94a3b8;
  }

  .confirm-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
  }

  .confirm-btn {
    padding: 7px 16px;
    border-radius: 8px;
    font-size: 0.82rem;
    font-weight: 600;
    cursor: pointer;
    border: 1px solid transparent;
  }

  .confirm-btn.secondary {
    background: transparent;
    border-color: #cbd5e1;
    color: #334155;
  }

  .confirm-btn.primary {
    background: #3b82f6;
    color: #fff;
  }

  .confirm-btn.primary.danger {
    background: #dc2626;
  }
</style>
