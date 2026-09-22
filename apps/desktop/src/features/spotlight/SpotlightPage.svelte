<script lang="ts">
  import Activity from "@lucide/svelte/icons/activity";
  import Bot from "@lucide/svelte/icons/bot";
  import Box from "@lucide/svelte/icons/box";
  import Brain from "@lucide/svelte/icons/brain";
  import Building2 from "@lucide/svelte/icons/building-2";
  import Check from "@lucide/svelte/icons/check";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Circle from "@lucide/svelte/icons/circle";
  import CloudUpload from "@lucide/svelte/icons/cloud-upload";
  import Cpu from "@lucide/svelte/icons/cpu";
  import Edit2 from "@lucide/svelte/icons/edit-2";
  import FileText from "@lucide/svelte/icons/file-text";
  import Maximize2 from "@lucide/svelte/icons/maximize-2";
  import Mic from "@lucide/svelte/icons/mic";
  import MoreVertical from "@lucide/svelte/icons/more-vertical";
  import Plus from "@lucide/svelte/icons/plus";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import Search from "@lucide/svelte/icons/search";
  import Send from "@lucide/svelte/icons/send";
  import Settings from "@lucide/svelte/icons/settings";
  import Sliders from "@lucide/svelte/icons/sliders";
  import Terminal from "@lucide/svelte/icons/terminal";
  import User from "@lucide/svelte/icons/user";
  import X from "@lucide/svelte/icons/x";
  import type { AppSettings, ChatMessage, ModelOption, RuntimeStatus } from "../../lib/types";
  import type { InstructionPersonality } from "../../lib/instructions";
  import { renderMarkdown } from "../../lib/markdown";
  import { initRichContent, destroyCharts } from "../../lib/richContent";

  let spotlightResultsEl: HTMLDivElement | null = null;

  import { afterUpdate, onDestroy } from "svelte";

  afterUpdate(() => {
    if (spotlightResultsEl) {
      initRichContent(spotlightResultsEl, currentTheme === "dark");
    }
  });

  onDestroy(() => {
    if (spotlightResultsEl) {
      destroyCharts(spotlightResultsEl);
    }
  });

  type SpotlightAttachment = {
    id: string;
    name: string;
    size: number;
    type: string;
    file: File;
    mode: "local-reference" | "cloud-object";
    uploadStatus: "local" | "uploading" | "uploaded" | "failed";
    fileId?: string | null;
    note?: string;
    error?: string;
  };

  type SpotlightLabels = {
    delete: string;
    addFilesTitle: string;
    stopSpeakingBtn: string;
    recordBtnTitle: string;
    chooseModelTitle: string;
    missingText: string;
    sendBtnTitle: string;
  };

  export let spotlightMessages: ChatMessage[];
  export let currentTheme: "light" | "dark";
  export let recording: boolean;
  export let spotlightInputEl: HTMLTextAreaElement;
  export let spotlightInput: string;
  export let cloudWriteLocked: boolean;
  export let recordingHint: string;
  export let currentLanguage: "fr" | "en";
  export let attachedFiles: SpotlightAttachment[];
  export let cloudAuthenticated: boolean;
  export let t: SpotlightLabels;
  export let assistantSpeaking: boolean;
  export let voiceVolume: number;
  export let spotlightPersonalityMenuOpen: boolean;
  export let modelMenuOpen: boolean;
  export let activePersonality: InstructionPersonality | null | undefined;
  export let allPersonalities: InstructionPersonality[];
  export let selectedPersonalityId: string;
  export let modelRuntimeReady: boolean;
  export let runtime: RuntimeStatus | null;
  export let modelReadinessLabel: string;
  export let settings: AppSettings | null;
  export let changingModel: boolean;
  export let currentModelLabel: string;
  export let modelSearchQuery: string;
  export let filteredModelOptions: ModelOption[];
  export let activeModelKey: string;
  export let spotlightSending: boolean;

  export let hideSpotlightWindow: () => void | Promise<void>;
  export let submitSpotlightMessage: () => void | Promise<void>;
  export let handleSpotlightKeydown: (event: KeyboardEvent) => void | Promise<void>;
  export let cloudWriteDisabledTitle: (action?: string) => string | null | undefined;
  export let previewAttachment: (file: SpotlightAttachment) => void | Promise<void>;
  export let formatFileSize: (size: number) => string;
  export let uploadAttachmentToCloud: (id: string) => void | Promise<void>;
  export let removeAttachment: (id: string) => void;
  export let openFilePicker: () => void | Promise<void>;
  export let toggleSpotlightRecording: () => void | Promise<void>;
  export let stopSpeaking: () => void;
  export let selectPersonality: (id: string) => void;
  export let selectModel: (id: string) => void | Promise<void>;
  export let openSettings: (tab: "models") => void;
  export let getWavePath: (phaseOffset: number, volume: number, height?: number) => string;
  export let expandToMainWindow: () => void | Promise<void>;
  export let startResizingWindow: (event: MouseEvent) => void | Promise<void>;

  let selectedProviderTab = "all";

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
      model.label.toLowerCase().includes("qwen") ||
      model.modelId.toLowerCase().includes("qwen")
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
    const list = filteredModelOptions || [];
    const tabs: ProviderTabItem[] = [
      { id: "all", label: currentLanguage === "fr" ? "Tous" : "All", count: list.length },
    ];
    const localCount = list.filter((m) => m.local || m.providerKind === "ollama" || m.providerKind === "llama-cpp").length;
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
      const c = list.filter((m) => {
        if (m.local || m.providerKind === "ollama" || m.providerKind === "llama-cpp") return false;
        return getModelProviderKey(m) === cp.id;
      }).length;
      if (c > 0) {
        tabs.push({ id: cp.id, label: cp.label, count: c });
      }
    }

    const otherCount = list.filter(
      (m) =>
        !m.local &&
        m.providerKind !== "ollama" &&
        m.providerKind !== "llama-cpp" &&
        !tabs.some((t) => t.id !== "all" && t.id === getModelProviderKey(m))
    ).length;
    if (otherCount > 0) {
      tabs.push({ id: "other", label: currentLanguage === "fr" ? "Autres" : "Other", count: otherCount });
    }

    return tabs;
  })();

  $: displayedSpotlightModels = (filteredModelOptions || []).filter((model) => {
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
    return true;
  });

  $: localDisplayedModels = displayedSpotlightModels.filter((m) => m.local || m.providerKind === "ollama" || m.providerKind === "llama-cpp");
  $: connectedDisplayedModels = displayedSpotlightModels.filter((m) => !m.local && m.providerKind !== "ollama" && m.providerKind !== "llama-cpp" && m.ready);
  $: needsSetupDisplayedModels = displayedSpotlightModels.filter((m) => !m.local && m.providerKind !== "ollama" && m.providerKind !== "llama-cpp" && !m.ready);
</script>
  <!-- Spotlight UI -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="spotlight-shell" on:mousedown|self={hideSpotlightWindow}>
    <div class="spotlight-container" class:dark-theme={currentTheme === "dark"} class:recording={recording} data-tauri-drag-region>

      <!-- Results Area (Only shown if we have messages) — TOP -->
      {#if spotlightMessages.length > 0}
        <div class="spotlight-results" bind:this={spotlightResultsEl}>
          {#each spotlightMessages as msg}
            <div class="spotlight-msg-row" class:user={msg.role === 'user'}>
              <div class="spotlight-avatar" style="background: {msg.role === 'user' ? 'linear-gradient(180deg, #2997ff 0%, #0071e3 100%)' : 'linear-gradient(135deg, #1C2232 0%, #0a0e17 100%)'}; width: 24px; height: 24px; border-radius: 50%; display: flex; align-items: center; justify-content: center; color: #fff; font-size: 10px; font-weight: 600; flex-shrink: 0; margin-top: 2px;">
                {#if msg.role === 'user'}
                  U
                {:else}
                  A
                {/if}
              </div>
              <div class="spotlight-msg-content" style="color: {msg.role === 'user' ? (currentTheme === 'dark' ? '#fff' : '#1d1d1f') : (currentTheme === 'dark' ? '#e3e3e7' : '#1d1d1f')}; flex-grow: 1; font-size: 14px; line-height: 1.5; word-break: break-word;">
                {#if msg.role === 'assistant'}
                  {#if msg.isGenerating}
                    <div class="thinking-text-placeholder" style="margin-top: 4px;">
                      <div class="thinking-gemini-gradient" style="max-width: 140px; margin-bottom: 6px;"></div>
                      <div class="thinking-bar-1" style="width: 90%; height: 8px;"></div>
                      <div class="thinking-bar-2" style="width: 60%; height: 8px;"></div>
                    </div>
                  {:else}
                    <div class="markdown-body">
                      {@html renderMarkdown(msg.content, currentLanguage)}
                    </div>
                  {/if}
                {:else}
                  {msg.content}
                {/if}
              </div>
            </div>
          {/each}
        </div>
      {/if}

      <!-- Spotlight Composer — BOTTOM -->
      <form class="composer" on:submit|preventDefault={submitSpotlightMessage} style="width: 100%; margin: 0; box-shadow: none; border-radius: 0; background: transparent; border: none; border-top: {spotlightMessages.length > 0 ? (currentTheme === 'dark' ? '1px solid rgba(255,255,255,0.06)' : '1px solid rgba(0,0,0,0.06)') : 'none'}; padding: 12px 16px; box-sizing: border-box; display: flex; flex-direction: column; gap: 8px;" data-tauri-drag-region>
        
        <!-- Textarea Row (Grande zone de saisie) -->
        <div style="width: 100%;">
          <textarea
            bind:this={spotlightInputEl}
            bind:value={spotlightInput}
            placeholder={cloudWriteLocked ? (cloudWriteDisabledTitle("envoyer un message") ?? "") : (recordingHint || (currentLanguage === 'fr' ? 'Demandez quelque chose…' : 'Ask anything…'))}
            rows="3"
            disabled={cloudWriteLocked}
            style="resize: none; width: 100%; min-height: 80px; max-height: 180px;"
            on:keydown={handleSpotlightKeydown}
          ></textarea>
        </div>

        {#if attachedFiles.length > 0}
          <div class="attachment-row" aria-label="Attached files" style="margin-bottom: 8px; width: 100%;">
            {#each attachedFiles as file}
              <div class:limited={file.note} class="attachment-chip" title={file.error ?? file.note ?? file.name}>
                <FileText size={14} />
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <span class="attachment-name clickable" on:click={() => previewAttachment(file)} title="Aperçu">{file.name}</span>
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
                    on:click={() => uploadAttachmentToCloud(file.id)}
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
                  aria-label={`${t.delete} ${file.name}`}
                  on:click={() => removeAttachment(file.id)}
                >
                  <X size={13} />
                </button>
              </div>
            {/each}
          </div>
        {/if}

        <!-- Bottom row: Buttons -->
        <div class="composer-buttons-row" style="display: flex; align-items: center; justify-content: space-between; width: 100%; margin-top: 4px;">
          <!-- Left side: Plus, Mic, Personality Picker -->
          <div style="display: flex; align-items: center; gap: 8px;">
            <!-- Plus Button -->
            <button
              class="attach-button"
              type="button"
              title={cloudWriteDisabledTitle("ajouter une piece jointe") ?? t.addFilesTitle}
              aria-label={t.addFilesTitle}
              disabled={cloudWriteLocked}
              on:click={openFilePicker}
            >
              <Plus size={19} />
            </button>
            
            <!-- Mic Button -->
            <button
              class="mic-button"
              class:recording
              class:speaking={assistantSpeaking}
              class:idle={!recording && !assistantSpeaking}
              style="--volume: {voiceVolume};"
              type="button"
              title={assistantSpeaking ? t.stopSpeakingBtn : (cloudWriteDisabledTitle("dicter un message") ?? t.recordBtnTitle)}
              aria-label={assistantSpeaking ? t.stopSpeakingBtn : t.recordBtnTitle}
              disabled={cloudWriteLocked}
              on:click={toggleSpotlightRecording}
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
              <button class="voice-stop-button compact" type="button" on:click={stopSpeaking}>
                <X size={14} />
                <span>{t.stopSpeakingBtn}</span>
              </button>
            {/if}

            <!-- Personality Picker -->
            <div class="personality-picker spotlight-personality-picker" style="position: relative;">
              <button
                class="model-button"
                type="button"
                title={cloudWriteDisabledTitle("changer la personnalité active") ?? "Choisir la personnalité"}
                aria-label="Choisir la personnalité"
                aria-expanded={spotlightPersonalityMenuOpen}
                disabled={cloudWriteLocked}
                on:click|stopPropagation={() => {
                  spotlightPersonalityMenuOpen = !spotlightPersonalityMenuOpen;
                  modelMenuOpen = false;
                }}
                style="display: flex; align-items: center; gap: 6px; padding: 4px 10px; border-radius: 20px; border: 1px solid {currentTheme === 'dark' ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.1)'}; background: {currentTheme === 'dark' ? 'rgba(255,255,255,0.04)' : 'rgba(0,0,0,0.02)'}; cursor: pointer; height: 28px;"
              >
                <div
                  style="width: 14px; height: 14px; border-radius: 50%; background: {activePersonality?.avatarColor || '#0071e3'}; display: flex; align-items: center; justify-content: center; color: #ffffff; flex-shrink: 0;"
                >
                  {#if activePersonality?.icon === "bot"}<Bot size={8} />
                  {:else if activePersonality?.icon === "code"}<Terminal size={8} />
                  {:else if activePersonality?.icon === "check"}<Check size={8} />
                  {:else if activePersonality?.icon === "edit-2"}<Edit2 size={8} />
                  {:else if activePersonality?.icon === "brain"}<Brain size={8} />
                  {:else if activePersonality?.icon === "cpu"}<Cpu size={8} />
                  {:else if activePersonality?.icon === "user"}<User size={8} />
                  {:else if activePersonality?.icon === "activity"}<Activity size={8} />
                  {:else if activePersonality?.icon === "sliders"}<Sliders size={8} />
                  {:else if activePersonality?.icon === "building-2"}<Building2 size={8} />
                  {:else}<Search size={8} />{/if}
                </div>
                <span style="font-size: 12px; font-weight: 550; color: {currentTheme === 'dark' ? '#f5f5f7' : '#1d1d1f'}; max-width: 100px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
                  {activePersonality?.name || 'Assistant'}
                </span>
                <ChevronDown size={13} style="opacity: 0.7;" />
              </button>

              {#if spotlightPersonalityMenuOpen}
                <div class="personality-menu spotlight-personality-menu" role="listbox" aria-label="Personalities">
                  {#each allPersonalities as pers}
                    <button
                      class:active={selectedPersonalityId === pers.id}
                      type="button"
                      role="option"
                      aria-selected={selectedPersonalityId === pers.id}
                      disabled={cloudWriteLocked}
                      on:click={() => {
                        selectPersonality(pers.id);
                        spotlightPersonalityMenuOpen = false;
                      }}
                    >
                      <div
                        style="width: 16px; height: 16px; border-radius: 50%; background: {pers.avatarColor}; display: flex; align-items: center; justify-content: center; color: #ffffff; flex-shrink: 0;"
                      >
                        {#if pers.icon === "bot"}<Bot size={8} />
                        {:else if pers.icon === "code"}<Terminal size={8} />
                        {:else if pers.icon === "check"}<Check size={8} />
                        {:else if pers.icon === "edit-2"}<Edit2 size={8} />
                        {:else if pers.icon === "brain"}<Brain size={8} />
                        {:else if pers.icon === "cpu"}<Cpu size={8} />
                        {:else if pers.icon === "user"}<User size={8} />
                        {:else if pers.icon === "activity"}<Activity size={8} />
                        {:else if pers.icon === "sliders"}<Sliders size={8} />
                        {:else if pers.icon === "building-2"}<Building2 size={8} />
                        {:else}<Search size={8} />{/if}
                      </div>
                      <span class="personality-name">{pers.name}</span>
                      {#if selectedPersonalityId === pers.id}
                        <Check size={14} style="color: #0071e3;" />
                      {:else}
                        <span style="width: 14px;"></span>
                      {/if}
                    </button>
                  {/each}
                </div>
              {/if}
            </div>
          </div>

          <!-- Right side: Model Picker, Send Button -->
          <div style="display: flex; align-items: center; gap: 8px;">
            <!-- Model Picker -->
            <div class="model-picker spotlight-model-picker">
              <button
                class="model-button"
                class:ready={modelRuntimeReady}
                class:missing={!modelRuntimeReady}
                type="button"
                title={cloudWriteDisabledTitle("changer de modèle") ?? runtime?.detail ?? modelReadinessLabel}
                aria-label={t.chooseModelTitle}
                aria-expanded={modelMenuOpen}
                disabled={!settings || changingModel || cloudWriteLocked}
                on:click|stopPropagation={() => {
                  modelMenuOpen = !modelMenuOpen;
                  spotlightPersonalityMenuOpen = false;
                }}
                style="height: 28px; padding: 4px 10px; border-radius: 20px;"
              >
                {#if modelRuntimeReady}
                  <Check size={12} />
                {:else}
                  <Circle size={12} />
                {/if}
                <span>{currentModelLabel}</span>
                <ChevronDown size={13} />
              </button>

              {#if modelMenuOpen}
                <div class="model-menu spotlight-model-menu" role="listbox" aria-label="Models">
                  <div class="model-menu-header">
                    <div class="model-menu-top-row">
                      <span class="model-menu-title">{currentLanguage === "fr" ? "Modèle d'IA" : "AI Model"}</span>
                      <span class="model-menu-total">
                        {displayedSpotlightModels.length} {displayedSpotlightModels.length > 1 ? (currentLanguage === "fr" ? "modèles" : "models") : (currentLanguage === "fr" ? "modèle" : "model")}
                      </span>
                    </div>

                    <div class="model-menu-search">
                      <Search size={13} />
                      <input
                        bind:value={modelSearchQuery}
                        placeholder={currentLanguage === "fr" ? "Rechercher un modèle ou fournisseur..." : "Search models or providers..."}
                      />
                      {#if modelSearchQuery}
                        <button
                          type="button"
                          class="model-search-clear"
                          title={currentLanguage === "fr" ? "Effacer la recherche" : "Clear search"}
                          on:click={() => (modelSearchQuery = "")}
                        >
                          <X size={12} />
                        </button>
                      {/if}
                    </div>

                    {#if providerTabs.length > 1}
                      <div class="model-provider-tabs" role="tablist" aria-label={currentLanguage === "fr" ? "Fournisseurs de modèles" : "Model providers"}>
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
                    {#if displayedSpotlightModels.length === 0}
                      <div class="model-menu-empty">
                        <Box size={20} />
                        <span>{currentLanguage === "fr" ? "Aucun modèle trouvé" : "No models found"}</span>
                      </div>
                    {:else if selectedProviderTab === "all"}
                      {#if localDisplayedModels.length > 0}
                        <div class="model-menu-section">{currentLanguage === "fr" ? "Local / Ollama" : "Local / Ollama"}</div>
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
                              selectModel(model.id);
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
                                    <span class="model-missing-hint">{t.missingText}</span>
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
                        <div class="model-menu-section">{currentLanguage === "fr" ? "Connecté (Cloud)" : "Connected"}</div>
                        {#each connectedDisplayedModels as model}
                          <button
                            class="model-option-item"
                            class:active={model.id === activeModelKey}
                            type="button"
                            role="option"
                            aria-selected={model.id === activeModelKey}
                            disabled={cloudWriteLocked}
                            on:click={() => {
                              selectModel(model.id);
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
                        <div class="model-menu-section">{currentLanguage === "fr" ? "Configuration requise" : "Needs setup"}</div>
                        {#each needsSetupDisplayedModels as model}
                          <button
                            class="model-option-item needs-setup"
                            type="button"
                            role="option"
                            aria-selected={false}
                            on:click={() => {
                              modelMenuOpen = false;
                              openSettings("models");
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
                                  <span class="model-missing-hint">{t.missingText}</span>
                                </div>
                              </div>
                            </div>
                          </button>
                        {/each}
                      {/if}
                    {:else}
                      {#each displayedSpotlightModels as model}
                        <button
                          class="model-option-item"
                          class:active={model.id === activeModelKey}
                          class:disabled={cloudWriteLocked}
                          class:needs-setup={!model.ready && !model.local}
                          type="button"
                          role="option"
                          aria-selected={model.id === activeModelKey}
                          disabled={cloudWriteLocked}
                          on:click={() => {
                            if (model.ready || model.local) {
                              selectModel(model.id);
                              modelMenuOpen = false;
                            } else {
                              modelMenuOpen = false;
                              openSettings("models");
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
                                  <span class="model-missing-hint">{t.missingText}</span>
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
                        openSettings("models");
                      }}
                    >
                      <Settings size={14} />
                      <span>{currentLanguage === "fr" ? "Gérer les fournisseurs et modèles..." : "Manage providers & models..."}</span>
                    </button>
                  </div>
                </div>
              {/if}
            </div>
            
            <!-- Send Button -->
            <button
              class="send-button"
              type="submit"
              title={cloudWriteDisabledTitle("envoyer un message") ?? t.sendBtnTitle}
              disabled={cloudWriteLocked || spotlightSending || (!spotlightInput.trim() && attachedFiles.length === 0)}
              style="width: 32px; height: 32px; display: flex; align-items: center; justify-content: center; border-radius: 50%;"
            >
              {#if spotlightSending}
                <MoreVertical size={16} />
              {:else}
                <Send size={16} />
              {/if}
            </button>
          </div>
        </div>

        <!-- Live Waveform animation if recording -->
        {#if recording}
          <div class="composer-waveform" style="display: flex; justify-content: center; width: 100%; height: 26px; margin-top: 8px; padding: 0 12px; box-sizing: border-box;">
            <svg viewBox="0 0 200 26" style="width: 100%; max-width: 320px; height: 26px;">
              <path d={getWavePath(0, voiceVolume, 26)} fill="none" stroke="rgba(10, 132, 255, 0.8)" stroke-width="1.8" stroke-linecap="round" />
              <path d={getWavePath(2, voiceVolume * 0.6, 26)} fill="none" stroke="rgba(162, 36, 214, 0.6)" stroke-width="1.4" stroke-linecap="round" />
              <path d={getWavePath(4, voiceVolume * 0.3, 26)} fill="none" stroke="rgba(255, 107, 107, 0.5)" stroke-width="1.0" stroke-linecap="round" />
            </svg>
          </div>
        {/if}
      </form>

      <!-- Footer Info -->
      <div class="spotlight-footer" data-tauri-drag-region>
        <div style="display: flex; align-items: center; gap: 4px;">
          <span>ARO Spotlight</span>
        </div>
        <div style="display: flex; gap: 12px; align-items: center;">
          <button class="spotlight-expand-btn" type="button" on:click={expandToMainWindow} title={currentLanguage === 'fr' ? 'Ouvrir dans la fenêtre principale' : 'Open in main window'}>
            <Maximize2 size={12} />
            <span>{currentLanguage === 'fr' ? 'Agrandir' : 'Expand'}</span>
          </button>
          <span>ESC {currentLanguage === 'fr' ? 'pour fermer' : 'to close'}</span>
          <span>⌥ Space {currentLanguage === 'fr' ? 'pour masquer' : 'to hide'}</span>
        </div>
      </div>

      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="spotlight-resize-handle" on:mousedown={startResizingWindow}>
        <svg width="8" height="8" viewBox="0 0 8 8" fill="none">
          <path d="M6 1L1 6M7 3L3 7M7 6L6 7" stroke={currentTheme === 'dark' ? 'rgba(255,255,255,0.4)' : 'rgba(0,0,0,0.3)'} stroke-width="1.8" stroke-linecap="round" />
        </svg>
      </div>
    </div>
  </div>

<style>
  .spotlight-model-picker {
    position: relative;
  }

  .spotlight-model-menu {
    position: absolute;
    right: 0;
    bottom: calc(100% + 8px) !important;
    top: auto !important;
    z-index: 1000;
    display: flex;
    flex-direction: column;
    width: 360px;
    max-width: min(360px, calc(100vw - 32px));
    max-height: min(420px, calc(100vh - 140px));
    border-radius: 16px;
    background: rgba(255, 255, 255, 0.95);
    border: 1px solid rgba(0, 0, 0, 0.1);
    box-shadow: 0 24px 56px rgba(0, 0, 0, 0.22), 0 0 0 1px rgba(0, 0, 0, 0.05);
    backdrop-filter: blur(32px) saturate(180%);
    -webkit-backdrop-filter: blur(32px) saturate(180%);
    overflow: hidden;
  }

  :global(.dark-theme) .spotlight-model-menu,
  :global(body.dark-theme) .spotlight-model-menu {
    background: rgba(28, 28, 32, 0.96);
    border-color: rgba(255, 255, 255, 0.12);
    box-shadow: 0 24px 64px rgba(0, 0, 0, 0.7), 0 0 0 1px rgba(255, 255, 255, 0.08);
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

  :global(.dark-theme) .model-menu-header,
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

  :global(.dark-theme) .model-menu-search,
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

  :global(.dark-theme) .model-menu-search:focus-within,
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

  :global(.dark-theme) .provider-tab-pill,
  :global(body.dark-theme) .provider-tab-pill {
    background: rgba(255, 255, 255, 0.06);
    color: #a1a1aa;
  }

  .provider-tab-pill:hover {
    background: rgba(0, 0, 0, 0.08);
    color: #111;
  }

  :global(.dark-theme) .provider-tab-pill:hover,
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

  :global(.dark-theme) .provider-tab-pill.active,
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

  :global(.dark-theme) .tab-count-badge,
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
    max-height: 260px;
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

  .model-menu-list::-webkit-scrollbar-thumb {
    background: rgba(0, 0, 0, 0.15);
    border-radius: 3px;
  }

  :global(.dark-theme) .model-menu-list::-webkit-scrollbar-thumb,
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

  :global(.dark-theme) .model-option-item:hover,
  :global(body.dark-theme) .model-option-item:hover {
    background: rgba(255, 255, 255, 0.06);
  }

  .model-option-item.active {
    background: rgba(0, 113, 227, 0.08);
  }

  :global(.dark-theme) .model-option-item.active,
  :global(body.dark-theme) .model-option-item.active {
    background: rgba(10, 132, 255, 0.16);
  }

  .model-option-item.disabled {
    opacity: 0.55;
    cursor: not-allowed;
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

  :global(.dark-theme) .model-name,
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

  .model-check-icon {
    color: #0071e3;
    flex-shrink: 0;
  }

  :global(.dark-theme) .model-check-icon,
  :global(body.dark-theme) .model-check-icon {
    color: #2997ff;
  }

  .model-menu-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 24px 16px;
    color: #86868b;
    font-size: 12px;
    text-align: center;
  }

  .model-menu-footer {
    flex-shrink: 0;
    padding: 8px 10px;
    border-top: 1px solid rgba(0, 0, 0, 0.06);
    background: rgba(255, 255, 255, 0.3);
  }

  :global(.dark-theme) .model-menu-footer,
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

  :global(.dark-theme) .model-menu-manage-btn,
  :global(body.dark-theme) .model-menu-manage-btn {
    color: #2997ff;
  }

  .model-menu-manage-btn:hover {
    background: rgba(0, 113, 227, 0.08);
  }

  :global(.dark-theme) .model-menu-manage-btn:hover,
  :global(body.dark-theme) .model-menu-manage-btn:hover {
    background: rgba(41, 151, 255, 0.12);
  }
</style>
