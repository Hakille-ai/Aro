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
    background: rgba(15, 23, 42, 0.88);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    border: 1px solid rgba(255, 255, 255, 0.12);
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.4), 0 2px 6px rgba(0, 0, 0, 0.2);
    color: #f8fafc;
    transition: all 0.2s ease;
  }

  :global(body.light-theme) .toast-card {
    background: rgba(255, 255, 255, 0.95);
    border-color: rgba(0, 0, 0, 0.1);
    color: #0f172a;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.12), 0 2px 6px rgba(0, 0, 0, 0.06);
  }

  .toast-card.agent-completed {
    border-color: rgba(16, 185, 129, 0.4);
  }

  .toast-card.error {
    border-color: rgba(239, 68, 68, 0.4);
  }

  .toast-icon-col {
    display: flex;
    align-items: center;
    padding-top: 2px;
  }

  .icon-wrap.success { color: #10b981; }
  .icon-wrap.error { color: #ef4444; }
  .icon-wrap.info { color: #3b82f6; }

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
  }

  .toast-body {
    font-size: 0.78rem;
    color: #94a3b8;
    line-height: 1.4;
    display: -webkit-box;
    line-clamp: 2;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  :global(body.light-theme) .toast-body {
    color: #475569;
  }

  .toast-action-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    margin-top: 6px;
    padding: 4px 10px;
    border-radius: 6px;
    background: rgba(59, 130, 246, 0.15);
    border: 1px solid rgba(59, 130, 246, 0.3);
    color: #3b82f6;
    font-size: 0.72rem;
    font-weight: 600;
    cursor: pointer;
    align-self: flex-start;
    transition: all 0.15s ease;
  }

  .toast-action-btn:hover {
    background: #3b82f6;
    color: #ffffff;
  }

  .toast-close-btn {
    background: transparent;
    border: none;
    color: #64748b;
    cursor: pointer;
    padding: 2px;
    border-radius: 4px;
    transition: all 0.15s ease;
  }

  .toast-close-btn:hover {
    color: #ffffff;
    background: rgba(255, 255, 255, 0.1);
  }
</style>
