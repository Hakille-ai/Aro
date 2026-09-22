<script context="module" lang="ts">
  export type ComposerAttachedFile = {
    id: string;
    name: string;
    size: number;
    type: string;
    // globalThis explicite : l'icône Lucide `FileIcon` importée dans le
    // script d'instance ne doit jamais capturer ce nom de type DOM.
    file: globalThis.File;
    mode: "local-reference" | "cloud-object";
    uploadStatus: "local" | "uploading" | "uploaded" | "failed";
    fileId?: string | null;
    note?: string;
    error?: string;
  };

  export type VoiceInputMode = "push-to-talk" | "dictation" | "hands-free";
  export type VoiceModeOption = { id: VoiceInputMode; label: string; title: string };
  export type ComposerLabels = {
    addFilesTitle: string;
    delete: string;
    askAroPlaceholder: string;
    stopSpeakingBtn: string;
    recordBtnTitle: string;
    chooseModelTitle: string;
    missingText: string;
    sendBtnTitle: string;
  };
</script>

<script lang="ts">
  import Box from "@lucide/svelte/icons/box";
  import Check from "@lucide/svelte/icons/check";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Circle from "@lucide/svelte/icons/circle";
  import CloudUpload from "@lucide/svelte/icons/cloud-upload";
  import FileText from "@lucide/svelte/icons/file-text";
  import FolderKanban from "@lucide/svelte/icons/folder-kanban";
  import Globe2 from "@lucide/svelte/icons/globe-2";
  import Lock from "@lucide/svelte/icons/lock";
  import Mic from "@lucide/svelte/icons/mic";
  import MoreVertical from "@lucide/svelte/icons/more-vertical";
  import Plus from "@lucide/svelte/icons/plus";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import Search from "@lucide/svelte/icons/search";
  import Send from "@lucide/svelte/icons/send";
  import Settings from "@lucide/svelte/icons/settings";
  import Shield from "@lucide/svelte/icons/shield";
  import X from "@lucide/svelte/icons/x";
  import Bot from "@lucide/svelte/icons/bot";
  import Brain from "@lucide/svelte/icons/brain";
  import Cpu from "@lucide/svelte/icons/cpu";
  import Network from "@lucide/svelte/icons/network";
  import Puzzle from "@lucide/svelte/icons/puzzle";
  import Zap from "@lucide/svelte/icons/zap";
  import FileIcon from "@lucide/svelte/icons/file";
  import FolderIcon from "@lucide/svelte/icons/folder";
  import VoiceOrb from "../voice/VoiceOrb.svelte";
  import type { ModelOption, WebAccessMode, PermissionProfile, PermissionPresetMode } from "../../lib/types";
  import {
    applyMentionSelection,
    buildUnifiedMentionItems,
    detectMentionQuery,
    filterUnifiedMentionItems,
    filterWorkspaceEntries,
    type MentionCategory,
    type MentionTrigger,
    type UnifiedMentionItem,
    type WorkspaceMentionEntry,
  } from "./mention-model";

  export let labels: ComposerLabels;
  export let language: "fr" | "en";
  export let errorMessage: string;
  export let input: string;
  export let fileInput: HTMLInputElement;
  export let composerInput: HTMLTextAreaElement;
  export let attachedFiles: ComposerAttachedFile[];
  export let cloudWriteLocked: boolean;
  export let cloudAuthenticated: boolean;
  export let messagesCount: number;
  export let webAccess: WebAccessMode;
  export let voiceModeOptions: VoiceModeOption[];
  export let voiceInputMode: VoiceInputMode;
  export let voiceHandsFreeArmed: boolean;
  export let recording: boolean;
  export let assistantSpeaking: boolean;
  export let voiceVolume: number;
  export let wakeWordEnabled: boolean;
  export let recordingHint: string;
  export let voiceStateLabel: string;
  export let voiceStateHint: string;
  export let modelRuntimeReady: boolean;
  export let voiceSpeechToTextReady: boolean;
  export let voiceWakeModelReady: boolean;
  export let modelReadinessLabel: string;
  export let speechReadinessLabel: string;
  export let wakeWordReadinessLabel: string;
  export let settingsAvailable: boolean;
  export let changingModel: boolean;
  export let runtimeDetail: string | null | undefined;
  export let currentModelLabel: string;
  export let modelMenuOpen: boolean;
  export let modelSearchQuery: string;
  export let modelOptions: ModelOption[];
  export let activeModelKey: string;
  export let arenaMode: boolean;
  export let arenaSending: boolean;
  export let arenaModelA: string;
  export let arenaModelB: string;
  export let sending: boolean;
  export let cloudWriteDisabledTitle: (action?: string) => string | null | undefined;
  export let webAccessTitle: (mode?: WebAccessMode) => string;
  export let webAccessAriaLabel: (mode?: WebAccessMode) => string;
  export let formatFileSize: (size: number) => string;
  export let getWavePath: (offset: number, volume: number, height: number) => string;
  export let onSubmitMessage: () => void | Promise<void>;
  export let onFilesChange: (event: Event) => void | Promise<void>;
  export let onPreviewAttachment: (file: ComposerAttachedFile) => void | Promise<void>;
  export let onUploadAttachment: (fileId: string) => void | Promise<void>;
  export let onRemoveAttachment: (fileId: string) => void;
  export let onResizeComposer: (event?: Event) => void;
  export let onOpenFilePicker: () => void;
  export let onToggleWebAccess: () => void;
  export let onSetVoiceInputMode: (mode: VoiceInputMode) => void | Promise<void>;
  export let onToggleRecording: () => void | Promise<void>;
  export let onStopSpeaking: () => void;
  export let onSelectModel: (modelId: string) => void | Promise<void>;
  export let onOpenModelSettings: () => void;
  // Sovereign AI (navigateur) : modelIds bruts executables cote serveur, ou
  // null quand inconnu / Tauri-local (comportement actuel inchange). Les
  // modeles non locaux absents de la liste basculent en "configuration requise".
  export let serverRunnableIds: string[] | null = null;

  export let activePermissionMode: PermissionPresetMode = "standard";
  export let activePermissionLabel: string = "Standard";
  export let permissionProfiles: PermissionProfile[] = [];
  export let activePermissionProfileId: string = "";
  export let onSelectPermissionPreset: (preset: "standard" | "read-only" | "developer" | "sandbox") => void = () => {};
  export let onSelectPermissionProfile: (profileId: string) => void = () => {};
  export let onOpenPermissionSettings: () => void = () => {};
  export let onOpenVoiceLive: (() => void) | undefined = undefined;
  // Destination choisie sur la page d'accueil (null = masqué, ex. conversation ouverte).
  export let destinationLabel: string | null = null;
  export let showDestinationBadge = false;
  export let onDestinationClick: () => void = () => {};
  // M4 — @ mentions: workspace entries indexed by the app shell.
  export let workspaceMentionEntries: WorkspaceMentionEntry[] = [];
  export let skills: any[] = [];
  export let plugins: any[] = [];
  export let mcpServers: any[] = [];
  export let agents: any[] = [];

  // Reference optional voice and counter props passed by parent
  $: void [
    messagesCount,
    voiceModeOptions,
    voiceInputMode,
    voiceHandsFreeArmed,
    wakeWordEnabled,
    voiceStateLabel,
    voiceStateHint,
    voiceSpeechToTextReady,
    voiceWakeModelReady,
    speechReadinessLabel,
    wakeWordReadinessLabel,
    onSetVoiceInputMode,
  ];

  let permissionMenuOpen = false;
  let permissionPickerRef: HTMLDivElement | undefined;
  let modelPickerRef: HTMLDivElement | undefined;
  let modelSearchInputRef: HTMLInputElement | undefined;
  let selectedProviderTab: string = "all";

  interface ProviderTabItem {
    id: string;
    label: string;
    count: number;
  }

  function getModelProviderKey(model: ModelOption): string {
    if (model.local || model.providerKind === "ollama" || model.providerKind === "llama-cpp") {
      return "local";
    }
    if (model.providerKind === "openai") return "openai";
    if (model.providerKind === "anthropic") return "anthropic";
    if (model.providerKind === "google") return "google";
    if (model.providerKind === "mistral") return "mistral";
    if (
      (model.providerKind as string) === "qwen" ||
      (model.family || "").toLowerCase().includes("qwen") ||
      (model.label || "").toLowerCase().includes("qwen") ||
      (model.modelId || "").toLowerCase().includes("qwen")
    ) {
      return "qwen";
    }
    return model.providerKind || "other";
  }

  function formatProviderBadge(model: ModelOption): string {
    if (model.local || model.providerKind === "ollama") return "Ollama";
    if (model.providerKind === "llama-cpp") return "Llama.cpp";
    if (model.providerKind === "openai") return "OpenAI";
    if (model.providerKind === "anthropic") return "Claude";
    if (model.providerKind === "google") return "Gemini";
    if (model.providerKind === "mistral") return "Mistral";
    if (
      (model.providerKind as string) === "qwen" ||
      (model.family || "").toLowerCase().includes("qwen") ||
      model.label.toLowerCase().includes("qwen")
    ) {
      return "Qwen";
    }
    if (model.providerKind === "openai-compatible") return "Compatible";
    return model.providerKind ? model.providerKind.toUpperCase() : (model.local ? "Local" : "Cloud");
  }

  $: providerTabs = (() => {
    const tabs: ProviderTabItem[] = [
      { id: "all", label: language === "fr" ? "Tous" : "All", count: modelOptions.length },
    ];
    const localCount = modelOptions.filter((m) => m.local || m.providerKind === "ollama" || m.providerKind === "llama-cpp").length;
    if (localCount > 0) {
      tabs.push({ id: "local", label: "Local / Ollama", count: localCount });
    }

    const knownCloud = [
      { id: "openai", label: "OpenAI" },
      { id: "anthropic", label: "Anthropic" },
      { id: "google", label: "Gemini" },
      { id: "mistral", label: "Mistral" },
      { id: "qwen", label: "Qwen" },
    ];

    for (const cp of knownCloud) {
      const c = modelOptions.filter((m) => {
        if (m.local || m.providerKind === "ollama" || m.providerKind === "llama-cpp") return false;
        return getModelProviderKey(m) === cp.id;
      }).length;
      if (c > 0) {
        tabs.push({ id: cp.id, label: cp.label, count: c });
      }
    }

    const otherCount = modelOptions.filter(
      (m) =>
        !m.local &&
        m.providerKind !== "ollama" &&
        m.providerKind !== "llama-cpp" &&
        !tabs.some((t) => t.id !== "all" && t.id === getModelProviderKey(m))
    ).length;
    if (otherCount > 0) {
      tabs.push({ id: "other", label: language === "fr" ? "Autres" : "Other", count: otherCount });
    }

    return tabs;
  })();

  $: if (selectedProviderTab !== "all" && !providerTabs.some((t) => t.id === selectedProviderTab)) {
    selectedProviderTab = "all";
  }

  $: displayedModelList = modelOptions.filter((model) => {
    if (selectedProviderTab !== "all") {
      const provKey = getModelProviderKey(model);
      if (selectedProviderTab === "other") {
        if (provKey === "local" || ["openai", "anthropic", "google", "mistral", "qwen"].includes(provKey)) {
          return false;
        }
      } else if (provKey !== selectedProviderTab) {
        return false;
      }
    }

    const q = (modelSearchQuery || "").trim().toLowerCase();
    if (!q) return true;
    return `${model.label} ${model.modelId} ${model.family ?? ""} ${model.providerKind} ${formatProviderBadge(model)}`.toLowerCase().includes(q);
  });

  function isServerBlocked(model: ModelOption): boolean {
    if (serverRunnableIds === null) return false;
    if (
      model.local ||
      model.providerKind === "ollama" ||
      model.providerKind === "llama-cpp" ||
      model.providerKind === "mock"
    ) {
      return false;
    }
    return !serverRunnableIds.includes(model.modelId || model.id);
  }

  function serverBlockedTitle(): string {
    return language === "fr"
      ? "Non exécutable depuis le navigateur : utilisez un modèle local ou demandez l'opt-in cloud à l'administrateur"
      : "Not runnable from the browser: use a local model or ask the admin for the cloud opt-in";
  }

  $: localDisplayedModels = displayedModelList.filter((m) => m.local || m.providerKind === "ollama" || m.providerKind === "llama-cpp");
  $: connectedDisplayedModels = displayedModelList.filter((m) => !m.local && m.providerKind !== "ollama" && m.providerKind !== "llama-cpp" && m.ready && !isServerBlocked(m));
  $: needsSetupDisplayedModels = displayedModelList.filter((m) => !m.local && m.providerKind !== "ollama" && m.providerKind !== "llama-cpp" && (!m.ready || isServerBlocked(m)));

  function handleModelSearchKeydown(event: KeyboardEvent) {
    if (event.key === "Enter") {
      event.preventDefault();
      event.stopPropagation();
      const firstAvailable = displayedModelList.find((m) => (m.ready || m.local) && !isServerBlocked(m)) || displayedModelList[0];
      if (firstAvailable) {
        if ((firstAvailable.ready || firstAvailable.local) && !isServerBlocked(firstAvailable)) {
          onSelectModel(firstAvailable.id);
          modelMenuOpen = false;
        } else {
          modelMenuOpen = false;
          onOpenModelSettings();
        }
      }
      return;
    }
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      if (modelSearchQuery) {
        modelSearchQuery = "";
      } else {
        modelMenuOpen = false;
      }
      return;
    }
  }

  $: if (modelMenuOpen) {
    if (typeof requestAnimationFrame === "function") {
      requestAnimationFrame(() => {
        modelSearchInputRef?.focus();
      });
    }
  }

  // --- @ mention popover state (M4 / F4.2) ---
  let mentionTrigger: MentionTrigger | null = null;
  let mentionIndex = 0;
  let popoverRef: HTMLDivElement | undefined;
  let mentionSearchInput = "";
  let selectedMentionCategory: MentionCategory = "all";
  let searchInputRef: HTMLInputElement | undefined;

  $: allMentionItems = buildUnifiedMentionItems({
    files: workspaceMentionEntries,
    skills,
    plugins,
    mcpServers,
    agents,
    models: modelOptions,
  });

  $: categoryCounts = {
    all: allMentionItems.length,
    file: allMentionItems.filter((i) => i.category === "file").length,
    skill: allMentionItems.filter((i) => i.category === "skill").length,
    plugin: allMentionItems.filter((i) => i.category === "plugin").length,
    mcp: allMentionItems.filter((i) => i.category === "mcp").length,
    agent: allMentionItems.filter((i) => i.category === "agent").length,
    model: allMentionItems.filter((i) => i.category === "model").length,
  };

  $: effectiveMentionQuery = mentionSearchInput.trim() || (mentionTrigger?.query ?? "");

  $: mentionSuggestions =
    mentionTrigger && mentionTrigger.active
      ? filterUnifiedMentionItems(allMentionItems, effectiveMentionQuery, selectedMentionCategory, 60)
      : [];

  $: if (mentionSuggestions.length > 0 && mentionIndex >= mentionSuggestions.length) {
    mentionIndex = 0;
  }

  function refreshMentionState() {
    try {
      const cursor = composerInput ? composerInput.selectionStart : input.length;
      const next = detectMentionQuery(input, cursor);
      if (next) {
        if (!mentionTrigger || next.startIndex !== mentionTrigger.startIndex || next.query !== mentionTrigger.query) {
          mentionIndex = 0;
          mentionSearchInput = next.query;
        }
        mentionTrigger = next;
      } else {
        mentionTrigger = null;
        mentionIndex = 0;
        mentionSearchInput = "";
        selectedMentionCategory = "all";
      }
    } catch {
      mentionTrigger = null;
    }
  }

  function dismissMentions() {
    mentionTrigger = null;
    mentionIndex = 0;
    mentionSearchInput = "";
    selectedMentionCategory = "all";
  }

  function selectCategory(cat: MentionCategory) {
    selectedMentionCategory = cat;
    mentionIndex = 0;
  }

  // External resets (e.g. message sent clears `input`) must close the popover.
  $: if (!input && mentionTrigger) {
    dismissMentions();
  }

  function selectMention(entry: UnifiedMentionItem | WorkspaceMentionEntry) {
    if (!mentionTrigger) return;
    const token = "insertToken" in entry ? entry.insertToken : entry.relativePath;
    const { newText, newCursor } = applyMentionSelection(input, mentionTrigger, token);
    input = newText;
    dismissMentions();
    onResizeComposer();
    const refocus = () => {
      try {
        composerInput?.focus();
        composerInput?.setSelectionRange(newCursor, newCursor);
      } catch {
        /* focus indisponible : on ignore */
      }
      refreshMentionState();
    };
    if (typeof requestAnimationFrame === "function") requestAnimationFrame(refocus);
    else refocus();
  }

  function scrollSelectedMentionIntoView() {
    if (typeof requestAnimationFrame === "function") {
      requestAnimationFrame(() => {
        const el = popoverRef?.querySelector<HTMLElement>(".mention-item.selected");
        el?.scrollIntoView({ block: "nearest" });
      });
    }
  }

  function handleComposerKeydown(event: KeyboardEvent) {
    if (mentionTrigger) {
      if (event.key === "Escape") {
        event.preventDefault();
        dismissMentions();
        return;
      }
      if (mentionSuggestions.length > 0) {
        if (event.key === "ArrowDown") {
          event.preventDefault();
          mentionIndex = (mentionIndex + 1) % mentionSuggestions.length;
          scrollSelectedMentionIntoView();
          return;
        }
        if (event.key === "ArrowUp") {
          event.preventDefault();
          mentionIndex = (mentionIndex - 1 + mentionSuggestions.length) % mentionSuggestions.length;
          scrollSelectedMentionIntoView();
          return;
        }
        if (event.key === "Enter" || event.key === "Tab") {
          if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
            dismissMentions();
            event.preventDefault();
            onSubmitMessage();
            return;
          }
          event.preventDefault();
          selectMention(mentionSuggestions[mentionIndex] ?? mentionSuggestions[0]);
          return;
        }
      }
    }
    if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
      event.preventDefault();
      dismissMentions();
      onSubmitMessage();
    }
  }
</script>

<svelte:window
  on:click={(event) => {
    if (permissionMenuOpen && permissionPickerRef && event.target instanceof Node && !permissionPickerRef.contains(event.target)) {
      permissionMenuOpen = false;
    }
    if (modelMenuOpen && modelPickerRef && event.target instanceof Node && !modelPickerRef.contains(event.target)) {
      modelMenuOpen = false;
    }
  }}
    on:keydown={(event) => {
      if (event.key === "Escape") {
        permissionMenuOpen = false;
        modelMenuOpen = false;
        dismissMentions();
      }
    }}
/>

<div class="composer-wrapper">
  {#if errorMessage}
    <div class="error-strip">
      <span>{errorMessage}</span>
      <button type="button" on:click={() => (errorMessage = "")} aria-label="Dismiss">
        <X size={14} />
      </button>
    </div>
  {/if}

  <form class="composer" on:submit|preventDefault={onSubmitMessage}>
    <input
      bind:this={fileInput}
      class="file-input"
      type="file"
      multiple
      disabled={cloudWriteLocked}
      aria-label={labels.addFilesTitle}
      on:change={onFilesChange}
    />

    {#if attachedFiles.length > 0}
      <div class="attachment-row" aria-label="Attached files">
        {#each attachedFiles as file}
          <div class:limited={file.note} class="attachment-chip" title={file.error ?? file.note ?? file.name}>
            <FileText size={14} />
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <span class="attachment-name clickable" on:click={() => onPreviewAttachment(file)} title="AperÃ§u">{file.name}</span>
            <span class="attachment-size">{formatFileSize(file.size)}</span>
            {#if file.mode === "cloud-object"}
              <span class="attachment-mode">Cloud</span>
            {:else}
              <button
                type="button"
                class="attachment-action"
                disabled={file.uploadStatus === "uploading" || !cloudAuthenticated}
                title={cloudAuthenticated ? "Uploader au cloud" : "Cloud login required"}
                aria-label={`Uploader ${file.name} au cloud`}
                on:click={() => onUploadAttachment(file.id)}
              >
                {#if file.uploadStatus === "uploading"}
                  <RefreshCw size={13} />
                {:else}
                  <CloudUpload size={13} />
                {/if}
              </button>
            {/if}
            <button
              type="button"
              aria-label={`${labels.delete} ${file.name}`}
              on:click={() => onRemoveAttachment(file.id)}
            >
              <X size={13} />
            </button>
          </div>
        {/each}
      </div>
    {/if}

    <!-- Reorganized composer: Textarea on top, full width -->
    <div class="composer-field">
      {#if mentionTrigger && mentionTrigger.active}
        <div bind:this={popoverRef} class="mention-popover" role="listbox" aria-label={language === "fr" ? "Fichiers du projet" : "Project files"}>
          <!-- Apple Spotlight Search Header -->
          <div class="mention-header">
            <div class="mention-search-bar">
              <Search size={14} class="mention-search-icon" />
              <input
                bind:this={searchInputRef}
                type="search"
                class="mention-search-input"
                placeholder={language === "fr" ? "Rechercher fichiers, skills, plugins, agents, modèles..." : "Search files, skills, plugins, agents, models..."}
                bind:value={mentionSearchInput}
                on:keydown={(e) => {
                  if (e.key === "ArrowDown" || e.key === "ArrowUp" || e.key === "Enter" || e.key === "Escape") {
                    handleComposerKeydown(e);
                  }
                }}
              />
              {#if mentionSearchInput}
                <button
                  type="button"
                  class="mention-search-clear"
                  on:click={() => {
                    mentionSearchInput = "";
                    searchInputRef?.focus();
                  }}
                  title={language === "fr" ? "Effacer la recherche" : "Clear search"}
                >
                  <X size={12} />
                </button>
              {/if}
            </div>

            <!-- Category Pills Bar -->
            <div class="mention-categories-bar" role="tablist">
              <button
                type="button"
                role="tab"
                aria-selected={selectedMentionCategory === "all"}
                class="mention-cat-pill"
                class:active={selectedMentionCategory === "all"}
                on:click={() => selectCategory("all")}
              >
                <span>{language === "fr" ? "Tout" : "All"}</span>
                <span class="cat-count">{categoryCounts.all}</span>
              </button>

              {#if categoryCounts.file > 0}
                <button
                  type="button"
                  role="tab"
                  aria-selected={selectedMentionCategory === "file"}
                  class="mention-cat-pill"
                  class:active={selectedMentionCategory === "file"}
                  on:click={() => selectCategory("file")}
                >
                  <FolderIcon size={12} />
                  <span>{language === "fr" ? "Fichiers" : "Files"}</span>
                  <span class="cat-count">{categoryCounts.file}</span>
                </button>
              {/if}

              {#if categoryCounts.skill > 0}
                <button
                  type="button"
                  role="tab"
                  aria-selected={selectedMentionCategory === "skill"}
                  class="mention-cat-pill"
                  class:active={selectedMentionCategory === "skill"}
                  on:click={() => selectCategory("skill")}
                >
                  <Zap size={12} />
                  <span>Skills</span>
                  <span class="cat-count">{categoryCounts.skill}</span>
                </button>
              {/if}

              {#if categoryCounts.plugin > 0}
                <button
                  type="button"
                  role="tab"
                  aria-selected={selectedMentionCategory === "plugin"}
                  class="mention-cat-pill"
                  class:active={selectedMentionCategory === "plugin"}
                  on:click={() => selectCategory("plugin")}
                >
                  <Puzzle size={12} />
                  <span>Plugins</span>
                  <span class="cat-count">{categoryCounts.plugin}</span>
                </button>
              {/if}

              {#if categoryCounts.mcp > 0}
                <button
                  type="button"
                  role="tab"
                  aria-selected={selectedMentionCategory === "mcp"}
                  class="mention-cat-pill"
                  class:active={selectedMentionCategory === "mcp"}
                  on:click={() => selectCategory("mcp")}
                >
                  <Network size={12} />
                  <span>MCP</span>
                  <span class="cat-count">{categoryCounts.mcp}</span>
                </button>
              {/if}

              {#if categoryCounts.agent > 0}
                <button
                  type="button"
                  role="tab"
                  aria-selected={selectedMentionCategory === "agent"}
                  class="mention-cat-pill"
                  class:active={selectedMentionCategory === "agent"}
                  on:click={() => selectCategory("agent")}
                >
                  <Bot size={12} />
                  <span>Agents</span>
                  <span class="cat-count">{categoryCounts.agent}</span>
                </button>
              {/if}

              {#if categoryCounts.model > 0}
                <button
                  type="button"
                  role="tab"
                  aria-selected={selectedMentionCategory === "model"}
                  class="mention-cat-pill"
                  class:active={selectedMentionCategory === "model"}
                  on:click={() => selectCategory("model")}
                >
                  <Cpu size={12} />
                  <span>{language === "fr" ? "Modèles" : "Models"}</span>
                  <span class="cat-count">{categoryCounts.model}</span>
                </button>
              {/if}
            </div>
          </div>

          <!-- Items List Scroll -->
          <div class="mention-list-scroll">
            {#if mentionSuggestions.length === 0}
              <div class="mention-empty">
                {language === "fr" ? "Aucun élément correspondant." : "No matching files."}
              </div>
            {:else}
              {#each mentionSuggestions as entry, index}
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <div
                  class="mention-item"
                  class:selected={index === mentionIndex}
                  role="option"
                  tabindex="-1"
                  aria-selected={index === mentionIndex}
                  title={entry.subtitle || entry.title}
                  on:click={() => selectMention(entry)}
                  on:mouseenter={() => (mentionIndex = index)}
                >
                  <div class="mention-icon-badge {entry.category} {entry.isDir ? 'is-dir' : ''}">
                    {#if entry.category === "file"}
                      {#if entry.isDir}
                        <FolderIcon size={14} />
                      {:else}
                        <FileIcon size={14} />
                      {/if}
                    {:else if entry.category === "skill"}
                      <Zap size={14} />
                    {:else if entry.category === "plugin"}
                      <Puzzle size={14} />
                    {:else if entry.category === "mcp"}
                      <Network size={14} />
                    {:else if entry.category === "agent"}
                      <Bot size={14} />
                    {:else if entry.category === "model"}
                      <Cpu size={14} />
                    {/if}
                  </div>

                  <div class="mention-text">
                    <div class="mention-title-row">
                      <span class="mention-name">{entry.title}</span>
                      {#if entry.badge}
                        <span class="mention-badge-tag {entry.category}">{entry.badge}</span>
                      {/if}
                    </div>
                    {#if entry.subtitle}
                      <span class="mention-path">{entry.subtitle}</span>
                    {/if}
                  </div>
                </div>
              {/each}
            {/if}
          </div>

          <!-- Apple Keyboard Hint Footer -->
          <div class="mention-hint">
            <span class="hint-group"><kbd>↑</kbd><kbd>↓</kbd> {language === "fr" ? "naviguer" : "navigate"}</span>
            <span class="hint-group"><kbd>↵</kbd> {language === "fr" ? "insérer" : "insert"}</span>
            <span class="hint-group"><kbd>esc</kbd> {language === "fr" ? "fermer" : "dismiss"}</span>
          </div>
        </div>
      {/if}
      <textarea
        bind:this={composerInput}
        bind:value={input}
        placeholder={cloudWriteLocked ? (cloudWriteDisabledTitle("envoyer un message") ?? labels.askAroPlaceholder) : (recordingHint || labels.askAroPlaceholder)}
        rows="1"
        disabled={cloudWriteLocked}
        on:input={() => {
          onResizeComposer();
          refreshMentionState();
        }}
        on:click={refreshMentionState}
        on:keyup={refreshMentionState}
        on:keydown={handleComposerKeydown}
      ></textarea>
    </div>

    <!-- Controls/Buttons row below -->
    <div class="composer-row">
      <div class="composer-row-left">
        {#if showDestinationBadge}
          <button
            type="button"
            class="destination-badge"
            class:placed={Boolean(destinationLabel)}
            title={language === "fr" ? "Projet / dossier de la nouvelle conversation" : "New conversation's project / folder"}
            disabled={cloudWriteLocked}
            on:click={onDestinationClick}
          >
            <FolderKanban size={14} />
            <span>{destinationLabel ?? (language === "fr" ? "Sans classement" : "Unassigned")}</span>
          </button>
        {/if}
        <button
          class="attach-button"
          type="button"
          title={cloudWriteDisabledTitle("ajouter une piece jointe") ?? labels.addFilesTitle}
          aria-label={labels.addFilesTitle}
          disabled={cloudWriteLocked}
          on:click={onOpenFilePicker}
        >
          <Plus size={19} />
        </button>

        <button
          class="web-toggle-button"
          class:web-auto={webAccess === "auto"}
          class:web-on={webAccess === "on"}
          type="button"
          title={webAccessTitle()}
          aria-label={webAccessAriaLabel()}
          aria-pressed={webAccess !== "off"}
          disabled={cloudWriteLocked}
          on:click={onToggleWebAccess}
        >
          <Globe2 size={18} />
        </button>

        <button
          class="mic-button"
          class:recording
          class:speaking={assistantSpeaking}
          class:idle={!recording && !assistantSpeaking}
          style="--volume: {voiceVolume};"
          type="button"
          title={assistantSpeaking ? labels.stopSpeakingBtn : (cloudWriteDisabledTitle("dicter un message") ?? labels.recordBtnTitle)}
          aria-label={assistantSpeaking ? labels.stopSpeakingBtn : labels.recordBtnTitle}
          disabled={cloudWriteLocked}
          on:click={onToggleRecording}
        >
          <div class="mic-button-glow"></div>
          <div class="mic-button-ring ring-1"></div>
          <div class="mic-button-ring ring-2"></div>
          <div class="mic-button-core">
            {#if assistantSpeaking}
              <div class="mini-siri-wave">
                <span></span>
                <span></span>
                <span></span>
              </div>
            {:else}
              <Mic size={20} />
            {/if}
          </div>
        </button>

        <button
          class="live-orb-button"
          type="button"
          title={language === "fr" ? "Ouvrir l'expérience vocale AGI Live" : "Open AGI Voice Live experience"}
          aria-label="AGI Voice Live"
          on:click={onOpenVoiceLive}
        >
          <VoiceOrb size={26} interactive={false} />
          <span class="live-pill-tag">Live</span>
        </button>

        {#if assistantSpeaking}
          <button class="voice-stop-button compact" type="button" on:click={onStopSpeaking}>
            <X size={14} />
            <span>{labels.stopSpeakingBtn}</span>
          </button>
        {/if}
      </div>

      <div class="composer-row-right">
        <div class="permission-picker" bind:this={permissionPickerRef}>
          <button
            class="permission-button {activePermissionMode}"
            type="button"
            title={language === "fr" ? "Niveau d'autorisations de l'IA" : "AI permission level"}
            aria-label="Permissions"
            aria-expanded={permissionMenuOpen}
            disabled={cloudWriteLocked}
            on:click={() => {
              permissionMenuOpen = !permissionMenuOpen;
              if (permissionMenuOpen) modelMenuOpen = false;
            }}
          >
            {#if activePermissionMode === "standard"}
              <Shield size={14} />
            {:else if activePermissionMode === "read-only"}
              <Lock size={14} />
            {:else if activePermissionMode === "developer"}
              <Zap size={14} />
            {:else if activePermissionMode === "sandbox"}
              <Box size={14} />
            {:else}
              <Settings size={14} />
            {/if}
            <span class="permission-label">{activePermissionLabel}</span>
            <ChevronDown size={14} />
          </button>

          {#if permissionMenuOpen}
            <div class="permission-menu" role="listbox" aria-label="Permissions">
              <div class="permission-menu-header">
                <span>{language === "fr" ? "AUTONOMIE & PERMISSIONS" : "AUTONOMY & PERMISSIONS"}</span>
              </div>

              <button
                class="permission-option"
                class:active={activePermissionMode === "standard"}
                type="button"
                role="option"
                aria-selected={activePermissionMode === "standard"}
                on:click={() => {
                  onSelectPermissionPreset("standard");
                  permissionMenuOpen = false;
                }}
              >
                <div class="perm-icon-box perm-bg-standard">
                  <Shield size={15} />
                </div>
                <div class="perm-text-box">
                  <div class="perm-title-row">
                    <span class="perm-title">Standard</span>
                    <span class="perm-badge-recommended">{language === "fr" ? "Recommandé" : "Recommended"}</span>
                  </div>
                  <span class="perm-desc">
                    {language === "fr" ? "Fichiers & web autorisés, confirmation shell" : "Files & web allowed, shell requires confirmation"}
                  </span>
                </div>
                {#if activePermissionMode === "standard"}
                  <Check size={14} class="perm-check" />
                {/if}
              </button>

              <button
                class="permission-option"
                class:active={activePermissionMode === "read-only"}
                type="button"
                role="option"
                aria-selected={activePermissionMode === "read-only"}
                on:click={() => {
                  onSelectPermissionPreset("read-only");
                  permissionMenuOpen = false;
                }}
              >
                <div class="perm-icon-box perm-bg-readonly">
                  <Lock size={15} />
                </div>
                <div class="perm-text-box">
                  <div class="perm-title-row">
                    <span class="perm-title">{language === "fr" ? "Lecture seule" : "Read-only"}</span>
                  </div>
                  <span class="perm-desc">
                    {language === "fr" ? "Consultation sans modification de fichiers" : "Read files without any write or execution"}
                  </span>
                </div>
                {#if activePermissionMode === "read-only"}
                  <Check size={14} class="perm-check" />
                {/if}
              </button>

              <button
                class="permission-option"
                class:active={activePermissionMode === "developer"}
                type="button"
                role="option"
                aria-selected={activePermissionMode === "developer"}
                on:click={() => {
                  onSelectPermissionPreset("developer");
                  permissionMenuOpen = false;
                }}
              >
                <div class="perm-icon-box perm-bg-developer">
                  <Zap size={15} />
                </div>
                <div class="perm-text-box">
                  <div class="perm-title-row">
                    <span class="perm-title">{language === "fr" ? "Autonome / Développeur" : "Autonomous / Dev"}</span>
                  </div>
                  <span class="perm-desc">
                    {language === "fr" ? "Plein accès dev, terminal & outils sans confirmation" : "Full dev access, terminal & scripts without prompts"}
                  </span>
                </div>
                {#if activePermissionMode === "developer"}
                  <Check size={14} class="perm-check" />
                {/if}
              </button>

              <button
                class="permission-option"
                class:active={activePermissionMode === "sandbox"}
                type="button"
                role="option"
                aria-selected={activePermissionMode === "sandbox"}
                on:click={() => {
                  onSelectPermissionPreset("sandbox");
                  permissionMenuOpen = false;
                }}
              >
                <div class="perm-icon-box perm-bg-sandbox">
                  <Box size={15} />
                </div>
                <div class="perm-text-box">
                  <div class="perm-title-row">
                    <span class="perm-title">{language === "fr" ? "Isolé (Sandbox)" : "Isolated (Sandbox)"}</span>
                  </div>
                  <span class="perm-desc">
                    {language === "fr" ? "Raisonnement pur, aucun accès fichiers ni réseau" : "Pure reasoning, zero file or network access"}
                  </span>
                </div>
                {#if activePermissionMode === "sandbox"}
                  <Check size={14} class="perm-check" />
                {/if}
              </button>

              {#if permissionProfiles && permissionProfiles.length > 0}
                <div class="permission-menu-section">
                  <span>{language === "fr" ? "Profils personnalisés" : "Custom profiles"}</span>
                </div>
                {#each permissionProfiles as profile}
                  <button
                    class="permission-option"
                    class:active={activePermissionMode === "custom" && activePermissionProfileId === profile.id}
                    type="button"
                    role="option"
                    aria-selected={activePermissionMode === "custom" && activePermissionProfileId === profile.id}
                    on:click={() => {
                      onSelectPermissionProfile(profile.id);
                      permissionMenuOpen = false;
                    }}
                  >
                    <div class="perm-icon-box perm-bg-custom">
                      <Settings size={14} />
                    </div>
                    <div class="perm-text-box">
                      <span class="perm-title">{profile.name}</span>
                      <span class="perm-desc">
                        {profile.allowWrite ? (language === "fr" ? "Écriture" : "Write") : (language === "fr" ? "Lecture" : "Read")}
                        • {profile.allowShell ? "Shell" : (language === "fr" ? "Sans shell" : "No shell")}
                      </span>
                    </div>
                    {#if activePermissionMode === "custom" && activePermissionProfileId === profile.id}
                      <Check size={14} class="perm-check" />
                    {/if}
                  </button>
                {/each}
              {/if}

              <button
                class="permission-menu-manage"
                type="button"
                on:click={() => {
                  permissionMenuOpen = false;
                  onOpenPermissionSettings();
                }}
              >
                <Settings size={14} />
                <span>{language === "fr" ? "Gérer les permissions..." : "Manage permissions..."}</span>
              </button>
            </div>
          {/if}
        </div>

        <div class="model-picker" bind:this={modelPickerRef}>
          <button
            class="model-button"
            class:ready={modelRuntimeReady}
            class:missing={!modelRuntimeReady}
            type="button"
            title={cloudWriteDisabledTitle("changer de modèle") ?? runtimeDetail ?? modelReadinessLabel}
            aria-label={labels.chooseModelTitle}
            aria-expanded={modelMenuOpen}
            disabled={!settingsAvailable || changingModel || cloudWriteLocked}
            on:click={() => {
              modelMenuOpen = !modelMenuOpen;
              if (modelMenuOpen) permissionMenuOpen = false;
            }}
          >
            {#if modelRuntimeReady}
              <Check size={14} />
            {:else}
              <Circle size={14} />
            {/if}
            <span>{currentModelLabel}</span>
            <ChevronDown size={15} />
          </button>

          {#if modelMenuOpen}
            <div class="model-menu" role="listbox" aria-label="Models">
              <div class="model-menu-header">
                <div class="model-menu-top-row">
                  <span class="model-menu-title">{language === "fr" ? "Modèle d'IA" : "AI Model"}</span>
                  <span class="model-menu-total">
                    {displayedModelList.length} {displayedModelList.length > 1 ? (language === "fr" ? "modèles" : "models") : (language === "fr" ? "modèle" : "model")}
                  </span>
                </div>

                <div class="model-menu-search">
                  <Search size={13} />
                  <input
                    bind:this={modelSearchInputRef}
                    bind:value={modelSearchQuery}
                    on:keydown={handleModelSearchKeydown}
                    placeholder={language === "fr" ? "Rechercher un modèle ou fournisseur..." : "Search models or providers..."}
                  />
                  {#if modelSearchQuery}
                    <button
                      type="button"
                      class="model-search-clear"
                      title={language === "fr" ? "Effacer la recherche" : "Clear search"}
                      on:click={() => (modelSearchQuery = "")}
                    >
                      <X size={12} />
                    </button>
                  {/if}
                </div>

                {#if providerTabs.length > 1}
                  <div class="model-provider-tabs" role="tablist" aria-label={language === "fr" ? "Fournisseurs de modèles" : "Model providers"}>
                    {#each providerTabs as tab}
                      <button
                        type="button"
                        class="provider-tab-pill"
                        class:active={selectedProviderTab === tab.id}
                        role="tab"
                        aria-selected={selectedProviderTab === tab.id}
                        on:click={() => (selectedProviderTab = tab.id)}
                      >
                        <span>{tab.label}</span>
                        <span class="tab-count-badge">{tab.count}</span>
                      </button>
                    {/each}
                  </div>
                {/if}
              </div>

              <div class="model-menu-list">
                {#if displayedModelList.length === 0}
                  <div class="model-menu-empty">
                    <Box size={22} />
                    <span>{language === "fr" ? "Aucun modèle trouvé" : "No models found"}</span>
                    {#if modelSearchQuery || selectedProviderTab !== "all"}
                      <button
                        type="button"
                        class="reset-filters-btn"
                        on:click={() => {
                          modelSearchQuery = "";
                          selectedProviderTab = "all";
                        }}
                      >
                        {language === "fr" ? "Réinitialiser les filtres" : "Reset filters"}
                      </button>
                    {/if}
                  </div>
                {:else if selectedProviderTab === "all"}
                  {#if localDisplayedModels.length > 0}
                    <div class="model-menu-section">{language === "fr" ? "Local / Ollama" : "Local / Ollama"}</div>
                    {#each localDisplayedModels as model}
                      <button
                        class="model-option-item"
                        class:active={model.id === activeModelKey}
                        class:disabled={cloudWriteLocked || !model.ready}
                        type="button"
                        role="option"
                        aria-selected={model.id === activeModelKey}
                        disabled={cloudWriteLocked || !model.ready}
                        on:click={() => {
                          onSelectModel(model.id);
                          modelMenuOpen = false;
                        }}
                      >
                        <div class="model-option-content">
                          <div class="model-status-dot" class:ready={model.ready} class:missing={!model.ready}></div>
                          <div class="model-text-stack">
                            <div class="model-title-row">
                              <span class="model-name">{model.label}</span>
                              <span class="provider-pill pill-{model.providerKind}">{formatProviderBadge(model)}</span>
                            </div>
                            <div class="model-subtitle-row">
                              <span class="model-family-text">{model.family || model.modelId}</span>
                              {#if !model.ready}
                                <span class="model-missing-hint">{labels.missingText}</span>
                              {/if}
                            </div>
                          </div>
                        </div>
                        {#if model.id === activeModelKey}
                          <Check size={14} class="model-check-icon" />
                        {/if}
                      </button>
                    {/each}
                  {/if}

                  {#if connectedDisplayedModels.length > 0}
                    <div class="model-menu-section">{language === "fr" ? "Connecté (Cloud)" : "Connected"}</div>
                    {#each connectedDisplayedModels as model}
                      <button
                        class="model-option-item"
                        class:active={model.id === activeModelKey}
                        type="button"
                        role="option"
                        aria-selected={model.id === activeModelKey}
                        disabled={cloudWriteLocked}
                        on:click={() => {
                          onSelectModel(model.id);
                          modelMenuOpen = false;
                        }}
                      >
                        <div class="model-option-content">
                          <div class="model-status-dot ready"></div>
                          <div class="model-text-stack">
                            <div class="model-title-row">
                              <span class="model-name">{model.label}</span>
                              <span class="provider-pill pill-{model.providerKind}">{formatProviderBadge(model)}</span>
                            </div>
                            <div class="model-subtitle-row">
                              <span class="model-family-text">{model.family || model.providerKind}</span>
                            </div>
                          </div>
                        </div>
                        {#if model.id === activeModelKey}
                          <Check size={14} class="model-check-icon" />
                        {/if}
                      </button>
                    {/each}
                  {/if}

                  {#if needsSetupDisplayedModels.length > 0}
                    <div class="model-menu-section">{language === "fr" ? "Configuration requise" : "Needs setup"}</div>
                    {#each needsSetupDisplayedModels as model}
                      <button
                        class="model-option-item needs-setup"
                        type="button"
                        role="option"
                        aria-selected={false}
                        title={isServerBlocked(model) ? serverBlockedTitle() : (language === "fr" ? "Configurer ce fournisseur dans les paramètres" : "Configure this provider in settings")}
                        on:click={() => {
                          modelMenuOpen = false;
                          onOpenModelSettings();
                        }}
                      >
                        <div class="model-option-content">
                          <div class="model-status-dot missing"></div>
                          <div class="model-text-stack">
                            <div class="model-title-row">
                              <span class="model-name">{model.label}</span>
                              <span class="provider-pill pill-{model.providerKind}">{formatProviderBadge(model)}</span>
                            </div>
                            <div class="model-subtitle-row">
                              <span class="model-family-text">{model.family || model.modelId}</span>
                              <span class="model-missing-hint">{isServerBlocked(model) ? (language === "fr" ? "Non exécutable dans le navigateur" : "Not runnable in browser") : labels.missingText}</span>
                            </div>
                          </div>
                        </div>
                      </button>
                    {/each}
                  {/if}
                {:else}
                  {#each displayedModelList as model}
                    <button
                      class="model-option-item"
                      class:active={model.id === activeModelKey}
                      class:disabled={cloudWriteLocked}
                      class:needs-setup={(!model.ready && !model.local) || isServerBlocked(model)}
                      type="button"
                      role="option"
                      aria-selected={model.id === activeModelKey}
                      disabled={cloudWriteLocked}
                      title={isServerBlocked(model) ? serverBlockedTitle() : (!model.ready && !model.local ? (language === "fr" ? "Configurer ce fournisseur dans les paramètres" : "Configure this provider in settings") : undefined)}
                      on:click={() => {
                        if ((model.ready || model.local) && !isServerBlocked(model)) {
                          onSelectModel(model.id);
                          modelMenuOpen = false;
                        } else {
                          modelMenuOpen = false;
                          onOpenModelSettings();
                        }
                      }}
                    >
                      <div class="model-option-content">
                        <div class="model-status-dot" class:ready={model.ready} class:missing={!model.ready}></div>
                        <div class="model-text-stack">
                          <div class="model-title-row">
                            <span class="model-name">{model.label}</span>
                            <span class="provider-pill pill-{model.providerKind}">{formatProviderBadge(model)}</span>
                          </div>
                          <div class="model-subtitle-row">
                            <span class="model-family-text">{model.family || model.modelId}</span>
                            {#if !model.ready}
                              <span class="model-missing-hint">{labels.missingText}</span>
                            {/if}
                          </div>
                        </div>
                      </div>
                      {#if model.id === activeModelKey}
                        <Check size={14} class="model-check-icon" />
                      {/if}
                    </button>
                  {/each}
                {/if}
              </div>

              <div class="model-menu-footer">
                <button
                  class="model-menu-manage-btn"
                  type="button"
                  on:click={() => {
                    modelMenuOpen = false;
                    onOpenModelSettings();
                  }}
                >
                  <Settings size={14} />
                  <span>{language === "fr" ? "Gérer les fournisseurs et modèles..." : "Manage providers & models..."}</span>
                </button>
              </div>
            </div>
          {/if}
        </div>

        <button
          class="arena-toggle-button"
          class:arena-active={arenaMode}
          type="button"
          title={arenaMode ? "Exit Arena Mode" : "⚔️ Enter Arena Mode — compare two models side-by-side"}
          aria-label="Toggle Arena Mode"
          on:click={() => (arenaMode = !arenaMode)}
        >
          ⚔️
        </button>

        <button
          class="send-button"
          type="submit"
          title={cloudWriteDisabledTitle("envoyer un message") ?? labels.sendBtnTitle}
          aria-label={labels.sendBtnTitle}
          disabled={cloudWriteLocked || (arenaMode ? (arenaSending || !arenaModelA || !arenaModelB) : sending) || (!input.trim() && attachedFiles.length === 0)}
        >
          {#if arenaMode ? arenaSending : sending}
            <MoreVertical size={18} />
          {:else}
            <Send size={18} />
          {/if}
        </button>
      </div>
    </div>

    {#if recording}
      <div class="composer-waveform" style="display: flex; justify-content: center; width: 100%; height: 26px; margin-top: 4px; padding: 0 12px; box-sizing: border-box;">
        <svg viewBox="0 0 200 26" style="width: 100%; max-width: 320px; height: 26px;">
          <path d={getWavePath(0, voiceVolume, 26)} fill="none" stroke="rgba(10, 132, 255, 0.8)" stroke-width="1.8" stroke-linecap="round" />
          <path d={getWavePath(2, voiceVolume * 0.6, 26)} fill="none" stroke="rgba(162, 36, 214, 0.6)" stroke-width="1.4" stroke-linecap="round" />
          <path d={getWavePath(4, voiceVolume * 0.3, 26)} fill="none" stroke="rgba(255, 107, 107, 0.5)" stroke-width="1.0" stroke-linecap="round" />
        </svg>
      </div>
    {/if}
  </form>
</div>

<style>
  .composer-field {
    position: relative;
  }
  /* ==========================================================================
     Apple-Grade Mention Popover (@) Styling
     ========================================================================== */
  .mention-popover {
    position: absolute;
    left: 0;
    right: 0;
    bottom: calc(100% + 10px);
    z-index: 70;
    max-height: 440px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    padding: 0;
    border-radius: 16px;
    background: rgba(255, 255, 255, 0.98);
    border: 1px solid rgba(0, 0, 0, 0.08);
    box-shadow: 0 20px 48px rgba(0, 0, 0, 0.14), 0 4px 14px rgba(0, 0, 0, 0.04);
    backdrop-filter: blur(24px);
    -webkit-backdrop-filter: blur(24px);
    color: #1d1d1f;
  }

  :global(body.dark-theme) .mention-popover {
    background: rgba(26, 26, 30, 0.98);
    border-color: rgba(255, 255, 255, 0.12);
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.55), 0 6px 18px rgba(0, 0, 0, 0.3);
    color: #f5f5f7;
  }

  /* Header & Search Bar */
  .mention-header {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 12px 6px 12px;
    border-bottom: 1px solid rgba(0, 0, 0, 0.06);
    background: rgba(0, 0, 0, 0.01);
  }

  :global(body.dark-theme) .mention-header {
    border-bottom-color: rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.02);
  }

  .mention-search-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 11px;
    border-radius: 10px;
    background: rgba(0, 0, 0, 0.04);
    border: 1px solid rgba(0, 0, 0, 0.06);
    transition: all 0.15s ease;
  }

  :global(body.dark-theme) .mention-search-bar {
    background: rgba(255, 255, 255, 0.06);
    border-color: rgba(255, 255, 255, 0.08);
  }

  .mention-search-bar:focus-within {
    border-color: #0071e3;
    background: #ffffff;
    box-shadow: 0 0 0 3px rgba(0, 113, 227, 0.16);
  }

  :global(body.dark-theme) .mention-search-bar:focus-within {
    border-color: #2997ff;
    background: rgba(36, 36, 42, 0.95);
    box-shadow: 0 0 0 3px rgba(41, 151, 255, 0.22);
  }

  :global(.mention-search-icon) {
    color: #86868b;
    flex-shrink: 0;
  }

  .mention-search-input {
    flex: 1;
    border: none;
    outline: none;
    background: transparent;
    font-size: 13px;
    font-weight: 450;
    color: inherit;
    padding: 0;
  }

  .mention-search-input::placeholder {
    color: #86868b;
  }

  .mention-search-clear {
    background: none;
    border: none;
    padding: 3px;
    border-radius: 50%;
    color: #86868b;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.15s ease;
  }

  .mention-search-clear:hover {
    background: rgba(0, 0, 0, 0.08);
    color: #1d1d1f;
  }

  :global(body.dark-theme) .mention-search-clear:hover {
    background: rgba(255, 255, 255, 0.15);
    color: #ffffff;
  }

  /* Category Filter Pills */
  .mention-categories-bar {
    display: flex;
    align-items: center;
    gap: 6px;
    overflow-x: auto;
    scrollbar-width: none;
    padding-bottom: 2px;
  }

  .mention-categories-bar::-webkit-scrollbar {
    display: none;
  }

  .mention-cat-pill {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 4px 9px;
    border-radius: 8px;
    font-size: 11.5px;
    font-weight: 500;
    cursor: pointer;
    border: 1px solid transparent;
    white-space: nowrap;
    background: rgba(0, 0, 0, 0.03);
    color: #6e6e73;
    transition: all 0.15s ease;
  }

  :global(body.dark-theme) .mention-cat-pill {
    background: rgba(255, 255, 255, 0.05);
    color: #a1a1a6;
  }

  .mention-cat-pill:hover {
    background: rgba(0, 0, 0, 0.06);
    color: #1d1d1f;
  }

  :global(body.dark-theme) .mention-cat-pill:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #ffffff;
  }

  .mention-cat-pill.active {
    background: #0071e3;
    color: #ffffff;
    font-weight: 600;
    box-shadow: 0 2px 6px rgba(0, 113, 227, 0.3);
  }

  :global(body.dark-theme) .mention-cat-pill.active {
    background: #2997ff;
    color: #ffffff;
    font-weight: 600;
    box-shadow: 0 2px 6px rgba(41, 151, 255, 0.35);
  }

  .cat-count {
    font-size: 10px;
    padding: 1px 5px;
    border-radius: 6px;
    background: rgba(0, 0, 0, 0.08);
  }

  :global(body.dark-theme) .cat-count {
    background: rgba(255, 255, 255, 0.12);
  }

  .mention-cat-pill.active .cat-count {
    background: rgba(255, 255, 255, 0.25);
    color: #ffffff;
  }

  /* List Scroll Area */
  .mention-list-scroll {
    flex: 1;
    overflow-y: auto;
    max-height: 290px;
    padding: 6px 8px;
    scrollbar-width: thin;
  }

  /* Mention Item */
  .mention-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 10px;
    border-radius: 10px;
    cursor: pointer;
    margin-bottom: 2px;
    transition: background-color 0.12s ease;
  }

  .mention-item:hover,
  .mention-item.selected {
    background: rgba(0, 113, 227, 0.09);
  }

  :global(body.dark-theme) .mention-item:hover,
  :global(body.dark-theme) .mention-item.selected {
    background: rgba(41, 151, 255, 0.16);
  }

  /* Icon Badge */
  .mention-icon-badge {
    width: 28px;
    height: 28px;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    background: rgba(0, 113, 227, 0.1);
    color: #0071e3;
  }

  .mention-icon-badge.file {
    background: rgba(0, 113, 227, 0.1);
    color: #0071e3;
  }

  .mention-icon-badge.file.is-dir {
    background: rgba(255, 149, 0, 0.12);
    color: #ff9500;
  }

  .mention-icon-badge.skill {
    background: rgba(255, 149, 0, 0.12);
    color: #f59e0b;
  }

  .mention-icon-badge.plugin {
    background: rgba(175, 82, 222, 0.12);
    color: #af52de;
  }

  .mention-icon-badge.mcp {
    background: rgba(52, 199, 89, 0.12);
    color: #34c759;
  }

  .mention-icon-badge.agent {
    background: rgba(88, 86, 214, 0.12);
    color: #5856d6;
  }

  .mention-icon-badge.model {
    background: rgba(255, 45, 85, 0.12);
    color: #ff2d55;
  }

  :global(body.dark-theme) .mention-icon-badge.file {
    background: rgba(41, 151, 255, 0.18);
    color: #58a6ff;
  }

  :global(body.dark-theme) .mention-icon-badge.file.is-dir {
    background: rgba(255, 159, 10, 0.2);
    color: #ffd60a;
  }

  :global(body.dark-theme) .mention-icon-badge.skill {
    background: rgba(255, 159, 10, 0.2);
    color: #ff9f0a;
  }

  :global(body.dark-theme) .mention-icon-badge.plugin {
    background: rgba(191, 90, 242, 0.2);
    color: #bf5af2;
  }

  :global(body.dark-theme) .mention-icon-badge.mcp {
    background: rgba(48, 209, 88, 0.2);
    color: #30d158;
  }

  :global(body.dark-theme) .mention-icon-badge.agent {
    background: rgba(94, 92, 230, 0.2);
    color: #9d9aff;
  }

  :global(body.dark-theme) .mention-icon-badge.model {
    background: rgba(255, 55, 95, 0.2);
    color: #ff6482;
  }

  /* Item Text */
  .mention-text {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }

  .mention-title-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .mention-name {
    font-size: 13px;
    font-weight: 600;
    color: #1d1d1f;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  :global(body.dark-theme) .mention-name {
    color: #f5f5f7;
  }

  .mention-path {
    font-size: 11px;
    color: #86868b;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    margin-top: 1px;
  }

  :global(body.dark-theme) .mention-path {
    color: #a1a1a6;
  }

  /* Category / Extension Badge */
  .mention-badge-tag {
    font-size: 9.5px;
    font-weight: 600;
    letter-spacing: 0.3px;
    padding: 1.5px 6px;
    border-radius: 5px;
    background: rgba(0, 0, 0, 0.05);
    color: #6e6e73;
    flex-shrink: 0;
  }

  :global(body.dark-theme) .mention-badge-tag {
    background: rgba(255, 255, 255, 0.08);
    color: #a1a1a6;
  }

  .mention-badge-tag.skill {
    background: rgba(255, 149, 0, 0.12);
    color: #b45309;
  }

  .mention-badge-tag.plugin {
    background: rgba(175, 82, 222, 0.12);
    color: #7e22ce;
  }

  .mention-badge-tag.mcp {
    background: rgba(52, 199, 89, 0.12);
    color: #15803d;
  }

  .mention-badge-tag.agent {
    background: rgba(88, 86, 214, 0.12);
    color: #4338ca;
  }

  .mention-badge-tag.model {
    background: rgba(255, 45, 85, 0.12);
    color: #be123c;
  }

  :global(body.dark-theme) .mention-badge-tag.skill {
    background: rgba(255, 159, 10, 0.18);
    color: #ffb340;
  }

  :global(body.dark-theme) .mention-badge-tag.plugin {
    background: rgba(191, 90, 242, 0.18);
    color: #d17fff;
  }

  :global(body.dark-theme) .mention-badge-tag.mcp {
    background: rgba(48, 209, 88, 0.18);
    color: #5ce680;
  }

  :global(body.dark-theme) .mention-badge-tag.agent {
    background: rgba(94, 92, 230, 0.18);
    color: #9d9aff;
  }

  :global(body.dark-theme) .mention-badge-tag.model {
    background: rgba(255, 55, 95, 0.18);
    color: #ff859d;
  }

  /* Empty State */
  .mention-empty {
    padding: 24px 16px;
    text-align: center;
    font-size: 12.5px;
    color: #86868b;
  }

  /* Footer Keyboard Hints */
  .mention-hint {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 12px;
    padding: 6px 12px;
    border-top: 1px solid rgba(0, 0, 0, 0.05);
    background: rgba(0, 0, 0, 0.015);
    font-size: 11px;
    color: #86868b;
  }

  :global(body.dark-theme) .mention-hint {
    border-top-color: rgba(255, 255, 255, 0.06);
    background: rgba(255, 255, 255, 0.02);
    color: #a1a1a6;
  }

  .hint-group {
    display: inline-flex;
    align-items: center;
    gap: 3px;
  }

  .mention-hint kbd {
    background: rgba(0, 0, 0, 0.05);
    border: 1px solid rgba(0, 0, 0, 0.08);
    border-radius: 4px;
    padding: 1px 5px;
    font-size: 10px;
    font-family: inherit;
    color: #1d1d1f;
  }

  :global(body.dark-theme) .mention-hint kbd {
    background: rgba(255, 255, 255, 0.1);
    border-color: rgba(255, 255, 255, 0.15);
    color: #f5f5f7;
  }

  .live-orb-button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: rgba(175, 82, 222, 0.1);
    border: 1px solid rgba(175, 82, 222, 0.28);
    border-radius: 20px;
    padding: 3px 8px 3px 4px;
    cursor: pointer;
    transition: all 0.2s cubic-bezier(0.34, 1.56, 0.64, 1);
    flex-shrink: 0;
  }

  .live-orb-button:hover {
    background: rgba(175, 82, 222, 0.2);
    border-color: rgba(175, 82, 222, 0.5);
    transform: translateY(-1px);
    box-shadow: 0 4px 14px rgba(175, 82, 222, 0.3);
  }

  .live-pill-tag {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: #df9aff;
  }

  /* Apple-grade Model Selector Popover */
  .model-picker {
    position: relative;
  }

  .model-menu {
    position: absolute;
    right: 0;
    bottom: calc(100% + 10px);
    z-index: 1000;
    display: flex;
    flex-direction: column;
    width: 380px;
    max-width: min(380px, calc(100vw - 32px));
    max-height: min(450px, calc(100vh - 130px));
    border-radius: 16px;
    background: rgba(255, 255, 255, 0.95);
    border: 1px solid rgba(0, 0, 0, 0.1);
    box-shadow: 0 24px 56px rgba(0, 0, 0, 0.22), 0 0 0 1px rgba(0, 0, 0, 0.05);
    backdrop-filter: blur(32px) saturate(180%);
    -webkit-backdrop-filter: blur(32px) saturate(180%);
    overflow: hidden;
    animation: modelMenuFadeIn 0.18s cubic-bezier(0.16, 1, 0.3, 1);
  }

  :global(body.dark-theme) .model-menu {
    background: rgba(28, 28, 32, 0.95);
    border-color: rgba(255, 255, 255, 0.12);
    box-shadow: 0 24px 64px rgba(0, 0, 0, 0.65), 0 0 0 1px rgba(255, 255, 255, 0.08);
  }

  @keyframes modelMenuFadeIn {
    from {
      opacity: 0;
      transform: translateY(6px) scale(0.98);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }

  .model-menu-header {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 12px 8px;
    border-bottom: 1px solid rgba(0, 0, 0, 0.06);
    background: rgba(255, 255, 255, 0.5);
  }

  :global(body.dark-theme) .model-menu-header {
    border-bottom-color: rgba(255, 255, 255, 0.08);
    background: rgba(36, 36, 42, 0.5);
  }

  .model-menu-top-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .model-menu-title {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: #86868b;
  }

  .model-menu-total {
    font-size: 11px;
    font-weight: 500;
    color: #86868b;
  }

  .model-menu-search {
    display: flex;
    align-items: center;
    gap: 7px;
    height: 32px;
    padding: 0 10px;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.04);
    border: 1px solid rgba(0, 0, 0, 0.06);
    color: #1d1d1f;
    transition: all 0.15s ease;
  }

  :global(body.dark-theme) .model-menu-search {
    background: rgba(255, 255, 255, 0.06);
    border-color: rgba(255, 255, 255, 0.08);
    color: #f5f5f7;
  }

  .model-menu-search:focus-within {
    background: #ffffff;
    border-color: #0071e3;
    box-shadow: 0 0 0 3px rgba(0, 113, 227, 0.15);
  }

  :global(body.dark-theme) .model-menu-search:focus-within {
    background: rgba(255, 255, 255, 0.1);
    border-color: #0a84ff;
    box-shadow: 0 0 0 3px rgba(10, 132, 255, 0.25);
  }

  .model-menu-search input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: none;
    background: transparent;
    color: inherit;
    font-size: 12px;
  }

  .model-search-clear {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 16px;
    border: none;
    background: rgba(0, 0, 0, 0.1);
    color: #666;
    border-radius: 50%;
    cursor: pointer;
    padding: 0;
    transition: background 0.15s;
  }

  :global(body.dark-theme) .model-search-clear {
    background: rgba(255, 255, 255, 0.12);
    color: #a1a1aa;
  }

  .model-search-clear:hover {
    background: rgba(0, 0, 0, 0.18);
    color: #111;
  }

  :global(body.dark-theme) .model-search-clear:hover {
    background: rgba(255, 255, 255, 0.22);
    color: #ffffff;
  }

  .model-provider-tabs {
    display: flex;
    align-items: center;
    gap: 5px;
    overflow-x: auto;
    padding-bottom: 2px;
    scrollbar-width: none;
    -webkit-overflow-scrolling: touch;
    flex-shrink: 0;
  }

  .model-provider-tabs::-webkit-scrollbar {
    display: none;
  }

  .provider-tab-pill {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 3px 9px;
    border-radius: 12px;
    border: 1px solid transparent;
    background: rgba(0, 0, 0, 0.04);
    font-size: 11px;
    font-weight: 500;
    color: #555;
    cursor: pointer;
    white-space: nowrap;
    transition: all 0.15s ease;
  }

  :global(body.dark-theme) .provider-tab-pill {
    background: rgba(255, 255, 255, 0.06);
    color: #a1a1aa;
  }

  .provider-tab-pill:hover {
    background: rgba(0, 0, 0, 0.08);
    color: #111;
  }

  :global(body.dark-theme) .provider-tab-pill:hover {
    background: rgba(255, 255, 255, 0.12);
    color: #ffffff;
  }

  .provider-tab-pill.active {
    background: #0071e3;
    color: #ffffff;
    font-weight: 600;
    box-shadow: 0 2px 6px rgba(0, 113, 227, 0.3);
  }

  :global(body.dark-theme) .provider-tab-pill.active {
    background: #0a84ff;
    color: #ffffff;
    box-shadow: 0 2px 8px rgba(10, 132, 255, 0.4);
  }

  .tab-count-badge {
    font-size: 9.5px;
    padding: 1px 4px;
    border-radius: 6px;
    background: rgba(0, 0, 0, 0.06);
  }

  :global(body.dark-theme) .tab-count-badge {
    background: rgba(255, 255, 255, 0.1);
  }

  .provider-tab-pill.active .tab-count-badge {
    background: rgba(255, 255, 255, 0.25);
    color: #ffffff;
  }

  .model-menu-list {
    flex: 1 1 auto;
    min-height: 0;
    max-height: 280px;
    overflow-y: auto !important;
    overflow-x: hidden;
    overscroll-behavior: contain;
    padding: 6px 8px;
    display: flex;
    flex-direction: column;
    gap: 3px;
    scroll-behavior: smooth;
  }

  .model-menu-list::-webkit-scrollbar {
    width: 5px;
  }

  .model-menu-list::-webkit-scrollbar-track {
    background: transparent;
  }

  .model-menu-list::-webkit-scrollbar-thumb {
    background: rgba(0, 0, 0, 0.15);
    border-radius: 3px;
  }

  :global(body.dark-theme) .model-menu-list::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.2);
  }

  .model-menu-section {
    padding: 8px 8px 3px;
    color: #86868b;
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .model-option-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 6px 10px;
    border-radius: 8px;
    border: none;
    background: transparent;
    cursor: pointer;
    text-align: left;
    transition: all 0.12s ease;
    width: 100%;
    box-sizing: border-box;
  }

  .model-option-item:hover {
    background: rgba(0, 0, 0, 0.04);
  }

  :global(body.dark-theme) .model-option-item:hover {
    background: rgba(255, 255, 255, 0.06);
  }

  .model-option-item.active {
    background: rgba(0, 113, 227, 0.08);
  }

  :global(body.dark-theme) .model-option-item.active {
    background: rgba(10, 132, 255, 0.16);
  }

  .model-option-item.disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .model-option-item.needs-setup:hover {
    background: rgba(255, 59, 48, 0.08);
  }

  :global(body.dark-theme) .model-option-item.needs-setup:hover {
    background: rgba(255, 69, 58, 0.12);
  }

  .model-option-content {
    display: flex;
    align-items: center;
    gap: 9px;
    flex: 1;
    min-width: 0;
  }

  .model-status-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex-shrink: 0;
    background: #8e8e93;
  }

  .model-status-dot.ready {
    background: #34c759;
    box-shadow: 0 0 0 2px rgba(52, 199, 89, 0.2);
  }

  .model-status-dot.missing {
    background: #ff9f0a;
  }

  .model-text-stack {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
    flex: 1;
  }

  .model-title-row {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }

  .model-name {
    font-size: 12.5px;
    font-weight: 600;
    color: #1d1d1f;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  :global(body.dark-theme) .model-name {
    color: #f5f5f7;
  }

  .provider-pill {
    font-size: 9px;
    font-weight: 700;
    padding: 1px 5px;
    border-radius: 4px;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    flex-shrink: 0;
    background: rgba(0, 0, 0, 0.05);
    color: #555;
  }

  .model-subtitle-row {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    color: #86868b;
  }

  .model-family-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .model-missing-hint {
    color: #ff9500;
    font-weight: 500;
  }

  :global(body.dark-theme) .model-missing-hint {
    color: #ff9f0a;
  }

  .model-check-icon {
    color: #0071e3;
    flex-shrink: 0;
  }

  :global(body.dark-theme) .model-check-icon {
    color: #2997ff;
  }

  .model-menu-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 28px 16px;
    color: #86868b;
    font-size: 12px;
    text-align: center;
  }

  .reset-filters-btn {
    margin-top: 4px;
    padding: 4px 10px;
    border-radius: 6px;
    border: 1px solid rgba(0, 113, 227, 0.3);
    background: rgba(0, 113, 227, 0.08);
    color: #0071e3;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  :global(body.dark-theme) .reset-filters-btn {
    border-color: rgba(41, 151, 255, 0.4);
    background: rgba(41, 151, 255, 0.12);
    color: #2997ff;
  }

  .reset-filters-btn:hover {
    background: rgba(0, 113, 227, 0.16);
  }

  .model-menu-footer {
    flex-shrink: 0;
    padding: 8px 10px;
    border-top: 1px solid rgba(0, 0, 0, 0.06);
    background: rgba(255, 255, 255, 0.3);
  }

  :global(body.dark-theme) .model-menu-footer {
    border-top-color: rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.02);
  }

  .model-menu-manage-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    width: 100%;
    padding: 6px 10px;
    border-radius: 8px;
    border: none;
    background: transparent;
    color: #0071e3;
    font-size: 12px;
    font-weight: 550;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  :global(body.dark-theme) .model-menu-manage-btn {
    color: #2997ff;
  }

  .model-menu-manage-btn:hover {
    background: rgba(0, 113, 227, 0.08);
  }

  :global(body.dark-theme) .model-menu-manage-btn:hover {
    background: rgba(41, 151, 255, 0.12);
  }
</style>
