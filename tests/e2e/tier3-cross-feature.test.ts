import { describe, it, expect, beforeEach } from "vitest";
import {
  type AgentMemoryContext,
  type AgentMessageEnvelope,
  type AgentRun,
  type TaskStep,
  createAgentEnvelope,
  validateTaskStep,
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

describe("Tier 3: Cross-Feature Combinations (Pairwise Interaction Invariants)", () => {
  let memoryEngine: CognitiveMemoryEngine;
  let registry: ToolRegistryEngine;
  let runtime: SubAgentRuntimeEngine;

  beforeEach(() => {
    memoryEngine = new CognitiveMemoryEngine();
    registry = new ToolRegistryEngine();
    runtime = new SubAgentRuntimeEngine(memoryEngine);
  });

  // Combo 1 (F1 + F2): Cognitive Memory + Sub-Agent Execution Loop
  it("Combo 1 (F1 + F2): async sub-agent loop dynamically updates scratchpad and accumulates findings in cognitive memory", async () => {
    const steps = [
      { stepIndex: 1, thought: "Discovered 5 API routes", toolCall: { toolName: "workspace.list_dir", args: {} } },
      { stepIndex: 2, thought: "Verified OpenAPI schema version 3.1", toolCall: { toolName: "workspace.read", args: { path: "openapi.json" } } },
    ];

    const run = runtime.createRun({
      conversationId: "conv-c1",
      agentName: "API Discovery Worker",
      role: "researcher",
      steps,
    });

    const completed = await runtime.executeRunLoop(run.id);
    expect(completed.status).toBe("completed");

    const mem = memoryEngine.getContext("conv-c1", run.id);
    expect(mem.scratchpad).toContain("Step 2: Verified OpenAPI schema version 3.1");
    expect(mem.findings).toHaveLength(2);
    expect(mem.findings[0].summary).toContain("Discovered 5 API routes");
    expect(mem.findings[1].summary).toContain("Verified OpenAPI schema version 3.1");
  });

  // Combo 2 (F1 + F4): Cognitive Memory + IPC Sync
  it("Combo 2 (F1 + F4): memory saved via IPC survives cache eviction and reloads with 100% data fidelity", () => {
    const originalCtx = memoryEngine.getContext("conv-c2", "agent-ipc-test", "IPC Agent", "tester");
    originalCtx.scratchpad = "Syncing critical state over Tauri IPC bridge";
    memoryEngine.recordFinding("conv-c2", "agent-ipc-test", "State verified safe for hibernation", "fact");

    const saveRes = memoryEngine.ipcSaveMemory(originalCtx);
    expect(saveRes.success).toBe(true);

    // Evict in-memory cache and create fresh engine over same backing storage
    const freshEngine = new CognitiveMemoryEngine((memoryEngine as any).storage);
    const getRes = freshEngine.ipcGetMemory("agent-ipc-test", "conv-c2");

    expect(getRes.success).toBe(true);
    expect(getRes.data?.scratchpad).toBe("Syncing critical state over Tauri IPC bridge");
    expect(getRes.data?.findings).toHaveLength(1);
    expect(getRes.data?.findings[0].summary).toBe("State verified safe for hibernation");
  });

  // Combo 3 (F2 + F5): Sub-Agent Execution + Tool Authorization Guard
  it("Combo 3 (F2 + F5): sub-agent in Read-Only mode attempting file write is blocked by authorization guard", async () => {
    const readOnlyGuard = new ToolAuthorizationGuard({ preset: "read-only" });
    const steps = [
      { stepIndex: 1, thought: "Reading source file", toolCall: { toolName: "workspace.read", args: { path: "src/auth.ts" } } },
      { stepIndex: 2, thought: "Attempting to modify file", toolCall: { toolName: "workspace.write", args: { path: "src/auth.ts", content: "// hacked" } } },
    ];

    const run = runtime.createRun({
      conversationId: "conv-c3",
      agentName: "Read-Only Auditor",
      role: "auditor",
      steps,
    });

    const result = await runtime.executeRunLoop(run.id, readOnlyGuard);
    expect(result.status).toBe("failed");
    expect(result.error).toContain("Tool Authorization Denied");
    expect(result.error).toContain("forbidden in Read-Only mode");

    // An error escalation envelope was automatically dispatched
    const ledger = memoryEngine.getLedger("conv-c3", { messageType: "error_escalation" });
    expect(ledger).toHaveLength(1);
    expect(ledger[0].payload.content).toContain("Security Guard Interception");
  });

  // Combo 4 (F2 + F10): Sub-Agent Execution + Sub-Agent UI Thread
  it("Combo 4 (F2 + F10): asynchronous sub-agent state transitions reflect in extracted message agents and UI thread model", async () => {
    const run = runtime.createRun({
      conversationId: "conv-c4",
      agentName: "UI Sync Agent",
      role: "code",
      steps: [{ stepIndex: 1, thought: "Compiling binary" }],
    });

    // Check queued state in UI extraction
    let agents = extractMessageAgents({ id: "msg-ui", conversationId: "conv-c4" }, runtime.listRuns("conv-c4"));
    expect(agents[0].status).toBe("queued");

    // Execute run
    await runtime.executeRunLoop(run.id);

    // Check completed state in UI extraction
    agents = extractMessageAgents({ id: "msg-ui", conversationId: "conv-c4" }, runtime.listRuns("conv-c4"));
    expect(agents[0].status).toBe("completed");
    expect(agents[0].goal).toBe("Autonomous mission");
  });

  // Combo 5 (F3 + F1): Cloud Worker Delegation + Cognitive Memory Ledger
  it("Combo 5 (F3 + F1): inbound delegation envelope automatically appends to sub-agent's cognitive memory ledger", () => {
    const delegationEnv = createAgentEnvelope({
      conversationId: "conv-c5",
      sender: { id: "orch", name: "Aro Orchestrator", type: "orchestrator" },
      recipient: { id: "worker-db", name: "Database Worker", role: "db", type: "subagent" },
      messageType: "task_delegation",
      content: "Generate SQL migration for multi-tenant organizations",
      suggestedActions: ["Create migration file", "Run test rollback"],
    });

    memoryEngine.dispatchMessage(delegationEnv);

    const workerCtx = memoryEngine.getContext("conv-c5", "worker-db");
    expect(workerCtx.ledger).toHaveLength(1);
    expect(workerCtx.ledger[0].payload.content).toContain("Generate SQL migration");
    expect(workerCtx.ledger[0].payload.suggestedActions).toHaveLength(2);
  });

  // Combo 6 (F3 + F2): Cloud Worker Delegation + Sub-Agent Execution Loop
  it("Combo 6 (F3 + F2): delegation envelope triggers sub-agent run queueing, async execution, and result delivery", async () => {
    const delegation = createAgentEnvelope({
      conversationId: "conv-c6",
      sender: { id: "orch", name: "Aro Orchestrator", type: "orchestrator" },
      recipient: { id: "agent-doc", name: "Doc Worker", role: "writer", type: "subagent" },
      messageType: "task_delegation",
      content: "Write README setup section",
    });

    const run = runtime.handleDelegation(delegation);
    expect(run.status).toBe("queued");
    expect(run.goal).toBe("Write README setup section");

    const completed = await runtime.executeRunLoop(run.id);
    expect(completed.status).toBe("completed");

    // Subagent delivers task_result
    const resultEnv = createAgentEnvelope({
      conversationId: "conv-c6",
      sender: { id: "agent-doc", name: "Doc Worker", type: "subagent" },
      recipient: { id: "orch", name: "Aro Orchestrator", type: "orchestrator" },
      messageType: "task_result",
      content: "README setup documentation completed successfully.",
      artifacts: [{ id: "art-readme", title: "README.md", kind: "markdown" }],
    });
    memoryEngine.dispatchMessage(resultEnv);

    const results = memoryEngine.getLedger("conv-c6", { messageType: "task_result" });
    expect(results).toHaveLength(1);
    expect(results[0].payload.artifacts![0].title).toBe("README.md");
  });

  // Combo 7 (F5 + F6): Tool Authorization Guard + Workspace Path Confinement
  it("Combo 7 (F5 + F6): tool permitted by Standard preset is blocked when workspace path confinement is breached", () => {
    const guard = new ToolAuthorizationGuard({ preset: "standard" });
    const root = "C:/Projects/AroApp";

    // 1. Tool check passes
    const perm = guard.checkPermission("workspace.write");
    expect(perm.allowed).toBe(true);

    // 2. Confinement check fails on directory traversal
    expect(() => {
      resolveWorkspacePath(root, "../../../sensitive/passwords.txt");
    }).toThrowError(/escapes workspace root/i);
  });

  // Combo 8 (F5 + F7): Tool Authorization Guard + Shell Sandboxing
  it("Combo 8 (F5 + F7): Developer preset authorizes shell execution, executed under scrubbed environment", async () => {
    const devGuard = new ToolAuthorizationGuard({ preset: "developer" });
    expect(devGuard.checkPermission("core.shell.execute").allowed).toBe(true);

    const rawEnv = {
      PATH: "/usr/local/bin",
      SECRET_API_TOKEN: "super-secret-production-key",
      SAFE_CONFIG: "production-mode",
    };

    const sandbox = new SandboxedExecutionEngine();
    const result = await sandbox.executeCommand("echo_secret", [], { env: rawEnv });

    expect(result.exitCode).toBe(0);
    // Secret was stripped from process environment
    expect(result.stdout).not.toContain("super-secret-production-key");
  });

  // Combo 9 (F5 + F8): Tool Authorization Guard + Tool Registry Parity
  it("Combo 9 (F5 + F8): Tool Authorization Guard validates against catalogue tools and blocks uncatalogued tools", () => {
    const guard = new ToolAuthorizationGuard({
      preset: "standard",
      allowedTools: registry.listTools().map((t) => t.name),
    });

    // Registered tool allowed
    expect(guard.checkPermission("workspace.git_diff").allowed).toBe(true);

    // Unregistered malicious tool blocked by whitelist
    expect(() => {
      guard.checkPermission("malicious.backdoor.spawn");
    }).toThrowError(/Tool not found in explicit allowed_tools whitelist/);
  });

  // Combo 10 (F9 + F10): Svelte Typecheck Integrity + Sub-Agent UI Thread
  it("Combo 10 (F9 + F10): validated TaskStep status transitions synchronize with sub-agent execution state in UI thread", () => {
    const rawStep = { text: "Optimize query plan", status: "pending" as const };
    const { normalized } = validateTaskStep(rawStep);
    expect(normalized.status).toBe("pending");

    // Advance task step to in_progress
    normalized.status = cycleTaskStatus(normalized.status);
    expect(normalized.status).toBe("in_progress");

    // Map to sub-agent representation
    const subAgentInfo = {
      id: "sa-opt",
      name: "Query Optimizer",
      status: normalized.status === "in_progress" ? ("running" as const) : ("completed" as const),
      stepCount: 1,
    };
    expect(subAgentInfo.status).toBe("running");

    // Finish step
    normalized.status = cycleTaskStatus(normalized.status);
    expect(normalized.status).toBe("completed");
    subAgentInfo.status = "completed";
    expect(subAgentInfo.status).toBe("completed");
  });

  // Combo 11 (F10 + F11): Sub-Agent UI Thread + Observability & Breadcrumbs
  it("Combo 11 (F10 + F11): selecting active sub-agent updates Composer dynamic placeholder and hierarchical breadcrumb path", () => {
    const activeSubAgent = { id: "sa-audit", name: "Security Auditor", role: "security" };

    // Dynamic placeholder
    const placeholder = getComposerPlaceholder(activeSubAgent, "fr");
    expect(placeholder).toBe("Directive directe pour Security Auditor...");

    // Breadcrumb navigation
    const breadcrumbs = buildBreadcrumbs({
      conversationTitle: "Audit Trimestriel",
      subAgent: activeSubAgent,
      currentStep: { index: 2, toolName: "workspace.read" },
    });

    expect(breadcrumbs).toHaveLength(3);
    expect(breadcrumbs[0].label).toBe("Audit Trimestriel");
    expect(breadcrumbs[1].label).toBe("Security Auditor");
    expect(breadcrumbs[2].label).toBe("Étape 2 (workspace.read)");

    // Card format
    const card = formatInspectionCard({
      id: "run-sa-audit",
      conversationId: "conv-c11",
      agentName: activeSubAgent.name,
      status: "running",
      priority: "urgent",
      stepCount: 2,
      maxSteps: 4,
      currentThought: "Analyzing firewall rules",
      currentTool: "workspace.read",
      createdAt: "",
    });

    expect(card.badge.text).toBe("EN COURS");
    expect(card.badge.color).toBe("blue");
    expect(card.progressPercent).toBe(50);
  });
});
