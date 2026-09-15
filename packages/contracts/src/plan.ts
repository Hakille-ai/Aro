export type TaskStepStatus = "pending" | "in_progress" | "completed" | "error";

export const VALID_TASK_STATUSES: TaskStepStatus[] = [
  "pending",
  "in_progress",
  "completed",
  "error",
];

export interface TaskStep {
  id: string;
  text: string;
  completed: boolean;
  status?: TaskStepStatus;
  error?: string | null;
  assignedAgent?: string | null;
  startedAt?: string | null;
  completedAt?: string | null;
}

export interface Plan {
  id: string;
  conversationId: string;
  title: string;
  description?: string;
  tasks: TaskStep[];
  status: "active" | "completed" | "archived";
  createdAt: string;
  updatedAt: string;
}

export interface PlanCreateRequest {
  conversationId: string;
  title: string;
  description?: string;
  tasks?: Array<{ text: string; status?: TaskStepStatus }>;
}

export interface PlanUpdateRequest {
  title?: string;
  description?: string;
  tasks?: TaskStep[];
  status?: "active" | "completed" | "archived";
}

export interface PlanProgress {
  total: number;
  completed: number;
  inProgress: number;
  pending: number;
  error: number;
  percentage: number;
}

/**
 * Validate and normalize a partial task step with sensible defaults.
 */
export function validateTaskStep(task: Partial<TaskStep>): { valid: boolean; normalized: TaskStep } {
  const status: TaskStepStatus = task.status && VALID_TASK_STATUSES.includes(task.status)
    ? task.status
    : task.completed
    ? "completed"
    : "pending";

  const normalized: TaskStep = {
    id: task.id?.trim() || `task-${Date.now()}-${Math.random().toString(36).substring(2, 9)}`,
    text: (task.text ?? "").trim(),
    completed: status === "completed",
    status,
    error: task.error ?? null,
    assignedAgent: task.assignedAgent ?? null,
    startedAt: task.startedAt ?? (status === "in_progress" ? new Date().toISOString() : null),
    completedAt: task.completedAt ?? (status === "completed" ? new Date().toISOString() : null),
  };

  return { valid: Boolean(normalized.text.length > 0), normalized };
}

/**
 * Calculate plan progress totals, breakdown, and completion percentage.
 */
export function calculatePlanProgress(tasks?: TaskStep[] | null): PlanProgress {
  if (!tasks || tasks.length === 0) {
    return { total: 0, completed: 0, inProgress: 0, pending: 0, error: 0, percentage: 0 };
  }

  const completed = tasks.filter((t) => t.completed || t.status === "completed").length;
  const inProgress = tasks.filter((t) => t.status === "in_progress").length;
  const error = tasks.filter((t) => t.status === "error").length;
  const pending = tasks.filter((t) => !t.status || t.status === "pending").length;
  const percentage = Math.round((completed / tasks.length) * 100);

  return {
    total: tasks.length,
    completed,
    inProgress,
    pending,
    error,
    percentage,
  };
}

/**
 * Cycle task status: pending -> in_progress -> completed -> error -> pending.
 */
export function cycleTaskStatus(current?: TaskStepStatus | null): TaskStepStatus {
  switch (current) {
    case "pending":
      return "in_progress";
    case "in_progress":
      return "completed";
    case "completed":
      return "error";
    case "error":
      return "pending";
    default:
      return "pending";
  }
}

/**
 * Format error message with optional max length truncation.
 */
export function formatTaskError(error: string | null | undefined, maxLen = 200): string | null {
  if (!error) return null;
  return error.length > maxLen ? error.slice(0, maxLen) + "..." : error;
}
