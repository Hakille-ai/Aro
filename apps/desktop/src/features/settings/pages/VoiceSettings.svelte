<script lang="ts">
  import Check from "@lucide/svelte/icons/check";
  import Circle from "@lucide/svelte/icons/circle";
  import Edit2 from "@lucide/svelte/icons/edit-2";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import Volume2 from "@lucide/svelte/icons/volume-2";
  import CustomSelect from "../../../lib/CustomSelect.svelte";
  import type { AppSettings, RuntimeStatus, VoiceProfile } from "../../../lib/types";
  type MaybeAsync = void | Promise<void>;
  type VoiceIssueStatus = { issues?: { message: string }[] } | null | undefined;
  export let settingsDraft: AppSettings;
  export let language: "fr" | "en";
  export let theme: "light" | "dark";
  export let labels: Record<string, string>;
  export let runtime: RuntimeStatus | null;
  export let modelRuntimeReady: boolean;
  export let modelReadinessLabel: string;
  export let voiceSpeechToTextReady: boolean;
  export let voiceSpeechToTextStatus: VoiceIssueStatus;
  export let speechReadinessLabel: string;
  export let voiceWakeModelReady: boolean;
  export let wakeWordEnabled: boolean;
  export let wakeWordReadinessLabel: string;
  export let voiceTextToSpeechReady: boolean;
  export let voiceTextToSpeechStatus: VoiceIssueStatus;
  export let ttsReadinessLabel: string;
  export let voiceReadinessIssue: string;
  export let wakeWordDraftEnabled: boolean;
  export let sttOptions: Array<{ value: any; label: string }>;
  export let ttsOptions: Array<{ value: any; label: string }>;
  export let voices: VoiceProfile[];
  export let selectedVoiceId: string;
  export let writeLocked: boolean;
  export let firstVoiceIssue: (status: VoiceIssueStatus) => string;
  export let formatTime: (value: string) => string;
  export let writeDisabledTitle: (action?: string) => string | null | undefined;
  export let onAutosave: () => MaybeAsync;
  export let onOpenCreateVoice: () => MaybeAsync;
  export let onOpenEditVoice: (voice: VoiceProfile) => MaybeAsync;
  export let onDeleteVoice: (voiceId: string) => MaybeAsync;
  export let onSelectVoice: (voiceId: string) => MaybeAsync;
</script>

<div class="settings-tab-panel">
                <div class="panel-header">
                  <h2>{labels.voiceTabTitle}</h2>
                  <p>{labels.voiceTabDesc}</p>
                </div>

                <div class="voice-runtime-panel">
                  <div class="voice-readiness-strip settings-readiness" aria-label={language === "fr" ? "Etat runtime voix" : "Voice runtime status"}>
                    <span class:ready={modelRuntimeReady} class:warning={!modelRuntimeReady} class="voice-readiness-pill" title={runtime?.detail ?? modelReadinessLabel}>
                      {#if modelRuntimeReady}<Check size={12} />{:else}<Circle size={12} />{/if}
                      {modelReadinessLabel}
                    </span>
                    <span class:ready={voiceSpeechToTextReady} class:warning={!voiceSpeechToTextReady} class="voice-readiness-pill" title={firstVoiceIssue(voiceSpeechToTextStatus) || speechReadinessLabel}>
                      {speechReadinessLabel}
                    </span>
                    <span class:ready={voiceWakeModelReady} class:inactive={!wakeWordEnabled} class:warning={wakeWordEnabled && !voiceWakeModelReady} class="voice-readiness-pill" title={wakeWordReadinessLabel}>
                      {wakeWordReadinessLabel}
                    </span>
                    <span class:ready={voiceTextToSpeechReady} class:inactive={!settingsDraft?.speakResponses} class:warning={settingsDraft?.speakResponses && !voiceTextToSpeechReady} class="voice-readiness-pill" title={firstVoiceIssue(voiceTextToSpeechStatus) || ttsReadinessLabel}>
                      {ttsReadinessLabel}
                    </span>
                  </div>
                  {#if voiceReadinessIssue}
                    <p>{voiceReadinessIssue}</p>
                  {:else if runtime?.checkedAt}
                    <p>{language === "fr" ? "Derniere verification" : "Last checked"}: {formatTime(runtime.checkedAt)}</p>
                  {/if}
                </div>

                <div class="settings-group">
                  <div class="settings-row">
                    <div class="settings-label-col">
                      <span class="settings-title">{labels.speakResponsesTitle}</span>
                      <span class="settings-desc">{labels.speakResponsesDesc}</span>
                    </div>
                    <div class="settings-control-col">
                      <label class="ios-toggle">
                        <input type="checkbox" bind:checked={settingsDraft.speakResponses} disabled={writeLocked} on:change={onAutosave} />
                        <span class="slider"></span>
                      </label>
                    </div>
                  </div>

                  <div class="settings-row">
                    <div class="settings-label-col">
                      <span class="settings-title">{labels.voiceActivationTitle}</span>
                      <span class="settings-desc">{labels.voiceActivationDesc}</span>
                    </div>
                    <div class="settings-control-col">
                      <label class="ios-toggle">
                        <input type="checkbox" bind:checked={settingsDraft.voice.enabled} disabled={writeLocked} on:change={onAutosave} />
                        <span class="slider"></span>
                      </label>
                    </div>
                  </div>

                  <div class="settings-row" style="opacity: {settingsDraft?.voice.enabled ? '1' : '0.5'}; transition: opacity 0.2s ease;">
                    <div class="settings-label-col">
                      <span class="settings-title">{labels.wakeWordTitle}</span>
                      <span class="settings-desc">{labels.wakeWordDesc}</span>
                    </div>
                    <div class="settings-control-col">
                      <label class="ios-toggle">
                        <input type="checkbox" bind:checked={wakeWordDraftEnabled} disabled={writeLocked || !settingsDraft?.voice.enabled} on:change={onAutosave} />
                        <span class="slider"></span>
                      </label>
                    </div>
                  </div>
                </div>

                <div class="settings-group">
                  <div class="settings-row">
                    <div class="settings-label-col">
                      <span class="settings-title">{labels.sttEngineTitle}</span>
                      <span class="settings-desc">{labels.sttEngineDesc}</span>
                    </div>
                    <div class="settings-control-col">
                      <CustomSelect bind:value={settingsDraft.voice.speechToText} options={sttOptions} on:change={onAutosave} />
                    </div>
                  </div>

                  <div class="settings-row">
                    <div class="settings-label-col">
                      <span class="settings-title">{labels.ttsEngineTitle}</span>
                      <span class="settings-desc">{labels.ttsEngineDesc}</span>
                    </div>
                    <div class="settings-control-col">
                      <CustomSelect bind:value={settingsDraft.voice.textToSpeech} options={ttsOptions} on:change={onAutosave} />
                    </div>
                  </div>
                </div>

                <!-- Custom Voices Section -->
                <div class="panel-header" style="display: flex; justify-content: space-between; align-items: center; width: 100%; margin-top: 24px; padding-top: 16px; border-top: 1px solid {theme === 'dark' ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.08)'};">
                  <div>
                    <h3 style="margin: 0; font-size: 15px; font-weight: 600; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'};">{language === "fr" ? "Voix disponibles" : "Available Voices"}</h3>
                    <p style="margin: 4px 0 0; font-size: 11px; color: #86868b;">{language === "fr" ? "Gérez et sélectionnez la voix locale active pour la synthèse vocale." : "Manage and select the active local voice for text-to-speech."}</p>
                  </div>
                  <button
                    type="button"
                    class="apple-btn primary"
                    style="font-size: 12px; font-weight: 500; height: 28px; padding: 0 12px; border-radius: 6px;"
                    disabled={writeLocked}
                    title={writeDisabledTitle("creer une voix") ?? (language === "fr" ? "Ajouter" : "Add")}
                    on:click={onOpenCreateVoice}
                  >
                    <Plus size={14} style="margin-right: 4px;" />
                    <span>{language === "fr" ? "Ajouter" : "Add"}</span>
                  </button>
                </div>

                <div class="voices-list" style="display: flex; flex-direction: column; gap: 10px; margin-top: 12px;">
                  {#each voices as voice}
                    <div
                      class="personality-card"
                      class:active={selectedVoiceId === voice.id}
                      style="display: flex; align-items: center; justify-content: space-between; padding: 12px 14px; border-radius: 10px; background: {theme === 'dark' ? '#242426' : '#ffffff'}; border: 1px solid {selectedVoiceId === voice.id ? '#0071e3' : (theme === 'dark' ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.08)')}; transition: all 0.2s;"
                    >
                      <div class="flex-row align-center gap-12" style="flex: 1; min-width: 0;">
                        <div
                          class="voice-avatar"
                          style="width: 36px; height: 36px; border-radius: 50%; background: {voice.avatarColor}; display: flex; align-items: center; justify-content: center; color: #ffffff; flex-shrink: 0; box-shadow: 0 1px 4px rgba(0,0,0,0.1);"
                        >
                          <Volume2 size={18} />
                        </div>
                        <div class="voice-info" style="min-width: 0; flex: 1;">
                          <div class="flex-row align-center gap-6">
                            <h4 style="margin: 0; font-size: 13px; font-weight: 600; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'};">{voice.name}</h4>
                            <span style="font-size: 9px; text-transform: uppercase; background: {voice.language === 'fr' ? 'rgba(59,139,219,0.15)' : 'rgba(52,168,83,0.15)'}; padding: 1px 4px; border-radius: 3px; color: {voice.language === 'fr' ? '#3B8BDB' : '#34A853'}; font-weight: 600;">{voice.language}</span>
                            {#if voice.isDefault}
                              <span style="font-size: 9px; background: {theme === 'dark' ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.05)'}; padding: 1px 4px; border-radius: 3px; color: #86868b; font-weight: 500;">{language === "fr" ? "Défaut" : "Default"}</span>
                            {:else}
                              <span style="font-size: 9px; background: rgba(0, 113, 227, 0.1); padding: 1px 4px; border-radius: 3px; color: #0071e3; font-weight: 500;">Perso</span>
                            {/if}
                            {#if voice.speakerId !== null && voice.speakerId !== undefined}
                              <span style="font-size: 9px; background: {theme === 'dark' ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.06)'}; padding: 1px 4px; border-radius: 3px; color: #86868b;">ID: {voice.speakerId}</span>
                            {/if}
                          </div>
                          <p style="margin: 2px 0 0; font-size: 11px; color: #86868b; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;" title={voice.path}>{voice.description || voice.path}</p>
                        </div>
                      </div>

                      <div class="flex-row align-center gap-8" style="margin-left: 12px; flex-shrink: 0;">
                        {#if selectedVoiceId !== voice.id}
                          <button
                            type="button"
                            class="apple-btn secondary"
                            style="font-size: 10px; padding: 3px 8px; height: 22px; border-radius: 5px;"
                            disabled={writeLocked}
                            title={writeDisabledTitle("changer la voix active") ?? (language === "fr" ? "Activer" : "Activate")}
                            on:click={() => onSelectVoice(voice.id)}
                          >
                            {language === "fr" ? "Activer" : "Activate"}
                          </button>
                        {:else}
                          <span style="font-size: 11px; color: #0071e3; font-weight: 600; display: flex; align-items: center; gap: 3px; padding: 3px 6px;">
                            <Check size={12} />
                            {language === "fr" ? "Actif" : "Active"}
                          </span>
                        {/if}

                        {#if !voice.isDefault}
                          <button
                            type="button"
                            class="icon-button"
                            style="padding: 3px; border-radius: 5px;"
                            disabled={writeLocked}
                            title={writeDisabledTitle("modifier une voix") ?? (language === "fr" ? "Modifier" : "Edit")}
                            on:click={() => onOpenEditVoice(voice)}
                          >
                            <Edit2 size={12} />
                          </button>
                          <button
                            type="button"
                            class="icon-button danger"
                            style="padding: 3px; border-radius: 5px; color: #ff453a;"
                            disabled={writeLocked}
                            title={writeDisabledTitle("supprimer une voix") ?? (language === "fr" ? "Supprimer" : "Delete")}
                            on:click={() => onDeleteVoice(voice.id)}
                          >
                            <Trash2 size={12} />
                          </button>
                        {/if}
                      </div>
                    </div>
                  {/each}
                </div>
              </div>
