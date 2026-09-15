<script lang="ts">
  import { onMount } from "svelte";
  import Check from "@lucide/svelte/icons/check";
  import Download from "@lucide/svelte/icons/download";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import Folder from "@lucide/svelte/icons/folder";
  import GitBranch from "@lucide/svelte/icons/git-branch";
  import Info from "@lucide/svelte/icons/info";
  import Loader2 from "@lucide/svelte/icons/loader-2";
  import Play from "@lucide/svelte/icons/play";
  import Plus from "@lucide/svelte/icons/plus";
  import Puzzle from "@lucide/svelte/icons/puzzle";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import Search from "@lucide/svelte/icons/search";
  import Server from "@lucide/svelte/icons/server";
  import Sparkles from "@lucide/svelte/icons/sparkles";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import Wrench from "@lucide/svelte/icons/wrench";
  import X from "@lucide/svelte/icons/x";
  import Globe from "@lucide/svelte/icons/globe";
  import Key from "@lucide/svelte/icons/key";
  import Shield from "@lucide/svelte/icons/shield";
  import ShieldCheck from "@lucide/svelte/icons/shield-check";
  import Users from "@lucide/svelte/icons/users";
  import { fade, fly } from "svelte/transition";
  import { requestConfirm } from "../../../lib/confirm";
  import PluginDetailView from "./PluginDetailView.svelte";

  import {
    listInstalledPlugins,
    listMarketplacePlugins,
    installPlugin,
    createCustomPlugin,
    togglePlugin,
    uninstallPlugin,
    testPluginMcpServer,
    startPluginOAuthConnect,
    connectPluginApiKey,
    CURATED_MARKETPLACE,
    type InstalledPlugin,
    type MarketplacePlugin,
    type CreateCustomPluginRequest,
  } from "../../../lib/api";

  // Theme & Compatibility props
  export let currentTheme: "light" | "dark" = "light";
  export let theme: "light" | "dark" | undefined = undefined;

  $: isDark = (theme || currentTheme) === "dark";

  // State
  let activeTab: "marketplace" | "installed" = "marketplace";
  let loading = true;
  let refreshing = false;
  let actionLoading: Record<string, boolean> = {};
  let errorMessage: string | null = null;
  let successMessage: string | null = null;

  let installedPlugins: InstalledPlugin[] = [];
  // Initialize with curated catalog immediately so it never flashes empty
  let marketplacePlugins: MarketplacePlugin[] = CURATED_MARKETPLACE;

  // Filter & Search
  let searchQuery = "";
  let selectedCategory = "Tous";
  const categories = [
    "Tous",
    "Filesystem",
    "Development",
    "Web & Research",
    "Database",
    "Data & Code",
    "Code Quality",
    "Productivity",
    "AI & Models",
    "Communication",
  ];

  // Dedicated Plugin Details View
  let selectedPluginId: string | null = null;

  function handleSelectCategoryFromDetail(cat: string) {
    selectedCategory = cat;
    activeTab = "marketplace";
    selectedPluginId = null;
  }

  // Add Plugin Modal
  let showAddModal = false;
  let addMethod: "marketplace" | "git" | "local" | "custom" = "marketplace";
  let customSourceTarget = "";
  let customSkillInstructions = "Step 1: Analyze user request.\nStep 2: Execute task.\nStep 3: Report results cleanly.";
  let customPluginForm: CreateCustomPluginRequest = {
    name: "",
    version: "1.0.0",
    description: "",
    author: "ARO User",
    license: "MIT",
    keywords: [],
    skills: [],
    mcpServers: {
      "default-server": {
        type: "stdio",
        transportType: "stdio",
        command: "cmd",
        args: ["/c", "echo", "MCP Server ready at ${PLUGIN_ROOT}"],
        env: {},
        cwd: "${PLUGIN_ROOT}",
        headers: {},
      },
    },
  };

  async function loadData() {
    loading = true;
    errorMessage = null;
    try {
      const [installed, market] = await Promise.all([
        listInstalledPlugins(),
        listMarketplacePlugins(),
      ]);
      installedPlugins = installed;
      const installedIds = new Set(installed.map((p) => p.id));
      const sourceList = Array.isArray(market) && market.length > 0 ? market : CURATED_MARKETPLACE;
      marketplacePlugins = sourceList.map((m) => ({
        ...m,
        installed: installedIds.has(m.id),
      }));
    } catch (err: any) {
      console.warn("Failed to load plugins from API, using fallback:", err);
      errorMessage = err?.message || "Impossible de contacter l'API des plugins.";
      const installedIds = new Set(installedPlugins.map((p) => p.id));
      marketplacePlugins = CURATED_MARKETPLACE.map((m) => ({
        ...m,
        installed: installedIds.has(m.id),
      }));
    } finally {
      loading = false;
      refreshing = false;
    }
  }

  async function refresh() {
    refreshing = true;
    await loadData();
  }

  onMount(() => {
    loadData();
  });

  // Guided Auth Installation State
  let showGuidedAuthModal = false;
  let guidedAuthPlugin: MarketplacePlugin | null = null;
  let guidedAuthMethod: "oauth2" | "api_key" | "pat" = "oauth2";
  let guidedAccountLabel = "";
  let guidedAccountApiKey = "";
  let guidedAccountIdentifier = "";
  let guidedAuthLoading = false;
  let guidedAuthError: string | null = null;

  function initiateInstall(item: MarketplacePlugin) {
    if (item.auth && item.auth.auth_type !== "none") {
      guidedAuthPlugin = item;
      guidedAuthMethod = item.auth.supported_methods[0] || "oauth2";
      guidedAccountLabel = "";
      guidedAccountApiKey = "";
      guidedAccountIdentifier = "";
      guidedAuthError = null;
      showGuidedAuthModal = true;
    } else {
      handleInstallMarketplace(item);
    }
  }

  async function handleGuidedInstallAndConnect() {
    if (!guidedAuthPlugin) return;
    guidedAuthLoading = true;
    guidedAuthError = null;
    const plugin = guidedAuthPlugin;
    try {
      // 1. Install the plugin first
      await installPlugin({
        source: "marketplace",
        target: plugin.id,
      });

      // 2. Connect the account
      if (guidedAuthMethod === "oauth2") {
        await startPluginOAuthConnect({
          pluginId: plugin.id,
          label: guidedAccountLabel.trim() || undefined,
          openBrowser: true,
        });
        showNotification(`Plugin '${plugin.name}' installé ! Ouverture du navigateur pour autoriser l'accès OAuth.`);
      } else {
        await connectPluginApiKey({
          pluginId: plugin.id,
          apiKey: guidedAccountApiKey.trim(),
          label: guidedAccountLabel.trim() || undefined,
          accountIdentifier: guidedAccountIdentifier.trim() || undefined,
          authMethod: guidedAuthMethod,
        });
        showNotification(`Plugin '${plugin.name}' installé et compte connecté avec succès !`);
      }

      showGuidedAuthModal = false;
      await loadData();
    } catch (err: any) {
      guidedAuthError = err?.message || String(err);
    } finally {
      guidedAuthLoading = false;
    }
  }

  async function handleGuidedInstallSkipAuth() {
    if (!guidedAuthPlugin) return;
    const plugin = guidedAuthPlugin;
    showGuidedAuthModal = false;
    await handleInstallMarketplace(plugin);
  }

  async function handleInstallMarketplace(item: MarketplacePlugin) {
    actionLoading[item.id] = true;
    errorMessage = null;
    try {
      await installPlugin({
        source: "marketplace",
        target: item.id,
      });
      showNotification(`Plugin '${item.name}' installé avec succès !`);
      await loadData();
    } catch (err: any) {
      errorMessage = `Échec de l'installation : ${err?.message || err}`;
    } finally {
      delete actionLoading[item.id];
      actionLoading = actionLoading;
    }
  }

  async function handleToggle(plugin: InstalledPlugin) {
    actionLoading[plugin.id] = true;
    try {
      const updated = await togglePlugin(plugin.id, !plugin.enabled);
      installedPlugins = installedPlugins.map((p) =>
        p.id === updated.id ? updated : p
      );
      showNotification(
        `Plugin '${plugin.name}' ${updated.enabled ? "activé" : "désactivé"} !`
      );
    } catch (err: any) {
      errorMessage = `Impossible de modifier le statut : ${err?.message || err}`;
    } finally {
      delete actionLoading[plugin.id];
      actionLoading = actionLoading;
    }
  }

  async function handleUninstall(plugin: InstalledPlugin) {
    if (plugin.isSystem) {
      errorMessage = "Les plugins système indispensables ne peuvent pas être désinstallés.";
      return;
    }
    const confirmed = await requestConfirm({
      title: `Désinstaller le plugin '${plugin.name}' ?`,
      body: "Ses skills et serveurs MCP seront retirés.",
      confirmLabel: "Désinstaller",
      cancelLabel: "Annuler",
    });
    if (!confirmed) {
      return;
    }
    actionLoading[plugin.id] = true;
    try {
      await uninstallPlugin(plugin.id);
      showNotification(`Plugin '${plugin.name}' désinstallé.`);
      await loadData();
    } catch (err: any) {
      errorMessage = `Échec de la désinstallation : ${err?.message || err}`;
    } finally {
      delete actionLoading[plugin.id];
      actionLoading = actionLoading;
    }
  }

  async function handleAddPluginSubmit() {
    errorMessage = null;
    actionLoading["modal"] = true;
    try {
      if (addMethod === "local") {
        if (!customSourceTarget.trim()) {
          throw new Error("Veuillez renseigner le chemin absolu du dossier.");
        }
        await installPlugin({
          source: "local",
          target: customSourceTarget.trim(),
        });
      } else if (addMethod === "git") {
        if (!customSourceTarget.trim()) {
          throw new Error("Veuillez renseigner l'URL du dépôt Git.");
        }
        await installPlugin({
          source: "git",
          target: customSourceTarget.trim(),
        });
      } else if (addMethod === "custom") {
        if (!customPluginForm.name.trim()) {
          throw new Error("Le nom du plugin est obligatoire (ex: my-custom-plugin).");
        }
        customPluginForm.skills = [
          {
            name: "Assistant Skill",
            description: "Skill autonome généré pour l'assistant",
            instructions: customSkillInstructions,
            icon: "⚡",
            tags: ["custom", "automation"],
          },
        ];
        await createCustomPlugin(customPluginForm);
      }
      showAddModal = false;
      customSourceTarget = "";
      showNotification("Plugin installé avec succès !");
      activeTab = "installed";
      await loadData();
    } catch (err: any) {
      errorMessage = err?.message || String(err);
    } finally {
      delete actionLoading["modal"];
      actionLoading = actionLoading;
    }
  }

  function showNotification(msg: string) {
    successMessage = msg;
    setTimeout(() => {
      if (successMessage === msg) successMessage = null;
    }, 4000);
  }

  // Filtered lists
  $: filteredMarketplace = marketplacePlugins.filter((item) => {
    const q = searchQuery.toLowerCase().trim();
    const matchesSearch =
      !q ||
      item.name.toLowerCase().includes(q) ||
      item.description.toLowerCase().includes(q) ||
      (item.keywords && item.keywords.some((k) => k.toLowerCase().includes(q)));
    const matchesCategory =
      selectedCategory === "Tous" || item.category === selectedCategory;
    return matchesSearch && matchesCategory;
  });

  $: filteredInstalled = installedPlugins.filter((item) => {
    const q = searchQuery.toLowerCase().trim();
    return (
      !q ||
      item.name.toLowerCase().includes(q) ||
      (item.description && item.description.toLowerCase().includes(q)) ||
      (item.keywords && item.keywords.some((k) => k.toLowerCase().includes(q)))
    );
  });

  // Metrics
  $: totalSkills = installedPlugins.reduce((acc, p) => acc + (p.skills?.length || 0), 0);
  $: totalMcpServers = installedPlugins.reduce((acc, p) => acc + (p.mcpServers?.length || 0), 0);
</script>

<div class="plugins-page-root {isDark ? 'dark' : 'light'}">
  <!-- Notification Banner (rendered globally so feedback is visible in both grid and details view) -->
  {#if successMessage}
    <div class="feedback-banner success" transition:fly={{ y: -8, duration: 200 }}>
      <Check size={16} />
      <span>{successMessage}</span>
      <button type="button" class="close-banner" on:click={() => (successMessage = null)}>
        <X size={14} />
      </button>
    </div>
  {/if}

  {#if errorMessage}
    <div class="feedback-banner error" transition:fly={{ y: -8, duration: 200 }}>
      <Info size={16} />
      <span>{errorMessage}</span>
      <button type="button" class="close-banner" on:click={() => (errorMessage = null)}>
        <X size={14} />
      </button>
    </div>
  {/if}

  {#if selectedPluginId}
    <PluginDetailView
      pluginId={selectedPluginId}
      {isDark}
      {installedPlugins}
      {marketplacePlugins}
      {actionLoading}
      onBack={() => (selectedPluginId = null)}
      onSelectCategory={handleSelectCategoryFromDetail}
      onInstall={initiateInstall}
      onToggle={handleToggle}
      onUninstall={handleUninstall}
      onRefresh={refresh}
    />
  {:else}
    <!-- Top Panel Header -->
  <div class="panel-header">
    <div class="header-titles">
      <div class="title-with-badge">
        <h2>Agent Plugins</h2>
        <a
          href="https://agent-plugins.org/"
          target="_blank"
          rel="noopener noreferrer"
          class="spec-badge"
          title="Consulter la spécification officielle agent-plugins.org"
        >
          <Sparkles size={12} />
          <span>Spec v1.0.0</span>
          <ExternalLink size={10} />
        </a>
      </div>
      <p class="subtitle">
        Architecture modulaire et portable conforme au standard officiel <a href="https://agent-plugins.org/" target="_blank" rel="noopener noreferrer">agent-plugins.org</a>.
        Chaque plugin intègre des <strong>Skills autonomes</strong> et des <strong>serveurs MCP</strong> directement exploitables par l'assistant.
      </p>

      <!-- Quick Metrics Pill Row -->
      <div class="metrics-row">
        <span class="metric-pill">
          <Puzzle size={12} />
          <strong>{installedPlugins.length}</strong> plugin{installedPlugins.length > 1 ? "s" : ""} installé{installedPlugins.length > 1 ? "s" : ""}
        </span>
        <span class="metric-pill">
          <Wrench size={12} />
          <strong>{totalSkills}</strong> skill{totalSkills > 1 ? "s" : ""} actif{totalSkills > 1 ? "s" : ""}
        </span>
        <span class="metric-pill">
          <Server size={12} />
          <strong>{totalMcpServers}</strong> serveur{totalMcpServers > 1 ? "s" : ""} MCP
        </span>
      </div>
    </div>

    <div class="header-actions">
      <button
        class="refresh-btn"
        type="button"
        on:click={refresh}
        disabled={refreshing}
        title="Actualiser les plugins"
      >
        <RefreshCw size={15} class={refreshing ? "spin" : ""} />
      </button>

      <button
        class="primary-btn"
        type="button"
        on:click={() => (showAddModal = true)}
      >
        <Plus size={15} />
        <span>Ajouter un plugin</span>
      </button>
    </div>
  </div>

  <!-- Controls Bar: Segmented Switcher & Search Bar -->
  <div class="controls-bar">
    <!-- Apple Native Style Segmented Switcher -->
    <div class="segmented-control">
      <button
        type="button"
        class="segment-button {activeTab === 'marketplace' ? 'active' : ''}"
        on:click={() => (activeTab = "marketplace")}
      >
        <Puzzle size={14} />
        <span>Catalogue / Marketplace</span>
        <span class="segment-badge">{marketplacePlugins.length}</span>
      </button>

      <button
        type="button"
        class="segment-button {activeTab === 'installed' ? 'active' : ''}"
        on:click={() => (activeTab = "installed")}
      >
        <Check size={14} />
        <span>Plugins Installés</span>
        <span class="segment-badge">{installedPlugins.length}</span>
      </button>
    </div>

    <!-- Search Input -->
    <div class="search-box-wrapper">
      <Search size={14} class="search-icon" />
      <input
        type="text"
        bind:value={searchQuery}
        placeholder="Rechercher plugins, skills, MCP..."
        class="search-input"
      />
      {#if searchQuery}
        <button type="button" class="clear-search-btn" on:click={() => (searchQuery = "")}>
          <X size={13} />
        </button>
      {/if}
    </div>
  </div>

  <!-- Tab Content: Marketplace View -->
  {#if activeTab === "marketplace"}
    <!-- Category Filter Chips -->
    <div class="category-filters-row">
      {#each categories as cat}
        <button
          type="button"
          class="cat-chip {selectedCategory === cat ? 'active' : ''}"
          on:click={() => (selectedCategory = cat)}
        >
          {cat}
        </button>
      {/each}
    </div>

    <!-- Grid of Marketplace Plugins -->
    {#if loading && marketplacePlugins.length === 0}
      <div class="loading-state">
        <Loader2 size={26} class="spin" />
        <p>Chargement du catalogue agent-plugins.org…</p>
      </div>
    {:else if filteredMarketplace.length === 0}
      <div class="empty-state">
        <div class="empty-icon-box">
          <Puzzle size={32} />
        </div>
        <h3>Aucun plugin trouvé</h3>
        <p>Aucun plugin ne correspond à vos filtres actuels ou à votre recherche.</p>
        {#if searchQuery || selectedCategory !== 'Tous'}
          <button
            type="button"
            class="reset-filters-btn"
            on:click={() => {
              searchQuery = "";
              selectedCategory = "Tous";
            }}
          >
            Réinitialiser les filtres
          </button>
        {/if}
      </div>
    {:else}
      <div class="plugins-grid">
        {#each filteredMarketplace as item (item.id)}
          <div
            class="plugin-card {item.installed ? 'installed-card' : ''}"
            role="button"
            tabindex="0"
            aria-label={item.name}
            on:click={(e) => {
              if (e.target instanceof HTMLElement && e.target.closest("button, input, a")) return;
              selectedPluginId = item.id;
            }}
            on:keydown={(e) => {
              if (e.target === e.currentTarget && (e.key === "Enter" || e.key === " ")) {
                e.preventDefault();
                selectedPluginId = item.id;
              }
            }}
          >
            <div class="card-body">
              <div class="card-header-row">
                <div class="icon-avatar">
                  <span>{item.icon || "📦"}</span>
                </div>
                <div class="card-meta">
                  <div class="title-row">
                    <h4 class="plugin-title">{item.name}</h4>
                    <span class="version-tag">v{item.version}</span>
                  </div>
                  <div class="sub-meta-row">
                    <span class="author-tag">{item.author}</span>
                    <span class="dot-sep">•</span>
                    <span class="category-tag">{item.category}</span>
                  </div>
                </div>
              </div>

              <p class="card-description">{item.description}</p>

              <!-- Components Capabilities -->
              <div class="capabilities-row">
                <span class="cap-badge mcp" title="Serveurs Model Context Protocol">
                  <Server size={12} />
                  <span>{item.mcpServersCount} {item.mcpServersCount > 1 ? "MCPs" : "MCP"}</span>
                </span>
                <span class="cap-badge skill" title="Skills autonomes pour l'agent">
                  <Wrench size={12} />
                  <span>{item.skillsCount} {item.skillsCount > 1 ? "Skills" : "Skill"}</span>
                </span>
                {#if item.auth && item.auth.auth_type !== "none"}
                  <span class="cap-badge auth" title="Authentification {item.auth.auth_type === 'oauth2' ? 'OAuth 2.0 PKCE' : 'sécurisée'} requise">
                    <Key size={11} />
                    <span>{item.auth.auth_type === "oauth2" ? "OAuth" : "Clé API"}</span>
                  </span>
                {/if}
              </div>

              {#if item.keywords && item.keywords.length > 0}
                <div class="keywords-row">
                  {#each item.keywords.slice(0, 3) as kw}
                    <span class="keyword-pill">#{kw}</span>
                  {/each}
                </div>
              {/if}
            </div>

            <div class="card-footer">
              {#if item.installed}
                <button
                  type="button"
                  class="installed-btn"
                  on:click|stopPropagation={() => (selectedPluginId = item.id)}
                  on:keydown|stopPropagation={(e) => e.stopPropagation()}
                >
                  <Check size={14} />
                  <span>Détails</span>
                </button>
              {:else}
                <button
                  type="button"
                  class="install-btn"
                  disabled={actionLoading[item.id]}
                  on:click|stopPropagation={() => initiateInstall(item)}
                  on:keydown|stopPropagation={(e) => e.stopPropagation()}
                >
                  {#if actionLoading[item.id]}
                    <Loader2 size={13} class="spin" />
                    <span>Installation…</span>
                  {:else}
                    <Download size={13} />
                    <span>Installer</span>
                  {/if}
                </button>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    {/if}
  {:else}
    <!-- Tab Content: Installed Plugins View -->
    {#if loading && installedPlugins.length === 0}
      <div class="loading-state">
        <Loader2 size={26} class="spin" />
        <p>Vérification des plugins installés…</p>
      </div>
    {:else if filteredInstalled.length === 0}
      <div class="empty-state">
        <div class="empty-icon-box">
          <Server size={32} />
        </div>
        <h3>Aucun plugin installé</h3>
        <p>
          Parcourez le <button type="button" class="link-action" on:click={() => (activeTab = "marketplace")}>Catalogue</button> pour installer des plugins prêts à l'emploi ou cliquez sur « Ajouter un plugin » pour connecter un dépôt Git ou un dossier local.
        </p>
      </div>
    {:else}
      <div class="plugins-grid">
        {#each filteredInstalled as plugin (plugin.id)}
          <div
            class="plugin-card {plugin.enabled ? '' : 'card-disabled'}"
            role="button"
            tabindex="0"
            aria-label={plugin.name}
            on:click={(e) => {
              if (e.target instanceof HTMLElement && e.target.closest("button, input, a")) return;
              selectedPluginId = plugin.id;
            }}
            on:keydown={(e) => {
              if (e.target === e.currentTarget && (e.key === "Enter" || e.key === " ")) {
                e.preventDefault();
                selectedPluginId = plugin.id;
              }
            }}
          >
            <div class="card-body">
              <div class="card-header-row">
                <div class="icon-avatar">
                  <span>{marketplacePlugins.find((m) => m.id === plugin.id)?.icon || plugin.skills[0]?.icon || "🧩"}</span>
                </div>
                <div class="card-meta">
                  <div class="title-row">
                    <h4 class="plugin-title">{plugin.name}</h4>
                    {#if plugin.version}
                      <span class="version-tag">v{plugin.version}</span>
                    {/if}
                  </div>
                  <div class="sub-meta-row">
                    {#if plugin.isSystem}
                      <span class="source-tag system-badge" title="Plugin système inclus par défaut avec ARO">Système</span>
                    {:else}
                      <span class="source-tag">{plugin.source}</span>
                    {/if}
                    <span class="status-indicator {plugin.enabled ? 'status-active' : 'status-inactive'}">
                      <span class="status-dot"></span>
                      <span>{plugin.enabled ? "Actif" : "Inactif"}</span>
                    </span>
                  </div>
                </div>

                <!-- Apple iOS Toggle Switch -->
                <div class="toggle-container">
                  <label
                    class="ios-switch"
                    title={plugin.enabled ? "Désactiver" : "Activer"}
                  >
                    <input
                      type="checkbox"
                      checked={plugin.enabled}
                      disabled={actionLoading[plugin.id]}
                      on:click|stopPropagation
                      on:keydown|stopPropagation={(e) => e.stopPropagation()}
                      on:change={() => handleToggle(plugin)}
                    />
                    <span class="switch-track"></span>
                  </label>
                </div>
              </div>

              {#if plugin.description}
                <p class="card-description">{plugin.description}</p>
              {/if}

              <div class="capabilities-row">
                <span class="cap-badge mcp" title="Serveurs MCP déclarés dans mcp.json">
                  <Server size={12} />
                  <span>{plugin.mcpServers.length} {plugin.mcpServers.length > 1 ? "Serveurs MCP" : "Serveur MCP"}</span>
                </span>
                <span class="cap-badge skill" title="Skills découverts sous skills/*/SKILL.md">
                  <Wrench size={12} />
                  <span>{plugin.skills.length} {plugin.skills.length > 1 ? "Skills" : "Skill"}</span>
                </span>
              </div>
            </div>

            <div class="card-footer installed-footer">
              <button
                type="button"
                class="inspect-btn"
                on:click|stopPropagation={() => (selectedPluginId = plugin.id)}
                on:keydown|stopPropagation={(e) => e.stopPropagation()}
              >
                <Info size={13} />
                <span>Détails</span>
              </button>

              <button
                type="button"
                class="delete-btn"
                title="Désinstaller le plugin"
                disabled={actionLoading[plugin.id]}
                on:click|stopPropagation={() => handleUninstall(plugin)}
                on:keydown|stopPropagation={(e) => e.stopPropagation()}
              >
                <Trash2 size={14} />
              </button>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  {/if}
{/if}

  <!-- Add Plugin Modal -->
  {#if showAddModal}
    <div
      class="modal-backdrop"
      role="button"
      tabindex="0"
      transition:fade={{ duration: 150 }}
      on:click={() => (showAddModal = false)}
      on:keydown={(e) => e.key === "Escape" && (showAddModal = false)}
    >
      <div
        class="modal-card {isDark ? 'dark' : 'light'}"
        role="dialog"
        tabindex="-1"
        on:click|stopPropagation
        on:keydown|stopPropagation
      >
        <div class="modal-header">
          <div>
            <h3 class="modal-title">Ajouter un Agent Plugin</h3>
            <p class="modal-subtitle">Conforme à la spécification standard agent-plugins.org</p>
          </div>
          <button type="button" class="modal-close-btn" on:click={() => (showAddModal = false)}>
            <X size={16} />
          </button>
        </div>

        <!-- Add Method Tabs -->
        <div class="modal-subtabs">
          <button
            type="button"
            class="modal-tab {addMethod === 'marketplace' ? 'active' : ''}"
            on:click={() => (addMethod = "marketplace")}
          >
            <Puzzle size={13} />
            <span>Catalogue</span>
          </button>
          <button
            type="button"
            class="modal-tab {addMethod === 'git' ? 'active' : ''}"
            on:click={() => (addMethod = "git")}
          >
            <GitBranch size={13} />
            <span>Dépôt Git</span>
          </button>
          <button
            type="button"
            class="modal-tab {addMethod === 'local' ? 'active' : ''}"
            on:click={() => (addMethod = "local")}
          >
            <Folder size={13} />
            <span>Dossier local</span>
          </button>
          <button
            type="button"
            class="modal-tab {addMethod === 'custom' ? 'active' : ''}"
            on:click={() => (addMethod = "custom")}
          >
            <Sparkles size={13} />
            <span>Créer Personnalisé</span>
          </button>
        </div>

        <div class="modal-body-scroll">
          {#if addMethod === "marketplace"}
            <p class="modal-body-hint">
              Sélectionnez un plugin dans le catalogue officiel pour l'ajouter instantanément à votre environnement ARO.
            </p>
            <div class="quick-catalog-list">
              {#each marketplacePlugins as p}
                <div class="quick-catalog-card">
                  <div class="quick-left">
                    <span class="quick-avatar">{p.icon}</span>
                    <div class="quick-text">
                      <div class="quick-title-row">
                        <strong class="quick-name">{p.name}</strong>
                        <span class="version-tag">v{p.version}</span>
                      </div>
                      <span class="quick-desc">{p.description}</span>
                      <div class="quick-badges">
                        <span class="cap-badge mcp"><Server size={11} /> {p.mcpServersCount} MCP</span>
                        <span class="cap-badge skill"><Wrench size={11} /> {p.skillsCount} Skill</span>
                      </div>
                    </div>
                  </div>
                  {#if p.installed}
                    <span class="quick-installed-badge">
                      <Check size={13} />
                      <span>Installé</span>
                    </span>
                  {:else}
                    <button
                      type="button"
                      class="quick-install-btn"
                      disabled={actionLoading[p.id]}
                      on:click={async () => {
                        await handleInstallMarketplace(p);
                        showAddModal = false;
                        activeTab = "installed";
                      }}
                    >
                      {#if actionLoading[p.id]}
                        <Loader2 size={12} class="spin" />
                      {:else}
                        <Download size={12} />
                      {/if}
                      <span>Installer</span>
                    </button>
                  {/if}
                </div>
              {/each}
            </div>
          {:else if addMethod === "git"}
            <div class="form-wrapper">
              <p class="modal-body-hint">
                Cloner et charger un plugin autonome depuis un dépôt GitHub ou GitLab contenant un <code>plugin.json</code>.
              </p>
              <div class="form-item">
                <label for="git-url">URL du dépôt Git</label>
                <input
                  id="git-url"
                  type="text"
                  bind:value={customSourceTarget}
                  placeholder="https://github.com/agentplugins/my-plugin.git"
                  class="form-input"
                />
                <span class="input-hint">Le dépôt doit contenir un fichier <code>plugin.json</code> valide à la racine.</span>
              </div>
            </div>
          {:else if addMethod === "local"}
            <div class="form-wrapper">
              <p class="modal-body-hint">
                Monter un plugin existant depuis votre système de fichiers local.
              </p>
              <div class="form-item">
                <label for="local-path">Chemin absolu du dossier</label>
                <input
                  id="local-path"
                  type="text"
                  bind:value={customSourceTarget}
                  placeholder="C:\Users\...\mon-plugin-agent"
                  class="form-input"
                />
                <span class="input-hint">Le dossier doit inclure la structure <code>plugin.json</code> et un sous-dossier <code>skills/</code>.</span>
              </div>
            </div>
          {:else}
            <!-- Custom Creator Form -->
            <div class="form-wrapper">
              <p class="modal-body-hint">
                Générez un nouveau package plugin standard avec son manifeste, son serveur MCP et ses instructions Skill.
              </p>
              <div class="custom-grid">
                <div class="form-item">
                  <label for="custom-name">Nom du plugin (kebab-case strict)</label>
                  <input
                    id="custom-name"
                    type="text"
                    bind:value={customPluginForm.name}
                    placeholder="mon-plugin-assistant"
                    class="form-input"
                  />
                </div>

                <div class="form-item">
                  <label for="custom-version">Version initiale</label>
                  <input
                    id="custom-version"
                    type="text"
                    bind:value={customPluginForm.version}
                    placeholder="1.0.0"
                    class="form-input"
                  />
                </div>

                <div class="form-item full-span">
                  <label for="custom-desc">Description</label>
                  <input
                    id="custom-desc"
                    type="text"
                    bind:value={customPluginForm.description}
                    placeholder="Automatisation avancée et outils d'assistance..."
                    class="form-input"
                  />
                </div>

                <div class="form-item full-span">
                  <label for="custom-skill">Instructions du Skill (SKILL.md)</label>
                  <textarea
                    id="custom-skill"
                    bind:value={customSkillInstructions}
                    rows="4"
                    class="form-textarea"
                  ></textarea>
                  <span class="input-hint">Ces instructions guideront l'assistant lors de l'exécution autonome du skill.</span>
                </div>
              </div>
            </div>
          {/if}
        </div>

        {#if addMethod !== "marketplace"}
          <div class="modal-footer">
            <button type="button" class="footer-cancel-btn" on:click={() => (showAddModal = false)}>
              Annuler
            </button>
            <button
              type="button"
              class="primary-btn"
              disabled={actionLoading["modal"]}
              on:click={handleAddPluginSubmit}
            >
              {#if actionLoading["modal"]}
                <Loader2 size={14} class="spin" />
                <span>Traitement…</span>
              {:else}
                <Check size={14} />
                <span>Installer le plugin</span>
              {/if}
            </button>
          </div>
        {/if}
      </div>
    </div>
  {/if}

  <!-- Guided Auth Installation Modal -->
  {#if showGuidedAuthModal && guidedAuthPlugin}
    <div
      class="modal-backdrop"
      role="button"
      tabindex="0"
      transition:fade={{ duration: 150 }}
      on:click={() => (showGuidedAuthModal = false)}
      on:keydown={(e) => e.key === "Escape" && (showGuidedAuthModal = false)}
    >
      <div
        class="modal-card {isDark ? 'dark' : 'light'}"
        role="dialog"
        tabindex="-1"
        on:click|stopPropagation
        on:keydown|stopPropagation
      >
        <div class="modal-header">
          <div class="guided-header-info">
            <div class="guided-plugin-icon">
              <span>{guidedAuthPlugin.icon || "🧩"}</span>
            </div>
            <div>
              <h3 class="modal-title">Authentification requise</h3>
              <p class="modal-subtitle">{guidedAuthPlugin.name} v{guidedAuthPlugin.version}</p>
            </div>
          </div>
          <button type="button" class="modal-close-btn" on:click={() => (showGuidedAuthModal = false)}>
            <X size={16} />
          </button>
        </div>

        <div class="modal-body guided-modal-body">
          <div class="guided-intro-banner">
            <ShieldCheck size={18} class="guided-banner-icon" />
            <div class="guided-banner-text">
              <p>
                Le plugin <strong>{guidedAuthPlugin.name}</strong> requiert une authentification pour accéder à ses services.
                Vous pouvez connecter votre compte dès maintenant ou finaliser l'installation et connecter vos comptes plus tard.
              </p>
            </div>
          </div>

          <!-- Method Selector if multiple -->
          {#if guidedAuthPlugin.auth && guidedAuthPlugin.auth.supported_methods.length > 1}
            <div class="guided-method-tabs">
              {#each guidedAuthPlugin.auth.supported_methods as method}
                <button
                  type="button"
                  class="guided-method-tab {guidedAuthMethod === method ? 'active' : ''}"
                  on:click={() => {
                    guidedAuthMethod = method;
                    guidedAuthError = null;
                  }}
                >
                  {#if method === "oauth2"}
                    <Globe size={13} />
                    <span>OAuth 2.0 PKCE</span>
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
          {/if}

          <!-- Form inputs -->
          {#if guidedAuthMethod === "oauth2"}
            <div class="guided-oauth-info">
              <p class="oauth-desc">
                La connexion s'effectue via un flux d'autorisation sécurisé <strong>OAuth 2.0 PKCE</strong>.
                Une fenêtre de navigateur va s'ouvrir pour vous permettre de sélectionner votre compte et d'accorder les autorisations.
              </p>
              {#if guidedAuthPlugin.auth?.scopes && guidedAuthPlugin.auth.scopes.length > 0}
                <div class="scopes-list">
                  <span class="scopes-title">Permissions requises :</span>
                  {#each guidedAuthPlugin.auth.scopes as sc}
                    <span class="scope-pill">{sc}</span>
                  {/each}
                </div>
              {/if}

              <div class="form-item">
                <label for="guided-acc-label">Libellé du compte (optionnel)</label>
                <input
                  id="guided-acc-label"
                  type="text"
                  bind:value={guidedAccountLabel}
                  placeholder="Ex: Mon compte Google Pro, Personnel..."
                  class="form-input"
                />
              </div>
            </div>
          {:else}
            <div class="guided-key-form">
              <p class="key-prompt">
                {guidedAuthPlugin.auth?.api_key_prompt || "Saisissez votre clé secrète ou Personal Access Token. Elle sera chiffrée dans le trousseau OS."}
              </p>
              {#if guidedAuthPlugin.auth?.documentation_url}
                <a
                  href={guidedAuthPlugin.auth.documentation_url}
                  target="_blank"
                  rel="noreferrer"
                  class="doc-link"
                >
                  <ExternalLink size={12} />
                  <span>Comment obtenir cette clé ?</span>
                </a>
              {/if}

              <div class="form-item">
                <label for="guided-key-input">
                  {guidedAuthMethod === "pat" ? "Personal Access Token (PAT)" : "Clé API"} *
                </label>
                <input
                  id="guided-key-input"
                  type="password"
                  bind:value={guidedAccountApiKey}
                  placeholder={guidedAuthMethod === "pat" ? "ghp_... ou token secret" : "sk-... ou clé secrète"}
                  class="form-input"
                />
              </div>

              <div class="form-item">
                <label for="guided-key-label">Libellé du compte (optionnel)</label>
                <input
                  id="guided-key-label"
                  type="text"
                  bind:value={guidedAccountLabel}
                  placeholder="Ex: Clé Production, Compte Équipe..."
                  class="form-input"
                />
              </div>

              <div class="form-item">
                <label for="guided-key-ident">Identifiant ou Email (optionnel)</label>
                <input
                  id="guided-key-ident"
                  type="text"
                  bind:value={guidedAccountIdentifier}
                  placeholder="Ex: user@example.com"
                  class="form-input"
                />
              </div>
            </div>
          {/if}

          {#if guidedAuthError}
            <div class="feedback-banner error modal-error">
              <Info size={14} />
              <span>{guidedAuthError}</span>
            </div>
          {/if}
        </div>

        <div class="modal-footer guided-footer">
          <button
            type="button"
            class="footer-cancel-btn"
            disabled={guidedAuthLoading}
            on:click={() => (showGuidedAuthModal = false)}
          >
            Annuler
          </button>
          <button
            type="button"
            class="footer-cancel-btn"
            disabled={guidedAuthLoading}
            on:click={handleGuidedInstallSkipAuth}
          >
            Installer sans connecter
          </button>
          <button
            type="button"
            class="primary-btn"
            disabled={guidedAuthLoading || (guidedAuthMethod !== 'oauth2' && !guidedAccountApiKey.trim())}
            on:click={handleGuidedInstallAndConnect}
          >
            {#if guidedAuthLoading}
              <Loader2 size={14} class="spin" />
              <span>Installation & Connexion…</span>
            {:else if guidedAuthMethod === 'oauth2'}
              <ExternalLink size={14} />
              <span>Installer & Connecter (OAuth)</span>
            {:else}
              <ShieldCheck size={14} />
              <span>Installer & Enregistrer la clé</span>
            {/if}
          </button>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  /* -------------------------------------------------------------
     Root & Typography Base
     ------------------------------------------------------------- */
  .plugins-page-root {
    display: flex;
    flex-direction: column;
    width: 100%;
    min-height: 100%;
    box-sizing: border-box;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  }

  /* -------------------------------------------------------------
     Header Section
     ------------------------------------------------------------- */
  .panel-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 24px;
    margin-bottom: 24px;
    width: 100%;
  }

  .header-titles {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-width: 660px;
  }

  .title-with-badge {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .title-with-badge h2 {
    font-size: 22px;
    font-weight: 700;
    margin: 0;
    letter-spacing: -0.5px;
  }

  .plugins-page-root.light .title-with-badge h2 {
    color: #1d1d1f;
  }

  .plugins-page-root.dark .title-with-badge h2 {
    color: #ffffff;
  }

  .spec-badge {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 3px 9px;
    border-radius: 980px;
    font-size: 11px;
    font-weight: 600;
    text-decoration: none;
    transition: all 0.2s ease;
  }

  .plugins-page-root.light .spec-badge {
    background: rgba(0, 113, 227, 0.08);
    color: #0071e3;
    border: 1px solid rgba(0, 113, 227, 0.2);
  }

  .plugins-page-root.light .spec-badge:hover {
    background: rgba(0, 113, 227, 0.16);
  }

  .plugins-page-root.dark .spec-badge {
    background: rgba(41, 151, 255, 0.15);
    color: #2997ff;
    border: 1px solid rgba(41, 151, 255, 0.3);
  }

  .plugins-page-root.dark .spec-badge:hover {
    background: rgba(41, 151, 255, 0.25);
  }

  .subtitle {
    margin: 0;
    font-size: 13px;
    line-height: 1.5;
    color: #86868b;
  }

  .subtitle a {
    color: #0071e3;
    text-decoration: none;
  }

  .plugins-page-root.dark .subtitle a {
    color: #2997ff;
  }

  .subtitle a:hover {
    text-decoration: underline;
  }

  /* Quick Metrics */
  .metrics-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-top: 6px;
  }

  .metric-pill {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 11px;
    padding: 3px 9px;
    border-radius: 6px;
  }

  .plugins-page-root.light .metric-pill {
    background: rgba(0, 0, 0, 0.04);
    color: #515154;
    border: 1px solid rgba(0, 0, 0, 0.05);
  }

  .plugins-page-root.light .metric-pill strong {
    color: #1d1d1f;
  }

  .plugins-page-root.dark .metric-pill {
    background: rgba(255, 255, 255, 0.06);
    color: #a1a1a6;
    border: 1px solid rgba(255, 255, 255, 0.06);
  }

  .plugins-page-root.dark .metric-pill strong {
    color: #ffffff;
  }

  .header-actions {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-shrink: 0;
  }

  .refresh-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: 36px;
    border-radius: 50%;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .plugins-page-root.light .refresh-btn {
    background: rgba(0, 0, 0, 0.04);
    border: 1px solid rgba(0, 0, 0, 0.08);
    color: #1d1d1f;
  }

  .plugins-page-root.light .refresh-btn:hover {
    background: rgba(0, 0, 0, 0.08);
  }

  .plugins-page-root.dark .refresh-btn {
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #ffffff;
  }

  .plugins-page-root.dark .refresh-btn:hover {
    background: rgba(255, 255, 255, 0.12);
  }

  .primary-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: linear-gradient(180deg, #2997ff 0%, #0071e3 100%);
    color: #ffffff !important;
    border: none;
    border-radius: 980px;
    padding: 8px 18px;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    box-shadow: 0 4px 12px rgba(0, 113, 227, 0.25);
    white-space: nowrap;
    transition: all 0.2s ease;
  }

  .primary-btn:hover {
    transform: translateY(-1px);
    box-shadow: 0 6px 16px rgba(0, 113, 227, 0.35);
  }

  /* -------------------------------------------------------------
     Notification Banners
     ------------------------------------------------------------- */
  .feedback-banner {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 16px;
    border-radius: 10px;
    margin-bottom: 20px;
    font-size: 13px;
    font-weight: 500;
  }

  .feedback-banner.success {
    background: rgba(52, 199, 89, 0.12);
    border: 1px solid rgba(52, 199, 89, 0.3);
    color: #34c759;
  }

  .feedback-banner.error {
    background: rgba(255, 69, 58, 0.12);
    border: 1px solid rgba(255, 69, 58, 0.3);
    color: #ff453a;
  }

  .close-banner {
    margin-left: auto;
    background: none;
    border: none;
    color: inherit;
    cursor: pointer;
  }

  /* -------------------------------------------------------------
     Controls Bar: Segmented Switcher & Search
     ------------------------------------------------------------- */
  .controls-bar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 16px;
    margin-bottom: 22px;
    flex-wrap: wrap;
  }

  /* Segmented Control - Native Apple style */
  .segmented-control {
    display: inline-flex;
    padding: 3px;
    border-radius: 10px;
    gap: 2px;
    box-sizing: border-box;
  }

  .plugins-page-root.light .segmented-control {
    background: rgba(0, 0, 0, 0.05);
    border: 1px solid rgba(0, 0, 0, 0.06);
  }

  .plugins-page-root.dark .segmented-control {
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.08);
  }

  .segment-button {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    border: none;
    border-radius: 8px;
    padding: 7px 16px;
    font-size: 13px;
    cursor: pointer;
    transition: all 0.2s ease;
    background: transparent;
  }

  /* Inactive segment */
  .plugins-page-root.light .segment-button {
    color: #6e6e73;
    font-weight: 500;
  }

  .plugins-page-root.light .segment-button:hover {
    color: #1d1d1f;
  }

  .plugins-page-root.dark .segment-button {
    color: #86868b;
    font-weight: 500;
  }

  .plugins-page-root.dark .segment-button:hover {
    color: #ffffff;
  }

  /* Active segment - HIGH CONTRAST */
  .plugins-page-root.light .segment-button.active {
    background: #ffffff !important;
    color: #1d1d1f !important;
    font-weight: 600;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.12), 0 1px 2px rgba(0, 0, 0, 0.06);
  }

  .plugins-page-root.dark .segment-button.active {
    background: rgba(255, 255, 255, 0.16) !important;
    color: #ffffff !important;
    font-weight: 600;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.35);
  }

  .segment-badge {
    font-size: 11px;
    font-weight: 600;
    padding: 2px 7px;
    border-radius: 980px;
    transition: all 0.2s ease;
  }

  .plugins-page-root.light .segment-button .segment-badge {
    background: rgba(0, 0, 0, 0.06);
    color: #6e6e73;
  }

  .plugins-page-root.light .segment-button.active .segment-badge {
    background: rgba(0, 0, 0, 0.08);
    color: #1d1d1f;
  }

  .plugins-page-root.dark .segment-button .segment-badge {
    background: rgba(255, 255, 255, 0.08);
    color: #86868b;
  }

  .plugins-page-root.dark .segment-button.active .segment-badge {
    background: rgba(255, 255, 255, 0.14);
    color: #ffffff;
  }

  /* Search Input Wrapper */
  .search-box-wrapper {
    position: relative;
    display: flex;
    align-items: center;
    width: 100%;
    max-width: 320px;
  }

  .search-input {
    width: 100%;
    padding: 8px 30px 8px 34px;
    border-radius: 10px;
    font-size: 13px;
    outline: none;
    transition: all 0.2s ease;
    box-sizing: border-box;
  }

  .plugins-page-root.light .search-input {
    border: 1px solid rgba(0, 0, 0, 0.12);
    background: rgba(0, 0, 0, 0.02);
    color: #1d1d1f;
  }

  .plugins-page-root.light .search-input:focus {
    border-color: #0071e3;
    background: #ffffff;
    box-shadow: 0 0 0 3px rgba(0, 113, 227, 0.12);
  }

  .plugins-page-root.dark .search-input {
    border: 1px solid rgba(255, 255, 255, 0.1);
    background: rgba(255, 255, 255, 0.04);
    color: #ffffff;
  }

  .plugins-page-root.dark .search-input:focus {
    border-color: #2997ff;
    background: rgba(255, 255, 255, 0.07);
    box-shadow: 0 0 0 3px rgba(41, 151, 255, 0.2);
  }

  .search-box-wrapper :global(.search-icon) {
    position: absolute;
    left: 11px;
    color: #86868b;
    pointer-events: none;
  }

  .clear-search-btn {
    position: absolute;
    right: 8px;
    background: none;
    border: none;
    color: #86868b;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 2px;
  }

  /* -------------------------------------------------------------
     Category Filter Pills
     ------------------------------------------------------------- */
  .category-filters-row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-bottom: 22px;
  }

  .cat-chip {
    border: none;
    border-radius: 980px;
    padding: 6px 14px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.18s ease;
  }

  .plugins-page-root.light .cat-chip {
    background: rgba(0, 0, 0, 0.04);
    color: #515154;
    border: 1px solid rgba(0, 0, 0, 0.05);
  }

  .plugins-page-root.light .cat-chip:hover {
    background: rgba(0, 0, 0, 0.07);
    color: #1d1d1f;
  }

  .plugins-page-root.dark .cat-chip {
    background: rgba(255, 255, 255, 0.05);
    color: #86868b;
    border: 1px solid rgba(255, 255, 255, 0.06);
  }

  .plugins-page-root.dark .cat-chip:hover {
    background: rgba(255, 255, 255, 0.09);
    color: #ffffff;
  }

  .cat-chip.active {
    background: #0071e3 !important;
    color: #ffffff !important;
    box-shadow: 0 2px 8px rgba(0, 113, 227, 0.3);
    font-weight: 600;
    border-color: transparent !important;
  }

  /* -------------------------------------------------------------
     Cards Grid
     ------------------------------------------------------------- */
  .plugins-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
    gap: 16px;
    width: 100%;
  }

  .plugin-card {
    border-radius: 14px;
    padding: 18px;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    transition: all 0.2s cubic-bezier(0.16, 1, 0.3, 1);
    box-sizing: border-box;
    cursor: pointer;
    user-select: none;
    outline: none;
  }

  .plugin-card:focus-visible {
    box-shadow: 0 0 0 3px #0071e3 !important;
  }

  .plugins-page-root.light .plugin-card {
    background: #ffffff;
    border: 1px solid rgba(0, 0, 0, 0.08);
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.02);
  }

  .plugins-page-root.light .plugin-card:hover {
    transform: translateY(-2px);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.08);
    border-color: rgba(0, 0, 0, 0.12);
  }

  .plugins-page-root.dark .plugin-card {
    background: #242426;
    border: 1px solid rgba(255, 255, 255, 0.07);
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.25);
  }

  .plugins-page-root.dark .plugin-card:hover {
    transform: translateY(-2px);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45);
    border-color: rgba(255, 255, 255, 0.12);
  }

  .card-disabled {
    opacity: 0.6;
  }

  .card-body {
    display: flex;
    flex-direction: column;
  }

  .card-header-row {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    margin-bottom: 12px;
  }

  .icon-avatar {
    width: 42px;
    height: 42px;
    border-radius: 10px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 20px;
    flex-shrink: 0;
  }

  .plugins-page-root.light .icon-avatar {
    background: #f5f5f7;
    border: 1px solid rgba(0, 0, 0, 0.06);
  }

  .plugins-page-root.dark .icon-avatar {
    background: #2c2c2e;
    border: 1px solid rgba(255, 255, 255, 0.06);
  }

  .card-meta {
    flex: 1;
    min-width: 0;
  }

  .title-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .plugin-title {
    font-size: 15px;
    font-weight: 600;
    margin: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .plugins-page-root.light .plugin-title {
    color: #1d1d1f;
  }

  .plugins-page-root.dark .plugin-title {
    color: #ffffff;
  }

  .version-tag {
    font-size: 11px;
    font-weight: 600;
    padding: 2px 6px;
    border-radius: 6px;
    color: #86868b;
  }

  .plugins-page-root.light .version-tag {
    background: rgba(0, 0, 0, 0.05);
  }

  .plugins-page-root.dark .version-tag {
    background: rgba(255, 255, 255, 0.07);
  }

  .sub-meta-row {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: #86868b;
    margin-top: 2px;
  }

  .dot-sep {
    opacity: 0.4;
  }

  .source-tag {
    text-transform: capitalize;
  }

  .status-indicator {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 11px;
  }

  .status-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
  }

  .status-active .status-dot {
    background: #34c759;
    box-shadow: 0 0 6px rgba(52, 199, 89, 0.6);
  }

  .status-inactive .status-dot {
    background: #86868b;
  }

  .card-description {
    font-size: 13px;
    line-height: 1.45;
    margin: 0 0 14px;
    min-height: 38px;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .plugins-page-root.light .card-description {
    color: #6e6e73;
  }

  .plugins-page-root.dark .card-description {
    color: #a1a1a6;
  }

  .capabilities-row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-bottom: 12px;
  }

  .cap-badge {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 3px 8px;
    border-radius: 6px;
    font-size: 11px;
    font-weight: 500;
  }

  .cap-badge.mcp {
    background: rgba(0, 113, 227, 0.08);
    color: #0071e3;
    border: 1px solid rgba(0, 113, 227, 0.16);
  }

  .plugins-page-root.dark .cap-badge.mcp {
    background: rgba(41, 151, 255, 0.14);
    color: #2997ff;
    border-color: rgba(41, 151, 255, 0.25);
  }

  .cap-badge.skill {
    background: rgba(175, 82, 222, 0.08);
    color: #af52de;
    border: 1px solid rgba(175, 82, 222, 0.16);
  }

  .plugins-page-root.dark .cap-badge.skill {
    background: rgba(191, 90, 242, 0.14);
    color: #bf5af2;
    border-color: rgba(191, 90, 242, 0.25);
  }

  .keywords-row {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    margin-bottom: 14px;
  }

  .keyword-pill {
    font-size: 11px;
    color: #86868b;
    padding: 2px 6px;
    border-radius: 4px;
  }

  .plugins-page-root.light .keyword-pill {
    background: rgba(0, 0, 0, 0.03);
  }

  .plugins-page-root.dark .keyword-pill {
    background: rgba(255, 255, 255, 0.04);
  }

  .card-footer {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    padding-top: 14px;
    border-top: 1px solid;
    margin-top: auto;
  }

  .plugins-page-root.light .card-footer {
    border-top-color: rgba(0, 0, 0, 0.06);
  }

  .plugins-page-root.dark .card-footer {
    border-top-color: rgba(255, 255, 255, 0.06);
  }

  .installed-footer {
    justify-content: space-between;
  }

  .install-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: #0071e3;
    color: #ffffff;
    border: none;
    border-radius: 8px;
    padding: 7px 16px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .install-btn:hover {
    background: #0077ed;
  }

  .installed-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    background: rgba(52, 199, 89, 0.12);
    color: #34c759;
    border: 1px solid rgba(52, 199, 89, 0.25);
    border-radius: 8px;
    padding: 6px 14px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
  }

  .inspect-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    border-radius: 8px;
    padding: 6px 12px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .plugins-page-root.light .inspect-btn {
    background: rgba(0, 0, 0, 0.04);
    border: 1px solid rgba(0, 0, 0, 0.08);
    color: #1d1d1f;
  }

  .plugins-page-root.light .inspect-btn:hover {
    background: rgba(0, 0, 0, 0.08);
  }

  .plugins-page-root.dark .inspect-btn {
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.08);
    color: #f5f5f7;
  }

  .plugins-page-root.dark .inspect-btn:hover {
    background: rgba(255, 255, 255, 0.1);
  }

  .delete-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    border-radius: 6px;
    background: transparent;
    border: none;
    color: #86868b;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .delete-btn:hover {
    background: rgba(255, 69, 58, 0.12);
    color: #ff453a;
  }

  /* -------------------------------------------------------------
     Apple iOS Toggle Switch
     ------------------------------------------------------------- */
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

  .plugins-page-root.light .switch-track {
    background-color: #e5e5ea;
  }

  .plugins-page-root.dark .switch-track {
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

  /* -------------------------------------------------------------
     Empty & Loading States
     ------------------------------------------------------------- */
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    padding: 60px 20px;
    color: #86868b;
  }

  .empty-icon-box {
    margin-bottom: 12px;
    opacity: 0.35;
  }

  .empty-state h3 {
    font-size: 16px;
    font-weight: 600;
    margin: 0 0 6px;
  }

  .plugins-page-root.light .empty-state h3 {
    color: #1d1d1f;
  }

  .plugins-page-root.dark .empty-state h3 {
    color: #ffffff;
  }

  .empty-state p {
    font-size: 13px;
    max-width: 440px;
    line-height: 1.5;
    margin: 0;
  }

  .reset-filters-btn {
    margin-top: 14px;
    background: transparent;
    border: 1px solid #0071e3;
    color: #0071e3;
    padding: 6px 14px;
    border-radius: 8px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .reset-filters-btn:hover {
    background: rgba(0, 113, 227, 0.08);
  }

  .link-action {
    background: none;
    border: none;
    color: #0071e3;
    cursor: pointer;
    padding: 0;
    font-size: inherit;
    text-decoration: underline;
  }

  .loading-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    padding: 60px 0;
    color: #86868b;
    font-size: 13px;
  }

  /* -------------------------------------------------------------
     Modals (Inspect & Add Plugin)
     ------------------------------------------------------------- */
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.45);
    backdrop-filter: blur(12px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 9999;
    padding: 20px;
    box-sizing: border-box;
  }

  .modal-card {
    border-radius: 18px;
    width: 100%;
    max-width: 620px;
    max-height: 85vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    box-sizing: border-box;
  }

  .modal-card.light {
    background: #ffffff !important;
    color: #1d1d1f !important;
    border: 1px solid rgba(0, 0, 0, 0.12);
    box-shadow: 0 24px 64px rgba(0, 0, 0, 0.2);
  }

  .modal-card.dark {
    background: #1e1e20 !important;
    color: #f5f5f7 !important;
    border: 1px solid rgba(255, 255, 255, 0.12);
    box-shadow: 0 24px 64px rgba(0, 0, 0, 0.6);
  }

  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 20px 24px;
    border-bottom: 1px solid;
  }

  .modal-card.light .modal-header {
    border-bottom-color: rgba(0, 0, 0, 0.08);
    background: #ffffff;
  }

  .modal-card.dark .modal-header {
    border-bottom-color: rgba(255, 255, 255, 0.08);
    background: #1e1e20;
  }

  .modal-title {
    margin: 0;
    font-size: 18px;
    font-weight: 700;
  }

  .modal-card.light .modal-title {
    color: #1d1d1f !important;
  }

  .modal-card.dark .modal-title {
    color: #ffffff !important;
  }

  .modal-subtitle {
    margin: 4px 0 0;
    font-size: 12px;
    color: #86868b;
  }

  .modal-close-btn {
    border: none;
    border-radius: 50%;
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .modal-card.light .modal-close-btn {
    background: rgba(0, 0, 0, 0.06);
    color: #1d1d1f;
  }

  .modal-card.light .modal-close-btn:hover {
    background: rgba(0, 0, 0, 0.1);
  }

  .modal-card.dark .modal-close-btn {
    background: rgba(255, 255, 255, 0.08);
    color: #f5f5f7;
  }

  .modal-card.dark .modal-close-btn:hover {
    background: rgba(255, 255, 255, 0.14);
  }

  .modal-subtabs {
    display: flex;
    padding: 0 20px;
    border-bottom: 1px solid;
  }

  .modal-card.light .modal-subtabs {
    background: #fafafa;
    border-bottom-color: rgba(0, 0, 0, 0.08);
  }

  .modal-card.dark .modal-subtabs {
    background: #18181a;
    border-bottom-color: rgba(255, 255, 255, 0.08);
  }

  .modal-tab {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    padding: 12px 14px;
    font-size: 13px;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .modal-card.light .modal-tab {
    color: #6e6e73;
    font-weight: 500;
  }

  .modal-card.light .modal-tab:hover {
    color: #1d1d1f;
  }

  .modal-card.light .modal-tab.active {
    color: #0071e3 !important;
    border-bottom-color: #0071e3 !important;
    font-weight: 600;
  }

  .modal-card.dark .modal-tab {
    color: #86868b;
    font-weight: 500;
  }

  .modal-card.dark .modal-tab:hover {
    color: #ffffff;
  }

  .modal-card.dark .modal-tab.active {
    color: #2997ff !important;
    border-bottom-color: #2997ff !important;
    font-weight: 600;
  }

  .modal-body-scroll {
    padding: 24px;
    overflow-y: auto;
    flex: 1;
  }

  .modal-body-hint {
    font-size: 13px;
    color: #86868b;
    margin: 0 0 16px;
    line-height: 1.4;
  }

  /* Quick Catalog in Modal */
  .quick-catalog-list {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .quick-catalog-card {
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-radius: 10px;
    padding: 12px 16px;
    border: 1px solid;
    transition: all 0.2s ease;
  }

  .modal-card.light .quick-catalog-card {
    background: #fbfbfd;
    border-color: rgba(0, 0, 0, 0.08);
  }

  .modal-card.dark .quick-catalog-card {
    background: rgba(255, 255, 255, 0.03);
    border-color: rgba(255, 255, 255, 0.06);
  }

  .quick-left {
    display: flex;
    align-items: center;
    gap: 12px;
    flex: 1;
    min-width: 0;
  }

  .quick-avatar {
    font-size: 22px;
    flex-shrink: 0;
  }

  .quick-text {
    flex: 1;
    min-width: 0;
  }

  .quick-title-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .quick-name {
    font-size: 14px;
    font-weight: 600;
  }

  .modal-card.light .quick-name {
    color: #1d1d1f;
  }

  .modal-card.dark .quick-name {
    color: #ffffff;
  }

  .quick-desc {
    display: block;
    font-size: 12px;
    color: #86868b;
    margin-top: 2px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .quick-badges {
    display: flex;
    gap: 6px;
    margin-top: 6px;
  }

  .quick-installed-badge {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    font-weight: 600;
    color: #34c759;
    padding: 4px 10px;
    border-radius: 6px;
    background: rgba(52, 199, 89, 0.12);
  }

  .quick-install-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    background: #0071e3;
    color: #ffffff;
    border: none;
    border-radius: 6px;
    padding: 6px 14px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .quick-install-btn:hover {
    background: #0077ed;
  }

  /* Form Elements in Modal */
  .form-wrapper {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .form-item {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .form-item label {
    font-size: 12px;
    font-weight: 600;
  }

  .modal-card.light .form-item label {
    color: #1d1d1f;
  }

  .modal-card.dark .form-item label {
    color: #e5e5ea;
  }

  .form-input,
  .form-textarea {
    border-radius: 8px;
    padding: 9px 12px;
    font-size: 13px;
    outline: none;
    transition: all 0.2s ease;
    box-sizing: border-box;
  }

  .modal-card.light .form-input,
  .modal-card.light .form-textarea {
    border: 1px solid rgba(0, 0, 0, 0.14);
    background: #ffffff;
    color: #1d1d1f;
  }

  .modal-card.light .form-input:focus,
  .modal-card.light .form-textarea:focus {
    border-color: #0071e3;
    box-shadow: 0 0 0 3px rgba(0, 113, 227, 0.12);
  }

  .modal-card.dark .form-input,
  .modal-card.dark .form-textarea {
    border: 1px solid rgba(255, 255, 255, 0.12);
    background: rgba(255, 255, 255, 0.05);
    color: #ffffff;
  }

  .modal-card.dark .form-input:focus,
  .modal-card.dark .form-textarea:focus {
    border-color: #2997ff;
    box-shadow: 0 0 0 3px rgba(41, 151, 255, 0.2);
  }

  .form-textarea {
    font-family: monospace;
    font-size: 12px;
    resize: vertical;
  }

  .input-hint {
    font-size: 11px;
    color: #86868b;
  }

  .custom-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 14px;
  }

  .full-span {
    grid-column: 1 / -1;
  }

  /* Modal Footer */
  .modal-footer {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    padding: 16px 24px;
    border-top: 1px solid;
  }

  .modal-card.light .modal-footer {
    background: #f8f8fa;
    border-top-color: rgba(0, 0, 0, 0.08);
  }

  .modal-card.dark .modal-footer {
    background: #18181a;
    border-top-color: rgba(255, 255, 255, 0.08);
  }

  .footer-cancel-btn {
    background: transparent;
    border-radius: 8px;
    padding: 8px 16px;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .modal-card.light .footer-cancel-btn {
    border: 1px solid rgba(0, 0, 0, 0.15);
    color: #1d1d1f;
  }

  .modal-card.light .footer-cancel-btn:hover {
    background: rgba(0, 0, 0, 0.04);
  }

  .modal-card.dark .footer-cancel-btn {
    border: 1px solid rgba(255, 255, 255, 0.15);
    color: #f5f5f7;
  }

  .modal-card.dark .footer-cancel-btn:hover {
    background: rgba(255, 255, 255, 0.06);
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

  /* -------------------------------------------------------------
     Guided Auth Installation Modal Styles
     ------------------------------------------------------------- */
  .guided-header-info {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .guided-plugin-icon {
    width: 38px;
    height: 38px;
    border-radius: 10px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 20px;
    background: rgba(0, 113, 227, 0.12);
  }

  .modal-card.dark .guided-plugin-icon {
    background: rgba(41, 151, 255, 0.18);
  }

  .guided-modal-body {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .guided-intro-banner {
    display: flex;
    gap: 12px;
    padding: 12px;
    border-radius: 10px;
    background: rgba(0, 113, 227, 0.08);
    border: 1px solid rgba(0, 113, 227, 0.18);
  }

  .modal-card.dark .guided-intro-banner {
    background: rgba(41, 151, 255, 0.12);
    border-color: rgba(41, 151, 255, 0.22);
  }

  .guided-banner-icon {
    color: #0071e3;
    flex-shrink: 0;
    margin-top: 2px;
  }

  .modal-card.dark .guided-banner-icon {
    color: #2997ff;
  }

  .guided-banner-text p {
    margin: 0;
    font-size: 13px;
    line-height: 1.45;
  }

  .guided-method-tabs {
    display: flex;
    gap: 6px;
    background: rgba(134, 134, 139, 0.08);
    padding: 4px;
    border-radius: 8px;
  }

  .guided-method-tab {
    flex: 1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 6px 12px;
    border: none;
    border-radius: 6px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    background: transparent;
    color: #86868b;
    transition: all 0.15s ease;
  }

  .guided-method-tab.active {
    background: #ffffff;
    color: #1d1d1f;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.12);
  }

  .modal-card.dark .guided-method-tab.active {
    background: #2c2c2e;
    color: #ffffff;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
  }

  .guided-oauth-info, .guided-key-form {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .oauth-desc, .key-prompt {
    font-size: 13px;
    line-height: 1.45;
    color: #86868b;
    margin: 0;
  }

  .scopes-list {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }

  .scopes-title {
    font-size: 11px;
    font-weight: 600;
    color: #86868b;
  }

  .scope-pill {
    font-size: 10px;
    font-family: monospace;
    padding: 2px 6px;
    border-radius: 4px;
    background: rgba(0, 113, 227, 0.1);
    color: #0071e3;
  }

  .modal-card.dark .scope-pill {
    background: rgba(41, 151, 255, 0.15);
    color: #2997ff;
  }

  .doc-link {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 12px;
    color: #0071e3;
    text-decoration: none;
  }

  .doc-link:hover {
    text-decoration: underline;
  }

  .modal-error {
    margin: 0;
  }

  .guided-footer {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }

  .cap-badge.auth {
    background: rgba(175, 82, 222, 0.1);
    color: #af52de;
    border: 1px solid rgba(175, 82, 222, 0.2);
  }
</style>
