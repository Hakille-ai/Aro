<script lang="ts">
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import Edit2 from "@lucide/svelte/icons/edit-2";
  import FolderIcon from "@lucide/svelte/icons/folder";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import FolderPlus from "@lucide/svelte/icons/folder-plus";
  import MessageSquare from "@lucide/svelte/icons/message-square";
  import MoreHorizontal from "@lucide/svelte/icons/more-horizontal";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import type { Conversation, Folder } from "../../lib/types";

  export let folder: Folder;
  export let conversations: Conversation[];
  export let activeConversation: Conversation | null;
  export let activeSidebarMenuId: string | null;
  export let cloudWriteLocked = false;
  export let isConversationSending: (id: string) => boolean;
  export let formatRelativeTime: (date: string) => string;
  export let onOpenConversation: (c: Conversation) => void;
  export let onRenameFolder: (f: Folder) => void;
  export let onDeleteFolder: (f: Folder) => void;
  export let onRenameConversation: (c: Conversation) => void;
  export let onRemoveConversation: (c: Conversation, e: MouseEvent) => void;
  export let onDropConversationToFolder: (conversationId: string, folderId: string) => void;

  let expanded = true;
  let isDragOver = false;

  function handleDragOver(e: DragEvent) {
    e.preventDefault();
    isDragOver = true;
  }

  function handleDragLeave() {
    isDragOver = false;
  }

  function handleDrop(e: DragEvent) {
    e.preventDefault();
    isDragOver = false;
    const conversationId = e.dataTransfer?.getData("text/plain");
    if (conversationId) {
      onDropConversationToFolder(conversationId, folder.id);
    }
  }

  function handleDragStart(e: DragEvent, conversationId: string) {
    if (e.dataTransfer) {
      e.dataTransfer.setData("text/plain", conversationId);
      e.dataTransfer.effectAllowed = "move";
    }
  }
</script>

<div
  class="folder-tree-node"
  class:drag-over={isDragOver}
  on:dragover={handleDragOver}
  on:dragleave={handleDragLeave}
  on:drop={handleDrop}
  role="region"
  aria-label={folder.name}
>
  <div class="folder-header-row">
    <button class="folder-toggle-btn" type="button" on:click={() => (expanded = !expanded)}>
      {#if expanded}
        <ChevronDown size={11} class="folder-chevron" />
        <FolderOpen size={13} class="folder-icon" style="color: {folder.color || '#818cf8'};" />
      {:else}
        <ChevronRight size={11} class="folder-chevron" />
        <FolderIcon size={13} class="folder-icon" style="color: {folder.color || '#818cf8'};" />
      {/if}
      <span class="folder-name">{folder.name}</span>
      {#if conversations.length > 0}
        <span class="folder-count-badge">{conversations.length}</span>
      {/if}
    </button>

    <div class="folder-actions-menu">
      <button
        class="icon-menu-btn"
        type="button"
        title="Options du dossier"
        on:click={(e) => {
          e.stopPropagation();
          activeSidebarMenuId = activeSidebarMenuId === folder.id ? null : folder.id;
        }}
      >
        <MoreHorizontal size={13} />
      </button>

      {#if activeSidebarMenuId === folder.id}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="sidebar-dropdown" on:mouseleave={() => (activeSidebarMenuId = null)}>
          <button
            type="button"
            disabled={cloudWriteLocked}
            on:click={(e) => {
              e.stopPropagation();
              activeSidebarMenuId = null;
              onRenameFolder(folder);
            }}
          >
            <Edit2 size={12} />
            <span>Renommer dossier</span>
          </button>
          <button
            type="button"
            class="danger"
            disabled={cloudWriteLocked}
            on:click={(e) => {
              e.stopPropagation();
              activeSidebarMenuId = null;
              onDeleteFolder(folder);
            }}
          >
            <Trash2 size={12} />
            <span>Supprimer dossier</span>
          </button>
        </div>
      {/if}
    </div>
  </div>

  {#if expanded}
    <div class="folder-children">
      {#if conversations.length === 0}
        {#if isDragOver}
          <div class="empty-folder-dropzone">
            <FolderPlus size={12} />
            <span>Déposer ici</span>
          </div>
        {/if}
      {:else}
        {#each conversations as conversation}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="conversation-item child-item"
            class:active={activeConversation?.id === conversation.id}
            class:menu-open={activeSidebarMenuId === conversation.id}
            class:working={isConversationSending(conversation.id)}
            draggable="true"
            on:dragstart={(e) => handleDragStart(e, conversation.id)}
            title={conversation.title}
          >
            <button
              class="conversation-main"
              type="button"
              on:click={() => onOpenConversation(conversation)}
            >
              <span class="conversation-icon-wrapper">
                <MessageSquare size={14} />
              </span>
              <span class="conversation-title">{conversation.title}</span>
            </button>
            <div class="conversation-right">
              {#if isConversationSending(conversation.id)}
                <span class="conversation-working-indicator"></span>
              {:else}
                <span class="conversation-meta" class:hidden={activeSidebarMenuId === conversation.id}>
                  {formatRelativeTime(conversation.updatedAt)}
                </span>
              {/if}
              <div class="sidebar-menu-container">
                <button
                  class="conversation-actions"
                  class:active={activeSidebarMenuId === conversation.id}
                  type="button"
                  on:click={(e) => {
                    e.stopPropagation();
                    activeSidebarMenuId = activeSidebarMenuId === conversation.id ? null : conversation.id;
                  }}
                >
                  <MoreHorizontal size={13} />
                </button>
                {#if activeSidebarMenuId === conversation.id}
                  <!-- svelte-ignore a11y_no_static_element_interactions -->
                  <div class="sidebar-dropdown" on:mouseleave={() => (activeSidebarMenuId = null)}>
                    <button
                      type="button"
                      disabled={cloudWriteLocked}
                      on:click={(e) => {
                        e.stopPropagation();
                        activeSidebarMenuId = null;
                        onRenameConversation(conversation);
                      }}
                    >
                      <Edit2 size={12} />
                      <span>Renommer</span>
                    </button>
                    <button
                      type="button"
                      class="danger"
                      disabled={cloudWriteLocked}
                      on:click={(e) => {
                        e.stopPropagation();
                        activeSidebarMenuId = null;
                        onRemoveConversation(conversation, e);
                      }}
                    >
                      <Trash2 size={12} />
                      <span>Supprimer</span>
                    </button>
                  </div>
                {/if}
              </div>
            </div>
          </div>
        {/each}
      {/if}
    </div>
  {/if}
</div>

<style>
  .folder-tree-node {
    margin-bottom: 2px;
    border-radius: 6px;
    transition: background 0.15s ease, box-shadow 0.15s ease;
  }

  .folder-tree-node.drag-over {
    background: rgba(59, 130, 246, 0.1) !important;
    outline: 1.5px dashed #3b82f6 !important;
    outline-offset: -1px;
    box-shadow: 0 2px 8px rgba(59, 130, 246, 0.15);
  }

  .folder-header-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 2px 4px;
    border-radius: 5px;
    position: relative;
    min-height: 24px;
    transition: background 0.12s ease;
  }

  .folder-header-row:hover {
    background: rgba(15, 23, 42, 0.04);
  }

  :global(body.dark-theme) .folder-header-row:hover {
    background: rgba(255, 255, 255, 0.04);
  }

  .folder-toggle-btn {
    display: flex;
    align-items: center;
    gap: 5px;
    background: transparent;
    border: none;
    color: #334155;
    font-size: 0.78rem;
    font-weight: 500;
    cursor: pointer;
    flex: 1;
    text-align: left;
    overflow: hidden;
    letter-spacing: -0.01em;
    padding: 1px 0;
  }

  :global(body.dark-theme) .folder-toggle-btn {
    color: #cbd5e1;
  }

  :global(.folder-chevron) {
    color: #94a3b8;
    flex-shrink: 0;
    transition: transform 0.15s ease;
  }

  :global(.folder-icon) {
    flex-shrink: 0;
    transition: transform 0.15s ease;
  }

  .folder-name {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .folder-count-badge {
    font-size: 0.62rem;
    font-weight: 600;
    padding: 0 5px;
    border-radius: 999px;
    background: rgba(15, 23, 42, 0.06);
    color: #64748b;
    margin-left: auto;
    font-variant-numeric: tabular-nums;
  }

  :global(body.dark-theme) .folder-count-badge {
    background: rgba(255, 255, 255, 0.08);
    color: #94a3b8;
  }

  .folder-actions-menu {
    position: relative;
    display: flex;
    align-items: center;
  }

  .icon-menu-btn {
    background: transparent;
    border: none;
    color: #94a3b8;
    padding: 2px;
    border-radius: 4px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    opacity: 0;
    transition: opacity 0.12s ease, background 0.12s ease, color 0.12s ease;
  }

  .folder-header-row:hover .icon-menu-btn,
  .folder-actions-menu:focus-within .icon-menu-btn {
    opacity: 1;
  }

  .icon-menu-btn:hover {
    color: #0f172a;
    background: rgba(15, 23, 42, 0.08);
  }

  :global(body.dark-theme) .icon-menu-btn {
    color: #8e8e93;
  }

  :global(body.dark-theme) .icon-menu-btn:hover {
    color: #ffffff;
    background: rgba(255, 255, 255, 0.1);
  }

  .folder-children {
    padding-left: 8px;
    margin-left: 6px;
    border-left: 1px solid rgba(148, 163, 184, 0.18);
    margin-top: 1px;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  :global(body.dark-theme) .folder-children {
    border-left-color: rgba(255, 255, 255, 0.07);
  }

  .empty-folder-dropzone {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 0.72rem;
    font-weight: 500;
    color: #3b82f6;
    padding: 3px 6px;
    border-radius: 5px;
    background: rgba(59, 130, 246, 0.08);
    border: 1px dashed #3b82f6;
    margin: 1px 0;
  }

  .child-item {
    font-size: 0.76rem;
    padding: 0 6px;
    height: 26px;
    min-height: 26px;
    border-radius: 5px;
  }

  .child-item.active {
    background: rgba(0, 0, 0, 0.05) !important;
    border-color: rgba(0, 0, 0, 0.06) !important;
  }

  .child-item.active::before {
    content: "";
    position: absolute;
    left: 0;
    top: 4px;
    bottom: 4px;
    width: 2.5px;
    border-radius: 99px;
    background: #0071e3;
  }

  .child-item.active .conversation-title {
    color: #0f172a !important;
    font-weight: 550;
  }

  .child-item.active .conversation-icon-wrapper {
    color: #0071e3 !important;
    opacity: 1;
  }

  :global(body.dark-theme) .child-item.active {
    background: rgba(255, 255, 255, 0.08) !important;
    border-color: rgba(255, 255, 255, 0.08) !important;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.25) !important;
  }

  :global(body.dark-theme) .child-item.active::before {
    background: #38bdf8;
    box-shadow: 0 0 6px rgba(56, 189, 248, 0.5);
  }

  :global(body.dark-theme) .child-item.active .conversation-title {
    color: #ffffff !important;
    font-weight: 500;
  }

  :global(body.dark-theme) .child-item.active .conversation-icon-wrapper {
    color: #38bdf8 !important;
    opacity: 1;
  }

  .child-item .conversation-actions {
    opacity: 0;
    transition: opacity 0.12s ease;
  }

  .child-item:hover .conversation-actions,
  .child-item .sidebar-menu-container:focus-within .conversation-actions {
    opacity: 1;
  }

  .child-item .conversation-icon-wrapper {
    display: flex;
    align-items: center;
    justify-content: center;
    margin-right: 5px;
    color: #64748b;
    flex-shrink: 0;
    opacity: 0.6;
    transition: opacity 0.12s ease, color 0.12s ease;
  }

  .child-item:hover .conversation-icon-wrapper {
    color: #cbd5e1;
    opacity: 1;
  }
</style>
