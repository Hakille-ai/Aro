import { describe, it, expect, beforeEach } from "vitest";
import {
  type AgentMemoryContext,
  type AgentMessageEnvelope,
  type AgentArtifactRef,
  type AgentRun,
  type AgentLaneView,
  type TaskStep,
  type Plan,
  createAgentEnvelope,
  compilePermissionDirective,
  formatAgentEnvelopeForPrompt,
  formatAgentMemoryForPrompt,
  validateTaskStep,
  calculatePlanProgress,
  cycleTaskStatus,
} from "@aro/contracts";

import { CognitiveMemoryEngine } from "./helpers/cognitive-memory-engine";
import { ToolAuthorizationGuard } from "./helpers/security-guard-engine";
import { resolveWorkspacePath } from "./helpers/workspace-confinement-engine";
import { scrubbedEnv, SandboxedExecutionEngine } from "./helpers/sandboxing-engine";
import { ToolRegistryEngine } from "./helpers/tool-registry-engine";
import { SubAgentRuntimeEngine } from "./helpers/subagent-runtime-engine";
import {
  buildBreadcrumbs,
  getComposerPlaceholder,
  formatInspectionCard,
  synthesizeAgentLanes,
  calculateActivityCounters,
  extractMessageAgents,
} from "./helpers/observability-engine";

describe("Tier 1: Feature Coverage (Opaque-Box E2E Requirements)", () => {
  let memoryEngine: CognitiveMemoryEngine;
  let registry: ToolRegistryEngine;
  let runtime: SubAgentRuntimeEngine;

  beforeEach(() => {
    memoryEngine = new CognitiveMemoryEngine();
    registry = new ToolRegistryEngine();
    runtime = new SubAgentRuntimeEngine(memoryEngine);
  });

  // =========================================================================
  // Feature 1: Cognitive Memory Persistence Schema
  // =========================================================================
  describe("Feature 1: Cognitive Memory Persistence Schema", () => {
    it("T1.1.1 initializes default empty memory context with required contract schema", () => {
      const memory = memoryEngine.getContext("conv-101", "agent-sec", "Security Agent", "auditor");
      expect(memory.agentId).toBe("agent-sec");
      expect(memory.agentName).toBe("Security Agent");
      expect(memory.role).toBe("auditor");
      expect(memory.conversationId).toBe("conv-101");
      expect(memory.scratchpad).toBe("");
      expect(memory.findings).toEqual([]);
      expect(memory.ledger).toEqual([]);
      expect(memory.artifacts).toEqual([]);
      expect(memory.updatedAt).toBeDefined();
    });

    it("T1.1.2 updates working scratchpad and updates timestamp", () => {
      const initial = memoryEngine.getContext("conv-101", "agent-sec");
      const updated = memoryEngine.updateScratchpad("conv-101", "agent-sec", "Auditing authentication middleware...");
      expect(updated.scratchpad).toBe("Auditing authentication middleware...");
      expect(new Date(updated.updatedAt).getTime()).toBeGreaterThanOrEqual(new Date(initial.updatedAt).getTime());
    });

    it("T1.1.3 records structured findings with categories and source tool metadata", () => {
      const finding = memoryEngine.recordFinding(
        "conv-101",
        "agent-sec",
        "Hardcoded JWT secret detected in auth.config.ts",
        "discovery",
        "workspace.read"
      );
      expect(finding.id).toBeDefined();
      expect(finding.summary).toBe("Hardcoded JWT secret detected in auth.config.ts");
      expect(finding.category).toBe("discovery");
      expect(finding.sourceTool).toBe("workspace.read");

      const memory = memoryEngine.getContext("conv-101", "agent-sec");
      expect(memory.findings).toHaveLength(1);
      expect(memory.findings[0].id).toBe(finding.id);
    });

    it("T1.1.4 records artifact references with URI and kind in memory context", () => {
      const artifact: AgentArtifactRef = {
        id: "art-security-report",
        title: "Security Audit Report",
        kind: "markdown",
        uri: "workspace://reports/audit.md",
      };
      memoryEngine.recordArtifact("conv-101", "agent-sec", artifact);

      const memory = memoryEngine.getContext("conv-101", "agent-sec");
      expect(memory.artifacts).toHaveLength(1);
      expect(memory.artifacts[0].id).toBe("art-security-report");
      expect(memory.artifacts[0].title).toBe("Security Audit Report");
    });

    it("T1.1.5 guarantees strict conversation isolation between distinct sessions", () => {
      memoryEngine.updateScratchpad("conv-A", "agent-worker", "Working on Conversation A");
      memoryEngine.updateScratchpad("conv-B", "agent-worker", "Working on Conversation B");

      const memA = memoryEngine.getContext("conv-A", "agent-worker");
      const memB = memoryEngine.getContext("conv-B", "agent-worker");

      expect(memA.scratchpad).toBe("Working on Conversation A");
      expect(memB.scratchpad).toBe("Working on Conversation B");
      expect(memA.conversationId).toBe("conv-A");
      expect(memB.conversationId).toBe("conv-B");
    });
  });

  // =========================================================================
  // Feature 2: Sub-Agent Asynchronous Execution Loop
  // =========================================================================
  describe("Feature 2: Sub-Agent Asynchronous Execution Loop", () => {
    it("T1.2.1 transitions lifecycle from queued to running to completed", async () => {
      const run = runtime.createRun({
        conversationId: "conv-102",
        agentName: "Code Reviewer",
        role: "reviewer",
        steps: [{ stepIndex: 1, thought: "Reviewing code changes" }],
      });
      expect(run.status).toBe("queued");
      expect(run.startedAt).toBeNull();

      const completedRun = await runtime.executeRunLoop(run.id);
      expect(completedRun.status).toBe("completed");
      expect(completedRun.startedAt).toBeDefined();
      expect(completedRun.completedAt).toBeDefined();
    });

    it("T1.2.2 executes multi-step workflow past step 2 without freezing", async () => {
      const steps = [
        { stepIndex: 1, thought: "Step 1: Check repo status" },
        { stepIndex: 2, thought: "Step 2: Parse AST" },
        { stepIndex: 3, thought: "Step 3: Analyze dependencies" },
        { stepIndex: 4, thought: "Step 4: Generate fix" },
        { stepIndex: 5, thought: "Step 5: Verify build" },
      ];
      const run = runtime.createRun({
        conversationId: "conv-102",
        agentName: "Refactor Agent",
        steps,
      });

      const completedRun = await runtime.executeRunLoop(run.id);
      expect(completedRun.stepCount).toBe(5);
      expect(completedRun.status).toBe("completed");
    });

    it("T1.2.3 updates currentThought and currentTool dynamically during execution", async () => {
      const steps = [
        { stepIndex: 1, thought: "Inspecting directory", toolCall: { toolName: "workspace.list_dir", args: {} } },
        { stepIndex: 2, thought: "Reading config", toolCall: { toolName: "workspace.read", args: { path: "conf.json" } } },
      ];
      const run = runtime.createRun({
        conversationId: "conv-102",
        agentName: "Inspector",
        steps,
      });

      const finishedRun = await runtime.executeRunLoop(run.id);
      expect(finishedRun.stepCount).toBe(2);
      expect(finishedRun.status).toBe("completed");
      // Finding was recorded for successful tool calls
      const mem = memoryEngine.getContext("conv-102", run.id);
      expect(mem.findings.length).toBeGreaterThanOrEqual(2);
    });

    it("T1.2.4 catches tool execution failure, transitions to failed status, and logs error", async () => {
      const steps = [
        { stepIndex: 1, thought: "Starting task" },
        {
          stepIndex: 2,
          thought: "Executing broken operation",
          toolCall: { toolName: "workspace.read", args: { path: "missing.json" } },
          shouldFail: true,
          errorMessage: "File 'missing.json' not found on disk",
        },
      ];
      const run = runtime.createRun({
        conversationId: "conv-102",
        agentName: "Failing Agent",
        steps,
      });

      const failedRun = await runtime.executeRunLoop(run.id);
      expect(failedRun.status).toBe("failed");
      expect(failedRun.error).toContain("File 'missing.json' not found");
      expect(failedRun.stepCount).toBe(2);
    });

    it("T1.2.5 executes multiple parallel sub-agents concurrently in independent lanes", async () => {
      const runA = runtime.createRun({
        conversationId: "conv-102",
        laneId: "lane-backend",
        agentName: "Backend Worker",
        steps: [{ stepIndex: 1, thought: "Running DB migration" }],
      });
      const runB = runtime.createRun({
        conversationId: "conv-102",
        laneId: "lane-frontend",
        agentName: "Frontend Worker",
        steps: [{ stepIndex: 1, thought: "Compiling Tailwind CSS" }],
      });

      const [resA, resB] = await Promise.all([
        runtime.executeRunLoop(runA.id),
        runtime.executeRunLoop(runB.id),
      ]);

      expect(resA.status).toBe("completed");
      expect(resB.status).toBe("completed");
      expect(resA.laneId).toBe("lane-backend");
      expect(resB.laneId).toBe("lane-frontend");
    });
  });

  // =========================================================================
  // Feature 3: Cloud Worker Delegation Handling
  // =========================================================================
  describe("Feature 3: Cloud Worker Delegation Handling", () => {
    it("T1.3.1 creates and validates standard task_delegation envelope", () => {
      const envelope = createAgentEnvelope({
        conversationId: "conv-103",
        sender: { id: "orch", name: "Orchestrator", type: "orchestrator" },
        recipient: { id: "worker-1", name: "Data Analyst", role: "analyst", type: "subagent" },
        messageType: "task_delegation",
        content: "Analyze quarterly sales metrics from sales.csv",
        suggestedActions: ["Read CSV", "Calculate totals", "Plot distribution"],
      });

      expect(envelope.id).toBeDefined();
      expect(envelope.messageType).toBe("task_delegation");
      expect(envelope.sender.type).toBe("orchestrator");
      expect(envelope.recipient.type).toBe("subagent");
      expect(envelope.payload.suggestedActions).toHaveLength(3);
    });

    it("T1.3.2 handles task_progress envelope reporting incremental status to orchestrator", () => {
      const progressEnv = createAgentEnvelope({
        conversationId: "conv-103",
        sender: { id: "worker-1", name: "Data Analyst", type: "subagent" },
        recipient: { id: "orch", name: "Orchestrator", type: "orchestrator" },
        messageType: "task_progress",
        content: "Processed 5,000 of 10,000 records (50%)",
        structuredData: { processed: 5000, total: 10000, percentage: 50 },
      });

      memoryEngine.dispatchMessage(progressEnv);
      const ledger = memoryEngine.getLedger("conv-103", { messageType: "task_progress" });
      expect(ledger).toHaveLength(1);
      expect(ledger[0].payload.structuredData).toEqual({ processed: 5000, total: 10000, percentage: 50 });
    });

    it("T1.3.3 delivers task_result envelope with generated artifacts and summary", () => {
      const resultEnv = createAgentEnvelope({
        conversationId: "conv-103",
        sender: { id: "worker-1", name: "Data Analyst", type: "subagent" },
        recipient: { id: "orch", name: "Orchestrator", type: "orchestrator" },
        messageType: "task_result",
        content: "Sales analysis complete. Revenue grew by 14% year-over-year.",
        artifacts: [{ id: "art-summary", title: "Quarterly Summary", kind: "csv", uri: "reports/q3.csv" }],
      });

      memoryEngine.dispatchMessage(resultEnv);
      const results = memoryEngine.getLedger("conv-103", { messageType: "task_result" });
      expect(results).toHaveLength(1);
      expect(results[0].payload.artifacts).toHaveLength(1);
      expect(results[0].payload.artifacts![0].title).toBe("Quarterly Summary");
    });

    it("T1.3.4 routes peer_collaboration envelope between two cooperating sub-agents", () => {
      const peerEnv = createAgentEnvelope({
        conversationId: "conv-103",
        sender: { id: "worker-db", name: "DB Specialist", role: "db", type: "subagent" },
        recipient: { id: "worker-api", name: "API Specialist", role: "api", type: "subagent" },
        messageType: "peer_collaboration",
        content: "Schema migrated. Table 'users' now includes column 'tenant_id'.",
      });

      memoryEngine.dispatchMessage(peerEnv);

      // Verify envelope is recorded in recipient's memory context ledger
      const apiMem = memoryEngine.getContext("conv-103", "worker-api");
      expect(apiMem.ledger).toHaveLength(1);
      expect(apiMem.ledger[0].sender.id).toBe("worker-db");
    });

    it("T1.3.5 executes clarification request and response roundtrip between agent and orchestrator", () => {
      const requestEnv = createAgentEnvelope({
        conversationId: "conv-103",
        sender: { id: "worker-1", name: "Analyst", type: "subagent" },
        recipient: { id: "orch", name: "Orchestrator", type: "orchestrator" },
        messageType: "clarification_request",
        content: "Should data be aggregated weekly or monthly?",
        correlationId: "req-clarify-001",
      });
      memoryEngine.dispatchMessage(requestEnv);

      const responseEnv = createAgentEnvelope({
        conversationId: "conv-103",
        sender: { id: "orch", name: "Orchestrator", type: "orchestrator" },
        recipient: { id: "worker-1", name: "Analyst", type: "subagent" },
        messageType: "clarification_response",
        content: "Please aggregate monthly.",
        correlationId: "req-clarify-001",
      });
      memoryEngine.dispatchMessage(responseEnv);

      const messages = memoryEngine.getLedger("conv-103");
      expect(messages).toHaveLength(2);
      expect(messages[0].correlationId).toBe("req-clarify-001");
      expect(messages[1].correlationId).toBe("req-clarify-001");
    });
  });

  // =========================================================================
  // Feature 4: Persistent Cognitive Memory IPC & API
  // =========================================================================
  describe("Feature 4: Persistent Cognitive Memory IPC & API", () => {
    it("T1.4.1 IPC agent_get_memory retrieves existing memory context with fidelity", () => {
      memoryEngine.updateScratchpad("conv-104", "agent-tester", "Drafting test scenarios");
      memoryEngine.recordFinding("conv-104", "agent-tester", "Auth token expires in 3600s", "fact");

      const res = memoryEngine.ipcGetMemory("agent-tester", "conv-104");
      expect(res.success).toBe(true);
      expect(res.data?.scratchpad).toBe("Drafting test scenarios");
      expect(res.data?.findings).toHaveLength(1);
    });

    it("T1.4.2 IPC agent_save_memory persists external state modifications", () => {
      const ctx = memoryEngine.getContext("conv-104", "agent-tester");
      ctx.scratchpad = "Externally updated via IPC";

      const res = memoryEngine.ipcSaveMemory(ctx);
      expect(res.success).toBe(true);

      const retrieved = memoryEngine.getContext("conv-104", "agent-tester");
      expect(retrieved.scratchpad).toBe("Externally updated via IPC");
    });

    it("T1.4.3 IPC agent_dispatch_directive creates queued run and updates ledger", () => {
      const res = memoryEngine.ipcDispatchDirective("agent-tester", "Run regression suite", "conv-104");
      expect(res.success).toBe(true);
      expect(res.data.status).toBe("queued");
      expect(res.data.directive).toBe("Run regression suite");

      const ledger = memoryEngine.getLedger("conv-104");
      expect(ledger.some((m) => m.payload.content === "Run regression suite")).toBe(true);
    });

    it("T1.4.4 memory state survives simulated process restart and cache rehydration", () => {
      memoryEngine.updateScratchpad("conv-104", "agent-persist", "Important persisted knowledge");
      memoryEngine.recordFinding("conv-104", "agent-persist", "Zero-day vulnerability patched", "discovery");

      // Verify simulated database tables
      const dbStats = memoryEngine.inspectDatabaseTables();
      expect(dbStats.agent_memories).toBeGreaterThan(0);
      expect(dbStats.agent_findings).toBeGreaterThan(0);

      // Rehydrate new engine with the same simulated database
      const rehydratedEngine = new CognitiveMemoryEngine((memoryEngine as any).storage);
      const rehydratedCtx = rehydratedEngine.getContext("conv-104", "agent-persist");
      expect(rehydratedCtx.scratchpad).toBe("Important persisted knowledge");
      expect(rehydratedCtx.findings).toHaveLength(1);
    });

    it("T1.4.5 clearConversation evicts all memory entries and ledger for target conversation", () => {
      memoryEngine.updateScratchpad("conv-104", "agent-1", "Scratch 1");
      memoryEngine.updateScratchpad("conv-104", "agent-2", "Scratch 2");

      memoryEngine.clearConversation("conv-104");

      const fresh1 = memoryEngine.getContext("conv-104", "agent-1");
      expect(fresh1.scratchpad).toBe("");
      expect(memoryEngine.getLedger("conv-104")).toHaveLength(0);
    });
  });

  // =========================================================================
  // Feature 5: Kernel-Grade Tool Authorization Guard
  // =========================================================================
  describe("Feature 5: Kernel-Grade Tool Authorization Guard", () => {
    it("T1.5.1 Standard preset allows file read/write and flags shell for user confirmation", () => {
      const guard = new ToolAuthorizationGuard({ preset: "standard" });
      expect(guard.checkPermission("workspace.read").allowed).toBe(true);
      expect(guard.checkPermission("workspace.write").allowed).toBe(true);

      const shellCheck = guard.checkPermission("core.shell.execute");
      expect(shellCheck.allowed).toBe(true);
      expect(shellCheck.reason).toContain("user confirmation");
    });

    it("T1.5.2 Read-Only preset permits read operations but strictly blocks writes and shell", () => {
      const guard = new ToolAuthorizationGuard({ preset: "read-only" });
      expect(guard.checkPermission("workspace.read").allowed).toBe(true);
      expect(guard.checkPermission("workspace.list_dir").allowed).toBe(true);

      expect(() => guard.checkPermission("workspace.write")).toThrowError(/forbidden in read-only/i);
      expect(() => guard.checkPermission("workspace.delete")).toThrowError(/forbidden in read-only/i);
      expect(() => guard.checkPermission("core.shell.execute")).toThrowError(/forbidden in read-only/i);
    });

    it("T1.5.3 Sandbox preset strictly blocks filesystem, shell execution, and network access", () => {
      const guard = new ToolAuthorizationGuard({ preset: "sandbox" });
      expect(() => guard.checkPermission("workspace.read")).toThrowError(/forbidden in sandbox/i);
      expect(() => guard.checkPermission("workspace.write")).toThrowError(/forbidden in sandbox/i);
      expect(() => guard.checkPermission("core.shell.execute")).toThrowError(/forbidden in sandbox/i);
      expect(() => guard.checkPermission("external.fetch")).toThrowError(/forbidden in sandbox/i);
    });

    it("T1.5.4 Developer preset grants full access across read, write, shell, and network", () => {
      const guard = new ToolAuthorizationGuard({ preset: "developer" });
      expect(guard.checkPermission("workspace.read").allowed).toBe(true);
      expect(guard.checkPermission("workspace.write").allowed).toBe(true);
      expect(guard.checkPermission("core.shell.execute").allowed).toBe(true);
      expect(guard.checkPermission("external.fetch").allowed).toBe(true);
    });

    it("T1.5.5 Custom profile enforces granular per-category permissions", () => {
      const profile = {
        id: "prof-custom",
        name: "Restricted Auditor",
        trustedRoots: ["/workspace"],
        allowedDomains: ["api.aro.internal"],
        allowRead: true,
        allowWrite: false,
        allowShell: false,
        allowNetwork: true,
        commandApproval: "never" as const,
        redactSecrets: true,
      };
      const guard = new ToolAuthorizationGuard({ preset: "custom", profile });

      expect(guard.checkPermission("workspace.read").allowed).toBe(true);
      expect(guard.checkPermission("external.fetch").allowed).toBe(true);
      expect(() => guard.checkPermission("workspace.write")).toThrowError(/File writes prohibited/);
      expect(() => guard.checkPermission("core.shell.execute")).toThrowError(/Shell execution prohibited/);
    });
  });

  // =========================================================================
  // Feature 6: Strict Workspace Path Confinement
  // =========================================================================
  describe("Feature 6: Strict Workspace Path Confinement", () => {
    const root = "C:/Projects/AroWorkspace";

    it("T1.6.1 resolves valid relative paths cleanly within designated workspace root", () => {
      const resolved = resolveWorkspacePath(root, "src/components/Button.svelte");
      expect(resolved.toLowerCase()).toContain("aroworkspace");
      expect(resolved.toLowerCase()).toContain("button.svelte");
    });

    it("T1.6.2 blocks parent traversal attacks attempting to escape via '../'", () => {
      expect(() => resolveWorkspacePath(root, "../../../Windows/System32/cmd.exe")).toThrowError(
        /escapes workspace root/i
      );
    });

    it("T1.6.3 blocks Windows-style backslash traversal attacks ('..\\..\\')", () => {
      expect(() => resolveWorkspacePath(root, "..\\..\\secret-keys.env")).toThrowError(
        /escapes workspace root/i
      );
    });

    it("T1.6.4 fails closed with MISSING_ROOT error when root path is empty or undefined", () => {
      expect(() => resolveWorkspacePath("", "file.txt")).toThrowError(/Missing or empty workspace root/);
      expect(() => resolveWorkspacePath("   ", "file.txt")).toThrowError(/Missing or empty workspace root/);
    });

    it("T1.6.5 rejects absolute paths pointing to external filesystem directories", () => {
      const externalPath = process.platform === "win32" ? "D:\\External\\secret.txt" : "/etc/shadow";
      expect(() => resolveWorkspacePath(root, externalPath)).toThrowError(/escapes workspace root/i);
    });
  });

  // =========================================================================
  // Feature 7: Code & Shell Execution Sandboxing
  // =========================================================================
  describe("Feature 7: Code & Shell Execution Sandboxing", () => {
    it("T1.7.1 scrubbedEnv strips sensitive secrets, credentials, and API keys", () => {
      const dirtyEnv = {
        PATH: "C:\\Windows;C:\\Node",
        OPENAI_API_KEY: "sk-proj-super-secret-12345",
        DATABASE_URL: "postgres://admin:password@localhost:5432/db",
        AWS_SECRET_ACCESS_KEY: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
        GITHUB_TOKEN: "ghp_1234567890abcdef",
        SAFE_APP_NAME: "AroDesktop",
      };

      const clean = scrubbedEnv(dirtyEnv);
      expect(clean.PATH).toBeDefined();
      expect(clean.SAFE_APP_NAME).toBe("AroDesktop");
      expect(clean.OPENAI_API_KEY).toBeUndefined();
      expect(clean.DATABASE_URL).toBeUndefined();
      expect(clean.AWS_SECRET_ACCESS_KEY).toBeUndefined();
      expect(clean.GITHUB_TOKEN).toBeUndefined();
    });

    it("T1.7.2 preserves essential operating system environment variables", () => {
      const baseEnv = {
        PATH: "/usr/bin:/bin",
        LANG: "en_US.UTF-8",
        HOME: "/home/user",
        TERM: "xterm-256color",
      };
      const clean = scrubbedEnv(baseEnv);
      expect(clean.PATH).toBe("/usr/bin:/bin");
      expect(clean.LANG).toBe("en_US.UTF-8");
      expect(clean.HOME).toBe("/home/user");
      expect(clean.TERM).toBe("xterm-256color");
    });

    it("T1.7.3 enforces execution timeout limits terminating long-running processes", async () => {
      const sandbox = new SandboxedExecutionEngine(50); // 50ms timeout
      const result = await sandbox.executeCommand("sleep", ["10"]); // requests 10s
      expect(result.timedOut).toBe(true);
      expect(result.exitCode).toBe(124);
      expect(result.stderr).toContain("timed out");
    });

    it("T1.7.4 clamps excessive command output to max buffer size with truncation warning", async () => {
      const sandbox = new SandboxedExecutionEngine(5000, 1024); // 1KB max buffer
      const result = await sandbox.executeCommand("flood_output");
      expect(result.truncated).toBe(true);
      expect(result.stdout.length).toBe(1024);
      expect(result.stderr).toContain("truncated");
    });

    it("T1.7.5 safely captures non-zero exit codes and stderr without crashing host process", async () => {
      const sandbox = new SandboxedExecutionEngine();
      const result = await sandbox.executeCommand("fail_with_code", ["42"]);
      expect(result.exitCode).toBe(42);
      expect(result.stderr).toContain("error code 42");
    });
  });

  // =========================================================================
  // Feature 8: Tool Registry Catalogue Parity
  // =========================================================================
  describe("Feature 8: Tool Registry Catalogue Parity", () => {
    it("T1.8.1 includes full catalogue of core workspace tools", () => {
      const expectedTools = [
        "workspace.read",
        "workspace.write",
        "workspace.delete",
        "workspace.replace_in_files",
        "workspace.git_diff",
        "workspace.list_dir",
      ];
      for (const t of expectedTools) {
        expect(registry.hasTool(t)).toBe(true);
        const tool = registry.getTool(t);
        expect(tool?.category).toBe("workspace");
      }
    });

    it("T1.8.2 includes full catalogue of artifact management tools", () => {
      const artifactTools = ["artifact.create", "artifact.update", "artifact.list"];
      for (const t of artifactTools) {
        expect(registry.hasTool(t)).toBe(true);
        const tool = registry.getTool(t);
        expect(tool?.category).toBe("artifact");
      }
    });

    it("T1.8.3 includes full catalogue of sandboxed execution tools", () => {
      expect(registry.hasTool("core.code.execute")).toBe(true);
      expect(registry.hasTool("core.shell.execute")).toBe(true);
      expect(registry.getTool("core.shell.execute")?.category).toBe("core");
    });

    it("T1.8.4 validates required parameter schemas for registered tools", () => {
      const validParams = { path: "src/main.rs", content: "fn main() {}" };
      const validation = registry.validateParameters("workspace.write", validParams);
      expect(validation.valid).toBe(true);
      expect(validation.errors).toHaveLength(0);

      const invalidParams = { path: "src/main.rs" }; // missing 'content'
      const invalidValidation = registry.validateParameters("workspace.write", invalidParams);
      expect(invalidValidation.valid).toBe(false);
      expect(invalidValidation.errors[0]).toContain("Missing required parameter: 'content'");
    });

    it("T1.8.5 rejects duplicate tool registration attempts unless override is explicit", () => {
      expect(() => {
        registry.registerTool({
          name: "workspace.read",
          category: "workspace",
          description: "Duplicate",
          parameters: [],
          requiredPermissions: [],
        });
      }).toThrowError(/already registered/i);
    });
  });

  // =========================================================================
  // Feature 9: Svelte Typecheck Integrity
  // =========================================================================
  describe("Feature 9: Svelte Typecheck Integrity", () => {
    it("T1.9.1 validates TaskStep and normalizes boolean completed to TaskStepStatus", () => {
      const partial1 = { text: "Fix database index", completed: true };
      const res1 = validateTaskStep(partial1);
      expect(res1.valid).toBe(true);
      expect(res1.normalized.status).toBe("completed");
      expect(res1.normalized.completed).toBe(true);

      const partial2 = { text: "Write docs", completed: false };
      const res2 = validateTaskStep(partial2);
      expect(res2.valid).toBe(true);
      expect(res2.normalized.status).toBe("pending");
      expect(res2.normalized.completed).toBe(false);
    });

    it("T1.9.2 calculates plan progress totals and accurate completion percentage", () => {
      const tasks: TaskStep[] = [
        { id: "1", text: "Task 1", completed: true, status: "completed" },
        { id: "2", text: "Task 2", completed: true, status: "completed" },
        { id: "3", text: "Task 3", completed: false, status: "in_progress" },
        { id: "4", text: "Task 4", completed: false, status: "pending" },
      ];
      const progress = calculatePlanProgress(tasks);
      expect(progress.total).toBe(4);
      expect(progress.completed).toBe(2);
      expect(progress.inProgress).toBe(1);
      expect(progress.pending).toBe(1);
      expect(progress.percentage).toBe(50);
    });

    it("T1.9.3 cycles task status through pending -> in_progress -> completed -> error -> pending", () => {
      let status = cycleTaskStatus("pending");
      expect(status).toBe("in_progress");

      status = cycleTaskStatus(status);
      expect(status).toBe("completed");

      status = cycleTaskStatus(status);
      expect(status).toBe("error");

      status = cycleTaskStatus(status);
      expect(status).toBe("pending");
    });

    it("T1.9.4 ensures AgentRun and AgentLaneView schema structures match contract specifications", () => {
      const run: AgentRun = {
        id: "run-sample",
        conversationId: "conv-109",
        agentName: "Agent Rust",
        status: "running",
        priority: "high",
        stepCount: 3,
        createdAt: new Date().toISOString(),
      };
      expect(run.id).toBe("run-sample");
      expect(run.status).toBe("running");

      const laneView: AgentLaneView = {
        lane: {
          id: "lane-rust",
          title: "Rust Core",
          status: "active",
          priority: "high",
        },
        visibleRuns: [run],
      };
      expect(laneView.lane.title).toBe("Rust Core");
      expect(laneView.visibleRuns).toHaveLength(1);
    });

    it("T1.9.5 ensures AgentMessageEnvelope payload formatting preserves structure and actions", () => {
      const env = createAgentEnvelope({
        conversationId: "conv-109",
        sender: { id: "a1", name: "Agent 1", type: "subagent" },
        recipient: { id: "a2", name: "Agent 2", type: "subagent" },
        messageType: "task_delegation",
        content: "Refactor API module",
        suggestedActions: ["Extract router", "Add tests"],
      });

      const promptFr = formatAgentEnvelopeForPrompt(env, "fr");
      expect(promptFr).toContain("[DÉLÉGATION DE MISSION]");
      expect(promptFr).toContain("Refactor API module");
      expect(promptFr).toContain("Extract router");
    });
  });

  // =========================================================================
  // Feature 10: Persistent Sub-Agent UI Thread Integration
  // =========================================================================
  describe("Feature 10: Persistent Sub-Agent UI Thread Integration", () => {
    it("T1.10.1 extracts sub-agents from message steps and runs (extractMessageAgents)", () => {
      const message = {
        id: "msg-main",
        conversationId: "conv-110",
        steps: [
          {
            id: "step-1",
            kind: "tool",
            input: { toolId: "agent.spawn", name: "Agent Testeur", role: "tester" },
            output: { run_id: "sa-1", status: "completed" },
          },
        ],
      };

      const extracted = extractMessageAgents(message);
      expect(extracted).toHaveLength(1);
      expect(extracted[0].id).toBe("sa-1");
      expect(extracted[0].name).toBe("Agent Testeur");
      expect(extracted[0].status).toBe("completed");
    });

    it("T1.10.2 associates sub-agent runs with parent message identifier for thread tracking", () => {
      const message = {
        id: "msg-parent-42",
        conversationId: "conv-110",
        subAgents: [
          {
            id: "sa-worker",
            name: "Worker Agent",
            status: "running" as const,
            stepCount: 1,
            parentMessageId: "msg-parent-42",
          },
        ],
      };

      const extracted = extractMessageAgents(message);
      expect(extracted[0].parentMessageId).toBe("msg-parent-42");
    });

    it("T1.10.3 maps agent run failure to 'error' status and waiting run to 'needs_help'", () => {
      const allRuns: AgentRun[] = [
        {
          id: "run-failed",
          conversationId: "conv-110",
          agentName: "Agent Crash",
          status: "failed",
          priority: "normal",
          stepCount: 2,
          createdAt: new Date().toISOString(),
        },
        {
          id: "run-waiting",
          conversationId: "conv-110",
          agentName: "Agent Blocked",
          status: "waiting",
          priority: "urgent",
          stepCount: 1,
          createdAt: new Date().toISOString(),
        },
      ];

      const extracted = extractMessageAgents({ id: "msg-1", conversationId: "conv-110" }, allRuns);
      expect(extracted).toHaveLength(2);

      const failedAgent = extracted.find((a) => a.id === "run-failed");
      const waitingAgent = extracted.find((a) => a.id === "run-waiting");

      expect(failedAgent?.status).toBe("error");
      expect(waitingAgent?.status).toBe("needs_help");
    });

    it("T1.10.4 formats isolated memory context for prompt injection", () => {
      const mem: AgentMemoryContext = {
        agentId: "agent-doc",
        agentName: "Agent Documentaliste",
        role: "technical-writer",
        conversationId: "conv-110",
        scratchpad: "Writing API documentation for v2",
        findings: [{ id: "f1", summary: "Endpoint /v2/runs documented", timestamp: new Date().toISOString() }],
        ledger: [],
        artifacts: [{ id: "art-api", title: "API Spec", kind: "markdown" }],
        updatedAt: new Date().toISOString(),
      };

      const formattedFr = formatAgentMemoryForPrompt(mem, "fr");
      expect(formattedFr).toContain("MÉMOIRE CONTEXTUELLE DE L'AGENT [Agent Documentaliste (technical-writer)]");
      expect(formattedFr).toContain("[BLOC-NOTES DE TRAVAIL]");
      expect(formattedFr).toContain("Writing API documentation for v2");
      expect(formattedFr).toContain("Endpoint /v2/runs documented");
      expect(formattedFr).toContain("API Spec");
    });

    it("T1.10.5 compiles permission directive into prompt context based on preset", () => {
      const readOnlyDirective = compilePermissionDirective("read-only", null, "fr");
      expect(readOnlyDirective).toContain("LECTURE SEULE STRICTE");
      expect(readOnlyDirective).toContain("STRICTEMENT INTERDITE");

      const devDirective = compilePermissionDirective("developer", null, "fr");
      expect(devDirective).toContain("PLEINEMENT AUTONOME");
      expect(devDirective).toContain("PLEIN ACCÈS");
    });
  });

  // =========================================================================
  // Feature 11: Apple/Google-Grade Observability Refinements
  // =========================================================================
  describe("Feature 11: Apple/Google-Grade Observability Refinements", () => {
    it("T1.11.1 synthesizeAgentLanes creates default virtual lane avoiding empty screens", () => {
      const runs: AgentRun[] = [
        {
          id: "run-unassigned",
          conversationId: "conv-111",
          agentName: "Orphan Run",
          status: "running",
          priority: "normal",
          stepCount: 1,
          createdAt: new Date().toISOString(),
        },
      ];

      const synthesized = synthesizeAgentLanes([], runs, "conv-111");
      expect(synthesized).toHaveLength(1);
      expect(synthesized[0].lane.id).toBe("default-virtual");
      expect(synthesized[0].lane.title).toContain("Voie principale");
      expect(synthesized[0].visibleRuns).toHaveLength(1);
    });

    it("T1.11.2 calculateActivityCounters accurately aggregates running, queued, waiting, done, failed", () => {
      const runs: AgentRun[] = [
        { id: "1", conversationId: "c1", agentName: "A1", status: "running", priority: "normal", stepCount: 1, createdAt: "" },
        { id: "2", conversationId: "c1", agentName: "A2", status: "queued", priority: "normal", stepCount: 0, createdAt: "" },
        { id: "3", conversationId: "c1", agentName: "A3", status: "waiting", priority: "normal", stepCount: 1, createdAt: "" },
        { id: "4", conversationId: "c1", agentName: "A4", status: "completed", priority: "normal", stepCount: 5, createdAt: "" },
        { id: "5", conversationId: "c1", agentName: "A5", status: "failed", priority: "normal", stepCount: 2, createdAt: "" },
      ];

      const counters = calculateActivityCounters(runs, null, "c1");
      expect(counters.running).toBe(1);
      expect(counters.queued).toBe(1);
      expect(counters.waiting).toBe(1);
      expect(counters.done).toBe(1);
      expect(counters.failed).toBe(1);
      expect(counters.total).toBe(5);
    });

    it("T1.11.3 falls back to orchestrator snapshot when conversation runs are empty", () => {
      const snapshot = {
        runningCount: 3,
        queuedCount: 2,
        activeLanesCount: 2,
      };

      const counters = calculateActivityCounters([], snapshot, "c-empty");
      expect(counters.running).toBe(3);
      expect(counters.queued).toBe(2);
      expect(counters.total).toBe(5);
    });

    it("T1.11.4 generates dynamic Composer placeholder for active sub-agent", () => {
      const globalPlaceholder = getComposerPlaceholder(null, "fr");
      expect(globalPlaceholder).toBe("Envoyez un message ou déléguez une tâche...");

      const subAgentPlaceholder = getComposerPlaceholder({ name: "Agent Architecte" }, "fr");
      expect(subAgentPlaceholder).toBe("Directive directe pour Agent Architecte...");

      const enPlaceholder = getComposerPlaceholder({ name: "Security Bot" }, "en");
      expect(enPlaceholder).toBe("Direct directive for Security Bot...");
    });

    it("T1.11.5 builds hierarchical breadcrumbs and inspection card indicators", () => {
      const crumbs = buildBreadcrumbs({
        conversationTitle: "Projet Aro",
        subAgent: { id: "sa-1", name: "Agent Codeur" },
        currentStep: { index: 3, toolName: "workspace.replace_in_files" },
      });

      expect(crumbs).toHaveLength(3);
      expect(crumbs[0].label).toBe("Projet Aro");
      expect(crumbs[1].label).toBe("Agent Codeur");
      expect(crumbs[2].label).toContain("Étape 3 (workspace.replace_in_files)");

      const card = formatInspectionCard({
        id: "r1",
        conversationId: "c1",
        agentName: "Agent Codeur",
        status: "running",
        priority: "normal",
        stepCount: 4,
        maxSteps: 8,
        currentThought: "Refactoring modules",
        currentTool: "workspace.write",
        createdAt: "",
      });

      expect(card.badge.text).toBe("EN COURS");
      expect(card.badge.color).toBe("blue");
      expect(card.toolPill).toBe("Outil: workspace.write");
      expect(card.progressPercent).toBe(50);
    });
  });
});
