<script lang="ts">
  import Bot from "@lucide/svelte/icons/bot";
  import Check from "@lucide/svelte/icons/check";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import User from "@lucide/svelte/icons/user";
  import type { AppSettings } from "../../../lib/types";
  import { estimateInstructionTokens } from "../../../lib/instructions";
  import type { InstructionPersonality } from "../../../lib/instructions";
  import type { SettingsTab } from "../types";

  type InstructionPresetView = {
    id: string;
    name: string;
    sub: string;
  };

  export let t: { previewBehavior: string };
  export let currentLanguage: "fr" | "en";
  export let currentTheme: "light" | "dark";
  export let activeSettingsTab: SettingsTab;
  export let customSystemPromptsEnabled: boolean;
  export let cloudWriteLocked: boolean;
  export let activeColor: string;
  export let activeColorLight: string;
  export let selectedPromptMode: string;
  export let promptPreviewPersonality: InstructionPersonality | null;
  export let visibleInstructionPresets: InstructionPresetView[];
  export let agiIdentity: Record<string, string>;
  export let agiRules: Record<string, string>;
  export let agiFormatting: Record<string, string>;
  export let compiledPromptPreview: string;
  export let settingsDraft: AppSettings | null;
  export let allPersonalities: InstructionPersonality[];
  export let selectedPersonalityId: string;

  export let saveCustomSystemPrompts: () => void;
  export let cloudWriteDisabledTitle: (action?: string) => string | undefined;
  export let applyAgiPreset: (presetId: string) => void;
  export let saveAgiPrompts: () => void;
  export let resetSelectedPromptToDefault: () => void;
  export let autosaveSettings: () => void | Promise<void>;
  export let compileSystemPrompt: (
    personality: InstructionPersonality | null,
    mode: string,
    userInput?: string,
  ) => string;
</script>
              <div class="settings-tab-panel">
                <div class="panel-header">
                  <h2>{currentLanguage === "fr" ? "Instructions ARO" : "ARO Instructions"}</h2>
                  <p>{currentLanguage === "fr" ? "Configurez l'identité, le ton, les règles de réponse et les modes de l'assistant." : "Configure the assistant identity, tone, response rules, and modes."}</p>
                </div>

                <!-- Custom system prompts toggle -->
                <div class="settings-group" style="margin-bottom: 24px;">
                  <div class="settings-row" style="padding-bottom: 12px; border-bottom: none;">
                    <div class="settings-label-col">
                      <span class="settings-title">{currentLanguage === "fr" ? "Activer les instructions personnalisées" : "Enable custom instructions"}</span>
                      <span class="settings-desc">{currentLanguage === "fr" ? "Utilise vos consignes structurées à la place des valeurs par défaut ARO." : "Use your structured instructions instead of ARO's defaults."}</span>
                    </div>
                    <div class="settings-control-col">
                      <label class="ios-toggle">
                        <input 
                          type="checkbox" 
                          bind:checked={customSystemPromptsEnabled} 
                          disabled={cloudWriteLocked}
                          on:change={saveCustomSystemPrompts}
                        />
                        <span class="slider"></span>
                      </label>
                    </div>
                  </div>
                </div>

                {#if customSystemPromptsEnabled}
                  <div style="display: flex; align-items: center; gap: 14px; padding: 16px 18px; background: {currentTheme === 'dark' ? 'rgba(255,255,255,0.03)' : 'rgba(0,0,0,0.02)'}; border: 1px solid {currentTheme === 'dark' ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.05)'}; border-radius: 12px; margin-bottom: 20px;">
                    <div style="width: 40px; height: 40px; border-radius: 10px; background: {activeColorLight}; color: {activeColor}; display: flex; align-items: center; justify-content: center; flex-shrink: 0;">
                      <Bot size={20} />
                    </div>

                    <div style="flex: 1; min-width: 0;">
                      <div style="font-size: 10px; font-weight: 700; text-transform: uppercase; color: {activeColor}; margin-bottom: 2px;">
                        {currentLanguage === "fr" ? "Profil d'instructions ARO" : "ARO instruction profile"}
                      </div>
                      <h3 style="margin: 0; font-size: 15px; font-weight: 600; color: {currentTheme === 'dark' ? '#ffffff' : '#1d1d1f'};">
                        {currentLanguage === "fr" ? "Instructions actives et synchronisées" : "Instructions active and synced"}
                      </h3>
                      <p style="margin: 4px 0 0; font-size: 12px; color: #86868b;">
                        {currentLanguage === "fr" ? "Mode :" : "Mode:"} <strong style="text-transform: uppercase; color: {activeColor};">{selectedPromptMode}</strong>
                        <span> · </span>
                        {currentLanguage === "fr" ? "Profil :" : "Profile:"} <strong>{promptPreviewPersonality ? promptPreviewPersonality.name : "ARO"}</strong>
                      </p>
                    </div>
                    <button
                      type="button"
                      class="apple-btn secondary"
                      style="height: 30px; padding: 0 12px; font-size: 12px; border-radius: 7px; flex-shrink: 0;"
                      on:click={() => (activeSettingsTab = "instructions")}
                    >
                      <User size={14} style="margin-right: 4px;" />
                      <span>{currentLanguage === "fr" ? "Profils" : "Profiles"}</span>
                    </button>
                  </div>

                  <!-- Apple Segmented Selector for Modes -->
                  <div style="display: flex; background: {currentTheme === 'dark' ? 'rgba(255, 255, 255, 0.05)' : 'rgba(0, 0, 0, 0.05)'}; padding: 3px; border-radius: 9px; margin-bottom: 20px; width: 100%; box-sizing: border-box;">
                    {#each [
                      { id: "chat", label: "Chat" },
                      { id: "think", label: currentLanguage === "fr" ? "Réflexion" : "Think" },
                      { id: "code", label: "Code" },
                      { id: "summarize", label: currentLanguage === "fr" ? "Synthèse" : "Summary" },
                      { id: "quiet", label: currentLanguage === "fr" ? "Minimal" : "Quiet" }
                    ] as modeOption}
                      <button
                        type="button"
                        style="flex: 1; border: none; background: {selectedPromptMode === modeOption.id ? (currentTheme === 'dark' ? '#3a3a3c' : '#ffffff') : 'transparent'}; color: {selectedPromptMode === modeOption.id ? (currentTheme === 'dark' ? '#ffffff' : '#1d1d1f') : '#86868b'}; font-size: 13px; font-weight: 500; padding: 6px 12px; border-radius: 7px; cursor: pointer; transition: all 0.2s; box-shadow: {selectedPromptMode === modeOption.id ? '0 1px 3px rgba(0,0,0,0.1)' : 'none'}; outline: none;"
                        disabled={cloudWriteLocked}
                        title={cloudWriteDisabledTitle("changer de mode de directive") ?? modeOption.label}
                        on:click={() => selectedPromptMode = modeOption.id}
                      >
                        {modeOption.label}
                      </button>
                    {/each}
                  </div>

                  <!-- Presets Selection -->
                  <div style="margin-bottom: 24px;">
                    <span style="font-size: 11px; font-weight: 600; text-transform: uppercase; color: #86868b; display: block; margin-bottom: 8px;">
                      {currentLanguage === "fr" ? "Préréglages ARO" : "ARO presets"}
                    </span>
                    <div style="display: flex; gap: 12px;">
                      {#each visibleInstructionPresets as preset}
                        <button
                          type="button"
                          class="preset-card"
                          style="--accent-color: {activeColor}; --accent-light: {activeColorLight}; --bg-card: {currentTheme === 'dark' ? '#242426' : '#ffffff'}; --border-color: {currentTheme === 'dark' ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.08)'}; --text-color: {currentTheme === 'dark' ? '#ffffff' : '#1d1d1f'};"
                          disabled={cloudWriteLocked}
                          title={cloudWriteDisabledTitle("appliquer un preset de directives") ?? preset.name}
                          on:click={() => applyAgiPreset(preset.id)}
                        >
                          <span class="preset-title">{preset.name}</span>
                          <span class="preset-subtitle">{preset.sub}</span>
                        </button>
                      {/each}
                    </div>
                  </div>

                  <!-- Structured Directive Editor Panel -->
                  <div class="settings-group" style="padding: 20px; display: flex; flex-direction: column; gap: 20px; margin-bottom: 24px;">
                    
                    <!-- Identity Field -->
                    <label style="display: flex; flex-direction: column; gap: 6px; font-size: 13px; font-weight: 600; color: {currentTheme === 'dark' ? '#ffffff' : '#1d1d1f'};">
                      <span>1. {currentLanguage === "fr" ? "Identité et mission" : "Identity and mission"}</span>
                      <textarea
                        style="width: 100%; min-height: 50px; padding: 10px 12px; border-radius: 8px; background: {currentTheme === 'dark' ? '#1c1c1e' : '#f5f5f7'}; color: {currentTheme === 'dark' ? '#f5f5f7' : '#1d1d1f'}; border: 1px solid {currentTheme === 'dark' ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.1)'}; font-family: inherit; font-size: 13px; font-weight: normal; line-height: 1.5; resize: vertical; outline: none; box-sizing: border-box; transition: all 0.2s;"
                        bind:value={agiIdentity[selectedPromptMode]}
                        disabled={cloudWriteLocked}
                        placeholder={currentLanguage === "fr" ? "Définissez le rôle et l'identité d'ARO..." : "Define ARO's role and identity..."}
                        on:blur={saveAgiPrompts}
                        on:change={saveAgiPrompts}
                      ></textarea>
                    </label>

                    <!-- Rules Field -->
                    <label style="display: flex; flex-direction: column; gap: 6px; font-size: 13px; font-weight: 600; color: {currentTheme === 'dark' ? '#ffffff' : '#1d1d1f'};">
                      <span>2. {currentLanguage === "fr" ? "Règles de comportement (une par ligne)" : "Behavior rules (one per line)"}</span>
                      <textarea
                        style="width: 100%; min-height: 85px; padding: 10px 12px; border-radius: 8px; background: {currentTheme === 'dark' ? '#1c1c1e' : '#f5f5f7'}; color: {currentTheme === 'dark' ? '#f5f5f7' : '#1d1d1f'}; border: 1px solid {currentTheme === 'dark' ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.1)'}; font-family: inherit; font-size: 13px; font-weight: normal; line-height: 1.5; resize: vertical; outline: none; box-sizing: border-box; transition: all 0.2s;"
                        bind:value={agiRules[selectedPromptMode]}
                        disabled={cloudWriteLocked}
                        placeholder={currentLanguage === "fr" ? "Entrez les règles de comportement d'ARO..." : "Enter ARO's behavior rules..."}
                        on:blur={saveAgiPrompts}
                        on:change={saveAgiPrompts}
                      ></textarea>
                    </label>

                    <!-- Formatting Field -->
                    <label style="display: flex; flex-direction: column; gap: 6px; font-size: 13px; font-weight: 600; color: {currentTheme === 'dark' ? '#ffffff' : '#1d1d1f'};">
                      <span>3. {currentLanguage === "fr" ? "Format de réponse" : "Response format"}</span>
                      <textarea
                        style="width: 100%; min-height: 50px; padding: 10px 12px; border-radius: 8px; background: {currentTheme === 'dark' ? '#1c1c1e' : '#f5f5f7'}; color: {currentTheme === 'dark' ? '#f5f5f7' : '#1d1d1f'}; border: 1px solid {currentTheme === 'dark' ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.1)'}; font-family: inherit; font-size: 13px; font-weight: normal; line-height: 1.5; resize: vertical; outline: none; box-sizing: border-box; transition: all 0.2s;"
                        bind:value={agiFormatting[selectedPromptMode]}
                        disabled={cloudWriteLocked}
                        placeholder={currentLanguage === "fr" ? "Spécifiez la structure de réponse préférée..." : "Specify the preferred response structure..."}
                        on:blur={saveAgiPrompts}
                        on:change={saveAgiPrompts}
                      ></textarea>
                    </label>

                    <!-- Actions & Stats -->
                    <div style="display: flex; justify-content: space-between; align-items: center; font-size: 12px; color: #86868b; border-top: 1px solid {currentTheme === 'dark' ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.06)'}; padding-top: 12px;">
                      <button
                        type="button"
                        style="background: transparent; border: none; color: #ff453a; font-size: 12px; font-weight: 500; cursor: pointer; display: flex; align-items: center; gap: 4px;"
                        disabled={cloudWriteLocked}
                        title={cloudWriteDisabledTitle("modifier les directives systeme") ?? (currentLanguage === "fr" ? "Réinitialiser" : "Reset")}
                        on:click={resetSelectedPromptToDefault}
                      >
                        <RefreshCw size={12} style="margin-right: 2px;" />
                        <span>{currentLanguage === "fr" ? "Réinitialiser les valeurs ARO" : "Reset to ARO defaults"}</span>
                      </button>
                      <div style="display: flex; align-items: center; gap: 4px; color: #34c759; font-weight: 500;">
                        <Check size={14} />
                        <span>{currentLanguage === "fr" ? "Enregistré automatiquement" : "Auto-saved"}</span>
                      </div>
                    </div>
                  </div>

                  <!-- Live Compiled Prompt Preview -->
                  <div style="margin-bottom: 24px;">
                    <span style="font-size: 11px; font-weight: 600; text-transform: uppercase; color: #86868b; display: block; margin-bottom: 8px;">
                      {currentLanguage === "fr" ? "Aperçu du prompt final" : "Final prompt preview"}
                      <span style="float: right; text-transform: none; font-weight: 500;">~{estimateInstructionTokens(compiledPromptPreview)} tokens</span>
                    </span>
                    <div style="position: relative; border-radius: 12px; overflow: hidden; border: 1px solid {currentTheme === 'dark' ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.08)'}; background: {currentTheme === 'dark' ? '#1c1c1e' : '#f5f5f7'};">
                      <!-- Top header bar of code block -->
                      <div style="display: flex; align-items: center; justify-content: space-between; padding: 8px 14px; background: {currentTheme === 'dark' ? 'rgba(255,255,255,0.02)' : 'rgba(0,0,0,0.02)'}; border-bottom: 1px solid {currentTheme === 'dark' ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.06)'};">
                        <div style="display: flex; gap: 6px;">
                          <span style="width: 8px; height: 8px; border-radius: 50%; background: #ff5f56;"></span>
                          <span style="width: 8px; height: 8px; border-radius: 50%; background: #ffbd2e;"></span>
                          <span style="width: 8px; height: 8px; border-radius: 50%; background: #27c93f;"></span>
                        </div>
                        <span style="font-family: monospace; font-size: 10px; color: #86868b; letter-spacing: 0.5px;">SYSTEM_PROMPT.md</span>
                      </div>
                      <!-- Code Content -->
                      <pre style="margin: 0; padding: 14px; font-family: SFMono-Regular, Consolas, 'Liberation Mono', Menlo, monospace; font-size: 12px; line-height: 1.6; color: {currentTheme === 'dark' ? '#a1a1a6' : '#424245'}; overflow-x: auto; white-space: pre-wrap; word-break: break-all; max-height: 220px; overflow-y: auto;"><code>{compiledPromptPreview}</code></pre>
                    </div>
                  </div>

                  {#if settingsDraft}
                    <!-- Generation settings -->
                    <div class="settings-group" style="padding: 20px; display: flex; flex-direction: column; gap: 16px; margin-bottom: 24px;">
                      <h3 style="margin: 0 0 4px; font-size: 13px; font-weight: 600; color: {currentTheme === 'dark' ? '#ffffff' : '#1d1d1f'}; text-transform: uppercase; letter-spacing: 0.5px; color: #86868b;">
                        {currentLanguage === "fr" ? "Paramètres de génération" : "Generation settings"}
                      </h3>

                      <!-- Temperature Slider -->
                      <div style="display: flex; flex-direction: column; gap: 6px;">
                        <div style="display: flex; justify-content: space-between; align-items: center;">
                          <span style="font-size: 13px; font-weight: 500; color: {currentTheme === 'dark' ? '#ffffff' : '#1d1d1f'};">
                            {currentLanguage === "fr" ? "Température" : "Temperature"}
                          </span>
                          <span style="font-family: monospace; font-size: 12px; font-weight: 600; padding: 2px 6px; border-radius: 4px; background: {activeColorLight}; color: {activeColor};">
                            {settingsDraft.model.temperature} ({
                              settingsDraft.model.temperature <= 0.3 ? (currentLanguage === "fr" ? "Déterministe / Précis" : "Deterministic / Precise") :
                              settingsDraft.model.temperature >= 0.9 ? (currentLanguage === "fr" ? "Créatif / Emergent" : "Lateral / Creative") :
                              (currentLanguage === "fr" ? "Équilibré / Synthétique" : "Balanced / Synthesis")
                            })
                          </span>
                        </div>
                        <div style="display: flex; align-items: center; width: 100%;">
                          <input
                            type="range"
                            min="0"
                            max="1.4"
                            step="0.1"
                            style="flex: 1; accent-color: {activeColor}; cursor: pointer; background: transparent; border: none; padding: 0;"
                            bind:value={settingsDraft.model.temperature}
                            disabled={cloudWriteLocked}
                            on:change={autosaveSettings}
                          />
                        </div>
                      </div>

                      <!-- Max Tokens Input -->
                      <div style="display: flex; justify-content: space-between; align-items: center; border-top: 1px solid {currentTheme === 'dark' ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.06)'}; padding-top: 12px;">
                        <div style="display: flex; flex-direction: column;">
                          <span style="font-size: 13px; font-weight: 500; color: {currentTheme === 'dark' ? '#ffffff' : '#1d1d1f'};">
                            {currentLanguage === "fr" ? "Horizon de génération (Max Tokens)" : "Generation Horizon (Max Tokens)"}
                          </span>
                          <span style="font-size: 11px; color: #86868b;">
                            {currentLanguage === "fr" ? "Limite supérieure des jetons générés par réponse" : "Maximum length constraint per response output"}
                          </span>
                        </div>
                        <input
                          type="number"
                          min="64"
                          max="4096"
                          style="width: 80px; padding: 6px 10px; border-radius: 6px; background: {currentTheme === 'dark' ? '#1c1c1e' : '#ffffff'}; color: {currentTheme === 'dark' ? '#f5f5f7' : '#1d1d1f'}; border: 1px solid {currentTheme === 'dark' ? 'rgba(255,255,255,0.1)' : 'rgba(0,0,0,0.15)'}; font-family: monospace; font-size: 12px; outline: none; text-align: center;"
                          bind:value={settingsDraft.model.maxTokens}
                          disabled={cloudWriteLocked}
                          on:change={autosaveSettings}
                          on:blur={autosaveSettings}
                        />
                      </div>
                    </div>
                  {/if}

                  <!-- Live Behavior Preview Box -->
                  <h3 class="panel-subtitle" style="margin-top: 24px; margin-bottom: 12px;">{t.previewBehavior}</h3>
                  <div class="preview-chat-container">
                    <div class="preview-msg user-msg">
                      <span class="preview-sender">User</span>
                      <p class="preview-text">"Qui es-tu et quelles sont tes consignes ?"</p>
                    </div>
                    <div class="preview-msg bot-msg">
                      <div class="flex-row align-center gap-4">
                        <span class="preview-sender">ARO ({selectedPromptMode})</span>
                        <span class="ai-badge">AI</span>
                      </div>
                      <p class="preview-text font-italic">
                        "Je suis ARO, configuré pour le mode <strong>{selectedPromptMode}</strong>.<br/>
                        Mes consignes de comportement actuelles sont : <em>&ldquo;{(compileSystemPrompt(allPersonalities.find(p => p.id === selectedPersonalityId) || allPersonalities[0], selectedPromptMode) || "").slice(0, 120)}{(compileSystemPrompt(allPersonalities.find(p => p.id === selectedPersonalityId) || allPersonalities[0], selectedPromptMode) || "").length > 120 ? '...' : ''}&rdquo;</em>"
                      </p>
                    </div>
                  </div>
                {/if}
              </div>
