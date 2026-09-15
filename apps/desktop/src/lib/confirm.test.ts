import { describe, expect, it } from "vitest";
import { get } from "svelte/store";
import { confirmRequest, requestConfirm, resolveConfirm } from "./confirm";

describe("requestConfirm", () => {
  it("opens with defaults and resolves true", async () => {
    const pending = requestConfirm({ title: "Supprimer ?" });
    expect(get(confirmRequest)?.title).toBe("Supprimer ?");
    expect(get(confirmRequest)?.confirmLabel).toBe("Confirmer");
    expect(get(confirmRequest)?.danger).toBe(true);
    resolveConfirm(true);
    await expect(pending).resolves.toBe(true);
    expect(get(confirmRequest)).toBeNull();
  });

  it("resolves false on dismiss", async () => {
    const pending = requestConfirm({ title: "Sûr ?", danger: false });
    expect(get(confirmRequest)?.danger).toBe(false);
    resolveConfirm(false);
    await expect(pending).resolves.toBe(false);
  });

  it("rejects the previous request when a new one supersedes it", async () => {
    const first = requestConfirm({ title: "Première" });
    const second = requestConfirm({ title: "Seconde" });
    await expect(first).resolves.toBe(false);
    expect(get(confirmRequest)?.title).toBe("Seconde");
    resolveConfirm(true);
    await expect(second).resolves.toBe(true);
  });
});
