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

  export let language: "fr" | "en";
  export let writeLocked: boolean;
  
  export let agents: any[] = [];
  export let onStartCreateAgent: () => void;
  export let onStartEditAgent: (agent: any) => void;
  export let onDeleteAgent: (id: string) => void | Promise<void>;
</script>

<div class="settings-tab-panel">
  <div class="panel-header">
    <h2>{language === "fr" ? "Agents Autonomes" : "Durable Agents"}</h2>
    <p>{language === "fr" ? "Configurez des agents autonomes capables de déléguer des tâches et de collaborer en équipe." : "Configure autonomous agents that can delegate tasks and collaborate in teams."}</p>
  </div>

  <div class="agents-header-row">
    <h3>{language === "fr" ? "Liste des Agents" : "Agents List"}</h3>
    <button class="model-add-button animate-hover" type="button" on:click={onStartCreateAgent} disabled={writeLocked}>
      <Plus size={15} />
      <span>{language === "fr" ? "Créer un Agent" : "Create Agent"}</span>
    </button>
  </div>

  {#if agents.length === 0}
    <div class="models-empty">
      <Bot size={24} style="color: #af52de;" />
      <span>{language === "fr" ? "Aucun agent personnalisé configuré." : "No custom agents configured."}</span>
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
              <h4>{agent.name}</h4>
              {#if agent.description}
                <p class="desc">{agent.description}</p>
              {/if}
              <div class="agent-badges">
                <span class="badge model"><Cpu size={12} /> {agent.modelId || "LLM Standard"}</span>
                {#if Array.isArray(agent.enabledTools)}
                  {#each agent.enabledTools as tool}
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

<style>
  .panel-header {
    margin-bottom: 24px;
  }
  .panel-header h2 {
    margin: 0 0 4px 0;
    font-size: 17px;
    font-weight: 700;
  }
  .panel-header p {
    margin: 0;
    font-size: 13px;
    color: #8e8e93;
  }
  .agents-header-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 16px;
  }
  .agents-header-row h3 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
  }
  .model-add-button {
    background: #af52de;
    color: #ffffff;
    border: none;
    padding: 6px 12px;
    border-radius: 8px;
    font-size: 12px;
    font-weight: 500;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
    transition: background 0.15s ease;
  }
  .model-add-button:hover:not(:disabled) {
    background: #9b3ec7;
  }
  .model-add-button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .models-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 40px 20px;
    border: 1px dashed rgba(255, 255, 255, 0.08);
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.01);
    color: #8e8e93;
    font-size: 13px;
  }
  .agents-list {
    display: grid;
    grid-template-columns: 1fr;
    gap: 12px;
    margin-top: 12px;
  }
  .agent-card {
    display: flex;
    justify-content: space-between;
    align-items: center;
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 10px;
    padding: 16px;
    transition: all 0.2s ease;
  }
  .agent-card:hover {
    background: rgba(255, 255, 255, 0.04);
    border-color: rgba(175, 82, 222, 0.2);
  }
  .agent-card-info {
    display: flex;
    gap: 16px;
    align-items: flex-start;
  }
  .agent-avatar {
    width: 36px;
    height: 36px;
    border-radius: 50%;
    background: rgba(175, 82, 222, 0.15);
    color: #af52de;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }
  .agent-details h4 {
    margin: 0 0 4px 0;
    font-size: 14px;
    font-weight: 600;
  }
  .agent-details .desc {
    margin: 0 0 8px 0;
    font-size: 12px;
    color: #8e8e93;
    line-height: 1.4;
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
    padding: 2px 8px;
    border-radius: 4px;
    font-size: 10px;
    font-weight: 500;
  }
  .badge.model {
    background: rgba(255, 255, 255, 0.06);
    color: #e5e5ea;
  }
  .badge.tool {
    background: rgba(175, 82, 222, 0.1);
    color: #af52de;
  }
  .agent-card-actions {
    display: flex;
    gap: 8px;
  }
  .btn-icon {
    background: transparent;
    border: none;
    cursor: pointer;
    color: #8e8e93;
    padding: 6px;
    border-radius: 6px;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.15s ease;
  }
  .btn-icon:hover {
    background: rgba(255, 255, 255, 0.08);
    color: #ffffff;
  }
  .btn-icon.delete:hover {
    background: rgba(255, 69, 58, 0.15);
    color: #ff453a;
  }
</style>
