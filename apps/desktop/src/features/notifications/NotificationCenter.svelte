<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import Bell from "@lucide/svelte/icons/bell";
  import Check from "@lucide/svelte/icons/check";
  import CheckCheck from "@lucide/svelte/icons/check-check";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import Settings from "@lucide/svelte/icons/settings";
  import Bot from "@lucide/svelte/icons/bot";
  import Clock from "@lucide/svelte/icons/clock";
  import AlertCircle from "@lucide/svelte/icons/alert-circle";
  import Info from "@lucide/svelte/icons/info";
  import ShieldAlert from "@lucide/svelte/icons/shield-alert";
  import Search from "@lucide/svelte/icons/search";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import X from "@lucide/svelte/icons/x";
  import Volume2 from "@lucide/svelte/icons/volume-2";
  import VolumeX from "@lucide/svelte/icons/volume-x";
  import { fly, fade } from "svelte/transition";
  import {
    fetchNotifications,
    markNotificationRead,
    markAllNotificationsRead,
    deleteNotification,
    clearAllNotifications,
    getUnreadNotificationCount,
    playChimeSound,
    type NotificationItem,
    type NotificationKind,
    type NotificationSource,
  } from "./model";

  export let activeOrganizationId: string | null = null;
  export let language: "fr" | "en" = "fr";
  export let soundEnabled: boolean = true;
  export let theme: "light" | "dark" = "light";
  export let onOpenConversation: (id: string) => void = () => {};
  export let onOpenSettingsTab: (tab: string) => void = () => {};

  let isOpen = false;
  let unreadCount = 0;
  let notifications: NotificationItem[] = [];
  let activeTab: "all" | "unread" | "agent" | "routine" = "all";
  let searchQuery = "";
  let pollInterval: ReturnType<typeof setInterval> | null = null;
  let containerRef: HTMLDivElement | null = null;

  let lastOrgId: string | null | undefined = undefined;

  $: isDark = theme === "dark" || (typeof document !== "undefined" && document.body?.classList.contains("dark-theme"));

  async function refresh() {
    try {
      const [items, count] = await Promise.all([
        fetchNotifications({
          organizationId: activeOrganizationId || undefined,
          personalOnly: !activeOrganizationId,
          limit: 100,
        }),
        getUnreadNotificationCount(activeOrganizationId),
      ]);
      // Detect if new unread notification arrived while sound enabled
      if (soundEnabled && count > unreadCount && unreadCount >= 0) {
        playChimeSound();
      }
      notifications = items;
      unreadCount = count;
    } catch (err) {
      console.error("Failed to refresh notifications:", err);
    }
  }

  $: if (activeOrganizationId !== lastOrgId) {
    lastOrgId = activeOrganizationId;
    void refresh();
  }

  function toggleOpen() {
    isOpen = !isOpen;
    if (isOpen) {
      void refresh();
    }
  }

  function handleClickOutside(event: MouseEvent) {
    if (isOpen && containerRef && !containerRef.contains(event.target as Node)) {
      isOpen = false;
    }
  }

  function handleKeyDown(event: KeyboardEvent) {
    if (event.key === "Escape" && isOpen) {
      isOpen = false;
    }
  }

  onMount(() => {
    lastOrgId = activeOrganizationId;
    void refresh();
    pollInterval = setInterval(() => {
      void refresh();
    }, 15000);
    window.addEventListener("click", handleClickOutside);
    window.addEventListener("keydown", handleKeyDown);
  });

  onDestroy(() => {
    if (pollInterval) clearInterval(pollInterval);
    window.removeEventListener("click", handleClickOutside);
    window.removeEventListener("keydown", handleKeyDown);
  });

  $: filteredNotifications = notifications.filter((notif) => {
    // Filter by tab
    if (activeTab === "unread" && notif.status === "read") return false;
    if (activeTab === "agent" && notif.source !== "agent" && notif.kind !== "agent-completion") return false;
    if (activeTab === "routine" && notif.source !== "routine" && notif.kind !== "routine") return false;

    // Filter by search
    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase().trim();
      return (
        notif.title.toLowerCase().includes(query) ||
        notif.body.toLowerCase().includes(query)
      );
    }
    return true;
  });

  async function handleMarkAsRead(id: string) {
    await markNotificationRead(id);
    notifications = notifications.map((n) =>
      n.id === id ? { ...n, status: "read" as const } : n
    );
    unreadCount = Math.max(0, unreadCount - 1);
  }

  async function handleMarkAllRead() {
    await markAllNotificationsRead(activeOrganizationId);
    notifications = notifications.map((n) => ({ ...n, status: "read" as const }));
    unreadCount = 0;
  }

  async function handleDelete(id: string) {
    const wasUnread = notifications.find((n) => n.id === id)?.status === "unread";
    await deleteNotification(id);
    notifications = notifications.filter((n) => n.id !== id);
    if (wasUnread) {
      unreadCount = Math.max(0, unreadCount - 1);
    }
  }

  async function handleClearAll() {
    await clearAllNotifications(activeOrganizationId);
    notifications = [];
    unreadCount = 0;
  }

  function handleAction(notif: NotificationItem) {
    if (notif.status === "unread") {
      void handleMarkAsRead(notif.id);
    }
    if (notif.actionUrl) {
      if (notif.actionUrl.startsWith("conversation:")) {
        const conversationId = notif.actionUrl.replace("conversation:", "");
        onOpenConversation(conversationId);
        isOpen = false;
      } else if (notif.actionUrl.startsWith("http")) {
        window.open(notif.actionUrl, "_blank");
      }
    }
  }

  function formatRelativeTime(dateStr: string): string {
    try {
      const date = new Date(dateStr);
      const now = new Date();
      const diffMs = now.getTime() - date.getTime();
      const diffSec = Math.floor(diffMs / 1000);
      const diffMin = Math.floor(diffSec / 60);
      const diffHours = Math.floor(diffMin / 60);
      const diffDays = Math.floor(diffHours / 24);

      if (language === "fr") {
        if (diffSec < 45) return "À l'instant";
        if (diffMin < 60) return `Il y a ${diffMin} min`;
        if (diffHours < 24) return `Il y a ${diffHours} h`;
        if (diffDays === 1) return "Hier";
        return date.toLocaleDateString("fr-FR", { month: "short", day: "numeric" });
      } else {
        if (diffSec < 45) return "Just now";
        if (diffMin < 60) return `${diffMin}m ago`;
        if (diffHours < 24) return `${diffHours}h ago`;
        if (diffDays === 1) return "Yesterday";
        return date.toLocaleDateString("en-US", { month: "short", day: "numeric" });
      }
    } catch {
      return dateStr;
    }
  }
</script>

<div class="notification-center-wrap" class:dark-theme={isDark} bind:this={containerRef}>
  <!-- Top Bar Bell Button -->
  <button
    type="button"
    class="bell-trigger-btn {isOpen ? 'active' : ''}"
    title={language === "fr" ? "Centre de notifications" : "Notification Center"}
    aria-label={language === "fr" ? "Centre de notifications" : "Notification Center"}
    on:click={toggleOpen}
  >
    <Bell size={16} class="bell-icon" />
    {#if unreadCount > 0}
      <span class="unread-badge-pill" in:fade={{ duration: 150 }}>
        {unreadCount > 99 ? "99+" : unreadCount}
      </span>
    {/if}
  </button>

  <!-- Flyout Notification Center -->
  {#if isOpen}
    <div
      class="notification-flyout"
      class:dark-theme={isDark}
      in:fly={{ y: -8, duration: 200 }}
      out:fade={{ duration: 150 }}
    >
      <!-- Header -->
      <div class="flyout-header">
        <div class="header-left">
          <div class="header-title-wrap">
            <h2 class="flyout-title">{language === "fr" ? "Notifications" : "Notifications"}</h2>
            {#if unreadCount > 0}
              <span class="header-count-pill">{unreadCount} {language === "fr" ? "nouvelles" : "new"}</span>
            {/if}
          </div>
        </div>

        <div class="header-actions">
          {#if soundEnabled}
            <button
              type="button"
              class="header-icon-btn active"
              title={language === "fr" ? "Son activé (cliquez pour tester)" : "Sound enabled (click to test)"}
              on:click={() => playChimeSound()}
            >
              <Volume2 size={15} />
            </button>
          {:else}
            <button
              type="button"
              class="header-icon-btn muted"
              title={language === "fr" ? "Son désactivé" : "Sound muted"}
            >
              <VolumeX size={15} />
            </button>
          {/if}

          {#if unreadCount > 0}
            <button
              type="button"
              class="header-icon-btn"
              title={language === "fr" ? "Tout marquer comme lu" : "Mark all as read"}
              on:click={handleMarkAllRead}
            >
              <CheckCheck size={15} />
            </button>
          {/if}

          {#if notifications.length > 0}
            <button
              type="button"
              class="header-icon-btn danger"
              title={language === "fr" ? "Effacer toutes les notifications" : "Clear all notifications"}
              on:click={handleClearAll}
            >
              <Trash2 size={15} />
            </button>
          {/if}

          <button
            type="button"
            class="header-icon-btn"
            title={language === "fr" ? "Paramètres des notifications" : "Notification settings"}
            on:click={() => {
              isOpen = false;
              onOpenSettingsTab("notifications");
            }}
          >
            <Settings size={15} />
          </button>
        </div>
      </div>

      <!-- Filter Tabs - Authentic Apple Segmented Control (4 Tabs, Zero Scrollbar) -->
      <div class="tabs-wrapper">
        <div class="tabs-bar segmented-control" role="tablist">
          <button
            type="button"
            class="tab-btn {activeTab === 'all' ? 'active' : ''}"
            on:click={() => (activeTab = "all")}
          >
            <span>{language === "fr" ? "Toutes" : "All"}</span>
            <span class="tab-badge">{notifications.length}</span>
          </button>

          <button
            type="button"
            class="tab-btn {activeTab === 'unread' ? 'active' : ''}"
            on:click={() => (activeTab = "unread")}
          >
            <span>{language === "fr" ? "Non lues" : "Unread"}</span>
            {#if unreadCount > 0}
              <span class="tab-badge unread-highlight">{unreadCount}</span>
            {/if}
          </button>

          <button
            type="button"
            class="tab-btn {activeTab === 'agent' ? 'active' : ''}"
            on:click={() => (activeTab = "agent")}
          >
            <Bot size={13} style="margin-right: 3px;" />
            <span>Agents</span>
          </button>

          <button
            type="button"
            class="tab-btn {activeTab === 'routine' ? 'active' : ''}"
            on:click={() => (activeTab = "routine")}
          >
            <Clock size={13} style="margin-right: 3px;" />
            <span>Routines</span>
          </button>
        </div>
      </div>

      <!-- Search Input -->
      <div class="search-box-row">
        <Search size={14} class="search-icon" />
        <input
          type="text"
          class="search-input"
          placeholder={language === "fr" ? "Filtrer les notifications..." : "Search notifications..."}
          bind:value={searchQuery}
        />
        {#if searchQuery}
          <button
            type="button"
            class="clear-search-btn"
            on:click={() => (searchQuery = "")}
          >
            <X size={12} />
          </button>
        {/if}
      </div>

      <!-- Notifications List -->
      <div class="notifications-scroll-list">
        {#if filteredNotifications.length === 0}
          <div class="empty-state">
            <div class="empty-icon-wrap">
              <Bell size={28} class="empty-icon" />
            </div>
            <div class="empty-title">
              {#if activeTab === "unread" && notifications.length > 0}
                {language === "fr" ? "Aucune notification non lue" : "No unread notifications"}
              {:else}
                {language === "fr" ? "Aucune notification" : "No notifications"}
              {/if}
            </div>
            <div class="empty-desc">
              {#if activeTab === "unread" && notifications.length > 0}
                {language === "fr"
                  ? "Toutes vos notifications ont été lues et traitées."
                  : "All your notifications have been read and reviewed."}
              {:else}
                {language === "fr"
                  ? "Tout est à jour. Les alertes d'agents et les routines planifiées apparaîtront ici."
                  : "You're all caught up. Agent completions and scheduled routines will appear here."}
              {/if}
            </div>
          </div>
        {:else}
          {#each filteredNotifications as notif (notif.id)}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
            <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
            <div
              class="notif-item {notif.status === 'unread' ? 'unread' : ''} priority-{notif.priority} {notif.actionUrl ? 'clickable' : ''}"
              role={notif.actionUrl ? "button" : "article"}
              tabindex={notif.actionUrl ? 0 : undefined}
              on:click={() => handleAction(notif)}
              on:keydown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  handleAction(notif);
                }
              }}
            >
              <!-- Kind Indicator Icon -->
              <div class="notif-icon-col kind-{notif.kind}">
                {#if notif.kind === "agent-completion" || notif.source === "agent"}
                  <Bot size={16} />
                {:else if notif.kind === "routine" || notif.source === "routine"}
                  <Clock size={16} />
                {:else if notif.kind === "security"}
                  <ShieldAlert size={16} />
                {:else if notif.kind === "warning" || notif.kind === "error"}
                  <AlertCircle size={16} />
                {:else}
                  <Info size={16} />
                {/if}
              </div>

              <!-- Main Content -->
              <div class="notif-body-col">
                <div class="notif-top-row">
                  <span class="notif-title">{notif.title}</span>
                  <span class="notif-time">{formatRelativeTime(notif.createdAt)}</span>
                </div>

                <div class="notif-message-text">{notif.body}</div>

                <!-- Action Button if applicable -->
                {#if notif.actionUrl}
                  <button
                    type="button"
                    class="notif-action-btn"
                    on:click|stopPropagation={() => handleAction(notif)}
                  >
                    <span>{language === "fr" ? "Consulter" : "View"}</span>
                    <ExternalLink size={12} />
                  </button>
                {/if}
              </div>

              <!-- Item Actions -->
              <div class="notif-actions-col">
                {#if notif.status === "unread"}
                  <button
                    type="button"
                    class="item-action-btn mark-read"
                    title={language === "fr" ? "Marquer comme lu" : "Mark as read"}
                    on:click|stopPropagation={() => handleMarkAsRead(notif.id)}
                  >
                    <Check size={13} />
                  </button>
                {/if}
                <button
                  type="button"
                  class="item-action-btn delete-btn"
                  title={language === "fr" ? "Supprimer" : "Delete"}
                  on:click|stopPropagation={() => handleDelete(notif.id)}
                >
                  <X size={13} />
                </button>
              </div>
            </div>
          {/each}
        {/if}
      </div>

      <!-- Footer Quick Status -->
      <div class="flyout-footer">
        <span class="footer-hint">
          {language === "fr"
            ? "ARO Intelligence • Notification & Delivery Engine"
            : "ARO Intelligence • Notification & Delivery Engine"}
        </span>
      </div>
    </div>
  {/if}
</div>

<style>
  .notification-center-wrap {
    position: relative;
    display: inline-flex;
    align-items: center;
    font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "SF Pro Display", "SF Pro", system-ui, -apple-system, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  }

  /* Bell Trigger Button - Matches 32px Topbar Actions */
  .bell-trigger-btn {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border-radius: 8px;
    background: transparent;
    border: 1px solid transparent;
    color: #555558;
    cursor: pointer;
    transition: all 0.2s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .bell-trigger-btn:hover {
    background: rgba(0, 0, 0, 0.05);
    color: #1d1d1f;
    border-color: rgba(0, 0, 0, 0.06);
    transform: scale(1.04);
  }

  .bell-trigger-btn:active {
    transform: scale(0.96);
  }

  .bell-trigger-btn.active {
    background: rgba(0, 113, 227, 0.12);
    color: #0071e3;
    border-color: rgba(0, 113, 227, 0.25);
  }

  :global(body.dark-theme) .bell-trigger-btn,
  .notification-center-wrap.dark-theme .bell-trigger-btn {
    color: #98989d;
  }

  :global(body.dark-theme) .bell-trigger-btn:hover,
  .notification-center-wrap.dark-theme .bell-trigger-btn:hover {
    background: rgba(255, 255, 255, 0.08);
    color: #f5f5f7;
    border-color: rgba(255, 255, 255, 0.1);
  }

  :global(body.dark-theme) .bell-trigger-btn.active,
  .notification-center-wrap.dark-theme .bell-trigger-btn.active {
    background: rgba(10, 132, 255, 0.18);
    color: #2997ff;
    border-color: rgba(10, 132, 255, 0.35);
  }

  /* Unread Count Badge - Apple Notification Red */
  .unread-badge-pill {
    position: absolute;
    top: -2px;
    right: -2px;
    min-width: 16px;
    height: 16px;
    padding: 0 4px;
    border-radius: 8px;
    background: linear-gradient(135deg, #ff453a 0%, #d70015 100%);
    color: #ffffff;
    font-size: 9.5px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    display: flex;
    align-items: center;
    justify-content: center;
    box-shadow: 0 1px 4px rgba(215, 0, 21, 0.4);
    border: 2px solid #ffffff;
    animation: apple-badge-pulse 2.5s infinite;
  }

  :global(body.dark-theme) .unread-badge-pill,
  .notification-center-wrap.dark-theme .unread-badge-pill {
    border-color: #1c1c1e;
    box-shadow: 0 1px 6px rgba(255, 69, 58, 0.5);
  }

  @keyframes apple-badge-pulse {
    0% { transform: scale(1); box-shadow: 0 1px 4px rgba(215, 0, 21, 0.4); }
    50% { transform: scale(1.08); box-shadow: 0 2px 8px rgba(215, 0, 21, 0.65); }
    100% { transform: scale(1); box-shadow: 0 1px 4px rgba(215, 0, 21, 0.4); }
  }

  /* Notification Popover Panel - Apple Solid Opaque Card (Zero Background Bleed) */
  .notification-flyout {
    position: absolute;
    top: calc(100% + 8px);
    right: 0;
    width: 356px;
    max-width: 90vw;
    height: auto;
    max-height: min(480px, 85vh);
    border-radius: 14px;
    background: #ffffff;
    border: 1px solid rgba(0, 0, 0, 0.09);
    box-shadow:
      0 16px 36px -4px rgba(0, 0, 0, 0.14),
      0 4px 12px -2px rgba(0, 0, 0, 0.06),
      0 0 0 1px rgba(0, 0, 0, 0.04);
    display: flex;
    flex-direction: column;
    z-index: 100000;
    overflow: hidden;
    color: #1d1d1f;
  }

  :global(body.dark-theme) .notification-flyout,
  .notification-flyout.dark-theme {
    background: #1c1c1e;
    border: 1px solid rgba(255, 255, 255, 0.11);
    box-shadow:
      0 20px 48px -4px rgba(0, 0, 0, 0.7),
      0 4px 16px -2px rgba(0, 0, 0, 0.4),
      0 0 0 1px rgba(255, 255, 255, 0.08);
    color: #f5f5f7;
  }

  /* Header - Compact Apple Bar */
  .flyout-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 9px 12px 8px;
    border-bottom: 1px solid rgba(0, 0, 0, 0.06);
    flex-shrink: 0;
  }

  :global(body.dark-theme) .flyout-header,
  .notification-flyout.dark-theme .flyout-header {
    border-bottom-color: rgba(255, 255, 255, 0.08);
  }

  .header-title-wrap {
    display: flex;
    align-items: center;
    gap: 7px;
  }

  .flyout-title {
    font-size: 13.5px;
    font-weight: 600;
    color: #1d1d1f;
    margin: 0;
    letter-spacing: -0.015em;
  }

  :global(body.dark-theme) .flyout-title,
  .notification-flyout.dark-theme .flyout-title {
    color: #f5f5f7;
  }

  .header-count-pill {
    padding: 1px 6.5px;
    border-radius: 8px;
    background: rgba(0, 113, 227, 0.08);
    color: #0071e3;
    font-size: 10px;
    font-weight: 600;
    border: 1px solid rgba(0, 113, 227, 0.16);
  }

  :global(body.dark-theme) .header-count-pill,
  .notification-flyout.dark-theme .header-count-pill {
    background: rgba(10, 132, 255, 0.18);
    color: #2997ff;
    border-color: rgba(10, 132, 255, 0.25);
  }

  .header-actions {
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .header-icon-btn {
    width: 25px;
    height: 25px;
    border-radius: 6px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: transparent;
    border: none;
    color: #6e6e73;
    cursor: pointer;
    transition: all 0.15s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .header-icon-btn:hover {
    background: rgba(0, 0, 0, 0.05);
    color: #1d1d1f;
    transform: scale(1.04);
  }

  .header-icon-btn:active {
    transform: scale(0.96);
  }

  .header-icon-btn.danger:hover {
    background: rgba(255, 59, 48, 0.1);
    color: #ff3b30;
  }

  .header-icon-btn.active {
    color: #0071e3;
  }

  .header-icon-btn.muted {
    color: #8e8e93;
  }

  :global(body.dark-theme) .header-icon-btn,
  .notification-flyout.dark-theme .header-icon-btn {
    color: #98989d;
  }

  :global(body.dark-theme) .header-icon-btn:hover,
  .notification-flyout.dark-theme .header-icon-btn:hover {
    background: rgba(255, 255, 255, 0.09);
    color: #f5f5f7;
  }

  :global(body.dark-theme) .header-icon-btn.danger:hover,
  .notification-flyout.dark-theme .header-icon-btn.danger:hover {
    background: rgba(255, 69, 58, 0.18);
    color: #ff453a;
  }

  :global(body.dark-theme) .header-icon-btn.active,
  .notification-flyout.dark-theme .header-icon-btn.active {
    color: #2997ff;
  }

  /* Filter Tabs - Apple Segmented Control */
  .tabs-wrapper {
    padding: 5px 10px 4px;
    border-bottom: 1px solid rgba(0, 0, 0, 0.05);
    overflow: hidden;
    scrollbar-width: none;
    -ms-overflow-style: none;
    flex-shrink: 0;
  }

  .tabs-wrapper::-webkit-scrollbar {
    display: none !important;
    width: 0 !important;
    height: 0 !important;
  }

  :global(body.dark-theme) .tabs-wrapper,
  .notification-flyout.dark-theme .tabs-wrapper {
    border-bottom-color: rgba(255, 255, 255, 0.06);
  }

  .tabs-bar.segmented-control {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 2px;
    width: 100%;
    box-sizing: border-box;
    background: rgba(118, 118, 128, 0.09);
    border-radius: 7px;
    overflow: hidden;
    overflow-x: hidden;
    scrollbar-width: none;
    -ms-overflow-style: none;
  }

  .tabs-bar.segmented-control::-webkit-scrollbar,
  :global(.segmented-control::-webkit-scrollbar) {
    display: none !important;
    width: 0 !important;
    height: 0 !important;
  }

  :global(body.dark-theme) .tabs-bar.segmented-control,
  .notification-flyout.dark-theme .tabs-bar.segmented-control {
    background: rgba(118, 118, 128, 0.2);
  }

  .tab-btn {
    flex: 1 1 0;
    min-width: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 3px;
    padding: 4px 2px;
    border-radius: 5px;
    font-size: 11px;
    font-weight: 500;
    font-family: inherit;
    color: #636366;
    background: transparent;
    border: none;
    cursor: pointer;
    white-space: nowrap;
    user-select: none;
    transition: all 0.18s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .tab-btn:hover {
    color: #1d1d1f;
  }

  .tab-btn:active {
    transform: scale(0.97);
  }

  .tab-btn.active {
    color: #1d1d1f;
    background: #ffffff;
    font-weight: 600;
    box-shadow: 0 1px 2.5px rgba(0, 0, 0, 0.08), 0 0.5px 1px rgba(0, 0, 0, 0.04);
  }

  :global(body.dark-theme) .tab-btn,
  .notification-flyout.dark-theme .tab-btn {
    color: #8e8e93;
  }

  :global(body.dark-theme) .tab-btn:hover,
  .notification-flyout.dark-theme .tab-btn:hover {
    color: #f5f5f7;
  }

  :global(body.dark-theme) .tab-btn.active,
  .notification-flyout.dark-theme .tab-btn.active {
    color: #ffffff;
    background: rgba(255, 255, 255, 0.18);
    box-shadow: 0 1px 2.5px rgba(0, 0, 0, 0.35);
  }

  .tab-badge {
    padding: 0 4px;
    border-radius: 6px;
    background: rgba(0, 0, 0, 0.05);
    color: #636366;
    font-size: 9px;
    font-weight: 600;
    line-height: 1.3;
  }

  .tab-btn.active .tab-badge {
    background: rgba(0, 0, 0, 0.07);
    color: #1d1d1f;
  }

  :global(body.dark-theme) .tab-badge,
  .notification-flyout.dark-theme .tab-badge {
    background: rgba(255, 255, 255, 0.1);
    color: #aeaeb2;
  }

  :global(body.dark-theme) .tab-btn.active .tab-badge,
  .notification-flyout.dark-theme .tab-btn.active .tab-badge {
    background: rgba(255, 255, 255, 0.18);
    color: #ffffff;
  }

  .tab-badge.unread-highlight {
    background: #ff3b30 !important;
    color: #ffffff !important;
  }

  :global(body.dark-theme) .tab-badge.unread-highlight,
  .notification-flyout.dark-theme .tab-badge.unread-highlight {
    background: #ff453a !important;
    color: #ffffff !important;
  }

  /* Search Input - Compact Apple Spotlight */
  .search-box-row {
    position: relative;
    display: flex;
    align-items: center;
    padding: 5px 10px;
    border-bottom: 1px solid rgba(0, 0, 0, 0.05);
    flex-shrink: 0;
  }

  :global(body.dark-theme) .search-box-row,
  .notification-flyout.dark-theme .search-box-row {
    border-bottom-color: rgba(255, 255, 255, 0.06);
  }

  .search-input {
    width: 100%;
    height: 26px;
    padding: 0 24px 0 27px;
    border-radius: 6px;
    background: rgba(0, 0, 0, 0.035);
    border: 1px solid rgba(0, 0, 0, 0.06);
    color: #1d1d1f;
    font-size: 11.5px;
    outline: none;
    transition: all 0.18s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .search-input::placeholder {
    color: #86868b;
  }

  .search-input:focus {
    background: #ffffff;
    border-color: #0071e3;
    box-shadow: 0 0 0 2.5px rgba(0, 113, 227, 0.14);
  }

  :global(body.dark-theme) .search-input,
  .notification-flyout.dark-theme .search-input {
    background: rgba(255, 255, 255, 0.06);
    border-color: rgba(255, 255, 255, 0.08);
    color: #f5f5f7;
  }

  :global(body.dark-theme) .search-input::placeholder,
  .notification-flyout.dark-theme .search-input::placeholder {
    color: #636366;
  }

  :global(body.dark-theme) .search-input:focus,
  .notification-flyout.dark-theme .search-input:focus {
    background: rgba(255, 255, 255, 0.09);
    border-color: #0a84ff;
    box-shadow: 0 0 0 2.5px rgba(10, 132, 255, 0.22);
  }

  :global(.search-icon) {
    position: absolute;
    left: 18px;
    color: #86868b;
    pointer-events: none;
  }

  :global(body.dark-theme) :global(.search-icon),
  .notification-flyout.dark-theme :global(.search-icon) {
    color: #636366;
  }

  .clear-search-btn {
    position: absolute;
    right: 17px;
    background: rgba(0, 0, 0, 0.06);
    border: none;
    color: #636366;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    transition: all 0.12s ease;
  }

  .clear-search-btn:hover {
    background: rgba(0, 0, 0, 0.12);
    color: #1d1d1f;
  }

  :global(body.dark-theme) .clear-search-btn,
  .notification-flyout.dark-theme .clear-search-btn {
    background: rgba(255, 255, 255, 0.12);
    color: #aeaeb2;
  }

  :global(body.dark-theme) .clear-search-btn:hover,
  .notification-flyout.dark-theme .clear-search-btn:hover {
    background: rgba(255, 255, 255, 0.2);
    color: #ffffff;
  }

  /* Notifications Scroll List - Content-Hugging & Compact */
  .notifications-scroll-list {
    flex: 1 1 auto;
    max-height: 330px;
    min-height: 48px;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 6px 7px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    scrollbar-width: thin;
    scrollbar-color: rgba(0, 0, 0, 0.15) transparent;
  }

  .notifications-scroll-list::-webkit-scrollbar {
    width: 4px;
  }

  .notifications-scroll-list::-webkit-scrollbar-thumb {
    border-radius: 999px;
    background: rgba(0, 0, 0, 0.12);
  }

  .notifications-scroll-list::-webkit-scrollbar-thumb:hover {
    background: rgba(0, 0, 0, 0.22);
  }

  :global(body.dark-theme) .notifications-scroll-list,
  .notification-flyout.dark-theme .notifications-scroll-list {
    scrollbar-color: rgba(255, 255, 255, 0.15) transparent;
  }

  :global(body.dark-theme) .notifications-scroll-list::-webkit-scrollbar-thumb,
  .notification-flyout.dark-theme .notifications-scroll-list::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.14);
  }

  :global(body.dark-theme) .notifications-scroll-list::-webkit-scrollbar-thumb:hover,
  .notification-flyout.dark-theme .notifications-scroll-list::-webkit-scrollbar-thumb:hover {
    background: rgba(255, 255, 255, 0.22);
  }

  /* Item Card - Apple Refined Notification Card */
  .notif-item {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 8px 9px;
    border-radius: 9px;
    background: rgba(0, 0, 0, 0.015);
    border: 1px solid rgba(0, 0, 0, 0.04);
    transition: all 0.16s cubic-bezier(0.16, 1, 0.3, 1);
    position: relative;
    cursor: default;
  }

  .notif-item.clickable {
    cursor: pointer;
  }

  .notif-item:hover {
    background: #ffffff;
    border-color: rgba(0, 0, 0, 0.08);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.04), 0 0.5px 2px rgba(0, 0, 0, 0.02);
    transform: translateY(-0.5px);
  }

  .notif-item:active:hover.clickable {
    transform: translateY(0) scale(0.995);
  }

  :global(body.dark-theme) .notif-item,
  .notification-flyout.dark-theme .notif-item {
    background: rgba(255, 255, 255, 0.025);
    border-color: rgba(255, 255, 255, 0.05);
  }

  :global(body.dark-theme) .notif-item:hover,
  .notification-flyout.dark-theme .notif-item:hover {
    background: rgba(255, 255, 255, 0.065);
    border-color: rgba(255, 255, 255, 0.1);
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.3);
    transform: translateY(-0.5px);
  }

  .notif-item.unread {
    background: rgba(0, 113, 227, 0.035);
    border-color: rgba(0, 113, 227, 0.12);
  }

  .notif-item.unread:hover {
    background: rgba(0, 113, 227, 0.06);
    border-color: rgba(0, 113, 227, 0.18);
  }

  :global(body.dark-theme) .notif-item.unread,
  .notification-flyout.dark-theme .notif-item.unread {
    background: rgba(10, 132, 255, 0.07);
    border-color: rgba(10, 132, 255, 0.2);
  }

  :global(body.dark-theme) .notif-item.unread:hover,
  .notification-flyout.dark-theme .notif-item.unread:hover {
    background: rgba(10, 132, 255, 0.11);
    border-color: rgba(10, 132, 255, 0.3);
  }

  /* Unread Accent Indicator */
  .notif-item.unread::before {
    content: "";
    position: absolute;
    left: 0;
    top: 6px;
    bottom: 6px;
    width: 3px;
    border-radius: 0 2px 2px 0;
    background: #0071e3;
    box-shadow: 0 0 6px rgba(0, 113, 227, 0.4);
  }

  :global(body.dark-theme) .notif-item.unread::before,
  .notification-flyout.dark-theme .notif-item.unread::before {
    background: #0a84ff;
    box-shadow: 0 0 8px rgba(10, 132, 255, 0.5);
  }

  /* Kind Indicator Icon */
  .notif-icon-col {
    width: 28px;
    height: 28px;
    border-radius: 7px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    margin-top: 1px;
    background: rgba(0, 0, 0, 0.04);
    color: #636366;
    transition: transform 0.18s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .notif-item:hover .notif-icon-col {
    transform: scale(1.05);
  }

  :global(body.dark-theme) .notif-icon-col {
    background: rgba(255, 255, 255, 0.08);
    color: #aeaeb2;
  }

  .notif-icon-col.kind-agent-completion {
    background: rgba(52, 199, 89, 0.14);
    color: #28cd41;
  }

  :global(body.dark-theme) .notif-icon-col.kind-agent-completion {
    background: rgba(48, 209, 88, 0.2);
    color: #30d158;
  }

  .notif-icon-col.kind-routine {
    background: rgba(0, 113, 227, 0.12);
    color: #0071e3;
  }

  :global(body.dark-theme) .notif-icon-col.kind-routine {
    background: rgba(10, 132, 255, 0.18);
    color: #2997ff;
  }

  .notif-icon-col.kind-security {
    background: rgba(255, 149, 0, 0.14);
    color: #ff9500;
  }

  :global(body.dark-theme) .notif-icon-col.kind-security {
    background: rgba(255, 159, 10, 0.2);
    color: #ff9f0a;
  }

  .notif-icon-col.kind-warning,
  .notif-icon-col.kind-error {
    background: rgba(255, 59, 48, 0.14);
    color: #ff3b30;
  }

  :global(body.dark-theme) .notif-icon-col.kind-warning,
  :global(body.dark-theme) .notif-icon-col.kind-error {
    background: rgba(255, 69, 58, 0.2);
    color: #ff453a;
  }

  /* Body Content */
  .notif-body-col {
    flex: 1;
    min-width: 0;
  }

  .notif-top-row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 6px;
    margin-bottom: 2px;
  }

  .notif-title {
    font-size: 12px;
    font-weight: 600;
    color: #1d1d1f;
    letter-spacing: -0.01em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  :global(body.dark-theme) .notif-title {
    color: #f5f5f7;
  }

  .notif-time {
    font-size: 10px;
    color: #86868b;
    font-weight: 400;
    flex-shrink: 0;
  }

  :global(body.dark-theme) .notif-time {
    color: #8e8e93;
  }

  .notif-message-text {
    font-size: 11.5px;
    color: #48484a;
    line-height: 1.38;
    word-break: break-word;
  }

  :global(body.dark-theme) .notif-message-text {
    color: #d1d1d6;
  }

  /* Action Button inside notif item */
  .notif-action-btn {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    margin-top: 5px;
    padding: 2.5px 8px;
    border-radius: 5px;
    background: rgba(0, 113, 227, 0.08);
    border: 1px solid rgba(0, 113, 227, 0.16);
    color: #0071e3;
    font-size: 10.5px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.15s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .notif-action-btn:hover {
    background: #0071e3;
    border-color: #0071e3;
    color: #ffffff;
    transform: translateY(-0.5px);
    box-shadow: 0 2px 5px rgba(0, 113, 227, 0.25);
  }

  .notif-action-btn:active {
    transform: translateY(0) scale(0.98);
  }

  :global(body.dark-theme) .notif-action-btn {
    background: rgba(10, 132, 255, 0.12);
    border-color: rgba(10, 132, 255, 0.25);
    color: #2997ff;
  }

  :global(body.dark-theme) .notif-action-btn:hover {
    background: #0a84ff;
    border-color: #0a84ff;
    color: #ffffff;
    box-shadow: 0 2px 6px rgba(10, 132, 255, 0.35);
  }

  /* Item Actions (Check / Delete) */
  .notif-actions-col {
    display: flex;
    flex-direction: column;
    gap: 3px;
    opacity: 0;
    transform: translateX(3px);
    transition: all 0.15s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .notif-item:hover .notif-actions-col {
    opacity: 1;
    transform: translateX(0);
  }

  .item-action-btn {
    width: 22px;
    height: 22px;
    border-radius: 5px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: transparent;
    border: none;
    color: #86868b;
    cursor: pointer;
    transition: all 0.12s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .item-action-btn:hover {
    background: rgba(0, 0, 0, 0.06);
    color: #1d1d1f;
    transform: scale(1.08);
  }

  .item-action-btn:active {
    transform: scale(0.94);
  }

  .item-action-btn.mark-read:hover {
    background: rgba(52, 199, 89, 0.14);
    color: #28cd41;
  }

  .item-action-btn.delete-btn:hover {
    background: rgba(255, 59, 48, 0.12);
    color: #ff3b30;
  }

  :global(body.dark-theme) .item-action-btn {
    color: #8e8e93;
  }

  :global(body.dark-theme) .item-action-btn:hover {
    background: rgba(255, 255, 255, 0.12);
    color: #f5f5f7;
  }

  :global(body.dark-theme) .item-action-btn.mark-read:hover {
    background: rgba(48, 209, 88, 0.2);
    color: #30d158;
  }

  :global(body.dark-theme) .item-action-btn.delete-btn:hover {
    background: rgba(255, 69, 58, 0.2);
    color: #ff453a;
  }

  /* Empty State - Compact & Refined */
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 22px 14px;
    text-align: center;
  }

  .empty-icon-wrap {
    width: 38px;
    height: 38px;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.04);
    display: flex;
    align-items: center;
    justify-content: center;
    margin-bottom: 7px;
  }

  :global(body.dark-theme) .empty-icon-wrap {
    background: rgba(255, 255, 255, 0.06);
  }

  :global(.empty-icon) {
    color: #86868b;
  }

  :global(body.dark-theme) :global(.empty-icon) {
    color: #636366;
  }

  .empty-title {
    font-size: 12px;
    font-weight: 600;
    color: #1d1d1f;
    margin-bottom: 2px;
  }

  :global(body.dark-theme) .empty-title {
    color: #f5f5f7;
  }

  .empty-desc {
    font-size: 10.5px;
    color: #86868b;
    max-width: 230px;
    line-height: 1.38;
  }

  :global(body.dark-theme) .empty-desc {
    color: #8e8e93;
  }

  /* Footer - Compact Status Bar */
  .flyout-footer {
    padding: 6px 12px;
    border-top: 1px solid rgba(0, 0, 0, 0.05);
    text-align: center;
    flex-shrink: 0;
  }

  :global(body.dark-theme) .flyout-footer {
    border-top-color: rgba(255, 255, 255, 0.06);
  }

  .footer-hint {
    font-size: 9.5px;
    color: #8e8e93;
    letter-spacing: 0.01em;
  }

  :global(body.dark-theme) .footer-hint {
    color: #636366;
  }
</style>
