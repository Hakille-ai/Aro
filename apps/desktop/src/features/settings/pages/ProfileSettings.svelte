<script lang="ts">
  import Key from "@lucide/svelte/icons/key";
  import Lock from "@lucide/svelte/icons/lock";
  import LogOut from "@lucide/svelte/icons/log-out";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import type { CloudSessionView } from "../../../lib/types";

  type MaybeAsync = void | Promise<void>;

  interface UserProfile {
    name: string;
    email: string;
    roleTitle: string;
    avatarColor: string;
  }

  interface ApiKeyRecord {
    id: string;
    name: string;
    secret?: string;
    prefix?: string;
    lastUsedAt?: string | null;
    oneTimeSecret?: string;
    createdAt: string;
    visible: boolean;
  }

  export let labels: Record<string, string>;
  export let language: "fr" | "en";
  export let userProfile: UserProfile;
  export let apiKeys: ApiKeyRecord[];
  export let apiKeyNameDraft: string;
  export let cloudSession: CloudSessionView | null;
  export let writeLocked: boolean;
  export let writeDisabledTitle: (action?: string) => string | null | undefined;
  export let ensureWriteAllowed: (action: string) => boolean;
  export let getInitials: (name: string) => string;
  export let onSaveUserProfile: () => MaybeAsync;
  export let onGenerateApiKey: (name: string) => MaybeAsync;
  export let onToggleKeyVisibility: (keyId: string) => MaybeAsync;
  export let onRevokeApiKey: (keyId: string) => MaybeAsync;
  export let onCloseSettings: () => MaybeAsync;
  export let onDisconnectCloud: () => MaybeAsync;
</script>

<div class="settings-tab-panel">
                <div class="panel-header">
                  <h2>{labels.profileTabTitle}</h2>
                  <p>{labels.profileTabDesc}</p>
                </div>

                <!-- User Profile Form Card -->
                <div class="profile-card">
                  <div class="profile-header">
                    <div class="profile-avatar-container" style="background: {userProfile.avatarColor}">
                      <span class="profile-avatar-text">{getInitials(userProfile.name)}</span>
                    </div>
                    <div class="profile-meta">
                      <h3>{userProfile.name}</h3>
                      <p>{userProfile.roleTitle} • {userProfile.email}</p>
                    </div>
                  </div>

                  <div class="settings-group" style="margin-top: 20px;">
                    <div class="option-row flex-column">
                      <label for="prof-name" class="option-label">{labels.profileName}</label>
                      <input 
                        id="prof-name"
                        type="text" 
                        class="settings-input" 
                        bind:value={userProfile.name}
                        disabled={writeLocked}
                        on:input={onSaveUserProfile}
                      />
                    </div>

                    <div class="option-row flex-column">
                      <label for="prof-email" class="option-label">{labels.profileEmail}</label>
                      <input 
                        id="prof-email"
                        type="email" 
                        class="settings-input" 
                        bind:value={userProfile.email}
                        disabled={writeLocked}
                        on:input={onSaveUserProfile}
                      />
                    </div>

                    <div class="option-row flex-column">
                      <label for="prof-role" class="option-label">{labels.profileRole}</label>
                      <input 
                        id="prof-role"
                        type="text" 
                        class="settings-input" 
                        bind:value={userProfile.roleTitle}
                        disabled={writeLocked}
                        on:input={onSaveUserProfile}
                      />
                    </div>

                    <div class="option-row flex-column">
                      <span class="option-label">{labels.avatarColorLabel}</span>
                      <div class="avatar-color-picker">
                        {#each [
                          "linear-gradient(135deg, #0071e3 0%, #00c6ff 100%)",
                          "linear-gradient(135deg, #bf5af2 0%, #ff2d55 100%)",
                          "linear-gradient(135deg, #34c759 0%, #00d2ff 100%)",
                          "linear-gradient(135deg, #ff9500 0%, #ffcc00 100%)",
                          "linear-gradient(135deg, #1c1c1e 0%, #3a3a3c 100%)",
                          "linear-gradient(135deg, #3B8BDB 0%, #0071e3 100%)"
                        ] as grad}
                          <button 
                            type="button"
                            class="color-dot" 
                            style="background: {grad}"
                            class:active={userProfile.avatarColor === grad}
                            aria-label="Choose avatar color"
                            disabled={writeLocked}
                            title={writeDisabledTitle("modifier le profil") ?? "Choose avatar color"}
                            on:click={() => {
                              if (!ensureWriteAllowed("modifier le profil")) return;
                              userProfile.avatarColor = grad;
                              onSaveUserProfile();
                            }}
                          ></button>
                        {/each}
                      </div>
                    </div>
                  </div>
                </div>

                <!-- API Keys Management Card -->
                <div class="api-keys-card">
                  <div class="card-header-stats">
                    <h3 class="panel-subtitle" style="margin-bottom: 4px;">{labels.apiKeysTitle}</h3>
                    <p style="font-size: 11px; color: #86868b; margin-bottom: 15px;">{labels.apiKeysDesc}</p>
                  </div>

                  <!-- Generate API Key Form -->
                  <div class="generate-key-row">
                    <input 
                      type="text" 
                      class="settings-input" 
                      placeholder={labels.keyNamePlaceholder}
                      bind:value={apiKeyNameDraft} 
                      disabled={writeLocked}
                    />
                    <button 
                      type="button"
                      class="apple-btn primary"
                      style="padding: 10px 16px; font-size: 12px; height: 36px; display: flex; align-items: center; gap: 4px;"
                      disabled={writeLocked || !apiKeyNameDraft.trim()}
                      title={writeDisabledTitle("creer une cle API") ?? labels.generateKeyBtn}
                      on:click={() => { onGenerateApiKey(apiKeyNameDraft); apiKeyNameDraft = ""; }}
                    >
                      <Plus size={14} />
                      <span>{labels.generateKeyBtn}</span>
                    </button>
                  </div>

                  <!-- API Keys List -->
                  <div class="keys-list" style="margin-top: 15px;">
                    {#each apiKeys as key}
                      <div class="key-item">
                        <div class="key-info">
                          <span class="key-name">{key.name}</span>
                          <span class="key-date">{key.createdAt}</span>
                        </div>
                        <div class="key-secret-container">
                          {#if key.visible}
                            <code class="key-secret">{key.oneTimeSecret ?? key.prefix ?? "aro_live"}</code>
                          {:else}
                            <code class="key-secret">{key.prefix ?? key.secret?.substring(0, 9) ?? "aro_live"}****************</code>
                          {/if}
                        </div>
                        <div class="key-actions">
                          <button 
                            type="button"
                            class="key-action-btn"
                            title={key.visible ? "Hide" : "Show"}
                            on:click={() => onToggleKeyVisibility(key.id)}
                          >
                            {#if key.visible}
                              <Lock size={12} />
                            {:else}
                              <Key size={12} />
                            {/if}
                          </button>
                          <button 
                            type="button"
                            class="key-action-btn delete"
                            disabled={writeLocked}
                            title={writeDisabledTitle("revoquer une cle API") ?? labels.revokeBtn}
                            on:click={() => onRevokeApiKey(key.id)}
                          >
                            <Trash2 size={12} />
                          </button>
                        </div>
                      </div>
                    {/each}
                  </div>
                </div>

                <!-- Session management (Logout) -->
                {#if cloudSession}
                  <div class="session-management-card" style="margin-top: 20px; padding: 15px; border-radius: 12px; border: 1px solid rgba(255, 59, 48, 0.2); background: rgba(255, 59, 48, 0.03); display: flex; align-items: center; justify-content: space-between;">
                    <div style="display: flex; flex-direction: column; gap: 4px;">
                      <h3 style="font-size: 14px; font-weight: 600; color: #ff3b30; margin: 0;">{language === "fr" ? "Session de travail" : "Work Session"}</h3>
                      <p style="font-size: 12px; color: #86868b; margin: 0;">
                        {language === "fr" ? `Connecté en tant que ${userProfile.email} (${cloudSession.activeOrganization.name})` : `Signed in as ${userProfile.email} (${cloudSession.activeOrganization.name})`}
                      </p>
                    </div>
                    <button
                      type="button"
                      class="apple-btn"
                      style="background: #ff3b30; color: #ffffff; border: none; padding: 8px 14px; font-size: 12px; height: 32px; border-radius: 6px; font-weight: 600; display: flex; align-items: center; gap: 6px; cursor: pointer;"
                      on:click={() => { onCloseSettings(); onDisconnectCloud(); }}
                    >
                      <LogOut size={13} />
                      <span>{language === "fr" ? "Se déconnecter" : "Sign out"}</span>
                    </button>
                  </div>
                {/if}
              </div>
