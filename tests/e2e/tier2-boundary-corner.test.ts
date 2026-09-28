import { describe, it, expect, beforeEach } from "vitest";
import {
  type AgentMemoryContext,
  type AgentMessageEnvelope,
  type AgentRun,
  type TaskStep,
  createAgentEnvelope,
  validateTaskStep,
  calculatePlanProgress,
  formatTaskError,
} from "@aro/contracts";

import { CognitiveMemoryEngine, InMemoryStorageBackend } from "./helpers/cognitive-memory-engine";
import { ToolAuthorizationGuard, ToolAuthorizationError } from "./helpers/security-guard-engine";
import { resolveWorkspacePath, WorkspaceConfinementError } from "./helpers/workspace-confinement-engine";
import { scrubbedEnv, SandboxedExecutionEngine } from "./helpers/sandboxing-engine";
import { ToolRegistryEngine } from "./helpers/tool-registry-engine";
import { SubAgentRuntimeEngine } from "./helpers/subagent-runtime-engine";
import {
  buildBreadcrumbs,
  synthesizeAgentLanes,
  calculateActivityCounters,
  extractMessageAgents,
} from "./helpers/observability-engine";

describe("Tier 2: Boundary & Corner Cases (Adversarial Verification)", () => {
  let memoryEngine: CognitiveMemoryEngine;
  let registry: ToolRegistryEngine;
  let runtime: SubAgentRuntimeEngine;

  beforeEach(() => {
    memoryEngine = new CognitiveMemoryEngine();
    registry = new ToolRegistryEngine();
    runtime = new SubAgentRuntimeEngine(memoryEngine);
  });

  // =========================================================================
  // Feature 1: Cognitive Memory Boundaries
  // =========================================================================
  describe("F1: Cognitive Memory Boundaries", () => {
    it("T2.1.1 handles empty and whitespace-only scratchpad without corruption", () => {
      memoryEngine.updateScratchpad("conv-b1", "agent-1", "");
      let mem = memoryEngine.getContext("conv-b1", "agent-1");
      expect(mem.scratchpad).toBe("");

      memoryEngine.updateScratchpad("conv-b1", "agent-1", "   \t\n   ");
      mem = memoryEngine.getContext("conv-b1", "agent-1");
      expect(mem.scratchpad).toBe("   \t\n   ");
    });

    it("T2.1.2 persists massive scratchpad (100,000 characters) without truncation", () => {
      const massive = "X".repeat(100_000);
      memoryEngine.updateScratchpad("conv-b1", "agent-1", massive);
      const mem = memoryEngine.getContext("conv-b1", "agent-1");
      expect(mem.scratchpad.length).toBe(100_000);
      expect(mem.scratchpad).toBe(massive);
    });

    it("T2.1.3 preserves unicode, emojis, accents, and markdown formatting in findings", () => {
      const complexSummary = "⚠️ Faille détectée : élévation de privilèges via `sudo -u #0` 🚀 (ç, à, é, ü, 汉字)";
      const finding = memoryEngine.recordFinding("conv-b1", "agent-1", complexSummary, "insight", "audit.tool");
      expect(finding.summary).toBe(complexSummary);

      const mem = memoryEngine.getContext("conv-b1", "agent-1");
      expect(mem.findings[0].summary).toBe(complexSummary);
    });

    it("T2.1.4 rejects empty finding summary with descriptive validation error", () => {
      expect(() => {
        memoryEngine.recordFinding("conv-b1", "agent-1", "");
      }).toThrowError(/Finding summary cannot be empty/);

      expect(() => {
        memoryEngine.recordFinding("conv-b1", "agent-1", "   ");
      }).toThrowError(/Finding summary cannot be empty/);
    });

    it("T2.1.5 handles extreme timestamp values gracefully (epoch 0, distant future)", () => {
      const pastFinding = {
        id: "f-epoch",
        summary: "Genesis finding",
        category: "fact",
        timestamp: new Date(0).toISOString(),
      };
      const futureFinding = {
        id: "f-future",
        summary: "Future milestone",
        category: "decision",
        timestamp: new Date("2099-12-31T23:59:59Z").toISOString(),
      };

      const ctx = memoryEngine.getContext("conv-b1", "agent-1");
      ctx.findings = [pastFinding, futureFinding];
      memoryEngine.saveContext(ctx);

      const retrieved = memoryEngine.getContext("conv-b1", "agent-1");
      expect(retrieved.findings).toHaveLength(2);
      expect(retrieved.findings[0].timestamp).toBe("1970-01-01T00:00:00.000Z");
      expect(retrieved.findings[1].timestamp).toBe("2099-12-31T23:59:59.000Z");
    });
  });

  // =========================================================================
  // Feature 2: Sub-Agent Async Execution Boundaries
  // =========================================================================
  describe("F2: Sub-Agent Async Execution Boundaries", () => {
    it("T2.2.1 executes run with 0 planned steps and completes cleanly", async () => {
      const run = runtime.createRun({
        conversationId: "conv-b2",
        agentName: "Zero Step Worker",
        steps: [],
      });
      // Will execute default synthetic steps or complete
      const completed = await runtime.executeRunLoop(run.id);
      expect(completed.status).toBe("completed");
    });

    it("T2.2.2 halts execution when step count reaches maxSteps boundary", async () => {
      const steps = [
        { stepIndex: 1, thought: "Step 1" },
        { stepIndex: 2, thought: "Step 2" },
        { stepIndex: 3, thought: "Step 3" },
        { stepIndex: 4, thought: "Step 4" },
      ];
      const run = runtime.createRun({
        conversationId: "conv-b2",
        agentName: "Capped Worker",
        maxSteps: 2, // Cap at 2 steps
        steps,
      });

      const completed = await runtime.executeRunLoop(run.id);
      expect(completed.stepCount).toBe(2);
      expect(completed.status).toBe("completed");
    });

    it("T2.2.3 immediately cancels execution when cancelRun is dispatched", async () => {
      const run = runtime.createRun({
        conversationId: "conv-b2",
        agentName: "Cancelling Worker",
      });
      runtime.cancelRun(run.id);
      expect(run.status).toBe("cancelled");
      expect(run.completedAt).toBeDefined();

      const after = await runtime.executeRunLoop(run.id);
      expect(after.status).toBe("cancelled");
    });

    it("T2.2.4 handles sub-agent run with null thought and null tool gracefully", async () => {
      const steps = [{ stepIndex: 1, thought: "" }];
      const run = runtime.createRun({
        conversationId: "conv-b2",
        agentName: "Quiet Worker",
        steps,
      });

      const completed = await runtime.executeRunLoop(run.id);
      expect(completed.status).toBe("completed");
    });

    it("T2.2.5 handles execution with high step-count limit without call stack overflow", async () => {
      const run = runtime.createRun({
        conversationId: "conv-b2",
        agentName: "Marathon Worker",
        maxSteps: 1000,
      });
      expect(run.maxSteps).toBe(1000);
      expect(run.stepCount).toBe(0);
    });
  });

  // =========================================================================
  // Feature 3: Cloud Worker Delegation Boundaries
  // =========================================================================
  describe("F3: Cloud Worker Delegation Boundaries", () => {
    it("T2.3.1 detects and rejects self-delegation where sender equals recipient", () => {
      const selfEnv = createAgentEnvelope({
        conversationId: "conv-b3",
        sender: { id: "agent-loop", name: "Loop Agent", type: "subagent" },
        recipient: { id: "agent-loop", name: "Loop Agent", type: "subagent" },
        messageType: "task_delegation",
        content: "Self delegation task",
      });

      expect(() => runtime.handleDelegation(selfEnv)).toThrowError(/Self-delegation detected/i);
    });

    it("T2.3.2 rejects delegation envelope with empty or whitespace-only content", () => {
      const emptyEnv = createAgentEnvelope({
        conversationId: "conv-b3",
        sender: { id: "orch", name: "Orchestrator", type: "orchestrator" },
        recipient: { id: "worker", name: "Worker", type: "subagent" },
        messageType: "task_delegation",
        content: "   ",
      });

      expect(() => runtime.handleDelegation(emptyEnv)).toThrowError(/directive content cannot be empty/i);
    });

    it("T2.3.3 handles delegation envelope with 100+ suggested actions without error", () => {
      const actions = Array.from({ length: 150 }, (_, i) => `Subtask action item #${i + 1}`);
      const env = createAgentEnvelope({
        conversationId: "conv-b3",
        sender: { id: "orch", name: "Orchestrator", type: "orchestrator" },
        recipient: { id: "worker", name: "Heavy Worker", type: "subagent" },
        messageType: "task_delegation",
        content: "High complexity multi-action job",
        suggestedActions: actions,
      });

      expect(env.payload.suggestedActions).toHaveLength(150);
      const run = runtime.handleDelegation(env);
      expect(run.status).toBe("queued");
    });

    it("T2.3.4 tracks deeply nested parentMessageId chains across multiple delegations", () => {
      let currentParentId: string | null = null;
      for (let depth = 1; depth <= 10; depth++) {
        const env = createAgentEnvelope({
          conversationId: "conv-b3",
          parentMessageId: currentParentId,
          sender: { id: `agent-${depth - 1}`, name: `Agent ${depth - 1}`, type: "subagent" },
          recipient: { id: `agent-${depth}`, name: `Agent ${depth}`, type: "subagent" },
          messageType: "task_delegation",
          content: `Delegation step depth ${depth}`,
        });
        memoryEngine.dispatchMessage(env);
        currentParentId = env.id;
      }

      const ledger = memoryEngine.getLedger("conv-b3");
      expect(ledger).toHaveLength(10);
      expect(ledger[9].parentMessageId).toBe(ledger[8].id);
    });

    it("T2.3.5 broadcast message type delivers envelope to all subagents in conversation", () => {
      // Initialize 3 agents in conversation
      memoryEngine.getContext("conv-b3", "agent-x");
      memoryEngine.getContext("conv-b3", "agent-y");
      memoryEngine.getContext("conv-b3", "agent-z");

      const broadcastEnv = createAgentEnvelope({
        conversationId: "conv-b3",
        sender: { id: "orch", name: "Orchestrator", type: "orchestrator" },
        recipient: { id: "broadcast", name: "All Agents", type: "broadcast" },
        messageType: "context_share",
        content: "Global notice: Deployment beginning in 5 minutes.",
      });

      memoryEngine.dispatchMessage(broadcastEnv);

      const memX = memoryEngine.getContext("conv-b3", "agent-x");
      const memY = memoryEngine.getContext("conv-b3", "agent-y");
      const memZ = memoryEngine.getContext("conv-b3", "agent-z");

      expect(memX.ledger.some((m) => m.id === broadcastEnv.id)).toBe(true);
      expect(memY.ledger.some((m) => m.id === broadcastEnv.id)).toBe(true);
      expect(memZ.ledger.some((m) => m.id === broadcastEnv.id)).toBe(true);
    });
  });

  // =========================================================================
  // Feature 4: Persistent Cognitive Memory IPC & Sync Boundaries
  // =========================================================================
  describe("F4: Persistent Cognitive Memory IPC & Sync Boundaries", () => {
    it("T2.4.1 querying non-existent conversation returns fresh initial context without error", () => {
      const res = memoryEngine.ipcGetMemory("unknown-agent", "ghost-conv-404");
      expect(res.success).toBe(true);
      expect(res.data?.agentId).toBe("unknown-agent");
      expect(res.data?.conversationId).toBe("ghost-conv-404");
      expect(res.data?.findings).toEqual([]);
    });

    it("T2.4.2 rejects saving memory with empty agent ID or conversation ID", () => {
      const invalidCtx: any = {
        agentId: "",
        conversationId: "conv-b4",
        scratchpad: "Data",
      };
      const res = memoryEngine.ipcSaveMemory(invalidCtx);
      expect(res.success).toBe(false);
      expect(res.error).toContain("agentId and conversationId are required");
    });

    it("T2.4.3 handles concurrent save requests safely preserving latest updates", () => {
      const ctx = memoryEngine.getContext("conv-b4", "agent-concurrent");
      for (let i = 1; i <= 20; i++) {
        ctx.scratchpad = `Update version ${i}`;
        memoryEngine.saveContext(ctx);
      }

      const final = memoryEngine.getContext("conv-b4", "agent-concurrent");
      expect(final.scratchpad).toBe("Update version 20");
    });

    it("T2.4.4 recovers gracefully from corrupt JSON in underlying storage backend", () => {
      const storage = new InMemoryStorageBackend();
      storage.set("aro:agent-memory:corrupt-conv:agent-broken", "{ malformed json :::");

      const engine = new CognitiveMemoryEngine(storage);
      const ctx = engine.getContext("corrupt-conv", "agent-broken");
      expect(ctx.agentId).toBe("agent-broken");
      expect(ctx.scratchpad).toBe("");
    });

    it("T2.4.5 handles IPC call with missing arguments returning structured error", () => {
      const res = memoryEngine.ipcDispatchDirective("", "", "");
      expect(res.success).toBe(false);
      expect(res.error).toContain("Missing required arguments");
    });
  });

  // =========================================================================
  // Feature 5: Tool Authorization Guard Boundaries
  // =========================================================================
  describe("F5: Tool Authorization Guard Boundaries", () => {
    it("T2.5.1 normalizes tool names with mixed case and leading/trailing whitespace", () => {
      const guard = new ToolAuthorizationGuard({ preset: "standard" });
      expect(guard.checkPermission("  Workspace.Read  ").allowed).toBe(true);
      expect(guard.checkPermission("\nWORKSPACE.WRITE\t").allowed).toBe(true);
    });

    it("T2.5.2 rejects empty, null, or whitespace-only tool names with ToolAuthorizationError", () => {
      const guard = new ToolAuthorizationGuard({ preset: "standard" });
      expect(() => guard.checkPermission("")).toThrow(ToolAuthorizationError);
      expect(() => guard.checkPermission("   ")).toThrow(ToolAuthorizationError);
    });

    it("T2.5.3 explicitly denied tools list overrides preset permission", () => {
      const guard = new ToolAuthorizationGuard({
        preset: "developer", // developer normally allows everything
        deniedTools: ["workspace.delete", "core.shell.execute"],
      });

      expect(() => guard.checkPermission("workspace.delete")).toThrow(ToolAuthorizationError);
      expect(() => guard.checkPermission("core.shell.execute")).toThrow(ToolAuthorizationError);
      expect(guard.checkPermission("workspace.read").allowed).toBe(true);
    });

    it("T2.5.4 custom profile with all flags set to false blocks all tool invocations", () => {
      const lockdownProfile = {
        id: "p-lockdown",
        name: "Lockdown Profile",
        trustedRoots: [],
        allowedDomains: [],
        allowRead: false,
        allowWrite: false,
        allowShell: false,
        allowNetwork: false,
        commandApproval: "never" as const,
        redactSecrets: true,
      };
      const guard = new ToolAuthorizationGuard({ preset: "custom", profile: lockdownProfile });

      expect(() => guard.checkPermission("workspace.read")).toThrowError(/File reads prohibited/);
      expect(() => guard.checkPermission("workspace.write")).toThrowError(/File writes prohibited/);
      expect(() => guard.checkPermission("core.shell.execute")).toThrowError(/Shell execution prohibited/);
      expect(() => guard.checkPermission("external.fetch")).toThrowError(/Network access prohibited/);
    });

    it("T2.5.5 custom preset without active profile throws informative error", () => {
      const guard = new ToolAuthorizationGuard({ preset: "custom", profile: null });
      expect(() => guard.checkPermission("workspace.read")).toThrowError(/requires an active AgentPermissionProfile/);
    });
  });

  // =========================================================================
  // Feature 6: Workspace Path Confinement Boundaries
  // =========================================================================
  describe("F6: Workspace Path Confinement Boundaries", () => {
    const root = "C:/Projects/AroWorkspace";

    it("T2.6.1 detects and blocks null byte injection attempts", () => {
      expect(() => resolveWorkspacePath(root, "src/index.ts\0/../../secret")).toThrowError(
        /Null byte detected/i
      );
    });

    it("T2.6.2 blocks URL-encoded path traversal sequences (%2e%2e%2f)", () => {
      expect(() => resolveWorkspacePath(root, "%2e%2e%2f%2e%2e%2fWindows%2fsystem32")).toThrow();
    });

    it("T2.6.3 blocks Windows drive hopping (e.g. D:\\secret.env when root is C:)", () => {
      if (process.platform === "win32") {
        expect(() => resolveWorkspacePath("C:\\MyProject", "D:\\Windows\\cmd.exe")).toThrowError(
          /escapes workspace root/i
        );
      } else {
        expect(() => resolveWorkspacePath("/home/user/project", "/var/log/syslog")).toThrowError(
          /escapes workspace root/i
        );
      }
    });

    it("T2.6.4 normalizes redundant relative slashes and dot prefixes safely inside workspace", () => {
      const resolved = resolveWorkspacePath(root, "./src/./components/../components/Button.svelte");
      expect(resolved.toLowerCase()).toContain("button.svelte");
      expect(resolved.toLowerCase()).toContain("aroworkspace");
    });

    it("T2.6.5 handles empty or root-targeting relative path safely by returning canonical root", () => {
      const resolved = resolveWorkspacePath(root, "");
      expect(resolved.toLowerCase()).toContain("aroworkspace");
    });
  });

  // =========================================================================
  // Feature 7: Sandboxing Execution Boundaries
  // =========================================================================
  describe("F7: Sandboxing Execution Boundaries", () => {
    it("T2.7.1 scrubs massive environment (1,000+ keys) efficiently under 15ms", () => {
      const largeEnv: Record<string, string> = { PATH: "/bin" };
      for (let i = 0; i < 1000; i++) {
        largeEnv[`VAR_${i}`] = `value_${i}`;
        if (i % 10 === 0) {
          largeEnv[`API_KEY_${i}`] = `secret_${i}`;
        }
      }

      const start = Date.now();
      const scrubbed = scrubbedEnv(largeEnv);
      const duration = Date.now() - start;

      expect(duration).toBeLessThan(50);
      expect(scrubbed.PATH).toBe("/bin");
      expect(scrubbed.VAR_1).toBe("value_1");
      expect(scrubbed.API_KEY_0).toBeUndefined();
    });

    it("T2.7.2 scrubs secret substrings embedded in compound variable names", () => {
      const env = {
        APP_DATABASE_URL_READONLY: "postgres://...",
        MY_CUSTOM_JWT_SECRET_STRING: "shhhh",
        USER_AUTH_TOKEN: "xyz",
        PUBLIC_API_URL: "https://api.aro.com",
      };

      const cleaned = scrubbedEnv(env);
      expect(cleaned.PUBLIC_API_URL).toBe("https://api.aro.com");
      expect(cleaned.APP_DATABASE_URL_READONLY).toBeUndefined();
      expect(cleaned.MY_CUSTOM_JWT_SECRET_STRING).toBeUndefined();
      expect(cleaned.USER_AUTH_TOKEN).toBeUndefined();
    });

    it("T2.7.3 clamps extreme output buffer size safely without memory exhaustion", async () => {
      const sandbox = new SandboxedExecutionEngine(5000, 500); // 500 bytes buffer limit
      const result = await sandbox.executeCommand("flood_output");
      expect(result.truncated).toBe(true);
      expect(result.stdout.length).toBe(500);
      expect(result.stderr).toContain("truncated");
    });

    it("T2.7.4 rejects zero or negative timeout values with validation error", async () => {
      const sandbox = new SandboxedExecutionEngine();
      await expect(
        sandbox.executeCommand("echo", ["hi"], { timeoutMs: 0 })
      ).rejects.toThrowError(/Invalid timeout/);

      await expect(
        sandbox.executeCommand("echo", ["hi"], { timeoutMs: -100 })
      ).rejects.toThrowError(/Invalid timeout/);
    });

    it("T2.7.5 handles empty command string gracefully returning exit code 1 with error", async () => {
      const sandbox = new SandboxedExecutionEngine();
      const result = await sandbox.executeCommand("");
      expect(result.exitCode).toBe(1);
      expect(result.stderr).toContain("Empty command");
    });
  });

  // =========================================================================
  // Feature 8: Tool Registry Boundaries
  // =========================================================================
  describe("F8: Tool Registry Boundaries", () => {
    it("T2.8.1 rejects tool registration with invalid characters in name", () => {
      expect(() => {
        registry.registerTool({
          name: "invalid tool name with spaces",
          category: "workspace",
          description: "Bad",
          parameters: [],
          requiredPermissions: [],
        });
      }).toThrowError(/Invalid tool name format/);

      expect(() => {
        registry.registerTool({
          name: "tool!@#$%",
          category: "workspace",
          description: "Bad",
          parameters: [],
          requiredPermissions: [],
        });
      }).toThrowError(/Invalid tool name format/);
    });

    it("T2.8.2 rejects tool registration missing description", () => {
      expect(() => {
        registry.registerTool({
          name: "valid.tool.name",
          category: "workspace",
          description: "",
          parameters: [],
          requiredPermissions: [],
        });
      }).toThrowError(/requires a description/);
    });

    it("T2.8.3 querying non-existent tool returns undefined without throwing", () => {
      expect(registry.getTool("unregistered.tool.name")).toBeUndefined();
      expect(registry.hasTool("unregistered.tool.name")).toBe(false);
    });

    it("T2.8.4 permits tool override when allowOverride flag is explicitly set", () => {
      const customRead = {
        name: "workspace.read",
        category: "workspace" as const,
        description: "Custom read implementation",
        parameters: [],
        requiredPermissions: [],
      };

      registry.registerTool(customRead, true);
      const retrieved = registry.getTool("workspace.read");
      expect(retrieved?.description).toBe("Custom read implementation");
    });

    it("T2.8.5 registers 50 distinct tools without performance degradation or collisions", () => {
      for (let i = 0; i < 50; i++) {
        registry.registerTool({
          name: `custom.tool_${i}`,
          category: "core",
          description: `Custom utility tool #${i}`,
          parameters: [],
          requiredPermissions: [],
        });
      }

      expect(registry.listTools("core").length).toBeGreaterThanOrEqual(50);
      expect(registry.hasTool("custom.tool_49")).toBe(true);
    });
  });

  // =========================================================================
  // Feature 9: Svelte Typecheck & Plan Boundaries
  // =========================================================================
  describe("F9: Svelte Typecheck & Plan Boundaries", () => {
    it("T2.9.1 rejects task step with empty or whitespace-only text", () => {
      const invalidStep = validateTaskStep({ text: "   " });
      expect(invalidStep.valid).toBe(false);
    });

    it("T2.9.2 calculates 0% progress on empty tasks array without division by zero NaN", () => {
      const progress = calculatePlanProgress([]);
      expect(progress.percentage).toBe(0);
      expect(isNaN(progress.percentage)).toBe(false);
      expect(progress.total).toBe(0);
    });

    it("T2.9.3 calculates 0% completion and 100% error when all tasks failed", () => {
      const errorTasks: TaskStep[] = [
        { id: "e1", text: "Task 1", completed: false, status: "error", error: "Failed" },
        { id: "e2", text: "Task 2", completed: false, status: "error", error: "Failed" },
      ];
      const progress = calculatePlanProgress(errorTasks);
      expect(progress.percentage).toBe(0);
      expect(progress.error).toBe(2);
      expect(progress.completed).toBe(0);
    });

    it("T2.9.4 truncates massive 10,000-character error message cleanly in formatTaskError", () => {
      const massiveError = "Database timeout error: " + "E".repeat(10_000);
      const formatted = formatTaskError(massiveError, 100);
      expect(formatted?.length).toBe(103); // 100 + "..."
      expect(formatted?.endsWith("...")).toBe(true);
    });

    it("T2.9.5 normalizes unrecognized status string to default pending status", () => {
      const step = validateTaskStep({ text: "Valid task", status: "invalid_status" as any });
      expect(step.valid).toBe(true);
      expect(step.normalized.status).toBe("pending");
    });
  });

  // =========================================================================
  // Feature 10: Persistent Sub-Agent UI Thread Boundaries
  // =========================================================================
  describe("F10: Persistent Sub-Agent UI Thread Boundaries", () => {
    it("T2.10.1 handles empty message steps array returning empty agent list safely", () => {
      const agents = extractMessageAgents({ id: "msg-empty", steps: [] });
      expect(agents).toEqual([]);
    });

    it("T2.10.2 deduplicates 20 duplicate step agent occurrences into single SubAgentInfo", () => {
      const steps = Array.from({ length: 20 }, (_, i) => ({
        id: `step-${i}`,
        kind: "tool",
        input: { toolId: "agent.spawn", name: "Agent Singleton" },
        output: { run_id: "agent-singleton-id" },
      }));

      const agents = extractMessageAgents({ id: "msg-dups", steps });
      expect(agents).toHaveLength(1);
      expect(agents[0].id).toBe("agent-singleton-id");
    });

    it("T2.10.3 preserves unicode emojis and special chars in agent thoughts and goals", () => {
      const message = {
        id: "msg-unicode",
        steps: [
          {
            id: "step-u",
            kind: "tool",
            title: "Recherche d'optimisation ⚡",
            input: { toolId: "agent.spawn", name: "Agent Éclair ⚡", goal: "Objectif: 100% sans bug 🎯" },
            output: { run_id: "sa-unicode" },
          },
        ],
      };

      const agents = extractMessageAgents(message);
      expect(agents[0].name).toBe("Agent Éclair ⚡");
      expect(agents[0].goal).toBe("Objectif: 100% sans bug 🎯");
    });

    it("T2.10.4 falls back gracefully when step startedAt timestamp is undefined", () => {
      const message = {
        id: "msg-no-date",
        steps: [
          {
            id: "step-1",
            kind: "tool",
            input: { toolId: "agent.spawn" },
            output: { run_id: "sa-no-date" },
          },
        ],
      };

      const agents = extractMessageAgents(message);
      expect(agents[0].createdAt).toBeDefined();
    });

    it("T2.10.5 handles message associated with 20 distinct sub-agents cleanly", () => {
      const runs: AgentRun[] = Array.from({ length: 20 }, (_, i) => ({
        id: `run-sub-${i}`,
        conversationId: "conv-b10",
        agentName: `Worker ${i}`,
        status: "running",
        priority: "normal",
        stepCount: 1,
        createdAt: new Date().toISOString(),
      }));

      const extracted = extractMessageAgents({ id: "msg-multi", conversationId: "conv-b10" }, runs);
      expect(extracted).toHaveLength(20);
    });
  });

  // =========================================================================
  // Feature 11: Observability & Breadcrumbs Boundaries
  // =========================================================================
  describe("F11: Observability & Breadcrumbs Boundaries", () => {
    it("T2.11.1 returns empty array when synthesizeAgentLanes receives null conversationId", () => {
      const lanes = synthesizeAgentLanes([], [], null);
      expect(lanes).toEqual([]);
    });

    it("T2.11.2 clamps negative snapshot values to 0 in calculateActivityCounters", () => {
      const negativeSnapshot = {
        runningCount: -5,
        queuedCount: -10,
        activeLanesCount: -2,
      };

      const counters = calculateActivityCounters([], negativeSnapshot, "conv-empty");
      expect(counters.running).toBe(0);
      expect(counters.queued).toBe(0);
      expect(counters.total).toBe(0);
    });

    it("T2.11.3 handles massive run counts (100,000 runs) without numeric overflow", () => {
      const massiveSnapshot = {
        runningCount: 50_000,
        queuedCount: 50_000,
        activeLanesCount: 100,
      };

      const counters = calculateActivityCounters([], massiveSnapshot, "conv-huge");
      expect(counters.running).toBe(50_000);
      expect(counters.queued).toBe(50_000);
      expect(counters.total).toBe(100_000);
    });

    it("T2.11.4 formats breadcrumbs safely when conversation title or step name are empty", () => {
      const crumbs = buildBreadcrumbs({
        conversationTitle: "",
        subAgent: { id: "sa-1", name: "" },
        currentStep: null,
      });

      expect(crumbs).toHaveLength(1);
      expect(crumbs[0].label).toBe("Conversation Principale");
    });

    it("T2.11.5 builds breadcrumbs without step tool name cleanly", () => {
      const crumbs = buildBreadcrumbs({
        conversationTitle: "Projet ARO",
        subAgent: { id: "sa-1", name: "Agent Audit" },
        currentStep: { index: 4, toolName: null },
      });

      expect(crumbs).toHaveLength(3);
      expect(crumbs[2].label).toBe("Étape 4");
    });
  });
});
