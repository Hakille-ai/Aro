<script lang="ts">
  import { fade } from "svelte/transition";
  import Plus from "@lucide/svelte/icons/plus";
  import Clock from "@lucide/svelte/icons/clock";
  import Activity from "@lucide/svelte/icons/activity";
  import Settings from "@lucide/svelte/icons/settings";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import X from "@lucide/svelte/icons/x";
  import { SCHEDULER_PRESETS } from "../../integrations/model";
  import type { SchedulerPreset } from "../../integrations/model";

  type MaybeAsync = void | Promise<void>;
  type SchedulerModalMode = "add" | "edit";
  type SchedulerType = "timer" | "cron";

  const [coffeeBreakPreset, weekdayStandupPreset, fridayReviewPreset] = SCHEDULER_PRESETS;

  interface ScheduledTask {
    id: string;
    cloudId?: string;
    name: string;
    prompt: string;
    type: SchedulerType;
    durationMinutes?: number;
    cronExpression?: string;
    enabled: boolean;
    createdAt: string;
    lastRun?: string;
    status: "idle" | "running" | "completed" | "error";
  }

  export let theme: "light" | "dark";
  export let scheduledTasks: ScheduledTask[];
  export let showSchedulerModal: boolean;
  export let schedulerModalMode: SchedulerModalMode;
  export let schedulerFormName: string;
  export let schedulerFormPrompt: string;
  export let schedulerFormType: SchedulerType;
  export let schedulerFormDuration: number;
  export let schedulerFormCron: string;
  export let onOpenAddSchedulerModal: () => MaybeAsync;
  export let onOpenEditSchedulerModal: (task: ScheduledTask) => MaybeAsync;
  export let onSaveTask: () => MaybeAsync;
  export let onTriggerTaskImmediately: (taskId: string) => MaybeAsync;
  export let onDeleteTask: (taskId: string) => MaybeAsync;
  export let onToggleTask: (taskId: string) => MaybeAsync;

  function applySchedulerPreset(preset: SchedulerPreset) {
    schedulerFormName = preset.name;
    schedulerFormPrompt = preset.prompt;
    schedulerFormType = preset.type;
    if (preset.type === "timer" && preset.durationMinutes !== undefined) {
      schedulerFormDuration = preset.durationMinutes;
    }
    if (preset.type === "cron" && preset.cronExpression !== undefined) {
      schedulerFormCron = preset.cronExpression;
    }
    showSchedulerModal = true;
  }
</script>

<!-- Scheduler Task Configuration Modal -->
{#if showSchedulerModal}
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
                      {schedulerModalMode === 'add' ? 'Planifier une tâche' : 'Modifier la tâche planifiée'}
                    </h3>
                    <button
                      type="button"
                      on:click={() => (showSchedulerModal = false)}
                      style="background: none; border: none; color: #86868b; cursor: pointer; display: flex; align-items: center; justify-content: center; padding: 4px; border-radius: 50%; hover:background: rgba(0,0,0,0.05);"
                    >
                      <X size={16} />
                    </button>
                  </div>

                  <!-- Form Content -->
                  <div style="padding: 24px; display: flex; flex-direction: column; gap: 16px; max-height: 60vh; overflow-y: auto;">
                    <!-- Task Name -->
                    <label style="display: flex; flex-direction: column; gap: 6px; font-size: 13px; font-weight: 600; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'};">
                      Nom de la tâche / Automation
                      <input
                        type="text"
                        bind:value={schedulerFormName}
                        placeholder="Ex: standup quotidien, rappel étirement..."
                        style="width: 100%; padding: 8px 10px; border-radius: 8px; background: {theme === 'dark' ? '#2c2c2e' : '#ffffff'}; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'}; border: 1px solid {theme === 'dark' ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.12)'}; outline: none; box-sizing: border-box; font-weight: normal; font-size: 13px;"
                      />
                    </label>

                    <!-- Type Segmented Control -->
                    <div>
                      <span style="font-size: 13px; font-weight: 600; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'}; display: block; margin-bottom: 8px;">Type de déclencheur</span>
                      <div style="display: flex; background: {theme === 'dark' ? '#2c2c2e' : 'rgba(0,0,0,0.05)'}; padding: 2px; border-radius: 8px; width: 100%; box-sizing: border-box;">
                        <button
                          type="button"
                          on:click={() => (schedulerFormType = "timer")}
                          style="flex: 1; border: none; padding: 6px 12px; border-radius: 6px; font-size: 12px; font-weight: 500; cursor: pointer; transition: all 0.2s; background: {schedulerFormType === 'timer' ? (theme === 'dark' ? '#636366' : '#ffffff') : 'transparent'}; color: {schedulerFormType === 'timer' ? (theme === 'dark' ? '#ffffff' : '#1d1d1f') : '#86868b'}; box-shadow: {schedulerFormType === 'timer' ? '0 1px 3px rgba(0,0,0,0.1)' : 'none'};"
                        >
                          Minuteur (Intervalle)
                        </button>
                        <button
                          type="button"
                          on:click={() => (schedulerFormType = "cron")}
                          style="flex: 1; border: none; padding: 6px 12px; border-radius: 6px; font-size: 12px; font-weight: 500; cursor: pointer; transition: all 0.2s; background: {schedulerFormType === 'cron' ? (theme === 'dark' ? '#636366' : '#ffffff') : 'transparent'}; color: {schedulerFormType === 'cron' ? (theme === 'dark' ? '#ffffff' : '#1d1d1f') : '#86868b'}; box-shadow: {schedulerFormType === 'cron' ? '0 1px 3px rgba(0,0,0,0.1)' : 'none'};"
                        >
                          Planification (Cron)
                        </button>
                      </div>
                    </div>

                    <!-- Type Specific Input -->
                    {#if schedulerFormType === 'timer'}
                      <label style="display: flex; flex-direction: column; gap: 6px; font-size: 13px; font-weight: 600; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'};">
                        Intervalle d'exécution (Minutes)
                        <input
                          type="number"
                          bind:value={schedulerFormDuration}
                          min="1"
                          style="width: 100%; padding: 8px 10px; border-radius: 8px; background: {theme === 'dark' ? '#2c2c2e' : '#ffffff'}; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'}; border: 1px solid {theme === 'dark' ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.12)'}; outline: none; box-sizing: border-box; font-weight: normal; font-size: 13px;"
                        />
                      </label>
                    {:else}
                      <label style="display: flex; flex-direction: column; gap: 6px; font-size: 13px; font-weight: 600; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'};">
                        Expression Cron de planification
                        <input
                          type="text"
                          bind:value={schedulerFormCron}
                          placeholder="*/30 * * * * (Toutes les 30 min)"
                          style="width: 100%; padding: 8px 10px; border-radius: 8px; background: {theme === 'dark' ? '#2c2c2e' : '#ffffff'}; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'}; border: 1px solid {theme === 'dark' ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.12)'}; outline: none; box-sizing: border-box; font-weight: normal; font-size: 13px;"
                        />
                        <span style="font-size: 10px; color: #86868b; font-weight: normal;">Format: minuteur heure jour-du-mois mois jour-de-la-semaine</span>
                      </label>
                    {/if}

                    <!-- Action Prompt to execute -->
                    <label style="display: flex; flex-direction: column; gap: 6px; font-size: 13px; font-weight: 600; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'};">
                      Message / Invite à exécuter
                      <textarea
                        bind:value={schedulerFormPrompt}
                        placeholder="Qu'est-ce que l'assistant doit faire ou dire ?"
                        rows="3"
                        style="width: 100%; padding: 8px 10px; border-radius: 8px; background: {theme === 'dark' ? '#2c2c2e' : '#ffffff'}; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'}; border: 1px solid {theme === 'dark' ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.12)'}; outline: none; box-sizing: border-box; font-weight: normal; font-size: 13px; resize: vertical; font-family: inherit;"
                      ></textarea>
                    </label>
                  </div>

                  <!-- Footer Buttons -->
                  <div style="display: flex; align-items: center; justify-content: flex-end; gap: 12px; padding: 18px 24px; border-top: 1px solid {theme === 'dark' ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.06)'};">
                    <button
                      type="button"
                      on:click={() => (showSchedulerModal = false)}
                      style="background: none; border: 1px solid {theme === 'dark' ? 'rgba(255,255,255,0.1)' : 'rgba(0,0,0,0.15)'}; border-radius: 8px; padding: 8px 16px; font-size: 13px; font-weight: 500; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'}; cursor: pointer;"
                    >
                      Annuler
                    </button>
                    <button
                      type="button"
                      on:click={onSaveTask}
                      disabled={!schedulerFormName.trim() || !schedulerFormPrompt.trim()}
                      style="background: {!schedulerFormName.trim() || !schedulerFormPrompt.trim() ? '#86868b' : 'linear-gradient(180deg, #2997ff 0%, #0071e3 100%)'}; color: white; border: none; border-radius: 8px; padding: 8px 16px; font-size: 13px; font-weight: 500; cursor: {!schedulerFormName.trim() || !schedulerFormPrompt.trim() ? 'not-allowed' : 'pointer'}; opacity: {!schedulerFormName.trim() || !schedulerFormPrompt.trim() ? 0.5 : 1}; box-shadow: 0 1px 3px rgba(0,0,0,0.15);"
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
                    <h2 style="font-size: 22px; font-weight: 600; letter-spacing: -0.5px; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'};">Planificateur & Automations</h2>
                    <p style="color: #86868b; font-size: 13px; margin-top: 4px; max-width: 600px; line-height: 1.5;">
                      Automatisez l'exécution de requêtes, d'invites ou de rappels. Planifiez des tâches à exécuter après un délai ou de manière récurrente.
                    </p>
                  </div>
                  <button
                    class="primary-btn"
                    style="background: linear-gradient(180deg, #2997ff 0%, #0071e3 100%); color: white; border: none; border-radius: 980px; padding: 8px 16px; font-size: 13px; font-weight: 500; cursor: pointer; box-shadow: 0 4px 12px rgba(0, 113, 227, 0.2); display: flex; align-items: center; gap: 6px; white-space: nowrap; flex-shrink: 0; transition: all 0.2s ease;"
                    type="button"
                    on:click={onOpenAddSchedulerModal}
                  >
                    <Plus size={15} />
                    <span>Planifier une tâche</span>
                  </button>
                </div>

                <!-- Presets Grid -->
                <div style="margin-bottom: 32px;">
                  <h3 style="font-size: 14px; font-weight: 600; color: {theme === 'dark' ? '#f5f5f7' : '#1d1d1f'}; margin-bottom: 12px; letter-spacing: -0.2px;">Modèles d'Automation</h3>
                  <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(210px, 1fr)); gap: 12px;">
                    <!-- Timer Preset -->
                    <div style="background: {theme === 'dark' ? 'rgba(255,255,255,0.03)' : 'rgba(0,0,0,0.02)'}; border: 1px solid {theme === 'dark' ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.06)'}; border-radius: 12px; padding: 14px; display: flex; flex-direction: column; justify-content: space-between; min-height: 120px; transition: all 0.2s ease;">
                      <div>
                        <div style="display: flex; align-items: center; gap: 8px; margin-bottom: 6px;">
                          <div style="width: 24px; height: 24px; border-radius: 6px; background: rgba(52, 199, 89, 0.12); display: flex; align-items: center; justify-content: center; color: #34c759;">
                            <Clock size={14} />
                          </div>
                          <span style="font-size: 13px; font-weight: 600; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'};">Minuteur Rapide</span>
                        </div>
                        <p style="font-size: 11px; color: #86868b; line-height: 1.4; margin: 0;">Lance une invite de rappel après un délai de 10 minutes.</p>
                      </div>
                      <button
                        type="button"
                        on:click={() => applySchedulerPreset(coffeeBreakPreset)}
                        style="width: 100%; border: none; background: rgba(52,199,89,0.1); color: #34c759; font-size: 11px; font-weight: 600; padding: 6px 0; border-radius: 6px; cursor: pointer; transition: all 0.2s ease; margin-top: 10px;"
                      >
                        Créer Minuteur
                      </button>
                    </div>

                    <!-- Daily Standup -->
                    <div style="background: {theme === 'dark' ? 'rgba(255,255,255,0.03)' : 'rgba(0,0,0,0.02)'}; border: 1px solid {theme === 'dark' ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.06)'}; border-radius: 12px; padding: 14px; display: flex; flex-direction: column; justify-content: space-between; min-height: 120px; transition: all 0.2s ease;">
                      <div>
                        <div style="display: flex; align-items: center; gap: 8px; margin-bottom: 6px;">
                          <div style="width: 24px; height: 24px; border-radius: 6px; background: rgba(0, 122, 255, 0.12); display: flex; align-items: center; justify-content: center; color: #007aff;">
                            <Clock size={14} />
                          </div>
                          <span style="font-size: 13px; font-weight: 600; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'};">Rapport Matinal</span>
                        </div>
                        <p style="font-size: 11px; color: #86868b; line-height: 1.4; margin: 0;">Lance un résumé de votre journée tous les matins à 9h00.</p>
                      </div>
                      <button
                        type="button"
                        on:click={() => applySchedulerPreset(weekdayStandupPreset)}
                        style="width: 100%; border: none; background: rgba(0,122,255,0.1); color: #007aff; font-size: 11px; font-weight: 600; padding: 6px 0; border-radius: 6px; cursor: pointer; transition: all 0.2s ease; margin-top: 10px;"
                      >
                        Configurer Matin
                      </button>
                    </div>

                    <!-- Git Sync Check -->
                    <div style="background: {theme === 'dark' ? 'rgba(255,255,255,0.03)' : 'rgba(0,0,0,0.02)'}; border: 1px solid {theme === 'dark' ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.06)'}; border-radius: 12px; padding: 14px; display: flex; flex-direction: column; justify-content: space-between; min-height: 120px; transition: all 0.2s ease;">
                      <div>
                        <div style="display: flex; align-items: center; gap: 8px; margin-bottom: 6px;">
                          <div style="width: 24px; height: 24px; border-radius: 6px; background: rgba(255, 149, 0, 0.12); display: flex; align-items: center; justify-content: center; color: #ff9500;">
                            <Clock size={14} />
                          </div>
                          <span style="font-size: 13px; font-weight: 600; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'};">Rapport Hebdo</span>
                        </div>
                        <p style="font-size: 11px; color: #86868b; line-height: 1.4; margin: 0;">Fait une synthèse de votre semaine de code tous les vendredis à 17h00.</p>
                      </div>
                      <button
                        type="button"
                        on:click={() => applySchedulerPreset(fridayReviewPreset)}
                        style="width: 100%; border: none; background: rgba(255,149,0,0.1); color: #ff9500; font-size: 11px; font-weight: 600; padding: 6px 0; border-radius: 6px; cursor: pointer; transition: all 0.2s ease; margin-top: 10px;"
                      >
                        Configurer Hebdo
                      </button>
                    </div>
                  </div>
                </div>

                <!-- Configured Tasks List -->
                <div>
                  <h3 style="font-size: 14px; font-weight: 600; color: {theme === 'dark' ? '#f5f5f7' : '#1d1d1f'}; margin-bottom: 12px; letter-spacing: -0.2px;">Automations Planifiées</h3>
                  {#if scheduledTasks.length === 0}
                    <div style="text-align: center; padding: 48px 24px; background: {theme === 'dark' ? 'rgba(255,255,255,0.02)' : 'rgba(0,0,0,0.01)'}; border: 1px dashed {theme === 'dark' ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.1)'}; border-radius: 16px;">
                      <Clock size={32} style="color: #86868b; margin-bottom: 12px; display: inline-block;" />
                      <p style="font-size: 13px; color: #86868b; margin: 0;">Aucune tâche planifiée pour le moment.</p>
                      <button
                        type="button"
                        on:click={onOpenAddSchedulerModal}
                        style="margin-top: 12px; background: none; border: 1px solid #007aff; color: #007aff; border-radius: 980px; padding: 6px 14px; font-size: 12px; font-weight: 500; cursor: pointer;"
                      >
                        Créer une tâche
                      </button>
                    </div>
                  {:else}
                    <div style="display: flex; flex-direction: column; gap: 8px;">
                      {#each scheduledTasks as task}
                        <div
                          style="background: {theme === 'dark' ? 'rgba(255,255,255,0.02)' : '#ffffff'}; border: 1px solid {theme === 'dark' ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.08)'}; border-radius: 12px; padding: 14px 18px; display: flex; align-items: center; justify-content: space-between; transition: all 0.2s ease;"
                        >
                          <div style="display: flex; align-items: center; gap: 14px;">
                            <!-- Task Status icon -->
                            <div style="position: relative;">
                              <div style="width: 32px; height: 32px; border-radius: 50%; background: {theme === 'dark' ? 'rgba(255,255,255,0.04)' : 'rgba(0,0,0,0.03)'}; display: flex; align-items: center; justify-content: center; color: #86868b;">
                                <Clock size={15} />
                              </div>
                              <span
                                style="position: absolute; bottom: -2px; right: -2px; width: 10px; height: 10px; border-radius: 50%; border: 2px solid {theme === 'dark' ? '#1c1c1e' : '#f5f5f7'}; background: {!task.enabled ? '#86868b' : task.status === 'completed' ? '#34c759' : task.status === 'running' ? '#007aff' : '#ff9500'};"
                                class:pulse-loading={task.status === 'running'}
                              ></span>
                            </div>

                            <div>
                              <div style="display: flex; align-items: center; gap: 8px; flex-wrap: wrap;">
                                <span style="font-size: 14px; font-weight: 600; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'};">{task.name}</span>
                                <span style="font-size: 9px; text-transform: uppercase; background: {task.type === 'timer' ? 'rgba(52, 199, 89, 0.12)' : 'rgba(0, 122, 255, 0.12)'}; color: {task.type === 'timer' ? '#34c759' : '#007aff'}; padding: 1px 6px; border-radius: 4px; font-weight: 600;">
                                  {task.type === 'timer' ? 'Minuteur' : 'Récurrente'}
                                </span>
                                <span style="font-size: 11px; color: #86868b;">
                                  {task.type === 'timer' ? `Chaque ${task.durationMinutes} min` : `Cron: ${task.cronExpression}`}
                                </span>
                              </div>
                              <div style="font-size: 12px; color: #86868b; margin-top: 2px; max-width: 450px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-style: italic;">
                                Prompt: "{task.prompt}"
                              </div>
                              {#if task.lastRun}
                                <div style="font-size: 10px; color: #86868b; margin-top: 2px;">
                                  Dernière exécution: {new Date(task.lastRun).toLocaleString()}
                                </div>
                              {/if}
                            </div>
                          </div>

                          <div style="display: flex; align-items: center; gap: 12px;">
                            <!-- Run task button -->
                            <button
                              type="button"
                              on:click={() => onTriggerTaskImmediately(task.id)}
                              disabled={task.status === 'running'}
                              style="background: {theme === 'dark' ? 'rgba(255,255,255,0.05)' : 'rgba(0,0,0,0.04)'}; border: 1px solid {theme === 'dark' ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.1)'}; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'}; border-radius: 6px; padding: 4px 10px; font-size: 11px; font-weight: 600; cursor: pointer; display: flex; align-items: center; gap: 4px;"
                            >
                              <Activity size={12} />
                              <span>{task.status === 'running' ? 'Execution...' : 'Lancer'}</span>
                            </button>

                            <!-- Edit/Delete -->
                            <div style="display: flex; align-items: center; gap: 6px;">
                              <button
                                type="button"
                                on:click={() => onOpenEditSchedulerModal(task)}
                                style="background: none; border: none; color: #86868b; cursor: pointer; padding: 4px;"
                                title="Modifier"
                              >
                                <Settings size={14} />
                              </button>
                              <button
                                type="button"
                                on:click={() => onDeleteTask(task.id)}
                                style="background: none; border: none; color: #ff453a; cursor: pointer; padding: 4px;"
                                title="Supprimer"
                              >
                                <Trash2 size={14} />
                              </button>
                            </div>

                            <!-- iOS Switch -->
                            <label class="ios-switch" style="margin-left: 4px;">
                              <input
                                type="checkbox"
                                checked={task.enabled}
                                on:change={() => onToggleTask(task.id)}
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
