<script lang="ts">
  import Globe from "@lucide/svelte/icons/globe";
  import Shield from "@lucide/svelte/icons/shield";
  import ShieldAlert from "@lucide/svelte/icons/shield-alert";
  import ShieldCheck from "@lucide/svelte/icons/shield-check";
  import Key from "@lucide/svelte/icons/key";
  import History from "@lucide/svelte/icons/history";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import Plus from "@lucide/svelte/icons/plus";
  import Eye from "@lucide/svelte/icons/eye";
  import EyeOff from "@lucide/svelte/icons/eye-off";
  import Search from "@lucide/svelte/icons/search";
  import Cpu from "@lucide/svelte/icons/cpu";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import Check from "@lucide/svelte/icons/check";
  import {
    browserHistory,
    browserCredentials,
    browserPermissions,
    clearBrowserHistory,
    deleteBrowserHistoryEntry,
    addBrowserCredential,
    deleteBrowserCredential,
    updateBrowserPermissions,
    type BrowserCredential,
  } from "../../browser/browser-store";

  export let language: "fr" | "en" = "fr";
  export let writeLocked: boolean = false;

  let historySearch = "";
  let showAddCredModal = false;
  let newCredDomain = "";
  let newCredUsername = "";
  let newCredPassword = "";
  let visiblePasswordIds = new Set<string>();
  let clearedHistoryNotice = false;

  $: filteredHistory = $browserHistory.filter((item) => {
    if (!historySearch.trim()) return true;
    const q = historySearch.toLowerCase();
    return item.title.toLowerCase().includes(q) || item.url.toLowerCase().includes(q);
  });

  function togglePasswordVisibility(id: string) {
    if (visiblePasswordIds.has(id)) {
      visiblePasswordIds.delete(id);
    } else {
      visiblePasswordIds.add(id);
    }
    visiblePasswordIds = new Set(visiblePasswordIds);
  }

  function handleAddCredential() {
    if (!newCredDomain.trim() || !newCredUsername.trim() || !newCredPassword.trim()) return;
    addBrowserCredential({
      domain: newCredDomain.trim().replace(/^https?:\/\//i, "").replace(/\/.*$/, ""),
      username: newCredUsername.trim(),
      password: newCredPassword,
    });
    newCredDomain = "";
    newCredUsername = "";
    newCredPassword = "";
    showAddCredModal = false;
  }

  function handleClearHistory() {
    if (confirm(language === "fr" ? "Êtes-vous sûr de vouloir effacer tout l'historique de navigation ?" : "Are you sure you want to clear all browsing history?")) {
      clearBrowserHistory();
      clearedHistoryNotice = true;
      setTimeout(() => (clearedHistoryNotice = false), 2500);
    }
  }

  function formatDate(isoStr: string): string {
    try {
      const d = new Date(isoStr);
      return d.toLocaleDateString(language === "fr" ? "fr-FR" : "en-US", {
        month: "short",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      });
    } catch {
      return isoStr;
    }
  }
</script>

<div class="settings-tab-panel browser-settings animate-fade-in">
  <!-- Header -->
  <div class="panel-header">
    <h2>{language === "fr" ? "Navigateur Web" : "Web Browser"}</h2>
    <p>
      {language === "fr"
        ? "Configurez l'intégration du navigateur web, les autorisations de recherche et de navigation, l'historique et les identifiants enregistrés."
        : "Configure integrated web browsing, search permissions, navigation history, and saved credentials."}
    </p>
  </div>

  <!-- SECTION 1: Permissions de Navigation IA -->
  <div class="settings-group">
    <div class="group-title-row">
      <Globe size={16} class="group-icon blue" />
      <h3>{language === "fr" ? "Permissions de Navigation de l'IA" : "AI Web Browsing Permissions"}</h3>
    </div>

    <!-- Browsing Toggle -->
    <div class="settings-row">
      <div class="settings-label-col">
        <span class="settings-title">{language === "fr" ? "Navigation web autonome" : "Autonomous Web Browsing"}</span>
        <span class="settings-desc">
          {language === "fr"
            ? "Les agents peuvent visiter des sites internet, rechercher des données et analyser la documentation en ligne."
            : "Agents can navigate web pages, fetch documentation, and perform real-time research."}
        </span>
      </div>
      <div class="settings-control-col">
        <label class="ios-toggle">
          <input
            type="checkbox"
            checked={$browserPermissions.allowAiBrowsing}
            disabled={writeLocked}
            on:change={(e) => updateBrowserPermissions({ allowAiBrowsing: e.currentTarget.checked })}
          />
          <span class="slider"></span>
        </label>
      </div>
    </div>

    <!-- AI Interaction Toggle -->
    <div class="settings-row">
      <div class="settings-label-col">
        <span class="settings-title">{language === "fr" ? "Interactions avec les pages web" : "Web Page Interactions"}</span>
        <span class="settings-desc">
          {language === "fr"
            ? "Autoriser l'agent à cliquer, faire défiler, remplir des formulaires et extraire les données du DOM."
            : "Allow the agent to click elements, scroll, submit forms, and extract structured page content."}
        </span>
      </div>
      <div class="settings-control-col">
        <label class="ios-toggle">
          <input
            type="checkbox"
            checked={$browserPermissions.allowAiInteraction}
            disabled={writeLocked}
            on:change={(e) => updateBrowserPermissions({ allowAiInteraction: e.currentTarget.checked })}
          />
          <span class="slider"></span>
        </label>
      </div>
    </div>

    <!-- Confirmation Prompt -->
    <div class="settings-row">
      <div class="settings-label-col">
        <span class="settings-title">{language === "fr" ? "Confirmer les actions sensibles" : "Require Confirmation for Sensitive Actions"}</span>
        <span class="settings-desc">
          {language === "fr"
            ? "Demande systématiquement votre validation avant toute action modifiant des fichiers système ou envoyant des formulaires critiques."
            : "Prompt for user confirmation before modifying critical system files or submitting sensitive forms."}
        </span>
      </div>
      <div class="settings-control-col">
        <label class="ios-toggle">
          <input
            type="checkbox"
            checked={$browserPermissions.confirmSensitiveActions}
            disabled={writeLocked}
            on:change={(e) => updateBrowserPermissions({ confirmSensitiveActions: e.currentTarget.checked })}
          />
          <span class="slider"></span>
        </label>
      </div>
    </div>
  </div>

  <!-- SECTION 2: Paramètres du Navigateur Intégré -->
  <div class="settings-group">
    <div class="group-title-row">
      <Globe size={16} class="group-icon blue" />
      <h3>{language === "fr" ? "Paramètres du Navigateur" : "Browser Settings"}</h3>
    </div>

    <!-- Search engine selector -->
    <div class="settings-row">
      <div class="settings-label-col">
        <span class="settings-title">{language === "fr" ? "Moteur de recherche par défaut" : "Default Search Engine"}</span>
        <span class="settings-desc">{language === "fr" ? "Utilisé pour la barre d'adresses et les recherches de l'agent." : "Used for address bar queries and agent research."}</span>
      </div>
      <div class="settings-control-col">
        <select
          class="apple-select"
          value={$browserPermissions.defaultSearchEngine}
          disabled={writeLocked}
          on:change={(e) => updateBrowserPermissions({ defaultSearchEngine: e.currentTarget.value as any })}
        >
          <option value="duckduckgo">DuckDuckGo (Confidentialité)</option>
          <option value="google">Google</option>
          <option value="brave">Brave Search</option>
          <option value="bing">Microsoft Bing</option>
        </select>
      </div>
    </div>

    <!-- JavaScript toggle -->
    <div class="settings-row">
      <div class="settings-label-col">
        <span class="settings-title">{language === "fr" ? "Activer JavaScript" : "Enable JavaScript"}</span>
        <span class="settings-desc">{language === "fr" ? "Nécessaire pour le rendu des applications web modernes." : "Required for modern dynamic web applications."}</span>
      </div>
      <div class="settings-control-col">
        <label class="ios-toggle">
          <input
            type="checkbox"
            checked={$browserPermissions.javascriptEnabled}
            disabled={writeLocked}
            on:change={(e) => updateBrowserPermissions({ javascriptEnabled: e.currentTarget.checked })}
          />
          <span class="slider"></span>
        </label>
      </div>
    </div>

    <!-- Cookies toggle -->
    <div class="settings-row">
      <div class="settings-label-col">
        <span class="settings-title">{language === "fr" ? "Bloquer les cookies tiers et pisteurs" : "Block Third-Party Cookies & Trackers"}</span>
        <span class="settings-desc">{language === "fr" ? "Renforce la confidentialité lors des sessions de navigation." : "Enhances privacy across automated navigation sessions."}</span>
      </div>
      <div class="settings-control-col">
        <label class="ios-toggle">
          <input
            type="checkbox"
            checked={$browserPermissions.blockThirdPartyCookies}
            disabled={writeLocked}
            on:change={(e) => updateBrowserPermissions({ blockThirdPartyCookies: e.currentTarget.checked })}
          />
          <span class="slider"></span>
        </label>
      </div>
    </div>
  </div>

  <!-- SECTION 3: Mots de passe et Identifiants Enregistrés -->
  <div class="settings-group">
    <div class="group-title-row header-between">
      <div class="flex-row align-center gap-8">
        <Key size={16} class="group-icon orange" />
        <h3>{language === "fr" ? "Mots de Passe & Identifiants Enregistrés" : "Saved Passwords & Credentials"}</h3>
      </div>
      <button
        type="button"
        class="apple-btn primary small"
        disabled={writeLocked}
        on:click={() => (showAddCredModal = !showAddCredModal)}
      >
        <Plus size={13} style="margin-right: 4px;" />
        <span>{language === "fr" ? "Ajouter un identifiant" : "Add Credential"}</span>
      </button>
    </div>

    <!-- Add Credential Inline Form -->
    {#if showAddCredModal}
      <div class="inline-cred-form animate-fade-in">
        <h4>{language === "fr" ? "Enregistrer un nouvel identifiant" : "Save New Credential"}</h4>
        <div class="form-grid">
          <div class="form-field">
            <label for="cred-domain">{language === "fr" ? "Domaine ou Service" : "Domain or Service"}</label>
            <input id="cred-domain" type="text" bind:value={newCredDomain} placeholder="ex: github.com" />
          </div>
          <div class="form-field">
            <label for="cred-username">{language === "fr" ? "Nom d'utilisateur / Email" : "Username / Email"}</label>
            <input id="cred-username" type="text" bind:value={newCredUsername} placeholder="alice@example.com" />
          </div>
          <div class="form-field">
            <label for="cred-password">{language === "fr" ? "Mot de passe" : "Password"}</label>
            <input id="cred-password" type="password" bind:value={newCredPassword} placeholder="••••••••••••" />
          </div>
        </div>
        <div class="form-buttons">
          <button type="button" class="apple-btn secondary small" on:click={() => (showAddCredModal = false)}>
            {language === "fr" ? "Annuler" : "Cancel"}
          </button>
          <button type="button" class="apple-btn primary small" on:click={handleAddCredential}>
            {language === "fr" ? "Enregistrer" : "Save"}
          </button>
        </div>
      </div>
    {/if}

    <!-- Credentials List -->
    {#if $browserCredentials.length === 0}
      <div class="empty-placeholder">
        <Key size={24} style="color: #86868b; margin-bottom: 8px;" />
        <p>{language === "fr" ? "Aucun identifiant enregistré pour le navigateur." : "No saved credentials yet."}</p>
      </div>
    {:else}
      <div class="credentials-list">
        {#each $browserCredentials as cred}
          <div class="credential-row">
            <div class="cred-left">
              <div class="cred-avatar">
                <Globe size={16} />
              </div>
              <div class="cred-meta">
                <span class="cred-domain">{cred.domain}</span>
                <span class="cred-username">{cred.username}</span>
              </div>
            </div>
            <div class="cred-right">
              <div class="cred-pwd-wrap">
                <span class="cred-password">
                  {visiblePasswordIds.has(cred.id) ? cred.password : "••••••••••••"}
                </span>
                <button
                  type="button"
                  class="icon-btn"
                  title={visiblePasswordIds.has(cred.id) ? "Masquer" : "Afficher"}
                  on:click={() => togglePasswordVisibility(cred.id)}
                >
                  {#if visiblePasswordIds.has(cred.id)}
                    <EyeOff size={13} />
                  {:else}
                    <Eye size={13} />
                  {/if}
                </button>
              </div>
              <button
                type="button"
                class="icon-btn danger"
                title={language === "fr" ? "Supprimer" : "Delete"}
                disabled={writeLocked}
                on:click={() => deleteBrowserCredential(cred.id)}
              >
                <Trash2 size={13} />
              </button>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>

  <!-- SECTION 4: Historique de Navigation -->
  <div class="settings-group">
    <div class="group-title-row header-between">
      <div class="flex-row align-center gap-8">
        <History size={16} class="group-icon green" />
        <h3>{language === "fr" ? "Historique de Navigation" : "Browsing History"}</h3>
      </div>
      {#if $browserHistory.length > 0}
        <button
          type="button"
          class="apple-btn danger-subtle small"
          disabled={writeLocked}
          on:click={handleClearHistory}
        >
          <Trash2 size={12} style="margin-right: 4px;" />
          <span>{language === "fr" ? "Effacer l'historique" : "Clear History"}</span>
        </button>
      {/if}
    </div>

    <!-- Search in history -->
    <div class="history-search-row">
      <div class="search-input-wrap">
        <Search size={14} class="search-icon" />
        <input
          type="text"
          bind:value={historySearch}
          placeholder={language === "fr" ? "Rechercher dans l'historique..." : "Search history..."}
        />
      </div>
    </div>

    {#if clearedHistoryNotice}
      <div class="notice-banner animate-fade-in">
        <Check size={14} />
        <span>{language === "fr" ? "Historique effacé avec succès." : "History cleared successfully."}</span>
      </div>
    {/if}

    <!-- History List -->
    {#if filteredHistory.length === 0}
      <div class="empty-placeholder">
        <History size={24} style="color: #86868b; margin-bottom: 8px;" />
        <p>{language === "fr" ? "Aucune page visitée dans l'historique." : "No browsing history found."}</p>
      </div>
    {:else}
      <div class="history-list">
        {#each filteredHistory as item}
          <div class="history-row">
            <div class="hist-left">
              <Globe size={14} class="hist-icon" />
              <div class="hist-info">
                <span class="hist-title">{item.title}</span>
                <span class="hist-url">{item.url}</span>
              </div>
            </div>
            <div class="hist-right">
              <span class="hist-time">{formatDate(item.visitedAt)}</span>
              <button
                type="button"
                class="icon-btn"
                title={language === "fr" ? "Supprimer de l'historique" : "Remove from history"}
                on:click={() => deleteBrowserHistoryEntry(item.id)}
              >
                <Trash2 size={13} />
              </button>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .browser-settings {
    max-width: 820px;
  }

  .panel-header {
    margin-bottom: 24px;
  }

  .panel-header h2 {
    margin: 0 0 6px 0;
    font-size: 19px;
    font-weight: 700;
  }

  .panel-header p {
    margin: 0;
    font-size: 13px;
    color: #8e8e93;
    line-height: 1.4;
  }

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

  .group-title-row.header-between {
    justify-content: space-between;
  }

  .group-title-row h3 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }

  .group-icon {
    flex-shrink: 0;
  }

  .group-icon.purple { color: #af52de; }
  .group-icon.blue { color: #0071e3; }
  .group-icon.orange { color: #ff9500; }
  .group-icon.green { color: #30d158; }

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

  .settings-row.highlight-box {
    background: rgba(175, 82, 222, 0.08);
    border: 1px solid rgba(175, 82, 222, 0.2);
    border-radius: 10px;
    padding: 14px 16px;
    margin-bottom: 12px;
  }

  .title-with-badge {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 4px;
  }

  .security-badge {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 10px;
    font-weight: 600;
    padding: 2px 6px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.08);
    color: #8e8e93;
  }

  .security-badge.active {
    background: rgba(255, 149, 0, 0.2);
    color: #ff9f0a;
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

  .apple-select {
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.12);
    color: #ffffff;
    border-radius: 6px;
    padding: 6px 10px;
    font-size: 12px;
    outline: none;
  }

  /* Credentials */
  .credentials-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .credential-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 14px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 8px;
  }

  .cred-left {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .cred-avatar {
    width: 32px;
    height: 32px;
    border-radius: 8px;
    background: rgba(0, 113, 227, 0.15);
    color: #0071e3;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .cred-meta {
    display: flex;
    flex-direction: column;
  }

  .cred-domain {
    font-size: 13px;
    font-weight: 600;
  }

  .cred-username {
    font-size: 11px;
    color: #8e8e93;
  }

  .cred-right {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .cred-pwd-wrap {
    display: flex;
    align-items: center;
    gap: 6px;
    background: rgba(0, 0, 0, 0.25);
    padding: 4px 8px;
    border-radius: 6px;
    border: 1px solid rgba(255, 255, 255, 0.06);
  }

  .cred-password {
    font-family: monospace;
    font-size: 12px;
    letter-spacing: 1px;
  }

  .inline-cred-form {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    padding: 14px;
    margin-bottom: 14px;
  }

  .inline-cred-form h4 {
    margin: 0 0 12px 0;
    font-size: 13px;
  }

  .form-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: 12px;
    margin-bottom: 12px;
  }

  .form-field label {
    display: block;
    font-size: 11px;
    color: #8e8e93;
    margin-bottom: 4px;
  }

  .form-field input {
    width: 100%;
    box-sizing: border-box;
    background: rgba(0, 0, 0, 0.3);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #ffffff;
    border-radius: 6px;
    padding: 6px 8px;
    font-size: 12px;
    outline: none;
  }

  .form-buttons {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  /* History */
  .history-search-row {
    margin-bottom: 12px;
  }

  .search-input-wrap {
    display: flex;
    align-items: center;
    background: rgba(0, 0, 0, 0.25);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 6px;
    padding: 6px 10px;
    gap: 8px;
  }

  .search-icon {
    color: #8e8e93;
  }

  .search-input-wrap input {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    color: #ffffff;
    font-size: 12px;
  }

  .history-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-height: 280px;
    overflow-y: auto;
  }

  .history-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 10px;
    background: rgba(255, 255, 255, 0.02);
    border-radius: 6px;
    transition: background 0.15s;
  }

  .history-row:hover {
    background: rgba(255, 255, 255, 0.05);
  }

  .hist-left {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: 1;
    min-width: 0;
  }

  .hist-icon {
    color: #8e8e93;
    flex-shrink: 0;
  }

  .hist-info {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .hist-title {
    font-size: 12px;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .hist-url {
    font-size: 10px;
    color: #71717a;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .hist-right {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-shrink: 0;
  }

  .hist-time {
    font-size: 10px;
    color: #8e8e93;
  }

  .empty-placeholder {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 24px;
    color: #8e8e93;
    font-size: 12px;
  }

  .notice-banner {
    display: flex;
    align-items: center;
    gap: 6px;
    background: rgba(48, 209, 88, 0.15);
    border: 1px solid rgba(48, 209, 88, 0.3);
    color: #30d158;
    padding: 8px 12px;
    border-radius: 6px;
    font-size: 12px;
    margin-bottom: 12px;
  }

  .icon-btn {
    background: transparent;
    border: none;
    color: #8e8e93;
    cursor: pointer;
    padding: 4px;
    border-radius: 4px;
    display: flex;
    align-items: center;
    transition: all 0.15s;
  }

  .icon-btn:hover {
    color: #ffffff;
    background: rgba(255, 255, 255, 0.08);
  }

  .icon-btn.danger:hover {
    color: #ff453a;
    background: rgba(255, 69, 58, 0.15);
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

  .apple-btn.danger-subtle {
    background: rgba(255, 69, 58, 0.12);
    color: #ff453a;
    border: 1px solid rgba(255, 69, 58, 0.2);
  }

  .apple-btn.danger-subtle:hover {
    background: rgba(255, 69, 58, 0.2);
  }

  .flex-row {
    display: flex;
  }

  .align-center {
    align-items: center;
  }

  .gap-8 {
    gap: 8px;
  }
</style>
