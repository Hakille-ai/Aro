<script lang="ts">
  import Activity from "@lucide/svelte/icons/activity";
  import Bot from "@lucide/svelte/icons/bot";
  import Brain from "@lucide/svelte/icons/brain";
  import Building2 from "@lucide/svelte/icons/building-2";
  import Check from "@lucide/svelte/icons/check";
  import Cpu from "@lucide/svelte/icons/cpu";
  import Edit2 from "@lucide/svelte/icons/edit-2";
  import Plus from "@lucide/svelte/icons/plus";
  import Search from "@lucide/svelte/icons/search";
  import Sliders from "@lucide/svelte/icons/sliders";
  import Terminal from "@lucide/svelte/icons/terminal";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import User from "@lucide/svelte/icons/user";
  import type { InstructionPersonality } from "../../../lib/instructions";

  type InstructionLabels = {
    enableCustomPrompt: string;
    previewBehavior: string;
  };

  export let t: InstructionLabels;
  export let currentLanguage: "fr" | "en";
  export let currentTheme: "light" | "dark";
  export let cloudWriteLocked: boolean;
  export let customInstructionsEnabled: boolean;
  export let allPersonalities: InstructionPersonality[];
  export let selectedPersonalityId: string;

  export let cloudWriteDisabledTitle: (action?: string) => string | undefined;
  export let openCreatePersonality: () => void;
  export let saveInstructions: () => void;
  export let selectPersonality: (id: string) => void;
  export let openEditPersonality: (personality: InstructionPersonality) => void;
  export let deletePersonality: (id: string) => void | Promise<void>;
</script>
              <div class="settings-tab-panel">
                <div class="panel-header" style="display: flex; justify-content: space-between; align-items: center; width: 100%;">
                  <div>
                    <h2>{currentLanguage === "fr" ? "Profils ARO" : "ARO Profiles"}</h2>
                    <p>{currentLanguage === "fr" ? "Créez et configurez les profils de réponse qui complètent les instructions ARO." : "Create and configure response profiles that complement ARO instructions."}</p>
                  </div>
                  <button
                    type="button"
                    class="apple-btn primary"
                    style="font-size: 13px; font-weight: 500; height: 32px; padding: 0 16px; border-radius: 8px;"
                    disabled={cloudWriteLocked}
                    title={cloudWriteDisabledTitle("creer une personnalite") ?? (currentLanguage === "fr" ? "Creer" : "Create")}
                    on:click={openCreatePersonality}
                  >
                    <Plus size={16} style="margin-right: 4px;" />
                    <span>{currentLanguage === "fr" ? "Créer" : "Create"}</span>
                  </button>
                </div>

                <!-- Custom instructions toggle -->
                <div class="settings-group" style="margin-top: 12px; margin-bottom: 12px;">
                  <div class="settings-row" style="padding-bottom: 12px;">
                    <div class="settings-label-col">
                      <span class="settings-title">{t.enableCustomPrompt}</span>
                      <span class="settings-desc">{currentLanguage === "fr" ? "Permet d'appliquer les consignes du profil actif." : "Applies the instructions from the active profile."}</span>
                    </div>
                    <div class="settings-control-col">
                      <label class="ios-toggle">
                        <input 
                          type="checkbox" 
                          bind:checked={customInstructionsEnabled} 
                          disabled={cloudWriteLocked}
                          on:change={saveInstructions}
                        />
                        <span class="slider"></span>
                      </label>
                    </div>
                  </div>
                </div>

                {#if customInstructionsEnabled}
                  <div class="personalities-list" style="display: flex; flex-direction: column; gap: 12px; margin-top: 8px;">
                    {#each allPersonalities as pers}
                      <div
                        class="personality-card"
                        class:active={selectedPersonalityId === pers.id}
                        style="display: flex; align-items: center; justify-content: space-between; padding: 14px 16px; border-radius: 12px; background: {currentTheme === 'dark' ? '#242426' : '#ffffff'}; border: 1px solid {selectedPersonalityId === pers.id ? '#0071e3' : (currentTheme === 'dark' ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.08)')}; transition: all 0.2s;"
                      >
                        <div class="flex-row align-center gap-12" style="flex: 1; min-width: 0;">
                          <div
                            class="personality-avatar"
                            style="width: 40px; height: 40px; border-radius: 50%; background: {pers.avatarColor}; display: flex; align-items: center; justify-content: center; color: #ffffff; flex-shrink: 0; box-shadow: 0 2px 8px rgba(0,0,0,0.1);"
                          >
                            {#if pers.icon === "bot"}
                              <Bot size={20} />
                            {:else if pers.icon === "code"}
                              <Terminal size={20} />
                            {:else if pers.icon === "check"}
                              <Check size={20} />
                            {:else if pers.icon === "edit-2"}
                              <Edit2 size={20} />
                            {:else if pers.icon === "brain"}
                              <Brain size={20} />
                            {:else if pers.icon === "cpu"}
                              <Cpu size={20} />
                            {:else if pers.icon === "user"}
                              <User size={20} />
                            {:else if pers.icon === "activity"}
                              <Activity size={20} />
                            {:else if pers.icon === "sliders"}
                              <Sliders size={20} />
                            {:else if pers.icon === "building-2"}
                              <Building2 size={20} />
                            {:else}
                              <Search size={20} />
                            {/if}
                          </div>
                          <div class="personality-info" style="min-width: 0; flex: 1;">
                            <div class="flex-row align-center gap-6">
                              <h4 style="margin: 0; font-size: 14px; font-weight: 600; color: {currentTheme === 'dark' ? '#ffffff' : '#1d1d1f'};">{pers.name}</h4>
                              {#if pers.isDefault}
                                <span style="font-size: 10px; background: {currentTheme === 'dark' ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.05)'}; padding: 2px 6px; border-radius: 4px; color: #86868b; font-weight: 500;">{currentLanguage === "fr" ? "Défaut" : "Default"}</span>
                              {:else}
                                <span style="font-size: 10px; background: rgba(0, 113, 227, 0.1); padding: 2px 6px; border-radius: 4px; color: #0071e3; font-weight: 500;">Perso</span>
                              {/if}
                            </div>
                            <p style="margin: 4px 0 0; font-size: 12px; color: #86868b; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;">{pers.description}</p>
                          </div>
                        </div>

                        <div class="flex-row align-center gap-8" style="margin-left: 16px; flex-shrink: 0;">
                          {#if selectedPersonalityId !== pers.id}
                            <button
                              type="button"
                              class="apple-btn secondary"
                              style="font-size: 11px; padding: 4px 10px; height: 26px; border-radius: 6px;"
                              disabled={cloudWriteLocked}
                              title={cloudWriteDisabledTitle("changer la personnalite active") ?? (currentLanguage === "fr" ? "Activer" : "Activate")}
                              on:click={() => selectPersonality(pers.id)}
                            >
                              {currentLanguage === "fr" ? "Activer" : "Activate"}
                            </button>
                          {:else}
                            <span style="font-size: 12px; color: #0071e3; font-weight: 600; display: flex; align-items: center; gap: 4px; padding: 4px 8px;">
                              <Check size={14} />
                              {currentLanguage === "fr" ? "Actif" : "Active"}
                            </span>
                          {/if}

                          {#if !pers.isDefault}
                            <button
                              type="button"
                              class="icon-button"
                              style="padding: 4px; border-radius: 6px;"
                              disabled={cloudWriteLocked}
                              title={cloudWriteDisabledTitle("modifier une personnalite") ?? (currentLanguage === "fr" ? "Modifier" : "Edit")}
                              on:click={() => openEditPersonality(pers)}
                            >
                              <Edit2 size={14} />
                            </button>
                            <button
                              type="button"
                              class="icon-button danger"
                              style="padding: 4px; border-radius: 6px; color: #ff453a;"
                              disabled={cloudWriteLocked}
                              title={cloudWriteDisabledTitle("supprimer une personnalite") ?? (currentLanguage === "fr" ? "Supprimer" : "Delete")}
                              on:click={() => deletePersonality(pers.id)}
                            >
                              <Trash2 size={14} />
                            </button>
                          {/if}
                        </div>
                      </div>
                    {/each}
                  </div>

                  <!-- Active Personality Prompt Details -->
                  {#if selectedPersonalityId}
                    {@const activePers = allPersonalities.find(p => p.id === selectedPersonalityId) || allPersonalities[0]}
                    <div class="settings-group" style="margin-top: 24px; padding: 16px;">
                      <h3 class="panel-subtitle" style="margin: 0 0 12px; font-size: 14px; font-weight: 600;">
                        {currentLanguage === "fr" ? "Configuration du profil actif" : "Active Profile Details"}
                      </h3>
                      
                      <div style="display: flex; flex-direction: column; gap: 14px;">
                        <div>
                          <span style="font-size: 11px; font-weight: 600; text-transform: uppercase; color: #86868b; display: block; margin-bottom: 4px;">
                            {currentLanguage === "fr" ? "Consignes du profil" : "Profile Instructions"}
                          </span>
                          <div style="font-size: 13px; line-height: 1.5; color: {currentTheme === 'dark' ? '#f5f5f7' : '#1d1d1f'}; padding: 10px 12px; border-radius: 6px; background: {currentTheme === 'dark' ? 'rgba(255,255,255,0.03)' : 'rgba(0,0,0,0.02)'}; border: 1px solid {currentTheme === 'dark' ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.05)'};">
                            {activePers.prompt}
                          </div>
                        </div>

                        <div style="display: flex; justify-content: space-between; align-items: center;">
                          <div>
                            <span style="font-size: 11px; font-weight: 600; text-transform: uppercase; color: #86868b; display: block; margin-bottom: 2px;">
                              {currentLanguage === "fr" ? "Température du modèle" : "Model Temperature"}
                            </span>
                            <span style="font-size: 12px; color: #86868b;">
                              {activePers.temperature} ({activePers.temperature <= 0.3 ? (currentLanguage === "fr" ? "Précis / Déterministe" : "Precise / Deterministic") : activePers.temperature >= 0.8 ? (currentLanguage === "fr" ? "Très créatif" : "Very creative") : (currentLanguage === "fr" ? "Équilibré" : "Balanced")})
                            </span>
                          </div>
                        </div>
                      </div>
                    </div>

                    <!-- Live Behavior Preview Box -->
                    <h3 class="panel-subtitle" style="margin-top: 24px; margin-bottom: 12px;">{t.previewBehavior}</h3>
                    <div class="preview-chat-container">
                      <div class="preview-msg user-msg">
                        <span class="preview-sender">User</span>
                        <p class="preview-text">"Qui es-tu et quelles sont tes consignes ?"</p>
                      </div>
                      <div class="preview-msg bot-msg">
                        <div class="flex-row align-center gap-4">
                          <span class="preview-sender">ARO ({activePers.name})</span>
                          <span class="ai-badge">AI</span>
                        </div>
                        <p class="preview-text font-italic">
                          "Je suis ARO, configuré avec le profil <strong>{activePers.name}</strong>.<br/>
                          Mes consignes de comportement actuelles sont : <em>&ldquo;{activePers.prompt.slice(0, 110)}{activePers.prompt.length > 110 ? '...' : ''}&rdquo;</em>"
                        </p>
                      </div>
                    </div>
                  {/if}
                {/if}
              </div>
