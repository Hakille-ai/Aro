export type AroEventName =
  | "aro:plans-updated"
  | "aro:agent-step-update"
  | "aro:agent-lane-update"
  | "aro:message-stream"
  | "aro:artifact-created"
  | "aro:connection-status";

export interface AroEventPayloads {
  "aro:plans-updated": { planId: string; conversationId: string };
  "aro:agent-step-update": { runId: string; step: number; status: string; thought?: string; tool?: string };
  "aro:agent-lane-update": { laneId: string; status: string };
  "aro:message-stream": { conversationId: string; messageId: string; delta: string };
  "aro:artifact-created": { artifactId: string; conversationId: string; title: string };
  "aro:connection-status": { connected: boolean; latencyMs?: number };
}
