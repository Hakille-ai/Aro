<script lang="ts">
  import { onMount } from "svelte";
  import Cpu from "@lucide/svelte/icons/cpu";
  import Monitor from "@lucide/svelte/icons/monitor";
  import Volume2 from "@lucide/svelte/icons/volume-2";
  import VolumeX from "@lucide/svelte/icons/volume-x";
  import Battery from "@lucide/svelte/icons/battery";
  import BatteryCharging from "@lucide/svelte/icons/battery-charging";
  import Wifi from "@lucide/svelte/icons/wifi";
  import Shield from "@lucide/svelte/icons/shield";
  import ShieldAlert from "@lucide/svelte/icons/shield-alert";
  import ShieldCheck from "@lucide/svelte/icons/shield-check";
  import Camera from "@lucide/svelte/icons/camera";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import Terminal from "@lucide/svelte/icons/terminal";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import Check from "@lucide/svelte/icons/check";
  import AlertCircle from "@lucide/svelte/icons/alert-circle";
  import Clipboard from "@lucide/svelte/icons/clipboard";
  import HardDrive from "@lucide/svelte/icons/hard-drive";
  import {
    computerPermissions,
    updateComputerPermissions,
    getLiveComputerEnvironment,
    formatEnvironmentContextForPrompt,
    executeComputerTool,
  } from "../../../lib/computer/computer-service";
  import type { ComputerEnvironmentContext } from "../../../lib/computer/computer-types";

  export let language: "fr" | "en" = "fr";
  export let writeLocked: boolean = false;

  let envContext: ComputerEnvironmentContext | null = null;
  let isRefreshing = false;
  let testVolumeValue = 50;
  let testActionLoading = false;
  let testActionResult: string | null = null;
  let testActionError: string | null = null;

  async function loadLiveEnvironment() {
    isRefreshing = true;
    try {
      envContext = await getLiveComputerEnvironment($computerPermissions);
      if (envContext?.volume) {
        testVolumeValue = envContext.volume.level;
      }
    } catch (err) {
      console.error("Failed to load computer environment context:", err);
    } finally {
      isRefreshing = false;
    }
  }

  onMount(() => {
    loadLiveEnvironment();
  });

  $: promptPreview = envContext
    ? formatEnvironmentContextForPrompt($computerPermissions, envContext, language)
    : "";

  async function handleTestVolumeChange(newLevel: number) {
    testVolumeValue = newLevel;
    testActionLoading = true;
    testActionResult = null;
    testActionError = null;
    try {
      const res = await executeComputerTool("volume_set", { level: newLevel }, $computerPermissions);
      if (res.success) {
        testActionResult = res.message || (language === "fr" ? `Volume réglé à ${newLevel}%` : `Volume set to ${newLevel}%`);
        await loadLiveEnvironment();
      } else {
        testActionError = res.error || (language === "fr" ? "Erreur de réglage" : "Volume error");
      }
    } catch (err: any) {
      testActionError = err?.message || String(err);
    } finally {
      testActionLoading = false;
      setTimeout(() => {
        testActionResult = null;
        testActionError = null;
      }, 4000);
    }
  }

  async function handleTestScreenshot() {
    testActionLoading = true;
    testActionResult = null;
    testActionError = null;
    try {
      const res = await executeComputerTool("screenshot", {}, $computerPermissions);
      if (res.success) {
        testActionResult = res.message || (language === "fr" ? "Capture d'écran réussie !" : "Screenshot captured!");
      } else {
        testActionError = res.error || (language === "fr" ? "Erreur lors de la capture" : "Screenshot error");
      }
    } catch (err: any) {
      testActionError = err?.message || String(err);
    } finally {
      testActionLoading = false;
      setTimeout(() => {
        testActionResult = null;
        testActionError = null;
      }, 5000);
    }
  }
</script>

<div class="settings-tab-panel computer-settings animate-fade-in">
  <!-- Header -->
  <div class="panel-header">
    <div class="panel-header-title-row">
      <div class="header-icon-wrap">
        <Monitor size={22} class="header-icon" />
      </div>
      <div>
        <h2>{language === "fr" ? "Ordinateur & Système" : "Computer & OS Integration"}</h2>
        <p>
          {language === "fr"
            ? "Configurez les autorisations d'interaction native avec votre système d'exploitation, l'injection de contexte en temps réel (batterie, volume, réseau) et les outils autonomes de l'agent."
            : "Configure native workstation control permissions, real-time environment context injection (battery, volume, network), and agent system tools."}
        </p>
      </div>
    </div>
  </div>

  <!-- SECTION 1: Master Switches & Status -->
  <div class="settings-group master-group">
    <div class="group-title-row header-between">
      <div class="title-with-icon">
        <Cpu size={16} class="group-icon purple" />
        <h3>{language === "fr" ? "Contrôle Système & Autorisation Globale" : "Workstation Control & Master Switch"}</h3>
      </div>
      <span class="security-badge" class:active={$computerPermissions.enabled && $computerPermissions.allowComputerUse}>
        {#if $computerPermissions.enabled && $computerPermissions.allowComputerUse}
          <ShieldAlert size={12} />
          {language === "fr" ? "Contrôle Système Actif" : "System Control Active"}
        {:else}
          <ShieldCheck size={12} />
          {language === "fr" ? "Bac à sable sécurisé" : "Sandbox Protected"}
        {/if}
      </span>
    </div>

    <!-- Master Switch -->
    <div class="settings-row highlight-box">
      <div class="settings-label-col">
        <span class="settings-title">{language === "fr" ? "Activer l'intégration Ordinateur" : "Enable Workstation Integration"}</span>
        <span class="settings-desc">
          {language === "fr"
            ? "Interrupteur principal. Autorise l'agent IA à connaître les paramètres de votre ordinateur et à exécuter des outils système dédiés."
            : "Master switch. Allows the AI agent to perceive your workstation environment and invoke system-level tools."}
        </span>
      </div>
      <div class="settings-control-col">
        <label class="ios-toggle">
          <input
            type="checkbox"
            checked={$computerPermissions.enabled}
            disabled={writeLocked}
            on:change={(e) => updateComputerPermissions({ enabled: e.currentTarget.checked })}
          />
          <span class="slider"></span>
        </label>
      </div>
    </div>

    <!-- Computer Use Toggle -->
    <div class="settings-row">
      <div class="settings-label-col">
        <span class="settings-title">{language === "fr" ? "Autoriser l'IA à utiliser l'ordinateur (Computer Use)" : "Allow AI Agent Computer Use"}</span>
        <span class="settings-desc">
          {language === "fr"
            ? "Permet aux agents d'ajuster le volume, de capturer des écrans, d'ouvrir des applications ou des documents locaux sur votre demande."
            : "Permits agents to adjust volume, capture screenshots, and launch applications or documents at your direction."}
        </span>
      </div>
      <div class="settings-control-col">
        <label class="ios-toggle">
          <input
            type="checkbox"
            checked={$computerPermissions.allowComputerUse}
            disabled={writeLocked || !$computerPermissions.enabled}
            on:change={(e) => updateComputerPermissions({ allowComputerUse: e.currentTarget.checked })}
          />
          <span class="slider"></span>
        </label>
      </div>
    </div>

    <!-- Require Confirmation Toggle -->
    <div class="settings-row">
      <div class="settings-label-col">
        <span class="settings-title">{language === "fr" ? "Confirmer les actions sensibles" : "Require Confirmation for Sensitive Actions"}</span>
        <span class="settings-desc">
          {language === "fr"
            ? "Demande votre approbation explicite avant toute action modifiant l'environnement système ou lançant des programmes externes."
            : "Prompts for explicit approval before running external programs or modifying hardware configurations."}
        </span>
      </div>
      <div class="settings-control-col">
        <label class="ios-toggle">
          <input
            type="checkbox"
            checked={$computerPermissions.confirmDangerousActions}
            disabled={writeLocked || !$computerPermissions.enabled}
            on:change={(e) => updateComputerPermissions({ confirmDangerousActions: e.currentTarget.checked })}
          />
          <span class="slider"></span>
        </label>
      </div>
    </div>
  </div>

  <!-- SECTION 2: Environment Context Sharing (Instructions Injection) -->
  <div class="settings-group">
    <div class="group-title-row">
      <Monitor size={16} class="group-icon blue" />
      <h3>{language === "fr" ? "Partage du Contexte Environnement (Instructions IA)" : "Environment Context Sharing in AI Prompts"}</h3>
    </div>
    <div class="group-callout">
      <p>
        {language === "fr"
          ? "Ces données sont injectées dynamiquement dans les instructions système pour permettre à l'IA de connaître votre contexte matériel en temps réel sans jamais envoyer vos fichiers privés."
          : "These telemetry points are dynamically injected into system prompts so the model is aware of your environment in real time."}
      </p>
    </div>

    <!-- Battery -->
    <div class="settings-row">
      <div class="settings-label-col">
        <div class="flex-row align-center gap-6">
          <BatteryCharging size={14} class="row-sub-icon green" />
          <span class="settings-title">{language === "fr" ? "Niveau de batterie & état de charge" : "Battery Level & Charging State"}</span>
        </div>
        <span class="settings-desc">{language === "fr" ? "Partage le pourcentage de batterie et le mode d'alimentation (AC / sur batterie)." : "Shares percentage and power status."}</span>
      </div>
      <div class="settings-control-col">
        <label class="ios-toggle">
          <input
            type="checkbox"
            checked={$computerPermissions.shareBattery}
            disabled={writeLocked || !$computerPermissions.enabled}
            on:change={(e) => updateComputerPermissions({ shareBattery: e.currentTarget.checked })}
          />
          <span class="slider"></span>
        </label>
      </div>
    </div>

    <!-- Volume -->
    <div class="settings-row">
      <div class="settings-label-col">
        <div class="flex-row align-center gap-6">
          <Volume2 size={14} class="row-sub-icon orange" />
          <span class="settings-title">{language === "fr" ? "Volume sonore du système" : "System Audio Volume"}</span>
        </div>
        <span class="settings-desc">{language === "fr" ? "Partage le pourcentage du volume actuel et l'état muet/actif." : "Shares current volume level and mute state."}</span>
      </div>
      <div class="settings-control-col">
        <label class="ios-toggle">
          <input
            type="checkbox"
            checked={$computerPermissions.shareVolume}
            disabled={writeLocked || !$computerPermissions.enabled}
            on:change={(e) => updateComputerPermissions({ shareVolume: e.currentTarget.checked })}
          />
          <span class="slider"></span>
        </label>
      </div>
    </div>

    <!-- Network -->
    <div class="settings-row">
      <div class="settings-label-col">
        <div class="flex-row align-center gap-6">
          <Wifi size={14} class="row-sub-icon blue" />
          <span class="settings-title">{language === "fr" ? "État du réseau et Wi-Fi" : "Network & Wi-Fi Status"}</span>
        </div>
        <span class="settings-desc">{language === "fr" ? "Partage le statut en ligne/hors ligne et le nom du réseau Wi-Fi connecté (SSID)." : "Shares online state and connected Wi-Fi SSID."}</span>
      </div>
      <div class="settings-control-col">
        <label class="ios-toggle">
          <input
            type="checkbox"
            checked={$computerPermissions.shareNetwork}
            disabled={writeLocked || !$computerPermissions.enabled}
            on:change={(e) => updateComputerPermissions({ shareNetwork: e.currentTarget.checked })}
          />
          <span class="slider"></span>
        </label>
      </div>
    </div>

    <!-- IP Address -->
    <div class="settings-row">
      <div class="settings-label-col">
        <div class="flex-row align-center gap-6">
          <Terminal size={14} class="row-sub-icon purple" />
          <span class="settings-title">{language === "fr" ? "Adresse IP interne et locale" : "Internal Local IP Address"}</span>
        </div>
        <span class="settings-desc">{language === "fr" ? "Permet à l'agent de connaître l'IP locale (ex: 192.168.x.x) pour les diagnostics et serveurs locaux." : "Shares local IPv4 address for network tools and dev servers."}</span>
      </div>
      <div class="settings-control-col">
        <label class="ios-toggle">
          <input
            type="checkbox"
            checked={$computerPermissions.shareIp}
            disabled={writeLocked || !$computerPermissions.enabled}
            on:change={(e) => updateComputerPermissions({ shareIp: e.currentTarget.checked })}
          />
          <span class="slider"></span>
        </label>
      </div>
    </div>

    <!-- Display -->
    <div class="settings-row">
      <div class="settings-label-col">
        <div class="flex-row align-center gap-6">
          <Monitor size={14} class="row-sub-icon blue" />
          <span class="settings-title">{language === "fr" ? "Résolution d'écran et échelle DPI" : "Display Resolution & Scaling"}</span>
        </div>
        <span class="settings-desc">{language === "fr" ? "Partage les dimensions d'affichage (ex: 1920x1080) pour adapter les rendus et captures." : "Shares resolution and pixel scaling for visual tasks."}</span>
      </div>
      <div class="settings-control-col">
        <label class="ios-toggle">
          <input
            type="checkbox"
            checked={$computerPermissions.shareDisplay}
            disabled={writeLocked || !$computerPermissions.enabled}
            on:change={(e) => updateComputerPermissions({ shareDisplay: e.currentTarget.checked })}
          />
          <span class="slider"></span>
        </label>
      </div>
    </div>

    <!-- System Stats -->
    <div class="settings-row">
      <div class="settings-label-col">
        <div class="flex-row align-center gap-6">
          <HardDrive size={14} class="row-sub-icon green" />
          <span class="settings-title">{language === "fr" ? "Ressources système (CPU & RAM)" : "System Resources (CPU & RAM)"}</span>
        </div>
        <span class="settings-desc">{language === "fr" ? "Partage le nombre de cœurs CPU et la quantité de mémoire RAM totale et disponible." : "Shares CPU core count and memory stats."}</span>
      </div>
      <div class="settings-control-col">
        <label class="ios-toggle">
          <input
            type="checkbox"
            checked={$computerPermissions.shareSystemStats}
            disabled={writeLocked || !$computerPermissions.enabled}
            on:change={(e) => updateComputerPermissions({ shareSystemStats: e.currentTarget.checked })}
          />
          <span class="slider"></span>
        </label>
      </div>
    </div>

    <!-- Clipboard -->
    <div class="settings-row">
      <div class="settings-label-col">
        <div class="flex-row align-center gap-6">
          <Clipboard size={14} class="row-sub-icon orange" />
          <span class="settings-title">{language === "fr" ? "Extraits récents du presse-papiers" : "Clipboard Snippet Sharing"}</span>
        </div>
        <span class="settings-desc">{language === "fr" ? "Désactivé par défaut. Partage le texte copié pour que l'IA puisse vous aider immédiatement sans collage manuel." : "Disabled by default for privacy. Injects current copied text."}</span>
      </div>
      <div class="settings-control-col">
        <label class="ios-toggle">
          <input
            type="checkbox"
            checked={$computerPermissions.shareClipboard}
            disabled={writeLocked || !$computerPermissions.enabled}
            on:change={(e) => updateComputerPermissions({ shareClipboard: e.currentTarget.checked })}
          />
          <span class="slider"></span>
        </label>
      </div>
    </div>
  </div>

  <!-- SECTION 3: Agent System Tools Permissions -->
  <div class="settings-group">
    <div class="group-title-row">
      <Terminal size={16} class="group-icon purple" />
      <h3>{language === "fr" ? "Outils Système Autonomes de l'IA" : "Autonomous System Tools for AI Agents"}</h3>
    </div>

    <!-- Tool: Volume Control -->
    <div class="settings-row">
      <div class="settings-label-col">
        <span class="settings-title">{language === "fr" ? "Outil Contrôle du Volume" : "Volume Control Tool"}</span>
        <span class="settings-desc">{language === "fr" ? "Permet à l'agent de monter, baisser ou couper le volume sonore sur votre commande." : "Allows the agent to get/set volume and mute/unmute audio."}</span>
      </div>
      <div class="settings-control-col">
        <label class="ios-toggle">
          <input
            type="checkbox"
            checked={$computerPermissions.allowVolumeControl}
            disabled={writeLocked || !$computerPermissions.enabled || !$computerPermissions.allowComputerUse}
            on:change={(e) => updateComputerPermissions({ allowVolumeControl: e.currentTarget.checked })}
          />
          <span class="slider"></span>
        </label>
      </div>
    </div>

    <!-- Tool: Screen Capture -->
    <div class="settings-row">
      <div class="settings-label-col">
        <span class="settings-title">{language === "fr" ? "Outil Capture d'Écran" : "Screen Capture Tool"}</span>
        <span class="settings-desc">{language === "fr" ? "Permet à l'agent de prendre une capture d'écran pour analyser un bug, une interface ou un document affiché." : "Allows the agent to capture screenshots for visual inspection."}</span>
      </div>
      <div class="settings-control-col">
        <label class="ios-toggle">
          <input
            type="checkbox"
            checked={$computerPermissions.allowScreenCapture}
            disabled={writeLocked || !$computerPermissions.enabled || !$computerPermissions.allowComputerUse}
            on:change={(e) => updateComputerPermissions({ allowScreenCapture: e.currentTarget.checked })}
          />
          <span class="slider"></span>
        </label>
      </div>
    </div>

    <!-- Tool: Network Inspection -->
    <div class="settings-row">
      <div class="settings-label-col">
        <span class="settings-title">{language === "fr" ? "Outil Inspection Réseau & Wi-Fi" : "Network & Wi-Fi Inspector"}</span>
        <span class="settings-desc">{language === "fr" ? "Permet à l'agent de vérifier la connectivité réseau, la force du signal Wi-Fi et les adaptateurs." : "Allows inspecting active network adapters and Wi-Fi quality."}</span>
      </div>
      <div class="settings-control-col">
        <label class="ios-toggle">
          <input
            type="checkbox"
            checked={$computerPermissions.allowNetworkInspection}
            disabled={writeLocked || !$computerPermissions.enabled || !$computerPermissions.allowComputerUse}
            on:change={(e) => updateComputerPermissions({ allowNetworkInspection: e.currentTarget.checked })}
          />
          <span class="slider"></span>
        </label>
      </div>
    </div>

    <!-- Tool: App Launcher -->
    <div class="settings-row">
      <div class="settings-label-col">
        <span class="settings-title">{language === "fr" ? "Outil Lanceur d'Applications & Fichiers" : "Application & File Launcher"}</span>
        <span class="settings-desc">{language === "fr" ? "Permet à l'agent de lancer un logiciel local (calculatrice, bloc-notes, navigateur) ou d'ouvrir un dossier." : "Allows the agent to launch applications and open files on command."}</span>
      </div>
      <div class="settings-control-col">
        <label class="ios-toggle">
          <input
            type="checkbox"
            checked={$computerPermissions.allowAppLauncher}
            disabled={writeLocked || !$computerPermissions.enabled || !$computerPermissions.allowComputerUse}
            on:change={(e) => updateComputerPermissions({ allowAppLauncher: e.currentTarget.checked })}
          />
          <span class="slider"></span>
        </label>
      </div>
    </div>

    <!-- Tool: System Inspector -->
    <div class="settings-row">
      <div class="settings-label-col">
        <span class="settings-title">{language === "fr" ? "Outil Diagnostic & Ressources Système" : "System Diagnostics & Resources Inspector"}</span>
        <span class="settings-desc">{language === "fr" ? "Permet à l'agent de surveiller l'état de la machine, de la mémoire et du processeur." : "Allows checking CPU load, RAM usage, and machine state."}</span>
      </div>
      <div class="settings-control-col">
        <label class="ios-toggle">
          <input
            type="checkbox"
            checked={$computerPermissions.allowSystemInspector}
            disabled={writeLocked || !$computerPermissions.enabled || !$computerPermissions.allowComputerUse}
            on:change={(e) => updateComputerPermissions({ allowSystemInspector: e.currentTarget.checked })}
          />
          <span class="slider"></span>
        </label>
      </div>
    </div>
  </div>

  <!-- SECTION 4: Live Telemetry & Interactive Testing -->
  <div class="settings-group telemetry-group">
    <div class="group-title-row header-between">
      <div class="title-with-icon">
        <HardDrive size={16} class="group-icon green" />
        <h3>{language === "fr" ? "Télémétrie en Direct & Actions de Test" : "Live Telemetry & Interactive Testing"}</h3>
      </div>
      <button
        type="button"
        class="apple-btn secondary small"
        disabled={isRefreshing}
        on:click={loadLiveEnvironment}
      >
        <RefreshCw size={12} class={isRefreshing ? "spin" : ""} style="margin-right: 4px;" />
        <span>{language === "fr" ? "Actualiser" : "Refresh"}</span>
      </button>
    </div>

    <!-- Metric Cards Grid -->
    <div class="telemetry-grid">
      <!-- Battery Card -->
      <div class="metric-card">
        <div class="metric-icon-wrap green">
          <BatteryCharging size={18} />
        </div>
        <div class="metric-data">
          <span class="metric-label">{language === "fr" ? "Batterie" : "Battery"}</span>
          <span class="metric-val">{envContext?.battery?.label || (language === "fr" ? "Non détectée" : "N/A")}</span>
        </div>
      </div>

      <!-- Volume Card with Interactive Slider -->
      <div class="metric-card">
        <div class="metric-icon-wrap orange">
          {#if testVolumeValue === 0}
            <VolumeX size={18} />
          {:else}
            <Volume2 size={18} />
          {/if}
        </div>
        <div class="metric-data flex-1">
          <div class="flex-row justify-between align-center">
            <span class="metric-label">{language === "fr" ? "Volume Système" : "System Volume"}</span>
            <span class="metric-val">{testVolumeValue}%</span>
          </div>
          <input
            type="range"
            min="0"
            max="100"
            value={testVolumeValue}
            class="volume-slider"
            disabled={testActionLoading || !$computerPermissions.enabled || !$computerPermissions.allowVolumeControl}
            on:change={(e) => handleTestVolumeChange(Number(e.currentTarget.value))}
          />
        </div>
      </div>

      <!-- Network Card -->
      <div class="metric-card">
        <div class="metric-icon-wrap blue">
          <Wifi size={18} />
        </div>
        <div class="metric-data">
          <span class="metric-label">{language === "fr" ? "Réseau" : "Network"}</span>
          <span class="metric-val">{envContext?.network?.ssid ? `${envContext.network.ssid} (${envContext.network.signal || "OK"})` : (envContext?.network?.online ? (language === "fr" ? "Connecté" : "Online") : (language === "fr" ? "Hors ligne" : "Offline"))}</span>
          {#if envContext?.network?.internalIp}
            <span class="metric-sub">IP: {envContext.network.internalIp}</span>
          {/if}
        </div>
      </div>

      <!-- Display Card -->
      <div class="metric-card">
        <div class="metric-icon-wrap purple">
          <Monitor size={18} />
        </div>
        <div class="metric-data">
          <span class="metric-label">{language === "fr" ? "Écran" : "Display"}</span>
          <span class="metric-val">{envContext?.display ? `${envContext.display.width}x${envContext.display.height} (${envContext.display.scaleFactor}x)` : "Standard"}</span>
        </div>
      </div>
    </div>

    <!-- Test Actions Bar -->
    <div class="test-actions-bar">
      <button
        type="button"
        class="apple-btn primary small"
        disabled={testActionLoading || !$computerPermissions.enabled || !$computerPermissions.allowScreenCapture}
        on:click={handleTestScreenshot}
      >
        <Camera size={13} style="margin-right: 5px;" />
        <span>{language === "fr" ? "Tester la capture d'écran" : "Test Screenshot Capture"}</span>
      </button>

      {#if testActionResult}
        <div class="action-feedback success animate-fade-in">
          <Check size={13} />
          <span>{testActionResult}</span>
        </div>
      {/if}
      {#if testActionError}
        <div class="action-feedback error animate-fade-in">
          <AlertCircle size={13} />
          <span>{testActionError}</span>
        </div>
      {/if}
    </div>

    <!-- Live System Prompt Injection Preview -->
    <div class="prompt-preview-container">
      <div class="prompt-preview-header">
        <span>{language === "fr" ? "Aperçu du bloc injecté dans le System Prompt de l'IA :" : "Live System Prompt Injection Preview:"}</span>
      </div>
      <pre class="prompt-preview-box"><code>{promptPreview || (language === "fr" ? "(Aucun contexte système partagé selon vos réglages actuels)" : "(No system context shared according to your current settings)")}</code></pre>
    </div>
  </div>
</div>

<style>
  .computer-settings {
    max-width: 820px;
  }

  .panel-header {
    margin-bottom: 24px;
  }

  .panel-header-title-row {
    display: flex;
    align-items: flex-start;
    gap: 14px;
  }

  .header-icon-wrap {
    width: 44px;
    height: 44px;
    border-radius: 10px;
    background: rgba(0, 113, 227, 0.12);
    color: #0071e3;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  :global(body.dark-theme) .header-icon-wrap {
    background: rgba(10, 132, 255, 0.18);
    color: #2997ff;
  }

  .panel-header h2 {
    margin: 0 0 6px 0;
    font-size: 20px;
    font-weight: 700;
  }

  .panel-header p {
    margin: 0;
    font-size: 13px;
    color: #8e8e93;
    line-height: 1.45;
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

  .title-with-icon {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .group-title-row h3 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }

  .group-callout {
    background: rgba(0, 113, 227, 0.06);
    border: 1px solid rgba(0, 113, 227, 0.15);
    border-radius: 8px;
    padding: 10px 14px;
    margin-bottom: 16px;
  }

  .group-callout p {
    margin: 0;
    font-size: 12px;
    color: #8e8e93;
    line-height: 1.45;
  }

  .group-icon {
    flex-shrink: 0;
  }
  .group-icon.purple { color: #af52de; }
  .group-icon.blue { color: #0071e3; }
  .group-icon.orange { color: #ff9500; }
  .group-icon.green { color: #30d158; }

  .row-sub-icon {
    flex-shrink: 0;
  }
  .row-sub-icon.green { color: #30d158; }
  .row-sub-icon.orange { color: #ff9500; }
  .row-sub-icon.blue { color: #0071e3; }
  .row-sub-icon.purple { color: #af52de; }

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
    background: rgba(0, 113, 227, 0.06);
    border: 1px solid rgba(0, 113, 227, 0.15);
    border-radius: 10px;
    padding: 14px 16px;
    margin-bottom: 12px;
  }

  .security-badge {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 11px;
    font-weight: 600;
    padding: 3px 8px;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.08);
    color: #8e8e93;
  }

  .security-badge.active {
    background: rgba(48, 209, 88, 0.15);
    color: #30d158;
    border: 1px solid rgba(48, 209, 88, 0.25);
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

  /* Telemetry Grid */
  .telemetry-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 12px;
    margin-bottom: 18px;
  }

  .metric-card {
    display: flex;
    align-items: center;
    gap: 12px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 10px;
    padding: 12px 14px;
  }

  .metric-icon-wrap {
    width: 36px;
    height: 36px;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .metric-icon-wrap.green { background: rgba(48, 209, 88, 0.15); color: #30d158; }
  .metric-icon-wrap.orange { background: rgba(255, 149, 0, 0.15); color: #ff9500; }
  .metric-icon-wrap.blue { background: rgba(0, 113, 227, 0.15); color: #0071e3; }
  .metric-icon-wrap.purple { background: rgba(175, 82, 222, 0.15); color: #af52de; }

  .metric-data {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .metric-label {
    font-size: 11px;
    color: #8e8e93;
    font-weight: 500;
  }

  .metric-val {
    font-size: 13px;
    font-weight: 600;
    color: #f4f4f5;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .metric-sub {
    font-size: 10px;
    color: #71717a;
  }

  .volume-slider {
    width: 100%;
    margin-top: 6px;
    accent-color: #ff9500;
    cursor: pointer;
  }

  .test-actions-bar {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 20px;
    flex-wrap: wrap;
  }

  .action-feedback {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    font-weight: 500;
    padding: 4px 10px;
    border-radius: 6px;
  }

  .action-feedback.success {
    background: rgba(48, 209, 88, 0.15);
    color: #30d158;
  }

  .action-feedback.error {
    background: rgba(255, 69, 58, 0.15);
    color: #ff453a;
  }

  /* Prompt Preview Box */
  .prompt-preview-container {
    background: rgba(0, 0, 0, 0.35);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 8px;
    overflow: hidden;
  }

  .prompt-preview-header {
    background: rgba(255, 255, 255, 0.03);
    padding: 8px 12px;
    font-size: 11px;
    font-weight: 600;
    color: #8e8e93;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  }

  .prompt-preview-box {
    margin: 0;
    padding: 12px;
    font-family: monospace;
    font-size: 11px;
    color: #a1a1aa;
    line-height: 1.5;
    max-height: 220px;
    overflow-y: auto;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .spin {
    animation: rotate 1s linear infinite;
  }

  @keyframes rotate {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  .flex-row { display: flex; }
  .align-center { align-items: center; }
  .justify-between { justify-content: space-between; }
  .flex-1 { flex: 1; }
  .gap-6 { gap: 6px; }
</style>
