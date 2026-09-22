<script lang="ts">
  import X from "@lucide/svelte/icons/x";
  import Mic from "@lucide/svelte/icons/mic";
  import MicOff from "@lucide/svelte/icons/mic-off";
  import Volume2 from "@lucide/svelte/icons/volume-2";
  import Sparkles from "@lucide/svelte/icons/sparkles";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import VoiceOrb from "./VoiceOrb.svelte";
  import {
    voiceState,
    liveTranscript,
    assistantTranscript,
    activeVoiceProvider,
    selectedVoiceId,
    AVAILABLE_VOICES,
    startVoiceSession,
    stopVoiceSession,
    stopSpeaking,
  } from "./voice-service";

  export let language: "fr" | "en" = "fr";
  export let onClose: () => void;
  export let onSubmitToChat: ((userText: string) => void) | undefined = undefined;

  let showVoiceMenu = false;

  $: activeVoice = AVAILABLE_VOICES.find((v) => v.id === $selectedVoiceId) || AVAILABLE_VOICES[0];

  function toggleMic() {
    if ($voiceState === "listening") {
      stopVoiceSession();
      if ($liveTranscript.trim() && onSubmitToChat) {
        onSubmitToChat($liveTranscript.trim());
      }
    } else {
      startVoiceSession();
    }
  }

  function handleSelectVoice(id: string) {
    selectedVoiceId.set(id);
    showVoiceMenu = false;
  }
</script>

<div class="voice-live-overlay animate-fade-in">
  <!-- Top Bar -->
  <div class="overlay-topbar">
    <div class="provider-pill">
      <Sparkles size={13} class="sparkle-icon" />
      <span>
        {$activeVoiceProvider === "openai_realtime"
          ? "OpenAI Realtime Live"
          : $activeVoiceProvider === "gemini_live"
          ? "Google Gemini Live"
          : "ARO Moteur AGI Natif"}
      </span>
    </div>

    <!-- Active Voice Selector Button -->
    <div class="voice-selector-wrapper">
      <button
        type="button"
        class="voice-picker-btn"
        on:click={() => (showVoiceMenu = !showVoiceMenu)}
      >
        <Volume2 size={13} />
        <span>{activeVoice.name}</span>
        <ChevronDown size={12} />
      </button>

      {#if showVoiceMenu}
        <div class="voice-dropdown animate-fade-in">
          {#each AVAILABLE_VOICES as voice}
            <button
              type="button"
              class="dropdown-voice-item"
              class:selected={voice.id === $selectedVoiceId}
              on:click={() => handleSelectVoice(voice.id)}
            >
              <span class="v-name">{voice.name}</span>
              <span class="v-lang">{voice.language}</span>
            </button>
          {/each}
        </div>
      {/if}
    </div>

    <!-- Close Button -->
    <button type="button" class="overlay-close-btn" on:click={onClose} title={language === "fr" ? "Fermer" : "Close"}>
      <X size={18} />
    </button>
  </div>

  <!-- Center Stage: The Glowing AGI Orb -->
  <div class="center-stage">
    <VoiceOrb size={140} showStateLabel={true} {language} interactive={true} />

    <!-- Live Transcripts -->
    <div class="transcripts-container">
      {#if $liveTranscript}
        <div class="transcript-bubble user-bubble animate-fade-in">
          <span class="bubble-speaker">{language === "fr" ? "Vous" : "You"}</span>
          <p>{$liveTranscript}</p>
        </div>
      {/if}

      {#if $assistantTranscript}
        <div class="transcript-bubble assistant-bubble animate-fade-in">
          <span class="bubble-speaker">ARO</span>
          <p>{$assistantTranscript}</p>
        </div>
      {/if}

      {#if !$liveTranscript && !$assistantTranscript}
        <p class="voice-instruction">
          {language === "fr"
            ? "Parlez naturellement. L'intelligence artificielle ARO vous écoute et analyse votre requête."
            : "Speak naturally. ARO autonomous intelligence is listening and ready to assist."}
        </p>
      {/if}
    </div>
  </div>

  <!-- Bottom Floating Actions -->
  <div class="overlay-bottom-bar">
    <button
      type="button"
      class="control-circle-btn"
      class:active={$voiceState === "listening"}
      on:click={toggleMic}
      title={$voiceState === "listening" ? "Mettre en pause le micro" : "Activer le micro"}
    >
      {#if $voiceState === "listening"}
        <Mic size={22} />
      {:else}
        <MicOff size={22} />
      {/if}
    </button>

    {#if $voiceState === "speaking"}
      <button
        type="button"
        class="interrupt-btn animate-fade-in"
        on:click={stopSpeaking}
      >
        <span>{language === "fr" ? "Interrompre" : "Interrupt"}</span>
      </button>
    {/if}
  </div>
</div>

<style>
  .voice-live-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: radial-gradient(circle at center, rgba(30, 20, 50, 0.85), rgba(10, 10, 14, 0.95));
    backdrop-filter: blur(40px) saturate(180%);
    z-index: 9999;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    padding: 24px 32px;
    color: #ffffff;
  }

  .overlay-topbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    position: relative;
  }

  .provider-pill {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 14px;
    border-radius: 20px;
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.12);
    font-size: 12px;
    font-weight: 500;
  }

  .sparkle-icon {
    color: #bf5af2;
  }

  .voice-selector-wrapper {
    position: relative;
  }

  .voice-picker-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 14px;
    border-radius: 20px;
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.12);
    color: #ffffff;
    font-size: 12px;
    cursor: pointer;
    transition: all 0.15s;
  }

  .voice-picker-btn:hover {
    background: rgba(255, 255, 255, 0.14);
  }

  .voice-dropdown {
    position: absolute;
    top: 100%;
    right: 0;
    margin-top: 8px;
    background: #1e1e24;
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 12px;
    padding: 6px;
    width: 220px;
    max-height: 260px;
    overflow-y: auto;
    box-shadow: 0 16px 32px rgba(0, 0, 0, 0.5);
    z-index: 100;
  }

  .dropdown-voice-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    width: 100%;
    padding: 8px 10px;
    background: transparent;
    border: none;
    color: #f4f4f5;
    font-size: 12px;
    border-radius: 6px;
    cursor: pointer;
    text-align: left;
  }

  .dropdown-voice-item:hover {
    background: rgba(255, 255, 255, 0.08);
  }

  .dropdown-voice-item.selected {
    background: rgba(191, 90, 242, 0.2);
    color: #bf5af2;
    font-weight: 600;
  }

  .v-lang {
    font-size: 10px;
    color: #8e8e93;
    text-transform: uppercase;
  }

  .overlay-close-btn {
    width: 36px;
    height: 36px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.08);
    border: none;
    color: #a1a1aa;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.15s;
  }

  .overlay-close-btn:hover {
    background: rgba(255, 255, 255, 0.15);
    color: #ffffff;
  }

  .center-stage {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    flex: 1;
  }

  .transcripts-container {
    max-width: 640px;
    width: 100%;
    margin-top: 32px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    max-height: 220px;
    overflow-y: auto;
  }

  .transcript-bubble {
    padding: 14px 18px;
    border-radius: 16px;
    font-size: 14px;
    line-height: 1.5;
  }

  .transcript-bubble p {
    margin: 4px 0 0 0;
  }

  .bubble-speaker {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .user-bubble {
    background: rgba(0, 113, 227, 0.2);
    border: 1px solid rgba(0, 113, 227, 0.35);
    color: #e0f2fe;
    align-self: flex-end;
  }

  .assistant-bubble {
    background: rgba(191, 90, 242, 0.2);
    border: 1px solid rgba(191, 90, 242, 0.35);
    color: #fae8ff;
    align-self: flex-start;
  }

  .voice-instruction {
    text-align: center;
    color: #a1a1aa;
    font-size: 14px;
    margin: 0;
  }

  .overlay-bottom-bar {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 16px;
    padding-bottom: 24px;
  }

  .control-circle-btn {
    width: 64px;
    height: 64px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.1);
    border: 1px solid rgba(255, 255, 255, 0.15);
    color: #ffffff;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: all 0.2s cubic-bezier(0.34, 1.56, 0.64, 1);
  }

  .control-circle-btn:hover {
    transform: scale(1.08);
    background: rgba(255, 255, 255, 0.18);
  }

  .control-circle-btn.active {
    background: #0071e3;
    box-shadow: 0 0 24px rgba(0, 113, 227, 0.6);
  }

  .interrupt-btn {
    padding: 10px 20px;
    border-radius: 20px;
    background: rgba(255, 69, 58, 0.2);
    border: 1px solid rgba(255, 69, 58, 0.4);
    color: #ff453a;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.15s;
  }

  .interrupt-btn:hover {
    background: rgba(255, 69, 58, 0.35);
  }
</style>
