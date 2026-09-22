<script lang="ts">
  import Check from "@lucide/svelte/icons/check";
  import Play from "@lucide/svelte/icons/play";
  import Sparkles from "@lucide/svelte/icons/sparkles";
  import Volume2 from "@lucide/svelte/icons/volume-2";
  import Key from "@lucide/svelte/icons/key";
  import Cpu from "@lucide/svelte/icons/cpu";
  import Mic from "@lucide/svelte/icons/mic";
  import ShieldCheck from "@lucide/svelte/icons/shield-check";
  import VoiceOrb from "../../voice/VoiceOrb.svelte";
  import {
    activeVoiceProvider,
    selectedVoiceId as selectedVoiceIdStore,
    voiceApiKeys,
    AVAILABLE_VOICES,
    testVoiceSample,
    type VoiceProvider,
    type VoiceDefinition,
  } from "../../voice/voice-service";
  import type { AppSettings, RuntimeStatus, VoiceProfile } from "../../../lib/types";

  type MaybeAsync = void | Promise<void>;
  type VoiceIssueStatus = { issues?: { message: string }[] } | null | undefined;

  export let settingsDraft: AppSettings;
  export let language: "fr" | "en" = "fr";
  export let theme: "light" | "dark" = "dark";
  export let labels: Record<string, string> = {};
  export let runtime: RuntimeStatus | null = null;
  export let modelRuntimeReady: boolean = true;
  export let modelReadinessLabel: string = "Prêt";
  export let voiceSpeechToTextReady: boolean = true;
  export let voiceSpeechToTextStatus: VoiceIssueStatus = null;
  export let speechReadinessLabel: string = "Prêt";
  export let voiceWakeModelReady: boolean = true;
  export let wakeWordEnabled: boolean = false;
  export let wakeWordReadinessLabel: string = "Prêt";
  export let voiceTextToSpeechReady: boolean = true;
  export let voiceTextToSpeechStatus: VoiceIssueStatus = null;
  export let ttsReadinessLabel: string = "Prêt";
  export let voiceReadinessIssue: string = "";
  export let wakeWordDraftEnabled: boolean = false;
  export let sttOptions: Array<{ value: any; label: string }> = [];
  export let ttsOptions: Array<{ value: any; label: string }> = [];
  export let voices: VoiceProfile[] = [];
  export let writeLocked: boolean = false;
  export let firstVoiceIssue: (status: VoiceIssueStatus) => string = () => "";
  export let formatTime: (value: string) => string = (v) => v;
  export let writeDisabledTitle: (action?: string) => string | null | undefined = () => null;
  export let onAutosave: () => MaybeAsync = () => {};
  export let onOpenCreateVoice: () => MaybeAsync = () => {};
  export let onOpenEditVoice: (voice: VoiceProfile) => MaybeAsync = () => {};
  export let onDeleteVoice: (voiceId: string) => MaybeAsync = () => {};
  export let onSelectVoice: (voiceId: string) => MaybeAsync = () => {};
  export let selectedVoiceId: string = "";

  let openaiKeyInput = $voiceApiKeys.openai || "";
  let geminiKeyInput = $voiceApiKeys.gemini || "";
  let keySaveNotice = false;

  $: if (selectedVoiceId) {
    selectedVoiceIdStore.set(selectedVoiceId);
  }

  function handleSaveApiKeys() {
    voiceApiKeys.set({
      openai: openaiKeyInput.trim(),
      gemini: geminiKeyInput.trim(),
    });
    keySaveNotice = true;
    setTimeout(() => (keySaveNotice = false), 2000);
  }

  function handleSelectProvider(provider: VoiceProvider) {
    activeVoiceProvider.set(provider);
    // Select first voice of this provider
    const match = AVAILABLE_VOICES.find((v: VoiceDefinition) => v.provider === provider);
    if (match) {
      selectedVoiceIdStore.set(match.id);
      selectedVoiceId = match.id;
    }
  }

  function handlePickVoice(id: string) {
    selectedVoiceIdStore.set(id);
    selectedVoiceId = id;
    onSelectVoice(id);
  }

  $: filteredVoices = AVAILABLE_VOICES.filter((v: VoiceDefinition) => v.provider === $activeVoiceProvider);
</script>

<div class="settings-tab-panel futuristic-voice-settings animate-fade-in">
  <!-- Header -->
  <div class="panel-header">
    <div class="header-with-badge">
      <h2>{language === "fr" ? "Voix & Intelligence Vocale AGI" : "Voice & AGI Speech Intelligence"}</h2>
      <span class="agi-badge">
        <Sparkles size={12} />
        Apple-grade Zero Config
      </span>
    </div>
    <p>
      {language === "fr"
        ? "Bénéficiez d'une expérience vocale fluide et instantanée, sans installation ni téléchargement de modèles. Choisissez entre le moteur ARO intégré ou les API en streaming ultra-faible latence."
        : "Experience frictionless, real-time voice intelligence with zero manual downloads. Select between the native zero-config engine or cloud streaming APIs."}
    </p>
  </div>

  <!-- Realtime Interactive ORB Stage Preview in Settings -->
  <div class="orb-preview-card">
    <div class="orb-preview-left">
      <VoiceOrb size={90} showStateLabel={true} {language} interactive={true} />
    </div>
    <div class="orb-preview-right">
      <div class="orb-preview-title">
        <Sparkles size={14} style="color: #bf5af2;" />
        <h4>{language === "fr" ? "ARO Luminous ORB (Interactif)" : "ARO Luminous ORB (Interactive)"}</h4>
      </div>
      <p>
        {language === "fr"
          ? "Cliquez sur l'ORB pour tester la détection vocale en temps réel. L'aura chromatique s'adapte dynamiquement au spectre de fréquence de votre voix."
          : "Click the ORB to test real-time voice reactivity. The chromatic aura dynamically adjusts to your voice frequencies."}
      </p>
      <div class="orb-stats">
        <span class="stat-pill">
          <ShieldCheck size={11} />
          {language === "fr" ? "Modèle pré-installé" : "Pre-installed model"}
        </span>
        <span class="stat-pill">
          <Mic size={11} />
          {language === "fr" ? "Latence < 120ms" : "Latency < 120ms"}
        </span>
      </div>
    </div>
  </div>

  <!-- SECTION 1: Choix du Fournisseur Vocal -->
  <div class="settings-group">
    <div class="group-title-row">
      <Cpu size={16} class="group-icon purple" />
      <h3>{language === "fr" ? "Moteur & Fournisseur Vocal" : "Voice Engine & Provider"}</h3>
    </div>

    <div class="provider-cards-grid">
      <!-- Builtin Native -->
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="provider-card"
        class:selected={$activeVoiceProvider === "builtin"}
        on:click={() => handleSelectProvider("builtin")}
      >
        <div class="provider-card-header">
          <div class="provider-icon blue">
            <Sparkles size={18} />
          </div>
          {#if $activeVoiceProvider === "builtin"}
            <span class="active-check"><Check size={14} /></span>
          {/if}
        </div>
        <h4>{language === "fr" ? "Moteur ARO Intégré" : "ARO Native Engine"}</h4>
        <p class="provider-desc">
          {language === "fr"
            ? "Zéro configuration, instantané et disponible hors-ligne. Utilise la synthèse neurale système."
            : "Zero setup, instant and available offline. Utilizes neural system synthesis."}
        </p>
        <span class="provider-badge free">{language === "fr" ? "Inclus / Gratuit" : "Included / Free"}</span>
      </div>

      <!-- OpenAI Realtime / GPT Live -->
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="provider-card"
        class:selected={$activeVoiceProvider === "openai_realtime"}
        on:click={() => handleSelectProvider("openai_realtime")}
      >
        <div class="provider-card-header">
          <div class="provider-icon green">
            <Volume2 size={18} />
          </div>
          {#if $activeVoiceProvider === "openai_realtime"}
            <span class="active-check"><Check size={14} /></span>
          {/if}
        </div>
        <h4>OpenAI GPT Live</h4>
        <p class="provider-desc">
          {language === "fr"
            ? "Streaming vocal bidirectionnel ultra-faible latence propulsé par GPT-4o Realtime."
            : "Ultra-low latency bidirectional voice streaming powered by GPT-4o Realtime."}
        </p>
        <span class="provider-badge pro">API Key</span>
      </div>

      <!-- Google Gemini Live -->
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="provider-card"
        class:selected={$activeVoiceProvider === "gemini_live"}
        on:click={() => handleSelectProvider("gemini_live")}
      >
        <div class="provider-card-header">
          <div class="provider-icon purple">
            <Cpu size={18} />
          </div>
          {#if $activeVoiceProvider === "gemini_live"}
            <span class="active-check"><Check size={14} /></span>
          {/if}
        </div>
        <h4>Google Gemini Live</h4>
        <p class="provider-desc">
          {language === "fr"
            ? "Intelligence vocale multimodale Gemini 2.0 Flash avec expressivité avancée."
            : "Gemini 2.0 Flash multimodal voice intelligence with expressive nuances."}
        </p>
        <span class="provider-badge pro">API Key</span>
      </div>
    </div>

    <!-- External API Key Inputs if external provider selected -->
    {#if $activeVoiceProvider !== "builtin"}
      <div class="api-key-config-box animate-fade-in">
        <div class="api-key-header">
          <Key size={14} class="key-icon" />
          <span>
            {$activeVoiceProvider === "openai_realtime"
              ? "Clé API OpenAI pour GPT-4o Realtime"
              : "Clé API Google AI Studio pour Gemini Live"}
          </span>
        </div>
        <div class="api-key-input-row">
          {#if $activeVoiceProvider === "openai_realtime"}
            <input
              type="password"
              class="api-input"
              bind:value={openaiKeyInput}
              placeholder="sk-proj-••••••••••••••••••••••••"
            />
          {:else}
            <input
              type="password"
              class="api-input"
              bind:value={geminiKeyInput}
              placeholder="AIzaSy••••••••••••••••••••••••"
            />
          {/if}
          <button type="button" class="apple-btn primary small" on:click={handleSaveApiKeys}>
            {language === "fr" ? "Enregistrer" : "Save"}
          </button>
        </div>
        {#if keySaveNotice}
          <div class="key-saved-notice animate-fade-in">
            <Check size={12} />
            <span>{language === "fr" ? "Clé API configurée avec succès." : "API Key saved successfully."}</span>
          </div>
        {/if}
      </div>
    {/if}
  </div>

  <!-- SECTION 2: Sélection des Voix -->
  <div class="settings-group">
    <div class="group-title-row">
      <Volume2 size={16} class="group-icon blue" />
      <h3>{language === "fr" ? "Voix Disponibles" : "Available Voices"}</h3>
    </div>

    <div class="voices-cards-list">
      {#each filteredVoices as voice}
        <div
          class="voice-card"
          class:active={$selectedVoiceIdStore === voice.id}
        >
          <div class="voice-card-left">
            <div class="voice-avatar" class:female={voice.gender === "female"} class:male={voice.gender === "male"}>
              <Volume2 size={16} />
            </div>
            <div class="voice-details">
              <div class="voice-name-row">
                <span class="voice-title">{voice.name}</span>
                <span class="voice-gender-badge">{voice.gender}</span>
                <span class="voice-lang-badge">{voice.language}</span>
              </div>
              <span class="voice-desc">{voice.description}</span>
            </div>
          </div>

          <div class="voice-card-actions">
            <button
              type="button"
              class="apple-btn secondary small test-btn"
              title={language === "fr" ? "Écouter un extrait de cette voix" : "Listen to a sample"}
              on:click={() => testVoiceSample(voice.id)}
            >
              <Play size={12} style="margin-right: 4px;" />
              <span>{language === "fr" ? "Tester" : "Test"}</span>
            </button>

            {#if $selectedVoiceIdStore === voice.id}
              <span class="active-badge">
                <Check size={12} />
                {language === "fr" ? "Active" : "Active"}
              </span>
            {:else}
              <button
                type="button"
                class="apple-btn primary small"
                disabled={writeLocked}
                on:click={() => handlePickVoice(voice.id)}
              >
                {language === "fr" ? "Choisir" : "Select"}
              </button>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  </div>

  <!-- SECTION 3: Options Vocales Avancées -->
  <div class="settings-group">
    <div class="group-title-row">
      <Sparkles size={16} class="group-icon green" />
      <h3>{language === "fr" ? "Options de Lecture & Interaction" : "Playback & Interaction"}</h3>
    </div>

    <div class="settings-row">
      <div class="settings-label-col">
        <span class="settings-title">{labels.speakResponsesTitle || (language === "fr" ? "Énoncer automatiquement les réponses" : "Speak Responses Automatically")}</span>
        <span class="settings-desc">{labels.speakResponsesDesc || (language === "fr" ? "L'assistant lit à voix haute chaque réponse générée par l'IA." : "Assistant automatically reads out responses.")}</span>
      </div>
      <div class="settings-control-col">
        <label class="ios-toggle">
          <input
            type="checkbox"
            bind:checked={settingsDraft.speakResponses}
            disabled={writeLocked}
            on:change={onAutosave}
          />
          <span class="slider"></span>
        </label>
      </div>
    </div>

    <div class="settings-row">
      <div class="settings-label-col">
        <span class="settings-title">{labels.voiceActivationTitle || (language === "fr" ? "Activer l'interaction vocale mains-libres" : "Hands-Free Voice Activation")}</span>
        <span class="settings-desc">{labels.voiceActivationDesc || (language === "fr" ? "Permet de converser en continu avec l'ORB sans cliquer sur le micro." : "Continuous conversational session with the ORB.")}</span>
      </div>
      <div class="settings-control-col">
        <label class="ios-toggle">
          <input
            type="checkbox"
            bind:checked={settingsDraft.voice.enabled}
            disabled={writeLocked}
            on:change={onAutosave}
          />
          <span class="slider"></span>
        </label>
      </div>
    </div>
  </div>
</div>

<style>
  .futuristic-voice-settings {
    max-width: 820px;
  }

  .panel-header {
    margin-bottom: 24px;
  }

  .header-with-badge {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 6px;
  }

  .panel-header h2 {
    margin: 0;
    font-size: 19px;
    font-weight: 700;
  }

  .agi-badge {
    display: flex;
    align-items: center;
    gap: 4px;
    background: linear-gradient(135deg, rgba(175, 82, 222, 0.2), rgba(0, 113, 227, 0.2));
    border: 1px solid rgba(175, 82, 222, 0.35);
    color: #df9aff;
    font-size: 10px;
    font-weight: 700;
    padding: 3px 8px;
    border-radius: 20px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .panel-header p {
    margin: 0;
    font-size: 13px;
    color: #8e8e93;
    line-height: 1.4;
  }

  /* ORB Preview Card */
  .orb-preview-card {
    display: flex;
    align-items: center;
    gap: 24px;
    background: radial-gradient(circle at left, rgba(175, 82, 222, 0.15), rgba(255, 255, 255, 0.03) 70%);
    border: 1px solid rgba(175, 82, 222, 0.3);
    border-radius: 14px;
    padding: 16px 20px;
    margin-bottom: 24px;
  }

  .orb-preview-left {
    flex-shrink: 0;
  }

  .orb-preview-right {
    flex: 1;
  }

  .orb-preview-title {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 4px;
  }

  .orb-preview-title h4 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
  }

  .orb-preview-right p {
    margin: 0 0 10px 0;
    font-size: 12px;
    color: #a1a1aa;
    line-height: 1.4;
  }

  .orb-stats {
    display: flex;
    gap: 8px;
  }

  .stat-pill {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 10px;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.1);
    padding: 2px 8px;
    border-radius: 12px;
    color: #e4e4e7;
  }

  /* Settings group */
  .settings-group {
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 12px;
    padding: 18px 20px;
    margin-bottom: 20px;
  }

  .group-title-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 16px;
  }

  .group-title-row h3 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }

  .group-icon.purple { color: #af52de; }
  .group-icon.blue { color: #0071e3; }
  .group-icon.green { color: #30d158; }

  /* Provider Cards Grid */
  .provider-cards-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 12px;
  }

  .provider-card {
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 10px;
    padding: 14px;
    cursor: pointer;
    transition: all 0.2s cubic-bezier(0.34, 1.56, 0.64, 1);
    position: relative;
    display: flex;
    flex-direction: column;
  }

  .provider-card:hover {
    background: rgba(255, 255, 255, 0.06);
    border-color: rgba(255, 255, 255, 0.15);
    transform: translateY(-2px);
  }

  .provider-card.selected {
    background: rgba(0, 113, 227, 0.12);
    border-color: #0071e3;
    box-shadow: 0 0 12px rgba(0, 113, 227, 0.25);
  }

  .provider-card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 8px;
  }

  .provider-icon {
    width: 32px;
    height: 32px;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .provider-icon.blue { background: rgba(0, 113, 227, 0.15); color: #0071e3; }
  .provider-icon.green { background: rgba(48, 209, 88, 0.15); color: #30d158; }
  .provider-icon.purple { background: rgba(175, 82, 222, 0.15); color: #bf5af2; }

  .active-check {
    color: #0071e3;
  }

  .provider-card h4 {
    margin: 0 0 4px 0;
    font-size: 13px;
    font-weight: 600;
  }

  .provider-desc {
    margin: 0 0 12px 0;
    font-size: 11px;
    color: #8e8e93;
    line-height: 1.4;
    flex: 1;
  }

  .provider-badge {
    align-self: flex-start;
    font-size: 9px;
    font-weight: 700;
    padding: 2px 6px;
    border-radius: 4px;
    text-transform: uppercase;
  }

  .provider-badge.free {
    background: rgba(48, 209, 88, 0.15);
    color: #30d158;
  }

  .provider-badge.pro {
    background: rgba(175, 82, 222, 0.15);
    color: #bf5af2;
  }

  /* API Key Config Box */
  .api-key-config-box {
    margin-top: 14px;
    background: rgba(0, 0, 0, 0.25);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 8px;
    padding: 12px 14px;
  }

  .api-key-header {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    font-weight: 500;
    margin-bottom: 8px;
    color: #f4f4f5;
  }

  .key-icon {
    color: #ff9f0a;
  }

  .api-key-input-row {
    display: flex;
    gap: 8px;
  }

  .api-input {
    flex: 1;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #ffffff;
    border-radius: 6px;
    padding: 6px 10px;
    font-size: 12px;
    outline: none;
    font-family: monospace;
  }

  .api-input:focus {
    border-color: #0071e3;
  }

  .key-saved-notice {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-top: 6px;
    font-size: 11px;
    color: #30d158;
  }

  /* Voices Cards List */
  .voices-cards-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .voice-card {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 14px;
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 8px;
    transition: all 0.15s;
  }

  .voice-card.active {
    background: rgba(0, 113, 227, 0.08);
    border-color: rgba(0, 113, 227, 0.4);
  }

  .voice-card-left {
    display: flex;
    align-items: center;
    gap: 12px;
    flex: 1;
  }

  .voice-avatar {
    width: 34px;
    height: 34px;
    border-radius: 50%;
    background: #0071e3;
    color: #ffffff;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .voice-avatar.female { background: #af52de; }
  .voice-avatar.male { background: #0071e3; }

  .voice-details {
    display: flex;
    flex-direction: column;
  }

  .voice-name-row {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 2px;
  }

  .voice-title {
    font-size: 13px;
    font-weight: 600;
  }

  .voice-gender-badge,
  .voice-lang-badge {
    font-size: 9px;
    padding: 1px 4px;
    border-radius: 3px;
    text-transform: uppercase;
    font-weight: 600;
    background: rgba(255, 255, 255, 0.08);
    color: #8e8e93;
  }

  .voice-desc {
    font-size: 11px;
    color: #8e8e93;
  }

  .voice-card-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  .active-badge {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    font-weight: 600;
    color: #0071e3;
    padding: 4px 8px;
  }

  /* Settings row */
  .settings-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  }

  .settings-row:last-child {
    border-bottom: none;
    padding-bottom: 0;
  }

  .settings-label-col {
    flex: 1;
    margin-right: 16px;
  }

  .settings-title {
    display: block;
    font-size: 13px;
    font-weight: 600;
    color: #f4f4f5;
  }

  .settings-desc {
    display: block;
    font-size: 12px;
    color: #8e8e93;
    margin-top: 2px;
    line-height: 1.4;
  }

  .apple-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 6px 12px;
    border-radius: 6px;
    font-size: 12px;
    font-weight: 500;
    border: none;
    cursor: pointer;
    transition: all 0.15s;
  }

  .apple-btn.small {
    padding: 4px 8px;
    font-size: 11px;
  }

  .apple-btn.primary {
    background: #0071e3;
    color: #ffffff;
  }

  .apple-btn.primary:hover {
    background: #0077ed;
  }

  .apple-btn.secondary {
    background: rgba(255, 255, 255, 0.08);
    color: #f4f4f5;
  }

  .apple-btn.secondary:hover {
    background: rgba(255, 255, 255, 0.14);
  }
</style>
