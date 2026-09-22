import { invoke } from "@tauri-apps/api/core";
import { isTauri, isWeb, webToken, webFetch } from "../../lib/api/transport";

export type NotificationKind =
  | "info"
  | "success"
  | "warning"
  | "error"
  | "agent-completion"
  | "routine"
  | "system"
  | "security";

export type NotificationPriority = "low" | "normal" | "high" | "urgent";

export type NotificationStatus = "unread" | "read" | "archived";

export type NotificationSource = "agent" | "routine" | "system" | "cloud";

export interface NotificationItem {
  id: string;
  organizationId?: string | null;
  userId?: string | null;
  title: string;
  body: string;
  kind: NotificationKind;
  priority: NotificationPriority;
  status: NotificationStatus;
  source: NotificationSource;
  actionUrl?: string | null;
  metadata?: Record<string, unknown> | null;
  createdAt: string;
  readAt?: string | null;
}

export interface ToastNotification {
  id: string;
  type: "agent-completed" | "info" | "success" | "error";
  title: string;
  body: string;
  conversationId?: string;
  createdAt: number;
}

export interface NotificationFilter {
  status?: NotificationStatus;
  kind?: NotificationKind;
  source?: NotificationSource;
  search?: string;
  organizationId?: string;
  personalOnly?: boolean;
  limit?: number;
  offset?: number;
}

export interface NotificationSettings {
  desktopNotificationsEnabled: boolean;
  soundEnabled: boolean;
  agentCompletionNotifications: boolean;
  routineNotifications: boolean;
  emailNotificationsEnabled: boolean;
  emailOnAgentCompletion: boolean;
  emailOnRoutineSummary: boolean;
  emailRecipient?: string | null;
  emailProvider: string;
  smtpHost?: string | null;
  smtpPort?: number | null;
  smtpUser?: string | null;
  smtpPassword?: string | null;
  smtpFrom?: string | null;
  smtpTlsMode?: string | null;
  apiKey?: string | null;
  authConfigured: boolean;
}

/**
 * Joue un carillon cristallin Apple-grade via la Web Audio API.
 * Génère deux harmoniques sinusoïdales à 880Hz (La5) et 1760Hz (La6) avec décroissance exponentielle.
 */
export function playChimeSound(): void {
  try {
    const AudioContextClass = window.AudioContext || (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
    if (!AudioContextClass) return;
    const ctx = new AudioContextClass();
    if (ctx.state === "suspended") {
      void ctx.resume();
    }

    const now = ctx.currentTime;

    // Fondamentale
    const osc1 = ctx.createOscillator();
    const gain1 = ctx.createGain();
    osc1.type = "sine";
    osc1.frequency.setValueAtTime(880, now);
    osc1.frequency.exponentialRampToValueAtTime(876, now + 0.5);

    gain1.gain.setValueAtTime(0.001, now);
    gain1.gain.linearRampToValueAtTime(0.2, now + 0.015);
    gain1.gain.exponentialRampToValueAtTime(0.0001, now + 0.55);

    osc1.connect(gain1);
    gain1.connect(ctx.destination);

    // Harmonique brillante
    const osc2 = ctx.createOscillator();
    const gain2 = ctx.createGain();
    osc2.type = "sine";
    osc2.frequency.setValueAtTime(1760, now + 0.04);

    gain2.gain.setValueAtTime(0.001, now + 0.04);
    gain2.gain.linearRampToValueAtTime(0.12, now + 0.055);
    gain2.gain.exponentialRampToValueAtTime(0.0001, now + 0.6);

    osc2.connect(gain2);
    gain2.connect(ctx.destination);

    osc1.start(now);
    osc1.stop(now + 0.56);
    osc2.start(now + 0.04);
    osc2.stop(now + 0.61);
  } catch (err) {
    console.debug("Web Audio notification chime omitted:", err);
  }
}

// ==========================================
// FALLBACK DEMO STORE (in-memory for browser / tests)
// ==========================================
let demoNotifications: NotificationItem[] = [];

// ==========================================
// TAURI & CLOUD BRIDGE COMMANDS
// ==========================================

export async function fetchNotifications(filter?: NotificationFilter): Promise<NotificationItem[]> {
  if (isTauri()) {
    try {
      return await invoke<NotificationItem[]>("notification_list", { filter });
    } catch (err) {
      console.error("fetchNotifications error:", err);
      return [];
    }
  }

  if (isWeb() && webToken()) {
    try {
      const params = new URLSearchParams();
      if (filter?.status) params.set("status", filter.status);
      if (filter?.kind) params.set("kind", filter.kind);
      if (filter?.source) params.set("source", filter.source);
      if (filter?.search) params.set("search", filter.search);
      if (filter?.limit) params.set("limit", String(filter.limit));
      if (filter?.offset) params.set("offset", String(filter.offset));
      return await webFetch<NotificationItem[]>("GET", `/notifications?${params.toString()}`);
    } catch (err) {
      console.error("Cloud fetchNotifications error:", err);
    }
  }

  // Fallback demo store
  let items = [...demoNotifications];
  if (filter?.organizationId) {
    items = items.filter((n) => n.organizationId === filter.organizationId);
  } else if (filter?.personalOnly) {
    items = items.filter((n) => !n.organizationId);
  }
  if (filter?.status) {
    items = items.filter((n) => n.status === filter.status);
  }
  if (filter?.kind) {
    items = items.filter((n) => n.kind === filter.kind);
  }
  if (filter?.source) {
    items = items.filter((n) => n.source === filter.source);
  }
  if (filter?.search) {
    const q = filter.search.toLowerCase();
    items = items.filter((n) => n.title.toLowerCase().includes(q) || n.body.toLowerCase().includes(q));
  }
  return items;
}

export async function createNotification(item: Partial<NotificationItem>): Promise<NotificationItem | null> {
  const payload: NotificationItem = {
    id: item.id || crypto.randomUUID(),
    organizationId: item.organizationId ?? null,
    userId: item.userId ?? null,
    title: item.title || "Notification ARO",
    body: item.body || "",
    kind: item.kind || "info",
    priority: item.priority || "normal",
    status: item.status || "unread",
    source: item.source || "system",
    actionUrl: item.actionUrl ?? null,
    metadata: item.metadata ?? null,
    createdAt: item.createdAt || new Date().toISOString(),
    readAt: item.readAt ?? null,
  };

  if (isTauri()) {
    try {
      return await invoke<NotificationItem>("notification_create", { item: payload });
    } catch (err) {
      console.error("createNotification error:", err);
      return null;
    }
  }

  if (isWeb() && webToken()) {
    try {
      return await webFetch<NotificationItem>("POST", "/notifications", payload);
    } catch (err) {
      console.error("Cloud createNotification error:", err);
    }
  }

  // Fallback demo store
  demoNotifications.unshift(payload);
  return payload;
}

export async function markNotificationRead(id: string): Promise<boolean> {
  if (isTauri()) {
    try {
      return await invoke<boolean>("notification_mark_read", { id });
    } catch (err) {
      console.error("markNotificationRead error:", err);
      return false;
    }
  }

  if (isWeb() && webToken()) {
    try {
      await webFetch<{ success: boolean }>("POST", `/notifications/${id}/read`);
      return true;
    } catch {
      return false;
    }
  }

  demoNotifications = demoNotifications.map((n) =>
    n.id === id ? { ...n, status: "read", readAt: new Date().toISOString() } : n
  );
  return true;
}

export async function markAllNotificationsRead(organizationId?: string | null): Promise<number> {
  if (isTauri()) {
    try {
      return await invoke<number>("notification_mark_all_read", { organizationId: organizationId || null });
    } catch (err) {
      console.error("markAllNotificationsRead error:", err);
      return 0;
    }
  }

  if (isWeb() && webToken()) {
    try {
      const res = await webFetch<{ markedCount: number }>("POST", "/notifications/read-all");
      return res.markedCount;
    } catch {
      return 0;
    }
  }

  const now = new Date().toISOString();
  let count = 0;
  demoNotifications = demoNotifications.map((n) => {
    if (organizationId ? n.organizationId === organizationId : !n.organizationId) {
      if (n.status === "unread") count++;
      return { ...n, status: "read", readAt: now };
    }
    return n;
  });
  return count;
}

export async function deleteNotification(id: string): Promise<boolean> {
  if (isTauri()) {
    try {
      return await invoke<boolean>("notification_delete", { id });
    } catch (err) {
      console.error("deleteNotification error:", err);
      return false;
    }
  }

  if (isWeb() && webToken()) {
    try {
      await webFetch<{ success: boolean }>("DELETE", `/notifications/${id}`);
      return true;
    } catch {
      return false;
    }
  }

  demoNotifications = demoNotifications.filter((n) => n.id !== id);
  return true;
}

export async function clearAllNotifications(organizationId?: string | null): Promise<number> {
  if (isTauri()) {
    try {
      return await invoke<number>("notification_clear_all", { organizationId: organizationId || null });
    } catch (err) {
      console.error("clearAllNotifications error:", err);
      return 0;
    }
  }

  if (isWeb() && webToken()) {
    try {
      const res = await webFetch<{ clearedCount: number }>("DELETE", "/notifications/clear");
      return res.clearedCount;
    } catch {
      return 0;
    }
  }

  const before = demoNotifications.length;
  demoNotifications = demoNotifications.filter((n) =>
    organizationId ? n.organizationId !== organizationId : Boolean(n.organizationId)
  );
  return before - demoNotifications.length;
}

export async function getUnreadNotificationCount(organizationId?: string | null): Promise<number> {
  if (isTauri()) {
    try {
      return await invoke<number>("notification_unread_count", { organizationId: organizationId || null });
    } catch (err) {
      console.error("getUnreadNotificationCount error:", err);
      return 0;
    }
  }

  if (isWeb() && webToken()) {
    try {
      const res = await webFetch<{ unreadCount: number }>("GET", "/notifications/unread-count");
      return res.unreadCount;
    } catch {
      return 0;
    }
  }

  return demoNotifications.filter((n) =>
    (organizationId ? n.organizationId === organizationId : !n.organizationId) && n.status === "unread"
  ).length;
}

export async function sendDirectEmail(
  to: string,
  subject: string,
  body: string,
  isHtml?: boolean
): Promise<{ success: boolean; recipient: string; detail?: string }> {
  if (isTauri()) {
    return await invoke<{ success: boolean; recipient: string; detail?: string }>("email_send_direct", {
      to,
      subject,
      body,
      isHtml: isHtml ?? false,
    });
  }

  if (isWeb() && webToken()) {
    return await webFetch<{ success: boolean; recipient: string; detail?: string }>("POST", "/email/send", {
      to,
      subject,
      body,
      isHtml: isHtml ?? false,
    });
  }

  return { success: true, recipient: to, detail: "Dispatched (web demo simulation)" };
}

export async function testEmailConnection(): Promise<{ success: boolean; recipient: string; detail?: string }> {
  if (isTauri()) {
    return await invoke<{ success: boolean; recipient: string; detail?: string }>("email_test_connection");
  }

  if (isWeb() && webToken()) {
    return await webFetch<{ success: boolean; recipient: string; detail?: string }>("POST", "/email/test");
  }

  return { success: true, recipient: "test@aro.local", detail: "Connection test succeeded (demo simulation)" };
}

export async function setNotificationSecret(secretType: string, secretValue: string): Promise<any> {
  if (isTauri()) {
    return await invoke("notification_set_secret", { secretType, secretValue });
  }
  return { success: true };
}

export async function clearNotificationSecret(secretType: string): Promise<any> {
  if (isTauri()) {
    return await invoke("notification_clear_secret", { secretType });
  }
  return { success: true };
}
