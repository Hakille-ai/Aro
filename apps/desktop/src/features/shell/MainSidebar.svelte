<script context="module" lang="ts">
  export type MainSidebarLabels = {
    localAssistant: string;
    hideSidebar: string;
    showSidebar: string;
    newConversation: string;
    searchPlaceholder: string;
    clearSearch: string;
    conversations: string;
    noConversations: string;
    conversationOptions: string;
    rename: string;
    delete: string;
    settings: string;
    cleanEmptyConversations: string;
  };
</script>

<script lang="ts">
  import BrushCleaning from "@lucide/svelte/icons/brush-cleaning";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import ChevronsUpDown from "@lucide/svelte/icons/chevrons-up-down";
  import Database from "@lucide/svelte/icons/database";
  import Download from "@lucide/svelte/icons/download";
  import Edit2 from "@lucide/svelte/icons/edit-2";
  import FolderPlus from "@lucide/svelte/icons/folder-plus";
  import FolderKanban from "@lucide/svelte/icons/folder-kanban";
  import FolderTree from "@lucide/svelte/icons/folder-tree";
  import MessageSquare from "@lucide/svelte/icons/message-square";
  import MoreHorizontal from "@lucide/svelte/icons/more-horizontal";
  import PanelLeftClose from "@lucide/svelte/icons/panel-left-close";
  import PanelLeftOpen from "@lucide/svelte/icons/panel-left-open";
  import Plus from "@lucide/svelte/icons/plus";
  import Search from "@lucide/svelte/icons/search";
  import Settings from "@lucide/svelte/icons/settings";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import X from "@lucide/svelte/icons/x";
  import type {
    CloudSessionView,
    Conversation,
    Folder,
    Project,
    RuntimeStatus,
    SyncStatus,
  } from "../../lib/types";
  import FolderTreeItem from "../folders/FolderTreeItem.svelte";
  import { buildFolderTree } from "../folders/model";
  import { computeProjectStats } from "../projects/model";

  export let sidebarOpen: boolean = true;
  export let onToggleSidebar: () => void = () => {};
  export let labels: MainSidebarLabels;
  export let searchQuery: string;
  export let activeSidebarMenuId: string | null;
  export let showCloudAuthPanel: boolean;
  export let conversations: Conversation[];
  export let projects: Project[] = [];
  export let folders: Folder[] = [];
  export let activeConversation: Conversation | null;
  export let sendingByConversation: Record<string, boolean>;
  export let cloudWriteLocked: boolean;
  export let cloudAuthenticated: boolean;
  export let cloudSyncStatus: SyncStatus;
  export let cloudSession: CloudSessionView | null;
  export let cloudStatusShortLabel: string;
  export let runtime: RuntimeStatus | null;
  export let isConversationSending: (conversationId: string, trigger?: Record<string, boolean>) => boolean;
  export let formatRelativeTime: (value: string) => string;
  export let cloudWriteDisabledTitle: (action?: string) => string | null | undefined;
  export let onStartConversation: () => void | Promise<void>;
  export let onOpenConversation: (conversation: Conversation) => void | Promise<void>;
  export let onRenameConversation: (conversation: Conversation) => void;
  export let onRemoveConversation: (conversation: Conversation, event: MouseEvent) => void | Promise<void>;
  export let onOpenSettings: () => void;
  export let onOpenCreateProjectModal: () => void = () => {};
  export let onOpenCreateFolderModal: (projectId?: string | null) => void = () => {};
  export let onEditProject: (project: Project) => void = () => {};
  export let onDeleteProject: (project: Project) => void = () => {};
  export let onEditFolder: (folder: Folder) => void = () => {};
  export let onDeleteFolder: (folder: Folder) => void = () => {};
  export let onOpenMoveModal: (conversation: Conversation) => void = () => {};
  export let onDropConversationToFolder: (conversationId: string, folderId: string) => void = () => {};
  export let onDropConversationToProject: (conversationId: string, projectId: string) => void = () => {};
  export let onMoveConversation: (conversationId: string, projectId: string | null, folderId: string | null) => void = () => {};
  export let onExportProject: (project: Project) => void = () => {};
  export let onCleanEmptyConversations: () => void = () => {};

  const FALLBACK_PROJECT_COLORS = [
    "#38bdf8", // Sky blue
    "#818cf8", // Indigo
    "#f472b6", // Pink
    "#fb923c", // Orange
    "#34d399", // Emerald
    "#a78bfa", // Violet
    "#f43f5e", // Rose
    "#06b6d4", // Cyan
  ];

  function getProjectColor(proj: Project, index: number = 0): string {
    const raw = (proj.color || "").trim().toLowerCase();
    const isInvalid = !raw ||
      raw === "#000000" || raw === "#000" || raw === "black" ||
      raw === "#ffffff" || raw === "#fff" || raw === "white" ||
      raw === "transparent" || raw === "none" ||
      raw === "#12141a" || raw === "#1a1a1a" || raw === "#1e1e20" ||
      raw === "#0f172a" || raw === "rgb(0,0,0)" || raw === "rgb(255,255,255)";
    if (isInvalid) {
      return FALLBACK_PROJECT_COLORS[index % FALLBACK_PROJECT_COLORS.length];
    }
    return proj.color;
  }

  let expandedProjects: Record<string, boolean> = {};
  let dragOverProjectId: string | null = null;
  let dragOverUnassigned = false;

  $: projectStats = computeProjectStats(projects, folders, conversations);

  $: globalFolders = folders.filter((f) => !f.projectId);
  $: globalFolderTree = buildFolderTree(globalFolders, conversations, null);

  $: unassignedConversations = conversations.filter((c) => {
    if (c.projectId) return false;
    if (c.folderId && folders.some((f) => f.id === c.folderId)) return false;
    return true;
  });

  function toggleProjectExpanded(id: string) {
    expandedProjects[id] = !(expandedProjects[id] ?? true);
  }

  function handleProjectDragOver(e: DragEvent, projectId: string) {
    e.preventDefault();
    dragOverProjectId = projectId;
  }

  function handleProjectDragLeave() {
    dragOverProjectId = null;
  }

  function handleProjectDrop(e: DragEvent, projectId: string) {
    e.preventDefault();
    dragOverProjectId = null;
    const conversationId = e.dataTransfer?.getData("text/plain");
    if (conversationId) {
      onDropConversationToProject(conversationId, projectId);
    }
  }

  function handleUnassignedDragOver(e: DragEvent) {
    e.preventDefault();
    dragOverUnassigned = true;
  }

  function handleUnassignedDragLeave() {
    dragOverUnassigned = false;
  }

  function handleUnassignedDrop(e: DragEvent) {
    e.preventDefault();
    dragOverUnassigned = false;
    const conversationId = e.dataTransfer?.getData("text/plain");
    if (conversationId) {
      onMoveConversation(conversationId, null, null);
    }
  }

  function handleDragStart(e: DragEvent, conversationId: string) {
    if (e.dataTransfer) {
      e.dataTransfer.setData("text/plain", conversationId);
      e.dataTransfer.effectAllowed = "move";
    }
  }
</script>

<aside class="sidebar" class:sidebar-collapsed={!sidebarOpen}>
  {#if !sidebarOpen}
    <!-- MODE REPLIÉ: DOCK COMPACT APPLE -->
    <div class="sidebar-collapsed-dock" aria-label="Menu replié">
      <div class="dock-top">
        <button
          class="dock-brand-btn"
          type="button"
          title={labels.showSidebar ?? "Afficher la barre latérale"}
          on:click={onToggleSidebar}
        >
          <div class="brand-mark">
            <img src="/logo.png" alt="ARO logo" class="brand-logo-img" />
          </div>
        </button>

        <button
          class="dock-icon-btn"
          type="button"
          title={labels.showSidebar ?? "Afficher la barre latérale"}
          on:click={onToggleSidebar}
        >
          <PanelLeftOpen size={14} />
        </button>

        <button
          class="dock-icon-btn primary-action"
          type="button"
          disabled={cloudWriteLocked}
          title={cloudWriteDisabledTitle("demarrer une conversation") ?? labels.newConversation}
          on:click={onStartConversation}
        >
          <Plus size={15} />
        </button>

        <button
          class="dock-icon-btn"
          type="button"
          title={labels.searchPlaceholder}
          on:click={onToggleSidebar}
        >
          <Search size={13} />
        </button>
      </div>

      <div class="dock-divider"></div>

      <div class="dock-scroll" aria-label="Projets">
        {#each projectStats as proj, index (proj.id)}
          {@const pColor = getProjectColor(proj, index)}
          <button
            class="dock-project-pill"
            type="button"
            title="{proj.name} ({proj.conversationCount} discussion{proj.conversationCount > 1 ? 's' : ''})"
            on:click={onToggleSidebar}
          >
            <span
              class="dock-project-dot"
              style="background-color: {pColor}; box-shadow: 0 0 5px {pColor}80;"
            ></span>
          </button>
        {/each}

        {#if activeConversation}
          <div class="dock-divider"></div>
          <button
            class="dock-icon-btn active-chat"
            type="button"
            title={activeConversation.title}
            on:click={() => onOpenConversation(activeConversation)}
          >
            <MessageSquare size={13} />
          </button>
        {/if}
      </div>

      <div class="dock-bottom">
        <button
          class="dock-icon-btn cloud-dock-btn"
          class:connected={cloudAuthenticated}
          class:offline={cloudSyncStatus.health !== "online"}
          type="button"
          title={cloudAuthenticated ? (cloudWriteDisabledTitle("modifier cet espace") ?? (cloudSession?.activeOrganization.name || "Cloud connecté")) : "Connecter ARO Cloud"}
          on:click|stopPropagation={() => (showCloudAuthPanel = !showCloudAuthPanel)}
        >
          <Database size={13} />
          <span class="dock-status-dot" class:connected={cloudAuthenticated}></span>
        </button>

        <button
          class="dock-icon-btn"
          type="button"
          title={labels.settings}
          aria-label={labels.settings}
          on:click={onOpenSettings}
        >
          <Settings size={14} />
        </button>
      </div>
    </div>
  {:else}
    <!-- MODE DÉVELOPPÉ: BARRE LATÉRALE APPLE COMPACTE & ÉLÉGANTE -->
    <div class="sidebar-expanded-container">
      <div class="brand-row">
        <div class="brand">
          <div class="brand-mark">
            <img src="/logo.png" alt="ARO logo" class="brand-logo-img" />
          </div>
          <div class="brand-copy">
            <span>ARO</span>
            <small>{labels.localAssistant}</small>
          </div>
        </div>
        <button
          class="sidebar-collapse-toggle"
          type="button"
          title={labels.hideSidebar ?? "Masquer la barre latérale"}
          on:click={onToggleSidebar}
        >
          <PanelLeftClose size={14} />
        </button>
      </div>

      <!-- Navigation Actions -->
      <div class="sidebar-top-actions">
        <button
          class="sidebar-action-btn primary-new-chat"
          type="button"
          disabled={cloudWriteLocked}
          title={cloudWriteDisabledTitle("demarrer une conversation") ?? labels.newConversation}
          on:click={onStartConversation}
        >
          <Edit2 size={13} />
          <span>{labels.newConversation}</span>
        </button>

        <div class="sidebar-search-container">
          <Search size={13} class="search-icon" />
          <input
            type="text"
            placeholder={labels.searchPlaceholder}
            bind:value={searchQuery}
            class="sidebar-search-input"
          />
          {#if searchQuery}
            <button
              type="button"
              class="search-clear-btn"
              on:click={() => (searchQuery = "")}
              title={labels.clearSearch}
            >
              <X size={11} />
            </button>
          {/if}
        </div>
      </div>

      <!-- Section Title: PROJETS -->
      <div class="sidebar-section-header project-header-bar">
        <span>PROJETS</span>
        <div class="header-actions">
          <button
            class="icon-action-btn"
            type="button"
            title="Nouveau Projet"
            disabled={cloudWriteLocked}
            on:click={onOpenCreateProjectModal}
          >
            <FolderKanban size={13} />
          </button>
          <button
            class="icon-action-btn"
            type="button"
            title="Nouveau Dossier"
            disabled={cloudWriteLocked}
            on:click={() => onOpenCreateFolderModal(null)}
          >
            <FolderPlus size={13} />
          </button>
        </div>
      </div>

      <div class="projects-and-folders-scroll" aria-label="Arborescence des projets et dossiers">
        <!-- List of Projects -->
        {#each projectStats as proj, index (proj.id)}
          {@const isExpanded = expandedProjects[proj.id] ?? true}
          {@const projFolders = folders.filter((f) => f.projectId === proj.id)}
          {@const projFolderTree = buildFolderTree(projFolders, conversations, proj.id)}
          {@const projDirectConversations = conversations.filter((c) => c.projectId === proj.id && !c.folderId)}
          {@const projColor = getProjectColor(proj, index)}

          <div
            class="project-node"
            class:drag-over={dragOverProjectId === proj.id}
            on:dragover={(e) => handleProjectDragOver(e, proj.id)}
            on:dragleave={handleProjectDragLeave}
            on:drop={(e) => handleProjectDrop(e, proj.id)}
            role="region"
            aria-label={proj.name}
          >
            <div class="project-header-row">
              <button class="project-toggle-btn" type="button" on:click={() => toggleProjectExpanded(proj.id)}>
                {#if isExpanded}
                  <ChevronDown size={11} class="project-chevron" />
                {:else}
                  <ChevronRight size={11} class="project-chevron" />
                {/if}
                <span class="project-color-badge" style="background-color: {projColor}; box-shadow: 0 0 5px {projColor}80;"></span>
                <span class="project-name">{proj.name}</span>
                {#if proj.conversationCount > 0}
                  <span class="project-count">{proj.conversationCount}</span>
                {/if}
              </button>

              <div class="sidebar-menu-container">
                <button
                  class="conversation-actions"
                  class:active={activeSidebarMenuId === proj.id}
                  type="button"
                  on:click={(e) => {
                    e.stopPropagation();
                    activeSidebarMenuId = activeSidebarMenuId === proj.id ? null : proj.id;
                  }}
                >
                  <MoreHorizontal size={12} />
                </button>
                {#if activeSidebarMenuId === proj.id}
                  <!-- svelte-ignore a11y_no_static_element_interactions -->
                  <div class="sidebar-dropdown" on:mouseleave={() => (activeSidebarMenuId = null)}>
                    <button
                      type="button"
                      disabled={cloudWriteLocked}
                      on:click={(e) => {
                        e.stopPropagation();
                        activeSidebarMenuId = null;
                        onOpenCreateFolderModal(proj.id);
                      }}
                    >
                      <FolderPlus size={11} />
                      <span>Ajouter un dossier</span>
                    </button>
                    <button
                      type="button"
                      disabled={cloudWriteLocked}
                      on:click={(e) => {
                        e.stopPropagation();
                        activeSidebarMenuId = null;
                        onEditProject(proj);
                      }}
                    >
                      <Edit2 size={11} />
                      <span>Modifier projet</span>
                    </button>
                    <button
                      type="button"
                      on:click={(e) => {
                        e.stopPropagation();
                        activeSidebarMenuId = null;
                        onExportProject(proj);
                      }}
                    >
                      <Download size={11} />
                      <span>Exporter le projet</span>
                    </button>
                    <button
                      type="button"
                      class="danger"
                      disabled={cloudWriteLocked}
                      on:click={(e) => {
                        e.stopPropagation();
                        activeSidebarMenuId = null;
                        onDeleteProject(proj);
                      }}
                    >
                      <Trash2 size={11} />
                      <span>Supprimer projet</span>
                    </button>
                  </div>
                {/if}
              </div>
            </div>

            {#if isExpanded}
              <div class="project-body">
                <!-- Folders in project -->
                {#each projFolderTree as item}
                  <FolderTreeItem
                    folder={item.folder}
                    conversations={item.conversations}
                    {activeConversation}
                    {activeSidebarMenuId}
                    {cloudWriteLocked}
                    {isConversationSending}
                    {formatRelativeTime}
                    {onOpenConversation}
                    onRenameFolder={onEditFolder}
                    onDeleteFolder={onDeleteFolder}
                    {onRenameConversation}
                    {onRemoveConversation}
                    {onDropConversationToFolder}
                  />
                {/each}

                <!-- Direct conversations in project -->
                {#each projDirectConversations as conversation}
                  <!-- svelte-ignore a11y_no_static_element_interactions -->
                  <div
                    class="conversation-item child-item"
                    class:active={activeConversation?.id === conversation.id}
                    class:menu-open={activeSidebarMenuId === conversation.id}
                    class:working={isConversationSending(conversation.id, sendingByConversation)}
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
                        <MessageSquare size={12} />
                      </span>
                      <span class="conversation-title">{conversation.title}</span>
                    </button>
                    <div class="conversation-right">
                      {#if isConversationSending(conversation.id, sendingByConversation)}
                        <span class="conversation-working-indicator" title="Generation en cours" aria-label="Generation en cours"></span>
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
                          title={labels.conversationOptions}
                          aria-label={labels.conversationOptions}
                          on:click={(event) => {
                            event.stopPropagation();
                            activeSidebarMenuId = activeSidebarMenuId === conversation.id ? null : conversation.id;
                          }}
                        >
                          <MoreHorizontal size={12} />
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
                                onOpenMoveModal(conversation);
                              }}
                            >
                              <FolderTree size={11} />
                              <span>Déplacer vers...</span>
                            </button>
                            <button
                              type="button"
                              disabled={cloudWriteLocked}
                              on:click={(e) => {
                                e.stopPropagation();
                                activeSidebarMenuId = null;
                                onRenameConversation(conversation);
                              }}
                            >
                              <Edit2 size={11} />
                              <span>{labels.rename}</span>
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
                              <Trash2 size={11} />
                              <span>{labels.delete}</span>
                            </button>
                          </div>
                        {/if}
                      </div>
                    </div>
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        {/each}

        <!-- Global Folders -->
        {#each globalFolderTree as item}
          <FolderTreeItem
            folder={item.folder}
            conversations={item.conversations}
            {activeConversation}
            {activeSidebarMenuId}
            {cloudWriteLocked}
            {isConversationSending}
            {formatRelativeTime}
            {onOpenConversation}
            onRenameFolder={onEditFolder}
            onDeleteFolder={onDeleteFolder}
            {onRenameConversation}
            {onRemoveConversation}
            {onDropConversationToFolder}
          />
        {/each}

        <!-- Section Title: CONVERSATIONS -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="sidebar-section-header unassigned-header"
          class:drag-over={dragOverUnassigned}
          on:dragover={handleUnassignedDragOver}
          on:dragleave={handleUnassignedDragLeave}
          on:drop={handleUnassignedDrop}
          title="Glissez une conversation ici pour la retirer de son projet/dossier"
        >
          <span>{projects.length > 0 ? "CONVERSATIONS" : labels.conversations}</span>
          {#if unassignedConversations.length > 0}
            <span class="section-count-badge">{unassignedConversations.length}</span>
          {/if}
          <button
            type="button"
            class="section-clean-btn"
            title={labels.cleanEmptyConversations}
            aria-label={labels.cleanEmptyConversations}
            on:click={onCleanEmptyConversations}
          >
            <BrushCleaning size={12} />
          </button>
        </div>

        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="conversation-list unassigned-drop-zone"
          class:drag-over={dragOverUnassigned}
          aria-label={labels.conversations}
          on:dragover={handleUnassignedDragOver}
          on:dragleave={handleUnassignedDragLeave}
          on:drop={handleUnassignedDrop}
        >
          {#if unassignedConversations.length === 0}
            <div class="empty-list">{labels.noConversations}</div>
          {/if}
          {#each unassignedConversations as conversation}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div
              class:active={activeConversation?.id === conversation.id}
              class:menu-open={activeSidebarMenuId === conversation.id}
              class:working={isConversationSending(conversation.id, sendingByConversation)}
              class="conversation-item"
              draggable="true"
              on:dragstart={(e) => handleDragStart(e, conversation.id)}
              title={conversation.title}
            >
              <button
                class="conversation-main"
                type="button"
                aria-current={activeConversation?.id === conversation.id ? "page" : undefined}
                on:click={() => onOpenConversation(conversation)}
              >
                <span class="conversation-icon-wrapper">
                  <MessageSquare size={13} />
                </span>
                <span class="conversation-title">{conversation.title}</span>
              </button>
              <div class="conversation-right">
                {#if isConversationSending(conversation.id, sendingByConversation)}
                  <span class="conversation-working-indicator" title="Generation en cours" aria-label="Generation en cours"></span>
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
                    title={labels.conversationOptions}
                    aria-label={labels.conversationOptions}
                    on:click={(event) => {
                      event.stopPropagation();
                      activeSidebarMenuId = activeSidebarMenuId === conversation.id ? null : conversation.id;
                    }}
                  >
                    <MoreHorizontal size={12} />
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
                          onOpenMoveModal(conversation);
                        }}
                      >
                        <FolderTree size={11} />
                        <span>Déplacer vers...</span>
                      </button>
                      <button
                        type="button"
                        disabled={cloudWriteLocked}
                        on:click={(e) => {
                          e.stopPropagation();
                          activeSidebarMenuId = null;
                          onRenameConversation(conversation);
                        }}
                      >
                        <Edit2 size={11} />
                        <span>{labels.rename}</span>
                      </button>
                      <button
                        type="button"
                        class="danger"
                        disabled={cloudWriteLocked}
                        on:click={(event) => {
                          event.stopPropagation();
                          activeSidebarMenuId = null;
                          onRemoveConversation(conversation, event);
                        }}
                      >
                        <Trash2 size={11} />
                        <span>{labels.delete}</span>
                      </button>
                    </div>
                  {/if}
                </div>
              </div>
            </div>
          {/each}
        </div>
      </div>

      <button
        class="cloud-status-button"
        class:connected={cloudAuthenticated}
        class:offline={cloudSyncStatus.health !== "online"}
        type="button"
        title={cloudAuthenticated ? (cloudWriteDisabledTitle("modifier cet espace") ?? cloudSyncStatus.health) : "Connecter ARO Cloud"}
        on:click|stopPropagation={() => (showCloudAuthPanel = !showCloudAuthPanel)}
      >
        <Database size={13} class="db-icon" />
        <span>{cloudAuthenticated ? (cloudSession?.activeOrganization.name || "Cloud") : "Cloud"}</span>
        {#if cloudAuthenticated}
          <ChevronsUpDown size={11} class="chevron-icon" />
        {:else}
          {#if cloudStatusShortLabel}
            <small>{cloudStatusShortLabel}</small>
          {:else if cloudSyncStatus.pendingEvents > 0}
            <small>{cloudSyncStatus.pendingEvents}</small>
          {/if}
        {/if}
      </button>

      <div class="sidebar-footer">
        <button
          class="settings-btn"
          type="button"
          title={labels.settings}
          aria-label={labels.settings}
          on:click={onOpenSettings}
        >
          <Settings size={14} />
          <span class="settings-text">{labels.settings}</span>
        </button>

        <div class="runtime-pill" title={runtime?.detail ?? "Local runtime"}>
          <span class:ready={runtime?.modelReady} class="status-dot"></span>
          <span class="runtime-name">{runtime?.modelProvider ?? "mock"}</span>
        </div>
      </div>
    </div>
  {/if}
</aside>

<style>
  /* BASE SIDEBAR CONTAINER */
  .sidebar {
    position: relative;
    isolation: isolate;
    display: flex;
    flex-direction: column;
    height: 100%;
    box-sizing: border-box;
    border-right: 1px solid rgba(0, 0, 0, 0.06);
    background: rgba(247, 249, 252, 0.82);
    backdrop-filter: blur(28px) saturate(130%);
    overflow: hidden;
    transition: width 240ms cubic-bezier(0.16, 1, 0.3, 1), background 240ms ease, border-color 240ms ease;
  }

  :global(body.dark-theme) .sidebar {
    background: rgba(18, 20, 26, 0.92);
    border-right-color: rgba(255, 255, 255, 0.07);
  }

  /* COLLAPSED DOCK MODE */
  .sidebar-collapsed-dock {
    display: flex;
    flex-direction: column;
    align-items: center;
    width: 100%;
    height: 100%;
    padding: 8px 4px 8px;
    gap: 4px;
    box-sizing: border-box;
    overflow: hidden;
  }

  .dock-top {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    width: 100%;
  }

  .dock-brand-btn {
    background: transparent;
    border: none;
    padding: 0;
    cursor: pointer;
    border-radius: 7px;
    transition: transform 0.15s ease;
  }

  .dock-brand-btn:hover {
    transform: scale(1.08);
  }

  .dock-icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    border-radius: 7px;
    background: transparent;
    border: 1px solid transparent;
    color: #4a5464;
    cursor: pointer;
    position: relative;
    transition: all 0.12s ease;
  }

  .dock-icon-btn:hover {
    background: rgba(0, 0, 0, 0.05);
    color: #0f172a;
    transform: translateY(-0.5px);
  }

  :global(body.dark-theme) .dock-icon-btn {
    color: #94a3b8;
  }

  :global(body.dark-theme) .dock-icon-btn:hover {
    background: rgba(255, 255, 255, 0.08);
    color: #f8fafc;
  }

  .dock-icon-btn.primary-action {
    background: rgba(0, 113, 227, 0.09);
    border-color: rgba(0, 113, 227, 0.2);
    color: #0071e3;
  }

  .dock-icon-btn.primary-action:hover {
    background: rgba(0, 113, 227, 0.16);
    border-color: rgba(0, 113, 227, 0.32);
    color: #0071e3;
  }

  :global(body.dark-theme) .dock-icon-btn.primary-action {
    background: rgba(59, 130, 246, 0.14);
    border-color: rgba(59, 130, 246, 0.25);
    color: #60a5fa;
  }

  :global(body.dark-theme) .dock-icon-btn.primary-action:hover {
    background: rgba(59, 130, 246, 0.22);
    color: #93c5fd;
  }

  .dock-icon-btn.active-chat {
    background: rgba(0, 0, 0, 0.05);
    border-color: rgba(0, 0, 0, 0.06);
    color: #0071e3;
  }

  :global(body.dark-theme) .dock-icon-btn.active-chat {
    background: rgba(255, 255, 255, 0.08);
    border-color: rgba(255, 255, 255, 0.08);
    color: #38bdf8;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.25);
  }

  .dock-divider {
    width: 18px;
    height: 1px;
    background: rgba(0, 0, 0, 0.07);
    margin: 2px 0;
    flex-shrink: 0;
  }

  :global(body.dark-theme) .dock-divider {
    background: rgba(255, 255, 255, 0.08);
  }

  .dock-scroll {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
    width: 100%;
    overflow-y: auto;
    scrollbar-width: none;
    padding-top: 2px;
  }

  .dock-scroll::-webkit-scrollbar {
    display: none;
  }

  .dock-project-pill {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border-radius: 6px;
    background: transparent;
    border: none;
    cursor: pointer;
    transition: background 0.12s ease, transform 0.12s ease;
  }

  .dock-project-pill:hover {
    background: rgba(0, 0, 0, 0.05);
    transform: scale(1.1);
  }

  :global(body.dark-theme) .dock-project-pill:hover {
    background: rgba(255, 255, 255, 0.08);
  }

  .dock-project-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    display: block;
  }

  .dock-bottom {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    width: 100%;
    margin-top: auto;
    padding-top: 4px;
    border-top: 1px solid rgba(0, 0, 0, 0.05);
  }

  :global(body.dark-theme) .dock-bottom {
    border-top-color: rgba(255, 255, 255, 0.06);
  }

  .dock-status-dot {
    position: absolute;
    bottom: 5px;
    right: 5px;
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: #94a3b8;
  }

  .dock-status-dot.connected {
    background: #34c759;
    box-shadow: 0 0 5px rgba(52, 199, 89, 0.6);
  }

  /* EXPANDED SIDEBAR CONTAINER */
  .sidebar-expanded-container {
    display: flex;
    flex-direction: column;
    height: 100%;
    width: 100%;
    overflow: hidden;
    gap: 6px;
    padding: 8px 8px 8px;
    box-sizing: border-box;
  }

  .sidebar-collapse-toggle {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: 5px;
    background: transparent;
    border: none;
    color: #8e8e93;
    cursor: pointer;
    transition: all 0.12s ease;
  }

  .sidebar-collapse-toggle:hover {
    background: rgba(0, 0, 0, 0.05);
    color: #111;
  }

  :global(body.dark-theme) .sidebar-collapse-toggle {
    color: #94a3b8;
  }

  :global(body.dark-theme) .sidebar-collapse-toggle:hover {
    background: rgba(255, 255, 255, 0.08);
    color: #f1f5f9;
  }

  .primary-new-chat {
    background: rgba(0, 113, 227, 0.08) !important;
    border: 1px solid rgba(0, 113, 227, 0.16) !important;
    color: #0071e3 !important;
    font-weight: 550 !important;
    border-radius: 7px !important;
    height: 32px !important;
    display: flex !important;
    align-items: center !important;
    justify-content: center !important;
    gap: 7px !important;
    font-size: 12.5px !important;
    transition: all 0.12s cubic-bezier(0.16, 1, 0.3, 1) !important;
  }

  .primary-new-chat:hover {
    background: rgba(0, 113, 227, 0.14) !important;
    border-color: rgba(0, 113, 227, 0.28) !important;
    transform: translateY(-0.5px);
  }

  .primary-new-chat:active {
    transform: scale(0.99);
  }

  :global(body.dark-theme) .primary-new-chat {
    background: rgba(59, 130, 246, 0.12) !important;
    border-color: rgba(59, 130, 246, 0.22) !important;
    color: #60a5fa !important;
  }

  :global(body.dark-theme) .primary-new-chat:hover {
    background: rgba(59, 130, 246, 0.2) !important;
    border-color: rgba(59, 130, 246, 0.32) !important;
    color: #93c5fd !important;
  }

  /* SECTION HEADER */
  .sidebar-section-header {
    display: flex;
    align-items: center;
    color: #71717a;
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  :global(body.dark-theme) .sidebar-section-header {
    color: #8e8e93;
  }

  .project-header-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 3px 4px 1px;
    margin-top: 2px;
  }

  .header-actions {
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .icon-action-btn {
    background: transparent;
    border: none;
    color: #8e8e93;
    padding: 2px;
    width: 20px;
    height: 20px;
    border-radius: 4px;
    cursor: pointer;
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background 0.12s ease, color 0.12s ease;
  }

  .icon-action-btn:hover {
    color: #0f172a;
    background: rgba(15, 23, 42, 0.08);
  }

  :global(body.dark-theme) .icon-action-btn {
    color: #8e8e93;
  }

  :global(body.dark-theme) .icon-action-btn:hover {
    color: #f1f5f9;
    background: rgba(255, 255, 255, 0.08);
  }

  /* SCROLLABLE ARBORESCENCE */
  .projects-and-folders-scroll {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: 0 2px;
    scrollbar-width: thin;
  }

  .project-node {
    border-radius: 5px;
    transition: background 0.12s ease;
    margin-bottom: 1px;
  }

  .project-node.drag-over {
    background: rgba(59, 130, 246, 0.1) !important;
    outline: 1.5px dashed #3b82f6 !important;
    outline-offset: -1px;
  }

  .project-header-row {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 2px 4px;
    min-height: 26px;
    border-radius: 5px;
    transition: background 0.12s ease;
  }

  .project-header-row:hover {
    background: rgba(15, 23, 42, 0.04);
  }

  :global(body.dark-theme) .project-header-row:hover {
    background: rgba(255, 255, 255, 0.04);
  }

  .project-toggle-btn {
    display: flex;
    align-items: center;
    gap: 5px;
    background: transparent;
    border: none;
    color: #0f172a;
    font-size: 0.78rem;
    font-weight: 500;
    cursor: pointer;
    flex: 1;
    text-align: left;
    overflow: hidden;
    letter-spacing: -0.01em;
    padding: 1px 0;
  }

  :global(body.dark-theme) .project-toggle-btn {
    color: #cbd5e1;
  }

  :global(.project-chevron) {
    color: #8e8e93;
    flex-shrink: 0;
    transition: transform 0.12s ease;
  }

  .project-color-badge {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex-shrink: 0;
    transition: transform 0.12s ease;
  }

  .project-header-row:hover .project-color-badge {
    transform: scale(1.15);
  }

  .project-name {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .project-count {
    font-size: 0.62rem;
    font-weight: 600;
    padding: 0 5px;
    border-radius: 999px;
    background: rgba(15, 23, 42, 0.06);
    color: #64748b;
    margin-left: auto;
    font-variant-numeric: tabular-nums;
  }

  :global(body.dark-theme) .project-count {
    background: rgba(255, 255, 255, 0.08);
    color: #94a3b8;
  }

  .project-body {
    padding-left: 8px;
    margin-left: 6px;
    border-left: 1px solid rgba(148, 163, 184, 0.18);
    display: flex;
    flex-direction: column;
    gap: 1px;
    margin-top: 1px;
    margin-bottom: 2px;
  }

  :global(body.dark-theme) .project-body {
    border-left-color: rgba(255, 255, 255, 0.07);
  }

  .unassigned-header {
    margin-top: 6px;
    padding: 3px 4px 1px;
    display: flex;
    align-items: center;
    transition: background 0.12s ease;
  }

  .section-count-badge {
    font-size: 0.62rem;
    font-weight: 600;
    padding: 0 5px;
    border-radius: 999px;
    background: rgba(15, 23, 42, 0.06);
    color: #64748b;
    margin-left: 5px;
    font-variant-numeric: tabular-nums;
  }

  :global(body.dark-theme) .section-count-badge {
    background: rgba(255, 255, 255, 0.08);
    color: #94a3b8;
  }

  .section-clean-btn {
    margin-left: auto;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border-radius: 6px;
    border: none;
    background: transparent;
    color: #94a3b8;
    cursor: pointer;
    transition: all 0.12s ease;
  }

  .section-clean-btn:hover {
    background: rgba(59, 130, 246, 0.12);
    color: #3b82f6;
  }

  :global(body.dark-theme) .section-clean-btn:hover {
    background: rgba(255, 255, 255, 0.08);
    color: #e2e8f0;
  }

  .unassigned-header.drag-over,
  .unassigned-drop-zone.drag-over {
    background: rgba(59, 130, 246, 0.1) !important;
    border-radius: 5px;
    outline: 1.5px dashed #3b82f6 !important;
    outline-offset: -1px;
  }

  /* CONVERSATION ITEMS */
  .conversation-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 28px;
    min-height: 28px;
    padding: 0 6px 0 8px;
    border-radius: 5px;
    background: transparent;
    border: 1px solid transparent;
    cursor: pointer;
    transition: background 0.12s ease, border-color 0.12s ease;
    user-select: none;
    position: relative;
    width: 100%;
    box-sizing: border-box;
  }

  .conversation-item:hover {
    background: rgba(0, 0, 0, 0.04);
  }

  :global(body.dark-theme) .conversation-item:hover {
    background: rgba(255, 255, 255, 0.04);
  }

  /* ACTIVE STATE - REFINED APPLE MINIMAL */
  .conversation-item.active {
    background: rgba(0, 0, 0, 0.05) !important;
    border-color: rgba(0, 0, 0, 0.06) !important;
  }

  .conversation-item.active::before {
    content: "";
    position: absolute;
    left: 0;
    top: 5px;
    bottom: 5px;
    width: 2.5px;
    border-radius: 99px;
    background: #0071e3;
  }

  .conversation-item.active .conversation-title {
    color: #0f172a !important;
    font-weight: 550;
  }

  .conversation-item.active .conversation-icon-wrapper {
    color: #0071e3 !important;
    opacity: 1;
  }

  :global(body.dark-theme) .conversation-item.active {
    background: rgba(255, 255, 255, 0.08) !important;
    border-color: rgba(255, 255, 255, 0.08) !important;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.25) !important;
  }

  :global(body.dark-theme) .conversation-item.active::before {
    background: #38bdf8;
    box-shadow: 0 0 6px rgba(56, 189, 248, 0.5);
  }

  :global(body.dark-theme) .conversation-item.active .conversation-title {
    color: #ffffff !important;
    font-weight: 500;
  }

  :global(body.dark-theme) .conversation-item.active .conversation-icon-wrapper {
    color: #38bdf8 !important;
    opacity: 1;
  }

  .conversation-main {
    display: flex;
    align-items: center;
    min-width: 0;
    flex: 1;
    background: transparent;
    border: none;
    padding: 0;
    text-align: left;
    cursor: pointer;
    color: inherit;
    gap: 6px;
  }

  .conversation-title {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 12px;
    font-weight: 450;
    color: #374151;
    transition: color 0.12s ease;
  }

  :global(body.dark-theme) .conversation-title {
    color: #94a3b8;
  }

  .conversation-icon-wrapper {
    display: flex;
    align-items: center;
    justify-content: center;
    color: #8e8e93;
    flex-shrink: 0;
    transition: color 0.12s ease;
  }

  .conversation-item:hover .conversation-icon-wrapper {
    color: #4a5464;
  }

  :global(body.dark-theme) .conversation-icon-wrapper {
    color: #64748b;
  }

  :global(body.dark-theme) .conversation-item:hover .conversation-icon-wrapper {
    color: #cbd5e1;
  }

  .conversation-right {
    display: flex;
    align-items: center;
    position: relative;
    flex-shrink: 0;
  }

  .conversation-meta {
    font-size: 10px;
    color: #8e8e93;
    transition: opacity 0.12s ease;
  }

  .conversation-item:hover .conversation-meta {
    opacity: 0;
  }

  .conversation-actions {
    position: absolute;
    right: 0;
    opacity: 0;
    transition: opacity 0.12s ease;
    background: transparent;
    border: none;
    color: #8e8e93;
    padding: 2px;
    width: 18px;
    height: 18px;
    border-radius: 4px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .conversation-actions:hover {
    color: #0f172a;
    background: rgba(0, 0, 0, 0.06);
  }

  :global(body.dark-theme) .conversation-actions:hover {
    color: #fff;
    background: rgba(255, 255, 255, 0.1);
  }

  .sidebar-menu-container {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    flex-shrink: 0;
  }

  .project-header-row:hover .conversation-actions,
  .conversation-item:hover .conversation-actions,
  .conversation-actions.active,
  .sidebar-menu-container:focus-within .conversation-actions {
    opacity: 1;
  }

  .child-item {
    font-size: 0.76rem;
    padding: 0 6px;
    height: 26px;
    min-height: 26px;
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
    color: #8e8e93;
    flex-shrink: 0;
    opacity: 0.7;
    transition: opacity 0.12s ease, color 0.12s ease;
  }

  .child-item:hover .conversation-icon-wrapper {
    color: #cbd5e1;
    opacity: 1;
  }

  /* FOOTER & CLOUD */
  .cloud-status-button {
    height: 28px;
    min-height: 28px;
    padding: 0 8px;
    border-radius: 6px;
    font-size: 11px;
    display: flex;
    align-items: center;
    gap: 6px;
    background: rgba(0, 0, 0, 0.02);
    border: 1px solid rgba(0, 0, 0, 0.05);
    color: #4b5563;
    cursor: pointer;
    margin-top: auto;
    transition: all 0.12s ease;
  }

  :global(body.dark-theme) .cloud-status-button {
    background: rgba(255, 255, 255, 0.03);
    border-color: rgba(255, 255, 255, 0.06);
    color: #94a3b8;
  }

  .cloud-status-button:hover {
    background: rgba(0, 0, 0, 0.05);
    color: #111;
  }

  :global(body.dark-theme) .cloud-status-button:hover {
    background: rgba(255, 255, 255, 0.07);
    color: #f8fafc;
  }

  .sidebar-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-top: 4px;
    margin-top: 0;
    border-top: 1px solid rgba(0, 0, 0, 0.05);
  }

  :global(body.dark-theme) .sidebar-footer {
    border-top-color: rgba(255, 255, 255, 0.06);
  }

  .settings-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 24px;
    padding: 0 6px;
    border-radius: 5px;
    background: transparent;
    border: none;
    color: #6b7280;
    font-size: 11.5px;
    cursor: pointer;
    transition: all 0.12s ease;
  }

  :global(body.dark-theme) .settings-btn {
    color: #8e8e93;
  }

  .settings-btn:hover {
    background: rgba(0, 0, 0, 0.05);
    color: #111;
  }

  :global(body.dark-theme) .settings-btn:hover {
    background: rgba(255, 255, 255, 0.08);
    color: #f8fafc;
  }

  .runtime-pill {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 20px;
    padding: 0 6px;
    border-radius: 999px;
    background: rgba(0, 0, 0, 0.03);
    color: #6b7280;
    font-size: 10px;
    font-weight: 500;
  }

  :global(body.dark-theme) .runtime-pill {
    background: rgba(255, 255, 255, 0.05);
    color: #8e8e93;
  }

  .status-dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: #94a3b8;
  }

  .status-dot.ready {
    background: #34c759;
    box-shadow: 0 0 5px rgba(52, 199, 89, 0.6);
  }
</style>
