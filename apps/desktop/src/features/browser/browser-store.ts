// Integrated Browser & Computer Use Store for ARO
import { writable, get } from "svelte/store";

export interface BrowserHistoryEntry {
  id: string;
  url: string;
  title: string;
  visitedAt: string;
  favicon?: string;
}

export interface BrowserCredential {
  id: string;
  domain: string;
  username: string;
  /** Legacy plaintext slot — always empty since the vault migration (secrets live in the OS keyring). */
  password: string;
  createdAt: string;
  lastUsedAt?: string;
}

export interface BrowserPermissions {
  allowAiBrowsing: boolean;
  allowAiInteraction: boolean;
  allowComputerUse: boolean;
  confirmSensitiveActions: boolean;
  defaultSearchEngine: "duckduckgo" | "google" | "brave" | "bing";
  javascriptEnabled: boolean;
  blockThirdPartyCookies: boolean;
  userAgent: string;
}

export interface BrowserTab {
  id: string;
  url: string;
  title: string;
  loading: boolean;
  canGoBack: boolean;
  canGoForward: boolean;
  history: string[];
  historyIndex: number;
  extractedContent?: string;
  isAiControlled: boolean;
  aiStatusMessage?: string;
  readerMode?: boolean;
}

const STORAGE_KEY_HISTORY = "aro_browser_history";
const STORAGE_KEY_CREDENTIALS = "aro_browser_credentials";
const STORAGE_KEY_PERMISSIONS = "aro_browser_permissions";

function loadFromStorage<T>(key: string, defaultValue: T): T {
  if (typeof localStorage === "undefined") return defaultValue;
  try {
    const raw = localStorage.getItem(key);
    return raw ? JSON.parse(raw) : defaultValue;
  } catch {
    return defaultValue;
  }
}

function saveToStorage<T>(key: string, value: T): void {
  if (typeof localStorage === "undefined") return;
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch (err) {
    console.error(`Failed to save ${key} to storage:`, err);
  }
}

const defaultPermissions: BrowserPermissions = {
  allowAiBrowsing: true,
  allowAiInteraction: true,
  allowComputerUse: false, // Explicit opt-in required for OS control
  confirmSensitiveActions: true,
  defaultSearchEngine: "duckduckgo",
  javascriptEnabled: true,
  blockThirdPartyCookies: true,
  userAgent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) AroAI/1.0 Safari/537.36",
};

export const browserHistory = writable<BrowserHistoryEntry[]>(
  loadFromStorage<BrowserHistoryEntry[]>(STORAGE_KEY_HISTORY, [
    {
      id: "hist-1",
      url: "https://duckduckgo.com",
      title: "DuckDuckGo — Privacy, simplified.",
      visitedAt: new Date(Date.now() - 3600000).toISOString(),
    },
    {
      id: "hist-2",
      url: "https://en.wikipedia.org/wiki/Artificial_general_intelligence",
      title: "Artificial general intelligence - Wikipedia",
      visitedAt: new Date(Date.now() - 7200000).toISOString(),
    },
    {
      id: "hist-3",
      url: "https://github.com",
      title: "GitHub: Let's build from here",
      visitedAt: new Date(Date.now() - 14400000).toISOString(),
    },
  ])
);

export const browserCredentials = writable<BrowserCredential[]>(
  loadFromStorage<BrowserCredential[]>(STORAGE_KEY_CREDENTIALS, [])
);

// One-time vault migration: purge any legacy plaintext password that may
// still sit in localStorage from older versions. Secrets now live only in
// the OS keyring (see browser_vault_save).
browserCredentials.update((list) => {
  let scrubbed = false;
  const next = list.map((cred) => {
    if (cred.password) {
      scrubbed = true;
      return { ...cred, password: "" };
    }
    return cred;
  });
  if (scrubbed) {
    try {
      if (typeof localStorage !== "undefined") {
        localStorage.setItem(STORAGE_KEY_CREDENTIALS, JSON.stringify(next));
      }
    } catch {
      // Ignore persistence failures: in-memory state is already clean.
    }
  }
  return next;
});

export const browserPermissions = writable<BrowserPermissions>(
  loadFromStorage<BrowserPermissions>(STORAGE_KEY_PERMISSIONS, defaultPermissions)
);

// Initial tab definition
const initialTab: BrowserTab = {
  id: "tab-default",
  url: "https://duckduckgo.com",
  title: "DuckDuckGo — Privacy, simplified.",
  loading: false,
  canGoBack: false,
  canGoForward: false,
  history: ["https://duckduckgo.com"],
  historyIndex: 0,
  isAiControlled: false,
  readerMode: false,
};

// Multi-tab stores
export const browserTabs = writable<BrowserTab[]>([initialTab]);
export const activeTabId = writable<string>(initialTab.id);

// Internal active tab store
const internalActiveTab = writable<BrowserTab>(initialTab);

// Active browser state (fully reactive and backwards compatible with standard writable)
export const activeBrowserTab = {
  subscribe: internalActiveTab.subscribe,
  set(tab: BrowserTab) {
    internalActiveTab.set(tab);
    browserTabs.update((tabs) => {
      const idx = tabs.findIndex((t) => t.id === tab.id);
      if (idx >= 0) {
        const copy = [...tabs];
        copy[idx] = tab;
        return copy;
      }
      return [...tabs, tab];
    });
    activeTabId.set(tab.id);
  },
  update(fn: (tab: BrowserTab) => BrowserTab) {
    internalActiveTab.update((prev) => {
      const next = fn(prev);
      browserTabs.update((tabs) => {
        const idx = tabs.findIndex((t) => t.id === next.id);
        if (idx >= 0) {
          const copy = [...tabs];
          copy[idx] = next;
          return copy;
        }
        return [...tabs, next];
      });
      activeTabId.set(next.id);
      return next;
    });
  },
};

// Auto-persist subscribers
browserHistory.subscribe((val) => saveToStorage(STORAGE_KEY_HISTORY, val));
browserCredentials.subscribe((val) => saveToStorage(STORAGE_KEY_CREDENTIALS, val));
browserPermissions.subscribe((val) => saveToStorage(STORAGE_KEY_PERMISSIONS, val));

// Browser Tab Actions
export function createBrowserTab(
  url: string = "https://duckduckgo.com",
  title?: string,
  isAiControlled: boolean = false,
  aiStatusMessage?: string
): string {
  const id = "tab-" + Date.now() + "-" + Math.random().toString(36).substring(2, 7);
  const finalTitle = title || (url ? extractDomain(url) : "Nouvel onglet");
  const newTab: BrowserTab = {
    id,
    url,
    title: finalTitle,
    loading: false,
    canGoBack: false,
    canGoForward: false,
    history: [url],
    historyIndex: 0,
    isAiControlled,
    aiStatusMessage,
    readerMode: false,
  };

  browserTabs.update((tabs) => [...tabs, newTab]);
  activeTabId.set(id);
  internalActiveTab.set(newTab);
  return id;
}

export function switchBrowserTab(tabId: string): void {
  const tabs = get(browserTabs);
  const found = tabs.find((t) => t.id === tabId);
  if (found) {
    activeTabId.set(tabId);
    internalActiveTab.set(found);
  }
}

export function closeBrowserTab(tabId: string): void {
  const tabs = get(browserTabs);
  const closingIndex = tabs.findIndex((t) => t.id === tabId);
  if (closingIndex === -1) return;

  const remaining = tabs.filter((t) => t.id !== tabId);
  if (remaining.length === 0) {
    browserTabs.set([]);
    createBrowserTab("https://duckduckgo.com", "DuckDuckGo — Privacy, simplified.");
    return;
  }

  browserTabs.set(remaining);
  const currentActiveId = get(activeTabId);
  if (currentActiveId === tabId) {
    const nextIndex = Math.min(closingIndex, remaining.length - 1);
    const nextTab = remaining[nextIndex];
    activeTabId.set(nextTab.id);
    internalActiveTab.set(nextTab);
  }
}

export function takeBrowserControl(tabId?: string): void {
  const targetId = tabId || get(activeTabId);
  browserTabs.update((tabs) =>
    tabs.map((tab) => {
      if (tab.id !== targetId) return tab;
      const updated: BrowserTab = {
        ...tab,
        isAiControlled: false,
        aiStatusMessage: undefined,
      };
      if (tab.id === get(activeTabId)) {
        internalActiveTab.set(updated);
      }
      return updated;
    })
  );
  if (targetId) {
    activeTabId.set(targetId);
  }
}

export function toggleReaderMode(tabId?: string, forceMode?: boolean): void {
  const targetId = tabId || get(activeTabId);
  browserTabs.update((tabs) =>
    tabs.map((tab) => {
      if (tab.id !== targetId) return tab;
      const updated: BrowserTab = {
        ...tab,
        readerMode: forceMode !== undefined ? forceMode : !tab.readerMode,
      };
      if (tab.id === get(activeTabId)) {
        internalActiveTab.set(updated);
      }
      return updated;
    })
  );
}

// Browser Navigation Actions
export function navigateBrowser(
  targetUrl: string,
  isAiAction: boolean = false,
  aiStatus?: string,
  tabId?: string
): void {
  let finalUrl = targetUrl.trim();
  if (!finalUrl) return;

  if (!/^https?:\/\//i.test(finalUrl)) {
    if (finalUrl.includes(".") && !finalUrl.includes(" ")) {
      finalUrl = "https://" + finalUrl;
    } else {
      const perms = get(browserPermissions);
      const searchEngines: Record<string, string> = {
        duckduckgo: "https://duckduckgo.com/?q=",
        google: "https://www.google.com/search?q=",
        brave: "https://search.brave.com/search?q=",
        bing: "https://www.bing.com/search?q=",
      };
      const base = searchEngines[perms.defaultSearchEngine] || searchEngines.duckduckgo;
      finalUrl = base + encodeURIComponent(finalUrl);
    }
  }

  const targetTabId = tabId || get(activeTabId);

  browserTabs.update((tabs) =>
    tabs.map((tab) => {
      if (tab.id !== targetTabId) return tab;
      const newHistory = tab.history.slice(0, tab.historyIndex + 1);
      newHistory.push(finalUrl);
      const updated: BrowserTab = {
        ...tab,
        url: finalUrl,
        title: extractDomain(finalUrl),
        loading: true,
        history: newHistory,
        historyIndex: newHistory.length - 1,
        canGoBack: newHistory.length > 1,
        canGoForward: false,
        isAiControlled: isAiAction,
        aiStatusMessage: aiStatus || (isAiAction ? "L'IA navigue vers la page..." : undefined),
      };
      if (tab.id === get(activeTabId)) {
        internalActiveTab.set(updated);
      }
      return updated;
    })
  );

  // Record history
  addBrowserHistoryEntry(finalUrl, extractDomain(finalUrl));

  // Simulated content extraction / finish load
  setTimeout(() => {
    browserTabs.update((tabs) =>
      tabs.map((tab) => {
        if (tab.id !== targetTabId) return tab;
        const loaded: BrowserTab = {
          ...tab,
          loading: false,
          aiStatusMessage: isAiAction ? "Page chargée et analysée par l'agent." : undefined,
        };
        if (tab.id === get(activeTabId)) {
          internalActiveTab.set(loaded);
        }
        return loaded;
      })
    );
  }, 600);
}

export function browserGoBack(tabId?: string): void {
  const targetTabId = tabId || get(activeTabId);
  browserTabs.update((tabs) =>
    tabs.map((tab) => {
      if (tab.id !== targetTabId || tab.historyIndex <= 0) return tab;
      const newIndex = tab.historyIndex - 1;
      const targetUrl = tab.history[newIndex];
      const updated: BrowserTab = {
        ...tab,
        url: targetUrl,
        title: extractDomain(targetUrl),
        historyIndex: newIndex,
        canGoBack: newIndex > 0,
        canGoForward: true,
      };
      if (tab.id === get(activeTabId)) {
        internalActiveTab.set(updated);
      }
      return updated;
    })
  );
}

export function browserGoForward(tabId?: string): void {
  const targetTabId = tabId || get(activeTabId);
  browserTabs.update((tabs) =>
    tabs.map((tab) => {
      if (tab.id !== targetTabId || tab.historyIndex >= tab.history.length - 1) return tab;
      const newIndex = tab.historyIndex + 1;
      const targetUrl = tab.history[newIndex];
      const updated: BrowserTab = {
        ...tab,
        url: targetUrl,
        title: extractDomain(targetUrl),
        historyIndex: newIndex,
        canGoBack: true,
        canGoForward: newIndex < tab.history.length - 1,
      };
      if (tab.id === get(activeTabId)) {
        internalActiveTab.set(updated);
      }
      return updated;
    })
  );
}

export function browserReload(tabId?: string): void {
  const targetTabId = tabId || get(activeTabId);
  browserTabs.update((tabs) =>
    tabs.map((tab) => {
      if (tab.id !== targetTabId) return tab;
      const updated = { ...tab, loading: true };
      if (tab.id === get(activeTabId)) {
        internalActiveTab.set(updated);
      }
      return updated;
    })
  );
  setTimeout(() => {
    browserTabs.update((tabs) =>
      tabs.map((tab) => {
        if (tab.id !== targetTabId) return tab;
        const updated = { ...tab, loading: false };
        if (tab.id === get(activeTabId)) {
          internalActiveTab.set(updated);
        }
        return updated;
      })
    );
  }, 400);
}

export function addBrowserHistoryEntry(url: string, title: string): void {
  const entry: BrowserHistoryEntry = {
    id: "hist-" + Date.now() + "-" + Math.random().toString(36).substring(2, 7),
    url,
    title: title || extractDomain(url),
    visitedAt: new Date().toISOString(),
  };
  browserHistory.update((list) => [entry, ...list.filter((item) => item.url !== url)].slice(0, 500));
}

export function clearBrowserHistory(): void {
  browserHistory.set([]);
}

export function deleteBrowserHistoryEntry(id: string): void {
  browserHistory.update((list) => list.filter((item) => item.id !== id));
}

export function addBrowserCredential(cred: Omit<BrowserCredential, "id" | "createdAt">): void {
  // Vault migration: passwords are NEVER persisted here anymore (OS keyring
  // via browser_vault_save). The index keeps domain + username only.
  const newCred: BrowserCredential = {
    ...cred,
    password: "",
    id: "cred-" + Date.now(),
    createdAt: new Date().toISOString(),
  };
  browserCredentials.update((list) => [newCred, ...list]);
}

export function updateBrowserCredential(id: string, updates: Partial<BrowserCredential>): void {
  browserCredentials.update((list) =>
    list.map((item) => (item.id === id ? { ...item, ...updates } : item))
  );
}

export function deleteBrowserCredential(id: string): void {
  browserCredentials.update((list) => list.filter((item) => item.id !== id));
}

export function updateBrowserPermissions(perms: Partial<BrowserPermissions>): void {
  browserPermissions.update((prev) => ({ ...prev, ...perms }));
}

// ── Live backend bridge (real Chromium via Tauri) ──────────────────────
// Graceful outside Tauri (web preview / vitest): every call rejects with a
// clear error instead of crashing, so the UI can show its offline state.

export interface LiveBrowserTab {
  id: string;
  url: string;
  title: string;
  loading: boolean;
  aiControlled: boolean;
  aiStatusMessage?: string;
  createdAt: string;
  lastSnapshotAt?: string;
}

export interface LiveElementRef {
  id: string;
  role: string;
  name: string;
  inputType?: string;
  value?: string;
  inViewport: boolean;
}

export interface LivePageSnapshot {
  tabId: string;
  url: string;
  title: string;
  screenshotBase64: string;
  screenshotWidth: number;
  screenshotHeight: number;
  elements: LiveElementRef[];
  textExcerpt: string;
  capturedAt: string;
}

async function liveInvoke<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  if (!isTauri()) throw new Error("live browser requires the Tauri desktop app");
  const { invoke } = await import("@tauri-apps/api/core");
  return await invoke<T>(cmd, args);
}

export const liveBrowserAvailable = (): boolean => isTauri();

export async function liveTabsList(): Promise<LiveBrowserTab[]> {
  const tabs = await liveInvoke<Array<Record<string, unknown>>>("browser_tabs_list");
  return tabs.map((t) => ({
    id: String(t.id ?? (t as { tabId?: unknown }).tabId ?? ""),
    url: String(t.url ?? ""),
    title: String(t.title ?? ""),
    loading: Boolean(t.loading ?? false),
    aiControlled: Boolean(t.aiControlled ?? (t as { ai_controlled?: unknown }).ai_controlled ?? false),
    aiStatusMessage: (t.aiStatusMessage as string | undefined) ?? undefined,
    createdAt: String((t as { createdAt?: unknown }).createdAt ?? new Date().toISOString()),
    lastSnapshotAt: (t as { lastSnapshotAt?: unknown }) as string | undefined,
  }));
}

export async function liveTabOpen(url?: string): Promise<LiveBrowserTab> {
  const t = await liveInvoke<Record<string, unknown>>("browser_tab_open", { url: url ?? null });
  return {
    id: String(t.id ?? ""),
    url: String(t.url ?? url ?? ""),
    title: String(t.title ?? ""),
    loading: false,
    aiControlled: false,
    createdAt: new Date().toISOString(),
  };
}

export async function liveTabClose(tabId: string): Promise<boolean> {
  return await liveInvoke<boolean>("browser_tab_close", { tabId });
}

export async function liveNavigate(tabId: string | null, url: string): Promise<LivePageSnapshot> {
  return await liveInvoke<LivePageSnapshot>("browser_navigate", { tabId, url });
}

export async function liveSnapshot(tabId: string): Promise<LivePageSnapshot> {
  return await liveInvoke<LivePageSnapshot>("browser_snapshot", { tabId });
}

export async function liveFrame(tabId: string): Promise<{ imageBase64: string; mime: string }> {
  return await liveInvoke<{ imageBase64: string; mime: string }>("browser_frame", { tabId });
}

export async function liveClick(tabId: string, x: number, y: number): Promise<LivePageSnapshot> {
  return await liveInvoke<LivePageSnapshot>("browser_click", { tabId, x, y });
}

export async function liveClickRef(tabId: string, targetRef: string): Promise<LivePageSnapshot> {
  return await liveInvoke<LivePageSnapshot>("browser_click_ref", { tabId, targetRef });
}

export async function liveTypeText(tabId: string, text: string): Promise<LivePageSnapshot> {
  return await liveInvoke<LivePageSnapshot>("browser_type_text", { tabId, text });
}

export async function livePressKey(tabId: string, key: string): Promise<LivePageSnapshot> {
  return await liveInvoke<LivePageSnapshot>("browser_press_key", { tabId, key });
}

export async function liveWheel(tabId: string, deltaX: number, deltaY: number): Promise<LivePageSnapshot> {
  return await liveInvoke<LivePageSnapshot>("browser_wheel", { tabId, deltaX, deltaY });
}

export async function liveAutofillUser(tabId: string, usernameRef: string, passwordRef: string, account: string): Promise<LivePageSnapshot> {
  return await liveInvoke<LivePageSnapshot>("browser_autofill_user", { tabId, usernameRef, passwordRef, account });
}

// ── Vault (OS keyring — write/has/delete only, never read) ─────────────

export async function vaultSave(account: string, kind: "username" | "password", secret: string): Promise<boolean> {
  try {
    return await liveInvoke<boolean>("browser_vault_save", { account, kind, secret });
  } catch {
    return false;
  }
}

export async function vaultHas(account: string, kind: "username" | "password"): Promise<boolean> {
  try {
    return await liveInvoke<boolean>("browser_vault_has", { account, kind });
  } catch {
    return false;
  }
}

export async function vaultDelete(account: string, kind: "username" | "password"): Promise<boolean> {
  try {
    return await liveInvoke<boolean>("browser_vault_delete", { account, kind });
  } catch {
    return false;
  }
}

// ── Vault account index (domains only — never secrets) ─────────────────

const STORAGE_KEY_VAULT_ACCOUNTS = "aro_vault_accounts";

export const vaultAccounts = writable<string[]>(
  loadFromStorage<string[]>(STORAGE_KEY_VAULT_ACCOUNTS, [])
);

vaultAccounts.subscribe((val) => saveToStorage(STORAGE_KEY_VAULT_ACCOUNTS, val));

export function vaultAccountAdd(account: string): void {
  const clean = account.trim().toLowerCase().replace(/^https?:\/\//i, "").split("/")[0];
  if (!clean) return;
  vaultAccounts.update((list) => (list.includes(clean) ? list : [...list, clean]));
}

export function vaultAccountRemove(account: string): void {
  vaultAccounts.update((list) => list.filter((a) => a !== account));
}

// ── System browser import ──────────────────────────────────────────────

export interface ImportedBookmark {
  name: string;
  url: string;
}

export interface ImportedHistoryEntry {
  url: string;
  title: string;
  visitCount: number;
}

export async function importSystemBookmarks(): Promise<ImportedBookmark[]> {
  return await liveInvoke<ImportedBookmark[]>("browser_import_bookmarks");
}

export async function importSystemHistory(limit = 100): Promise<ImportedHistoryEntry[]> {
  return await liveInvoke<ImportedHistoryEntry[]>("browser_import_history", { limit });
}

function extractDomain(url: string): string {
  try {
    const parsed = new URL(url);
    return parsed.hostname.replace(/^www\./, "");
  } catch {
    return url;
  }
}

function isTauri(): boolean {
  return typeof window !== "undefined" && Boolean((window as any).__TAURI_INTERNALS__);
}

async function invokeTauri<T>(cmd: string, args: Record<string, any> = {}): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return await invoke<T>(cmd, args);
}

export interface ReaderPageSnapshot {
  url: string;
  finalUrl: string;
  title: string;
  excerpt: string;
  content: string;
  status: number;
}

// Onglets pour lesquels une extraction lecteur est déjà en cours
// (évite les doubles appels du $: réactif Svelte).
const readerFetchInflight = new Set<string>();

function setTabReaderState(
  targetTabId: string,
  patch: Partial<Pick<BrowserTab, "extractedContent" | "loading" | "title">>
): void {
  browserTabs.update((tabs) =>
    tabs.map((tab) => {
      if (tab.id !== targetTabId) return tab;
      const updated: BrowserTab = { ...tab, ...patch };
      if (tab.id === get(activeTabId)) {
        internalActiveTab.set(updated);
      }
      return updated;
    })
  );
}

/**
 * Récupère le contenu texte réel d'une page via le backend Rust
 * (`browser_fetch_page_text`) et le stocke dans `extractedContent`
 * pour le mode lecteur. Utilisé quand un site refuse l'iframe
 * (X-Frame-Options / CSP) : le panneau affiche du vrai contenu
 * au lieu d'un cadre vide. No-op hors Tauri ou si déjà extrait.
 */
export async function fetchReaderContent(tabId?: string): Promise<void> {
  const targetTabId = tabId || get(activeTabId);
  const tabs = get(browserTabs);
  const tab = tabs.find((t) => t.id === targetTabId);
  if (!tab || !isTauri()) return;
  if (tab.extractedContent || readerFetchInflight.has(targetTabId)) return;
  if (!/^https?:\/\//i.test(tab.url)) return;
  readerFetchInflight.add(targetTabId);
  setTabReaderState(targetTabId, { loading: true });
  try {
    const snap = await invokeTauri<ReaderPageSnapshot>("browser_fetch_page_text", {
      url: tab.url,
      maxChars: 24000,
    });
    const body = (snap.content || snap.excerpt || "").trim();
    setTabReaderState(targetTabId, {
      loading: false,
      title: snap.title || tab.title,
      extractedContent: body
        ? `${snap.title ? `# ${snap.title}\n\n` : ""}${body}`
        : undefined,
    });
    if (!body) readerFetchInflight.delete(targetTabId);
  } catch (err) {
    console.warn("Reader content fetch failed for", tab.url, err);
    setTabReaderState(targetTabId, { loading: false });
    readerFetchInflight.delete(targetTabId);
  }
}
