import { describe, it, expect, beforeEach } from "vitest";
import {
  type AgentMemoryContext,
  type AgentMessageEnvelope,
  type AgentArtifactRef,
  type AgentRun,
  type AgentLaneView,
  createAgentEnvelope,
} from "@aro/contracts";

import { CognitiveMemoryEngine, InMemoryStorageBackend } from "./helpers/cognitive-memory-engine";
import { ToolAuthorizationGuard, ToolAuthorizationError } from "./helpers/security-guard-engine";
import { resolveWorkspacePath, WorkspaceConfinementError } from "./helpers/workspace-confinement-engine";
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

describe("Tier 4: Real-World Application Scenarios (Comprehensive End-to-End Workflows)", () => {
  let memoryEngine: CognitiveMemoryEngine;
  let registry: ToolRegistryEngine;
  let runtime: SubAgentRuntimeEngine;

  beforeEach(() => {
    memoryEngine = new CognitiveMemoryEngine();
    registry = new ToolRegistryEngine();
    runtime = new SubAgentRuntimeEngine(memoryEngine);
  });

  // =========================================================================
  // Scenario 1: Autonomous Multi-Agent Code Audit
  // Features: F1, F2, F5, F6, F10, F11
  // =========================================================================
  it("Scenario 1: Autonomous Multi-Agent Code Audit (F1, F2, F5, F6, F10, F11)", async () => {
    const conversationId = "conv-audit-2026";
    const workspaceRoot = "C:/Projects/AroCore";

    // 1. Orchestrator delegates security audit to "Security Auditor" subagent
    const delegation = createAgentEnvelope({
      conversationId,
      sender: { id: "orch-master", name: "Aro Orchestrator", type: "orchestrator" },
      recipient: { id: "sec-auditor", name: "Security Auditor", role: "security", type: "subagent" },
      messageType: "task_delegation",
      content: "Audit crates/aro-tools for path traversal and command injection vulnerabilities",
      suggestedActions: ["List files", "Read security.rs", "Identify CVEs", "Generate report"],
    });

    const run = runtime.handleDelegation(delegation);
    expect(run.status).toBe("queued");

    // 2. Set up Read-Only Guard with strict workspace confinement
    const auditGuard = new ToolAuthorizationGuard({ preset: "read-only" });

    // 3. Security Auditor executes planned steps under confinement
    const validFilePath = resolveWorkspacePath(workspaceRoot, "src/security.rs");
    expect(validFilePath.toLowerCase()).toContain("arocore");

    // Simulated audit steps
    const auditSteps = [
      { stepIndex: 1, thought: "Listing workspace tools", toolCall: { toolName: "workspace.list_dir", args: { path: "src" } } },
      { stepIndex: 2, thought: "Reading security filter implementation", toolCall: { toolName: "workspace.read", args: { path: "src/security.rs" } } },
      { stepIndex: 3, thought: "Documenting CWE-22 Path Traversal edge cases" },
    ];

    const plannedRun = runtime.createRun({
      id: run.id,
      conversationId,
      agentName: run.agentName,
      role: run.role,
      steps: auditSteps,
    });

    const completedRun = await runtime.executeRunLoop(plannedRun.id, auditGuard);
    expect(completedRun.status).toBe("completed");

    // 4. Record findings and audit report artifact in cognitive memory
    const finding = memoryEngine.recordFinding(
      conversationId,
      run.id,
      "CWE-22: resolve_workspace_path must fail closed when root path is empty",
      "discovery",
      "workspace.read"
    );
    expect(finding.id).toBeDefined();

    const reportArtifact: AgentArtifactRef = {
      id: "art-audit-001",
      title: "Security Audit Report 2026",
      kind: "markdown",
      uri: "workspace://reports/security-audit.md",
    };
    memoryEngine.recordArtifact(conversationId, run.id, reportArtifact);

    // 5. Verify Observability: Breadcrumbs & Inspection Card
    const breadcrumbs = buildBreadcrumbs({
      conversationTitle: "Security Audit 2026",
      subAgent: { id: run.id, name: run.agentName },
      currentStep: { index: 3, toolName: "workspace.read" },
    });
    expect(breadcrumbs[0].label).toBe("Security Audit 2026");
    expect(breadcrumbs[1].label).toBe("Security Auditor");
    expect(breadcrumbs[2].label).toContain("Étape 3");

    const inspection = formatInspectionCard(completedRun);
    expect(inspection.badge.text).toBe("TERMINÉ");
    expect(inspection.badge.color).toBe("green");

    // 6. Deliver task_result envelope back to orchestrator
    const resultEnv = createAgentEnvelope({
      conversationId,
      sender: { id: run.id, name: run.agentName, type: "subagent" },
      recipient: { id: "orch-master", name: "Aro Orchestrator", type: "orchestrator" },
      messageType: "task_result",
      content: "Audit complete. 1 vulnerability identified and documented in report artifact.",
      artifacts: [reportArtifact],
    });
    memoryEngine.dispatchMessage(resultEnv);

    const memory = memoryEngine.getContext(conversationId, run.id);
    expect(memory.findings).toHaveLength(3); // 2 from tools + 1 explicit
    expect(memory.artifacts).toHaveLength(1);
    expect(memory.ledger.some((m) => m.messageType === "task_result")).toBe(true);
  });

  // =========================================================================
  // Scenario 2: Read-Only Sandbox Exploration
  // Features: F5, F6, F7, F8, F11
  // =========================================================================
  it("Scenario 2: Read-Only Sandbox Exploration (F5, F6, F7, F8, F11)", async () => {
    const sandboxGuard = new ToolAuthorizationGuard({ preset: "sandbox" });

    // Verify all tools in catalogue exist
    const attemptedTools = ["workspace.write", "core.shell.execute", "external.fetch", "workspace.read"];
    for (const t of attemptedTools) {
      expect(registry.hasTool(t)).toBe(true);
    }

    // Intercept and assert every disallowed action fails closed
    expect(() => sandboxGuard.checkPermission("workspace.write")).toThrowError(
      /File write and mutation operations are forbidden in Sandbox mode/
    );
    expect(() => sandboxGuard.checkPermission("core.shell.execute")).toThrowError(
      /Command execution is forbidden in Sandbox mode/
    );
    expect(() => sandboxGuard.checkPermission("external.fetch")).toThrowError(
      /External network access is forbidden in Sandbox mode/
    );
    expect(() => sandboxGuard.checkPermission("workspace.read")).toThrowError(
      /Local filesystem access is forbidden in Sandbox mode/
    );

    // Permitted: context reading without filesystem touch
    const allowedContext = sandboxGuard.checkPermission("agent.read_context");
    expect(allowedContext.allowed).toBe(true);

    // Observability live log simulation: errors logged for blocked attempts
    const counters = calculateActivityCounters([], { runningCount: 0, queuedCount: 0, activeLanesCount: 1 }, "conv-s2");
    expect(counters.total).toBe(0);
  });

  // =========================================================================
  // Scenario 3: State Recovery Across Restart
  // Features: F1, F4, F10
  // =========================================================================
  it("Scenario 3: State Recovery Across Restart (F1, F4, F10)", () => {
    const sharedStorage = new InMemoryStorageBackend();
    const engineBeforeRestart = new CognitiveMemoryEngine(sharedStorage);
    const convId = "conv-recovery-test";

    // Subagent 1: Backend Coder
    engineBeforeRestart.updateScratchpad(convId, "agent-backend", "Refactored SQLite connection pooling");
    engineBeforeRestart.recordFinding(convId, "agent-backend", "Pool size optimal at 16 connections", "fact");
    engineBeforeRestart.recordArtifact(convId, "agent-backend", {
      id: "art-pool-config",
      title: "pool.rs",
      kind: "rust",
      uri: "crates/aro-memory/src/pool.rs",
    });

    // Subagent 2: Frontend Coder
    engineBeforeRestart.updateScratchpad(convId, "agent-frontend", "Wired Svelte 5 runes for agent lanes");
    engineBeforeRestart.recordFinding(convId, "agent-frontend", "Zero reactivity lag measured", "discovery");

    // Inter-agent message between them
    const peerMessage = createAgentEnvelope({
      conversationId: convId,
      sender: { id: "agent-backend", name: "Backend Lead", type: "subagent" },
      recipient: { id: "agent-frontend", name: "Frontend Lead", type: "subagent" },
      messageType: "peer_collaboration",
      content: "IPC endpoints ready for pool stats",
    });
    engineBeforeRestart.dispatchMessage(peerMessage);

    // -------------------------------------------------------------
    // SIMULATED DESKTOP RESTART: Engine destroyed, new instance spawned
    // -------------------------------------------------------------
    const engineAfterRestart = new CognitiveMemoryEngine(sharedStorage);

    // Rehydrate and verify Backend Lead state
    const backendRes = engineAfterRestart.ipcGetMemory("agent-backend", convId);
    expect(backendRes.success).toBe(true);
    expect(backendRes.data?.scratchpad).toBe("Refactored SQLite connection pooling");
    expect(backendRes.data?.findings).toHaveLength(1);
    expect(backendRes.data?.findings[0].summary).toBe("Pool size optimal at 16 connections");
    expect(backendRes.data?.artifacts).toHaveLength(1);

    // Rehydrate and verify Frontend Lead state
    const frontendRes = engineAfterRestart.ipcGetMemory("agent-frontend", convId);
    expect(frontendRes.success).toBe(true);
    expect(frontendRes.data?.scratchpad).toBe("Wired Svelte 5 runes for agent lanes");
    expect(frontendRes.data?.findings).toHaveLength(1);

    // Rehydrate and verify ledger
    const ledger = engineAfterRestart.getLedger(convId);
    expect(ledger).toHaveLength(1);
    expect(ledger[0].payload.content).toBe("IPC endpoints ready for pool stats");

    // UI Thread extraction verification
    const agents = extractMessageAgents({ id: "msg-rehydrate", conversationId: convId });
    expect(agents).toBeDefined();
  });

  // =========================================================================
  // Scenario 4: Sandboxed Code Execution with Secret Scrubbing
  // Features: F5, F7
  // =========================================================================
  it("Scenario 4: Sandboxed Code Execution with Secret Scrubbing (F5, F7)", async () => {
    const devGuard = new ToolAuthorizationGuard({ preset: "developer" });

    // Developer preset permits shell execution
    expect(devGuard.checkPermission("core.shell.execute").allowed).toBe(true);
    expect(devGuard.checkPermission("core.code.execute").allowed).toBe(true);

    // Contaminated environment with simulated sensitive production secrets
    const contaminatedEnv: Record<string, string> = {
      PATH: "/usr/bin:/bin",
      LANG: "fr_FR.UTF-8",
      OPENAI_API_KEY: "sk-live-0987654321fedcba",
      ANTHROPIC_API_KEY: "sk-ant-live-secret-token",
      DATABASE_URL: "postgresql://postgres:secretpw@prod-db.internal:5432/arodb",
      INTERNAL_JWT_SECRET: "my-jwt-signing-secret",
      AWS_SECRET_ACCESS_KEY: "AKIAIOSFODNN7EXAMPLE",
      SAFE_RELEASE_VERSION: "1.2.0-rc1",
    };

    // Scrub environment
    const sanitizedEnv = scrubbedEnv(contaminatedEnv);
    expect(sanitizedEnv.PATH).toBe("/usr/bin:/bin");
    expect(sanitizedEnv.SAFE_RELEASE_VERSION).toBe("1.2.0-rc1");
    expect(sanitizedEnv.OPENAI_API_KEY).toBeUndefined();
    expect(sanitizedEnv.ANTHROPIC_API_KEY).toBeUndefined();
    expect(sanitizedEnv.DATABASE_URL).toBeUndefined();
    expect(sanitizedEnv.INTERNAL_JWT_SECRET).toBeUndefined();
    expect(sanitizedEnv.AWS_SECRET_ACCESS_KEY).toBeUndefined();

    // Execute sandboxed execution engine
    const sandbox = new SandboxedExecutionEngine(3000, 1024 * 64);
    const result = await sandbox.executeCommand("cargo", ["test", "--workspace"], {
      env: contaminatedEnv, // Passed through sandbox engine which scrubs it
    });

    expect(result.exitCode).toBe(0);
    expect(result.timedOut).toBe(false);
    expect(result.truncated).toBe(false);
    expect(result.stdout).toContain("cargo test --workspace");
  });

  // =========================================================================
  // Scenario 5: Complex Collaborative Task with Artifact Handover
  // Features: F1, F2, F3, F4, F10
  // =========================================================================
  it("Scenario 5: Complex Collaborative Task with Artifact Handover (F1, F2, F3, F4, F10)", async () => {
    const convId = "conv-webhook-system";

    // 1. Orchestrator delegates architecture to Architect Agent
    const archDelegation = createAgentEnvelope({
      conversationId: convId,
      sender: { id: "orch", name: "Orchestrator", type: "orchestrator" },
      recipient: { id: "agent-arch", name: "Architect Agent", role: "architect", type: "subagent" },
      messageType: "task_delegation",
      content: "Design robust Webhook notification delivery architecture with retry backoff",
    });
    runtime.handleDelegation(archDelegation);

    // 2. Architect Agent produces spec artifact
    const specArtifact: AgentArtifactRef = {
      id: "art-webhook-spec",
      title: "Webhook Architecture Specification",
      kind: "markdown",
      uri: "specs/webhooks.md",
    };
    memoryEngine.recordArtifact(convId, "agent-arch", specArtifact);
    memoryEngine.updateScratchpad(convId, "agent-arch", "Webhook spec completed with exponential backoff strategy.");

    // 3. Architect Agent hands over artifact to Implementation Agent via peer_collaboration
    const handoverMessage = createAgentEnvelope({
      conversationId: convId,
      sender: { id: "agent-arch", name: "Architect Agent", role: "architect", type: "subagent" },
      recipient: { id: "agent-impl", name: "Implementation Agent", role: "code", type: "subagent" },
      messageType: "peer_collaboration",
      content: "Architecture specification ready. Please implement crates/aro-webhooks.",
      artifacts: [specArtifact],
    });

    // 4. Implementation Agent receives handover and executes tasks
    const implRun = runtime.handleDelegation(handoverMessage);
    const implSteps = [
      { stepIndex: 1, thought: "Reading webhook specification from handover" },
      { stepIndex: 2, thought: "Implementing retry scheduler" },
      { stepIndex: 3, thought: "Writing unit tests" },
    ];

    const plannedImplRun = runtime.createRun({
      id: implRun.id,
      conversationId: convId,
      agentName: "Implementation Agent",
      role: "code",
      steps: implSteps,
    });

    const completedImplRun = await runtime.executeRunLoop(plannedImplRun.id);
    expect(completedImplRun.status).toBe("completed");
    expect(completedImplRun.stepCount).toBe(3);

    // 5. Implementation Agent delivers final task_result to Orchestrator
    const finalResult = createAgentEnvelope({
      conversationId: convId,
      sender: { id: "agent-impl", name: "Implementation Agent", type: "subagent" },
      recipient: { id: "orch", name: "Orchestrator", type: "orchestrator" },
      messageType: "task_result",
      content: "Webhook engine implemented with 100% test coverage.",
      artifacts: [
        specArtifact,
        { id: "art-impl-code", title: "crates/aro-webhooks", kind: "rust", uri: "crates/aro-webhooks" },
      ],
    });
    memoryEngine.dispatchMessage(finalResult);

    // 6. Verify total conversation ledger and memory coherence
    const ledger = memoryEngine.getLedger(convId);
    expect(ledger).toHaveLength(3); // delegation, peer_collaboration, task_result
    expect(ledger[1].payload.artifacts).toHaveLength(1);
    expect(ledger[2].payload.artifacts).toHaveLength(2);

    const implMemory = memoryEngine.getContext(convId, "agent-impl");
    expect(implMemory.ledger).toHaveLength(2); // received peer message, dispatched task_result
  });

  // =========================================================================
  // Scenario 6: Full Permission Escalation Prevention
  // Features: F5, F6, F7, F8
  // =========================================================================
  it("Scenario 6: Full Permission Escalation Prevention (F5, F6, F7, F8)", async () => {
    const workspaceRoot = "C:/Projects/AroSecure";

    // Setup an adversarial sub-agent assigned to Read-Only preset
    const adversarialGuard = new ToolAuthorizationGuard({
      preset: "read-only",
      allowedTools: registry.listTools().map((t) => t.name),
    });

    // Vector 1: Attempt to execute shell command (privilege escalation)
    expect(() => {
      adversarialGuard.checkPermission("core.shell.execute");
    }).toThrowError(/Command execution is forbidden in Read-Only mode/);

    // Vector 2: Attempt path traversal to escape workspace and access system credentials
    expect(() => {
      resolveWorkspacePath(workspaceRoot, "../../../Windows/System32/config/SAM");
    }).toThrowError(/escapes workspace root/);

    // Vector 3: Attempt null byte injection to bypass file extension checks
    expect(() => {
      resolveWorkspacePath(workspaceRoot, "safe_doc.pdf\0/../../secret.env");
    }).toThrowError(/Null byte detected/);

    // Vector 4: Attempt to invoke unregistered shadow backdoor tool
    expect(() => {
      adversarialGuard.checkPermission("exploit.kernel_escalate");
    }).toThrowError(/Tool not found in explicit allowed_tools whitelist/);

    // Vector 5: Attempt file deletion or tampering while restricted
    expect(() => {
      adversarialGuard.checkPermission("workspace.delete");
    }).toThrowError(/File write and mutation operations are forbidden in Read-Only mode/);

    // Vector 6: Attempt environment secret exfiltration
    const secretEnv = {
      SYSTEM_ROOT_PASSWORD_HASH: "$6$rounds=5000$saltsalt$hashedpassword",
      PATH: "C:\\Windows",
    };
    const cleaned = scrubbedEnv(secretEnv);
    expect(cleaned.SYSTEM_ROOT_PASSWORD_HASH).toBeUndefined();
    expect(cleaned.PATH).toBe("C:\\Windows");
  });
});
