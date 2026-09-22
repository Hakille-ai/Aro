<script lang="ts">
  import AlertCircle from "@lucide/svelte/icons/alert-circle";
  import AlertTriangle from "@lucide/svelte/icons/alert-triangle";
  import ArrowLeft from "@lucide/svelte/icons/arrow-left";
  import Check from "@lucide/svelte/icons/check";
  import CheckCheck from "@lucide/svelte/icons/check-check";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import Code from "@lucide/svelte/icons/code";
  import Copy from "@lucide/svelte/icons/copy";
  import Download from "@lucide/svelte/icons/download";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import Folder from "@lucide/svelte/icons/folder";
  import GitBranch from "@lucide/svelte/icons/git-branch";
  import Globe from "@lucide/svelte/icons/globe";
  import Info from "@lucide/svelte/icons/info";
  import Loader2 from "@lucide/svelte/icons/loader-2";
  import Play from "@lucide/svelte/icons/play";
  import Puzzle from "@lucide/svelte/icons/puzzle";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import Activity from "@lucide/svelte/icons/activity";
  import Key from "@lucide/svelte/icons/key";
  import Plus from "@lucide/svelte/icons/plus";
  import Server from "@lucide/svelte/icons/server";
  import Shield from "@lucide/svelte/icons/shield";
  import ShieldCheck from "@lucide/svelte/icons/shield-check";
  import Sparkles from "@lucide/svelte/icons/sparkles";
  import Star from "@lucide/svelte/icons/star";
  import Terminal from "@lucide/svelte/icons/terminal";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import Users from "@lucide/svelte/icons/users";
  import Wrench from "@lucide/svelte/icons/wrench";
  import X from "@lucide/svelte/icons/x";
  import BrandLogo from "../../../lib/BrandLogo.svelte";
  import Zap from "@lucide/svelte/icons/zap";
  import { fade } from "svelte/transition";
  import { requestConfirm } from "../../../lib/confirm";

  import {
    testPluginMcpServer,
    callPluginMcpTool,
    invokePluginSkill,
    listPluginAccounts,
    startPluginOAuthConnect,
    connectPluginApiKey,
    setDefaultPluginAccount,
    updatePluginAccountLabel,
    disconnectPluginAccount,
    testPluginAccountHealth,
    type InstalledPlugin,
    type MarketplacePlugin,
    type PluginAccount,
  } from "../../../lib/api";

  // Props
  export let pluginId: string;
  export let isDark: boolean = false;
  export let installedPlugins: InstalledPlugin[] = [];
  export let marketplacePlugins: MarketplacePlugin[] = [];
  export let actionLoading: Record<string, boolean> = {};

  // Events / Callbacks
  export let onBack: () => void;
  export let onSelectCategory: ((cat: string) => void) | undefined = undefined;
  export let onInstall: (item: MarketplacePlugin) => Promise<void>;
  export let onToggle: (plugin: InstalledPlugin) => Promise<void>;
  export let onUninstall: (plugin: InstalledPlugin) => Promise<void>;
  export let onRefresh: () => Promise<void>;

  // Detail Sub-tabs
  type DetailTab = "overview" | "skills" | "mcp" | "accounts" | "permissions" | "manifest";
  let activeTab: DetailTab = "overview";

  // MCP Live Testing State
  let mcpTesting: Record<string, boolean> = {};
  let mcpTestResults: Record<string, { tools?: any[]; error?: string }> = {};
  let callingToolName: string | null = null;
  let toolArgsInputs: Record<string, string> = {};
  let toolCallResults: Record<string, { success: boolean; data?: any; error?: string }> = {};

  // Skill Live Testing State
  let skillInputText: Record<string, string> = {};
  let skillResults: Record<string, { success: boolean; data?: any; error?: string; loading?: boolean }> = {};

  // Copy Feedback State
  let copiedField: string | null = null;

  // Plugin Accounts Multi-account State
  let accountsList: PluginAccount[] = [];
  let accountsLoading = false;
  let accountActionLoading: Record<string, boolean> = {};
  let accountHealthStatus: Record<string, { healthy: boolean; status: string; message: string }> = {};

  // Add Account Modal / Form State
  let showAddAccountModal = false;
  let addAccountAuthMethod: "oauth2" | "api_key" | "pat" = "oauth2";
  let newAccountLabel = "";
  let newAccountApiKey = "";
  let newAccountIdentifier = "";
  let addAccountLoading = false;
  let addAccountError: string | null = null;
  let addAccountSuccess: string | null = null;

  // Inline Label Editing
  let editingAccountId: string | null = null;
  let editingAccountLabel = "";

  $: authConfig = market?.auth || (installed?.auth ?? null);

  async function loadAccounts() {
    accountsLoading = true;
    try {
      accountsList = await listPluginAccounts(pluginId);
    } catch (err) {
      console.warn("Failed to load accounts for plugin:", err);
      accountsList = [];
    } finally {
      accountsLoading = false;
    }
  }

  $: if (pluginId) {
    loadAccounts();
  }

  async function handleSetDefault(accId: string) {
    accountActionLoading[accId] = true;
    try {
      await setDefaultPluginAccount(accId);
      await loadAccounts();
    } catch (err: any) {
      console.error("Failed to set default account:", err);
    } finally {
      delete accountActionLoading[accId];
      accountActionLoading = accountActionLoading;
    }
  }

  async function handleSaveLabel(accId: string) {
    if (!editingAccountLabel.trim()) return;
    accountActionLoading[accId] = true;
    try {
      await updatePluginAccountLabel(accId, editingAccountLabel.trim());
      editingAccountId = null;
      await loadAccounts();
    } catch (err: any) {
      console.error("Failed to update account label:", err);
    } finally {
      delete accountActionLoading[accId];
      accountActionLoading = accountActionLoading;
    }
  }

  async function handleTestHealth(accId: string) {
    accountActionLoading[accId] = true;
    try {
      const res = await testPluginAccountHealth(accId);
      accountHealthStatus[accId] = res;
      accountHealthStatus = accountHealthStatus;
      await loadAccounts();
    } catch (err: any) {
      accountHealthStatus[accId] = {
        healthy: false,
        status: "error",
        message: err?.message || "Échec du test de santé",
      };
      accountHealthStatus = accountHealthStatus;
    } finally {
      delete accountActionLoading[accId];
      accountActionLoading = accountActionLoading;
    }
  }

  async function handleDisconnectAccount(accId: string, accLabel: string) {
    const confirmed = await requestConfirm({
      title: "Déconnecter le compte",
      message: `Êtes-vous sûr de vouloir déconnecter le compte "${accLabel}" ? Les identifiants et jetons seront supprimés du trousseau sécurisé de l'OS.`,
      confirmText: "Déconnecter",
      cancelText: "Annuler",
      danger: true,
    });
    if (!confirmed) return;

    accountActionLoading[accId] = true;
    try {
      await disconnectPluginAccount(accId);
      await loadAccounts();
    } catch (err: any) {
      console.error("Failed to disconnect account:", err);
    } finally {
      delete accountActionLoading[accId];
      accountActionLoading = accountActionLoading;
    }
  }

  async function handleStartOAuthConnect() {
    addAccountLoading = true;
    addAccountError = null;
    addAccountSuccess = null;
    try {
      await startPluginOAuthConnect({
        pluginId,
        label: newAccountLabel.trim() || undefined,
        openBrowser: true,
      });
      addAccountSuccess = "Navigateur ouvert. Complétez la connexion pour ajouter le compte.";
      setTimeout(async () => {
        await loadAccounts();
      }, 3000);
    } catch (err: any) {
      addAccountError = err?.message || "Impossible de démarrer le flux OAuth";
    } finally {
      addAccountLoading = false;
    }
  }

  async function handleConnectApiKey() {
    if (!newAccountApiKey.trim()) {
      addAccountError = "Veuillez renseigner une clé API ou un jeton";
      return;
    }
    addAccountLoading = true;
    addAccountError = null;
    addAccountSuccess = null;
    try {
      await connectPluginApiKey({
        pluginId,
        apiKey: newAccountApiKey.trim(),
        label: newAccountLabel.trim() || undefined,
        accountIdentifier: newAccountIdentifier.trim() || undefined,
        authMethod: addAccountAuthMethod,
      });
      addAccountSuccess = "Compte connecté avec succès et stocké dans le trousseau sécurisé.";
      await loadAccounts();
      setTimeout(() => {
        showAddAccountModal = false;
      }, 800);
    } catch (err: any) {
      addAccountError = err?.message || "Échec de l'enregistrement de la clé";
    } finally {
      addAccountLoading = false;
    }
  }

  // Reactively resolve plugin details
  $: installed = installedPlugins.find((p) => p.id === pluginId) || null;
  $: market = marketplacePlugins.find((m) => m.id === pluginId) || null;
  $: isInstalled = !!installed;

  $: name = installed?.name || market?.name || pluginId;
  $: version = installed?.version || market?.version || "1.0.0";
  $: description = installed?.description || market?.description || "Aucune description fournie pour ce plugin.";
  $: author = installed?.author || market?.author || "ARO Ecosystem";
  $: category = market?.category || "Outils";
  $: icon = market?.icon || installed?.skills?.[0]?.icon || "🧩";
  $: repository = installed?.repository || market?.repository || "";
  $: homepage = installed?.homepage || "";
  $: license = installed?.license || "MIT";
  $: enabled = installed ? installed.enabled : false;
  $: isSystem = !!installed?.isSystem;
  $: status = installed?.status || (isInstalled ? "active" : "available");
  $: statusMessage = installed?.statusMessage || "";
  $: source = installed?.source || (market ? "marketplace" : "custom");

  $: keywords = installed?.keywords && installed.keywords.length > 0
    ? installed.keywords
    : (market?.keywords || []);

  // Standardized Skills list
  $: skillsList = installed?.skills && installed.skills.length > 0
    ? installed.skills
    : (market?.sampleSkills || []).map((s) => ({
        id: s,
        name: s
          .split("-")
          .map((w: string) => w.charAt(0).toUpperCase() + w.slice(1))
          .join(" "),
        description: `Compétence autonome déclarée pour l'assistant fournie par ${name}.`,
        tags: keywords,
        hasScripts: true,
        icon: icon,
      }));

  // Standardized MCP Servers list
  $: mcpServersList = (installed?.mcpServers && installed.mcpServers.length > 0
    ? installed.mcpServers
    : (market?.sampleMcpServers || []).map((s) => ({
        name: s,
        transportType: "stdio" as const,
        command: `npx -y @agentplugins/${s}`,
        url: undefined as string | undefined,
        status: "available",
      }))) as import("../../../lib/api").PluginMcpServerSummary[];

  // Manifest payload for JSON inspector
  $: manifestPayload = installed
    ? {
        id: installed.id,
        name: installed.name,
        version: installed.version,
        description: installed.description,
        author: installed.author,
        license: installed.license,
        repository: installed.repository,
        homepage: installed.homepage,
        enabled: installed.enabled,
        isSystem: installed.isSystem,
        status: installed.status,
        statusMessage: installed.statusMessage,
        installedAt: installed.installedAt,
        rootPath: installed.rootPath,
        dataPath: installed.dataPath,
        source: installed.source,
        keywords: installed.keywords,
        skills: installed.skills,
        mcpServers: installed.mcpServers,
      }
    : market
    ? {
        id: market.id,
        name: market.name,
        version: market.version,
        description: market.description,
        author: market.author,
        category: market.category,
        repository: market.repository,
        keywords: market.keywords,
        mcpServersCount: market.mcpServersCount,
        skillsCount: market.skillsCount,
        sampleSkills: market.sampleSkills,
        sampleMcpServers: market.sampleMcpServers,
        standard: "agent-plugins.org v1.0.0",
      }
    : { id: pluginId, name };

  async function handleCopy(text: string, fieldKey: string) {
    try {
      await navigator.clipboard.writeText(text);
      copiedField = fieldKey;
      setTimeout(() => {
        if (copiedField === fieldKey) copiedField = null;
      }, 2000);
    } catch (err) {
      console.warn("Clipboard copy error:", err);
    }
  }

  async function handleTestMcp(serverName: string) {
    mcpTesting[serverName] = true;
    mcpTesting = mcpTesting;
    try {
      const tools = await testPluginMcpServer(pluginId, serverName);
      mcpTestResults[serverName] = { tools: Array.isArray(tools) ? tools : [] };
    } catch (err: any) {
      console.warn("MCP Test failure, falling back to schema inspection:", err);
      mcpTestResults[serverName] = {
        error: err?.message || "Impossible de joindre le processus MCP local.",
      };
    } finally {
      mcpTesting[serverName] = false;
      mcpTesting = mcpTesting;
      mcpTestResults = mcpTestResults;
    }
  }

  async function handleCallMcpTool(serverName: string, toolName: string) {
    callingToolName = `${serverName}:${toolName}`;
    const rawArgs = toolArgsInputs[`${serverName}:${toolName}`] || "{}";
    let parsedArgs: Record<string, any> = {};
    try {
      parsedArgs = JSON.parse(rawArgs);
    } catch (e) {
      parsedArgs = { input: rawArgs };
    }

    try {
      const res = await callPluginMcpTool(pluginId, serverName, toolName, parsedArgs);
      toolCallResults[`${serverName}:${toolName}`] = { success: true, data: res };
    } catch (err: any) {
      toolCallResults[`${serverName}:${toolName}`] = {
        success: false,
        error: err?.message || String(err),
      };
    } finally {
      callingToolName = null;
      toolCallResults = toolCallResults;
    }
  }

  async function handleInvokeSkill(skillId: string) {
    skillResults[skillId] = { loading: true, success: false };
    skillResults = skillResults;
    const promptInput = skillInputText[skillId] || "Test execution for " + skillId;

    try {
      const res = await invokePluginSkill(pluginId, skillId, { prompt: promptInput });
      skillResults[skillId] = {
        loading: false,
        success: true,
        data: res || { status: "success", message: `Skill ${skillId} exécuté avec succès.` },
      };
    } catch (err: any) {
      skillResults[skillId] = {
        loading: false,
        success: false,
        error: err?.message || String(err),
      };
    } finally {
      skillResults = skillResults;
    }
  }
</script>

<div class="plugin-detail-root {isDark ? 'dark' : 'light'}" in:fade={{ duration: 180 }}>
  <!-- Breadcrumb Navigation Bar -->
  <div class="breadcrumb-header">
    <div class="breadcrumb-left">
      <button type="button" class="back-link-btn" on:click={onBack} title="Revenir à la liste des plugins">
        <ArrowLeft size={16} />
        <span>Plugins</span>
      </button>

      <span class="crumb-separator"><ChevronRight size={14} /></span>
      {#if onSelectCategory}
        <button
          type="button"
          class="crumb-category-btn"
          on:click={() => onSelectCategory && onSelectCategory(category)}
          title="Filtrer le catalogue par {category}"
        >
          <span>{category}</span>
        </button>
      {:else}
        <span class="crumb-category">{category}</span>
      {/if}

      <span class="crumb-separator"><ChevronRight size={14} /></span>
      <span class="crumb-active">{name}</span>
    </div>

    <!-- Right Header Actions -->
    <div class="header-action-group">
      {#if isInstalled && installed}
        <!-- iOS Enable Switch with label -->
        <div class="switch-badge-wrapper">
          <span class="switch-label {enabled ? 'active' : 'inactive'}">
            {enabled ? "Activé" : "Désactivé"}
          </span>
          <label class="ios-switch" title={enabled ? "Désactiver le plugin" : "Activer le plugin"}>
            <input
              type="checkbox"
              checked={enabled}
              disabled={actionLoading[pluginId]}
              on:change={() => installed && onToggle(installed)}
            />
            <span class="switch-track"></span>
          </label>
        </div>

        {#if isSystem}
          <div class="system-locked-pill" title="Ce plugin essentiel est fourni par ARO et ne peut pas être supprimé">
            <Shield size={13} />
            <span>Système protégé</span>
          </div>
        {:else}
          <button
            type="button"
            class="action-btn-danger"
            disabled={actionLoading[pluginId]}
            on:click={() => installed && onUninstall(installed)}
            title="Désinstaller ce plugin"
          >
            {#if actionLoading[pluginId]}
              <Loader2 size={13} class="spin" />
              <span>Désinstallation…</span>
            {:else}
              <Trash2 size={14} />
              <span>Désinstaller</span>
            {/if}
          </button>
        {/if}
      {:else}
        <button
          type="button"
          class="action-btn-primary"
          disabled={actionLoading[pluginId]}
          on:click={() => market && onInstall(market)}
        >
          {#if actionLoading[pluginId]}
            <Loader2 size={14} class="spin" />
            <span>Installation en cours…</span>
          {:else}
            <Download size={14} />
            <span>Installer le plugin</span>
          {/if}
        </button>
      {/if}

      {#if repository}
        <a
          href={repository}
          target="_blank"
          rel="noopener noreferrer"
          class="action-btn-secondary"
          title="Voir le code source sur Git"
        >
          <GitBranch size={13} />
          <span>Dépôt</span>
          <ExternalLink size={11} />
        </a>
      {/if}

      {#if homepage}
        <a
          href={homepage}
          target="_blank"
          rel="noopener noreferrer"
          class="action-btn-secondary"
          title="Consulter le site officiel"
        >
          <Globe size={13} />
          <span>Site officiel</span>
          <ExternalLink size={11} />
        </a>
      {/if}

      <button
        type="button"
        class="refresh-icon-btn"
        on:click={onRefresh}
        title="Actualiser les informations"
      >
        <RefreshCw size={14} />
      </button>
    </div>
  </div>

  <!-- Hero Showcase Card -->
  <div class="hero-showcase-card">
    <div class="hero-left">
      <BrandLogo
        pluginId={pluginId}
        icon={icon}
        logo={installed?.logo}
        logoKind={installed?.logoKind}
        brandColor={installed?.brandColor}
        size={64}
        radius={16}
      />

      <div class="hero-info">
        <div class="hero-title-row">
          <h1 class="hero-title">{name}</h1>
          <span class="version-pill">v{version}</span>
          {#if isSystem}
            <span class="hero-tag system">Système</span>
          {/if}
          {#if isInstalled}
            <span class="hero-tag source">{source}</span>
          {/if}
          {#if status === "error"}
            <span class="hero-tag error" title={statusMessage}>
              <AlertTriangle size={11} />
              <span>Erreur</span>
            </span>
          {/if}
        </div>

        <div class="hero-meta-badges">
          <span class="author-badge">Par <strong>{author}</strong></span>
          <span class="dot-sep">•</span>
          <span class="category-badge">{category}</span>
          <span class="dot-sep">•</span>
          <span class="license-badge">Licence {license}</span>
          <span class="dot-sep">•</span>
          <a
            href="https://agent-plugins.org/"
            target="_blank"
            rel="noopener noreferrer"
            class="spec-pill"
            title="Spécification officielle standard agent-plugins.org"
          >
            <Sparkles size={11} />
            <span>Spec v1.0.0</span>
            <ExternalLink size={10} />
          </a>
        </div>

        <p class="hero-description">{description}</p>

        {#if statusMessage && status === "error"}
          <div class="hero-error-banner">
            <AlertCircle size={14} />
            <span>{statusMessage}</span>
          </div>
        {/if}

        {#if keywords && keywords.length > 0}
          <div class="hero-keywords">
            {#each keywords as kw}
              <span class="keyword-chip">#{kw}</span>
            {/each}
          </div>
        {/if}
      </div>
    </div>

    <!-- Quick Stat Cards -->
    <div class="hero-stats-grid">
      <div class="stat-card">
        <div class="stat-icon skill"><Wrench size={16} /></div>
        <div class="stat-content">
          <strong class="stat-num">{skillsList.length}</strong>
          <span class="stat-label">Skills autonomes</span>
        </div>
      </div>

      <div class="stat-card">
        <div class="stat-icon mcp"><Server size={16} /></div>
        <div class="stat-content">
          <strong class="stat-num">{mcpServersList.length}</strong>
          <span class="stat-label">Serveur{mcpServersList.length > 1 ? "s" : ""} MCP</span>
        </div>
      </div>

      <div class="stat-card">
        <div class="stat-icon security">
          {#if status === "error"}
            <AlertTriangle size={16} />
          {:else}
            <ShieldCheck size={16} />
          {/if}
        </div>
        <div class="stat-content">
          <strong class="stat-num">
            {#if status === "error"}
              Erreur
            {:else if isInstalled}
              {enabled ? "Actif" : "Inactif"}
            {:else}
              Disponible
            {/if}
          </strong>
          <span class="stat-label">{isInstalled ? "Statut runtime" : "Prêt à installer"}</span>
        </div>
      </div>
    </div>
  </div>

  <!-- Segmented Tabs Navigation (Apple Native style pill capsule) -->
  <div class="detail-tabs-bar">
    <div class="segmented-pill-container">
      <button
        type="button"
        class="detail-tab-btn {activeTab === 'overview' ? 'active' : ''}"
        on:click={() => (activeTab = "overview")}
      >
        <Info size={14} />
        <span>Vue d'ensemble</span>
      </button>

      <button
        type="button"
        class="detail-tab-btn {activeTab === 'skills' ? 'active' : ''}"
        on:click={() => (activeTab = "skills")}
      >
        <Wrench size={14} />
        <span>Skills ({skillsList.length})</span>
      </button>

      <button
        type="button"
        class="detail-tab-btn {activeTab === 'mcp' ? 'active' : ''}"
        on:click={() => (activeTab = "mcp")}
      >
        <Server size={14} />
        <span>Serveurs MCP ({mcpServersList.length})</span>
      </button>

      <button
        type="button"
        class="detail-tab-btn {activeTab === 'accounts' ? 'active' : ''}"
        on:click={() => (activeTab = "accounts")}
      >
        <Users size={14} />
        <span>Comptes ({accountsList.length})</span>
      </button>

      <button
        type="button"
        class="detail-tab-btn {activeTab === 'permissions' ? 'active' : ''}"
        on:click={() => (activeTab = "permissions")}
      >
        <ShieldCheck size={14} />
        <span>Permissions & Sécurité</span>
      </button>

      <button
        type="button"
        class="detail-tab-btn {activeTab === 'manifest' ? 'active' : ''}"
        on:click={() => (activeTab = "manifest")}
      >
        <Code size={14} />
        <span>Manifeste (plugin.json)</span>
      </button>
    </div>
  </div>

  <!-- Tab Contents Area -->
  <div class="tab-content-container">
    <!-- TAB 1: OVERVIEW -->
    {#if activeTab === "overview"}
      <div id="tab-panel-overview" role="tabpanel" class="overview-section-grid" in:fade={{ duration: 150 }}>
        <!-- About Block -->
        <div class="content-block">
          <div class="block-header">
            <Info size={16} class="block-icon" />
            <h3 class="block-title">À propos de ce Plugin</h3>
          </div>
          <p class="block-text">
            {description}
          </p>
          <div class="architecture-pills">
            <div class="arch-card">
              <span class="arch-badge mcp">MCP Protocol</span>
              <h4>Communication Découplée</h4>
              <p>Permet à l'assistant d'introspecter et d'invoquer des outils en temps réel via des flux JSON-RPC standardisés.</p>
            </div>
            <div class="arch-card">
              <span class="arch-badge skill">Agent Skills</span>
              <h4>Instructions Expertes</h4>
              <p>Injecte des directives et des workflows spécialisés directement dans le contexte cognitif de l'agent.</p>
            </div>
            <div class="arch-card">
              <span class="arch-badge sandboxed">Sécurité ARO</span>
              <h4>Sandbox & Isolation</h4>
              <p>Contrôle granulaire des accès filesystem, des variables d'environnement et de l'exécution des processus.</p>
            </div>
          </div>
        </div>

        <!-- Technical Metadata Specifications -->
        <div class="content-block">
          <div class="block-header">
            <Terminal size={16} class="block-icon" />
            <h3 class="block-title">Spécifications Techniques</h3>
          </div>

          <div class="specs-table">
            <div class="spec-row">
              <span class="spec-label">Identifiant Unique (ID)</span>
              <div class="spec-value-copy">
                <code>{pluginId}</code>
                <button
                  type="button"
                  class="mini-copy-btn"
                  on:click={() => handleCopy(pluginId, "id")}
                  title="Copier l'ID"
                >
                  {#if copiedField === "id"}
                    <CheckCheck size={12} class="green-text" />
                  {:else}
                    <Copy size={12} />
                  {/if}
                </button>
              </div>
            </div>

            <div class="spec-row">
              <span class="spec-label">Version du Manifeste</span>
              <span class="spec-value">v{version} (Spec agent-plugins.org 1.0)</span>
            </div>

            <div class="spec-row">
              <span class="spec-label">Mainteneur / Développeur</span>
              <span class="spec-value">{author}</span>
            </div>

            <div class="spec-row">
              <span class="spec-label">Catégorie</span>
              <span class="spec-value">{category}</span>
            </div>

            <div class="spec-row">
              <span class="spec-label">Licence Logicielle</span>
              <span class="spec-value">{license}</span>
            </div>

            <div class="spec-row">
              <span class="spec-label">Statut Actuel</span>
              <span class="spec-value">
                {#if status === "error"}
                  <span class="status-pill error">
                    ● Erreur système {statusMessage ? `(${statusMessage})` : ""}
                  </span>
                {:else if isInstalled}
                  <span class="status-pill {enabled ? 'active' : 'inactive'}">
                    ● {enabled ? "Installé & Activé" : "Installé (Désactivé)"}
                  </span>
                {:else}
                  <span class="status-pill available">○ Disponible au catalogue</span>
                {/if}
              </span>
            </div>

            {#if installed}
              <div class="spec-row">
                <span class="spec-label">Dossier Racine (rootPath)</span>
                <div class="spec-value-copy">
                  <code class="path-code">{installed.rootPath}</code>
                  <button
                    type="button"
                    class="mini-copy-btn"
                    on:click={() => installed && handleCopy(installed.rootPath, "rootPath")}
                    title="Copier le chemin"
                  >
                    {#if copiedField === "rootPath"}
                      <CheckCheck size={12} class="green-text" />
                    {:else}
                      <Copy size={12} />
                    {/if}
                  </button>
                </div>
              </div>

              {#if installed.dataPath}
                <div class="spec-row">
                  <span class="spec-label">Dossier Données (dataPath)</span>
                  <div class="spec-value-copy">
                    <code class="path-code">{installed.dataPath}</code>
                    <button
                      type="button"
                      class="mini-copy-btn"
                      on:click={() => installed && handleCopy(installed.dataPath, "dataPath")}
                      title="Copier le chemin"
                    >
                      {#if copiedField === "dataPath"}
                        <CheckCheck size={12} class="green-text" />
                      {:else}
                        <Copy size={12} />
                      {/if}
                    </button>
                  </div>
                </div>
              {/if}

              {#if installed.installedAt}
                <div class="spec-row">
                  <span class="spec-label">Installé le</span>
                  <span class="spec-value">{new Date(installed.installedAt).toLocaleString()}</span>
                </div>
              {/if}

              <div class="spec-row">
                <span class="spec-label">Source d'installation</span>
                <span class="spec-value source-spec">{installed.source}</span>
              </div>
            {/if}

            {#if repository}
              <div class="spec-row">
                <span class="spec-label">Dépôt Source</span>
                <a href={repository} target="_blank" rel="noopener noreferrer" class="link-url">
                  <span>{repository}</span>
                  <ExternalLink size={12} />
                </a>
              </div>
            {/if}

            {#if homepage}
              <div class="spec-row">
                <span class="spec-label">Site officiel</span>
                <a href={homepage} target="_blank" rel="noopener noreferrer" class="link-url">
                  <span>{homepage}</span>
                  <ExternalLink size={12} />
                </a>
              </div>
            {/if}
          </div>
        </div>
      </div>

    <!-- TAB 2: SKILLS -->
    {:else if activeTab === "skills"}
      <div id="tab-panel-skills" role="tabpanel" class="skills-tab-view" in:fade={{ duration: 150 }}>
        <div class="section-intro">
          <p>
            Les <strong>Skills</strong> permettent à l'assistant d'exécuter des workflows spécialisés et autonomes.
            Chaque skill dispose d'instructions structurées sous <code>skills/*/SKILL.md</code> et d'outils contextuels.
          </p>
        </div>

        {#if skillsList.length === 0}
          <div class="empty-tab-state">
            <Wrench size={32} class="faded-icon" />
            <h4>Aucun skill déclaré</h4>
            <p>Ce plugin ne contient pas de skills autonomes dans son manifeste.</p>
          </div>
        {:else}
          <div class="skills-items-grid">
            {#each skillsList as skill}
              <div class="skill-detail-card">
                <div class="skill-card-top">
                  <div class="skill-avatar">
                    <span>{skill.icon || "⚡"}</span>
                  </div>
                  <div class="skill-main-info">
                    <div class="skill-name-row">
                      <h4 class="skill-title">{skill.name}</h4>
                      <div class="skill-id-copy">
                        <code class="skill-id-pill">{skill.id}</code>
                        <button
                          type="button"
                          class="mini-copy-btn"
                          on:click={() => handleCopy(skill.id, `skill-${skill.id}`)}
                          title="Copier l'ID du skill"
                        >
                          {#if copiedField === `skill-${skill.id}`}
                            <CheckCheck size={11} class="green-text" />
                          {:else}
                            <Copy size={11} />
                          {/if}
                        </button>
                      </div>
                      {#if skill.hasScripts}
                        <span class="skill-badge-script" title="Ce skill intègre des scripts exécutables">
                          <Code size={10} />
                          <span>Script autonome</span>
                        </span>
                      {/if}
                    </div>
                    <p class="skill-desc">{skill.description}</p>
                  </div>
                </div>

                {#if skill.tags && skill.tags.length > 0}
                  <div class="skill-tags-row">
                    {#each skill.tags as tag}
                      <span class="skill-tag">#{tag}</span>
                    {/each}
                  </div>
                {/if}

                <!-- Skill Tester Action Bar -->
                <div class="skill-actions-bar">
                  {#if isInstalled}
                    {#if !enabled}
                      <div class="skill-disabled-warning">
                        <AlertTriangle size={13} />
                        <span>Ce plugin est actuellement inactif. Activez-le pour exécuter ce skill en direct.</span>
                      </div>
                    {/if}

                    <div class="skill-test-wrapper">
                      <div class="skill-test-input-row">
                        <input
                          type="text"
                          placeholder="Paramètre de test (ex: instructions pour le skill)..."
                          bind:value={skillInputText[skill.id]}
                          on:keydown={(e) => e.key === "Enter" && handleInvokeSkill(skill.id)}
                          class="skill-test-input"
                        />
                        <button
                          type="button"
                          class="skill-run-btn"
                          disabled={skillResults[skill.id]?.loading || !enabled}
                          on:click={() => handleInvokeSkill(skill.id)}
                        >
                          {#if skillResults[skill.id]?.loading}
                            <Loader2 size={13} class="spin" />
                            <span>Exécution…</span>
                          {:else}
                            <Play size={13} />
                            <span>Tester le Skill</span>
                          {/if}
                        </button>
                      </div>

                      {#if skillResults[skill.id]}
                        <div class="test-feedback-box {skillResults[skill.id].success ? 'success' : 'error'}">
                          {#if skillResults[skill.id].success}
                            <div class="feedback-header">
                              <Check size={14} />
                              <strong>Exécution réussie :</strong>
                            </div>
                            <pre class="test-pre">{JSON.stringify(skillResults[skill.id].data, null, 2)}</pre>
                          {:else}
                            <div class="feedback-header">
                              <X size={14} />
                              <strong>Erreur d'exécution :</strong>
                            </div>
                            <p class="feedback-error-msg">{skillResults[skill.id].error}</p>
                          {/if}
                        </div>
                      {/if}
                    </div>
                  {:else}
                    <div class="skill-offline-hint">
                      <Info size={13} />
                      <span>Installez ce plugin pour exécuter et tester ce skill en direct.</span>
                    </div>
                  {/if}
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </div>

    <!-- TAB 3: MCP SERVERS -->
    {:else if activeTab === "mcp"}
      <div id="tab-panel-mcp" role="tabpanel" class="mcp-tab-view" in:fade={{ duration: 150 }}>
        <div class="section-intro">
          <p>
            Le <strong>Model Context Protocol (MCP)</strong> connecte l'agent à des outils et sources de données externes
            de façon sécurisée et standardisée. Vous pouvez tester la connexion en direct et visualiser les outils exposés.
          </p>
        </div>

        {#if mcpServersList.length === 0}
          <div class="empty-tab-state">
            <Server size={32} class="faded-icon" />
            <h4>Aucun serveur MCP configuré</h4>
            <p>Ce plugin ne déclare pas de serveurs MCP dans sa configuration.</p>
          </div>
        {:else}
          <div class="mcp-servers-list">
            {#each mcpServersList as srv}
              <div class="mcp-server-card">
                <div class="mcp-card-header">
                  <div class="mcp-title-group">
                    <div class="mcp-icon-box">
                      <Server size={16} />
                    </div>
                    <div>
                      <div class="mcp-name-row">
                        <h4 class="mcp-server-name">{srv.name}</h4>
                        <span class="transport-tag">{srv.transportType}</span>
                        <span class="mcp-status-badge {isInstalled && enabled ? 'ready' : 'idle'}">
                          {isInstalled && enabled ? "Prêt" : (isInstalled ? "Inactif" : "Disponible")}
                        </span>
                      </div>
                      <span class="mcp-sub-info">
                        {#if srv.command}
                          Commande: <code>{srv.command}</code>
                        {:else if srv.url}
                          URL: <code>{srv.url}</code>
                        {/if}
                      </span>
                    </div>
                  </div>

                  <!-- Test MCP Button -->
                  {#if isInstalled}
                    <button
                      type="button"
                      class="mcp-test-btn"
                      disabled={mcpTesting[srv.name]}
                      on:click={() => handleTestMcp(srv.name)}
                    >
                      {#if mcpTesting[srv.name]}
                        <Loader2 size={13} class="spin" />
                        <span>Test en cours…</span>
                      {:else}
                        <Play size={13} />
                        <span>Tester le Serveur MCP</span>
                      {/if}
                    </button>
                  {:else}
                    <button
                      type="button"
                      class="mcp-install-cta"
                      disabled={actionLoading[pluginId]}
                      on:click={() => market && onInstall(market)}
                    >
                      <Download size={13} />
                      <span>Installer pour tester</span>
                    </button>
                  {/if}
                </div>

                <!-- MCP Server Live Test Results -->
                {#if mcpTestResults[srv.name]}
                  <div class="mcp-test-box">
                    {#if mcpTestResults[srv.name]?.error}
                      <div class="test-banner error">
                        <X size={15} />
                        <div>
                          <strong>Échec de connexion :</strong>
                          <p>{mcpTestResults[srv.name]?.error}</p>
                        </div>
                      </div>
                    {:else if mcpTestResults[srv.name]?.tools}
                      <div class="test-banner success">
                        <Check size={15} />
                        <div>
                          <strong>Connexion réussie !</strong>
                          <span>{mcpTestResults[srv.name]?.tools?.length || 0} outil(s) MCP disponible(s)</span>
                        </div>
                      </div>

                      {#if (mcpTestResults[srv.name]?.tools?.length ?? 0) === 0}
                        <div class="no-tools-notice">
                          <Info size={14} />
                          <span>Le serveur MCP est connecté mais aucun outil n'est actuellement exposé.</span>
                        </div>
                      {:else}
                        <div class="tools-discovered-list">
                          {#each mcpTestResults[srv.name]?.tools || [] as tool}
                            <div class="tool-item-card">
                              <div class="tool-item-header">
                                <div class="tool-title-group">
                                  <Zap size={14} class="zap-icon" />
                                  <span class="tool-name">{tool.name}</span>
                                </div>
                                <button
                                  type="button"
                                  class="tool-run-btn"
                                  disabled={callingToolName === `${srv.name}:${tool.name}`}
                                  on:click={() => handleCallMcpTool(srv.name, tool.name)}
                                >
                                  {#if callingToolName === `${srv.name}:${tool.name}`}
                                    <Loader2 size={12} class="spin" />
                                    <span>Exécution…</span>
                                  {:else}
                                    <Play size={12} />
                                    <span>Appeler l'outil</span>
                                  {/if}
                                </button>
                              </div>
                              <p class="tool-desc">{tool.description || "Aucune description fournie pour cet outil."}</p>

                              <!-- Tool Arguments Input & Result -->
                              <div class="tool-call-sub">
                                <input
                                  type="text"
                                  placeholder={'Arguments JSON (ex: {"path": "."})'}
                                  bind:value={toolArgsInputs[`${srv.name}:${tool.name}`]}
                                  on:keydown={(e) => e.key === "Enter" && handleCallMcpTool(srv.name, tool.name)}
                                  class="tool-args-input"
                                />
                                {#if toolCallResults[`${srv.name}:${tool.name}`]}
                                  <div class="tool-res-box {toolCallResults[`${srv.name}:${tool.name}`].success ? 'success' : 'error'}">
                                    <pre class="tool-pre">{JSON.stringify(toolCallResults[`${srv.name}:${tool.name}`].data || toolCallResults[`${srv.name}:${tool.name}`].error, null, 2)}</pre>
                                  </div>
                                {/if}
                              </div>
                            </div>
                          {/each}
                        </div>
                      {/if}
                    {/if}
                  </div>
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      </div>

    <!-- TAB: ACCOUNTS (Multi-account & OS Keyring) -->
    {:else if activeTab === "accounts"}
      <div id="tab-panel-accounts" role="tabpanel" class="accounts-tab-view" in:fade={{ duration: 150 }}>
        <!-- Accounts Header Toolbar -->
        <div class="accounts-toolbar">
          <div class="accounts-toolbar-info">
            <h3 class="accounts-toolbar-title">Comptes connectés</h3>
            <p class="accounts-toolbar-desc">
              Gérez les comptes associés à ce plugin. Vous pouvez connecter plusieurs comptes (ex: personnel et professionnel) et désigner un compte actif par défaut. Les jetons et clés sont stockés de façon chiffrée dans le trousseau sécurisé de votre système d'exploitation.
            </p>
          </div>
          <button
            type="button"
            class="add-account-btn"
            on:click={() => {
              showAddAccountModal = true;
              addAccountError = null;
              addAccountSuccess = null;
              newAccountLabel = "";
              newAccountApiKey = "";
              newAccountIdentifier = "";
              if (authConfig && authConfig.supported_methods && authConfig.supported_methods.length > 0) {
                addAccountAuthMethod = authConfig.supported_methods[0];
              } else {
                addAccountAuthMethod = "oauth2";
              }
            }}
          >
            <Plus size={14} />
            <span>Connecter un compte</span>
          </button>
        </div>

        {#if accountsLoading}
          <div class="accounts-loading-state">
            <Loader2 size={24} class="spin" />
            <p>Chargement des comptes...</p>
          </div>
        {:else if accountsList.length === 0}
          <div class="empty-tab-state">
            <Users size={44} class="faded-icon" />
            <h4>Aucun compte connecté</h4>
            <p>
              Ce plugin n'a actuellement aucun compte configuré. Connectez un compte pour permettre à l'assistant d'exécuter des requêtes authentifiées.
            </p>
            <button
              type="button"
              class="add-account-btn mt-4"
              on:click={() => {
                showAddAccountModal = true;
                addAccountError = null;
                addAccountSuccess = null;
                newAccountLabel = "";
                newAccountApiKey = "";
                newAccountIdentifier = "";
                if (authConfig && authConfig.supported_methods && authConfig.supported_methods.length > 0) {
                  addAccountAuthMethod = authConfig.supported_methods[0];
                } else {
                  addAccountAuthMethod = "oauth2";
                }
              }}
            >
              <Plus size={14} />
              <span>Connecter un premier compte</span>
            </button>
          </div>
        {:else}
          <div class="accounts-grid">
            {#each accountsList as acc (acc.id)}
              <div class="account-card {acc.is_default ? 'is-default' : ''}">
                <div class="account-card-header">
                  <div class="account-avatar-wrapper">
                    {#if acc.avatar_url}
                      <img src={acc.avatar_url} alt={acc.label} class="account-avatar" />
                    {:else if acc.auth_method === 'oauth2'}
                      <div class="account-avatar-placeholder oauth">
                        <Users size={18} />
                      </div>
                    {:else}
                      <div class="account-avatar-placeholder api">
                        <Key size={18} />
                      </div>
                    {/if}
                  </div>

                  <div class="account-info">
                    <div class="account-title-row">
                      {#if editingAccountId === acc.id}
                        <div class="account-inline-edit">
                          <input
                            type="text"
                            bind:value={editingAccountLabel}
                            class="account-edit-input"
                            placeholder="Nom du compte"
                            on:keydown={(e) => {
                              if (e.key === 'Enter') handleSaveLabel(acc.id);
                              if (e.key === 'Escape') editingAccountId = null;
                            }}
                          />
                          <button
                            type="button"
                            class="account-save-btn"
                            disabled={accountActionLoading[acc.id]}
                            on:click={() => handleSaveLabel(acc.id)}
                          >
                            <Check size={12} />
                          </button>
                          <button
                            type="button"
                            class="account-cancel-btn"
                            on:click={() => (editingAccountId = null)}
                          >
                            <X size={12} />
                          </button>
                        </div>
                      {:else}
                        <h4 class="account-label-text">{acc.label}</h4>
                        <button
                          type="button"
                          class="account-edit-label-btn"
                          title="Modifier le libellé"
                          on:click={() => {
                            editingAccountId = acc.id;
                            editingAccountLabel = acc.label;
                          }}
                        >
                          <Wrench size={12} />
                        </button>
                      {/if}

                      {#if acc.is_default}
                        <span class="default-badge" title="Compte actif par défaut pour les appels d'outils">
                          <Star size={11} fill="currentColor" />
                          <span>Par défaut</span>
                        </span>
                      {/if}
                    </div>

                    <div class="account-meta-row">
                      <span class="account-identifier">
                        {acc.email || acc.account_identifier || "Identifiant sécurisé"}
                      </span>
                      <span class="account-method-pill">
                        {acc.auth_method === "oauth2" ? "OAuth 2.0 PKCE" : acc.auth_method === "pat" ? "PAT" : "Clé API"}
                      </span>
                      <span class="account-status-pill {acc.status}">
                        {acc.status === "active" ? "Actif" : acc.status === "expired" ? "Expiré" : acc.status === "revoked" ? "Révoqué" : "Erreur"}
                      </span>
                    </div>

                    {#if accountHealthStatus[acc.id]}
                      <div class="account-health-banner {accountHealthStatus[acc.id].healthy ? 'healthy' : 'unhealthy'}">
                        {#if accountHealthStatus[acc.id].healthy}
                          <CheckCheck size={13} />
                        {:else}
                          <AlertCircle size={13} />
                        {/if}
                        <span>{accountHealthStatus[acc.id].message}</span>
                      </div>
                    {/if}
                  </div>
                </div>

                <div class="account-card-footer">
                  <div class="account-timestamps">
                    {#if acc.last_used_at}
                      <span>Utilisé: {new Date(acc.last_used_at).toLocaleDateString()}</span>
                    {:else}
                      <span>Jamais utilisé</span>
                    {/if}
                  </div>

                  <div class="account-actions-group">
                    <button
                      type="button"
                      class="account-action-btn health"
                      disabled={accountActionLoading[acc.id]}
                      title="Tester la validité du compte"
                      on:click={() => handleTestHealth(acc.id)}
                    >
                      {#if accountActionLoading[acc.id]}
                        <Loader2 size={13} class="spin" />
                      {:else}
                        <Activity size={13} />
                      {/if}
                      <span>Tester</span>
                    </button>

                    {#if !acc.is_default}
                      <button
                        type="button"
                        class="account-action-btn default"
                        disabled={accountActionLoading[acc.id]}
                        title="Définir ce compte comme compte par défaut"
                        on:click={() => handleSetDefault(acc.id)}
                      >
                        <Star size={13} />
                        <span>Activer</span>
                      </button>
                    {/if}

                    <button
                      type="button"
                      class="account-action-btn disconnect"
                      disabled={accountActionLoading[acc.id]}
                      title="Déconnecter ce compte"
                      on:click={() => handleDisconnectAccount(acc.id, acc.label)}
                    >
                      <Trash2 size={13} />
                      <span>Déconnecter</span>
                    </button>
                  </div>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </div>

    <!-- TAB 4: PERMISSIONS & SECURITY -->
    {:else if activeTab === "permissions"}
      <div id="tab-panel-permissions" role="tabpanel" class="permissions-tab-view" in:fade={{ duration: 150 }}>
        <div class="section-intro">
          <p>
            ARO assure une sécurité de bout en bout conforme au standard officiel <strong>agent-plugins.org v1.0.0</strong>.
            Chaque plugin s'exécute dans une enveloppe d'isolation contrôlée avec validation des privilèges.
          </p>
        </div>

        <div class="security-cards-grid">
          <div class="sec-card">
            <div class="sec-card-header">
              <div class="sec-icon green"><ShieldCheck size={20} /></div>
              <div>
                <h4 class="sec-title">Sandbox & Isolation des Processus</h4>
                <span class="sec-subtitle">Exécution locale sandboxée</span>
              </div>
            </div>
            <p class="sec-body">
              Les serveurs MCP STDIO s'exécutent dans des sous-processus isolés avec des variables d'environnement filtrées.
              Le plugin n'a pas accès direct aux clés système sensibles sans autorisation explicite.
            </p>
            <div class="sec-tag-row">
              <span class="sec-pill">PID Sandboxé</span>
              <span class="sec-pill">Variables ENV restreintes</span>
            </div>
          </div>

          <div class="sec-card">
            <div class="sec-card-header">
              <div class="sec-icon blue"><Folder size={20} /></div>
              <div>
                <h4 class="sec-title">Accès au Système de Fichiers</h4>
                <span class="sec-subtitle">Espace de travail délimité</span>
              </div>
            </div>
            <p class="sec-body">
              L'accès en lecture et écriture est strictement confiné au répertoire racine du plugin (<code>rootPath</code>)
              et aux dossiers de projets autorisés dans les paramètres d'espace de travail ARO.
            </p>
            <div class="sec-tag-row">
              <span class="sec-pill">Chroot virtuel</span>
              <span class="sec-pill">Validation des chemins relatifs</span>
            </div>
          </div>

          <div class="sec-card">
            <div class="sec-card-header">
              <div class="sec-icon purple"><Terminal size={20} /></div>
              <div>
                <h4 class="sec-title">Appels d'Outils et Confirmation</h4>
                <span class="sec-subtitle">Protection contre les actions destructrices</span>
              </div>
            </div>
            <p class="sec-body">
              Toute commande pouvant altérer des données ou modifier l'environnement nécessite une confirmation utilisateur
              ou respecte les garde-fous établis dans la politique d'audit d'ARO.
            </p>
            <div class="sec-tag-row">
              <span class="sec-pill">Journalisation d'audit</span>
              <span class="sec-pill">Confirmation requise</span>
            </div>
          </div>

          <div class="sec-card">
            <div class="sec-card-header">
              <div class="sec-icon orange"><Sparkles size={20} /></div>
              <div>
                <h4 class="sec-title">Conformité agent-plugins.org</h4>
                <span class="sec-subtitle">Spécification universelle v1.0.0</span>
              </div>
            </div>
            <p class="sec-body">
              Ce plugin utilise la structure standardisée avec <code>plugin.json</code>, <code>skills/*/SKILL.md</code>
              et <code>.mcp.json</code> garantissant l'interopérabilité et la portabilité totale.
            </p>
            <div class="sec-tag-row">
              <span class="sec-pill">Schéma JSON validé</span>
              <span class="sec-pill">Standard ouvert</span>
            </div>
          </div>
        </div>
      </div>

    <!-- TAB 5: MANIFEST (plugin.json) -->
    {:else if activeTab === "manifest"}
      <div id="tab-panel-manifest" role="tabpanel" class="manifest-tab-view" in:fade={{ duration: 150 }}>
        <div class="manifest-toolbar">
          <div class="manifest-info-left">
            <Code size={16} />
            <span class="manifest-filename">plugin.json</span>
            <span class="manifest-size-badge">
              {JSON.stringify(manifestPayload).length} octets
            </span>
          </div>

          <button
            type="button"
            class="copy-manifest-btn"
            on:click={() => handleCopy(JSON.stringify(manifestPayload, null, 2), "manifest")}
          >
            {#if copiedField === "manifest"}
              <CheckCheck size={14} class="green-text" />
              <span>Copié dans le presse-papier !</span>
            {:else}
              <Copy size={14} />
              <span>Copier le Manifeste</span>
            {/if}
          </button>
        </div>

        <div class="manifest-code-wrapper">
          <pre class="manifest-code"><code>{JSON.stringify(manifestPayload, null, 2)}</code></pre>
        </div>
      </div>
    {/if}
  </div>

  <!-- Connect Account Modal (OAuth2 / API Key / PAT) -->
  {#if showAddAccountModal}
    <div class="modal-backdrop" in:fade={{ duration: 150 }}>
      <div class="modal-card">
        <div class="modal-header">
          <div class="modal-header-left">
            <div class="modal-icon-badge">
              <Key size={18} />
            </div>
            <div>
              <h3 class="modal-title">Connecter un compte</h3>
              <p class="modal-subtitle">{name} — Authentification sécurisée</p>
            </div>
          </div>
          <button
            type="button"
            class="modal-close-btn"
            on:click={() => (showAddAccountModal = false)}
          >
            <X size={16} />
          </button>
        </div>

        <div class="modal-body">
          {#if authConfig && authConfig.supported_methods && authConfig.supported_methods.length > 1}
            <div class="method-selector">
              <span class="method-selector-label">Méthode d'authentification :</span>
              <div class="method-tabs">
                {#each authConfig.supported_methods as method}
                  <button
                    type="button"
                    class="method-tab-btn {addAccountAuthMethod === method ? 'active' : ''}"
                    on:click={() => {
                      addAccountAuthMethod = method;
                      addAccountError = null;
                      addAccountSuccess = null;
                    }}
                  >
                    {#if method === "oauth2"}
                      <Globe size={13} />
                      <span>OAuth 2.0 (PKCE)</span>
                    {:else if method === "api_key"}
                      <Key size={13} />
                      <span>Clé API</span>
                    {:else}
                      <Shield size={13} />
                      <span>Personal Token</span>
                    {/if}
                  </button>
                {/each}
              </div>
            </div>
          {/if}

          <!-- Form contents depending on method -->
          {#if addAccountAuthMethod === "oauth2"}
            <div class="oauth-connect-section">
              <div class="info-alert">
                <ShieldCheck size={16} class="info-alert-icon" />
                <div class="info-alert-text">
                  <p>
                    L'authentification s'effectue directement auprès du fournisseur via <strong>OAuth 2.0 avec PKCE</strong>.
                    Une fenêtre de navigateur sécurisée va s'ouvrir pour vous permettre d'accorder l'accès.
                  </p>
                  {#if authConfig?.scopes && authConfig.scopes.length > 0}
                    <div class="scope-tags">
                      <span class="scope-tag-title">Permissions requises :</span>
                      {#each authConfig.scopes as sc}
                        <span class="scope-tag">{sc}</span>
                      {/each}
                    </div>
                  {/if}
                </div>
              </div>

              <div class="form-group">
                <label for="new-acc-label-oauth" class="form-label">Libellé du compte (optionnel)</label>
                <input
                  id="new-acc-label-oauth"
                  type="text"
                  class="form-input"
                  placeholder="Ex: Mon compte Google Pro, Personnel..."
                  bind:value={newAccountLabel}
                />
              </div>

              {#if addAccountError}
                <div class="form-error-alert">
                  <AlertCircle size={14} />
                  <span>{addAccountError}</span>
                </div>
              {/if}

              {#if addAccountSuccess}
                <div class="form-success-alert">
                  <CheckCheck size={14} />
                  <span>{addAccountSuccess}</span>
                </div>
              {/if}

              <div class="modal-actions">
                <button
                  type="button"
                  class="btn-secondary"
                  on:click={() => (showAddAccountModal = false)}
                >
                  Annuler
                </button>
                <button
                  type="button"
                  class="btn-primary"
                  disabled={addAccountLoading}
                  on:click={handleStartOAuthConnect}
                >
                  {#if addAccountLoading}
                    <Loader2 size={14} class="spin" />
                    <span>Ouverture du navigateur...</span>
                  {:else}
                    <ExternalLink size={14} />
                    <span>Ouvrir la connexion OAuth</span>
                  {/if}
                </button>
              </div>
            </div>
          {:else}
            <!-- API Key or PAT form -->
            <div class="apikey-connect-section">
              <p class="section-prompt-text">
                {authConfig?.api_key_prompt || "Saisissez votre clé API ou jeton d'accès personnel. Les identifiants sont chiffrés et protégés par le trousseau OS."}
              </p>

              {#if authConfig?.documentation_url}
                <a
                  href={authConfig.documentation_url}
                  target="_blank"
                  rel="noreferrer"
                  class="doc-link-hint"
                >
                  <ExternalLink size={12} />
                  <span>Où trouver ou générer votre clé ?</span>
                </a>
              {/if}

              <div class="form-group">
                <label for="new-acc-key" class="form-label">
                  {addAccountAuthMethod === "pat" ? "Personal Access Token (PAT)" : "Clé API"} *
                </label>
                <input
                  id="new-acc-key"
                  type="password"
                  class="form-input"
                  placeholder={addAccountAuthMethod === "pat" ? "ghp_... ou token secret" : "sk-... ou clé secrète"}
                  bind:value={newAccountApiKey}
                />
              </div>

              <div class="form-group">
                <label for="new-acc-label" class="form-label">Libellé du compte (optionnel)</label>
                <input
                  id="new-acc-label"
                  type="text"
                  class="form-input"
                  placeholder="Ex: Compte équipe Dev, Clé perso..."
                  bind:value={newAccountLabel}
                />
              </div>

              <div class="form-group">
                <label for="new-acc-identifier" class="form-label">Identifiant ou Email associé (optionnel)</label>
                <input
                  id="new-acc-identifier"
                  type="text"
                  class="form-input"
                  placeholder="Ex: developpeur@societe.com"
                  bind:value={newAccountIdentifier}
                />
              </div>

              {#if addAccountError}
                <div class="form-error-alert">
                  <AlertCircle size={14} />
                  <span>{addAccountError}</span>
                </div>
              {/if}

              {#if addAccountSuccess}
                <div class="form-success-alert">
                  <CheckCheck size={14} />
                  <span>{addAccountSuccess}</span>
                </div>
              {/if}

              <div class="modal-actions">
                <button
                  type="button"
                  class="btn-secondary"
                  on:click={() => (showAddAccountModal = false)}
                >
                  Annuler
                </button>
                <button
                  type="button"
                  class="btn-primary"
                  disabled={addAccountLoading || !newAccountApiKey.trim()}
                  on:click={handleConnectApiKey}
                >
                  {#if addAccountLoading}
                    <Loader2 size={14} class="spin" />
                    <span>Enregistrement sécurisé...</span>
                  {:else}
                    <ShieldCheck size={14} />
                    <span>Enregistrer le compte</span>
                  {/if}
                </button>
              </div>
            </div>
          {/if}
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  /* Root & Layout */
  .plugin-detail-root {
    display: flex;
    flex-direction: column;
    width: 100%;
    min-height: 100%;
    box-sizing: border-box;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  }

  /* Breadcrumb Header */
  .breadcrumb-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 16px;
    margin-bottom: 20px;
    flex-wrap: wrap;
  }

  .breadcrumb-left {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    font-weight: 500;
  }

  .back-link-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border: none;
    border-radius: 8px;
    padding: 6px 12px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .plugin-detail-root.light .back-link-btn {
    background: rgba(0, 0, 0, 0.05);
    color: #0071e3;
  }

  .plugin-detail-root.light .back-link-btn:hover {
    background: rgba(0, 113, 227, 0.12);
    color: #0077ed;
  }

  .plugin-detail-root.dark .back-link-btn {
    background: rgba(255, 255, 255, 0.07);
    color: #2997ff;
  }

  .plugin-detail-root.dark .back-link-btn:hover {
    background: rgba(41, 151, 255, 0.2);
    color: #ffffff;
  }

  .crumb-separator {
    color: #86868b;
    opacity: 0.5;
    display: flex;
    align-items: center;
  }

  .crumb-category {
    color: #86868b;
  }

  .crumb-category-btn {
    background: none;
    border: none;
    padding: 2px 6px;
    border-radius: 4px;
    font-size: 13px;
    font-weight: 500;
    color: #86868b;
    cursor: pointer;
    transition: all 0.18s ease;
  }

  .crumb-category-btn:hover {
    color: #0071e3;
    background: rgba(0, 113, 227, 0.08);
  }

  .plugin-detail-root.dark .crumb-category-btn:hover {
    color: #2997ff;
    background: rgba(41, 151, 255, 0.12);
  }

  .crumb-active {
    font-weight: 600;
  }

  .plugin-detail-root.light .crumb-active {
    color: #1d1d1f;
  }

  .plugin-detail-root.dark .crumb-active {
    color: #ffffff;
  }

  /* Header Action Group */
  .header-action-group {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }

  .switch-badge-wrapper {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 10px;
    border-radius: 8px;
    border: 1px solid;
  }

  .plugin-detail-root.light .switch-badge-wrapper {
    background: #ffffff;
    border-color: rgba(0, 0, 0, 0.08);
  }

  .plugin-detail-root.dark .switch-badge-wrapper {
    background: #242426;
    border-color: rgba(255, 255, 255, 0.08);
  }

  .switch-label {
    font-size: 12px;
    font-weight: 600;
  }

  .switch-label.active {
    color: #34c759;
  }

  .switch-label.inactive {
    color: #86868b;
  }

  .system-locked-pill {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 11px;
    font-weight: 600;
    padding: 6px 12px;
    border-radius: 8px;
    background: rgba(255, 149, 0, 0.12);
    color: #ff9500;
    border: 1px solid rgba(255, 149, 0, 0.25);
  }

  .action-btn-primary {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: linear-gradient(180deg, #2997ff 0%, #0071e3 100%);
    color: #ffffff;
    border: none;
    border-radius: 8px;
    padding: 7px 16px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    box-shadow: 0 2px 8px rgba(0, 113, 227, 0.25);
    transition: all 0.2s ease;
  }

  .action-btn-primary:hover {
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(0, 113, 227, 0.35);
  }

  .action-btn-secondary {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    border-radius: 8px;
    padding: 7px 12px;
    font-size: 12px;
    font-weight: 500;
    text-decoration: none;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .plugin-detail-root.light .action-btn-secondary {
    background: rgba(0, 0, 0, 0.05);
    color: #1d1d1f;
    border: 1px solid rgba(0, 0, 0, 0.08);
  }

  .plugin-detail-root.light .action-btn-secondary:hover {
    background: rgba(0, 0, 0, 0.08);
  }

  .plugin-detail-root.dark .action-btn-secondary {
    background: rgba(255, 255, 255, 0.06);
    color: #ffffff;
    border: 1px solid rgba(255, 255, 255, 0.08);
  }

  .plugin-detail-root.dark .action-btn-secondary:hover {
    background: rgba(255, 255, 255, 0.1);
  }

  .action-btn-danger {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border-radius: 8px;
    padding: 7px 14px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s ease;
    border: 1px solid rgba(255, 69, 58, 0.25);
    background: rgba(255, 69, 58, 0.08);
    color: #ff453a;
  }

  .action-btn-danger:hover {
    background: rgba(255, 69, 58, 0.16);
  }

  .refresh-icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border-radius: 8px;
    border: none;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .plugin-detail-root.light .refresh-icon-btn {
    background: rgba(0, 0, 0, 0.04);
    color: #1d1d1f;
  }

  .plugin-detail-root.light .refresh-icon-btn:hover {
    background: rgba(0, 0, 0, 0.08);
  }

  .plugin-detail-root.dark .refresh-icon-btn {
    background: rgba(255, 255, 255, 0.06);
    color: #ffffff;
  }

  .plugin-detail-root.dark .refresh-icon-btn:hover {
    background: rgba(255, 255, 255, 0.12);
  }

  /* Hero Showcase Card */
  .hero-showcase-card {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 24px;
    padding: 24px;
    border-radius: 16px;
    margin-bottom: 24px;
    border: 1px solid;
    transition: all 0.2s ease;
  }

  .plugin-detail-root.light .hero-showcase-card {
    background: linear-gradient(135deg, rgba(255, 255, 255, 0.95) 0%, rgba(245, 245, 247, 0.9) 100%);
    border-color: rgba(0, 0, 0, 0.08);
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.03);
  }

  .plugin-detail-root.dark .hero-showcase-card {
    background: linear-gradient(135deg, rgba(36, 36, 38, 0.9) 0%, rgba(28, 28, 30, 0.95) 100%);
    border-color: rgba(255, 255, 255, 0.08);
    box-shadow: 0 6px 24px rgba(0, 0, 0, 0.3);
  }

  .hero-left {
    display: flex;
    align-items: flex-start;
    gap: 20px;
    flex: 1;
    min-width: 0;
  }

  .plugin-large-icon {
    width: 68px;
    height: 68px;
    border-radius: 16px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 32px;
    flex-shrink: 0;
  }

  .plugin-detail-root.light .plugin-large-icon {
    background: linear-gradient(135deg, #f5f5f7 0%, #ebebef 100%);
    border: 1px solid rgba(0, 0, 0, 0.08);
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.04);
  }

  .plugin-detail-root.dark .plugin-large-icon {
    background: linear-gradient(135deg, #2c2c2e 0%, #1f1f21 100%);
    border: 1px solid rgba(255, 255, 255, 0.08);
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.4);
  }

  .hero-info {
    flex: 1;
    min-width: 0;
  }

  .hero-title-row {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }

  .hero-title {
    font-size: 24px;
    font-weight: 700;
    margin: 0;
    letter-spacing: -0.5px;
  }

  .plugin-detail-root.light .hero-title {
    color: #1d1d1f;
  }

  .plugin-detail-root.dark .hero-title {
    color: #ffffff;
  }

  .version-pill {
    font-size: 11px;
    font-weight: 600;
    padding: 3px 8px;
    border-radius: 6px;
    color: #86868b;
  }

  .plugin-detail-root.light .version-pill {
    background: rgba(0, 0, 0, 0.05);
  }

  .plugin-detail-root.dark .version-pill {
    background: rgba(255, 255, 255, 0.08);
  }

  .hero-tag {
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    padding: 3px 7px;
    border-radius: 4px;
  }

  .hero-tag.system {
    background: rgba(255, 149, 0, 0.15);
    color: #ff9500;
  }

  .hero-tag.source {
    background: rgba(0, 113, 227, 0.1);
    color: #0071e3;
  }

  .plugin-detail-root.dark .hero-tag.source {
    background: rgba(41, 151, 255, 0.15);
    color: #2997ff;
  }

  .hero-tag.error {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    background: rgba(255, 69, 58, 0.15);
    color: #ff453a;
  }

  .hero-meta-badges {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 6px;
    font-size: 12px;
    color: #86868b;
    flex-wrap: wrap;
  }

  .dot-sep {
    opacity: 0.4;
  }

  .author-badge strong {
    color: inherit;
  }

  .plugin-detail-root.light .author-badge strong {
    color: #1d1d1f;
  }

  .plugin-detail-root.dark .author-badge strong {
    color: #ffffff;
  }

  .spec-pill {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    font-weight: 600;
    padding: 2px 8px;
    border-radius: 980px;
    text-decoration: none;
    transition: all 0.2s ease;
  }

  .plugin-detail-root.light .spec-pill {
    background: rgba(0, 113, 227, 0.08);
    color: #0071e3;
    border: 1px solid rgba(0, 113, 227, 0.16);
  }

  .plugin-detail-root.dark .spec-pill {
    background: rgba(41, 151, 255, 0.14);
    color: #2997ff;
    border: 1px solid rgba(41, 151, 255, 0.25);
  }

  .hero-description {
    font-size: 14px;
    line-height: 1.55;
    margin: 10px 0 12px;
    max-width: 720px;
  }

  .plugin-detail-root.light .hero-description {
    color: #515154;
  }

  .plugin-detail-root.dark .hero-description {
    color: #a1a1a6;
  }

  .hero-error-banner {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border-radius: 8px;
    background: rgba(255, 69, 58, 0.1);
    border: 1px solid rgba(255, 69, 58, 0.25);
    color: #ff453a;
    font-size: 12px;
    margin-bottom: 12px;
  }

  .hero-keywords {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .keyword-chip {
    font-size: 11px;
    padding: 2px 7px;
    border-radius: 4px;
    color: #86868b;
  }

  .plugin-detail-root.light .keyword-chip {
    background: rgba(0, 0, 0, 0.04);
  }

  .plugin-detail-root.dark .keyword-chip {
    background: rgba(255, 255, 255, 0.05);
  }

  /* Hero Stats Grid */
  .hero-stats-grid {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 170px;
  }

  .stat-card {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 14px;
    border-radius: 12px;
    border: 1px solid;
  }

  .plugin-detail-root.light .stat-card {
    background: #fbfbfd;
    border-color: rgba(0, 0, 0, 0.06);
  }

  .plugin-detail-root.dark .stat-card {
    background: #1c1c1e;
    border-color: rgba(255, 255, 255, 0.06);
  }

  .stat-icon {
    width: 32px;
    height: 32px;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .stat-icon.skill {
    background: rgba(175, 82, 222, 0.12);
    color: #af52de;
  }

  .stat-icon.mcp {
    background: rgba(0, 113, 227, 0.12);
    color: #0071e3;
  }

  .stat-icon.security {
    background: rgba(52, 199, 89, 0.12);
    color: #34c759;
  }

  .stat-content {
    display: flex;
    flex-direction: column;
  }

  .stat-num {
    font-size: 14px;
    font-weight: 700;
  }

  .plugin-detail-root.light .stat-num {
    color: #1d1d1f;
  }

  .plugin-detail-root.dark .stat-num {
    color: #ffffff;
  }

  .stat-label {
    font-size: 11px;
    color: #86868b;
  }

  /* Detail Tabs Bar - Apple Native Segmented Control */
  .detail-tabs-bar {
    display: flex;
    margin-bottom: 22px;
    overflow-x: auto;
  }

  .segmented-pill-container {
    display: inline-flex;
    padding: 4px;
    border-radius: 12px;
    gap: 3px;
    box-sizing: border-box;
  }

  .plugin-detail-root.light .segmented-pill-container {
    background: rgba(0, 0, 0, 0.05);
    border: 1px solid rgba(0, 0, 0, 0.06);
  }

  .plugin-detail-root.dark .segmented-pill-container {
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.08);
  }

  .detail-tab-btn {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    border: none;
    border-radius: 9px;
    padding: 8px 16px;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.18s cubic-bezier(0.16, 1, 0.3, 1);
    background: transparent;
    white-space: nowrap;
  }

  .plugin-detail-root.light .detail-tab-btn {
    color: #515154;
  }

  .plugin-detail-root.light .detail-tab-btn:hover {
    color: #1d1d1f;
    background: rgba(0, 0, 0, 0.03);
  }

  .plugin-detail-root.light .detail-tab-btn.active {
    background: #ffffff !important;
    color: #1d1d1f !important;
    font-weight: 600;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08), 0 1px 2px rgba(0, 0, 0, 0.04);
  }

  .plugin-detail-root.dark .detail-tab-btn {
    color: #a1a1a6;
  }

  .plugin-detail-root.dark .detail-tab-btn:hover {
    color: #ffffff;
    background: rgba(255, 255, 255, 0.04);
  }

  .plugin-detail-root.dark .detail-tab-btn.active {
    background: #2c2c2e !important;
    color: #ffffff !important;
    font-weight: 600;
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.4), 0 1px 3px rgba(0, 0, 0, 0.2);
  }

  /* Section Intro & Content Blocks */
  .section-intro {
    margin-bottom: 20px;
    font-size: 13px;
    line-height: 1.5;
    color: #86868b;
  }

  .overview-section-grid {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .content-block {
    border-radius: 14px;
    padding: 20px;
    border: 1px solid;
  }

  .plugin-detail-root.light .content-block {
    background: #ffffff;
    border-color: rgba(0, 0, 0, 0.08);
  }

  .plugin-detail-root.dark .content-block {
    background: #242426;
    border-color: rgba(255, 255, 255, 0.08);
  }

  .block-header {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 12px;
  }

  .block-header :global(.block-icon) {
    color: #0071e3;
  }

  .plugin-detail-root.dark .block-header :global(.block-icon) {
    color: #2997ff;
  }

  .block-title {
    font-size: 16px;
    font-weight: 700;
    margin: 0;
  }

  .plugin-detail-root.light .block-title {
    color: #1d1d1f;
  }

  .plugin-detail-root.dark .block-title {
    color: #ffffff;
  }

  .block-text {
    font-size: 13px;
    line-height: 1.6;
    margin: 0 0 16px;
    color: #86868b;
  }

  .architecture-pills {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 12px;
  }

  .arch-card {
    border-radius: 10px;
    padding: 14px;
    border: 1px solid;
  }

  .plugin-detail-root.light .arch-card {
    background: #fbfbfd;
    border-color: rgba(0, 0, 0, 0.06);
  }

  .plugin-detail-root.dark .arch-card {
    background: #1c1c1e;
    border-color: rgba(255, 255, 255, 0.06);
  }

  .arch-badge {
    display: inline-block;
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    padding: 2px 6px;
    border-radius: 4px;
    margin-bottom: 6px;
  }

  .arch-badge.mcp {
    background: rgba(0, 113, 227, 0.12);
    color: #0071e3;
  }

  .arch-badge.skill {
    background: rgba(175, 82, 222, 0.12);
    color: #af52de;
  }

  .arch-badge.sandboxed {
    background: rgba(52, 199, 89, 0.12);
    color: #34c759;
  }

  .arch-card h4 {
    font-size: 13px;
    font-weight: 600;
    margin: 0 0 4px;
  }

  .plugin-detail-root.light .arch-card h4 {
    color: #1d1d1f;
  }

  .plugin-detail-root.dark .arch-card h4 {
    color: #ffffff;
  }

  .arch-card p {
    font-size: 12px;
    line-height: 1.45;
    margin: 0;
    color: #86868b;
  }

  /* Specs Table */
  .specs-table {
    display: flex;
    flex-direction: column;
    border-radius: 10px;
    overflow: hidden;
    border: 1px solid;
  }

  .plugin-detail-root.light .specs-table {
    border-color: rgba(0, 0, 0, 0.08);
  }

  .plugin-detail-root.dark .specs-table {
    border-color: rgba(255, 255, 255, 0.08);
  }

  .spec-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px 14px;
    font-size: 12px;
    border-bottom: 1px solid;
  }

  .spec-row:last-child {
    border-bottom: none;
  }

  .plugin-detail-root.light .spec-row {
    background: #ffffff;
    border-bottom-color: rgba(0, 0, 0, 0.06);
  }

  .plugin-detail-root.light .spec-row:nth-child(even) {
    background: #fafafc;
  }

  .plugin-detail-root.dark .spec-row {
    background: #242426;
    border-bottom-color: rgba(255, 255, 255, 0.06);
  }

  .plugin-detail-root.dark .spec-row:nth-child(even) {
    background: #1e1e20;
  }

  .spec-label {
    color: #86868b;
    font-weight: 500;
  }

  .spec-value {
    font-weight: 500;
  }

  .plugin-detail-root.light .spec-value {
    color: #1d1d1f;
  }

  .plugin-detail-root.dark .spec-value {
    color: #ffffff;
  }

  .source-spec {
    text-transform: capitalize;
  }

  .spec-value-copy {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .spec-value-copy code {
    font-family: monospace;
    font-size: 11px;
    padding: 2px 6px;
    border-radius: 4px;
  }

  .path-code {
    max-width: 380px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .plugin-detail-root.light .spec-value-copy code {
    background: rgba(0, 0, 0, 0.05);
    color: #1d1d1f;
  }

  .plugin-detail-root.dark .spec-value-copy code {
    background: rgba(255, 255, 255, 0.06);
    color: #f5f5f7;
  }

  .mini-copy-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: 4px;
    background: transparent;
    border: none;
    color: #86868b;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .mini-copy-btn:hover {
    color: #0071e3;
  }

  :global(.green-text) {
    color: #34c759 !important;
  }

  .status-pill {
    font-size: 11px;
    font-weight: 600;
    padding: 2px 8px;
    border-radius: 6px;
  }

  .status-pill.active {
    background: rgba(52, 199, 89, 0.12);
    color: #34c759;
  }

  .status-pill.inactive {
    background: rgba(142, 142, 147, 0.12);
    color: #8e8e93;
  }

  .status-pill.available {
    background: rgba(0, 113, 227, 0.12);
    color: #0071e3;
  }

  .status-pill.error {
    background: rgba(255, 69, 58, 0.12);
    color: #ff453a;
  }

  .link-url {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: #0071e3;
    text-decoration: none;
    font-size: 12px;
  }

  .link-url:hover {
    text-decoration: underline;
  }

  /* Skills Grid */
  .skills-items-grid {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .skill-detail-card {
    border-radius: 12px;
    padding: 16px;
    border: 1px solid;
    transition: all 0.2s ease;
  }

  .plugin-detail-root.light .skill-detail-card {
    background: #ffffff;
    border-color: rgba(0, 0, 0, 0.08);
  }

  .plugin-detail-root.dark .skill-detail-card {
    background: #242426;
    border-color: rgba(255, 255, 255, 0.08);
  }

  .skill-card-top {
    display: flex;
    align-items: flex-start;
    gap: 14px;
  }

  .skill-avatar {
    width: 38px;
    height: 38px;
    border-radius: 10px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 18px;
    flex-shrink: 0;
  }

  .plugin-detail-root.light .skill-avatar {
    background: rgba(175, 82, 222, 0.1);
  }

  .plugin-detail-root.dark .skill-avatar {
    background: rgba(191, 90, 242, 0.15);
  }

  .skill-main-info {
    flex: 1;
  }

  .skill-name-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 4px;
    flex-wrap: wrap;
  }

  .skill-title {
    font-size: 15px;
    font-weight: 600;
    margin: 0;
  }

  .plugin-detail-root.light .skill-title {
    color: #1d1d1f;
  }

  .plugin-detail-root.dark .skill-title {
    color: #ffffff;
  }

  .skill-id-copy {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }

  .skill-id-pill {
    font-size: 11px;
    color: #86868b;
    padding: 2px 6px;
    border-radius: 4px;
    background: rgba(0, 0, 0, 0.04);
  }

  .plugin-detail-root.dark .skill-id-pill {
    background: rgba(255, 255, 255, 0.06);
  }

  .skill-badge-script {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 10px;
    font-weight: 600;
    padding: 2px 6px;
    border-radius: 4px;
    background: rgba(0, 113, 227, 0.08);
    color: #0071e3;
  }

  .plugin-detail-root.dark .skill-badge-script {
    background: rgba(41, 151, 255, 0.12);
    color: #2997ff;
  }

  .skill-desc {
    font-size: 13px;
    line-height: 1.45;
    margin: 0;
    color: #86868b;
  }

  .skill-tags-row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 10px;
  }

  .skill-tag {
    font-size: 11px;
    padding: 2px 6px;
    border-radius: 4px;
    color: #af52de;
    background: rgba(175, 82, 222, 0.08);
  }

  .skill-actions-bar {
    margin-top: 14px;
    padding-top: 12px;
    border-top: 1px solid;
  }

  .plugin-detail-root.light .skill-actions-bar {
    border-top-color: rgba(0, 0, 0, 0.06);
  }

  .plugin-detail-root.dark .skill-actions-bar {
    border-top-color: rgba(255, 255, 255, 0.06);
  }

  .skill-disabled-warning {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    color: #ff9500;
    background: rgba(255, 149, 0, 0.1);
    border: 1px solid rgba(255, 149, 0, 0.2);
    border-radius: 6px;
    padding: 6px 10px;
    margin-bottom: 8px;
  }

  .skill-test-wrapper {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .skill-test-input-row {
    display: flex;
    gap: 8px;
  }

  .skill-test-input {
    flex: 1;
    border-radius: 8px;
    padding: 7px 12px;
    font-size: 12px;
    outline: none;
  }

  .plugin-detail-root.light .skill-test-input {
    border: 1px solid rgba(0, 0, 0, 0.12);
    background: #fafafc;
    color: #1d1d1f;
  }

  .plugin-detail-root.dark .skill-test-input {
    border: 1px solid rgba(255, 255, 255, 0.1);
    background: rgba(255, 255, 255, 0.04);
    color: #ffffff;
  }

  .skill-run-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border-radius: 8px;
    padding: 7px 14px;
    font-size: 12px;
    font-weight: 600;
    border: none;
    cursor: pointer;
    background: #af52de;
    color: #ffffff;
    transition: all 0.2s ease;
  }

  .skill-run-btn:hover {
    background: #9d40ce;
  }

  .test-feedback-box {
    border-radius: 8px;
    padding: 10px 14px;
    font-size: 12px;
    border: 1px solid;
  }

  .test-feedback-box.success {
    background: rgba(52, 199, 89, 0.08);
    border-color: rgba(52, 199, 89, 0.25);
    color: #34c759;
  }

  .test-feedback-box.error {
    background: rgba(255, 69, 58, 0.08);
    border-color: rgba(255, 69, 58, 0.25);
    color: #ff453a;
  }

  .feedback-header {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 6px;
  }

  .feedback-error-msg {
    margin: 0;
    font-size: 12px;
    line-height: 1.4;
    word-break: break-word;
  }

  .test-pre {
    margin: 0;
    font-family: monospace;
    font-size: 11px;
    padding: 8px;
    border-radius: 6px;
    background: rgba(0, 0, 0, 0.04);
    overflow-x: auto;
  }

  .plugin-detail-root.dark .test-pre {
    background: rgba(0, 0, 0, 0.3);
  }

  .skill-offline-hint {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: #86868b;
  }

  /* MCP Servers List */
  .mcp-servers-list {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .mcp-server-card {
    border-radius: 14px;
    padding: 18px;
    border: 1px solid;
  }

  .plugin-detail-root.light .mcp-server-card {
    background: #ffffff;
    border-color: rgba(0, 0, 0, 0.08);
  }

  .plugin-detail-root.dark .mcp-server-card {
    background: #242426;
    border-color: rgba(255, 255, 255, 0.08);
  }

  .mcp-card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 16px;
    flex-wrap: wrap;
  }

  .mcp-title-group {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .mcp-icon-box {
    width: 40px;
    height: 40px;
    border-radius: 10px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 113, 227, 0.1);
    color: #0071e3;
  }

  .plugin-detail-root.dark .mcp-icon-box {
    background: rgba(41, 151, 255, 0.15);
    color: #2997ff;
  }

  .mcp-name-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .mcp-server-name {
    font-size: 16px;
    font-weight: 700;
    margin: 0;
  }

  .plugin-detail-root.light .mcp-server-name {
    color: #1d1d1f;
  }

  .plugin-detail-root.dark .mcp-server-name {
    color: #ffffff;
  }

  .transport-tag {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    padding: 2px 6px;
    border-radius: 4px;
    background: rgba(0, 113, 227, 0.12);
    color: #0071e3;
  }

  .mcp-status-badge {
    font-size: 11px;
    font-weight: 600;
    padding: 2px 8px;
    border-radius: 6px;
  }

  .mcp-status-badge.ready {
    background: rgba(52, 199, 89, 0.12);
    color: #34c759;
  }

  .mcp-status-badge.idle {
    background: rgba(142, 142, 147, 0.12);
    color: #8e8e93;
  }

  .mcp-sub-info {
    font-size: 12px;
    color: #86868b;
    margin-top: 3px;
    display: block;
  }

  .mcp-sub-info code {
    font-family: monospace;
    font-size: 11px;
    padding: 1px 5px;
    border-radius: 4px;
    background: rgba(0, 0, 0, 0.04);
  }

  .plugin-detail-root.dark .mcp-sub-info code {
    background: rgba(255, 255, 255, 0.06);
  }

  .mcp-test-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border-radius: 8px;
    padding: 8px 16px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    border: none;
    background: #0071e3;
    color: #ffffff;
    transition: all 0.2s ease;
  }

  .mcp-test-btn:hover {
    background: #0077ed;
  }

  .mcp-install-cta {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border-radius: 8px;
    padding: 7px 14px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    border: 1px solid #0071e3;
    background: rgba(0, 113, 227, 0.08);
    color: #0071e3;
  }

  /* MCP Test Box */
  .mcp-test-box {
    margin-top: 16px;
    padding-top: 16px;
    border-top: 1px solid;
  }

  .plugin-detail-root.light .mcp-test-box {
    border-top-color: rgba(0, 0, 0, 0.06);
  }

  .plugin-detail-root.dark .mcp-test-box {
    border-top-color: rgba(255, 255, 255, 0.06);
  }

  .test-banner {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 12px 16px;
    border-radius: 10px;
    font-size: 13px;
    margin-bottom: 12px;
  }

  .test-banner.success {
    background: rgba(52, 199, 89, 0.1);
    border: 1px solid rgba(52, 199, 89, 0.25);
    color: #34c759;
  }

  .test-banner.error {
    background: rgba(255, 69, 58, 0.1);
    border: 1px solid rgba(255, 69, 58, 0.25);
    color: #ff453a;
  }

  .no-tools-notice {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: #86868b;
    padding: 8px 12px;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.04);
  }

  .plugin-detail-root.dark .no-tools-notice {
    background: rgba(255, 255, 255, 0.04);
  }

  .tools-discovered-list {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .tool-item-card {
    border-radius: 10px;
    padding: 12px 14px;
    border: 1px solid;
  }

  .plugin-detail-root.light .tool-item-card {
    background: #fbfbfd;
    border-color: rgba(0, 0, 0, 0.06);
  }

  .plugin-detail-root.dark .tool-item-card {
    background: #1c1c1e;
    border-color: rgba(255, 255, 255, 0.06);
  }

  .tool-item-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .tool-title-group {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  :global(.zap-icon) {
    color: #ff9500;
  }

  .tool-name {
    font-size: 13px;
    font-weight: 700;
  }

  .plugin-detail-root.light .tool-name {
    color: #1d1d1f;
  }

  .plugin-detail-root.dark .tool-name {
    color: #ffffff;
  }

  .tool-run-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    border-radius: 6px;
    padding: 4px 10px;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    border: 1px solid rgba(0, 113, 227, 0.3);
    background: rgba(0, 113, 227, 0.08);
    color: #0071e3;
    transition: all 0.2s ease;
  }

  .tool-run-btn:hover {
    background: #0071e3;
    color: #ffffff;
  }

  .tool-desc {
    font-size: 12px;
    line-height: 1.4;
    color: #86868b;
    margin: 6px 0 10px;
  }

  .tool-call-sub {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .tool-args-input {
    border-radius: 6px;
    padding: 6px 10px;
    font-size: 12px;
    font-family: monospace;
    outline: none;
  }

  .plugin-detail-root.light .tool-args-input {
    border: 1px solid rgba(0, 0, 0, 0.12);
    background: #ffffff;
    color: #1d1d1f;
  }

  .plugin-detail-root.dark .tool-args-input {
    border: 1px solid rgba(255, 255, 255, 0.1);
    background: rgba(0, 0, 0, 0.3);
    color: #ffffff;
  }

  .tool-res-box {
    border-radius: 6px;
    padding: 8px 10px;
    font-size: 11px;
  }

  .tool-res-box.success {
    background: rgba(52, 199, 89, 0.08);
    color: #34c759;
  }

  .tool-res-box.error {
    background: rgba(255, 69, 58, 0.08);
    color: #ff453a;
  }

  .tool-pre {
    margin: 0;
    font-family: monospace;
    font-size: 11px;
    overflow-x: auto;
  }

  /* Security Cards Grid */
  .security-cards-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
    gap: 16px;
  }

  .sec-card {
    border-radius: 14px;
    padding: 20px;
    border: 1px solid;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
  }

  .plugin-detail-root.light .sec-card {
    background: #ffffff;
    border-color: rgba(0, 0, 0, 0.08);
  }

  .plugin-detail-root.dark .sec-card {
    background: #242426;
    border-color: rgba(255, 255, 255, 0.08);
  }

  .sec-card-header {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 12px;
  }

  .sec-icon {
    width: 38px;
    height: 38px;
    border-radius: 10px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .sec-icon.green {
    background: rgba(52, 199, 89, 0.12);
    color: #34c759;
  }

  .sec-icon.blue {
    background: rgba(0, 113, 227, 0.12);
    color: #0071e3;
  }

  .sec-icon.purple {
    background: rgba(175, 82, 222, 0.12);
    color: #af52de;
  }

  .sec-icon.orange {
    background: rgba(255, 149, 0, 0.12);
    color: #ff9500;
  }

  .sec-title {
    font-size: 15px;
    font-weight: 700;
    margin: 0;
  }

  .plugin-detail-root.light .sec-title {
    color: #1d1d1f;
  }

  .plugin-detail-root.dark .sec-title {
    color: #ffffff;
  }

  .sec-subtitle {
    font-size: 11px;
    color: #86868b;
  }

  .sec-body {
    font-size: 13px;
    line-height: 1.5;
    color: #86868b;
    margin: 0 0 16px;
  }

  .sec-body code {
    font-family: monospace;
    font-size: 11px;
    padding: 1px 4px;
    border-radius: 4px;
    background: rgba(0, 0, 0, 0.05);
  }

  .plugin-detail-root.dark .sec-body code {
    background: rgba(255, 255, 255, 0.08);
  }

  .sec-tag-row {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }

  .sec-pill {
    font-size: 11px;
    padding: 3px 8px;
    border-radius: 6px;
    font-weight: 500;
  }

  .plugin-detail-root.light .sec-pill {
    background: rgba(0, 0, 0, 0.04);
    color: #515154;
  }

  .plugin-detail-root.dark .sec-pill {
    background: rgba(255, 255, 255, 0.06);
    color: #a1a1a6;
  }

  /* Manifest Tab */
  .manifest-tab-view {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .manifest-toolbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px 16px;
    border-radius: 10px;
    border: 1px solid;
  }

  .plugin-detail-root.light .manifest-toolbar {
    background: #ffffff;
    border-color: rgba(0, 0, 0, 0.08);
  }

  .plugin-detail-root.dark .manifest-toolbar {
    background: #242426;
    border-color: rgba(255, 255, 255, 0.08);
  }

  .manifest-info-left {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    font-weight: 600;
  }

  .manifest-size-badge {
    font-size: 11px;
    color: #86868b;
    font-weight: 400;
  }

  .copy-manifest-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border-radius: 8px;
    padding: 6px 12px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .plugin-detail-root.light .copy-manifest-btn {
    background: rgba(0, 0, 0, 0.05);
    border: 1px solid rgba(0, 0, 0, 0.08);
    color: #1d1d1f;
  }

  .plugin-detail-root.light .copy-manifest-btn:hover {
    background: rgba(0, 0, 0, 0.08);
  }

  .plugin-detail-root.dark .copy-manifest-btn {
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #ffffff;
  }

  .plugin-detail-root.dark .copy-manifest-btn:hover {
    background: rgba(255, 255, 255, 0.12);
  }

  .manifest-code-wrapper {
    border-radius: 12px;
    overflow: hidden;
    border: 1px solid;
  }

  .plugin-detail-root.light .manifest-code-wrapper {
    background: #fbfbfd;
    border-color: rgba(0, 0, 0, 0.08);
  }

  .plugin-detail-root.dark .manifest-code-wrapper {
    background: #18181a;
    border-color: rgba(255, 255, 255, 0.08);
  }

  .manifest-code {
    margin: 0;
    padding: 20px;
    font-family: "SFMono-Regular", Consolas, "Liberation Mono", Menlo, Courier, monospace;
    font-size: 12px;
    line-height: 1.6;
    overflow-x: auto;
    color: #0071e3;
  }

  .plugin-detail-root.dark .manifest-code {
    color: #79c0ff;
  }

  /* Empty Tab State */
  .empty-tab-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    padding: 60px 20px;
    color: #86868b;
  }

  :global(.faded-icon) {
    opacity: 0.3;
    margin-bottom: 12px;
  }

  .empty-tab-state h4 {
    font-size: 16px;
    font-weight: 600;
    margin: 0 0 6px;
  }

  .plugin-detail-root.light .empty-tab-state h4 {
    color: #1d1d1f;
  }

  .plugin-detail-root.dark .empty-tab-state h4 {
    color: #ffffff;
  }

  .empty-tab-state p {
    font-size: 13px;
    margin: 0;
  }

  /* iOS Switch */
  .ios-switch {
    position: relative;
    display: inline-block;
    width: 36px;
    height: 20px;
  }

  .ios-switch input {
    opacity: 0;
    width: 0;
    height: 0;
  }

  .switch-track {
    position: absolute;
    cursor: pointer;
    inset: 0;
    transition: 0.22s ease;
    border-radius: 20px;
  }

  .plugin-detail-root.light .switch-track {
    background-color: #e5e5ea;
  }

  .plugin-detail-root.dark .switch-track {
    background-color: #3a3a3c;
  }

  .switch-track:before {
    position: absolute;
    content: "";
    height: 16px;
    width: 16px;
    left: 2px;
    bottom: 2px;
    background-color: white;
    transition: 0.22s ease;
    border-radius: 50%;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.25);
  }

  input:checked + .switch-track {
    background-color: #34c759 !important;
  }

  input:checked + .switch-track:before {
    transform: translateX(16px);
  }

  :global(.spin) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from {
      transform: rotate(0deg);
    }
    to {
      transform: rotate(360deg);
    }
  }

  /* ==========================================================================
     ACCOUNTS TAB & MODAL STYLES
     ========================================================================== */

  .accounts-tab-view {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .accounts-toolbar {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 20px;
    padding-bottom: 16px;
    border-bottom: 1px solid;
    flex-wrap: wrap;
  }

  .plugin-detail-root.light .accounts-toolbar {
    border-color: rgba(0, 0, 0, 0.08);
  }

  .plugin-detail-root.dark .accounts-toolbar {
    border-color: rgba(255, 255, 255, 0.08);
  }

  .accounts-toolbar-info {
    flex: 1;
    min-width: 260px;
  }

  .accounts-toolbar-title {
    font-size: 16px;
    font-weight: 700;
    margin: 0 0 6px;
  }

  .plugin-detail-root.light .accounts-toolbar-title {
    color: #1d1d1f;
  }

  .plugin-detail-root.dark .accounts-toolbar-title {
    color: #ffffff;
  }

  .accounts-toolbar-desc {
    font-size: 13px;
    line-height: 1.5;
    color: #86868b;
    margin: 0;
  }

  .add-account-btn {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 8px 16px;
    border-radius: 980px;
    font-size: 13px;
    font-weight: 600;
    border: none;
    cursor: pointer;
    background: #0071e3;
    color: #ffffff;
    transition: all 0.2s ease;
    white-space: nowrap;
    box-shadow: 0 2px 6px rgba(0, 113, 227, 0.25);
  }

  .add-account-btn:hover {
    background: #0077ed;
    transform: translateY(-1px);
    box-shadow: 0 4px 10px rgba(0, 113, 227, 0.35);
  }

  .add-account-btn.mt-4 {
    margin-top: 16px;
  }

  .accounts-loading-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    padding: 60px 20px;
    color: #86868b;
    font-size: 14px;
  }

  /* Accounts Grid & Cards */
  .accounts-grid {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .account-card {
    border-radius: 14px;
    padding: 18px 20px;
    border: 1px solid;
    display: flex;
    flex-direction: column;
    gap: 14px;
    transition: all 0.2s ease;
  }

  .plugin-detail-root.light .account-card {
    background: #ffffff;
    border-color: rgba(0, 0, 0, 0.08);
  }

  .plugin-detail-root.dark .account-card {
    background: #242426;
    border-color: rgba(255, 255, 255, 0.08);
  }

  .account-card.is-default {
    border-color: rgba(0, 113, 227, 0.4);
    box-shadow: 0 2px 12px rgba(0, 113, 227, 0.08);
  }

  .plugin-detail-root.dark .account-card.is-default {
    border-color: rgba(41, 151, 255, 0.4);
    box-shadow: 0 2px 12px rgba(41, 151, 255, 0.1);
  }

  .account-card-header {
    display: flex;
    align-items: flex-start;
    gap: 16px;
  }

  .account-avatar-wrapper {
    flex-shrink: 0;
  }

  .account-avatar {
    width: 42px;
    height: 42px;
    border-radius: 50%;
    object-fit: cover;
    border: 1px solid rgba(0, 0, 0, 0.1);
  }

  .account-avatar-placeholder {
    width: 42px;
    height: 42px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .account-avatar-placeholder.oauth {
    background: rgba(0, 113, 227, 0.12);
    color: #0071e3;
  }

  .plugin-detail-root.dark .account-avatar-placeholder.oauth {
    background: rgba(41, 151, 255, 0.18);
    color: #2997ff;
  }

  .account-avatar-placeholder.api {
    background: rgba(175, 82, 222, 0.12);
    color: #af52de;
  }

  .plugin-detail-root.dark .account-avatar-placeholder.api {
    background: rgba(175, 82, 222, 0.2);
    color: #d084ff;
  }

  .account-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .account-title-row {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }

  .account-label-text {
    font-size: 15px;
    font-weight: 700;
    margin: 0;
  }

  .plugin-detail-root.light .account-label-text {
    color: #1d1d1f;
  }

  .plugin-detail-root.dark .account-label-text {
    color: #ffffff;
  }

  .account-edit-label-btn {
    background: none;
    border: none;
    cursor: pointer;
    color: #86868b;
    padding: 3px;
    border-radius: 4px;
    display: flex;
    align-items: center;
    transition: color 0.15s;
  }

  .account-edit-label-btn:hover {
    color: #0071e3;
  }

  .account-inline-edit {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .account-edit-input {
    font-size: 13px;
    padding: 4px 8px;
    border-radius: 6px;
    border: 1px solid #0071e3;
    outline: none;
    background: transparent;
    color: inherit;
  }

  .account-save-btn, .account-cancel-btn {
    border: none;
    border-radius: 4px;
    padding: 4px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .account-save-btn {
    background: #34c759;
    color: #ffffff;
  }

  .account-cancel-btn {
    background: rgba(134, 134, 139, 0.2);
    color: inherit;
  }

  .default-badge {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    font-weight: 600;
    padding: 2px 8px;
    border-radius: 980px;
    background: rgba(255, 149, 0, 0.15);
    color: #ff9500;
  }

  .plugin-detail-root.dark .default-badge {
    background: rgba(255, 159, 10, 0.2);
    color: #ff9f0a;
  }

  .account-meta-row {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    flex-wrap: wrap;
  }

  .account-identifier {
    color: #86868b;
    font-family: monospace;
  }

  .account-method-pill {
    padding: 2px 7px;
    border-radius: 4px;
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    background: rgba(0, 113, 227, 0.08);
    color: #0071e3;
  }

  .plugin-detail-root.dark .account-method-pill {
    background: rgba(41, 151, 255, 0.14);
    color: #2997ff;
  }

  .account-status-pill {
    padding: 2px 7px;
    border-radius: 4px;
    font-size: 10px;
    font-weight: 600;
  }

  .account-status-pill.active {
    background: rgba(52, 199, 89, 0.12);
    color: #34c759;
  }

  .account-status-pill.expired, .account-status-pill.revoked, .account-status-pill.error {
    background: rgba(255, 59, 48, 0.12);
    color: #ff3b30;
  }

  .account-health-banner {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    margin-top: 4px;
    padding: 4px 8px;
    border-radius: 6px;
  }

  .account-health-banner.healthy {
    background: rgba(52, 199, 89, 0.1);
    color: #34c759;
  }

  .account-health-banner.unhealthy {
    background: rgba(255, 59, 48, 0.1);
    color: #ff3b30;
  }

  .account-card-footer {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding-top: 10px;
    border-top: 1px solid;
    gap: 12px;
    flex-wrap: wrap;
  }

  .plugin-detail-root.light .account-card-footer {
    border-color: rgba(0, 0, 0, 0.05);
  }

  .plugin-detail-root.dark .account-card-footer {
    border-color: rgba(255, 255, 255, 0.05);
  }

  .account-timestamps {
    font-size: 11px;
    color: #86868b;
  }

  .account-actions-group {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .account-action-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 5px 10px;
    border-radius: 6px;
    font-size: 12px;
    font-weight: 500;
    border: none;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .account-action-btn.health {
    background: rgba(0, 113, 227, 0.08);
    color: #0071e3;
  }

  .account-action-btn.health:hover {
    background: rgba(0, 113, 227, 0.15);
  }

  .plugin-detail-root.dark .account-action-btn.health {
    background: rgba(41, 151, 255, 0.12);
    color: #2997ff;
  }

  .account-action-btn.default {
    background: rgba(255, 149, 0, 0.1);
    color: #ff9500;
  }

  .account-action-btn.default:hover {
    background: rgba(255, 149, 0, 0.18);
  }

  .account-action-btn.disconnect {
    background: rgba(255, 59, 48, 0.08);
    color: #ff3b30;
  }

  .account-action-btn.disconnect:hover {
    background: rgba(255, 59, 48, 0.16);
  }

  /* Modal Styles */
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    backdrop-filter: blur(8px);
    -webkit-backdrop-filter: blur(8px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    padding: 20px;
    box-sizing: border-box;
  }

  .modal-card {
    width: 100%;
    max-width: 480px;
    border-radius: 16px;
    border: 1px solid;
    display: flex;
    flex-direction: column;
    box-shadow: 0 20px 40px rgba(0, 0, 0, 0.25);
    overflow: hidden;
  }

  .plugin-detail-root.light .modal-card {
    background: #ffffff;
    border-color: rgba(0, 0, 0, 0.1);
  }

  .plugin-detail-root.dark .modal-card {
    background: #1c1c1e;
    border-color: rgba(255, 255, 255, 0.1);
  }

  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px 20px;
    border-bottom: 1px solid;
  }

  .plugin-detail-root.light .modal-header {
    border-color: rgba(0, 0, 0, 0.08);
  }

  .plugin-detail-root.dark .modal-header {
    border-color: rgba(255, 255, 255, 0.08);
  }

  .modal-header-left {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .modal-icon-badge {
    width: 36px;
    height: 36px;
    border-radius: 10px;
    background: rgba(0, 113, 227, 0.12);
    color: #0071e3;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .plugin-detail-root.dark .modal-icon-badge {
    background: rgba(41, 151, 255, 0.15);
    color: #2997ff;
  }

  .modal-title {
    font-size: 16px;
    font-weight: 700;
    margin: 0;
  }

  .plugin-detail-root.light .modal-title {
    color: #1d1d1f;
  }

  .plugin-detail-root.dark .modal-title {
    color: #ffffff;
  }

  .modal-subtitle {
    font-size: 11px;
    color: #86868b;
    margin: 2px 0 0;
  }

  .modal-close-btn {
    background: none;
    border: none;
    cursor: pointer;
    color: #86868b;
    padding: 6px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.15s;
  }

  .modal-close-btn:hover {
    background: rgba(134, 134, 139, 0.15);
    color: inherit;
  }

  .modal-body {
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .method-selector {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .method-selector-label {
    font-size: 12px;
    font-weight: 600;
    color: #86868b;
  }

  .method-tabs {
    display: flex;
    gap: 6px;
    background: rgba(134, 134, 139, 0.08);
    padding: 4px;
    border-radius: 8px;
  }

  .method-tab-btn {
    flex: 1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 6px 10px;
    border: none;
    border-radius: 6px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    background: transparent;
    color: #86868b;
    transition: all 0.15s ease;
  }

  .method-tab-btn.active {
    background: #ffffff;
    color: #1d1d1f;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.12);
  }

  .plugin-detail-root.dark .method-tab-btn.active {
    background: #2c2c2e;
    color: #ffffff;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
  }

  .info-alert {
    display: flex;
    gap: 12px;
    padding: 12px;
    border-radius: 10px;
    background: rgba(0, 113, 227, 0.08);
    border: 1px solid rgba(0, 113, 227, 0.2);
    font-size: 12px;
    line-height: 1.4;
  }

  .info-alert-icon {
    color: #0071e3;
    flex-shrink: 0;
    margin-top: 1px;
  }

  .plugin-detail-root.dark .info-alert {
    background: rgba(41, 151, 255, 0.1);
    border-color: rgba(41, 151, 255, 0.2);
  }

  .plugin-detail-root.dark .info-alert-icon {
    color: #2997ff;
  }

  .info-alert-text p {
    margin: 0 0 6px;
  }

  .scope-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    align-items: center;
  }

  .scope-tag-title {
    font-size: 11px;
    font-weight: 600;
    color: #86868b;
  }

  .scope-tag {
    font-size: 10px;
    font-family: monospace;
    padding: 2px 6px;
    border-radius: 4px;
    background: rgba(0, 113, 227, 0.12);
    color: #0071e3;
  }

  .plugin-detail-root.dark .scope-tag {
    background: rgba(41, 151, 255, 0.18);
    color: #2997ff;
  }

  .section-prompt-text {
    font-size: 13px;
    line-height: 1.5;
    color: #86868b;
    margin: 0;
  }

  .doc-link-hint {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 12px;
    color: #0071e3;
    text-decoration: none;
    margin-bottom: 4px;
  }

  .doc-link-hint:hover {
    text-decoration: underline;
  }

  .form-group {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .form-label {
    font-size: 12px;
    font-weight: 600;
  }

  .plugin-detail-root.light .form-label {
    color: #1d1d1f;
  }

  .plugin-detail-root.dark .form-label {
    color: #f5f5f7;
  }

  .form-input {
    font-size: 13px;
    padding: 8px 12px;
    border-radius: 8px;
    border: 1px solid;
    outline: none;
    box-sizing: border-box;
    width: 100%;
    transition: border-color 0.15s ease;
  }

  .plugin-detail-root.light .form-input {
    background: #f5f5f7;
    border-color: rgba(0, 0, 0, 0.12);
    color: #1d1d1f;
  }

  .plugin-detail-root.light .form-input:focus {
    border-color: #0071e3;
    background: #ffffff;
  }

  .plugin-detail-root.dark .form-input {
    background: #2c2c2e;
    border-color: rgba(255, 255, 255, 0.12);
    color: #ffffff;
  }

  .plugin-detail-root.dark .form-input:focus {
    border-color: #2997ff;
    background: #1c1c1e;
  }

  .form-error-alert {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border-radius: 8px;
    background: rgba(255, 59, 48, 0.12);
    color: #ff3b30;
    font-size: 12px;
  }

  .form-success-alert {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border-radius: 8px;
    background: rgba(52, 199, 89, 0.12);
    color: #34c759;
    font-size: 12px;
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    margin-top: 10px;
  }

  .btn-secondary {
    padding: 8px 16px;
    border-radius: 8px;
    font-size: 13px;
    font-weight: 500;
    border: none;
    cursor: pointer;
    background: rgba(134, 134, 139, 0.12);
    color: inherit;
    transition: background 0.15s ease;
  }

  .btn-secondary:hover {
    background: rgba(134, 134, 139, 0.2);
  }

  .btn-primary {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 8px 18px;
    border-radius: 8px;
    font-size: 13px;
    font-weight: 600;
    border: none;
    cursor: pointer;
    background: #0071e3;
    color: #ffffff;
    transition: all 0.15s ease;
  }

  .btn-primary:hover:not(:disabled) {
    background: #0077ed;
  }

  .btn-primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
