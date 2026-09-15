import { describe, expect, it } from "vitest";

describe("API compatibility facade", () => {
  it("re-exports the exact domain function instances", async () => {
    const facade = await import("./api");
    const domains = await Promise.all([
      import("./api/bootstrap"),
      import("./api/auth-organizations"),
      import("./api/conversations-memory-files"),
      import("./api/agents-permissions-arena"),
      import("./api/settings-models-runtime"),
      import("./api/voice"),
      import("./api/integrations"),
      import("./api/plugins"),
    ]);

    const domainExports = Object.assign({}, ...domains);
    expect(Object.keys(facade).sort()).toEqual(Object.keys(domainExports).sort());

    for (const [name, exportedValue] of Object.entries(domainExports)) {
      expect(facade[name as keyof typeof facade]).toBe(exportedValue);
    }
  });
});
