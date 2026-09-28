<script lang="ts">
  import Bot from "@lucide/svelte/icons/bot";
  import Search from "@lucide/svelte/icons/search";
  import Terminal from "@lucide/svelte/icons/terminal";
  import Brain from "@lucide/svelte/icons/brain";
  import Cpu from "@lucide/svelte/icons/cpu";
  import Check from "@lucide/svelte/icons/check";
  import AlertCircle from "@lucide/svelte/icons/alert-circle";
  import HelpCircle from "@lucide/svelte/icons/help-circle";
  import type { SubAgentInfo } from "../../lib/types";

  export let agents: SubAgentInfo[] = [];
  export let language: "fr" | "en" = "fr";
  export let onSelectAgent: (agent: SubAgentInfo) => void = () => {};

  function getStatusLabel(status: SubAgentInfo["status"]): string {
    switch (status) {
      case "running":
        return language === "fr" ? "En cours d'exécution" : "Running";
      case "waiting":
      case "needs_help":
        return language === "fr" ? "Besoin d'aide" : "Needs help";
      case "failed":
      case "error":
        return language === "fr" ? "Erreur rencontrée" : "Error occurred";
      case "completed":
      default:
        return language === "fr" ? "Tâche terminée" : "Completed";
    }
  }
</script>

{#if agents && agents.length > 0}
  <div class="agent-pills-row" role="group" aria-label={language === "fr" ? "Agents assignés" : "Assigned agents"}>
    {#each agents as agent (agent.id)}
      <button
        type="button"
        class="agent-pill status-{agent.status}"
        title="{agent.name} • {getStatusLabel(agent.status)}"
        aria-label="{agent.name} ({getStatusLabel(agent.status)})"
        on:click|stopPropagation={() => onSelectAgent(agent)}
        on:keydown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            e.stopPropagation();
            onSelectAgent(agent);
          }
        }}
      >
        <span class="pill-icon">
          {#if agent.icon === "search"}
            <Search size={11} strokeWidth={2.2} />
          {:else if agent.icon === "terminal" || agent.icon === "code"}
            <Terminal size={11} strokeWidth={2.2} />
          {:else if agent.icon === "brain" || agent.icon === "think"}
            <Brain size={11} strokeWidth={2.2} />
          {:else if agent.icon === "cpu"}
            <Cpu size={11} strokeWidth={2.2} />
          {:else if agent.icon && !["search", "terminal", "code", "brain", "think", "cpu", "bot"].includes(agent.icon) && agent.icon.length <= 4}
            <span class="pill-emoji">{agent.icon}</span>
          {:else}
            <Bot size={11} strokeWidth={2.2} />
          {/if}
        </span>

        <span class="pill-title">{agent.name}</span>

        <span class="pill-status-indicator {agent.status}" aria-hidden="true">
          {#if agent.status === "running"}
            <span class="pulse-ring running"></span>
          {:else if agent.status === "waiting" || agent.status === "needs_help"}
            <span class="pulse-ring amber"></span>
            <HelpCircle size={8} class="indicator-icon amber" />
          {:else if agent.status === "failed" || agent.status === "error"}
            <AlertCircle size={8} class="indicator-icon error" />
          {:else}
            <Check size={8} strokeWidth={2.8} class="indicator-icon success" />
          {/if}
        </span>
      </button>
    {/each}
  </div>
{/if}

<style>
  .agent-pills-row {
    display: inline-flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin-top: 8px;
    margin-bottom: 3px;
  }

  .agent-pill {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 25px;
    padding: 0 10px 0 8px;
    border-radius: 9999px;
    font-size: 11px;
    font-weight: 500;
    line-height: 1;
    font-family: -apple-system, BlinkMacSystemFont, SF Pro Text, Segoe UI, Roboto, sans-serif;
    letter-spacing: -0.01em;
    cursor: pointer;
    border: 1px solid rgba(0, 0, 0, 0.08);
    background: rgba(0, 0, 0, 0.03);
    color: #1d1d1f;
    transition: all 0.16s cubic-bezier(0.16, 1, 0.3, 1);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.02);
    user-select: none;
    outline: none;
  }

  :global(.dark) .agent-pill {
    border-color: rgba(255, 255, 255, 0.09);
    background: rgba(255, 255, 255, 0.04);
    color: #f5f5f7;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.2);
  }

  .agent-pill:hover {
    background: rgba(0, 113, 227, 0.08);
    border-color: rgba(0, 113, 227, 0.35);
    color: #0071e3;
    transform: translateY(-0.5px);
    box-shadow: 0 2px 8px rgba(0, 113, 227, 0.12);
  }

  :global(.dark) .agent-pill:hover {
    background: rgba(41, 151, 255, 0.12);
    border-color: rgba(41, 151, 255, 0.45);
    color: #2997ff;
    box-shadow: 0 2px 8px rgba(41, 151, 255, 0.18);
  }

  .agent-pill:active {
    transform: translateY(0);
  }

  .pill-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: #86868b;
    transition: color 0.16s ease;
  }

  .pill-emoji {
    font-size: 11px;
    line-height: 1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .agent-pill:hover .pill-icon {
    color: inherit;
  }

  .pill-title {
    max-width: 140px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .pill-status-indicator {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    position: relative;
    width: 10px;
    height: 10px;
    margin-left: 1px;
  }

  .pulse-ring {
    position: absolute;
    width: 6px;
    height: 6px;
    border-radius: 50%;
  }

  .pulse-ring.running {
    background: #0071e3;
    box-shadow: 0 0 0 0 rgba(0, 113, 227, 0.7);
    animation: pill-pulse 1.8s infinite;
  }

  :global(.dark) .pulse-ring.running {
    background: #2997ff;
    box-shadow: 0 0 0 0 rgba(41, 151, 255, 0.7);
  }

  .pulse-ring.amber {
    background: #ff9f0a;
    box-shadow: 0 0 0 0 rgba(255, 159, 10, 0.7);
    animation: pill-pulse 1.8s infinite;
  }

  :global(.indicator-icon.success) {
    color: #34c759;
  }

  :global(.indicator-icon.error) {
    color: #ff453a;
  }

  :global(.indicator-icon.amber) {
    color: #ff9f0a;
  }

  @keyframes pill-pulse {
    0% {
      transform: scale(0.95);
      box-shadow: 0 0 0 0 rgba(0, 113, 227, 0.6);
    }
    70% {
      transform: scale(1.1);
      box-shadow: 0 0 0 4px rgba(0, 113, 227, 0);
    }
    100% {
      transform: scale(0.95);
      box-shadow: 0 0 0 0 rgba(0, 113, 227, 0);
    }
  }
</style>
