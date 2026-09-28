import { describe, it, expect, beforeEach } from "vitest";
import {
  getAgentMemoryContext,
  saveAgentMemoryContext,
  updateAgentScratchpad,
  recordAgentFinding,
  recordAgentArtifact,
  dispatchInterAgentMessage,
  getInterAgentMessages,
  validateToolAgainstPermission,
  buildAgentExecutionPromptContext,
  clearAgentMemoryForConversation,
} from "./agent-protocol";
import {
  createAgentEnvelope,
  compilePermissionDirective,
  formatAgentEnvelopeForPrompt,
  formatAgentMemoryForPrompt,
} from "@aro/contracts";

describe("agent-protocol", () => {
  beforeEach(() => {
    clearAgentMemoryForConversation("conv-123");
  });

  describe("Agent Memory Context", () => {
    it("initializes empty memory context and retrieves it", () => {
      const memory = getAgentMemoryContext("conv-123", "agent-code", "Agent Codeur", "code");
      expect(memory.agentId).toBe("agent-code");
      expect(memory.agentName).toBe("Agent Codeur");
      expect(memory.role).toBe("code");
      expect(memory.scratchpad).toBe("");
      expect(memory.findings).toEqual([]);
      expect(memory.artifacts).toEqual([]);
    });

    it("updates scratchpad and persists it", () => {
      updateAgentScratchpad("conv-123", "agent-code", "Refactoring AST parser...");
      const memory = getAgentMemoryContext("conv-123", "agent-code");
      expect(memory.scratchpad).toBe("Refactoring AST parser...");
    });

    it("records verified findings and artifacts", () => {
      recordAgentFinding("conv-123", "agent-code", "Found 3 deprecated API calls in auth.ts", "grep_search", "security");
      recordAgentArtifact("conv-123", "agent-code", {
        id: "art-1",
        title: "Architecture Plan",
        kind: "markdown",
        uri: "file:///plan.md",
      });

      const memory = getAgentMemoryContext("conv-123", "agent-code");
      expect(memory.findings).toHaveLength(1);
      expect(memory.findings[0].summary).toBe("Found 3 deprecated API calls in auth.ts");
      expect(memory.findings[0].sourceTool).toBe("grep_search");
      expect(memory.artifacts).toHaveLength(1);
      expect(memory.artifacts[0].title).toBe("Architecture Plan");
    });
  });

  describe("Inter-Agent Messaging Protocol", () => {
    it("dispatches envelope and records in ledger for both sender and recipient", () => {
      const envelope = createAgentEnvelope({
        conversationId: "conv-123",
        sender: { id: "orchestrator", name: "Aro Orchestrator", type: "orchestrator" },
        recipient: { id: "agent-code", name: "Agent Codeur", role: "code", type: "subagent" },
        messageType: "task_delegation",
        content: "Please implement the new authentication endpoint with JWT validation.",
        suggestedActions: ["Read auth schema", "Generate route handler", "Run tests"],
      });

      dispatchInterAgentMessage(envelope);

      const allMessages = getInterAgentMessages("conv-123");
      expect(allMessages).toHaveLength(1);
      expect(allMessages[0].messageType).toBe("task_delegation");
      expect(allMessages[0].payload.content).toContain("authentication endpoint");

      const recipientMemory = getAgentMemoryContext("conv-123", "agent-code");
      expect(recipientMemory.ledger).toHaveLength(1);
      expect(recipientMemory.ledger[0].id).toBe(envelope.id);
    });

    it("formats message envelope for prompt context in French and English", () => {
      const envelope = createAgentEnvelope({
        conversationId: "conv-123",
        sender: { id: "agent-research", name: "Agent Recherche", role: "researcher", type: "subagent" },
        recipient: { id: "agent-code", name: "Agent Codeur", role: "coder", type: "subagent" },
        messageType: "peer_collaboration",
        content: "API endpoint changed from /v1/auth to /v2/auth.",
        suggestedActions: ["Update client endpoints"],
      });

      const promptFr = formatAgentEnvelopeForPrompt(envelope, "fr");
      expect(promptFr).toContain("[COLLABORATION PAIR-À-PAIR]");
      expect(promptFr).toContain("De: Agent Recherche");
      expect(promptFr).toContain("À: Agent Codeur");
      expect(promptFr).toContain("/v2/auth");

      const promptEn = formatAgentEnvelopeForPrompt(envelope, "en");
      expect(promptEn).toContain("[PEER COLLABORATION]");
    });
  });

  describe("Permission Directive & Tool Guarding", () => {
    it("compiles standard preset directive", () => {
      const dirFr = compilePermissionDirective("standard", null, "fr");
      expect(dirFr).toContain("STANDARD");
      expect(dirFr).toContain("SOUMISES À CONFIRMATION");

      const dirEn = compilePermissionDirective("standard", null, "en");
      expect(dirEn).toContain("STANDARD");
      expect(dirEn).toContain("REQUIRES USER CONFIRMATION");
    });

    it("compiles read-only preset directive with strict prohibitions", () => {
      const dir = compilePermissionDirective("read-only", null, "fr");
      expect(dir).toContain("LECTURE SEULE STRICTE");
      expect(dir).toContain("STRICTEMENT INTERDITE");
    });

    it("compiles custom profile directive with granular settings", () => {
      const customProfile = {
        id: "p-custom",
        name: "Dev SecOps",
        trustedRoots: ["C:/Projects/Aro"],
        allowedDomains: ["api.github.com"],
        allowRead: true,
        allowWrite: true,
        allowShell: false,
        allowNetwork: true,
        commandApproval: "never" as const,
        redactSecrets: true,
      };

      const dir = compilePermissionDirective("custom", customProfile, "fr");
      expect(dir).toContain('PROFIL PERSONNALISÉ "Dev SecOps"');
      expect(dir).toContain("Commandes Shell : INTERDITE");
      expect(dir).toContain("api.github.com");
      expect(dir).toContain("Masquage des secrets : ACTIF");
    });

    it("validates tool execution against sandbox and read-only presets", () => {
      // Sandbox rejects shell, write, network
      expect(validateToolAgainstPermission("shell_execute", null, "sandbox").allowed).toBe(false);
      expect(validateToolAgainstPermission("file_write", null, "sandbox").allowed).toBe(false);
      expect(validateToolAgainstPermission("web_search", null, "sandbox").allowed).toBe(false);

      // Read-only rejects write and shell, allows read
      expect(validateToolAgainstPermission("file_read", null, "read-only").allowed).toBe(true);
      expect(validateToolAgainstPermission("file_write", null, "read-only").allowed).toBe(false);
      expect(validateToolAgainstPermission("shell_execute", null, "read-only").allowed).toBe(false);
    });

    it("validates tool execution against custom profile", () => {
      const profile = {
        id: "profile-1",
        name: "Audit Profile",
        trustedRoots: [],
        allowedDomains: [],
        allowRead: true,
        allowWrite: false,
        allowShell: false,
        allowNetwork: true,
        commandApproval: "always" as const,
        redactSecrets: true,
      };

      expect(validateToolAgainstPermission("file_read", profile, "custom").allowed).toBe(true);
      expect(validateToolAgainstPermission("file_write", profile, "custom").allowed).toBe(false);
      expect(validateToolAgainstPermission("terminal_command", profile, "custom").allowed).toBe(false);
      expect(validateToolAgainstPermission("web_search", profile, "custom").allowed).toBe(true);
    });
  });

  describe("Agent Context Prompt Builder", () => {
    it("builds comprehensive context block with memory, findings, and ledger", () => {
      updateAgentScratchpad("conv-123", "agent-code", "Currently optimizing SQLite indexes.");
      recordAgentFinding("conv-123", "agent-code", "Query latency dropped by 80%", "sqlite_explain");

      dispatchInterAgentMessage(
        createAgentEnvelope({
          conversationId: "conv-123",
          sender: { id: "orchestrator", name: "Aro Orchestrator", type: "orchestrator" },
          recipient: { id: "agent-code", name: "Agent Codeur", type: "subagent" },
          messageType: "task_delegation",
          content: "Proceed with index migrations.",
        })
      );

      const contextPrompt = buildAgentExecutionPromptContext("conv-123", "agent-code", "fr");
      expect(contextPrompt).toContain("Currently optimizing SQLite indexes.");
      expect(contextPrompt).toContain("Query latency dropped by 80%");
      expect(contextPrompt).toContain("Proceed with index migrations.");
    });
  });
});
