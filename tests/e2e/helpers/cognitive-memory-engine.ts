import {
  type AgentMemoryContext,
  type AgentMemoryFinding,
  type AgentMessageEnvelope,
  type AgentArtifactRef,
  type AgentParticipant,
  type AgentMessageType,
  type AgentRunPriority,
  createAgentEnvelope,
} from "@aro/contracts";

export interface IMemoryStorageBackend {
  get(key: string): string | null;
  set(key: string, value: string): void;
  remove(key: string): void;
  clear(): void;
  keys(): string[];
}

export class InMemoryStorageBackend implements IMemoryStorageBackend {
  private store = new Map<string, string>();

  get(key: string): string | null {
    return this.store.get(key) ?? null;
  }

  set(key: string, value: string): void {
    this.store.set(key, value);
  }

  remove(key: string): void {
    this.store.delete(key);
  }

  clear(): void {
    this.store.clear();
  }

  keys(): string[] {
    return Array.from(this.store.keys());
  }
}

export class CognitiveMemoryEngine {
  private memoryCache = new Map<string, AgentMemoryContext>();
  private ledgerCache = new Map<string, AgentMessageEnvelope[]>();
  private storage: IMemoryStorageBackend;

  constructor(storageBackend?: IMemoryStorageBackend) {
    this.storage = storageBackend ?? new InMemoryStorageBackend();
  }

  private getMemoryKey(conversationId: string, agentId: string): string {
    return `aro:agent-memory:${conversationId}:${agentId}`;
  }

  private getLedgerKey(conversationId: string): string {
    return `aro:agent-ledger:${conversationId}`;
  }

  public getContext(
    conversationId: string,
    agentId: string,
    agentName = "Sous-Agent",
    role = "worker"
  ): AgentMemoryContext {
    const key = this.getMemoryKey(conversationId, agentId);
    if (this.memoryCache.has(key)) {
      return this.memoryCache.get(key)!;
    }

    const stored = this.storage.get(key);
    if (stored) {
      try {
        const parsed = JSON.parse(stored) as AgentMemoryContext;
        this.memoryCache.set(key, parsed);
        return parsed;
      } catch {
        // Fall back to new context if JSON is corrupt
      }
    }

    const initial: AgentMemoryContext = {
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

    this.memoryCache.set(key, initial);
    return initial;
  }

  public saveContext(context: AgentMemoryContext): void {
    if (!context.agentId || !context.conversationId) {
      throw new Error("Invalid memory context: agentId and conversationId are required");
    }
    const key = this.getMemoryKey(context.conversationId, context.agentId);
    const updated = {
      ...context,
      updatedAt: new Date().toISOString(),
    };
    this.memoryCache.set(key, updated);
    this.storage.set(key, JSON.stringify(updated));
  }

  public updateScratchpad(conversationId: string, agentId: string, scratchpad: string): AgentMemoryContext {
    const ctx = this.getContext(conversationId, agentId);
    ctx.scratchpad = scratchpad;
    this.saveContext(ctx);
    return ctx;
  }

  public recordFinding(
    conversationId: string,
    agentId: string,
    summary: string,
    category: "fact" | "constraint" | "decision" | "discovery" | "insight" = "discovery",
    sourceTool?: string
  ): AgentMemoryFinding {
    if (!summary || summary.trim().length === 0) {
      throw new Error("Finding summary cannot be empty");
    }

    const ctx = this.getContext(conversationId, agentId);
    const finding: AgentMemoryFinding = {
      id: `find-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
      summary,
      category,
      sourceTool,
      timestamp: new Date().toISOString(),
    };

    // Deduplicate if identical ID already exists
    const exists = ctx.findings.some((f) => f.id === finding.id);
    if (!exists) {
      ctx.findings = [...ctx.findings, finding];
      this.saveContext(ctx);
    }
    return finding;
  }

  public recordArtifact(
    conversationId: string,
    agentId: string,
    artifact: AgentArtifactRef
  ): void {
    if (!artifact.id || !artifact.title) {
      throw new Error("Artifact requires id and title");
    }
    const ctx = this.getContext(conversationId, agentId);
    const exists = ctx.artifacts.some((a) => a.id === artifact.id);
    if (!exists) {
      ctx.artifacts = [...ctx.artifacts, artifact];
      this.saveContext(ctx);
    }
  }

  public getLedger(
    conversationId: string,
    filter?: { agentId?: string; messageType?: AgentMessageType }
  ): AgentMessageEnvelope[] {
    const key = this.getLedgerKey(conversationId);
    let list = this.ledgerCache.get(conversationId);
    if (!list) {
      const stored = this.storage.get(key);
      if (stored) {
        try {
          list = JSON.parse(stored) as AgentMessageEnvelope[];
        } catch {
          list = [];
        }
      } else {
        list = [];
      }
      this.ledgerCache.set(conversationId, list);
    }

    if (!filter) return list;

    return list.filter((envelope) => {
      if (filter.agentId) {
        const isSender = envelope.sender.id === filter.agentId;
        const isRecipient =
          envelope.recipient.id === filter.agentId || envelope.recipient.type === "broadcast";
        if (!isSender && !isRecipient) return false;
      }
      if (filter.messageType && envelope.messageType !== filter.messageType) {
        return false;
      }
      return true;
    });
  }

  public dispatchMessage(envelope: AgentMessageEnvelope): AgentMessageEnvelope {
    const conversationId = envelope.conversationId;
    const currentLedger = this.getLedger(conversationId);
    const updatedLedger = [...currentLedger, envelope];
    this.ledgerCache.set(conversationId, updatedLedger);
    this.storage.set(this.getLedgerKey(conversationId), JSON.stringify(updatedLedger));

    // Update sender's memory context ledger if subagent
    if (envelope.sender.type === "subagent") {
      const senderCtx = this.getContext(
        conversationId,
        envelope.sender.id,
        envelope.sender.name,
        envelope.sender.role
      );
      senderCtx.ledger = [...senderCtx.ledger, envelope];
      this.saveContext(senderCtx);
    }

    // Update recipient's memory context ledger if subagent
    if (envelope.recipient.type === "subagent") {
      const recipientCtx = this.getContext(
        conversationId,
        envelope.recipient.id,
        envelope.recipient.name,
        envelope.recipient.role
      );
      recipientCtx.ledger = [...recipientCtx.ledger, envelope];
      this.saveContext(recipientCtx);
    } else if (envelope.recipient.type === "broadcast") {
      for (const [key, ctx] of this.memoryCache.entries()) {
        if (ctx.conversationId === conversationId && ctx.agentId !== envelope.sender.id) {
          ctx.ledger = [...ctx.ledger, envelope];
          this.saveContext(ctx);
        }
      }
    }

    return envelope;
  }

  public clearConversation(conversationId: string): void {
    for (const key of Array.from(this.memoryCache.keys())) {
      if (key.includes(`:${conversationId}:`)) {
        this.memoryCache.delete(key);
        this.storage.remove(key);
      }
    }
    this.ledgerCache.delete(conversationId);
    this.storage.remove(this.getLedgerKey(conversationId));
  }

  // Simulated IPC Handlers
  public ipcGetMemory(agentId: string, conversationId: string): { success: boolean; data?: AgentMemoryContext; error?: string } {
    if (!agentId || !conversationId) {
      return { success: false, error: "Missing required agent_id or conversation_id" };
    }
    return { success: true, data: this.getContext(conversationId, agentId) };
  }

  public ipcSaveMemory(memory: AgentMemoryContext): { success: boolean; error?: string } {
    try {
      this.saveContext(memory);
      return { success: true };
    } catch (e: any) {
      return { success: false, error: e.message };
    }
  }

  public ipcDispatchDirective(
    agentId: string,
    directive: string,
    conversationId: string
  ): { success: boolean; data?: any; error?: string } {
    if (!agentId || !directive || !conversationId) {
      return { success: false, error: "Missing required arguments" };
    }
    const memory = this.getContext(conversationId, agentId);
    // Append directive to scratchpad or ledger
    const envelope = createAgentEnvelope({
      conversationId,
      sender: { id: "orchestrator", name: "Aro Orchestrator", type: "orchestrator" },
      recipient: { id: agentId, name: memory.agentName, role: memory.role, type: "subagent" },
      messageType: "task_delegation",
      content: directive,
    });
    this.dispatchMessage(envelope);

    return {
      success: true,
      data: {
        runId: `run-${Date.now()}`,
        agentId,
        status: "queued",
        directive,
      },
    };
  }

  // Simulated SQLite Table Verification
  public inspectDatabaseTables(): {
    agent_memories: number;
    agent_findings: number;
    agent_message_envelopes: number;
  } {
    let memoriesCount = 0;
    let findingsCount = 0;
    let envelopesCount = 0;

    for (const key of this.storage.keys()) {
      if (key.startsWith("aro:agent-memory:")) {
        memoriesCount++;
        const ctx = JSON.parse(this.storage.get(key) || "{}") as AgentMemoryContext;
        findingsCount += ctx.findings?.length || 0;
      } else if (key.startsWith("aro:agent-ledger:")) {
        const envelopes = JSON.parse(this.storage.get(key) || "[]") as AgentMessageEnvelope[];
        envelopesCount += envelopes.length;
      }
    }

    return {
      agent_memories: memoriesCount,
      agent_findings: findingsCount,
      agent_message_envelopes: envelopesCount,
    };
  }
}
