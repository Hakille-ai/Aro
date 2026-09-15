import { describe, it, expect } from "vitest";
import {
  VALID_TASK_STATUSES,
  validateTaskStep,
  calculatePlanProgress,
  cycleTaskStatus,
  formatTaskError,
} from "./plan-utils";
import type { TaskStep, TaskStepStatus } from "./api/transport";

describe("plan-utils", () => {
  describe("VALID_TASK_STATUSES", () => {
    it("exports the 4 canonical task statuses", () => {
      expect(VALID_TASK_STATUSES).toEqual(["pending", "in_progress", "completed", "error"]);
    });
  });

  describe("validateTaskStep", () => {
    it("normalizes a valid step and preserves existing id", () => {
      const { valid, normalized } = validateTaskStep({
        id: "step-1",
        text: "Run test suite",
        status: "in_progress",
      });
      expect(valid).toBe(true);
      expect(normalized.id).toBe("step-1");
      expect(normalized.text).toBe("Run test suite");
      expect(normalized.status).toBe("in_progress");
      expect(normalized.completed).toBe(false);
      expect(normalized.error).toBeNull();
    });

    it("generates a random UUID if id is missing or whitespace", () => {
      const res1 = validateTaskStep({ text: "Task A" });
      expect(res1.normalized.id).toBeDefined();
      expect(res1.normalized.id.length).toBeGreaterThan(10);

      const res2 = validateTaskStep({ id: "   ", text: "Task B" });
      expect(res2.normalized.id).toBeDefined();
      expect(res2.normalized.id.trim().length).toBeGreaterThan(0);
    });

    it("infers status from completed flag when status is absent", () => {
      const resCompleted = validateTaskStep({ id: "1", text: "Done", completed: true });
      expect(resCompleted.normalized.status).toBe("completed");
      expect(resCompleted.normalized.completed).toBe(true);

      const resPending = validateTaskStep({ id: "2", text: "Todo", completed: false });
      expect(resPending.normalized.status).toBe("pending");
      expect(resPending.normalized.completed).toBe(false);
    });

    it("falls back to completed flag if provided status is unknown", () => {
      const res = validateTaskStep({
        id: "3",
        text: "Fallback",
        status: "unknown_status" as any,
        completed: true,
      });
      expect(res.normalized.status).toBe("completed");
      expect(res.normalized.completed).toBe(true);
    });

    it("preserves task error string", () => {
      const res = validateTaskStep({
        id: "4",
        text: "Failing step",
        status: "error",
        error: "Fatal: Out of memory",
      });
      expect(res.normalized.status).toBe("error");
      expect(res.normalized.error).toBe("Fatal: Out of memory");
    });

    it("returns valid: false when text is undefined", () => {
      const res = validateTaskStep({});
      expect(res.valid).toBe(false);
      expect(res.normalized.text).toBe("");
    });
  });

  describe("calculatePlanProgress", () => {
    it("returns 0/0/0 for null, undefined, or empty tasks", () => {
      expect(calculatePlanProgress(null)).toEqual({ total: 0, completed: 0, percentage: 0 });
      expect(calculatePlanProgress(undefined)).toEqual({ total: 0, completed: 0, percentage: 0 });
      expect(calculatePlanProgress([])).toEqual({ total: 0, completed: 0, percentage: 0 });
    });

    it("calculates percentage correctly across mixed completion flags", () => {
      const tasks: TaskStep[] = [
        { id: "1", text: "A", completed: true, status: "completed" },
        { id: "2", text: "B", completed: false, status: "completed" }, // status completed overrides completed: false
        { id: "3", text: "C", completed: false, status: "in_progress" },
        { id: "4", text: "D", completed: false, status: "pending" },
      ];
      const progress = calculatePlanProgress(tasks);
      expect(progress.total).toBe(4);
      expect(progress.completed).toBe(2);
      expect(progress.percentage).toBe(50);
    });

    it("rounds percentage to nearest integer", () => {
      const tasks: TaskStep[] = [
        { id: "1", text: "A", completed: true, status: "completed" },
        { id: "2", text: "B", completed: false, status: "pending" },
        { id: "3", text: "C", completed: false, status: "pending" },
      ];
      // 1 / 3 = 33.333% -> 33%
      expect(calculatePlanProgress(tasks)).toEqual({ total: 3, completed: 1, percentage: 33 });
    });

    it("handles 100% completion", () => {
      const tasks: TaskStep[] = [
        { id: "1", text: "A", completed: true, status: "completed" },
        { id: "2", text: "B", completed: true, status: "completed" },
      ];
      expect(calculatePlanProgress(tasks)).toEqual({ total: 2, completed: 2, percentage: 100 });
    });
  });

  describe("cycleTaskStatus", () => {
    it("cycles pending -> in_progress -> completed -> error -> pending", () => {
      let status: TaskStepStatus = "pending";
      status = cycleTaskStatus(status);
      expect(status).toBe("in_progress");

      status = cycleTaskStatus(status);
      expect(status).toBe("completed");

      status = cycleTaskStatus(status);
      expect(status).toBe("error");

      status = cycleTaskStatus(status);
      expect(status).toBe("pending");
    });

    it("defaults to pending when status is null, undefined, or unrecognized", () => {
      expect(cycleTaskStatus(null)).toBe("pending");
      expect(cycleTaskStatus(undefined)).toBe("pending");
      expect(cycleTaskStatus("invalid" as any)).toBe("pending");
    });
  });

  describe("formatTaskError", () => {
    it("returns null for null, undefined, or empty errors", () => {
      expect(formatTaskError(null)).toBeNull();
      expect(formatTaskError(undefined)).toBeNull();
      expect(formatTaskError("")).toBeNull();
    });

    it("returns full text if within max length", () => {
      expect(formatTaskError("Error code 404", 50)).toBe("Error code 404");
    });

    it("truncates long errors and appends ellipsis", () => {
      const longMsg = "A".repeat(250);
      const formatted = formatTaskError(longMsg, 200);
      expect(formatted).toHaveLength(203); // 200 chars + "..."
      expect(formatted?.endsWith("...")).toBe(true);
    });
  });
});
