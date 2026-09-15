export type SettingsTab =
  | "general"
  | "models"
  | "system-prompt"
  | "voice"
  | "paths"
  | "system"
  | "preferences"
  | "monitoring"
  | "profile"
  | "organization"
  | "instructions"
  | "memory"
  | "skills"
  | "plugins"
  | "mcp"
  | "hooks"
  | "scheduler"
  | "agents"
  | "search"
  | "permissions"
  | "shortcuts";

export type ShortcutKey = "spotlight" | "voice" | "settings" | "newChat";
export type ShortcutMap = Record<ShortcutKey, string>;
