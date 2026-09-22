<script context="module" lang="ts">
  export type ConversationViewLabels = {
    loadingText: string;
    stopSpeakingBtn: string;
    recordBtnTitle: string;
    youLabel: string;
    copyBtn: string;
    copiedBtn: string;
    goodAnswerTitle: string;
    badAnswerTitle: string;
    cancel: string;
    save: string;
  };
  export type ConversationPersonality = {
    id: string;
    name: string;
    icon: string;
    avatarColor: string;
  };
  export type VoiceInputMode = "push-to-talk" | "dictation" | "hands-free";
  export type VoiceModeOption = { id: VoiceInputMode; label: string; title: string };
  export type FeedbackRating = "good" | "bad";
</script>

<script lang="ts">
  import Activity from "@lucide/svelte/icons/activity";
  import Bot from "@lucide/svelte/icons/bot";
  import Brain from "@lucide/svelte/icons/brain";
  import Building2 from "@lucide/svelte/icons/building-2";
  import Check from "@lucide/svelte/icons/check";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Copy from "@lucide/svelte/icons/copy";
  import Cpu from "@lucide/svelte/icons/cpu";
  import Edit2 from "@lucide/svelte/icons/edit-2";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import FileText from "@lucide/svelte/icons/file-text";
  import Globe from "@lucide/svelte/icons/globe";
  import Puzzle from "@lucide/svelte/icons/puzzle";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import Search from "@lucide/svelte/icons/search";
  import Sliders from "@lucide/svelte/icons/sliders";
  import Terminal from "@lucide/svelte/icons/terminal";
  import ThumbsDown from "@lucide/svelte/icons/thumbs-down";
  import ThumbsUp from "@lucide/svelte/icons/thumbs-up";
  import User from "@lucide/svelte/icons/user";
  import X from "@lucide/svelte/icons/x";
  import { renderMarkdown } from "../../lib/markdown";
  import { parseMessageThinking } from "../../lib/thinking";
  import type { AttachmentRef, ChatMessage } from "../../lib/types";
  import type { Folder as FolderType, Project as ProjectType } from "../../lib/types/projects-folders";
  import { initRichContent, destroyCharts } from "../../lib/richContent";
  import ConversationDestinationPicker from "../conversations/ConversationDestinationPicker.svelte";
  import AgentStepCard from "./AgentStepCard.svelte";

  let expandedThinkingBlocks: Record<string, boolean> = {};
  let copiedThinkingId: string | null = null;
  let copiedThinkingTimeout: ReturnType<typeof setTimeout> | null = null;

  function isThinkingExpanded(
    expandedMap: Record<string, boolean>,
    msgId: string,
    isReasoningComplete: boolean
  ): boolean {
    if (expandedMap && expandedMap[msgId] !== undefined) {
      return expandedMap[msgId];
    }
    // While generating/streaming, auto-expand so reasoning is visible live.
    // When completed, fold into a sleek Apple-style summary card.
    return !isReasoningComplete;
  }

  function toggleThinkingBlock(msgId: string, isReasoningComplete: boolean) {
    const current = expandedThinkingBlocks[msgId] !== undefined
      ? expandedThinkingBlocks[msgId]
      : !isReasoningComplete;
    expandedThinkingBlocks = {
      ...expandedThinkingBlocks,
      [msgId]: !current,
    };
  }

  async function handleCopyThinking(msgId: string, reasoningText: string) {
    try {
      if (navigator?.clipboard?.writeText) {
        await navigator.clipboard.writeText(reasoningText);
      }
      copiedThinkingId = msgId;
      if (copiedThinkingTimeout) clearTimeout(copiedThinkingTimeout);
      copiedThinkingTimeout = setTimeout(() => {
        copiedThinkingId = null;
      }, 2000);
    } catch (e) {
      console.error("Failed to copy thinking", e);
    }
  }

  function hasMeaningfulAgentSteps(steps: any[] | undefined): boolean {
    if (!steps || steps.length === 0) return false;
    return steps.some((s) => s.kind === "tool" || s.kind === "error");
  }

  function getMeaningfulAgentSteps(steps: any[] | undefined): any[] {
    if (!steps) return [];
    return steps.filter((s) => s.kind === "tool" || s.kind === "error");
  }

  function isBrowserStep(step: any): boolean {
    if (!step) return false;
    const tid = (step.input?.toolId || "").toLowerCase();
    const st = (step.title || "").toLowerCase();
    return (
      tid.includes("browser") ||
      st.includes("browser") ||
      st.includes("navigate") ||
      tid.includes("web.page.read") ||
      tid.includes("web.fetch") ||
      Boolean(step.input?.url) ||
      Boolean(step.output?.url)
    );
  }

  function getStepUrl(step: any): string {
    return step?.input?.url || step?.output?.url || "";
  }

  function handleTakeBrowserControl(url: string, takeControl = true, newTab = false) {
    window.dispatchEvent(
      new CustomEvent("aro:open-browser", {
        detail: { url, takeControl, newTab },
      })
    );
  }

  function getActiveToolStep(steps: any[] | undefined, isGenerating?: boolean | null, isExpanded?: boolean | null): any | null {
    if (!steps || steps.length === 0) return null;
    const running = steps.find((s) => s.status === "running");
    if (running) return running;
    if (isGenerating) return steps[steps.length - 1];
    if (!isExpanded) {
      return [...steps].reverse().find((s) => isBrowserStep(s)) || null;
    }
    return null;
  }

  import { onMount, afterUpdate, onDestroy } from "svelte";

  export let onSendUserPrompt: ((promptText: string) => void) | undefined = undefined;

  function dispatchPrompt(text: string) {
    if (!text || !text.trim()) return;
    const cleanText = text.trim();
    if (onSendUserPrompt) {
      onSendUserPrompt(cleanText);
    } else {
      window.dispatchEvent(new CustomEvent("aro:send-prompt", { detail: { text: cleanText } }));
    }
  }

  $: {
    (window as any).__useToolPrompt = (target: string | HTMLElement) => {
      let toolName = "";
      if (typeof target === "string") {
        toolName = target;
      } else if (target) {
        const raw = target.getAttribute("data-tool");
        if (raw) {
          try { toolName = decodeURIComponent(raw); } catch { toolName = raw; }
        }
      }
      if (toolName) dispatchPrompt(`Exécute l'outil ${toolName}`);
    };
    (window as any).__sendChatOption = (target: string | HTMLElement) => {
      let optionText = "";
      if (typeof target === "string") {
        optionText = target;
      } else if (target) {
        const raw = target.getAttribute("data-option");
        if (raw) {
          try { optionText = decodeURIComponent(raw); } catch { optionText = raw; }
        }
        if (!optionText && target.textContent) {
          optionText = target.textContent.trim();
        }
      }
      if (optionText) dispatchPrompt(optionText);
    };
    (window as any).__sendChatInput = (inputId: string) => {
      const el = document.getElementById(inputId) as HTMLInputElement;
      if (el && el.value.trim()) {
        const val = el.value.trim();
        el.value = "";
        dispatchPrompt(val);
      }
    };
    (window as any).__previewChatDiff = (encodedPath: string, encodedDiff: string) => {
      let filePath = "";
      let diff = "";
      try { filePath = decodeURIComponent(encodedPath); } catch { filePath = encodedPath; }
      try { diff = decodeURIComponent(encodedDiff); } catch { diff = encodedDiff; }
      window.dispatchEvent(new CustomEvent("aro:preview-diff", { detail: { filePath, diff } }));
      window.dispatchEvent(new CustomEvent("aro:select-artifact", { detail: { filePath, diff } }));
    };
    (window as any).__applyChatDiff = (encodedPath: string, encodedDiff: string) => {
      let filePath = "";
      let diff = "";
      try { filePath = decodeURIComponent(encodedPath); } catch { filePath = encodedPath; }
      try { diff = decodeURIComponent(encodedDiff); } catch { diff = encodedDiff; }
      window.dispatchEvent(new CustomEvent("aro:apply-diff", { detail: { filePath, diff } }));
    };
  }

  function handleContainerClick(e: MouseEvent) {
    const chip = (e.target as HTMLElement)?.closest(".form-option-chip") as HTMLElement | null;
    if (chip) {
      e.preventDefault();
      e.stopPropagation();
      let optionText = "";
      const raw = chip.getAttribute("data-option");
      if (raw) {
        try { optionText = decodeURIComponent(raw); } catch { optionText = raw; }
      }
      if (!optionText && chip.textContent) {
        optionText = chip.textContent.trim();
      }
      if (optionText) dispatchPrompt(optionText);
      return;
    }

    const submitBtn = (e.target as HTMLElement)?.closest(".form-submit-btn") as HTMLElement | null;
    if (submitBtn) {
      e.preventDefault();
      e.stopPropagation();
      const row = submitBtn.closest(".form-input-row");
      const inputEl = row?.querySelector(".form-text-input") as HTMLInputElement | null;
      if (inputEl && inputEl.value.trim()) {
        const val = inputEl.value.trim();
        inputEl.value = "";
        dispatchPrompt(val);
      }
      return;
    }

    const toolBtn = (e.target as HTMLElement)?.closest(".tool-card-act-btn") as HTMLElement | null;
    if (toolBtn) {
      e.preventDefault();
      e.stopPropagation();
      const raw = toolBtn.getAttribute("data-tool");
      let toolName = "";
      if (raw) {
        try { toolName = decodeURIComponent(raw); } catch { toolName = raw; }
      }
      if (toolName) dispatchPrompt(`Exécute l'outil ${toolName}`);
      return;
    }
  }

  function handleContainerKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      const target = e.target as HTMLElement;
      if (target && target.classList.contains("form-text-input")) {
        e.preventDefault();
        const inputEl = target as HTMLInputElement;
        if (inputEl.value.trim()) {
          const val = inputEl.value.trim();
          inputEl.value = "";
          dispatchPrompt(val);
        }
      }
    }
  }

  afterUpdate(() => {
    if (conversationContainer) {
      initRichContent(conversationContainer, theme === "dark");
    }
  });

  onDestroy(() => {
    if (conversationContainer) {
      destroyCharts(conversationContainer);
    }
  });

  export let conversationContainer: HTMLDivElement;
  export let messagesEnd: HTMLDivElement;
  export let loading: boolean;
  export let messages: ChatMessage[];
  export let labels: ConversationViewLabels;
  export let language: "fr" | "en";
  export let theme: "light" | "dark";
  export let personalities: ConversationPersonality[];
  export let selectedPersonalityId: string;
  export let cloudWriteLocked: boolean;
  export let cloudWriteDisabledTitle: (action?: string) => string | null | undefined;
  export let recording: boolean;
  export let assistantSpeaking: boolean;
  export let voiceVolume: number;
  export let wakeWordEnabled: boolean;
  export let voiceStateLabel: string;
  export let recordingHint: string;
  export let voiceStateHint: string;
  export let runtimeDetail: string | null | undefined;
  export let modelRuntimeReady: boolean;
  export let voiceSpeechToTextReady: boolean;
  export let voiceWakeModelReady: boolean;
  export let voiceTextToSpeechReady: boolean;
  export let speakResponses: boolean;
  export let speechToTextIssue: string;
  export let textToSpeechIssue: string;
  export let modelReadinessLabel: string;
  export let speechReadinessLabel: string;
  export let wakeWordReadinessLabel: string;
  export let textToSpeechReadinessLabel: string;
  export let voiceModeOptions: VoiceModeOption[];
  export let voiceInputMode: VoiceInputMode;
  export let voiceHandsFreeArmed: boolean;
  export let expandedMessageSteps: Record<string, boolean>;
  export let expandedStepDetails: Record<string, boolean>;
  export let copiedMessageId: string | null;
  export let messageFeedback: Record<string, FeedbackRating | null>;
  export let editingMessageId: string | null;
  export let editingMessageText: string;
  export let sending: boolean;
  export let getWavePath: (offset: number, volume: number, height?: number) => string;
  export let formatTime: (value: string) => string;
  export let formatFileSize: (size: number) => string;
  export let onToggleRecording: () => void | Promise<void>;
  export let onStopSpeaking: () => void;
  export let onSetVoiceInputMode: (mode: VoiceInputMode) => void | Promise<void>;
  export let onSelectPersonality: (personalityId: string) => void;
  export let projects: ProjectType[] = [];
  export let folders: FolderType[] = [];
  export let pendingProjectId: string | null = null;
  export let pendingFolderId: string | null = null;
  export let showDestinationPicker = false;
  export let onSelectDestination: (projectId: string | null, folderId: string | null) => void = () => {};
  export let onToggleMessageSteps: (messageId: string) => void;
  export let onToggleStepDetails: (messageId: string, sequence: number) => void;
  export let onPreviewAttachment: (attachment: AttachmentRef) => void | Promise<void>;
  export let onCopyMessage: (messageId: string, text: string) => void | Promise<void>;
  export let onRememberMessage: (message: ChatMessage) => void | Promise<void>;
  export let onFeedback: (messageId: string, rating: FeedbackRating) => void;
  export let onStartEdit: (message: ChatMessage) => void;
  export let onCancelEdit: () => void;
  export let onSaveEdit: (message: ChatMessage) => void | Promise<void>;

  // Reference voice, model and runtime readiness props passed from parent
  $: void [
    wakeWordEnabled,
    voiceStateLabel,
    recordingHint,
    voiceStateHint,
    runtimeDetail,
    modelRuntimeReady,
    voiceSpeechToTextReady,
    voiceWakeModelReady,
    voiceTextToSpeechReady,
    speakResponses,
    speechToTextIssue,
    textToSpeechIssue,
    modelReadinessLabel,
    speechReadinessLabel,
    wakeWordReadinessLabel,
    textToSpeechReadinessLabel,
    voiceModeOptions,
    voiceInputMode,
    voiceHandsFreeArmed,
    onSetVoiceInputMode,
    onToggleRecording,
  ];
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div 
  class="conversation" 
  bind:this={conversationContainer}
  on:click={handleContainerClick}
  on:keydown={handleContainerKeydown}
>
  {#if loading}
    <div class="center-state">
      <RefreshCw size={20} />
      <span>{labels.loadingText}</span>
    </div>
  {:else if messages.length === 0}
    <div class="empty-state">
      <div class="empty-state-logo-container" class:dark={theme === "dark"}>
        <img
          src="/aro-core-logo.png"
          alt="ARO"
          class="empty-state-logo logo-light"
          draggable="false"
        />
        <img
          src="/aro-core-logo-dark.png"
          alt="ARO"
          class="empty-state-logo logo-dark"
          draggable="false"
        />
      </div>

      {#if assistantSpeaking}
        <div class="voice-speaking-indicator" style="display: flex; align-items: center; gap: 10px; margin: 4px 0;">
          <div class="siri-wave">
            <div class="bar bar-1"></div><div class="bar bar-2"></div><div class="bar bar-3"></div><div class="bar bar-4"></div><div class="bar bar-5"></div>
          </div>
          <button class="voice-stop-button" type="button" on:click={onStopSpeaking}>
            <X size={14} />
            <span>{labels.stopSpeakingBtn}</span>
          </button>
        </div>
      {/if}

      {#if recording}
        <div class="waveform-container" style="display: flex; justify-content: center; width: 100%; height: 60px; margin-top: 10px; margin-bottom: 4px;">
          <svg class="live-waveform" viewBox="0 0 200 60" style="width: 100%; max-width: 280px; height: 60px;">
            <path d={getWavePath(0, voiceVolume, 60)} fill="none" stroke="rgba(10, 132, 255, 0.8)" stroke-width="2.5" stroke-linecap="round" />
            <path d={getWavePath(2, voiceVolume * 0.6, 60)} fill="none" stroke="rgba(162, 36, 214, 0.6)" stroke-width="2" stroke-linecap="round" />
            <path d={getWavePath(4, voiceVolume * 0.3, 60)} fill="none" stroke="rgba(255, 107, 107, 0.5)" stroke-width="1.5" stroke-linecap="round" />
          </svg>
        </div>
      {/if}

      <h1>ARO</h1>

      <!-- Selected active profile in empty state -->
      <div class="empty-state-personality-selector" style="margin-top: 24px; display: flex; flex-direction: column; align-items: center; gap: 8px; max-width: 520px; width: 100%; padding: 0 20px; box-sizing: border-box;">
        <span style="font-size: 10px; font-weight: 700; text-transform: uppercase; color: #86868b; letter-spacing: 0.05em; margin-bottom: 2px;">{language === "fr" ? "Profil ARO" : "ARO Profile"}</span>
        <div style="display: flex; flex-wrap: wrap; justify-content: center; gap: 8px; width: 100%;">
          {#each personalities as personality}
            <button
              type="button"
              style="display: flex; align-items: center; gap: 8px; padding: 6px 14px; border-radius: 20px; border: 1px solid {selectedPersonalityId === personality.id ? '#0071e3' : (theme === 'dark' ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.1)')}; background: {selectedPersonalityId === personality.id ? 'rgba(0,113,227,0.12)' : (theme === 'dark' ? 'rgba(255,255,255,0.03)' : 'rgba(255,255,255,0.8)')}; cursor: pointer; transition: all 0.2s;"
              disabled={cloudWriteLocked}
              title={cloudWriteDisabledTitle("changer la personnalite active") ?? personality.name}
              on:click={() => onSelectPersonality(personality.id)}
            >
              <div style="width: 18px; height: 18px; border-radius: 50%; background: {personality.avatarColor}; display: flex; align-items: center; justify-content: center; color: #ffffff; flex-shrink: 0;">
                {#if personality.icon === "bot"}<Bot size={10} />
                {:else if personality.icon === "code"}<Terminal size={10} />
                {:else if personality.icon === "check"}<Check size={10} />
                {:else if personality.icon === "edit-2"}<Edit2 size={10} />
                {:else if personality.icon === "brain"}<Brain size={10} />
                {:else if personality.icon === "cpu"}<Cpu size={10} />
                {:else if personality.icon === "user"}<User size={10} />
                {:else if personality.icon === "activity"}<Activity size={10} />
                {:else if personality.icon === "sliders"}<Sliders size={10} />
                {:else if personality.icon === "building-2"}<Building2 size={10} />
                {:else}<Search size={10} />{/if}
              </div>
              <span style="font-size: 12px; font-weight: 500; color: {selectedPersonalityId === personality.id ? '#0071e3' : (theme === 'dark' ? '#f5f5f7' : '#1d1d1f')};">{personality.name}</span>
            </button>
          {/each}
        </div>
      </div>

      {#if showDestinationPicker}
        <div class="empty-state-destination-selector" style="margin-top: 18px; display: flex; flex-direction: column; align-items: center; gap: 8px; max-width: 520px; width: 100%; padding: 0 20px; box-sizing: border-box;">
          <span style="font-size: 10px; font-weight: 700; text-transform: uppercase; color: #86868b; letter-spacing: 0.05em; margin-bottom: 2px;">{language === "fr" ? "Classer dans" : "File under"}</span>
          <ConversationDestinationPicker
            {projects}
            {folders}
            {pendingProjectId}
            {pendingFolderId}
            {language}
            onSelect={onSelectDestination}
          />
        </div>
      {/if}
    </div>
  {:else}
    <div class="message-stream">
      {#each messages as message}
        <article class:assistant={message.role === "assistant"} class:user={message.role === "user"} class="message">
          <div class="avatar">{#if message.role === "assistant"}<Bot size={16} />{:else}<User size={16} />{/if}</div>
          <div class="bubble">
            <div class="message-meta">
              <span class="sender-name">{message.role === "assistant" ? "ARO" : labels.youLabel}</span>
              {#if message.role === "assistant"}<span class="ai-badge">AI</span>{/if}
              <span class="msg-time">{formatTime(message.createdAt)}</span>
            </div>
            {#if message.role === "assistant"}
              {#if message.isGenerating && !hasMeaningfulAgentSteps(message.steps) && !message.content}
                <div class="thinking-bubble"><div class="thinking-gemini-gradient"></div><div class="thinking-text-placeholder"><div class="thinking-bar-1"></div><div class="thinking-bar-2"></div></div></div>
              {:else}
                {#if hasMeaningfulAgentSteps(message.steps)}
                  <div class="agent-steps-container">
                    <div class="agent-steps-header">
                      <div class="agent-active-badge"><span class="pulse-dot" class:generating={message.isGenerating} class:completed={!message.isGenerating}></span><span class="badge-label">{message.isGenerating ? (language === "fr" ? "Outil en cours" : "Tool in progress") : (language === "fr" ? "Outils exécutés" : "Tools completed")}</span></div>
                      <button class="agent-steps-toggle-btn" type="button" on:click={() => onToggleMessageSteps(message.id)}>{expandedMessageSteps[message.id] ? (language === "fr" ? "Masquer les étapes" : "Hide steps") : (language === "fr" ? `Afficher les étapes (${getMeaningfulAgentSteps(message.steps).length})` : `Show steps (${getMeaningfulAgentSteps(message.steps).length})`)}</button>
                    </div>

                    {#if getActiveToolStep(getMeaningfulAgentSteps(message.steps), message.isGenerating, Boolean(expandedMessageSteps[message.id]))}
                      {@const activeToolStep = getActiveToolStep(getMeaningfulAgentSteps(message.steps), message.isGenerating, Boolean(expandedMessageSteps[message.id]))}
                      <div class="agent-live-tool-strip" class:is-browser={isBrowserStep(activeToolStep)}>
                        <div class="live-tool-desc">
                          {#if isBrowserStep(activeToolStep)}
                            <Globe size={13} class="live-tool-icon browser" />
                          {:else if ((activeToolStep.input?.toolId || "") + " " + (activeToolStep.title || "")).toLowerCase().includes("connector") || ((activeToolStep.input?.toolId || "") + " " + (activeToolStep.title || "")).toLowerCase().includes("plugin")}
                            <Puzzle size={13} class="live-tool-icon connector" />
                          {:else if (activeToolStep.input?.toolId || "").toLowerCase().includes("shell") || (activeToolStep.input?.toolId || "").toLowerCase().includes("terminal")}
                            <Terminal size={13} class="live-tool-icon shell" />
                          {:else}
                            <Cpu size={13} class="live-tool-icon default" />
                          {/if}
                          <div class="live-tool-texts">
                            <span class="live-tool-title">{activeToolStep.title || (language === "fr" ? "Exécution de l'outil" : "Running tool")}</span>
                            {#if getStepUrl(activeToolStep)}
                              <span class="live-tool-url" title={getStepUrl(activeToolStep)}>{getStepUrl(activeToolStep)}</span>
                            {/if}
                          </div>
                        </div>
                        {#if isBrowserStep(activeToolStep) && getStepUrl(activeToolStep)}
                          <div class="live-tool-buttons">
                            <button
                              type="button"
                              class="live-take-control-btn"
                              title={language === "fr" ? "Prendre le contrôle immédiat du navigateur" : "Take immediate browser control"}
                              on:click|stopPropagation={() => handleTakeBrowserControl(getStepUrl(activeToolStep), true)}
                            >
                              <Sliders size={11} />
                              <span>{language === "fr" ? "Prendre le contrôle" : "Take Control"}</span>
                            </button>
                            <button
                              type="button"
                              class="live-open-tab-btn"
                              title={language === "fr" ? "Ouvrir dans un onglet dédié" : "Open in new tab"}
                              on:click|stopPropagation={() => handleTakeBrowserControl(getStepUrl(activeToolStep), false, true)}
                            >
                              <ExternalLink size={11} />
                              <span>{language === "fr" ? "Ouvrir l'onglet" : "Open tab"}</span>
                            </button>
                          </div>
                        {/if}
                      </div>
                    {/if}

                    {#if expandedMessageSteps[message.id]}
                      <div class="agent-steps-list-embedded">
                        {#each getMeaningfulAgentSteps(message.steps) as step}
                          <AgentStepCard
                            {step}
                            messageId={message.id}
                            isExpanded={Boolean(expandedStepDetails[`${message.id}-${step.sequence}`])}
                            {language}
                            {theme}
                            onToggleDetails={() => onToggleStepDetails(message.id, step.sequence)}
                          />
                        {/each}
                      </div>
                    {/if}
                  </div>
                {/if}
                {#if message.content}
                  {@const parsed = parseMessageThinking(message.content)}
                  {#if parsed.hasReasoning}
                    {@const isExpanded = isThinkingExpanded(expandedThinkingBlocks, message.id, parsed.isReasoningComplete)}
                    <div class="thinking-block-wrapper" class:generating={!parsed.isReasoningComplete}>
                      <!-- svelte-ignore a11y_click_events_have_key_events -->
                      <!-- svelte-ignore a11y_no_static_element_interactions -->
                      <div 
                        class="thinking-block-header" 
                        role="button"
                        tabindex="0"
                        on:click={() => toggleThinkingBlock(message.id, parsed.isReasoningComplete)}
                        on:keydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); toggleThinkingBlock(message.id, parsed.isReasoningComplete); } }}
                        aria-expanded={isExpanded}
                      >
                        <div class="thinking-header-left">
                          <Brain class="thinking-brain-icon" size={14} />
                          <span class="thinking-title">
                            {#if !parsed.isReasoningComplete}
                              {language === "fr" ? "ARO réfléchit..." : "ARO is thinking..."}
                            {:else}
                              {language === "fr" ? "Réflexion" : "Reasoning"}
                            {/if}
                          </span>
                          {#if !parsed.isReasoningComplete}
                            <span class="thinking-pulse-dot"></span>
                          {/if}
                        </div>
                        <div class="thinking-header-right">
                          <button
                            type="button"
                            class="thinking-copy-btn"
                            title={language === "fr" ? "Copier la réflexion" : "Copy reasoning"}
                            on:click|stopPropagation={() => handleCopyThinking(message.id, parsed.reasoning)}
                          >
                            {#if copiedThinkingId === message.id}
                              <Check size={12} class="thinking-copied-icon" />
                            {:else}
                              <Copy size={12} />
                            {/if}
                          </button>
                          <span class="thinking-chevron" class:expanded={isExpanded}>
                            <ChevronDown size={14} />
                          </span>
                        </div>
                      </div>
                      {#if isExpanded}
                        <div class="thinking-block-content">
                          <div class="thinking-inner-text">
                            {parsed.reasoning}
                          </div>
                        </div>
                      {/if}
                    </div>
                  {/if}
                  
                  {#if parsed.actualContent}
                    <div class="markdown-body">
                      {@html renderMarkdown(parsed.actualContent, language)}
                    </div>
                  {:else if message.isGenerating && parsed.hasReasoning && !parsed.isReasoningComplete}
                    <!-- Thinking in progress, wait for actual response content -->
                  {:else if message.isGenerating && !parsed.hasReasoning}
                    <div class="thinking-gemini-gradient" style="height: 4px; border-radius: 2px; margin-top: 8px;"></div>
                  {:else if !message.isGenerating && parsed.hasReasoning && !parsed.actualContent}
                    <div class="thinking-no-content-notice" style="margin-top: 8px; font-size: 13px; color: #86868b; font-style: italic;">
                      {language === "fr" ? "Réflexion terminée sans réponse textuelle." : "Reasoning completed without textual response."}
                    </div>
                  {/if}
                {:else if message.isGenerating}
                  <div class="thinking-bubble">
                    <div class="thinking-gemini-gradient"></div>
                    <div class="thinking-text-placeholder">
                      <div class="thinking-bar-1"></div>
                      <div class="thinking-bar-2"></div>
                    </div>
                  </div>
                {/if}
              {/if}
              {#if message.attachments?.length}<div class="message-attachments">{#each message.attachments as attachment}<!-- svelte-ignore a11y_click_events_have_key_events --><!-- svelte-ignore a11y_no_static_element_interactions --><span class="message-attachment-chip clickable" on:click={() => onPreviewAttachment(attachment)} title="Aperçu du fichier"><FileText size={13} /><span>{attachment.displayName}</span><span>{formatFileSize(attachment.sizeBytes)}</span></span>{/each}</div>{/if}
              <div class="message-actions">
                <button class="msg-action-btn" class:copied={copiedMessageId === message.id} type="button" title={labels.copyBtn} on:click={() => onCopyMessage(message.id, message.content)}><Copy size={13} /><span>{copiedMessageId === message.id ? labels.copiedBtn : labels.copyBtn}</span></button>
                <button class="msg-action-btn" type="button" title={language === "fr" ? "Retenir" : "Remember"} on:click={() => onRememberMessage(message)}><Brain size={13} /><span>{language === "fr" ? "Retenir" : "Remember"}</span></button>
                <div class="feedback-actions">
                  <button class="msg-action-btn feedback-btn" class:active={messageFeedback[message.id] === "good"} type="button" title={labels.goodAnswerTitle} on:click={() => onFeedback(message.id, "good")}><ThumbsUp size={13} /></button>
                  <button class="msg-action-btn feedback-btn" class:active={messageFeedback[message.id] === "bad"} type="button" title={labels.badAnswerTitle} on:click={() => onFeedback(message.id, "bad")}><ThumbsDown size={13} /></button>
                </div>
              </div>
            {:else}
              {#if editingMessageId === message.id}
                <div class="message-edit-container">
                  <textarea bind:value={editingMessageText} rows="2" class="message-edit-textarea" on:keydown={(event) => { if ((event.metaKey || event.ctrlKey) && event.key === "Enter") onSaveEdit(message); else if (event.key === "Escape") onCancelEdit(); }}></textarea>
                  <div class="message-edit-actions"><button class="edit-btn secondary" type="button" on:click={onCancelEdit}>{labels.cancel}</button><button class="edit-btn primary" type="button" on:click={() => onSaveEdit(message)} disabled={!editingMessageText.trim() || sending}>{labels.save}</button></div>
                </div>
              {:else}
                <p>{message.content}</p>
                {#if message.attachments?.length}<div class="message-attachments">{#each message.attachments as attachment}<!-- svelte-ignore a11y_click_events_have_key_events --><!-- svelte-ignore a11y_no_static_element_interactions --><span class="message-attachment-chip clickable" on:click={() => onPreviewAttachment(attachment)} title="AperÃ§u du fichier"><FileText size={13} /><span>{attachment.displayName}</span><span>{formatFileSize(attachment.sizeBytes)}</span></span>{/each}</div>{/if}
                <div class="message-actions user-actions">
                  <button class="msg-action-btn" class:copied={copiedMessageId === message.id} type="button" title={labels.copyBtn} on:click={() => onCopyMessage(message.id, message.content)}><Copy size={13} /><span>{copiedMessageId === message.id ? labels.copiedBtn : labels.copyBtn}</span></button>
                  <button class="msg-action-btn" type="button" title={language === "fr" ? "Retenir" : "Remember"} on:click={() => onRememberMessage(message)}><Brain size={13} /><span>{language === "fr" ? "Retenir" : "Remember"}</span></button>
                  <button class="msg-action-btn" type="button" title={language === "fr" ? "Modifier" : "Edit"} on:click={() => onStartEdit(message)}><Edit2 size={13} /><span>{language === "fr" ? "Modifier" : "Edit"}</span></button>
                </div>
              {/if}
            {/if}
          </div>
        </article>
      {/each}
      <div bind:this={messagesEnd}></div>
    </div>
  {/if}
</div>
