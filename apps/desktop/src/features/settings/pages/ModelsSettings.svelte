<script lang="ts">
  import Activity from "@lucide/svelte/icons/activity";
  import Check from "@lucide/svelte/icons/check";
  import Cpu from "@lucide/svelte/icons/cpu";
  import Key from "@lucide/svelte/icons/key";
  import Lock from "@lucide/svelte/icons/lock";
  import Plus from "@lucide/svelte/icons/plus";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import Search from "@lucide/svelte/icons/search";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import type { AppSettings, ModelOption, ModelProviderConnection, RuntimeStatus } from "../../../lib/types";
  type MaybeAsync = void | Promise<void>;
  export let settingsDraft: AppSettings;
  export let language: "fr" | "en";
  export let labels: Record<string, string>;
  export let currentModelLabel: string;
  export let currentModel: ModelOption | null | undefined;
  export let modelProviders: ModelProviderConnection[];
  export let modelProviderBusy: Record<string, boolean>;
  export let modelProviderStatus: Record<string, RuntimeStatus>;
  export let modelProviderKeyDrafts: Record<string, string>;
  export let modelSearchQuery: string;
  export let filteredModelOptions: ModelOption[];
  export let activeModelKey: string;
  export let writeLocked: boolean;
  export let onOpenAddProvider: () => MaybeAsync;
  export let onTestProvider: (providerId: string) => MaybeAsync;
  export let onRefreshProviderCatalog: (providerId: string) => MaybeAsync;
  export let onSaveProviderKey: (providerId: string) => MaybeAsync;
  export let onClearProviderKey: (providerId: string) => MaybeAsync;
  export let onRemoveProvider: (providerId: string) => MaybeAsync;
  export let onSelectModel: (modelId: string) => MaybeAsync;
  export let onAutosave: () => MaybeAsync;
</script>

<div class="settings-tab-panel">
                <div class="panel-header">
                  <h2>{language === "fr" ? "Models" : "Models"}</h2>
                  <p>{language === "fr" ? "Local par défaut. Les providers privés ne s'activent que quand vous choisissez explicitement leurs modèles." : "Local by default. Private providers run only when you explicitly choose one of their models."}</p>
                </div>

                <div class="models-hero">
                  <div>
                    <span class="models-kicker">{settingsDraft.model.fallbackPolicy === "local-first" ? "Local-first" : settingsDraft.model.fallbackPolicy}</span>
                    <h3>{currentModelLabel}</h3>
                    <p>{currentModel?.local ? (language === "fr" ? "Ce modèle tourne sur votre machine ou votre runtime local." : "This model runs on your machine or local runtime.") : (language === "fr" ? "Les requêtes partent vers le provider sélectionné avec votre clé locale." : "Requests go to the selected provider using your local key.")}</p>
                  </div>
                  <button class="model-add-button" type="button" on:click={() => onOpenAddProvider()} disabled={writeLocked}>
                    <Plus size={15} />
                    <span>{language === "fr" ? "Ajouter" : "Add"}</span>
                  </button>
                </div>

                <div class="models-layout">
                  <section class="models-column">
                    <div class="models-section-header">
                      <span>{language === "fr" ? "Recommended local" : "Recommended local"}</span>
                    </div>
                    {#each modelProviders.filter((provider) => provider.kind === "mock" || provider.kind === "ollama" || provider.kind === "llama-cpp") as provider}
                      <article class="provider-card" class:active={provider.id === settingsDraft.model.activeModelRef?.providerId}>
                        <div class="provider-card-main">
                          <div class="provider-icon"><Cpu size={16} /></div>
                          <div>
                            <h4>{provider.displayName}</h4>
                            <p>{provider.endpoint || "Built in"}</p>
                          </div>
                          <span class="provider-badge local">Local</span>
                        </div>
                        <div class="provider-actions">
                          <button type="button" on:click={() => onTestProvider(provider.id)} disabled={modelProviderBusy[provider.id]}><Activity size={14} /> Test</button>
                          <button type="button" on:click={() => onRefreshProviderCatalog(provider.id)} disabled={modelProviderBusy[provider.id] || writeLocked}><RefreshCw size={14} /> Refresh</button>
                        </div>
                        {#if modelProviderStatus[provider.id]}
                          <p class="provider-status">{modelProviderStatus[provider.id].detail}</p>
                        {/if}
                      </article>
                    {/each}

                    <div class="models-section-header">
                      <span>{language === "fr" ? "Connected providers" : "Connected providers"}</span>
                    </div>
                    {#if modelProviders.filter((provider) => provider.kind !== "mock" && provider.kind !== "ollama" && provider.kind !== "llama-cpp").length === 0}
                      <div class="models-empty">
                        <Key size={18} />
                        <span>{language === "fr" ? "Aucun provider privé connecté." : "No private provider connected."}</span>
                      </div>
                    {/if}
                    {#each modelProviders.filter((provider) => provider.kind !== "mock" && provider.kind !== "ollama" && provider.kind !== "llama-cpp") as provider}
                      <article class="provider-card" class:active={provider.id === settingsDraft.model.activeModelRef?.providerId}>
                        <div class="provider-card-main">
                          <div class="provider-icon private"><Key size={16} /></div>
                          <div>
                            <h4>{provider.displayName}</h4>
                            <p>{provider.endpoint}</p>
                          </div>
                          <span class:ready={provider.authConfigured} class="provider-badge">{provider.authConfigured ? "Ready" : "Needs key"}</span>
                        </div>
                        <div class="provider-key-row">
                          <input
                            type="password"
                            placeholder={provider.authConfigured ? "Key saved locally" : "API key"}
                            bind:value={modelProviderKeyDrafts[provider.id]}
                            disabled={writeLocked}
                            on:keydown={(e) => {
                              if (e.key === "Enter") {
                                e.preventDefault();
                                onSaveProviderKey(provider.id);
                              }
                            }}
                          />
                          <button type="button" on:click={() => onSaveProviderKey(provider.id)} disabled={writeLocked || modelProviderBusy[provider.id] || !modelProviderKeyDrafts[provider.id]?.trim()}><Key size={14} /></button>
                        </div>
                        <div class="provider-actions">
                          <button type="button" on:click={() => onTestProvider(provider.id)} disabled={modelProviderBusy[provider.id]}><Activity size={14} /> Test</button>
                          <button type="button" on:click={() => onRefreshProviderCatalog(provider.id)} disabled={modelProviderBusy[provider.id] || writeLocked}><RefreshCw size={14} /> Refresh</button>
                          {#if provider.authConfigured}
                            <button type="button" on:click={() => onClearProviderKey(provider.id)} disabled={modelProviderBusy[provider.id] || writeLocked}><Lock size={14} /> Clear</button>
                          {/if}
                          <button type="button" on:click={() => onRemoveProvider(provider.id)} disabled={modelProviderBusy[provider.id] || writeLocked}><Trash2 size={14} /></button>
                        </div>
                        {#if modelProviderStatus[provider.id]}
                          <p class="provider-status">{modelProviderStatus[provider.id].detail}</p>
                        {/if}
                      </article>
                    {/each}
                  </section>

                  <section class="models-column model-catalog-panel">
                    <div class="models-section-header catalog">
                      <span>{language === "fr" ? "Model catalog" : "Model catalog"}</span>
                      <div class="model-search-inline">
                        <Search size={14} />
                        <input bind:value={modelSearchQuery} placeholder="Search" />
                      </div>
                    </div>
                    <div class="model-catalog-list">
                      {#each filteredModelOptions as model}
                        <button
                          type="button"
                          class="model-catalog-item"
                          class:active={model.id === activeModelKey}
                          disabled={writeLocked || !model.ready}
                          on:click={() => onSelectModel(model.id)}
                        >
                          <div>
                            <span>{model.label}</span>
                            <small>{model.family || model.providerKind}</small>
                          </div>
                          <span class:local={model.local} class:missing={!model.ready} class="provider-badge">
                            {model.local ? "Local" : model.ready ? "Private API" : "Unavailable"}
                          </span>
                          {#if model.id === activeModelKey}
                            <Check size={14} />
                          {/if}
                        </button>
                      {/each}
                    </div>
                  </section>
                </div>

                <div class="settings-group">
                  <div class="settings-row">
                    <div class="settings-label-col">
                      <span class="settings-title">{language === "fr" ? "Generation defaults" : "Generation defaults"}</span>
                      <span class="settings-desc">{language === "fr" ? "Ces réglages s'appliquent au modèle sélectionné sauf override futur." : "These defaults apply to the selected model unless a future override is set."}</span>
                    </div>
                    <div class="settings-control-col">
                      <span class="provider-badge local">Local-first</span>
                    </div>
                  </div>
                  <div class="settings-row">
                    <div class="settings-label-col">
                      <span class="settings-title">{labels.temperatureTitle} ({settingsDraft.model.temperature})</span>
                      <span class="settings-desc">{labels.temperatureDesc}</span>
                    </div>
                    <div class="settings-control-col slider-control">
                      <input
                        type="range"
                        min="0"
                        max="1.4"
                        step="0.1"
                        bind:value={settingsDraft.model.temperature}
                        disabled={writeLocked}
                        on:change={onAutosave}
                      />
                    </div>
                  </div>

                  <div class="settings-row">
                    <div class="settings-label-col">
                      <span class="settings-title">{labels.maxTokensTitle}</span>
                      <span class="settings-desc">{labels.maxTokensDesc}</span>
                    </div>
                    <div class="settings-control-col">
                      <input type="number" min="64" max="4096" bind:value={settingsDraft.model.maxTokens} disabled={writeLocked} on:change={onAutosave} on:blur={onAutosave} on:keydown={(e) => e.key === 'Enter' && e.currentTarget.blur()} />
                    </div>
                  </div>

                  <div class="settings-row">
                    <div class="settings-label-col">
                      <span class="settings-title">{labels.retainHistoryTitle}</span>
                      <span class="settings-desc">{labels.retainHistoryDesc}</span>
                    </div>
                    <div class="settings-control-col">
                      <label class="ios-toggle">
                        <input type="checkbox" bind:checked={settingsDraft.retainHistory} disabled={writeLocked} on:change={onAutosave} />
                        <span class="slider"></span>
                      </label>
                    </div>
                  </div>
                </div>
              </div>

