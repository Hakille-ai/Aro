<script lang="ts">
  import { onMount } from "svelte";
  import X from "@lucide/svelte/icons/x";
  import Bot from "@lucide/svelte/icons/bot";
  import Sliders from "@lucide/svelte/icons/sliders";
  import Globe from "@lucide/svelte/icons/globe";
  import Link from "@lucide/svelte/icons/link";
  import Users from "@lucide/svelte/icons/users";
  import Check from "@lucide/svelte/icons/check";
  import Shield from "@lucide/svelte/icons/shield";
  import Cpu from "@lucide/svelte/icons/cpu";
  import Puzzle from "@lucide/svelte/icons/puzzle";
  import Terminal from "@lucide/svelte/icons/terminal";
  import Edit2 from "@lucide/svelte/icons/edit-2";
  import Brain from "@lucide/svelte/icons/brain";
  import User from "@lucide/svelte/icons/user";
  import Activity from "@lucide/svelte/icons/activity";
  import Building2 from "@lucide/svelte/icons/building-2";
  import Search from "@lucide/svelte/icons/search";
  import type { ConversationPersonality } from "./ConversationTopbar.svelte";

  export let language: "fr" | "en";
  export let writeLocked: boolean;
  export let agent: any = null; // null if new, or existing custom agent object
  export let personalities: ConversationPersonality[] = [];
  export let modelProviders: any[] = [];
  export let modelOptions: any[] = [];
  export let permissionProfiles: any[] = [];
  export let userSkills: any[] = [];
  export let userPlugins: any[] = [];
  export let onClose: () => void;
  export let onSave: (agent: any) => void | Promise<void>;

  // Form values
  let name = "";
  let description = "";
  let systemPrompt = "";
  let selectedProviderId = "";
  let selectedModelId = "";
  let permissionProfileId = "";
  let commandApproval = ""; // "" (inherit), "always", "safe-auto", "never"
  let selectedIcon = "bot";
  let enabledTools: string[] = [];

  // Collapsible tools list state
  let showToolsExpanded = false;

  // Temp templates selection
  let selectedTemplateId = "";

  const availableIcons = [
    { id: "bot", component: Bot },
    { id: "code", component: Terminal },
    { id: "check", component: Check },
    { id: "edit-2", component: Edit2 },
    { id: "brain", component: Brain },
    { id: "cpu", component: Cpu },
    { id: "user", component: User },
    { id: "activity", component: Activity },
    { id: "sliders", component: Sliders },
    { id: "building-2", component: Building2 },
    { id: "search", component: Search }
  ];

  onMount(() => {
    if (agent) {
      name = agent.name || "";
      description = agent.description || "";
      systemPrompt = agent.systemPrompt || agent.system_prompt || "";
      selectedProviderId = agent.modelProviderId || agent.model_provider_id || "";
      selectedModelId = agent.modelId || agent.model_id || "";
      permissionProfileId = agent.permissionProfileId || agent.permission_profile_id || "";
      selectedIcon = agent.icon || "bot";
      commandApproval = agent.commandApproval || agent.command_approval || "";
      if (Array.isArray(agent.enabledTools || agent.enabled_tools)) {
        enabledTools = [...(agent.enabledTools || agent.enabled_tools)];
      }
    } else {
      // By default, check all tools (system + user skills + user plugins)
      const defaultTools = ["web.search", "web.fetch", "agent.delegate"];
      const skillIds = userSkills.map((s) => s.id);
      const pluginIds = userPlugins.map((p) => p.id);
      enabledTools = [...defaultTools, ...skillIds, ...pluginIds];
    }
  });

  // Filter models based on selected provider
  $: filteredModels = modelOptions.filter(
    (m) => !selectedProviderId || m.providerId === selectedProviderId
  );

  function handleSave() {
    if (!name.trim()) return;
    onSave({
      id: agent?.id || undefined,
      cloudId: agent?.cloudId || undefined,
      name: name.trim(),
      description: description.trim(),
      prompt: systemPrompt.trim(),
      systemPrompt: systemPrompt.trim(),
      modelProviderId: selectedProviderId,
      modelId: selectedModelId,
      permissionProfileId: permissionProfileId,
      commandApproval: commandApproval,
      icon: selectedIcon,
      enabledTools
    });
    onClose();
  }

  function handleTemplateChange(e: Event) {
    const target = e.target as HTMLSelectElement;
    const templateId = target.value;
    if (!templateId) return;
    const found = personalities.find((p) => p.id === templateId);
    if (found) {
      systemPrompt = found.prompt || "";
      if (!name) name = found.name;
      if (!description) description = found.description;
      if (found.icon) selectedIcon = found.icon;
    }
    selectedTemplateId = ""; // Reset selector
  }

  function toggleTool(tool: string) {
    if (enabledTools.includes(tool)) {
      enabledTools = enabledTools.filter((t) => t !== tool);
    } else {
      enabledTools = [...enabledTools, tool];
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="modal-backdrop animate-fade-in" role="presentation" on:click={onClose}>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="modal-window animate-slide-up" on:click|stopPropagation>
    <div class="modal-header">
      <div class="title-wrap">
        <span class="header-icon"><Bot size={18} /></span>
        <h3>
          {agent?.id
            ? (language === "fr" ? "Modifier l'Agent Autonome" : "Edit Autonomous Agent")
            : (language === "fr" ? "Nouvel Agent Autonome" : "New Autonomous Agent")}
        </h3>
      </div>
      <button type="button" class="close-btn" on:click={onClose}>
        <X size={16} />
      </button>
    </div>

    <div class="modal-body">
      <!-- Icon Selector Row -->
      <div class="form-group">
        <span class="form-label">{language === "fr" ? "Icône de l'Agent" : "Agent Icon"}</span>
        <div class="icon-selector-grid">
          {#each availableIcons as iconOpt}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
            <div
              class="icon-option-btn"
              class:selected={selectedIcon === iconOpt.id}
              on:click={() => !writeLocked && (selectedIcon = iconOpt.id)}
              title={iconOpt.id}
            >
              <svelte:component this={iconOpt.component} size={16} />
            </div>
          {/each}
        </div>
      </div>

      <!-- Agent Identity Row -->
      <div class="form-row two-cols">
        <div class="form-group">
          <label for="modal-agent-name">
            {language === "fr" ? "Nom de l'Agent" : "Agent Name"} <span class="required">*</span>
          </label>
          <input
            type="text"
            id="modal-agent-name"
            bind:value={name}
            disabled={writeLocked}
            placeholder={language === "fr" ? "Ex: Assistant de Recherche" : "e.g. Research Assistant"}
          />
        </div>

        <div class="form-group">
          <label for="modal-agent-desc">
            {language === "fr" ? "Rôle / Description" : "Role / Description"}
          </label>
          <input
            type="text"
            id="modal-agent-desc"
            bind:value={description}
            disabled={writeLocked}
            placeholder={language === "fr" ? "Ex: Expert en analyse de code" : "e.g. Code analysis expert"}
          />
        </div>
      </div>

      <!-- Instructions & Presets Row -->
      <div class="form-group">
        <div class="group-header-row">
          <label for="modal-agent-prompt">
            {language === "fr" ? "Instructions Système" : "System Instructions"}
          </label>
          
          {#if personalities.length > 0}
            <div class="template-selector-wrap">
              <span class="template-label">{language === "fr" ? "Pré-remplir :" : "Pre-fill:"}</span>
              <select value={selectedTemplateId} on:change={handleTemplateChange} disabled={writeLocked}>
                <option value="">{language === "fr" ? "Choisir un profil..." : "Select profile..."}</option>
                {#each personalities as p}
                  <option value={p.id}>{p.name}</option>
                {/each}
              </select>
            </div>
          {/if}
        </div>
        <textarea
          id="modal-agent-prompt"
          rows="5"
          bind:value={systemPrompt}
          disabled={writeLocked}
          placeholder={language === "fr" ? "Définissez les règles et le comportement de votre agent..." : "Define the rules and behavior of your agent..."}
        ></textarea>
      </div>

      <!-- Models Configuration Row -->
      <div class="form-row two-cols">
        <div class="form-group">
          <label for="modal-agent-provider">
            {language === "fr" ? "Fournisseur de Modèle" : "Model Provider"}
          </label>
          <select id="modal-agent-provider" bind:value={selectedProviderId} disabled={writeLocked}>
            <option value="">{language === "fr" ? "Modèle système (par défaut)" : "System model (default)"}</option>
            {#each modelProviders as provider}
              <option value={provider.id}>{provider.name}</option>
            {/each}
          </select>
        </div>

        <div class="form-group">
          <label for="modal-agent-model">
            {language === "fr" ? "Modèle IA" : "AI Model"}
          </label>
          <select id="modal-agent-model" bind:value={selectedModelId} disabled={writeLocked}>
            <option value="">{language === "fr" ? "Modèle système (par défaut)" : "System model (default)"}</option>
            {#each filteredModels as model}
              <option value={model.id}>{model.name}</option>
            {/each}
          </select>
        </div>
      </div>

      <!-- Accès & Autorisations (Permissions) -->
      <div class="form-row two-cols">
        <div class="form-group">
          <label for="modal-agent-permissions">
            {language === "fr" ? "Profil d'Accès / Autorisations" : "Access / Permissions Profile"}
          </label>
          <div style="display: flex; align-items: center; gap: 8px;">
            <span style="color: #af52de; display: inline-flex;"><Shield size={16} /></span>
            <select 
              id="modal-agent-permissions" 
              bind:value={permissionProfileId} 
              disabled={writeLocked} 
              style="flex: 1;"
            >
              <option value="">{language === "fr" ? "Permissions de l'organisation (par défaut)" : "Organization permissions (default)"}</option>
              {#each permissionProfiles as profile}
                <option value={profile.id}>{profile.name}</option>
              {/each}
            </select>
          </div>
        </div>

        <div class="form-group">
          <label for="modal-agent-approval">
            {language === "fr" ? "Approbation des Commandes" : "Command Approval Mode"}
          </label>
          <div style="display: flex; align-items: center; gap: 8px;">
            <span style="color: #af52de; display: inline-flex;"><Sliders size={16} /></span>
            <select 
              id="modal-agent-approval" 
              bind:value={commandApproval} 
              disabled={writeLocked} 
              style="flex: 1;"
            >
              <option value="">{language === "fr" ? "Hériter du profil d'accès (par défaut)" : "Inherit from access profile (default)"}</option>
              <option value="always">{language === "fr" ? "Toujours demander validation" : "Always ask for approval"}</option>
              <option value="safe-auto">{language === "fr" ? "Auto pour actions sûres" : "Auto for safe actions"}</option>
              <option value="never">{language === "fr" ? "Exécuter sans jamais demander" : "Never ask (unsafe)"}</option>
            </select>
          </div>
        </div>
      </div>

      <!-- Collapsible Tools Section -->
      <div class="form-group" style="margin-top: 4px;">
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="expandable-section-header" on:click={() => (showToolsExpanded = !showToolsExpanded)}>
          <span class="section-title">
            <span style="color: #af52de; display: inline-flex;"><Puzzle size={15} /></span>
            <span>{language === "fr" ? "Capacités & Outils de l'Agent" : "Agent Capabilities & Tools"}</span>
            <span class="count-badge">{enabledTools.length} {language === "fr" ? "actifs" : "active"}</span>
          </span>
          <span class="expand-arrow" class:expanded={showToolsExpanded}>▼</span>
        </div>

        {#if showToolsExpanded}
          <div class="expanded-tools-container animate-fade-in" style="margin-top: 10px; display: flex; flex-direction: column; gap: 12px;">
            <!-- Core Tools -->
            <div>
              <span class="sub-label">{language === "fr" ? "Outils Système" : "System Tools"}</span>
              <div class="capabilities-checkbox-grid">
                <!-- Web Search -->
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
                <div
                  class="capability-checkbox-card"
                  class:active={enabledTools.includes("web.search")}
                  on:click={() => !writeLocked && toggleTool("web.search")}
                >
                  <span class="icon-wrap search"><Globe size={14} /></span>
                  <div class="checkbox-info">
                    <span class="checkbox-title">{language === "fr" ? "Recherche Web" : "Web Search"}</span>
                    <span class="checkbox-desc">{language === "fr" ? "Recherche en direct sur internet" : "Live search on the internet"}</span>
                  </div>
                  <div class="custom-checkbox" class:checked={enabledTools.includes("web.search")}>
                    {#if enabledTools.includes("web.search")}<Check size={10} />{/if}
                  </div>
                </div>

                <!-- Web Fetch -->
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
                <div
                  class="capability-checkbox-card"
                  class:active={enabledTools.includes("web.fetch")}
                  on:click={() => !writeLocked && toggleTool("web.fetch")}
                >
                  <span class="icon-wrap fetch"><Link size={14} /></span>
                  <div class="checkbox-info">
                    <span class="checkbox-title">{language === "fr" ? "Extraction de Contenu" : "Web Fetch"}</span>
                    <span class="checkbox-desc">{language === "fr" ? "Le contenu des pages" : "The page content"}</span>
                  </div>
                  <div class="custom-checkbox" class:checked={enabledTools.includes("web.fetch")}>
                    {#if enabledTools.includes("web.fetch")}<Check size={10} />{/if}
                  </div>
                </div>

                <!-- Team Delegation -->
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
                <div
                  class="capability-checkbox-card"
                  class:active={enabledTools.includes("agent.delegate")}
                  on:click={() => !writeLocked && toggleTool("agent.delegate")}
                >
                  <span class="icon-wrap delegate"><Users size={14} /></span>
                  <div class="checkbox-info">
                    <span class="checkbox-title">{language === "fr" ? "Délégation" : "Delegation"}</span>
                    <span class="checkbox-desc">{language === "fr" ? "Délègue à d'autres agents" : "Delegates to other agents"}</span>
                  </div>
                  <div class="custom-checkbox" class:checked={enabledTools.includes("agent.delegate")}>
                    {#if enabledTools.includes("agent.delegate")}<Check size={10} />{/if}
                  </div>
                </div>
              </div>
            </div>

            <!-- User Skills -->
            {#if userSkills.length > 0}
              <div>
                <span class="sub-label">{language === "fr" ? "Skills Utilisateur" : "User Skills"}</span>
                <div class="capabilities-checkbox-grid">
                  {#each userSkills as skill}
                    <!-- svelte-ignore a11y_click_events_have_key_events -->
                    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
                    <div
                      class="capability-checkbox-card"
                      class:active={enabledTools.includes(skill.id)}
                      on:click={() => !writeLocked && toggleTool(skill.id)}
                    >
                      <span class="icon-wrap skill-icon"><Cpu size={14} /></span>
                      <div class="checkbox-info">
                        <span class="checkbox-title">{skill.name}</span>
                        <span class="checkbox-desc" title={skill.description}>{skill.description || (language === "fr" ? "Skill personnalisé" : "Custom skill")}</span>
                      </div>
                      <div class="custom-checkbox" class:checked={enabledTools.includes(skill.id)}>
                        {#if enabledTools.includes(skill.id)}<Check size={10} />{/if}
                      </div>
                    </div>
                  {/each}
                </div>
              </div>
            {/if}

            <!-- Plugins -->
            {#if userPlugins.length > 0}
              <div>
                <span class="sub-label">{language === "fr" ? "Plugins Installés" : "Installed Plugins"}</span>
                <div class="capabilities-checkbox-grid">
                  {#each userPlugins as plugin}
                    <!-- svelte-ignore a11y_click_events_have_key_events -->
                    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
                    <div
                      class="capability-checkbox-card"
                      class:active={enabledTools.includes(plugin.id)}
                      on:click={() => !writeLocked && toggleTool(plugin.id)}
                    >
                      <span class="icon-wrap plugin-icon"><Puzzle size={14} /></span>
                      <div class="checkbox-info">
                        <span class="checkbox-title">{plugin.name}</span>
                        <span class="checkbox-desc" title={plugin.description}>{plugin.description || (language === "fr" ? "Plugin d'intégration" : "Integration plugin")}</span>
                      </div>
                      <div class="custom-checkbox" class:checked={enabledTools.includes(plugin.id)}>
                        {#if enabledTools.includes(plugin.id)}<Check size={10} />{/if}
                      </div>
                    </div>
                  {/each}
                </div>
              </div>
            {/if}
          </div>
        {/if}
      </div>
    </div>

    <div class="modal-footer">
      <button type="button" class="cancel-btn" on:click={onClose}>
        {language === "fr" ? "Annuler" : "Cancel"}
      </button>
      <button type="button" class="save-btn" disabled={!name.trim() || writeLocked} on:click={handleSave}>
        {language === "fr" ? "Enregistrer" : "Save"}
      </button>
    </div>
  </div>
</div>

<style>
  /* MODAL BACKDROP */
  .modal-backdrop {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.4);
    backdrop-filter: blur(15px);
    -webkit-backdrop-filter: blur(15px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 9999;
    padding: 20px;
  }

  /* MODAL WINDOW */
  .modal-window {
    background: #ffffff;
    border: 1px solid rgba(0, 0, 0, 0.08);
    border-radius: 16px;
    width: 100%;
    max-width: 650px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.15);
    display: flex;
    flex-direction: column;
    max-height: calc(100vh - 40px);
    overflow: hidden;
  }

  :global(body.dark-theme) .modal-window {
    background: #1c1c1e;
    border-color: rgba(255, 255, 255, 0.08);
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.35);
  }

  /* HEADER */
  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 20px;
    border-bottom: 1px solid rgba(0, 0, 0, 0.05);
  }

  :global(body.dark-theme) .modal-header {
    border-bottom-color: rgba(255, 255, 255, 0.05);
  }

  .title-wrap {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .header-icon {
    color: #af52de;
    display: inline-flex;
  }

  .modal-header h3 {
    margin: 0;
    font-size: 15px;
    font-weight: 700;
    color: #1d1d1f;
  }

  :global(body.dark-theme) .modal-header h3 {
    color: #ffffff;
  }

  .close-btn {
    background: transparent;
    border: none;
    color: #86868b;
    cursor: pointer;
    padding: 4px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background 0.15s ease;
  }

  .close-btn:hover {
    background: rgba(0, 0, 0, 0.05);
    color: #1d1d1f;
  }

  :global(body.dark-theme) .close-btn:hover {
    background: rgba(255, 255, 255, 0.05);
    color: #ffffff;
  }

  /* BODY */
  .modal-body {
    padding: 20px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 16px;
    
    /* CUSTOM SLEEK SCROLLBAR FOR MODAL BODY */
    scrollbar-width: thin;
    scrollbar-color: rgba(175, 82, 222, 0.25) transparent;
  }

  :global(body.dark-theme) .modal-body {
    scrollbar-color: rgba(175, 82, 222, 0.4) transparent;
  }

  .modal-body::-webkit-scrollbar {
    width: 6px;
    height: 6px;
  }

  .modal-body::-webkit-scrollbar-track {
    background: transparent;
  }

  .modal-body::-webkit-scrollbar-thumb {
    background: rgba(175, 82, 222, 0.25);
    border-radius: 10px;
  }

  :global(body.dark-theme) .modal-body::-webkit-scrollbar-thumb {
    background: rgba(175, 82, 222, 0.4);
  }

  .modal-body::-webkit-scrollbar-thumb:hover {
    background: rgba(175, 82, 222, 0.45);
  }

  :global(body.dark-theme) .modal-body::-webkit-scrollbar-thumb:hover {
    background: rgba(175, 82, 222, 0.6);
  }

  /* EXPANDABLE SECTION */
  .expandable-section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 14px;
    background: #f5f5f7;
    border: 1px solid rgba(0, 0, 0, 0.08);
    border-radius: 10px;
    cursor: pointer;
    transition: all 0.15s ease;
    user-select: none;
  }

  :global(body.dark-theme) .expandable-section-header {
    background: rgba(255, 255, 255, 0.02);
    border-color: rgba(255, 255, 255, 0.08);
  }

  .expandable-section-header:hover {
    background: rgba(0, 0, 0, 0.05);
    border-color: rgba(175, 82, 222, 0.25);
  }

  :global(body.dark-theme) .expandable-section-header:hover {
    background: rgba(255, 255, 255, 0.04);
  }

  .section-title {
    font-size: 12.5px;
    font-weight: 600;
    color: #1d1d1f;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  :global(body.dark-theme) .section-title {
    color: #ffffff;
  }

  .count-badge {
    font-size: 10px;
    background: rgba(175, 82, 222, 0.12);
    color: #af52de;
    padding: 2px 6px;
    border-radius: 8px;
    font-weight: 700;
  }

  .expand-arrow {
    font-size: 9px;
    color: #86868b;
    transition: transform 0.2s ease;
  }

  .expand-arrow.expanded {
    transform: rotate(180deg);
  }

  /* ICON SELECTOR GRID */
  .icon-selector-grid {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    padding: 10px;
    background: #f5f5f7;
    border-radius: 10px;
    border: 1px solid rgba(0, 0, 0, 0.05);
  }

  :global(body.dark-theme) .icon-selector-grid {
    background: rgba(255, 255, 255, 0.01);
    border-color: rgba(255, 255, 255, 0.05);
  }

  .icon-option-btn {
    width: 32px;
    height: 32px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #ffffff;
    border: 1px solid rgba(0, 0, 0, 0.08);
    color: #86868b;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  :global(body.dark-theme) .icon-option-btn {
    background: rgba(255, 255, 255, 0.03);
    border-color: rgba(255, 255, 255, 0.08);
    color: #a1a1a6;
  }

  .icon-option-btn:hover {
    color: #af52de;
    border-color: rgba(175, 82, 222, 0.3);
    transform: scale(1.08);
  }

  .icon-option-btn.selected {
    background: #af52de;
    color: #ffffff;
    border-color: #af52de;
    box-shadow: 0 2px 8px rgba(175, 82, 222, 0.35);
  }

  .form-row {
    display: flex;
    gap: 16px;
  }

  .form-row.two-cols > .form-group {
    flex: 1;
    min-width: 0;
  }

  .form-group {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .sub-label {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    color: #86868b;
    letter-spacing: 0.5px;
    margin-bottom: 6px;
    display: block;
  }

  .group-header-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .template-selector-wrap {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .template-label {
    font-size: 10.5px;
    color: #86868b;
  }

  .template-selector-wrap select {
    padding: 3px 8px;
    font-size: 10.5px;
    border-radius: 6px;
    background: #f5f5f7;
    border: 1px solid rgba(0, 0, 0, 0.05);
    color: #1d1d1f;
    cursor: pointer;
  }

  :global(body.dark-theme) .template-selector-wrap select {
    background: rgba(255, 255, 255, 0.04);
    border-color: rgba(255, 255, 255, 0.05);
    color: #ffffff;
  }

  .form-group label {
    font-size: 11.5px;
    font-weight: 600;
    color: #86868b;
  }

  .required {
    color: #ff3b30;
  }

  .form-group input,
  .form-group textarea,
  .form-group select {
    border-radius: 8px;
    border: 1px solid rgba(0, 0, 0, 0.1);
    background: #ffffff;
    padding: 8px 12px;
    font-size: 12.5px;
    color: #1d1d1f;
    outline: none;
    transition: all 0.15s ease;
  }

  /* DROPDOWN SELECT OPTION COLOR BUG FIX */
  select {
    background-color: #ffffff !important;
    color: #1d1d1f !important;
    border: 1px solid rgba(0, 0, 0, 0.15) !important;
  }

  select option {
    background-color: #ffffff !important;
    color: #1d1d1f !important;
  }

  :global(body.dark-theme) select {
    background-color: #1c1c1e !important;
    color: #ffffff !important;
    border: 1px solid rgba(255, 255, 255, 0.15) !important;
  }

  :global(body.dark-theme) select option {
    background-color: #1c1c1e !important;
    color: #ffffff !important;
  }

  :global(body.dark-theme) .form-group input,
  :global(body.dark-theme) .form-group textarea {
    background: rgba(255, 255, 255, 0.02);
    border-color: rgba(255, 255, 255, 0.08);
    color: #ffffff;
  }

  .form-group input:focus,
  .form-group textarea:focus,
  .form-group select:focus {
    border-color: #af52de;
    box-shadow: 0 0 0 3px rgba(175, 82, 222, 0.15);
  }

  .form-group textarea {
    resize: vertical;
    font-family: inherit;
    line-height: 1.4;
  }

  /* CAPABILITY CHECKBOX GRID */
  .capabilities-checkbox-grid {
    display: grid;
    grid-template-columns: 1fr;
    gap: 8px;
  }

  @media (min-width: 480px) {
    .capabilities-checkbox-grid {
      grid-template-columns: repeat(3, 1fr);
    }
  }

  .capability-checkbox-card {
    border: 1px solid rgba(0, 0, 0, 0.08);
    border-radius: 10px;
    padding: 8px 10px;
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    transition: all 0.15s ease;
    background: #f5f5f7;
    position: relative;
  }

  :global(body.dark-theme) .capability-checkbox-card {
    background: rgba(255, 255, 255, 0.01);
    border-color: rgba(255, 255, 255, 0.05);
  }

  .capability-checkbox-card:hover {
    border-color: rgba(175, 82, 222, 0.2);
    background: #ffffff;
  }

  :global(body.dark-theme) .capability-checkbox-card:hover {
    background: rgba(255, 255, 255, 0.03);
  }

  .capability-checkbox-card.active {
    border-color: #af52de;
    background: rgba(175, 82, 222, 0.04);
  }

  :global(body.dark-theme) .capability-checkbox-card.active {
    background: rgba(175, 82, 222, 0.08);
  }

  .icon-wrap {
    width: 24px;
    height: 24px;
    border-radius: 6px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .icon-wrap.search { background: rgba(52, 152, 219, 0.1); color: #3498db; }
  .icon-wrap.fetch { background: rgba(46, 204, 113, 0.1); color: #2ecc71; }
  .icon-wrap.delegate { background: rgba(155, 89, 182, 0.1); color: #af52de; }
  .icon-wrap.skill-icon { background: rgba(241, 196, 15, 0.1); color: #f1c40f; }
  .icon-wrap.plugin-icon { background: rgba(230, 126, 34, 0.1); color: #e67e22; }

  .checkbox-info {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }

  .checkbox-title {
    font-size: 11px;
    font-weight: 600;
    color: #1d1d1f;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  :global(body.dark-theme) .checkbox-title {
    color: #ffffff;
  }

  .checkbox-desc {
    font-size: 9px;
    color: #86868b;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .custom-checkbox {
    width: 14px;
    height: 14px;
    border-radius: 4px;
    border: 1px solid rgba(0, 0, 0, 0.15);
    background: #ffffff;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #ffffff;
    flex-shrink: 0;
    transition: all 0.12s ease;
  }

  :global(body.dark-theme) .custom-checkbox {
    background: rgba(0, 0, 0, 0.2);
    border-color: rgba(255, 255, 255, 0.2);
  }

  .custom-checkbox.checked {
    background: #af52de;
    border-color: #af52de;
  }

  /* FOOTER */
  .modal-footer {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
    padding: 12px 20px;
    border-top: 1px solid rgba(0, 0, 0, 0.05);
    background: #f5f5f7;
  }

  :global(body.dark-theme) .modal-footer {
    border-top-color: rgba(255, 255, 255, 0.05);
    background: rgba(30, 30, 35, 0.5);
  }

  .modal-footer button {
    font-size: 11.5px;
    font-weight: 600;
    padding: 6px 16px;
    border-radius: 8px;
    border: none;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .cancel-btn {
    background: rgba(0, 0, 0, 0.04);
    color: #86868b;
  }

  :global(body.dark-theme) .cancel-btn {
    background: rgba(255, 255, 255, 0.05);
    color: #a1a1a6;
  }

  .cancel-btn:hover {
    background: rgba(0, 0, 0, 0.08);
  }

  :global(body.dark-theme) .cancel-btn:hover {
    background: rgba(255, 255, 255, 0.08);
  }

  .save-btn {
    background: #af52de;
    color: #ffffff;
  }

  .save-btn:hover:not(:disabled) {
    background: #9b3ec7;
    transform: translateY(-1px);
  }

  .save-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* ANIMATIONS */
  .animate-fade-in {
    animation: fadeIn 180ms cubic-bezier(0.25, 1, 0.5, 1) both;
  }

  .animate-slide-up {
    animation: slideUp 240ms cubic-bezier(0.25, 1, 0.5, 1) both;
  }

  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  @keyframes slideUp {
    from { transform: translateY(16px); opacity: 0; }
    to { transform: translateY(0); opacity: 1; }
  }
</style>
