import { describe, expect, it, vi } from "vitest";
import {
  DEFAULT_SHORTCUTS,
  SHORTCUT_STORAGE_KEYS,
  buildCommandPaletteItems,
  filterCommandPaletteItems,
  formatShortcut,
  loadShortcuts,
  matchShortcutEvent,
  shortcutFromKeyboardEvent,
  updateShortcut,
  type CommandPaletteActions,
  type ShortcutKeyboardEvent,
} from "./model";

function keyboard(key: string, modifiers: Partial<ShortcutKeyboardEvent> = {}): ShortcutKeyboardEvent {
  return { key, ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, ...modifiers };
}

function actions(): CommandPaletteActions {
  return {
    close: vi.fn(),
    startConversation: vi.fn(),
    openSettings: vi.fn(),
    applyTheme: vi.fn(),
    toggleVoiceRecording: vi.fn(),
    clearMemory: vi.fn(),
    selectMode: vi.fn(),
    openConversation: vi.fn(),
  };
}

describe("shortcut model", () => {
  it("preserves defaults, exact storage keys, and ignores empty persisted values", () => {
    const saved = new Map([
      ["aro-shortcut-spotlight", "Meta+p"],
      ["aro-shortcut-voice", ""],
      ["aro-shortcut-settings", "Alt+s"],
    ]);
    const storage = {
      getItem: (key: string) => saved.get(key) ?? null,
      setItem: vi.fn(),
    };

    expect(DEFAULT_SHORTCUTS).toEqual({
      spotlight: "Control+k", voice: "Control+Shift+v", settings: "Control+,", newChat: "Control+n",
    });
    expect(SHORTCUT_STORAGE_KEYS).toEqual({
      spotlight: "aro-shortcut-spotlight", voice: "aro-shortcut-voice",
      settings: "aro-shortcut-settings", newChat: "aro-shortcut-new-chat",
    });
    expect(loadShortcuts(storage)).toEqual({
      spotlight: "Meta+p", voice: "Control+Shift+v", settings: "Alt+s", newChat: "Control+n",
    });

    const updated = updateShortcut(DEFAULT_SHORTCUTS, "newChat", "Meta+N", storage);
    expect(updated).toEqual({ ...DEFAULT_SHORTCUTS, newChat: "Meta+N" });
    expect(storage.setItem).toHaveBeenCalledWith("aro-shortcut-new-chat", "Meta+N");
    expect(DEFAULT_SHORTCUTS.newChat).toBe("Control+n");
  });

  it("formats, records, and matches shortcuts with the existing semantics", () => {
    expect(formatShortcut("Control+Shift+v")).toBe("Ctrl + Shift + v");
    expect(formatShortcut("Control+Shift+v", true)).toBe("⌘ + ⇧ + v");
    expect(formatShortcut("Meta+Alt+ ", true)).toBe("Cmd + ⌥ +  ");
    expect(shortcutFromKeyboardEvent(keyboard(" ", { ctrlKey: true }))).toBe("Control+Space");
    expect(shortcutFromKeyboardEvent(keyboard("a", { ctrlKey: true, shiftKey: true }))).toBe("Control+Shift+A");
    expect(shortcutFromKeyboardEvent(keyboard("Control", { ctrlKey: true }))).toBeNull();
    expect(matchShortcutEvent(keyboard("K", { ctrlKey: true }), "Control+k")).toBe(true);
    expect(matchShortcutEvent(keyboard(" ", { metaKey: true }), "Meta+Space")).toBe(true);
    expect(matchShortcutEvent(keyboard("k", { ctrlKey: true, shiftKey: true }), "Control+k")).toBe(false);
  });
});

describe("command palette model", () => {
  it("preserves item order, French labels, shortcuts, and action sequencing", () => {
    const callbacks = actions();
    const conversation = { id: "c1", title: "", mode: "think" };
    const items = buildCommandPaletteItems({
      language: "fr", theme: "dark", shortcuts: DEFAULT_SHORTCUTS,
      conversations: [conversation], actions: callbacks,
    });

    expect(items.map((item) => item.id)).toEqual([
      "new-chat", "new-project", "new-folder", "export-md", "export-json",
      "open-settings", "toggle-theme", "toggle-voice", "reset-memory",
      "mode-chat", "mode-think", "mode-code", "mode-quiet", "conv-c1",
    ]);
    expect(items[0]).toMatchObject({
      title: "Nouvelle discussion", subtitle: "Démarrer une nouvelle discussion",
      category: "Actions", icon: "chat", shortcut: "Control+n",
    });
    expect(items[6].subtitle).toBe("Passer au thème clair");
    expect(items[12]).toMatchObject({
      title: "Activer le mode Quiet / Bref", subtitle: "Réponses minimales et concises",
      category: "Assistant Modes",
    });
    expect(items[13]).toMatchObject({
      title: "Discussion sans titre", subtitle: "Discussion · Mode think", category: "Discussions récentes",
    });

    items[6].action();
    expect(callbacks.close).toHaveBeenCalledBefore(callbacks.applyTheme as ReturnType<typeof vi.fn>);
    expect(callbacks.applyTheme).toHaveBeenCalledWith("light");
    items[11].action();
    expect(callbacks.selectMode).toHaveBeenCalledWith("code");
    items[13].action();
    expect(callbacks.openConversation).toHaveBeenCalledWith(conversation);
  });

  it("preserves English labels and filters trimmed, case-insensitive title, subtitle, or category", () => {
    const items = buildCommandPaletteItems({
      language: "en", theme: "light", shortcuts: DEFAULT_SHORTCUTS,
      conversations: [{ id: "c1", title: "Release Plan", mode: "chat" }], actions: actions(),
    });
    expect(items[5]).toMatchObject({ title: "Settings", subtitle: "Open settings panel" });
    expect(items[6].subtitle).toBe("Switch to dark theme");
    expect(items[13]).toMatchObject({ subtitle: "Conversation · Mode chat", category: "Recent Chats" });
    expect(filterCommandPaletteItems(items, "  RELEASE ").map((item) => item.id)).toEqual(["conv-c1"]);
    expect(filterCommandPaletteItems(items, "local history").map((item) => item.id)).toEqual(["reset-memory"]);
    expect(filterCommandPaletteItems(items, "assistant modes").map((item) => item.id)).toEqual([
      "mode-chat", "mode-think", "mode-code", "mode-quiet",
    ]);
    expect(filterCommandPaletteItems(items, "   ")).toBe(items);
  });
});
