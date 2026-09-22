export type SettingsTab =
  | "general"
  | "models"
  | "system-prompt"
  | "voice"
  | "paths"
  | "system"
  | "preferences"
  | "notifications"
  | "monitoring"
  | "profile"
  | "billing"
  | "organization"
  | "instructions"
  | "memory"
  | "skills"
  | "plugins"
  | "mcp"
  | "hooks"
  | "scheduler"
  | "agents"
  | "browser"
  | "computer"
  | "search"
  | "permissions"
  | "shortcuts";

export type ShortcutKey = "spotlight" | "voice" | "settings" | "newChat";
export type ShortcutMap = Record<ShortcutKey, string>;
