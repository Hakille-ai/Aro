import {
  type AgentRun,
  type AgentRunStatus,
  type AgentRunPriority,
  type AgentMessageEnvelope,
  type SubAgentInfo,
  createAgentEnvelope,
} from "@aro/contracts";
import { ToolAuthorizationGuard } from "./security-guard-engine";
import { CognitiveMemoryEngine } from "./cognitive-memory-engine";

export interface ExecutionStepPlan {
  stepIndex: number;
  thought: string;
  toolCall?: {
    toolName: string;
    args: Record<string, unknown>;
  };
  shouldFail?: boolean;
  errorMessage?: string;
}

export class SubAgentRuntimeEngine {
  private activeRuns = new Map<string, AgentRun>();
  private runSteps = new Map<string, ExecutionStepPlan[]>();
  private memoryEngine: CognitiveMemoryEngine;
  private defaultGuard: ToolAuthorizationGuard;

  constructor(memoryEngine?: CognitiveMemoryEngine, guard?: ToolAuthorizationGuard) {
    this.memoryEngine = memoryEngine ?? new CognitiveMemoryEngine();
    this.defaultGuard = guard ?? new ToolAuthorizationGuard({ preset: "standard" });
  }

  public createRun(params: {
    id?: string;
    conversationId: string;
    laneId?: string;
    agentName: string;
    role?: string;
    goal?: string;
    priority?: AgentRunPriority;
    maxSteps?: number;
    steps?: ExecutionStepPlan[];
  }): AgentRun {
    const id = params.id || `run-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
    const run: AgentRun = {
      id,
      conversationId: params.conversationId,
      laneId: params.laneId || "default-virtual",
      agentName: params.agentName,
      role: params.role || "worker",
      goal: params.goal || "Autonomous mission",
      status: "queued",
      priority: params.priority || "normal",
      stepCount: 0,
      maxSteps: params.maxSteps || 10,
      currentThought: null,
      currentTool: null,
      error: null,
      createdAt: new Date().toISOString(),
      startedAt: null,
      completedAt: null,
    };

    this.activeRuns.set(id, run);
    if (params.steps) {
      this.runSteps.set(id, params.steps);
    }

    return run;
  }

  public getRun(runId: string): AgentRun | undefined {
    return this.activeRuns.get(runId);
  }

  public listRuns(conversationId?: string): AgentRun[] {
    const all = Array.from(this.activeRuns.values());
    if (!conversationId) return all;
    return all.filter((r) => r.conversationId === conversationId);
  }

  public cancelRun(runId: string): AgentRun {
    const run = this.activeRuns.get(runId);
    if (!run) throw new Error(`Run ${runId} not found`);
    run.status = "cancelled";
    run.completedAt = new Date().toISOString();
    return run;
  }

  /**
   * Simulates the async tokio::spawn execution loop.
   * Runs model turns and tool executions without freezing at step 2.
   */
  public async executeRunLoop(
    runId: string,
    guard?: ToolAuthorizationGuard
  ): Promise<AgentRun> {
    const run = this.activeRuns.get(runId);
    if (!run) throw new Error(`Run ${runId} not found`);

    if (run.status === "cancelled") {
      return run;
    }

    const effectiveGuard = guard || this.defaultGuard;
    run.status = "running";
    run.startedAt = new Date().toISOString();

    const plannedSteps = this.runSteps.get(runId) || [];
    const maxSteps = run.maxSteps || 10;

    // If no planned steps provided, generate synthetic multi-step progress
    const stepsToRun: ExecutionStepPlan[] = plannedSteps.length > 0
      ? plannedSteps
      : [
          { stepIndex: 1, thought: "Analyzing user objective", toolCall: { toolName: "workspace.list_dir", args: {} } },
          { stepIndex: 2, thought: "Reading workspace manifest", toolCall: { toolName: "workspace.read", args: { path: "package.json" } } },
          { stepIndex: 3, thought: "Generating refactor patch", toolCall: { toolName: "workspace.write", args: { path: "src/index.ts", content: "// ok" } } },
          { stepIndex: 4, thought: "Validating git diff", toolCall: { toolName: "workspace.git_diff", args: {} } },
          { stepIndex: 5, thought: "Completing autonomous mission" },
        ];

    for (const step of stepsToRun) {
      // Check if cancelled
      if (run.status === "cancelled") {
        break;
      }

      // Check step count boundary
      if (run.stepCount >= maxSteps) {
        run.status = "completed";
        run.completedAt = new Date().toISOString();
        return run;
      }

      run.stepCount++;
      run.currentThought = step.thought;

      // Update scratchpad in cognitive memory
      this.memoryEngine.updateScratchpad(
        run.conversationId,
        run.id,
        `Step ${run.stepCount}: ${step.thought}`
      );

      // Execute tool call if present
      if (step.toolCall) {
        run.currentTool = step.toolCall.toolName;

        // Check permission guard
        try {
          effectiveGuard.checkPermission(step.toolCall.toolName);
        } catch (guardErr: any) {
          run.status = "failed";
          run.error = guardErr.message;
          run.completedAt = new Date().toISOString();

          // Escalate error envelope
          this.memoryEngine.dispatchMessage(
            createAgentEnvelope({
              conversationId: run.conversationId,
              sender: { id: run.id, name: run.agentName, role: run.role, type: "subagent" },
              recipient: { id: "orchestrator", name: "Aro Orchestrator", type: "orchestrator" },
              messageType: "error_escalation",
              content: `Security Guard Interception on step ${run.stepCount}: ${guardErr.message}`,
            })
          );
          return run;
        }

        // Check planned failure
        if (step.shouldFail) {
          run.status = "failed";
          run.error = step.errorMessage || `Tool ${step.toolCall.toolName} failed during execution`;
          run.completedAt = new Date().toISOString();

          this.memoryEngine.dispatchMessage(
            createAgentEnvelope({
              conversationId: run.conversationId,
              sender: { id: run.id, name: run.agentName, role: run.role, type: "subagent" },
              recipient: { id: "orchestrator", name: "Aro Orchestrator", type: "orchestrator" },
              messageType: "error_escalation",
              content: run.error,
            })
          );
          return run;
        }

        // Record finding for successful tool output
        this.memoryEngine.recordFinding(
          run.conversationId,
          run.id,
          `Completed step ${run.stepCount}: ${step.thought}`,
          "discovery",
          step.toolCall.toolName
        );
      }

      // Small async tick simulation
      await new Promise((resolve) => setTimeout(resolve, 1));
    }

    if (run.status !== "failed" && run.status !== "cancelled") {
      run.status = "completed";
      run.currentThought = "Task completed successfully";
      run.currentTool = null;
      run.completedAt = new Date().toISOString();
    }

    return run;
  }

  /**
   * Handle delegation envelope from orchestrator or peer.
   */
  public handleDelegation(envelope: AgentMessageEnvelope): AgentRun {
    if (envelope.messageType !== "task_delegation" && envelope.messageType !== "peer_collaboration") {
      throw new Error(`Unsupported delegation message type: ${envelope.messageType}`);
    }

    if (!envelope.payload.content || envelope.payload.content.trim().length === 0) {
      throw new Error("Delegation directive content cannot be empty");
    }

    if (envelope.sender.id === envelope.recipient.id) {
      throw new Error("Self-delegation detected: sender and recipient cannot be identical");
    }

    // Save message to ledger
    this.memoryEngine.dispatchMessage(envelope);

    // Create and queue run
    const run = this.createRun({
      conversationId: envelope.conversationId,
      agentName: envelope.recipient.name,
      role: envelope.recipient.role || "worker",
      goal: envelope.payload.content,
      priority: envelope.priority || "normal",
    });

    return run;
  }
}
