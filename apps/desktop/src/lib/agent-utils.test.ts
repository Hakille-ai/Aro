import { describe, it, expect } from "vitest";
import {
  synthesizeAgentLanes,
  calculateActivityCounters,
  agentStepToLogEntry,
} from "./agent-utils";

describe("agent-utils", () => {
  describe("synthesizeAgentLanes", () => {
    it("returns empty array when activeConversationId is null", () => {
      expect(synthesizeAgentLanes([{ lane: { id: "l1", conversationId: "c1" } }], [], null)).toEqual([]);
    });

    it("synthesizes a default-virtual lane when unassigned runs exist", () => {
      const lanes = [
        {
          lane: { id: "lane-main", conversationId: "c1", title: "Main Work" },
          collapsed: false,
        },
      ];
      const runs = [
        { id: "r1", laneId: "lane-main", conversationId: "c1" },
        { id: "r2", laneId: "", conversationId: "c1" }, // unassigned
        { id: "r3", laneId: "non-existent-lane", conversationId: "c1" }, // unassigned
      ];

      const synthesized = synthesizeAgentLanes(lanes, runs, "c1");
      expect(synthesized).toHaveLength(2);
      expect(synthesized[0].lane.id).toBe("default-virtual");
      expect(synthesized[0].lane.title).toBe("Voie principale / Tâches actives");
      expect(synthesized[0].visibleRuns).toHaveLength(2);
      expect(synthesized[0].visibleRuns.map((r: any) => r.id)).toEqual(["r2", "r3"]);

      expect(synthesized[1].lane.id).toBe("lane-main");
      expect(synthesized[1].visibleRuns).toHaveLength(1);
    });

    it("does not create default-virtual lane when all runs have existing lanes", () => {
      const lanes = [
        {
          lane: { id: "lane-1", conversationId: "c1", title: "Lane 1" },
          collapsed: false,
        },
      ];
      const runs = [
        { id: "r1", laneId: "lane-1", conversationId: "c1" },
      ];

      const synthesized = synthesizeAgentLanes(lanes, runs, "c1");
      expect(synthesized).toHaveLength(1);
      expect(synthesized[0].lane.id).toBe("lane-1");
    });
  });

  describe("calculateActivityCounters", () => {
    it("calculates accurate counters for conversation runs", () => {
      const runs = [
        { id: "1", conversationId: "c1", status: "running" },
        { id: "2", conversationId: "c1", status: "queued" },
        { id: "3", conversationId: "c1", status: "waiting" },
        { id: "4", conversationId: "c1", status: "paused" },
        { id: "5", conversationId: "c1", status: "completed" },
        { id: "6", conversationId: "c1", status: "failed" },
        { id: "7", conversationId: "c1", status: "cancelled" },
        { id: "8", conversationId: "c2", status: "running" }, // different conversation
      ];

      const counters = calculateActivityCounters(runs, null, "c1");
      expect(counters.running).toBe(1);
      expect(counters.queued).toBe(1);
      expect(counters.waiting).toBe(2); // waiting + paused
      expect(counters.done).toBe(1); // completed
      expect(counters.failed).toBe(2); // failed + cancelled
      expect(counters.total).toBe(7);
    });

    it("falls back to orchestrator snapshot when conversation runs are empty", () => {
      const snapshot = {
        runningCount: 3,
        queuedCount: 5,
      };

      const counters = calculateActivityCounters([], snapshot, "c1");
      expect(counters.running).toBe(3);
      expect(counters.queued).toBe(5);
      expect(counters.total).toBe(8);
      expect(counters.waiting).toBe(0);
      expect(counters.done).toBe(0);
    });

    it("clamps negative values to 0", () => {
      const snapshot = {
        runningCount: -2,
        queuedCount: -5,
      };

      const counters = calculateActivityCounters([], snapshot, "c1");
      expect(counters.running).toBe(0);
      expect(counters.queued).toBe(0);
      expect(counters.total).toBe(0);
    });
  });

  describe("agentStepToLogEntry", () => {
    it("maps a tool step to tool level and extracts input", () => {
      const step = {
        id: "step-10",
        runId: "run-1",
        sequence: 1,
        kind: "tool",
        status: "in_progress",
        title: "Executing tool: search_files",
        input: { query: "App.svelte" },
        startedAt: "2026-09-05T01:00:00Z",
      };

      const log = agentStepToLogEntry(step);
      expect(log.id).toBe("step-10");
      expect(log.level).toBe("tool");
      expect(log.category).toBe("tool");
      expect(log.message).toBe("Executing tool: search_files");
      expect(log.details).toContain("App.svelte");
    });

    it("maps error and failed status to error level", () => {
      const step = {
        runId: "run-2",
        sequence: 2,
        status: "failed",
        error: "Disk write permission denied",
      };

      const log = agentStepToLogEntry(step);
      expect(log.id).toBe("run-2-2");
      expect(log.level).toBe("error");
      expect(log.details).toBe("Error: Disk write permission denied");
    });

    it("maps succeeded step to success level and formats output", () => {
      const step = {
        id: "step-3",
        status: "succeeded",
        output: "All 5 files indexed successfully.",
      };

      const log = agentStepToLogEntry(step);
      expect(log.level).toBe("success");
      expect(log.details).toBe("All 5 files indexed successfully.");
    });
  });
});
