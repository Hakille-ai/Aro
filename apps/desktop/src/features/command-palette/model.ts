export type ShortcutKey = "spotlight" | "voice" | "settings" | "newChat";

export type Shortcuts = Record<ShortcutKey, string>;

export const DEFAULT_SHORTCUTS: Shortcuts = {
  spotlight: "Control+k",
  voice: "Control+Shift+v",
  settings: "Control+,",
  newChat: "Control+n",
};

export const SHORTCUT_STORAGE_KEYS: Record<ShortcutKey, string> = {
  spotlight: "aro-shortcut-spotlight",
  voice: "aro-shortcut-voice",
  settings: "aro-shortcut-settings",
  newChat: "aro-shortcut-new-chat",
};

export type ShortcutStorage = Pick<Storage, "getItem" | "setItem">;

export function loadShortcuts(storage?: ShortcutStorage): Shortcuts {
  const shortcuts = { ...DEFAULT_SHORTCUTS };
  if (!storage) return shortcuts;

  for (const key of Object.keys(SHORTCUT_STORAGE_KEYS) as ShortcutKey[]) {
    const saved = storage.getItem(SHORTCUT_STORAGE_KEYS[key]);
    if (saved) shortcuts[key] = saved;
  }
  return shortcuts;
}

export function updateShortcut(
  shortcuts: Shortcuts,
  key: ShortcutKey,
  value: string,
  storage?: ShortcutStorage,
): Shortcuts {
  const updated = { ...shortcuts, [key]: value };
  storage?.setItem(SHORTCUT_STORAGE_KEYS[key], value);
  return updated;
}

export type ShortcutKeyboardEvent = Pick<
  KeyboardEvent,
  "key" | "ctrlKey" | "metaKey" | "altKey" | "shiftKey"
>;

export function formatShortcut(shortcut: string, isMac = false): string {
  if (!shortcut) return "";
  return shortcut
    .split("+")
    .map((part) => {
      if (part === "Control") return isMac ? "⌘" : "Ctrl";
      if (part === "Meta" || part === "Cmd") return "Cmd";
      if (part === "Shift") return isMac ? "⇧" : "Shift";
      if (part === "Alt") return isMac ? "⌥" : "Alt";
      return part;
    })
    .join(" + ");
}

export function hasModifiers(event: ShortcutKeyboardEvent): boolean {
  return event.ctrlKey || event.metaKey || event.altKey || event.shiftKey;
}

export function matchShortcutEvent(event: ShortcutKeyboardEvent, shortcut: string): boolean {
  if (!shortcut) return false;
  const parts = shortcut.split("+");
  const ctrlRequired = parts.includes("Ctrl") || parts.includes("Control");
  const cmdRequired = parts.includes("Cmd") || parts.includes("Meta");
  const altRequired = parts.includes("Alt");
  const shiftRequired = parts.includes("Shift");

  if (ctrlRequired !== event.ctrlKey) return false;
  if (cmdRequired !== event.metaKey) return false;
  if (altRequired !== event.altKey) return false;
  if (shiftRequired !== event.shiftKey) return false;

  const keyPart = parts[parts.length - 1];
  if (!keyPart) return false;
  const eventKey = event.key === " " ? "Space" : event.key;
  return eventKey.toLowerCase() === keyPart.toLowerCase();
}

const MODIFIER_KEYS = ["Control", "Meta", "Alt", "Shift"];

export function shortcutFromKeyboardEvent(event: ShortcutKeyboardEvent): string | null {
  const keys: string[] = [];
  if (event.ctrlKey) keys.push("Control");
  if (event.metaKey) keys.push("Meta");
  if (event.altKey) keys.push("Alt");
  if (event.shiftKey) keys.push("Shift");

  if (!MODIFIER_KEYS.includes(event.key)) {
    if (event.key === " ") keys.push("Space");
    else if (event.key.length === 1) keys.push(event.key.toUpperCase());
    else keys.push(event.key);
  }

  return keys.length > 0 && !MODIFIER_KEYS.includes(event.key) ? keys.join("+") : null;
}

export type CommandPaletteLanguage = "fr" | "en";
export type CommandPaletteTheme = "light" | "dark";
export type CommandPaletteMode = "chat" | "think" | "code" | "quiet";

export interface CommandPaletteConversation {
  id: string;
  title: string;
  mode: string;
}

export interface CommandPaletteItem {
  id: string;
  title: string;
  subtitle?: string;
  category: string;
  icon: string;
  action: () => void;
  shortcut?: string;
}

export interface CommandPaletteProject {
  id: string;
  name: string;
  color?: string | null;
  icon?: string | null;
}

export interface CommandPaletteFolder {
  id: string;
  name: string;
  color?: string | null;
  icon?: string | null;
}

export interface CommandPaletteActions {
  close: () => void;
  startConversation: () => void;
  openSettings: () => void;
  applyTheme: (theme: CommandPaletteTheme) => void;
  toggleVoiceRecording: () => void;
  clearMemory: () => void;
  selectMode: (mode: CommandPaletteMode) => void;
  openConversation: (conversation: CommandPaletteConversation) => void;
  openProject?: (projectId: string) => void;
  openFolder?: (folderId: string) => void;
  createProject?: () => void;
  createFolder?: () => void;
  exportActiveMarkdown?: () => void;
  exportActiveJson?: () => void;
}

export interface BuildCommandPaletteOptions {
  language: CommandPaletteLanguage;
  theme: CommandPaletteTheme;
  shortcuts: Shortcuts;
  conversations: CommandPaletteConversation[];
  projects?: CommandPaletteProject[];
  folders?: CommandPaletteFolder[];
  actions: CommandPaletteActions;
}

const MODES: { mode: CommandPaletteMode; label: string; desc: string }[] = [
  { mode: "chat", label: "Chat", desc: "Mode conversation standard" },
  { mode: "think", label: "Think", desc: "Mode raisonnement approfondi" },
  { mode: "code", label: "Code", desc: "Mode développement et algorithmes" },
  { mode: "quiet", label: "Quiet / Bref", desc: "Réponses minimales et concises" },
];

export function buildCommandPaletteItems(options: BuildCommandPaletteOptions): CommandPaletteItem[] {
  const { language, theme, shortcuts, conversations, projects = [], folders = [], actions } = options;
  const items: CommandPaletteItem[] = [
    {
      id: "new-chat",
      title: language === "fr" ? "Nouvelle discussion" : "New Chat",
      subtitle: language === "fr" ? "Démarrer une nouvelle discussion" : "Start a new conversation",
      category: "Actions",
      icon: "chat",
      shortcut: shortcuts.newChat,
      action: () => { actions.close(); actions.startConversation(); },
    },
    {
      id: "new-project",
      title: language === "fr" ? "Nouveau Projet" : "New Project",
      subtitle: language === "fr" ? "Créer un nouveau projet d'organisation" : "Create a new project workspace",
      category: "Actions",
      icon: "project",
      action: () => { actions.close(); actions.createProject?.(); },
    },
    {
      id: "new-folder",
      title: language === "fr" ? "Nouveau Dossier" : "New Folder",
      subtitle: language === "fr" ? "Créer un nouveau dossier" : "Create a new folder",
      category: "Actions",
      icon: "folder",
      action: () => { actions.close(); actions.createFolder?.(); },
    },
    {
      id: "export-md",
      title: language === "fr" ? "Exporter la discussion (Markdown)" : "Export Chat (Markdown)",
      subtitle: language === "fr" ? "Télécharger la conversation active au format .md" : "Download active chat as .md",
      category: "Actions",
      icon: "export",
      action: () => { actions.close(); actions.exportActiveMarkdown?.(); },
    },
    {
      id: "export-json",
      title: language === "fr" ? "Exporter la discussion (JSON)" : "Export Chat (JSON)",
      subtitle: language === "fr" ? "Télécharger la conversation au format .json" : "Download chat as .json",
      category: "Actions",
      icon: "export",
      action: () => { actions.close(); actions.exportActiveJson?.(); },
    },
    {
      id: "open-settings",
      title: language === "fr" ? "Paramètres" : "Settings",
      subtitle: language === "fr" ? "Ouvrir le panneau de configuration" : "Open settings panel",
      category: "Actions",
      icon: "settings",
      shortcut: shortcuts.settings,
      action: () => { actions.close(); actions.openSettings(); },
    },
    {
      id: "toggle-theme",
      title: language === "fr" ? "Basculer le thème" : "Toggle Theme",
      subtitle: language === "fr"
        ? `Passer au thème ${theme === "dark" ? "clair" : "sombre"}`
        : `Switch to ${theme === "dark" ? "light" : "dark"} theme`,
      category: "Actions",
      icon: "theme",
      action: () => { actions.close(); actions.applyTheme(theme === "dark" ? "light" : "dark"); },
    },
    {
      id: "toggle-voice",
      title: language === "fr" ? "Mode Voix" : "Voice Mode",
      subtitle: language === "fr" ? "Activer ou désactiver la dictée vocale" : "Toggle voice mode",
      category: "Actions",
      icon: "voice",
      shortcut: shortcuts.voice,
      action: () => { actions.close(); actions.toggleVoiceRecording(); },
    },
    {
      id: "reset-memory",
      title: language === "fr" ? "Réinitialiser la mémoire" : "Reset Memory",
      subtitle: language === "fr" ? "Effacer l'historique et la mémoire locale" : "Clear local history and memory",
      category: "Actions",
      icon: "trash",
      action: () => { actions.close(); actions.clearMemory(); },
    },
  ];

  for (const proj of projects) {
    items.push({
      id: `proj-${proj.id}`,
      title: proj.name,
      subtitle: language === "fr" ? "Ouvrir la discussion récente du projet" : "Open the project's recent chat",
      category: language === "fr" ? "Projets" : "Projects",
      icon: "project",
      action: () => { actions.close(); actions.openProject?.(proj.id); },
    });
  }

  for (const fold of folders) {
    items.push({
      id: `fold-${fold.id}`,
      title: fold.name,
      subtitle: language === "fr" ? "Ouvrir la discussion récente du dossier" : "Open the folder's recent chat",
      category: language === "fr" ? "Dossiers" : "Folders",
      icon: "folder",
      action: () => { actions.close(); actions.openFolder?.(fold.id); },
    });
  }

  for (const item of MODES) {
    items.push({
      id: `mode-${item.mode}`,
      title: `${language === "fr" ? "Activer le mode" : "Enable mode"} ${item.label}`,
      subtitle: item.desc,
      category: "Assistant Modes",
      icon: "mode",
      action: () => { actions.close(); actions.selectMode(item.mode); },
    });
  }

  for (const conversation of conversations) {
    items.push({
      id: `conv-${conversation.id}`,
      title: conversation.title || (language === "fr" ? "Discussion sans titre" : "Untitled Conversation"),
      subtitle: language === "fr"
        ? `Discussion · Mode ${conversation.mode}`
        : `Conversation · Mode ${conversation.mode}`,
      category: language === "fr" ? "Discussions récentes" : "Recent Chats",
      icon: "chat-item",
      action: () => { actions.close(); actions.openConversation(conversation); },
    });
  }

  return items;
}

export function filterCommandPaletteItems(
  items: CommandPaletteItem[],
  search: string,
): CommandPaletteItem[] {
  const query = search.toLowerCase().trim();
  if (!query) return items;
  return items.filter((item) =>
    item.title.toLowerCase().includes(query)
    || Boolean(item.subtitle?.toLowerCase().includes(query))
    || item.category.toLowerCase().includes(query),
  );
}

export function buildCommandPaletteResults(
  options: BuildCommandPaletteOptions,
  search: string,
): CommandPaletteItem[] {
  return filterCommandPaletteItems(buildCommandPaletteItems(options), search);
}
