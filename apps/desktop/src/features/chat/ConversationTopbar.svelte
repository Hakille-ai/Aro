<script context="module" lang="ts">
  export type ConversationPersonality = {
    id: string;
    cloudId?: string;
    name: string;
    description: string;
    prompt: string;
    icon: string;
    avatarColor: string;
    temperature: number;
    voiceId?: string | null;
    isDefault?: boolean;
  };

  export type ConversationTopbarLabels = {
    conversationOptions: string;
    renameConversationDropdown: string;
    deleteConversationDropdown: string;
    showContext: string;
  };
</script>

<script lang="ts">
  import Activity from "@lucide/svelte/icons/activity";
  import Bot from "@lucide/svelte/icons/bot";
  import Brain from "@lucide/svelte/icons/brain";
  import Building2 from "@lucide/svelte/icons/building-2";
  import Check from "@lucide/svelte/icons/check";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Cpu from "@lucide/svelte/icons/cpu";
  import Download from "@lucide/svelte/icons/download";
  import Edit2 from "@lucide/svelte/icons/edit-2";
  import FileText from "@lucide/svelte/icons/file-text";
  import FolderTree from "@lucide/svelte/icons/folder-tree";
  import MoreHorizontal from "@lucide/svelte/icons/more-horizontal";
  import PanelRight from "@lucide/svelte/icons/panel-right";
  import Search from "@lucide/svelte/icons/search";
  import Sliders from "@lucide/svelte/icons/sliders";
  import Terminal from "@lucide/svelte/icons/terminal";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import User from "@lucide/svelte/icons/user";
  import type { Conversation, Project } from "../../lib/types";

  export let activeConversation: Conversation | null;
  export let activeProject: Project | null = null;
  export let conversationPersonalities: Record<string, string>;
  export let selectedPersonalityId: string;
  export let personalities: ConversationPersonality[];
  export let conversationCustomAgents: Record<string, string> = {};
  export let customAgentsList: any[] = [];
  export let theme: "light" | "dark";
  export let language: "fr" | "en";
  export let labels: ConversationTopbarLabels;
  export let showTopbarPersonalityDropdown: boolean;
  export let showConversationMenu: boolean;
  export let showRightPanel: boolean = false;
  export let attachedFileCount: number;
  export let cloudWriteLocked: boolean;
  export let cloudWriteDisabledTitle: (action?: string) => string | null | undefined;
  export let ensureCloudWriteAllowed: (action?: string) => boolean;
  export let onSelectConversationPersonality: (conversationId: string, personalityId: string) => void;
  export let onSelectConversationCustomAgent: ((conversationId: string, agentId: string | null) => void) | undefined = undefined;
  export let onRenameConversation: (conversation: Conversation) => void;
  export let onRemoveConversation: (conversation: Conversation, event: MouseEvent) => void | Promise<void>;
  export let onMoveConversation: ((conversation: Conversation) => void) | undefined = undefined;
  export let onExportConversation: ((conversation: Conversation, format: 'markdown' | 'json') => void) | undefined = undefined;
  export let onOpenMemorySettings: (() => void) | undefined = undefined;

  let showProjectVaultPopover = false;
  $: void attachedFileCount;
</script>

<header class="topbar">
  <div class="topbar-title">
    {#if activeConversation}
      <span>{activeConversation.title}</span>

      <!-- Topbar Personality Selector -->
      {@const pId = conversationPersonalities[activeConversation.id] || selectedPersonalityId}
      {@const activePers = personalities.find((personality) => personality.id === pId) || personalities[0]}
      {@const customAgentId = conversationCustomAgents[activeConversation.id]}
      {@const matchedCustomAgent = customAgentId ? customAgentsList.find(a => a.id === customAgentId) : null}
      {@const activeName = matchedCustomAgent ? matchedCustomAgent.name : activePers.name}
      {@const activeAvatarColor = matchedCustomAgent ? "linear-gradient(135deg, #af52de 0%, #7d26cd 100%)" : activePers.avatarColor}

      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="topbar-personality-badge" style="display: flex; align-items: center; gap: 6px; padding: 4px 10px; border-radius: 12px; background: {theme === 'dark' ? 'rgba(255,255,255,0.04)' : 'rgba(0,0,0,0.03)'}; border: 1px solid {theme === 'dark' ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.05)'}; font-size: 11px; font-weight: 500; margin-left: 12px; cursor: pointer; position: relative;"
           title={cloudWriteDisabledTitle("changer la personnalite de la conversation") ?? activeName}
           on:click={() => {
             if (cloudWriteLocked) {
               ensureCloudWriteAllowed("changer la personnalite de la conversation");
               return;
             }
             showTopbarPersonalityDropdown = !showTopbarPersonalityDropdown;
           }}>
        <div style="width: 14px; height: 14px; border-radius: 50%; background: {activeAvatarColor}; display: flex; align-items: center; justify-content: center; color: #ffffff; flex-shrink: 0;">
          {#if matchedCustomAgent}
            {#if matchedCustomAgent.icon === "bot"}<Bot size={8} />
            {:else if matchedCustomAgent.icon === "code"}<Terminal size={8} />
            {:else if matchedCustomAgent.icon === "check"}<Check size={8} />
            {:else if matchedCustomAgent.icon === "edit-2"}<Edit2 size={8} />
            {:else if matchedCustomAgent.icon === "brain"}<Brain size={8} />
            {:else if matchedCustomAgent.icon === "cpu"}<Cpu size={8} />
            {:else if matchedCustomAgent.icon === "user"}<User size={8} />
            {:else if matchedCustomAgent.icon === "activity"}<Activity size={8} />
            {:else if matchedCustomAgent.icon === "sliders"}<Sliders size={8} />
            {:else if matchedCustomAgent.icon === "building-2"}<Building2 size={8} />
            {:else}<Search size={8} />{/if}
          {:else}
            {#if activePers.icon === "bot"}<Bot size={8} />
            {:else if activePers.icon === "code"}<Terminal size={8} />
            {:else if activePers.icon === "check"}<Check size={8} />
            {:else if activePers.icon === "edit-2"}<Edit2 size={8} />
            {:else if activePers.icon === "brain"}<Brain size={8} />
            {:else if activePers.icon === "cpu"}<Cpu size={8} />
            {:else if activePers.icon === "user"}<User size={8} />
            {:else if activePers.icon === "activity"}<Activity size={8} />
            {:else if activePers.icon === "sliders"}<Sliders size={8} />
            {:else if activePers.icon === "building-2"}<Building2 size={8} />
            {:else}<Search size={8} />{/if}
          {/if}
        </div>
        <span style="color: {theme === 'dark' ? '#f5f5f7' : '#1d1d1f'};">{activeName}</span>
        <ChevronDown size={10} style="color: #86868b;" />

        {#if showTopbarPersonalityDropdown}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="conversation-dropdown" style="top: 26px; left: 0; min-width: 160px; z-index: 1000;" on:mouseleave={() => (showTopbarPersonalityDropdown = false)}>
            {#each personalities as personality}
              <button
                type="button"
                class="dropdown-item"
                disabled={cloudWriteLocked}
                style="display: flex; align-items: center; gap: 8px; font-weight: {!customAgentId && personality.id === pId ? '600' : 'normal'};"
                on:click|stopPropagation={() => {
                  if (activeConversation) {
                    onSelectConversationCustomAgent?.(activeConversation.id, null);
                    onSelectConversationPersonality(activeConversation.id, personality.id);
                  }
                  showTopbarPersonalityDropdown = false;
                }}
              >
                <div style="width: 12px; height: 12px; border-radius: 50%; background: {personality.avatarColor}; display: flex; align-items: center; justify-content: center; color: #ffffff; flex-shrink: 0;">
                  {#if personality.icon === "bot"}<Bot size={7} />
                  {:else if personality.icon === "code"}<Terminal size={7} />
                  {:else if personality.icon === "check"}<Check size={7} />
                  {:else if personality.icon === "edit-2"}<Edit2 size={7} />
                  {:else if personality.icon === "brain"}<Brain size={7} />
                  {:else if personality.icon === "cpu"}<Cpu size={7} />
                  {:else if personality.icon === "user"}<User size={7} />
                  {:else if personality.icon === "activity"}<Activity size={7} />
                  {:else if personality.icon === "sliders"}<Sliders size={7} />
                  {:else if personality.icon === "building-2"}<Building2 size={7} />
                  {:else}<Search size={7} />{/if}
                </div>
                <span>{personality.name}</span>
                {#if !customAgentId && personality.id === pId}
                  <Check size={10} style="margin-left: auto; color: #0071e3;" />
                {/if}
              </button>
            {/each}

            {#if customAgentsList.length > 0}
              <div style="padding: 4px 10px; font-size: 9px; font-weight: 700; text-transform: uppercase; color: #86868b; letter-spacing: 0.5px; border-top: 1px solid {theme === 'dark' ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.05)'}; margin-top: 4px;">
                {language === "fr" ? "Mes Agents" : "My Agents"}
              </div>
              {#each customAgentsList as agent}
                <button
                  type="button"
                  class="dropdown-item"
                  disabled={cloudWriteLocked}
                  style="display: flex; align-items: center; gap: 8px; font-weight: {customAgentId === agent.id ? '600' : 'normal'};"
                  on:click|stopPropagation={() => {
                    if (activeConversation) {
                      onSelectConversationCustomAgent?.(activeConversation.id, agent.id);
                    }
                    showTopbarPersonalityDropdown = false;
                  }}
                >
                  <div style="width: 12px; height: 12px; border-radius: 50%; background: linear-gradient(135deg, #af52de 0%, #7d26cd 100%); display: flex; align-items: center; justify-content: center; color: #ffffff; flex-shrink: 0;">
                    {#if agent.icon === "bot"}<Bot size={7} />
                    {:else if agent.icon === "code"}<Terminal size={7} />
                    {:else if agent.icon === "check"}<Check size={7} />
                    {:else if agent.icon === "edit-2"}<Edit2 size={7} />
                    {:else if agent.icon === "brain"}<Brain size={7} />
                    {:else if agent.icon === "cpu"}<Cpu size={7} />
                    {:else if agent.icon === "user"}<User size={7} />
                    {:else if agent.icon === "activity"}<Activity size={7} />
                    {:else if agent.icon === "sliders"}<Sliders size={7} />
                    {:else if agent.icon === "building-2"}<Building2 size={7} />
                    {:else}<Search size={7} />{/if}
                  </div>
                  <span>{agent.name}</span>
                  {#if customAgentId === agent.id}
                    <Check size={10} style="margin-left: auto; color: #0071e3;" />
                  {/if}
                </button>
              {/each}
            {/if}
          </div>
        {/if}
      </div>

      <!-- Project Context Vault Badge -->
      {#if activeProject}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="topbar-project-badge"
          on:click={() => (showProjectVaultPopover = !showProjectVaultPopover)}
          title="Consignes & Context Vault du Projet"
        >
          <span class="badge-color-dot" style="background-color: {activeProject.color};"></span>
          <span class="badge-project-name">{activeProject.name}</span>
          {#if activeProject.instructions}
            <FileText size={11} class="vault-icon" />
          {/if}

          {#if showProjectVaultPopover}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div class="project-vault-popover" on:click|stopPropagation on:mouseleave={() => (showProjectVaultPopover = false)}>
              <header class="popover-header">
                <span class="popover-title">Context Vault · {activeProject.name}</span>
              </header>
              <div class="popover-body">
                {#if activeProject.instructions}
                  <div class="vault-instructions-text">{activeProject.instructions}</div>
                {:else}
                  <div class="vault-empty-text">Aucune consigne spécifique configurée pour ce projet.</div>
                {/if}
              </div>
            </div>
          {/if}
        </div>
      {/if}

      {#if onOpenMemorySettings}
        <button
          type="button"
          class="topbar-memory-badge"
          style="display: inline-flex; align-items: center; gap: 5px; padding: 4px 9px; border-radius: 12px; background: {theme === 'dark' ? 'rgba(191,90,242,0.12)' : 'rgba(191,90,242,0.08)'}; border: 1px solid {theme === 'dark' ? 'rgba(191,90,242,0.25)' : 'rgba(191,90,242,0.18)'}; color: #bf5af2; font-size: 11px; font-weight: 600; cursor: pointer; transition: all 0.2s; margin-left: 6px;"
          title={language === "fr" ? "Mémoire Cognitive ARO (Ouvrir les réglages)" : "ARO Cognitive Memory (Open settings)"}
          on:click={() => onOpenMemorySettings?.()}
        >
          <Brain size={12} />
          <span>{language === "fr" ? "Mémoire" : "Memory"}</span>
        </button>
      {/if}

      <div class="conversation-menu-container">
        <button
          class="conversation-menu-btn"
          class:active={showConversationMenu}
          type="button"
          title={labels.conversationOptions}
          aria-label={labels.conversationOptions}
          on:click={() => (showConversationMenu = !showConversationMenu)}
        >
          <MoreHorizontal size={15} />
        </button>
        {#if showConversationMenu}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="conversation-dropdown" on:mouseleave={() => (showConversationMenu = false)}>
            <button
              type="button"
              class="dropdown-item"
              disabled={cloudWriteLocked}
              title={cloudWriteDisabledTitle("renommer une conversation") ?? labels.renameConversationDropdown}
              on:click={() => {
                showConversationMenu = false;
                if (activeConversation) onRenameConversation(activeConversation);
              }}
            >
              <Edit2 size={13} />
              <span>{labels.renameConversationDropdown}</span>
            </button>
            {#if onMoveConversation}
              <button
                type="button"
                class="dropdown-item"
                disabled={cloudWriteLocked}
                on:click={() => {
                  showConversationMenu = false;
                  if (activeConversation) onMoveConversation?.(activeConversation);
                }}
              >
                <FolderTree size={13} />
                <span>Classer / Déplacer</span>
              </button>
            {/if}
            <button
              type="button"
              class="dropdown-item"
              on:click={() => {
                showConversationMenu = false;
                if (activeConversation) onExportConversation?.(activeConversation, 'markdown');
              }}
            >
              <Download size={13} />
              <span>Exporter (Markdown)</span>
            </button>
            <button
              type="button"
              class="dropdown-item"
              on:click={() => {
                showConversationMenu = false;
                if (activeConversation) onExportConversation?.(activeConversation, 'json');
              }}
            >
              <Download size={13} />
              <span>Exporter (JSON)</span>
            </button>
            <button
              type="button"
              class="dropdown-item danger"
              disabled={cloudWriteLocked}
              title={cloudWriteDisabledTitle("supprimer une conversation") ?? labels.deleteConversationDropdown}
              on:click={(event) => {
                showConversationMenu = false;
                if (activeConversation) onRemoveConversation(activeConversation, event);
              }}
            >
              <Trash2 size={13} />
              <span>{labels.deleteConversationDropdown}</span>
            </button>
          </div>
        {/if}
      </div>
    {/if}
  </div>

  <div class="topbar-actions">
    <button
      class="topbar-action-btn"
      class:active={showRightPanel}
      type="button"
      title={showRightPanel ? (language === "fr" ? "Fermer le volet latéral droit" : "Close right panel") : (language === "fr" ? "Ouvrir le volet latéral droit" : "Open right panel")}
      aria-label={language === "fr" ? "Volet latéral droit" : "Right panel"}
      aria-pressed={showRightPanel}
      on:click={() => (showRightPanel = !showRightPanel)}
    >
      <PanelRight size={17} />
    </button>
  </div>
</header>

<style>
  .topbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .topbar-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-left: auto;
    flex-shrink: 0;
  }

  .topbar-action-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border-radius: 8px;
    background: transparent;
    border: 1px solid transparent;
    color: #6e6e73;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .topbar-action-btn:hover {
    background: rgba(0, 0, 0, 0.05);
    border-color: rgba(0, 0, 0, 0.08);
    color: #1d1d1f;
  }

  :global(body.dark-theme) .topbar-action-btn {
    color: #a1a1a6;
  }

  :global(body.dark-theme) .topbar-action-btn:hover {
    background: rgba(255, 255, 255, 0.08);
    border-color: rgba(255, 255, 255, 0.12);
    color: #ffffff;
  }

  .topbar-action-btn.active {
    background: rgba(0, 113, 227, 0.12);
    border-color: rgba(0, 113, 227, 0.25);
    color: #0071e3;
  }

  :global(body.dark-theme) .topbar-action-btn.active {
    background: rgba(56, 189, 248, 0.14);
    border-color: rgba(56, 189, 248, 0.3);
    color: #38bdf8;
    box-shadow: 0 0 10px rgba(56, 189, 248, 0.15);
  }

  .topbar-project-badge {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px;
    border-radius: 12px;
    background: rgba(0, 0, 0, 0.03);
    border: 1px solid rgba(0, 0, 0, 0.05);
    font-size: 11px;
    font-weight: 500;
    margin-left: 8px;
    cursor: pointer;
    position: relative;
    color: #1d1d1f;
    transition: all 0.15s ease;
  }

  :global(body.dark-theme) .topbar-project-badge {
    background: rgba(255, 255, 255, 0.04);
    border-color: rgba(255, 255, 255, 0.06);
    color: #f5f5f7;
  }

  .topbar-project-badge:hover {
    border-color: #3b82f6;
  }

  .badge-color-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .badge-project-name {
    font-weight: 600;
  }

  .project-vault-popover {
    position: absolute;
    top: 28px;
    left: 0;
    width: 280px;
    background: #ffffff;
    border: 1px solid rgba(0, 0, 0, 0.1);
    border-radius: 12px;
    box-shadow: 0 10px 25px rgba(0, 0, 0, 0.12);
    z-index: 1000;
    padding: 12px;
    cursor: default;
  }

  :global(body.dark-theme) .project-vault-popover {
    background: #1e1e20;
    border-color: rgba(255, 255, 255, 0.12);
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.4);
  }

  .popover-header {
    margin-bottom: 8px;
    padding-bottom: 6px;
    border-bottom: 1px solid rgba(0, 0, 0, 0.06);
  }

  :global(body.dark-theme) .popover-header {
    border-bottom-color: rgba(255, 255, 255, 0.08);
  }

  .popover-title {
    font-size: 0.75rem;
    font-weight: 700;
    color: #3b82f6;
  }

  .vault-instructions-text {
    font-size: 0.78rem;
    line-height: 1.45;
    color: #334155;
    white-space: pre-wrap;
    max-height: 140px;
    overflow-y: auto;
  }

  :global(body.dark-theme) .vault-instructions-text {
    color: #cbd5e1;
  }

  .vault-empty-text {
    font-size: 0.75rem;
    color: #64748b;
    font-style: italic;
  }
</style>
