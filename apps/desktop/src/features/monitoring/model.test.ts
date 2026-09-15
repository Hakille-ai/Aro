import { describe, expect, it } from "vitest";
import {
  PERFORMANCE_HISTORY_LIMIT,
  activityLevel,
  buildActivityWeeks,
  createInitialMonitoringState,
  createPerformanceRecord,
  incrementActivityLog,
  populateMockActivity,
  prependPerformanceRecord,
  summarizePerformance,
  type PerformanceRecord,
} from "./model";

describe("monitoring model", () => {
  it("preserves the monitoring defaults and independent history arrays", () => {
    const first = createInitialMonitoringState();
    const second = createInitialMonitoringState();
    expect(first).toMatchObject({ cpu: 4, ram: 4.2, ramMax: 16, gpu: 2, gpuVram: 2.4, gpuVramMax: 8 });
    expect(first.cpuHistory).toEqual(Array(20).fill(4));
    expect(first.ramHistory).toEqual(Array(20).fill(4.2));
    expect(first.gpuHistory).toEqual(Array(20).fill(2));
    expect(first.cpuHistory).not.toBe(second.cpuHistory);
  });

  it.each([
    [-1, 0], [0, 0], [1, 1], [2, 1], [3, 2], [4, 2], [5, 3], [7, 3], [8, 4],
  ])("maps activity count %s to level %s", (count, level) => {
    expect(activityLevel(count)).toBe(level);
  });

  it("increments activity immutably", () => {
    const original = { "2026-07-15": 2 };
    expect(incrementActivityLog(original, "2026-07-15")).toEqual({ "2026-07-15": 3 });
    expect(incrementActivityLog(original, "2026-07-16")).toEqual({ "2026-07-15": 2, "2026-07-16": 1 });
    expect(original).toEqual({ "2026-07-15": 2 });
  });

  it("builds exactly 53 ordered Sunday-to-Saturday weeks", () => {
    const now = new Date("2026-07-15T12:00:00.000Z");
    const weeks = buildActivityWeeks({ "2026-07-15": 8 }, "en", now);
    expect(weeks).toHaveLength(53);
    expect(weeks.every((week) => week.length === 7)).toBe(true);
    expect(weeks[0][0].dayName.toLowerCase()).toContain("sun");
    expect(weeks.flat().find((day) => day.dateStr === "2026-07-15")).toMatchObject({ count: 8, level: 4 });
  });

  it("keeps the weekday/weekend mock probabilities and 1..8 count range", () => {
    const randomValues = [0.1, 0, ...Array(800).fill(0.99)];
    const activity = populateMockActivity(new Date("2026-07-15T12:00:00.000Z"), () => randomValues.shift() ?? 0.99);
    expect(activity["2026-07-15"]).toBe(1);
    expect(Object.values(activity).every((count) => count >= 1 && count <= 8)).toBe(true);
  });

  it("creates localized, rounded performance records", () => {
    const record = createPerformanceRecord("", 1.234, 10, "en", new Date("2026-07-15T12:34:56Z"), "fixed");
    expect(record).toMatchObject({ id: "fixed", conversationTitle: "Untitled chat", responseTime: 1.23, tokens: 10, speed: 8.1 });
  });

  it("caps newest-first history at 50 and summarizes it", () => {
    const history: PerformanceRecord[] = Array.from({ length: PERFORMANCE_HISTORY_LIMIT }, (_, index) => ({
      id: String(index), timestamp: "", conversationTitle: "", responseTime: 2, tokens: 3, speed: 1.5,
    }));
    const record = { ...history[0], id: "new", responseTime: 1, tokens: 5 };
    const next = prependPerformanceRecord(history, record);
    expect(next).toHaveLength(50);
    expect(next[0].id).toBe("new");
    expect(next.at(-1)?.id).toBe("48");
    expect(summarizePerformance(next)).toEqual({ averageResponseTime: 1.98, totalTokensGenerated: 152 });
    expect(summarizePerformance([])).toEqual({ averageResponseTime: 0, totalTokensGenerated: 0 });
  });
});
