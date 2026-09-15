<script lang="ts">
  import { onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import Check from "@lucide/svelte/icons/check";
  import Key from "@lucide/svelte/icons/key";
  import Link from "@lucide/svelte/icons/link";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import Unplug from "@lucide/svelte/icons/unplug";
  import X from "@lucide/svelte/icons/x";
  import {
    createIntegrationCredential,
    disconnectIntegration,
    getIntegrationAuthorizationAttempt,
    requestIntegrationHealthCheck,
    startIntegrationAuthorization,
  } from "../../lib/api";
  import type {
    ConnectorDefinition,
    IntegrationInstallation,
  } from "../../lib/integrations/types";

  export let currentTheme: "light" | "dark";
  export let currentUserId: string;
  export let catalog: ConnectorDefinition[];
  export let installations: IntegrationInstallation[];
  export let onRefresh: () => void | Promise<void>;

  let selected: ConnectorDefinition | null = null;
  let selectedMode: "manual" | "oauth" = "manual";
  let secretDraft = "";
  let labelDraft = "";
  let capabilityIds: string[] = [];
  let busy = false;
  let error = "";
  let authorizationPolling: AbortController | null = null;

  onDestroy(() => authorizationPolling?.abort());

  $: installationByProvider = new Map(
    installations
      .filter((installation) => installation.lifecycleStatus !== "deleted")
      .map((installation) => [installation.providerId, installation]),
  );

  function openCredentialForm(connector: ConnectorDefinition) {
    selected = connector;
    selectedMode = "manual";
    secretDraft = "";
    labelDraft = connector.displayName;
    capabilityIds = connector.capabilities.map((capability) => capability.id);
    error = "";
  }

  function openAuthorizationForm(connector: ConnectorDefinition) {
    selected = connector;
    selectedMode = "oauth";
    secretDraft = "";
    labelDraft = connector.displayName;
    capabilityIds = connector.capabilities.map((capability) => capability.id);
    error = "";
  }

  function closeCredentialForm() {
    if (!busy) selected = null;
  }

  function toggleCapability(capabilityId: string) {
    capabilityIds = capabilityIds.includes(capabilityId)
      ? capabilityIds.filter((id) => id !== capabilityId)
      : [...capabilityIds, capabilityId];
  }

  async function saveCredential() {
    if (!selected || !secretDraft.trim()) return;
    busy = true;
    error = "";
    try {
      const method = selected.authMethods.includes("personal_access_token")
        ? "personal_access_token"
        : selected.authMethods.includes("service_account")
          ? "service_account"
          : "api_key";
      await createIntegrationCredential(selected.id, {
        owner: { type: "user", userId: currentUserId },
        credentialType: method,
        secret: method === "service_account" ? JSON.parse(secretDraft) : secretDraft,
        label: labelDraft.trim() || selected.displayName,
        publicConfig: {},
        capabilityIds,
      });
      secretDraft = "";
      selected = null;
      await onRefresh();
    } catch (caught) {
      error = caught instanceof Error ? caught.message : String(caught);
    } finally {
      busy = false;
    }
  }

  async function saveAuthorization() {
    if (!selected) return;
    const connector = selected;
    const method = preferredBrowserMethod(connector);
    if (!method) return;
    busy = true;
    error = "";
    authorizationPolling?.abort();
    const controller = new AbortController();
    authorizationPolling = controller;
    try {
      const attempt = await startIntegrationAuthorization(connector.id, {
        owner: { type: "user", userId: currentUserId },
        method,
        capabilityIds,
      });
      selected = null;
      await invoke("open_url", { url: attempt.interaction.url });
      await pollAuthorizationAttempt(attempt.attemptId, attempt.expiresAt, controller.signal);
    } catch (caught) {
      if (!controller.signal.aborted) {
        error = caught instanceof Error ? caught.message : String(caught);
      }
    } finally {
      busy = false;
    }
  }

  async function pollAuthorizationAttempt(attemptId: string, expiresAt: string, signal: AbortSignal) {
    let delayMs = 500;
    const deadline = new Date(expiresAt).getTime();
    while (!signal.aborted && Date.now() < deadline) {
      await abortableDelay(delayMs, signal);
      const attempt = await getIntegrationAuthorizationAttempt(attemptId);
      if (attempt.status === "authorized") {
        await onRefresh();
        return;
      }
      if (["denied", "failed", "expired", "cancelled"].includes(attempt.status)) {
        const labels: Record<string, string> = {
          denied: "Autorisation refusée.",
          failed: "La connexion n’a pas pu être finalisée.",
          expired: "La tentative de connexion a expiré.",
          cancelled: "La tentative de connexion a été annulée.",
        };
        throw new Error(labels[attempt.status] ?? "Connexion interrompue.");
      }
      delayMs = Math.min(Math.round(delayMs * 1.6), 5_000);
    }
    if (!signal.aborted) throw new Error("La tentative de connexion a expiré.");
  }

  function abortableDelay(milliseconds: number, signal: AbortSignal): Promise<void> {
    return new Promise((resolve, reject) => {
      const timer = window.setTimeout(resolve, milliseconds);
      signal.addEventListener("abort", () => {
        window.clearTimeout(timer);
        reject(new DOMException("Polling cancelled", "AbortError"));
      }, { once: true });
    });
  }

  async function runAction(action: "health" | "disconnect", installationId: string) {
    busy = true;
    error = "";
    try {
      if (action === "health") await requestIntegrationHealthCheck(installationId);
      else await disconnectIntegration(installationId);
      await onRefresh();
    } catch (caught) {
      error = caught instanceof Error ? caught.message : String(caught);
    } finally {
      busy = false;
    }
  }

  function statusLabel(installation: IntegrationInstallation | undefined): string {
    if (!installation) return "Déconnecté";
    if (installation.lifecycleStatus === "pending") return "Connexion en cours";
    if (installation.lifecycleStatus === "reauthorization_required") return "Reconnexion requise";
    if (installation.lifecycleStatus === "disconnecting") return "Déconnexion en cours";
    if (installation.lifecycleStatus === "revoked") return "Révoqué";
    if (installation.healthStatus === "provider_unavailable") return "Fournisseur indisponible";
    if (installation.healthStatus === "permissions_insufficient") return "Permissions insuffisantes";
    if (installation.healthStatus === "credentials_expired") return "Expiré";
    if (installation.healthStatus === "degraded" || installation.healthStatus === "sync_error") return "Erreur temporaire";
    return "Connecté";
  }

  function supportsManual(connector: ConnectorDefinition): boolean {
    return connector.authMethods.some((method) =>
      method === "api_key" || method === "personal_access_token" || method === "service_account"
    );
  }

  function preferredBrowserMethod(connector: ConnectorDefinition) {
    if (connector.authMethods.includes("open_id_connect")) return "open_id_connect" as const;
    if (connector.authMethods.includes("app_installation")) return "app_installation" as const;
    if (connector.authMethods.includes("authorization_code_pkce")) return "authorization_code_pkce" as const;
    return null;
  }
</script>

<section class="settings-tab-panel" aria-labelledby="integrations-v3-title">
  <div class="panel-header">
    <h2 id="integrations-v3-title">Intégrations</h2>
    <p>Catalogue certifié et connexions gérées par le serveur.</p>
  </div>

  {#if error}<div role="alert" class="integration-alert">{error}</div>{/if}

  <div class="integration-grid">
    {#each catalog as connector (connector.id)}
      {@const installation = installationByProvider.get(connector.id)}
      <article class:dark={currentTheme === "dark"} class="integration-card">
        <header>
          <div class="integration-icon">{connector.icon ?? "🔌"}</div>
          <div>
            <h3>{connector.displayName}</h3>
            <span class="status" data-status={installation?.lifecycleStatus ?? "disconnected"}>
              {statusLabel(installation)}
            </span>
          </div>
        </header>
        <p>{connector.description}</p>
        {#if installation?.account?.displayName}
          <div class="account"><Check size={14} /> {installation.account.displayName}</div>
        {/if}
        {#if connector.permissions.length}
          <ul aria-label="Permissions">
            {#each connector.permissions as permission}
              <li>{permission.label}{permission.sensitive ? " • sensible" : ""}</li>
            {/each}
          </ul>
        {/if}
        <footer>
          {#if installation && installation.lifecycleStatus === "active"}
            <button type="button" disabled={busy} on:click={() => runAction("health", installation.id)}>
              <RefreshCw size={14} /> Vérifier
            </button>
            <button class="danger" type="button" disabled={busy} on:click={() => runAction("disconnect", installation.id)}>
              <Unplug size={14} /> Déconnecter
            </button>
          {:else if connector.enabled && connector.certificationStatus === "certified"}
            {#if preferredBrowserMethod(connector)}
              <button class="primary" type="button" disabled={busy} on:click={() => openAuthorizationForm(connector)}>
                <Link size={14} /> Connecter
              </button>
            {/if}
            {#if supportsManual(connector)}
              <button type="button" disabled={busy} on:click={() => openCredentialForm(connector)}>
                <Key size={14} /> Clé manuelle
              </button>
            {/if}
          {:else}
            <button type="button" disabled title="Ce connecteur doit terminer sa certification fournisseur.">
              Certification en cours
            </button>
          {/if}
        </footer>
      </article>
    {/each}
  </div>
</section>

{#if selected}
  <div class="modal-backdrop" role="presentation" on:click={closeCredentialForm}>
    <div
      class:dark={currentTheme === "dark"}
      class="credential-dialog"
      role="dialog"
      tabindex="-1"
      aria-modal="true"
      aria-labelledby="credential-dialog-title"
      on:click|stopPropagation
      on:keydown={(event) => event.key === "Escape" && closeCredentialForm()}
    >
      <header>
        <h3 id="credential-dialog-title">Connecter {selected.displayName}</h3>
        <button aria-label="Fermer" type="button" on:click={closeCredentialForm}><X size={16} /></button>
      </header>
      {#if selectedMode === "manual"}
        <label>
          Libellé
          <input bind:value={labelDraft} maxlength="120" />
        </label>
        <label>
          {selected.authMethods.includes("service_account") ? "Compte de service JSON" : "Clé ou jeton"}
          {#if selected.authMethods.includes("service_account")}
            <textarea bind:value={secretDraft} rows="8" spellcheck="false"></textarea>
          {:else}
            <input type="password" bind:value={secretDraft} autocomplete="new-password" />
          {/if}
        </label>
      {:else}
        <p>ARO ouvrira le navigateur du système. Aucun jeton ne sera transmis au lien de retour.</p>
      {/if}
      {#if selected.capabilities.length}
        <fieldset>
          <legend>Capacités autorisées</legend>
          {#each selected.capabilities as capability}
            <label class="capability">
              <input
                type="checkbox"
                checked={capabilityIds.includes(capability.id)}
                on:change={() => toggleCapability(capability.id)}
              />
              <span><strong>{capability.label}</strong><small>{capability.description}</small></span>
            </label>
          {/each}
        </fieldset>
      {/if}
      {#if error}<div role="alert" class="integration-alert">{error}</div>{/if}
      <footer>
        <button type="button" on:click={closeCredentialForm}>Annuler</button>
        {#if selectedMode === "manual"}
          <button class="primary" type="button" disabled={busy || !secretDraft.trim()} on:click={saveCredential}>
            {busy ? "Connexion…" : "Connecter"}
          </button>
        {:else}
          <button class="primary" type="button" disabled={busy} on:click={saveAuthorization}>
            {busy ? "Ouverture…" : "Continuer dans le navigateur"}
          </button>
        {/if}
      </footer>
    </div>
  </div>
{/if}

<style>
  .integration-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 16px; }
  .integration-card { border: 1px solid rgba(0,0,0,.09); border-radius: 14px; padding: 18px; background: #fff; display: flex; flex-direction: column; gap: 12px; }
  .integration-card.dark, .credential-dialog.dark { background: #242426; color: #fff; border-color: rgba(255,255,255,.09); }
  .integration-card header { display: flex; gap: 12px; align-items: center; }
  .integration-card h3 { margin: 0 0 4px; font-size: 15px; }
  .integration-card p, .integration-card li { color: #86868b; font-size: 12px; line-height: 1.45; }
  .integration-card ul { padding-left: 18px; margin: 0; }
  .integration-icon { width: 38px; height: 38px; display: grid; place-items: center; border-radius: 10px; background: rgba(127,127,127,.1); }
  .status { font-size: 10px; text-transform: uppercase; color: #86868b; }
  .account { display: flex; align-items: center; gap: 6px; color: #34c759; font-size: 12px; }
  footer { display: flex; gap: 8px; margin-top: auto; }
  button { border: 1px solid rgba(127,127,127,.25); border-radius: 8px; padding: 8px 12px; background: transparent; color: inherit; display: inline-flex; align-items: center; gap: 6px; cursor: pointer; }
  button.primary { color: #fff; background: #0071e3; border-color: #0071e3; }
  button.danger { color: #ff453a; }
  button:disabled { cursor: not-allowed; opacity: .55; }
  .integration-alert { padding: 10px 12px; border-radius: 8px; color: #ff453a; background: rgba(255,69,58,.1); margin-bottom: 12px; }
  .modal-backdrop { position: fixed; inset: 0; z-index: 1000; display: grid; place-items: center; padding: 20px; background: rgba(0,0,0,.55); }
  .credential-dialog { width: min(520px, 100%); max-height: 90vh; overflow: auto; background: #fff; border-radius: 16px; padding: 20px; box-shadow: 0 24px 70px rgba(0,0,0,.35); display: grid; gap: 16px; }
  .credential-dialog > header { display: flex; align-items: center; justify-content: space-between; }
  .credential-dialog h3 { margin: 0; }
  .credential-dialog label { display: grid; gap: 7px; font-size: 12px; font-weight: 600; }
  .credential-dialog input, .credential-dialog textarea { border: 1px solid rgba(127,127,127,.28); border-radius: 8px; padding: 10px; background: transparent; color: inherit; font: inherit; }
  .credential-dialog fieldset { border: 1px solid rgba(127,127,127,.2); border-radius: 10px; }
  .credential-dialog .capability { display: flex; grid-template-columns: auto 1fr; align-items: flex-start; }
  .credential-dialog small { display: block; color: #86868b; font-weight: 400; margin-top: 3px; }
  .credential-dialog footer { justify-content: flex-end; }
</style>
