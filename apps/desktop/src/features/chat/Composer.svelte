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
  import Zap from "@lucide/svelte/icons/zap";
  import FileIcon from "@lucide/svelte/icons/file";
  import FolderIcon from "@lucide/svelte/icons/folder";
  import type { ModelOption, WebAccessMode, PermissionProfile, PermissionPresetMode } from "../../lib/types";
  import {
    applyMentionSelection,
    detectMentionQuery,
    filterWorkspaceEntries,
    type MentionTrigger,
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

  export let activePermissionMode: PermissionPresetMode = "standard";
  export let activePermissionLabel: string = "Standard";
  export let permissionProfiles: PermissionProfile[] = [];
  export let activePermissionProfileId: string = "";
  export let onSelectPermissionPreset: (preset: "standard" | "read-only" | "developer" | "sandbox") => void = () => {};
  export let onSelectPermissionProfile: (profileId: string) => void = () => {};
  export let onOpenPermissionSettings: () => void = () => {};
  // Destination choisie sur la page d'accueil (null = masqué, ex. conversation ouverte).
  export let destinationLabel: string | null = null;
  export let showDestinationBadge = false;
  export let onDestinationClick: () => void = () => {};
  // M4 — @ mentions: workspace entries indexed by the app shell.
  export let workspaceMentionEntries: WorkspaceMentionEntry[] = [];

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

  // --- @ mention popover state (M4 / F4.2) ---
  let mentionTrigger: MentionTrigger | null = null;
  let mentionIndex = 0;
  let popoverRef: HTMLDivElement | undefined;
  $: mentionSuggestions =
    mentionTrigger && mentionTrigger.active
      ? filterWorkspaceEntries(workspaceMentionEntries, mentionTrigger.query, 12)
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
        }
        mentionTrigger = next;
      } else {
        mentionTrigger = null;
        mentionIndex = 0;
      }
    } catch {
      mentionTrigger = null;
    }
  }

  function dismissMentions() {
    mentionTrigger = null;
    mentionIndex = 0;
  }

  // External resets (e.g. message sent clears `input`) must close the popover.
  $: if (!input && mentionTrigger) {
    dismissMentions();
  }

  function selectMention(entry: WorkspaceMentionEntry) {
    if (!mentionTrigger) return;
    const { newText, newCursor } = applyMentionSelection(input, mentionTrigger, entry.relativePath);
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
          {#if mentionSuggestions.length === 0}
            <div class="mention-empty">
              {language === "fr" ? "Aucun fichier correspondant." : "No matching files."}
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
                title={entry.relativePath}
                on:click={() => selectMention(entry)}
                on:mouseenter={() => (mentionIndex = index)}
              >
                <span class="mention-icon" class:is-dir={entry.isDir}>
                  {#if entry.isDir}
                    <FolderIcon size={14} />
                  {:else}
                    <FileIcon size={14} />
                  {/if}
                </span>
                <span class="mention-text">
                  <span class="mention-name">{entry.name}</span>
                  <span class="mention-path">{entry.relativePath}</span>
                </span>
                {#if entry.extension}
                  <span class="mention-ext">{entry.extension}</span>
                {/if}
              </div>
            {/each}
          {/if}
          <div class="mention-hint">
            <span><kbd>↑</kbd><kbd>↓</kbd> {language === "fr" ? "naviguer" : "navigate"}</span>
            <span><kbd>↵</kbd> {language === "fr" ? "insérer" : "insert"}</span>
            <span><kbd>esc</kbd> {language === "fr" ? "fermer" : "dismiss"}</span>
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

        <div class="model-picker">
          <button
            class="model-button"
            class:ready={modelRuntimeReady}
            class:missing={!modelRuntimeReady}
            type="button"
            title={cloudWriteDisabledTitle("changer de modèle") ?? runtimeDetail ?? modelReadinessLabel}
            aria-label={labels.chooseModelTitle}
            aria-expanded={modelMenuOpen}
            disabled={!settingsAvailable || changingModel || cloudWriteLocked}
            on:click={() => (modelMenuOpen = !modelMenuOpen)}
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
              <div class="model-menu-search">
                <Search size={13} />
                <input bind:value={modelSearchQuery} placeholder="Search models" />
              </div>
              {#if modelOptions.filter((model) => model.local).length > 0}
                <div class="model-menu-section">Local</div>
                {#each modelOptions.filter((model) => model.local) as model}
                  <button
                    class:active={model.id === activeModelKey}
                    type="button"
                    role="option"
                    aria-selected={model.id === activeModelKey}
                    disabled={cloudWriteLocked || !model.ready}
                    on:click={() => onSelectModel(model.id)}
                  >
                    <span class="model-name">{model.label}</span>
                    <span class:missing={!model.ready} class="model-state">
                      {model.ready ? (model.family || model.providerKind) : labels.missingText}
                    </span>
                    {#if model.id === activeModelKey}
                      <Check size={14} />
                    {/if}
                  </button>
                {/each}
              {/if}
              {#if modelOptions.filter((model) => !model.local && model.ready).length > 0}
                <div class="model-menu-section">Connected</div>
                {#each modelOptions.filter((model) => !model.local && model.ready) as model}
                  <button
                    class:active={model.id === activeModelKey}
                    type="button"
                    role="option"
                    aria-selected={model.id === activeModelKey}
                    disabled={cloudWriteLocked}
                    on:click={() => onSelectModel(model.id)}
                  >
                    <span class="model-name">{model.label}</span>
                    <span class="model-state">{model.family || model.providerKind}</span>
                    {#if model.id === activeModelKey}
                      <Check size={14} />
                    {/if}
                  </button>
                {/each}
              {/if}
              {#if modelOptions.filter((model) => !model.local && !model.ready).length > 0}
                <div class="model-menu-section">Needs setup</div>
                {#each modelOptions.filter((model) => !model.local && !model.ready) as model}
                  <button type="button" role="option" aria-selected={false} disabled>
                    <span class="model-name">{model.label}</span>
                    <span class="model-state missing">{model.family || labels.missingText}</span>
                  </button>
                {/each}
              {/if}
              <button
                class="model-menu-manage"
                type="button"
                on:click={() => {
                  modelMenuOpen = false;
                  onOpenModelSettings();
                }}
              >
                <Settings size={14} />
                <span>Manage models...</span>
              </button>
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
  .mention-popover {
    position: absolute;
    left: 0;
    right: 0;
    bottom: calc(100% + 8px);
    z-index: 60;
    max-height: 288px;
    overflow-y: auto;
    padding: 6px;
    border-radius: 14px;
    background: rgba(13, 20, 36, 0.97);
    border: 1px solid rgba(148, 163, 184, 0.22);
    box-shadow: 0 18px 44px rgba(0, 0, 0, 0.5);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
  }
  .mention-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border-radius: 10px;
    cursor: pointer;
    border-left: 3px solid transparent;
  }
  .mention-item.selected,
  .mention-item:hover {
    background: rgba(59, 130, 246, 0.16);
    border-left-color: #3b82f6;
  }
  .mention-icon {
    display: flex;
    align-items: center;
    color: #93c5fd;
    flex-shrink: 0;
  }
  .mention-icon.is-dir {
    color: #fbbf24;
  }
  .mention-text {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }
  .mention-name {
    font-size: 0.85rem;
    font-weight: 600;
    color: #f1f5f9;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .mention-path {
    font-size: 0.72rem;
    color: #64748b;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .mention-ext {
    font-size: 0.68rem;
    color: #94a3b8;
    background: rgba(148, 163, 184, 0.14);
    border-radius: 6px;
    padding: 1px 6px;
    flex-shrink: 0;
  }
  .mention-empty {
    padding: 14px;
    text-align: center;
    font-size: 0.82rem;
    color: #64748b;
  }
  .mention-hint {
    display: flex;
    gap: 12px;
    justify-content: flex-end;
    padding: 6px 10px 4px;
    font-size: 0.7rem;
    color: #64748b;
  }
  .mention-hint kbd {
    background: rgba(255, 255, 255, 0.1);
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 4px;
    padding: 1px 5px;
    font-size: 0.66rem;
    color: #cbd5e1;
    margin-right: 3px;
  }
</style>
