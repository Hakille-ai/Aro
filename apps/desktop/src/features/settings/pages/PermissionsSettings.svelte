<script lang="ts">
  import Check from "@lucide/svelte/icons/check";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import Info from "@lucide/svelte/icons/info";
  import Network from "@lucide/svelte/icons/network";
  import Plus from "@lucide/svelte/icons/plus";
  import X from "@lucide/svelte/icons/x";
  import type { PermissionCommandApproval, PermissionProfile } from "../../../lib/types";

  export let t: { permissionsTab: string };
  export let currentLanguage: "fr" | "en";
  export let permissionProfiles: PermissionProfile[];
  export let activePermissionProfileId: string;
  export let permissionsLoading: boolean;
  export let permissionsSaving: boolean;
  export let permissionsStatus: string;
  export let permissionsError: string;
  export let permReadFile: boolean;
  export let permWriteFile: boolean;
  export let permExecuteCommands: boolean;
  export let permCommandApprovalMode: PermissionCommandApproval;
  export let permNetworkAccess: boolean;
  export let permRedactSecrets: boolean;
  export let permAllowedDomains: string[];
  export let permAllowedPaths: string[];
  export let newDomainInput: string;
  export let newPathInput: string;
  export let cloudWriteLocked: boolean;

  export let cloudWriteDisabledTitle: (action?: string) => string | undefined;
  export let savePermissionProfile: () => void | Promise<void>;
  export let selectPermissionProfile: (profileId: string) => void;
  export let addAllowedDomain: () => void | Promise<void>;
  export let removeAllowedDomain: (domain: string) => void | Promise<void>;
  export let addAllowedPath: () => void | Promise<void>;
  export let removeAllowedPath: (path: string) => void | Promise<void>;
</script>
              <div class="settings-tab-panel">
                <div class="panel-header" style="display: flex; align-items: flex-start; justify-content: space-between; gap: 16px;">
                  <div>
                    <h2>{t.permissionsTab}</h2>
                    <p>{currentLanguage === "fr" ? "Controlez ce que l'assistant peut lire, modifier, executer et contacter depuis ce poste." : "Control what the assistant can read, modify, execute, and contact from this device."}</p>
                  </div>
                  <button
                    type="button"
                    class="apple-btn primary"
                    disabled={permissionsSaving || cloudWriteLocked}
                    title={cloudWriteDisabledTitle("modifier les permissions") ?? (currentLanguage === "fr" ? "Synchroniser" : "Sync")}
                    on:click={savePermissionProfile}
                  >
                    {#if permissionsSaving}
                      {currentLanguage === "fr" ? "Sauvegarde..." : "Saving..."}
                    {:else}
                      {currentLanguage === "fr" ? "Sauvegarder" : "Save"}
                    {/if}
                  </button>
                </div>

                {#if permissionsError}
                  <div class="settings-readonly-banner" role="alert" style="border-color: rgba(255, 69, 58, 0.35); background: rgba(255, 69, 58, 0.08); color: #b42318;">
                    <Info size={14} />
                    <span>{permissionsError}</span>
                  </div>
                {:else if permissionsStatus}
                  <div class="settings-readonly-banner" role="status" style="border-color: rgba(52, 199, 89, 0.28); background: rgba(52, 199, 89, 0.08); color: #1f7a3a;">
                    <Check size={14} />
                    <span>{permissionsStatus}</span>
                  </div>
                {/if}

                <div class="settings-group">
                  <div class="settings-row">
                    <div class="settings-label-col">
                      <span class="settings-title">{currentLanguage === "fr" ? "Profil d'autorisations" : "Permission profile"}</span>
                      <span class="settings-desc">{currentLanguage === "fr" ? "Les profils sont stockes dans PostgreSQL et scopes par organisation." : "Profiles are stored in PostgreSQL and scoped to the active organization."}</span>
                    </div>
                    <div class="settings-control-col">
                      {#if permissionProfiles.length > 0}
                        <select
                          class="settings-select"
                          bind:value={activePermissionProfileId}
                          disabled={permissionsLoading}
                          on:change={(event) => selectPermissionProfile(event.currentTarget.value)}
                        >
                          {#each permissionProfiles as profile}
                            <option value={profile.id}>{profile.name}</option>
                          {/each}
                        </select>
                      {:else}
                        <span class="provider-badge">{permissionsLoading ? (currentLanguage === "fr" ? "Chargement" : "Loading") : (currentLanguage === "fr" ? "Nouveau profil" : "New profile")}</span>
                      {/if}
                    </div>
                  </div>
                </div>

                <div class="settings-group">
                  <div class="settings-row">
                    <div class="settings-label-col">
                      <span class="settings-title">{currentLanguage === "fr" ? "Lecture des fichiers" : "Read files"}</span>
                      <span class="settings-desc">{currentLanguage === "fr" ? "Autorise l'assistant a inspecter les fichiers dans les chemins approuves." : "Allow the assistant to inspect files inside trusted roots."}</span>
                    </div>
                    <div class="settings-control-col">
                      <label class="ios-toggle">
                        <input type="checkbox" bind:checked={permReadFile} disabled={cloudWriteLocked || permissionsSaving} on:change={savePermissionProfile} />
                        <span class="slider"></span>
                      </label>
                    </div>
                  </div>

                  <div class="settings-row">
                    <div class="settings-label-col">
                      <span class="settings-title">{currentLanguage === "fr" ? "Ecriture des fichiers" : "Write files"}</span>
                      <span class="settings-desc">{currentLanguage === "fr" ? "Autorise les modifications dans les chemins approuves." : "Allow edits inside trusted roots."}</span>
                    </div>
                    <div class="settings-control-col">
                      <label class="ios-toggle">
                        <input type="checkbox" bind:checked={permWriteFile} disabled={cloudWriteLocked || permissionsSaving} on:change={savePermissionProfile} />
                        <span class="slider"></span>
                      </label>
                    </div>
                  </div>

                  <div class="settings-row">
                    <div class="settings-label-col">
                      <span class="settings-title">{currentLanguage === "fr" ? "Commandes terminal" : "Shell commands"}</span>
                      <span class="settings-desc">{currentLanguage === "fr" ? "Autorise l'assistant a lancer des commandes selon le mode d'approbation choisi." : "Allow the assistant to run commands according to the approval mode."}</span>
                    </div>
                    <div class="settings-control-col">
                      <label class="ios-toggle">
                        <input type="checkbox" bind:checked={permExecuteCommands} disabled={cloudWriteLocked || permissionsSaving} on:change={savePermissionProfile} />
                        <span class="slider"></span>
                      </label>
                    </div>
                  </div>

                  <div class="settings-row">
                    <div class="settings-label-col">
                      <span class="settings-title">{currentLanguage === "fr" ? "Acces reseau" : "Network access"}</span>
                      <span class="settings-desc">{currentLanguage === "fr" ? "Limite les connexions aux domaines autorises." : "Restrict connections to allowed domains."}</span>
                    </div>
                    <div class="settings-control-col">
                      <label class="ios-toggle">
                        <input type="checkbox" bind:checked={permNetworkAccess} disabled={cloudWriteLocked || permissionsSaving} on:change={savePermissionProfile} />
                        <span class="slider"></span>
                      </label>
                    </div>
                  </div>

                  <div class="settings-row">
                    <div class="settings-label-col">
                      <span class="settings-title">{currentLanguage === "fr" ? "Redaction des secrets" : "Redact secrets"}</span>
                      <span class="settings-desc">{currentLanguage === "fr" ? "Masque les tokens et cles sensibles dans les traces agent." : "Hide sensitive tokens and keys in agent traces."}</span>
                    </div>
                    <div class="settings-control-col">
                      <label class="ios-toggle">
                        <input type="checkbox" bind:checked={permRedactSecrets} disabled={cloudWriteLocked || permissionsSaving} on:change={savePermissionProfile} />
                        <span class="slider"></span>
                      </label>
                    </div>
                  </div>

                  <div class="settings-row">
                    <div class="settings-label-col">
                      <span class="settings-title">{currentLanguage === "fr" ? "Approbation des commandes" : "Command approval"}</span>
                      <span class="settings-desc">{currentLanguage === "fr" ? "Definit quand une action shell doit attendre une validation." : "Choose when shell actions require approval."}</span>
                    </div>
                    <div class="settings-control-col">
                      <select class="settings-select" bind:value={permCommandApprovalMode} disabled={cloudWriteLocked || permissionsSaving || !permExecuteCommands} on:change={savePermissionProfile}>
                        <option value="always">{currentLanguage === "fr" ? "Toujours demander" : "Always ask"}</option>
                        <option value="safe-auto">{currentLanguage === "fr" ? "Auto pour actions sures" : "Auto for safe actions"}</option>
                        <option value="never">{currentLanguage === "fr" ? "Ne jamais demander" : "Never ask"}</option>
                      </select>
                    </div>
                  </div>
                </div>

                <div class="settings-group" style="padding-top: 10px;">
                  <div class="permission-row-stacked">
                    <div class="permission-row-header">
                      <span class="settings-title">{currentLanguage === "fr" ? "Chemins approuvés" : "Trusted roots"}</span>
                      <span class="settings-desc">{currentLanguage === "fr" ? "Les lectures et écritures de l'assistant sont limitées à ces dossiers." : "File reads and writes are limited to these folders."}</span>
                    </div>
                    <div class="permission-row-body">
                      <div class="permission-chip-list">
                        {#if permAllowedPaths.length === 0}
                          <span style="font-size: 12px; color: #86868b; padding: 4px 0;">
                            {currentLanguage === "fr" ? "Aucun chemin configuré. L'assistant n'aura aucun accès aux fichiers." : "No paths configured. The assistant will have no file access."}
                          </span>
                        {/if}
                        {#each permAllowedPaths as path}
                          <span class="permission-chip">
                            <FolderOpen size={13} style="flex-shrink: 0;" />
                            <span title={path}>{path}</span>
                            <button type="button" disabled={cloudWriteLocked || permissionsSaving} on:click={() => removeAllowedPath(path)} aria-label="Remove path">
                              <X size={11} />
                            </button>
                          </span>
                        {/each}
                      </div>
                      <div class="permission-add-row">
                        <input class="settings-input" bind:value={newPathInput} disabled={cloudWriteLocked || permissionsSaving} placeholder={currentLanguage === "fr" ? "Ex: C:/Users/Stagiaire/Documents/ARO" : "E.g., C:/Users/Stagiaire/Documents/ARO"} on:keydown={(event) => event.key === "Enter" && addAllowedPath()} />
                        <button type="button" disabled={cloudWriteLocked || permissionsSaving || !newPathInput.trim()} on:click={addAllowedPath} aria-label="Add path">
                          <Plus size={14} />
                        </button>
                      </div>
                    </div>
                  </div>

                  <div class="permission-row-stacked">
                    <div class="permission-row-header">
                      <span class="settings-title">{currentLanguage === "fr" ? "Domaines autorisés" : "Allowed domains"}</span>
                      <span class="settings-desc">{currentLanguage === "fr" ? "L'assistant peut uniquement contacter les domaines explicitement listés ici." : "The assistant may contact only the domains explicitly listed here."}</span>
                    </div>
                    <div class="permission-row-body">
                      <div class="permission-chip-list">
                        {#if permAllowedDomains.length === 0}
                          <span style="font-size: 12px; color: #86868b; padding: 4px 0;">
                            {currentLanguage === "fr" ? "Aucun domaine autorisé. L'assistant n'aura aucun accès réseau." : "No domains allowed. The assistant will have no network access."}
                          </span>
                        {/if}
                        {#each permAllowedDomains as domain}
                          <span class="permission-chip">
                            <Network size={13} style="flex-shrink: 0;" />
                            <span title={domain}>{domain}</span>
                            <button type="button" disabled={cloudWriteLocked || permissionsSaving} on:click={() => removeAllowedDomain(domain)} aria-label="Remove allowed domain">
                              <X size={11} />
                            </button>
                          </span>
                        {/each}
                      </div>
                      <div class="permission-add-row">
                        <input class="settings-input" bind:value={newDomainInput} disabled={cloudWriteLocked || permissionsSaving} placeholder="Ex: facebook.com" on:keydown={(event) => event.key === "Enter" && addAllowedDomain()} />
                        <button type="button" disabled={cloudWriteLocked || permissionsSaving || !newDomainInput.trim()} on:click={addAllowedDomain} aria-label="Allow domain">
                          <Plus size={14} />
                        </button>
                      </div>
                    </div>
                  </div>
                </div>
              </div>

<style>
  .permission-row-stacked {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 16px 0;
  }

  .permission-row-stacked:not(:last-child) {
    border-bottom: 1px solid rgba(0, 0, 0, 0.05);
  }

  .permission-row-header {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .permission-row-header .settings-title {
    font-size: 13px;
    font-weight: 600;
    color: #1d1d1f;
  }

  .permission-row-header .settings-desc {
    font-size: 11px;
    color: #86868b;
    line-height: 1.35;
  }

  .permission-row-body {
    display: flex;
    flex-direction: column;
    gap: 12px;
    background: rgba(0, 0, 0, 0.015);
    border: 1px solid rgba(0, 0, 0, 0.06);
    border-radius: 10px;
    padding: 14px;
  }

  .permission-chip-list {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    width: 100%;
  }

  .permission-chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px;
    border: 1px solid rgba(0, 122, 255, 0.15);
    border-radius: 16px;
    background: rgba(0, 122, 255, 0.05);
    color: #007aff;
    font-size: 12px;
    font-weight: 500;
    transition: all 0.15s ease;
  }

  .permission-chip:hover {
    background: rgba(0, 122, 255, 0.08);
    border-color: rgba(0, 122, 255, 0.25);
  }

  .permission-chip span {
    max-width: 400px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .permission-chip button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 15px;
    height: 15px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: rgba(0, 122, 255, 0.12);
    color: #007aff;
    cursor: pointer;
    transition: all 0.12s ease;
  }

  .permission-chip button:hover {
    background: #007aff;
    color: #ffffff;
  }

  .permission-add-row {
    display: flex;
    gap: 8px;
    max-width: 460px;
    width: 100%;
    margin-top: 2px;
  }

  .permission-add-row input {
    flex: 1;
    height: 30px;
    border: 1px solid rgba(0, 0, 0, 0.15);
    border-radius: 6px;
    padding: 0 10px;
    font-size: 12px;
    background: #ffffff;
    color: #1d1d1f;
    transition: all 0.15s ease;
  }

  .permission-add-row input:focus {
    border-color: #007aff;
    box-shadow: 0 0 0 3px rgba(0, 122, 255, 0.15);
    outline: none;
  }

  .permission-add-row button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    padding: 0;
    border: none;
    border-radius: 6px;
    background: #007aff;
    color: #ffffff;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .permission-add-row button:hover:not(:disabled) {
    background: #0062cc;
  }

  .permission-add-row button:disabled {
    background: rgba(0, 0, 0, 0.05);
    color: #86868b;
    cursor: not-allowed;
  }
</style>
