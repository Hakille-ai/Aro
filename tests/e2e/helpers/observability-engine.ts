import {
  type AgentRun,
  type AgentLaneView,
  type AgentActivityCounters,
  type AgentOrchestratorSnapshot,
  type SubAgentInfo,
  synthesizeAgentLanes,
  calculateActivityCounters,
  extractMessageAgents,
} from "@aro/contracts";

export interface BreadcrumbItem {
  id: string;
  label: string;
  type: "conversation" | "subagent" | "step" | "tool";
}

/**
 * Builds breadcrumbs reflecting conversation, active sub-agent, and execution step/tool.
 */
export function buildBreadcrumbs(params: {
  conversationTitle?: string;
  subAgent?: { id: string; name: string } | null;
  currentStep?: { index: number; toolName?: string | null } | null;
}): BreadcrumbItem[] {
  const crumbs: BreadcrumbItem[] = [
    {
      id: "root-conv",
      label: params.conversationTitle?.trim() || "Conversation Principale",
      type: "conversation",
    },
  ];

  if (params.subAgent && params.subAgent.name?.trim()) {
    crumbs.push({
      id: `sa-${params.subAgent.id}`,
      label: params.subAgent.name.trim(),
      type: "subagent",
    });

    if (params.currentStep) {
      const stepLabel = params.currentStep.toolName
        ? `Étape ${params.currentStep.index} (${params.currentStep.toolName})`
        : `Étape ${params.currentStep.index}`;
      crumbs.push({
        id: `step-${params.currentStep.index}`,
        label: stepLabel,
        type: params.currentStep.toolName ? "tool" : "step",
      });
    }
  }

  return crumbs;
}

/**
 * Computes dynamic placeholder for Composer based on active sub-agent.
 */
export function getComposerPlaceholder(
  activeSubAgent?: { name: string; role?: string } | null,
  language: "fr" | "en" = "fr"
): string {
  const isFr = language === "fr";

  if (!activeSubAgent) {
    return isFr
      ? "Envoyez un message ou déléguez une tâche..."
      : "Send a message or delegate a task...";
  }

  return isFr
    ? `Directive directe pour ${activeSubAgent.name}...`
    : `Direct directive for ${activeSubAgent.name}...`;
}

/**
 * Formats an inspection card for live sub-agent observability.
 */
export function formatInspectionCard(run: AgentRun): {
  badge: { text: string; color: string };
  thoughtSummary: string;
  toolPill: string | null;
  progressPercent: number;
} {
  let badgeColor = "gray";
  let badgeText = run.status.toUpperCase();

  switch (run.status) {
    case "running":
      badgeColor = "blue";
      badgeText = "EN COURS";
      break;
    case "completed":
      badgeColor = "green";
      badgeText = "TERMINÉ";
      break;
    case "failed":
      badgeColor = "red";
      badgeText = "ÉCHEC";
      break;
    case "waiting":
    case "paused":
      badgeColor = "amber";
      badgeText = "EN ATTENTE";
      break;
    case "queued":
      badgeColor = "purple";
      badgeText = "EN FILE";
      break;
  }

  const progressPercent = run.maxSteps && run.maxSteps > 0
    ? Math.min(100, Math.round((run.stepCount / run.maxSteps) * 100))
    : 0;

  return {
    badge: { text: badgeText, color: badgeColor },
    thoughtSummary: run.currentThought || "En attente d'instruction...",
    toolPill: run.currentTool ? `Outil: ${run.currentTool}` : null,
    progressPercent,
  };
}

export {
  synthesizeAgentLanes,
  calculateActivityCounters,
  extractMessageAgents,
};
