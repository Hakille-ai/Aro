<script lang="ts">
  import type { AppSettings } from "../../../lib/types";
  import CustomSelect from "../../../lib/CustomSelect.svelte";
  import Bell from "@lucide/svelte/icons/bell";
  import Volume2 from "@lucide/svelte/icons/volume-2";
  import Mail from "@lucide/svelte/icons/mail";
  import Bot from "@lucide/svelte/icons/bot";
  import Clock from "@lucide/svelte/icons/clock";
  import Server from "@lucide/svelte/icons/server";
  import Key from "@lucide/svelte/icons/key";
  import ShieldCheck from "@lucide/svelte/icons/shield-check";
  import Check from "@lucide/svelte/icons/check";
  import AlertCircle from "@lucide/svelte/icons/alert-circle";
  import Send from "@lucide/svelte/icons/send";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import {
    playChimeSound,
    testEmailConnection,
    setNotificationSecret,
    clearNotificationSecret,
  } from "../../notifications/model";

  type MaybeAsync = void | Promise<void>;
  export let settingsDraft: AppSettings | null | undefined;
  export let language: "fr" | "en" = "fr";
  export let labels: Record<string, string> = {};
  void labels;
  export let theme: "light" | "dark" = "light";
  export let onAutosave: () => MaybeAsync = () => {};

  $: isDark = theme === "dark" || (typeof document !== "undefined" && document.body?.classList.contains("dark-theme"));

  let smtpPasswordDraft = "";
  let apiKeyDraft = "";
  let secretSaveSuccess = false;
  let secretSaveError = "";
  let testEmailStatus: "idle" | "loading" | "success" | "error" = "idle";
  let testEmailMessage = "";

  const providerOptions = [
    { value: "smtp", label: "Serveur SMTP Standard (Gmail, Outlook, Pro...)" },
    { value: "resend", label: "Resend API" },
    { value: "sendgrid", label: "SendGrid API" },
    { value: "ses", label: "Amazon SES" },
  ];

  const tlsOptions = [
    { value: "starttls", label: "STARTTLS (Port 587 - Recommandé)" },
    { value: "tls", label: "TLS / SSL Direct (Port 465)" },
  ];

  // Initialize notification defaults if missing
  $: if (settingsDraft && !settingsDraft.notification) {
    settingsDraft.notification = {
      desktopNotificationsEnabled: true,
      soundEnabled: true,
      agentCompletionNotifications: true,
      routineNotifications: true,
      emailNotificationsEnabled: false,
      emailOnAgentCompletion: false,
      emailOnRoutineSummary: false,
      emailRecipient: "",
      emailProvider: "smtp",
      smtpHost: "",
      smtpPort: 587,
      smtpUser: "",
      smtpPassword: "",
      smtpFrom: "noreply@aro-ai.com",
      smtpTlsMode: "starttls",
      apiKey: "",
      authConfigured: false,
    };
  }

  function handleAutosave() {
    onAutosave();
  }

  async function handleSaveSmtpPassword() {
    if (!smtpPasswordDraft.trim()) return;
    secretSaveError = "";
    secretSaveSuccess = false;
    try {
      await setNotificationSecret("smtp_password", smtpPasswordDraft.trim());
      if (settingsDraft?.notification) {
        settingsDraft.notification.authConfigured = true;
      }
      smtpPasswordDraft = "";
      secretSaveSuccess = true;
      setTimeout(() => (secretSaveSuccess = false), 3000);
      handleAutosave();
    } catch (err) {
      secretSaveError = err instanceof Error ? err.message : String(err);
    }
  }

  async function handleClearSmtpPassword() {
    secretSaveError = "";
    secretSaveSuccess = false;
    try {
      await clearNotificationSecret("smtp_password");
      if (settingsDraft?.notification) {
        settingsDraft.notification.authConfigured = false;
      }
      smtpPasswordDraft = "";
      handleAutosave();
    } catch (err) {
      secretSaveError = err instanceof Error ? err.message : String(err);
    }
  }

  async function handleSaveApiKey() {
    if (!apiKeyDraft.trim()) return;
    secretSaveError = "";
    secretSaveSuccess = false;
    try {
      await setNotificationSecret("api_key", apiKeyDraft.trim());
      if (settingsDraft?.notification) {
        settingsDraft.notification.authConfigured = true;
      }
      apiKeyDraft = "";
      secretSaveSuccess = true;
      setTimeout(() => (secretSaveSuccess = false), 3000);
      handleAutosave();
    } catch (err) {
      secretSaveError = err instanceof Error ? err.message : String(err);
    }
  }

  async function handleClearApiKey() {
    secretSaveError = "";
    secretSaveSuccess = false;
    try {
      await clearNotificationSecret("api_key");
      if (settingsDraft?.notification) {
        settingsDraft.notification.authConfigured = false;
      }
      apiKeyDraft = "";
      handleAutosave();
    } catch (err) {
      secretSaveError = err instanceof Error ? err.message : String(err);
    }
  }

  async function handleTestEmail() {
    testEmailStatus = "loading";
    testEmailMessage = "";
    try {
      const res = await testEmailConnection();
      testEmailStatus = "success";
      testEmailMessage = language === "fr"
        ? `E-mail envoyé avec succès à ${res.recipient} !`
        : `Test email sent successfully to ${res.recipient}!`;
    } catch (err) {
      testEmailStatus = "error";
      testEmailMessage = err instanceof Error ? err.message : String(err);
    }
  }
</script>

{#if settingsDraft && settingsDraft.notification}
  <div class="settings-tab-panel" class:dark-theme={isDark}>
    <!-- Header -->
    <div class="panel-header">
      <h2>{language === "fr" ? "Notifications & Envoi d'E-mails" : "Notifications & Email Delivery"}</h2>
      <p>
        {language === "fr"
          ? "Contrôlez la manière dont ARO vous avertit lors de la réalisation de tâches longues, alertes critiques ou exécutions de routines."
          : "Control how ARO notifies you upon long-running task completions, critical alerts, or routine executions."}
      </p>
    </div>

    <!-- Section 1: Desktop & Audio Alerts -->
    <div class="settings-group" class:dark-theme={isDark}>
      <div class="settings-group-header">
        <div class="squircle-icon-wrap bell-squircle">
          <Bell size={14} />
        </div>
        <span>{language === "fr" ? "Alertes Système & Sonores" : "Desktop & Sound Alerts"}</span>
      </div>

      <!-- Desktop Notifications Toggle -->
      <div class="settings-row">
        <div class="settings-label-col">
          <span class="settings-title">{language === "fr" ? "Notifications du bureau OS" : "OS Desktop Notifications"}</span>
          <span class="settings-desc">
            {language === "fr"
              ? "Affiche une notification native du système d'exploitation même si ARO est minimisé ou en arrière-plan."
              : "Displays native OS notifications when ARO is minimized or running in the background."}
          </span>
        </div>
        <div class="settings-control-col">
          <label class="ios-toggle toggle-switch">
            <input
              type="checkbox"
              bind:checked={settingsDraft.notification.desktopNotificationsEnabled}
              on:change={handleAutosave}
            />
            <span class="slider"></span>
          </label>
        </div>
      </div>

      <!-- Sound Chime Toggle -->
      <div class="settings-row">
        <div class="settings-label-col">
          <span class="settings-title">{language === "fr" ? "Carillon sonore cristallin" : "Crystal Audio Chime"}</span>
          <span class="settings-desc">
            {language === "fr"
              ? "Joue un carillon harmonique doux et élégant lors de la réception d'une notification."
              : "Plays a soft, high-fidelity harmonic chime cue when a notification arrives."}
          </span>
        </div>
        <div class="settings-control-col actions-col">
          <button
            type="button"
            class="secondary-action-btn"
            title={language === "fr" ? "Tester le son" : "Test sound"}
            on:click={() => playChimeSound()}
          >
            <Volume2 size={14} />
            <span>{language === "fr" ? "Tester" : "Test"}</span>
          </button>
          <label class="ios-toggle toggle-switch">
            <input
              type="checkbox"
              bind:checked={settingsDraft.notification.soundEnabled}
              on:change={handleAutosave}
            />
            <span class="slider"></span>
          </label>
        </div>
      </div>
    </div>

    <!-- Section 2: Triggers & Agents -->
    <div class="settings-group" class:dark-theme={isDark}>
      <div class="settings-group-header">
        <div class="squircle-icon-wrap bot-squircle">
          <Bot size={14} />
        </div>
        <span>{language === "fr" ? "Événements Déclencheurs" : "Trigger Events"}</span>
      </div>

      <!-- Agent Completion -->
      <div class="settings-row">
        <div class="settings-label-col">
          <span class="settings-title">{language === "fr" ? "Achèvement de tâche par l'agent IA" : "AI Agent Task Completion"}</span>
          <span class="settings-desc">
            {language === "fr"
              ? "Recevoir une notification dès qu'un agent autonome ou un sous-agent termine son travail."
              : "Receive a notification as soon as an autonomous agent or subagent completes its execution."}
          </span>
        </div>
        <div class="settings-control-col">
          <label class="ios-toggle toggle-switch">
            <input
              type="checkbox"
              bind:checked={settingsDraft.notification.agentCompletionNotifications}
              on:change={handleAutosave}
            />
            <span class="slider"></span>
          </label>
        </div>
      </div>

      <!-- Scheduled Routine -->
      <div class="settings-row">
        <div class="settings-label-col">
          <span class="settings-title">{language === "fr" ? "Exécution de routines planifiées" : "Scheduled Routine Executions"}</span>
          <span class="settings-desc">
            {language === "fr"
              ? "Recevoir un récapitulatif lorsque vos tâches planifiées ou cron s'exécutent en arrière-plan."
              : "Receive a summary when background cron or recurring routines run."}
          </span>
        </div>
        <div class="settings-control-col">
          <label class="ios-toggle toggle-switch">
            <input
              type="checkbox"
              bind:checked={settingsDraft.notification.routineNotifications}
              on:change={handleAutosave}
            />
            <span class="slider"></span>
          </label>
        </div>
      </div>
    </div>

    <!-- Section 3: Email Delivery -->
    <div class="settings-group" class:dark-theme={isDark}>
      <div class="settings-group-header">
        <div class="squircle-icon-wrap mail-squircle">
          <Mail size={14} />
        </div>
        <span>{language === "fr" ? "Envoi d'E-mails Automatique" : "Automated Email Delivery"}</span>
      </div>

      <!-- Master Email Toggle -->
      <div class="settings-row">
        <div class="settings-label-col">
          <span class="settings-title">{language === "fr" ? "Activer l'envoi d'e-mails" : "Enable Email Notifications"}</span>
          <span class="settings-desc">
            {language === "fr"
              ? "Permet à ARO de vous expédier des rapports et e-mails récapitulatifs sur votre boîte personnelle ou professionnelle."
              : "Allows ARO to dispatch reports and summaries directly to your personal or work inbox."}
          </span>
        </div>
        <div class="settings-control-col">
          <label class="ios-toggle toggle-switch">
            <input
              type="checkbox"
              bind:checked={settingsDraft.notification.emailNotificationsEnabled}
              on:change={handleAutosave}
            />
            <span class="slider"></span>
          </label>
        </div>
      </div>

      {#if settingsDraft.notification.emailNotificationsEnabled}
        <!-- Recipient Email -->
        <div class="settings-row">
          <div class="settings-label-col">
            <span class="settings-title">{language === "fr" ? "Adresse e-mail du destinataire" : "Recipient Email Address"}</span>
            <span class="settings-desc">
              {language === "fr"
                ? "L'adresse sur laquelle les rapports et notifications seront délivrés."
                : "The address where reports and alerts should be delivered."}
            </span>
          </div>
          <div class="settings-control-col">
            <input
              type="email"
              class="settings-input"
              placeholder="votre.email@domaine.com"
              bind:value={settingsDraft.notification.emailRecipient}
              on:change={handleAutosave}
            />
          </div>
        </div>

        <!-- Agent completion email -->
        <div class="settings-row">
          <div class="settings-label-col">
            <span class="settings-title">{language === "fr" ? "E-mail de fin de tâche agent" : "Agent Completion Email"}</span>
            <span class="settings-desc">
              {language === "fr"
                ? "Envoie un e-mail avec le résultat de synthèse dès qu'une tâche longue se termine."
                : "Dispatches an email summary when a long-running agent completes."}
            </span>
          </div>
          <div class="settings-control-col">
            <label class="ios-toggle toggle-switch">
              <input
                type="checkbox"
                bind:checked={settingsDraft.notification.emailOnAgentCompletion}
                on:change={handleAutosave}
              />
              <span class="slider"></span>
            </label>
          </div>
        </div>

        <!-- Routine summary email -->
        <div class="settings-row">
          <div class="settings-label-col">
            <span class="settings-title">{language === "fr" ? "E-mail récapitulatif de routine" : "Routine Summary Email"}</span>
            <span class="settings-desc">
              {language === "fr"
                ? "Transmet par e-mail les résultats d'exécution de vos routines automatiques."
                : "Forwards routine execution output to your email address."}
            </span>
          </div>
          <div class="settings-control-col">
            <label class="ios-toggle toggle-switch">
              <input
                type="checkbox"
                bind:checked={settingsDraft.notification.emailOnRoutineSummary}
                on:change={handleAutosave}
              />
              <span class="slider"></span>
            </label>
          </div>
        </div>
      {/if}
    </div>

    <!-- Section 4: Server / Provider Config -->
    <div class="settings-group" class:dark-theme={isDark}>
      <div class="settings-group-header">
        <div class="squircle-icon-wrap server-squircle">
          <Server size={14} />
        </div>
        <span>{language === "fr" ? "Configuration du Serveur E-mail / SMTP" : "Email / SMTP Server Configuration"}</span>
      </div>

      <!-- Provider Select -->
      <div class="settings-row">
        <div class="settings-label-col">
          <span class="settings-title">{language === "fr" ? "Fournisseur d'envoi" : "Delivery Provider"}</span>
          <span class="settings-desc">
            {language === "fr"
              ? "Sélectionnez votre serveur d'expédition (SMTP dédié, Gmail, Brevo, Resend...)."
              : "Select your mail transport relay (dedicated SMTP, Gmail, Brevo, Resend...)"}
          </span>
        </div>
        <div class="settings-control-col">
          <CustomSelect
            bind:value={settingsDraft.notification.emailProvider}
            options={providerOptions}
            on:change={handleAutosave}
          />
        </div>
      </div>

      {#if settingsDraft.notification.emailProvider === "smtp"}
        <!-- SMTP Host & Port -->
        <div class="settings-row">
          <div class="settings-label-col">
            <span class="settings-title">{language === "fr" ? "Hôte SMTP & Port" : "SMTP Host & Port"}</span>
            <span class="settings-desc">
              {language === "fr"
                ? "Ex: smtp.gmail.com, mail.infomaniak.com, smtp.office365.com..."
                : "E.g., smtp.gmail.com, mail.infomaniak.com, smtp.office365.com..."}
            </span>
          </div>
          <div class="settings-control-col">
            <div class="dual-inputs">
              <input
                type="text"
                class="settings-input host-input"
                placeholder="smtp.example.com"
                bind:value={settingsDraft.notification.smtpHost}
                on:change={handleAutosave}
              />
              <input
                type="number"
                class="settings-input port-input"
                placeholder="587"
                bind:value={settingsDraft.notification.smtpPort}
                on:change={handleAutosave}
              />
            </div>
          </div>
        </div>

        <!-- TLS Mode -->
        <div class="settings-row">
          <div class="settings-label-col">
            <span class="settings-title">{language === "fr" ? "Protocole de Chiffrement" : "Encryption Protocol"}</span>
            <span class="settings-desc">
              {language === "fr" ? "Mode de sécurisation de la connexion SMTP." : "TLS encryption mode for the SMTP relay."}
            </span>
          </div>
          <div class="settings-control-col">
            <CustomSelect
              bind:value={settingsDraft.notification.smtpTlsMode}
              options={tlsOptions}
              on:change={handleAutosave}
            />
          </div>
        </div>

        <!-- Sender Address -->
        <div class="settings-row">
          <div class="settings-label-col">
            <span class="settings-title">{language === "fr" ? "Adresse Expéditeur (From)" : "Sender Address (From)"}</span>
            <span class="settings-desc">
              {language === "fr" ? "Adresse affichée dans l'en-tête de l'e-mail." : "Email address shown in the message header."}
            </span>
          </div>
          <div class="settings-control-col">
            <input
              type="text"
              class="settings-input"
              placeholder="ARO Notifications <noreply@votredomaine.com>"
              bind:value={settingsDraft.notification.smtpFrom}
              on:change={handleAutosave}
            />
          </div>
        </div>

        <!-- SMTP User -->
        <div class="settings-row">
          <div class="settings-label-col">
            <span class="settings-title">{language === "fr" ? "Identifiant / Utilisateur SMTP" : "SMTP Username"}</span>
            <span class="settings-desc">
              {language === "fr" ? "Généralement votre adresse e-mail complète." : "Usually your full email account address."}
            </span>
          </div>
          <div class="settings-control-col">
            <input
              type="text"
              class="settings-input"
              placeholder="user@example.com"
              bind:value={settingsDraft.notification.smtpUser}
              on:change={handleAutosave}
            />
          </div>
        </div>

        <!-- SMTP Password (isolated in OS Keyring) -->
        <div class="settings-row">
          <div class="settings-label-col">
            <span class="settings-title">
              <span style="display: inline-flex; align-items: center; gap: 6px;">
                <Key size={14} />
                <span>{language === "fr" ? "Mot de passe SMTP (Trousseau Sécurisé)" : "SMTP Password (Secure Keyring)"}</span>
              </span>
            </span>
            <span class="settings-desc">
              {language === "fr"
                ? "Isolé dans le trousseau sécurisé de l'OS. Jamais stocké dans les fichiers de configuration texte."
                : "Safely isolated in your OS secure keyring. Never written to plaintext settings."}
            </span>
          </div>
          <div class="settings-control-col">
            {#if settingsDraft.notification.authConfigured}
              <div class="configured-pill-row">
                <span class="configured-badge">
                  <ShieldCheck size={14} />
                  <span>{language === "fr" ? "Mot de passe enregistré dans le trousseau" : "Password stored in OS Keyring"}</span>
                </span>
                <button
                  type="button"
                  class="danger-link-btn"
                  on:click={handleClearSmtpPassword}
                >
                  {language === "fr" ? "Effacer" : "Clear"}
                </button>
              </div>
            {/if}

            <div class="secret-input-row" style="margin-top: {settingsDraft.notification.authConfigured ? '8px' : '0'};">
              <input
                type="password"
                class="settings-input"
                placeholder={settingsDraft.notification.authConfigured ? (language === "fr" ? "Remplacer le mot de passe..." : "Replace password...") : "••••••••••••"}
                bind:value={smtpPasswordDraft}
                on:keydown={(e) => {
                  if (e.key === "Enter" && smtpPasswordDraft.trim()) {
                    void handleSaveSmtpPassword();
                  }
                }}
              />
              <button
                type="button"
                class="primary-action-btn"
                disabled={!smtpPasswordDraft.trim()}
                on:click={handleSaveSmtpPassword}
              >
                <Check size={14} />
                <span>{language === "fr" ? "Enregistrer" : "Save"}</span>
              </button>
            </div>

            {#if secretSaveSuccess}
              <div class="feedback-msg success">
                <Check size={13} />
                <span>{language === "fr" ? "Identifiant sécurisé enregistré !" : "Secure credential stored!"}</span>
              </div>
            {/if}
            {#if secretSaveError}
              <div class="feedback-msg error">
                <AlertCircle size={13} />
                <span>{secretSaveError}</span>
              </div>
            {/if}
          </div>
        </div>
      {:else}
        <!-- Sender Address for API provider -->
        <div class="settings-row">
          <div class="settings-label-col">
            <span class="settings-title">{language === "fr" ? "Adresse Expéditeur (From)" : "Sender Address (From)"}</span>
            <span class="settings-desc">
              {language === "fr"
                ? (settingsDraft.notification.emailProvider === "resend"
                    ? "Adresse vérifiée sur Resend (ex: onboarding@resend.dev ou contact@votredomaine.com)."
                    : "Adresse expéditeur validée sur votre plateforme d'envoi.")
                : "Verified sender email address on your delivery platform."}
            </span>
          </div>
          <div class="settings-control-col">
            <input
              type="text"
              class="settings-input"
              placeholder={settingsDraft.notification.emailProvider === "resend" ? "onboarding@resend.dev" : "noreply@votredomaine.com"}
              bind:value={settingsDraft.notification.smtpFrom}
              on:change={handleAutosave}
            />
          </div>
        </div>

        <!-- API Key Row -->
        <div class="settings-row">
          <div class="settings-label-col">
            <span class="settings-title">
              <span style="display: inline-flex; align-items: center; gap: 6px;">
                <Key size={14} />
                <span>{language === "fr" ? "Clé API / Token Sécurisé" : "API Key / Secure Token"}</span>
              </span>
            </span>
            <span class="settings-desc">
              {language === "fr"
                ? (settingsDraft.notification.emailProvider === "resend"
                    ? "Clé secrète Resend (ex: re_1234...). Stockée dans le trousseau sécurisé de l'OS."
                    : (settingsDraft.notification.emailProvider === "sendgrid"
                        ? "Clé d'API SendGrid avec permission Mail Send. Stockée dans le trousseau sécurisé."
                        : "Identifiant API sécurisé dans le trousseau OS."))
                : "Secure provider token stored isolated in your OS Keyring."}
            </span>
          </div>
          <div class="settings-control-col">
            {#if settingsDraft.notification.authConfigured}
              <div class="configured-pill-row">
                <span class="configured-badge">
                  <ShieldCheck size={14} />
                  <span>{language === "fr" ? "Clé API enregistrée dans le trousseau" : "API key stored in OS Keyring"}</span>
                </span>
                <button
                  type="button"
                  class="danger-link-btn"
                  on:click={handleClearApiKey}
                >
                  {language === "fr" ? "Effacer" : "Clear"}
                </button>
              </div>
            {/if}

            <div class="secret-input-row" style="margin-top: {settingsDraft.notification.authConfigured ? '8px' : '0'};">
              <input
                type="password"
                class="settings-input"
                placeholder={settingsDraft.notification.authConfigured ? (language === "fr" ? "Remplacer la clé API..." : "Replace API key...") : (settingsDraft.notification.emailProvider === "resend" ? "re_123456789..." : "SG.123456789...")}
                bind:value={apiKeyDraft}
                on:keydown={(e) => {
                  if (e.key === "Enter" && apiKeyDraft.trim()) {
                    void handleSaveApiKey();
                  }
                }}
              />
              <button
                type="button"
                class="primary-action-btn"
                disabled={!apiKeyDraft.trim()}
                on:click={handleSaveApiKey}
              >
                <Check size={14} />
                <span>{language === "fr" ? "Enregistrer" : "Save"}</span>
              </button>
            </div>

            {#if secretSaveSuccess}
              <div class="feedback-msg success">
                <Check size={13} />
                <span>{language === "fr" ? "Identifiant sécurisé enregistré !" : "Secure credential stored!"}</span>
              </div>
            {/if}
            {#if secretSaveError}
              <div class="feedback-msg error">
                <AlertCircle size={13} />
                <span>{secretSaveError}</span>
              </div>
            {/if}
          </div>
        </div>
      {/if}

      <!-- Test Connection Button -->
      <div class="settings-row test-connection-row">
        <div class="settings-label-col">
          <span class="settings-title">{language === "fr" ? "Vérification du Transport" : "Transport Verification"}</span>
          <span class="settings-desc">
            {language === "fr"
              ? "Envoie un e-mail test à votre adresse de destination pour valider la chaîne de distribution."
              : "Dispatches an authentic test email to your destination to certify delivery."}
          </span>
        </div>
        <div class="settings-control-col">
          <button
            type="button"
            class="test-email-btn"
            disabled={testEmailStatus === "loading" || !settingsDraft.notification.emailRecipient}
            on:click={handleTestEmail}
          >
            {#if testEmailStatus === "loading"}
              <RefreshCw size={14} class="spin" />
              <span>{language === "fr" ? "Envoi en cours..." : "Sending..."}</span>
            {:else}
              <Send size={14} />
              <span>{language === "fr" ? "Tester l'envoi d'e-mail" : "Test Email Delivery"}</span>
            {/if}
          </button>

          {#if testEmailStatus === "success"}
            <div class="feedback-msg success" style="margin-top: 8px;">
              <Check size={14} />
              <span>{testEmailMessage}</span>
            </div>
          {:else if testEmailStatus === "error"}
            <div class="feedback-msg error" style="margin-top: 8px;">
              <AlertCircle size={14} />
              <span>{testEmailMessage}</span>
            </div>
          {/if}
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .settings-tab-panel {
    display: flex;
    flex-direction: column;
    gap: 22px;
    max-width: 820px;
    font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "SF Pro Display", "SF Pro", system-ui, -apple-system, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  }

  .panel-header {
    margin-bottom: 6px;
  }

  .panel-header h2 {
    font-size: 22px;
    font-weight: 700;
    color: #1d1d1f;
    margin: 0 0 6px 0;
    letter-spacing: -0.025em;
  }

  :global(body.dark-theme) .panel-header h2 {
    color: #f5f5f7;
  }

  .panel-header p {
    font-size: 13px;
    color: #86868b;
    margin: 0;
    line-height: 1.5;
  }

  :global(body.dark-theme) .panel-header p {
    color: #a1a1a6;
  }

  /* Setting Group Cards - Apple Settings Card */
  .settings-group {
    background: #ffffff;
    border: 1px solid rgba(0, 0, 0, 0.08);
    border-radius: 14px;
    padding: 18px 20px;
    display: flex;
    flex-direction: column;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.03), 0 4px 12px rgba(0, 0, 0, 0.015);
    transition: all 0.2s ease;
  }

  :global(body.dark-theme) .settings-group,
  .settings-group.dark-theme {
    background: rgba(30, 30, 34, 0.7);
    backdrop-filter: blur(20px);
    -webkit-backdrop-filter: blur(20px);
    border: 1px solid rgba(255, 255, 255, 0.09);
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.35);
  }

  .settings-group-header {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 13.5px;
    font-weight: 600;
    color: #1d1d1f;
    letter-spacing: -0.01em;
    padding-bottom: 12px;
    margin-bottom: 4px;
    border-bottom: 1px solid rgba(0, 0, 0, 0.05);
  }

  :global(body.dark-theme) .settings-group-header,
  .settings-group.dark-theme .settings-group-header {
    color: #f5f5f7;
    border-bottom-color: rgba(255, 255, 255, 0.07);
  }

  /* Apple System Settings Squircle Icons */
  .squircle-icon-wrap {
    width: 26px;
    height: 26px;
    border-radius: 7px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #ffffff;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.15);
    flex-shrink: 0;
  }

  .bell-squircle {
    background: linear-gradient(135deg, #ff375f 0%, #d70015 100%);
  }

  .bot-squircle {
    background: linear-gradient(135deg, #af52de 0%, #8944ab 100%);
  }

  .mail-squircle {
    background: linear-gradient(135deg, #0071e3 0%, #0051a8 100%);
  }

  .server-squircle {
    background: linear-gradient(135deg, #34c759 0%, #248a3d 100%);
  }

  /* Ligne paramètre : toujours label à gauche + contrôle à droite (style iOS).
     flex-direction explicite pour immuniser contre le media global ≤960px
     qui empilait les lignes en colonne (toggle orphelin). */
  .settings-row {
    display: flex;
    flex-direction: row;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 12px 24px;
    padding: 14px 0;
    border-bottom: 1px solid rgba(0, 0, 0, 0.05);
  }

  .settings-row:last-child {
    border-bottom: none;
    padding-bottom: 0;
  }

  :global(body.dark-theme) .settings-row {
    border-bottom-color: rgba(255, 255, 255, 0.06);
  }

  .settings-label-col {
    display: flex;
    flex-direction: column;
    gap: 4px;
    flex: 1 1 220px;
    min-width: 200px;
  }

  .settings-title {
    font-size: 13.5px;
    font-weight: 600;
    color: #1d1d1f;
    letter-spacing: -0.01em;
  }

  :global(body.dark-theme) .settings-title {
    color: #f5f5f7;
  }

  .settings-desc {
    font-size: 12px;
    color: #86868b;
    line-height: 1.45;
  }

  :global(body.dark-theme) .settings-desc {
    color: #a1a1a6;
  }

  /* Colonne contrôle : épouse son contenu (toggle 44px) et reste collée
     à droite via margin-left:auto ; s'étire jusqu'à 320px pour les inputs.
     Le width:100% du media global ≤960px est neutralisé ici. */
  .settings-control-col {
    width: auto;
    max-width: 100%;
    flex: 0 1 320px;
    min-width: fit-content;
    margin-left: auto;
    align-self: center;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    justify-content: center;
  }

  .settings-control-col :global(.custom-select-container) {
    width: 100%;
    max-width: 320px;
  }

  .settings-control-col.actions-col {
    flex-direction: row;
    align-items: center;
    justify-content: flex-end;
    gap: 12px;
  }

  .settings-control-col > .settings-input {
    width: 100% !important;
  }

  .dual-inputs {
    display: flex;
    flex-direction: row !important;
    align-items: center;
    gap: 8px;
    width: 100%;
    box-sizing: border-box;
  }

  .dual-inputs .host-input {
    width: auto !important;
    flex: 1 1 auto !important;
    min-width: 0 !important;
  }

  .dual-inputs .port-input {
    width: 76px !important;
    max-width: 76px !important;
    flex: 0 0 76px !important;
    text-align: center;
  }

  .settings-input {
    width: 100%;
    box-sizing: border-box;
    padding: 8px 12px;
    border-radius: 8px;
    background: #fbfbfd;
    border: 1px solid rgba(0, 0, 0, 0.12);
    color: #1d1d1f;
    font-size: 13px;
    font-family: inherit;
    outline: none;
    box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.03);
    transition: all 0.2s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .settings-input::placeholder {
    color: #86868b;
  }

  .settings-input:focus {
    background: #ffffff;
    border-color: #0071e3;
    box-shadow: 0 0 0 3px rgba(0, 113, 227, 0.15);
  }

  :global(body.dark-theme) .settings-input {
    background: rgba(255, 255, 255, 0.05);
    border-color: rgba(255, 255, 255, 0.12);
    color: #f5f5f7;
    box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.2);
  }

  :global(body.dark-theme) .settings-input::placeholder {
    color: #636366;
  }

  :global(body.dark-theme) .settings-input:focus {
    background: rgba(255, 255, 255, 0.09);
    border-color: #2997ff;
    box-shadow: 0 0 0 3px rgba(10, 132, 255, 0.25);
  }

  /* Apple iOS Toggle Switch */
  .toggle-switch,
  .ios-toggle {
    position: relative;
    display: inline-block;
    width: 44px;
    height: 26px;
    flex-shrink: 0;
    cursor: pointer;
  }

  .toggle-switch input,
  .ios-toggle input {
    opacity: 0;
    width: 0;
    height: 0;
    position: absolute;
  }

  .slider {
    position: absolute;
    cursor: pointer;
    inset: 0;
    background-color: #e9e9eb;
    transition: background-color 0.25s cubic-bezier(0.16, 1, 0.3, 1);
    border-radius: 26px;
  }

  :global(body.dark-theme) .slider {
    background-color: rgba(120, 120, 128, 0.32);
  }

  .slider:before {
    position: absolute;
    content: "";
    height: 22px;
    width: 22px;
    left: 2px;
    bottom: 2px;
    background-color: #ffffff;
    transition: transform 0.25s cubic-bezier(0.16, 1, 0.3, 1);
    border-radius: 50%;
    box-shadow: 0 3px 8px rgba(0, 0, 0, 0.15), 0 1px 1px rgba(0, 0, 0, 0.06);
  }

  input:checked + .slider {
    background-color: #34c759;
  }

  input:checked + .slider:before {
    transform: translateX(18px);
  }

  /* Audio Chime Test Button */
  .secondary-action-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 5px 12px;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.04);
    border: 1px solid rgba(0, 0, 0, 0.08);
    color: #1d1d1f;
    font-size: 12px;
    font-weight: 500;
    font-family: inherit;
    cursor: pointer;
    transition: all 0.18s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .secondary-action-btn:hover {
    background: rgba(0, 0, 0, 0.08);
    border-color: rgba(0, 0, 0, 0.14);
    transform: translateY(-0.5px);
  }

  .secondary-action-btn:active {
    transform: scale(0.97);
  }

  :global(body.dark-theme) .secondary-action-btn {
    background: rgba(255, 255, 255, 0.08);
    border-color: rgba(255, 255, 255, 0.12);
    color: #f5f5f7;
  }

  :global(body.dark-theme) .secondary-action-btn:hover {
    background: rgba(255, 255, 255, 0.14);
    border-color: rgba(255, 255, 255, 0.2);
    transform: translateY(-0.5px);
  }

  /* Keyring Save Button */
  .primary-action-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 7px 14px;
    border-radius: 8px;
    background: #0071e3;
    border: none;
    color: #ffffff;
    font-size: 12px;
    font-weight: 600;
    font-family: inherit;
    cursor: pointer;
    transition: all 0.18s cubic-bezier(0.16, 1, 0.3, 1);
    white-space: nowrap;
  }

  .primary-action-btn:hover:not(:disabled) {
    background: #0062cc;
    box-shadow: 0 2px 8px rgba(0, 113, 227, 0.25);
    transform: translateY(-0.5px);
  }

  .primary-action-btn:active:not(:disabled) {
    transform: scale(0.98);
  }

  .primary-action-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  :global(body.dark-theme) .primary-action-btn {
    background: #0a84ff;
  }

  :global(body.dark-theme) .primary-action-btn:hover:not(:disabled) {
    background: #0071e3;
  }

  .secret-input-row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .configured-pill-row {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .configured-badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 3px 8px;
    border-radius: 6px;
    background: rgba(52, 199, 89, 0.1);
    color: #28cd41;
    border: 1px solid rgba(52, 199, 89, 0.2);
    font-size: 11px;
    font-weight: 600;
  }

  :global(body.dark-theme) .configured-badge {
    background: rgba(48, 209, 88, 0.16);
    color: #30d158;
    border-color: rgba(48, 209, 88, 0.28);
  }

  .danger-link-btn {
    background: transparent;
    border: none;
    color: #ff3b30;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    text-decoration: underline;
    transition: opacity 0.15s ease;
  }

  .danger-link-btn:hover {
    opacity: 0.8;
  }

  :global(body.dark-theme) .danger-link-btn {
    color: #ff453a;
  }

  .feedback-msg {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    font-weight: 500;
    margin-top: 6px;
  }

  .feedback-msg.success {
    color: #28cd41;
  }

  :global(body.dark-theme) .feedback-msg.success {
    color: #30d158;
  }

  .feedback-msg.error {
    color: #ff3b30;
  }

  :global(body.dark-theme) .feedback-msg.error {
    color: #ff453a;
  }

  /* Transport Verification Button - Apple Blue */
  .test-email-btn {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 9px 18px;
    border-radius: 8px;
    background: #0071e3;
    border: 1px solid rgba(0, 113, 227, 0.8);
    color: #ffffff;
    font-size: 13px;
    font-weight: 600;
    font-family: inherit;
    cursor: pointer;
    box-shadow: 0 1px 3px rgba(0, 113, 227, 0.3), inset 0 1px 0 rgba(255, 255, 255, 0.2);
    transition: all 0.18s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .test-email-btn:hover:not(:disabled) {
    background: #0077ed;
    transform: translateY(-1px);
    box-shadow: 0 4px 14px rgba(0, 113, 227, 0.35);
  }

  .test-email-btn:active:not(:disabled) {
    transform: translateY(0);
    box-shadow: 0 1px 2px rgba(0, 113, 227, 0.2);
  }

  .test-email-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  :global(body.dark-theme) .test-email-btn {
    background: #0a84ff;
    border-color: rgba(10, 132, 255, 0.8);
  }

  :global(body.dark-theme) .test-email-btn:hover:not(:disabled) {
    background: #0077ed;
  }

  :global(.spin) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  /* Petit écran : le label prend toute la largeur, les toggles restent
     à droite (pattern iOS), les inputs s'étirent en pleine largeur. */
  @media (max-width: 640px) {
    .settings-row {
      gap: 10px 16px;
      padding: 12px 0;
    }

    .settings-label-col {
      flex: 1 1 100%;
      min-width: 0;
    }

    .settings-control-col {
      flex: 1 1 auto;
      margin-left: 0;
    }

    .settings-control-col:has(> .ios-toggle:only-child),
    .settings-control-col.actions-col {
      flex: 0 0 auto;
      margin-left: auto;
    }

    .settings-group {
      padding: 14px 16px;
    }
  }
</style>
