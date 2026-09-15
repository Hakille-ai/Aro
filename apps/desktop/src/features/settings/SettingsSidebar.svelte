<script lang="ts">
  import { onDestroy } from "svelte";
  import Activity from "@lucide/svelte/icons/activity";
  import ArrowLeft from "@lucide/svelte/icons/arrow-left";
  import Bot from "@lucide/svelte/icons/bot";
  import Brain from "@lucide/svelte/icons/brain";
  import Building2 from "@lucide/svelte/icons/building-2";
  import Clock from "@lucide/svelte/icons/clock";
  import Cpu from "@lucide/svelte/icons/cpu";
  import FileText from "@lucide/svelte/icons/file-text";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import Keyboard from "@lucide/svelte/icons/keyboard";
  import LineChart from "@lucide/svelte/icons/line-chart";
  import Network from "@lucide/svelte/icons/network";
  import PanelLeftClose from "@lucide/svelte/icons/panel-left-close";
  import PanelLeftOpen from "@lucide/svelte/icons/panel-left-open";
  import Puzzle from "@lucide/svelte/icons/puzzle";
  import Search from "@lucide/svelte/icons/search";
  import Shield from "@lucide/svelte/icons/shield";
  import Sliders from "@lucide/svelte/icons/sliders";
  import Terminal from "@lucide/svelte/icons/terminal";
  import User from "@lucide/svelte/icons/user";
  import Volume2 from "@lucide/svelte/icons/volume-2";
  import X from "@lucide/svelte/icons/x";
  import Zap from "@lucide/svelte/icons/zap";
  import type { SettingsTab } from "./types";

  export let activeTab: SettingsTab;
  export let open: boolean;
  export let width: number;
  export let language: "fr" | "en";
  export let labels: Record<string, string>;
  export let onClose: () => void;

  let isResizing = false;
  let settingsSearchQuery = "";

  function startResizing(event: MouseEvent) {
    event.preventDefault();
    isResizing = true;
    document.body.style.cursor = "col-resize";
    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", stopResizing);
  }

  function handleMouseMove(event: MouseEvent) {
    if (!isResizing) return;
    width = Math.max(160, Math.min(event.clientX, 360));
  }

  function stopResizing() {
    isResizing = false;
    document.body.style.cursor = "";
    window.removeEventListener("mousemove", handleMouseMove);
    window.removeEventListener("mouseup", stopResizing);
  }

  onDestroy(stopResizing);

  function matchesSearch(tabId: string) {
    if (!settingsSearchQuery.trim()) return true;
    const query = settingsSearchQuery.toLowerCase().trim();

    switch (tabId) {
      case "profile":
        return "profil profile nom email clé api avatar".includes(query);
      case "organization":
        return "organisation entreprise équipe collaborateur membre admin manager".includes(query) || "organization team member guest".includes(query);
      case "general":
      case "models":
        return "modèle models provider openai anthropic google mistral ollama llama backend température créativité tokens gemma phi3 api key".includes(query);
      case "system-prompt":
        return "instructions système system prompt instructions generales comportement modes chat think code summarize quiet".includes(query);
      case "instructions":
        return "profils profil personnalités personnalite role preset instructions système prompt rôle développeur reviewer writer écrivain character agent persona".includes(query);
      case "memory":
        return "mémoire memory fait souvenir technique personnel système préférence".includes(query);
      case "voice":
        return "voix audio parole synthèse tts stt parler whisper piper".includes(query) || "voice speech translation".includes(query);
      case "preferences":
        return "préférences preferences langue theme thème français anglais color".includes(query);
      case "skills":
        return "skills compétences outils actions extensions plugins fonctions script créateur".includes(query) || "skills tools functions plugins creator scripts".includes(query);
      case "paths":
        return "chemins paths dossier binaire whisper piper model modèle path".includes(query);
      case "monitoring":
        return "monitoring performance cpu ram gpu vitesse contribution historique".includes(query) || "contributions statistics".includes(query);
      case "system":
        return "maintenance system reset ollama réinitialiser".includes(query);
      case "mcp":
        return "mcp model context protocol serveurs tools outils ressources sse stdio connection".includes(query);
      case "permissions":
        return "permissions autorisations sécurité sécurité accès fichiers réseau domaine path dossier terminaux commandes".includes(query) || "permissions security access file network".includes(query);
      case "search":
        return "moteur recherche search web google duckduckgo brave api key custom endpoint".includes(query) || "search engine key google duckduckgo brave".includes(query);
      case "agents":
        return "agents custom agent definitions equipe team delegation".includes(query) || "agent".includes(query);
      default:
        return false;
    }
  }

  function hasVisibleTabsInGroup(group: string) {
    if (group === "account") return matchesSearch("profile") || matchesSearch("organization");
    if (group === "ia") return matchesSearch("models") || matchesSearch("system-prompt") || matchesSearch("instructions") || matchesSearch("memory") || matchesSearch("permissions") || matchesSearch("agents") || matchesSearch("search");
    if (group === "extensions") return matchesSearch("skills") || matchesSearch("plugins") || matchesSearch("mcp") || matchesSearch("hooks") || matchesSearch("scheduler");
    if (group === "preferences") return matchesSearch("voice") || matchesSearch("preferences");
    if (group === "system") return matchesSearch("paths") || matchesSearch("monitoring") || matchesSearch("system");
    return false;
  }
</script>

<!-- Sidebar Navigation -->
<aside
  class="settings-sidebar"
  class:collapsed={!open}
  style="width: {open ? width : 68}px;"
>
  <div class="settings-sidebar-header" style="margin-bottom: 8px;">
    <div class="settings-sidebar-top-row">
      <button
        class="sidebar-back-btn"
        type="button"
        aria-label={labels.backToChat}
        title={labels.backToChat}
        on:click={onClose}
      >
        <ArrowLeft size={16} />
        <span class="settings-back-text">{labels.back}</span>
      </button>
      <button
        class="settings-sidebar-toggle-btn-inner"
        type="button"
        title={open ? labels.hideSidebar : labels.showSidebar}
        aria-label={open ? labels.hideSidebar : labels.showSidebar}
        on:click={() => (open = !open)}
      >
        {#if open}
          <PanelLeftClose size={16} />
        {:else}
          <PanelLeftOpen size={16} />
        {/if}
      </button>
    </div>
    <h1 class="sidebar-title settings-title-text">{labels.settings}</h1>
  </div>

    <!-- Tab Search Bar -->
    <div class="settings-search-wrapper" style="margin-top: 4px; margin-bottom: 12px; position: relative; display: flex; align-items: center; width: 100%;">
      <Search size={13} style="position: absolute; left: 8px; color: #86868b; flex-shrink: 0;" />
      <input
        type="text"
        placeholder={language === "fr" ? "Rechercher..." : "Search..."}
        bind:value={settingsSearchQuery}
        class="settings-search-input"
      />
      {#if settingsSearchQuery}
        <button
          type="button"
          on:click={() => (settingsSearchQuery = "")}
          style="position: absolute; right: 8px; background: transparent; border: none; cursor: pointer; color: #86868b; padding: 2px; display: flex; align-items: center; justify-content: center; outline: none;"
        >
          <X size={12} />
        </button>
      {/if}
    </div>

    <nav class="settings-sidebar-nav">
      {#if hasVisibleTabsInGroup("account")}
        <div class="settings-nav-section-title">{labels.groupAccount}</div>
        {#if matchesSearch("profile")}
          <button
            type="button"
            class="sidebar-nav-item"
            class:active={activeTab === "profile"}
            on:click={() => (activeTab = "profile")}
          >
            <User size={16} />
            <span>{labels.profileTab}</span>
          </button>
        {/if}
        {#if matchesSearch("organization")}
          <button
            type="button"
            class="sidebar-nav-item"
            class:active={activeTab === "organization"}
            on:click={() => (activeTab = "organization")}
          >
            <Building2 size={16} />
            <span>{labels.orgTab}</span>
          </button>
        {/if}
      {/if}

      {#if hasVisibleTabsInGroup("ia")}
        <div class="settings-nav-section-title" style="margin-top: 14px;">{labels.groupApp}</div>
        {#if matchesSearch("models")}
          <button
            type="button"
            class="sidebar-nav-item"
            class:active={activeTab === "models"}
            on:click={() => (activeTab = "models")}
          >
            <Cpu size={16} />
            <span>{labels.modelTab}</span>
          </button>
        {/if}
        {#if matchesSearch("system-prompt")}
          <button
            type="button"
            class="sidebar-nav-item"
            class:active={activeTab === "system-prompt"}
            on:click={() => (activeTab = "system-prompt")}
          >
            <FileText size={16} />
            <span>{language === "fr" ? "Instructions ARO" : "ARO Instructions"}</span>
          </button>
        {/if}
        {#if matchesSearch("instructions")}
          <button
            type="button"
            class="sidebar-nav-item"
            class:active={activeTab === "instructions"}
            on:click={() => (activeTab = "instructions")}
          >
            <Bot size={16} />
            <span>{labels.instructionsTab}</span>
          </button>
        {/if}
        {#if matchesSearch("memory")}
          <button
            type="button"
            class="sidebar-nav-item"
            class:active={activeTab === "memory"}
            on:click={() => (activeTab = "memory")}
          >
            <Brain size={16} />
            <span>{labels.memoryTab}</span>
          </button>
        {/if}
        {#if matchesSearch("permissions")}
          <button
            type="button"
            class="sidebar-nav-item"
            class:active={activeTab === "permissions"}
            on:click={() => (activeTab = "permissions")}
          >
            <Shield size={16} />
            <span>{labels.permissionsTab}</span>
          </button>
        {/if}
        {#if matchesSearch("agents")}
          <button
            type="button"
            class="sidebar-nav-item"
            class:active={activeTab === "agents"}
            on:click={() => (activeTab = "agents")}
          >
            <Bot size={16} />
            <span>{language === "fr" ? "Agents" : "Agents"}</span>
          </button>
        {/if}
        {#if matchesSearch("search")}
          <button
            type="button"
            class="sidebar-nav-item"
            class:active={activeTab === "search"}
            on:click={() => (activeTab = "search")}
          >
            <Search size={16} />
            <span>{language === "fr" ? "Recherche Web" : "Web Search"}</span>
          </button>
        {/if}
      {/if}

      {#if hasVisibleTabsInGroup("extensions")}
        <div class="settings-nav-section-title" style="margin-top: 14px;">{labels.groupExtensions}</div>
        {#if matchesSearch("skills")}
          <button
            type="button"
            class="sidebar-nav-item"
            class:active={activeTab === "skills"}
            on:click={() => (activeTab = "skills")}
          >
            <Terminal size={16} />
            <span>Skills</span>
          </button>
        {/if}
        {#if matchesSearch("plugins")}
          <button
            type="button"
            class="sidebar-nav-item"
            class:active={activeTab === "plugins"}
            on:click={() => (activeTab = "plugins")}
          >
            <Puzzle size={16} />
            <span>Plugins</span>
          </button>
        {/if}
        {#if matchesSearch("mcp")}
          <button
            type="button"
            class="sidebar-nav-item"
            class:active={activeTab === "mcp"}
            on:click={() => (activeTab = "mcp")}
          >
            <Network size={16} />
            <span>MCP Servers</span>
          </button>
        {/if}
        {#if matchesSearch("hooks")}
          <button
            type="button"
            class="sidebar-nav-item"
            class:active={activeTab === "hooks"}
            on:click={() => (activeTab = "hooks")}
          >
            <Zap size={16} />
            <span>Hooks & Webhooks</span>
          </button>
        {/if}
        {#if matchesSearch("scheduler")}
          <button
            type="button"
            class="sidebar-nav-item"
            class:active={activeTab === "scheduler"}
            on:click={() => (activeTab = "scheduler")}
          >
            <Clock size={16} />
            <span>Planificateur</span>
          </button>
        {/if}
      {/if}

      {#if hasVisibleTabsInGroup("preferences")}
        <div class="settings-nav-section-title" style="margin-top: 14px;">{labels.groupPreferences}</div>
        {#if matchesSearch("voice")}
          <button
            type="button"
            class="sidebar-nav-item"
            class:active={activeTab === "voice"}
            on:click={() => (activeTab = "voice")}
          >
            <Volume2 size={16} />
            <span>{labels.voiceTab}</span>
          </button>
        {/if}
        {#if matchesSearch("preferences")}
          <button
            type="button"
            class="sidebar-nav-item"
            class:active={activeTab === "preferences"}
            on:click={() => (activeTab = "preferences")}
          >
            <Sliders size={16} />
            <span>{labels.preferencesTab}</span>
          </button>
        {/if}
        {#if matchesSearch("shortcuts") || matchesSearch("raccourcis")}
          <button
            type="button"
            class="sidebar-nav-item"
            class:active={activeTab === "shortcuts"}
            on:click={() => (activeTab = "shortcuts")}
          >
            <Keyboard size={16} />
            <span>{language === "fr" ? "Raccourcis" : "Shortcuts"}</span>
          </button>
        {/if}
      {/if}

      {#if hasVisibleTabsInGroup("system")}
        <div class="settings-nav-section-title" style="margin-top: 14px;">{labels.groupSystem}</div>
        {#if matchesSearch("paths")}
          <button
            type="button"
            class="sidebar-nav-item"
            class:active={activeTab === "paths"}
            on:click={() => (activeTab = "paths")}
          >
            <FolderOpen size={16} />
            <span>{labels.pathsTab}</span>
          </button>
        {/if}
        {#if matchesSearch("monitoring")}
          <button
            type="button"
            class="sidebar-nav-item"
            class:active={activeTab === "monitoring"}
            on:click={() => (activeTab = "monitoring")}
          >
            <LineChart size={16} />
            <span>{labels.monitoringTab}</span>
          </button>
        {/if}
        {#if matchesSearch("system")}
          <button
            type="button"
            class="sidebar-nav-item"
            class:active={activeTab === "system"}
            on:click={() => (activeTab = "system")}
          >
            <Activity size={16} />
            <span>{labels.maintenanceTab}</span>
          </button>
        {/if}
      {/if}
    </nav>
  </aside>
  {#if open}
    <!-- Resizer Separator -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      class="settings-sidebar-resizer"
      class:resizing={isResizing}
      role="separator"
      aria-label="Resize settings sidebar"
      on:mousedown={startResizing}
    ></div>
  {/if}
