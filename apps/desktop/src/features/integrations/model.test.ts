import { describe, expect, it } from "vitest";
import {
  DEFAULT_HOOK_FORM,
  DEFAULT_MCP_FORM,
  DEFAULT_SCHEDULER_FORM,
  HOOK_EVENT_CATALOG,
  HOOK_PRESETS,
  MCP_SERVER_PRESETS,
  SCHEDULER_PRESETS,
} from "./model";

function expectUniqueIds(items: readonly { id: string }[]) {
  const ids = items.map((item) => item.id);
  expect(new Set(ids).size).toBe(ids.length);
  expect(ids.every(Boolean)).toBe(true);
}

describe("integration catalogs", () => {
  it("keeps stable, unique MCP preset identifiers and valid transports", () => {
    expectUniqueIds(MCP_SERVER_PRESETS);
    expect(MCP_SERVER_PRESETS.map((preset) => preset.id)).toEqual([
      "filesystem-local",
      "git-controller",
      "postgresql",
      "slack-remote",
    ]);
    for (const preset of MCP_SERVER_PRESETS) {
      expect(preset.type === "stdio" ? preset.command : preset.url).toBeTruthy();
    }
  });

  it("keeps hook presets within the supported event catalog", () => {
    expectUniqueIds(HOOK_EVENT_CATALOG);
    expectUniqueIds(HOOK_PRESETS);
    const supportedEvents = new Set(HOOK_EVENT_CATALOG.map((event) => event.id));
    expect(HOOK_EVENT_CATALOG.map((event) => event.id)).toEqual([
      "message.sent",
      "task.completed",
      "conversation.created",
    ]);
    for (const preset of HOOK_PRESETS) {
      expect(preset.events.length).toBeGreaterThan(0);
      expect(preset.events.every((event) => supportedEvents.has(event))).toBe(true);
    }
  });

  it("keeps scheduler presets complete for their trigger type", () => {
    expectUniqueIds(SCHEDULER_PRESETS);
    expect(SCHEDULER_PRESETS.map((preset) => preset.id)).toEqual([
      "coffee-break",
      "weekday-standup",
      "friday-review",
    ]);
    for (const preset of SCHEDULER_PRESETS) {
      expect(preset.type === "timer" ? preset.durationMinutes : preset.cronExpression).toBeTruthy();
    }
  });

  it("preserves the form defaults used by App", () => {
    expect(DEFAULT_MCP_FORM).toMatchObject({ type: "stdio", env: [] });
    expect(DEFAULT_HOOK_FORM.events).toEqual(["message.sent"]);
    expect(DEFAULT_SCHEDULER_FORM).toMatchObject({
      type: "timer",
      duration: 15,
      durationMinutes: 15,
      cron: "*/30 * * * *",
      cronExpression: "*/30 * * * *",
    });
  });
});
