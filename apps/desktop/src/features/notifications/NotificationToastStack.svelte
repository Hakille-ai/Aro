<script lang="ts">
  import CheckCircle from "@lucide/svelte/icons/check-circle";
  import Info from "@lucide/svelte/icons/info";
  import AlertTriangle from "@lucide/svelte/icons/alert-triangle";
  import X from "@lucide/svelte/icons/x";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import { fly, fade } from "svelte/transition";
  import type { ToastNotification } from "./model";

  export let notifications: ToastNotification[] = [];
  export let onDismiss: (id: string) => void = () => {};
  export let onOpenConversation: (conversationId: string) => void = () => {};
</script>

<div class="toast-stack-container">
  {#each notifications as toast (toast.id)}
    <div
      class="toast-card glassmorphic-toast"
      class:agent-completed={toast.type === "agent-completed"}
      class:error={toast.type === "error"}
      in:fly={{ y: 20, duration: 250 }}
      out:fade={{ duration: 180 }}
    >
      <div class="toast-icon-col">
        {#if toast.type === "agent-completed" || toast.type === "success"}
          <span class="icon-wrap success"><CheckCircle size={18} /></span>
        {:else if toast.type === "error"}
          <span class="icon-wrap error"><AlertTriangle size={18} /></span>
        {:else}
          <span class="icon-wrap info"><Info size={18} /></span>
        {/if}
      </div>

      <div class="toast-content">
        <div class="toast-title">{toast.title}</div>
        <div class="toast-body">{toast.body}</div>

        {#if toast.conversationId}
          <button
            type="button"
            class="toast-action-btn"
            on:click={() => {
              if (toast.conversationId) onOpenConversation(toast.conversationId);
              onDismiss(toast.id);
            }}
          >
            <ExternalLink size={12} />
            <span>Voir la conversation</span>
          </button>
        {/if}
      </div>

      <button class="toast-close-btn" type="button" on:click={() => onDismiss(toast.id)}>
        <X size={14} />
      </button>
    </div>
  {/each}
</div>

<style>
  .toast-stack-container {
    position: fixed;
    bottom: 24px;
    right: 24px;
    z-index: 99999;
    display: flex;
    flex-direction: column;
    gap: 10px;
    max-width: 380px;
    width: 100%;
    pointer-events: none;
  }

  .toast-card {
    pointer-events: auto;
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 14px 16px;
    border-radius: 14px;
    background: rgba(255, 255, 255, 0.88);
    backdrop-filter: blur(24px) saturate(180%);
    -webkit-backdrop-filter: blur(24px) saturate(180%);
    border: 1px solid rgba(0, 0, 0, 0.08);
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.12), 0 2px 6px rgba(0, 0, 0, 0.04);
    color: #1d1d1f;
    font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "SF Pro Display", "SF Pro", system-ui, -apple-system, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
    transition: all 0.2s cubic-bezier(0.16, 1, 0.3, 1);
  }

  :global(body.dark-theme) .toast-card {
    background: rgba(28, 28, 32, 0.88);
    backdrop-filter: blur(24px) saturate(190%);
    -webkit-backdrop-filter: blur(24px) saturate(190%);
    border: 1px solid rgba(255, 255, 255, 0.12);
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.5), 0 2px 6px rgba(0, 0, 0, 0.3);
    color: #f5f5f7;
  }

  .toast-card.agent-completed {
    border-color: rgba(52, 199, 89, 0.4);
  }

  .toast-card.error {
    border-color: rgba(255, 59, 48, 0.4);
  }

  .toast-icon-col {
    display: flex;
    align-items: center;
    padding-top: 2px;
  }

  .icon-wrap.success { color: #34c759; }
  .icon-wrap.error { color: #ff3b30; }
  .icon-wrap.info { color: #0071e3; }

  :global(body.dark-theme) .icon-wrap.success { color: #30d158; }
  :global(body.dark-theme) .icon-wrap.error { color: #ff453a; }
  :global(body.dark-theme) .icon-wrap.info { color: #2997ff; }

  .toast-content {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .toast-title {
    font-size: 0.85rem;
    font-weight: 600;
    line-height: 1.3;
    color: #1d1d1f;
  }

  :global(body.dark-theme) .toast-title {
    color: #f5f5f7;
  }

  .toast-body {
    font-size: 0.78rem;
    color: #86868b;
    line-height: 1.4;
    display: -webkit-box;
    line-clamp: 2;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  :global(body.dark-theme) .toast-body {
    color: #a1a1a6;
  }

  .toast-action-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    margin-top: 6px;
    padding: 4px 10px;
    border-radius: 6px;
    background: rgba(0, 113, 227, 0.1);
    border: 1px solid rgba(0, 113, 227, 0.2);
    color: #0071e3;
    font-size: 0.72rem;
    font-weight: 600;
    cursor: pointer;
    align-self: flex-start;
    transition: all 0.15s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .toast-action-btn:hover {
    background: #0071e3;
    color: #ffffff;
    transform: translateY(-0.5px);
  }

  :global(body.dark-theme) .toast-action-btn {
    background: rgba(10, 132, 255, 0.16);
    border-color: rgba(10, 132, 255, 0.3);
    color: #2997ff;
  }

  :global(body.dark-theme) .toast-action-btn:hover {
    background: #0a84ff;
    color: #ffffff;
  }

  .toast-close-btn {
    background: transparent;
    border: none;
    color: #86868b;
    cursor: pointer;
    padding: 3px;
    border-radius: 6px;
    transition: all 0.15s ease;
  }

  .toast-close-btn:hover {
    color: #1d1d1f;
    background: rgba(0, 0, 0, 0.06);
  }

  :global(body.dark-theme) .toast-close-btn {
    color: #8e8e93;
  }

  :global(body.dark-theme) .toast-close-btn:hover {
    color: #ffffff;
    background: rgba(255, 255, 255, 0.12);
  }
</style>
