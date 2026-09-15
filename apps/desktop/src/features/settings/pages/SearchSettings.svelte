<script lang="ts">
  import CustomSelect from "../../../lib/CustomSelect.svelte";
  import type { AppSettings } from "../../../lib/types";
  import Key from "@lucide/svelte/icons/key";
  import Link from "@lucide/svelte/icons/link";
  import Globe from "@lucide/svelte/icons/globe";
  import { clearSearchProviderApiKey, setSearchProviderApiKey } from "../../../lib/api/transport";

  type MaybeAsync = void | Promise<void>;
  export let settingsDraft: AppSettings | null | undefined;
  export let language: "fr" | "en";
  export let onAutosave: () => MaybeAsync;
  let credentialDraft = "";
  let credentialError = "";

  // Search Engine Options
  const searchEngineOptions = [
    { value: "google-scrape", label: "Google (Scraper - Recommandé)" },
    { value: "duckduckgo", label: "DuckDuckGo (Scraper)" },
    { value: "brave-api", label: "Brave Search API" },
    { value: "serper", label: "Serper Google Search API" },
    { value: "searxng", label: "SearXNG (Custom Instance)" }
  ];

  // If search settings are not initialized yet, initialize them reactively
  $: if (settingsDraft && !settingsDraft.search) {
    settingsDraft.search = {
      provider: "google-scrape",
      apiKey: "",
      authConfigured: false,
      endpoint: ""
    };
  }

  function handleAutosave() {
    onAutosave();
  }

  async function saveCredential() {
    if (!settingsDraft?.search || !credentialDraft.trim()) return;
    credentialError = "";
    try {
      const updated = await setSearchProviderApiKey(
        settingsDraft.search.provider,
        credentialDraft,
      );
      settingsDraft.search.authConfigured = updated.search.authConfigured;
      credentialDraft = "";
    } catch (error) {
      credentialError = error instanceof Error ? error.message : String(error);
    }
  }

  async function clearCredential() {
    if (!settingsDraft?.search) return;
    credentialError = "";
    try {
      const updated = await clearSearchProviderApiKey(settingsDraft.search.provider);
      settingsDraft.search.authConfigured = updated.search.authConfigured;
      credentialDraft = "";
    } catch (error) {
      credentialError = error instanceof Error ? error.message : String(error);
    }
  }
</script>

{#if settingsDraft && settingsDraft.search}
  <div class="settings-tab-panel">
    <div class="panel-header">
      <h2>{language === "fr" ? "Moteur de Recherche Web" : "Web Search Engine"}</h2>
      <p>{language === "fr" ? "Configurez le moteur de recherche utilisé par ARO pour récupérer des informations fraîches sur le Web." : "Configure the search engine used by ARO to fetch fresh information from the Web."}</p>
    </div>

    <div class="settings-group">
      <div class="settings-row">
        <div class="settings-label-col">
          <span class="settings-title">{language === "fr" ? "Fournisseur de recherche" : "Search Provider"}</span>
          <span class="settings-desc">{language === "fr" ? "Choisissez la méthode de recherche web." : "Choose the web search method."}</span>
        </div>
        <div class="settings-control-col">
          <CustomSelect bind:value={settingsDraft.search.provider} options={searchEngineOptions} on:change={handleAutosave} />
        </div>
      </div>

      {#if settingsDraft.search.provider === "brave-api" || settingsDraft.search.provider === "serper"}
        <div class="settings-row">
          <div class="settings-label-col">
            <span class="settings-title">
              <span style="display: inline-flex; align-items: center; gap: 6px;">
                <Key size={14} />
                <span>{language === "fr" ? "Clé API" : "API Key"}</span>
              </span>
            </span>
            <span class="settings-desc">
              {settingsDraft.search.provider === "brave-api" 
                ? (language === "fr" ? "Entrez votre clé API Brave Search." : "Enter your Brave Search API key.")
                : (language === "fr" ? "Entrez votre clé API Serper.dev." : "Enter your Serper.dev API key.")}
            </span>
          </div>
          <div class="settings-control-col">
            <input
              type="password"
              class="settings-input"
              placeholder={settingsDraft.search.authConfigured ? "•••••••• (configured)" : "api_key_..."}
              bind:value={credentialDraft}
              on:blur={saveCredential}
            />
            {#if settingsDraft.search.authConfigured}
              <button type="button" class="settings-button" on:click={clearCredential}>
                {language === "fr" ? "Supprimer la clé" : "Remove key"}
              </button>
            {/if}
            {#if credentialError}<span role="alert" class="settings-error">{credentialError}</span>{/if}
          </div>
        </div>
      {/if}

      {#if settingsDraft.search.provider === "searxng"}
        <div class="settings-row">
          <div class="settings-label-col">
            <span class="settings-title">
              <span style="display: inline-flex; align-items: center; gap: 6px;">
                <Link size={14} />
                <span>{language === "fr" ? "URL de l'instance" : "Instance URL"}</span>
              </span>
            </span>
            <span class="settings-desc">{language === "fr" ? "URL de votre instance SearXNG." : "URL of your SearXNG instance."}</span>
          </div>
          <div class="settings-control-col">
            <input
              type="url"
              class="settings-input"
              placeholder="https://search.mydomain.com"
              bind:value={settingsDraft.search.endpoint}
              on:blur={handleAutosave}
            />
          </div>
        </div>
      {/if}
    </div>

    <div class="settings-group" style="margin-top: 20px; opacity: 0.85;">
      <div style="display: flex; gap: 10px; align-items: flex-start; padding: 12px; border-radius: 8px; border: 1px solid var(--border-color, rgba(0,0,0,0.1)); background: var(--bg-hover, rgba(0,0,0,0.02));">
        <Globe size={16} style="color: var(--accent-color, #007aff); margin-top: 2px; flex-shrink: 0;" />
        <span style="font-size: 13px; line-height: 1.4;">
          {#if language === "fr"}
            <strong>Note de sécurité :</strong> Vos clés API de recherche sont stockées dans le coffre-fort du système d'exploitation et ne sont jamais synchronisées dans les paramètres ARO.
          {:else}
            <strong>Security note:</strong> Search API keys are stored in the operating-system keyring and are never synchronized in ARO settings.
          {/if}
        </span>
      </div>
    </div>
  </div>
{/if}
