/**
 * CloudSyncManager — Apple/Google-grade state synchronization engine for ARO Desktop.
 *
 * Features:
 * 1. Per-Key Debouncing & Coalescing: collates rapid writes to the same key into a single remote sync call.
 * 2. Dirty-Checking: compares JSON snapshots against last synced state; skips no-op writes.
 * 3. Ephemeral/Telemetry Filtering: excludes local-only high-frequency keys (e.g. aro-perf-history, aro-user-activity).
 * 4. Circuit Breaker with Adaptive Backoff: pauses network sync gracefully upon receiving HTTP 429 rate limits.
 * 5. Lifecycle Flushing: dispatches pending mutations on app minimize or window unload (best-effort).
 * 6. Durable Queue: entries survive reloads/crashes via localStorage, are never
 *    dropped on failure (only re-scoped on identity change), and retry with
 *    capped exponential backoff + jitter. Sync of one key never blocks others.
 * 7. Identity Scoping: pending work is tagged `userId:orgId`; switching
 *    account/org drops stale entries instead of syncing them into the
 *    wrong tenant.
 */

import { setCloudState, deleteCloudState } from "./transport";

const DEBOUNCE_DELAY_MS = 600;
const MAX_LATENCY_MS = 2500;
const RATE_LIMIT_COOLDOWN_MS = 15000;
const AUTH_RETRY_COOLDOWN_MS = 60000;
const MAX_BACKOFF_MS = 5 * 60 * 1000;
const QUEUE_STORAGE_KEY = "aro-cloud-sync-queue-v1";
const QUEUE_STORAGE_MAX_ENTRIES = 200;
const QUEUE_STORAGE_MAX_ENTRY_CHARS = 256 * 1024;

// Keys that are strictly local caches or high-frequency telemetry and should not hit the cloud state API
const LOCAL_ONLY_KEYS = new Set([
  "aro-perf-history",
  "aro-user-activity",
  "networkAlertDismissed",
  "aro_web_access_token",
  "aro_web_refresh_token",
]);

type SyncFailureKind = "rate_limited" | "auth" | "network" | "rejected";

interface PendingUpdate {
  value: unknown;
  serialized: string;
  isDelete: boolean;
  timer: ReturnType<typeof setTimeout> | null;
  firstScheduledAt: number;
  attempts: number;
  nextRetryAt: number;
  inFlight: boolean;
  scopeId: string;
}

interface PersistedEntry {
  key: string;
  value: unknown;
  serialized: string;
  isDelete: boolean;
  attempts: number;
  firstScheduledAt: number;
  scopeId: string;
}

function classifySyncError(error: any): SyncFailureKind {
  const status = typeof error?.status === "number" ? error.status : undefined;
  const msg = String(error?.message ?? error ?? "");
  if (status === 429 || /429|rate limit|too many requests/i.test(msg)) return "rate_limited";
  if (status === 401 || status === 403 || /401|403|unauthor|forbidden|\bauth\b/i.test(msg)) {
    return "auth";
  }
  if (/failed to fetch|network|offline|timeout|timed out|econn|enotfound|invoke|abort/i.test(msg)) {
    return "network";
  }
  // Validation/4xx rejections are parked with capped backoff, never dropped:
  // only an identity change discards queued work.
  return "rejected";
}

function backoffDelayMs(attempts: number): number {
  const capped = Math.min(attempts, 8);
  const base = Math.min(1000 * 2 ** capped, MAX_BACKOFF_MS);
  return base + Math.floor(Math.random() * 500);
}

function readStoredQueue(): PersistedEntry[] {
  try {
    if (typeof localStorage === "undefined") return [];
    const raw = localStorage.getItem(QUEUE_STORAGE_KEY);
    if (!raw) return [];
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(
      (entry): entry is PersistedEntry =>
        !!entry &&
        typeof entry === "object" &&
        typeof (entry as PersistedEntry).key === "string" &&
        typeof (entry as PersistedEntry).serialized === "string" &&
        typeof (entry as PersistedEntry).scopeId === "string"
    );
  } catch {
    return [];
  }
}

class CloudSyncManager {
  private pending = new Map<string, PendingUpdate>();
  private lastSyncedHash = new Map<string, string>();
  private rateLimitedUntil = 0;
  private isFlushing = false;
  private isEnabled = true;
  private scopeId = "default";

  constructor() {
    this.restoreQueue();
    if (typeof window !== "undefined" && typeof window.addEventListener === "function") {
      window.addEventListener("beforeunload", () => {
        void this.flush();
      });
    }
    if (typeof document !== "undefined" && typeof document.addEventListener === "function") {
      document.addEventListener("visibilitychange", () => {
        if (document.visibilityState === "hidden") {
          void this.flush();
        }
      });
    }
  }

  /**
   * Determine if a key should be excluded from cloud state synchronization.
   */
  public isLocalOnlyKey(key: string): boolean {
    if (!key.startsWith("aro-")) return true;
    if (LOCAL_ONLY_KEYS.has(key)) return true;
    if (
      key === "aro-api-keys" ||
      key === "aro-user-plugins" ||
      key === "aro-mcp-servers" ||
      key === "aro-hooks" ||
      key.includes("token") ||
      key.includes("secret") ||
      key.includes("password") ||
      key.includes("key")
    ) {
      return true;
    }
    return false;
  }

  /** Global on/off switch (tests, offline modes). */
  public setEnabled(enabled: boolean): void {
    this.isEnabled = enabled;
  }

  /** Full reset: timers, queue, hashes, cooldowns, scope. Logout path. */
  public reset(): void {
    for (const [, update] of this.pending) {
      if (update.timer) clearTimeout(update.timer);
    }
    this.pending.clear();
    this.lastSyncedHash.clear();
    this.rateLimitedUntil = 0;
    this.isFlushing = false;
    this.scopeId = "default";
    this.persistQueue();
  }

  /** Number of keys currently awaiting (re)sync. Never throws. */
  public pendingCount(): number {
    return this.pending.size;
  }

  /** Keys currently awaiting (re)sync (diagnostics/UI). */
  public getPendingKeys(): string[] {
    return Array.from(this.pending.keys());
  }

  /**
   * Scope the queue to an identity (`userId:orgId`, or `"default"` when
   * logged out). Entries from another scope are dropped so pending writes
   * can never leak into the wrong tenant; call on login, logout and
   * organization switch.
   */
  public setIdentityScope(scopeId: string): void {
    const next = scopeId && scopeId.trim() ? scopeId.trim() : "default";
    if (next === this.scopeId) return;
    for (const [, update] of this.pending) {
      if (update.timer) clearTimeout(update.timer);
    }
    this.pending.clear();
    this.lastSyncedHash.clear();
    this.rateLimitedUntil = 0;
    this.scopeId = next;
    this.persistQueue();
  }

  /**
   * Pre-seed the known synced state (e.g. during initial cloud hydration)
   * to avoid echoing back data that just arrived from the server.
   */
  public seedSyncedState(key: string, value: unknown): void {
    try {
      const serialized = typeof value === "string" ? value : JSON.stringify(value);
      this.lastSyncedHash.set(key, serialized);
    } catch {
      // Ignore serialization failures for non-standard values
    }
  }

  /**
   * Schedule a state update to be synchronized to the cloud.
   */
  public enqueueSet(key: string, value: unknown): void {
    if (!this.isEnabled || this.isLocalOnlyKey(key)) return;

    let serialized: string;
    try {
      serialized = typeof value === "string" ? value : JSON.stringify(value);
    } catch {
      serialized = String(value);
    }

    // Dirty checking: If value hasn't changed since last successful sync, skip entirely.
    if (this.lastSyncedHash.get(key) === serialized) {
      const existing = this.pending.get(key);
      if (existing) {
        if (existing.timer) clearTimeout(existing.timer);
        this.pending.delete(key);
        this.persistQueue();
      }
      return;
    }

    const now = Date.now();
    const existing = this.pending.get(key);

    if (existing) {
      if (existing.timer) clearTimeout(existing.timer);
      existing.value = value;
      existing.serialized = serialized;
      existing.isDelete = false;
      existing.attempts = 0;
      existing.nextRetryAt = 0;
      existing.scopeId = this.scopeId;

      // If max latency exceeded, flush immediately
      if (now - existing.firstScheduledAt >= MAX_LATENCY_MS) {
        void this.syncKey(key);
        return;
      }

      existing.timer = setTimeout(() => {
        void this.syncKey(key);
      }, DEBOUNCE_DELAY_MS);
    } else {
      const update: PendingUpdate = {
        value,
        serialized,
        isDelete: false,
        firstScheduledAt: now,
        attempts: 0,
        nextRetryAt: 0,
        inFlight: false,
        scopeId: this.scopeId,
        timer: setTimeout(() => {
          void this.syncKey(key);
        }, DEBOUNCE_DELAY_MS),
      };
      this.pending.set(key, update);
    }
    this.persistQueue();
  }

  /**
   * Schedule a state deletion to be synchronized to the cloud.
   */
  public enqueueDelete(key: string): void {
    if (!this.isEnabled || this.isLocalOnlyKey(key)) return;

    const existing = this.pending.get(key);
    if (existing?.timer) clearTimeout(existing.timer);

    const now = Date.now();
    const firstScheduledAt = existing ? existing.firstScheduledAt : now;
    const update: PendingUpdate = {
      value: null,
      serialized: "",
      isDelete: true,
      firstScheduledAt,
      attempts: 0,
      nextRetryAt: 0,
      inFlight: false,
      scopeId: this.scopeId,
      timer: null,
    };
    // Same max-latency guarantee as sets: never hold a delete hostage.
    if (now - firstScheduledAt >= MAX_LATENCY_MS) {
      this.pending.set(key, update);
      this.persistQueue();
      void this.syncKey(key);
      return;
    }
    update.timer = setTimeout(() => {
      void this.syncKey(key);
    }, DEBOUNCE_DELAY_MS);
    this.pending.set(key, update);
    this.persistQueue();
  }

  /**
   * Synchronize a specific key to the cloud API. The entry stays queued for
   * the whole attempt and is removed only on success; every failure keeps
   * it with a scheduled retry. Never throws.
   */
  private async syncKey(key: string): Promise<void> {
    const update = this.pending.get(key);
    if (!update || update.inFlight) return;

    // Stale identity: drop instead of syncing into the wrong tenant.
    // Exception: while logged out ("default") we hold foreign entries —
    // the user may simply not have logged back in yet (restart case).
    if (update.scopeId !== this.scopeId) {
      if (this.scopeId === "default") {
        update.timer = setTimeout(() => {
          void this.syncKey(key);
        }, 5000);
        return;
      }
      if (update.timer) clearTimeout(update.timer);
      this.pending.delete(key);
      this.persistQueue();
      return;
    }

    if (update.timer) {
      clearTimeout(update.timer);
      update.timer = null;
    }

    // Backoff / circuit-breaker checks (entry is kept in both cases).
    const now = Date.now();
    if (now < this.rateLimitedUntil) {
      const waitTime = Math.max(this.rateLimitedUntil - now, 1000);
      update.timer = setTimeout(() => {
        void this.syncKey(key);
      }, waitTime);
      return;
    }
    if (now < update.nextRetryAt) {
      update.timer = setTimeout(() => {
        void this.syncKey(key);
      }, Math.max(update.nextRetryAt - now, 1000));
      return;
    }

    update.inFlight = true;
    try {
      if (update.isDelete) {
        await deleteCloudState(key);
        this.lastSyncedHash.delete(key);
      } else {
        await setCloudState(key, update.value);
        this.lastSyncedHash.set(key, update.serialized);
      }
      if (update.timer) clearTimeout(update.timer);
      this.pending.delete(key);
      this.persistQueue();
    } catch (error: any) {
      update.inFlight = false;
      update.attempts += 1;
      const kind = classifySyncError(error);
      if (kind === "rate_limited") {
        console.warn(`[CloudSync] Rate limit detected. Backing off for ${RATE_LIMIT_COOLDOWN_MS}ms.`);
        this.rateLimitedUntil = Date.now() + RATE_LIMIT_COOLDOWN_MS;
      } else if (kind === "auth") {
        console.warn(
          `[CloudSync] Auth failure for "${key}" (attempt ${update.attempts}); parked for ${AUTH_RETRY_COOLDOWN_MS}ms. Re-login or identity change resolves it.`
        );
        update.nextRetryAt = Date.now() + AUTH_RETRY_COOLDOWN_MS;
      } else if (kind === "network") {
        const delay = backoffDelayMs(update.attempts);
        console.warn(
          `[CloudSync] Network failure for "${key}" (attempt ${update.attempts}); retry in ${delay}ms.`
        );
        update.nextRetryAt = Date.now() + delay;
      } else {
        const delay = backoffDelayMs(update.attempts);
        console.warn(
          `[CloudSync] Sync rejected for "${key}" (attempt ${update.attempts}, kept queued); retry in ${delay}ms:`,
          error
        );
        update.nextRetryAt = Date.now() + delay;
      }
      update.timer = setTimeout(() => {
        void this.syncKey(key);
      }, Math.max((update.nextRetryAt || this.rateLimitedUntil) - Date.now(), 1000));
      this.persistQueue();
    } finally {
      const current = this.pending.get(key);
      if (current) current.inFlight = false;
    }
  }

  /**
   * Immediately flush all pending synchronization operations. One key's
   * failure never blocks the others.
   */
  public async flush(): Promise<void> {
    if (this.isFlushing || this.pending.size === 0) return;
    this.isFlushing = true;

    try {
      const keys = Array.from(this.pending.keys());
      for (const key of keys) {
        try {
          await this.syncKey(key);
        } catch (error) {
          console.warn(`[CloudSync] Flush failed for "${key}":`, error);
        }
      }
    } finally {
      this.isFlushing = false;
    }
  }

  private persistQueue(): void {
    try {
      if (typeof localStorage === "undefined") return;
      const entries: PersistedEntry[] = [];
      for (const [key, update] of this.pending) {
        if (entries.length >= QUEUE_STORAGE_MAX_ENTRIES) break;
        if (update.serialized.length > QUEUE_STORAGE_MAX_ENTRY_CHARS) continue;
        entries.push({
          key,
          value: update.value,
          serialized: update.serialized,
          isDelete: update.isDelete,
          attempts: update.attempts,
          firstScheduledAt: update.firstScheduledAt,
          scopeId: update.scopeId,
        });
      }
      localStorage.setItem(QUEUE_STORAGE_KEY, JSON.stringify(entries));
    } catch {
      // Quota or access errors must never break syncing.
    }
  }

  private restoreQueue(): void {
    const now = Date.now();
    for (const entry of readStoredQueue()) {
      if (this.pending.size >= QUEUE_STORAGE_MAX_ENTRIES) break;
      this.pending.set(entry.key, {
        value: entry.value,
        serialized: entry.serialized,
        isDelete: entry.isDelete,
        timer: setTimeout(() => {
          void this.syncKey(entry.key);
        }, DEBOUNCE_DELAY_MS),
        firstScheduledAt: entry.firstScheduledAt || now,
        attempts: entry.attempts || 0,
        nextRetryAt: 0,
        inFlight: false,
        scopeId: entry.scopeId || "default",
      });
    }
  }
}

export const cloudSync = new CloudSyncManager();

// Exported for tests and advanced use (the app uses the singleton).
export { CloudSyncManager };
