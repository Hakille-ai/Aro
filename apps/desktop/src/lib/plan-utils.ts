import type { TaskStep, TaskStepStatus } from "./api/transport";

export const VALID_TASK_STATUSES: TaskStepStatus[] = [
  "pending",
  "in_progress",
  "completed",
  "error"
];

/**
 * Validate and normalize a partial task step with sensible defaults.
 */
export function validateTaskStep(task: Partial<TaskStep>): { valid: boolean; normalized: TaskStep } {
  const status: TaskStepStatus = task.status && VALID_TASK_STATUSES.includes(task.status)
    ? task.status
    : (task.completed ? "completed" : "pending");

  const normalized: TaskStep = {
    id: task.id?.trim() || crypto.randomUUID(),
    text: task.text ?? "",
    completed: status === "completed",
    status,
    error: task.error ?? null,
  };

  return { valid: Boolean(task.text !== undefined), normalized };
}

/**
 * Calculate plan progress totals and percentage.
 */
export function calculatePlanProgress(tasks?: TaskStep[] | null): { total: number; completed: number; percentage: number } {
  if (!tasks || tasks.length === 0) return { total: 0, completed: 0, percentage: 0 };
  const completed = tasks.filter((t) => t.completed || t.status === "completed").length;
  const percentage = Math.round((completed / tasks.length) * 100);
  return { total: tasks.length, completed, percentage };
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
