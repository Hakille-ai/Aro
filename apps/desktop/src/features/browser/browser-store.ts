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
  loadFromStorage<BrowserCredential[]>(STORAGE_KEY_CREDENTIALS, [
    {
      id: "cred-demo-1",
      domain: "github.com",
      username: "developer@aro.internal",
      password: "••••••••••••",
      createdAt: new Date().toISOString(),
      lastUsedAt: new Date().toISOString(),
    },
  ])
);

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
  const newCred: BrowserCredential = {
    ...cred,
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

function extractDomain(url: string): string {
  try {
    const parsed = new URL(url);
    return parsed.hostname.replace(/^www\./, "");
  } catch {
    return url;
  }
}
