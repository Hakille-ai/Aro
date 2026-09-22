import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

// Mock transport module before importing cloudSync
vi.mock("./transport", () => ({
  setCloudState: vi.fn().mockResolvedValue(undefined),
  deleteCloudState: vi.fn().mockResolvedValue(undefined),
}));

import { cloudSync, CloudSyncManager } from "./cloud-sync";
import { setCloudState, deleteCloudState } from "./transport";

function memoryLocalStorage() {
  const store = new Map<string, string>();
  return {
    getItem: (key: string) => (store.has(key) ? store.get(key)! : null),
    setItem: (key: string, value: string) => {
      store.set(key, String(value));
    },
    removeItem: (key: string) => {
      store.delete(key);
    },
    clear: () => store.clear(),
    _store: store,
  };
}

describe("CloudSyncManager", () => {
  beforeEach(() => {
    // mockReset (not just clear): drops leftover once-implementations so
    // queued rejections/resolutions never leak from one test into the next.
    vi.mocked(setCloudState).mockReset().mockResolvedValue(undefined);
    vi.mocked(deleteCloudState).mockReset().mockResolvedValue(undefined);
    cloudSync.reset();
  });

  afterEach(() => {
    cloudSync.reset();
    vi.useRealTimers();
  });

  it("filters out local-only and telemetry keys from sync", () => {
    expect(cloudSync.isLocalOnlyKey("aro-perf-history")).toBe(true);
    expect(cloudSync.isLocalOnlyKey("aro-user-activity")).toBe(true);
    expect(cloudSync.isLocalOnlyKey("networkAlertDismissed")).toBe(true);
    expect(cloudSync.isLocalOnlyKey("aro-api-keys")).toBe(true); // contains "key"
    expect(cloudSync.isLocalOnlyKey("some-other-key")).toBe(true); // doesn't start with aro-
    expect(cloudSync.isLocalOnlyKey("aro-theme")).toBe(false);
    expect(cloudSync.isLocalOnlyKey("aro-language")).toBe(false);
    expect(cloudSync.isLocalOnlyKey("aro-browser-tabs")).toBe(false);
  });

  it("skips syncing when the value has not changed (dirty-checking)", async () => {
    cloudSync.seedSyncedState("aro-theme", "dark");
    cloudSync.enqueueSet("aro-theme", "dark");
    await cloudSync.flush();

    expect(setCloudState).not.toHaveBeenCalled();
  });

  it("debounces rapid consecutive writes into a single update", async () => {
    cloudSync.enqueueSet("aro-language", "en");
    cloudSync.enqueueSet("aro-language", "es");
    cloudSync.enqueueSet("aro-language", "fr");

    await cloudSync.flush();

    expect(setCloudState).toHaveBeenCalledTimes(1);
    expect(setCloudState).toHaveBeenCalledWith("aro-language", "fr");
  });

  it("handles deletion enqueue and execution", async () => {
    cloudSync.enqueueDelete("aro-custom-field");
    await cloudSync.flush();

    expect(deleteCloudState).toHaveBeenCalledTimes(1);
    expect(deleteCloudState).toHaveBeenCalledWith("aro-custom-field");
  });

  it("keeps failed entries and retries with backoff until success", async () => {
    vi.mocked(setCloudState)
      .mockRejectedValueOnce(new Error("network down"))
      .mockRejectedValueOnce(new Error("500 boom"));
    // Third attempt falls through to the base mockResolvedValue: success.

    const forceRetryNow = () => {
      const entry = (cloudSync as any).pending.get("aro-theme");
      if (entry) entry.nextRetryAt = 0;
    };

    cloudSync.enqueueSet("aro-theme", "dark");
    await cloudSync.flush();
    expect(cloudSync.pendingCount()).toBe(1);

    // Backoff guard: an immediate flush must NOT hammer the server.
    await cloudSync.flush();
    expect(setCloudState).toHaveBeenCalledTimes(1);

    forceRetryNow();
    await cloudSync.flush();
    expect(cloudSync.pendingCount()).toBe(1);

    forceRetryNow();
    await cloudSync.flush();

    expect(setCloudState).toHaveBeenCalledTimes(3);
    expect(setCloudState).toHaveBeenLastCalledWith("aro-theme", "dark");
    expect(cloudSync.pendingCount()).toBe(0);
  });

  it("backs off on 429 without losing the entry", async () => {
    vi.mocked(setCloudState).mockRejectedValueOnce(new Error("429 Too Many Requests"));

    cloudSync.enqueueSet("aro-language", "fr");
    await cloudSync.flush();

    expect(setCloudState).toHaveBeenCalledTimes(1);
    expect(cloudSync.pendingCount()).toBe(1);

    vi.mocked(setCloudState).mockResolvedValueOnce(undefined);
    const entry = (cloudSync as any).pending.get("aro-language");
    entry.nextRetryAt = 0;
    (cloudSync as any).rateLimitedUntil = 0;
    await cloudSync.flush();

    expect(setCloudState).toHaveBeenCalledTimes(2);
    expect(cloudSync.pendingCount()).toBe(0);
  });

  it("one poison entry never blocks the others", async () => {
    vi.mocked(setCloudState).mockImplementation(async (key: string) => {
      if (key === "aro-bad") throw new Error("400 validation failed");
    });

    cloudSync.enqueueSet("aro-good-a", "1");
    cloudSync.enqueueSet("aro-bad", "x");
    cloudSync.enqueueSet("aro-good-c", "3");
    await cloudSync.flush();

    const calls = vi.mocked(setCloudState).mock.calls.map((c) => c[0]);
    expect(calls).toContain("aro-good-a");
    expect(calls).toContain("aro-good-c");
    expect(cloudSync.getPendingKeys()).toEqual(["aro-bad"]);
  });

  it("drops other-scope entries on identity change", async () => {
    cloudSync.setIdentityScope("user-1:org-1");
    cloudSync.enqueueSet("aro-theme", "dark");
    expect(cloudSync.pendingCount()).toBe(1);

    cloudSync.setIdentityScope("user-2:org-2");
    expect(cloudSync.pendingCount()).toBe(0);
    expect(setCloudState).not.toHaveBeenCalled();
  });

  it("persists the queue and resumes it after restart", async () => {
    const fakeStorage = memoryLocalStorage();
    vi.stubGlobal("localStorage", fakeStorage);

    cloudSync.enqueueSet("aro-theme", "dark");
    const stored = JSON.parse(fakeStorage.getItem("aro-cloud-sync-queue-v1")!);
    expect(stored).toHaveLength(1);
    expect(stored[0].key).toBe("aro-theme");
    expect(stored[0].value).toBe("dark");

    const fresh = new CloudSyncManager();
    expect(fresh.pendingCount()).toBe(1);
    await fresh.flush();
    expect(setCloudState).toHaveBeenCalledWith("aro-theme", "dark");
    expect(fresh.pendingCount()).toBe(0);

    vi.unstubAllGlobals();
  });

  it("reset() clears queue, hashes and cooldowns", async () => {
    vi.mocked(setCloudState).mockRejectedValueOnce(new Error("offline"));
    cloudSync.enqueueSet("aro-theme", "dark");
    await cloudSync.flush();
    expect(cloudSync.pendingCount()).toBe(1);

    cloudSync.reset();
    expect(cloudSync.pendingCount()).toBe(0);
    expect(cloudSync.getPendingKeys()).toEqual([]);
  });
});
