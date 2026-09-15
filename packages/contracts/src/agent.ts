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
