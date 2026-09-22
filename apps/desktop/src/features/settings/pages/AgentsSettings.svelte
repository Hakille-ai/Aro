<script lang="ts">
  import Bot from "@lucide/svelte/icons/bot";
  import Brain from "@lucide/svelte/icons/brain";
  import Cpu from "@lucide/svelte/icons/cpu";
  import Plus from "@lucide/svelte/icons/plus";
  import Edit3 from "@lucide/svelte/icons/edit-3";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import Terminal from "@lucide/svelte/icons/terminal";
  import Check from "@lucide/svelte/icons/check";
  import Edit2 from "@lucide/svelte/icons/edit-2";
  import User from "@lucide/svelte/icons/user";
  import Activity from "@lucide/svelte/icons/activity";
  import Sliders from "@lucide/svelte/icons/sliders";
  import Building2 from "@lucide/svelte/icons/building-2";
  import Search from "@lucide/svelte/icons/search";
  import Globe from "@lucide/svelte/icons/globe";
  import Play from "@lucide/svelte/icons/play";
  import Pause from "@lucide/svelte/icons/pause";
  import Square from "@lucide/svelte/icons/square";
  import Sparkles from "@lucide/svelte/icons/sparkles";
  import ListTodo from "@lucide/svelte/icons/list-todo";
  import Puzzle from "@lucide/svelte/icons/puzzle";
  import ShieldCheck from "@lucide/svelte/icons/shield-check";
  import Cloud from "@lucide/svelte/icons/cloud";
  import HardDrive from "@lucide/svelte/icons/hard-drive";

  export let language: "fr" | "en" = "fr";
  export let writeLocked: boolean = false;
  
  export let agents: any[] = [];
  export let onStartCreateAgent: () => void;
  export let onStartEditAgent: (agent: any) => void;
  export let onDeleteAgent: (id: string) => void | Promise<void>;

  // Execution runs state
  let simulatedRuns = [
    {
      id: "run-orch-01",
      agentName: "Architecte & Codeur",
      target: "local",
      model: "Ollama / Llama 3.3 70B",
      status: "running",
      activeTool: "core.workspace.write",
      planStep: "Implémenter le visualiseur de diff",
      duration: "1m 42s",
    },
    {
      id: "run-orch-02",
      agentName: "Navigateur de Recherche",
      target: "cloud",
      model: "Claude 3.5 Sonnet",
      status: "completed",
      activeTool: "core.browser.navigate",
      planStep: "Extraire la documentation API",
      duration: "45s",
    },
  ];

  function toggleRunStatus(runId: string) {
    simulatedRuns = simulatedRuns.map((r) => {
      if (r.id === runId) {
        return {
          ...r,
          status: r.status === "running" ? "paused" : r.status === "paused" ? "running" : r.status,
        };
      }
      return r;
    });
  }

  function stopRun(runId: string) {
    simulatedRuns = simulatedRuns.map((r) => {
      if (r.id === runId) {
        return { ...r, status: "completed" };
      }
      return r;
    });
  }
</script>

<div class="settings-tab-panel agents-settings animate-fade-in">
  <!-- Header -->
  <div class="panel-header">
    <div class="header-badge-row">
      <h2>{language === "fr" ? "Agents & Orchestration Autonome" : "Autonomous Agents & Orchestration"}</h2>
      <span class="orch-badge">
        <Sparkles size={12} />
        Local & Cloud Hybrid
      </span>
    </div>
    <p>
      {language === "fr"
        ? "Créez et configurez des agents autonomes, gérez leurs boucles d'exécution en local ou sur le cloud, et orchestrez leurs outils, compétences et synchronisation de plans de travail."
        : "Create and configure autonomous agents, manage local and cloud execution runs, and orchestrate tools, skills, and work plan synchronization."}
    </p>
  </div>

  <!-- SECTION 1: Liste et Création des Agents -->
  <div class="settings-group">
    <div class="group-header-row">
      <div class="flex-row align-center gap-8">
        <Bot size={16} class="group-icon purple" />
        <h3>{language === "fr" ? "Catalogue des Agents" : "Agents Catalog"}</h3>
      </div>
      <button class="apple-btn primary small" type="button" on:click={onStartCreateAgent} disabled={writeLocked}>
        <Plus size={14} style="margin-right: 4px;" />
        <span>{language === "fr" ? "Créer un Agent" : "Create Agent"}</span>
      </button>
    </div>

    {#if agents.length === 0}
      <div class="models-empty">
        <Bot size={32} style="color: #af52de; margin-bottom: 8px;" />
        <span>{language === "fr" ? "Aucun agent personnalisé configuré." : "No custom agents configured."}</span>
        <button type="button" class="apple-btn secondary small" on:click={onStartCreateAgent} style="margin-top: 10px;">
          <Plus size={13} style="margin-right: 4px;" />
          <span>{language === "fr" ? "Créer votre premier agent" : "Create first agent"}</span>
        </button>
      </div>
    {:else}
      <div class="agents-list">
        {#each agents as agent}
          <div class="agent-card">
            <div class="agent-card-info">
              <div class="agent-avatar">
                {#if agent.icon === "bot"}<Bot size={18} />
                {:else if agent.icon === "code"}<Terminal size={18} />
                {:else if agent.icon === "check"}<Check size={18} />
                {:else if agent.icon === "edit-2"}<Edit2 size={18} />
                {:else if agent.icon === "brain"}<Brain size={18} />
                {:else if agent.icon === "cpu"}<Cpu size={18} />
                {:else if agent.icon === "user"}<User size={18} />
                {:else if agent.icon === "activity"}<Activity size={18} />
                {:else if agent.icon === "sliders"}<Sliders size={18} />
                {:else if agent.icon === "building-2"}<Building2 size={18} />
                {:else}<Search size={18} />{/if}
              </div>
              <div class="agent-details">
                <div class="agent-title-row">
                  <h4>{agent.name}</h4>
                  <span class="target-chip" class:local={agent.executionTarget !== "cloud"}>
                    {#if agent.executionTarget === "cloud"}
                      <Cloud size={10} />
                      Cloud
                    {:else}
                      <HardDrive size={10} />
                      Local-First
                    {/if}
                  </span>
                </div>
                {#if agent.description}
                  <p class="desc">{agent.description}</p>
                {/if}
                <div class="agent-badges">
                  <span class="badge model"><Cpu size={12} /> {agent.modelId || "LLM Standard"}</span>
                  <span class="badge tool"><Globe size={11} /> Navigateur</span>
                  <span class="badge tool"><Terminal size={11} /> Shell / Code</span>
                  {#if Array.isArray(agent.enabledTools)}
                    {#each agent.enabledTools.slice(0, 3) as tool}
                      <span class="badge tool">{tool}</span>
                    {/each}
                  {/if}
                </div>
              </div>
            </div>
            <div class="agent-card-actions">
              <button type="button" class="btn-icon" on:click={() => onStartEditAgent(agent)} disabled={writeLocked}>
                <Edit3 size={15} />
              </button>
              <button type="button" class="btn-icon delete" on:click={() => { if (agent.id || agent.cloudId) onDeleteAgent(agent.id || agent.cloudId); }} disabled={writeLocked}>
                <Trash2 size={15} />
              </button>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>

  <!-- SECTION 2: Gestion des Exécutions d'Agents (Live Execution Runs) -->
  <div class="settings-group">
    <div class="group-header-row">
      <div class="flex-row align-center gap-8">
        <Activity size={16} class="group-icon green" />
        <h3>{language === "fr" ? "Gestion des Exécutions & Processus en Cours" : "Execution Runs Management"}</h3>
      </div>
      <span class="runs-count-pill">{simulatedRuns.filter(r => r.status === 'running').length} en cours</span>
    </div>

    <div class="runs-list">
      {#each simulatedRuns as run}
        <div class="run-row" class:running={run.status === 'running'} class:paused={run.status === 'paused'}>
          <div class="run-left">
            <div class="run-status-indicator">
              {#if run.status === 'running'}
                <span class="status-pulse-dot"></span>
              {:else if run.status === 'paused'}
                <span class="status-paused-dot"></span>
              {:else}
                <Check size={12} class="status-done-icon" />
              {/if}
            </div>
            <div class="run-info">
              <div class="run-title-line">
                <span class="run-agent-name">{run.agentName}</span>
                <span class="run-target-badge" class:cloud={run.target === 'cloud'}>
                  {run.target === 'cloud' ? 'Frontier Cloud' : 'Local Ollama'}
                </span>
                <span class="run-time">{run.duration}</span>
              </div>
              <div class="run-details-line">
                <span class="run-tool">Outil : <code>{run.activeTool}</code></span>
                <span class="run-step">Étape : {run.planStep}</span>
              </div>
            </div>
          </div>

          <div class="run-actions">
            {#if run.status === 'running' || run.status === 'paused'}
              <button
                type="button"
                class="icon-btn-circle"
                title={run.status === 'running' ? "Mettre en pause" : "Reprendre l'exécution"}
                on:click={() => toggleRunStatus(run.id)}
              >
                {#if run.status === 'running'}
                  <Pause size={13} />
                {:else}
                  <Play size={13} />
                {/if}
              </button>
              <button
                type="button"
                class="icon-btn-circle danger"
                title="Arrêter cette exécution"
                on:click={() => stopRun(run.id)}
              >
                <Square size={12} />
              </button>
            {:else}
              <span class="run-done-label">{language === "fr" ? "Terminé" : "Completed"}</span>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  </div>

  <!-- SECTION 3: Orchestration, Planification & Plugins -->
  <div class="settings-group">
    <div class="group-header-row">
      <div class="flex-row align-center gap-8">
        <ListTodo size={16} class="group-icon blue" />
        <h3>{language === "fr" ? "Orchestration & Synchronisation des Plans" : "Orchestration & Plan Sync"}</h3>
      </div>
    </div>

    <div class="orchestration-cards-grid">
      <div class="orch-card">
        <div class="orch-card-header">
          <ListTodo size={18} class="orch-icon blue" />
          <h4>{language === "fr" ? "Plan de Travail Dynamique" : "Dynamic Work Plan"}</h4>
        </div>
        <p>
          {language === "fr"
            ? "Les étapes planifiées par l'utilisateur sont directement synchronisées avec les agents autonomes qui rapportent leur état en temps réel."
            : "User task steps are seamlessly tracked and updated by autonomous subagents."}
        </p>
        <div class="orch-meta">
          <Check size={12} style="color: #30d158;" />
          <span>Synchronisation SQLite WAL active</span>
        </div>
      </div>

      <div class="orch-card">
        <div class="orch-card-header">
          <Puzzle size={18} class="orch-icon purple" />
          <h4>{language === "fr" ? "Compétences & Plugins MCP" : "Skills & MCP Plugins"}</h4>
        </div>
        <p>
          {language === "fr"
            ? "Tous les serveurs MCP installés et les compétences personnalisées sont exposés aux agents selon leur profil d'autorisation."
            : "All installed MCP tools and skills are safely provided to agents based on permissions."}
        </p>
        <div class="orch-meta">
          <ShieldCheck size={12} style="color: #0071e3;" />
          <span>Isolation des permissions active</span>
        </div>
      </div>
    </div>
  </div>
</div>

<style>
  .agents-settings {
    max-width: 820px;
  }

  .panel-header {
    margin-bottom: 24px;
  }

  .header-badge-row {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 6px;
  }

  .panel-header h2 {
    margin: 0;
    font-size: 19px;
    font-weight: 700;
  }

  .orch-badge {
    display: flex;
    align-items: center;
    gap: 4px;
    background: linear-gradient(135deg, rgba(175, 82, 222, 0.2), rgba(0, 113, 227, 0.2));
    border: 1px solid rgba(175, 82, 222, 0.35);
    color: #df9aff;
    font-size: 10px;
    font-weight: 700;
    padding: 3px 8px;
    border-radius: 20px;
    text-transform: uppercase;
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

  .group-header-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 16px;
  }

  .group-header-row h3 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }

  .group-icon.purple { color: #af52de; }
  .group-icon.green { color: #30d158; }
  .group-icon.blue { color: #0071e3; }

  /* Agents List */
  .agents-list {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .agent-card {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 14px;
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 10px;
    transition: all 0.15s;
  }

  .agent-card:hover {
    background: rgba(255, 255, 255, 0.05);
    border-color: rgba(255, 255, 255, 0.12);
  }

  .agent-card-info {
    display: flex;
    align-items: center;
    gap: 12px;
    flex: 1;
  }

  .agent-avatar {
    width: 36px;
    height: 36px;
    border-radius: 50%;
    background: linear-gradient(135deg, #af52de, #5856d6);
    color: #ffffff;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .agent-details {
    display: flex;
    flex-direction: column;
    flex: 1;
  }

  .agent-title-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 2px;
  }

  .agent-title-row h4 {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
  }

  .target-chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 9px;
    padding: 1px 6px;
    border-radius: 4px;
    font-weight: 600;
    background: rgba(0, 113, 227, 0.15);
    color: #2997ff;
  }

  .target-chip.local {
    background: rgba(48, 209, 88, 0.15);
    color: #30d158;
  }

  .desc {
    margin: 0 0 6px 0;
    font-size: 11px;
    color: #8e8e93;
  }

  .agent-badges {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .badge {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 10px;
    padding: 2px 6px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.05);
    color: #a1a1aa;
  }

  .badge.model {
    background: rgba(175, 82, 222, 0.12);
    color: #df9aff;
  }

  .agent-card-actions {
    display: flex;
    gap: 4px;
  }

  .btn-icon {
    background: transparent;
    border: none;
    color: #8e8e93;
    padding: 6px;
    border-radius: 6px;
    cursor: pointer;
    display: flex;
    align-items: center;
    transition: all 0.15s;
  }

  .btn-icon:hover {
    color: #ffffff;
    background: rgba(255, 255, 255, 0.08);
  }

  .btn-icon.delete:hover {
    color: #ff453a;
    background: rgba(255, 69, 58, 0.15);
  }

  /* Runs List */
  .runs-count-pill {
    font-size: 10px;
    font-weight: 700;
    padding: 2px 8px;
    border-radius: 10px;
    background: rgba(48, 209, 88, 0.15);
    color: #30d158;
  }

  .runs-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .run-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px 14px;
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 8px;
  }

  .run-row.running {
    border-color: rgba(48, 209, 88, 0.3);
    background: rgba(48, 209, 88, 0.03);
  }

  .run-left {
    display: flex;
    align-items: center;
    gap: 12px;
    flex: 1;
  }

  .run-status-indicator {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
  }

  .status-pulse-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #30d158;
    animation: runPulse 1.2s infinite;
  }

  @keyframes runPulse {
    0%, 100% { transform: scale(0.8); opacity: 0.6; }
    50% { transform: scale(1.3); opacity: 1; }
  }

  .status-paused-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #ff9f0a;
  }

  .status-done-icon {
    color: #8e8e93;
  }

  .run-info {
    display: flex;
    flex-direction: column;
    flex: 1;
  }

  .run-title-line {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 2px;
  }

  .run-agent-name {
    font-size: 13px;
    font-weight: 600;
  }

  .run-target-badge {
    font-size: 9px;
    padding: 1px 5px;
    border-radius: 3px;
    background: rgba(255, 255, 255, 0.06);
    color: #a1a1aa;
  }

  .run-target-badge.cloud {
    background: rgba(0, 113, 227, 0.15);
    color: #2997ff;
  }

  .run-time {
    font-size: 11px;
    color: #71717a;
  }

  .run-details-line {
    display: flex;
    gap: 12px;
    font-size: 11px;
    color: #8e8e93;
  }

  .run-details-line code {
    color: #e4e4e7;
    background: rgba(255, 255, 255, 0.06);
    padding: 1px 4px;
    border-radius: 3px;
    font-size: 10px;
  }

  .run-actions {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .icon-btn-circle {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.06);
    border: none;
    color: #ffffff;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.15s;
  }

  .icon-btn-circle:hover {
    background: rgba(255, 255, 255, 0.14);
  }

  .icon-btn-circle.danger:hover {
    background: rgba(255, 69, 58, 0.2);
    color: #ff453a;
  }

  .run-done-label {
    font-size: 11px;
    color: #71717a;
  }

  /* Orchestration Grid */
  .orchestration-cards-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
    gap: 12px;
  }

  .orch-card {
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 10px;
    padding: 14px;
    display: flex;
    flex-direction: column;
  }

  .orch-card-header {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 6px;
  }

  .orch-card-header h4 {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
  }

  .orch-icon.blue { color: #0071e3; }
  .orch-icon.purple { color: #af52de; }

  .orch-card p {
    margin: 0 0 12px 0;
    font-size: 11px;
    color: #8e8e93;
    line-height: 1.4;
    flex: 1;
  }

  .orch-meta {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 10px;
    color: #a1a1aa;
  }

  .models-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 32px;
    color: #8e8e93;
    font-size: 12px;
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

  .flex-row { display: flex; }
  .align-center { align-items: center; }
  .gap-8 { gap: 8px; }
</style>
