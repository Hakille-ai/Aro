export type AgentRunStatus =
  | "queued"
  | "running"
  | "waiting"
  | "paused"
  | "completed"
  | "failed"
  | "cancelled";

export type AgentRunPriority = "low" | "normal" | "high" | "urgent";

export interface AgentRun {
  id: string;
  conversationId: string;
  laneId?: string | null;
  agentName: string;
  role?: string;
  goal?: string;
  status: AgentRunStatus;
  priority: AgentRunPriority;
  stepCount: number;
  maxSteps?: number;
  currentThought?: string | null;
  currentTool?: string | null;
  error?: string | null;
  createdAt: string;
  startedAt?: string | null;
  completedAt?: string | null;
}

export interface AgentLane {
  id: string;
  conversationId?: string | null;
  title: string;
  status: "active" | "paused" | "completed";
  priority: AgentRunPriority;
  createdAt?: string;
}

export interface AgentLaneView {
  lane: AgentLane;
  collapsed?: boolean;
  visibleRuns?: AgentRun[];
  latestRuns?: AgentRun[];
  queuedCount?: number;
  runningCount?: number;
  waitingCount?: number;
}

export interface AgentOrchestratorSnapshot {
  runningCount: number;
  queuedCount: number;
  activeLanesCount: number;
  totalRunsToday?: number;
}

export interface AgentLiveLog {
  id: string;
  runId?: string;
  timestamp: string;
  level: "info" | "tool" | "success" | "warning" | "error";
  category: string;
  message: string;
  details?: string;
}

export interface AgentActivityCounters {
  running: number;
  queued: number;
  waiting: number;
  done: number;
  failed: number;
  total: number;
}

export interface AgentContextItem {
  id: string;
  title: string;
  content: string;
  relevanceScore?: number;
}

/**
 * Synthesize agent inbox lanes avoiding empty screens by supporting global lanes
 * (conversationId is null/undefined) as well as conversation-scoped lanes.
 */
export function synthesizeAgentLanes(
  lanes: AgentLaneView[],
  runs: AgentRun[],
  activeConversationId?: string | null
): AgentLaneView[] {
  if (!activeConversationId) return [];

  const convRuns = runs.filter((r) => r.conversationId === activeConversationId);
  const matchedLanes: AgentLaneView[] = lanes
    .filter((l) => !l.lane?.conversationId || l.lane?.conversationId === activeConversationId)
    .map((l) => {
      const assignedRuns = convRuns.filter((r) => r.laneId === l.lane?.id);
      const fallbackRuns = (l.visibleRuns || l.latestRuns || []).filter(
        (r) => r.conversationId === activeConversationId
      );
      const runsForLane = assignedRuns.length > 0 ? assignedRuns : fallbackRuns;
      return {
        ...l,
        visibleRuns: runsForLane,
        latestRuns: runsForLane,
      };
    });

  const matchedLaneIds = new Set(matchedLanes.map((l) => l.lane?.id));
  const unassignedRuns = convRuns.filter(
    (r) => !r.laneId || !matchedLaneIds.has(r.laneId)
  );

  if (unassignedRuns.length > 0) {
    matchedLanes.unshift({
      lane: {
        id: "default-virtual",
        title: "Voie principale / Tâches actives",
        status: "active",
        priority: "normal",
        conversationId: activeConversationId,
      },
      collapsed: false,
      visibleRuns: unassignedRuns,
      latestRuns: unassignedRuns,
    });
  }

  return matchedLanes;
}

/**
 * Calculate live activity counters for the active conversation.
 */
export function calculateActivityCounters(
  runs: AgentRun[],
  snapshot?: AgentOrchestratorSnapshot | null,
  activeConversationId?: string | null
): AgentActivityCounters {
  const convRuns = activeConversationId
    ? runs.filter((r) => r.conversationId === activeConversationId)
    : runs;

  if (convRuns.length === 0 && snapshot) {
    const running = Math.max(0, snapshot.runningCount ?? 0);
    const queued = Math.max(0, snapshot.queuedCount ?? 0);
    return {
      running,
      queued,
      waiting: 0,
      done: 0,
      failed: 0,
      total: Math.max(0, running + queued),
    };
  }

  return {
    running: Math.max(0, convRuns.filter((r) => r.status === "running").length),
    queued: Math.max(0, convRuns.filter((r) => r.status === "queued").length),
    waiting: Math.max(0, convRuns.filter((r) => r.status === "waiting" || r.status === "paused").length),
    done: Math.max(0, convRuns.filter((r) => r.status === "completed").length),
    failed: Math.max(0, convRuns.filter((r) => r.status === "failed" || r.status === "cancelled").length),
    total: Math.max(0, convRuns.length),
  };
}

export interface SubAgentInfo {
  id: string;
  name: string;
  role?: string;
  icon?: string;
  avatarColor?: string;
  status: AgentRunStatus | "needs_help" | "error";
  goal?: string;
  currentThought?: string | null;
  currentTool?: string | null;
  stepCount?: number;
  steps?: any[];
  error?: string | null;
  createdAt?: string;
  parentMessageId?: string;
}

/**
 * Extract active or executed sub-agents associated with a message.
 */
export function extractMessageAgents(
  message: {
    id: string;
    conversationId?: string;
    steps?: any[];
    subAgents?: SubAgentInfo[];
    agentRunId?: string | null;
    content?: string;
  },
  allRuns: AgentRun[] = []
): SubAgentInfo[] {
  const result: SubAgentInfo[] = [];
  const seenIds = new Set<string>();

  // 1. Direct subAgents array on message
  if (Array.isArray(message.subAgents)) {
    for (const sa of message.subAgents) {
      if (sa && sa.id && !seenIds.has(sa.id)) {
        seenIds.add(sa.id);
        result.push(sa);
      }
    }
  }

  // 2. Derive from steps if any steps represent agent execution or delegation
  if (Array.isArray(message.steps)) {
    for (const step of message.steps) {
      const toolId = String(step?.input?.toolId || "").toLowerCase();
      const stepTitle = String(step?.title || "").toLowerCase();
      const isAgentStep =
        toolId.includes("agent") ||
        toolId.includes("delegate") ||
        toolId.includes("spawn") ||
        stepTitle.includes("agent") ||
        stepTitle.includes("sous-agent") ||
        stepTitle.includes("délég") ||
        Boolean(step?.output?.run_id);

      if (isAgentStep || step.kind === "tool") {
        const id = String(step?.output?.run_id || `agent-${step.id || step.sequence || Math.random()}`);
        if (!seenIds.has(id)) {
          seenIds.add(id);
          const rawRole = String(step?.output?.role || step?.input?.role || step?.input?.mode || "worker");
          const name = String(
            step?.output?.name ||
              step?.input?.name ||
              (toolId.includes("search") || toolId.includes("fetch") || stepTitle.includes("recherche")
                ? "Agent Recherche"
                : toolId.includes("code") || toolId.includes("shell") || toolId.includes("diff") || stepTitle.includes("code")
                  ? "Agent Codeur"
                  : rawRole === "research"
                    ? "Agent Recherche"
                    : rawRole === "code"
                      ? "Agent Codeur"
                      : rawRole === "architect"
                        ? "Agent Architecte"
                        : "Sous-Agent")
          );
          const icon = String(
            step?.output?.icon ||
              step?.input?.icon ||
              (name.includes("Recherche") ? "search" : name.includes("Code") ? "terminal" : "bot")
          );
          const status: SubAgentInfo["status"] =
            step.status === "failed"
              ? "error"
              : step.status === "running"
                ? "running"
                : (step.output?.status as SubAgentInfo["status"]) || "completed";

          result.push({
            id,
            name,
            role: rawRole,
            icon,
            status,
            goal: String(step?.input?.goal || step?.title || "Exécution autonome"),
            stepCount: 1,
            steps: [step],
            currentThought: step?.title || null,
            currentTool: step?.input?.toolId || null,
            createdAt: step.startedAt || new Date().toISOString(),
            parentMessageId: message.id,
          });
        }
      }
    }
  }

  // 3. Match from allRuns linked to this conversation
  for (const run of allRuns) {
    if (run.conversationId && message.conversationId && run.conversationId === message.conversationId) {
      if (!seenIds.has(run.id)) {
        seenIds.add(run.id);
        result.push({
          id: run.id,
          name: run.agentName || "Agent Autonome",
          role: run.role || "worker",
          icon: run.role === "research" ? "search" : run.role === "code" ? "terminal" : "bot",
          status: run.status === "failed" ? "error" : run.status === "waiting" ? "needs_help" : run.status,
          goal: run.goal || "Mission autonome",
          stepCount: run.stepCount || 0,
          currentThought: run.currentThought || null,
          currentTool: run.currentTool || null,
          createdAt: run.createdAt,
          parentMessageId: message.id,
        });
      }
    }
  }

  return result;
}

// ---------------------------------------------------------------------------
// ARO AGI Inter-Agent Communication Protocol & Memory Persistence
// ---------------------------------------------------------------------------

export type AgentMessageType =
  | "task_delegation"
  | "task_progress"
  | "task_result"
  | "clarification_request"
  | "clarification_response"
  | "peer_collaboration"
  | "context_query"
  | "context_share"
  | "error_escalation";

export type AgentParticipantKind = "orchestrator" | "subagent" | "user" | "tool" | "broadcast";

export interface AgentParticipant {
  id: string;
  name: string;
  role?: string;
  icon?: string;
  type: AgentParticipantKind;
}

export interface AgentArtifactRef {
  id: string;
  title: string;
  kind?: string;
  uri?: string;
}

export interface AgentMessageEnvelope {
  id: string;
  conversationId: string;
  parentMessageId?: string | null;
  correlationId?: string;
  sender: AgentParticipant;
  recipient: AgentParticipant;
  messageType: AgentMessageType;
  payload: {
    content: string;
    structuredData?: Record<string, unknown>;
    artifacts?: AgentArtifactRef[];
    suggestedActions?: string[];
  };
  permissionProfileId?: string | null;
  priority?: AgentRunPriority;
  timestamp: string;
}

export interface AgentMemoryFinding {
  id: string;
  summary: string;
  category?: string;
  sourceTool?: string;
  timestamp: string;
}

export interface AgentMemoryContext {
  agentId: string;
  agentName: string;
  role: string;
  conversationId: string;
  scratchpad: string;
  findings: AgentMemoryFinding[];
  ledger: AgentMessageEnvelope[];
  artifacts: AgentArtifactRef[];
  permissionProfileId?: string | null;
  updatedAt: string;
}

export type PermissionCommandApproval = "always" | "safe-auto" | "never";

export type PermissionPresetMode = "standard" | "read-only" | "developer" | "sandbox" | "custom";

export interface AgentPermissionProfile {
  id: string;
  name: string;
  trustedRoots: string[];
  allowedDomains: string[];
  allowRead: boolean;
  allowWrite: boolean;
  allowShell: boolean;
  allowNetwork: boolean;
  commandApproval: PermissionCommandApproval;
  redactSecrets: boolean;
  createdAt?: string;
  updatedAt?: string;
}

/**
 * Create a validated, standardized AGI Agent Message Envelope.
 */
export function createAgentEnvelope(params: {
  conversationId: string;
  sender: AgentParticipant;
  recipient: AgentParticipant;
  messageType: AgentMessageType;
  content: string;
  parentMessageId?: string | null;
  correlationId?: string;
  structuredData?: Record<string, unknown>;
  artifacts?: AgentArtifactRef[];
  suggestedActions?: string[];
  permissionProfileId?: string | null;
  priority?: AgentRunPriority;
}): AgentMessageEnvelope {
  return {
    id: typeof globalThis.crypto?.randomUUID === "function" ? globalThis.crypto.randomUUID() : `msg-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
    conversationId: params.conversationId,
    parentMessageId: params.parentMessageId ?? null,
    correlationId: params.correlationId,
    sender: params.sender,
    recipient: params.recipient,
    messageType: params.messageType,
    payload: {
      content: params.content,
      structuredData: params.structuredData,
      artifacts: params.artifacts,
      suggestedActions: params.suggestedActions,
    },
    permissionProfileId: params.permissionProfileId ?? null,
    priority: params.priority ?? "normal",
    timestamp: new Date().toISOString(),
  };
}

/**
 * Compile a strict, verifiable permission directive based on active preset or custom profile.
 * Applies uniformly across orchestrator and all sub-agents.
 */
export function compilePermissionDirective(
  preset: PermissionPresetMode,
  profile?: AgentPermissionProfile | null,
  language: "fr" | "en" = "fr"
): string {
  const isFr = language === "fr";

  if (preset === "read-only") {
    return isFr
      ? "\n\n[POLITIQUE DE SÉCURITÉ ET PERMISSIONS : LECTURE SEULE STRICTE]\n" +
          "- Lecture de fichiers : AUTORISÉE (racines sûres uniquement).\n" +
          "- Écriture / Modification de fichiers : STRICTEMENT INTERDITE.\n" +
          "- Exécution de commandes Shell : STRICTEMENT INTERDITE.\n" +
          "- Accès Réseau Extérieur : DÉSACTIVÉ.\n" +
          "- Règle d'or : Répondez uniquement par l'analyse, l'explication et la consultation documentaire. N'effectuez aucune mutation."
      : "\n\n[SECURITY & PERMISSION POLICY: STRICT READ-ONLY]\n" +
          "- File Reading: ALLOWED (safe project roots only).\n" +
          "- File Writing / Mutation: STRICTLY FORBIDDEN.\n" +
          "- Shell Execution: STRICTLY FORBIDDEN.\n" +
          "- External Network Access: DISABLED.\n" +
          "- Golden Rule: Provide analysis, explanation, and doc consultation only. Do not attempt mutations.";
  }

  if (preset === "sandbox") {
    return isFr
      ? "\n\n[POLITIQUE DE SÉCURITÉ ET PERMISSIONS : ISOLÉ (SANDBOX)]\n" +
          "- Lecture de fichiers : STRICTEMENT INTERDITE.\n" +
          "- Écriture de fichiers : STRICTEMENT INTERDITE.\n" +
          "- Commandes Shell : STRICTEMENT INTERDITE.\n" +
          "- Accès Réseau : AUCUN (isolé).\n" +
          "- Règle d'or : Travaillez exclusivement avec le contexte textuel fourni dans la conversation."
      : "\n\n[SECURITY & PERMISSION POLICY: SANDBOX / ISOLATED]\n" +
          "- File Reading: STRICTLY FORBIDDEN.\n" +
          "- File Writing: STRICTLY FORBIDDEN.\n" +
          "- Shell Execution: STRICTLY FORBIDDEN.\n" +
          "- Network Access: NONE (air-gapped).\n" +
          "- Golden Rule: Operate exclusively on the prompt and provided conversational text context.";
  }

  if (preset === "developer") {
    return isFr
      ? "\n\n[POLITIQUE DE SÉCURITÉ ET PERMISSIONS : PLEINEMENT AUTONOME (DÉVELOPPEUR)]\n" +
          "- Lecture de fichiers : PLEIN ACCÈS.\n" +
          "- Écriture de fichiers : PLEIN ACCÈS (création, modification, refactoring).\n" +
          "- Commandes Shell : AUTORISÉES en mode automatique (commandApproval: never).\n" +
          "- Accès Réseau : AUTORISÉ.\n" +
          "- Règle d'or : Agissez comme un ingénieur autonome de classe mondiale. Résolvez la tâche de bout en bout avec précision."
      : "\n\n[SECURITY & PERMISSION POLICY: FULL AUTONOMY (DEVELOPER)]\n" +
          "- File Reading: FULL ACCESS.\n" +
          "- File Writing: FULL ACCESS (create, edit, refactor).\n" +
          "- Shell Commands: ALLOWED in autonomous mode (commandApproval: never).\n" +
          "- Network Access: ALLOWED.\n" +
          "- Golden Rule: Operate as a world-class autonomous software engineer. Deliver end-to-end solutions.";
  }

  if (preset === "custom" && profile) {
    const readStr = profile.allowRead ? (isFr ? "AUTORISÉE" : "ALLOWED") : (isFr ? "INTERDITE" : "FORBIDDEN");
    const writeStr = profile.allowWrite ? (isFr ? "AUTORISÉE" : "ALLOWED") : (isFr ? "INTERDITE" : "FORBIDDEN");
    const shellStr = profile.allowShell
      ? isFr
        ? `AUTORISÉ (approbation: ${profile.commandApproval})`
        : `ALLOWED (approval: ${profile.commandApproval})`
      : (isFr ? "INTERDITE" : "FORBIDDEN");
    const netStr = profile.allowNetwork ? (isFr ? "AUTORISÉ" : "ALLOWED") : (isFr ? "DÉSACTIVÉ" : "DISABLED");

    const domains = profile.allowedDomains?.length
      ? profile.allowedDomains.join(", ")
      : isFr
        ? "aucun domaine restreint"
        : "unrestricted";
    const roots = profile.trustedRoots?.length
      ? profile.trustedRoots.join("; ")
      : isFr
        ? "répertoire du projet"
        : "project workspace";

    return isFr
      ? `\n\n[POLITIQUE DE SÉCURITÉ ET PERMISSIONS : PROFIL PERSONNALISÉ "${profile.name}"]\n` +
          `- Lecture de fichiers : ${readStr} (racines autorisées: ${roots})\n` +
          `- Écriture de fichiers : ${writeStr}\n` +
          `- Commandes Shell : ${shellStr}\n` +
          `- Accès Réseau : ${netStr} (domaines autorisés: ${domains})\n` +
          `- Masquage des secrets : ${profile.redactSecrets ? "ACTIF (masquer les clés d'API et jetons)" : "INACTIF"}\n` +
          `- Règle d'or : Respectez impérativement ces frontières techniques à chaque appel d'outil ou étape.`
      : `\n\n[SECURITY & PERMISSION POLICY: CUSTOM PROFILE "${profile.name}"]\n` +
          `- File Reading: ${readStr} (allowed roots: ${roots})\n` +
          `- File Writing: ${writeStr}\n` +
          `- Shell Commands: ${shellStr}\n` +
          `- Network Access: ${netStr} (allowed domains: ${domains})\n` +
          `- Redact Secrets: ${profile.redactSecrets ? "ACTIVE (redact keys and tokens)" : "INACTIVE"}\n` +
          `- Golden Rule: Strictly observe these boundaries for every tool invocation or reasoning step.`;
  }

  // Default Standard preset
  return isFr
    ? "\n\n[POLITIQUE DE SÉCURITÉ ET PERMISSIONS : STANDARD (ÉQUILIBRÉ)]\n" +
        "- Lecture de fichiers : AUTORISÉE dans le projet.\n" +
        "- Écriture de fichiers : AUTORISÉE pour accomplir la tâche demandée.\n" +
        "- Commandes Shell : SOUMISES À CONFIRMATION systématique de l'utilisateur.\n" +
        "- Accès Réseau : AUTORISÉ pour les requêtes de recherche et documentation.\n" +
        "- Règle d'or : Opérez de manière sûre, productive et transparente."
    : "\n\n[SECURITY & PERMISSION POLICY: STANDARD (BALANCED)]\n" +
        "- File Reading: ALLOWED in project workspace.\n" +
        "- File Writing: ALLOWED to achieve user goals.\n" +
        "- Shell Commands: REQUIRES USER CONFIRMATION before execution.\n" +
        "- Network Access: ALLOWED for search and documentation fetch.\n" +
        "- Golden Rule: Operate safely, productively, and transparently.";
}

/**
 * Format inter-agent communication messages for model context injection.
 */
export function formatAgentEnvelopeForPrompt(envelope: AgentMessageEnvelope, language: "fr" | "en" = "fr"): string {
  const isFr = language === "fr";
  const typeLabel =
    envelope.messageType === "task_delegation"
      ? isFr ? "DÉLÉGATION DE MISSION" : "TASK DELEGATION"
      : envelope.messageType === "task_progress"
        ? isFr ? "AVANCEMENT DE TÂCHE" : "TASK PROGRESS"
        : envelope.messageType === "task_result"
          ? isFr ? "RÉSULTAT DE MISSION" : "TASK RESULT"
          : envelope.messageType === "clarification_request"
            ? isFr ? "DEMANDE DE CLARIFICATION" : "CLARIFICATION REQUEST"
            : envelope.messageType === "clarification_response"
              ? isFr ? "RÉPONSE DE CLARIFICATION" : "CLARIFICATION RESPONSE"
              : envelope.messageType === "peer_collaboration"
                ? isFr ? "COLLABORATION PAIR-À-PAIR" : "PEER COLLABORATION"
                : envelope.messageType.toUpperCase();

  let formatted = `[${typeLabel}] De: ${envelope.sender.name} (${envelope.sender.role || envelope.sender.type}) -> À: ${envelope.recipient.name}\n${envelope.payload.content}`;
  if (envelope.payload.suggestedActions?.length) {
    formatted += `\nActions suggérées: ${envelope.payload.suggestedActions.join(" | ")}`;
  }
  return formatted;
}

/**
 * Format isolated agent memory context for inclusion in prompt.
 */
export function formatAgentMemoryForPrompt(memory: AgentMemoryContext, language: "fr" | "en" = "fr"): string {
  const isFr = language === "fr";
  const header = isFr
    ? `=== MÉMOIRE CONTEXTUELLE DE L'AGENT [${memory.agentName} (${memory.role})] ===`
    : `=== AGENT CONTEXTUAL MEMORY [${memory.agentName} (${memory.role})] ===`;

  const sections: string[] = [header];

  if (memory.scratchpad.trim()) {
    sections.push(isFr ? `[BLOC-NOTES DE TRAVAIL]\n${memory.scratchpad}` : `[WORKING SCRATCHPAD]\n${memory.scratchpad}`);
  }

  if (memory.findings.length > 0) {
    const findingsList = memory.findings.map((f, i) => `${i + 1}. ${f.summary}${f.sourceTool ? ` (${f.sourceTool})` : ""}`).join("\n");
    sections.push(isFr ? `[CONNAISSANCES ET FAITS DÉCOUVERTS]\n${findingsList}` : `[DISCOVERED FINDINGS & FACTS]\n${findingsList}`);
  }

  if (memory.artifacts.length > 0) {
    const artifactsList = memory.artifacts.map((a) => `- ${a.title} (${a.kind || "document"}${a.uri ? `: ${a.uri}` : ""})`).join("\n");
    sections.push(isFr ? `[ARTEFACTS PRODUITS]\n${artifactsList}` : `[PRODUCED ARTIFACTS]\n${artifactsList}`);
  }

  return sections.join("\n\n");
}

