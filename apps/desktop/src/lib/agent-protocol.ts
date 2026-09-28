import {
  type AgentMessageEnvelope,
  type AgentMessageType,
  type AgentParticipant,
  type AgentMemoryContext,
  type AgentMemoryFinding,
  type AgentArtifactRef,
  type AgentPermissionProfile,
  type PermissionPresetMode,
  createAgentEnvelope,
  compilePermissionDirective,
  formatAgentEnvelopeForPrompt,
  formatAgentMemoryForPrompt,
} from "@aro/contracts";

const MEMORY_STORAGE_PREFIX = "aro:agent-memory:";
const LEDGER_STORAGE_PREFIX = "aro:agent-ledger:";

// In-memory caches
const memoryCache = new Map<string, AgentMemoryContext>();
const ledgerCache = new Map<string, AgentMessageEnvelope[]>();

const isTauri = () => typeof window !== "undefined" && Boolean((window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);

function getMemoryKey(conversationId: string, agentId: string): string {
  return `${conversationId}:${agentId}`;
}

/**
 * Retrieve or initialize an agent's isolated contextual working memory.
 */
export function getAgentMemoryContext(
  conversationId: string,
  agentId: string,
  agentName = "Sous-Agent",
  role = "worker"
): AgentMemoryContext {
  const key = getMemoryKey(conversationId, agentId);
  if (memoryCache.has(key)) {
    return memoryCache.get(key)!;
  }

  if (typeof localStorage !== "undefined") {
    try {
      const stored = localStorage.getItem(`${MEMORY_STORAGE_PREFIX}${key}`);
      if (stored) {
        const parsed = JSON.parse(stored) as AgentMemoryContext;
        memoryCache.set(key, parsed);
        return parsed;
      }
    } catch (e) {
      console.warn("Failed to load agent memory from localStorage:", e);
    }
  }

  const initialContext: AgentMemoryContext = {
    agentId,
    agentName,
    role,
    conversationId,
    scratchpad: "",
    findings: [],
    ledger: [],
    artifacts: [],
    updatedAt: new Date().toISOString(),
  };

  memoryCache.set(key, initialContext);
  return initialContext;
}

/**
 * Persist an agent's memory context to cache and local storage.
 */
export function saveAgentMemoryContext(context: AgentMemoryContext): void {
  const key = getMemoryKey(context.conversationId, context.agentId);
  context.updatedAt = new Date().toISOString();
  memoryCache.set(key, context);

  if (typeof localStorage !== "undefined") {
    try {
      localStorage.setItem(`${MEMORY_STORAGE_PREFIX}${key}`, JSON.stringify(context));
    } catch (e) {
      console.warn("Failed to persist agent memory:", e);
    }
  }

  if (isTauri()) {
    import("@tauri-apps/api/core").then(({ invoke }) => {
      invoke("agent_save_memory", { memory: context }).catch((err) => {
        console.warn("Failed to persist agent memory to backend:", err);
      });
    }).catch(() => {});
  }
}

/**
 * Asynchronously load an agent's persistent memory from the backend (SQLite via Tauri IPC or web API),
 * falling back to in-memory/localStorage cache. Populates the memoryCache on success.
 */
export async function loadAgentMemoryFromBackend(
  conversationId: string,
  agentId: string,
  agentName = "Sous-Agent",
  role = "worker",
): Promise<AgentMemoryContext> {
  const key = getMemoryKey(conversationId, agentId);
  if (isTauri()) {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const remote = await invoke<AgentMemoryContext>("agent_get_memory", {
        agentId,
        conversationId,
      });
      if (remote) {
        memoryCache.set(key, remote);
        return remote;
      }
    } catch (e) {
      console.warn("Failed to load agent memory via IPC:", e);
    }
  }
  return getAgentMemoryContext(conversationId, agentId, agentName, role);
}

export const loadAgentMemory = loadAgentMemoryFromBackend;

/**
 * Update the working scratchpad for an agent.
 */
export function updateAgentScratchpad(
  conversationId: string,
  agentId: string,
  scratchpad: string
): AgentMemoryContext {
  const context = getAgentMemoryContext(conversationId, agentId);
  context.scratchpad = scratchpad;
  saveAgentMemoryContext(context);
  return context;
}

/**
 * Record a verified finding or key insight in the agent's memory.
 */
export function recordAgentFinding(
  conversationId: string,
  agentId: string,
  summary: string,
  sourceTool?: string,
  category = "insight"
): AgentMemoryFinding {
  const context = getAgentMemoryContext(conversationId, agentId);
  const finding: AgentMemoryFinding = {
    id: typeof crypto !== "undefined" && crypto.randomUUID ? crypto.randomUUID() : `find-${Date.now()}`,
    summary,
    sourceTool,
    category,
    timestamp: new Date().toISOString(),
  };

  context.findings = [...context.findings, finding];
  saveAgentMemoryContext(context);
  return finding;
}

/**
 * Record an artifact created by this agent.
 */
export function recordAgentArtifact(
  conversationId: string,
  agentId: string,
  artifact: AgentArtifactRef
): void {
  const context = getAgentMemoryContext(conversationId, agentId);
  const exists = context.artifacts.some((a) => a.id === artifact.id);
  if (!exists) {
    context.artifacts = [...context.artifacts, artifact];
    saveAgentMemoryContext(context);
  }
}

/**
 * Retrieve the inter-agent message ledger for a conversation.
 */
export function getInterAgentMessages(
  conversationId: string,
  filter?: { agentId?: string; messageType?: AgentMessageType }
): AgentMessageEnvelope[] {
  let list = ledgerCache.get(conversationId);
  if (!list) {
    if (typeof localStorage !== "undefined") {
      try {
        const raw = localStorage.getItem(`${LEDGER_STORAGE_PREFIX}${conversationId}`);
        if (raw) {
          list = JSON.parse(raw) as AgentMessageEnvelope[];
        }
      } catch (e) {
        console.warn("Failed to read inter-agent ledger:", e);
      }
    }
    list = list || [];
    ledgerCache.set(conversationId, list);
  }

  if (!filter) return list;

  return list.filter((envelope) => {
    if (filter.agentId) {
      const matchSender = envelope.sender.id === filter.agentId;
      const matchRecipient =
        envelope.recipient.id === filter.agentId || envelope.recipient.type === "broadcast";
      if (!matchSender && !matchRecipient) return false;
    }
    if (filter.messageType && envelope.messageType !== filter.messageType) {
      return false;
    }
    return true;
  });
}

/**
 * Dispatch and route an inter-agent message envelope across participants.
 */
export function dispatchInterAgentMessage(envelope: AgentMessageEnvelope): AgentMessageEnvelope {
  const conversationId = envelope.conversationId;
  const currentLedger = getInterAgentMessages(conversationId);
  const updatedLedger = [...currentLedger, envelope];
  ledgerCache.set(conversationId, updatedLedger);

  if (typeof localStorage !== "undefined") {
    try {
      localStorage.setItem(`${LEDGER_STORAGE_PREFIX}${conversationId}`, JSON.stringify(updatedLedger));
    } catch (e) {
      console.warn("Failed to persist ledger envelope:", e);
    }
  }

  // Update sender's memory context ledger if it's an agent
  if (envelope.sender.type === "subagent") {
    const senderMemory = getAgentMemoryContext(
      conversationId,
      envelope.sender.id,
      envelope.sender.name,
      envelope.sender.role
    );
    senderMemory.ledger = [...senderMemory.ledger, envelope];
    saveAgentMemoryContext(senderMemory);
  }

  // Update recipient's memory context ledger if it's an agent
  if (envelope.recipient.type === "subagent") {
    const recipientMemory = getAgentMemoryContext(
      conversationId,
      envelope.recipient.id,
      envelope.recipient.name,
      envelope.recipient.role
    );
    recipientMemory.ledger = [...recipientMemory.ledger, envelope];
    saveAgentMemoryContext(recipientMemory);
  } else if (envelope.recipient.type === "broadcast") {
    // Notify all active agent contexts in this conversation
    for (const [key, ctx] of memoryCache.entries()) {
      if (ctx.conversationId === conversationId && ctx.agentId !== envelope.sender.id) {
        ctx.ledger = [...ctx.ledger, envelope];
        saveAgentMemoryContext(ctx);
      }
    }
  }

  // Emit event for UI & sub-chat reactivity
  if (typeof window !== "undefined") {
    window.dispatchEvent(
      new CustomEvent("aro:inter-agent-message", {
        detail: { envelope },
      })
    );
  }

  return envelope;
}

/**
 * Client-side permission guard to validate tool execution against the active permission policy.
 */
export function validateToolAgainstPermission(
  toolId: string,
  profile: AgentPermissionProfile | null,
  preset: PermissionPresetMode
): { allowed: boolean; reason?: string } {
  const lower = toolId.toLowerCase();

  // Read-only inspection tools
  const isRead =
    lower.includes("read") ||
    lower.includes("fetch") ||
    lower.includes("view") ||
    lower.includes("search") ||
    lower.includes("list") ||
    lower.includes("inspect");

  // Mutation / write tools
  const isWrite =
    lower.includes("write") ||
    lower.includes("edit") ||
    lower.includes("replace") ||
    lower.includes("patch") ||
    lower.includes("create") ||
    lower.includes("delete") ||
    lower.includes("remove") ||
    lower.includes("save") ||
    lower.includes("diff");

  // Shell execution tools
  const isShell =
    lower.includes("shell") ||
    lower.includes("bash") ||
    lower.includes("terminal") ||
    lower.includes("cmd") ||
    lower.includes("exec") ||
    lower.includes("run_command") ||
    lower.includes("process");

  // Network tools
  const isNetwork =
    lower.includes("web") ||
    lower.includes("http") ||
    lower.includes("download") ||
    lower.includes("curl") ||
    lower.includes("browser");

  // 1. Preset level checks
  if (preset === "sandbox") {
    if (isShell) return { allowed: false, reason: "Command execution is disabled in Sandbox mode." };
    if (isWrite) return { allowed: false, reason: "File mutation is disabled in Sandbox mode." };
    if (isRead && !lower.includes("context")) return { allowed: false, reason: "Local file access is disabled in Sandbox mode." };
    if (isNetwork) return { allowed: false, reason: "External network access is disabled in Sandbox mode." };
  }

  if (preset === "read-only") {
    if (isShell) return { allowed: false, reason: "Command execution is disabled in Read-only mode." };
    if (isWrite) return { allowed: false, reason: "File writing and mutation are disabled in Read-only mode." };
    if (isNetwork) return { allowed: false, reason: "External network access is disabled in Read-only mode." };
  }

  // 2. Custom profile level checks
  if (profile) {
    if (isShell && !profile.allowShell) {
      return { allowed: false, reason: `Shell commands are prohibited by permission profile "${profile.name}".` };
    }
    if (isWrite && !profile.allowWrite) {
      return { allowed: false, reason: `File writes are prohibited by permission profile "${profile.name}".` };
    }
    if (isRead && !profile.allowRead) {
      return { allowed: false, reason: `File reads are prohibited by permission profile "${profile.name}".` };
    }
    if (isNetwork && !profile.allowNetwork) {
      return { allowed: false, reason: `Network access is disabled by permission profile "${profile.name}".` };
    }
  }

  return { allowed: true };
}

/**
 * Format agent contextual prompt injection (combines memory, scratchpad, findings, and ledger).
 */
export function buildAgentExecutionPromptContext(
  conversationId: string,
  agentId: string,
  language: "fr" | "en" = "fr"
): string {
  const memory = getAgentMemoryContext(conversationId, agentId);
  const ledger = getInterAgentMessages(conversationId, { agentId });

  let promptBlock = formatAgentMemoryForPrompt(memory, language);

  if (ledger.length > 0) {
    const recentLedger = ledger.slice(-5);
    const ledgerHeader =
      language === "fr"
        ? "\n\n[COMMUNICATIONS RÉCENTES DU PROTOCOLE INTER-AGENTS]\n"
        : "\n\n[RECENT INTER-AGENT PROTOCOL COMMUNICATIONS]\n";

    promptBlock += ledgerHeader + recentLedger.map((msg) => formatAgentEnvelopeForPrompt(msg, language)).join("\n---\n");
  }

  return promptBlock;
}

/**
 * Clear cached and stored agent memory for a conversation.
 */
export function clearAgentMemoryForConversation(conversationId: string): void {
  for (const key of memoryCache.keys()) {
    if (key.startsWith(`${conversationId}:`)) {
      memoryCache.delete(key);
      if (typeof localStorage !== "undefined") {
        try {
          localStorage.removeItem(`${MEMORY_STORAGE_PREFIX}${key}`);
        } catch {
          // ignore
        }
      }
    }
  }
  ledgerCache.delete(conversationId);
  if (typeof localStorage !== "undefined") {
    try {
      localStorage.removeItem(`${LEDGER_STORAGE_PREFIX}${conversationId}`);
    } catch {
      // ignore
    }
  }
}

export {
  createAgentEnvelope,
  compilePermissionDirective,
  formatAgentEnvelopeForPrompt,
  formatAgentMemoryForPrompt,
};
