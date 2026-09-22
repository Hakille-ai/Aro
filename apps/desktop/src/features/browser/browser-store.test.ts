import { describe, expect, it, beforeEach } from "vitest";
import { get } from "svelte/store";
import {
  browserHistory,
  browserCredentials,
  browserPermissions,
  activeBrowserTab,
  browserTabs,
  activeTabId,
  createBrowserTab,
  switchBrowserTab,
  closeBrowserTab,
  takeBrowserControl,
  toggleReaderMode,
  navigateBrowser,
  browserGoBack,
  browserGoForward,
  browserReload,
  addBrowserHistoryEntry,
  clearBrowserHistory,
  deleteBrowserHistoryEntry,
  addBrowserCredential,
  updateBrowserCredential,
  deleteBrowserCredential,
  updateBrowserPermissions,
} from "./browser-store";

describe("browser-store", () => {
  beforeEach(() => {
    clearBrowserHistory();
    browserCredentials.set([]);
    browserPermissions.set({
      allowAiBrowsing: true,
      allowAiInteraction: true,
      allowComputerUse: false,
      confirmSensitiveActions: true,
      defaultSearchEngine: "duckduckgo",
      javascriptEnabled: true,
      blockThirdPartyCookies: true,
      userAgent: "AroBrowser/1.0",
    });
    activeBrowserTab.set({
      id: "tab-test",
      url: "https://duckduckgo.com",
      title: "DuckDuckGo",
      loading: false,
      canGoBack: false,
      canGoForward: false,
      history: ["https://duckduckgo.com"],
      historyIndex: 0,
      isAiControlled: false,
    });
  });

  it("handles URL navigation and history tracking", () => {
    navigateBrowser("https://example.com");
    const tab = get(activeBrowserTab);
    expect(tab.url).toBe("https://example.com");
    expect(tab.canGoBack).toBe(true);
    expect(tab.history).toContain("https://example.com");

    const history = get(browserHistory);
    expect(history.length).toBeGreaterThan(0);
    expect(history[0].url).toBe("https://example.com");
  });

  it("formats search queries when non-URL string is entered", () => {
    navigateBrowser("how does quantum computing work");
    const tab = get(activeBrowserTab);
    expect(tab.url).toContain("https://duckduckgo.com/?q=");
    expect(tab.url).toContain("quantum");
  });

  it("supports back and forward navigation", () => {
    navigateBrowser("https://first.com");
    navigateBrowser("https://second.com");
    expect(get(activeBrowserTab).url).toBe("https://second.com");

    browserGoBack();
    expect(get(activeBrowserTab).url).toBe("https://first.com");
    expect(get(activeBrowserTab).canGoForward).toBe(true);

    browserGoForward();
    expect(get(activeBrowserTab).url).toBe("https://second.com");
  });

  it("manages credentials vault entries", () => {
    addBrowserCredential({
      domain: "github.com",
      username: "testuser",
      password: "secretpassword123",
    });

    const creds = get(browserCredentials);
    expect(creds.length).toBe(1);
    expect(creds[0].domain).toBe("github.com");
    expect(creds[0].username).toBe("testuser");

    updateBrowserCredential(creds[0].id, { username: "updateduser" });
    expect(get(browserCredentials)[0].username).toBe("updateduser");

    deleteBrowserCredential(creds[0].id);
    expect(get(browserCredentials).length).toBe(0);
  });

  it("toggles computer use and browser permissions safely", () => {
    expect(get(browserPermissions).allowComputerUse).toBe(false);

    updateBrowserPermissions({ allowComputerUse: true });
    expect(get(browserPermissions).allowComputerUse).toBe(true);

    updateBrowserPermissions({ defaultSearchEngine: "google" });
    expect(get(browserPermissions).defaultSearchEngine).toBe("google");
  });

  it("manages browsing history entries and clearing", () => {
    addBrowserHistoryEntry("https://page1.org", "Page 1");
    addBrowserHistoryEntry("https://page2.org", "Page 2");
    expect(get(browserHistory).length).toBe(2);

    deleteBrowserHistoryEntry(get(browserHistory)[0].id);
    expect(get(browserHistory).length).toBe(1);

    clearBrowserHistory();
    expect(get(browserHistory).length).toBe(0);
  });

  it("supports multi-tab creation, switching, and closing", () => {
    const initialTabs = get(browserTabs);
    expect(initialTabs.length).toBeGreaterThanOrEqual(1);

    const newTabId = createBrowserTab("https://news.ycombinator.com", "Hacker News");
    expect(get(browserTabs).length).toBe(initialTabs.length + 1);
    expect(get(activeTabId)).toBe(newTabId);
    expect(get(activeBrowserTab).url).toBe("https://news.ycombinator.com");

    const firstTabId = initialTabs[0].id;
    switchBrowserTab(firstTabId);
    expect(get(activeTabId)).toBe(firstTabId);
    expect(get(activeBrowserTab).id).toBe(firstTabId);

    // Close the new tab
    closeBrowserTab(newTabId);
    expect(get(browserTabs).some((t) => t.id === newTabId)).toBe(false);
  });

  it("handles closing the active tab by switching to adjacent tab", () => {
    const tab1 = createBrowserTab("https://site1.org", "Site 1");
    const tab2 = createBrowserTab("https://site2.org", "Site 2");
    expect(get(activeTabId)).toBe(tab2);

    closeBrowserTab(tab2);
    expect(get(activeTabId)).not.toBe(tab2);
    expect(get(browserTabs).some((t) => t.id === tab2)).toBe(false);
  });

  it("handles closing all tabs by automatically opening a fresh default tab", () => {
    const allTabIds = get(browserTabs).map((t) => t.id);
    for (const id of allTabIds) {
      closeBrowserTab(id);
    }
    const currentTabs = get(browserTabs);
    expect(currentTabs.length).toBe(1);
    for (const oldId of allTabIds) {
      expect(currentTabs.some((t) => t.id === oldId)).toBe(false);
    }
    expect(get(activeBrowserTab)).toBeDefined();
    expect(get(activeBrowserTab).id).toBe(currentTabs[0].id);
  });

  it("allows user to take browser control from autonomous agent", () => {
    const tabId = createBrowserTab("https://ai-target.org", "AI Target", true, "AI is navigating");
    expect(get(activeBrowserTab).isAiControlled).toBe(true);
    expect(get(activeBrowserTab).aiStatusMessage).toBe("AI is navigating");

    takeBrowserControl(tabId);
    const updated = get(browserTabs).find((t) => t.id === tabId);
    expect(updated?.isAiControlled).toBe(false);
    expect(updated?.aiStatusMessage).toBeUndefined();
    expect(get(activeBrowserTab).isAiControlled).toBe(false);
  });

  it("toggles reader mode for content extraction", () => {
    expect(get(activeBrowserTab).readerMode).toBeFalsy();

    toggleReaderMode();
    expect(get(activeBrowserTab).readerMode).toBe(true);

    toggleReaderMode(undefined, false);
    expect(get(activeBrowserTab).readerMode).toBe(false);
  });
});
