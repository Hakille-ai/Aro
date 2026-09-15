<script lang="ts">
  import { fade } from "svelte/transition";
  import Activity from "@lucide/svelte/icons/activity";
  import Plus from "@lucide/svelte/icons/plus";
  import Settings from "@lucide/svelte/icons/settings";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import X from "@lucide/svelte/icons/x";
  import Zap from "@lucide/svelte/icons/zap";
  import { HOOK_EVENT_CATALOG, HOOK_PRESETS } from "../../integrations/model";
  import type { HookPreset } from "../../integrations/model";

  type MaybeAsync = void | Promise<void>;
  type HookModalMode = "add" | "edit";

  const [messageSentEvent, taskCompletedEvent, conversationCreatedEvent] = HOOK_EVENT_CATALOG;
  const [slackHookPreset, discordHookPreset] = HOOK_PRESETS;

  interface AroHook {
    id: string;
    cloudId?: string;
    name: string;
    url: string;
    secret?: string;
    events: string[];
    enabled: boolean;
    createdAt: string;
    lastTriggered?: string;
    status?: "idle" | "testing" | "success" | "error";
  }

  export let theme: "light" | "dark";
  export let hooks: AroHook[];
  export let showHooksModal: boolean;
  export let hooksModalMode: HookModalMode;
  export let hookFormName: string;
  export let hookFormUrl: string;
  export let hookFormSecret: string;
  export let hookFormEvents: string[];
  export let onOpenAddHookModal: () => MaybeAsync;
  export let onOpenEditHookModal: (hook: AroHook) => MaybeAsync;
  export let onToggleHookFormEvent: (event: string) => MaybeAsync;
  export let onSaveHook: () => MaybeAsync;
  export let onTestHook: (hookId: string) => MaybeAsync;
  export let onDeleteHook: (hookId: string) => MaybeAsync;
  export let onToggleHook: (hookId: string) => MaybeAsync;

  function applyHookPreset(preset: HookPreset) {
    hookFormName = preset.name;
    hookFormUrl = preset.url;
    hookFormEvents = [...preset.events];
    showHooksModal = true;
  }
</script>

<!-- Hooks / Webhooks Configuration Modal -->
{#if showHooksModal}
              <div
                class="skill-modal-overlay"
                style="position: fixed; inset: 0; background: rgba(0,0,0,0.4); backdrop-filter: blur(12px); display: flex; align-items: center; justify-content: center; z-index: 10000;"
                transition:fade={{ duration: 200 }}
              >
                <div 
                  class="skill-modal"
                  style="width: 480px; max-width: 95vw; background: {theme === 'dark' ? '#1c1c1e' : '#f5f5f7'}; border: 1px solid {theme === 'dark' ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.1)'}; border-radius: 16px; box-shadow: 0 20px 50px rgba(0,0,0,0.35); overflow: hidden; display: flex; flex-direction: column;"
                >
                  <!-- Header -->
                  <div style="display: flex; align-items: center; justify-content: space-between; padding: 18px 24px; border-bottom: 1px solid {theme === 'dark' ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.06)'};">
                    <h3 style="margin: 0; font-size: 16px; font-weight: 600; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'};">
                      {hooksModalMode === 'add' ? 'Ajouter un Webhook' : 'Modifier le Webhook'}
                    </h3>
                    <button
                      type="button"
                      on:click={() => (showHooksModal = false)}
                      style="background: none; border: none; color: #86868b; cursor: pointer; display: flex; align-items: center; justify-content: center; padding: 4px; border-radius: 50%; hover:background: rgba(0,0,0,0.05);"
                    >
                      <X size={16} />
                    </button>
                  </div>

                  <!-- Form Content -->
                  <div style="padding: 24px; display: flex; flex-direction: column; gap: 16px; max-height: 60vh; overflow-y: auto;">
                    <!-- Hook Name -->
                    <label style="display: flex; flex-direction: column; gap: 6px; font-size: 13px; font-weight: 600; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'};">
                      Nom de l'intégration / Webhook
                      <input
                        type="text"
                        bind:value={hookFormName}
                        placeholder="Ex: Alerte Slack Equipe"
                        style="width: 100%; padding: 8px 10px; border-radius: 8px; background: {theme === 'dark' ? '#2c2c2e' : '#ffffff'}; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'}; border: 1px solid {theme === 'dark' ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.12)'}; outline: none; box-sizing: border-box; font-weight: normal; font-size: 13px;"
                      />
                    </label>

                    <!-- Endpoint URL -->
                    <label style="display: flex; flex-direction: column; gap: 6px; font-size: 13px; font-weight: 600; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'};">
                      URL de destination (HTTP POST)
                      <input
                        type="url"
                        bind:value={hookFormUrl}
                        placeholder="https://yourserver.com/webhooks/receiver"
                        style="width: 100%; padding: 8px 10px; border-radius: 8px; background: {theme === 'dark' ? '#2c2c2e' : '#ffffff'}; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'}; border: 1px solid {theme === 'dark' ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.12)'}; outline: none; box-sizing: border-box; font-weight: normal; font-size: 13px;"
                      />
                    </label>

                    <!-- Secret Token (for signature verification) -->
                    <label style="display: flex; flex-direction: column; gap: 6px; font-size: 13px; font-weight: 600; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'};">
                      Clé secrète de signature (Signature Secret - Optionnel)
                      <input
                        type="password"
                        bind:value={hookFormSecret}
                        placeholder="Laisser vide ou saisir une clé secrète"
                        style="width: 100%; padding: 8px 10px; border-radius: 8px; background: {theme === 'dark' ? '#2c2c2e' : '#ffffff'}; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'}; border: 1px solid {theme === 'dark' ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.12)'}; outline: none; box-sizing: border-box; font-weight: normal; font-size: 13px;"
                      />
                    </label>

                    <!-- Event triggers checklist -->
                    <div>
                      <span style="font-size: 13px; font-weight: 600; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'}; display: block; margin-bottom: 8px;">Événements déclencheurs</span>
                      <div style="display: flex; flex-direction: column; gap: 8px;">
                        <button
                          type="button"
                          on:click={() => onToggleHookFormEvent(messageSentEvent.id)}
                          style="background: none; border: 1px solid {hookFormEvents.includes(messageSentEvent.id) ? '#007aff' : (theme === 'dark' ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.1)')}; border-radius: 8px; padding: 10px 14px; text-align: left; cursor: pointer; display: flex; align-items: center; justify-content: space-between; transition: all 0.2s;"
                        >
                          <div>
                            <span style="display: block; font-size: 13px; font-weight: 600; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'};">{messageSentEvent.label}</span>
                            <span style="display: block; font-size: 11px; color: #86868b; margin-top: 2px;">{messageSentEvent.description}</span>
                          </div>
                          <span style="width: 16px; height: 16px; border-radius: 50%; border: 2px solid {hookFormEvents.includes(messageSentEvent.id) ? '#007aff' : '#86868b'}; background: {hookFormEvents.includes(messageSentEvent.id) ? '#007aff' : 'transparent'}; display: flex; align-items: center; justify-content: center; color: white; font-size: 9px; font-weight: bold;">
                            {#if hookFormEvents.includes(messageSentEvent.id)}✓{/if}
                          </span>
                        </button>

                        <button
                          type="button"
                          on:click={() => onToggleHookFormEvent(taskCompletedEvent.id)}
                          style="background: none; border: 1px solid {hookFormEvents.includes(taskCompletedEvent.id) ? '#007aff' : (theme === 'dark' ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.1)')}; border-radius: 8px; padding: 10px 14px; text-align: left; cursor: pointer; display: flex; align-items: center; justify-content: space-between; transition: all 0.2s;"
                        >
                          <div>
                            <span style="display: block; font-size: 13px; font-weight: 600; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'};">{taskCompletedEvent.label}</span>
                            <span style="display: block; font-size: 11px; color: #86868b; margin-top: 2px;">{taskCompletedEvent.description}</span>
                          </div>
                          <span style="width: 16px; height: 16px; border-radius: 50%; border: 2px solid {hookFormEvents.includes(taskCompletedEvent.id) ? '#007aff' : '#86868b'}; background: {hookFormEvents.includes(taskCompletedEvent.id) ? '#007aff' : 'transparent'}; display: flex; align-items: center; justify-content: center; color: white; font-size: 9px; font-weight: bold;">
                            {#if hookFormEvents.includes(taskCompletedEvent.id)}✓{/if}
                          </span>
                        </button>

                        <button
                          type="button"
                          on:click={() => onToggleHookFormEvent(conversationCreatedEvent.id)}
                          style="background: none; border: 1px solid {hookFormEvents.includes(conversationCreatedEvent.id) ? '#007aff' : (theme === 'dark' ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.1)')}; border-radius: 8px; padding: 10px 14px; text-align: left; cursor: pointer; display: flex; align-items: center; justify-content: space-between; transition: all 0.2s;"
                        >
                          <div>
                            <span style="display: block; font-size: 13px; font-weight: 600; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'};">{conversationCreatedEvent.label}</span>
                            <span style="display: block; font-size: 11px; color: #86868b; margin-top: 2px;">{conversationCreatedEvent.description}</span>
                          </div>
                          <span style="width: 16px; height: 16px; border-radius: 50%; border: 2px solid {hookFormEvents.includes(conversationCreatedEvent.id) ? '#007aff' : '#86868b'}; background: {hookFormEvents.includes(conversationCreatedEvent.id) ? '#007aff' : 'transparent'}; display: flex; align-items: center; justify-content: center; color: white; font-size: 9px; font-weight: bold;">
                            {#if hookFormEvents.includes(conversationCreatedEvent.id)}✓{/if}
                          </span>
                        </button>
                      </div>
                    </div>
                  </div>

                  <!-- Footer Buttons -->
                  <div style="display: flex; align-items: center; justify-content: flex-end; gap: 12px; padding: 18px 24px; border-top: 1px solid {theme === 'dark' ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.06)'};">
                    <button
                      type="button"
                      on:click={() => (showHooksModal = false)}
                      style="background: none; border: 1px solid {theme === 'dark' ? 'rgba(255,255,255,0.1)' : 'rgba(0,0,0,0.15)'}; border-radius: 8px; padding: 8px 16px; font-size: 13px; font-weight: 500; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'}; cursor: pointer;"
                    >
                      Annuler
                    </button>
                    <button
                      type="button"
                      on:click={onSaveHook}
                      disabled={!hookFormName.trim() || !hookFormUrl.trim() || hookFormEvents.length === 0}
                      style="background: {!hookFormName.trim() || !hookFormUrl.trim() || hookFormEvents.length === 0 ? '#86868b' : 'linear-gradient(180deg, #2997ff 0%, #0071e3 100%)'}; color: white; border: none; border-radius: 8px; padding: 8px 16px; font-size: 13px; font-weight: 500; cursor: {!hookFormName.trim() || !hookFormUrl.trim() || hookFormEvents.length === 0 ? 'not-allowed' : 'pointer'}; opacity: {!hookFormName.trim() || !hookFormUrl.trim() || hookFormEvents.length === 0 ? 0.5 : 1}; box-shadow: 0 1px 3px rgba(0,0,0,0.15);"
                    >
                      Enregistrer
                    </button>
                  </div>
                </div>
              </div>
            {/if}

<div class="settings-tab-panel">
                <!-- Header -->
                <div class="panel-header" style="display: flex; justify-content: space-between; align-items: flex-start; width: 100%; margin-bottom: 24px;">
                  <div>
                    <h2 style="font-size: 22px; font-weight: 600; letter-spacing: -0.5px; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'};">Hooks & Webhooks</h2>
                    <p style="color: #86868b; font-size: 13px; margin-top: 4px; max-width: 600px; line-height: 1.5;">
                      Configurez des webhooks sortants pour recevoir des notifications en temps réel ou déclencher des automatisations lors d'événements spécifiques de l'assistant.
                    </p>
                  </div>
                  <button
                    class="primary-btn"
                    style="background: linear-gradient(180deg, #2997ff 0%, #0071e3 100%); color: white; border: none; border-radius: 980px; padding: 8px 16px; font-size: 13px; font-weight: 500; cursor: pointer; box-shadow: 0 4px 12px rgba(0, 113, 227, 0.2); display: flex; align-items: center; gap: 6px; white-space: nowrap; flex-shrink: 0; transition: all 0.2s ease;"
                    type="button"
                    on:click={onOpenAddHookModal}
                  >
                    <Plus size={15} />
                    <span>Ajouter un hook</span>
                  </button>
                </div>

                <!-- Presets / Quick Setup -->
                <div style="margin-bottom: 32px;">
                  <h3 style="font-size: 14px; font-weight: 600; color: {theme === 'dark' ? '#f5f5f7' : '#1d1d1f'}; margin-bottom: 12px; letter-spacing: -0.2px;">Modèles de Webhooks Rapides</h3>
                  <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(210px, 1fr)); gap: 12px;">
                    <!-- Slack Preset -->
                    <div style="background: {theme === 'dark' ? 'rgba(255,255,255,0.03)' : 'rgba(0,0,0,0.02)'}; border: 1px solid {theme === 'dark' ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.06)'}; border-radius: 12px; padding: 14px; display: flex; flex-direction: column; justify-content: space-between; min-height: 120px; transition: all 0.2s ease;">
                      <div>
                        <div style="display: flex; align-items: center; gap: 8px; margin-bottom: 6px;">
                          <div style="width: 24px; height: 24px; border-radius: 6px; background: rgba(175, 82, 222, 0.12); display: flex; align-items: center; justify-content: center; color: #af52de;">
                            <Zap size={14} />
                          </div>
                          <span style="font-size: 13px; font-weight: 600; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'};">Flux Slack</span>
                        </div>
                        <p style="font-size: 11px; color: #86868b; line-height: 1.4; margin: 0;">Alertez un canal Slack de vos réussites de tâches ou messages.</p>
                      </div>
                      <button
                        type="button"
                        on:click={() => applyHookPreset(slackHookPreset)}
                        style="width: 100%; border: none; background: rgba(175,82,222,0.1); color: #af52de; font-size: 11px; font-weight: 600; padding: 6px 0; border-radius: 6px; cursor: pointer; transition: all 0.2s ease; margin-top: 10px;"
                      >
                        Configurer Slack
                      </button>
                    </div>

                    <!-- Discord Preset -->
                    <div style="background: {theme === 'dark' ? 'rgba(255,255,255,0.03)' : 'rgba(0,0,0,0.02)'}; border: 1px solid {theme === 'dark' ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.06)'}; border-radius: 12px; padding: 14px; display: flex; flex-direction: column; justify-content: space-between; min-height: 120px; transition: all 0.2s ease;">
                      <div>
                        <div style="display: flex; align-items: center; gap: 8px; margin-bottom: 6px;">
                          <div style="width: 24px; height: 24px; border-radius: 6px; background: rgba(88, 101, 242, 0.12); display: flex; align-items: center; justify-content: center; color: #5865f2;">
                            <Zap size={14} />
                          </div>
                          <span style="font-size: 13px; font-weight: 600; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'};">Serveur Discord</span>
                        </div>
                        <p style="font-size: 11px; color: #86868b; line-height: 1.4; margin: 0;">Postez des notifications automatiques sur vos salons Discord.</p>
                      </div>
                      <button
                        type="button"
                        on:click={() => applyHookPreset(discordHookPreset)}
                        style="width: 100%; border: none; background: rgba(88,101,242,0.1); color: #5865f2; font-size: 11px; font-weight: 600; padding: 6px 0; border-radius: 6px; cursor: pointer; transition: all 0.2s ease; margin-top: 10px;"
                      >
                        Configurer Discord
                      </button>
                    </div>

                    <!-- Custom HTTP POST Preset -->
                    <div style="background: {theme === 'dark' ? 'rgba(255,255,255,0.03)' : 'rgba(0,0,0,0.02)'}; border: 1px solid {theme === 'dark' ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.06)'}; border-radius: 12px; padding: 14px; display: flex; flex-direction: column; justify-content: space-between; min-height: 120px; transition: all 0.2s ease;">
                      <div>
                        <div style="display: flex; align-items: center; gap: 8px; margin-bottom: 6px;">
                          <div style="width: 24px; height: 24px; border-radius: 6px; background: rgba(52, 199, 89, 0.12); display: flex; align-items: center; justify-content: center; color: #34c759;">
                            <Zap size={14} />
                          </div>
                          <span style="font-size: 13px; font-weight: 600; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'};">HTTP POST Générique</span>
                        </div>
                        <p style="font-size: 11px; color: #86868b; line-height: 1.4; margin: 0;">Transmettez des requêtes POST structurées signées avec clé secrète.</p>
                      </div>
                      <button
                        type="button"
                        on:click={onOpenAddHookModal}
                        style="width: 100%; border: none; background: rgba(52,199,89,0.1); color: #34c759; font-size: 11px; font-weight: 600; padding: 6px 0; border-radius: 6px; cursor: pointer; transition: all 0.2s ease; margin-top: 10px;"
                      >
                        Configurer HTTP
                      </button>
                    </div>
                  </div>
                </div>

                <!-- Hooks List -->
                <div>
                  <h3 style="font-size: 14px; font-weight: 600; color: {theme === 'dark' ? '#f5f5f7' : '#1d1d1f'}; margin-bottom: 12px; letter-spacing: -0.2px;">Webhooks Enregistrés</h3>
                  {#if hooks.length === 0}
                    <div style="text-align: center; padding: 48px 24px; background: {theme === 'dark' ? 'rgba(255,255,255,0.02)' : 'rgba(0,0,0,0.01)'}; border: 1px dashed {theme === 'dark' ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.1)'}; border-radius: 16px;">
                      <Zap size={32} style="color: #86868b; margin-bottom: 12px; display: inline-block;" />
                      <p style="font-size: 13px; color: #86868b; margin: 0;">Aucun webhook configuré pour le moment.</p>
                      <button
                        type="button"
                        on:click={onOpenAddHookModal}
                        style="margin-top: 12px; background: none; border: 1px solid #007aff; color: #007aff; border-radius: 980px; padding: 6px 14px; font-size: 12px; font-weight: 500; cursor: pointer;"
                      >
                        Créer un premier webhook
                      </button>
                    </div>
                  {:else}
                    <div style="display: flex; flex-direction: column; gap: 8px;">
                      {#each hooks as hook}
                        <div
                          style="background: {theme === 'dark' ? 'rgba(255,255,255,0.02)' : '#ffffff'}; border: 1px solid {theme === 'dark' ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.08)'}; border-radius: 12px; padding: 14px 18px; display: flex; align-items: center; justify-content: space-between; transition: all 0.2s ease;"
                        >
                          <div style="display: flex; align-items: center; gap: 14px;">
                            <!-- Connection Status -->
                            <div style="position: relative;">
                              <div style="width: 32px; height: 32px; border-radius: 50%; background: {theme === 'dark' ? 'rgba(255,255,255,0.04)' : 'rgba(0,0,0,0.03)'}; display: flex; align-items: center; justify-content: center; color: #86868b;">
                                <Zap size={15} />
                              </div>
                              <span
                                style="position: absolute; bottom: -2px; right: -2px; width: 10px; height: 10px; border-radius: 50%; border: 2px solid {theme === 'dark' ? '#1c1c1e' : '#f5f5f7'}; background: {!hook.enabled ? '#86868b' : hook.status === 'success' ? '#34c759' : hook.status === 'testing' ? '#007aff' : hook.status === 'error' ? '#ff453a' : '#30b0c7'};"
                                class:pulse-loading={hook.status === 'testing'}
                              ></span>
                            </div>

                            <div>
                              <div style="display: flex; align-items: center; gap: 8px; flex-wrap: wrap;">
                                <span style="font-size: 14px; font-weight: 600; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'};">{hook.name}</span>
                                {#each hook.events as event}
                                  <span style="font-size: 9px; text-transform: uppercase; background: {theme === 'dark' ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.04)'}; color: #86868b; padding: 1px 6px; border-radius: 4px; font-weight: 600;">{event}</span>
                                {/each}
                              </div>
                              <div style="font-size: 11px; color: #86868b; margin-top: 2px; max-width: 400px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-family: monospace;">
                                {hook.url}
                              </div>
                              {#if hook.lastTriggered}
                                <div style="font-size: 10px; color: #86868b; margin-top: 2px;">
                                  Dernier déclenchement: {new Date(hook.lastTriggered).toLocaleString()}
                                </div>
                              {/if}
                            </div>
                          </div>

                          <div style="display: flex; align-items: center; gap: 12px;">
                            <!-- Test/Trigger button -->
                            <button
                              type="button"
                              on:click={() => onTestHook(hook.id)}
                              disabled={hook.status === 'testing'}
                              style="background: {theme === 'dark' ? 'rgba(255,255,255,0.05)' : 'rgba(0,0,0,0.04)'}; border: 1px solid {theme === 'dark' ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.1)'}; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'}; border-radius: 6px; padding: 4px 10px; font-size: 11px; font-weight: 600; cursor: pointer; display: flex; align-items: center; gap: 4px;"
                            >
                              <Activity size={12} />
                              <span>{hook.status === 'testing' ? 'Test...' : 'Tester'}</span>
                            </button>

                            <!-- Edit/Delete -->
                            <div style="display: flex; align-items: center; gap: 6px;">
                              <button
                                type="button"
                                on:click={() => onOpenEditHookModal(hook)}
                                style="background: none; border: none; color: #86868b; cursor: pointer; padding: 4px;"
                                title="Modifier"
                              >
                                <Settings size={14} />
                              </button>
                              <button
                                type="button"
                                on:click={() => onDeleteHook(hook.id)}
                                style="background: none; border: none; color: #ff453a; cursor: pointer; padding: 4px;"
                                title="Supprimer"
                              >
                                <Trash2 size={14} />
                              </button>
                            </div>

                            <!-- iOS Style toggle switch -->
                            <label class="ios-switch" style="margin-left: 4px;">
                              <input
                                type="checkbox"
                                checked={hook.enabled}
                                on:change={() => onToggleHook(hook.id)}
                              />
                              <span class="ios-slider"></span>
                            </label>
                          </div>
                        </div>
                      {/each}
                    </div>
                  {/if}
                </div>
              </div>
