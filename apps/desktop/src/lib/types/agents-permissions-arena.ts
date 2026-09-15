import type { AssistantMode } from "./conversations-memory-files";
import type { ModelRef } from "./settings-models-runtime";

export type AgentRunStatus =
  | "queued"
  | "running"
  | "waiting"
  | "paused"
  | "completed"
  | "failed"
  | "cancelled";

export type AgentLaneStatus = "active" | "paused";

export type AgentRunPriority = "low" | "normal" | "high" | "critical";

export type AgentStepKind =
  | "run-started"
  | "context-built"
  | "model"
  | "tool"
  | "checkpoint"
  | "final"
  | "error";

export type AgentStepStatus = "running" | "completed" | "failed" | "skipped";

export type ToolSource = "built-in" | "skill" | "mcp" | "plugin";

export type PermissionCommandApproval = "always" | "safe-auto" | "never";

export type PermissionPresetMode = "standard" | "read-only" | "developer" | "sandbox" | "custom";

export interface PermissionProfile {
  id: string;
  name: string;
  trustedRoots: string[];
  allowedDomains: string[];
  allowRead: boolean;
  allowWrite: boolean;
  allowShell: boolean;
  allowNetwork: boolean;
  commandApproval: PermissionCommandApproval;
  redactSecrets: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface ToolRef {
  id: string;
  name: string;
  description: string;
  source: ToolSource;
  enabled: boolean;
  dangerous: boolean;
  inputSchema: unknown;
}

export interface ContextSource {
  id: string;
  kind: string;
  title: string;
  excerpt: string;
  uri?: string | null;
  score: number;
  createdAt?: string | null;
}

export interface ContextPack {
  id: string;
  runId: string;
  goal: string;
  summary: string;
  sources: ContextSource[];
  tools: ToolRef[];
  tokenEstimate: number;
  builtAt: string;
}

export interface AgentRun {
  id: string;
  laneId?: string | null;
  conversationId?: string | null;
  goal: string;
  mode: AssistantMode;
  status: AgentRunStatus;
  priority: AgentRunPriority;
  modelProviderId?: string | null;
  modelId?: string | null;
  autonomyProfileId?: string | null;
  checkpointSummary?: string | null;
  lastError?: string | null;
  createdAt: string;
  updatedAt: string;
  heartbeatAt?: string | null;
  completedAt?: string | null;
}

export interface AgentLane {
  id: string;
  conversationId?: string | null;
  title: string;
  status: AgentLaneStatus;
  priority: AgentRunPriority;
  maxConcurrentRuns: number;
  createdAt: string;
  updatedAt: string;
}

export interface AgentLaneView {
  lane: AgentLane;
  queuedCount: number;
  runningCount: number;
  waitingCount: number;
  latestRuns: AgentRun[];
}

export interface AgentOrchestratorSnapshot {
  maxGlobalRunning: number;
  runningCount: number;
  queuedCount: number;
  lanes: AgentLaneView[];
}

export interface AgentStep {
  id: string;
  runId: string;
  sequence: number;
  kind: AgentStepKind;
  status: AgentStepStatus;
  title: string;
  input: any;
  output: any;
  error?: string | null;
  startedAt: string;
  finishedAt?: string | null;
}

export interface AgentArtifact {
  id: string;
  runId: string;
  kind: string;
  title: string;
  uri?: string | null;
  content?: string | null;
  metadata: unknown;
  createdAt: string;
}

export interface AgentContextItem {
  id: string;
  runId?: string | null;
  conversationId?: string | null;
  kind: string;
  title: string;
  content: string;
  uri?: string | null;
  metadata: unknown;
  createdAt: string;
}

export interface AgentRunStartRequest {
  runId?: string | null;
  laneId?: string | null;
  conversationId?: string | null;
  goal: string;
  mode: AssistantMode;
  systemPrompt?: string | null;
  modelId?: string | null;
  provider?: string | null;
  autonomyProfileId?: string | null;
  priority?: AgentRunPriority | null;
  maxSteps?: number | null;
}

export interface AgentRunView {
  run: AgentRun;
  steps: AgentStep[];
  artifacts: AgentArtifact[];
  contextPack?: ContextPack | null;
}

export interface ArenaRequest {
  content: string;
  mode: AssistantMode;
  systemPrompt?: string | null;
  modelA: ModelRef;
  modelB: ModelRef;
  tempMessageIdA: string;
  tempMessageIdB: string;
}

export interface ArenaStreamChunk {
  slot: 'a' | 'b';
  messageId: string;
  content: string;
  done: boolean;
  modelId: string;
}

export interface ArenaSlotState {
  modelId: string;
  content: string;
  done: boolean;
  ttft: number | null;     // Time to first token (ms)
  tokensPerSec: number;    // Estimated tokens/s
  chunkCount: number;
  startedAt: number;       // performance.now() at first chunk
  firstChunkAt: number | null;
  vote: 'winner' | 'loser' | 'tie' | null;
  messageId?: string | null; // Temp id for stream routing
  error?: string | null;     // Failure text when generation failed
}
