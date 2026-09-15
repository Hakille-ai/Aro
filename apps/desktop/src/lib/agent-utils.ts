export interface LogEntry {
  id: string;
  timestamp: string;
  level: "info" | "tool" | "success" | "warning" | "error";
  category: string;
  message: string;
  details?: string;
}

/**
 * Synthesize agent inbox lanes avoiding empty screens by creating a virtual default lane
 * for runs that have no assigned lane or whose lane is not found.
 */
export function synthesizeAgentLanes(lanes: any[], runs: any[], activeConversationId: string | null): any[] {
  if (!activeConversationId) return [];
  const convRuns = runs.filter((r) => r.conversationId === activeConversationId);
  const matchedLanes = lanes
    .filter((l) => l.lane?.conversationId === activeConversationId)
    .map((l) => ({
      ...l,
      visibleRuns: convRuns.filter((r) => r.laneId === l.lane?.id),
    }));

  const unassignedRuns = convRuns.filter((r) => !r.laneId || !lanes.some((l) => l.lane?.id === r.laneId));
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
    });
  }
  return matchedLanes;
}

/**
 * Calculate live activity counters for the active conversation, with fallback to
 * orchestrator snapshot when conversation runs are empty.
 */
export function calculateActivityCounters(
  runs: any[],
  snapshot: any | null,
  activeConversationId: string | null
): { running: number; queued: number; waiting: number; done: number; failed: number; total: number } {
  const convRuns = activeConversationId ? runs.filter((r) => r.conversationId === activeConversationId) : runs;
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

/**
 * Map an agent step object into a standardized LogEntry for live terminal viewing.
 */
export function agentStepToLogEntry(step: any): LogEntry {
  let level: LogEntry["level"] = "info";
  if (step.status === "failed" || step.error) level = "error";
  else if (step.status === "succeeded") level = "success";
  else if (step.kind === "tool") level = "tool";

  let details: string | undefined = undefined;
  if (step.error) details = `Error: ${step.error}`;
  else if (step.output) details = typeof step.output === "string" ? step.output : JSON.stringify(step.output, null, 2);
  else if (step.input) details = typeof step.input === "string" ? step.input : JSON.stringify(step.input, null, 2);

  return {
    id: step.id || `${step.runId || "run"}-${step.sequence || 0}`,
    timestamp: step.startedAt ? new Date(step.startedAt).toLocaleTimeString() : new Date().toLocaleTimeString(),
    level,
    category: step.kind || "agent",
    message: step.title || step.kind || "Agent step execution",
    details,
  };
}
